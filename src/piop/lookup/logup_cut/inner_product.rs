use crate::{
    piop::lookup::gkr_product::absorb_field_slice,
    poly::univariate::binary_gf128::Gf128 as Gf,
    transcript::traits::Transcript,
    utils::wide_mul::WideMulAcc,
};

#[cfg(feature = "parallel")]
use rayon::prelude::*;

const DOMAIN: &[u8] = b"bitz/logup-cut/inner-product/v1";
#[cfg(feature = "parallel")]
const PARALLEL_THRESHOLD: usize = 1 << 13;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EvalClaim {
    pub point: Vec<Gf>,
    pub value: Gf,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InnerProductProof {
    messages: Vec<[Gf; 2]>,
    evaluation: Gf,
}

impl InnerProductProof {
    pub(crate) fn proof_size_bytes(&self) -> usize {
        (2 * self.messages.len() + 1) * 16
    }
}

pub fn prove_inner_product(
    transcript: &mut impl Transcript,
    committed: &[Gf],
    public: &[Gf],
    mut claim: Gf,
) -> (InnerProductProof, EvalClaim) {
    assert_eq!(committed.len(), public.len());
    assert!(!committed.is_empty() && committed.len().is_power_of_two());
    transcript.absorb_slice(DOMAIN);
    transcript.absorb_slice(&(committed.len().ilog2() as u64).to_le_bytes());

    let mut committed = committed.to_vec();
    let mut public = public.to_vec();
    let mut len = committed.len();
    let mut point = Vec::with_capacity(len.ilog2() as usize);
    let mut messages = Vec::with_capacity(point.capacity());
    while len > 1 {
        let [constant, linear] = round(&committed[..len], &public[..len]);
        messages.push([constant, linear]);
        absorb_field_slice(transcript, &[constant, linear]);
        let challenge = transcript.get_field_challenge::<Gf>(&());
        let quadratic = claim + linear;
        claim = constant + challenge * (linear + challenge * quadratic);
        fold(&mut committed[..len], challenge);
        fold(&mut public[..len], challenge);
        len >>= 1;
        point.push(challenge);
    }
    let evaluation = committed[0];
    absorb_field_slice(transcript, &[evaluation]);
    debug_assert_eq!(claim, evaluation * public[0]);
    (
        InnerProductProof {
            messages,
            evaluation,
        },
        EvalClaim {
            point,
            value: evaluation,
        },
    )
}

pub fn verify_inner_product(
    transcript: &mut impl Transcript,
    proof: &InnerProductProof,
    dimension: usize,
    mut claim: Gf,
    public_eval: impl FnOnce(&[Gf]) -> Gf,
) -> Option<EvalClaim> {
    if proof.messages.len() != dimension {
        return None;
    }
    transcript.absorb_slice(DOMAIN);
    transcript.absorb_slice(&(dimension as u64).to_le_bytes());
    let mut point = Vec::with_capacity(dimension);
    for &[constant, linear] in &proof.messages {
        absorb_field_slice(transcript, &[constant, linear]);
        let quadratic = claim + linear;
        let challenge = transcript.get_field_challenge::<Gf>(&());
        claim = constant + challenge * (linear + challenge * quadratic);
        point.push(challenge);
    }
    absorb_field_slice(transcript, &[proof.evaluation]);
    if claim != proof.evaluation * public_eval(&point) {
        return None;
    }
    Some(EvalClaim {
        point,
        value: proof.evaluation,
    })
}

fn round(a: &[Gf], b: &[Gf]) -> [Gf; 2] {
    debug_assert_eq!(a.len(), b.len());
    debug_assert_eq!(a.len() & 1, 0);
    #[cfg(feature = "parallel")]
    const CHUNK: usize = 1 << 12;
    let partial = |(a, b): (&[Gf], &[Gf])| {
        let mut constant = <Gf as WideMulAcc>::wide_zero(&Gf::ZERO);
        let mut linear = <Gf as WideMulAcc>::wide_zero(&Gf::ZERO);
        for (a, b) in a.chunks_exact(2).zip(b.chunks_exact(2)) {
            <Gf as WideMulAcc>::wide_add_assign(
                &mut constant,
                &<Gf as WideMulAcc>::mul_wide(&a[0], &b[0]),
            );
            <Gf as WideMulAcc>::wide_add_assign(
                &mut linear,
                &<Gf as WideMulAcc>::mul_wide(&a[0], &b[1]),
            );
            <Gf as WideMulAcc>::wide_add_assign(
                &mut linear,
                &<Gf as WideMulAcc>::mul_wide(&a[1], &b[0]),
            );
        }
        [
            <Gf as WideMulAcc>::from_wide(constant),
            <Gf as WideMulAcc>::from_wide(linear),
        ]
    };
    #[cfg(feature = "parallel")]
    if a.len() >= PARALLEL_THRESHOLD {
        a.par_chunks(CHUNK)
            .zip(b.par_chunks(CHUNK))
            .map(partial)
            .reduce(
                || [Gf::ZERO, Gf::ZERO],
                |left, right| [left[0] + right[0], left[1] + right[1]],
            )
    } else {
        partial((a, b))
    }
    #[cfg(not(feature = "parallel"))]
    partial((a, b))
}

fn fold(values: &mut [Gf], challenge: Gf) {
    let half = values.len() / 2;
    if <Gf as WideMulAcc>::eqf_fold_in_place(values, &challenge, half) {
        return;
    }
    for index in 0..half {
        let low = values[2 * index];
        values[index] = low + challenge * (low + values[2 * index + 1]);
    }
}

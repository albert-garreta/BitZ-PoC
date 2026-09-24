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

#[derive(Default)]
pub struct InnerProductWorkspace {
    committed: Vec<Gf>,
    public: Vec<Gf>,
    committed_back: Vec<Gf>,
    public_back: Vec<Gf>,
}

impl InnerProductWorkspace {
    pub fn reserve(&mut self, len: usize) {
        assert!(len.is_power_of_two());
        self.committed.resize(len, Gf::ZERO);
        self.public.resize(len, Gf::ZERO);
        self.committed_back.resize(len, Gf::ZERO);
        self.public_back.resize(len, Gf::ZERO);
    }

    pub fn retained_bytes(&self) -> usize {
        (self.committed.capacity()
            + self.public.capacity()
            + self.committed_back.capacity()
            + self.public_back.capacity())
            * core::mem::size_of::<Gf>()
    }
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
    workspace: &mut InnerProductWorkspace,
) -> (InnerProductProof, EvalClaim) {
    assert_eq!(committed.len(), public.len());
    assert!(!committed.is_empty() && committed.len().is_power_of_two());
    transcript.absorb_slice(DOMAIN);
    transcript.absorb_slice(&(committed.len().ilog2() as u64).to_le_bytes());

    let max_len = committed.len();
    assert!(workspace.committed.len() >= max_len);
    assert!(workspace.public.len() >= max_len);
    assert!(workspace.committed_back.len() >= max_len);
    assert!(workspace.public_back.len() >= max_len);
    workspace.committed[..max_len].copy_from_slice(committed);
    workspace.public[..max_len].copy_from_slice(public);
    let mut len = max_len;
    let mut point = Vec::with_capacity(len.ilog2() as usize);
    let mut messages = Vec::with_capacity(point.capacity());
    let mut prefetched = None;
    while len > 1 {
        let [constant, quadratic] = prefetched
            .take()
            .unwrap_or_else(|| round(&workspace.committed[..len], &workspace.public[..len]));
        let linear = claim + quadratic;
        messages.push([constant, linear]);
        absorb_field_slice(transcript, &[constant, linear]);
        let challenge = transcript.get_field_challenge::<Gf>(&());
        claim = constant + challenge * (linear + challenge * quadratic);
        let next_len = len / 2;
        if next_len > 1 {
            prefetched = Some(fold_and_round(
                &workspace.committed[..len],
                &workspace.public[..len],
                &mut workspace.committed_back[..next_len],
                &mut workspace.public_back[..next_len],
                challenge,
            ));
        } else {
            fold_prepared(
                &workspace.committed[..len],
                &workspace.public[..len],
                &mut workspace.committed_back[..next_len],
                &mut workspace.public_back[..next_len],
                challenge,
            );
        }
        core::mem::swap(&mut workspace.committed, &mut workspace.committed_back);
        core::mem::swap(&mut workspace.public, &mut workspace.public_back);
        len = next_len;
        point.push(challenge);
    }
    let evaluation = workspace.committed[0];
    absorb_field_slice(transcript, &[evaluation]);
    debug_assert_eq!(claim, evaluation * workspace.public[0]);
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
        let mut quadratic = <Gf as WideMulAcc>::wide_zero(&Gf::ZERO);
        for (a, b) in a.chunks_exact(2).zip(b.chunks_exact(2)) {
            <Gf as WideMulAcc>::wide_add_assign(
                &mut constant,
                &<Gf as WideMulAcc>::mul_wide(&a[0], &b[0]),
            );
            <Gf as WideMulAcc>::wide_add_assign(
                &mut quadratic,
                &<Gf as WideMulAcc>::mul_wide(&(a[0] + a[1]), &(b[0] + b[1])),
            );
        }
        [
            <Gf as WideMulAcc>::from_wide(constant),
            <Gf as WideMulAcc>::from_wide(quadratic),
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

fn fold_and_round(
    a: &[Gf],
    b: &[Gf],
    a_out: &mut [Gf],
    b_out: &mut [Gf],
    challenge: Gf,
) -> [Gf; 2] {
    debug_assert_eq!(a.len(), b.len());
    debug_assert_eq!(a_out.len(), a.len() / 2);
    debug_assert_eq!(a_out.len(), b_out.len());
    debug_assert_eq!(a_out.len() & 1, 0);
    let challenge = field::PreparedGf128Mul::new(challenge);
    let process = |(pair, (a_out, b_out)): (usize, (&mut [Gf], &mut [Gf]))| {
        let base = 4 * pair;
        let a0 = a[base] + challenge.mul(&(a[base] + a[base + 1]));
        let a1 = a[base + 2] + challenge.mul(&(a[base + 2] + a[base + 3]));
        let b0 = b[base] + challenge.mul(&(b[base] + b[base + 1]));
        let b1 = b[base + 2] + challenge.mul(&(b[base + 2] + b[base + 3]));
        a_out[0] = a0;
        a_out[1] = a1;
        b_out[0] = b0;
        b_out[1] = b1;
        [
            <Gf as WideMulAcc>::mul_wide(&a0, &b0),
            <Gf as WideMulAcc>::mul_wide(&(a0 + a1), &(b0 + b1)),
        ]
    };
    let zero = || [
        <Gf as WideMulAcc>::wide_zero(&Gf::ZERO),
        <Gf as WideMulAcc>::wide_zero(&Gf::ZERO),
    ];
    let merge = |
        mut left: [<Gf as WideMulAcc>::Wide; 2],
        right: [<Gf as WideMulAcc>::Wide; 2],
    | {
        <Gf as WideMulAcc>::wide_add_assign(&mut left[0], &right[0]);
        <Gf as WideMulAcc>::wide_add_assign(&mut left[1], &right[1]);
        left
    };
    #[cfg(feature = "parallel")]
    let sum = if a_out.len() >= PARALLEL_THRESHOLD {
        a_out
            .par_chunks_mut(2)
            .zip(b_out.par_chunks_mut(2))
            .enumerate()
            .map(process)
            .reduce(zero, merge)
    } else {
        a_out
            .chunks_mut(2)
            .zip(b_out.chunks_mut(2))
            .enumerate()
            .map(process)
            .fold(zero(), merge)
    };
    #[cfg(not(feature = "parallel"))]
    let sum = a_out
        .chunks_mut(2)
        .zip(b_out.chunks_mut(2))
        .enumerate()
        .map(process)
        .fold(zero(), merge);
    [
        <Gf as WideMulAcc>::from_wide(sum[0].clone()),
        <Gf as WideMulAcc>::from_wide(sum[1].clone()),
    ]
}

fn fold_prepared(
    a: &[Gf],
    b: &[Gf],
    a_out: &mut [Gf],
    b_out: &mut [Gf],
    challenge: Gf,
) {
    let challenge = field::PreparedGf128Mul::new(challenge);
    let fold = |index: usize, a_out: &mut Gf, b_out: &mut Gf| {
        let a0 = a[2 * index];
        let b0 = b[2 * index];
        *a_out = a0 + challenge.mul(&(a0 + a[2 * index + 1]));
        *b_out = b0 + challenge.mul(&(b0 + b[2 * index + 1]));
    };
    #[cfg(feature = "parallel")]
    if a_out.len() >= PARALLEL_THRESHOLD {
        a_out
            .par_iter_mut()
            .zip(b_out.par_iter_mut())
            .enumerate()
            .for_each(|(index, (a_out, b_out))| fold(index, a_out, b_out));
    } else {
        a_out
            .iter_mut()
            .zip(b_out)
            .enumerate()
            .for_each(|(index, (a_out, b_out))| fold(index, a_out, b_out));
    }
    #[cfg(not(feature = "parallel"))]
    a_out
        .iter_mut()
        .zip(b_out)
        .enumerate()
        .for_each(|(index, (a_out, b_out))| fold(index, a_out, b_out));
}

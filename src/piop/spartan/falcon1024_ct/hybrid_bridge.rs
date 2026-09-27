//! Prime-field source evaluation reduced through the optimized wfbitz forests.
//!
//! The two bounded limbs share a wfbitz forest with the limb coordinate left
//! unmultiplied. Its terminal claim is authenticated by the shared binary PCS.
use super::hybrid_keccak::grinding::{
    ProverBlockGrindingTranscript, VerifierBlockGrindingTranscript,
};
use super::{FalconError, FalconSourceWitness};
use crate::{
    hybrid::BinaryClaim,
    ligerito::{fold_values_bits_multi, pack_columns_from_rows},
    pcs::{IntegerMatrixLayout, chunk_row_weights, mod_q_chunk_width, mod_q_num_chunks},
    piop::spartan::{SpartanBitzField, SpartanField, grinding::GrindingDomain, matrix::eq_table},
    poly::{univariate::binary_gf128::Gf128 as Gf, utils::build_eq_x_r_vec},
    transcript::traits::Transcript,
    bitz::{
        FixedBasePow, WINDOW,
        forest::Forest,
        gkr::{GkrProverTranscript, GkrVerifierTranscript, gpgkr_verify},
    },
};
use field::Uint;
#[cfg(feature = "parallel")]
use rayon::prelude::*;

type F = SpartanBitzField;
type Cfg = <F as SpartanField>::Config;

/// One arity-2 forest: root projection degree s, degree-three
/// sumcheck rounds, and one linear child fold per layer. See the bridge audit.
pub(super) fn error_numerator(layout: &super::FalconSourceLayout) -> usize {
    let p = layout.bitz_params();
    let d = p.row_vars;
    let s = p.col_vars + 1;
    s + 3 * (d * (d - 1) / 2 + d * s) + d
}

fn message_count(p: &IntegerMatrixLayout) -> usize {
    p.row_vars * (p.row_vars - 1) / 2 + p.row_vars * (p.col_vars + 1) + p.row_vars
}

fn binary_claim(p: &IntegerMatrixLayout, images: &[Gf], point: &[Gf], value: Gf) -> BinaryClaim {
    // The lowest geometric row bit is the unmultiplied limb coordinate.
    // wfbitz returns [column | limb | original row]; Falcon uses [row | column].
    let (column_point, tail) = point.split_at(p.col_vars);
    let (limb, row_point) = tail.split_first().expect("limb coordinate");
    let low = build_eq_x_r_vec(row_point, &())
        .expect("nonempty row point")
        .into_iter()
        .zip(images.chunks_exact(2))
        .map(|(weight, pair)| {
            let factor =
                (Gf::one() + *limb) * (pair[0] + Gf::one()) + *limb * (pair[1] + Gf::one());
            weight * factor
        })
        .collect();
    BinaryClaim {
        low,
        high_point: column_point.to_vec(),
        value: value + Gf::one(),
    }
}

struct BridgeGrinding;
impl GrindingDomain for BridgeGrinding {
    const DOMAIN: &'static [u8] = b"bitz/falcon-hybrid/bridge-grinding/v4";
}

#[derive(Clone, Debug)]
pub(super) struct Proof {
    pub sums: Vec<Vec<u128>>,
    pub forest: Vec<[Gf; 2]>,
    pub nonces: Vec<u64>,
}

fn absorb_pair(t: &mut impl Transcript, pair: &[Gf; 2]) {
    let mut bytes = [0u8; 32];
    bytes[..16].copy_from_slice(&pair[0].to_bytes());
    bytes[16..].copy_from_slice(&pair[1].to_bytes());
    t.absorb_slice(&bytes);
}

/// Keep wfbitz's optimized arithmetic and variable order while drawing every
/// challenge through Falcon's domain-separated, per-block grinding schedule.
struct ForestProver<'a, T> {
    transcript: &'a mut T,
    messages: Vec<[Gf; 2]>,
}
impl<T: Transcript> GkrProverTranscript for ForestProver<'_, T> {
    fn write_pair(&mut self, pair: [Gf; 2]) {
        absorb_pair(self.transcript, &pair);
        self.messages.push(pair);
    }
    fn challenge(&mut self) -> Gf {
        self.transcript.get_field_challenge(&())
    }
}
struct ForestVerifier<'a, T> {
    transcript: &'a mut T,
    messages: &'a [[Gf; 2]],
    cursor: usize,
}
impl<T: Transcript> GkrVerifierTranscript for ForestVerifier<'_, T> {
    fn read_pair(&mut self) -> Option<[Gf; 2]> {
        let pair = *self.messages.get(self.cursor)?;
        self.cursor += 1;
        absorb_pair(self.transcript, &pair);
        Some(pair)
    }
    fn challenge(&mut self) -> Option<Gf> {
        Some(self.transcript.get_field_challenge(&()))
    }
}

fn err(message: &'static str) -> FalconError {
    FalconError::Piop(message.into())
}

fn weights(point: &[F], field: &Cfg) -> Vec<u128> {
    eq_table(point, field)
        .expect("validated bridge point")
        .into_iter()
        .map(|x| u128::from(field.to_integer(&x)))
        .collect()
}

fn bind(t: &mut impl Transcript, sums: &[Vec<u128>]) {
    t.absorb_slice(b"bitz/falcon-hybrid/integer-folds/v2");
    t.absorb_slice(&(sums.len() as u64).to_le_bytes());
    for column in sums {
        t.absorb_slice(&(column.len() as u64).to_le_bytes());
        for x in column {
            t.absorb_slice(&x.to_le_bytes());
        }
    }
}

fn root_point(t: &mut impl Transcript, p: &IntegerMatrixLayout) -> Vec<Gf> {
    // Roots are indexed [column | limb]. This frame starts the initial
    // grinding block after both sets of integer sums have been bound.
    t.absorb_slice(b"bitz/falcon-hybrid/wfbitz-joint-limbs/v1");
    t.absorb_slice(&(p.row_vars as u64).to_le_bytes());
    t.absorb_slice(&(p.col_vars as u64).to_le_bytes());
    t.get_field_challenges(p.col_vars + 1, &())
}

fn row_images(chunks: &[Vec<u128>], comb: &FixedBasePow) -> Vec<Gf> {
    (0..chunks[0].len())
        .flat_map(|r| [comb.pow(chunks[0][r]), comb.pow(chunks[1][r])])
        .collect()
}

pub(super) fn prove(
    t: &mut impl Transcript,
    source: &FalconSourceWitness,
    point: &[F],
    modulus: u128,
    grinding_bits: u32,
    row_weights: &[u128],
) -> Result<(Proof, Vec<BinaryClaim>), FalconError> {
    let p = source.layout().bitz_params();
    if point.len() != p.row_vars + p.col_vars {
        return Err(err("bridge point shape"));
    }
    F::make_cfg(&Uint::from(modulus)).map_err(|_| err("bridge modulus"))?;
    if row_weights.len() != p.rows() || row_weights.iter().any(|&weight| weight >= modulus) {
        return Err(err("bridge row weights"));
    }
    let q_bits = 128 - modulus.leading_zeros() as usize;
    let chunks = chunk_row_weights(
        row_weights,
        mod_q_chunk_width(&p),
        mod_q_num_chunks(&p, q_bits),
    );
    if chunks.len() != 2 {
        return Err(err("bridge needs two bounded prime limbs"));
    }
    let fold_span = tracing::info_span!("falcon_bridge:integer_folds").entered();
    let chunk_refs: Vec<_> = chunks.iter().map(Vec::as_slice).collect();
    let sums = fold_values_bits_multi(&p, source.rows(), &chunk_refs);
    drop(fold_span);
    bind(t, &sums);
    let packing_span = tracing::info_span!("falcon_bridge:column_packing").entered();
    let mut packed_cols = pack_columns_from_rows(&p, source.rows());
    // Interleave a repeated source bit at row 2*r+limb. Highest-row-bit
    // products reduce only original rows; after d layers the limb survives.
    crate::utils::cfg_iter_mut!(&mut packed_cols).for_each(|column| {
        let rows = column.len();
        column.resize(2 * rows, 0);
        for r in (0..rows).rev() {
            let word = column[r];
            column[2 * r] = word;
            column[2 * r + 1] = word;
        }
    });
    drop(packing_span);
    let comb = FixedBasePow::new(crate::pcs::smallest_generator(), 128, WINDOW);
    let power_span = tracing::info_span!("falcon_bridge:power_tables").entered();
    let images = row_images(&chunks, &comb);
    drop(power_span);
    let mut grinder = ProverBlockGrindingTranscript::<_, BridgeGrinding>::new(t, grinding_bits);
    let zeta = root_point(&mut grinder, &p);
    let forest_span = tracing::info_span!("falcon_bridge:wfbitz_forest").entered();
    let mut state = ForestProver {
        transcript: &mut grinder,
        messages: Vec::with_capacity(message_count(&p)),
    };
    let (point, value) = Forest::new(p.row_vars + 1, p.col_vars, &packed_cols, &images)
        .prove_depth(&mut state, &zeta, p.row_vars);
    let forest = state.messages;
    drop(forest_span);
    let claim = binary_claim(&p, &images, &point, value);
    let nonces = grinder.finish();
    Ok((
        Proof {
            sums,
            forest,
            nonces,
        },
        vec![claim],
    ))
}

pub(super) fn verify(
    t: &mut impl Transcript,
    layout: &super::FalconSourceLayout,
    point: &[F],
    value: F,
    modulus: u128,
    proof: &Proof,
    grinding_bits: u32,
) -> Result<Vec<BinaryClaim>, FalconError> {
    let p = layout.bitz_params();
    if point.len() != p.row_vars + p.col_vars {
        return Err(err("bridge point shape"));
    }
    if proof.forest.len() != message_count(&p) {
        return Err(err("bridge forest message count"));
    }
    let field = F::make_cfg(&Uint::from(modulus)).map_err(|_| err("bridge modulus"))?;
    let q_bits = 128 - modulus.leading_zeros() as usize;
    let width = mod_q_chunk_width(&p);
    let chunks = chunk_row_weights(
        &weights(&point[..p.row_vars], &field),
        width,
        mod_q_num_chunks(&p, q_bits),
    );
    if chunks.len() != 2 || proof.sums.len() != chunks.len() {
        return Err(err("bridge chunk shape"));
    }
    let cols = weights(&point[p.row_vars..], &field);
    let mut result = 0;
    let mut scale = 1;
    for (w, sums) in chunks.iter().zip(&proof.sums) {
        let bound = w
            .iter()
            .try_fold(0u128, |a, &b| a.checked_add(b))
            .ok_or(err("bridge sum bound"))?;
        if bound == u128::MAX || sums.len() != p.cols() || sums.iter().any(|&s| s > bound) {
            return Err(err("bridge integer fold magnitude"));
        }
        let partial = sums.iter().zip(&cols).fold(0, |a, (&s, &c)| {
            field.add_u128(a, field.mul_u128(field.reduce_u128(s), c))
        });
        result = field.add_u128(result, field.mul_u128(scale, partial));
        scale = field.mul_u128(scale, 1u128 << width);
    }
    if result != u128::from(field.to_integer(&value)) {
        return Err(err("bridge prime read-off"));
    }
    bind(t, &proof.sums);
    let comb = FixedBasePow::new(crate::pcs::smallest_generator(), 128, WINDOW);
    let mut grinder =
        VerifierBlockGrindingTranscript::<_, BridgeGrinding>::new(t, grinding_bits, &proof.nonces);
    let zeta = root_point(&mut grinder, &p);
    let eq = build_eq_x_r_vec(&zeta, &()).expect("nonempty column and limb point");
    let root_claim = proof
        .sums
        .iter()
        .flatten()
        .zip(eq)
        .fold(Gf::zero(), |acc, (&sum, weight)| {
            acc + weight * comb.pow(sum)
        });
    let mut state = ForestVerifier {
        transcript: &mut grinder,
        messages: &proof.forest,
        cursor: 0,
    };
    let (point, value) = gpgkr_verify(&mut state, root_claim, &zeta, p.row_vars as u32)
        .ok_or(err("bridge forest"))?;
    if state.cursor != state.messages.len() {
        return Err(err("bridge forest trailing messages"));
    }
    let images = row_images(&chunks, &comb);
    let claim = binary_claim(&p, &images, &point, value);
    grinder.finish().map_err(|_| err("bridge grinding"))?;
    Ok(vec![claim])
}

#[cfg(test)]
mod tests {
    use super::super::{FalconSourceLayout, verification_trace};
    use super::*;
    use crate::transcript::Blake3Transcript;

    #[test]
    fn wfbitz_joint_limb_forest_fits_security_budget() {
        for batch in 1..=1024 {
            let layout = FalconSourceLayout::new(batch).unwrap();
            let p = layout.bitz_params();
            let d = p.row_vars;
            let s = p.col_vars + 1;
            assert_eq!(p.word_bits, 1);
            assert_eq!(d, 13);
            assert_eq!(s, 5 + layout.capacity().ilog2() as usize);
            // Count the accepted verifier rounds independently of the formula.
            let sumcheck_rounds: usize = (0..d).map(|ell| ell + s).sum();
            assert_eq!(error_numerator(&layout), s + 3 * sumcheck_rounds + d);
            assert!((447..=847).contains(&error_numerator(&layout)));
            assert_eq!(mod_q_chunk_width(&p), 113);
            for prime_bits in [126, 127] {
                assert_eq!(mod_q_num_chunks(&p, prime_bits), 2);
            }
            // Every canonical bit column has exponent < 2^126, strictly
            // below the order 2^128-1 of the fixed multiplicative generator.
            let max_limb = (1u128 << mod_q_chunk_width(&p)) - 1;
            assert!(max_limb * (p.rows() as u128) < (1u128 << 126));
        }
        assert!(crate::pcs::is_generator(crate::pcs::smallest_generator()));
    }

    #[test]
    fn bridge_rejects_nonces_from_other_domains_and_difficulties() {
        use crate::piop::spartan::grinding::{
            GrindingRound, derive_grinding_seed, derive_grinding_seed_in_domain,
            grinding_nonce_is_valid, verify_and_absorb,
        };

        const BITS: u32 = 3;
        let mut initial = Blake3Transcript::new();
        initial.absorb_slice(b"fixed bridge prefix");
        let current = derive_grinding_seed(
            &mut initial.clone(),
            GrindingRound::<BridgeGrinding>::new(0),
            BITS,
        )
        .unwrap();
        let other_domain =
            derive_grinding_seed_in_domain(&mut initial.clone(), b"unrelated-test-domain", 0, BITS)
                .unwrap();
        let other_difficulty = derive_grinding_seed(
            &mut initial.clone(),
            GrindingRound::<BridgeGrinding>::new(0),
            BITS + 1,
        )
        .unwrap();
        assert_ne!(current, other_domain);
        assert_ne!(current, other_difficulty);
        for (other_seed, other_bits) in [(&other_domain, BITS), (&other_difficulty, BITS + 1)] {
            // Choose a nonce whose rejection is certain, independent of a
            // chance nonce collision across the two independent seeds.
            let other_nonce = (0..u64::MAX)
                .find(|&nonce| {
                    grinding_nonce_is_valid(other_seed, nonce, other_bits).unwrap()
                        && !grinding_nonce_is_valid(&current, nonce, BITS).unwrap()
                })
                .unwrap();
            assert!(
                verify_and_absorb(
                    &mut initial.clone(),
                    GrindingRound::<BridgeGrinding>::new(0),
                    BITS,
                    other_nonce,
                )
                .is_err()
            );
        }
    }

    #[test]
    fn batched_limbs_authenticate_source_and_reject_compensating_changes() {
        const PK: &[u8] = include_bytes!("fixtures/public_key.bin");
        const MSG: &[u8] = include_bytes!("fixtures/message.bin");
        const SIG: &[u8] = include_bytes!("fixtures/signature_ct.bin");
        let layout = FalconSourceLayout::new(1).unwrap();
        let trace = verification_trace(PK, MSG, SIG).unwrap();
        let source =
            FalconSourceWitness::from_traces(layout.clone(), &[MSG], &[SIG], &[trace]).unwrap();
        let p = layout.bitz_params();
        let modulus = (1u128 << 127) - 1;
        let field = F::make_cfg(&Uint::from(modulus)).unwrap();
        let point: Vec<_> = (0..p.row_vars + p.col_vars)
            .map(|i| F::from_with_cfg(Uint::from(3 + 2 * i as u128), &field))
            .collect();
        let row_weights = weights(&point[..p.row_vars], &field);
        let column_weights = weights(&point[p.row_vars..], &field);
        // Independent prime-field evaluation over the original committed bits.
        let mut value = 0;
        for (c, row) in source.rows().iter().enumerate() {
            let mut column = 0;
            for r in 0..p.rows() {
                if (row[r >> 6] >> (r & 63)) & 1 == 1 {
                    column = field.add_u128(column, row_weights[r]);
                }
            }
            value = field.add_u128(value, field.mul_u128(column, column_weights[c]));
        }
        let value = F::from_with_cfg(Uint::from(value), &field);
        let mut pt = Blake3Transcript::new();
        let (proof, claims) = prove(&mut pt, &source, &point, modulus, 2, &row_weights).unwrap();
        assert_eq!(claims.len(), 1);
        let s = p.col_vars + 1;
        let rounds: usize = (0..p.row_vars).map(|ell| ell + s).sum();
        assert_eq!(proof.nonces.len(), 1 + rounds + p.row_vars);
        assert_eq!(proof.forest.len(), rounds + p.row_vars);
        let mut vt = Blake3Transcript::new();
        let verified = verify(&mut vt, &layout, &point, value, modulus, &proof, 2).unwrap();
        for (claim, verified) in claims.iter().zip(&verified) {
            assert_eq!(claim.low, verified.low);
            assert_eq!(claim.high_point, verified.high_point);
            assert_eq!(claim.value, verified.value);
            let column_eq = build_eq_x_r_vec(&claim.high_point, &()).unwrap();
            let mut binary_value = Gf::zero();
            for (c, row) in source.rows().iter().enumerate() {
                let mut column = Gf::zero();
                for r in 0..p.rows() {
                    if (row[r >> 6] >> (r & 63)) & 1 == 1 {
                        column += claim.low[r];
                    }
                }
                binary_value += column_eq[c] * column;
            }
            assert_eq!(binary_value, claim.value);
        }
        assert_eq!(pt.get_challenge::<u128>(), vt.get_challenge::<u128>());

        let reject = |changed: &Proof| {
            assert!(
                verify(
                    &mut Blake3Transcript::new(),
                    &layout,
                    &point,
                    value,
                    modulus,
                    changed,
                    2,
                )
                .is_err()
            );
        };
        let mut changed = proof.clone();
        changed.nonces.pop();
        reject(&changed);
        let mut changed = proof.clone();
        changed.nonces.push(0);
        reject(&changed);
        let mut changed = proof.clone();
        changed.nonces[0] ^= 1;
        reject(&changed);
        assert!(
            verify(
                &mut Blake3Transcript::new(),
                &layout,
                &point,
                value,
                modulus,
                &proof,
                3,
            )
            .is_err()
        );
        for limb in 0..2 {
            let mut changed = proof.clone();
            changed.sums[limb][0] ^= 1;
            reject(&changed);
        }
        // Preserve the prime read-off exactly while shifting mass between
        // limbs. Only the authenticated forest can detect this forgery.
        let mut changed = proof.clone();
        let radix = 1u128 << mod_q_chunk_width(&p);
        let c = changed.sums[0]
            .iter()
            .position(|&sum| sum >= radix)
            .unwrap();
        changed.sums[0][c] -= radix;
        changed.sums[1][c] += 1;
        let error = verify(
            &mut Blake3Transcript::new(),
            &layout,
            &point,
            value,
            modulus,
            &changed,
            2,
        )
        .err()
        .unwrap();
        assert!(error.to_string().contains("bridge forest"));
        let mut changed = proof.clone();
        changed.sums.swap(0, 1);
        reject(&changed);
        let mut changed = proof.clone();
        changed.forest[0][0] += Gf::one();
        reject(&changed);
        let mut changed = proof.clone();
        changed.forest.last_mut().unwrap()[1] += Gf::one();
        reject(&changed);
        let mut changed = proof.clone();
        changed.forest.pop();
        reject(&changed);
        let mut changed = proof.clone();
        changed.forest.push([Gf::one(); 2]);
        reject(&changed);
    }
}

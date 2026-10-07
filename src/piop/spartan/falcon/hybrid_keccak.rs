// Compact binary Keccak prefix for the Falcon hybrid composition.
//
// This proves independent Keccak permutations and returns two linear claims
// on an already committed binary witness. The caller must prove SHAKE input,
// chaining, and extraction constraints and authenticate all claims through
// its shared opening. Neither the nonce nor a public digest is absorbed here.
// This prefix alone is not a SHAKE proof and does not provide zero knowledge.

pub(crate) mod grinding;

use flock_core::{
    field::Gf128,
    lincheck::{LincheckCircuit, LincheckProof},
    proof::ZClaim,
    r1cs::{BlockR1cs, WitnessLayout},
    zerocheck::ZerocheckProof,
};
use flock_prover::r1cs_hashes::keccak as compact;

use crate::{
    ligerito_flock::ZincChallenger,
    piop::spartan::grinding::{GrindingDomain, GrindingError},
    transcript::traits::Transcript,
};
use grinding::{ProverBlockGrindingTranscript, VerifierBlockGrindingTranscript};

pub(crate) const PERMUTATIONS: usize = super::PARAMETERS.permutations();
pub(crate) const RATE_BYTES: usize = 136;
pub(crate) const SAMPLES: usize = super::HASH_TO_POINT_SAMPLES;
pub(crate) const MAX_CAPACITY: usize = 8_192;
const LOG_PACKING: usize = 7;

#[derive(Debug, thiserror::Error)]
pub(crate) enum KeccakError {
    #[error("invalid binary Keccak prefix parameters: {0}")]
    Invalid(&'static str),
    #[error("binary Keccak prefix verification failed: {0}")]
    Verification(String),
    #[error(transparent)]
    Grinding(#[from] GrindingError),
}

/// One normalized linear claim in packed-address order:
/// `sum_i z[i] * low[i % 128] * eq(high_point, i / 128) = value`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct BinaryLinearClaim {
    pub(crate) low: Vec<Gf128>,
    pub(crate) high_point: Vec<Gf128>,
    pub(crate) value: Gf128,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub(crate) struct PrefixProof {
    pub(crate) zerocheck: ZerocheckProof,
    pub(crate) lincheck: LincheckProof,
    pub(crate) grinding_nonces: Vec<u64>,
}

/// Scratch consumed by zerocheck and lincheck. The caller retains the local
/// packed source view for the joint opening.
pub(crate) struct KeccakAuxiliary {
    a: Vec<Gf128>,
    b: Vec<Gf128>,
    lincheck: Vec<u8>,
}

/// Conservative prefix contribution to the composition's computational
/// soundness budget. It excludes the shared opening and SHAKE copy checks.
#[derive(Clone, Copy, Debug)]
pub(crate) struct PrefixSecurity {
    pub(crate) component_bits: u32,
    pub(crate) grinding_bits: u32,
    pub(crate) raw_error_numerator: usize,
    pub(crate) challenge_blocks: usize,
}

impl PrefixSecurity {
    /// Grinding-adjusted error in the same work-normalized model used by the
    /// surrounding composition. Grinding does not change interactive error.
    pub(crate) fn error_bound(self) -> f64 {
        self.raw_error_numerator as f64 * 2f64.powi(-128 - self.grinding_bits as i32)
    }
}

enum PrefixGrinding {}
impl GrindingDomain for PrefixGrinding {
    const DOMAIN: &'static [u8] = b"bitz/falcon1024-ct/hybrid-keccak-block/v1";
}

/// Prepared public circuit. No witness-sized tables or commitments are built
/// during preparation. BatchMajor addresses are
/// `[7 in-word | log2(capacity) signature | log2(permutations) permutation | 9 chunk]`.
pub(crate) struct PreparedKeccak {
    batch_len: usize,
    capacity: usize,
    r1cs: BlockR1cs,
    first_permutation: usize,
    permutations: usize,
}

/// The block index is [signature | permutation], so the public constant
/// column is the prefix mask of the signature coordinates. All circuit
/// coefficients remain those of the real Keccak walker.
struct ActiveKeccakCircuit {
    batch_len: usize,
    capacity: usize,
}

impl LincheckCircuit for ActiveKeccakCircuit {
    fn n_cols(&self) -> usize {
        compact::KeccakLincheckCircuit.n_cols()
    }

    fn fold_alpha_batched(&self, alpha: Gf128, eq_inner: &[Gf128]) -> Vec<Gf128> {
        compact::KeccakLincheckCircuit.fold_alpha_batched(alpha, eq_inner)
    }

    fn const_pin_col(&self) -> Option<usize> {
        Some(compact::Z_CONST)
    }

    fn const_pin_value(&self, outer_point: &[Gf128]) -> Gf128 {
        let signature_vars = self.capacity.ilog2() as usize;
        assert!(outer_point.len() >= signature_vars);
        prefix_mask(&outer_point[..signature_vars], self.batch_len)
    }
}

/// MLE of `[index < count]`, in O(log capacity) field operations.
fn prefix_mask(point: &[Gf128], count: usize) -> Gf128 {
    assert!(count <= 1usize << point.len());
    if count == 1usize << point.len() {
        return Gf128::ONE;
    }
    let mut equal = Gf128::ONE;
    let mut less = Gf128::ZERO;
    for (bit, &r) in point.iter().enumerate().rev() {
        if count >> bit & 1 == 1 {
            less += equal * (Gf128::ONE + r);
            equal *= r;
        } else {
            equal *= Gf128::ONE + r;
        }
    }
    less
}

impl PreparedKeccak {
    pub(crate) fn new_slab(
        batch_len: usize,
        first: usize,
        permutations: usize,
    ) -> Result<Self, KeccakError> {
        if !(1..=MAX_CAPACITY).contains(&batch_len)
            || !(0..super::KECCAK_SLABS).any(|i| super::PARAMETERS.slab(i) == (first, permutations))
        {
            return Err(KeccakError::Invalid("invalid SHAKE permutation slab"));
        }
        // The Flock byte-stripe lincheck requires at least eight blocks.
        let capacity = batch_len.next_power_of_two().max(8 / permutations);
        let mut r1cs = compact::build_block_r1cs((capacity * permutations).ilog2() as usize);
        r1cs.layout = WitnessLayout::BatchMajor;
        Ok(Self {
            batch_len,
            capacity,
            r1cs,
            first_permutation: first,
            permutations,
        })
    }

    pub(crate) fn generate_chain(
        &self,
        initial: &[[u64; 25]],
    ) -> Result<(Vec<Gf128>, KeccakAuxiliary, Vec<Vec<[u64; 25]>>), KeccakError> {
        if initial.len() != self.batch_len {
            return Err(KeccakError::Invalid("SHAKE chain input shape"));
        }
        let (z, a, b, lincheck, outputs) = compact::generate_chained_witness_batch_major(
            initial,
            self.capacity,
            self.permutations,
        );
        Ok((z, KeccakAuxiliary { a, b, lincheck }, outputs))
    }

    pub(crate) fn capacity(&self) -> usize {
        self.capacity
    }
    pub(crate) fn bit_vars(&self) -> usize {
        self.r1cs.m
    }
    pub(crate) fn packed_vars(&self) -> usize {
        self.r1cs.m - LOG_PACKING
    }

    /// Component targets include the caller's margin for the full union bound
    /// (e.g. requested security + 8 bits). No proof chooses its own difficulty.
    pub(crate) fn security(&self, component_bits: u32) -> Result<PrefixSecurity, KeccakError> {
        if !(100..=150).contains(&component_bits) {
            return Err(KeccakError::Invalid(
                "component target must be in 100..=150",
            ));
        }
        // Initial random eq weights: <=m-7; inverse-at-one exceptional points:
        // <=m-13; constant-column fold over blocks: <=m-16. Univariate skip:
        // <128. Zerocheck: 2(m-6). Lincheck alpha/constant pin batching: <=2,
        // ten quadratic rounds: <=20, final univariate skip: <64.
        // Total <=5m+166 <4m+256 for every supported m<=34.
        // The seven fixed inner weights encode Boolean residuals in distinct
        // extension-field basis positions; they are not independent draws.
        let raw_error_numerator = 4 * self.bit_vars() + 256;
        let log_error = raw_error_numerator.next_power_of_two().ilog2();
        let grinding_bits = (component_bits + log_error).saturating_sub(128);
        if grinding_bits > 32 {
            return Err(KeccakError::Invalid(
                "prefix exceeds the 32-bit grinding cap",
            ));
        }
        // Initial vector, univariate skip, m-6 zerocheck rounds, alpha+beta,
        // ten lincheck rounds, and the final univariate skip.
        Ok(PrefixSecurity {
            component_bits,
            grinding_bits,
            raw_error_numerator,
            challenge_blocks: self.bit_vars() + 8,
        })
    }

    /// Global committed address of a per-permutation lane-contiguous bit.
    pub(crate) fn bit_index(
        &self,
        signature: usize,
        permutation: usize,
        local_bit: usize,
    ) -> usize {
        assert!(
            signature < self.capacity
                && (self.first_permutation..self.first_permutation + self.permutations)
                    .contains(&permutation)
                && local_bit < compact::K
        );
        let permutation = permutation - self.first_permutation;
        let block = permutation * self.capacity + signature;
        (((local_bit >> LOG_PACKING) << self.r1cs.n_log()) | block) * 128 + (local_bit & 127)
    }

    /// `state_bit = 64 * lane + bit`, matching SHAKE's little-endian bytes.
    pub(crate) fn initial_bit_index(
        &self,
        signature: usize,
        permutation: usize,
        state_bit: usize,
    ) -> usize {
        assert!(state_bit < compact::STATE_BITS);
        self.bit_index(signature, permutation, compact::STATE0_BIT_BASE + state_bit)
    }

    pub(crate) fn output_bit_index(
        &self,
        signature: usize,
        permutation: usize,
        state_bit: usize,
    ) -> usize {
        assert!(state_bit < compact::STATE_BITS);
        self.bit_index(
            signature,
            permutation,
            compact::STATE24_BIT_BASE + state_bit,
        )
    }

    /// Committed output bit for one big-endian Falcon sample; `bit` is the
    /// sample's numeric LSB-first bit index, as used by the arithmetic branch.
    pub(crate) fn sample_bit_index(&self, signature: usize, sample: usize, bit: usize) -> usize {
        assert!(sample < SAMPLES && bit < 16);
        let byte = 2 * sample + usize::from(bit < 8);
        self.output_bit_index(
            signature,
            byte / RATE_BYTES,
            8 * (byte % RATE_BYTES) + bit % 8,
        )
    }

    fn bind<T: Transcript>(
        &self,
        t: &mut T,
        source_root: &[u8; 32],
        statement_digest: &[u8; 32],
        security: PrefixSecurity,
    ) -> [u8; 32] {
        let mut h = blake3::Hasher::new();
        h.update(b"bitz/falcon1024-ct/hybrid-keccak-projected-prefix/v3");
        h.update(source_root);
        h.update(statement_digest);
        h.update(b"compact-keccak-24:state0,state24,chi24;batch-major/v1");
        h.update(b"constant=signature<batch;inactive=all-zero;signature-before-permutation/v1");
        h.update(&self.r1cs.statement_digest());
        for n in [
            self.first_permutation,
            self.permutations,
            self.batch_len,
            self.capacity,
            self.bit_vars(),
        ] {
            h.update(&(n as u64).to_le_bytes());
        }
        h.update(&security.component_bits.to_le_bytes());
        h.update(&security.grinding_bits.to_le_bytes());
        let digest = *h.finalize().as_bytes();
        t.absorb_slice(b"bitz/falcon1024-ct/hybrid-keccak-projected-prefix/v3");
        t.absorb_slice(&digest);
        digest
    }

    pub(crate) fn prove_prefix<T: Transcript + Send>(
        &self,
        packed: &[Gf128],
        auxiliary: KeccakAuxiliary,
        source_root: &[u8; 32],
        statement_digest: &[u8; 32],
        transcript: &mut T,
        component_bits: u32,
    ) -> Result<(PrefixProof, [BinaryLinearClaim; 2], [Vec<Gf128>; 2]), KeccakError> {
        let n = 1usize << self.packed_vars();
        if packed.len() != n
            || auxiliary.a.len() != n
            || auxiliary.b.len() != n
            || auxiliary.lincheck.len() != n * 16
        {
            return Err(KeccakError::Invalid(
                "witness buffers have the wrong dimensions",
            ));
        }
        let security = self.security(component_bits)?;
        let context = self.bind(transcript, source_root, statement_digest, security);
        let mut grinding = ProverBlockGrindingTranscript::<_, PrefixGrinding>::new(
            transcript,
            security.grinding_bits,
        );
        let circuit = ActiveKeccakCircuit {
            batch_len: self.batch_len,
            capacity: self.capacity,
        };
        let prefix = flock_prover::prover::prove_fast_source_prefix(
            &self.r1cs,
            &context,
            packed,
            auxiliary.a,
            auxiliary.b,
            auxiliary.lincheck,
            &circuit,
            &mut ZincChallenger(&mut grinding),
        );
        if grinding.block_count() != security.challenge_blocks {
            return Err(KeccakError::Invalid(
                "Flock prefix challenge schedule changed",
            ));
        }
        let claims = [
            normalize_claim(&prefix.ab, self.bit_vars())?,
            normalize_claim(&prefix.c, self.bit_vars())?,
        ];
        let proof = PrefixProof {
            zerocheck: prefix.zc_proof,
            lincheck: prefix.lc_proof,
            grinding_nonces: grinding.finish(),
        };
        let marginals = [
            prefix
                .s_hat_v_ab
                .ok_or(KeccakError::Invalid("missing Keccak AB marginals"))?,
            prefix.s_hat_v_c,
        ];
        if marginals.iter().any(|v| v.len() != 128) {
            return Err(KeccakError::Invalid("Keccak marginal shape"));
        }
        Ok((proof, claims, marginals))
    }

    pub(crate) fn verify_prefix<T: Transcript + Send>(
        &self,
        proof: &PrefixProof,
        source_root: &[u8; 32],
        statement_digest: &[u8; 32],
        transcript: &mut T,
        component_bits: u32,
    ) -> Result<[BinaryLinearClaim; 2], KeccakError> {
        let security = self.security(component_bits)?;
        let expected_nonces = if security.grinding_bits == 0 {
            0
        } else {
            security.challenge_blocks
        };
        if proof.grinding_nonces.len() != expected_nonces {
            return Err(KeccakError::Invalid(
                "wrong binary prefix grinding nonce count",
            ));
        }
        let context = self.bind(transcript, source_root, statement_digest, security);
        let mut grinding = VerifierBlockGrindingTranscript::<_, PrefixGrinding>::new(
            transcript,
            security.grinding_bits,
            &proof.grinding_nonces,
        );
        let circuit = ActiveKeccakCircuit {
            batch_len: self.batch_len,
            capacity: self.capacity,
        };
        let result = flock_core::verifier::verify_source_core(
            &self.r1cs,
            &proof.zerocheck,
            &proof.lincheck,
            &context,
            &circuit,
            &mut ZincChallenger(&mut grinding),
        );
        let blocks = grinding.block_count();
        grinding.finish()?;
        let (ab, c) = result.map_err(|error| KeccakError::Verification(format!("{error:?}")))?;
        if blocks != security.challenge_blocks {
            return Err(KeccakError::Invalid(
                "Flock prefix challenge schedule changed",
            ));
        }
        Ok([
            normalize_claim(&ab, self.bit_vars())?,
            normalize_claim(&c, self.bit_vars())?,
        ])
    }
}

pub(crate) fn initial_state(nonce: &[u8; 40], message: &[u8; 32]) -> [u64; 25] {
    let mut absorb = [0u8; RATE_BYTES];
    absorb[..40].copy_from_slice(nonce);
    absorb[40..72].copy_from_slice(message);
    absorb[72] = 0x1f;
    absorb[RATE_BYTES - 1] = 0x80;
    let mut lanes = [0u64; compact::N_LANES];
    for (lane, bytes) in lanes.iter_mut().zip(absorb.chunks_exact(8)) {
        *lane = u64::from_le_bytes(bytes.try_into().expect("one lane"));
    }
    lanes
}

/// Both Flock layouts expose ZClaim suffixes in physical address order.
/// The first suffix coordinate is address bit 6; combining it with the
/// arbitrary 64 skip weights gives exactly one 128-entry packed-word form.
fn normalize_claim(claim: &ZClaim, bit_vars: usize) -> Result<BinaryLinearClaim, KeccakError> {
    let suffix = claim
        .point
        .x_inner_rest
        .iter()
        .chain(&claim.point.x_outer)
        .copied()
        .collect::<Vec<_>>();
    if suffix.len() + compact::K_SKIP != bit_vars || suffix.is_empty() {
        return Err(KeccakError::Invalid("unexpected Flock claim point shape"));
    }
    let skip = claim.point.z_skip.weights(compact::K_SKIP);
    let bit6 = suffix[0];
    let mut low = vec![Gf128::ZERO; 128];
    for (index, weight) in skip.into_iter().enumerate() {
        low[index] = weight * (Gf128::ONE + bit6);
        low[index + 64] = weight * bit6;
    }
    Ok(BinaryLinearClaim {
        low,
        high_point: suffix[1..].to_vec(),
        value: claim.value,
    })
}
falcon_tests! {
mod tests {
    use super::*;
    use crate::transcript::Blake3Transcript;
    use flock_core::{
        lincheck::{QuirkyPoint, SkipPoint},
        merkle::HashKind,
        pcs::{PcsParams, commit},
    };

    fn bit(packed: &[Gf128], index: usize) -> bool {
        let word = packed[index / 128];
        if index % 128 < 64 {
            (word.lo >> (index % 64)) & 1 != 0
        } else {
            (word.hi >> (index % 64)) & 1 != 0
        }
    }

    fn eq(point: &[Gf128], index: usize) -> Gf128 {
        point.iter().enumerate().fold(Gf128::ONE, |acc, (j, &r)| {
            acc * if (index >> j) & 1 == 1 {
                r
            } else {
                Gf128::ONE + r
            }
        })
    }

    fn evaluate(packed: &[Gf128], claim: &BinaryLinearClaim) -> Gf128 {
        let high = flock_core::lincheck::build_eq_table(&claim.high_point);
        packed
            .iter()
            .enumerate()
            .fold(Gf128::ZERO, |sum, (word_index, word)| {
                let mut value = Gf128::ZERO;
                for (half, mut bits) in [word.lo, word.hi].into_iter().enumerate() {
                    while bits != 0 {
                        let index = bits.trailing_zeros() as usize;
                        value += claim.low[half * 64 + index];
                        bits &= bits - 1;
                    }
                }
                sum + value * high[word_index]
            })
    }

    fn params(m: usize) -> PcsParams {
        PcsParams {
            m,
            log_inv_rate: 1,
            log_batch_size: 4,
            profile: flock_core::pcs::ligerito::LigeritoProfile::Fast,
            merkle_hash: HashKind::Blake3,
        }
    }

    #[test]
    fn activity_mask_matches_dense_prefix_at_nonboolean_points() {
        for signature_vars in 0..=7 {
            let point: Vec<_> = (0..signature_vars)
                .map(|i| Gf128 {
                    lo: 31 + i as u64,
                    hi: 19 * i as u64 + 3,
                })
                .collect();
            let weights = flock_core::lincheck::build_eq_table(&point);
            for count in 0..=weights.len() {
                let expected = weights[..count]
                    .iter()
                    .copied()
                    .fold(Gf128::ZERO, |a, b| a + b);
                assert_eq!(prefix_mask(&point, count), expected);
                let circuit = ActiveKeccakCircuit {
                    batch_len: count,
                    capacity: weights.len(),
                };
                let mut outer = point.clone();
                outer.extend([Gf128 { lo: 71, hi: 9 }, Gf128 { lo: 13, hi: 42 }]);
                assert_eq!(circuit.const_pin_value(&outer), expected);
            }
        }
    }

    #[test]
    fn fused_chain_matches_reference_witness_in_every_buffer() {
        for (live, capacity, permutations) in [
            (1, 2, 4),
            (3, 4, 16),
            (7, 8, 4),
            (8, 8, 4),
            (9, 16, 16),
            (16, 16, 4),
        ] {
            let initial: Vec<_> = (0..live)
                .map(|i| initial_state(&[i as u8 + 7; 40], &[i as u8 + 31; 32]))
                .collect();
            let (z, a, b, stripe, outputs) =
                compact::generate_chained_witness_batch_major(&initial, capacity, permutations);
            let mut states = vec![[false; compact::STATE_BITS]; capacity * permutations];
            for signature in 0..live {
                let mut lanes = initial[signature];
                for permutation in 0..permutations {
                    states[permutation * capacity + signature] = compact::lanes_to_state(&lanes);
                    for round in 0..24 {
                        compact::keccak_round_lanes(&mut lanes, round);
                    }
                    assert_eq!(outputs[signature][permutation], lanes);
                }
            }
            let mut expected = compact::generate_witness_batch_major(
                &states,
                (capacity * permutations).ilog2() as usize,
            );
            // Reference generator sets the constant wire to one in every instance;
            // retain its live computations and replace every inactive block.
            for signature in live..capacity {
                assert!(outputs[signature].iter().all(|state| *state == [0; 25]));
                for permutation in 0..permutations {
                    let block = permutation * capacity + signature;
                    for source in [&mut expected.0, &mut expected.1, &mut expected.2] {
                        for chunk in 0..compact::K / 128 {
                            source[chunk * capacity * permutations + block] = Gf128::ZERO;
                        }
                    }
                    for local_bit in 0..compact::K {
                        expected.3[block / 8 * compact::K + local_bit] &= !(1 << (block % 8));
                    }
                }
            }
            assert_eq!(z, expected.0, "source: live={live}");
            assert_eq!(a, expected.1, "A: live={live}");
            assert_eq!(b, expected.2, "B: live={live}");
            assert_eq!(stripe, expected.3, "lincheck: live={live}");
        }
    }

    #[test]
    fn normalized_skip_claim_matches_manual_dense_weights() {
        let bit_vars = 11;
        let packed = (0..1 << (bit_vars - 7))
            .map(|i| Gf128 {
                lo: (i as u64 + 0x1234).wrapping_mul(0x987654321fedcba9),
                hi: !(i as u64).wrapping_mul(0xabcdef0123456789),
            })
            .collect::<Vec<_>>();
        let suffix = (1..=bit_vars - compact::K_SKIP)
            .map(|i| Gf128 {
                lo: i as u64 * 0x1fedcb,
                hi: i as u64 * 0x2345,
            })
            .collect::<Vec<_>>();
        let skip_point = SkipPoint::Phi8(Gf128 {
            lo: 0xabcde,
            hi: 0xdef123,
        });
        let skip_weights = skip_point.weights(compact::K_SKIP);
        let mut expected = Gf128::ZERO;
        for index in 0..1 << bit_vars {
            if bit(&packed, index) {
                expected += skip_weights[index & 63] * eq(&suffix, index >> 6);
            }
        }
        // Exercise both address-ordered point conventions.
        for inner_len in [1, 3] {
            let claim = ZClaim {
                point: QuirkyPoint {
                    z_skip: skip_point.clone(),
                    x_inner_rest: suffix[..inner_len].to_vec(),
                    x_outer: suffix[inner_len..].to_vec(),
                },
                value: expected,
            };
            let normalized = normalize_claim(&claim, bit_vars).unwrap();
            assert_eq!(evaluate(&packed, &normalized), expected);
            for index in 0..1 << bit_vars {
                assert_eq!(
                    normalized.low[index & 127] * eq(&normalized.high_point, index >> 7),
                    skip_weights[index & 63] * eq(&suffix, index >> 6)
                );
            }
        }
    }

    #[test]
    fn fixed_zerocheck_coordinates_encode_all_128_boolean_residuals_injectively() {
        use flock_core::zerocheck::univariate_skip_optimized::{
            medium_challenges_ghash, small_challenges_ghash,
        };
        let point = [
            small_challenges_ghash().as_slice(),
            medium_challenges_ghash().as_slice(),
        ]
        .concat();
        let mut weights = flock_core::lincheck::build_eq_table(&point)
            .into_iter()
            .map(|value| (u128::from(value.hi) << 64) | u128::from(value.lo))
            .collect::<Vec<_>>();
        assert_eq!(weights.len(), 128);
        let mut rank = 0;
        for column in (0..128).rev() {
            let mask = 1u128 << column;
            if let Some(pivot) = (rank..weights.len()).find(|&row| weights[row] & mask != 0) {
                weights.swap(rank, pivot);
                let pivot = weights[rank];
                for row in weights.iter_mut().skip(rank + 1) {
                    if *row & mask != 0 {
                        *row ^= pivot;
                    }
                }
                rank += 1;
            }
        }
        assert_eq!(
            rank, 128,
            "fixed coordinates must preserve every Boolean residual, including batch dimensions"
        );
    }

    #[test]
    fn shake_slabs_chaining_extraction_and_padding_match_native_trace() {
        for batch in [1, 3] {
            let nonces: Vec<_> = (0..batch).map(|i| [7 + i as u8 * 13; 40]).collect();
            let messages: Vec<_> = (0..batch).map(|i| [11 + i as u8 * 17; 32]).collect();
            let prepared = [
                PreparedKeccak::new_slab(batch, 0, 16).unwrap(),
                PreparedKeccak::new_slab(batch, 16, 4).unwrap(),
            ];
            assert_eq!(prepared[0].capacity(), batch.next_power_of_two());
            assert_eq!(prepared[1].capacity(), batch.next_power_of_two().max(2));
            let initial: Vec<_> = nonces
                .iter()
                .zip(&messages)
                .map(|(nonce, message)| initial_state(nonce, message))
                .collect();
            let (first, _, outputs) = prepared[0].generate_chain(&initial).unwrap();
            let next: Vec<_> = outputs.iter().take(batch).map(|chain| chain[15]).collect();
            let (last, _, _) = prepared[1].generate_chain(&next).unwrap();
            let packed = [first, last];
            for (slab, words) in prepared.iter().zip(&packed) {
                assert_eq!(words.len(), 1 << slab.packed_vars());
            }
            for signature in 0..batch {
                let input = [nonces[signature].as_slice(), messages[signature].as_slice()].concat();
                let (expected, trace) = super::super::shake256_with_trace(&input, 2 * SAMPLES);
                for sample in 0..SAMPLES {
                    let slab = usize::from(2 * sample / RATE_BYTES >= 16);
                    let word = u16::from_be_bytes([expected[2 * sample], expected[2 * sample + 1]]);
                    for b in 0..16 {
                        assert_eq!(
                            bit(
                                &packed[slab],
                                prepared[slab].sample_bit_index(signature, sample, b)
                            ),
                            (word >> b) & 1 != 0,
                        );
                    }
                }
                for permutation in 0..PERMUTATIONS {
                    let slab = usize::from(permutation >= 16);
                    assert!(bit(
                        &packed[slab],
                        prepared[slab].bit_index(signature, permutation, compact::Z_CONST)
                    ));
                    for state_bit in 0..compact::STATE_BITS {
                        let lane = state_bit / 64;
                        let b = state_bit % 64;
                        assert_eq!(
                            bit(
                                &packed[slab],
                                prepared[slab].initial_bit_index(signature, permutation, state_bit)
                            ),
                            (trace.permutation_inputs[permutation][lane] >> b) & 1 != 0,
                        );
                        let output = bit(
                            &packed[slab],
                            prepared[slab].output_bit_index(signature, permutation, state_bit),
                        );
                        let expected =
                            trace.round_states[(permutation * compact::N_ROUNDS + 23) * 25 + lane];
                        assert_eq!(output, (expected >> b) & 1 != 0);
                        if permutation + 1 < PERMUTATIONS {
                            let next_slab = usize::from(permutation + 1 >= 16);
                            assert_eq!(
                                output,
                                bit(
                                    &packed[next_slab],
                                    prepared[next_slab].initial_bit_index(
                                        signature,
                                        permutation + 1,
                                        state_bit
                                    )
                                )
                            );
                        }
                    }
                }
            }
            // Every inactive signature has an all-zero witness, including
            // the constant wire, intermediate χ outputs, and unused slots.
            for (slab, words) in prepared.iter().zip(&packed) {
                for signature in batch..slab.capacity() {
                    for permutation in
                        slab.first_permutation..slab.first_permutation + slab.permutations
                    {
                        for local_bit in 0..compact::K {
                            assert!(!bit(
                                words,
                                slab.bit_index(signature, permutation, local_bit)
                            ));
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn committed_prefix_replays_and_claims_match_witness_with_tamper_rejection() {
        for (first, permutations) in [(0, 16), (16, 4)] {
            let prepared = PreparedKeccak::new_slab(1, first, permutations).unwrap();
            let (packed, auxiliary, _) = prepared
                .generate_chain(&[initial_state(&[17; 40], &[29; 32])])
                .unwrap();
            let (commitment, _data) = commit(&packed, &params(prepared.bit_vars()));
            let start = || {
                let mut t = Blake3Transcript::new();
                t.absorb_slice(b"caller has already bound the source root and geometry");
                t
            };
            let mut pt = start();
            let (proof, claims, marginals) = prepared
                .prove_prefix(
                    &packed,
                    auxiliary,
                    &commitment.root,
                    &[0x42; 32],
                    &mut pt,
                    108,
                )
                .unwrap();
            for (claim, cached) in claims.iter().zip(&marginals) {
                assert_eq!(evaluate(&packed, claim), claim.value);
                let weights = crate::hybrid::sumcheck::eq_table(&claim.high_point);
                assert_eq!(
                    *cached,
                    crate::hybrid::sumcheck::bit_marginals(&packed, &weights, 1)
                );
            }
            let mut vt = start();
            let checked = prepared
                .verify_prefix(&proof, &commitment.root, &[0x42; 32], &mut vt, 108)
                .unwrap();
            assert_eq!(claims, checked);
            assert_eq!(pt.get_challenge::<u128>(), vt.get_challenge::<u128>());

            let mut tampered = proof.clone();
            tampered.zerocheck.round1_ab[0] += Gf128::ONE;
            assert!(
                prepared
                    .verify_prefix(&tampered, &commitment.root, &[0x42; 32], &mut start(), 108)
                    .is_err()
            );
            let mut tampered = proof.clone();
            tampered.lincheck.rounds[0].0 += Gf128::ONE;
            assert!(
                prepared
                    .verify_prefix(&tampered, &commitment.root, &[0x42; 32], &mut start(), 108)
                    .is_err()
            );
            let mut tampered = proof.clone();
            tampered.lincheck.z_partial[0] += Gf128::ONE;
            assert!(
                prepared
                    .verify_prefix(&tampered, &commitment.root, &[0x42; 32], &mut start(), 108)
                    .is_err()
            );
            let mut tampered = proof.clone();
            tampered.grinding_nonces.push(0);
            assert!(
                prepared
                    .verify_prefix(&tampered, &commitment.root, &[0x42; 32], &mut start(), 108)
                    .is_err()
            );
            let mut wrong_root = commitment.clone();
            wrong_root.root[0] ^= 1;
            assert!(
                prepared
                    .verify_prefix(&proof, &wrong_root.root, &[0x42; 32], &mut start(), 108)
                    .is_err()
            );
            assert!(
                prepared
                    .verify_prefix(&proof, &commitment.root, &[0x42; 32], &mut start(), 136)
                    .is_err()
            );
            assert!(
                prepared
                    .verify_prefix(&proof, &commitment.root, &[0x43; 32], &mut start(), 108)
                    .is_err()
            );
        }
    }

    #[test]
    fn security_accounts_for_every_block_and_scales_to_thousands() {
        for (first, permutations) in [(0, 16), (16, 4)] {
            for batch in [1, 3, 32, 1000, MAX_CAPACITY] {
                let prepared = PreparedKeccak::new_slab(batch, first, permutations).unwrap();
                for target in [108, 136] {
                    let security = prepared.security(target).unwrap();
                    assert!(security.error_bound() <= 2f64.powi(-(target as i32)));
                    assert_eq!(security.challenge_blocks, prepared.bit_vars() + 8);
                    assert_eq!(security.grinding_bits, if target == 108 { 0 } else { 17 });
                }
            }
            assert!(PreparedKeccak::new_slab(0, first, permutations).is_err());
            assert!(PreparedKeccak::new_slab(MAX_CAPACITY + 1, first, permutations).is_err());
        }
        for (first, permutations) in [(0, 4), (16, 16), (0, 32)] {
            assert!(PreparedKeccak::new_slab(1, first, permutations).is_err());
        }
    }

    #[test]
    fn component_136_grinds_every_prefix_block_and_rejects_nonce_tampering() {
        for (first, permutations) in [(0, 16), (16, 4)] {
            let prepared = PreparedKeccak::new_slab(1, first, permutations).unwrap();
            let (packed, auxiliary, _) = prepared
                .generate_chain(&[initial_state(&[53; 40], &[59; 32])])
                .unwrap();
            let (commitment, _data) = commit(&packed, &params(prepared.bit_vars()));
            let mut pt = Blake3Transcript::new();
            let (proof, claims, marginals) = prepared
                .prove_prefix(
                    &packed,
                    auxiliary,
                    &commitment.root,
                    &[0x42; 32],
                    &mut pt,
                    136,
                )
                .unwrap();
            assert_eq!(proof.grinding_nonces.len(), prepared.bit_vars() + 8);
            for (claim, cached) in claims.iter().zip(&marginals) {
                let weights = crate::hybrid::sumcheck::eq_table(&claim.high_point);
                assert_eq!(
                    *cached,
                    crate::hybrid::sumcheck::bit_marginals(&packed, &weights, 1)
                );
            }
            let mut vt = Blake3Transcript::new();
            assert_eq!(
                prepared
                    .verify_prefix(&proof, &commitment.root, &[0x42; 32], &mut vt, 136)
                    .unwrap(),
                claims
            );
            assert_eq!(pt.get_challenge::<u128>(), vt.get_challenge::<u128>());

            let mut tampered = proof.clone();
            // The prover returns the smallest hit. The preceding nonce is
            // therefore deterministically invalid at this exact boundary.
            let nonce = tampered
                .grinding_nonces
                .iter_mut()
                .find(|n| **n > 0)
                .unwrap();
            *nonce -= 1;
            assert!(
                prepared
                    .verify_prefix(
                        &tampered,
                        &commitment.root,
                        &[0x42; 32],
                        &mut Blake3Transcript::new(),
                        136
                    )
                    .is_err()
            );
            let mut missing = proof.clone();
            missing.grinding_nonces.pop();
            assert!(
                prepared
                    .verify_prefix(
                        &missing,
                        &commitment.root,
                        &[0x42; 32],
                        &mut Blake3Transcript::new(),
                        136
                    )
                    .is_err()
            );
            let mut extra = proof.clone();
            extra.grinding_nonces.push(0);
            assert!(
                prepared
                    .verify_prefix(
                        &extra,
                        &commitment.root,
                        &[0x42; 32],
                        &mut Blake3Transcript::new(),
                        136
                    )
                    .is_err()
            );
        }
    }

    #[test]
    fn all_zero_permutations_cannot_bypass_constant_wire_pin() {
        for (first, permutations) in [(0, 16), (16, 4)] {
            let prepared = PreparedKeccak::new_slab(1, first, permutations).unwrap();
            let n = 1usize << prepared.packed_vars();
            // All zeros satisfy the homogeneous A(z)B(z)=z rows, but are not
            // valid Keccak instances: the affine constant must equal one.
            let packed = vec![Gf128::ZERO; n];
            let auxiliary = KeccakAuxiliary {
                a: vec![Gf128::ZERO; n],
                b: vec![Gf128::ZERO; n],
                lincheck: vec![0; 16 * n],
            };
            let (commitment, _data) = commit(&packed, &params(prepared.bit_vars()));
            let (proof, _, _) = prepared
                .prove_prefix(
                    &packed,
                    auxiliary,
                    &commitment.root,
                    &[0x42; 32],
                    &mut Blake3Transcript::new(),
                    108,
                )
                .unwrap();
            assert!(
                prepared
                    .verify_prefix(
                        &proof,
                        &commitment.root,
                        &[0x42; 32],
                        &mut Blake3Transcript::new(),
                        108
                    )
                    .is_err()
            );
        }
    }

    #[test]
    fn valid_dummy_keccak_in_inactive_slot_is_rejected_by_activity_pin() {
        let prepared = PreparedKeccak::new_slab(1, 16, 4).unwrap();
        assert_eq!(prepared.capacity(), 2);
        // Every block is a valid Keccak permutation with constant one; the
        // blocks assigned to signature 1 violate the public activity policy.
        let states = vec![[false; compact::STATE_BITS]; prepared.capacity() * 4];
        let (packed, a, b, lincheck) = compact::generate_witness_batch_major(
            &states,
            (prepared.capacity() * 4).ilog2() as usize,
        );
        let root = [0x81; 32];
        let context = [0x82; 32];
        let (proof, _, _) = prepared
            .prove_prefix(
                &packed,
                KeccakAuxiliary { a, b, lincheck },
                &root,
                &context,
                &mut Blake3Transcript::new(),
                108,
            )
            .unwrap();
        assert!(
            prepared
                .verify_prefix(&proof, &root, &context, &mut Blake3Transcript::new(), 108,)
                .is_err()
        );
    }
}

}

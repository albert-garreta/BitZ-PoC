//! Spartan's field-generic R1CS PIOP.
//!
//! This module deliberately treats a field as the pair `(F, F::Config)`.
//! That distinction is load-bearing for runtime-configured field types such as
//! [`MontyField`]: one Rust type can represent elements modulo many different
//! primes.  The prepared statement binds the configuration, while the helpers
//! in this module give every protocol phase one canonical element encoding and
//! one unbiased Fiat--Shamir challenge sampler.

pub mod baby_bear_f2z;
pub mod baby_bear_mul;
pub mod cm;
pub mod f2z;
pub mod grinding;
pub mod matrix;
pub mod multiswap;
pub mod opening_mode;
pub mod piop;
pub mod profile;
pub(crate) mod raw_monty;
pub mod sha256;
pub(crate) mod slot_rows;
pub(crate) mod spliced_digest;
pub mod sumcheck;
pub mod u32_mul;
pub mod univariate_skip;
pub(crate) mod univariate_skip_native;

pub use baby_bear_f2z::{
    BabyBearBitifiedClaim, BabyBearMulPaperProof, BabyBearMulSpartanF2zProof,
    BabyBearSpartanF2zError, PreparedBabyBearMulRelation, baby_bear_mul_instance_facts,
    bitify_baby_bear_mul_spartan_claim, commit_baby_bear_mul_witness, prove_baby_bear_mul_paper,
    prove_baby_bear_mul_spartan_and_f2z, prove_baby_bear_mul_spartan_and_f2z_from_witness,
    prove_baby_bear_mul_spartan_and_f2z_with_strategy, verify_baby_bear_mul_paper,
    verify_baby_bear_mul_spartan_and_f2z,
};
pub use baby_bear_mul::{
    BABY_BEAR_MODULUS, BabyBearMulCoefficient, BabyBearMulError, BabyBearMulLayout,
    BabyBearMulNativeMles, BabyBearMulRelationBackend, BabyBearMulWitness,
    baby_bear_mul_constraint_matrices, prepare_baby_bear_mul_relation,
    project_baby_bear_mul_native_witness, project_baby_bear_mul_witness,
    sample_baby_bear_operand_with,
};
pub use cm::{
    CM_AND_F_LIVE_SLOTS, CM_AND_H_SLOTS, CM_AND_WORD_BITS, CmAndError, CmAndLayout, CmAndWitness,
    CmF2zError, CmF2zProof, CmOpeningClaim, PreparedCmAndRelation, ProjectedCmAndWitness,
    bitify_cm_and_claim, cm_and_map, commit_cm_and_witness, commit_cm_and_witness_with_config,
    prepare_cm_and_relation, project_cm_and_witness, prove_cm_and_f2z,
    prove_cm_and_f2z_with_config, verify_cm_and_f2z, verify_cm_and_f2z_with_config,
};
pub use f2z::{
    PreparedU32MulRelation, SpartanF2zError, SpartanF2zField, U32_MUL_UNIVARIATE_SKIP_DEGREE,
    U32_MUL_UNIVARIATE_SKIP_VARS, U32MulProof, commit_u32_mul_witness, prove_u32_mul,
    spartan_f2z_field_config, verify_u32_mul,
};

pub use crate::sparse_matrix::SparseMatrixError;
pub use matrix::{
    ConstraintMatrices, ConstraintMatricesSkeleton, MleClaimError, ModulusIndependentCoefficient,
    PreparedConstraintMatrices, ScaledMleEvaluationClaim, SparseMatrix, SpartanMatrixCoefficient,
    SpartanMatrixError, build_assignment_mle, build_boolean_assignment_mle, build_product_mles,
    eq_eval, eq_table, make_equality_factors,
};
pub use opening_mode::{
    Direct, EvaluatedSpartanAssignment, OpeningMode, SpartanF2zProof, Virtualized,
};
pub use piop::{
    SPARTAN_ASSIGNMENT_ORACLE_DOMAIN, SPARTAN_PIOP_DOMAIN, SPARTAN_UNIVARIATE_SKIP_PIOP_DOMAIN,
    SpartanError, SpartanPiopProof, SpartanReductionStrategy, prove_spartan_nonsuccinct,
    prove_spartan_piop, prove_spartan_piop_u32_native,
    prove_spartan_piop_u32_native_with_univariate_skip, prove_spartan_piop_with_strategy,
    prove_spartan_piop_with_univariate_skip, verify_spartan_proof,
    verify_spartan_univariate_skip_proof, verify_spartan_with_mle_claim,
};
#[cfg(feature = "bench-internals")]
#[doc(hidden)]
pub use piop::{
    SpartanInnerFieldAccumulation, SpartanInnerNativeFold, SpartanInnerPolicy,
    prove_spartan_piop_u32_native_barrett_with_inner_policy,
};
pub use profile::{
    IopInstanceFacts, IopSecurityParams, IopSecurityProfile, Lambda100, Lambda128, Limber114,
    PrimePolicy, ProfileError, ReductionPrimeParams, Sha128ReferenceSchedule, SoundnessAccounting,
    SoundnessTerm,
};
pub use sha256::{
    PreparedSha256CompressionBatch, SHA256_COMMITMENT_FIELD_BITS, SHA256_CONSTRAINTS,
    SHA256_DEFAULT_INNER_PREFIX_VARS, SHA256_F_BAR_LIVE_BITS, SHA256_F_INSTANCE_BITS,
    SHA256_F_LIVE_BITS, SHA256_H_BAR_LIVE_BITS, SHA256_H_INSTANCE_BITS,
    SHA256_INNER_PREFIX_MAX_VARS, SHA256_MAX_LOG_COMPRESSIONS, SHA256_MIN_LOG_COMPRESSIONS,
    Sha256CompressionInput, Sha256CompressionProof, Sha256CompressionStatement,
    Sha256CompressionWitnessBatch, Sha256ConstraintError, Sha256F2zError, Sha256PrimeError,
    Sha256WitnessError, commit_sha256_compression_witness,
    commit_sha256_compression_witness_with_config, generate_sha256_compression_witnesses,
    prepare_sha256_compression_batch, prepare_sha256_compression_batch_for_assignment_rows,
    prepare_sha256_compression_batch_for_assignment_rows_with_profile,
    prepare_sha256_compression_batch_with_profile, prove_sha256_compressions,
    prove_sha256_compressions_with_config, prove_sha256_compressions_with_prefix_vars,
    prove_sha256_compressions_with_prefix_vars_and_config, sha256_compression_configs,
    verify_sha256_compressions, verify_sha256_compressions_with_config,
};
pub use sumcheck::{OuterSumcheckProof, R1csProductMles, SumcheckError, SumcheckProof};
pub use u32_mul::{
    U32_MUL_BIT_SLOTS, U32_MUL_PRODUCT_BITS, U32_MUL_X_BITS, U32_MUL_Y_BITS, U32MulError,
    U32MulF2zWidth, U32MulLayout, U32MulNativeMles, U32MulRelationBackend, U32MulWitness,
    prepare_u32_mul_relation, project_u32_mul_native_witness, u32_mul_constraint_matrices,
};
pub use univariate_skip::{
    UnivariateSkipOuterSumcheckProof, UnivariateSkipProof, UnivariateSkipSpartanPiopProof,
};

use std::slice;

use crypto_primes::{Flavor, is_prime};
use crypto_primitives::{
    ConstSemiring, PrimeField, crypto_bigint_monty::MontyField, crypto_bigint_uint::Uint,
};
use num_traits::ConstOne;
use thiserror::Error;

use crate::transcript::traits::{ConstTranscribable, GenTranscribable, Transcript};

const SPARTAN_TRANSCRIPT_FRAME_DOMAIN: &[u8] = b"f2z/spartan/transcript-frame/v1";
const FIELD_ELEMENTS_TAG: &[u8] = b"field-elements";

/// Minimum modulus size accepted by the Spartan PIOP.
///
/// This is the protocol's minimum accepted field-size security boundary;
/// adapters may select any prime modulus meeting it.
pub const SPARTAN_MIN_MODULUS_BITS: u32 = 100;

/// Why a runtime field configuration is unsafe for Spartan.
#[derive(Clone, Copy, Debug, Error, Eq, PartialEq)]
pub enum SpartanFieldError {
    /// The modulus is prime but below Spartan's minimum soundness boundary.
    #[error("Spartan requires at least a 100-bit modulus, got {actual_bits} bits")]
    ModulusTooSmall { actual_bits: u32 },

    /// Prime-field arithmetic is unsound when the runtime modulus is composite.
    #[error("the configured Spartan modulus is composite")]
    CompositeModulus,

    /// An unchecked constructor produced an invalid internal field residue.
    #[error("a Spartan field element has a noncanonical internal residue")]
    NonCanonicalElement,
}

/// Field operations required by the native Spartan protocol.
///
/// In addition to [`PrimeField`], an implementation fixes four protocol
/// details that `PrimeField` intentionally leaves unspecified:
///
/// - `validate_config` establishes that a runtime configuration is a genuine,
///   sufficiently large field before it reaches any protocol arithmetic.
/// - `canonical_element_encoding` is an injective, platform-independent
///   encoding of a reduced field element.  In particular, an implementation
///   must not expose an implementation-specific Montgomery residue.
/// - `canonical_modulus_encoding` identifies the configured field.  Protocol
///   statement digests must bind this encoding because `Self` alone need not
///   determine the modulus.
/// - `sample_uniform` maps transcript output to an *exactly uniform* field
///   element.  Reducing one fixed-width random integer modulo the field order
///   is generally biased and is not a valid implementation.
///
/// Rejection sampling is permitted.  Every rejected draw must still advance
/// the transcript, so prover and verifier consume the identical stream.
pub trait SpartanField: PrimeField {
    /// Checks that `field_cfg` defines a prime field with adequate soundness.
    ///
    /// [`PreparedConstraintMatrices::new`](matrix::PreparedConstraintMatrices::new)
    /// calls this once, before digesting coefficients or using the
    /// configuration in arithmetic.
    fn validate_config(field_cfg: &Self::Config) -> Result<(), SpartanFieldError>;

    /// Checks invariants that unchecked field-element constructors can bypass.
    fn validate_element(&self) -> Result<(), SpartanFieldError>;

    /// Canonical, injective encoding of this reduced element.
    fn canonical_element_encoding(&self) -> Vec<u8>;

    /// Canonical encoding of the modulus selected by `field_cfg`.
    fn canonical_modulus_encoding(field_cfg: &Self::Config) -> Vec<u8>;

    /// Fixed byte width shared by [`Self::canonical_element_encoding`] and
    /// [`Self::canonical_modulus_encoding`] under every configuration.
    fn canonical_encoding_width() -> usize;

    /// Draws an exactly uniform field element from `transcript`.
    ///
    /// This method advances the transcript for every raw draw, including
    /// rejected draws.  It does not absorb the accepted field element a second
    /// time; callers should normally use [`squeeze_field`], which performs the
    /// post-draw absorption required by this crate's transcript convention.
    fn sample_uniform<T: Transcript>(transcript: &mut T, field_cfg: &Self::Config) -> Self;
}

/// Type-level relation boundary connecting sparse coefficients, witness
/// entries, and matrix-product entries before they are folded into `F`.
pub trait SpartanRelationBackend<F>
where
    F: SpartanField,
{
    /// Coefficient type stored by the prepared sparse matrices.
    type MatrixCoeff: SpartanMatrixCoefficient<F>;
    /// Native value type stored by the assignment MLE.
    type Witness;
    /// Native value type stored by `Az`, `Bz`, and `Cz`.
    type Product;
}

impl<const LIMBS: usize> SpartanField for MontyField<LIMBS> {
    fn validate_config(field_cfg: &Self::Config) -> Result<(), SpartanFieldError> {
        let modulus = field_cfg.modulus().get();
        let actual_bits = modulus.bits();
        if actual_bits < SPARTAN_MIN_MODULUS_BITS {
            return Err(SpartanFieldError::ModulusTooSmall { actual_bits });
        }
        if !is_prime(Flavor::Any, &modulus) {
            return Err(SpartanFieldError::CompositeModulus);
        }
        Ok(())
    }

    fn validate_element(&self) -> Result<(), SpartanFieldError> {
        if self.inner() >= &Uint::new(self.cfg().modulus().get()) {
            return Err(SpartanFieldError::NonCanonicalElement);
        }
        Ok(())
    }

    fn canonical_element_encoding(&self) -> Vec<u8> {
        // `inner()` is a Montgomery residue and is therefore not the protocol
        // encoding. `retrieve()` returns the unique reduced standard residue.
        let canonical = self.retrieve();
        let mut encoding = vec![0; Uint::<LIMBS>::NUM_BYTES];
        canonical.write_transcription_bytes_exact(&mut encoding);
        encoding
    }

    fn canonical_modulus_encoding(field_cfg: &Self::Config) -> Vec<u8> {
        let modulus = Uint::new(field_cfg.modulus().get());
        let mut encoding = vec![0; Uint::<LIMBS>::NUM_BYTES];
        modulus.write_transcription_bytes_exact(&mut encoding);
        encoding
    }

    fn canonical_encoding_width() -> usize {
        Uint::<LIMBS>::NUM_BYTES
    }

    #[allow(clippy::arithmetic_side_effects)]
    fn sample_uniform<T: Transcript>(transcript: &mut T, field_cfg: &Self::Config) -> Self {
        let modulus = Uint::new(field_cfg.modulus().get());
        let max = <Uint<LIMBS> as ConstSemiring>::MAX;

        // A raw draw is uniform on [0, 2^N). Keep the largest prefix whose
        // cardinality is a multiple of the modulus. With M = 2^N - 1,
        //
        //   rejected = 2^N mod q = ((M mod q) + 1) mod q,
        //   accepted = [0, M - rejected].
        //
        // Every residue then has exactly floor(2^N / q) preimages.
        let rejection_size = ((max % modulus) + Uint::<LIMBS>::ONE) % modulus;
        let max_accepted = max - rejection_size;

        loop {
            let candidate = transcript.get_challenge::<Uint<LIMBS>>();
            if candidate <= max_accepted {
                return Self::new_with_cfg(candidate, field_cfg);
            }
        }
    }
}

/// Boolean matrices act by the field's one, whose canonical encoding is the
/// same fixed-width transcription of `1` under every configuration, so their
/// prepared statements can be cached modulus-independently.
impl<const LIMBS: usize> ModulusIndependentCoefficient<MontyField<LIMBS>> for bool {
    fn write_modulus_independent_encoding(&self, out: &mut Vec<u8>) {
        // Mirrors `canonical_element_encoding` of the field one: the
        // canonical residue written through the same fixed-width
        // transcription. Explicit zeros are rejected before encoding, but
        // stay total and correct here regardless.
        let value = if *self {
            Uint::<LIMBS>::ONE
        } else {
            <Uint<LIMBS> as num_traits::ConstZero>::ZERO
        };
        let start = out.len();
        out.resize(start + Uint::<LIMBS>::NUM_BYTES, 0);
        value.write_transcription_bytes_exact(&mut out[start..]);
    }

    fn is_unit(&self) -> bool {
        *self
    }
}

/// Absorbs field elements using [`SpartanField`]'s canonical encoding.
///
/// The collection count and every element length are encoded inside one typed,
/// length-prefixed Spartan frame. This is intentionally stronger than calling
/// [`Transcript::absorb_slice`] for each element: that legacy delimiter-only
/// framing is not injective for arbitrary byte strings.
pub(crate) fn absorb_field_elements<F, T>(transcript: &mut T, values: &[F])
where
    F: SpartanField,
    T: Transcript,
{
    let mut payload = Vec::new();
    extend_frame_len(&mut payload, values.len());
    for value in values {
        let encoding = value.canonical_element_encoding();
        extend_frame_len(&mut payload, encoding.len());
        payload.extend_from_slice(&encoding);
    }
    absorb_spartan_message(transcript, FIELD_ELEMENTS_TAG, &payload);
}

/// Absorbs one typed, self-delimiting Spartan transcript message.
pub(crate) fn absorb_spartan_message(transcript: &mut impl Transcript, tag: &[u8], payload: &[u8]) {
    let mut frame =
        Vec::with_capacity(SPARTAN_TRANSCRIPT_FRAME_DOMAIN.len() + tag.len() + payload.len() + 16);
    frame.extend_from_slice(SPARTAN_TRANSCRIPT_FRAME_DOMAIN);
    extend_frame_len(&mut frame, tag.len());
    frame.extend_from_slice(tag);
    extend_frame_len(&mut frame, payload.len());
    frame.extend_from_slice(payload);
    transcript.absorb_slice(&frame);
}

fn extend_frame_len(frame: &mut Vec<u8>, len: usize) {
    let len = u64::try_from(len).expect("an in-memory transcript message length fits u64");
    frame.extend_from_slice(&len.to_le_bytes());
}

/// Draws an unbiased field challenge and re-absorbs its canonical encoding.
///
/// Re-absorbing the accepted value is intentional: existing protocols in this
/// crate draw a challenge and then absorb that challenge before continuing.
/// Keeping the operation here prevents prover/verifier transcript schedules
/// from drifting apart.
pub(crate) fn squeeze_field<F, T>(transcript: &mut T, field_cfg: &F::Config) -> F
where
    F: SpartanField,
    T: Transcript,
{
    let challenge = F::sample_uniform(transcript, field_cfg);
    absorb_field_elements(transcript, slice::from_ref(&challenge));
    challenge
}

#[cfg(test)]
mod tests {
    use std::collections::VecDeque;

    use crypto_primitives::{
        ConstIntSemiring, FromWithConfig, PrimeField, crypto_bigint_monty::F128,
        crypto_bigint_uint::Uint,
    };

    use super::*;
    use crate::{transcript::traits::ConstTranscribable, utils::primality::PrimalityTest};

    struct ScriptedTranscript {
        draws: VecDeque<Vec<u8>>,
        draws_consumed: usize,
        absorbed: Vec<u8>,
    }

    impl ScriptedTranscript {
        fn new(draws: impl IntoIterator<Item = Uint<2>>) -> Self {
            Self {
                draws: draws
                    .into_iter()
                    .map(|draw| {
                        let mut bytes = vec![0; Uint::<2>::NUM_BYTES];
                        draw.write_transcription_bytes_exact(&mut bytes);
                        bytes
                    })
                    .collect(),
                draws_consumed: 0,
                absorbed: Vec::new(),
            }
        }
    }

    impl Transcript for ScriptedTranscript {
        fn get_challenge<T: ConstTranscribable>(&mut self) -> T {
            self.draws_consumed += 1;
            let bytes = self.draws.pop_front().expect("scripted challenge draw");
            assert_eq!(bytes.len(), T::NUM_BYTES);
            T::read_transcription_bytes_exact(&bytes)
        }

        fn get_prime<R: ConstIntSemiring + ConstTranscribable, T: PrimalityTest<R>>(
            &mut self,
        ) -> R {
            unreachable!("the rejection-sampler test does not request a prime")
        }

        fn absorb_inner(&mut self, value: &[u8]) {
            self.absorbed.extend_from_slice(value);
        }
    }

    #[test]
    fn uniform_sampler_rejects_the_incomplete_interval_and_reabsorbs_the_result() {
        // For q = 2^127 - 1, 2^128 mod q = 2. The two largest u128
        // candidates must therefore be rejected before reducing modulo q.
        let modulus = (1_u128 << 127) - 1;
        let field_cfg = F128::make_cfg(&Uint::from(modulus)).expect("Mersenne prime modulus");
        F128::validate_config(&field_cfg).unwrap();
        let max = <Uint<2> as ConstSemiring>::MAX;
        let accepted = Uint::from(42_u128);
        let mut transcript = ScriptedTranscript::new([max, max - Uint::<2>::ONE, accepted]);

        let challenge = squeeze_field::<F128, _>(&mut transcript, &field_cfg);

        assert_eq!(challenge, F128::from_with_cfg(42_u128, &field_cfg));
        assert_eq!(transcript.draws_consumed, 3);
        let encoding = challenge.canonical_element_encoding();
        let mut payload = Vec::new();
        payload.extend_from_slice(&1_u64.to_le_bytes());
        payload.extend_from_slice(&(encoding.len() as u64).to_le_bytes());
        payload.extend_from_slice(&encoding);
        let mut expected_absorption = vec![0x6];
        expected_absorption.extend_from_slice(SPARTAN_TRANSCRIPT_FRAME_DOMAIN);
        expected_absorption.extend_from_slice(&(FIELD_ELEMENTS_TAG.len() as u64).to_le_bytes());
        expected_absorption.extend_from_slice(FIELD_ELEMENTS_TAG);
        expected_absorption.extend_from_slice(&(payload.len() as u64).to_le_bytes());
        expected_absorption.extend_from_slice(&payload);
        expected_absorption.push(0x7);
        assert_eq!(transcript.absorbed, expected_absorption);
    }

    #[test]
    fn spartan_frames_distinguish_the_legacy_delimiter_collision() {
        let mut one_message = ScriptedTranscript::new([]);
        absorb_spartan_message(&mut one_message, b"test", &[0x7, 0x6]);

        let mut two_messages = ScriptedTranscript::new([]);
        absorb_spartan_message(&mut two_messages, b"test", &[]);
        absorb_spartan_message(&mut two_messages, b"test", &[]);

        assert_ne!(one_message.absorbed, two_messages.absorbed);
    }

    #[test]
    fn runtime_monty_config_must_be_prime_and_at_least_100_bits() {
        let q100 = F128::make_cfg(&Uint::from((1_u128 << 100) - 15)).unwrap();
        assert_eq!(F128::validate_config(&q100), Ok(()));

        let composite = F128::make_cfg(&Uint::from((1_u128 << 100) - 17)).unwrap();
        assert_eq!(
            F128::validate_config(&composite),
            Err(SpartanFieldError::CompositeModulus)
        );

        let undersized = F128::make_cfg(&Uint::from(97_u128)).unwrap();
        assert_eq!(
            F128::validate_config(&undersized),
            Err(SpartanFieldError::ModulusTooSmall { actual_bits: 7 })
        );
    }
}

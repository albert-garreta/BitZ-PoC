//! SHA-256 compression synthesis and its structured Spartan/BitZ adapter.

mod constraints;
pub(crate) mod inner_sumcheck;
mod prime;
mod proof;
mod witness;

pub use inner_sumcheck::SHA256_INNER_PREFIX_MAX_VARS;

#[cfg(feature = "bench-internals")]
#[doc(hidden)]
pub use constraints::prepare_sha256_compression_batch_for_product_t_fixed98;
pub use constraints::{
    PreparedSha256CompressionBatch, SHA256_CONSTRAINTS, SHA256_F_BAR_LIVE_BITS,
    SHA256_F_INSTANCE_BITS, SHA256_F_LIVE_BITS, SHA256_H_BAR_LIVE_BITS, SHA256_H_INSTANCE_BITS,
    Sha256ConstraintError, Sha256OpeningLayout, prepare_sha256_compression_batch,
    prepare_sha256_compression_batch_for_assignment_rows,
    prepare_sha256_compression_batch_for_assignment_rows_with_profile,
    prepare_sha256_compression_batch_with_profile,
    prepare_sha256_compression_batch_with_profile_and_layout,
};
#[cfg(feature = "bitz-parity")]
pub use constraints::{
    prepare_sha256_compression_batch_wfbitz_at, prepare_sha256_compression_batch_wfbitz_with_profile,
};
#[cfg(feature = "bitz-parity")]
pub use proof::{
    Sha256WfbitzProof, commit_sha256_compression_witness_wfbitz, prove_sha256_compressions_wfbitz,
    verify_sha256_compressions_wfbitz,
};
pub use prime::{
    SHA256_COMMITMENT_FIELD_BITS, SHA256_MAX_LOG_COMPRESSIONS, SHA256_MIN_LOG_COMPRESSIONS,
    Sha256PrimeError,
};
#[cfg(feature = "bench-internals")]
#[doc(hidden)]
pub use prime::{
    SHA256_FIXED_98_INITIAL_GRINDING_BITS, SHA256_FIXED_98_PRIME, SHA256_FIXED_98_PRIME_BITS,
    SHA256_FIXED_98_TERMINAL_GRINDING_BITS,
};
pub use proof::{
    SHA256_DEFAULT_INNER_PREFIX_VARS, commit_sha256_compression_witness,
    commit_sha256_compression_witness_with_config, prove_sha256_compressions,
    prove_sha256_compressions_with_config, prove_sha256_compressions_with_prefix_vars,
    prove_sha256_compressions_with_prefix_vars_and_config, sha256_compression_configs,
    verify_sha256_compressions, verify_sha256_compressions_with_config,
};
pub use witness::{
    Sha256CompressionInput, Sha256CompressionStatement, Sha256CompressionWitnessBatch,
    Sha256WitnessError, generate_sha256_compression_witnesses, sha256_compress,
};

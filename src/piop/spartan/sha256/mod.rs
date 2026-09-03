//! SHA-256 compression synthesis and its structured Spartan/F2Z adapter.

mod constraints;
pub(crate) mod inner_sumcheck;
mod prime;
mod proof;
mod witness;

pub use inner_sumcheck::SHA256_INNER_PREFIX_MAX_VARS;

pub use constraints::{
    PreparedSha256CompressionBatch, SHA256_CONSTRAINTS, SHA256_F_BAR_LIVE_BITS,
    SHA256_F_INSTANCE_BITS, SHA256_F_LIVE_BITS, SHA256_H_BAR_LIVE_BITS, SHA256_H_INSTANCE_BITS,
    Sha256ConstraintError, Sha256OpeningLayout, prepare_sha256_compression_batch,
    prepare_sha256_compression_batch_for_assignment_rows,
    prepare_sha256_compression_batch_for_assignment_rows_with_profile,
    prepare_sha256_compression_batch_with_profile,
    prepare_sha256_compression_batch_with_profile_and_layout,
};
pub use prime::{
    SHA256_COMMITMENT_FIELD_BITS, SHA256_MAX_LOG_COMPRESSIONS, SHA256_MIN_LOG_COMPRESSIONS,
    Sha256PrimeError,
};
pub use proof::{
    SHA256_DEFAULT_INNER_PREFIX_VARS, Sha256CompressionProof, Sha256F2zError,
    commit_sha256_compression_witness, commit_sha256_compression_witness_with_config,
    prove_sha256_compressions, prove_sha256_compressions_with_config,
    prove_sha256_compressions_with_prefix_vars,
    prove_sha256_compressions_with_prefix_vars_and_config, sha256_compression_configs,
    verify_sha256_compressions, verify_sha256_compressions_with_config,
};
pub use witness::{
    Sha256CompressionInput, Sha256CompressionStatement, Sha256CompressionWitnessBatch,
    Sha256WitnessError, generate_sha256_compression_witnesses,
};

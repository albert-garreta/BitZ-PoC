//! SHA-256 compression synthesis and its structured Spartan/F2Z adapter.

mod constraints;
mod prime;
mod proof;
mod witness;

pub use constraints::{
    PreparedSha256CompressionBatch, SHA256_CONSTRAINT_LOCAL_VARS, SHA256_CONSTRAINT_STRIDE,
    SHA256_CONSTRAINTS, SHA256_F_BAR_LIVE_BITS, SHA256_F_INSTANCE_BITS, SHA256_F_LIVE_BITS,
    SHA256_F_LOCAL_VARS, SHA256_F_STRIDE, SHA256_H_BAR_LIVE_BITS, SHA256_H_INSTANCE_BITS,
    SHA256_H_LOCAL_VARS, SHA256_H_STRIDE, Sha256ConstraintError, prepare_sha256_compression_batch,
    prepare_sha256_compression_batch_for_assignment_rows,
    prepare_sha256_compression_batch_for_assignment_rows_with_profile,
    prepare_sha256_compression_batch_with_profile,
};
pub use prime::{
    SHA256_COMMITMENT_FIELD_BITS, SHA256_MAX_LOG_COMPRESSIONS, SHA256_MIN_LOG_COMPRESSIONS,
    Sha256PrimeError,
};
pub use proof::{
    Sha256CompressionProof, Sha256F2zError, commit_sha256_compression_witness,
    commit_sha256_compression_witness_with_config, prove_sha256_compressions,
    prove_sha256_compressions_with_config, sha256_compression_configs, verify_sha256_compressions,
    verify_sha256_compressions_with_config,
};
pub use witness::{
    Sha256CompressionInput, Sha256CompressionStatement, Sha256CompressionWitnessBatch,
    Sha256WitnessError, generate_sha256_compression_witnesses,
};

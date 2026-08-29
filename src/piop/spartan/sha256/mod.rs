//! SHA-256 compression synthesis and its structured Spartan/F2Z adapter.

mod constraints;
mod prime;
mod proof;
mod witness;

pub use constraints::{
    prepare_sha256_compression_batch, prepare_sha256_compression_batch_integer,
    sha256_assignment_params, PreparedSha256CompressionBatch, Sha256ConstraintError,
    SHA256_CONSTRAINTS, SHA256_CONSTRAINT_LOCAL_VARS, SHA256_CONSTRAINT_STRIDE,
    SHA256_F_BAR_LIVE_BITS, SHA256_F_LIVE_BITS, SHA256_F_LOCAL_VARS, SHA256_F_STRIDE,
    SHA256_H_BAR_LIVE_BITS, SHA256_H_LOCAL_VARS, SHA256_H_STRIDE,
};
pub use prime::{
    sample_sha256_mod_q_context, sha256_prime_interval_bits, Sha256ModQContext, Sha256PrimeError,
    Sha256PrimeProfile, SHA256_COMMITMENT_FIELD_BITS, SHA256_MAX_LOG_COMPRESSIONS,
    SHA256_MIN_LOG_COMPRESSIONS,
};
pub use proof::{
    commit_sha256_compression_witness, commit_sha256_compression_witness_with_config,
    commit_sha256_paper128_witness, commit_sha256_paper128_witness_with_config,
    prove_sha256_compressions_paper128, prove_sha256_compressions_paper128_with_config,
    prove_sha256_compressions_spartan_and_f2z,
    prove_sha256_compressions_spartan_and_f2z_with_config, sha256_compression_configs,
    verify_sha256_compressions_paper128, verify_sha256_compressions_paper128_with_config,
    verify_sha256_compressions_spartan_and_f2z,
    verify_sha256_compressions_spartan_and_f2z_with_config, Sha256CompressionProof, Sha256F2zError,
    Sha256Paper128Proof,
};
pub use witness::{
    generate_sha256_compression_witnesses, generate_sha256_compression_witnesses_exact,
    ExactSha256CompressionWitnessBatch, Sha256CompressionInput, Sha256CompressionStatement,
    Sha256CompressionWitnessBatch, Sha256WitnessError,
};

//! SHA-256 compression synthesis and its structured Spartan/F2Z adapter.

mod constraints;
mod proof;
mod witness;

pub use constraints::{
    prepare_sha256_compression_batch, Sha256ConstraintError, SHA256_CONSTRAINTS,
    SHA256_CONSTRAINT_LOCAL_VARS, SHA256_CONSTRAINT_STRIDE, SHA256_F_BAR_LIVE_BITS,
    SHA256_F_LIVE_BITS, SHA256_F_LOCAL_VARS, SHA256_F_STRIDE, SHA256_H_BAR_LIVE_BITS,
    SHA256_H_LOCAL_VARS, SHA256_H_STRIDE,
};
pub use proof::{
    commit_sha256_compression_witness, commit_sha256_compression_witness_with_config,
    prove_sha256_compressions_spartan_and_f2z,
    prove_sha256_compressions_spartan_and_f2z_with_config, sha256_compression_configs,
    verify_sha256_compressions_spartan_and_f2z,
    verify_sha256_compressions_spartan_and_f2z_with_config, Sha256CompressionProof, Sha256F2zError,
};
pub use witness::{
    generate_sha256_compression_witnesses, Sha256CompressionInput, Sha256CompressionWitnessBatch,
    Sha256WitnessError,
};

//! Canonical witness identity shared by PCS-only and native full-proof benches.
use f2z::piop::spartan::{BabyBearMulWitness, U32MulWitness, U64MulWitness};
pub const U32_SEED: u64 = 0x5533_3250_4353_0064;
pub const BABY_BEAR_SEED: u64 = 0x4242_5043_5300_0064;
pub const U64_SEED: u64 = 0x5536_3450_4353_0064;
pub fn shape_seed(root: u64, exponent: usize) -> u64 {
    root ^ (exponent as u64).wrapping_mul(0x9e37_79b9_7f4a_7c15)
}
pub fn u32_digest(witness: &U32MulWitness) -> String {
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"f2z/u32-pcs-compare/integer-witness/v1");
    hasher.update(&(witness.assignment().len() as u64).to_le_bytes());
    for value in witness.assignment() {
        hasher.update(&value.to_le_bytes());
    }
    hasher.finalize().to_hex().to_string()
}

pub fn u64_digest(witness: &U64MulWitness) -> String {
    const CHUNK_VALUES: usize = 4096;
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"f2z/u64-mul-compare/integer-witness/v1");
    hasher.update(&(witness.assignment().len() as u64).to_le_bytes());
    let mut bytes = Vec::with_capacity(CHUNK_VALUES * std::mem::size_of::<u64>());
    for values in witness.assignment().chunks(CHUNK_VALUES) {
        bytes.clear();
        for value in values {
            bytes.extend_from_slice(&value.to_le_bytes());
        }
        hasher.update(&bytes);
    }
    hasher.finalize().to_hex().to_string()
}

pub fn baby_bear_digest(witness: &BabyBearMulWitness) -> String {
    const CHUNK_VALUES: usize = 4096;
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"f2z/baby-bear-pcs-compare/integer-witness/v1");
    hasher.update(&(witness.assignment().len() as u64).to_le_bytes());
    let mut bytes = Vec::with_capacity(CHUNK_VALUES * std::mem::size_of::<u64>());
    for values in witness.assignment().chunks(CHUNK_VALUES) {
        bytes.clear();
        for value in values {
            bytes.extend_from_slice(&value.to_le_bytes());
        }
        hasher.update(&bytes);
    }
    hasher.finalize().to_hex().to_string()
}

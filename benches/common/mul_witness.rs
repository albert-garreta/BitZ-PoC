//! Canonical witness identity shared by PCS-only and native full-proof benches.
use f2z::piop::spartan::{BabyBearMulWitness, U32MulWitness, U64MulWitness, U128MulWitness};
pub const U32_SEED: u64 = 0x5533_3250_4353_0064;
pub const BABY_BEAR_SEED: u64 = 0x4242_5043_5300_0064;
pub const U64_SEED: u64 = 0x5536_3450_4353_0064;
pub const U128_SEED: u64 = 0x5531_3238_5043_5364;
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

/// Digest of the canonical u128 assignment `[e0 | x | y | z]`: every entry
/// as 32 little-endian bytes (`x`, `y`, and the constant fit in the low 16;
/// `z` is the full 256-bit product).
pub fn u128_digest(witness: &U128MulWitness) -> String {
    const CHUNK_VALUES: usize = 2048;
    let layout = witness.layout();
    let capacity = layout.capacity();
    let live = layout.multiplications();
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"f2z/u128-mul-compare/integer-witness/v1");
    hasher.update(&(layout.assignment_len() as u64).to_le_bytes());
    let mut bytes = Vec::with_capacity(CHUNK_VALUES * 32);
    let push = |bytes: &mut Vec<u8>, lo: u128, hi: u128| {
        bytes.extend_from_slice(&lo.to_le_bytes());
        bytes.extend_from_slice(&hi.to_le_bytes());
    };
    let zero_pad = |bytes: &mut Vec<u8>, count: usize| bytes.extend(std::iter::repeat_n(0_u8, 32 * count));
    let flush = |hasher: &mut blake3::Hasher, bytes: &mut Vec<u8>| {
        hasher.update(bytes);
        bytes.clear();
    };
    // Block 0: the constant one, then padding.
    push(&mut bytes, 1, 0);
    zero_pad(&mut bytes, capacity - 1);
    flush(&mut hasher, &mut bytes);
    for values in [witness.x_values(), witness.y_values()] {
        for chunk in values[..live].chunks(CHUNK_VALUES) {
            for &value in chunk {
                push(&mut bytes, value, 0);
            }
            flush(&mut hasher, &mut bytes);
        }
        zero_pad(&mut bytes, capacity - live);
        flush(&mut hasher, &mut bytes);
    }
    for (lo, hi) in witness.z_lo_values()[..live]
        .chunks(CHUNK_VALUES)
        .zip(witness.z_hi_values()[..live].chunks(CHUNK_VALUES))
    {
        for (&lo, &hi) in lo.iter().zip(hi) {
            push(&mut bytes, lo, hi);
        }
        flush(&mut hasher, &mut bytes);
    }
    zero_pad(&mut bytes, capacity - live);
    flush(&mut hasher, &mut bytes);
    hasher.finalize().to_hex().to_string()
}

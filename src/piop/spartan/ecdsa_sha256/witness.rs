use circuit::{
    matrix_products::IntegerProducts,
    p256, sha256,
    witgen::{PackedWitness, ProductWitgen, Witgen},
};
use num_bigint::{BigInt, BigUint};
use num_traits::{One, Zero};
#[cfg(feature = "parallel")]
use rayon::prelude::*;
use std::array;

use super::{
    Result, error,
    relation::{P_INPUT_ALIAS, PreparedSha256Ecdsa, SHA_F, SHA_H, Sha256EcdsaStatement},
};
use crate::pcs::IntEvalParams;

/// Packed source and virtual assignment, with exact P-256 row operands.
pub struct Sha256EcdsaWitness {
    pub(crate) f_rows: Vec<Vec<u64>>,
    pub(crate) h_rows: Vec<Vec<u64>>,
    pub(crate) products: IntegerProducts,
    pub(crate) statement: Sha256EcdsaStatement,
}

impl Sha256EcdsaWitness {
    pub fn source_rows(&self) -> &[Vec<u64>] {
        &self.f_rows
    }
    pub fn assignment_rows(&self) -> &[Vec<u64>] {
        &self.h_rows
    }
    pub(crate) fn h_bit(&self, index: usize, p: &IntEvalParams) -> u64 {
        let row = index & (p.rows() - 1);
        (self.h_rows[index >> p.t][row / 64] >> (row % 64)) & 1
    }
}

pub(crate) fn inverse(value: &[u8; 32]) -> Result<[u8; 32]> {
    let modulus = BigUint::parse_bytes(
        b"ffffffff00000000ffffffffffffffffbce6faada7179e84f3b9cac2fc632551",
        16,
    )
    .unwrap();
    let n = BigUint::from_bytes_be(value);
    if n.is_zero() || n >= modulus {
        return Err(error("signature scalar is not in 1..n"));
    }
    let modulus = BigInt::from(modulus);
    let (mut r, mut next_r) = (modulus.clone(), BigInt::from(n));
    let (mut t, mut next_t) = (BigInt::zero(), BigInt::one());
    while !next_r.is_zero() {
        let q = &r / &next_r;
        (r, next_r) = (next_r.clone(), r - &q * next_r);
        (t, next_t) = (next_t.clone(), t - q * next_t);
    }
    if r != BigInt::one() {
        return Err(error("noninvertible signature scalar"));
    }
    let t = ((t % &modulus) + &modulus) % modulus;
    let bytes = t.to_biguint().unwrap().to_bytes_be();
    let mut out = [0; 32];
    out[32 - bytes.len()..].copy_from_slice(&bytes);
    Ok(out)
}

fn pack_bits(p: &IntEvalParams, live: usize, bit: impl Fn(usize) -> bool + Sync) -> Vec<Vec<u64>> {
    let column = |c: usize| {
        (0..p.rows().div_ceil(64))
            .map(|word| {
                let mut value = 0u64;
                for b in 0..64 {
                    let row = word * 64 + b;
                    let index = (c << p.t) + row;
                    if row < p.rows() && index < live && bit(index) {
                        value |= 1 << b;
                    }
                }
                value
            })
            .collect()
    };
    #[cfg(feature = "parallel")]
    {
        (0..p.cols()).into_par_iter().map(column).collect()
    }
    #[cfg(not(feature = "parallel"))]
    {
        (0..p.cols()).map(column).collect()
    }
}

/// Computes the SHA trace and all hint values. Signing is not part of this API.
pub fn generate_sha256_ecdsa_witness(
    prepared: &PreparedSha256Ecdsa,
    statement: &Sha256EcdsaStatement,
    message: &[u8],
) -> Result<Sha256EcdsaWitness> {
    if statement.log_compressions as usize != prepared.log_n
        || message.len() != prepared.message_bytes()
    {
        return Err(error(
            "message length or compression count differs from the prepared relation",
        ));
    }
    let blocks: Vec<[u32; 16]> = message
        .chunks_exact(64)
        .map(|bytes| {
            array::from_fn(|w| u32::from_be_bytes(bytes[w * 4..w * 4 + 4].try_into().unwrap()))
        })
        .chain(std::iter::once(prepared.padding()))
        .collect();
    let mut states = Vec::with_capacity(blocks.len() + 1);
    states.push(sha256::INITIAL_STATE);
    for block in &blocks {
        states.push(super::super::sha256::sha256_compress(
            *states.last().unwrap(),
            *block,
        ));
    }
    let generate = |i: usize| -> Result<(PackedWitness, PackedWitness)> {
        let input: [bool; sha256::COMPRESSION_INPUT_BITS] = array::from_fn(|bit| {
            if bit < 512 {
                blocks[i][bit / 32] >> (bit % 32) & 1 != 0
            } else {
                states[i][(bit - 512) / 32] >> (bit % 32) & 1 != 0
            }
        });
        let mut generator = Witgen::with_inputs_and_capacity(
            &input,
            sha256::COMPRESSION_INPUT_BITS + sha256::COMPRESSION_HINT_BITS,
        );
        let output = sha256::compression_circuit(&mut generator, &input);
        for (bit, value) in output.into_iter().enumerate() {
            if value != (states[i + 1][bit / 32] >> (bit % 32) & 1 != 0) {
                return Err(error("SHA circuit/native digest mismatch"));
            }
        }
        let pair = generator.into_witnesses();
        if pair.1.bit_len() != SHA_H {
            return Err(error("unexpected SHA assignment width"));
        }
        Ok(pair)
    };
    #[cfg(feature = "parallel")]
    let shards: Vec<_> = (0..blocks.len())
        .into_par_iter()
        .map(generate)
        .collect::<Result<_>>()?;
    #[cfg(not(feature = "parallel"))]
    let shards: Vec<_> = (0..blocks.len()).map(generate).collect::<Result<_>>()?;

    let digest: [u8; 32] =
        array::from_fn(|byte| states.last().unwrap()[byte / 4].to_be_bytes()[byte % 4]);
    let rinv = inverse(&statement.r)?;
    let sinv = inverse(&statement.s)?;
    let words = [
        &digest,
        &statement.qx,
        &statement.qy,
        &statement.r,
        &statement.s,
        &rinv,
        &sinv,
    ];
    let input: [bool; p256::VERIFY_DIGEST_INPUT_BITS] =
        array::from_fn(|bit| words[bit / 256][31 - (bit % 256) / 8] >> (bit % 8) & 1 != 0);
    let mut generator =
        ProductWitgen::with_inputs_and_capacity(&input, p256::VERIFY_DIGEST_WITNESS_BITS);
    p256::verify_digest_circuit(&mut generator, &input);
    let (p_f, p_h, products) = generator.into_parts();
    if p_h.bit_len() != prepared.local.p_map.rows()
        || p_f.bit_len() + 1 != prepared.local.p_map.cols()
    {
        return Err(error("unexpected P-256 witness width"));
    }
    let f_rows = pack_bits(&prepared.p_f, prepared.live_source_bits(), |index| {
        if index == 0 {
            true
        } else if index < prepared.map.f_offset {
            let offset = index - 1;
            let bit = offset % SHA_F;
            shards[offset / SHA_F]
                .0
                .bit(if bit < 512 { bit } else { bit + 256 })
        } else {
            p_f.bit(index - prepared.map.f_offset + P_INPUT_ALIAS - 1)
        }
    });
    let h_rows = pack_bits(&prepared.p_h, prepared.live_assignment_bits(), |index| {
        if index < prepared.map.h_offset {
            shards[index % prepared.compressions()]
                .1
                .bit(index / prepared.compressions())
        } else {
            p_h.bit(index - prepared.map.h_offset)
        }
    });
    Ok(Sha256EcdsaWitness {
        f_rows,
        h_rows,
        products,
        statement: statement.clone(),
    })
}

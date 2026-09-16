#[cfg(test)]
use crate::piop::spartan::SpartanField as _;
#[cfg(test)]
use crate::piop::spartan::protocol::{FieldConfig as Config, SpartanF2zField as F};
use circuit::{
    integer_storage::IntegerTable,
    matrix_products::IntegerProducts,
    p256, sha256,
    witgen::{PackedWitness, ProductWitgen, Witgen},
};
#[cfg(test)]
use num_bigint::BigInt;
#[cfg(feature = "parallel")]
use rayon::prelude::*;
use std::array;

use super::{
    Result, error,
    relation::{OuterMode, P_INPUT_ALIAS, PreparedSha256Ecdsa, SHA_F, SHA_H, Sha256EcdsaStatement},
};
use crate::{
    pcs::IntegerMatrixLayout,
    piop::spartan::raw_monty::{Raw, RawProducts},
};

/// Packed source and virtual assignment, with exact P-256 row operands.
pub struct Sha256EcdsaWitness {
    pub(crate) f_rows: Vec<Vec<u64>>,
    pub(crate) h_rows: Vec<Vec<u64>>,
    pub(crate) products: IntegerProducts,
    pub(crate) statement: Sha256EcdsaStatement,
}

impl Sha256EcdsaWitness {
    /// Raw residue tables of `(A h) mod q`, `(B h) mod q`, and `(C h) mod q`
    /// over the outer sumcheck's row space: the exact two's-complement row
    /// products reduced natively (Horner over their words), rows in
    /// parallel. Residue-for-residue the tables of
    /// [`Self::build_outer_product_mles`].
    pub(super) fn build_outer_raw_products(
        &self,
        prepared: &PreparedSha256Ecdsa,
        ctx: &field::FpCtx<2>,
    ) -> RawProducts {
        let len = 1usize << prepared.outer_sumcheck_num_vars();
        let products = [
            &self.products.a_mw,
            &self.products.b_mw,
            &self.products.c_mw,
        ];
        let max_words = products
            .iter()
            .map(|values| values.max_limbs())
            .max()
            .unwrap_or(0);
        let field = field::create_prime_field(field::Uint::from_words(*ctx.modulus().as_words()));
        let projection = field::PreparedSignedProjection::new(field, max_words);
        // Split mode packs the nonlinear rows at the front; all-row mode
        // places every local row after the (identically zero) SHA rows.
        let (offset, rows): (usize, Option<&[usize]>) = match prepared.mode {
            OuterMode::Split => (0, Some(&prepared.local.nonlinear)),
            OuterMode::AllRows => (256 * prepared.compressions(), None),
        };
        let count = rows.map_or(prepared.local.rows(), <[usize]>::len);
        let convert = |values: &IntegerTable| -> Vec<Raw> {
            let values = values.view();
            let residue = |i: usize| {
                let source = rows.map_or(i, |rows| rows[i]);
                {
                    let value = projection.project(&values[source]);
                    let words = value.as_montgomery_integer().as_words();
                    Raw::from(words[0]) | (Raw::from(words[1]) << 64)
                }
            };
            let mut table = vec![0 as Raw; len];
            #[cfg(feature = "parallel")]
            table[offset..offset + count]
                .par_iter_mut()
                .with_min_len(256)
                .enumerate()
                .for_each(|(i, slot)| *slot = residue(i));
            #[cfg(not(feature = "parallel"))]
            for (i, slot) in table[offset..offset + count].iter_mut().enumerate() {
                *slot = residue(i);
            }
            table
        };
        let [az, bz, cz] = products.map(|values| convert(values));
        RawProducts { az, bz, cz }
    }

    /// MLE tables of `(A h) mod q`, `(B h) mod q`, and `(C h) mod q`: the
    /// reference (BigInt) form of [`Self::build_outer_raw_products`].
    #[cfg(test)]
    pub(super) fn build_outer_product_mles(
        &self,
        prepared: &PreparedSha256Ecdsa,
        q: u128,
        cfg: &Config,
    ) -> crate::piop::spartan::sumcheck::R1csProductMles<F> {
        use crate::poly::mle::DenseMultilinearExtension;
        let vars = prepared.outer_sumcheck_num_vars();
        let zero = F::zero_with_cfg(cfg);
        let mut tables = std::array::from_fn::<_, 3, _>(|_| vec![zero.clone(); 1 << vars]);
        let products = [
            &self.products.a_mw,
            &self.products.b_mw,
            &self.products.c_mw,
        ];
        for (table, products) in tables.iter_mut().zip(products) {
            let mut set = |dst: usize, src: usize| {
                let bytes: Vec<_> = products[src].iter().flat_map(|w| w.to_le_bytes()).collect();
                table[dst] =
                    super::reduce_integer_mod_q(&BigInt::from_signed_bytes_le(&bytes), q, cfg);
            };
            match prepared.mode {
                OuterMode::Split => {
                    for (i, &r) in prepared.local.nonlinear.iter().enumerate() {
                        set(i, r);
                    }
                }
                OuterMode::AllRows => {
                    for r in 0..prepared.local.rows() {
                        set(256 * prepared.compressions() + r, r);
                    }
                }
            }
        }
        // Honest SHA row products are identically zero. These are prover
        // claims, not a verifier assumption: the shared inner check binds the
        // zero C claims to the committed SHA assignment. Reuse them instead of
        // charging the comparison mode for a second SHA matrix multiplication.
        let [a, b, c] = tables.map(|evaluations| DenseMultilinearExtension {
            evaluations,
            num_vars: vars,
        });
        crate::piop::spartan::sumcheck::R1csProductMles {
            az: a,
            bz: b,
            cz: c,
        }
    }

    pub fn source_rows(&self) -> &[Vec<u64>] {
        &self.f_rows
    }
    pub fn assignment_rows(&self) -> &[Vec<u64>] {
        &self.h_rows
    }
    pub(crate) fn h_bit(&self, index: usize, p: &IntegerMatrixLayout) -> u64 {
        let row = index & (p.rows() - 1);
        (self.h_rows[index >> p.row_vars][row / 64] >> (row % 64)) & 1
    }
}

pub(crate) fn inverse(value: &[u8; 32]) -> Result<[u8; 32]> {
    use field::{CanonicalCodec, IntegerOps, Uint};
    let mut bytes = *value;
    bytes.reverse();
    let scalar: Uint<4> = IntegerOps
        .decode_public(&bytes)
        .expect("fixed-width scalar encoding");
    let inverse = p256::scalar_inverse_ct(&scalar);
    if !inverse.validity().declassify() {
        return Err(error("signature scalar is not in 1..n"));
    }
    IntegerOps.encode_into(inverse.value(), &mut bytes);
    bytes.reverse();
    Ok(bytes)
}

fn pack_bits(
    p: &IntegerMatrixLayout,
    live: usize,
    bit: impl Fn(usize) -> bool + Sync,
) -> Vec<Vec<u64>> {
    let column = |c: usize| {
        (0..p.rows().div_ceil(64))
            .map(|word| {
                let mut value = 0u64;
                for b in 0..64 {
                    let row = word * 64 + b;
                    let index = (c << p.row_vars) + row;
                    if row < p.rows() && index < live {
                        value |= u64::from(bit(index)) << b;
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

/// Copies `len` bits of little-endian packed `src` starting at bit `src_off`
/// into `dst` starting at bit `dst_off`; the destination bits must be zero.
fn copy_bits(dst: &mut [u64], dst_off: usize, src: &[u64], src_off: usize, len: usize) {
    let mut done = 0;
    while done < len {
        let (s, d) = (src_off + done, dst_off + done);
        let take = (64 - d % 64).min(len - done);
        let shift = s % 64;
        let mut value = src[s / 64] >> shift;
        // Assemble a whole destination word even when the source is unaligned.
        // Read the next source word only when the requested bits cross into it.
        if take > 64 - shift {
            value |= src[s / 64 + 1] << (64 - shift);
        }
        let mask = u64::MAX >> (64 - take);
        dst[d / 64] |= (value & mask) << (d % 64);
        done += take;
    }
}

/// In-place transpose of a 64×64 bit matrix: afterwards bit `r` of word `c`
/// is what bit `c` of word `r` was.
fn transpose64(a: &mut [u64; 64]) {
    let (mut j, mut m) = (32usize, 0x0000_0000_FFFF_FFFFu64);
    while j != 0 {
        let mut k = 0;
        while k < 64 {
            let t = ((a[k] >> j) ^ a[k + j]) & m;
            a[k + j] ^= t;
            a[k] ^= t << j;
            k = (k + j + 1) & !j;
        }
        j >>= 1;
        m ^= m << j;
    }
}

/// The packed words of `witness` with the bits past its length cleared.
fn masked_word(witness: &PackedWitness, index: usize) -> u64 {
    let word = witness.words()[index];
    let valid = witness.bit_len() - 64 * index;
    if valid >= 64 {
        word
    } else {
        word & ((1u64 << valid) - 1)
    }
}

/// Splits the flat packed cells `(c << t) + row` into the per-column rows.
fn split_columns(flat: Vec<u64>, p: &IntegerMatrixLayout) -> Vec<Vec<u64>> {
    let words = p.rows() / 64;
    #[cfg(feature = "parallel")]
    {
        flat.par_chunks(words).map(<[u64]>::to_vec).collect()
    }
    #[cfg(not(feature = "parallel"))]
    {
        flat.chunks(words).map(<[u64]>::to_vec).collect()
    }
}

/// The source rows: `f[0] = 1`, then each compression's block and hint bits,
/// then the P-256 source bits after its aliased inputs — word blits, no
/// per-bit closure.
fn pack_source(
    prepared: &PreparedSha256Ecdsa,
    shards: &[(PackedWitness, PackedWitness)],
    p_f: &PackedWitness,
) -> Vec<Vec<u64>> {
    let p = &prepared.f_layout;
    let mut flat = vec![0u64; p.cols() * p.rows() / 64];
    flat[0] = 1;
    for (instance, shard) in shards.iter().enumerate() {
        let dst = 1 + instance * SHA_F;
        copy_bits(&mut flat, dst, shard.0.words(), 0, 512);
        copy_bits(&mut flat, dst + 512, shard.0.words(), 768, SHA_F - 512);
    }
    copy_bits(
        &mut flat,
        prepared.map.f_offset,
        p_f.words(),
        P_INPUT_ALIAS - 1,
        p_f.bit_len() - (P_INPUT_ALIAS - 1),
    );
    split_columns(flat, p)
}

/// The assignment rows `h[instance + N·local]`: every 64 consecutive cells are
/// one local wire across 64 compressions, so each 64×64 tile of (compression,
/// local) witness words is one bit-matrix transpose; the P-256 tail is a word
/// copy. Batches below 64 compressions keep the per-bit path.
fn pack_assignment(
    prepared: &PreparedSha256Ecdsa,
    shards: &[(PackedWitness, PackedWitness)],
    p_h: &PackedWitness,
) -> Vec<Vec<u64>> {
    let p = &prepared.h_layout;
    let n = prepared.compressions();
    if n < 64 {
        return pack_bits(p, prepared.live_assignment_bits(), |index| {
            if index < prepared.map.h_offset {
                shards[index % n].1.bit(index / n)
            } else {
                p_h.bit(index - prepared.map.h_offset)
            }
        });
    }
    let mut flat = vec![0u64; p.cols() * p.rows() / 64];
    let instance_blocks = n / 64;
    let (sha, tail) = flat.split_at_mut(prepared.map.h_offset / 64);
    // Local block `lb` (locals 64·lb ..) owns the contiguous words
    // [64·lb·instance_blocks, 64·(lb+1)·instance_blocks).
    let fill = |(lb, chunk): (usize, &mut [u64])| {
        let locals = chunk.len() / instance_blocks;
        let mut tile = [0u64; 64];
        for block in 0..instance_blocks {
            for (i, word) in tile.iter_mut().enumerate() {
                *word = masked_word(&shards[64 * block + i].1, lb);
            }
            transpose64(&mut tile);
            for (local, word) in tile[..locals].iter().enumerate() {
                chunk[local * instance_blocks + block] = *word;
            }
        }
    };
    #[cfg(feature = "parallel")]
    sha.par_chunks_mut(64 * instance_blocks)
        .enumerate()
        .for_each(fill);
    #[cfg(not(feature = "parallel"))]
    sha.chunks_mut(64 * instance_blocks)
        .enumerate()
        .for_each(fill);
    for (index, word) in tail.iter_mut().enumerate().take(p_h.words().len()) {
        *word = masked_word(p_h, index);
    }
    split_columns(flat, p)
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
    let f_rows = pack_source(prepared, &shards, &p_f);
    let h_rows = pack_assignment(prepared, &shards, &p_h);
    Ok(Sha256EcdsaWitness {
        f_rows,
        h_rows,
        products,
        statement: statement.clone(),
    })
}

#[cfg(test)]
mod packing_tests {
    use super::copy_bits;

    #[test]
    fn unaligned_copies_match_individual_bits() {
        copy_bits(&mut [], 0, &[], 0, 0);
        for pattern in 0..3 {
            let source: Vec<u64> = (0..20)
                .map(|i| match pattern {
                    0 => 0,
                    1 => u64::MAX,
                    _ => (i as u64).wrapping_mul(0x9e37_79b9_7f4a_7c15),
                })
                .collect();
            for src_off in 64usize..128 {
                for dst_off in 0usize..64 {
                    for len in [0, 1, 2, 31, 63, 64, 65, 127, 128, 129, 191, 1024] {
                        // Preserve nonzero bits outside the copy and provide
                        // exactly the source storage needed by this range.
                        let mut actual = vec![0xa5a5_a5a5_a5a5_a5a5; 20];
                        for bit in dst_off..dst_off + len {
                            actual[bit / 64] &= !(1u64 << (bit % 64));
                        }
                        let mut expected = actual.clone();
                        for bit in 0..len {
                            let value =
                                (source[(src_off + bit) / 64] >> ((src_off + bit) % 64)) & 1;
                            expected[(dst_off + bit) / 64] |= value << ((dst_off + bit) % 64);
                        }
                        copy_bits(
                            &mut actual,
                            dst_off,
                            &source[..(src_off + len).div_ceil(64)],
                            src_off,
                            len,
                        );
                        assert_eq!(
                            actual, expected,
                            "source {src_off}, destination {dst_off}, length {len}"
                        );
                    }
                }
            }
        }
    }
}

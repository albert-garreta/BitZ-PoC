//! SHA-256 compression witness generation.
//!
//! One [`ProductWitgen`] execution produces the Boolean source `f`, the
//! synthesized integer assignment `h_bar`, and exact `A h_bar`, `B h_bar`,
//! and `C h_bar` values. Batch packing only transposes these already-generated
//! values into the row layout consumed by F2Z; it never evaluates `M f`.

use std::array;

use circuit::{
    matrix_products::{IntegerProducts, MatrixProducts, RuntimeModulus},
    sha256::{compression_circuit, COMPRESSION_HINT_BITS, COMPRESSION_INPUT_BITS},
    witgen::{PackedWitness, ProductWitgen},
};
use crypto_primitives::{crypto_bigint_monty::F128, crypto_bigint_uint::Uint, PrimeField};
use num_bigint::BigUint;
use thiserror::Error;

#[cfg(feature = "parallel")]
use rayon::prelude::*;

use crate::{
    pcs::IntEvalParams,
    piop::spartan::{sumcheck::R1csProductMles, SpartanField},
    poly::mle::DenseMultilinearExtension,
};

use super::constraints::{
    SHA256_CONSTRAINTS, SHA256_CONSTRAINT_LOCAL_VARS, SHA256_CONSTRAINT_STRIDE,
    SHA256_F_BAR_LIVE_BITS, SHA256_F_INSTANCE_BITS, SHA256_F_LIVE_BITS,
    SHA256_H_BAR_LIVE_BITS, SHA256_H_INSTANCE_BITS,
};

/// One independent SHA-256 compression input: chaining state and message block.
pub type Sha256CompressionInput = ([u32; 8], [u32; 16]);

/// Public claim for one SHA-256 compression.
///
/// Let `R = Z / 2^32 Z` be the ring of 32-bit words. This statement is a
/// tuple `(H, M, H_hat) in R^8 x R^16 x R^8` asserting
///
/// `H_hat = Compress_SHA256(H, M)`.
///
/// If `V^(64) = (a, b, c, d, e, f, g, h)` is the working state after the 64
/// SHA-256 rounds, this is equivalently the component-wise relation
/// `H_hat = H + V^(64) mod 2^32`.
///
/// This statement covers one compression invocation only: `M` is an
/// already-parsed 512-bit message block, so no message padding or byte-to-word
/// parsing occurs here. Each word `x` is decomposed inside the circuit as
/// `x = sum_{b=0}^{31} x_b 2^b`, with Boolean bits `x_b` numbered
/// least-significant first. The canonical public order is `H`, `M`, `H_hat`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Sha256CompressionStatement {
    /// Initial chaining state `H = (H_0, ..., H_7) in R^8`.
    ///
    /// These words initialize the SHA-256 working state:
    /// `(a_0, b_0, c_0, d_0, e_0, f_0, g_0, h_0) = H`.
    pub state: [u32; 8],

    /// One 512-bit message block `M = (M_0, ..., M_15) in R^16`.
    ///
    /// The block supplies the first sixteen words of the 64-word message
    /// schedule: `W_j = M_j` for `0 <= j < 16`. For `16 <= j < 64`,
    ///
    /// `W_j = sigma_1(W_{j-2}) + W_{j-7}`
    /// `    + sigma_0(W_{j-15}) + W_{j-16} mod 2^32`.
    ///
    /// Thus `block[j]` represents `M_j = W_j` only for `0 <= j < 16`; the
    /// remaining schedule words are derived by the compression function.
    pub block: [u32; 16],

    /// Claimed post-compression state
    /// `H_hat = (H_hat_0, ..., H_hat_7) in R^8`.
    ///
    /// A valid statement satisfies
    /// `H_hat_j = H_j + V_j^(64) mod 2^32` for every `0 <= j < 8`. This is the
    /// next chaining state after one block, not necessarily a complete
    /// SHA-256 message digest.
    pub claimed_output: [u32; 8],
}

impl Sha256CompressionStatement {
    /// Builds a public compression statement from the circuit input and its
    /// claimed output.
    pub const fn new((state, block): Sha256CompressionInput, claimed_output: [u32; 8]) -> Self {
        Self {
            state,
            block,
            claimed_output,
        }
    }

    /// Iterates over the canonical transcript order: state, block, output.
    pub(crate) fn words(&self) -> impl Iterator<Item = u32> + '_ {
        self.state
            .iter()
            .chain(&self.block)
            .chain(&self.claimed_output)
            .copied()
    }
}

/// Tuple returned by [`generate_sha256_compression_witnesses`]: committed
/// source rows, synthesized assignment rows, Spartan products, and native
/// compression outputs. This remains a tuple so callers can immediately move
/// each component to its one consumer without retaining a wrapper object.
pub type Sha256CompressionWitnessBatch = (
    Vec<Vec<u64>>,
    Vec<Vec<u64>>,
    R1csProductMles<F128>,
    Vec<[u32; 8]>,
);

/// A complete q-independent SHA-256 witness batch.
///
/// Boolean source and assignment rows are packed immediately, but the three
/// R1CS matrix products remain exact signed integers. The caller can therefore
/// commit to [`Self::source_rows`] before a transcript-selected prime is known,
/// then invoke [`Self::project_products`] under that prime without replaying
/// the SHA circuit.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExactSha256CompressionWitnessBatch {
    /// Bit-sliced storage of the padded source table
    /// `F_tilde in F_2^{N x SHA256_F_STRIDE}`, where `N = instances`.
    ///
    /// Row `F_tilde[i, :]` is `f_tilde_i = ((1, f_i), 0, ..., 0)` for
    /// compression `i`: coordinate zero is the constant one, the next
    /// [`SHA256_F_LIVE_BITS`] coordinates contain the generated Boolean source
    /// `f_i`, and the remaining coordinates are zero padding. Storage is
    /// transposed and packed across instances:
    ///
    /// `F_tilde[i, j] = (source_rows[j][i / 64] >> (i % 64)) & 1`.
    ///
    /// Thus the outer length is [`SHA256_F_STRIDE`], while every inner vector
    /// contains `ceil(N / 64)` machine words. Unused lanes in the final word
    /// are zero.
    source_rows: Vec<Vec<u64>>,

    /// Bit-sliced storage of the padded assignment table
    /// `H_tilde in F_2^{N x SHA256_H_STRIDE}`.
    ///
    /// For compression `i`, let `f_tilde_i = F_tilde[i, :]^T` and
    /// `h_tilde_i = H_tilde[i, :]^T`. The padded Boolean assignment satisfies
    /// `h_tilde_i = M_pad f_tilde_i` over `F_2`, where `M_pad` is the
    /// zero-extended public witness map. Its first
    /// [`SHA256_H_BAR_LIVE_BITS`] coordinates are the augmented live assignment
    /// `h_bar_i = (1, h_i)`; all remaining coordinates are zero. It uses the
    /// same transposed packing as `source_rows`:
    ///
    /// `H_tilde[i, j] = (assignment_rows[j][i / 64] >> (i % 64)) & 1`.
    ///
    /// Although these values are stored as bits, the R1CS computation embeds
    /// each one canonically as the integer `0` or `1`.
    assignment_rows: Vec<Vec<u64>>,

    /// The three exact integer matrix-vector products for the live R1CS rows.
    ///
    /// If `h_bar_i in {0, 1}^SHA256_H_BAR_LIVE_BITS` is the augmented live
    /// assignment for instance `i`, canonically embedded into `Z`, then every
    /// live constraint `r` stores
    ///
    /// `a[i, r] = sum_j A_live[r, j] h_bar_i[j]`,
    /// `b[i, r] = sum_j B_live[r, j] h_bar_i[j]`, and
    /// `c[i, r] = sum_j C_live[r, j] h_bar_i[j]`
    ///
    /// as signed elements of `Z`, so a satisfying row obeys
    /// `a[i, r] * b[i, r] = c[i, r]` over the integers. Each component is
    /// flattened in instance-major order at
    /// `i * SHA256_CONSTRAINTS + r`. No reduction modulo `q` and no padding
    /// to [`SHA256_CONSTRAINT_STRIDE`] have happened yet.
    exact_products: IntegerProducts,

    /// Native compression results `O_i in (Z / 2^32 Z)^8`, in batch-instance
    /// and SHA-256 state-word order.
    ///
    /// Each `u32` is the canonical representative of one output word:
    /// `outputs[i][w] = sum_{b=0}^{31} output_bit[i, w, b] * 2^b`, with bits
    /// numbered least-significant first inside the word.
    outputs: Vec<[u32; 8]>,

    /// Batch cardinality `N = 2^t > 0`.
    ///
    /// This equals `outputs.len()` and the logical instance dimension of both
    /// F2Z tables. It also determines their `ceil(N / 64)` packed-word width
    /// and partitions each exact product vector into `N` blocks of
    /// [`SHA256_CONSTRAINTS`] live rows before projection pads each block to
    /// [`SHA256_CONSTRAINT_STRIDE`].
    instances: usize,
}

impl ExactSha256CompressionWitnessBatch {
    /// Packed committed Boolean source rows, including the leading-one row.
    pub fn source_rows(&self) -> &[Vec<u64>] {
        &self.source_rows
    }

    /// Packed synthesized integer-assignment rows.
    pub fn assignment_rows(&self) -> &[Vec<u64>] {
        &self.assignment_rows
    }

    /// Native SHA-256 compression outputs in batch-instance order.
    pub fn outputs(&self) -> &[[u32; 8]] {
        &self.outputs
    }

    /// Exact signed integer `A h`, `B h`, and `C h` values in
    /// instance-major, live-constraint-row order.
    pub const fn exact_products(&self) -> &IntegerProducts {
        &self.exact_products
    }

    /// Reduces the retained exact products under an explicitly selected and
    /// validated runtime prime field.
    pub fn project_products(
        &self,
        field_config: &<F128 as PrimeField>::Config,
    ) -> Result<R1csProductMles<F128>, Sha256WitnessError> {
        <F128 as SpartanField>::validate_config(field_config)
            .map_err(|_| Sha256WitnessError::InvalidFieldConfiguration)?;
        let modulus = RuntimeModulus::<2>::new(BigUint::from_bytes_le(
            &F128::canonical_modulus_encoding(field_config),
        ))
        .map_err(|_| Sha256WitnessError::InvalidFieldConfiguration)?;
        let products = self.exact_products.reduce_parallel(&modulus);
        Ok(pack_products(self.instances, &products, field_config))
    }

    /// Consumes this exact batch after explicitly projecting its products.
    pub fn into_projected(
        self,
        field_config: &<F128 as PrimeField>::Config,
    ) -> Result<Sha256CompressionWitnessBatch, Sha256WitnessError> {
        let products = self.project_products(field_config)?;
        Ok((
            self.source_rows,
            self.assignment_rows,
            products,
            self.outputs,
        ))
    }
}

/// Failures while generating or packing a SHA-256 compression batch.
#[derive(Clone, Copy, Debug, Error, Eq, PartialEq)]
pub enum Sha256WitnessError {
    /// Inputs and F2Z parameters do not describe one common power-of-two batch.
    #[error("SHA-256 witness inputs do not match the batch geometry")]
    InvalidGeometry,

    /// The generated circuit unexpectedly changed its fixed local shape.
    #[error("SHA-256 circuit witness has an unexpected local shape")]
    UnexpectedCircuitShape,

    /// Product projection requires a genuine, sufficiently large runtime
    /// prime field.
    #[error("SHA-256 witness products use an invalid runtime prime field")]
    InvalidFieldConfiguration,

    /// Product reduction must use the fixed field shared by Spartan and F2Z.
    #[deprecated(note = "runtime-prime projection supersedes the fixed-modulus restriction")]
    #[error("SHA-256 witness products use an unsupported field modulus")]
    UnsupportedFieldModulus,
}

struct CompressionShard {
    f: PackedWitness,
    h_bar: PackedWitness,
    exact_products: IntegerProducts,
    output: [u32; 8],
}

/// Generates packed Boolean rows and retains exact signed Spartan products
/// without consulting a runtime modulus.
pub fn generate_sha256_compression_witnesses_exact(
    inputs: &[Sha256CompressionInput],
    p_f: &IntEvalParams,
    p_h: &IntEvalParams,
) -> Result<ExactSha256CompressionWitnessBatch, Sha256WitnessError> {
    validate_geometry(inputs.len(), p_f, p_h)?;

    #[cfg(feature = "parallel")]
    let shards: Vec<CompressionShard> = inputs
        .par_iter()
        .map(generate_one)
        .collect::<Result<_, _>>()?;
    #[cfg(not(feature = "parallel"))]
    let shards: Vec<CompressionShard> =
        inputs.iter().map(generate_one).collect::<Result<_, _>>()?;

    let source_rows = pack_source_rows(&shards, p_f);
    let assignment_rows = pack_derived_rows(&shards, p_h);
    let outputs = shards.iter().map(|shard| shard.output).collect();
    let exact_products = flatten_exact_products(shards);

    Ok(ExactSha256CompressionWitnessBatch {
        source_rows,
        assignment_rows,
        exact_products,
        outputs,
        instances: inputs.len(),
    })
}

/// Generates packed `f`, synthesized `h_bar`, and reduced Spartan products for
/// a complete batch. The returned row stores are already in
/// [`crate::ligerito_flock::commit_rs_ligerito_rows`] layout.
///
/// This compatibility wrapper performs both phases back-to-back. New
/// commit-before-q flows should call
/// [`generate_sha256_compression_witnesses_exact`] and project only after the
/// transcript selects the runtime prime.
pub fn generate_sha256_compression_witnesses(
    inputs: &[Sha256CompressionInput],
    p_f: &IntEvalParams,
    p_h: &IntEvalParams,
    field_config: &<F128 as PrimeField>::Config,
) -> Result<Sha256CompressionWitnessBatch, Sha256WitnessError> {
    generate_sha256_compression_witnesses_exact(inputs, p_f, p_h)?.into_projected(field_config)
}

fn validate_geometry(
    instances: usize,
    p_f: &IntEvalParams,
    p_h: &IntEvalParams,
) -> Result<(), Sha256WitnessError> {
    if instances == 0
        || p_f.s != 0
        || p_h.s != 0
        || p_f.word_bits != 1
        || p_h.word_bits != 1
        || p_f.cells() != (1 + instances * SHA256_F_INSTANCE_BITS).next_power_of_two()
        || p_h.cells() != (1 + instances * SHA256_H_INSTANCE_BITS).next_power_of_two()
    {
        return Err(Sha256WitnessError::InvalidGeometry);
    }
    Ok(())
}

fn generate_one(input: &Sha256CompressionInput) -> Result<CompressionShard, Sha256WitnessError> {
    let input_bits = compression_input_bits(input);
    let mut generator = ProductWitgen::with_inputs_and_capacity(
        &input_bits,
        COMPRESSION_INPUT_BITS + COMPRESSION_HINT_BITS,
    );
    let output_bits = compression_circuit(&mut generator, &input_bits);
    let (f, h_bar, exact) = generator.into_parts();
    if f.bit_len() != SHA256_F_LIVE_BITS
        || h_bar.bit_len() != SHA256_H_BAR_LIVE_BITS
        || exact.a_mw.len() != SHA256_CONSTRAINTS
        || exact.b_mw.len() != SHA256_CONSTRAINTS
        || exact.c_mw.len() != SHA256_CONSTRAINTS
    {
        return Err(Sha256WitnessError::UnexpectedCircuitShape);
    }
    let output = array::from_fn(|word| {
        (0..32).fold(0u32, |value, bit| {
            value | (u32::from(output_bits[word * 32 + bit]) << bit)
        })
    });
    Ok(CompressionShard {
        f,
        h_bar,
        exact_products: exact,
        output,
    })
}

fn compression_input_bits(
    (state, block): &Sha256CompressionInput,
) -> [bool; COMPRESSION_INPUT_BITS] {
    array::from_fn(|index| {
        let word = if index < 512 {
            block[index / 32]
        } else {
            state[(index - 512) / 32]
        };
        word >> (index % 32) & 1 == 1
    })
}

/// Packs the compact Boolean source witnesses into the coordinate-major rows
/// consumed by the F2Z commitment.
///
/// Let `N = shards.len()`. For instance `i`, the shard contains
///
/// `f_i = (input_i, hints_i) in F_2^7_144`,
///
/// comprising [`COMPRESSION_INPUT_BITS`] input bits followed by
/// [`COMPRESSION_HINT_BITS`] circuit-hint bits. Distinguish its augmented live
/// source
///
/// `f_bar_i = (1, f_i) in F_2^7_145`
///
/// from its power-of-two-padded source
///
/// `f_tilde_i = (f_bar_i, 0^1_047) in F_2^8_192`.
///
/// These dimensions are [`SHA256_F_LIVE_BITS`],
/// [`SHA256_F_BAR_LIVE_BITS`], and [`SHA256_F_STRIDE`], respectively. If
/// `W = ceil(N / 64)`, the result has shape `8_192 x W` machine words and
/// transposes the conceptual instance-major matrix according to
///
/// `rows[j][g] = sum_{ell=0}^{63} f_tilde_{64g+ell}[j] 2^ell`,
///
/// where `f_tilde_i[j] = 0` for nonexistent instances `i >= N`. Equivalently,
/// for every active instance,
///
/// `((rows[j][i / 64] >> (i % 64)) & 1) = f_tilde_i[j]`.
///
/// Thus packing is least-significant-lane first: `rows[0]` contains the
/// leading one, `rows[1 + k]` contains `f_i[k]`, coordinates
/// `j >= SHA256_F_BAR_LIVE_BITS` are zero padding, and unused high lanes of
/// the final word are zero.
fn pack_source_rows(shards: &[CompressionShard], params: &IntEvalParams) -> Vec<Vec<u64>> {
    let mut rows = vec![vec![0u64; params.rows().div_ceil(64)]];
    set_packed_bit(&mut rows[0], 0);
    for (instance, shard) in shards.iter().enumerate() {
        let start = 1 + instance * SHA256_F_INSTANCE_BITS;
        for bit in 0..SHA256_F_LIVE_BITS {
            if shard.f.bit(bit) {
                set_packed_bit(&mut rows[0], start + bit);
            }
        }
    }
    rows
}

/// Packs the already-synthesized Boolean assignments into F2Z's
/// coordinate-major rows.
///
/// For instance `i`, let `f_bar_i in F_2^7_145` be the augmented live source
/// described by [`pack_source_rows`], and let
///
/// `M_live in F_2^(20_457 x 7_145)`
///
/// be the public live witness map. The shard's live assignment satisfies
///
/// `h_bar_i = (1, h_i) = M_live f_bar_i in F_2^20_457`.
///
/// [`ProductWitgen`] materializes `h_bar_i` while evaluating the circuit; this
/// function neither evaluates `M_live` nor derives new witness values. It only
/// appends the power-of-two padding
///
/// `h_tilde_i = (h_bar_i, 0^12_311) in F_2^32_768`
///
/// and transposes the existing bits. Equivalently, if
/// `M_pad in F_2^(32_768 x 8_192)` is `M_live` extended with zero rows and
/// columns, then `h_tilde_i = M_pad f_tilde_i`. In terms of the named
/// constants, the live and padded dimensions are
/// [`SHA256_H_BAR_LIVE_BITS`] and [`SHA256_H_STRIDE`]. For
/// `N = shards.len()` and `W = ceil(N / 64)`, the returned table has shape
/// `32_768 x W` machine words and satisfies
///
/// `rows[j][g] = sum_{ell=0}^{63} h_tilde_{64g+ell}[j] 2^ell`,
///
/// taking `h_tilde_i[j] = 0` for nonexistent instances `i >= N`.
/// Equivalently, for every active instance,
///
/// `((rows[j][i / 64] >> (i % 64)) & 1) = h_tilde_i[j]`.
///
/// Packing is least-significant-lane first. Coordinates
/// `j >= SHA256_H_BAR_LIVE_BITS` and unused high lanes of the final word are
/// zero.
fn pack_derived_rows(shards: &[CompressionShard], params: &IntEvalParams) -> Vec<Vec<u64>> {
    let mut rows = vec![vec![0u64; params.rows().div_ceil(64)]];
    set_packed_bit(&mut rows[0], 0);
    for (instance, shard) in shards.iter().enumerate() {
        let start = 1 + instance * SHA256_H_INSTANCE_BITS;
        for bit in 1..SHA256_H_BAR_LIVE_BITS {
            if shard.h_bar.bit(bit) {
                set_packed_bit(&mut rows[0], start + bit - 1);
            }
        }
    }
    rows
}

fn set_packed_bit(words: &mut [u64], bit: usize) {
    words[bit / u64::BITS as usize] |= 1u64 << (bit % u64::BITS as usize);
}

/// Concatenates each shard's already-evaluated integer R1CS products.
///
/// Let `N = shards.len()`, `m = SHA256_CONSTRAINTS = 184`, and
/// `n = SHA256_H_BAR_LIVE_BITS = 20_457`. For instance `i`, canonically embed
/// the live Boolean assignment `h_bar_i in {0, 1}^n` into `Z^n`. The live
/// integer matrices `A_live, B_live, C_live in Z^(m x n)` define
///
/// `a_{i,r} = sum_{j=0}^{n-1} A_live[r,j] h_bar_i[j]`,
/// `b_{i,r} = sum_{j=0}^{n-1} B_live[r,j] h_bar_i[j]`, and
/// `c_{i,r} = sum_{j=0}^{n-1} C_live[r,j] h_bar_i[j]`.
///
/// This function preserves instance order and stores those exact signed
/// integers at
///
/// `products.a_mw[i * m + r] = a_{i,r}`,
/// `products.b_mw[i * m + r] = b_{i,r}`, and
/// `products.c_mw[i * m + r] = c_{i,r}`
///
/// for `0 <= i < N` and `0 <= r < m`; hence each returned vector has length
/// `N * m`. A satisfying generated witness obeys the exact integer identity
///
/// `a_{i,r} b_{i,r} = c_{i,r}` in `Z`
///
/// on every live row. [`ProductWitgen`] has already evaluated the three
/// matrix products; this function only concatenates them and does not check
/// the identity. Taking ownership of `shards` moves the signed-integer
/// representations without cloning them.
///
/// No field projection or constraint-row padding occurs here. After a runtime
/// prime `q` is selected, [`ExactSha256CompressionWitnessBatch::project_products`]
/// writes blocks of width `R = SHA256_CONSTRAINT_STRIDE = 256` as
///
/// `az[i * R + r] = [a_{i,r}]_q` for `0 <= r < m`,
/// `az[i * R + r] = 0` for `m <= r < R`,
///
/// and analogously for `bz` and `cz`, where `[x]_q` is the image of `x in Z`
/// in `F_q`.
fn flatten_exact_products(shards: Vec<CompressionShard>) -> IntegerProducts {
    let capacity = shards.len() * SHA256_CONSTRAINTS;
    let mut products = IntegerProducts {
        a_mw: Vec::with_capacity(capacity),
        b_mw: Vec::with_capacity(capacity),
        c_mw: Vec::with_capacity(capacity),
    };
    for shard in shards {
        let IntegerProducts {
            mut a_mw,
            mut b_mw,
            mut c_mw,
        } = shard.exact_products;
        products.a_mw.append(&mut a_mw);
        products.b_mw.append(&mut b_mw);
        products.c_mw.append(&mut c_mw);
    }
    products
}

fn pack_products(
    instances: usize,
    products: &MatrixProducts<2>,
    field_config: &<F128 as PrimeField>::Config,
) -> R1csProductMles<F128> {
    debug_assert_eq!(products.a_mw.len(), instances * SHA256_CONSTRAINTS);
    debug_assert_eq!(products.b_mw.len(), instances * SHA256_CONSTRAINTS);
    debug_assert_eq!(products.c_mw.len(), instances * SHA256_CONSTRAINTS);

    let instance_capacity = instances.next_power_of_two();
    let table_len = instance_capacity * SHA256_CONSTRAINT_STRIDE;
    let zero = F128::zero_with_cfg(field_config);
    let mut az = vec![zero.clone(); table_len];
    let mut bz = vec![zero.clone(); table_len];
    let mut cz = vec![zero; table_len];

    #[cfg(feature = "parallel")]
    az.par_chunks_mut(SHA256_CONSTRAINT_STRIDE)
        .zip(bz.par_chunks_mut(SHA256_CONSTRAINT_STRIDE))
        .zip(cz.par_chunks_mut(SHA256_CONSTRAINT_STRIDE))
        .take(instances)
        .enumerate()
        .for_each(|(instance, ((a_out, b_out), c_out))| {
            let start = instance * SHA256_CONSTRAINTS;
            let end = start + SHA256_CONSTRAINTS;
            write_product_block(a_out, &products.a_mw.values()[start..end], field_config);
            write_product_block(b_out, &products.b_mw.values()[start..end], field_config);
            write_product_block(c_out, &products.c_mw.values()[start..end], field_config);
        });
    #[cfg(not(feature = "parallel"))]
    for instance in 0..instances {
        let output_start = instance * SHA256_CONSTRAINT_STRIDE;
        let output_end = output_start + SHA256_CONSTRAINT_STRIDE;
        let input_start = instance * SHA256_CONSTRAINTS;
        let input_end = input_start + SHA256_CONSTRAINTS;
        write_product_block(
            &mut az[output_start..output_end],
            &products.a_mw.values()[input_start..input_end],
            field_config,
        );
        write_product_block(
            &mut bz[output_start..output_end],
            &products.b_mw.values()[input_start..input_end],
            field_config,
        );
        write_product_block(
            &mut cz[output_start..output_end],
            &products.c_mw.values()[input_start..input_end],
            field_config,
        );
    }

    let num_vars = instance_capacity.ilog2() as usize + SHA256_CONSTRAINT_LOCAL_VARS;
    R1csProductMles {
        az: DenseMultilinearExtension {
            evaluations: az,
            num_vars,
        },
        bz: DenseMultilinearExtension {
            evaluations: bz,
            num_vars,
        },
        cz: DenseMultilinearExtension {
            evaluations: cz,
            num_vars,
        },
    }
}

fn write_product_block(
    output: &mut [F128],
    input: &[[u64; 2]],
    field_config: &<F128 as PrimeField>::Config,
) {
    debug_assert_eq!(input.len(), SHA256_CONSTRAINTS);
    for (target, words) in output.iter_mut().zip(input) {
        *target = F128::new_with_cfg(Uint::new((*words).into()), field_config);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        f2map::VirtualMap,
        pcs::FQ_MOD,
        piop::spartan::{
            f2z::spartan_f2z_field_config,
            sha256::constraints::{
                prepare_sha256_compression_batch, prepare_sha256_compression_batch_integer,
            },
        },
        sparse_matrix::SparseMatrix,
    };

    const OTHER_TEST_PRIME: u128 = (1_u128 << 127) - 1;

    fn config(modulus: u128) -> <F128 as PrimeField>::Config {
        F128::make_cfg(&Uint::from(modulus)).expect("odd test modulus")
    }

    fn abc_input() -> Sha256CompressionInput {
        (
            [
                0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
                0x5be0cd19,
            ],
            [
                0x61626380, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x00000018,
            ],
        )
    }

    #[test]
    fn generation_matches_fips_and_packs_both_witnesses() {
        let (matrices, map, p_f, p_h) = prepare_sha256_compression_batch(0).unwrap();
        let cfg = spartan_f2z_field_config();
        let (f_rows, h_rows, products, outputs) =
            generate_sha256_compression_witnesses(&[abc_input()], &p_f, &p_h, &cfg).unwrap();

        assert_eq!(
            outputs[0],
            [
                0xba7816bf, 0x8f01cfea, 0x414140de, 0x5dae2223, 0xb00361a3, 0x96177a9c, 0xb410ff61,
                0xf20015ad,
            ]
        );
        assert_eq!(f_rows.len(), SHA256_F_STRIDE);
        assert_eq!(h_rows.len(), SHA256_H_STRIDE);
        assert_eq!(f_rows[0][0] & 1, 1);
        assert_eq!(h_rows[0][0] & 1, 1);
        assert_eq!(products.az.evaluations.len(), SHA256_CONSTRAINT_STRIDE);
        assert!(products.az.evaluations[SHA256_CONSTRAINTS..]
            .iter()
            .all(PrimeField::is_zero));

        let mut mapped_h = vec![false; SHA256_H_STRIDE];
        for (source, row) in f_rows.iter().enumerate() {
            if row[0] & 1 == 0 {
                continue;
            }
            for derived in map.local().column_rows(source).unwrap() {
                mapped_h[derived] ^= true;
            }
        }
        assert!(mapped_h
            .iter()
            .enumerate()
            .all(|(row, expected)| *expected == (h_rows[row][0] & 1 == 1)));

        let h_bits = h_rows.iter().map(|row| row[0] & 1 == 1).collect::<Vec<_>>();
        assert_matrix_product(
            matrices.matrices().a(),
            &h_bits,
            &products.az.evaluations,
            &cfg,
        );
        assert_matrix_product(
            matrices.matrices().b(),
            &h_bits,
            &products.bz.evaluations,
            &cfg,
        );
        assert_matrix_product(
            matrices.matrices().c(),
            &h_bits,
            &products.cz.evaluations,
            &cfg,
        );
    }

    #[test]
    fn public_statement_words_use_canonical_order() {
        let state = array::from_fn(|index| index as u32);
        let block = array::from_fn(|index| 8 + index as u32);
        let output = array::from_fn(|index| 24 + index as u32);
        let statement = Sha256CompressionStatement::new((state, block), output);

        assert_eq!(
            statement.words().collect::<Vec<_>>(),
            (0..32).collect::<Vec<_>>()
        );
    }

    #[test]
    fn transposed_rows_preserve_instance_bits() {
        let (_, _, p_f, p_h) = prepare_sha256_compression_batch(6).unwrap();
        let cfg = spartan_f2z_field_config();
        let inputs = (0..64)
            .map(|instance| {
                let mut input = abc_input();
                input.1[0] ^= instance;
                input
            })
            .collect::<Vec<_>>();
        let (f_rows, h_rows, _, _) =
            generate_sha256_compression_witnesses(&inputs, &p_f, &p_h, &cfg).unwrap();
        assert_eq!(f_rows[0][0], u64::MAX);
        assert_eq!(h_rows[0][0], u64::MAX);
        assert!(f_rows[SHA256_F_BAR_LIVE_BITS..]
            .iter()
            .all(|row| row[0] == 0));
        assert!(h_rows[SHA256_H_BAR_LIVE_BITS..]
            .iter()
            .all(|row| row[0] == 0));
    }

    #[test]
    fn exact_products_project_under_distinct_runtime_primes() {
        let prepared = prepare_sha256_compression_batch_integer(0).unwrap();
        let exact = generate_sha256_compression_witnesses_exact(
            &[abc_input()],
            prepared.source_params(),
            prepared.assignment_params(),
        )
        .unwrap();

        assert_eq!(exact.exact_products().a_mw.len(), SHA256_CONSTRAINTS);
        assert_eq!(exact.exact_products().b_mw.len(), SHA256_CONSTRAINTS);
        assert_eq!(exact.exact_products().c_mw.len(), SHA256_CONSTRAINTS);
        let h_bits = exact
            .assignment_rows()
            .iter()
            .map(|row| row[0] & 1 == 1)
            .collect::<Vec<_>>();

        for modulus in [FQ_MOD, OTHER_TEST_PRIME] {
            let field_config = config(modulus);
            let matrices = prepared.project(&field_config).unwrap();
            let products = exact.project_products(&field_config).unwrap();
            assert_matrix_product(
                matrices.matrices().a(),
                &h_bits,
                &products.az.evaluations,
                &field_config,
            );
            assert_matrix_product(
                matrices.matrices().b(),
                &h_bits,
                &products.bz.evaluations,
                &field_config,
            );
            assert_matrix_product(
                matrices.matrices().c(),
                &h_bits,
                &products.cz.evaluations,
                &field_config,
            );
        }
    }

    fn assert_matrix_product(
        matrix: &SparseMatrix<F128>,
        assignment: &[bool],
        expected: &[F128],
        field_config: &<F128 as PrimeField>::Config,
    ) {
        let mut actual = vec![F128::zero_with_cfg(field_config); matrix.row_count()];
        for (column, bit) in assignment.iter().copied().enumerate() {
            if !bit {
                continue;
            }
            for (row, coefficient) in matrix.column(column).unwrap() {
                actual[row] += coefficient;
            }
        }
        assert_eq!(actual, expected);
    }
}

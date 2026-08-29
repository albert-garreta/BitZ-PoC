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
    SHA256_F_BAR_LIVE_BITS, SHA256_F_LIVE_BITS, SHA256_F_LOCAL_VARS, SHA256_F_STRIDE,
    SHA256_H_BAR_LIVE_BITS, SHA256_H_LOCAL_VARS, SHA256_H_STRIDE,
};

/// One independent SHA-256 compression input: chaining state and message block.
pub type Sha256CompressionInput = ([u32; 8], [u32; 16]);

/// Public input and claimed output for one SHA-256 compression.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Sha256CompressionStatement {
    /// Initial eight-word chaining state.
    pub state: [u32; 8],
    /// Sixteen-word message block.
    pub block: [u32; 16],
    /// Claimed eight-word state after compression.
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
    source_rows: Vec<Vec<u64>>,
    assignment_rows: Vec<Vec<u64>>,
    exact_products: IntegerProducts,
    outputs: Vec<[u32; 8]>,
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

    let source_rows = pack_source_rows(&shards);
    let assignment_rows = pack_derived_rows(&shards);
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
        || !instances.is_power_of_two()
        || p_f.rows() != instances
        || p_h.rows() != instances
        || p_f.s != SHA256_F_LOCAL_VARS
        || p_h.s != SHA256_H_LOCAL_VARS
        || p_f.word_bits != 1
        || p_h.word_bits != 1
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

fn pack_source_rows(shards: &[CompressionShard]) -> Vec<Vec<u64>> {
    let words_per_row = shards.len().div_ceil(64);
    let mut rows = vec![vec![0u64; words_per_row]; SHA256_F_STRIDE];
    for (group, word) in rows[0].iter_mut().enumerate() {
        let active = (shards.len() - group * 64).min(64);
        *word = if active == 64 {
            u64::MAX
        } else {
            (1u64 << active) - 1
        };
    }
    transpose_witness_rows(shards, &mut rows, SHA256_F_LIVE_BITS, 1, |shard| &shard.f);
    debug_assert!(rows[SHA256_F_BAR_LIVE_BITS..]
        .iter()
        .all(|row| row.iter().all(|word| *word == 0)));
    rows
}

fn pack_derived_rows(shards: &[CompressionShard]) -> Vec<Vec<u64>> {
    let words_per_row = shards.len().div_ceil(64);
    let mut rows = vec![vec![0u64; words_per_row]; SHA256_H_STRIDE];
    transpose_witness_rows(shards, &mut rows, SHA256_H_BAR_LIVE_BITS, 0, |shard| {
        &shard.h_bar
    });
    rows
}

fn transpose_witness_rows(
    shards: &[CompressionShard],
    rows: &mut [Vec<u64>],
    live_bits: usize,
    output_offset: usize,
    witness: impl Fn(&CompressionShard) -> &PackedWitness,
) {
    // Each group owns one word in every output row. A range loop makes that
    // intentionally strided 64x64 transpose explicit.
    #[allow(clippy::needless_range_loop)]
    for group in 0..shards.len().div_ceil(64) {
        let first_instance = group * 64;
        let active = (shards.len() - first_instance).min(64);
        for input_word in 0..live_bits.div_ceil(64) {
            let mut block = [0u64; 64];
            for lane in 0..active {
                block[lane] = witness(&shards[first_instance + lane])
                    .words()
                    .get(input_word)
                    .copied()
                    .unwrap_or(0);
            }
            transpose64(&mut block);
            let bit_start = input_word * 64;
            let bit_end = (bit_start + 64).min(live_bits);
            for bit in bit_start..bit_end {
                rows[output_offset + bit][group] = block[bit - bit_start];
            }
        }
    }
}

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

    let table_len = instances * SHA256_CONSTRAINT_STRIDE;
    let zero = F128::zero_with_cfg(field_config);
    let mut az = vec![zero.clone(); table_len];
    let mut bz = vec![zero.clone(); table_len];
    let mut cz = vec![zero; table_len];

    #[cfg(feature = "parallel")]
    az.par_chunks_mut(SHA256_CONSTRAINT_STRIDE)
        .zip(bz.par_chunks_mut(SHA256_CONSTRAINT_STRIDE))
        .zip(cz.par_chunks_mut(SHA256_CONSTRAINT_STRIDE))
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

    let num_vars = instances.ilog2() as usize + SHA256_CONSTRAINT_LOCAL_VARS;
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

/// In-place 64x64 bit-matrix transpose. Output word `j`'s bit `k` is
/// input word `k`'s bit `j`.
#[allow(clippy::arithmetic_side_effects)]
fn transpose64(matrix: &mut [u64; 64]) {
    let mut shift = 32usize;
    let mut mask = 0x0000_0000_ffff_ffffu64;
    while shift != 0 {
        let mut index = 0usize;
        while index < 64 {
            let swap = ((matrix[index] >> shift) ^ matrix[index | shift]) & mask;
            matrix[index | shift] ^= swap;
            matrix[index] ^= swap << shift;
            index = ((index | shift) + 1) & !shift;
        }
        shift >>= 1;
        mask ^= mask << shift;
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

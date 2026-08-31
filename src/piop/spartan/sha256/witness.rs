//! SHA-256 compression witness generation.
//!
//! One [`Witgen`] execution produces the Boolean source `f` and synthesized
//! integer assignment `h_bar` without evaluating or retaining any constraint
//! matrix products. A batch shares one leading constant cell, places every
//! instance immediately after the preceding instance, and pads only the final
//! suffix of the complete F2Z domain.

use std::array;

use circuit::{
    sha256::{COMPRESSION_HINT_BITS, COMPRESSION_INPUT_BITS, compression_circuit},
    witgen::{PackedWitness, Witgen},
};
use thiserror::Error;

#[cfg(feature = "parallel")]
use rayon::prelude::*;

use crate::pcs::IntEvalParams;

use super::constraints::{
    PreparedSha256CompressionBatch, SHA256_F_INSTANCE_BITS, SHA256_F_LIVE_BITS,
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

/// A complete product-free SHA-256 witness batch.
///
/// Boolean source and assignment rows are generated and packed immediately.
/// No exact or field-valued constraint-matrix products are materialized; the
/// linear prover consumes the signed sparse relation and packed assignment
/// bits directly.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Sha256CompressionWitnessBatch {
    /// Packed F2Z source rows with logical bit layout
    ///
    /// `source = [1 | f_0 | f_1 | ... | f_{N-1} | 0 ... 0]`.
    ///
    /// The leading constant is shared. Each `f_i` contributes exactly
    /// [`SHA256_F_INSTANCE_BITS`] adjacent cells, and only the complete
    /// power-of-two domain has a trailing zero suffix. For F2Z parameters
    /// `(t, s)`, flat sequence cell `j` is row `j mod 2^t` of column
    /// `j >> t`; the low `t` sequence bits are the packed-row coordinates.
    source_rows: Vec<Vec<u64>>,

    /// Packed F2Z assignment rows with logical bit layout
    ///
    /// `assignment = [1 | h_0[1..] | h_1[1..] | ... | h_{N-1}[1..] | 0 ... 0]`.
    ///
    /// Every local augmented assignment `h_bar_i = (1, h_i)` uses the shared
    /// leading one and contributes exactly [`SHA256_H_INSTANCE_BITS`] adjacent
    /// nonconstant cells. There are no gaps between instances. Although the
    /// values are stored as bits, the R1CS computation embeds them canonically
    /// as the integers `0` and `1`.
    assignment_rows: Vec<Vec<u64>>,

    /// Native compression results `O_i in (Z / 2^32 Z)^8`, in batch-instance
    /// and SHA-256 state-word order.
    ///
    /// Each `u32` is the canonical representative of one output word:
    /// `outputs[i][w] = sum_{b=0}^{31} output_bit[i, w, b] * 2^b`, with bits
    /// numbered least-significant first inside the word.
    outputs: Vec<[u32; 8]>,

    /// Positive batch cardinality `N` (not necessarily a power of two).
    ///
    /// This equals `outputs.len()` and the batch geometry's live instance
    /// count.
    instances: usize,
}

impl Sha256CompressionWitnessBatch {
    /// Packed committed Boolean source rows, including the shared leading-one cell.
    pub(crate) fn source_rows(&self) -> &[Vec<u64>] {
        &self.source_rows
    }

    /// Packed synthesized Boolean assignment rows.
    pub(crate) fn assignment_rows(&self) -> &[Vec<u64>] {
        &self.assignment_rows
    }

    /// Native SHA-256 compression outputs in batch-instance order.
    pub fn outputs(&self) -> &[[u32; 8]] {
        &self.outputs
    }

    /// Reads one cell of the padded packed source domain.
    #[cfg(test)]
    pub(crate) fn source_bit(&self, flat_column: usize) -> Option<bool> {
        packed_rows_bit(&self.source_rows, flat_column)
    }

    /// Reads one cell of the padded packed assignment domain.
    #[cfg(test)]
    pub(crate) fn assignment_bit(&self, flat_column: usize) -> Option<bool> {
        packed_rows_bit(&self.assignment_rows, flat_column)
    }

    /// Number of live compression instances represented by the packed rows.
    pub(crate) const fn instances(&self) -> usize {
        self.instances
    }

    #[cfg(test)]
    pub(super) fn source_rows_mut_for_tests(&mut self) -> &mut [Vec<u64>] {
        &mut self.source_rows
    }

    #[cfg(test)]
    pub(super) fn assignment_rows_mut_for_tests(&mut self) -> &mut [Vec<u64>] {
        &mut self.assignment_rows
    }
}

/// Failures while generating or packing a SHA-256 compression batch.
#[derive(Clone, Copy, Debug, Error, Eq, PartialEq)]
pub enum Sha256WitnessError {
    /// Inputs and F2Z parameters do not describe one common packed batch.
    #[error("SHA-256 witness inputs do not match the batch geometry")]
    InvalidGeometry,

    /// The generated circuit unexpectedly changed its fixed local shape.
    #[error("SHA-256 circuit witness has an unexpected local shape")]
    UnexpectedCircuitShape,
}

struct PackedCompressionShard {
    f: PackedWitness,
    h_bar: PackedWitness,
    output: [u32; 8],
}

/// Generates product-free packed Boolean source and assignment rows.
pub fn generate_sha256_compression_witnesses(
    prepared: &PreparedSha256CompressionBatch,
    inputs: &[Sha256CompressionInput],
) -> Result<Sha256CompressionWitnessBatch, Sha256WitnessError> {
    if inputs.len() != prepared.instances() {
        return Err(Sha256WitnessError::InvalidGeometry);
    }
    let p_f = prepared.source_params();
    let p_h = prepared.assignment_params();
    validate_geometry(inputs.len(), p_f, p_h)?;

    #[cfg(feature = "parallel")]
    let shards: Vec<PackedCompressionShard> = inputs
        .par_iter()
        .map(generate_one)
        .collect::<Result<_, _>>()?;
    #[cfg(not(feature = "parallel"))]
    let shards: Vec<PackedCompressionShard> =
        inputs.iter().map(generate_one).collect::<Result<_, _>>()?;

    let source_rows = pack_source_rows(&shards, p_f);
    let assignment_rows = pack_derived_rows(&shards, p_h);
    let outputs = shards.iter().map(|shard| shard.output).collect();

    Ok(Sha256CompressionWitnessBatch {
        source_rows,
        assignment_rows,
        outputs,
        instances: inputs.len(),
    })
}

fn validate_geometry(
    instances: usize,
    p_f: &IntEvalParams,
    p_h: &IntEvalParams,
) -> Result<(), Sha256WitnessError> {
    if instances == 0
        || p_f.word_bits != 1
        || p_h.word_bits != 1
        || p_f.cells() != (1 + instances * SHA256_F_INSTANCE_BITS).next_power_of_two()
        || p_h.cells() != (1 + instances * SHA256_H_INSTANCE_BITS).next_power_of_two()
    {
        return Err(Sha256WitnessError::InvalidGeometry);
    }
    Ok(())
}

fn generate_one(
    input: &Sha256CompressionInput,
) -> Result<PackedCompressionShard, Sha256WitnessError> {
    let input_bits = compression_input_bits(input);
    let mut generator = Witgen::with_inputs_and_capacity(
        &input_bits,
        COMPRESSION_INPUT_BITS + COMPRESSION_HINT_BITS,
    );
    let output_bits = compression_circuit(&mut generator, &input_bits);
    let (f, h_bar) = generator.into_witnesses();
    finish_packed_shard(f, h_bar, &output_bits)
}

fn finish_packed_shard(
    f: PackedWitness,
    h_bar: PackedWitness,
    output_bits: &[bool],
) -> Result<PackedCompressionShard, Sha256WitnessError> {
    if f.bit_len() != SHA256_F_LIVE_BITS || h_bar.bit_len() != SHA256_H_BAR_LIVE_BITS {
        return Err(Sha256WitnessError::UnexpectedCircuitShape);
    }
    let output = array::from_fn(|word| {
        (0..32).fold(0u32, |value, bit| {
            value | (u32::from(output_bits[word * 32 + bit]) << bit)
        })
    });
    Ok(PackedCompressionShard { f, h_bar, output })
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

/// Packs the compact source witnesses as
/// `[1 | f_0 | ... | f_{N-1} | trailing zeros]`.
///
/// Each `f_i` contains [`COMPRESSION_INPUT_BITS`] input bits followed by
/// [`COMPRESSION_HINT_BITS`] hint bits, for exactly
/// [`SHA256_F_INSTANCE_BITS`] cells. Logical cells are packed least-significant
/// bit first in the semantic sequence. The sequence index is also F2Z's
/// physical column-major index: its low `t` bits select a packed row and its
/// high `s` bits select a column.
fn pack_source_rows<'a>(
    shards: impl IntoIterator<Item = &'a PackedCompressionShard>,
    params: &IntEvalParams,
) -> Vec<Vec<u64>> {
    let mut rows = empty_packed_rows(params);
    set_flat_packed_bit(&mut rows, params, 0);
    for (instance, shard) in shards.into_iter().enumerate() {
        let start = 1 + instance * SHA256_F_INSTANCE_BITS;
        for bit in 0..SHA256_F_LIVE_BITS {
            if shard.f.bit(bit) {
                set_flat_packed_bit(&mut rows, params, start + bit);
            }
        }
    }
    rows
}

/// Packs the already-synthesized Boolean assignments into the F2Z row/column
/// view without changing their flat logical order.
///
/// [`Witgen`] has already materialized each augmented assignment
/// `h_bar_i = (1, h_i)`. This function stores the constant once, concatenates
/// the `h_i` portions without gaps, and leaves only the complete domain's
/// suffix zero: `[1 | h_0 | ... | h_{N-1} | trailing zeros]`.
fn pack_derived_rows<'a>(
    shards: impl IntoIterator<Item = &'a PackedCompressionShard>,
    params: &IntEvalParams,
) -> Vec<Vec<u64>> {
    let mut rows = empty_packed_rows(params);
    set_flat_packed_bit(&mut rows, params, 0);
    for (instance, shard) in shards.into_iter().enumerate() {
        let start = 1 + instance * SHA256_H_INSTANCE_BITS;
        for bit in 1..SHA256_H_BAR_LIVE_BITS {
            if shard.h_bar.bit(bit) {
                set_flat_packed_bit(&mut rows, params, start + bit - 1);
            }
        }
    }
    rows
}

fn empty_packed_rows(params: &IntEvalParams) -> Vec<Vec<u64>> {
    vec![vec![0u64; params.rows().div_ceil(64)]; params.cols()]
}

fn set_flat_packed_bit(rows: &mut [Vec<u64>], params: &IntEvalParams, flat_cell: usize) {
    let column = flat_cell >> params.t;
    let row = flat_cell & (params.rows() - 1);
    rows[column][row / u64::BITS as usize] |= 1u64 << (row % u64::BITS as usize);
}

#[cfg(test)]
fn packed_rows_bit(rows: &[Vec<u64>], flat_cell: usize) -> Option<bool> {
    let columns = rows.len();
    let words_per_column = rows.first()?.len();
    let rows_per_column = words_per_column.checked_mul(u64::BITS as usize)?;
    if !columns.is_power_of_two()
        || !rows_per_column.is_power_of_two()
        || rows.iter().any(|column| column.len() != words_per_column)
    {
        return None;
    }
    let column = flat_cell >> rows_per_column.ilog2();
    let row = flat_cell & (rows_per_column - 1);
    let words = rows.get(column)?;
    Some(words[row / u64::BITS as usize] >> (row % u64::BITS as usize) & 1 == 1)
}

#[cfg(test)]
mod tests {
    use num_bigint::BigInt;

    use super::*;
    use crate::{
        f2map::VirtualMap,
        piop::spartan::sha256::constraints::{
            SHA256_CONSTRAINTS, prepare_sha256_compression_batch_for_test,
        },
    };

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
        let prepared = prepare_sha256_compression_batch_for_test(0).unwrap();
        let witness = generate_sha256_compression_witnesses(&prepared, &[abc_input()]).unwrap();
        let f_rows = witness.source_rows();
        let h_rows = witness.assignment_rows();
        let outputs = witness.outputs();
        let map = prepared.map();
        let p_f = prepared.source_params();
        let p_h = prepared.assignment_params();

        assert_eq!(
            outputs[0],
            [
                0xba7816bf, 0x8f01cfea, 0x414140de, 0x5dae2223, 0xb00361a3, 0x96177a9c, 0xb410ff61,
                0xf20015ad,
            ]
        );
        assert_eq!(f_rows.len(), p_f.cols());
        assert_eq!(h_rows.len(), p_h.cols());
        assert_eq!(witness.instances(), 1);
        assert_eq!(witness.source_bit(0), Some(true));
        assert_eq!(witness.assignment_bit(0), Some(true));

        let mut mapped_h = vec![false; p_h.cells()];
        for source in 0..p_f.cells() {
            if !witness.source_bit(source).unwrap() {
                continue;
            }
            for derived in map.column_rows(source).unwrap() {
                mapped_h[derived] ^= true;
            }
        }
        assert!(
            mapped_h
                .iter()
                .enumerate()
                .all(|(row, expected)| *expected == witness.assignment_bit(row).unwrap())
        );
    }

    #[test]
    fn flat_sequence_uses_the_native_pcs_row_then_column_order() {
        let prepared = prepare_sha256_compression_batch_for_test(0).unwrap();
        let mut anchored_input = abc_input();
        anchored_input.1[0] |= 1;
        let witness = generate_sha256_compression_witnesses(&prepared, &[anchored_input]).unwrap();
        let p_f = prepared.source_params();
        let p_h = prepared.assignment_params();

        assert_eq!((p_f.t, p_f.s), (7, 6));
        assert_eq!((p_h.t, p_h.s), (8, 7));

        // Source sequence cell 1 is block[0]'s low bit. In native PCS order
        // it is row 1 of column 0, not row 0 of column 1.
        assert_eq!(witness.source_rows()[0][0] >> 1 & 1, 1);

        // The nonidentity Boolean map copies that source bit to local
        // assignment cell 257. Native order makes this row 1 of column 1.
        assert!(prepared.map().column_rows(1).unwrap().any(|row| row == 257));
        assert_eq!(witness.assignment_rows()[1][0] >> 1 & 1, 1);
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
    fn generation_rejects_an_input_count_different_from_the_prepared_batch() {
        let prepared = prepare_sha256_compression_batch_for_test(1).unwrap();
        assert_eq!(
            generate_sha256_compression_witnesses(&prepared, &[abc_input()]),
            Err(Sha256WitnessError::InvalidGeometry)
        );
    }

    #[test]
    fn packed_rows_place_instances_back_to_back() {
        let prepared = prepare_sha256_compression_batch_for_test(6).unwrap();
        let p_f = prepared.source_params();
        let p_h = prepared.assignment_params();
        let inputs = (0..64)
            .map(|instance| {
                let mut input = abc_input();
                input.1[0] ^= instance;
                input
            })
            .collect::<Vec<_>>();
        let exact = generate_sha256_compression_witnesses(&prepared, &inputs).unwrap();
        let f_rows = exact.source_rows();
        let h_rows = exact.assignment_rows();
        assert_eq!(f_rows.len(), p_f.cols());
        assert_eq!(h_rows.len(), p_h.cols());
        assert_eq!(exact.source_bit(0), Some(true));
        assert_eq!(exact.assignment_bit(0), Some(true));

        for &instance in &[0, 1, 63] {
            let one_prepared = prepare_sha256_compression_batch_for_test(0).unwrap();
            let one =
                generate_sha256_compression_witnesses(&one_prepared, &[inputs[instance]]).unwrap();
            for bit in 0..SHA256_F_INSTANCE_BITS {
                let expected = one.source_bit(1 + bit).unwrap();
                let packed = 1 + instance * SHA256_F_INSTANCE_BITS + bit;
                assert_eq!(exact.source_bit(packed), Some(expected));
            }
            for bit in 0..SHA256_H_INSTANCE_BITS {
                let expected = one.assignment_bit(1 + bit).unwrap();
                let packed = 1 + instance * SHA256_H_INSTANCE_BITS + bit;
                assert_eq!(exact.assignment_bit(packed), Some(expected));
            }
        }

        let live_f = 1 + inputs.len() * SHA256_F_INSTANCE_BITS;
        let live_h = 1 + inputs.len() * SHA256_H_INSTANCE_BITS;
        assert!((live_f..p_f.cells()).all(|bit| exact.source_bit(bit) == Some(false)));
        assert!((live_h..p_h.cells()).all(|bit| exact.assignment_bit(bit) == Some(false)));
    }

    #[test]
    fn packed_assignment_satisfies_the_exact_signed_linear_relation() {
        let prepared = prepare_sha256_compression_batch_for_test(1).unwrap();
        let mut second = abc_input();
        second.1[0] ^= 0x0102_0304;
        let witness =
            generate_sha256_compression_witnesses(&prepared, &[abc_input(), second]).unwrap();
        let relation = prepared.linear_relation();
        let mut residuals = vec![BigInt::default(); prepared.linear_row_count()];

        for instance in 0..prepared.instances() {
            for local_column in 0..relation.column_count() {
                let flat_column = prepared
                    .flat_assignment_column(instance, local_column)
                    .unwrap();
                if !witness.assignment_bit(flat_column).unwrap() {
                    continue;
                }
                for (local_row, coefficient) in relation
                    .matrix()
                    .column(local_column)
                    .expect("local assignment column")
                {
                    let flat_row = prepared.flat_constraint_row(instance, local_row).unwrap();
                    residuals[flat_row] += coefficient;
                }
            }
        }

        assert_eq!(residuals.len(), 2 * SHA256_CONSTRAINTS);
        assert!(
            residuals
                .iter()
                .all(|residual| residual == &BigInt::default())
        );
    }
}

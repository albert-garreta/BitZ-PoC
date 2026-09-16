//! The public map `h = M(1 ‖ f)` as their `MaterializedMTranspose` (CSC of
//! `M`, row-major `M^T`, from the vendored circuit crate): its digest
//! (`crates/circuit/src/matrix_transpose.rs:387-404`, blake3 over
//! `bitz/virtual-map/csc/v1`, the three counts and the two `u32` arrays) and
//! the transpose the virtual opening needs (`:323-365`, `:373-385`): for
//! every column `j` of `M` the XOR of the weights at the rows with
//! `M[i, j] = 1`, the constant column split off.

use circuit::matrix_transpose::MaterializedMTranspose;
#[cfg(feature = "parallel")]
use rayon::prelude::*;

use crate::cfg_into_iter;
use crate::poly::univariate::binary_gf128::BinaryFieldGF128 as Gf;

/// Their `DIGEST_DOMAIN`.
const DIGEST_DOMAIN: &[u8] = b"bitz/virtual-map/csc/v1";

/// Below this many nonzeros the transpose runs on one thread (their
/// `PARALLEL_MATRIX_NNZ_THRESHOLD`; values are identical either way).
const PARALLEL_NNZ: usize = 1 << 15;

/// `M^T v` split at the constant column (their `TransposedWeights`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransposedWeights {
    /// One weight per bit of `f`.
    pub weights: Vec<Gf>,
    /// The weight on the constant-one coordinate.
    pub constant_weight: Gf,
}

/// Their `MaterializedMTranspose::digest`.
pub fn map_digest(map: &MaterializedMTranspose) -> [u8; 32] {
    let (column_offsets, row_indices) = map.csc();
    let mut hasher = blake3::Hasher::new();
    hasher.update(DIGEST_DOMAIN);
    for count in [map.row_count(), map.column_count(), map.nonzero_count()] {
        hasher.update(&(count as u64).to_le_bytes());
    }
    for indices in [column_offsets, row_indices] {
        let bytes: Vec<u8> = indices.iter().flat_map(|index| index.to_le_bytes()).collect();
        hasher.update(&bytes);
    }
    *hasher.finalize().as_bytes()
}

/// Their `VirtualMap::transpose`: `weights[i]` multiplies `h[i]`; at least
/// `h_len` weights are required, any beyond multiply zero padding and are
/// ignored. Returns the `f_len − 1` bit weights and the constant weight.
pub fn transpose(map: &MaterializedMTranspose, weights: &[Gf]) -> Option<TransposedWeights> {
    let h_len = map.row_count();
    if weights.len() < h_len {
        return None;
    }
    let (column_offsets, row_indices) = map.csc();
    let columns = map.column_count();
    let evaluate = |column: usize| {
        let start = column_offsets[column] as usize;
        let end = column_offsets[column + 1] as usize;
        let mut acc = Gf::zero();
        for &row in &row_indices[start..end] {
            acc += weights[row as usize];
        }
        acc
    };
    let transposed: Vec<Gf> = if map.nonzero_count() >= PARALLEL_NNZ {
        cfg_into_iter!(0..columns, 1 << 10).map(evaluate).collect()
    } else {
        (0..columns).map(evaluate).collect()
    };
    let constant_weight = transposed[0];
    Some(TransposedWeights {
        weights: transposed[1..].to_vec(),
        constant_weight,
    })
}

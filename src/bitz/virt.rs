//! Their `common::{VirtualParams, VirtualStatement, virtual_map}` and the
//! virtual halves of `prover::prove_virtual` / `verifier::verify_virtual`: a
//! claim on the virtual bits `h = M(1 ‖ f)` is folded and reduced on `h`
//! (which the oracle never commits), the reduced inner-product claim is
//! transposed through `M^T` onto `1 ‖ f`, the constant coordinate folded
//! into the target, and the committed `f` is opened by the PCS on a
//! single-column claim over `Shape::new(m, 0)`.

#[cfg(feature = "parallel")]
use rayon::prelude::*;
use spongefish::Encoding;

use circuit::matrix_transpose::MaterializedMTranspose;

use super::map::transpose;
use super::params::{BitZParams, ClaimError, LinearClaim, LinearClaimGf, Root, Shape};
use super::pcs::{OpeningQuery, Pcs, StatementBinding};
use super::transcript::{ProverState, VerifierState};
use super::{BitZProver, BitZVerifier, ProveError, VerifyError, reduce, trace};
use crate::cfg_chunks_mut;
use crate::ligerito_flock::FlockCommitHint;
use crate::poly::univariate::binary_gf128::BinaryFieldGF128 as Gf;
use crate::poly::utils::build_eq_x_r_vec;

/// The domain that separates a virtual proof from a direct one.
pub const VIRTUAL_STATEMENT_LABEL: &[u8] = b"bitz/virtual-statement/v1";

/// A parameter set one of the pre-claim gates rejects.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VirtualParamsError {
    /// The map has no column for the constant-one coordinate.
    MissingConstantColumn,
    /// `h` has more coordinates than the claim shape indexes.
    ClaimShapeTooSmall,
    /// The map declares more bits of `f` than the committed shape holds.
    CommittedShapeTooSmall,
}

/// A map or claim with mismatched dimensions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VirtualStatementError {
    Parameters(VirtualParamsError),
    Claim(ClaimError),
}

/// A map that does not describe the protocol it belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VirtualMapError {
    PointLengthMismatch,
    ClaimWeightCountMismatch,
    WeightCountMismatch,
}

/// Their `VirtualParams<Q>`: the claim's parameters (shaping `h`) and the
/// committed shape (shaping `f`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VirtualParams {
    claim: BitZParams,
    committed: Shape,
}

impl VirtualParams {
    /// Their gates: `h` fits the claim shape, `1 ‖ f` has the constant column,
    /// `f` fits the committed shape.
    pub fn new(
        claim: BitZParams,
        committed: Shape,
        h_len: usize,
        f_len: usize,
    ) -> Result<Self, VirtualParamsError> {
        if h_len > 1 << claim.shape().log_bits() {
            return Err(VirtualParamsError::ClaimShapeTooSmall);
        }
        let committed_bits = f_len
            .checked_sub(1)
            .ok_or(VirtualParamsError::MissingConstantColumn)?;
        if committed_bits > 1 << committed.log_bits() {
            return Err(VirtualParamsError::CommittedShapeTooSmall);
        }
        Ok(Self { claim, committed })
    }

    pub fn claim(&self) -> &BitZParams {
        &self.claim
    }

    pub fn committed_shape(&self) -> &Shape {
        &self.committed
    }
}

/// Their 64-byte frame: the 48-byte `BitZParams` frame, then the committed
/// `log_rows` and `log_columns` as `u64` LE.
impl Encoding<[u8]> for VirtualParams {
    fn encode(&self) -> impl AsRef<[u8]> {
        let mut frame = [0u8; 64];
        frame[..48].copy_from_slice(self.claim.encode().as_ref());
        frame[48..56].copy_from_slice(&(self.committed.log_rows() as u64).to_le_bytes());
        frame[56..].copy_from_slice(&(self.committed.log_columns() as u64).to_le_bytes());
        frame
    }
}

/// Their `VirtualStatement`: the checked parameters, the map (its digest
/// computed once by the caller) and the input claim on `h`.
#[derive(Debug)]
pub struct VirtualStatement<'a> {
    params: VirtualParams,
    map: &'a MaterializedMTranspose,
    map_digest: [u8; 32],
    claim: &'a LinearClaim,
}

impl<'a> VirtualStatement<'a> {
    pub fn new(
        claim_params: BitZParams,
        committed_shape: Shape,
        map: &'a MaterializedMTranspose,
        map_digest: [u8; 32],
        claim: &'a LinearClaim,
    ) -> Result<Self, VirtualStatementError> {
        let params = VirtualParams::new(
            claim_params,
            committed_shape,
            map.row_count(),
            map.column_count(),
        )
        .map_err(VirtualStatementError::Parameters)?;
        if claim.row_weights().len() != claim_params.shape().rows() {
            return Err(VirtualStatementError::Claim(ClaimError::RowWeightCountMismatch));
        }
        if claim.column_weights().len() != claim_params.shape().columns() {
            return Err(VirtualStatementError::Claim(ClaimError::ColumnWeightCountMismatch));
        }
        Ok(Self {
            params,
            map,
            map_digest,
            claim,
        })
    }

    pub fn params(&self) -> &VirtualParams {
        &self.params
    }

    pub fn map(&self) -> &MaterializedMTranspose {
        self.map
    }

    pub fn map_digest(&self) -> &[u8; 32] {
        &self.map_digest
    }

    pub fn claim(&self) -> &LinearClaim {
        self.claim
    }

    /// Their `transpose_query`: the reduced claim on padded `h` (weights
    /// `column · rows + row`) pushed through `M^T`, the constant-column weight
    /// subtracted from the target, the `f` weights zero-padded to the
    /// commitment, as a single-column inner product with column weight one.
    pub fn transpose_query(&self, query: OpeningQuery) -> Result<OpeningQuery, VirtualMapError> {
        let shape = self.params.claim().shape();
        let (weights, target) = match query {
            OpeningQuery::Mle { point, target } => {
                if point.len() != shape.log_bits() {
                    return Err(VirtualMapError::PointLengthMismatch);
                }
                (build_eq_x_r_vec(&point, &()).expect("non-empty point"), target)
            }
            OpeningQuery::InnerProduct { claim } => {
                if claim.row_weights().len() != shape.rows()
                    || claim.column_weights().len() != shape.columns()
                {
                    return Err(VirtualMapError::ClaimWeightCountMismatch);
                }
                let rows = claim.row_weights();
                let columns = claim.column_weights();
                let mut weights = vec![Gf::zero(); rows.len() * columns.len()];
                cfg_chunks_mut!(weights, rows.len())
                    .enumerate()
                    .for_each(|(column, chunk)| {
                        let column_weight = columns[column];
                        for (weight, &row_weight) in chunk.iter_mut().zip(rows) {
                            *weight = row_weight * column_weight;
                        }
                    });
                (weights, claim.target())
            }
        };
        let transposed = transpose(self.map, &weights).ok_or(VirtualMapError::WeightCountMismatch)?;
        drop(weights);
        if transposed.weights.len() + 1 != self.map.column_count() {
            return Err(VirtualMapError::WeightCountMismatch);
        }
        let target = target - transposed.constant_weight;
        let mut weights = transposed.weights;
        weights.resize(1 << self.params.committed_shape().log_bits(), Gf::zero());
        let shape = Shape::new(self.params.committed_shape().log_bits(), 0)
            .expect("a valid committed bit count permits a single-column shape");
        let claim = LinearClaimGf::from_shape(&shape, weights, vec![Gf::one()], target)
            .map_err(|_| VirtualMapError::WeightCountMismatch)?;
        Ok(OpeningQuery::InnerProduct { claim })
    }
}

/// The virtual bits `h` laid out for the fold and the reduction at the claim
/// shape: per-column bit rows (column `c` = bits `c·2^t ..`) and the
/// column-lane packing the forest reads.
#[derive(Debug)]
pub struct VirtualTable {
    rows: Vec<Vec<u64>>,
    packed_cols: Vec<Vec<u64>>,
}

impl VirtualTable {
    /// From `h` as LSB-first `u64` words (`bit_len` meaningful bits, the rest
    /// of the last word zero), zero-padded to the shape.
    pub fn new(shape: &Shape, words: &[u64], bit_len: usize) -> Self {
        assert!(bit_len <= 1 << shape.log_bits(), "h does not fit the claim shape");
        assert!(words.len() * 64 >= bit_len);
        let words_per_column = shape.rows() / 64;
        let mut padded = words[..bit_len.div_ceil(64)].to_vec();
        if bit_len % 64 != 0 {
            let last = padded.len() - 1;
            padded[last] &= (1u64 << (bit_len % 64)) - 1;
        }
        padded.resize(words_per_column << shape.log_columns(), 0);
        let rows: Vec<Vec<u64>> = padded
            .chunks_exact(words_per_column)
            .map(<[u64]>::to_vec)
            .collect();
        let packed_cols = crate::ligerito::pack_columns_from_rows(&shape.layout(), &rows);
        Self { rows, packed_cols }
    }

    pub fn rows(&self) -> &[Vec<u64>] {
        &self.rows
    }

    pub fn packed_cols(&self) -> &[Vec<u64>] {
        &self.packed_cols
    }
}

impl BitZProver {
    /// Their `prove_virtual`: bind the virtual statement, fold and reduce on
    /// `h`, transpose the reduced claim onto `f`, open the commitment to `f`
    /// (`hint`, committed as one row of `2^m` bits).
    pub fn prove_virtual(
        &self,
        statement: &VirtualStatement<'_>,
        pcs: &Pcs,
        hint: &FlockCommitHint,
        virtual_bits: &VirtualTable,
        transcript: &mut ProverState,
    ) -> Result<(), ProveError> {
        let params = statement.params();
        let claim = statement.claim();
        if self.params() != params.claim()
            || pcs.bit_len() != 1 << params.committed_shape().log_bits()
        {
            return Err(ProveError::ParameterMismatch);
        }
        let shape = *self.params().shape();
        if virtual_bits.rows.len() != shape.columns()
            || virtual_bits.rows.iter().any(|row| row.len() * 64 != shape.rows())
        {
            return Err(ProveError::Witness);
        }
        let root = Root(*hint.root());

        // Step 1: bind the virtual statement before drawing fold challenges.
        transcript.public_message(VIRTUAL_STATEMENT_LABEL);
        transcript.public_message(&root.0);
        transcript.public_message(params);
        transcript.public_message(statement.map_digest());
        transcript.public_message(claim);

        // Steps 3 and 4 on the virtual columns.
        super::trace_start();
        let started = std::time::Instant::now();
        let fold = self
            .send_fold(claim, &virtual_bits.rows, transcript)
            .map_err(ProveError::Fold)?;
        trace("fold+images", started);
        let started = std::time::Instant::now();
        let query = reduce::gkr_reduce_prove(transcript, &fold, &shape, &virtual_bits.packed_cols)
            .map_err(ProveError::Reduction)?;
        trace("gkr", started);

        // The reduced claim moves from h to f.
        let started = std::time::Instant::now();
        let query = statement
            .transpose_query(query)
            .map_err(ProveError::VirtualMap)?;
        trace("transpose", started);

        // Step 6 on the committed bits.
        let started = std::time::Instant::now();
        let result = pcs
            .prove_lin(hint, &query, StatementBinding::Bind, transcript)
            .map_err(ProveError::Opening);
        trace("opening (all)", started);
        result
    }
}

impl BitZVerifier {
    /// Their `verify_virtual`, consuming the transcript so both streams are
    /// checked for exhaustion here.
    pub fn verify_virtual(
        &self,
        statement: &VirtualStatement<'_>,
        pcs: &Pcs,
        com: Root,
        mut transcript: VerifierState<'_>,
    ) -> Result<(), VerifyError> {
        let params = statement.params();
        let claim = statement.claim();
        if self.params() != params.claim()
            || pcs.bit_len() != 1 << params.committed_shape().log_bits()
        {
            return Err(VerifyError::ParameterMismatch);
        }
        transcript.public_message(VIRTUAL_STATEMENT_LABEL);
        transcript.public_message(&com.0);
        transcript.public_message(params);
        transcript.public_message(statement.map_digest());
        transcript.public_message(claim);

        let started = std::time::Instant::now();
        let fold = self
            .receive_fold(claim, &mut transcript)
            .map_err(VerifyError::Fold)?;
        trace("v: fold", started);
        let started = std::time::Instant::now();
        let query = reduce::gkr_reduce_verify(&mut transcript, &fold, self.params().shape())
            .map_err(VerifyError::Reduction)?;
        trace("v: gkr", started);
        let started = std::time::Instant::now();
        let query = statement
            .transpose_query(query)
            .map_err(VerifyError::VirtualMap)?;
        trace("v: transpose", started);
        let started = std::time::Instant::now();
        pcs.verify_lin(&com, &query, StatementBinding::Bind, &mut transcript)
            .map_err(VerifyError::Opening)?;
        trace("v: opening", started);
        transcript
            .check_eof()
            .map_err(|_| VerifyError::TrailingData)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pcs::smallest_generator;

    #[test]
    fn the_virtual_frame_is_the_params_frame_plus_the_committed_split() {
        let claim = BitZParams::new(Shape::new(7, 15).unwrap(), super::super::fq::Q, smallest_generator()).unwrap();
        let params = VirtualParams::new(claim, Shape::new(8, 14).unwrap(), 4, 4).unwrap();
        let encoded = params.encode();
        let encoded = encoded.as_ref();
        assert_eq!(encoded.len(), 64);
        assert_eq!(&encoded[..48], claim.encode().as_ref());
        assert_eq!(&encoded[48..56], &8u64.to_le_bytes());
        assert_eq!(&encoded[56..], &14u64.to_le_bytes());
        assert_eq!(
            VirtualParams::new(claim, Shape::new(8, 14).unwrap(), (1 << 22) + 1, 4).err(),
            Some(VirtualParamsError::ClaimShapeTooSmall)
        );
        assert_eq!(
            VirtualParams::new(claim, Shape::new(8, 14).unwrap(), 4, 0).err(),
            Some(VirtualParamsError::MissingConstantColumn)
        );
        assert_eq!(
            VirtualParams::new(claim, Shape::new(8, 14).unwrap(), 4, (1 << 22) + 2).err(),
            Some(VirtualParamsError::CommittedShapeTooSmall)
        );
    }

    #[test]
    fn the_virtual_table_splits_h_into_columns() {
        let shape = Shape::new(7, 15).unwrap();
        // Bits 0, 129 and 200 set: column 0 row 0, column 1 row 1, column 1 row 72.
        let words = [1u64, 0, 2, 1 << 8];
        let table = VirtualTable::new(&shape, &words, 201);
        assert_eq!(table.rows().len(), 1 << 15);
        assert_eq!(table.rows()[0], vec![1, 0]);
        assert_eq!(table.rows()[1], vec![2, 1 << 8]);
        assert!(table.rows()[2..].iter().all(|row| row == &[0, 0]));
        assert_eq!(table.packed_cols().len(), (1 << 15) / 64);
        // Lane c of word b holds bit b of column c.
        assert_eq!(table.packed_cols()[0][0] & 0b11, 0b01);
        assert_eq!(table.packed_cols()[0][1] & 0b11, 0b10);
        assert_eq!(table.packed_cols()[0][72] & 0b11, 0b10);
    }
}

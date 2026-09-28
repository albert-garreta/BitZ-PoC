//! Opening a claim on `h = (Id_N ⊗ M)·f` through a block-layout commitment.
//!
//! The generic virtual opening ([`super::virt`]) rewrites the GKR's exit
//! claim `⟨u₁ ⊗ u₂, h⟩` as `⟨Mᵀ(u₁ ⊗ u₂), f⟩` with one dense weight per
//! committed bit, so both roles pay `nnz` of the whole map and the verifier
//! folds `2^m` weights. When the map repeats one local map `M` over
//! `N = 2^k` independent instances, and both grids hold every instance in
//! its own power-of-two block (source cell `i·2^a + j`, derived cell
//! `i·2^b + ℓ`, `2^a ≥ M.cols()`, `2^b ≥ M.rows()`), the transpose factors.
//! Take the same `k − r` high instance bits as the columns of both grids:
//! the derived grid has `2^(b+r)` rows, the committed one `2^(a+r)`, and
//!
//! `⟨u₁ ⊗ u₂, h⟩ = ⟨R ⊗ u₂, f⟩`,  `R[lo·2^a + j] = Σ_{ℓ : M[ℓ, j] = 1} u₁[lo·2^b + ℓ]`
//!
//! for the `2^r` low instance values `lo`: `2^r·nnz(M)` field additions for
//! either role, a rank-one claim on the committed grid, no dense weights.
//! Committed cells outside the local map's columns get weight zero, exactly
//! as `Mᵀ` gives them: nothing reads them, and nothing weights them.

use circuit::linear_map::binary::{PreparedVirtualMap, VirtualMap};
use field::Gf128 as Gf;
use spongefish::Encoding;

use super::{
    BitZParams, BitZProver, BitZVerifier, LinearClaim, Pcs, ProveError, ProverState, Shape,
    VerifierState, VerifyError,
    params::{LinearClaimGf, Root},
    pcs::{OpeningQuery, StatementBinding},
    reduce,
};
use crate::{ligerito_flock::FlockCommitHint, pcs::IntegerMatrixLayout};

/// The frame separating a block-layout proof from a direct or virtual one.
const BLOCK_STATEMENT_LABEL: &[u8] = b"bitz/block-statement/v1";

/// A block map or statement the opening cannot take.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockError {
    /// The local map is empty, or a grid does not fit the index width.
    Map,
    /// The claim grid is not `2^(b+r) × 2^(k−r)` for some `r ≤ k`.
    ClaimShape,
    /// The committed grid is not `2^(a+r) × 2^(k−r)` for the claim's `r`.
    CommittedShape,
    /// The claim's weights do not match the claim grid.
    Claim,
    /// The prover's or the scheme's parameters are not the statement's.
    ParameterMismatch,
    /// The reduced query is not the factored claim the GKR produces.
    UnsupportedQuery,
}

/// `Id_N ⊗ M` over power-of-two blocks, instance-major on both sides:
/// source cell `i·2^a + j` feeds derived cell `i·2^b + ℓ` iff `M[ℓ, j] = 1`
/// (`2^a` = `M.cols()` and `2^b` = `M.rows()` rounded up to powers of two).
/// Only the local CSC is stored.
#[derive(Clone, Debug)]
pub struct BlockRepeatedMap {
    local: PreparedVirtualMap,
    log_instances: usize,
    log_source_block: usize,
    log_derived_block: usize,
    nnz: usize,
    digest: [u8; 32],
}

impl BlockRepeatedMap {
    pub fn new(local: PreparedVirtualMap, log_instances: usize) -> Result<Self, BlockError> {
        if local.rows() == 0 || local.cols() == 0 {
            return Err(BlockError::Map);
        }
        let log_source_block = local.cols().next_power_of_two().ilog2() as usize;
        let log_derived_block = local.rows().next_power_of_two().ilog2() as usize;
        if log_instances + log_source_block.max(log_derived_block) >= usize::BITS as usize - 1 {
            return Err(BlockError::Map);
        }
        let nnz = local
            .nnz()
            .checked_mul(1 << log_instances)
            .ok_or(BlockError::Map)?;
        let mut hash = blake3::Hasher::new();
        hash.update(b"bitz/block-repeated-map/v1");
        hash.update(&local.digest());
        for value in [log_instances, log_source_block, log_derived_block, nnz] {
            hash.update(&(value as u64).to_le_bytes());
        }
        let digest = *hash.finalize().as_bytes();
        Ok(Self {
            local,
            log_instances,
            log_source_block,
            log_derived_block,
            nnz,
            digest,
        })
    }

    pub fn local(&self) -> &PreparedVirtualMap {
        &self.local
    }

    pub fn log_instances(&self) -> usize {
        self.log_instances
    }

    /// `a`: the source block is `2^a` cells.
    pub fn log_source_block(&self) -> usize {
        self.log_source_block
    }

    /// `b`: the derived block is `2^b` cells.
    pub fn log_derived_block(&self) -> usize {
        self.log_derived_block
    }

    /// The committed grid that pairs with a claim grid of `2^(b+r)` rows:
    /// `2^(a+r)` rows (`2^r` whole source blocks per column), the same
    /// `2^(k−r)` columns.
    pub fn committed_layout(&self, claim_rows: usize) -> Result<IntegerMatrixLayout, BlockError> {
        let r = claim_rows
            .checked_sub(self.log_derived_block)
            .filter(|&r| r <= self.log_instances)
            .ok_or(BlockError::ClaimShape)?;
        Ok(IntegerMatrixLayout {
            row_vars: self.log_source_block + r,
            col_vars: self.log_instances - r,
            word_bits: 1,
        })
    }
}

/// Derived rows of one source column of a [`BlockRepeatedMap`].
pub struct BlockColumnRows<'a> {
    rows: core::slice::Iter<'a, usize>,
    base: usize,
}

impl Iterator for BlockColumnRows<'_> {
    type Item = usize;

    fn next(&mut self) -> Option<usize> {
        self.rows.next().map(|row| self.base + row)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.rows.size_hint()
    }
}

impl ExactSizeIterator for BlockColumnRows<'_> {}

impl VirtualMap for BlockRepeatedMap {
    type ColumnRows<'a> = BlockColumnRows<'a>;

    fn rows(&self) -> usize {
        1 << (self.log_instances + self.log_derived_block)
    }

    fn cols(&self) -> usize {
        1 << (self.log_instances + self.log_source_block)
    }

    fn nnz(&self) -> usize {
        self.nnz
    }

    fn digest(&self) -> [u8; 32] {
        self.digest
    }

    fn is_identity(&self) -> bool {
        self.log_instances == 0
            && self.local.is_identity()
            && self.local.rows() == 1 << self.log_derived_block
            && self.log_source_block == self.log_derived_block
    }

    fn column_rows(&self, column: usize) -> Option<Self::ColumnRows<'_>> {
        if column >= self.cols() {
            return None;
        }
        let instance = column >> self.log_source_block;
        let local_column = column & ((1 << self.log_source_block) - 1);
        let rows: &[usize] = if local_column < self.local.cols() {
            self.local.matrix().column(local_column)?.indices()
        } else {
            &[]
        };
        Some(BlockColumnRows {
            rows: rows.iter(),
            base: instance << self.log_derived_block,
        })
    }
}

/// A claim on the derived grid of a [`BlockRepeatedMap`], with the committed
/// block grid it is opened against.
///
/// Both roles bind the label, the root, the claim parameters, the committed
/// shape, the map's digest and the claim before the first fold challenge.
pub struct BlockStatement<'a> {
    claim_params: BitZParams,
    committed: Shape,
    map: &'a BlockRepeatedMap,
    claim: &'a LinearClaim,
    /// `r`: the low instance bits on the rows of both grids.
    low_instance_bits: usize,
}

impl<'a> BlockStatement<'a> {
    pub fn new(
        claim_params: BitZParams,
        committed: Shape,
        map: &'a BlockRepeatedMap,
        claim: &'a LinearClaim,
    ) -> Result<Self, BlockError> {
        let shape = claim_params.shape();
        let r = shape
            .log_rows()
            .checked_sub(map.log_derived_block)
            .filter(|&r| r <= map.log_instances)
            .ok_or(BlockError::ClaimShape)?;
        if shape.log_columns() != map.log_instances - r {
            return Err(BlockError::ClaimShape);
        }
        let expected = map.committed_layout(shape.log_rows())?;
        if committed.log_rows() != expected.row_vars || committed.log_columns() != expected.col_vars {
            return Err(BlockError::CommittedShape);
        }
        if claim.row_weights().len() != shape.rows()
            || claim.column_weights().len() != shape.columns()
        {
            return Err(BlockError::Claim);
        }
        Ok(Self {
            claim_params,
            committed,
            map,
            claim,
            low_instance_bits: r,
        })
    }

    pub fn claim_params(&self) -> &BitZParams {
        &self.claim_params
    }

    pub fn committed_shape(&self) -> Shape {
        self.committed
    }

    pub fn map(&self) -> &BlockRepeatedMap {
        self.map
    }

    pub fn claim(&self) -> &LinearClaim {
        self.claim
    }

    /// The bytes both roles bind after the root: the claim parameters, the
    /// committed shape, the map digest.
    fn frame(&self) -> Vec<u8> {
        let mut frame = Vec::with_capacity(48 + 16 + 32);
        frame.extend_from_slice(Encoding::<[u8]>::encode(&self.claim_params).as_ref());
        frame.extend_from_slice(&(self.committed.log_rows() as u64).to_le_bytes());
        frame.extend_from_slice(&(self.committed.log_columns() as u64).to_le_bytes());
        frame.extend_from_slice(&self.map.digest);
        frame
    }

    /// Rewrites the GKR's factored claim on the derived grid into the
    /// factored claim on the committed grid: the column weights carry over
    /// (the same high instance bits), and the row weight of committed row
    /// `lo·2^a + j` is the sum of the derived row weights `lo·2^b + ℓ` over
    /// the local rows `ℓ` that local column `j` feeds (zero for `j` past the
    /// local map's columns).
    pub fn transpose_query(&self, query: OpeningQuery) -> Result<OpeningQuery, BlockError> {
        let OpeningQuery::InnerProduct { claim } = query else {
            return Err(BlockError::UnsupportedQuery);
        };
        let shape = self.claim_params.shape();
        let (row_w, col_w) = (claim.row_weights(), claim.column_weights());
        if row_w.len() != shape.rows() || col_w.len() != shape.columns() {
            return Err(BlockError::Claim);
        }
        let local = self.map.local.matrix();
        let (a, b) = (self.map.log_source_block, self.map.log_derived_block);
        let mut rows = vec![Gf::zero(); self.committed.rows()];
        for (lo, block) in rows.chunks_mut(1 << a).enumerate() {
            let derived = &row_w[lo << b..(lo + 1) << b];
            for (j, weight) in block.iter_mut().enumerate().take(self.map.local.cols()) {
                let entries = local.column(j).ok_or(BlockError::Map)?;
                let mut acc = Gf::zero();
                for &l in entries.indices() {
                    acc += derived[l];
                }
                *weight = acc;
            }
        }
        let claim = LinearClaimGf::from_shape(&self.committed, rows, col_w.to_vec(), claim.target())
            .map_err(|_| BlockError::Claim)?;
        debug_assert_eq!(self.low_instance_bits, self.committed.log_rows() - a);
        Ok(OpeningQuery::InnerProduct { claim })
    }
}

impl BitZProver {
    /// Proves `claim` on the derived grid of a [`BlockRepeatedMap`] and opens
    /// it through the block-layout commitment (`hint`, made under `pcs` in
    /// the statement's committed shape). `derived_rows` are the per-column
    /// bit rows of `h` in the claim grid's shape. `ood` as in
    /// [`BitZProver::prove`].
    pub fn prove_block(
        &self,
        statement: &BlockStatement<'_>,
        pcs: &Pcs,
        hint: &FlockCommitHint,
        derived_rows: &[Vec<u64>],
        transcript: &mut ProverState,
        ood: Option<(&[Gf], Gf)>,
    ) -> Result<(), ProveError> {
        let shape = *self.params.shape();
        let committed = statement.committed;
        if self.params != statement.claim_params
            || pcs.bit_len() != 1usize << committed.log_bits()
            || hint.rows().len() != committed.columns()
            || hint.rows().iter().any(|row| row.len() * 64 != committed.rows())
        {
            return Err(ProveError::Block(BlockError::ParameterMismatch));
        }
        if derived_rows.len() != shape.columns()
            || derived_rows.iter().any(|row| row.len() * 64 != shape.rows())
        {
            return Err(ProveError::Witness);
        }
        let root = Root(*hint.root());
        transcript.public_message(BLOCK_STATEMENT_LABEL);
        transcript.public_message(&root.0);
        transcript.public_message(statement.frame().as_slice());
        transcript.public_message(statement.claim);

        // The derived grid's packing and the forest's nibble rows come first:
        // the column folds read the nibble rows too (as the direct opener's).
        super::trace_start();
        let started = std::time::Instant::now();
        let layout = IntegerMatrixLayout {
            row_vars: shape.log_rows(),
            col_vars: shape.log_columns(),
            word_bits: 1,
        };
        let packed = crate::ligerito::pack_columns_from_rows(&layout, derived_rows);
        let nibble = {
            let started = std::time::Instant::now();
            let nibble = super::forest::nibble_rows_for(shape.log_rows(), shape.log_columns(), &packed);
            if nibble.is_some() {
                super::trace("  nibble rows", started);
            }
            nibble
        };
        let fold = self
            .send_fold_with(statement.claim, derived_rows, nibble.as_ref(), transcript)
            .map_err(ProveError::Fold)?;
        super::trace("fold+images", started);
        let started = std::time::Instant::now();
        let query = reduce::gkr_reduce_prove_packed_with(transcript, &fold, &shape, &packed, nibble)
            .map_err(ProveError::Reduction)?;
        drop(packed);
        super::trace("gkr", started);

        let started = std::time::Instant::now();
        let query = statement.transpose_query(query).map_err(ProveError::Block)?;
        super::trace("transpose", started);

        let started = std::time::Instant::now();
        let result = pcs
            .prove_lin(hint, &query, StatementBinding::Bind, transcript, ood)
            .map_err(ProveError::Opening);
        super::trace("opening (all)", started);
        result
    }
}

impl BitZVerifier {
    /// Checks a [`BitZProver::prove_block`] proof, consuming the transcript.
    pub fn verify_block(
        &self,
        statement: &BlockStatement<'_>,
        pcs: &Pcs,
        com: Root,
        mut transcript: VerifierState<'_>,
        ood: Option<(&[Gf], Gf)>,
    ) -> Result<(), VerifyError> {
        if self.params != statement.claim_params
            || pcs.bit_len() != 1usize << statement.committed.log_bits()
        {
            return Err(VerifyError::Block(BlockError::ParameterMismatch));
        }
        transcript.public_message(BLOCK_STATEMENT_LABEL);
        transcript.public_message(&com.0);
        transcript.public_message(statement.frame().as_slice());
        transcript.public_message(statement.claim);

        let started = std::time::Instant::now();
        let fold = self
            .receive_fold(statement.claim, &mut transcript)
            .map_err(VerifyError::Fold)?;
        super::trace("v: fold", started);
        let started = std::time::Instant::now();
        let query = reduce::gkr_reduce_verify(&mut transcript, &fold, self.params.shape())
            .map_err(VerifyError::Reduction)?;
        super::trace("v: gkr", started);
        let started = std::time::Instant::now();
        let query = statement.transpose_query(query).map_err(VerifyError::Block)?;
        super::trace("v: transpose", started);
        let started = std::time::Instant::now();
        pcs.verify_lin(&com, &query, StatementBinding::Bind, &mut transcript, ood)
            .map_err(VerifyError::Opening)?;
        super::trace("v: opening", started);
        transcript
            .check_eof()
            .map_err(|_| VerifyError::TrailingData)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wfbitz::{VirtualStatement, params::LinearClaimGf};
    use circuit::linear_map::CscMatrix;

    fn local_map(rows: usize, cols: usize, seed: u64) -> PreparedVirtualMap {
        let mut state = seed | 1;
        let mut next = || {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            state
        };
        let columns = (0..cols)
            .map(|_| {
                let mut entries: Vec<usize> = (0..rows).filter(|_| next() % 3 == 0).collect();
                entries.dedup();
                entries.into_iter().map(|row| (row, true)).collect()
            })
            .collect();
        PreparedVirtualMap::new(CscMatrix::try_from_columns(rows, columns).unwrap()).unwrap()
    }

    fn gf(seed: u64) -> Gf {
        let low = seed.wrapping_mul(0x9e37_79b9_7f4a_7c15) ^ 0x0123_4567_89ab_cdef;
        let high = seed.wrapping_mul(0xc2b2_ae3d_27d4_eb4f).rotate_left(17);
        Gf::from_bytes(((u128::from(high) << 64) | u128::from(low)).to_le_bytes())
    }

    /// Every source column's derived rows follow the block definition.
    #[test]
    fn column_rows_follow_the_block_definition() {
        let local = local_map(11, 5, 7);
        let map = BlockRepeatedMap::new(local.clone(), 3).unwrap();
        assert_eq!((map.log_source_block(), map.log_derived_block()), (3, 4));
        assert_eq!((map.rows(), map.cols()), (128, 64));
        let mut nnz = 0;
        for column in 0..map.cols() {
            let (instance, j) = (column >> 3, column & 7);
            let expected: Vec<usize> = if j < 5 {
                local.matrix().column(j).unwrap().indices().iter().map(|&l| instance * 16 + l).collect()
            } else {
                Vec::new()
            };
            let got: Vec<usize> = map.column_rows(column).unwrap().collect();
            assert_eq!(got, expected, "column {column}");
            nnz += got.len();
        }
        assert_eq!(nnz, map.nnz());
        assert!(map.column_rows(map.cols()).is_none());
    }

    /// The structured transpose equals the generic dense one cell for cell,
    /// padding cells (zero weight) included, for every row split `r`.
    #[test]
    fn structured_transpose_matches_the_dense_transpose() {
        // Blocks of 2^8 derived and 2^7 source cells (unequal on purpose),
        // with 58 source cells past the local columns.
        let local = local_map(200, 70, 11);
        let log_instances = 3;
        let map = BlockRepeatedMap::new(local, log_instances).unwrap();
        let (a, b) = (map.log_source_block(), map.log_derived_block());
        let q = (1u128 << 61) - 1;
        for r in 0..=log_instances {
            let derived = Shape::new(b + r, log_instances - r).unwrap();
            let committed = Shape::new(a + r, log_instances - r).unwrap();
            let params = BitZParams::new(derived, q, crate::piop::spartan::protocol::bitz_generator().into())
                .unwrap();
            let claim = LinearClaim::new(
                &params,
                (0..derived.rows() as u128).map(|i| i % q).collect(),
                (0..derived.columns() as u128).map(|i| (3 * i + 1) % q).collect(),
                5,
            )
            .unwrap();
            let statement = BlockStatement::new(params, committed, &map, &claim).unwrap();
            let exit = LinearClaimGf::from_shape(
                &derived,
                (0..derived.rows() as u64).map(gf).collect(),
                (0..derived.columns() as u64).map(|i| gf(1000 + i)).collect(),
                gf(99),
            )
            .unwrap();
            let OpeningQuery::InnerProduct { claim: structured } = statement
                .transpose_query(OpeningQuery::InnerProduct { claim: exit.clone() })
                .unwrap()
            else {
                panic!("rank-one claim expected");
            };
            let virt = VirtualStatement::new(params, committed, &map, &claim).unwrap();
            let OpeningQuery::InnerProduct { claim: dense } = virt
                .transpose_query(OpeningQuery::InnerProduct { claim: exit })
                .unwrap()
            else {
                panic!("single-column claim expected");
            };
            assert_eq!(structured.target(), dense.target());
            let dense = dense.row_weights();
            assert_eq!(dense.len(), committed.rows() * committed.columns());
            for column in 0..committed.columns() {
                for row in 0..committed.rows() {
                    let cell = column * committed.rows() + row;
                    assert_eq!(
                        structured.row_weights()[row] * structured.column_weights()[column],
                        dense[cell],
                        "r={r} cell {cell}"
                    );
                }
            }
            // Cells past the local columns carry no weight.
            for lo in 0..1usize << r {
                for j in 70..1 << a {
                    assert!(structured.row_weights()[(lo << a) + j].is_zero());
                }
            }
        }
    }

    /// A claim grid or committed grid that is not the block pairing is
    /// rejected.
    #[test]
    fn mismatched_grids_are_rejected() {
        let map = BlockRepeatedMap::new(local_map(200, 70, 3), 4).unwrap();
        assert_eq!((map.log_source_block(), map.log_derived_block()), (7, 8));
        let q = (1u128 << 61) - 1;
        let generator = crate::piop::spartan::protocol::bitz_generator().into();
        let derived = Shape::new(8 + 1, 3).unwrap();
        let params = BitZParams::new(derived, q, generator).unwrap();
        let claim = LinearClaim::new(&params, vec![1; 512], vec![1; 8], 0).unwrap();
        assert!(BlockStatement::new(params, Shape::new(7 + 1, 3).unwrap(), &map, &claim).is_ok());
        assert_eq!(
            BlockStatement::new(params, Shape::new(7 + 2, 2).unwrap(), &map, &claim).err(),
            Some(BlockError::CommittedShape)
        );
        let wide = Shape::new(8 + 1, 4).unwrap();
        let wide_params = BitZParams::new(wide, q, generator).unwrap();
        let wide_claim = LinearClaim::new(&wide_params, vec![1; 512], vec![1; 16], 0).unwrap();
        assert_eq!(
            BlockStatement::new(wide_params, Shape::new(7 + 1, 4).unwrap(), &map, &wide_claim).err(),
            Some(BlockError::ClaimShape)
        );
    }
}

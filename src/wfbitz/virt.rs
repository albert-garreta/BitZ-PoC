//! Opening a claim on virtual bits `h = M·f` through the commitment to `f`.
//!
//! worldfnd/f2z-benchmark's `prove_virtual` (their commit 60bbe54, in
//! `common::virtual_map`, `prover::prove` and `verifier::verify`), carried
//! onto this crate's virtual maps: the fold and the grand product never read
//! the oracle, so they run over the derived grid `h` exactly as the direct
//! scheme runs over committed bits; the opening does read it, so the reduced
//! factored claim `<v, h>` is rewritten as `<Mᵀv, f>` — one dense weight per
//! committed bit, gathered through the map's CSC columns — and opened as a
//! single-column inner product on the committed bits (their design). The
//! prover's cost is the derived-grid GKR plus `nnz(M)` products for the
//! transpose plus an `m`-round dense sumcheck over the `2^m` committed bits;
//! the verifier repeats the transpose and folds the `2^m` weights, so it is
//! linear in the commitment — the price of the simple design.
//!
//! Their maps carry a constant-one column in front of `f`; this crate's maps
//! ([`VirtualMap`]) index committed cells only (a constant, where a relation
//! needs one, is a committed cell), so no target adjustment is made.
//!
//! Cell order is the crate's column-major one on both grids: cell
//! `c · 2^t + b` is row `b` of column `c`, the order the per-column bit rows
//! flatten to and the order [`LinearClaimGf`] weights.

use circuit::linear_map::binary::VirtualMap;
use field::Gf128 as Gf;
use flock_core::merkle::HashKind;
use num_traits::{One, Zero};
#[cfg(feature = "parallel")]
use rayon::prelude::*;
use spongefish::Encoding;

use super::{
    BitZParams, BitZProver, BitZVerifier, LinearClaim, Pcs, ProveError, ProverState, Shape,
    VerifierState, VerifyError,
    params::{ClaimError, LinearClaimGf, Root, ShapeError},
    pcs::{OpeningQuery, StatementBinding},
    reduce,
};
use crate::{cfg_into_iter, ligerito_flock::FlockCommitHint, pcs::IntegerMatrixLayout};

/// The frame separating a virtual proof from a direct one.
const VIRTUAL_STATEMENT_LABEL: &[u8] = b"bitz/virtual-statement/v1";

/// A statement or map the virtual opening cannot take.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VirtualError {
    /// The map has more derived cells than the claim grid.
    ClaimShapeTooSmall,
    /// The map has more source cells than the committed grid.
    CommittedShapeTooSmall,
    /// The claim's weights do not match the claim grid.
    Claim(ClaimError),
    /// The single-column committed shape is not a valid shape.
    Shape(ShapeError),
    /// The prover's or the scheme's parameters are not the statement's.
    ParameterMismatch,
    /// The reduced query is not the factored claim the GKR produces.
    UnsupportedQuery,
}

/// A claim on the derived grid `h = M·f`, with the committed grid it is
/// opened against and the map between them.
///
/// Both roles bind the label, the root, the claim parameters, the committed
/// shape, the map's digest and the claim before the first fold challenge.
pub struct VirtualStatement<'a, M: VirtualMap> {
    claim_params: BitZParams,
    committed: Shape,
    map: &'a M,
    claim: &'a LinearClaim,
}

impl<'a, M: VirtualMap> VirtualStatement<'a, M> {
    /// Checks the map and the claim against both grids. The statement keeps
    /// the borrowed map and claim; the map must not change underneath it.
    pub fn new(
        claim_params: BitZParams,
        committed: Shape,
        map: &'a M,
        claim: &'a LinearClaim,
    ) -> Result<Self, VirtualError> {
        if map.rows() > 1usize << claim_params.shape().log_bits() {
            return Err(VirtualError::ClaimShapeTooSmall);
        }
        if map.cols() > 1usize << committed.log_bits() {
            return Err(VirtualError::CommittedShapeTooSmall);
        }
        if claim.row_weights().len() != claim_params.shape().rows() {
            return Err(VirtualError::Claim(ClaimError::RowWeightCountMismatch));
        }
        if claim.column_weights().len() != claim_params.shape().columns() {
            return Err(VirtualError::Claim(ClaimError::ColumnWeightCountMismatch));
        }
        Ok(Self {
            claim_params,
            committed,
            map,
            claim,
        })
    }

    pub fn claim_params(&self) -> &BitZParams {
        &self.claim_params
    }

    pub fn committed_shape(&self) -> Shape {
        self.committed
    }

    pub fn map(&self) -> &M {
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
        frame.extend_from_slice(&self.map.digest());
        frame
    }

    /// Rewrites the GKR's factored claim on the derived grid into the
    /// single-column claim on the committed grid: weight of committed cell
    /// `s` = `Σ_{d ∈ column_rows(s)} row[d mod 2^t]·col[d div 2^t]`, one
    /// product per nonzero of the map, the cells spread over the thread pool.
    pub fn transpose_query(&self, query: OpeningQuery) -> Result<OpeningQuery, VirtualError> {
        let OpeningQuery::InnerProduct { claim } = query else {
            return Err(VirtualError::UnsupportedQuery);
        };
        let shape = self.claim_params.shape();
        let (row_w, col_w) = (claim.row_weights(), claim.column_weights());
        if row_w.len() != shape.rows() || col_w.len() != shape.columns() {
            return Err(VirtualError::Claim(ClaimError::RowWeightCountMismatch));
        }
        let t = shape.log_rows();
        let mask = shape.rows() - 1;
        let derived = 1usize << shape.log_bits();
        let cells = 1usize << self.committed.log_bits();
        let map = self.map;
        let weights: Vec<Gf> = cfg_into_iter!(0..cells, 1 << 12)
            .map(|source| {
                let mut acc = Gf::zero();
                if let Some(rows) = map.column_rows(source) {
                    for d in rows {
                        if d < derived {
                            acc += row_w[d & mask] * col_w[d >> t];
                        }
                    }
                }
                acc
            })
            .collect();
        let single = Shape::new(self.committed.log_bits(), 0).map_err(VirtualError::Shape)?;
        let claim = LinearClaimGf::from_shape(&single, weights, vec![Gf::one()], claim.target())
            .map_err(VirtualError::Claim)?;
        Ok(OpeningQuery::InnerProduct { claim })
    }
}

/// The committed rows as the one row of the single-column claim shape:
/// column after column, cell `c · 2^t + b` at bit `c · 2^t + b`.
fn flatten_rows(rows: &[Vec<u64>]) -> Vec<u64> {
    let mut flat = Vec::with_capacity(rows.iter().map(Vec::len).sum());
    for row in rows {
        flat.extend_from_slice(row);
    }
    flat
}

impl BitZProver {
    /// Proves `claim` on the derived grid `h = M·f` and opens it through the
    /// commitment to `f` (`hint`, made under `pcs`). `derived_rows` are the
    /// per-column bit rows of `h` in the claim grid's shape. `ood` as in
    /// [`BitZProver::prove`].
    pub fn prove_virtual<M: VirtualMap>(
        &self,
        statement: &VirtualStatement<'_, M>,
        pcs: &Pcs,
        hint: &FlockCommitHint,
        derived_rows: &[Vec<u64>],
        transcript: &mut ProverState,
        ood: Option<(&[Gf], Gf)>,
    ) -> Result<(), ProveError> {
        let shape = *self.params.shape();
        if self.params != statement.claim_params
            || pcs.bit_len() != 1usize << statement.committed.log_bits()
        {
            return Err(ProveError::Virtual(VirtualError::ParameterMismatch));
        }
        if derived_rows.len() != shape.columns()
            || derived_rows.iter().any(|row| row.len() * 64 != shape.rows())
        {
            return Err(ProveError::Witness);
        }
        let root = Root(*hint.root());
        transcript.public_message(VIRTUAL_STATEMENT_LABEL);
        transcript.public_message(&root.0);
        transcript.public_message(statement.frame().as_slice());
        transcript.public_message(statement.claim);

        super::trace_start();
        let started = std::time::Instant::now();
        let fold = self
            .send_fold(statement.claim, derived_rows, transcript)
            .map_err(ProveError::Fold)?;
        super::trace("fold+images", started);
        let started = std::time::Instant::now();
        let layout = IntegerMatrixLayout {
            row_vars: shape.log_rows(),
            col_vars: shape.log_columns(),
            word_bits: 1,
        };
        let packed = crate::ligerito::pack_columns_from_rows(&layout, derived_rows);
        let query = reduce::gkr_reduce_prove_packed(transcript, &fold, &shape, &packed)
            .map_err(ProveError::Reduction)?;
        drop(packed);
        super::trace("gkr", started);

        let started = std::time::Instant::now();
        let query = statement.transpose_query(query).map_err(ProveError::Virtual)?;
        super::trace("transpose", started);

        let started = std::time::Instant::now();
        let flat = flatten_rows(hint.rows());
        let result = pcs
            .prove_lin_rows(
                hint,
                core::slice::from_ref(&flat),
                &[],
                &query,
                StatementBinding::Bind,
                transcript,
                ood,
            )
            .map_err(ProveError::Opening);
        super::trace("opening (all)", started);
        result
    }
}

impl BitZVerifier {
    /// Checks a [`BitZProver::prove_virtual`] proof, consuming the transcript.
    pub fn verify_virtual<M: VirtualMap>(
        &self,
        statement: &VirtualStatement<'_, M>,
        pcs: &Pcs,
        com: Root,
        mut transcript: VerifierState<'_>,
        ood: Option<(&[Gf], Gf)>,
    ) -> Result<(), VerifyError> {
        if self.params != statement.claim_params
            || pcs.bit_len() != 1usize << statement.committed.log_bits()
        {
            return Err(VerifyError::Virtual(VirtualError::ParameterMismatch));
        }
        transcript.public_message(VIRTUAL_STATEMENT_LABEL);
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
        let query = statement.transpose_query(query).map_err(VerifyError::Virtual)?;
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

/// The scheme's Ligerito ladder for a committed shape, as shipped.
pub fn committed_pcs(committed: &Shape) -> Result<Pcs, super::pcs::ConfigError> {
    Pcs::new(committed, HashKind::Blake3)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wfbitz::{
        WINDOW, build_prover, build_verifier,
        fold::{fold_columns, reconstruct},
    };
    use circuit::linear_map::{CscMatrix, binary::PreparedVirtualMap};

    const Q: u128 = (1u128 << 100) - 15;

    fn xorshift(state: &mut u64) -> u64 {
        *state ^= *state << 13;
        *state ^= *state >> 7;
        *state ^= *state << 17;
        *state
    }

    /// Every committed cell feeds two derived cells (its own index and a
    /// scrambled one), so derived cells collect one or several sources.
    fn map(cells_f: usize, cells_h: usize) -> PreparedVirtualMap {
        let mut offsets = Vec::with_capacity(cells_f + 1);
        let mut indices = Vec::with_capacity(2 * cells_f);
        offsets.push(0usize);
        for source in 0..cells_f {
            let a = source % cells_h;
            let b = (source.wrapping_mul(0x9e37_79b9) ^ 0x5bd1) % cells_h;
            let (lo, hi) = if a < b { (a, b) } else { (b, a) };
            indices.push(lo);
            if hi != lo {
                indices.push(hi);
            }
            offsets.push(indices.len());
        }
        let matrix = CscMatrix::try_from_binary_csc(cells_h, offsets, indices).expect("csc");
        PreparedVirtualMap::from_implicit(matrix).expect("map")
    }

    fn bit(rows: &[Vec<u64>], t: usize, cell: usize) -> bool {
        let (c, b) = (cell >> t, cell & ((1 << t) - 1));
        rows[c][b / 64] >> (b % 64) & 1 == 1
    }

    fn setup() -> (
        Shape,
        Shape,
        BitZParams,
        Vec<Vec<u64>>,
        Vec<Vec<u64>>,
        PreparedVirtualMap,
        LinearClaim,
    ) {
        let committed = Shape::new(15, 7).unwrap();
        let derived = Shape::new(14, 8).unwrap();
        let alpha: Gf = crate::pcs::smallest_generator().into();
        let params = BitZParams::new(derived, Q, alpha).unwrap();
        let mut state = 0x1234_5678_9abc_def0u64;
        let words_f = committed.rows() / 64;
        let rows_f: Vec<Vec<u64>> = (0..committed.columns())
            .map(|_| (0..words_f).map(|_| xorshift(&mut state)).collect())
            .collect();
        let cells_f = 1usize << committed.log_bits();
        let cells_h = 1usize << derived.log_bits();
        let map = map(cells_f, cells_h);
        // h = M f.
        let mut h = vec![0u64; cells_h / 64];
        for source in 0..cells_f {
            if bit(&rows_f, committed.log_rows(), source) {
                for d in map.column_rows(source).unwrap() {
                    h[d / 64] ^= 1u64 << (d % 64);
                }
            }
        }
        let words_h = derived.rows() / 64;
        let rows_h: Vec<Vec<u64>> = (0..derived.columns())
            .map(|c| h[c * words_h..(c + 1) * words_h].to_vec())
            .collect();
        let mut residue = || {
            let hi = u128::from(xorshift(&mut state));
            let lo = u128::from(xorshift(&mut state));
            ((hi << 64) | lo) % Q
        };
        let row_weights: Vec<u128> = (0..derived.rows()).map(|_| residue()).collect();
        let column_weights: Vec<u128> = (0..derived.columns()).map(|_| residue()).collect();
        let folds = fold_columns(&derived, &rows_h, &row_weights);
        let unresolved =
            LinearClaim::new(&params, row_weights.clone(), column_weights.clone(), 0).unwrap();
        let target = reconstruct(&unresolved, &folds, Q);
        let claim = LinearClaim::new(&params, row_weights, column_weights, target).unwrap();
        (committed, derived, params, rows_f, rows_h, map, claim)
    }

    #[test]
    fn virtual_claim_opens_the_committed_bits() {
        let (committed, _derived, params, rows_f, rows_h, map, claim) = setup();
        let pcs = committed_pcs(&committed).unwrap();
        let (root, hint) = pcs.commit(&committed, rows_f).unwrap();
        let statement = VirtualStatement::new(params, committed, &map, &claim).unwrap();
        let prover = BitZProver::new(params, WINDOW);
        let mut transcript = build_prover("virt-test", "one");
        prover
            .prove_virtual(&statement, &pcs, &hint, &rows_h, &mut transcript, None)
            .unwrap();
        let proof = transcript.finish();
        let verifier = BitZVerifier::new(params, WINDOW);
        verifier
            .verify_virtual(
                &statement,
                &pcs,
                root,
                build_verifier("virt-test", "one", &proof),
                None,
            )
            .unwrap();

        // A claim off by one in its target is rejected.
        let wrong = LinearClaim::new(
            &params,
            claim.row_weights().to_vec(),
            claim.column_weights().to_vec(),
            (claim.target() + 1) % Q,
        )
        .unwrap();
        let wrong_statement = VirtualStatement::new(params, committed, &map, &wrong).unwrap();
        assert!(
            verifier
                .verify_virtual(
                    &wrong_statement,
                    &pcs,
                    root,
                    build_verifier("virt-test", "one", &proof),
                    None,
                )
                .is_err()
        );
    }

    #[test]
    fn inconsistent_derived_rows_cannot_be_proved() {
        let (committed, _derived, params, rows_f, mut rows_h, map, claim) = setup();
        let pcs = committed_pcs(&committed).unwrap();
        let (_root, hint) = pcs.commit(&committed, rows_f).unwrap();
        rows_h[0][0] ^= 1;
        let statement = VirtualStatement::new(params, committed, &map, &claim).unwrap();
        let prover = BitZProver::new(params, WINDOW);
        let mut transcript = build_prover("virt-test", "two");
        assert!(
            prover
                .prove_virtual(&statement, &pcs, &hint, &rows_h, &mut transcript, None)
                .is_err()
        );
    }
}

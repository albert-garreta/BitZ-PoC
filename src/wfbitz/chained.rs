//! A structured virtual opening for the SHA-256 chain + ECDSA map: the
//! same fold and GKR as [`super::virt`], but on grids laid out so that the
//! transposed weights `Mᵀ(u1 ⊗ u2)` are a short sum of tensors
//! ([`SumClaimGf`]) instead of one dense vector over every committed cell.
//!
//! The map (`chained_packed_source` + an identity `chained_packed_source_tail`)
//! has `n` instances of one local relation, a chain link from each instance's
//! cells into the next instance's rows, a distinguished last instance, one
//! shared constant source, and a tail relation whose first `aliases` source
//! columns alias existing cells (the constant and the last instance's digest)
//! while its other source columns are the tail's own committed cells; native
//! derived rows are local-major (`d = i + n·r`), native committed cells
//! instance-major with the non-power-of-two stride `SHA_F`.
//!
//! Both grids have `2^LOG_ROWS` rows and `2^s` columns ("blocks"):
//! * derived: a bit permutation of the native index `d` — the row is bits
//!   `k..k+LOG_ROWS` (the local row `r mod 2^LOG_ROWS`), the block the
//!   instance bits `0..k` plus the bits above (`r div 2^LOG_ROWS`, or the
//!   tail's high bits). The mod-q claim, an eq tensor over the bits of `d`,
//!   is a tensor over this split too, so the fold and GKR run unchanged;
//! * committed: two instances per block (`SHA_F ≤ 2^{LOG_ROWS-1}` cells each
//!   at rows `u·2^{LOG_ROWS-1} + c`), then the tail cells at the mirror of
//!   their derived position (the tail map is the identity, so tail source
//!   `σ` is derived row `σ`), the constant at the mirror of tail row 0.
//!
//! With that, every committed cell's weight is `(row factor)·(block factor)`
//! per map part and sub-block: the local relation, the chain link, the last
//! instance, the tail (its row factor is `u1` itself), the digest aliases
//! (one sparse row vector on the last instance's block) and the constant —
//! 13 tensors over the committed grid, each with a `2^LOG_ROWS`-entry row
//! vector the verifier derives from `u1` and the local maps in `O(nnz)` and
//! a block vector read off `u2`.

#[cfg(feature = "parallel")]
use rayon::prelude::*;

use super::{
    BitZProver, BitZVerifier, ProveError, VerifyError,
    params::{BitZParams, LinearClaim, Root, Shape, SumClaimGf},
    pcs::{OpeningQuery, Pcs, StatementBinding},
    reduce,
    transcript::{ProverState, VerifierState},
};
use crate::ligerito_flock::FlockCommitHint;
use crate::pcs::IntegerMatrixLayout;
use circuit::linear_map::binary::{ChainedPackedSourceParts, ChainedSourceTail, PreparedVirtualMap};
use field::Gf128 as Gf;
use spongefish::Encoding;

/// Rows of both grids: the largest the single-fold bound admits for a
/// 113-bit modulus, `(q − 1)(2^14 + 1) < 2^128`.
pub const LOG_ROWS: usize = 14;

const CHAINED_STATEMENT_LABEL: &[u8] = b"bitz/chained-virtual-statement/v1";

/// A map or claim the structured opening cannot take.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChainedError {
    /// The instance count is not a power of two of at least two.
    Instances,
    /// The local relation has more rows than two sub-blocks hold, or fewer
    /// than one block (then the tail would share blocks with the SHA rows).
    LocalRows,
    /// More nonconstant local cells than half a block.
    LocalCells,
    /// The tail map is not the identity, or its offsets are not the chain's.
    Tail,
    /// The native derived index width is too small for the grids.
    Width,
    /// The claim's weights do not match the derived grid.
    Claim,
    /// The reduced query is not the factored claim the GKR produces.
    UnsupportedQuery,
    /// The prover's or the scheme's parameters are not the statement's.
    ParameterMismatch,
}

/// The block geometry both roles derive from the map.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChainedGeometry {
    /// `k`: `log₂` of the instance count.
    pub log_instances: usize,
    /// `n`.
    pub instances: usize,
    /// Derived rows per instance (`SHA_H`).
    pub local_rows: usize,
    /// Nonconstant committed cells per instance (`SHA_F`); the local maps
    /// have `local_cells + 1` source columns, column 0 the shared constant.
    pub local_cells: usize,
    /// The tail relation's rows (= its source columns: identity).
    pub tail_rows: usize,
    /// Its aliased source prefix (`P_INPUT_ALIAS`).
    pub aliases: usize,
    /// Native derived index of tail row 0: `local_rows · instances`.
    pub h_offset: usize,
    /// Native committed index of tail cell 0: `1 + local_cells · instances`.
    pub f_offset: usize,
    /// Width of the native derived index: every derived cell is below
    /// `2^native_bits`, and both grids have `2^native_bits` cells.
    pub native_bits: usize,
    /// Committed blocks holding the instances, two per block.
    pub sha_blocks: usize,
}

impl ChainedGeometry {
    pub fn new(
        parts: &ChainedPackedSourceParts<'_>,
        tail: &ChainedSourceTail<'_>,
        native_bits: usize,
    ) -> Result<Self, ChainedError> {
        let instances = parts.instances;
        if instances < 2 || !instances.is_power_of_two() {
            return Err(ChainedError::Instances);
        }
        let log_instances = instances.ilog2() as usize;
        let local_rows = parts.local.rows();
        if local_rows > 2 << LOG_ROWS || local_rows < 1 << LOG_ROWS {
            return Err(ChainedError::LocalRows);
        }
        let local_cells = parts.local.cols().saturating_sub(1);
        if local_cells == 0 || local_cells > 1 << (LOG_ROWS - 1) {
            return Err(ChainedError::LocalCells);
        }
        for map in [parts.prev, parts.first, parts.last] {
            if map.rows() != local_rows || map.cols() != local_cells + 1 {
                return Err(ChainedError::LocalRows);
            }
        }
        let h_offset = local_rows * instances;
        let f_offset = 1 + local_cells * instances;
        let aliases = tail.aliases.len();
        if !tail.map.is_identity()
            || tail.map.rows() != tail.map.cols()
            || tail.row_offset != h_offset
            || tail.source_offset != f_offset
            || aliases == 0
            || aliases > tail.map.cols()
        {
            return Err(ChainedError::Tail);
        }
        let tail_rows = tail.map.rows();
        let cells = h_offset + tail_rows;
        if native_bits < log_instances + LOG_ROWS + 1 || cells > 1usize << native_bits {
            return Err(ChainedError::Width);
        }
        Ok(Self {
            log_instances,
            instances,
            local_rows,
            local_cells,
            tail_rows,
            aliases,
            h_offset,
            f_offset,
            native_bits,
            sha_blocks: instances / 2,
        })
    }

    /// Both grids' shape.
    pub fn shape(&self) -> Result<Shape, super::params::ShapeError> {
        Shape::new(LOG_ROWS, self.native_bits - LOG_ROWS)
    }

    /// `(row, block)` of native derived index `d`.
    #[inline]
    pub fn derived_position(&self, d: usize) -> (usize, usize) {
        let k = self.log_instances;
        let row = (d >> k) & ((1 << LOG_ROWS) - 1);
        let block = (d & (self.instances - 1)) + self.instances * (d >> (k + LOG_ROWS));
        (row, block)
    }

    /// Native derived index at `(row, block)`.
    #[inline]
    pub fn derived_index(&self, row: usize, block: usize) -> usize {
        let k = self.log_instances;
        let instance = block & (self.instances - 1);
        let high = block >> k;
        instance + (((high << LOG_ROWS) + row) << k)
    }

    /// `(row, block)` of instance `instance`'s nonconstant cell `cell` (0-based).
    #[inline]
    pub fn committed_sha_position(&self, instance: usize, cell: usize) -> (usize, usize) {
        ((instance & 1) << (LOG_ROWS - 1) | cell, instance >> 1)
    }

    /// `(row, block)` of the committed mirror of derived tail row `sigma`
    /// (tail cell `sigma − aliases`, or the constant for `sigma = 0`).
    #[inline]
    pub fn committed_tail_position(&self, sigma: usize) -> (usize, usize) {
        let (row, block) = self.derived_position(self.h_offset + sigma);
        (row, self.sha_blocks + block - self.instances)
    }

    /// The committed block of instance `n − 1`.
    pub fn last_block(&self) -> usize {
        (self.instances - 1) >> 1
    }

    /// The committed bit rows in the block layout, from the native committed
    /// rows (per native column, `native.rows()` bits each).
    pub fn committed_rows(&self, native: &IntegerMatrixLayout, f_rows: &[Vec<u64>]) -> Vec<Vec<u64>> {
        let shape = Shape::new(LOG_ROWS, self.native_bits - LOG_ROWS).expect("validated geometry");
        let native_cells = native.cells();
        let t_f = native.row_vars;
        let bit = |s: usize| -> bool {
            if s >= native_cells {
                return false;
            }
            let (c, b) = (s >> t_f, s & ((1 << t_f) - 1));
            f_rows[c][b / 64] >> (b % 64) & 1 == 1
        };
        let words = shape.rows() / 64;
        let build = |block: usize| -> Vec<u64> {
            let mut out = vec![0u64; words];
            if block < self.sha_blocks {
                // An instance's cells are one contiguous native range: copy
                // it word by word, native column by native column.
                for u in 0..2 {
                    let instance = 2 * block + u;
                    let mut source = 1 + instance * self.local_cells;
                    let end = source + self.local_cells;
                    let mut row = u << (LOG_ROWS - 1);
                    while source < end && source < native_cells {
                        let (c, b) = (source >> t_f, source & ((1 << t_f) - 1));
                        let len = (end - source).min((1 << t_f) - b);
                        copy_bits(&f_rows[c], b, &mut out, row, len);
                        source += len;
                        row += len;
                    }
                }
            } else {
                let derived_block = block - self.sha_blocks + self.instances;
                for row in 0..shape.rows() {
                    let d = self.derived_index(row, derived_block);
                    if d < self.h_offset {
                        continue;
                    }
                    let sigma = d - self.h_offset;
                    let source = if sigma == 0 {
                        0
                    } else if sigma >= self.aliases && sigma < self.tail_rows {
                        self.f_offset + sigma - self.aliases
                    } else {
                        continue;
                    };
                    if bit(source) {
                        out[row / 64] |= 1u64 << (row % 64);
                    }
                }
            }
            out
        };
        #[cfg(feature = "parallel")]
        {
            (0..shape.columns()).into_par_iter().map(build).collect()
        }
        #[cfg(not(feature = "parallel"))]
        {
            (0..shape.columns()).map(build).collect()
        }
    }

    /// The derived bit rows in the block layout, from the native derived rows.
    pub fn derived_rows(&self, native: &IntegerMatrixLayout, h_rows: &[Vec<u64>]) -> Vec<Vec<u64>> {
        let shape = Shape::new(LOG_ROWS, self.native_bits - LOG_ROWS).expect("validated geometry");
        let native_cells = native.cells();
        let t_h = native.row_vars;
        let words = shape.rows() / 64;
        let build = |block: usize| -> Vec<u64> {
            let mut out = vec![0u64; words];
            for row in 0..shape.rows() {
                let d = self.derived_index(row, block);
                if d >= native_cells {
                    continue;
                }
                let (c, b) = (d >> t_h, d & ((1 << t_h) - 1));
                if h_rows[c][b / 64] >> (b % 64) & 1 == 1 {
                    out[row / 64] |= 1u64 << (row % 64);
                }
            }
            out
        };
        #[cfg(feature = "parallel")]
        {
            (0..shape.columns()).into_par_iter().map(build).collect()
        }
        #[cfg(not(feature = "parallel"))]
        {
            (0..shape.columns()).map(build).collect()
        }
    }
}

/// ORs `len` bits of `src` from bit `src_bit` into `dst` at bit `dst_bit`,
/// 64 at a time (`dst`'s target bits are zero beforehand).
fn copy_bits(src: &[u64], src_bit: usize, dst: &mut [u64], dst_bit: usize, len: usize) {
    let mut done = 0;
    while done < len {
        let take = (len - done).min(64);
        let s = src_bit + done;
        let (sw, sb) = (s / 64, s % 64);
        let mut chunk = src[sw] >> sb;
        if sb != 0 && sw + 1 < src.len() {
            chunk |= src[sw + 1] << (64 - sb);
        }
        if take < 64 {
            chunk &= (1u64 << take) - 1;
        }
        let d = dst_bit + done;
        let (dw, db) = (d / 64, d % 64);
        dst[dw] |= chunk << db;
        if db != 0 && db + take > 64 {
            dst[dw + 1] |= chunk >> (64 - db);
        }
        done += take;
    }
}

impl Encoding<[u8]> for ChainedGeometry {
    fn encode(&self) -> impl AsRef<[u8]> {
        let mut bytes = Vec::with_capacity(11 * 8);
        for value in [
            self.log_instances,
            self.instances,
            self.local_rows,
            self.local_cells,
            self.tail_rows,
            self.aliases,
            self.h_offset,
            self.f_offset,
            self.native_bits,
            self.sha_blocks,
            LOG_ROWS,
        ] {
            bytes.extend_from_slice(&(value as u64).to_le_bytes());
        }
        bytes
    }
}

/// A claim on the derived grid of a chained map, opened against the
/// block-layout commitment of its sources.
pub struct ChainedStatement<'a> {
    claim_params: BitZParams,
    geometry: ChainedGeometry,
    parts: ChainedPackedSourceParts<'a>,
    tail: ChainedSourceTail<'a>,
    map_digest: [u8; 32],
    claim: &'a LinearClaim,
}

impl<'a> ChainedStatement<'a> {
    pub fn new(
        claim_params: BitZParams,
        geometry: ChainedGeometry,
        parts: ChainedPackedSourceParts<'a>,
        tail: ChainedSourceTail<'a>,
        map_digest: [u8; 32],
        claim: &'a LinearClaim,
    ) -> Result<Self, ChainedError> {
        let shape = geometry.shape().map_err(|_| ChainedError::Width)?;
        if *claim_params.shape() != shape {
            return Err(ChainedError::ParameterMismatch);
        }
        if claim.row_weights().len() != shape.rows() || claim.column_weights().len() != shape.columns() {
            return Err(ChainedError::Claim);
        }
        Ok(Self {
            claim_params,
            geometry,
            parts,
            tail,
            map_digest,
            claim,
        })
    }

    pub fn geometry(&self) -> &ChainedGeometry {
        &self.geometry
    }

    pub fn claim_params(&self) -> &BitZParams {
        &self.claim_params
    }

    pub fn shape(&self) -> Shape {
        *self.claim_params.shape()
    }

    /// The bytes both roles bind after the root.
    fn frame(&self) -> Vec<u8> {
        let mut frame = Vec::new();
        frame.extend_from_slice(Encoding::<[u8]>::encode(&self.claim_params).as_ref());
        frame.extend_from_slice(Encoding::<[u8]>::encode(&self.geometry).as_ref());
        frame.extend_from_slice(&self.map_digest);
        frame
    }

    /// Per local source column `c` and sub-block `v`: `Σ_{r ∈ rows(c), r div
    /// 2^LOG_ROWS = v} u1[r mod 2^LOG_ROWS]`.
    fn local_sums(map: &PreparedVirtualMap, u1: &[Gf]) -> [Vec<Gf>; 2] {
        let mask = (1usize << LOG_ROWS) - 1;
        let mut sums = [vec![Gf::zero(); map.cols()], vec![Gf::zero(); map.cols()]];
        for c in 0..map.cols() {
            if let Some(rows) = map.matrix().column_rows(c) {
                for &r in rows {
                    sums[r >> LOG_ROWS][c] += u1[r & mask];
                }
            }
        }
        sums
    }

    /// The row vector of one (slot `u`, sub-block `v`) term: the local sum
    /// of the cell each row holds, zero outside slot `u` and past the cells.
    fn row_vector(&self, sums: &[Gf], u: usize) -> Vec<Gf> {
        let mut rows = vec![Gf::zero(); 1 << LOG_ROWS];
        let base = u << (LOG_ROWS - 1);
        for cell in 0..self.geometry.local_cells {
            rows[base + cell] = sums[cell + 1];
        }
        rows
    }

    /// Rewrites the GKR's factored claim on the derived grid into the sum
    /// of tensors on the committed grid.
    pub fn transpose_query(&self, query: OpeningQuery) -> Result<OpeningQuery, ChainedError> {
        let OpeningQuery::InnerProduct { claim } = query else {
            return Err(ChainedError::UnsupportedQuery);
        };
        let shape = self.shape();
        let (u1, u2) = (claim.row_weights(), claim.column_weights());
        if u1.len() != shape.rows() || u2.len() != shape.columns() {
            return Err(ChainedError::Claim);
        }
        let g = &self.geometry;
        let (n, blocks) = (g.instances, shape.columns());
        let block_weight = |b: usize| if b < blocks { u2[b] } else { Gf::zero() };
        let mut terms: Vec<(Vec<Gf>, Vec<Gf>)> = Vec::with_capacity(13);

        // The local relation and the chain link: instance `2β + u`'s cells
        // feed its own rows (blocks `2β + u + n·v`) and the next instance's.
        let local = Self::local_sums(self.parts.local, u1);
        let prev = Self::local_sums(self.parts.prev, u1);
        for u in 0..2 {
            for v in 0..2 {
                let cols: Vec<Gf> = (0..blocks)
                    .map(|beta| {
                        let instance = 2 * beta + u;
                        if instance < n {
                            block_weight(instance + n * v)
                        } else {
                            Gf::zero()
                        }
                    })
                    .collect();
                terms.push((self.row_vector(&local[v], u), cols));
                let cols: Vec<Gf> = (0..blocks)
                    .map(|beta| {
                        let instance = 2 * beta + u;
                        if instance + 1 < n {
                            block_weight(instance + 1 + n * v)
                        } else {
                            Gf::zero()
                        }
                    })
                    .collect();
                terms.push((self.row_vector(&prev[v], u), cols));
            }
        }
        // The last instance's rows from its own cells (slot 1 of the last block).
        let last = Self::local_sums(self.parts.last, u1);
        for v in 0..2 {
            let mut cols = vec![Gf::zero(); blocks];
            cols[g.last_block()] = block_weight(n - 1 + n * v);
            terms.push((self.row_vector(&last[v], 1), cols));
        }
        // The tail: its cells mirror their derived rows.
        let cols: Vec<Gf> = (0..blocks)
            .map(|beta| {
                if beta >= g.sha_blocks {
                    block_weight(beta - g.sha_blocks + n)
                } else {
                    Gf::zero()
                }
            })
            .collect();
        terms.push((u1.to_vec(), cols));
        // The digest aliases: the last instance's cells the tail reads.
        let mut alias_rows = vec![Gf::zero(); shape.rows()];
        let mut alias_cols = vec![Gf::zero(); blocks];
        for sigma in 1..g.aliases {
            let source = self.tail.aliases[sigma];
            if source == 0 || source >= g.f_offset {
                return Err(ChainedError::Tail);
            }
            let (instance, cell) = ((source - 1) / g.local_cells, (source - 1) % g.local_cells);
            let (row, block) = g.committed_sha_position(instance, cell);
            if block != g.last_block() {
                return Err(ChainedError::Tail);
            }
            if let Some(rows) = self.tail.map.matrix().column_rows(sigma) {
                for &j in rows {
                    let (dr, db) = g.derived_position(g.h_offset + j);
                    alias_rows[row] += u1[dr] * block_weight(db);
                }
            }
        }
        alias_cols[g.last_block()] = Gf::one();
        terms.push((alias_rows, alias_cols));
        // The constant: every instance's rows from source column 0 (the
        // tail's own row 0 already reaches its mirror through the tail term).
        let first = Self::local_sums(self.parts.first, u1);
        let mut kappa = Gf::zero();
        for v in 0..2 {
            let mut instance_sum = Gf::zero();
            for instance in 0..n {
                instance_sum += block_weight(instance + n * v);
            }
            kappa += local[v][0] * instance_sum;
            kappa += first[v][0] * block_weight(n * v);
            kappa += last[v][0] * block_weight(n - 1 + n * v);
        }
        let (row0, block0) = g.committed_tail_position(0);
        let mut const_rows = vec![Gf::zero(); shape.rows()];
        const_rows[row0] = kappa;
        let mut const_cols = vec![Gf::zero(); blocks];
        const_cols[block0] = Gf::one();
        terms.push((const_rows, const_cols));

        let mut hasher = blake3::Hasher::new();
        hasher.update(&self.frame());
        hasher.update(Encoding::<[u8]>::encode(&claim).as_ref());
        let binding = hasher.finalize().as_bytes().to_vec();
        let claim = SumClaimGf::from_shape(&shape, terms, claim.target(), binding)
            .map_err(|_| ChainedError::Claim)?;
        Ok(OpeningQuery::InnerProductSum { claim })
    }
}

impl BitZProver {
    /// Proves the claim on the derived grid `derived_rows` (block layout)
    /// against `hint`, the block-layout commitment to the sources.
    pub fn prove_chained(
        &self,
        statement: &ChainedStatement<'_>,
        pcs: &Pcs,
        hint: &FlockCommitHint,
        derived_rows: &[Vec<u64>],
        transcript: &mut ProverState,
        ood: Option<(&[Gf], Gf)>,
    ) -> Result<(), ProveError> {
        let shape = statement.shape();
        if *self.params() != statement.claim_params || pcs.bit_len() != 1usize << shape.log_bits() {
            return Err(ProveError::Chained(ChainedError::ParameterMismatch));
        }
        if derived_rows.len() != shape.columns()
            || derived_rows.iter().any(|row| row.len() * 64 != shape.rows())
        {
            return Err(ProveError::Witness);
        }
        let root = Root(*hint.root());
        transcript.public_message(CHAINED_STATEMENT_LABEL);
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
        let query = statement.transpose_query(query).map_err(ProveError::Chained)?;
        super::trace("transpose (structured)", started);
        let started = std::time::Instant::now();
        let result = pcs
            .prove_lin(hint, &query, StatementBinding::Bind, transcript, ood)
            .map_err(ProveError::Opening);
        super::trace("opening (all)", started);
        result
    }
}

impl BitZVerifier {
    /// Checks a [`BitZProver::prove_chained`] proof, consuming the transcript.
    pub fn verify_chained(
        &self,
        statement: &ChainedStatement<'_>,
        pcs: &Pcs,
        com: Root,
        mut transcript: VerifierState<'_>,
        ood: Option<(&[Gf], Gf)>,
    ) -> Result<(), VerifyError> {
        let shape = statement.shape();
        if *self.params() != statement.claim_params || pcs.bit_len() != 1usize << shape.log_bits() {
            return Err(VerifyError::Chained(ChainedError::ParameterMismatch));
        }
        transcript.public_message(CHAINED_STATEMENT_LABEL);
        transcript.public_message(&com.0);
        transcript.public_message(statement.frame().as_slice());
        transcript.public_message(statement.claim);

        let started = std::time::Instant::now();
        let fold = self
            .receive_fold(statement.claim, &mut transcript)
            .map_err(VerifyError::Fold)?;
        super::trace("v: fold", started);
        let started = std::time::Instant::now();
        let query = reduce::gkr_reduce_verify(&mut transcript, &fold, &shape)
            .map_err(VerifyError::Reduction)?;
        super::trace("v: gkr", started);
        let started = std::time::Instant::now();
        let query = statement.transpose_query(query).map_err(VerifyError::Chained)?;
        super::trace("v: transpose (structured)", started);
        let started = std::time::Instant::now();
        pcs.verify_lin(&com, &query, StatementBinding::Bind, &mut transcript, ood)
            .map_err(VerifyError::Opening)?;
        super::trace("v: opening", started);
        transcript
            .check_eof()
            .map_err(|_| VerifyError::TrailingData)
    }
}

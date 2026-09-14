use std::{
    array,
    collections::HashMap,
    sync::{Arc, OnceLock},
};

use circuit::{
    constraints::ConstraintGenerator,
    matrix_wengert::{WengertGenerator, WengertTape},
    p256, sha256,
};
use num_bigint::{BigInt, BigUint, Sign};
use num_traits::Zero;

use super::{Result, error};
use crate::{
    f2map::{ChainedPackedSourceParts, ChainedSourceTail, PreparedVirtualMap, VirtualMap},
    pcs::IntegerMatrixLayout,
    sparse_matrix::SparseMatrix,
};

pub(crate) const SHA_H: usize = 20_457;
pub(crate) const SHA_F: usize = 512 + sha256::COMPRESSION_HINT_BITS;
pub(crate) const SHA_ROWS: usize = 184;
pub(crate) const P_INPUT_ALIAS: usize = 257;

/// Which original circuit rows participate in the outer sumcheck.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OuterMode {
    Split,
    AllRows,
}

/// Public values use fixed-width big-endian encodings. The message is a witness.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Sha256EcdsaStatement {
    pub log_compressions: u8,
    pub qx: [u8; 32],
    pub qy: [u8; 32],
    pub r: [u8; 32],
    pub s: [u8; 32],
}

impl Sha256EcdsaStatement {
    pub fn message_bytes(&self) -> Result<usize> {
        if !(3..=16).contains(&self.log_compressions) {
            return Err(error("compression exponent must be in 3..=16"));
        }
        Ok(64 * ((1usize << self.log_compressions) - 1))
    }
    pub(crate) fn bytes(&self) -> Vec<u8> {
        let mut bytes = vec![self.log_compressions];
        for word in [&self.qx, &self.qy, &self.r, &self.s] {
            bytes.extend_from_slice(word);
        }
        bytes
    }
    pub(crate) fn bit(&self, bit: usize) -> bool {
        let words = [&self.qx, &self.qy, &self.r, &self.s];
        words[bit / 256][31 - (bit % 256) / 8] >> (bit % 8) & 1 != 0
    }
}

/// Sparse integer rows whose coefficients index one shared table of distinct
/// values ([`LocalRelation::coefficients`]). The P-256 matrices hold 4.2M
/// entries but only ~3.4k distinct coefficients, so per-coefficient work — the
/// per-proof reduction modulo the sampled prime — is done on the table, never
/// on the entries, and an entry costs eight bytes instead of a heap integer.
pub(crate) struct CompactRows {
    row_ptr: Vec<u32>,
    cols: Vec<u32>,
    coefs: Vec<u32>,
}

impl CompactRows {
    pub(crate) fn rows(&self) -> usize {
        self.row_ptr.len() - 1
    }
    pub(crate) fn is_empty_row(&self, r: usize) -> bool {
        self.row_ptr[r] == self.row_ptr[r + 1]
    }
    /// `(column, coefficient index)` entries of row `r` in increasing column order.
    pub(crate) fn row(&self, r: usize) -> impl Iterator<Item = (usize, usize)> + '_ {
        let (start, end) = (self.row_ptr[r] as usize, self.row_ptr[r + 1] as usize);
        self.cols[start..end]
            .iter()
            .zip(&self.coefs[start..end])
            .map(|(&c, &k)| (c as usize, k as usize))
    }
}

/// Interns matrix coefficients into the shared table, first seen first.
#[derive(Default)]
struct Interner {
    table: Vec<BigInt>,
    index: HashMap<BigInt, u32>,
}

impl Interner {
    fn intern(&mut self, value: &BigInt) -> u32 {
        if let Some(&k) = self.index.get(value) {
            return k;
        }
        let k = u32::try_from(self.table.len()).expect("coefficient table fits u32");
        self.table.push(value.clone());
        self.index.insert(value.clone(), k);
        k
    }
    fn rows(&mut self, matrix: &circuit::constraints::SparseMatrix<BigInt>) -> CompactRows {
        let mut out = CompactRows {
            row_ptr: Vec::with_capacity(matrix.row_count() + 1),
            cols: Vec::new(),
            coefs: Vec::new(),
        };
        out.row_ptr.push(0);
        for row in matrix.rows() {
            for (column, coefficient) in row.entries() {
                out.cols
                    .push(u32::try_from(*column).expect("matrix column fits u32"));
                out.coefs.push(self.intern(coefficient));
            }
            out.row_ptr
                .push(u32::try_from(out.cols.len()).expect("matrix entries fit u32"));
        }
        out
    }
}

/// The three P-256 matrices transposed over the assignment tail, so the
/// combined coefficient of every tail cell is one independent gather: entry
/// `(slot, coefficient index)` with `slot = 3·row + m`, `m` = 0/1/2 for A/B/C.
pub(crate) struct TailColumns {
    col_ptr: Vec<u32>,
    slots: Vec<u32>,
    coefs: Vec<u32>,
}

impl TailColumns {
    fn new(columns: usize, matrices: [&CompactRows; 3]) -> Result<Self> {
        let mut col_ptr = vec![0u32; columns + 1];
        for matrix in matrices {
            for &c in &matrix.cols {
                if c as usize >= columns {
                    return Err(error("P-256 matrix column outside the assignment tail"));
                }
                col_ptr[c as usize + 1] += 1;
            }
        }
        for j in 0..columns {
            col_ptr[j + 1] += col_ptr[j];
        }
        let nnz = col_ptr[columns] as usize;
        let (mut slots, mut coefs) = (vec![0u32; nnz], vec![0u32; nnz]);
        let mut next = col_ptr.clone();
        for (m, matrix) in matrices.iter().enumerate() {
            for r in 0..matrix.rows() {
                let slot = u32::try_from(3 * r + m).expect("row slot fits u32");
                for (c, k) in matrix.row(r) {
                    let at = next[c] as usize;
                    next[c] += 1;
                    slots[at] = slot;
                    coefs[at] = k as u32;
                }
            }
        }
        Ok(Self {
            col_ptr,
            slots,
            coefs,
        })
    }
    pub(crate) fn columns(&self) -> usize {
        self.col_ptr.len() - 1
    }
    /// `(slot, coefficient index)` entries of tail cell `j`.
    pub(crate) fn column(&self, j: usize) -> impl Iterator<Item = (usize, usize)> + '_ {
        let (start, end) = (self.col_ptr[j] as usize, self.col_ptr[j + 1] as usize);
        self.slots[start..end]
            .iter()
            .zip(&self.coefs[start..end])
            .map(|(&s, &k)| (s as usize, k as usize))
    }
}

pub(crate) struct LocalRelation {
    pub sha_local: PreparedVirtualMap,
    pub sha_prev: PreparedVirtualMap,
    pub sha_first: PreparedVirtualMap,
    pub sha_c: CompactRows,
    pub sha_output: [usize; 256],
    pub p_map: PreparedVirtualMap,
    pub a: CompactRows,
    pub b: CompactRows,
    pub c: CompactRows,
    /// The distinct integer coefficients of `sha_c`, `a`, `b` and `c` (the
    /// test oracles' form; the protocol reads `coefficient_words`).
    #[cfg(test)]
    pub coefficients: Vec<BigInt>,
    /// The same coefficients as normalized two's-complement words (the form
    /// `RawMontyCtx::signed_words_residue` reduces natively per proof).
    pub coefficient_words: Vec<Box<[u64]>>,
    /// `a`, `b`, `c` column by column over the P-256 assignment tail.
    pub tail: TailColumns,
    /// The P-256 circuit's Z-side linear arithmetic as a reverse-mode tape:
    /// `r · (A + xB + x²C)` over every tail column by one pass over the
    /// circuit's DAG (97,986 edges) instead of the 4.2M expanded entries.
    pub tape: WengertTape,
    pub nonlinear: Vec<usize>,
    pub linear: Vec<usize>,
    pub public_h: [usize; 1024],
    pub digest: [u8; 32],
    pub defect_bits: u32,
}

impl LocalRelation {
    /// Number of P-256 rows (shared by `a`, `b` and `c`).
    pub(crate) fn rows(&self) -> usize {
        self.a.rows()
    }
}

/// The little-endian two's-complement words of `value`: zero is empty, and
/// the top bit of the last word is the sign (the final partial word is
/// padded with the sign byte), which is what
/// `RawMontyCtx::signed_words_residue` expects.
pub(crate) fn signed_words(value: &BigInt) -> Box<[u64]> {
    if value.is_zero() {
        return Box::default();
    }
    let fill = if value.sign() == Sign::Minus { 0xFF } else { 0 };
    value
        .to_signed_bytes_le()
        .chunks(8)
        .map(|chunk| {
            let mut word = [fill; 8];
            word[..chunk.len()].copy_from_slice(chunk);
            u64::from_le_bytes(word)
        })
        .collect()
}

fn bool_map(columns: usize, rows: Vec<Vec<usize>>) -> Result<PreparedVirtualMap> {
    let rows = rows
        .into_iter()
        .map(|row| row.into_iter().map(|c| (c, true)).collect())
        .collect();
    PreparedVirtualMap::new(SparseMatrix::try_from_rows(columns, rows).map_err(error)?)
        .map_err(error)
}

fn build_local() -> Result<LocalRelation> {
    let mut generator = ConstraintGenerator::new(sha256::COMPRESSION_INPUT_BITS);
    let inputs = generator.inputs();
    let outputs = sha256::compression_circuit(&mut generator, &inputs);
    let sha_output = array::from_fn(|i| {
        assert!(!outputs[i].constant() && outputs[i].witnesses().len() == 1);
        // The chained source omits the independent compression's state inputs.
        outputs[i].witnesses().first().copied().unwrap() + 1 - 256
    });
    let sha = generator.into_matrices();
    if sha.m.row_count() != SHA_H
        || sha.c.row_count() != SHA_ROWS
        || sha
            .a
            .rows()
            .iter()
            .chain(sha.b.rows())
            .any(|r| !r.entries().is_empty())
    {
        return Err(error("unexpected SHA compression shape"));
    }
    let mut local = vec![Vec::new(); SHA_H];
    let mut prev = vec![Vec::new(); SHA_H];
    let mut first = vec![Vec::new(); SHA_H];
    for (row, terms) in sha.m.rows().iter().enumerate() {
        let mut parity = false;
        for &column in terms.positions() {
            match column {
                0..=512 => local[row].push(column),
                513..=768 => {
                    let bit = column - 513;
                    prev[row].push(sha_output[bit]);
                    parity ^= sha256::INITIAL_STATE[bit / 32] >> (bit % 32) & 1 != 0;
                }
                _ => local[row].push(column - 256),
            }
        }
        if parity {
            first[row].push(0);
        }
    }
    let sha_local = bool_map(SHA_F + 1, local)?;
    let sha_prev = bool_map(SHA_F + 1, prev)?;
    let sha_first = bool_map(SHA_F + 1, first)?;
    let mut interner = Interner::default();
    let sha_c = interner.rows(&sha.c);
    drop(sha);

    let mut generator = ConstraintGenerator::new(p256::VERIFY_DIGEST_INPUT_BITS);
    let inputs = generator.inputs();
    p256::verify_digest_circuit(&mut generator, &inputs);
    let p = generator.into_matrices();
    let mut public_h = [usize::MAX; 1024];
    for (h, row) in p.m.rows().iter().enumerate() {
        if let [source] = row.positions() {
            if (257..1281).contains(source) && public_h[*source - 257] == usize::MAX {
                public_h[*source - 257] = h;
            }
        }
    }
    if public_h.contains(&usize::MAX) {
        return Err(error("public P-256 inputs have no direct lifts"));
    }
    let p_map = bool_map(
        p.m.column_count(),
        p.m.rows().iter().map(|r| r.positions().to_vec()).collect(),
    )?;
    let a = interner.rows(&p.a);
    let b = interner.rows(&p.b);
    let c = interner.rows(&p.c);
    let tail = TailColumns::new(p.m.row_count(), [&a, &b, &c])?;
    drop(p);
    let mut tape_generator = WengertGenerator::new(p256::VERIFY_DIGEST_INPUT_BITS);
    let tape_inputs = tape_generator.take_boxed_inputs::<{ p256::VERIFY_DIGEST_INPUT_BITS }>();
    p256::verify_digest_circuit(&mut tape_generator, &tape_inputs);
    let tape = tape_generator.finish();
    if tape.row_count() != a.rows() || tape.column_count() != tail.columns() {
        return Err(error("P-256 tape shape disagrees with the constraint matrices"));
    }
    let coefficients = interner.table;
    let coefficient_words = coefficients.iter().map(signed_words).collect();
    let (linear, nonlinear): (Vec<_>, Vec<_>) =
        (0..a.rows()).partition(|&i| a.is_empty_row(i) || b.is_empty_row(i));
    let magnitudes: Vec<BigUint> = coefficients.iter().map(|c| c.magnitude().clone()).collect();
    let norm = |rows: &CompactRows, r: usize| -> BigUint {
        rows.row(r)
            .fold(BigUint::zero(), |sum, (_, k)| sum + &magnitudes[k])
    };
    let mut bound = BigUint::from(1u8);
    for r in 0..sha_c.rows() {
        bound = bound.max(norm(&sha_c, r));
    }
    for i in 0..a.rows() {
        bound = bound.max(norm(&a, i) * norm(&b, i) + norm(&c, i));
    }
    let defect_bits = u32::try_from(bound.bits()).map_err(error)?;
    // The digest streams the same bytes as the original entry-wise encoding.
    let signed_bytes: Vec<Vec<u8>> = coefficients.iter().map(BigInt::to_signed_bytes_le).collect();
    let mut hash = blake3::Hasher::new();
    hash.update(b"f2z/sha256-ecdsa/local-relation/v1");
    for map in [&sha_local, &sha_prev, &sha_first, &p_map] {
        hash.update(&map.digest());
    }
    let mut buffer = Vec::new();
    for matrix in [&sha_c, &a, &b, &c] {
        hash.update(&(matrix.rows() as u64).to_le_bytes());
        for r in 0..matrix.rows() {
            buffer.clear();
            buffer.extend_from_slice(
                &((matrix.row_ptr[r + 1] - matrix.row_ptr[r]) as u64).to_le_bytes(),
            );
            for (column, k) in matrix.row(r) {
                buffer.extend_from_slice(&(column as u64).to_le_bytes());
                buffer.extend_from_slice(&(signed_bytes[k].len() as u64).to_le_bytes());
                buffer.extend_from_slice(&signed_bytes[k]);
            }
            hash.update(&buffer);
        }
    }
    Ok(LocalRelation {
        sha_local,
        sha_prev,
        sha_first,
        sha_c,
        sha_output,
        p_map,
        a,
        b,
        c,
        #[cfg(test)]
        coefficients,
        coefficient_words,
        tail,
        tape,
        nonlinear,
        linear,
        public_h,
        digest: *hash.finalize().as_bytes(),
        defect_bits,
    })
}

/// One implicit chained SHA map followed by a compact P-256 map.
pub struct Sha256EcdsaMap {
    pub(crate) local: Arc<LocalRelation>,
    pub(crate) last: PreparedVirtualMap,
    pub(crate) n: usize,
    pub(crate) h_offset: usize,
    pub(crate) f_offset: usize,
    aliases: [usize; P_INPUT_ALIAS],
    rows: usize,
    cols: usize,
    nnz: usize,
    digest: [u8; 32],
}

impl Sha256EcdsaMap {
    pub(crate) fn p_source(&self, column: usize) -> usize {
        match column {
            0 => 0,
            1..=256 => {
                let digest_bit = column - 1;
                let bit = (7 - digest_bit / 32) * 32 + digest_bit % 32;
                (self.n - 1) * SHA_F + self.local.sha_output[bit]
            }
            _ => self.f_offset + column - P_INPUT_ALIAS,
        }
    }
}

impl VirtualMap for Sha256EcdsaMap {
    type ColumnRows<'a> = std::vec::IntoIter<usize>;
    fn rows(&self) -> usize {
        self.rows
    }
    fn cols(&self) -> usize {
        self.cols
    }
    fn nnz(&self) -> usize {
        self.nnz
    }
    fn digest(&self) -> [u8; 32] {
        self.digest
    }
    fn is_identity(&self) -> bool {
        false
    }
    fn chained_packed_source(&self) -> Option<ChainedPackedSourceParts<'_>> {
        Some(ChainedPackedSourceParts {
            local: &self.local.sha_local,
            prev: &self.local.sha_prev,
            first: &self.local.sha_first,
            last: &self.last,
            instances: self.n,
        })
    }
    fn chained_packed_source_tail(&self) -> Option<ChainedSourceTail<'_>> {
        Some(ChainedSourceTail {
            map: &self.local.p_map,
            row_offset: self.h_offset,
            source_offset: self.f_offset,
            aliases: &self.aliases,
        })
    }
    fn column_rows(&self, column: usize) -> Option<Self::ColumnRows<'_>> {
        if column >= self.cols {
            return None;
        }
        let mut rows = Vec::new();
        let mut add = |map: &PreparedVirtualMap, c: usize, instance: usize| {
            if let Some(col) = map.matrix().column(c) {
                rows.extend(col.row_indices().iter().map(|r| instance + self.n * r));
            }
        };
        if column == 0 {
            for i in 0..self.n {
                add(&self.local.sha_local, 0, i);
            }
            add(&self.local.sha_first, 0, 0);
            add(&self.last, 0, self.n - 1);
        } else if column < self.f_offset {
            let instance = (column - 1) / SHA_F;
            let c = (column - 1) % SHA_F + 1;
            add(&self.local.sha_local, c, instance);
            if instance + 1 < self.n {
                add(&self.local.sha_prev, c, instance + 1);
            }
            if instance + 1 == self.n {
                add(&self.last, c, instance);
            }
        }
        let mut add_p = |c: usize| {
            if let Some(col) = self.local.p_map.matrix().column(c) {
                rows.extend(col.row_indices().iter().map(|r| self.h_offset + r));
            }
        };
        if column == 0 {
            add_p(0);
        } else if column >= self.f_offset {
            add_p(P_INPUT_ALIAS + column - self.f_offset);
        } else if column > (self.n - 1) * SHA_F {
            for c in 1..=256 {
                if self.p_source(c) == column {
                    add_p(c);
                }
            }
        }
        rows.sort_unstable();
        let mut parity = Vec::with_capacity(rows.len());
        for row in rows {
            if parity.last() == Some(&row) {
                parity.pop();
            } else {
                parity.push(row);
            }
        }
        Some(parity.into_iter())
    }
}

/// Prepared public relation. No message or signature is retained here.
pub struct PreparedSha256Ecdsa {
    pub(crate) local: Arc<LocalRelation>,
    pub(crate) map: Sha256EcdsaMap,
    pub(crate) log_n: usize,
    pub(crate) mode: OuterMode,
    pub(crate) lambda: u32,
    pub(crate) h_layout: IntegerMatrixLayout,
    pub(crate) f_layout: IntegerMatrixLayout,
    pub(crate) ligerito: crate::ligerito_flock::ResolvedLigerito,
}

impl PreparedSha256Ecdsa {
    pub fn ligerito_configuration(&self) -> &crate::ligerito_flock::ResolvedLigerito { &self.ligerito }

    pub fn with_ligerito(
        mut self,
        selection: crate::ligerito_flock::LigeritoSelection,
    ) -> Result<Self> {
        self.ligerito = selection
            .resolve(
                self.f_layout.row_vars + self.f_layout.col_vars - 7,
                self.lambda as usize,
            )
            .map_err(error)?;
        self.security()?;
        Ok(self)
    }

    pub fn compressions(&self) -> usize {
        self.map.n
    }
    pub fn message_bytes(&self) -> usize {
        64 * (self.compressions() - 1)
    }
    pub fn nonlinear_rows(&self) -> usize {
        self.local.nonlinear.len()
    }
    /// Original rows entering the outer sumcheck in the selected mode.
    pub fn outer_rows(&self) -> usize {
        match self.mode {
            OuterMode::Split => self.local.nonlinear.len(),
            OuterMode::AllRows => SHA_ROWS * self.compressions() + self.local.rows(),
        }
    }
    /// Allocated outer table slots, including layout and power-of-two padding.
    pub fn outer_domain_size(&self) -> usize {
        1 << self.outer_sumcheck_num_vars()
    }
    pub(super) fn outer_sumcheck_num_vars(&self) -> usize {
        let rows = match self.mode {
            OuterMode::Split => self.local.nonlinear.len(),
            OuterMode::AllRows => 256 * self.compressions() + self.local.rows(),
        };
        rows.next_power_of_two().ilog2() as usize
    }
    pub fn linear_rows(&self) -> usize {
        self.compressions() * SHA_ROWS + self.local.linear.len() + 1025
    }
    pub fn live_assignment_bits(&self) -> usize {
        self.map.h_offset + self.local.p_map.rows()
    }
    pub fn live_source_bits(&self) -> usize {
        self.map.f_offset + self.local.p_map.cols() - P_INPUT_ALIAS
    }
    pub fn assignment_params(&self) -> &IntegerMatrixLayout {
        &self.h_layout
    }
    pub fn source_params(&self) -> &IntegerMatrixLayout {
        &self.f_layout
    }
    pub fn map(&self) -> &impl VirtualMap {
        &self.map
    }
    pub fn security_target(&self) -> u32 {
        self.lambda
    }
    pub fn outer_mode(&self) -> OuterMode {
        self.mode
    }
    pub(crate) fn padding(&self) -> [u32; 16] {
        padding(self.compressions())
    }
    pub(crate) fn linear_vars(&self) -> usize {
        (256 * self.compressions() + self.local.linear.len() + 1025)
            .next_power_of_two()
            .ilog2() as usize
    }
}

pub(crate) fn padding(n: usize) -> [u32; 16] {
    let mut words = [0u32; 16];
    words[0] = 0x8000_0000;
    let bits = ((n - 1) * 512) as u64;
    words[14] = (bits >> 32) as u32;
    words[15] = bits as u32;
    words
}

pub fn prepare_sha256_ecdsa(
    log_compressions: usize,
    lambda: u32,
    mode: OuterMode,
) -> Result<PreparedSha256Ecdsa> {
    if !(3..=16).contains(&log_compressions) || ![100, 128].contains(&lambda) {
        return Err(error("expected exponent 3..=16 and security 100 or 128"));
    }
    static LOCAL: OnceLock<std::result::Result<Arc<LocalRelation>, String>> = OnceLock::new();
    let local = LOCAL
        .get_or_init(|| build_local().map(Arc::new).map_err(|e| e.to_string()))
        .as_ref()
        .map_err(error)?
        .clone();
    let n = 1usize << log_compressions;
    let h_offset = SHA_H * n;
    let f_offset = 1 + SHA_F * n;
    let h_bits = (h_offset + local.p_map.rows()).next_power_of_two().ilog2() as usize;
    let f_bits = (f_offset + local.p_map.cols() - P_INPUT_ALIAS)
        .next_power_of_two()
        .ilog2() as usize;
    let params = |bits: usize| {
        let t = bits.div_ceil(2).min(13);
        IntegerMatrixLayout {
            row_vars: t,
            col_vars: bits - t,
            word_bits: 1,
        }
    };
    // At the final block cancel its source block inputs and replace them by
    // the mandated padding bits, using the shared source-one coordinate.
    let mut last = vec![Vec::new(); SHA_H];
    let pad = padding(n);
    let mut canceled = 0;
    for bit in 0..512 {
        for &row in local
            .sha_local
            .matrix()
            .column(bit + 1)
            .unwrap()
            .row_indices()
        {
            last[row].push(bit + 1);
            canceled += 1;
            if pad[bit / 32] >> (bit % 32) & 1 != 0 {
                last[row].push(0);
            }
        }
    }
    for row in &mut last {
        row.sort_unstable();
        let mut out = Vec::new();
        for &c in row.iter() {
            if out.last() == Some(&c) {
                out.pop();
            } else {
                out.push(c);
            }
        }
        *row = out;
    }
    let last = bool_map(SHA_F + 1, last)?;
    let nnz = n * local.sha_local.nnz()
        + (n - 1) * local.sha_prev.nnz()
        + local.sha_first.nnz()
        + last.nnz()
        + local.p_map.nnz()
        - 2 * canceled;
    let mut hash = blake3::Hasher::new();
    hash.update(b"f2z/sha256-ecdsa/map/v1");
    hash.update(&local.digest);
    hash.update(&last.digest());
    hash.update(&(n as u64).to_le_bytes());
    let h_layout = params(h_bits);
    let f_layout = params(f_bits);
    let mut map = Sha256EcdsaMap {
        local: local.clone(),
        last,
        n,
        h_offset,
        f_offset,
        aliases: [0; P_INPUT_ALIAS],
        rows: h_layout.cells(),
        cols: f_layout.cells(),
        nnz,
        digest: *hash.finalize().as_bytes(),
    };
    map.aliases = array::from_fn(|c| map.p_source(c));
    Ok(PreparedSha256Ecdsa {
        local,
        map,
        log_n: log_compressions,
        mode,
        lambda,
        h_layout,
        f_layout,
        ligerito: crate::ligerito_flock::LigeritoSelection::for_target(lambda as usize)
            .resolve(f_bits - 7, lambda as usize).map_err(error)?,
    })
}

//! The recursion relation: a BitZ proof that a Flock proof verifies.
//!
//! The relation is the public-challenge Flock verifier circuit (the
//! `flockrec` crate) recorded as a [`StructuredCircuit`]: BLAKE3 compression
//! blocks sharing one template, wired to each other and to the committed
//! proof data, plus a sparse tail of linear checks. It is linear: committed
//! bits `f`, derived bits `h = M·f` over F2, integer rows `C·h = 0`.
//!
//! Layouts (both column-major: cell `c·2^t + b` is row `b` of column `c`):
//!
//! * committed `f`, `t_f = ⌈log₂ hints⌉`: block `i`'s committed bits are
//!   column `i`; then the constant one at `n·2^{t_f}`, then the tail's
//!   committed bits (proof data, settled values) in recording order. The
//!   boxed variant ([`Band`]) moves each block's hints read from outside it
//!   (its committed outputs) to a dyadic band at the top of its column and
//!   the constant and tail to the last dyadic columns, so the opening's
//!   committed weights are block tensors plus two small boxes
//!   ([`VirtualMap::structured_weights`]);
//! * derived `h`, `t_h = ⌈log₂ cells⌉`: block `i`'s cells are column `i`;
//!   then the constant one at `n·2^{t_h}` (`h = f_const`), then the tail cells.
//!
//! Rows: block `i`'s local row `r` is batched at `2^{t_r}·i + r` (`t_r =
//! ⌈log₂ rows⌉`), tail row `k` at `2^{t_r}·n + k`, so one equality tensor
//! `eq(ξ ⊗ η)` batches all of them;
//! the constant cell's row `h_one − 1 = 0` has its own weight `ω`. The
//! collapsed claim `Σ_y V(y)·h(y) = ω` runs an inner sumcheck over the
//! derived grid, and the terminal point claim on `h` is discharged by the
//! virtual BitZ opening of `f` through `M`. The map exposes the template
//! structure, so the transposed weights cost `O(nnz(M_0) + n·2^{t_f} +
//! wiring + tail)` instead of one product per nonzero.

use std::sync::Arc;

use circuit::compact::{BlockTemplate, Coeff, LAZY, PLACEHOLDER, StructuredCircuit};
use circuit::linear_map::binary::{StructuredWeights, VirtualMap};
use circuit::witgen::PackedWitness;
use field::{RingOps, Uint};
use flock_core::pcs::commit::Commitment;
use flock_core::pcs::ligerito::{ProverConfig as LigProverConfig, VerifierConfig as LigVerifierConfig};
#[cfg(feature = "parallel")]
use rayon::prelude::*;

use crate::bitz::{LinearClaim, VirtualStatement};
use crate::ligerito::LOG_PACKING;
use crate::{cfg_into_iter, cfg_iter_mut};
use crate::ligerito_flock::{
    FlockCommitHint, LigeritoSelection, ResolvedLigerito, bind_prover_ood, bind_verifier_ood,
    commit_rs_ligerito_rows,
};
use crate::pcs::IntegerMatrixLayout;
use crate::sumcheck::inner::packed::Sha256InnerGrinding;
use crate::sumcheck::boundary::ProverGrindingRoundBoundary;
use crate::transcript::traits::Transcript;

use super::matrix::eq_table;
use super::profile::{IopInstanceFacts, IopSecurityParams, IopSecurityProfile, Lambda100};
use super::protocol::bitz_opener::{
    self, BitZOpeningProof, opening_params, opening_shape, pcs_from_config,
};
use super::protocol::{ProtocolError, check_boundary, grind_boundary, sample_mod_q};
use super::sumcheck::SumcheckProof;
use super::u32_mul_relation::SpartanBitzField as F;
use super::{SpartanField, absorb_spartan_message, squeeze_field};

type Config = field::FpCtx<2>;
type Gf = field::Gf128;

/// Smallest committed column height (rows) of a block.
pub const MIN_F_ROW_VARS: usize = 7;

const PROTOCOL: &[u8] = b"bitz/flockrec/linear/v1";
pub(crate) const DOMAIN_INITIAL: &[u8] = b"bitz/flockrec/initial-grinding/v1";
pub(crate) const DOMAIN_PRIME: &[u8] = b"bitz/flockrec/prime/v1";
pub(crate) const DOMAIN_TERMINAL: &[u8] = b"bitz/flockrec/terminal-grinding/v1";
const DOMAIN_CLAIM: &[u8] = b"bitz/flockrec/opening-claim/v1";
const LOG_OPENER: &[u8] = b"bitz/flockrec/opener/bitz-log/v1";

/// How the relation's terminal claim on `h` is opened through `f`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FlockRecOpener {
    /// Standard BitZ: the column folds are sent.
    Standard,
    /// BitZ logarithmic ([`crate::bitz::logarithmic`]): the folds' bits are
    /// committed under `mu` and proved. Without explicit opening rows, the
    /// derived grid takes the reference split `t = ⌈0.6 n⌉ − 1`.
    Logarithmic { mu: LigeritoSelection },
}

/// A local column of the template: block-local cells reading it.
struct TemplateColumns {
    offsets: Vec<u32>,
    cells: Vec<u32>,
}

impl TemplateColumns {
    fn new(t: &BlockTemplate) -> Self {
        let cols = t.hints + t.slots;
        let mut counts = vec![0u32; cols + 1];
        for &c in &t.m_terms {
            counts[c as usize + 1] += 1;
        }
        for i in 0..cols {
            counts[i + 1] += counts[i];
        }
        let mut next = counts.clone();
        let mut cells = vec![0u32; t.m_terms.len()];
        for l in 0..t.cells() {
            for &c in t.cell(l) {
                cells[next[c as usize] as usize] = l as u32;
                next[c as usize] += 1;
            }
        }
        Self { offsets: counts, cells }
    }

    fn column(&self, c: usize) -> &[u32] {
        &self.cells[self.offsets[c] as usize..self.offsets[c + 1] as usize]
    }
}

/// CSR rows over `f` indices (wiring inputs and tail cells, after mapping).
#[derive(Default)]
struct Csr {
    offsets: Vec<u64>,
    terms: Vec<u32>,
}

impl Csr {
    fn row(&self, r: usize) -> &[u32] {
        &self.terms[self.offsets[r] as usize..self.offsets[r + 1] as usize]
    }
}

/// A lazy linear group with its inputs mapped to `f` (see
/// [`circuit::compact::LinearGroup`]).
struct Group {
    terms: Vec<([u128; 128], Vec<(Vec<u32>, bool)>)>,
}

impl Group {
    /// The 128 output bits from the committed bits.
    fn evaluate(&self, bit: &dyn Fn(usize) -> bool) -> u128 {
        let mut out = 0u128;
        for (columns, inputs) in &self.terms {
            for (j, (support, constant)) in inputs.iter().enumerate() {
                let v = support.iter().fold(*constant, |acc, &f| acc ^ bit(f as usize));
                if v {
                    out ^= columns[j];
                }
            }
        }
        out
    }

    /// `(f index, weight)` contributions of output weights `v` to the inputs:
    /// input bit `j` of a term gets `L_v(columns[j]) = Σ_i v_i·[columns[j]]_i`,
    /// through sixteen byte tables over `v`.
    fn scatter(&self, v: &[Gf; 128], constant: &mut Gf, out: &mut Vec<(u32, Gf)>) {
        let zero = Gf::from_bytes([0; 16]);
        let mut tables = vec![[zero; 256]; 16];
        for (b, table) in tables.iter_mut().enumerate() {
            for byte in 1..256usize {
                let low = byte & (byte - 1);
                table[byte] = table[low] + v[8 * b + (byte ^ low).trailing_zeros() as usize];
            }
        }
        for (columns, inputs) in &self.terms {
            for (column, (support, c)) in columns.iter().zip(inputs) {
                if support.is_empty() && !*c {
                    continue;
                }
                let mut w = zero;
                for (b, table) in tables.iter().enumerate() {
                    w += table[((column >> (8 * b)) & 0xFF) as usize];
                }
                for &f in support {
                    out.push((f, w));
                }
                if *c {
                    *constant += w;
                }
            }
        }
    }
}

/// Where the blocks' derived cells live. Block `i`'s first `2^ha` cells are
/// region A, `(i << ha) | l`; with `hb`, its remaining cells (at most `2^hb`,
/// `hb < ha`) are region B, `b0 + ((i << hb) | (l − 2^ha))` with `b0 =
/// n·2^ha`. A template of `2^ha + 2^hb` cells or fewer then pads to that,
/// not to `2^{ha+1}` (SHA-256: 20,456 cells in 20,480 instead of 32,768).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Regions {
    pub ha: usize,
    pub hb: Option<usize>,
    pub b0: usize,
}

impl Regions {
    /// The split for `cells` per block (`FLOCKREC_NO_SPLIT=1`: none).
    fn for_cells(cells: usize, n: usize) -> Self {
        let ht = ceil_log2(cells).max(MIN_F_ROW_VARS);
        let split = (ht > MIN_F_ROW_VARS && std::env::var_os("FLOCKREC_NO_SPLIT").is_none())
            .then(|| (ht - 1, ceil_log2(cells - (1 << (ht - 1))).max(MIN_F_ROW_VARS)))
            .filter(|&(ha, hb)| hb < ha);
        match split {
            Some((ha, hb)) => Self { ha, hb: Some(hb), b0: n << ha },
            None => Self { ha: ht, hb: None, b0: n << ht },
        }
    }

    /// Block `i`'s local cell `l` in the derived grid.
    fn cell(&self, i: usize, l: usize) -> usize {
        match self.hb {
            Some(hb) if l >= 1 << self.ha => self.b0 + ((i << hb) | (l - (1 << self.ha))),
            _ => (i << self.ha) | l,
        }
    }

    /// The first derived index after the blocks (`2^ha`-aligned).
    fn end(&self, n: usize) -> usize {
        match self.hb {
            Some(hb) => self.b0 + (n << hb).next_multiple_of(1 << self.ha),
            None => self.b0,
        }
    }
}

/// The boxed committed layout (see [`VirtualMap::structured_weights`]): a
/// block column of `2^{t_f}` rows holds the template's hints `0..local_start`
/// at their own rows and the suffix `local_start..hints` (every hint read
/// from outside its block: the committed output copies) in the dyadic band
/// `rows_start..rows_start + 2^log`; the constant cell and the tail start at
/// column `tail_col`, the last `2^tail_log` columns. The grid keeps the plain
/// layout's size.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Band {
    pub local_start: usize,
    pub rows_start: usize,
    pub log: usize,
    pub tail_col: usize,
    pub tail_log: usize,
}

impl Band {
    /// The band for `n` blocks of `hints` committed bits in columns of `2^ft`
    /// rows whose hints from `local_start` on are read from outside, with
    /// `tail` committed bits after them in a grid of `2^f_vars` cells, when
    /// it fits (`FLOCKREC_NO_BAND=1`: never). The band is at least a word.
    fn fit(hints: usize, local_start: usize, ft: usize, n: usize, tail: usize, f_vars: usize) -> Option<Self> {
        if std::env::var_os("FLOCKREC_NO_BAND").is_some() || local_start > hints {
            return None;
        }
        let log = ceil_log2(hints - local_start).max(6);
        let rows_start = (1usize << ft).checked_sub(1 << log)?;
        let tail_log = ceil_log2((1 + tail).div_ceil(1 << ft));
        let cols = 1usize << (f_vars - ft);
        (local_start <= rows_start && n + (1 << tail_log) <= cols)
            .then_some(Self { local_start, rows_start, log, tail_col: cols - (1 << tail_log), tail_log })
    }

    /// Committed row of template hint `h` inside its block's column.
    fn row(&self, h: usize) -> usize {
        if h >= self.local_start { self.rows_start + (h - self.local_start) } else { h }
    }

    /// Copies a block's `hints` committed bits (from bit `from` of `src`)
    /// into its column `dst`, whose other bits are untouched.
    fn copy_block(band: Option<Self>, src: &[u64], from: usize, hints: usize, dst: &mut [u64]) {
        match band {
            Some(b) => {
                copy_bits(src, from, b.local_start, dst);
                copy_bits(src, from + b.local_start, hints - b.local_start, &mut dst[b.rows_start / 64..]);
            }
            None => copy_bits(src, from, hints, dst),
        }
    }
}

/// The first hint of `template` read from outside its block. A block's only
/// handles outside it are its outputs (the recorder hands out nothing else),
/// so those are the hints in the outputs' supports; the structured weights
/// still check every external read lands in the band.
pub fn external_hints_start(template: &BlockTemplate) -> usize {
    template
        .outputs
        .iter()
        .flat_map(|output| output.terms.iter())
        .map(|&l| l as usize)
        .filter(|&l| l < template.hints)
        .min()
        .unwrap_or(template.hints)
}

/// The derived-from-committed map of the relation.
pub struct FlockRecMap {
    /// Row variables of the committed block columns, and of the derived grid
    /// the opening runs against (`hr ≥ ha`: blocks' regions stacked per
    /// opening column; flat derived indices are unchanged).
    ft: usize,
    pub(crate) regions: Regions,
    hr: usize,
    template: BlockTemplate,
    columns: TemplateColumns,
    n: usize,
    band: Option<Band>,
    /// Cells with a constant flag in the template.
    template_const_cells: Vec<u32>,
    /// Block `i`, slot `s`: its `f` support (`n·slots` rows) and constant.
    wiring: Csr,
    wiring_const: Vec<bool>,
    /// Tail cell `k` (derived index `h_tail + 1 + k`): its `f` support and
    /// the lazy group outputs (`128·group + bit`) it XORs in.
    tail: Csr,
    tail_lazy: Csr,
    tail_const: Vec<bool>,
    groups: Vec<Group>,
    /// Tail cells with every lazy output expanded (generic path only).
    expanded_tail: std::sync::OnceLock<Csr>,
    f_cells: usize,
    h_cells: usize,
    f_const: usize,
    h_tail: usize,
    nnz: usize,
    digest: [u8; 32],
}

/// Per class of a region (the blocks sharing rows of the opening grid): the
/// template columns' weights `T_c[col]` and the template constants' weight.
type Classes = Vec<(Vec<Gf>, Gf)>;

impl FlockRecMap {
    fn block_cell(&self, i: usize, l: u32) -> usize {
        self.regions.cell(i, l as usize)
    }

    /// Committed row of template hint `h` inside its block's column.
    fn f_row(&self, h: usize) -> usize {
        self.band.map_or(h, |b| b.row(h))
    }

    /// The template hint at row `r` of a block's column, if any.
    fn f_local(&self, r: usize) -> Option<usize> {
        let hints = self.template.hints;
        match self.band {
            Some(b) if r >= b.rows_start => Some(b.local_start + (r - b.rows_start)).filter(|&h| h < hints),
            Some(b) if r >= b.local_start => None,
            _ => (r < hints).then_some(r),
        }
    }

    /// The regions' classes for the opening grid's row weights: a region of
    /// `2^h` cells per block sits at rows `(k mod g)·2^h` of opening column
    /// `k / g`, `k` its index in the region (`g = 2^{hr−h}`); per class `c = k
    /// mod g`, `T_c[col] = Σ_{l ∈ M_0 column col, in the region} row_w[c·2^h +
    /// l − start]`. The columns list their cells in increasing order, so a
    /// region is a contiguous run of each.
    fn region_classes(&self, row_w: &[Gf]) -> (Classes, Option<Classes>) {
        let tm = &self.template;
        let zero = Gf::from_bytes([0; 16]);
        let reg = self.regions;
        let la = 1usize << reg.ha;
        let region = |h: usize, cells: std::ops::Range<usize>| -> Classes {
            let (lo, hi) = (cells.start as u32, cells.end as u32);
            let part = |list: &[u32]| -> std::ops::Range<usize> { list.partition_point(|&l| l < lo)..list.partition_point(|&l| l < hi) };
            cfg_into_iter!(0..1usize << (self.hr - h))
                .map(|c| {
                    let base = c << h;
                    let sum = |list: &[u32]| list[part(list)].iter().fold(zero, |acc, &l| acc + row_w[base + (l - lo) as usize]);
                    let local: Vec<Gf> = (0..tm.hints + tm.slots).map(|col| sum(self.columns.column(col))).collect();
                    (local, sum(&self.template_const_cells))
                })
                .collect()
        };
        let classes_a = region(reg.ha, 0..la);
        let classes_b = reg.hb.map(|hb| region(hb, la..tm.cells().max(la)));
        (classes_a, classes_b)
    }

    /// Block `i`'s (class, opening column) in region A, and in region B when
    /// there is one.
    fn block_terms(&self, i: usize) -> ((usize, usize), Option<(usize, usize)>) {
        let reg = self.regions;
        let ga = self.hr - reg.ha;
        let b = reg.hb.map(|hb| {
            let (k, gb) = ((reg.b0 >> hb) + i, self.hr - hb);
            (k & ((1 << gb) - 1), k >> gb)
        });
        ((i & ((1 << ga) - 1), i >> ga), b)
    }

    /// Every derived cell reading committed cell `j`, unsorted.
    fn targets(&self, j: usize) -> Vec<usize> {
        let mut out = Vec::new();
        let t = &self.template;
        if j < self.n << self.ft {
            if let Some(h) = self.f_local(j & ((1 << self.ft) - 1)) {
                out.extend(self.columns.column(h).iter().map(|&l| self.block_cell(j >> self.ft, l)));
            }
        }
        if j == self.f_const {
            out.push(self.h_tail);
            for i in 0..self.n {
                out.extend(self.template_const_cells.iter().map(|&l| self.block_cell(i, l)));
            }
        }
        for i in 0..self.n {
            for s in 0..t.slots {
                let row = i * t.slots + s;
                let hit = if j == self.f_const {
                    self.wiring_const[row]
                } else {
                    self.wiring.row(row).contains(&(j as u32))
                };
                if hit {
                    out.extend(self.columns.column(t.hints + s).iter().map(|&l| self.block_cell(i, l)));
                }
            }
        }
        let expanded = self.expanded_tail();
        for k in 0..self.tail_const.len() {
            let hit = if j == self.f_const {
                self.expanded_const(k)
            } else {
                expanded.row(k).contains(&(j as u32))
            };
            if hit {
                out.push(self.h_tail + 1 + k);
            }
        }
        out
    }

    /// Lazy output `id`'s explicit support and constant.
    fn expand_lazy(&self, id: u32) -> (Vec<u32>, bool) {
        let (g, i) = (id as usize / 128, id as usize % 128);
        let mut terms = Vec::new();
        let mut constant = false;
        for (columns, inputs) in &self.groups[g].terms {
            for (column, (support, c)) in columns.iter().zip(inputs) {
                if (column >> i) & 1 == 1 {
                    terms.extend_from_slice(support);
                    constant ^= *c;
                }
            }
        }
        terms.sort_unstable();
        let mut kept = Vec::with_capacity(terms.len());
        let mut a = 0;
        while a < terms.len() {
            let mut e = a + 1;
            while e < terms.len() && terms[e] == terms[a] {
                e += 1;
            }
            if (e - a) % 2 == 1 {
                kept.push(terms[a]);
            }
            a = e;
        }
        (kept, constant)
    }

    fn expanded_const(&self, k: usize) -> bool {
        self.tail_lazy.row(k).iter().fold(self.tail_const[k], |acc, &id| acc ^ self.expand_lazy(id).1)
    }

    fn expanded_tail(&self) -> &Csr {
        self.expanded_tail.get_or_init(|| {
            let mut csr = Csr { offsets: vec![0], terms: Vec::new() };
            for k in 0..self.tail_const.len() {
                let mut v = self.tail.row(k).to_vec();
                for &id in self.tail_lazy.row(k) {
                    v.extend(self.expand_lazy(id).0);
                }
                v.sort_unstable();
                let mut kept = Vec::with_capacity(v.len());
                let mut a = 0;
                while a < v.len() {
                    let mut e = a + 1;
                    while e < v.len() && v[e] == v[a] {
                        e += 1;
                    }
                    if (e - a) % 2 == 1 {
                        kept.push(v[a]);
                    }
                    a = e;
                }
                csr.terms.extend(kept);
                csr.offsets.push(csr.terms.len() as u64);
            }
            csr
        })
    }
}

impl VirtualMap for FlockRecMap {
    type ColumnRows<'a> = std::vec::IntoIter<usize>;

    fn rows(&self) -> usize {
        self.h_cells
    }

    fn cols(&self) -> usize {
        self.f_cells
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

    /// The generic (slow, `O(map)` per column) path; the opening uses
    /// [`VirtualMap::transpose_weights`].
    fn column_rows(&self, column: usize) -> Option<Self::ColumnRows<'_>> {
        if column >= self.f_cells {
            return None;
        }
        let mut rows = self.targets(column);
        rows.sort_unstable();
        // XOR semantics: a cell reading the same source twice does not read it.
        let mut out = Vec::with_capacity(rows.len());
        let mut i = 0;
        while i < rows.len() {
            let mut k = i + 1;
            while k < rows.len() && rows[k] == rows[i] {
                k += 1;
            }
            if (k - i) % 2 == 1 {
                out.push(rows[i]);
            }
            i = k;
        }
        Some(out.into_iter())
    }

    fn transpose_weights(&self, row_w: &[Gf], col_w: &[Gf], t: usize, cells: usize) -> Option<Vec<Gf>> {
        if t != self.hr {
            return None;
        }
        let tm = &self.template;
        let zero = Gf::from_bytes([0; 16]);
        let weight_of = |d: usize| row_w[d & ((1 << t) - 1)] * col_w[d >> t];
        let classes = self.region_classes(row_w);
        let in_a = |i: usize| {
            let ((c, col), _) = self.block_terms(i);
            (&classes.0[c], col_w[col])
        };
        let in_b = |i: usize| {
            let (_, b) = self.block_terms(i);
            b.zip(classes.1.as_ref()).map(|((c, col), cb)| (&cb[c], col_w[col]))
        };
        let mut out = vec![zero; cells];
        // Block committed bits: hint h of block i weighs Σ_regions col_w·T_c[h],
        // at row f_row(h).
        {
            let blocks = &mut out[..self.n << self.ft];
            let (ls, rs) = self.band.map_or((tm.hints, tm.hints), |b| (b.local_start, b.rows_start));
            let top = rs + (tm.hints - ls);
            let fill = |(i, column): (usize, &mut [Gf])| {
                let ((local, _), w) = in_a(i);
                for (dst, &lw) in column[..ls].iter_mut().zip(&local[..ls]) {
                    *dst = w * lw;
                }
                for (dst, &lw) in column[rs..top].iter_mut().zip(&local[ls..tm.hints]) {
                    *dst = w * lw;
                }
                if let Some(((local, _), w)) = in_b(i) {
                    for (dst, &lw) in column[..ls].iter_mut().zip(&local[..ls]) {
                        *dst += w * lw;
                    }
                    for (dst, &lw) in column[rs..top].iter_mut().zip(&local[ls..tm.hints]) {
                        *dst += w * lw;
                    }
                }
            };
            #[cfg(feature = "parallel")]
            blocks.par_chunks_mut(1 << self.ft).enumerate().for_each(fill);
            #[cfg(not(feature = "parallel"))]
            blocks.chunks_mut(1 << self.ft).enumerate().for_each(fill);
        }
        // Wiring: slot s of block i scatters col_w[i/g]·T_{i mod g}[hints + s],
        // blocks in parallel: field additions are XORs of the two words, so
        // atomic XORs give the serial sums exactly.
        let wiring_block = |i: usize, scatter: &mut dyn FnMut(usize, Gf)| -> Gf {
            let ((local, class_const), bw) = in_a(i);
            let b = in_b(i);
            let mut constant = bw * *class_const;
            if let Some(((_, b_const), bw_b)) = b {
                constant += bw_b * *b_const;
            }
            for s in 0..tm.slots {
                let row = i * tm.slots + s;
                let support = self.wiring.row(row);
                if support.is_empty() && !self.wiring_const[row] {
                    continue;
                }
                let mut w = bw * local[tm.hints + s];
                if let Some(((local_b, _), bw_b)) = b {
                    w += bw_b * local_b[tm.hints + s];
                }
                for &j in support {
                    scatter(j as usize, w);
                }
                if self.wiring_const[row] {
                    constant += w;
                }
            }
            constant
        };
        #[cfg(feature = "parallel")]
        let mut constant = {
            use std::sync::atomic::{AtomicU64, Ordering::Relaxed};
            // SAFETY: `Gf` is `repr(C)` `{lo, hi}: u64` and `out` is borrowed
            // exclusively for the scatter's duration.
            let words: &[AtomicU64] = unsafe { std::slice::from_raw_parts(out.as_mut_ptr() as *const AtomicU64, 2 * out.len()) };
            (0..self.n)
                .into_par_iter()
                .with_min_len(16)
                .map(|i| {
                    wiring_block(i, &mut |j, w| {
                        words[2 * j].fetch_xor(w.lo, Relaxed);
                        words[2 * j + 1].fetch_xor(w.hi, Relaxed);
                    })
                })
                .reduce(|| zero, |a, b| a + b)
        };
        #[cfg(not(feature = "parallel"))]
        let mut constant = (0..self.n).fold(zero, |acc, i| acc + wiring_block(i, &mut |j, w| out[j] += w));
        // The constant cell itself, then the tail: explicit supports scatter
        // their cell's weight; lazy outputs collect it per group bit.
        constant += weight_of(self.h_tail);
        let mut group_weights = vec![[zero; 128]; self.groups.len()];
        let tail_cell = |k: usize, scatter: &mut dyn FnMut(usize, Gf), lazy: &mut dyn FnMut(usize, Gf)| -> Gf {
            let w = weight_of(self.h_tail + 1 + k);
            for &j in self.tail.row(k) {
                scatter(j as usize, w);
            }
            for &id in self.tail_lazy.row(k) {
                lazy(id as usize, w);
            }
            if self.tail_const[k] { w } else { zero }
        };
        #[cfg(feature = "parallel")]
        {
            use std::sync::atomic::{AtomicU64, Ordering::Relaxed};
            // SAFETY: as for the wiring; `group_weights` is contiguous `Gf`s
            // (`[Gf; 128]` per group, lazy id `128·group + bit`).
            let words: &[AtomicU64] = unsafe { std::slice::from_raw_parts(out.as_mut_ptr() as *const AtomicU64, 2 * out.len()) };
            let xor = |words: &[AtomicU64], j: usize, w: Gf| {
                words[2 * j].fetch_xor(w.lo, Relaxed);
                words[2 * j + 1].fetch_xor(w.hi, Relaxed);
            };
            {
                let lazy_words: &[AtomicU64] = unsafe {
                    std::slice::from_raw_parts(group_weights.as_mut_ptr() as *const AtomicU64, 2 * 128 * group_weights.len())
                };
                constant += (0..self.tail_const.len())
                    .into_par_iter()
                    .with_min_len(1 << 10)
                    .map(|k| tail_cell(k, &mut |j, w| xor(words, j, w), &mut |id, w| xor(lazy_words, id, w)))
                    .reduce(|| zero, |a, b| a + b);
            }
            constant += (0..self.groups.len())
                .into_par_iter()
                .map(|g| {
                    let mut c = zero;
                    let mut contributions = Vec::new();
                    self.groups[g].scatter(&group_weights[g], &mut c, &mut contributions);
                    for (j, w) in contributions {
                        xor(words, j as usize, w);
                    }
                    c
                })
                .reduce(|| zero, |a, b| a + b);
        }
        #[cfg(not(feature = "parallel"))]
        {
            for k in 0..self.tail_const.len() {
                constant += tail_cell(k, &mut |j, w| out[j] += w, &mut |id, w| group_weights[id / 128][id % 128] += w);
            }
            for g in 0..self.groups.len() {
                let mut contributions = Vec::new();
                self.groups[g].scatter(&group_weights[g], &mut constant, &mut contributions);
                for (j, w) in contributions {
                    out[j as usize] += w;
                }
            }
        }
        out[self.f_const] += constant;
        Some(out)
    }

    fn has_transposed_mle(&self) -> bool {
        true
    }

    /// The transposed weights' extension at `point` without forming them:
    /// the block hints through the classes' dot products with the low row
    /// variables' equality table (block columns are aligned), every external
    /// read (wiring, tail, lazy groups) through its committed index.
    fn transposed_mle(&self, row_w: &[Gf], col_w: &[Gf], t: usize, point: &[Gf]) -> Option<Gf> {
        if t != self.hr || (1usize << point.len()) != self.f_cells || point.len() < self.ft {
            return None;
        }
        let tm = &self.template;
        let zero = Gf::from_bytes([0; 16]);
        let low = self.ft;
        let eq_table = |p: &[Gf]| -> Vec<Gf> {
            let mut table = vec![Gf::ONE];
            for &c in p {
                let half = table.len();
                for k in 0..half {
                    let hi = table[k] * c;
                    table.push(hi);
                    table[k] += hi;
                }
            }
            table
        };
        let (lo, hi) = (eq_table(&point[..low]), eq_table(&point[low..]));
        let eq = |j: usize| lo[j & ((1 << low) - 1)] * hi[j >> low];
        let weight_of = |d: usize| row_w[d & ((1 << t) - 1)] * col_w[d >> t];
        let support_eq = |support: &[u32]| support.iter().fold(zero, |acc, &j| acc + eq(j as usize));
        let eq_const = eq(self.f_const);
        let classes = self.region_classes(row_w);
        let dots = |region: &Classes| -> Vec<Gf> {
            region.iter().map(|(local, _)| (0..tm.hints).fold(zero, |acc, h| acc + local[h] * lo[self.f_row(h)])).collect()
        };
        let dots_a = dots(&classes.0);
        let dots_b = classes.1.as_ref().map(dots);
        // Blocks: hint h of block i weighs Σ_regions col_w·T_c[h], at row f_row(h)
        // of column i; then its slots' reads and the template constants.
        let blocks: Gf = cfg_into_iter!(0..self.n, 16)
            .map(|i| {
                let ((ca, cola), b) = self.block_terms(i);
                let (entry_a, wa) = (&classes.0[ca], col_w[cola]);
                let b = b.zip(classes.1.as_ref().zip(dots_b.as_ref())).map(|((cb, colb), (region, db))| (&region[cb], col_w[colb], db[cb]));
                let mut own = wa * dots_a[ca];
                let mut constant = wa * entry_a.1;
                if let Some((entry_b, wb, dot)) = b {
                    own += wb * dot;
                    constant += wb * entry_b.1;
                }
                let mut acc = own * hi[i] + constant * eq_const;
                for s in 0..tm.slots {
                    let row = i * tm.slots + s;
                    let support = self.wiring.row(row);
                    if support.is_empty() && !self.wiring_const[row] {
                        continue;
                    }
                    let mut w = wa * entry_a.0[tm.hints + s];
                    if let Some((entry_b, wb, _)) = b {
                        w += wb * entry_b.0[tm.hints + s];
                    }
                    let mut e = support_eq(support);
                    if self.wiring_const[row] {
                        e += eq_const;
                    }
                    acc += w * e;
                }
                acc
            })
            .reduce(|| zero, |a, b| a + b);
        let mut total = blocks + weight_of(self.h_tail) * eq_const;
        // Tail cells, then their lazy groups.
        let mut group_weights = vec![[zero; 128]; self.groups.len()];
        for k in 0..self.tail_const.len() {
            let w = weight_of(self.h_tail + 1 + k);
            let mut e = support_eq(self.tail.row(k));
            if self.tail_const[k] {
                e += eq_const;
            }
            total += w * e;
            for &id in self.tail_lazy.row(k) {
                group_weights[id as usize / 128][id as usize % 128] += w;
            }
        }
        let groups: Gf = cfg_into_iter!(0..self.groups.len())
            .map(|g| {
                let mut constant = zero;
                let mut contributions = Vec::new();
                self.groups[g].scatter(&group_weights[g], &mut constant, &mut contributions);
                contributions.iter().fold(constant * eq_const, |acc, &(j, w)| acc + w * eq(j as usize))
            })
            .reduce(|| zero, |a, b| a + b);
        Some(total + groups)
    }

    /// The boxed layout's weights: block column `i < n` is `col_w·T_c` of
    /// its class in region A (`term`) and in region B (`term2`), one row
    /// vector per class at its hints' rows; every external read lands in a
    /// block's band (the row box) or the tail columns (the column box), else
    /// `None`.
    fn structured_weights(&self, row_w: &[Gf], col_w: &[Gf], t: usize) -> Option<StructuredWeights> {
        let band = self.band?;
        if t != self.hr {
            return None;
        }
        let tm = &self.template;
        let zero = Gf::from_bytes([0; 16]);
        let (tf, n) = (self.ft, self.n);
        let s = (self.f_cells >> tf).ilog2() as usize;
        let weight_of = |d: usize| row_w[d & ((1 << t) - 1)] * col_w[d >> t];
        let classes = self.region_classes(row_w);
        let all: Vec<&(Vec<Gf>, Gf)> = classes.0.iter().chain(classes.1.iter().flatten()).collect();
        let rows: Vec<Vec<Gf>> = cfg_into_iter!(0..all.len())
            .map(|k| {
                let mut r = vec![zero; 1 << tf];
                for (h, &w) in all[k].0[..tm.hints].iter().enumerate() {
                    r[band.row(h)] = w;
                }
                r
            })
            .collect();
        let offset_b = classes.0.len() as u32;
        let (mut term, mut weight) = (vec![u32::MAX; 1 << s], vec![zero; 1 << s]);
        let two = classes.1.is_some();
        let (mut term2, mut weight2) = if two { (vec![u32::MAX; 1 << s], vec![zero; 1 << s]) } else { (Vec::new(), Vec::new()) };
        for c in 0..n {
            let ((ca, cola), b) = self.block_terms(c);
            (term[c], weight[c]) = (ca as u32, col_w[cola]);
            if let (Some((cb, colb)), true) = (b, two) {
                (term2[c], weight2[c]) = (offset_b + cb as u32, col_w[colb]);
            }
        }
        let (r1, b1, c2, b2) = (band.rows_start, band.log, band.tail_col, band.tail_log);
        // Reused buffers (their pages stay faulted in across proofs).
        let zeroed = |len: usize| {
            let mut v = crate::bitz::pool::take(len);
            cfg_iter_mut!(v, 1 << 14).for_each(|x| *x = zero);
            v
        };
        let mut row_box = zeroed(1 << (b1 + s));
        let mut col_box = zeroed(1 << (b2 + tf));
        let mask = (1usize << tf) - 1;
        {
            let (rb, cb) = (atomic_words(&mut row_box), atomic_words(&mut col_box));
            let put = |j: usize, w: Gf| -> bool {
                let (c, r) = (j >> tf, j & mask);
                if c >= c2 {
                    xor_at(cb, ((c - c2) << tf) | r, w);
                } else if c < n && r >= r1 {
                    xor_at(rb, (c << b1) | (r - r1), w);
                } else {
                    return false;
                }
                true
            };
            // Wiring: slot `slot` of block i scatters Σ_regions col_w·T_c[hints + slot].
            let (wiring_constant, wiring_ok) = cfg_into_iter!(0..n, 16)
                .map(|i| {
                    let ((ca, cola), b) = self.block_terms(i);
                    let ((local, class_const), bw) = (&classes.0[ca], col_w[cola]);
                    let b = b.zip(classes.1.as_ref()).map(|((cb, colb), region)| (&region[cb], col_w[colb]));
                    let mut constant = bw * *class_const;
                    if let Some(((_, b_const), bw_b)) = b {
                        constant += bw_b * *b_const;
                    }
                    let mut ok = true;
                    for slot in 0..tm.slots {
                        let row = i * tm.slots + slot;
                        let support = self.wiring.row(row);
                        if support.is_empty() && !self.wiring_const[row] {
                            continue;
                        }
                        let mut w = bw * local[tm.hints + slot];
                        if let Some(((local_b, _), bw_b)) = b {
                            w += bw_b * local_b[tm.hints + slot];
                        }
                        for &j in support {
                            ok &= put(j as usize, w);
                        }
                        if self.wiring_const[row] {
                            constant += w;
                        }
                    }
                    (constant, ok)
                })
                .reduce(|| (zero, true), |a, b| (a.0 + b.0, a.1 && b.1));
            // The tail: explicit supports scatter their cell's weight; lazy
            // outputs collect it per group bit.
            let mut group_weights = vec![zero; 128 * self.groups.len()];
            let (tail_constant, tail_ok) = {
                let gw = atomic_words(&mut group_weights);
                let cells = self.tail_const.len();
                cfg_into_iter!(0..cells.div_ceil(1024))
                    .map(|chunk| {
                        let mut constant = zero;
                        let mut ok = true;
                        for k in chunk * 1024..((chunk + 1) * 1024).min(cells) {
                            let w = weight_of(self.h_tail + 1 + k);
                            for &j in self.tail.row(k) {
                                ok &= put(j as usize, w);
                            }
                            for &id in self.tail_lazy.row(k) {
                                xor_at(gw, id as usize, w);
                            }
                            if self.tail_const[k] {
                                constant += w;
                            }
                        }
                        (constant, ok)
                    })
                    .reduce(|| (zero, true), |a, b| (a.0 + b.0, a.1 && b.1))
            };
            let (group_constant, group_ok) = cfg_into_iter!(0..self.groups.len())
                .map(|gi| {
                    let weights: &[Gf; 128] = group_weights[128 * gi..128 * (gi + 1)].try_into().expect("128 group bits");
                    let mut c = zero;
                    let mut contributions = Vec::new();
                    self.groups[gi].scatter(weights, &mut c, &mut contributions);
                    let ok = contributions.iter().fold(true, |ok, &(j, w)| ok & put(j as usize, w));
                    (c, ok)
                })
                .reduce(|| (zero, true), |a, b| (a.0 + b.0, a.1 && b.1));
            // The constant cell collects every constant's weight and its own.
            let constant = wiring_constant + tail_constant + group_constant + weight_of(self.h_tail);
            if !(wiring_ok && tail_ok && group_ok && put(self.f_const, constant)) {
                crate::bitz::pool::recycle(row_box);
                crate::bitz::pool::recycle(col_box);
                return None;
            }
        }
        Some(StructuredWeights {
            t: tf,
            s,
            rows,
            term,
            weight,
            term2,
            weight2,
            row_box_start: r1,
            row_box_log: b1,
            row_box,
            col_box_start: c2,
            col_box_log: b2,
            col_box,
        })
    }
}

/// The table as atomic polynomial words: GF(2^128) addition is XOR, so
/// concurrent scatters into it commute.
fn atomic_words(v: &mut [Gf]) -> &[std::sync::atomic::AtomicU64] {
    // SAFETY: `Gf128` is `repr(C)` over two `u64` words, `AtomicU64` has
    // `u64`'s layout, and the exclusive borrow keeps every other access out
    // while the atomics live.
    unsafe { std::slice::from_raw_parts(v.as_mut_ptr().cast::<std::sync::atomic::AtomicU64>(), 2 * v.len()) }
}

fn xor_at(words: &[std::sync::atomic::AtomicU64], k: usize, w: Gf) {
    use std::sync::atomic::Ordering::Relaxed;
    words[2 * k].fetch_xor(w.lo, Relaxed);
    words[2 * k + 1].fetch_xor(w.hi, Relaxed);
}

/// Facts about the circuit's integer rows the security profile consumes.
/// `lift_log2`: the fold's lift arity (`opening_t`, one more for the
/// logarithmic opening's 127-bit slots).
fn instance_facts(log_rows: u32, opening_t: u32, lift_log2: u32) -> IopInstanceFacts {
    IopInstanceFacts {
        // Every row value is below 2^66 in magnitude (64-bit zero checks,
        // 32-bit additions with their carries).
        defect_log2_bound: 66,
        lift_arity_log2: lift_log2,
        opening_t,
        direct_opening: false,
        tau_arity: log_rows,
        piop_degree: 2,
        step50_magnitude_log2: 0,
    }
}

/// The prepared relation (both roles build it from the public recording).
pub struct FlockRecRelation {
    st: Arc<StructuredCircuit>,
    pub(crate) map: FlockRecMap,
    pub(crate) n: usize,
    /// Template row variables (`t_r`).
    pub(crate) rt: usize,
    pub(crate) inst_vars: usize,
    pub(crate) f_layout: IntegerMatrixLayout,
    pub(crate) h_layout: IntegerMatrixLayout,
    /// Global committed index → `f` index.
    global_f: Vec<u32>,
    /// Tail cell global index → tail position (sorted search).
    tail_rows_local: Vec<u32>,
    pub(crate) security: IopSecurityParams,
    pub(crate) ligerito: ResolvedLigerito,
    source_digest: [u8; 32],
    /// The logarithmic opening ([`FlockRecOpener::Logarithmic`]); `None`
    /// opens with standard BitZ.
    log: Option<crate::bitz::logarithmic::LogOpener>,
}

/// The 8×64 bit transpose: byte `p` of the result has bit `k` = bit `p` of
/// `rows[k]` (eight 8×8 transposes by delta swaps).
fn transpose_8x64(rows: &[u64; 8]) -> [u8; 64] {
    let mut out = [0u8; 64];
    for j in 0..8 {
        // Byte k = rows[k]'s bits 8j..8j+8; afterwards byte p holds bit p of every row.
        let mut x = 0u64;
        for (k, &row) in rows.iter().enumerate() {
            x |= ((row >> (8 * j)) & 0xFF) << (8 * k);
        }
        let t = (x ^ (x >> 7)) & 0x00AA_00AA_00AA_00AA;
        x ^= t ^ (t << 7);
        let t = (x ^ (x >> 14)) & 0x0000_CCCC_0000_CCCC;
        x ^= t ^ (t << 14);
        let t = (x ^ (x >> 28)) & 0x0000_0000_F0F0_F0F0;
        x ^= t ^ (t << 28);
        out[8 * j..8 * j + 8].copy_from_slice(&x.to_le_bytes());
    }
    out
}


/// Writes bits `start..start + len` of `src` (least significant first) to
/// the first `len` bits of `dst`, whose remaining bits are untouched.
fn copy_bits(src: &[u64], start: usize, len: usize, dst: &mut [u64]) {
    let (word, shift) = (start / 64, start % 64);
    let full = len / 64;
    let at = |k: usize| src.get(k).copied().unwrap_or(0);
    let read = |w: usize| {
        if shift == 0 { at(word + w) } else { (at(word + w) >> shift) | (at(word + w + 1) << (64 - shift)) }
    };
    for (w, d) in dst[..full].iter_mut().enumerate() {
        *d = read(w);
    }
    let rest = len % 64;
    if rest != 0 {
        let mask = (1u64 << rest) - 1;
        dst[full] = (dst[full] & !mask) | (read(full) & mask);
    }
}

/// `G[l] = Σ_{i<n} e[i]·h_i[l]` for the `len` bits `h_i = words_of(i)` of
/// each block, eight blocks at a time: their bits at position `l` form a byte
/// indexing the subset sums of their `e[i]` (the same field sum, regrouped).
fn instance_combined<'a>(n: usize, len: usize, e: &[F], words_of: &(dyn Fn(usize) -> &'a [u64] + Sync), cfg: &Config) -> Vec<F> {
    let zero = F::zero_with_cfg(cfg);
    let partials: Vec<Vec<F>> = cfg_into_iter!(0..n.div_ceil(512))
        .map(|chunk| {
            let mut g = vec![zero; len];
            for group in (chunk * 512..((chunk + 1) * 512).min(n)).step_by(8) {
                let members = (n - group).min(8);
                let mut sums = [zero; 256];
                for byte in 1..256usize {
                    let low = byte & (byte - 1);
                    let k = (byte ^ low).trailing_zeros() as usize;
                    sums[byte] = if k < members { cfg.add(&sums[low], &e[group + k]) } else { sums[low] };
                }
                let rows: Vec<&[u64]> = (0..members).map(|k| words_of(group + k)).collect();
                for w in 0..len / 64 {
                    let mut lanes = [0u64; 8];
                    for (k, row) in rows.iter().enumerate() {
                        lanes[k] = row[w];
                    }
                    if lanes.iter().all(|&x| x == 0) {
                        continue;
                    }
                    // Byte p of the result: bit k = block k's bit 64·w + p.
                    let bytes = transpose_8x64(&lanes);
                    for (p, &byte) in bytes.iter().enumerate() {
                        if byte != 0 {
                            let l = 64 * w + p;
                            g[l] = cfg.add(&g[l], &sums[byte as usize]);
                        }
                    }
                }
            }
            g
        })
        .collect();
    let mut table = vec![zero; len];
    for part in &partials {
        for (dst, x) in table.iter_mut().zip(part) {
            *dst = cfg.add(dst, x);
        }
    }
    table
}

/// `h_i(r) = Σ_l eq_loc[l]·h_i[l]` for each block's `len` bits, through byte
/// tables over `eq_loc`, a cache-sized slice of them at a time across every
/// block (the same field sum, regrouped).
fn region_values<'a>(n: usize, len: usize, eq_loc: &[F], words_of: &(dyn Fn(usize) -> &'a [u64] + Sync), cfg: &Config) -> Vec<F> {
    let zero = F::zero_with_cfg(cfg);
    let tables: Vec<[F; 256]> = cfg_into_iter!(0..len / 8)
        .map(|q| {
            let mut t = [zero; 256];
            for byte in 1..256usize {
                let low = byte & (byte - 1);
                t[byte] = cfg.add(&t[low], &eq_loc[8 * q + (byte ^ low).trailing_zeros() as usize]);
            }
            t
        })
        .collect();
    const SLICE_WORDS: usize = 16;
    let values: Vec<Vec<F>> = cfg_into_iter!(0..n.div_ceil(256))
        .map(|chunk| {
            let blocks = chunk * 256..((chunk + 1) * 256).min(n);
            let mut acc = vec![zero; blocks.len()];
            for slice in (0..len / 64).step_by(SLICE_WORDS) {
                for (a, i) in acc.iter_mut().zip(blocks.clone()) {
                    let row = words_of(i);
                    for w in slice..(slice + SLICE_WORDS).min(len / 64) {
                        let word = row[w];
                        if word == 0 {
                            continue;
                        }
                        for k in 0..8 {
                            let byte = ((word >> (8 * k)) & 0xFF) as usize;
                            if byte != 0 {
                                *a = cfg.add(a, &tables[8 * w + k][byte]);
                            }
                        }
                    }
                }
            }
            acc
        })
        .collect();
    values.into_iter().flatten().collect()
}

fn ceil_log2(n: usize) -> usize {
    if n <= 1 { 0 } else { (usize::BITS - (n - 1).leading_zeros()) as usize }
}

impl FlockRecRelation {
    /// `source_digest` must commit to everything the recording is a function
    /// of (the verified statement, its configuration, the public stream).
    /// `opening_row_vars`: rows of the opening grid (`None`: the block
    /// height). Taller columns mean fewer per-column folds in the proof; the
    /// profile narrows the prime to keep the fold below `2^128`.
    pub fn new(
        st: Arc<StructuredCircuit>,
        source_digest: [u8; 32],
        selection: LigeritoSelection,
        opening_row_vars: Option<usize>,
    ) -> Result<Self, ProtocolError> {
        Self::new_with_profile::<Lambda100>(st, source_digest, selection, opening_row_vars)
    }

    pub fn new_with_profile<P: IopSecurityProfile>(
        st: Arc<StructuredCircuit>,
        source_digest: [u8; 32],
        selection: LigeritoSelection,
        opening_row_vars: Option<usize>,
    ) -> Result<Self, ProtocolError> {
        Self::new_with_opener::<P>(st, source_digest, selection, opening_row_vars, FlockRecOpener::Standard)
    }

    /// [`FlockRecRelation::new_with_profile`] with the terminal claim opened
    /// by `opener`.
    pub fn new_with_opener<P: IopSecurityProfile>(
        st: Arc<StructuredCircuit>,
        source_digest: [u8; 32],
        selection: LigeritoSelection,
        opening_row_vars: Option<usize>,
        opener: FlockRecOpener,
    ) -> Result<Self, ProtocolError> {
        let geometry = |msg: &str| ProtocolError::LigeritoConfig(format!("flockrec geometry: {msg}"));
        if st.templates.len() != 1 {
            return Err(geometry("exactly one block template"));
        }
        let template = BlockTemplate::clone(&st.templates[0]);
        let ft = ceil_log2(template.hints).max(MIN_F_ROW_VARS);
        let rt = ceil_log2(template.rows()).max(1);
        let n = st.instances.len();
        let regions = Regions::for_cells(template.cells(), n);
        // The committed grid, boxed when the band fits.
        let hints = template.hints;
        let f_vars = ceil_log2((n << ft) + 1 + st.tail_hints.len()).max(ft + 1);
        let band = Band::fit(hints, external_hints_start(&template), ft, n, st.tail_hints.len(), f_vars);
        let f_const = band.map_or(n << ft, |b| b.tail_col << ft);
        let f_row = |h: usize| band.map_or(h, |b| b.row(h));
        // Global committed → f, a chunk of global indices at a time: blocks'
        // hints are contiguous runs (instances in recording order), the tail's
        // hints ascending.
        let mut global_f = vec![u32::MAX; st.committed];
        const CHUNK: usize = 1 << 16;
        let fill = |(c, out): (usize, &mut [u32])| -> bool {
            let start = c * CHUNK;
            let mut i = st.instances.partition_point(|inst| inst.hint_base as usize + hints <= start);
            let mut k = st.tail_hints.partition_point(|&g| (g as usize) < start);
            for (offset, slot) in out.iter_mut().enumerate() {
                let g = start + offset;
                while i < n && st.instances[i].hint_base as usize + hints <= g {
                    i += 1;
                }
                if i < n && st.instances[i].hint_base as usize <= g {
                    *slot = ((i << ft) + f_row(g - st.instances[i].hint_base as usize)) as u32;
                } else if k < st.tail_hints.len() && st.tail_hints[k] as usize == g {
                    *slot = (f_const + 1 + k) as u32;
                    k += 1;
                }
            }
            out.iter().all(|&f| f != u32::MAX)
        };
        #[cfg(feature = "parallel")]
        let covered = global_f.par_chunks_mut(CHUNK).enumerate().map(fill).reduce(|| true, |a, b| a && b);
        #[cfg(not(feature = "parallel"))]
        let covered = global_f.chunks_mut(CHUNK).enumerate().map(fill).fold(true, |a, b| a && b);
        if !covered {
            return Err(geometry("a committed bit is neither a block's nor the tail's"));
        }
        let f_layout = IntegerMatrixLayout { row_vars: ft, col_vars: f_vars - ft };
        // Derived grid.
        let h_tail = regions.end(n);
        let h_live = h_tail + 1 + st.tail_cells.len();
        // The logarithmic opening pays nothing per column: the reference
        // split `t = ⌈0.6 n⌉ − 1` of the derived grid.
        let logarithmic = matches!(opener, FlockRecOpener::Logarithmic { .. });
        let reference = crate::bitz::params::reference_log_rows(ceil_log2(h_live).max(regions.ha + 1)).max(regions.ha);
        let hr = opening_row_vars.unwrap_or(if logarithmic { reference } else { regions.ha });
        if hr < regions.ha {
            return Err(geometry("opening columns lower than a block's region"));
        }
        let h_vars = ceil_log2(h_live).max(hr + 1);
        let h_layout = IntegerMatrixLayout { row_vars: hr, col_vars: h_vars - hr };
        // Row batching grid.
        let rows_live = (n << rt) + st.tail_rows.len();
        let inst_vars = ceil_log2(rows_live.div_ceil(1 << rt)).max(1);

        let map_f = |b: &circuit::compact::CBit| -> Vec<u32> {
            let mut v: Vec<u32> = b.terms.iter().map(|&g| global_f[g as usize]).collect();
            v.sort_unstable();
            v
        };
        // The wiring, a chunk of instances at a time: each input's support
        // mapped to f and sorted in place.
        let wire = |chunk: &[circuit::compact::BlockInstance]| -> Result<(Vec<u32>, Vec<u32>, Vec<bool>), ProtocolError> {
            let mut terms = Vec::new();
            let mut lens = Vec::with_capacity(chunk.len() * template.slots);
            let mut consts = Vec::with_capacity(chunk.len() * template.slots);
            for inst in chunk {
                if inst.template != 0 || inst.inputs.len() != template.slots {
                    return Err(geometry("instance shape"));
                }
                for input in &inst.inputs {
                    if input.terms.iter().any(|&t| t >= LAZY) {
                        return Err(geometry("a block input reads a lazy output"));
                    }
                    let at = terms.len();
                    terms.extend(input.terms.iter().map(|&g| global_f[g as usize]));
                    terms[at..].sort_unstable();
                    lens.push((terms.len() - at) as u32);
                    consts.push(input.constant);
                }
            }
            Ok((terms, lens, consts))
        };
        #[cfg(feature = "parallel")]
        let parts: Vec<_> = st.instances.par_chunks(64).map(wire).collect::<Result<_, _>>()?;
        #[cfg(not(feature = "parallel"))]
        let parts: Vec<_> = st.instances.chunks(64).map(wire).collect::<Result<_, _>>()?;
        let mut wiring = Csr { offsets: Vec::with_capacity(n * template.slots + 1), terms: Vec::with_capacity(parts.iter().map(|p| p.0.len()).sum()) };
        let mut wiring_const = Vec::with_capacity(n * template.slots);
        wiring.offsets.push(0);
        for (terms, lens, consts) in parts {
            let mut end = wiring.terms.len() as u64;
            wiring.offsets.extend(lens.iter().map(|&l| {
                end += u64::from(l);
                end
            }));
            wiring.terms.extend(terms);
            wiring_const.extend(consts);
        }
        let mut tail = Csr { offsets: vec![0], terms: Vec::new() };
        let mut tail_lazy = Csr { offsets: vec![0], terms: Vec::new() };
        for k in 0..st.tail_cells.len() {
            let (s, e) = (st.tail_m_offsets[k] as usize, st.tail_m_offsets[k + 1] as usize);
            let mut v: Vec<u32> = Vec::with_capacity(e - s);
            for &g in &st.tail_m_terms[s..e] {
                if (LAZY..PLACEHOLDER).contains(&g) {
                    tail_lazy.terms.push(g - LAZY);
                } else if g >= PLACEHOLDER {
                    return Err(geometry("a tail cell reads a block slot"));
                } else {
                    v.push(global_f[g as usize]);
                }
            }
            v.sort_unstable();
            tail.terms.extend(v);
            tail.offsets.push(tail.terms.len() as u64);
            tail_lazy.offsets.push(tail_lazy.terms.len() as u64);
        }
        let mut groups = Vec::with_capacity(st.groups.len());
        for group in &st.groups {
            let mut terms = Vec::with_capacity(group.terms.len());
            for (columns, inputs) in &group.terms {
                let mut mapped = Vec::with_capacity(128);
                for b in inputs {
                    if b.terms.iter().any(|&t| t >= LAZY) {
                        return Err(geometry("a lazy group reads another lazy output or a slot"));
                    }
                    mapped.push((map_f(b), b.constant));
                }
                terms.push((*columns, mapped));
            }
            if group.constant != 0 {
                return Err(geometry("lazy group constants live in the reading cells"));
            }
            groups.push(Group { terms });
        }
        let columns = TemplateColumns::new(&template);
        let template_const_cells: Vec<u32> = (0..template.cells())
            .filter(|&l| template.m_const[l])
            .map(|l| l as u32)
            .collect();
        let nnz = n * template.m_nnz() + tail.terms.len() + st.group_m_nnz() + 1;
        let mut hasher = blake3::Hasher::new();
        hasher.update(b"bitz/flockrec/map/v1");
        hasher.update(&source_digest);
        for v in [n, hr, template.hints, template.slots, template.cells(), template.rows(), f_vars, h_vars, st.tail_hints.len(), st.tail_cells.len(), st.tail_rows.len()] {
            hasher.update(&(v as u64).to_le_bytes());
        }
        // The split layout (absent from v1 maps, which have none).
        if let Some(hb) = regions.hb {
            hasher.update(b"regions");
            hasher.update(&(regions.ha as u64).to_le_bytes());
            hasher.update(&(hb as u64).to_le_bytes());
        }
        // The boxed committed layout (likewise).
        if let Some(b) = band {
            hasher.update(b"band");
            for v in [b.local_start, b.rows_start, b.log, b.tail_col, b.tail_log] {
                hasher.update(&(v as u64).to_le_bytes());
            }
        }
        let digest = *hasher.finalize().as_bytes();
        let map = FlockRecMap {
            ft,
            regions,
            hr,
            template,
            columns,
            n,
            band,
            template_const_cells,
            wiring,
            wiring_const,
            tail,
            tail_lazy,
            tail_const: st.tail_m_const.clone(),
            groups,
            expanded_tail: std::sync::OnceLock::new(),
            f_cells: 1 << f_vars,
            h_cells: 1 << h_vars,
            f_const,
            h_tail,
            nnz,
            digest,
        };
        let tail_rows_local = st.tail_cells.clone();

        // The logarithmic opening commits each fold in 127 live bit slots:
        // `(q − 1)·2^t < 2^127`, one more lift bit than the standard fold.
        let lift = hr + usize::from(logarithmic);
        let mut security = P::instantiate(&instance_facts((rt + inst_vars) as u32, hr as u32, lift as u32))?;
        let ligerito = selection
            .resolve(f_vars - LOG_PACKING, security.ligerito_target_bits)
            .map_err(ProtocolError::LigeritoConfig)?;
        security.adopt_ood_round(ligerito.ood_bits())?;
        // A GF(2^256) ladder (`ext:`) draws its Round-0 point there too.
        if let Some(params) = security.ood.as_mut() {
            params.extension = ligerito.extension();
        }
        let geom = crate::bitz::grinding::Geometry::opening(&h_layout, &f_layout, false);
        security.adopt_native_opening(geom)?;
        let log = match opener {
            FlockRecOpener::Standard => None,
            FlockRecOpener::Logarithmic { mu } => Some(
                crate::bitz::logarithmic::LogOpener::new(
                    &opening_shape(&h_layout)?,
                    mu,
                    None,
                    None,
                    security.ligerito_target_bits,
                    security.native_policy()?,
                )
                .map_err(|e| ProtocolError::LigeritoConfig(format!("flockrec log opener: {e:?}")))?,
            ),
        };
        Ok(Self {
            st,
            map,
            n,
            rt,
            inst_vars,
            f_layout,
            h_layout,
            global_f,
            tail_rows_local,
            security,
            ligerito,
            source_digest,
            log,
        })
    }

    pub fn security(&self) -> &IopSecurityParams {
        &self.security
    }

    pub fn ligerito(&self) -> &ResolvedLigerito {
        &self.ligerito
    }

    pub fn committed_layout(&self) -> &IntegerMatrixLayout {
        &self.f_layout
    }

    pub fn derived_layout(&self) -> &IntegerMatrixLayout {
        &self.h_layout
    }

    pub fn map(&self) -> &FlockRecMap {
        &self.map
    }

    /// The prover's committed and derived rows from the witness backend's
    /// `w` (committed bits, recording order) and `M·w` (entry 0 the constant).
    pub fn witness(&self, g: &PackedWitness, h: &PackedWitness) -> Result<FlockRecWitness, ProtocolError> {
        let st = &self.st;
        let t = &self.map.template;
        if g.bit_len() != st.committed || h.bit_len() != st.cells + 1 {
            return Err(ProtocolError::InvalidGeometry);
        }
        let mut f_rows = vec![vec![0u64; self.f_layout.rows() / 64]; self.f_layout.cols()];
        let set = |rows: &mut Vec<Vec<u64>>, t_vars: usize, idx: usize| {
            let (c, b) = (idx >> t_vars, idx & ((1 << t_vars) - 1));
            rows[c][b / 64] |= 1u64 << (b % 64);
        };
        // Block i's committed bits are column i (its band word-aligned); each
        // region of its cells starts at a word-aligned derived offset (region
        // heights ≥ 7). All are bit ranges of the witness, copied a word at a
        // time.
        let band = self.map.band;
        let fill_f = |(i, column): (usize, &mut Vec<u64>)| {
            if let Some(inst) = st.instances.get(i) {
                Band::copy_block(band, g.words(), inst.hint_base as usize, t.hints, column);
            }
        };
        #[cfg(feature = "parallel")]
        f_rows.par_iter_mut().enumerate().for_each(fill_f);
        #[cfg(not(feature = "parallel"))]
        f_rows.iter_mut().enumerate().for_each(fill_f);
        let f_const = self.map.f_const;
        let ft = self.map.ft;
        set(&mut f_rows, ft, f_const);
        for (k, &gidx) in st.tail_hints.iter().enumerate() {
            if g.bit(gidx as usize) {
                set(&mut f_rows, ft, f_const + 1 + k);
            }
        }
        let h_rows = self.derived_rows(h);
        Ok(FlockRecWitness { f_rows, h_rows })
    }

    /// The derived rows from `M·w` (entry 0 the constant).
    fn derived_rows(&self, h: &PackedWitness) -> Vec<Vec<u64>> {
        let st = &self.st;
        let t = &self.map.template;
        let mut h_rows = vec![vec![0u64; self.h_layout.rows() / 64]; self.h_layout.cols()];
        let set = |rows: &mut Vec<Vec<u64>>, t_vars: usize, idx: usize| {
            let (c, b) = (idx >> t_vars, idx & ((1 << t_vars) - 1));
            rows[c][b / 64] |= 1u64 << (b % 64);
        };
        let (reg, hr) = (self.map.regions, self.map.hr);
        let la = 1usize << reg.ha;
        let h_words = || h.words();
        let fill_h = |(c, column): (usize, &mut Vec<u64>)| {
            // A region of 2^rv cells per block holds block k − base at slot
            // k mod 2^{hr−rv} of column k / 2^{hr−rv}.
            let mut region = |rv: usize, base: usize, first: usize, len: usize| {
                let per = 1usize << (hr - rv);
                for k in (c * per).max(base)..((c + 1) * per).min(base + st.instances.len()) {
                    let inst = &st.instances[k - base];
                    let at = ((k & (per - 1)) << rv) / 64;
                    copy_bits(h_words(), 1 + inst.cell_base as usize + first, len, &mut column[at..]);
                }
            };
            region(reg.ha, 0, 0, t.cells().min(la));
            if let Some(hb) = reg.hb {
                region(hb, reg.b0 >> hb, la, t.cells() - la);
            }
        };
        #[cfg(feature = "parallel")]
        h_rows.par_iter_mut().enumerate().for_each(fill_h);
        #[cfg(not(feature = "parallel"))]
        h_rows.iter_mut().enumerate().for_each(fill_h);
        let h_tail = self.map.h_tail;
        set(&mut h_rows, hr, h_tail);
        for (k, &cidx) in st.tail_cells.iter().enumerate() {
            if h.bit(1 + cidx as usize) {
                set(&mut h_rows, hr, h_tail + 1 + k);
            }
        }
        h_rows
    }

    /// The committed rows and configuration [`FlockRecRelation::new`] would
    /// lay out for this witness, from the witness pass's block spans alone
    /// (`blocks`, in recording order): block `i`'s bits are column `i`, then
    /// the constant one, then every committed bit outside the blocks in
    /// order. A prover can commit with them before the recording ends; the
    /// relation then checks [`FlockRecRelation::matches_early`].
    /// `external_start`: the block template's [`external_hints_start`] (the
    /// boxed layout), `None` for the plain one.
    pub fn early_committed(
        g: &PackedWitness,
        blocks: &[circuit::witgen::BlockSpan],
        selection: LigeritoSelection,
        external_start: Option<usize>,
    ) -> Result<EarlyCommitted, ProtocolError> {
        Self::early_committed_with_profile::<Lambda100>(g, blocks, selection, external_start)
    }

    pub fn early_committed_with_profile<P: IopSecurityProfile>(
        g: &PackedWitness,
        blocks: &[circuit::witgen::BlockSpan],
        selection: LigeritoSelection,
        external_start: Option<usize>,
    ) -> Result<EarlyCommitted, ProtocolError> {
        let geometry = |msg: &str| ProtocolError::LigeritoConfig(format!("flockrec early commitment: {msg}"));
        let hints = blocks.first().ok_or_else(|| geometry("no blocks"))?.hints;
        if blocks.iter().any(|b| b.hints != hints) || blocks.windows(2).any(|w| w[0].hint_base + hints > w[1].hint_base) {
            return Err(geometry("blocks of one shape, in order"));
        }
        let n = blocks.len();
        let ft = ceil_log2(hints).max(MIN_F_ROW_VARS);
        let tail = g.bit_len() - n * hints;
        let f_vars = ceil_log2((n << ft) + 1 + tail).max(ft + 1);
        let band = external_start.and_then(|start| Band::fit(hints, start, ft, n, tail, f_vars));
        let f_const = band.map_or(n << ft, |b| b.tail_col << ft);
        let layout = IntegerMatrixLayout { row_vars: ft, col_vars: f_vars - ft };
        let mut rows = vec![vec![0u64; layout.rows() / 64]; layout.cols()];
        let fill = |(i, column): (usize, &mut Vec<u64>)| {
            if let Some(b) = blocks.get(i) {
                Band::copy_block(band, g.words(), b.hint_base, hints, column);
            }
        };
        #[cfg(feature = "parallel")]
        rows.par_iter_mut().enumerate().for_each(fill);
        #[cfg(not(feature = "parallel"))]
        rows.iter_mut().enumerate().for_each(fill);
        let mut set = |idx: usize| rows[idx >> ft][(idx & ((1 << ft) - 1)) / 64] |= 1u64 << (idx % 64);
        set(f_const);
        // The tail: the gaps between blocks, in order.
        let mut k = 0;
        let mut at = 0;
        for gap_end in blocks.iter().map(|b| b.hint_base).chain([g.bit_len()]) {
            for j in at..gap_end {
                if g.bit(j) {
                    set(f_const + 1 + k);
                }
                k += 1;
            }
            at = gap_end + hints;
        }
        let ligerito = selection.resolve(f_vars - LOG_PACKING, P::LIGERITO_TARGET_BITS).map_err(ProtocolError::LigeritoConfig)?;
        Ok(EarlyCommitted { layout, band, rows, ligerito })
    }

    /// Whether `early` is this relation's committed layout and configuration.
    pub fn matches_early(&self, early: &EarlyCommitted) -> bool {
        early.layout == self.f_layout && early.band == self.map.band && early.ligerito.digest() == self.ligerito.digest()
    }

    /// [`FlockRecRelation::witness`] with the committed rows already formed
    /// ([`FlockRecRelation::early_committed`]): only the derived rows.
    pub fn witness_with_committed(&self, f_rows: Vec<Vec<u64>>, h: &PackedWitness) -> Result<FlockRecWitness, ProtocolError> {
        let st = &self.st;
        if h.bit_len() != st.cells + 1 || f_rows.len() != self.f_layout.cols() {
            return Err(ProtocolError::InvalidGeometry);
        }
        let h_rows = self.derived_rows(h);
        Ok(FlockRecWitness { f_rows, h_rows })
    }

    pub fn commit(&self, witness: &FlockRecWitness) -> FlockCommitHint {
        commit_rs_ligerito_rows(&self.f_layout, witness.f_rows.clone(), self.ligerito.prover())
    }

    pub(crate) fn statement_frames(&self, commitment: &Commitment) -> Result<Vec<(&'static [u8], Vec<u8>)>, ProtocolError> {
        let mut frames = vec![
            (b"protocol".as_slice(), PROTOCOL.to_vec()),
            (b"relation".as_slice(), self.map.digest.to_vec()),
            (b"source".as_slice(), self.source_digest.to_vec()),
            (b"profile".as_slice(), self.security.profile_name.as_bytes().to_vec()),
            (
                b"commitment".as_slice(),
                bincode::serialize(commitment).map_err(|e| ProtocolError::LigeritoConfig(e.to_string()))?,
            ),
        ];
        // The logarithmic opening's geometry and fold-commitment ladder
        // (absent for the standard opening, whose frames are unchanged).
        if let Some(log) = &self.log {
            frames.push((b"opener".as_slice(), [LOG_OPENER, log.frame().as_slice()].concat()));
        }
        Ok(frames)
    }

    /// `c` in the runtime field, powers of two from `pow2` (`2^0..2^{len−1}`).
    fn coeff(c: Coeff, pow2: &[F], cfg: &Config) -> F {
        match c {
            Coeff::Pow2 { neg, shift } => {
                let v = match pow2.get(shift as usize) {
                    Some(v) => *v,
                    None => {
                        let mut v = pow2[pow2.len() - 1];
                        for _ in pow2.len() - 1..shift as usize {
                            v = cfg.add(&v, &v);
                        }
                        v
                    }
                };
                if neg { cfg.neg(&v) } else { v }
            }
            Coeff::Small(x) => {
                let v = F::from_with_cfg(x.unsigned_abs(), cfg);
                if x < 0 { cfg.neg(&v) } else { v }
            }
        }
    }

    fn powers_of_two(cfg: &Config) -> Vec<F> {
        let mut out = Vec::with_capacity(129);
        let mut v = F::one_with_cfg(cfg);
        for _ in 0..129 {
            out.push(v);
            v = cfg.add(&v, &v);
        }
        out
    }

    /// The batched coefficients: `β` over template cells (and its constant
    /// column), the tail cells' `V`, and the constant cell's `V`.
    pub(crate) fn batching(&self, xi: &[F], eta: &[F], omega: F, cfg: &Config) -> Result<Batching, ProtocolError> {
        let eq_xi = eq_table(xi, cfg).map_err(super::SpartanError::from)?;
        let eq_eta = eq_table(eta, cfg).map_err(super::SpartanError::from)?;
        let t = &self.map.template;
        let zero = F::zero_with_cfg(cfg);
        let pow2 = Self::powers_of_two(cfg);
        let reg = self.map.regions;
        let mut beta = vec![zero; (1usize << reg.ha) + reg.hb.map_or(0, |hb| 1 << hb)];
        let mut beta_const = zero;
        for r in 0..t.rows() {
            let w = eq_xi[r];
            for (col, c) in t.c.row(r) {
                let v = cfg.mul(&w, &Self::coeff(c, &pow2, cfg));
                if col == 0 {
                    beta_const = cfg.add(&beta_const, &v);
                } else {
                    beta[col as usize - 1] = cfg.add(&beta[col as usize - 1], &v);
                }
            }
            debug_assert!(t.a.row(r).next().is_none() && t.b.row(r).next().is_none());
        }
        let eta_sum = eq_eta[..self.n].iter().fold(zero, |acc, w| cfg.add(&acc, w));
        let mut v_one = cfg.add(&omega, &cfg.mul(&beta_const, &eta_sum));
        let st = &self.st;
        let mut v_tail = vec![zero; st.tail_cells.len()];
        let rows = &st.tail_c;
        for k in 0..st.tail_rows.len() {
            let pos = (self.n << self.rt) + k;
            let w = cfg.mul(&eq_xi[pos & ((1 << self.rt) - 1)], &eq_eta[pos >> self.rt]);
            for (col, c) in rows.row(k) {
                let v = cfg.mul(&w, &Self::coeff(c, &pow2, cfg));
                if col == 0 {
                    v_one = cfg.add(&v_one, &v);
                } else {
                    let cell = col - 1;
                    let pos = self
                        .tail_rows_local
                        .binary_search(&cell)
                        .map_err(|_| ProtocolError::InvalidGeometry)?;
                    v_tail[pos] = cfg.add(&v_tail[pos], &v);
                }
            }
            if st.tail_a.row(k).next().is_some() || st.tail_b.row(k).next().is_some() {
                return Err(ProtocolError::InvalidGeometry);
            }
        }
        Ok(Batching { beta, eq_eta, v_one, v_tail })
    }

    /// `MLE(V)(r)` from the batching, in `O(2^{ha} + 2^{hb} + 2^{s} + tail)`:
    /// each region is a tensor of the instance weights and its share of `β`.
    pub(crate) fn evaluate(&self, b: &Batching, point: &[F], cfg: &Config) -> Result<F, ProtocolError> {
        let reg = self.map.regions;
        let ha = reg.ha;
        let la = 1usize << ha;
        let zero = F::zero_with_cfg(cfg);
        let dot = |x: &[F], y: &[F]| x.iter().zip(y).fold(zero, |acc, (x, y)| cfg.add(&acc, &cfg.mul(x, y)));
        let low = eq_table(&point[..ha], cfg).map_err(super::SpartanError::from)?;
        let high = eq_table(&point[ha..], cfg).map_err(super::SpartanError::from)?;
        let eq_at = |d: usize| cfg.mul(&low[d & (la - 1)], &high[d >> ha]);
        let mut v = cfg.mul(&dot(&b.beta[..la.min(b.beta.len())], &low), &dot(&b.eq_eta[..self.n], &high));
        if let Some(hb) = reg.hb {
            let lb = 1usize << hb;
            let low_b = eq_table(&point[..hb], cfg).map_err(super::SpartanError::from)?;
            let high_b = eq_table(&point[hb..], cfg).map_err(super::SpartanError::from)?;
            let base = reg.b0 >> hb;
            let beta_b = dot(&b.beta[la..la + lb], &low_b);
            v = cfg.add(&v, &cfg.mul(&beta_b, &dot(&b.eq_eta[..self.n], &high_b[base..base + self.n])));
        }
        v = cfg.add(&v, &cfg.mul(&b.v_one, &eq_at(self.map.h_tail)));
        for (k, vt) in b.v_tail.iter().enumerate() {
            v = cfg.add(&v, &cfg.mul(vt, &eq_at(self.map.h_tail + 1 + k)));
        }
        Ok(v)
    }
}

pub(crate) struct Batching {
    beta: Vec<F>,
    eq_eta: Vec<F>,
    v_one: F,
    v_tail: Vec<F>,
}

/// A relation's committed side formed from the witness pass
/// ([`FlockRecRelation::early_committed`]).
pub struct EarlyCommitted {
    pub layout: IntegerMatrixLayout,
    pub band: Option<Band>,
    pub rows: Vec<Vec<u64>>,
    pub ligerito: ResolvedLigerito,
}

impl EarlyCommitted {
    /// The commitment to the rows.
    pub fn commit(&self) -> FlockCommitHint {
        commit_rs_ligerito_rows(&self.layout, self.rows.clone(), self.ligerito.prover())
    }
}

/// The prover's rows.
pub struct FlockRecWitness {
    pub f_rows: Vec<Vec<u64>>,
    pub h_rows: Vec<Vec<u64>>,
}

/// A proof: the boundary nonces, the inner sumcheck and the BitZ opening.
#[derive(Clone)]
pub struct FlockRecProof {
    pub initial_nonce: u64,
    pub terminal_nonce: u64,
    pub inner: SumcheckProof<F, 3>,
    pub inner_nonces: Vec<u64>,
    pub bitz: BitZOpeningProof,
}

impl FlockRecProof {
    /// Bytes outside the BitZ opening.
    pub fn piop_bytes(&self) -> usize {
        2 * 8 + 3 * 16 * self.inner.round_polynomials.len() + 8 * self.inner_nonces.len()
    }
}

fn trace(label: &str, started: std::time::Instant) {
    if std::env::var_os("BITZ_TRACE").is_some() {
        eprintln!("flockrec: {label:<24} {:>9.1?}", started.elapsed());
    }
}

pub(crate) fn challenge_point<T: Transcript>(t: &mut T, domain: &[u8], vars: usize, cfg: &Config) -> Result<Vec<F>, ProtocolError> {
    absorb_spartan_message(t, b"challenge-domain", domain);
    (0..vars).map(|_| squeeze_field(t, cfg).map_err(ProtocolError::from)).collect()
}

fn canonical(x: &F, cfg: &Config) -> u128 {
    let words = cfg.to_integer(x);
    let w: &Uint<2> = &words;
    u128::from(w.as_words()[0]) | (u128::from(w.as_words()[1]) << 64)
}

pub(crate) fn opening_claim(
    rel: &FlockRecRelation,
    point: &[F],
    v_at: &F,
    final_claim: &F,
    cfg: &Config,
) -> Result<(Vec<u128>, Vec<u128>, u128), ProtocolError> {
    let hr = rel.map.hr;
    let low = eq_table(&point[..hr], cfg).map_err(super::SpartanError::from)?;
    let high = eq_table(&point[hr..], cfg).map_err(super::SpartanError::from)?;
    let rows = low.iter().map(|w| canonical(&cfg.mul(w, v_at), cfg)).collect();
    let cols = high.iter().map(|w| canonical(w, cfg)).collect();
    Ok((rows, cols, canonical(final_claim, cfg)))
}

pub(crate) fn claim_digest(rel: &FlockRecRelation, xi: &[F], eta: &[F], omega: &F, point: &[F], v_at: &F, claim: &(Vec<u128>, Vec<u128>, u128), cfg: &Config) -> [u8; 32] {
    let mut h = blake3::Hasher::new();
    h.update(DOMAIN_CLAIM);
    h.update(&rel.map.digest);
    for p in [xi, eta, point] {
        h.update(&(p.len() as u64).to_le_bytes());
        for x in p {
            h.update(&canonical(x, cfg).to_le_bytes());
        }
    }
    h.update(&canonical(omega, cfg).to_le_bytes());
    h.update(&canonical(v_at, cfg).to_le_bytes());
    #[cfg(not(feature = "sha256-fs"))]
    h.update(&claim.2.to_le_bytes());
    #[cfg(feature = "sha256-fs")]
    let _ = claim;
    *h.finalize().as_bytes()
}

/// Binds the opening claim. Under `sha256-fs` the digest covers the public
/// fields only and the (proof-derived) target is absorbed in the clear, so a
/// verifier circuit absorbs it with the transcript's own hash.
#[allow(clippy::too_many_arguments)]
pub(crate) fn bind_opening_claim<T: Transcript>(
    transcript: &mut T,
    rel: &FlockRecRelation,
    xi: &[F],
    eta: &[F],
    omega: &F,
    point: &[F],
    v_at: &F,
    claim: &(Vec<u128>, Vec<u128>, u128),
    cfg: &Config,
) {
    let digest = claim_digest(rel, xi, eta, omega, point, v_at, claim, cfg);
    absorb_spartan_message(transcript, b"flockrec/opening-claim", &digest);
    #[cfg(feature = "sha256-fs")]
    absorb_spartan_message(transcript, OPENING_TARGET_TAG, &claim.2.to_le_bytes());
}

/// The tag of the opening target's message under `sha256-fs`.
pub(crate) const OPENING_TARGET_TAG: &[u8] = b"flockrec/opening-target";

impl FlockRecRelation {
    /// The inner sumcheck `Σ_d V(d)·h(d) = claim` for `V` the blocks'
    /// regions (each `eq(η)[..n] ⊗` its share of `β`) and the tail, run on the
    /// structure. Rounds `0..hb` bind the cells of both regions: each runs on
    /// its instance-combined table `G[l] = Σ_i eq(η)[i]·h(i, l)` against `β`
    /// (plus the tail's own small tables). Rounds `hb..ha` bind region B's
    /// low instance bits, on `h_B(i, r_loc)` against `eq(η)[i]·β_B(r_loc)`,
    /// while region A's cells continue on `G_A`. From round `ha` every block,
    /// region B's folded instances and the tail are one table of `2^{n−ha}`
    /// entries. Each round polynomial is the one the dense prover sends (the
    /// same sums, regrouped), so the transcript is the generic inner
    /// sumcheck's.
    #[allow(clippy::too_many_arguments)]
    fn structured_inner_sumcheck<T: Transcript>(
        &self,
        transcript: &mut T,
        b: &Batching,
        h_rows: &[Vec<u64>],
        claim: F,
        n_vars: usize,
        boundary: &mut ProverGrindingRoundBoundary<Sha256InnerGrinding>,
        cfg: &Config,
    ) -> Result<crate::sumcheck::inner::InnerSumcheckOutput<F>, ProtocolError> {
        use crate::sumcheck::RoundBoundaryPolicy;
        let (hr, n) = (self.map.hr, self.n);
        let reg = self.map.regions;
        let (ha, la) = (reg.ha, 1usize << reg.ha);
        let lb = reg.hb.map_or(0, |hb| 1usize << hb);
        let zero = F::zero_with_cfg(cfg);
        let one = F::one_with_cfg(cfg);
        let bit = |d: usize| -> bool {
            let (c, r) = (d >> hr, d & ((1 << hr) - 1));
            (h_rows[c][r / 64] >> (r % 64)) & 1 == 1
        };
        // A block region's `len` cells at derived offset `d`: words of one column.
        let words_at = |d: usize, len: usize| -> &[u64] {
            let (c, r) = (d >> hr, d & ((1 << hr) - 1));
            &h_rows[c][r / 64..r / 64 + len / 64]
        };
        let region_a = |i: usize| words_at(i << ha, la);
        let region_b = |i: usize| words_at(reg.b0 + (i << reg.hb.unwrap_or(0)), lb);
        let mut g_a = instance_combined(n, la, &b.eq_eta, &region_a, cfg);
        let mut beta_a = b.beta[..la].to_vec();
        let (mut g_b, mut beta_b) = match reg.hb {
            Some(_) => (instance_combined(n, lb, &b.eq_eta, &region_b, cfg), b.beta[la..la + lb].to_vec()),
            None => (Vec::new(), Vec::new()),
        };
        // Tail pseudo-instances: dense V and h tables per 2^ha chunk.
        let tail_len = 1 + b.v_tail.len();
        let tail_instances = tail_len.div_ceil(la);
        let tail_base = self.map.h_tail;
        let mut tail_v: Vec<Vec<F>> = (0..tail_instances).map(|_| vec![zero; la]).collect();
        let mut tail_h: Vec<Vec<F>> = (0..tail_instances).map(|_| vec![zero; la]).collect();
        for k in 0..tail_len {
            let value = if k == 0 { b.v_one } else { b.v_tail[k - 1] };
            tail_v[k / la][k % la] = value;
            if bit(tail_base + k) {
                tail_h[k / la][k % la] = one;
            }
        }
        let mut proof = SumcheckProof { round_polynomials: Vec::with_capacity(n_vars) };
        let mut point = Vec::with_capacity(n_vars);
        let mut claim = claim;
        let pair_sums = |v: &[F], h: &[F]| -> (F, F) {
            let mut c0 = zero;
            let mut c2 = zero;
            for p in 0..v.len() / 2 {
                let (v0, v1, h0, h1) = (v[2 * p], v[2 * p + 1], h[2 * p], h[2 * p + 1]);
                c0 = cfg.add(&c0, &cfg.mul(&v0, &h0));
                c2 = cfg.add(&c2, &cfg.mul(&cfg.sub(&v1, &v0), &cfg.sub(&h1, &h0)));
            }
            (c0, c2)
        };
        let fold = |t: &mut Vec<F>, r: &F| {
            let half = t.len() / 2;
            for p in 0..half {
                let (x0, x1) = (t[2 * p], t[2 * p + 1]);
                t[p] = cfg.add(&x0, &cfg.mul(r, &cfg.sub(&x1, &x0)));
            }
            t.truncate(half);
        };
        let mut round = |transcript: &mut T,
                         c0: F,
                         c2: F,
                         claim: &mut F,
                         point: &mut Vec<F>,
                         proof: &mut SumcheckProof<F, 3>,
                         index: usize|
         -> Result<F, ProtocolError> {
            let c1 = cfg.sub(&cfg.sub(claim, &cfg.add(&c0, &c0)), &c2);
            let message = [c0, c1, c2];
            super::absorb_field_elements(transcript, &message, cfg);
            proof.round_polynomials.push(message);
            boundary.after_round(transcript, index).map_err(super::SpartanError::from)?;
            let r = squeeze_field(transcript, cfg)?;
            *claim = cfg.add(&c0, &cfg.mul(&r, &cfg.add(&c1, &cfg.mul(&r, &c2))));
            point.push(r);
            Ok(r)
        };
        let add = |acc: &mut (F, F), (x, y): (F, F)| {
            acc.0 = cfg.add(&acc.0, &x);
            acc.1 = cfg.add(&acc.1, &y);
        };
        // Phase A: the regions' cells; region B's low instance bits once its
        // cells are bound (`inst_b`: its weights and values per instance).
        let mut inst_b: Option<(Vec<F>, Vec<F>)> = None;
        for j in 0..ha {
            if let Some(hb) = reg.hb
                && j == hb
            {
                let eq_loc = eq_table(&point, cfg).map_err(super::SpartanError::from)?;
                let values = region_values(n, lb, &eq_loc, &region_b, cfg);
                let padded = n.next_multiple_of(1 << (ha - hb));
                let mut weights = vec![zero; padded];
                let mut cells = vec![zero; padded];
                for i in 0..n {
                    weights[i] = cfg.mul(&b.eq_eta[i], &beta_b[0]);
                    cells[i] = values[i];
                }
                inst_b = Some((weights, cells));
            }
            let mut sums = pair_sums(&beta_a, &g_a);
            if !g_b.is_empty() && beta_b.len() > 1 {
                add(&mut sums, pair_sums(&beta_b, &g_b));
            }
            if let Some((w, v)) = &inst_b {
                add(&mut sums, pair_sums(w, v));
            }
            for (v, h) in tail_v.iter().zip(&tail_h) {
                add(&mut sums, pair_sums(v, h));
            }
            let r = round(transcript, sums.0, sums.1, &mut claim, &mut point, &mut proof, j)?;
            fold(&mut beta_a, &r);
            fold(&mut g_a, &r);
            if beta_b.len() > 1 {
                fold(&mut beta_b, &r);
                fold(&mut g_b, &r);
            }
            if let Some((w, v)) = &mut inst_b {
                fold(w, &r);
                fold(v, &r);
            }
            for (v, h) in tail_v.iter_mut().zip(tail_h.iter_mut()) {
                fold(v, &r);
                fold(h, &r);
            }
        }
        // Phase B: one table over `d >> ha` — blocks, region B, then the tail.
        let eq_loc = eq_table(&point, cfg).map_err(super::SpartanError::from)?;
        let instances = 1usize << (n_vars - ha);
        let mut weights = vec![zero; instances];
        let mut values = vec![zero; instances];
        let block_values = region_values(n, la, &eq_loc, &region_a, cfg);
        for i in 0..n {
            weights[i] = cfg.mul(&b.eq_eta[i], &beta_a[0]);
            values[i] = block_values[i];
        }
        let mut next = n;
        if let Some((w, v)) = inst_b {
            weights[n..n + w.len()].copy_from_slice(&w);
            values[n..n + v.len()].copy_from_slice(&v);
            next += w.len();
        }
        debug_assert_eq!(next, tail_base >> ha, "the tail follows the blocks' regions");
        for (k, (v, h)) in tail_v.iter().zip(&tail_h).enumerate() {
            weights[next + k] = v[0];
            values[next + k] = h[0];
        }
        for j in ha..n_vars {
            let (c0, c2) = pair_sums(&weights, &values);
            let r = round(transcript, c0, c2, &mut claim, &mut point, &mut proof, j)?;
            fold(&mut weights, &r);
            fold(&mut values, &r);
        }
        Ok(crate::sumcheck::inner::InnerSumcheckOutput {
            proof,
            point,
            final_claim: claim,
            terminal_evaluations: [weights[0], values[0]],
        })
    }

    pub fn prove<T: Transcript + Send>(
        &self,
        transcript: &mut T,
        witness: &FlockRecWitness,
        hint: &FlockCommitHint,
    ) -> Result<FlockRecProof, ProtocolError> {
        let security = &self.security;
        let started = std::time::Instant::now();
        for (tag, bytes) in self.statement_frames(&hint.commitment)? {
            absorb_spartan_message(transcript, tag, &bytes);
        }
        self.ligerito.bind(transcript);
        let ood = bind_prover_ood(transcript, hint, security.ood);
        trace("statement + round 0", started);
        let started = std::time::Instant::now();
        let initial_nonce = grind_boundary(transcript, DOMAIN_INITIAL, security.initial_grinding_bits)?;
        let prime = sample_mod_q(transcript, DOMAIN_PRIME, security.projection_min, security.projection_max)?;
        let cfg = &prime;
        absorb_spartan_message(transcript, b"runtime-field-modulus", &F::canonical_modulus_encoding(cfg));
        let xi = challenge_point(transcript, b"flockrec/local-rows", self.rt, cfg)?;
        let eta = challenge_point(transcript, b"flockrec/instances", self.inst_vars, cfg)?;
        let terminal_nonce = grind_boundary(transcript, DOMAIN_TERMINAL, security.terminal_grinding_bits)?;
        let omega = challenge_point(transcript, b"flockrec/constant-one", 1, cfg)?[0];
        let batching = self.batching(&xi, &eta, omega, cfg)?;
        trace("prime + batching", started);
        let started = std::time::Instant::now();

        // Inner sumcheck Σ_y V(y)·h(y) = ω over the derived grid: V is the
        // repeated block tensor `eq(η)[i]·β[l]` followed by the tail.
        let n_vars = self.h_layout.row_vars + self.h_layout.col_vars;
        let mut boundary =
            ProverGrindingRoundBoundary::<Sha256InnerGrinding>::with_round_offset(security.piop_round_grinding_bits, 0);
        let inner = self.structured_inner_sumcheck(transcript, &batching, &witness.h_rows, omega, n_vars, &mut boundary, cfg)?;
        let inner_nonces = boundary.into_nonces();
        if inner.final_claim != cfg.mul(&inner.terminal_evaluations[0], &inner.terminal_evaluations[1]) {
            return Err(ProtocolError::InvalidInnerTerminalClaim);
        }
        let v_at = inner.terminal_evaluations[0];
        let claim = opening_claim(self, &inner.point, &v_at, &inner.final_claim, cfg)?;
        bind_opening_claim(transcript, self, &xi, &eta, &omega, &inner.point, &v_at, &claim, cfg);
        trace("inner sumcheck", started);
        let started = std::time::Instant::now();

        let params = opening_params(&self.h_layout, prime.modulus_u128())?;
        let committed = opening_shape(&self.f_layout)?;
        let native_claim = LinearClaim::new(&params, claim.0, claim.1, claim.2)
            .map_err(|e| ProtocolError::LigeritoConfig(format!("BitZ claim: {e:?}")))?;
        let opening = VirtualStatement::new(params, committed, &self.map, &native_claim)
            .map_err(|e| ProtocolError::LigeritoConfig(format!("BitZ statement: {e:?}")))?;
        let pcs = pcs_from_config(&committed, self.ligerito.prover())?
            .with_native_policy(security.native_policy()?)
            .with_extension(self.ligerito.extension());
        let bitz = match &self.log {
            Some(log) => {
                let statement = self.log_statement(&inner.point, &v_at, &inner.final_claim, cfg);
                bitz_opener::prove_virtual_log_opening(transcript, log, &statement, &opening, &pcs, hint, &witness.h_rows, ood)?
            }
            None => bitz_opener::prove_virtual_opening(transcript, &opening, &pcs, hint, &witness.h_rows, ood)?,
        };
        trace("virtual opening", started);
        Ok(FlockRecProof { initial_nonce, terminal_nonce, inner: inner.proof, inner_nonces, bitz })
    }

    pub fn verify<T: Transcript + Send>(
        &self,
        transcript: &mut T,
        commitment: &Commitment,
        proof: &FlockRecProof,
    ) -> Result<(), ProtocolError> {
        let security = &self.security;
        let n_vars = self.h_layout.row_vars + self.h_layout.col_vars;
        if proof.inner.round_polynomials.len() != n_vars || commitment.params.m != self.f_layout.row_vars + self.f_layout.col_vars {
            return Err(ProtocolError::InvalidGeometry);
        }
        crate::ligerito_flock::validate_ligerito_commitment(commitment, self.ligerito.verifier())
            .map_err(ProtocolError::Bitz)?;
        for (tag, bytes) in self.statement_frames(commitment)? {
            absorb_spartan_message(transcript, tag, &bytes);
        }
        self.ligerito.bind(transcript);
        let ood = bind_verifier_ood(
            transcript,
            self.f_layout.row_vars + self.f_layout.col_vars - LOG_PACKING,
            security.ood,
            proof.bitz.ood.as_ref(),
        )
        .map_err(ProtocolError::Bitz)?;
        check_boundary(transcript, DOMAIN_INITIAL, security.initial_grinding_bits, proof.initial_nonce)?;
        let prime = sample_mod_q(transcript, DOMAIN_PRIME, security.projection_min, security.projection_max)?;
        let cfg = &prime;
        absorb_spartan_message(transcript, b"runtime-field-modulus", &F::canonical_modulus_encoding(cfg));
        let xi = challenge_point(transcript, b"flockrec/local-rows", self.rt, cfg)?;
        let eta = challenge_point(transcript, b"flockrec/instances", self.inst_vars, cfg)?;
        check_boundary(transcript, DOMAIN_TERMINAL, security.terminal_grinding_bits, proof.terminal_nonce)?;
        let omega = challenge_point(transcript, b"flockrec/constant-one", 1, cfg)?[0];
        let batching = self.batching(&xi, &eta, omega, cfg)?;
        let (point, final_claim) = proof
            .inner
            .verify_grinded::<Sha256InnerGrinding>(
                transcript,
                omega,
                n_vars,
                cfg,
                &proof.inner_nonces,
                security.piop_round_grinding_bits,
            )
            .map_err(super::SpartanError::from)?;
        let v_at = self.evaluate(&batching, &point, cfg)?;
        let claim = opening_claim(self, &point, &v_at, &final_claim, cfg)?;
        bind_opening_claim(transcript, self, &xi, &eta, &omega, &point, &v_at, &claim, cfg);
        let params = opening_params(&self.h_layout, prime.modulus_u128())?;
        let committed = opening_shape(&self.f_layout)?;
        let native_claim = LinearClaim::new(&params, claim.0, claim.1, claim.2)
            .map_err(|e| ProtocolError::LigeritoConfig(format!("BitZ claim: {e:?}")))?;
        let opening = VirtualStatement::new(params, committed, &self.map, &native_claim)
            .map_err(|e| ProtocolError::LigeritoConfig(format!("BitZ statement: {e:?}")))?;
        let pcs = pcs_from_config(&committed, self.ligerito.verifier())?
            .with_native_policy(security.native_policy()?)
            .with_extension(self.ligerito.extension());
        match &self.log {
            Some(log) => {
                let statement = self.log_statement(&point, &v_at, &final_claim, cfg);
                bitz_opener::verify_virtual_log_opening(transcript, log, &statement, &opening, &pcs, commitment, &proof.bitz, ood)
            }
            None => bitz_opener::verify_virtual_opening(transcript, &opening, &pcs, commitment, &proof.bitz, ood),
        }
    }

    /// The terminal claim by its point, for the logarithmic opening: `r₁`
    /// the derived grid's row coordinates, `r₂` its column coordinates (most
    /// significant first: [`crate::ligerito_flock::eq_table_mod_q`]'s order,
    /// the reverse of [`eq_table`]'s), the column weights scaled by `V(r)` —
    /// the claim [`opening_claim`] expands.
    fn log_statement(&self, point: &[F], v_at: &F, final_claim: &F, cfg: &Config) -> crate::bitz::logarithmic::LogStatement {
        let hr = self.map.hr;
        let coordinates = |part: &[F]| part.iter().rev().map(|x| canonical(x, cfg)).collect();
        crate::bitz::logarithmic::LogStatement {
            q: cfg.modulus_u128(),
            r1: coordinates(&point[..hr]),
            r2: coordinates(&point[hr..]),
            scale: canonical(v_at, cfg),
            target: canonical(final_claim, cfg),
        }
    }

    /// The derived rows recomputed from the committed rows through the map
    /// (`O(nnz)`, tests only).
    pub fn derive_rows(&self, f_rows: &[Vec<u64>]) -> Vec<Vec<u64>> {
        let mut h_rows = vec![vec![0u64; self.h_layout.rows() / 64]; self.h_layout.cols()];
        let ft = self.map.ft;
        let bit = |j: usize| (f_rows[j >> ft][(j & ((1 << ft) - 1)) / 64] >> (j % 64)) & 1 == 1;
        let t = &self.map.template;
        let hr = self.map.hr;
        let flip = |rows: &mut Vec<Vec<u64>>, d: usize| {
            let (c, b) = (d >> hr, d & ((1 << hr) - 1));
            rows[c][b / 64] ^= 1u64 << (b % 64);
        };
        for i in 0..self.n {
            let slot = |s: usize| -> bool {
                let row = i * t.slots + s;
                self.map.wiring.row(row).iter().fold(self.map.wiring_const[row], |acc, &j| acc ^ bit(j as usize))
            };
            let slots: Vec<bool> = (0..t.slots).map(slot).collect();
            for l in 0..t.cells() {
                let v = t.cell(l).iter().fold(t.m_const[l], |acc, &c| {
                    acc ^ if (c as usize) < t.hints { bit((i << ft) + self.map.f_row(c as usize)) } else { slots[c as usize - t.hints] }
                });
                if v {
                    flip(&mut h_rows, self.map.regions.cell(i, l));
                }
            }
        }
        if bit(self.map.f_const) {
            flip(&mut h_rows, self.map.h_tail);
        }
        let group_bits: Vec<u128> = self.map.groups.iter().map(|g| g.evaluate(&bit)).collect();
        for k in 0..self.map.tail_const.len() {
            let mut v = self.map.tail.row(k).iter().fold(self.map.tail_const[k], |acc, &j| acc ^ bit(j as usize));
            for &id in self.map.tail_lazy.row(k) {
                v ^= (group_bits[id as usize / 128] >> (id % 128)) & 1 == 1;
            }
            if v {
                flip(&mut h_rows, self.map.h_tail + 1 + k);
            }
        }
        h_rows
    }

    pub fn verifier_config(&self) -> &LigVerifierConfig {
        self.ligerito.verifier()
    }

    pub fn prover_config(&self) -> &LigProverConfig {
        self.ligerito.prover()
    }

}

#[cfg(test)]
mod tests {
    use super::*;
    use circuit::compact::CompactRecorder;
    use circuit::sha256::{self, Word};
    use circuit::witgen::CheckingWitgen;
    use circuit::{BoolRepresentation, Circuit, HintResult, PackedBits};

    /// A chain of `n` SHA-256 compression blocks over committed message
    /// words, with tail cells and rows reading block outputs (each output
    /// word lifted and checked against a committed copy).
    fn chain<CS: Circuit>(cs: &mut CS, n: usize) {
        let mut state = sha256::initial_state::<CS::Bool>();
        for k in 0..n {
            let words = cs.hint::<1, 512, 8, _>(move |_| {
                HintResult::Ok(PackedBits::from_fn(|i| (i * 7 + i / 3 + k) % 5 < 2))
            });
            let block: [Word<CS::Bool, 32>; 16] = std::array::from_fn(|w| Word::new(std::array::from_fn(|b| words.bit(32 * w + b))));
            state = sha256::compress_block(cs, &block, &state);
            // Tail: the low word lifted, a committed copy of it, their equality.
            let copy = {
                let bits: Vec<CS::Bool> = (0..32).map(|b| state[0].bit(b)).collect();
                let bits2 = bits.clone();
                let packed = cs.hint::<1, 32, 1, _>(move |ctx| {
                    HintResult::Ok(PackedBits::from_fn(|b| ctx.eval_bool(&bits2[b])))
                });
                (0..32).map(|b| packed.bit(b)).collect::<Vec<_>>()
            };
            let mut diff = CS::Z::<1>::from(CS::Coefficient::<1>::from(0));
            let mut power = CS::Coefficient::<1>::from(1);
            for b in 0..32 {
                let x = cs.bitz::<1>(state[0].bit(b));
                let y = cs.bitz::<1>(copy[b].clone());
                diff += (x - y) * power.clone();
                power += power.clone();
            }
            let zero = || CS::Z::<1>::from(CS::Coefficient::<1>::from(0));
            cs.assert_r1c::<1>(zero(), zero(), diff);
        }
    }

    fn relation(n: usize, t: usize) -> (FlockRecRelation, FlockRecWitness) {
        relation_with::<Lambda100>(n, Some(t), "custom:3:4", FlockRecOpener::Standard)
    }

    fn relation_with<P: IopSecurityProfile>(
        n: usize,
        t: Option<usize>,
        lig: &str,
        opener: FlockRecOpener,
    ) -> (FlockRecRelation, FlockRecWitness) {
        let mut w = CheckingWitgen::new();
        chain(&mut w, n);
        assert_eq!(w.first_failure, None);
        let mut rec = CompactRecorder::new();
        chain(&mut rec, n);
        let st = rec.finish_structured();
        assert_eq!(st.templates.len(), 1);
        let external_start = external_hints_start(&st.templates[0]);
        let selection = LigeritoSelection::parse(lig, P::LIGERITO_TARGET_BITS).unwrap();
        let rel = FlockRecRelation::new_with_opener::<P>(Arc::new(st), [7; 32], selection, t, opener).unwrap();
        let (g, h) = w.into_witnesses();
        let wit = rel.witness(&g, &h).unwrap();
        // The witness pass's own block spans give the same committed side.
        let mut fast = circuit::witgen::Witgen::new();
        chain(&mut fast, n);
        let early =
            FlockRecRelation::early_committed_with_profile::<P>(fast.witness(), fast.blocks(), selection, Some(external_start)).unwrap();
        assert!(rel.matches_early(&early));
        assert_eq!(early.rows, wit.f_rows, "early committed rows");
        (rel, wit)
    }

    /// The split layout (SHA-256's 20,456 cells as 2^14 + 2^12) and the boxed
    /// committed layout (`FLOCKREC_NO_BAND=1`: the plain one): the map derives
    /// the witness's own rows, its fast transposition is the generic
    /// column-by-column one and the structured one, and the relation proves
    /// and verifies.
    #[test]
    fn split_regions_match_the_map_and_prove() {
        let boxed = std::env::var_os("FLOCKREC_NO_BAND").is_none();
        // At least 2^20 committed cells for the Ligerito ladder.
        for (n, t) in [(130usize, 18usize), (131, 20)] {
            let (rel, wit) = relation(n, t);
            assert_eq!(rel.map.regions.hb, Some(12), "SHA-256 blocks split");
            assert_eq!(rel.map.band.is_some(), boxed, "SHA-256 blocks boxed: {:?}", rel.map.band);
            assert_eq!(rel.derive_rows(&wit.f_rows), wit.h_rows, "n {n}: h = M·f in the witness layout");
            // Transposed weights: fast path against column_rows.
            let mut state = 0x1234_5678_9abc_def1u64;
            let mut gf = || {
                state ^= state << 13;
                state ^= state >> 7;
                state ^= state << 17;
                Gf::new(state, state.rotate_left(29) ^ 0x5555)
            };
            let row_w: Vec<Gf> = (0..rel.h_layout.rows()).map(|_| gf()).collect();
            let col_w: Vec<Gf> = (0..rel.h_layout.cols()).map(|_| gf()).collect();
            let cells = rel.map.f_cells;
            let fast = rel.map.transpose_weights(&row_w, &col_w, rel.map.hr, cells).unwrap();
            let derived = row_w.len() * col_w.len();
            // The generic path costs O(map) a cell: a sample of every kind
            // (block hints of first, middle and last blocks, the constant,
            // the tail's committed bits, padding) and random cells.
            let ft = rel.map.ft;
            let hints = rel.map.template.hints;
            let mut sample: Vec<usize> = Vec::new();
            for i in [0, 1, n / 2, n - 1] {
                sample.extend((0..40).map(|h| (i << ft) + h));
                sample.extend((0..8).map(|h| (i << ft) + rel.map.f_row(hints - 1 - 3 * h)));
                // Around the band: the last local rows, the gap, its first rows.
                if let Some(b) = rel.map.band {
                    sample.extend((b.local_start - 4..b.local_start + 4).map(|r| (i << ft) + r));
                    sample.extend((b.rows_start - 4..b.rows_start + 8).map(|r| (i << ft) + r));
                }
            }
            sample.extend(rel.map.f_const..rel.map.f_const + 300);
            sample.extend((0..200).map(|_| gf().lo as usize % cells));
            for j in sample {
                let mut slow = Gf::ZERO;
                for d in rel.map.column_rows(j).unwrap() {
                    if d < derived {
                        slow += row_w[d & (row_w.len() - 1)] * col_w[d >> rel.map.hr];
                    }
                }
                assert_eq!(fast[j], slow, "n {n}: transposed weight {j}");
            }
            // The prover's structured weights (boxed layout only).
            match rel.map.structured_weights(&row_w, &col_w, rel.map.hr) {
                Some(sw) => assert!(sw.dense() == fast, "n {n}: structured weights"),
                None => assert!(!boxed, "n {n}: the boxed layout has structured weights"),
            }
            // The verifier's lazy extension against the dense weights (index
            // bit k against point[k]).
            let point: Vec<Gf> = (0..cells.ilog2()).map(|_| gf()).collect();
            let mut eq = vec![Gf::ONE];
            for &c in &point {
                let half = eq.len();
                for k in 0..half {
                    let high = eq[k] * c;
                    eq.push(high);
                    eq[k] += high;
                }
            }
            let dense = fast.iter().zip(&eq).fold(Gf::ZERO, |acc, (w, e)| acc + *w * *e);
            assert_eq!(rel.map.transposed_mle(&row_w, &col_w, rel.map.hr, &point), Some(dense), "n {n}: lazy transposed extension");
            let hint = rel.commit(&wit);
            let mut tp = crate::transcript::Sha256Transcript::new();
            let pi = rel.prove(&mut tp, &wit, &hint).unwrap();
            let mut tv = crate::transcript::Sha256Transcript::new();
            rel.verify(&mut tv, &hint.commitment, &pi).expect("the split relation verifies");
        }
    }

    /// The logarithmic opening at the reference split, at 100 bits (Johnson
    /// ladders, the fold commitment's Round 0) and 128 bits (unique
    /// decoding, every native challenge ground): the relation proves and
    /// verifies, and a flipped proof byte (narg or hint) is rejected.
    #[test]
    fn logarithmic_opening_proves_and_verifies() {
        use super::super::profile::Lambda128;
        fn run<P: IopSecurityProfile>(lig: &str) {
            let mu = LigeritoSelection::parse(lig, P::LIGERITO_TARGET_BITS).unwrap();
            let (rel, wit) = relation_with::<P>(131, None, lig, FlockRecOpener::Logarithmic { mu });
            let h_vars = rel.h_layout.row_vars + rel.h_layout.col_vars;
            let reference = crate::bitz::params::reference_log_rows(h_vars).max(rel.map.regions.ha);
            assert_eq!(rel.map.hr, reference, "{lig}: the reference split");
            assert_eq!(rel.derive_rows(&wit.f_rows), wit.h_rows, "{lig}: h = M·f");
            let hint = rel.commit(&wit);
            let mut tp = crate::transcript::Sha256Transcript::new();
            let pi = rel.prove(&mut tp, &wit, &hint).unwrap();
            let mut tv = crate::transcript::Sha256Transcript::new();
            rel.verify(&mut tv, &hint.commitment, &pi).expect("the logarithmic relation verifies");
            for hints in [false, true] {
                let mut bad = pi.clone();
                let bytes = if hints { &mut bad.bitz.transcript.hints } else { &mut bad.bitz.transcript.narg_string };
                let at = bytes.len() / 2;
                bytes[at] ^= 0x10;
                let mut tv = crate::transcript::Sha256Transcript::new();
                assert!(rel.verify(&mut tv, &hint.commitment, &bad).is_err(), "{lig}: tampered {}", if hints { "hint" } else { "narg" });
            }
        }
        run::<Lambda100>("custom:3:4");
        run::<Lambda128>("udrg:2:4");
    }

    /// Johnson over GF(2^256) (`ext:3:4`) at 128 bits with the standard
    /// opening: Round 0 runs over the extension, the proof survives its
    /// codec, verifies, and a flipped proof byte (narg or hint) or a
    /// tampered Round-0 value is rejected.
    #[test]
    fn extension_johnson_opening_proves_and_verifies() {
        use super::super::profile::Lambda128;
        let (rel, wit) = relation_with::<Lambda128>(131, Some(18), "ext:3:4", FlockRecOpener::Standard);
        assert!(rel.ligerito.extension(), "an ext: ladder opens over GF(2^256)");
        let hint = rel.commit(&wit);
        let mut tp = crate::transcript::Sha256Transcript::new();
        let pi = rel.prove(&mut tp, &wit, &hint).unwrap();
        let round = pi.bitz.ood.clone().expect("a Johnson ladder runs Round 0");
        assert!(round.y_ext.is_some(), "Round 0 over GF(2^256)");
        let decoded = BitZOpeningProof::from_bytes(&pi.bitz.to_bytes()).expect("the opening proof decodes");
        assert_eq!(decoded, pi.bitz, "codec roundtrip");
        let mut tv = crate::transcript::Sha256Transcript::new();
        rel.verify(&mut tv, &hint.commitment, &pi).expect("the GF(2^256) relation verifies");
        for hints in [false, true] {
            let mut bad = pi.clone();
            let bytes = if hints { &mut bad.bitz.transcript.hints } else { &mut bad.bitz.transcript.narg_string };
            let at = bytes.len() / 2;
            bytes[at] ^= 0x10;
            let mut tv = crate::transcript::Sha256Transcript::new();
            assert!(rel.verify(&mut tv, &hint.commitment, &bad).is_err(), "tampered {}", if hints { "hint" } else { "narg" });
        }
        let mut bad = pi.clone();
        if let Some(round) = bad.bitz.ood.as_mut() {
            round.y_ext = round.y_ext.map(|y| y + Gf::ONE);
        }
        let mut tv = crate::transcript::Sha256Transcript::new();
        assert!(rel.verify(&mut tv, &hint.commitment, &bad).is_err(), "tampered Round-0 value");
    }

    #[test]
    fn transpose_8x64_matches_the_definition() {
        let mut state = 0x9e37_79b9_7f4a_7c15u64;
        for _ in 0..64 {
            let rows: [u64; 8] = std::array::from_fn(|_| {
                state ^= state << 13;
                state ^= state >> 7;
                state ^= state << 17;
                state
            });
            let out = super::transpose_8x64(&rows);
            for p in 0..64 {
                for k in 0..8 {
                    assert_eq!((out[p] >> k) & 1, ((rows[k] >> p) & 1) as u8, "byte {p} bit {k}");
                }
            }
        }
    }
}

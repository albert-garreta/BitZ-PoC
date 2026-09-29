// Copyright 2024-2025 Irreducible, Inc.
// Modifications copyright 2026 Succinct Labs, Benedikt Bunz, William Wang
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// The algorithm skeleton (iterative LCH NTT, neighbors-last ordering) is
// derived from binius64's `NeighborsLastReference`
// (https://github.com/binius-zk/binius64, `crates/math/src/ntt/reference.rs`).
// The interleaved SoA layout, fused 2-layer butterfly, and parallelization
// strategy are original to Flock.

//! Additive NTT over F_{2^128} using the LCH novel polynomial basis.
//!
//! Iterative LCH NTT skeleton derived from binius64's `NeighborsLastReference`,
//! with an interleaved SoA layout, a fused 2-layer butterfly, and rayon-based
//! parallelization added on top. The forward transform maps polynomial
//! coefficients (in the novel polynomial basis) to evaluations over an
//! F_2-affine subspace; the inverse reverses this. Used by the PCS commit and
//! by FRI folding.
//!
//! ## Convention
//!
//! Given a basis `{β_0, …, β_{ℓ-1}}` of an F_2-subspace V ⊂ F_{2^128}, define
//! the subspace polynomials W_i recursively:
//! ```text
//!     W_0(z) = z
//!     W_i(z) = W_{i-1}(z) · (W_{i-1}(z) + W_{i-1}(β_{i-1}))     (for i ≥ 1)
//! ```
//! and the *normalized* forms `Ŵ_i(z) = W_i(z) / W_i(β_i)` so that
//! `Ŵ_i(β_i) = 1`. The "twiddle" at layer `l` and block `b` is then
//! `Ŵ_{ℓ-l-1}(z)` evaluated at the `b`-th element of the F_2-span of
//! `{β_{ℓ-l}, β_{ℓ-l+1}, …, β_{ℓ-1}}`.
//!
//! At forward-transform layer `l` (`l = 0, …, log_d − 1`):
//! - There are `2^l` blocks, each of size `2^(log_d − l)`.
//! - Within each block, pairs `(idx0, idx0 | block_size_half)` are
//!   butterflied with the block's twiddle.
//! - **Pairing at layer `l`**: positions differ by `block_size_half =
//!   2^(log_d − l − 1)`. So at layer 0 pairs are far (N/2 apart), and at the
//!   deepest layer pairs are adjacent (1 apart) — this is "neighbors-last."
//!
//! FRI fold processes layers in **reverse** (deepest first), at which level
//! pairs are adjacent — matching the standard `fold_pair` formula in DP24.

use crate::field::Gf128;

mod kernels;

/// A/B toggle: when set, the deep (cache-resident) pass of the interleaved
/// parallel NTT stays on the caller's (P-core) pool instead of hopping to
/// [`crate::all_core_pool`] for large transforms. `NTT_DEEP_PCORES_ONLY=1`
/// in the environment forces the same fallback (production kill-switch); the
/// AtomicBool exists for paired within-process A/B.
pub static NTT_DEEP_PCORES_ONLY: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

/// A/B toggle: when set, the deep pass runs every layer as its own sweep
/// instead of fusing general-width layer pairs. `NTT_DEEP_NOFUSE=1` env is
/// the production kill-switch; the AtomicBool is for paired within-process
/// A/B.
pub static NTT_DEEP_NOFUSE: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

/// Scheduling switches of the interleaved NTT, for A/B within one binary.
/// None of them changes a bit of the output: they regroup or reorder the
/// same butterflies with the same twiddles.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InterleavedNttSchedule {
    /// Walk each layer's blocks in order with incremental twiddles
    /// (`twiddle(l, b+1) = twiddle(l, b) + P_l[ctz(b+1)]`, one XOR per block)
    /// instead of an F_2-span sum over up to `l` basis elements per block
    /// (`NTT_TWIDDLE_WALK`, default on). At the deepest layer a twiddle serves only
    /// `num_ntts` half-width butterflies, so the span sum costs about as much
    /// as the work it feeds.
    pub twiddle_walk: bool,
    /// Run the half-width deepest layers of the deep pass depth-first: each
    /// block of the first of them (a few KB) runs through all of them while
    /// it is L1-resident, instead of one sweep of the ~2 MB sub-group per
    /// layer (`NTT_DEEP_TAIL_DF`, default off: it measured neutral at
    /// n = 28 and 30, 1 and 10 threads).
    pub depth_first_tail: bool,
    /// Run all blocks of a top pass as one parallel region instead of one
    /// region, and one join, per block (`NTT_TOP_FLAT`, default on; only at > 1 thread,
    /// where each join waits for the slowest core).
    pub flat_top: bool,
}

impl InterleavedNttSchedule {
    /// Every switch off: the schedule before the switches existed.
    pub const BASELINE: Self = Self {
        twiddle_walk: false,
        depth_first_tail: false,
        flat_top: false,
    };

    /// The environment's switches (`0` or `1` each; unset keeps the
    /// default), read once.
    pub fn from_env() -> Self {
        static SCHEDULE: std::sync::OnceLock<InterleavedNttSchedule> = std::sync::OnceLock::new();
        *SCHEDULE.get_or_init(|| {
            let flag = |name: &str, default: bool| match std::env::var(name).as_deref() {
                Ok("0") => false,
                Ok("1") => true,
                _ => default,
            };
            Self {
                twiddle_walk: flag("NTT_TWIDDLE_WALK", true),
                depth_first_tail: flag("NTT_DEEP_TAIL_DF", false),
                flat_top: flag("NTT_TOP_FLAT", true),
            }
        })
    }
}

/// Compute the normalized subspace-polynomial evaluation table.
///
/// Returns `evals` where `evals[i] = [Ŵ_i(β_i), Ŵ_i(β_{i+1}), …, Ŵ_i(β_{ℓ-1})]`.
/// The 0-th element of each row is always `1` (by normalization).
fn generate_evals_from_subspace(basis: &[Gf128]) -> Vec<Vec<Gf128>> {
    let l = basis.len();
    let mut evals: Vec<Vec<Gf128>> = Vec::with_capacity(l);
    // The zero-dimensional domain has one point and no butterfly layers.
    if basis.is_empty() {
        return evals;
    }

    // evals[0] = [W_0(β_0), W_0(β_1), …, W_0(β_{ℓ-1})] = basis.
    evals.push(basis.to_vec());

    // evals[i][k] = W_i(β_{i+k}) computed from evals[i-1].
    // evals[i-1] = [W_{i-1}(β_{i-1}), W_{i-1}(β_i), W_{i-1}(β_{i+1}), …]
    // We want W_i(β_{i+k}) = W_{i-1}(β_{i+k}) · (W_{i-1}(β_{i+k}) + W_{i-1}(β_{i-1}))
    //                     = evals[i-1][k+1] · (evals[i-1][k+1] + evals[i-1][0])
    for i in 1..l {
        let mut row = Vec::with_capacity(l - i);
        for k in 1..evals[i - 1].len() {
            let val = evals[i - 1][k] * (evals[i - 1][k] + evals[i - 1][0]);
            row.push(val);
        }
        evals.push(row);
    }

    // Normalize each row by its 0-th element (= W_i(β_i)).
    for row in evals.iter_mut() {
        let inv = row[0].inverse_or_zero();
        for v in row.iter_mut() {
            *v *= inv;
        }
    }

    evals
}

/// Compute `Σ_j bit_j(idx) · basis[j]` — the `idx`-th element of the F_2-span
/// of `basis`.
#[inline]
fn span_get(basis: &[Gf128], idx: usize) -> Gf128 {
    let mut acc = Gf128::ZERO;
    for (j, &b) in basis.iter().enumerate() {
        if (idx >> j) & 1 == 1 {
            acc += b;
        }
    }
    acc
}

/// Prefix sums `P[k] = basis[0] + … + basis[k]`: the step between the span
/// elements at `idx` and `idx + 1` is `P[ctz(idx + 1)]` (going from `idx` to
/// `idx + 1` flips exactly bits `0..=ctz(idx + 1)`).
fn span_steps(basis: &[Gf128]) -> Vec<Gf128> {
    let mut acc = Gf128::ZERO;
    basis
        .iter()
        .map(|&b| {
            acc += b;
            acc
        })
        .collect()
}

/// The twiddles of one layer at consecutive blocks `first, first + 1, …`.
/// Walking, each block costs one XOR ([`span_steps`]); otherwise each is the
/// span sum [`AdditiveNttF128::twiddle`] computes.
struct LayerTwiddles<'a> {
    span: &'a [Gf128],
    steps: &'a [Gf128],
    block: usize,
    current: Gf128,
    walk: bool,
}

impl LayerTwiddles<'_> {
    /// The twiddle of the next block.
    #[inline]
    fn take(&mut self) -> Gf128 {
        if !self.walk {
            let twiddle = span_get(self.span, self.block);
            self.block += 1;
            return twiddle;
        }
        let twiddle = self.current;
        self.block += 1;
        // Past the layer's last block there is no step (and no next block).
        if let Some(&step) = self.steps.get(self.block.trailing_zeros() as usize) {
            self.current += step;
        }
        twiddle
    }
}

/// Additive NTT over F_{2^128} with the standard polynomial-basis subspace.
///
/// The basis is `{1, x, x², …, x^(ℓ-1)}` in F_{2^128} = F_2[x]/(GHASH-poly).
/// This makes the F_2-subspace V = `{0, 1, …, 2^ℓ-1}` (under the natural
/// integer encoding of F_{2^128} elements).
#[derive(Clone, Debug)]
pub struct AdditiveNttF128 {
    /// `evals[i]` of length `ℓ − i`, the normalized subspace polynomial values.
    evals: Vec<Vec<Gf128>>,
    /// `steps[l]`: the [`span_steps`] of layer `l`'s twiddle span
    /// (`evals[ℓ − l − 1][1..]`), for walking its blocks in order.
    steps: Vec<Vec<Gf128>>,
}

impl AdditiveNttF128 {
    /// Construct an NTT from an explicit F_2-basis.
    pub fn new(basis: &[Gf128]) -> Self {
        let evals = generate_evals_from_subspace(basis);
        let steps = (0..evals.len())
            .map(|layer| span_steps(&evals[evals.len() - layer - 1][1..]))
            .collect();
        Self { evals, steps }
    }

    /// Standard NTT with basis `{1, x, x², …, x^(dim-1)}`. Requires `dim ≤ 64`
    /// (the low 64 bits of F_{2^128} hold these basis vectors).
    pub fn standard(dim: usize) -> Self {
        assert!(dim <= 64, "standard NTT requires dim ≤ 64");
        let basis: Vec<Gf128> = (0..dim).map(|i| Gf128::new(1u64 << i, 0)).collect();
        Self::new(&basis)
    }

    pub fn log_domain_size(&self) -> usize {
        self.evals.len()
    }

    /// Twiddle at `(layer, block)` for the forward NTT and FRI fold.
    ///
    /// At layer `l` ∈ `[0, ℓ)`, block index `b` ∈ `[0, 2^l)`:
    /// `twiddle(l, b) = Σ_j bit_j(b) · Ŵ_{ℓ-l-1}(β_{ℓ-l+j})`
    ///
    /// (The 0-th element of the row corresponds to `Ŵ_{ℓ-l-1}(β_{ℓ-l-1}) = 1`,
    /// which is "absorbed" into the butterfly and not in the twiddle.)
    pub fn twiddle(&self, layer: usize, block: usize) -> Gf128 {
        let v = &self.evals[self.log_domain_size() - layer - 1];
        span_get(&v[1..], block)
    }

    /// The twiddles of `layer` at blocks `first, first + 1, …`, walked
    /// incrementally when `walk` (see [`InterleavedNttSchedule::twiddle_walk`]).
    fn layer_twiddles(&self, layer: usize, first: usize, walk: bool) -> LayerTwiddles<'_> {
        let span = &self.evals[self.log_domain_size() - layer - 1][1..];
        LayerTwiddles {
            span,
            steps: &self.steps[layer],
            block: first,
            current: if walk { span_get(span, first) } else { Gf128::ZERO },
            walk,
        }
    }

    /// Split point between the top (full-buffer sweep) and deep
    /// (cache-resident sub-group) passes of the interleaved parallel NTT.
    ///
    /// Target sub-group size = 2 MB total bytes. Each position is
    /// `num_ntts × 16` bytes, so positions per sub-group =
    /// 2^21 / (num_ntts · 16). With num_ntts=1: 2^17 positions. With
    /// num_ntts=32: 2^12 positions. (Without this scaling, sub-groups at
    /// num_ntts=32 would be 64 MB and overflow L2 cache.)
    ///
    /// Parallelism floor. The cache heuristic keeps each sub-NTT ~2 MB, but
    /// for a mid-size transform whose whole codeword already fits that
    /// budget it yields `cache_n_top == 0` and the transform runs fully
    /// serial — e.g. the recursive Ligerito commits (~1 ms of NTT each,
    /// previously 1.0× across threads). When the transform is big enough to
    /// amortize rayon overhead, raise `n_top` so the deep-layer split
    /// produces ~one sub-NTT per worker thread (capped to keep each sub-NTT
    /// ≥ 2^MIN_SUB_LOG positions). The large initial PCS commit is unaffected:
    /// its `cache_n_top` already exceeds this floor.
    ///
    /// The floor (log_d ≥ 12) is the measured dispatch-vs-compute crossover
    /// for num_ntts≈8 recursive commits: at log_d=12 parallelizing cuts the
    /// NTT ~0.22 → ~0.08 ms, but at log_d=10 the rayon dispatch costs more
    /// than the ~0.04 ms of work, so those stay scalar.
    #[cfg(any(
        all(target_arch = "aarch64", target_feature = "aes"),
        all(target_arch = "x86_64", target_feature = "pclmulqdq"),
    ))]
    fn interleaved_n_top(&self, log_d: usize, num_ntts: usize) -> usize {
        const TARGET_SUBGROUP_LOG_BYTES: usize = 21;
        let log_bytes_per_position = 4 + log2_pow2(num_ntts);
        let target_log_positions = TARGET_SUBGROUP_LOG_BYTES.saturating_sub(log_bytes_per_position);
        let cache_n_top = log_d.saturating_sub(target_log_positions);
        const PARALLEL_FLOOR_LOG_D: usize = 12;
        const MIN_SUB_LOG: usize = 8;
        if log_d >= PARALLEL_FLOOR_LOG_D {
            let want_subs_log = log2_pow2(rayon::current_num_threads().next_power_of_two());
            let max_n_top = log_d.saturating_sub(MIN_SUB_LOG);
            cache_n_top.max(want_subs_log.min(max_n_top))
        } else {
            cache_n_top
        }
    }

    /// Forward additive NTT in place. `data.len()` must be `2^log_d` for some
    /// `log_d ≤ log_domain_size()`. Layer `l ∈ [0, log_d)` is processed in
    /// order (neighbors-last: top layer first).
    ///
    /// Dispatches to the cache-blocked batched implementation when available
    /// and the buffer is large enough to benefit; otherwise falls back to the
    /// per-layer parallel path or scalar.
    pub fn forward_transform(&self, data: &mut [Gf128]) {
        #[cfg(all(target_arch = "aarch64", target_feature = "aes"))]
        {
            self.forward_transform_batched(data);
        }
        #[cfg(not(all(target_arch = "aarch64", target_feature = "aes")))]
        {
            self.forward_transform_scalar(data);
        }
    }

    /// Interleaved forward NTT: process `num_ntts` independent NTTs in
    /// position-major SoA layout.
    ///
    /// `data` layout: `data[pos * num_ntts + lane]` for `pos ∈ 0..2^log_d`,
    /// `lane ∈ 0..num_ntts`. Each "lane" is an independent NTT instance over
    /// the same domain; all `num_ntts` instances share the twiddle structure
    /// (same `self.twiddle(layer, block)` is applied to every lane at the
    /// corresponding butterfly).
    ///
    /// `num_ntts` must be a positive power of 2. `data.len()` must equal
    /// `(1 << log_d) * num_ntts` for some `log_d ≤ log_domain_size()`.
    ///
    /// This produces the SAME RS code per lane as `forward_transform`, with
    /// FRI-compatible twiddles. The SoA layout is what makes each Merkle leaf
    /// = one position across all `num_ntts` lanes (= contiguous slice of
    /// `num_ntts` F_{2^128} elements).
    pub fn forward_transform_interleaved(&self, data: &mut [Gf128], num_ntts: usize) {
        self.forward_transform_interleaved_from_layer(data, num_ntts, 0);
    }

    /// Forward interleaved NTT starting at `start_layer`, assuming the first
    /// `start_layer` layers have already been applied to `data`.
    ///
    /// The RS-encoding use case: with `log_inv_rate = r` the upper
    /// `(2^r − 1)/2^r` of the coefficient buffer is zero, so each of the first
    /// `r` layers degenerates to a copy (butterfly with `v = 0` gives
    /// `(u, u)`). The caller replicates the message into all `2^r` sub-blocks
    /// — which IS the exact post-layer-`r` state — and skips those layers'
    /// reads and multiplies here.
    pub fn forward_transform_interleaved_from_layer(
        &self,
        data: &mut [Gf128],
        num_ntts: usize,
        start_layer: usize,
    ) {
        self.forward_transform_interleaved_from_layer_with(
            data,
            num_ntts,
            start_layer,
            InterleavedNttSchedule::from_env(),
        );
    }

    /// [`Self::forward_transform_interleaved_from_layer`] under an explicit
    /// [`InterleavedNttSchedule`] (same output for every schedule).
    pub fn forward_transform_interleaved_from_layer_with(
        &self,
        data: &mut [Gf128],
        num_ntts: usize,
        start_layer: usize,
        schedule: InterleavedNttSchedule,
    ) {
        assert!(num_ntts.is_power_of_two() && num_ntts > 0);
        let n_total = data.len();
        assert_eq!(n_total % num_ntts, 0);
        let log_d = log2_pow2(n_total / num_ntts);
        assert!(log_d <= self.log_domain_size());
        assert!(start_layer <= log_d);

        // Scalar; SIMD/parallel variants below dispatch from `forward_transform_interleaved`
        // on supported targets.
        #[cfg(any(
            all(target_arch = "aarch64", target_feature = "aes"),
            all(target_arch = "x86_64", target_feature = "pclmulqdq"),
        ))]
        {
            self.forward_transform_interleaved_parallel_from_layer_with(
                data,
                num_ntts,
                start_layer,
                schedule,
            );
        }
        #[cfg(not(any(
            all(target_arch = "aarch64", target_feature = "aes"),
            all(target_arch = "x86_64", target_feature = "pclmulqdq"),
        )))]
        {
            self.forward_transform_interleaved_scalar_from_layer_with(
                data,
                num_ntts,
                start_layer,
                schedule.twiddle_walk,
            );
        }
    }

    /// Scalar reference for the interleaved forward NTT.
    pub fn forward_transform_interleaved_scalar(&self, data: &mut [Gf128], num_ntts: usize) {
        self.forward_transform_interleaved_scalar_from_layer(data, num_ntts, 0);
    }

    /// Scalar interleaved forward NTT from `start_layer` (see
    /// [`Self::forward_transform_interleaved_from_layer`]). Every twiddle is
    /// its own span sum, so this stays the reference the tests compare to.
    pub fn forward_transform_interleaved_scalar_from_layer(
        &self,
        data: &mut [Gf128],
        num_ntts: usize,
        start_layer: usize,
    ) {
        self.forward_transform_interleaved_scalar_from_layer_with(
            data,
            num_ntts,
            start_layer,
            false,
        );
    }

    /// [`Self::forward_transform_interleaved_scalar_from_layer`], walking each
    /// layer's twiddles incrementally when `walk`.
    fn forward_transform_interleaved_scalar_from_layer_with(
        &self,
        data: &mut [Gf128],
        num_ntts: usize,
        start_layer: usize,
        walk: bool,
    ) {
        let n_total = data.len();
        let log_d = log2_pow2(n_total / num_ntts);

        for layer in start_layer..log_d {
            let num_blocks = 1usize << layer;
            let block_size = 1usize << (log_d - layer);
            let block_size_half = block_size >> 1;
            let block_size_bytes = block_size * num_ntts;
            let mut twiddles = self.layer_twiddles(layer, 0, walk);
            for block in 0..num_blocks {
                let twiddle = twiddles.take();
                let block_start = block * block_size_bytes;
                // Butterfly pairs (top, bot) at positions (row, row + block_size_half)
                // within the block. Each "position" holds num_ntts lanes side-by-side.
                for row in 0..block_size_half {
                    let off_top = block_start + row * num_ntts;
                    let off_bot = off_top + block_size_half * num_ntts;
                    for lane in 0..num_ntts {
                        let v = data[off_bot + lane];
                        let new_u = data[off_top + lane] + v * twiddle;
                        data[off_top + lane] = new_u;
                        data[off_bot + lane] = v + new_u;
                    }
                }
            }
        }
    }

    /// Parallel + NEON interleaved forward NTT. Cache-blocks the same way as
    /// `forward_transform_batched`: top layers process the full SoA buffer with
    /// per-block parallelism; deep layers process each sub-NTT-group in cache.
    ///
    /// Internally calls [`forward_transform_interleaved_scalar`] for very small
    /// inputs to avoid rayon overhead; for large inputs it uses an in-place
    /// scalar butterfly per lane (per-lane vectorization is future work — the
    /// big win at large `m` is cache locality + thread parallelism).
    #[cfg(any(
        all(target_arch = "aarch64", target_feature = "aes"),
        all(target_arch = "x86_64", target_feature = "pclmulqdq"),
    ))]
    pub fn forward_transform_interleaved_parallel(&self, data: &mut [Gf128], num_ntts: usize) {
        self.forward_transform_interleaved_parallel_from_layer(data, num_ntts, 0);
    }

    /// Parallel interleaved forward NTT from `start_layer` (see
    /// [`Self::forward_transform_interleaved_from_layer`]).
    #[cfg(any(
        all(target_arch = "aarch64", target_feature = "aes"),
        all(target_arch = "x86_64", target_feature = "pclmulqdq"),
    ))]
    pub fn forward_transform_interleaved_parallel_from_layer(
        &self,
        data: &mut [Gf128],
        num_ntts: usize,
        start_layer: usize,
    ) {
        self.forward_transform_interleaved_parallel_from_layer_with(
            data,
            num_ntts,
            start_layer,
            InterleavedNttSchedule::from_env(),
        );
    }

    /// [`Self::forward_transform_interleaved_parallel_from_layer`] under an
    /// explicit [`InterleavedNttSchedule`] (same output for every schedule).
    #[cfg(any(
        all(target_arch = "aarch64", target_feature = "aes"),
        all(target_arch = "x86_64", target_feature = "pclmulqdq"),
    ))]
    pub fn forward_transform_interleaved_parallel_from_layer_with(
        &self,
        data: &mut [Gf128],
        num_ntts: usize,
        start_layer: usize,
        schedule: InterleavedNttSchedule,
    ) {
        use rayon::prelude::*;
        let n_total = data.len();
        let log_d = log2_pow2(n_total / num_ntts);
        let walk = schedule.twiddle_walk;

        let n_top = self.interleaved_n_top(log_d, num_ntts);
        if n_top == 0 || log_d < 8 {
            self.forward_transform_interleaved_scalar_from_layer_with(
                data,
                num_ntts,
                start_layer,
                walk,
            );
            return;
        }

        // Top layers: full-buffer sweep. Parallelize **rows within each
        // block** so even layer 0 (1 huge block) gets rayon parallelism.
        //
        // Layer fusion: at top layers each layer is a separate full-buffer
        // sweep (read 512 MB + write 512 MB at m=31). Fusing two consecutive
        // layers in one pass loads each row once, applies both butterflies
        // in registers, stores once — halving memory traffic on the fused
        // layers. Each "outer block" at layer L has 4 contributing rows per
        // quarter-row; layer L butterflies (a,c) and (b,d) (distance =
        // block_size/2), layer L+1 butterflies (a,b) and (c,d) (distance =
        // block_size/4).
        // Fuse FOUR layers per pass only where a SIMD fused-4 kernel exists
        // (x86 AVX-512). On other targets the 16-point kernel falls back to
        // scalar, which is slower than the NEON fused-2 path — so keep fused-2
        // there. NEON fused-4 is a future addition.
        let fused4_ok = cfg!(all(
            target_arch = "x86_64",
            target_feature = "avx512f",
    target_feature = "avx512bw",
    target_feature = "pclmulqdq",
    target_feature = "sse4.1",
            target_feature = "vpclmulqdq"
        ));
        let flat = schedule.flat_top && rayon::current_num_threads() > 1;
        let mut layer = start_layer.min(n_top);
        while layer < n_top {
            let block_size = 1usize << (log_d - layer);
            let block_bytes = block_size * num_ntts;

            if fused4_ok && layer + 3 < n_top && block_size >= 16 {
                // Fuse four layers (layer..layer+4): one read+write per block
                // instead of four. Each block contributes a 16-point butterfly.
                let sixteenth = block_size >> 4;
                for_each_top_block(data, block_bytes, flat, |block, chunk| {
                    let mut tw = [Gf128 { lo: 0, hi: 0 }; 15];
                    tw[0] = self.twiddle(layer, block);
                    for s in 0..2 {
                        tw[1 + s] = self.twiddle(layer + 1, 2 * block + s);
                    }
                    for s in 0..4 {
                        tw[3 + s] = self.twiddle(layer + 2, 4 * block + s);
                    }
                    for s in 0..8 {
                        tw[7 + s] = self.twiddle(layer + 3, 8 * block + s);
                    }
                    butterfly_interleaved_fused_4layer_par_rows(chunk, &tw, sixteenth, num_ntts);
                });
                layer += 4;
            } else if layer + 1 < n_top && block_size >= 4 {
                // Fuse layers (layer, layer+1).
                let quarter = block_size >> 2;
                for_each_top_block(data, block_bytes, flat, |block, chunk| {
                    let t_outer = self.twiddle(layer, block);
                    let t_inner_a = self.twiddle(layer + 1, 2 * block);
                    let t_inner_b = self.twiddle(layer + 1, 2 * block + 1);
                    butterfly_interleaved_fused_2layer_par_rows(
                        chunk, t_outer, t_inner_a, t_inner_b, quarter, num_ntts,
                    );
                });
                layer += 2;
            } else {
                let block_size_half = block_size >> 1;
                for_each_top_block(data, block_bytes, flat, |block, chunk| {
                    let t = self.twiddle(layer, block);
                    butterfly_interleaved_block_par_rows(chunk, t, block_size_half, num_ntts);
                });
                layer += 1;
            }
        }

        // Deep layers: process each sub-NTT-group cache-resident.
        let sub_size_positions = 1usize << (log_d - n_top);
        let sub_bytes = sub_size_positions * num_ntts;

        // Within each sub-group, fuse consecutive GENERAL-width layer pairs
        // into one sweep (halves the sub's L1/L2 traffic and the butterfly
        // load/store count for those layers — the deep pass is compute-bound,
        // not DRAM-bound, so in-cache traffic and instruction count are what
        // matter). The three deepest layers stay single-layer: their twiddles
        // are all half-width (see `mul_polynomial_u64`) and the fast path in
        // `butterfly_interleaved_block` beats fusion's general muls. Under
        // `depth_first_tail` those three run depth-first instead of as three
        // sweeps of the sub-group.
        // `NTT_DEEP_NOFUSE` restores per-layer sweeps (A/B).
        let fuse = !NTT_DEEP_NOFUSE.load(std::sync::atomic::Ordering::Relaxed)
            && std::env::var("NTT_DEEP_NOFUSE").is_err();
        let halfwidth_start = log_d.saturating_sub(3);
        let deep = |data: &mut [Gf128]| {
            data.par_chunks_mut(sub_bytes)
                .enumerate()
                .for_each(|(sub_idx, sub_data)| {
                    let mut layer = n_top.max(start_layer);
                    while layer < log_d {
                        // The sub-group's blocks at this layer are the global
                        // blocks `first_block..first_block + 2^(layer − n_top)`.
                        let first_block = sub_idx << (layer - n_top);
                        let block_size = 1usize << (log_d - layer);
                        let block_bytes = block_size * num_ntts;

                        if schedule.depth_first_tail
                            && layer >= halfwidth_start
                            && layer + 1 < log_d
                        {
                            self.deep_tail_depth_first(
                                sub_data,
                                num_ntts,
                                layer,
                                log_d,
                                first_block,
                                walk,
                            );
                            break;
                        }
                        if fuse && layer + 2 <= halfwidth_start && block_size >= 4 {
                            let quarter = block_size >> 2;
                            let mut outer = self.layer_twiddles(layer, first_block, walk);
                            let mut inner = self.layer_twiddles(layer + 1, 2 * first_block, walk);
                            for block in sub_data.chunks_exact_mut(block_bytes) {
                                let t_outer = outer.take();
                                let t_inner_a = inner.take();
                                let t_inner_b = inner.take();
                                butterfly_interleaved_fused_2layer_serial(
                                    block, t_outer, t_inner_a, t_inner_b, quarter, num_ntts,
                                );
                            }
                            layer += 2;
                        } else {
                            let block_size_half = block_size >> 1;
                            let mut twiddles = self.layer_twiddles(layer, first_block, walk);
                            for block in sub_data.chunks_exact_mut(block_bytes) {
                                butterfly_interleaved_block(
                                    block,
                                    twiddles.take(),
                                    block_size_half,
                                    num_ntts,
                                );
                            }
                            layer += 1;
                        }
                    }
                });
        };
        // The deep pass is a flat parallel-for over independent ~2 MB
        // sub-groups with a single join — measured ~80% PMULL-compute-bound at
        // m=30 (one streaming pass of traffic, 11 in-cache layers). Unlike the
        // barrier-per-block top passes, it drains cleanly around slow cores,
        // and the E-cores have PMULL too — so large transforms hop to the
        // all-core (P+E) pool. Pool choice cannot change output bits (each
        // sub-group is written deterministically). Gate: enough sub-groups to
        // drain (≥ 4× workers) and ≥ 64 MB of data so the pool switch and
        // E-core L2 pressure can't hurt small/recursive commits.
        // `NTT_DEEP_PCORES_ONLY` (atomic or env) restores the caller's pool.
        let n_subs = data.len() / sub_bytes;
        let use_all_cores = std::mem::size_of_val(data) >= (64 << 20)
            && !NTT_DEEP_PCORES_ONLY.load(std::sync::atomic::Ordering::Relaxed)
            && std::env::var("NTT_DEEP_PCORES_ONLY").is_err()
            && {
                let pool = crate::all_core_pool();
                pool.current_num_threads() > rayon::current_num_threads()
                    && n_subs >= 4 * pool.current_num_threads()
            };
        if use_all_cores {
            crate::all_core_pool().install(|| deep(data));
        } else {
            deep(data);
        }
    }

    /// Layers `layer..log_d` of one deep sub-group, depth-first: each
    /// layer-`layer` block (`2^(log_d − layer)` positions, a few KB) runs
    /// through every remaining layer while it is L1-resident, instead of
    /// one sweep of the whole sub-group per layer. Same butterflies and
    /// twiddles; a block's butterflies at layer `l + 1` read only its own
    /// layer-`l` outputs. `first_block` is the sub-group's first global block
    /// at `layer`.
    #[cfg(any(
        all(target_arch = "aarch64", target_feature = "aes"),
        all(target_arch = "x86_64", target_feature = "pclmulqdq"),
    ))]
    fn deep_tail_depth_first(
        &self,
        sub_data: &mut [Gf128],
        num_ntts: usize,
        layer: usize,
        log_d: usize,
        first_block: usize,
        walk: bool,
    ) {
        let depth = log_d - layer;
        let block_bytes = (1usize << depth) * num_ntts;
        // Layer `layer + j`'s blocks inside layer-`layer` block `b` are
        // `2^j·b .. 2^j·(b + 1)`, so each layer's walk stays in order.
        let mut twiddles: Vec<LayerTwiddles<'_>> = (0..depth)
            .map(|j| self.layer_twiddles(layer + j, first_block << j, walk))
            .collect();
        for block in sub_data.chunks_exact_mut(block_bytes) {
            for (j, layer_twiddles) in twiddles.iter_mut().enumerate() {
                let sub_block_bytes = block_bytes >> j;
                let block_size_half = 1usize << (depth - j - 1);
                for sub_block in block.chunks_exact_mut(sub_block_bytes) {
                    butterfly_interleaved_block(
                        sub_block,
                        layer_twiddles.take(),
                        block_size_half,
                        num_ntts,
                    );
                }
            }
        }
    }

    /// RS-encode a message directly: `data` receives the forward interleaved
    /// NTT of the zero-padded coefficient vector `[msg, 0, …, 0]`, with the
    /// blow-up `data.len() / msg.len() = 2^r ≥ 2` giving the inverse rate.
    ///
    /// The first `r` layers on `[msg, 0, …, 0]` are pure copies (butterfly
    /// with `v = 0` gives `(u, u)`), so their post-state is `2^r` replicas of
    /// `msg`. Rather than materializing that state (a full-buffer write pass
    /// that the next layer immediately re-reads), the first fused top pass
    /// here reads its four input rows straight from `msg`, copies them into
    /// the destination rows, and butterflies them in place while the lines
    /// are L1-hot — DRAM sees one write per line, and the standalone
    /// replicate pass disappears. Shapes the fused-2 top pass cannot serve
    /// fall back to an internal replicate +
    /// [`Self::forward_transform_interleaved_from_layer`] (`data`'s prior
    /// contents may be arbitrary; every slot is written). Port of upstream
    /// flock's from-message entry, without its ranked radix-8 top.
    pub fn forward_transform_interleaved_from_message(
        &self,
        data: &mut [Gf128],
        msg: &[Gf128],
        num_ntts: usize,
    ) {
        self.forward_transform_interleaved_from_message_with(
            data,
            msg,
            num_ntts,
            InterleavedNttSchedule::from_env(),
        );
    }

    /// [`Self::forward_transform_interleaved_from_message`] under an explicit
    /// [`InterleavedNttSchedule`] (same output for every schedule).
    pub fn forward_transform_interleaved_from_message_with(
        &self,
        data: &mut [Gf128],
        msg: &[Gf128],
        num_ntts: usize,
        schedule: InterleavedNttSchedule,
    ) {
        let n_total = data.len();
        assert!(num_ntts.is_power_of_two() && num_ntts > 0);
        assert_eq!(n_total % msg.len(), 0);
        let reps = n_total / msg.len();
        assert!(reps.is_power_of_two() && reps >= 2, "need a blow-up ≥ 2");
        assert_eq!(msg.len() % num_ntts, 0);
        let start_layer = log2_pow2(reps);
        let log_d = log2_pow2(n_total / num_ntts);
        assert!(log_d <= self.log_domain_size());

        #[cfg(any(
            all(target_arch = "aarch64", target_feature = "aes"),
            all(target_arch = "x86_64", target_feature = "pclmulqdq"),
        ))]
        {
            let n_top = self.interleaved_n_top(log_d, num_ntts);
            let block_size = 1usize << (log_d - start_layer);
            // The fused first pass needs: the parallel path engaged, at least
            // two top layers left after the copy layers, quarterable blocks,
            // and the x86 fused-4 pass not claiming these layers (it fuses 4
            // at a time starting at `start_layer`, which the src-copy wrapper
            // doesn't implement — x86 keeps the replicate path).
            let fused4_ok = cfg!(all(
                target_arch = "x86_64",
                target_feature = "avx512f",
                target_feature = "vpclmulqdq"
            ));
            let fuse_fill = !fused4_ok
                && log_d >= 8
                && n_top > 0
                && start_layer + 2 <= n_top
                && block_size >= 4;
            if fuse_fill {
                // First fused-2 pass (layers start_layer, start_layer+1),
                // sourcing every block's rows from `msg`: each
                // layer-`start_layer` block is exactly one would-be replica, so
                // its input rows are `msg`'s rows.
                let quarter = block_size >> 2;
                let block_bytes = block_size * num_ntts;
                let flat = schedule.flat_top && rayon::current_num_threads() > 1;
                for_each_top_block(data, block_bytes, flat, |block, chunk| {
                    let t_outer = self.twiddle(start_layer, block);
                    let t_inner_a = self.twiddle(start_layer + 1, 2 * block);
                    let t_inner_b = self.twiddle(start_layer + 1, 2 * block + 1);
                    butterfly_interleaved_fused_2layer_par_rows_from_src(
                        chunk, msg, t_outer, t_inner_a, t_inner_b, quarter, num_ntts,
                    );
                });
                self.forward_transform_interleaved_parallel_from_layer_with(
                    data,
                    num_ntts,
                    start_layer + 2,
                    schedule,
                );
                return;
            }
        }
        replicate_interleaved(data, msg);
        self.forward_transform_interleaved_from_layer_with(data, num_ntts, start_layer, schedule);
    }

    /// Scalar reference implementation. Used as the test oracle and on
    /// platforms without NEON+PMULL.
    pub fn forward_transform_scalar(&self, data: &mut [Gf128]) {
        let log_d = log2_pow2(data.len());
        assert!(log_d <= self.log_domain_size());

        for layer in 0..log_d {
            let num_blocks = 1usize << layer;
            let block_size_half = 1usize << (log_d - layer - 1);
            for block in 0..num_blocks {
                let twiddle = self.twiddle(layer, block);
                let block_start = block << (log_d - layer);
                for idx0 in block_start..(block_start + block_size_half) {
                    let idx1 = idx0 | block_size_half;
                    // Forward butterfly: u += v·twiddle; v += u.
                    let v = data[idx1];
                    let new_u = data[idx0] + v * twiddle;
                    data[idx0] = new_u;
                    data[idx1] = v + new_u;
                }
            }
        }
    }

    /// Single-threaded NEON forward transform (uses `ghash_mul_vec2_neon` to
    /// batch 2 butterflies per PMULL pair).
    #[cfg(all(target_arch = "aarch64", target_feature = "aes"))]
    pub fn forward_transform_neon(&self, data: &mut [Gf128]) {
        let log_d = log2_pow2(data.len());
        assert!(log_d <= self.log_domain_size());

        for layer in 0..log_d {
            let num_blocks = 1usize << layer;
            let block_size = 1usize << (log_d - layer);
            let block_size_half = block_size >> 1;
            // SAFETY: target_feature = "aes" enabled at compile time.
            unsafe {
                if block_size_half >= 2 {
                    // Within-block: batch 2 pairs with shared twiddle.
                    for block in 0..num_blocks {
                        let twiddle = self.twiddle(layer, block);
                        let block_start = block * block_size;
                        let chunk = &mut data[block_start..block_start + block_size];
                        kernels::butterfly_neon_block(chunk, twiddle, block_size_half);
                    }
                } else {
                    // Deepest layer (half = 1): batch across 2 adjacent blocks
                    // (different twiddles). Handle odd tail with scalar when
                    // num_blocks = 1 (only happens at log_d = 1).
                    debug_assert_eq!(block_size_half, 1);
                    let mut block = 0;
                    while block + 1 < num_blocks {
                        let t_a = self.twiddle(layer, block);
                        let t_b = self.twiddle(layer, block + 1);
                        kernels::butterfly_neon_block_pair(data, block * 2, t_a, t_b);
                        block += 2;
                    }
                    // Scalar tail (num_blocks odd — only when num_blocks = 1).
                    while block < num_blocks {
                        let twiddle = self.twiddle(layer, block);
                        let idx0 = block * 2;
                        let idx1 = idx0 + 1;
                        let v = data[idx1];
                        let new_u = data[idx0] + v * twiddle;
                        data[idx0] = new_u;
                        data[idx1] = v + new_u;
                        block += 1;
                    }
                }
            }
        }
    }

    /// Rayon-parallel + NEON forward transform.
    #[cfg(all(target_arch = "aarch64", target_feature = "aes"))]
    pub fn forward_transform_parallel(&self, data: &mut [Gf128]) {
        use rayon::prelude::*;
        let log_d = log2_pow2(data.len());
        assert!(log_d <= self.log_domain_size());

        // For small data (or shallow layers with few large blocks), the rayon
        // overhead exceeds the gain — fall back to the NEON single-thread path.
        const PARALLEL_THRESHOLD_LOG: usize = 14; // 2^14 = 16K elements (256 KB)
        if log_d <= PARALLEL_THRESHOLD_LOG {
            self.forward_transform_neon(data);
            return;
        }

        for layer in 0..log_d {
            let num_blocks = 1usize << layer;
            let block_size = 1usize << (log_d - layer);
            let block_size_half = block_size >> 1;

            // Parallelize across blocks when there are enough; otherwise process
            // sequentially with NEON (still fast for small block counts).
            if num_blocks >= 4 && block_size_half >= 2 {
                let twiddles: Vec<Gf128> = (0..num_blocks).map(|b| self.twiddle(layer, b)).collect();
                data.par_chunks_mut(block_size)
                    .zip(twiddles.par_iter())
                    .for_each(|(chunk, &twiddle)| {
                        // SAFETY: aes target feature enabled.
                        unsafe { kernels::butterfly_neon_block(chunk, twiddle, block_size_half) };
                    });
            } else if block_size_half >= 2 {
                // Few large blocks — process sequentially with NEON.
                // SAFETY: aes target feature enabled.
                unsafe {
                    for block in 0..num_blocks {
                        let twiddle = self.twiddle(layer, block);
                        let block_start = block * block_size;
                        kernels::butterfly_neon_block(
                            &mut data[block_start..block_start + block_size],
                            twiddle,
                            block_size_half,
                        );
                    }
                }
            } else {
                // Deepest layer (half = 1): need num_blocks ≥ 2 to batch
                // pairs; if there are at least 2 blocks, batch across them.
                // (When num_blocks < 2, fall back to NEON-single-thread which
                // handles the trivial cases.)
                debug_assert_eq!(block_size_half, 1);
                if num_blocks >= 2 {
                    let twiddles: Vec<Gf128> =
                        (0..num_blocks).map(|b| self.twiddle(layer, b)).collect();
                    data.par_chunks_mut(4).zip(twiddles.par_chunks(2)).for_each(
                        |(chunk, twiddle_pair)| {
                            // SAFETY: aes target feature enabled.
                            unsafe {
                                kernels::butterfly_neon_block_pair_chunk(
                                    chunk,
                                    twiddle_pair[0],
                                    twiddle_pair[1],
                                )
                            };
                        },
                    );
                } else {
                    let twiddle = self.twiddle(layer, 0);
                    let v = data[1];
                    let new_u = data[0] + v * twiddle;
                    data[0] = new_u;
                    data[1] = v + new_u;
                }
            }
        }
    }

    /// Cache-blocked + parallel + NEON forward transform.
    ///
    /// **Strategy**: decompose the NTT into two stages so the deep layers
    /// (which dominate work) operate on sub-buffers small enough to fit in L2
    /// cache, avoiding the DRAM round-trip per layer.
    ///
    /// 1. **Top layers** (layers `0..n_top`): each layer touches the full buffer
    ///    in one sweep. Bandwidth-bound; parallelize across blocks.
    /// 2. **Deep layers** (layers `n_top..log_d`): treat the data as `2^n_top`
    ///    independent sub-NTTs, each of size `2^(log_d − n_top)`. For each
    ///    sub-NTT, process ALL remaining layers in one cache-resident pass.
    ///    Parallelize across sub-NTTs via rayon.
    ///
    /// `n_top` is chosen so each sub-NTT is `≈ 2 MB` (= `2^17` F_{2^128} ≈ 2 MB).
    /// For `log_d ≤ 17` the whole NTT fits in cache and we fall back to the
    /// per-layer parallel path.
    #[cfg(all(target_arch = "aarch64", target_feature = "aes"))]
    pub fn forward_transform_batched(&self, data: &mut [Gf128]) {
        use rayon::prelude::*;
        let log_d = log2_pow2(data.len());
        assert!(log_d <= self.log_domain_size());

        // Target sub-NTT size: 2^17 F_{2^128} = 2 MB. Tunable.
        const TARGET_SUB_NTT_LOG: usize = 17;
        if log_d <= TARGET_SUB_NTT_LOG {
            self.forward_transform_parallel(data);
            return;
        }
        let n_top = log_d - TARGET_SUB_NTT_LOG;
        let sub_ntt_size = 1usize << (log_d - n_top);

        // ---- Stage 1: top layers (full-buffer, bandwidth-bound).
        for layer in 0..n_top {
            let num_blocks = 1usize << layer;
            let block_size = 1usize << (log_d - layer);
            let block_size_half = block_size >> 1;

            if num_blocks >= 4 {
                let twiddles: Vec<Gf128> = (0..num_blocks).map(|b| self.twiddle(layer, b)).collect();
                data.par_chunks_mut(block_size)
                    .zip(twiddles.par_iter())
                    .for_each(|(chunk, &t)| {
                        // SAFETY: aes target feature enabled.
                        unsafe { kernels::butterfly_neon_block(chunk, t, block_size_half) };
                    });
            } else {
                // Few large blocks at very top layers: sequential NEON.
                unsafe {
                    for block in 0..num_blocks {
                        let t = self.twiddle(layer, block);
                        let block_start = block * block_size;
                        kernels::butterfly_neon_block(
                            &mut data[block_start..block_start + block_size],
                            t,
                            block_size_half,
                        );
                    }
                }
            }
        }

        // ---- Stage 2: deep layers as parallel cache-resident sub-NTTs.
        data.par_chunks_mut(sub_ntt_size)
            .enumerate()
            .for_each(|(sub_idx, sub_data)| {
                for layer in n_top..log_d {
                    let layer_in_sub = layer - n_top;
                    let num_blocks_in_sub = 1usize << layer_in_sub;
                    let block_size = 1usize << (log_d - layer);
                    let block_size_half = block_size >> 1;

                    for block_in_sub in 0..num_blocks_in_sub {
                        let global_block = sub_idx * num_blocks_in_sub + block_in_sub;
                        let twiddle = self.twiddle(layer, global_block);
                        let block_start = block_in_sub * block_size;
                        let block = &mut sub_data[block_start..block_start + block_size];
                        if block_size_half >= 2 {
                            // SAFETY: aes target feature enabled.
                            unsafe {
                                kernels::butterfly_neon_block(block, twiddle, block_size_half)
                            };
                        } else {
                            // Deepest layer: 1 pair per block, scalar.
                            let v = block[1];
                            let new_u = block[0] + v * twiddle;
                            block[0] = new_u;
                            block[1] = v + new_u;
                        }
                    }
                }
            });
    }

    /// Inverse additive NTT in place. Exact inverse of `forward_transform`.
    pub fn inverse_transform(&self, data: &mut [Gf128]) {
        let log_d = log2_pow2(data.len());
        assert!(log_d <= self.log_domain_size());

        for layer in (0..log_d).rev() {
            let num_blocks = 1usize << layer;
            let block_size_half = 1usize << (log_d - layer - 1);
            for block in 0..num_blocks {
                let twiddle = self.twiddle(layer, block);
                let block_start = block << (log_d - layer);
                for idx0 in block_start..(block_start + block_size_half) {
                    let idx1 = idx0 | block_size_half;
                    // Inverse butterfly: v += u; u += v·twiddle.
                    let u = data[idx0];
                    let new_v = data[idx1] + u;
                    data[idx1] = new_v;
                    data[idx0] = u + new_v * twiddle;
                }
            }
        }
    }
}

/// Like [`butterfly_interleaved_block`] but parallelizes across rows via
/// rayon. Used at top layers where the block is large (≥ 1024 rows) and only
/// 1-2 blocks exist (so block-level parallelism would be too coarse).
///
/// Falls back to sequential when the row count is small.
#[inline]
fn butterfly_interleaved_block_par_rows(
    block: &mut [Gf128],
    twiddle: Gf128,
    block_size_half: usize,
    num_ntts: usize,
) {
    use rayon::prelude::*;
    const PARALLEL_ROW_THRESHOLD: usize = 512;
    if block_size_half < PARALLEL_ROW_THRESHOLD {
        butterfly_interleaved_block(block, twiddle, block_size_half, num_ntts);
        return;
    }
    let half_offset = block_size_half * num_ntts;
    let (top, bot) = block.split_at_mut(half_offset);
    // Zero-twiddle fast path (see `butterfly_interleaved_block`).
    if twiddle == Gf128::ZERO {
        top.par_chunks_mut(num_ntts)
            .zip(bot.par_chunks_mut(num_ntts))
            .for_each(|(top_row, bot_row)| {
                for lane in 0..num_ntts {
                    bot_row[lane] += top_row[lane];
                }
            });
        return;
    }
    top.par_chunks_mut(num_ntts)
        .zip(bot.par_chunks_mut(num_ntts))
        .for_each(|(top_row, bot_row)| {
            kernels::butterfly_row_pair(top_row, bot_row, twiddle);
        });
}

/// One quarter-row of the fused 2-layer butterfly, with the zero-twiddle
/// short-circuit for outer block 0 (`t_outer = t_inner_a = 0` — only the
/// (c,d) inner butterfly multiplies; the branch is per-row, not per-lane).
/// The general case delegates to the arch-dispatched
/// [`kernels::butterfly_fused_2layer`].
#[inline(always)]
fn fused_2layer_row_op(
    row_a: &mut [Gf128],
    row_b: &mut [Gf128],
    row_c: &mut [Gf128],
    row_d: &mut [Gf128],
    t_outer: Gf128,
    t_inner_a: Gf128,
    t_inner_b: Gf128,
    zero_block: bool,
    num_ntts: usize,
) {
    if zero_block {
        for lane in 0..num_ntts {
            let a = row_a[lane];
            let b = row_b[lane];
            // Layer L with t_outer=0: a,b unchanged; c += a; d += b.
            let c = row_c[lane] + a;
            let d = row_d[lane] + b;
            // Layer L+1: (a,b) with t_inner_a=0: a unchanged; b += a.
            row_b[lane] = b + a;
            // (c,d) with the real t_inner_b.
            let new_c2 = c + d * t_inner_b;
            row_c[lane] = new_c2;
            row_d[lane] = d + new_c2;
        }
        return;
    }
    kernels::butterfly_fused_2layer(row_a, row_b, row_c, row_d, t_outer, t_inner_a, t_inner_b);
}

/// Forced-serial fused 2-layer butterfly for use INSIDE the deep pass's
/// per-sub-group workers (already running one task per pool worker — nested
/// row-parallelism would only add dispatch overhead). Same math as
/// [`butterfly_interleaved_fused_2layer_par_rows`].
fn butterfly_interleaved_fused_2layer_serial(
    block: &mut [Gf128],
    t_outer: Gf128,
    t_inner_a: Gf128,
    t_inner_b: Gf128,
    quarter: usize,
    num_ntts: usize,
) {
    let stride = quarter * num_ntts;
    debug_assert_eq!(block.len(), 4 * stride);
    let zero_block = t_outer == Gf128::ZERO && t_inner_a == Gf128::ZERO;
    let (top_half, bot_half) = block.split_at_mut(2 * stride);
    let (q1, q2) = top_half.split_at_mut(stride);
    let (q3, q4) = bot_half.split_at_mut(stride);
    for r in 0..quarter {
        let off = r * num_ntts;
        let (q1r, _) = q1[off..].split_at_mut(num_ntts);
        let (q2r, _) = q2[off..].split_at_mut(num_ntts);
        let (q3r, _) = q3[off..].split_at_mut(num_ntts);
        let (q4r, _) = q4[off..].split_at_mut(num_ntts);
        fused_2layer_row_op(
            q1r, q2r, q3r, q4r, t_outer, t_inner_a, t_inner_b, zero_block, num_ntts,
        );
    }
}

/// Fused 2-layer butterfly: combines layer L (twiddle `t_outer`, shared by
/// the whole outer block) with layer L+1 (twiddles `t_inner_a` for the top
/// half, `t_inner_b` for the bottom half). Reads each row of the outer
/// block once and writes once — halving memory traffic vs running the two
/// layers as separate sweeps.
///
/// `block` has length `4 * quarter * num_ntts` (= one layer-L block of
/// `4*quarter` rows). For each `r ∈ 0..quarter`, four rows participate:
/// `a=r`, `b=r+quarter`, `c=r+2*quarter`, `d=r+3*quarter`. Layer L
/// butterflies `(a,c)` and `(b,d)`; layer L+1 then butterflies `(a,b)` (in
/// the new top sub-block) and `(c,d)` (in the new bottom sub-block).
#[inline]
fn butterfly_interleaved_fused_2layer_par_rows(
    block: &mut [Gf128],
    t_outer: Gf128,
    t_inner_a: Gf128,
    t_inner_b: Gf128,
    quarter: usize,
    num_ntts: usize,
) {
    use rayon::prelude::*;
    const PARALLEL_ROW_THRESHOLD: usize = 256;
    let stride = quarter * num_ntts;
    debug_assert_eq!(block.len(), 4 * stride);

    // Block 0 of the outer layer has t_outer = 0 AND t_inner_a =
    // twiddle(L+1, 0) = 0 — only the (c,d) inner butterfly multiplies. The
    // branch is per-row, not per-lane.
    let zero_block = t_outer == Gf128::ZERO && t_inner_a == Gf128::ZERO;
    let do_one =
        |row_a: &mut [Gf128], row_b: &mut [Gf128], row_c: &mut [Gf128], row_d: &mut [Gf128]| {
            fused_2layer_row_op(
                row_a, row_b, row_c, row_d, t_outer, t_inner_a, t_inner_b, zero_block, num_ntts,
            );
        };

    // Split the block into four quarters, then zip row-wise. Each rayon task
    // processes one quarter-row index = 4 logical rows of work.
    let (top_half, bot_half) = block.split_at_mut(2 * stride);
    let (q1, q2) = top_half.split_at_mut(stride);
    let (q3, q4) = bot_half.split_at_mut(stride);

    if quarter < PARALLEL_ROW_THRESHOLD {
        for r in 0..quarter {
            let off = r * num_ntts;
            let (q1r, q1_rest) = q1[off..].split_at_mut(num_ntts);
            let _ = q1_rest;
            let (q2r, _) = q2[off..].split_at_mut(num_ntts);
            let (q3r, _) = q3[off..].split_at_mut(num_ntts);
            let (q4r, _) = q4[off..].split_at_mut(num_ntts);
            do_one(q1r, q2r, q3r, q4r);
        }
    } else {
        q1.par_chunks_mut(num_ntts)
            .zip(q2.par_chunks_mut(num_ntts))
            .zip(q3.par_chunks_mut(num_ntts))
            .zip(q4.par_chunks_mut(num_ntts))
            .for_each(|(((row_a, row_b), row_c), row_d)| {
                do_one(row_a, row_b, row_c, row_d);
            });
    }
}

/// Run `op(block, chunk)` on every `block_bytes` chunk of a top pass: in
/// order on the caller when not `flat`; otherwise as one parallel region,
/// so the pass pays one join instead of one per block (each `op` still
/// parallelizes over its rows; nested regions are stolen into, not joined
/// separately). Blocks are disjoint, so the output cannot depend on this.
#[cfg(any(
    all(target_arch = "aarch64", target_feature = "aes"),
    all(target_arch = "x86_64", target_feature = "pclmulqdq"),
))]
fn for_each_top_block<F>(data: &mut [Gf128], block_bytes: usize, flat: bool, op: F)
where
    F: Fn(usize, &mut [Gf128]) + Sync + Send,
{
    use rayon::prelude::*;
    if flat && data.len() > block_bytes {
        data.par_chunks_mut(block_bytes)
            .enumerate()
            .for_each(|(block, chunk)| op(block, chunk));
    } else {
        for (block, chunk) in data.chunks_mut(block_bytes).enumerate() {
            op(block, chunk);
        }
    }
}

/// [`butterfly_interleaved_fused_2layer_par_rows`] sourcing its input rows
/// from `src` instead of from `block`'s prior contents (which may be
/// arbitrary/uninit). Each task takes a group of up to 32 quarter-rows,
/// copies the four matching `src` runs into the destination with one bulk
/// copy each, and butterflies the group's rows in place while they are
/// L1-hot (four 8 KB runs at 16 lanes): DRAM sees one write per line, and
/// the standalone replicate pass this replaces disappears. Copying row by
/// row instead (four 256-byte copies per quarter-row, as upstream does)
/// costs more at 1 thread than the replicate pass it saves. `src.len() ==
/// block.len()` (one would-be replica), same quarter-row layout on both
/// sides.
#[cfg(any(
    all(target_arch = "aarch64", target_feature = "aes"),
    all(target_arch = "x86_64", target_feature = "pclmulqdq"),
))]
fn butterfly_interleaved_fused_2layer_par_rows_from_src(
    block: &mut [Gf128],
    src: &[Gf128],
    t_outer: Gf128,
    t_inner_a: Gf128,
    t_inner_b: Gf128,
    quarter: usize,
    num_ntts: usize,
) {
    use rayon::prelude::*;
    const GROUP_ROWS: usize = 32;
    const PARALLEL_ROW_THRESHOLD: usize = 256;
    let stride = quarter * num_ntts;
    debug_assert_eq!(block.len(), 4 * stride);
    debug_assert_eq!(src.len(), 4 * stride);

    let zero_block = t_outer == Gf128::ZERO && t_inner_a == Gf128::ZERO;
    let do_group = |dst: [&mut [Gf128]; 4], srcs: [&[Gf128]; 4]| {
        let [run_a, run_b, run_c, run_d] = dst;
        run_a.copy_from_slice(srcs[0]);
        run_b.copy_from_slice(srcs[1]);
        run_c.copy_from_slice(srcs[2]);
        run_d.copy_from_slice(srcs[3]);
        for (((row_a, row_b), row_c), row_d) in run_a
            .chunks_exact_mut(num_ntts)
            .zip(run_b.chunks_exact_mut(num_ntts))
            .zip(run_c.chunks_exact_mut(num_ntts))
            .zip(run_d.chunks_exact_mut(num_ntts))
        {
            fused_2layer_row_op(
                row_a, row_b, row_c, row_d, t_outer, t_inner_a, t_inner_b, zero_block, num_ntts,
            );
        }
    };

    let (top_half, bot_half) = block.split_at_mut(2 * stride);
    let (q1, q2) = top_half.split_at_mut(stride);
    let (q3, q4) = bot_half.split_at_mut(stride);
    let (s12, s34) = src.split_at(2 * stride);
    let (s1, s2) = s12.split_at(stride);
    let (s3, s4) = s34.split_at(stride);

    // The four quarters have the same length, so their `group` chunks
    // (the last one possibly short) line up.
    let group = GROUP_ROWS.min(quarter).max(1) * num_ntts;
    if quarter < PARALLEL_ROW_THRESHOLD {
        for (((((((a, b), c), d), sa), sb), sc), sd) in q1
            .chunks_mut(group)
            .zip(q2.chunks_mut(group))
            .zip(q3.chunks_mut(group))
            .zip(q4.chunks_mut(group))
            .zip(s1.chunks(group))
            .zip(s2.chunks(group))
            .zip(s3.chunks(group))
            .zip(s4.chunks(group))
        {
            do_group([a, b, c, d], [sa, sb, sc, sd]);
        }
    } else {
        q1.par_chunks_mut(group)
            .zip(q2.par_chunks_mut(group))
            .zip(q3.par_chunks_mut(group))
            .zip(q4.par_chunks_mut(group))
            .zip(s1.par_chunks(group))
            .zip(s2.par_chunks(group))
            .zip(s3.par_chunks(group))
            .zip(s4.par_chunks(group))
            .for_each(|(((((((a, b), c), d), sa), sb), sc), sd)| {
                do_group([a, b, c, d], [sa, sb, sc, sd]);
            });
    }
}

/// Fill `data` with `data.len() / msg.len()` replicas of `msg` — the exact
/// post-copy-layer state of the RS encoding. Fallback for shapes the fused
/// first pass of [`AdditiveNttF128::forward_transform_interleaved_from_message`]
/// cannot serve. Every slot of `data` is written (prior contents arbitrary).
fn replicate_interleaved(data: &mut [Gf128], msg: &[Gf128]) {
    use rayon::prelude::*;
    let msg_len = msg.len();
    debug_assert_eq!(data.len() % msg_len, 0);
    const COPY_CHUNK: usize = 1 << 16;
    if msg_len >= COPY_CHUNK {
        // Both are powers of two, so chunks never straddle a replica boundary.
        data.par_chunks_mut(COPY_CHUNK).enumerate().for_each(|(i, dst)| {
            let src_off = (i * COPY_CHUNK) % msg_len;
            dst.copy_from_slice(&msg[src_off..src_off + dst.len()]);
        });
    } else {
        for rep in data.chunks_mut(msg_len) {
            rep.copy_from_slice(msg);
        }
    }
}

#[cfg(all(target_arch = "aarch64", target_feature = "aes"))]
use field::gf128::kernels::aarch64::mul_polynomial_u64;

/// Butterfly one interleaved block with a shared twiddle. Rows contain
/// `num_ntts` independent lanes. Keep scalar lane dispatch: the historical
/// explicit two-lane NEON experiment regressed by 10–30% by limiting ILP and
/// adding load/store work. That observation is not a new performance result.
fn butterfly_interleaved_block(
    block: &mut [Gf128],
    twiddle: Gf128,
    block_size_half: usize,
    num_ntts: usize,
) {
    let off_bot = block_size_half * num_ntts;
    // Zero-twiddle fast path: block 0 of EVERY layer has twiddle 0
    // (`twiddle(l, 0) = span_get(_, 0) = 0`), so its butterfly degenerates to
    // (u, v + u) — no multiply. Σ_l 2^-l ≈ 1 layer-equivalent ≈ 6% of all NTT
    // mults, and the NTT is mult-throughput-bound.
    if twiddle == Gf128::ZERO {
        for r in 0..block_size_half {
            let off_top = r * num_ntts;
            let off_bot_r = off_top + off_bot;
            for lane in 0..num_ntts {
                let u = block[off_top + lane];
                block[off_bot_r + lane] += u;
            }
        }
        return;
    }
    // Half-width-twiddle fast path (see `mul_polynomial_u64`): the deep layers
    // this kernel serves are exactly where all twiddles are half-width.
    #[cfg(all(target_arch = "aarch64", target_feature = "aes"))]
    if twiddle.hi == 0 {
        for r in 0..block_size_half {
            let off_top = r * num_ntts;
            let off_bot_r = off_top + off_bot;
            for lane in 0..num_ntts {
                let v = block[off_bot_r + lane];
                let new_u = block[off_top + lane] + mul_polynomial_u64(v, twiddle.lo);
                block[off_top + lane] = new_u;
                block[off_bot_r + lane] = v + new_u;
            }
        }
        return;
    }
    let (top, bot) = block.split_at_mut(off_bot);
    for r in 0..block_size_half {
        let o = r * num_ntts;
        kernels::butterfly_row_pair(
            &mut top[o..o + num_ntts],
            &mut bot[o..o + num_ntts],
            twiddle,
        );
    }
}

/// Butterfly one top-layer block, fusing four layers `(L..L+4)`. `block` holds
/// `16 * sixteenth` rows of `num_ntts` lanes; `t` carries the 15 twiddles for
/// the sub-butterflies (see module comment above). Parallel over row groups.
#[inline]
fn butterfly_interleaved_fused_4layer_par_rows(
    block: &mut [Gf128],
    t: &[Gf128; 15],
    sixteenth: usize,
    num_ntts: usize,
) {
    use rayon::prelude::*;
    const PARALLEL_ROW_THRESHOLD: usize = 256;
    debug_assert_eq!(block.len(), 16 * sixteenth * num_ntts);
    // Carry the base as `usize` (Send+Sync) so rayon's per-`r` closure can hold
    // it without a raw-pointer `Sync` shim. Each `r` writes the disjoint rows
    // `{i*sixteenth + r : i ∈ 0..16}`, so concurrent writes never alias.
    let base = block.as_mut_ptr() as usize;
    if sixteenth < PARALLEL_ROW_THRESHOLD {
        for r in 0..sixteenth {
            // SAFETY: row group r writes disjoint rows of this block.
            unsafe {
                kernels::butterfly_fused_4layer_row(base as *mut Gf128, sixteenth, num_ntts, r, t)
            };
        }
    } else {
        (0..sixteenth).into_par_iter().for_each(|r| {
            // SAFETY: distinct r → disjoint row groups → no aliasing.
            unsafe {
                kernels::butterfly_fused_4layer_row(base as *mut Gf128, sixteenth, num_ntts, r, t)
            };
        });
    }
}

#[inline]
fn log2_pow2(n: usize) -> usize {
    assert!(
        n.is_power_of_two() && n > 0,
        "length must be a positive power of 2"
    );
    n.trailing_zeros() as usize
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zero_dimensional_transforms_are_identity() {
        for ntt in [AdditiveNttF128::new(&[]), AdditiveNttF128::standard(0)] {
            assert_eq!(ntt.log_domain_size(), 0);
            let value = Gf128::new(u64::MAX, 0x1234_5678_9abc_def0);
            let mut scalar = [value];
            ntt.forward_transform(&mut scalar);
            assert_eq!(scalar, [value]);
            ntt.inverse_transform(&mut scalar);
            assert_eq!(scalar, [value]);
            for lanes in [1, 2, 8, 32] {
                let input: Vec<_> = (0..lanes).map(|i| Gf128::new(i as u64, u64::MAX)).collect();
                let mut output = input.clone();
                ntt.forward_transform_interleaved(&mut output, lanes);
                assert_eq!(output, input);
            }
        }
    }

    struct Rng(u64);
    impl Rng {
        fn new(seed: u64) -> Self {
            Self(seed)
        }
        fn next_u64(&mut self) -> u64 {
            self.0 = self.0.wrapping_add(0x9E3779B97F4A7C15);
            let mut z = self.0;
            z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
            z = (z ^ (z >> 27)).wrapping_mul(0x94D049BB133111EB);
            z ^ (z >> 31)
        }
        fn f128(&mut self) -> Gf128 {
            Gf128 {
                lo: self.next_u64(),
                hi: self.next_u64(),
            }
        }
    }

    fn rand_vec(rng: &mut Rng, n: usize) -> Vec<Gf128> {
        (0..n).map(|_| rng.f128()).collect()
    }

    #[test]
    fn forward_inverse_roundtrip() {
        let mut rng = Rng::new(0xAB1);
        for log_d in [1usize, 2, 3, 4, 6, 8] {
            let ntt = AdditiveNttF128::standard(log_d);
            let original = rand_vec(&mut rng, 1 << log_d);
            let mut v = original.clone();
            ntt.forward_transform(&mut v);
            ntt.inverse_transform(&mut v);
            assert_eq!(v, original, "roundtrip failed at log_d={log_d}");
        }
    }

    #[test]
    fn inverse_forward_roundtrip() {
        let mut rng = Rng::new(0xAB2);
        for log_d in [1usize, 2, 3, 4, 6, 8] {
            let ntt = AdditiveNttF128::standard(log_d);
            let original = rand_vec(&mut rng, 1 << log_d);
            let mut v = original.clone();
            ntt.inverse_transform(&mut v);
            ntt.forward_transform(&mut v);
            assert_eq!(
                v, original,
                "inverse∘forward roundtrip failed at log_d={log_d}"
            );
        }
    }

    #[test]
    fn forward_is_linear() {
        let mut rng = Rng::new(0xAB3);
        for log_d in [1usize, 2, 3, 5] {
            let ntt = AdditiveNttF128::standard(log_d);
            let n = 1 << log_d;
            let a = rand_vec(&mut rng, n);
            let b = rand_vec(&mut rng, n);
            let ab: Vec<Gf128> = a.iter().zip(&b).map(|(x, y)| *x + *y).collect();

            let mut fa = a.clone();
            ntt.forward_transform(&mut fa);
            let mut fb = b.clone();
            ntt.forward_transform(&mut fb);
            let mut fab = ab.clone();
            ntt.forward_transform(&mut fab);

            for i in 0..n {
                assert_eq!(
                    fa[i] + fb[i],
                    fab[i],
                    "linearity fails at i={i}, log_d={log_d}"
                );
            }
        }
    }

    #[test]
    fn ntt_of_zero_is_zero() {
        for log_d in [1usize, 2, 3, 6] {
            let ntt = AdditiveNttF128::standard(log_d);
            let mut v = vec![Gf128::ZERO; 1 << log_d];
            ntt.forward_transform(&mut v);
            assert!(v.iter().all(|&x| x == Gf128::ZERO));
        }
    }

    #[test]
    fn twiddle_at_layer_0_uses_full_basis_minus_one() {
        // At layer 0 (topmost forward butterfly), there's 1 block.
        // twiddle(0, 0) = 0 (no bits set in block index 0).
        let ntt = AdditiveNttF128::standard(4);
        assert_eq!(ntt.twiddle(0, 0), Gf128::ZERO);
    }

    /// At layer log_d - 1 (deepest, where FRI starts), pairs are adjacent.
    /// twiddle should match the "domain points" indexing.
    #[cfg(all(target_arch = "aarch64", target_feature = "aes"))]
    #[test]
    fn neon_matches_scalar() {
        let mut rng = Rng::new(0xBB1);
        for log_d in 1..=10 {
            let ntt = AdditiveNttF128::standard(log_d);
            let original = rand_vec(&mut rng, 1 << log_d);
            let mut v_scalar = original.clone();
            ntt.forward_transform_scalar(&mut v_scalar);
            let mut v_neon = original.clone();
            ntt.forward_transform_neon(&mut v_neon);
            assert_eq!(
                v_neon, v_scalar,
                "NEON disagrees with scalar at log_d={log_d}"
            );
        }
    }

    #[test]
    fn interleaved_matches_per_lane() {
        let mut rng = Rng::new(0xCC1);
        // For several log_d and num_ntts, verify the interleaved transform
        // matches running the per-lane scalar transform on each sub-NTT.
        for log_d in [3usize, 4, 8] {
            for num_ntts in [1usize, 2, 4, 8] {
                let ntt = AdditiveNttF128::standard(log_d);
                let n_total = (1 << log_d) * num_ntts;
                let original = rand_vec(&mut rng, n_total);

                // Interleaved.
                let mut v_inter = original.clone();
                ntt.forward_transform_interleaved_scalar(&mut v_inter, num_ntts);

                // Reference: per-lane, gather + scalar transform + scatter.
                let mut v_ref = original.clone();
                for lane in 0..num_ntts {
                    let mut sub: Vec<Gf128> = (0..(1 << log_d))
                        .map(|pos| v_ref[pos * num_ntts + lane])
                        .collect();
                    ntt.forward_transform_scalar(&mut sub);
                    for pos in 0..(1 << log_d) {
                        v_ref[pos * num_ntts + lane] = sub[pos];
                    }
                }

                assert_eq!(
                    v_inter, v_ref,
                    "interleaved mismatch at log_d={log_d}, num_ntts={num_ntts}"
                );
            }
        }
    }

    // Runs on both SIMD backends so the x86 PCLMUL and aarch64 NEON parallel
    // paths are each validated against the scalar oracle. AVX-512 builds also
    // exercise the fused-4 top-layer kernel in the larger cases.
    #[cfg(any(
        all(target_arch = "aarch64", target_feature = "aes"),
        all(target_arch = "x86_64", target_feature = "pclmulqdq")
    ))]
    #[test]
    fn interleaved_parallel_matches_scalar() {
        let mut rng = Rng::new(0xCC2);
        for log_d in [4usize, 10, 14, 17, 19] {
            for &num_ntts in &[2usize, 8, 32] {
                let ntt = AdditiveNttF128::standard(log_d);
                let n_total = (1 << log_d) * num_ntts;
                let original = rand_vec(&mut rng, n_total);
                let mut v_scalar = original.clone();
                ntt.forward_transform_interleaved_scalar(&mut v_scalar, num_ntts);
                let mut v_par = original.clone();
                ntt.forward_transform_interleaved_parallel(&mut v_par, num_ntts);
                assert_eq!(
                    v_par, v_scalar,
                    "interleaved parallel mismatch at log_d={log_d}, num_ntts={num_ntts}"
                );
            }
        }
    }

    /// Every combination of the schedule switches.
    fn all_schedules() -> Vec<InterleavedNttSchedule> {
        (0..8u8)
            .map(|bits| InterleavedNttSchedule {
                twiddle_walk: bits & 1 != 0,
                depth_first_tail: bits & 2 != 0,
                flat_top: bits & 4 != 0,
            })
            .collect()
    }

    /// Walking a layer's blocks gives the same twiddles as the span sums,
    /// from any first block, across power-of-two boundaries, to the layer's
    /// last block.
    #[test]
    fn twiddle_walk_matches_span_sums() {
        let ntt = AdditiveNttF128::standard(12);
        for layer in 0..12 {
            let blocks = 1usize << layer;
            for first in [0usize, 1, 3, blocks / 2, blocks.saturating_sub(5), blocks - 1] {
                let first = first.min(blocks - 1);
                let mut walk = ntt.layer_twiddles(layer, first, true);
                let mut span = ntt.layer_twiddles(layer, first, false);
                for block in first..blocks {
                    let expected = ntt.twiddle(layer, block);
                    assert_eq!(walk.take(), expected, "walk layer={layer} block={block}");
                    assert_eq!(span.take(), expected, "span layer={layer} block={block}");
                }
            }
        }
    }

    /// Every schedule of the interleaved transform is byte-identical to the
    /// scalar reference: shapes with fused and single top layers, a fused
    /// deep pass ending on a single layer before the tail, sub-groups whose
    /// first deep layer is already in the tail, and start layers 0..=2, at
    /// 1 and 4 threads (the thread count moves the top/deep split and gates
    /// `flat_top`).
    #[test]
    fn schedules_match_scalar() {
        let mut rng = Rng::new(0x5CED);
        for threads in [1usize, 4] {
            let pool = rayon::ThreadPoolBuilder::new().num_threads(threads).build().unwrap();
            for (log_d, num_ntts, start_layer) in [
                (8usize, 2usize, 0usize),
                (10, 4, 1),
                (12, 1, 1),
                (13, 2, 2),
                (14, 16, 1),
                (15, 8, 1),
                (16, 16, 1),
            ] {
                let ntt = AdditiveNttF128::standard(log_d);
                let original = rand_vec(&mut rng, (1 << log_d) * num_ntts);
                let mut expect = original.clone();
                ntt.forward_transform_interleaved_scalar_from_layer(
                    &mut expect,
                    num_ntts,
                    start_layer,
                );
                for schedule in all_schedules() {
                    let mut got = original.clone();
                    pool.install(|| {
                        ntt.forward_transform_interleaved_from_layer_with(
                            &mut got,
                            num_ntts,
                            start_layer,
                            schedule,
                        )
                    });
                    assert!(
                        got == expect,
                        "{schedule:?} at log_d={log_d} num_ntts={num_ntts} \
                         start={start_layer} threads={threads}"
                    );
                }
            }
        }
    }

    /// The from-source fused pass equals copying the block from `src` and
    /// running the in-place fused pass: quarters below, at and above the
    /// copy-group size and the parallel threshold, a quarter that is not a
    /// multiple of the group, and outer block 0's zero twiddles.
    #[cfg(any(
        all(target_arch = "aarch64", target_feature = "aes"),
        all(target_arch = "x86_64", target_feature = "pclmulqdq"),
    ))]
    #[test]
    fn from_src_pass_matches_copy_then_in_place() {
        let mut rng = Rng::new(0x5EC7);
        for (quarter, num_ntts) in [
            (1usize, 4usize),
            (2, 16),
            (32, 1),
            (48, 2),
            (64, 16),
            (300, 4),
            (1024, 16),
        ] {
            let len = 4 * quarter * num_ntts;
            let src = rand_vec(&mut rng, len);
            let t = rand_vec(&mut rng, 3);
            for (t_outer, t_inner_a) in [(t[0], t[1]), (Gf128::ZERO, Gf128::ZERO)] {
                let mut expect = src.clone();
                butterfly_interleaved_fused_2layer_par_rows(
                    &mut expect,
                    t_outer,
                    t_inner_a,
                    t[2],
                    quarter,
                    num_ntts,
                );
                let mut got = rand_vec(&mut rng, len); // garbage start
                butterfly_interleaved_fused_2layer_par_rows_from_src(
                    &mut got, &src, t_outer, t_inner_a, t[2], quarter, num_ntts,
                );
                assert!(
                    got == expect,
                    "quarter={quarter} num_ntts={num_ntts} zero={}",
                    t_outer == Gf128::ZERO
                );
            }
        }
    }

    /// `from_message` (fused first pass reading the message directly) must
    /// equal replicate + `from_layer` on every shape: fused-eligible large
    /// shapes, fallback-tiny shapes, and both reps=2 and reps=4 blow-ups,
    /// under every schedule. Destination starts as garbage to prove every
    /// slot is written. (Port of upstream flock's test.)
    #[test]
    fn from_message_matches_replicate_then_from_layer() {
        let mut rng = Rng::new(0xF1F1);
        for (log_msg_positions, num_ntts, reps) in [
            (16usize, 32usize, 4usize), // large: fused path (cache_n_top ≥ start+2)
            (16, 32, 2),                // large, reps=2
            (13, 16, 2),                // the commit's lane count
            (12, 8, 4),                 // mid: exercises the parallelism-floor regime
            (6, 4, 4),                  // tiny: fallback replicate path
            (10, 1, 2),                 // single-lane
        ] {
            let log_d = log_msg_positions + log2_pow2(reps);
            let ntt = AdditiveNttF128::standard(log_d);
            let msg = rand_vec(&mut rng, (1 << log_msg_positions) * num_ntts);
            let n_total = (1 << log_d) * num_ntts;

            let mut expect = rand_vec(&mut rng, n_total);
            replicate_interleaved(&mut expect, &msg);
            ntt.forward_transform_interleaved_scalar_from_layer(
                &mut expect,
                num_ntts,
                log2_pow2(reps),
            );

            for schedule in all_schedules() {
                let mut got = rand_vec(&mut rng, n_total); // garbage start
                ntt.forward_transform_interleaved_from_message_with(
                    &mut got,
                    &msg,
                    num_ntts,
                    schedule,
                );
                assert!(
                    got == expect,
                    "from_message mismatch at log_msg={log_msg_positions}, num_ntts={num_ntts}, reps={reps}, {schedule:?}"
                );
            }
        }
    }

    #[cfg(all(target_arch = "aarch64", target_feature = "aes"))]
    #[test]
    fn batched_matches_scalar() {
        let mut rng = Rng::new(0xBB4);
        // Include sizes above the TARGET_SUB_NTT_LOG threshold (17) so we
        // exercise the cache-blocked path.
        for log_d in [4usize, 8, 12, 17, 18, 19, 20] {
            let ntt = AdditiveNttF128::standard(log_d);
            let original = rand_vec(&mut rng, 1 << log_d);
            let mut v_scalar = original.clone();
            ntt.forward_transform_scalar(&mut v_scalar);
            let mut v_batched = original.clone();
            ntt.forward_transform_batched(&mut v_batched);
            assert_eq!(
                v_batched, v_scalar,
                "batched disagrees with scalar at log_d={log_d}"
            );
        }
    }

    #[cfg(all(target_arch = "aarch64", target_feature = "aes"))]
    #[test]
    fn parallel_matches_scalar() {
        let mut rng = Rng::new(0xBB2);
        for log_d in [4usize, 8, 12, 15, 16] {
            let ntt = AdditiveNttF128::standard(log_d);
            let original = rand_vec(&mut rng, 1 << log_d);
            let mut v_scalar = original.clone();
            ntt.forward_transform_scalar(&mut v_scalar);
            let mut v_par = original.clone();
            ntt.forward_transform_parallel(&mut v_par);
            assert_eq!(
                v_par, v_scalar,
                "parallel disagrees with scalar at log_d={log_d}"
            );
        }
    }

    #[test]
    fn deepest_layer_twiddle_count() {
        let log_d = 4;
        let ntt = AdditiveNttF128::standard(log_d);
        // At layer log_d - 1 = 3, there are 2^3 = 8 blocks. twiddle(3, b) for b ∈ 0..8.
        for b in 0..8 {
            let _t = ntt.twiddle(log_d - 1, b);
        }
    }
}

//! Merged product forest — the multi-instance GKR ("tree index as MLE
//! variables") replacing the per-tree layer evals of
//! `crate::piop::lookup::gkr_product` for the f2-int pipeline.
//!
//! The per-tree forest carries 2 K-elements per tree per layer
//! (2·2^s·d ≈ 512 KiB at n=26). Here the 2^s trees' layers form ONE
//! multilinear family L_ℓ over (x ∈ {0,1}^ℓ, c ∈ {0,1}^s) (x = in-tree
//! index, LOW bits; c = tree index, HIGH bits) with the TOP-bit product
//! recursion `L_ℓ(x, c) = L_{ℓ+1}(x, 0, c)·L_{ℓ+1}(x, 1, c)`; the verifier
//! enters by evaluating the published roots' MLE at a random ζ ∈ K^s
//! ITSELF, and each layer is reduced by sumcheck — payload O(s+ℓ) per
//! layer instead of O(2^s).
//!
//! **Layer structure (v3, grouped)**: the layer-ℓ claim `L̂_ℓ(z)` with
//! `z = (z_x ∈ K^ℓ, z_c ∈ K^s)` factors coordinate-wise as
//!
//! ```text
//!   Σ_{x,c} eq(x,z_x)·eq(c,z_c)·E_c(x)·O_c(x)
//!     = Σ_c eq(c,z_c)·[ Σ_x eq(x,z_x)·E_c(x)·O_c(x) ]
//! ```
//!
//! — a shared-`q` multi-group instance of the eq-factored driver
//! ([`prove_eq_inner_sumcheck_mixed`]): **phase A** runs the trees as
//! groups (q = z_x shared, per-group scale eq(c,z_c)) binding the ℓ
//! in-tree variables with the SAME kernels as the per-tree forest
//! (WideMulAcc delayed reduction, the fused single-pair round, and the
//! bit-affine case-LUT leaf round — the leaf layer is never
//! materialised); **phase B** binds the s tree-index variables with one
//! more (tiny) driver call over the per-group finals, which stay
//! prover-internal — nothing per-tree is ever absorbed. A closing
//! child pair + line challenge μ chain to the next layer at
//! `z' = (r_x ++ [μ], r_c)`.
//!
//! Exit claim: `L̂_d(z)` with `z = (z_bj ∈ K^{t+log₂W}, z_c ∈ K^s)`. For
//! the f2-int leaves `1 + M·(A−1)` this gives `e_d − 1 =
//! Σ_{c,bj} eq(c,z_c)·eq(bj,z_bj)·M[c][bj]·A(bj)` — exactly the batched
//! claim the pre-sumcheck consumes, with (z_c, z_bj) replacing the old
//! (ξ, ρ).

use crate::pcs::IntEvalParams;
use crate::piop::sumcheck::eq_factored::{
    EqInnerGroupMixed, GroupBufs, Pair2TauSet, prove_eq_inner_sumcheck_mixed,
};
use crate::piop::sumcheck::{MLSumcheck, SumcheckProof};
use crate::poly::univariate::binary_gf128::BinaryFieldGF128 as Gf;
use crate::poly::utils::{build_eq_x_r_vec, eq_eval};
use crate::transcript::traits::Transcript;
use crate::utils::cfg_into_iter;

#[cfg(feature = "parallel")]
use rayon::prelude::*;

/// One merged layer: the phase-A (in-tree variables; `None` at layer 0,
/// which has none) and phase-B (tree-index variables) sumchecks, plus the
/// closing child pair.
#[derive(Clone, Debug)]
pub struct MergedLayer {
    pub sc_x: Option<SumcheckProof<Gf>>,
    pub sc_c: SumcheckProof<Gf>,
    pub pair: (Gf, Gf),
}

/// Merged-forest proof (roots live in the caller's proof object).
#[derive(Clone, Debug)]
pub struct MergedForestProof {
    pub layers: Vec<MergedLayer>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MergedForestError {
    Shape,
    /// A layer's sumcheck rejected or a chaining check failed.
    LayerClaim { layer: usize },
}

#[allow(clippy::arithmetic_side_effects)]
fn absorb_gfs(transcript: &mut impl Transcript, tag: u8, vals: &[Gf]) {
    let mut bytes = Vec::with_capacity(vals.len() * 16 + 1);
    bytes.push(tag);
    for v in vals {
        let w = v.words();
        bytes.extend_from_slice(&w[0].to_le_bytes());
        bytes.extend_from_slice(&w[1].to_le_bytes());
    }
    transcript.absorb_slice(&bytes);
}

/// Evaluate the multilinear with table `tbl` (bit k ↔ point[k]) at `point`.
#[allow(clippy::arithmetic_side_effects)]
fn mle_at(tbl: &[Gf], point: &[Gf]) -> Gf {
    debug_assert_eq!(tbl.len(), 1usize << point.len());
    let mut buf = tbl.to_vec();
    for &r in point {
        let half = buf.len() >> 1;
        for i in 0..half {
            let (u, v) = (buf[2 * i], buf[2 * i + 1]);
            buf[i] = u + r * (u + v);
        }
        buf.truncate(half);
    }
    buf[0]
}

/// Per-tree levels of the upper product tree, LEVEL-major:
/// `levels[ℓ−1][t]` = tree `t`'s level-`ℓ` values as TOP-bit-split
/// `(first half, second half)` — level ℓ has `2^ℓ` values, halves of
/// `2^{ℓ−1}`. Levels run `ℓ = 1..=depth−1`; the leaf level (`2^depth`) is
/// supplied lazily by the layer-(d−1) groups.
type TreeLevels = Vec<Vec<(Vec<Gf>, Vec<Gf>)>>;

/// Parent level's halves from a child's: values `v[i] = L[i]·R[i]`,
/// re-split at the midpoint (top bit of the parent index).
#[allow(clippy::arithmetic_side_effects)]
fn parent_halves_top(l: &[Gf], r: &[Gf]) -> (Vec<Gf>, Vec<Gf>) {
    let n = l.len();
    let h = n >> 1;
    (
        (0..h).map(|i| l[i] * r[i]).collect(),
        (h..n).map(|i| l[i] * r[i]).collect(),
    )
}

/// Build every tree's levels `1..=top` from its level-`top` halves
/// (`gen_top`), parallel across trees, then regroup LEVEL-major. Also
/// returns the roots. `top = depth` materialises everything (eager);
/// `top < depth` leaves the upper levels to bit-driven layers.
#[allow(clippy::arithmetic_side_effects)]
fn build_levels(
    num_trees: usize,
    top: usize,
    gen_top: impl Fn(usize) -> (Vec<Gf>, Vec<Gf>) + Sync,
) -> (TreeLevels, Vec<Gf>) {
    // Per-tree chains [level top, top−1, …, 1], parallel across trees.
    let _g = crate::utils::prof::scope("mf:build_levels");
    let chains: Vec<Vec<(Vec<Gf>, Vec<Gf>)>> = cfg_into_iter!(0..num_trees)
        .map(|c| {
            let mut chain = Vec::with_capacity(top);
            chain.push(gen_top(c));
            for _ in 1..top {
                let last = chain.last().expect("non-empty chain");
                let parent = parent_halves_top(&last.0, &last.1);
                chain.push(parent);
            }
            chain
        })
        .collect();
    let mut levels: TreeLevels = (0..top).map(|_| Vec::with_capacity(num_trees)).collect();
    let mut roots = Vec::with_capacity(num_trees);
    for chain in chains {
        {
            let l1 = chain.last().expect("chain has level 1");
            roots.push(l1.0[0] * l1.1[0]);
        }
        for (i, lvl) in chain.into_iter().enumerate() {
            // chain[i] = level (top−i)  →  slot (top−1−i).
            levels[top - 1 - i].push(lvl);
        }
    }
    (levels, roots)
}

/// Level-(d−2) values straight from one tree's TRANSPOSED leaf-bit halves
/// and the 16-case 4-leaf table: position `j` selects
/// `t4[(j≪4) | (cE≪2) | cO]` with `cE = lbits.bit(j) | rbits.bit(j)≪1` and
/// `cO = lbits.bit(j+q1) | rbits.bit(j+q1)≪1` — leaves `{j, j+q2}` live in
/// the E/O halves at offset `j`, `{j+q1, j+q1+q2}` at offset `j+q1`.
/// Reading the per-tree packed halves word-wise costs 4 u64 loads per 64
/// values — ~64× less read traffic than per-bit `packed_col_bit` gathers
/// over the column-lane store, which touch one full word per bit (the
/// build/JIT gen was the dominant residual read stream at big n).
#[allow(clippy::arithmetic_side_effects)]
fn t4_level_values(lbits: &[u64], rbits: &[u64], t4: &[Gf], q1: usize) -> Vec<Gf> {
    let mut out = Vec::with_capacity(q1);
    if q1 >= 64 {
        let wq = q1 >> 6;
        for w in 0..wq {
            let le = lbits[w];
            let ro = rbits[w];
            let lo = lbits[w + wq];
            let roo = rbits[w + wq];
            for b in 0..64 {
                let ce = (((le >> b) & 1) | (((ro >> b) & 1) << 1)) as usize;
                let co = (((lo >> b) & 1) | (((roo >> b) & 1) << 1)) as usize;
                out.push(t4[(((w << 6) | b) << 4) | (ce << 2) | co]);
            }
        }
    } else {
        let bit = |bits: &[u64], p: usize| -> usize { ((bits[p >> 6] >> (p & 63)) & 1) as usize };
        for j in 0..q1 {
            let ce = bit(lbits, j) | (bit(rbits, j) << 1);
            let co = bit(lbits, j + q1) | (bit(rbits, j + q1) << 1);
            out.push(t4[(j << 4) | (ce << 2) | co]);
        }
    }
    out
}

/// A layer whose phase-A group buffers come straight from the committed
/// bits (with the shared tau tables the driver's case-LUT rounds consume)
/// instead of a materialised level.
struct BitLayer {
    bufs: Vec<GroupBufs<Gf>>,
    tau_sets: Vec<(Vec<Gf>, Vec<Gf>)>,
    pair_tau_sets: Vec<Pair2TauSet<Gf>>,
    t4_sets: Vec<Vec<Gf>>,
}

/// The shared layer loop: layer ℓ = phase A (trees as groups over the ℓ
/// in-tree variables; skipped at ℓ = 0) + phase B (one group over the s
/// tree-index variables) + the closing pair / line challenge.
/// `bit_layer(ℓ)` may supply layer ℓ's phase-A buffers from the bits
/// (LeafBits / Pair2Bits); every other layer consumes `levels[ℓ]`.
#[allow(clippy::arithmetic_side_effects)]
fn drive_grouped(
    transcript: &mut impl Transcript,
    roots: Vec<Gf>,
    mut levels: TreeLevels,
    mut bit_layer: impl FnMut(usize) -> Option<BitLayer>,
    depth: usize,
    s: usize,
) -> (Vec<Gf>, MergedForestProof, Vec<Gf>, Gf) {
    assert!(depth >= 1 && s >= 1, "merged forest needs depth >= 1, s >= 1");
    let num_trees = roots.len();
    let one = Gf::one();
    absorb_gfs(transcript, 0x30, &roots);
    let zeta: Vec<Gf> = transcript.get_field_challenges(s, &());
    let mut claim = mle_at(&roots, &zeta);

    let mut z_x: Vec<Gf> = Vec::new();
    let mut z_c: Vec<Gf> = zeta;
    let mut out_layers = Vec::with_capacity(depth);
    // Layer ℓ uses level ℓ+1 = levels[ℓ] unless bit-driven; consumed via
    // mem::take per slot (front-first order).
    for ell in 0..depth {
        // Phase A: bind the ℓ in-tree variables (ℓ ≥ 1).
        let (sc_x, r_x, e_vec, o_vec) = if ell == 0 {
            let lvl1 = core::mem::take(&mut levels[0]);
            let mut e = Vec::with_capacity(num_trees);
            let mut o = Vec::with_capacity(num_trees);
            for (l, r) in lvl1 {
                e.push(l[0]);
                o.push(r[0]);
            }
            (None, Vec::new(), e, o)
        } else {
            let _g = crate::utils::prof::scope("mf:phaseA");
            let eq_zc = build_eq_x_r_vec(&z_c, &()).expect("s >= 1");
            let (groups, tau_sets, pair_tau_sets, t4_sets) = if let Some(bl) = {
                let _g = crate::utils::prof::scope("mf:bitgen");
                bit_layer(ell)
            } {
                let groups = bl
                    .bufs
                    .into_iter()
                    .zip(eq_zc.iter())
                    .map(|(bufs, &scale)| EqInnerGroupMixed { q: z_x.clone(), scale, bufs })
                    .collect::<Vec<_>>();
                (groups, bl.tau_sets, bl.pair_tau_sets, bl.t4_sets)
            } else {
                let lvl = core::mem::take(&mut levels[ell]);
                let groups = lvl
                    .into_iter()
                    .zip(eq_zc.iter())
                    .map(|(pair, &scale)| EqInnerGroupMixed {
                        q: z_x.clone(),
                        scale,
                        bufs: GroupBufs::Dense(vec![pair]),
                    })
                    .collect::<Vec<_>>();
                (groups, Vec::new(), Vec::new(), Vec::new())
            };
            let (sc, r_x, finals) = prove_eq_inner_sumcheck_mixed(
                transcript,
                groups,
                &tau_sets,
                &pair_tau_sets,
                &t4_sets,
                &(),
            );
            let mut e = Vec::with_capacity(num_trees);
            let mut o = Vec::with_capacity(num_trees);
            for f in finals {
                let (fe, fo) = f[0];
                e.push(fe);
                o.push(fo);
            }
            (Some(sc), r_x, e, o)
        };

        // Phase B: bind the s tree-index variables over the per-tree finals.
        let _g = crate::utils::prof::scope("mf:phaseB");
        let group_b = EqInnerGroupMixed {
            q: z_c.clone(),
            scale: one,
            bufs: GroupBufs::Dense(vec![(e_vec, o_vec)]),
        };
        let (sc_c, r_c, finals_b) =
            prove_eq_inner_sumcheck_mixed(transcript, vec![group_b], &[], &[], &[], &());
        let pair = finals_b[0][0];
        drop(_g);

        absorb_gfs(transcript, 0x32, &[pair.0, pair.1]);
        let mu: Gf = transcript.get_field_challenge(&());
        claim = pair.0 + mu * (pair.0 + pair.1);
        let mut nx = r_x;
        nx.push(mu);
        z_x = nx;
        z_c = r_c;
        out_layers.push(MergedLayer { sc_x, sc_c, pair });
    }
    let mut z = z_x;
    z.extend_from_slice(&z_c);
    (roots, MergedForestProof { layers: out_layers }, z, claim)
}

/// Prove all `2^s` per-tree grand products over the flat leaf table
/// (`leaves[i + 2^d·c]`, in-tree index low, tree index high). Returns
/// `(roots, proof, exit_point (len d+s), exit_eval)`.
///
/// Eager reference: materialised leaves in, the leaf layer runs as Dense
/// groups. Byte-identical to [`prove_merged_forest_lazy`] over the same
/// leaf values (the driver's LeafBits round is an exact char-2 identity).
#[allow(clippy::arithmetic_side_effects)]
pub fn prove_merged_forest(
    transcript: &mut impl Transcript,
    leaves: &[Gf],
    depth: usize,
    s: usize,
) -> (Vec<Gf>, MergedForestProof, Vec<Gf>, Gf) {
    let num_trees = 1usize << s;
    assert_eq!(leaves.len(), 1usize << (depth + s), "leaf table shape");
    assert!(depth >= 1, "depth must be positive");
    let per = 1usize << depth;
    let half = per >> 1;
    let leaf_halves = |c: usize| -> (Vec<Gf>, Vec<Gf>) {
        let base = c << depth;
        (leaves[base..base + half].to_vec(), leaves[base + half..base + per].to_vec())
    };
    // Everything materialised: the leaf level is `levels[depth−1]`.
    let (levels, roots) = build_levels(num_trees, depth, leaf_halves);
    drive_grouped(transcript, roots, levels, |_| None, depth, s)
}

/// Lazy bit-affine merged-forest prover over the committed bits: the leaf
/// layer AND the first product level (d−1) are never materialised, and at
/// depth ≥ 4 the stored chain tops out at level d−3 (≈ L/4 instead of L/2)
/// — level d−3 is generated per tree straight from the lane-packed bits by
/// TOP-paired products of the 16-case 4-leaf table `T4`, level d−2 is
/// regenerated JIT (Dense) at its own layer, layer d−2's sumcheck runs the
/// driver's [`GroupBufs::Pair2Bits`] case-LUT round, and layer d−1's runs
/// the two-round bit-affine [`GroupBufs::Leaf2Bits`] round (dense buffers
/// only after round 2, ≈ L/4) — all over the SAME per-tree leaf bit halves
/// + shared tau tables. Under [`forest_schedule_l8`] (opt-in) every bottom
/// layer goes ONE round deeper from bits (T4Bits / Pair3Bits / Leaf3Bits,
/// build top d−4) — every stage ≈ L/8. `pow2` is
/// [`crate::pcs::chunk_pow2_table`]'s per-row α-power chains.
#[allow(clippy::arithmetic_side_effects)]
pub fn prove_merged_forest_lazy(
    transcript: &mut impl Transcript,
    p: &IntEvalParams,
    packed_cols: &[Vec<u64>],
    pow2: &[Vec<Gf>],
) -> (Vec<Gf>, MergedForestProof, Vec<Gf>, Gf) {
    prove_merged_forest_lazy_sched(transcript, p, packed_cols, pow2, forest_schedule_l8())
}

/// The forest schedule knob: `F2_FOREST_SCHEDULE=l8` opts into the L/8
/// schedule (one more bit-driven round per bottom layer + build top d−4 —
/// peaks halve again, at a measured +5–13 % prove cost at the 2-col model
/// shapes); unset or anything else keeps the **L/4 default** (exactly the
/// shipped `077b4b1` path). Flip it for memory-bound runs — at nv=23
/// (2^17 compressions) the model projects ~2.6 GB under L/8 vs ~4.5 GB
/// under L/4. Read once per prove call.
fn forest_schedule_l8() -> bool {
    std::env::var("F2_FOREST_SCHEDULE").is_ok_and(|v| v == "l8")
}

/// Deeper L/4 bit-driven prefixes — the DEFAULT: one more LUT round per
/// bottom layer (Pair2Bits → Pair3Bits, Leaf2Bits → Leaf3Bits; depth ≥ 5)
/// WITHOUT changing the L/4 build top — both LUT materialization residues
/// halve and the following dense cascades start one round smaller (small
/// consistent win at DRAM-scale shapes on top of pass fusion; a wash at
/// cache-adjacent shapes). Byte-identical either way (every variant is an
/// exact char-2 identity, pinned against the eager forest). `F2Z_LUT3=0`
/// opts out. Read once per prove call.
fn forest_lut3() -> bool {
    std::env::var("F2Z_LUT3").map_or(true, |v| v != "0")
}

/// [`prove_merged_forest_lazy`] with the schedule explicit (`l8 = false`
/// → the L/4 default) — the testable entry point; BOTH schedules are
/// pinned byte-identical to the eager prover.
#[allow(clippy::arithmetic_side_effects)]
fn prove_merged_forest_lazy_sched(
    transcript: &mut impl Transcript,
    p: &IntEvalParams,
    packed_cols: &[Vec<u64>],
    pow2: &[Vec<Gf>],
    l8: bool,
) -> (Vec<Gf>, MergedForestProof, Vec<Gf>, Gf) {
    use crate::pcs::{
        build_column_layer1_halves, extract_column_bit_halves, layer1_pair_table,
        leaf_tau_halves,
    };
    let log_w = p.word_bits.trailing_zeros() as usize;
    let mask_w = p.word_bits.wrapping_sub(1);
    let row_len = p.rows() << log_w;
    let depth = row_len.trailing_zeros() as usize;
    let s = p.s;
    let num_trees = p.cols();
    let one = Gf::one();
    if depth < 3 {
        // Tiny trees: the LeafBits round needs k ≥ 2 — materialise.
        let dense: Vec<Gf> = (0..(row_len << s))
            .map(|idx| {
                let (c, i) = (idx >> depth, idx & (row_len - 1));
                if (packed_cols[c >> 6][i] >> (c & 63)) & 1 == 1 {
                    pow2[i >> log_w][i & mask_w]
                } else {
                    one
                }
            })
            .collect();
        return prove_merged_forest(transcript, &dense, depth, s);
    }

    let pair_tbl = layer1_pair_table(p, pow2, log_w, row_len);
    let leaf_tau = leaf_tau_halves(p, pow2, one, log_w, row_len);
    let mut col_bits = Some(extract_column_bit_halves(packed_cols, num_trees, row_len));

    if depth < 4 {
        // Only the leaf layer is bit-driven (Pair2Bits needs k = d−2 ≥ 2).
        let (levels, roots) = build_levels(num_trees, depth - 1, |c| {
            build_column_layer1_halves(p, packed_cols, c, pow2, &pair_tbl, one, log_w, row_len)
        });
        let bit_layer = |ell: usize| -> Option<BitLayer> {
            (ell == depth - 1).then(|| BitLayer {
                bufs: col_bits
                    .take()
                    .expect("leaf bits consumed once")
                    .into_iter()
                    .map(|(lbits, rbits)| GroupBufs::LeafBits { lbits, rbits, tau_set: 0 })
                    .collect(),
                tau_sets: vec![leaf_tau.clone()],
                pair_tau_sets: Vec::new(),
                t4_sets: Vec::new(),
            })
        };
        return drive_grouped(transcript, roots, levels, bit_layer, depth, s);
    }

    // The per-position 4-case VALUE tables of the level-(d−1) products:
    // E-side position y pairs leaves (y, y+2^{d−1}); O-side pairs
    // (y+2^{d−2}, y+3·2^{d−2}). Case = bit_lo | bit_hi≪1; case 3 comes
    // from the shared pair table.
    let q1 = row_len >> 2; // 2^{d−2}
    let q2 = row_len >> 1; // 2^{d−1}
    let v = |i: usize| -> Gf { pow2[i >> log_w][i & mask_w] };
    let build_cases = |base: usize| -> Vec<Gf> {
        let mut t = Vec::with_capacity(q1 << 2);
        for y in 0..q1 {
            let lo = base + y;
            t.push(one);
            t.push(v(lo));
            t.push(v(lo + q2));
            t.push(pair_tbl[lo]);
        }
        t
    };
    let te = build_cases(0);
    let to = build_cases(q1);

    // 4-leaf product table for building level d−2 straight from the bits:
    // T4[y≪4 | (cE≪2|cO)] = te[4y+cE]·to[4y+cO]. Collect the INDEXED
    // Vec<[Gf; 16]> and flatten in place: a parallel `.flatten()` here
    // treats every 16-entry row as its own nested parallel iterator and
    // the collect goes unindexed — measured ~836× slower than the same
    // arithmetic serially (upstream zinc-plus fix `e19b0e1`, 2026-07-16).
    let t4: Vec<Gf> = {
        let rows: Vec<[Gf; 16]> = cfg_into_iter!(0..q1, 1 << 10)
            .map(|y| {
                let mut row = [Gf::one(); 16];
                for (c, slot) in row.iter_mut().enumerate() {
                    *slot = te[(y << 2) | (c >> 2)] * to[(y << 2) | (c & 3)];
                }
                row
            })
            .collect();
        rows.into_flattened()
    };

    if depth == 4 || !l8 {
        // The L/4 schedule — the DEFAULT (and forced at depth 4, where
        // the deeper chain degenerates: T4Bits needs k = d−3 ≥ 2,
        // Leaf3Bits k = d−1 ≥ 4): stored chain tops at level d−3 via
        // TOP-paired T4 products, level d−2 JIT, then Pair2Bits /
        // Leaf2Bits.
        let h3 = q1 >> 1; // level d−3 positions = 2^{d−3}
        let (levels, roots) = build_levels(num_trees, depth - 3, |c| {
            let cb = col_bits.as_ref().expect("leaf bits alive for the build");
            let (lb, rb) = &cb[c];
            let full = t4_level_values(lb, rb, &t4, q1);
            let v3 = |y: usize| -> Gf { full[y] * full[y + h3] };
            let hh = h3 >> 1;
            ((0..hh).map(v3).collect(), (hh..h3).map(v3).collect())
        });
        let mut t4 = t4;
        let bit_layer = |ell: usize| -> Option<BitLayer> {
            if ell == depth - 3 {
                // JIT: regenerate level d−2 (Dense) per tree from the
                // bits + T4 — alive only while this layer runs.
                let hh = q1 >> 1;
                let cb = col_bits.as_ref().expect("leaf bits alive for the JIT regen");
                let bufs: Vec<GroupBufs<Gf>> = cfg_into_iter!(0..num_trees)
                    .map(|c| {
                        let (lb, rb) = &cb[c];
                        // Exact-capacity halves: these buffers live (and
                        // get truncate()-folded, which never releases
                        // capacity) through the whole layer — a split_off
                        // would carry a 2× allocation.
                        let full = t4_level_values(lb, rb, &t4, q1);
                        GroupBufs::Dense(vec![(full[..hh].to_vec(), full[hh..].to_vec())])
                    })
                    .collect();
                t4 = Vec::new();
                Some(BitLayer {
                    bufs,
                    tau_sets: Vec::new(),
                    pair_tau_sets: Vec::new(),
                    t4_sets: Vec::new(),
                })
            } else if ell == depth - 2 {
                // Under `F2Z_LUT3` (depth ≥ 5, so k = d−2 ≥ 3) the pair
                // layer runs one more bit-driven round (Pair3Bits): its
                // materialized residue halves.
                let deep = depth >= 5 && forest_lut3();
                Some(BitLayer {
                    bufs: col_bits
                        .as_ref()
                        .expect("leaf bits alive for the pair layer")
                        .iter()
                        .map(|(lbits, rbits)| {
                            let (lbits, rbits) = (lbits.clone(), rbits.clone());
                            if deep {
                                GroupBufs::Pair3Bits { lbits, rbits, tau_set: 0 }
                            } else {
                                GroupBufs::Pair2Bits { lbits, rbits, tau_set: 0 }
                            }
                        })
                        .collect(),
                    tau_sets: Vec::new(),
                    pair_tau_sets: vec![Pair2TauSet { te: te.clone(), to: to.clone() }],
                    t4_sets: Vec::new(),
                })
            } else if ell == depth - 1 {
                // Two bit-driven rounds (k = d−1 = 3): dense buffers only
                // after round 2. Under `F2Z_LUT3` (depth ≥ 5, so k ≥ 4)
                // three rounds (Leaf3Bits): the leaf residue halves.
                let deep = depth >= 5 && forest_lut3();
                Some(BitLayer {
                    bufs: col_bits
                        .take()
                        .expect("leaf bits consumed once")
                        .into_iter()
                        .map(|(lbits, rbits)| {
                            if deep {
                                GroupBufs::Leaf3Bits { lbits, rbits, tau_set: 0 }
                            } else {
                                GroupBufs::Leaf2Bits { lbits, rbits, tau_set: 0 }
                            }
                        })
                        .collect(),
                    tau_sets: vec![leaf_tau.clone()],
                    pair_tau_sets: Vec::new(),
                    t4_sets: Vec::new(),
                })
            } else {
                None
            }
        };
        return drive_grouped(transcript, roots, levels, bit_layer, depth, s);
    }

    // depth ≥ 5 — the L/8 schedule: the stored chain tops out at level
    // d−4 (TOP-paired products of T4 pairs), layer d−4 JIT-regenerates
    // level d−3, and layers d−3 / d−2 / d−1 run 1 / 2 / 3 bit-driven
    // rounds (T4Bits / Pair3Bits / Leaf3Bits) — every stage's resident
    // set is ≈ L/8. T4 stays alive through the T4Bits layer (moved into
    // its BitLayer, zero-copy).
    let h3 = q1 >> 1; // level d−3 positions = 2^{d−3}
    let h4 = q1 >> 2; // level d−4 positions = 2^{d−4}
    let (levels, roots) = build_levels(num_trees, depth - 4, |c| {
        let cb = col_bits.as_ref().expect("leaf bits alive for the build");
        let (lb, rb) = &cb[c];
        let full = t4_level_values(lb, rb, &t4, q1);
        let v4 =
            |y: usize| -> Gf { (full[y] * full[y + h3]) * (full[y + h4] * full[y + h4 + h3]) };
        let hh = h4 >> 1;
        ((0..hh).map(v4).collect(), (hh..h4).map(v4).collect())
    });
    let mut t4 = t4;

    let bit_layer = |ell: usize| -> Option<BitLayer> {
        if ell == depth - 4 {
            // JIT: regenerate level d−3 (Dense, exact-capacity halves)
            // per tree — TOP-paired T4 products; transient ≈ L/8.
            let hh = h3 >> 1;
            let cb = col_bits.as_ref().expect("leaf bits alive for the JIT regen");
            let bufs: Vec<GroupBufs<Gf>> = cfg_into_iter!(0..num_trees)
                .map(|c| {
                    let (lb, rb) = &cb[c];
                    let full = t4_level_values(lb, rb, &t4, q1);
                    let e: Vec<Gf> = (0..hh).map(|y| full[y] * full[y + h3]).collect();
                    let o: Vec<Gf> = (hh..h3).map(|y| full[y] * full[y + h3]).collect();
                    GroupBufs::Dense(vec![(e, o)])
                })
                .collect();
            Some(BitLayer {
                bufs,
                tau_sets: Vec::new(),
                pair_tau_sets: Vec::new(),
                t4_sets: Vec::new(),
            })
        } else if ell == depth - 3 {
            // One bit-driven round straight off T4 (k = d−3 ≥ 2): the
            // layer's input level is never stored nor regenerated; the
            // fold materialises ≈ L/8.
            let cb = col_bits.as_ref().expect("leaf bits alive for the T4 layer");
            Some(BitLayer {
                bufs: cb
                    .iter()
                    .map(|(lbits, rbits)| GroupBufs::T4Bits {
                        lbits: lbits.clone(),
                        rbits: rbits.clone(),
                        tau_set: 0,
                    })
                    .collect(),
                tau_sets: Vec::new(),
                pair_tau_sets: Vec::new(),
                t4_sets: vec![core::mem::take(&mut t4)],
            })
        } else if ell == depth - 2 {
            // Two bit-driven rounds (k = d−2 ≥ 3): fold materialises ≈ L/8.
            Some(BitLayer {
                bufs: col_bits
                    .as_ref()
                    .expect("leaf bits alive for the pair layer")
                    .iter()
                    .map(|(lbits, rbits)| GroupBufs::Pair3Bits {
                        lbits: lbits.clone(),
                        rbits: rbits.clone(),
                        tau_set: 0,
                    })
                    .collect(),
                tau_sets: Vec::new(),
                pair_tau_sets: vec![Pair2TauSet { te: te.clone(), to: to.clone() }],
                t4_sets: Vec::new(),
            })
        } else if ell == depth - 1 {
            // Three bit-driven rounds (k = d−1 ≥ 4): the leaf-round set
            // is ≈ L/8.
            Some(BitLayer {
                bufs: col_bits
                    .take()
                    .expect("leaf bits consumed once")
                    .into_iter()
                    .map(|(lbits, rbits)| GroupBufs::Leaf3Bits { lbits, rbits, tau_set: 0 })
                    .collect(),
                tau_sets: vec![leaf_tau.clone()],
                pair_tau_sets: Vec::new(),
                t4_sets: Vec::new(),
            })
        } else {
            None
        }
    };
    drive_grouped(transcript, roots, levels, bit_layer, depth, s)
}

/// Multi-claim batched lazy prover: `claims.len()` same-shape claims —
/// each `2^s` trees of the same depth over its OWN lane-packed bits and
/// τ chains `(packed_cols, pow2)` — run as ONE merged forest of
/// `N·2^s` trees (tree index `(n ≪ s) | c`, claim index HIGH), sharing
/// every layer's sumcheck rounds and messages. Per-claim τ selection
/// rides the driver's existing `tau_set` index — the kernels are
/// untouched. `N` must be a power of two (callers pad with zero-weight
/// claims: their τ chains are all 1, so their leaves are identically 1
/// whatever bits they carry) and the depth ≥ 4 (the deployed x shapes
/// have `t' ≥ 6`). Honours the [`forest_schedule_l8`] knob (L/4 default).
#[allow(clippy::arithmetic_side_effects)]
pub fn prove_merged_forest_lazy_multi(
    transcript: &mut impl Transcript,
    p: &IntEvalParams,
    claims: &[(&[Vec<u64>], &[Vec<Gf>])],
) -> (Vec<Gf>, MergedForestProof, Vec<Gf>, Gf) {
    prove_merged_forest_lazy_multi_sched(transcript, p, claims, forest_schedule_l8())
}

/// [`prove_merged_forest_lazy_multi`] with the schedule explicit — the
/// testable entry point; both schedules pinned byte-identical to eager.
#[allow(clippy::arithmetic_side_effects)]
fn prove_merged_forest_lazy_multi_sched(
    transcript: &mut impl Transcript,
    p: &IntEvalParams,
    claims: &[(&[Vec<u64>], &[Vec<Gf>])],
    l8: bool,
) -> (Vec<Gf>, MergedForestProof, Vec<Gf>, Gf) {
    use crate::pcs::{extract_column_bit_halves, layer1_pair_table, leaf_tau_halves};
    let n_claims = claims.len();
    assert!(n_claims.is_power_of_two(), "pad the claim list to a power of two");
    let log_n = n_claims.trailing_zeros() as usize;
    let log_w = p.word_bits.trailing_zeros() as usize;
    let mask_w = p.word_bits.wrapping_sub(1);
    let row_len = p.rows() << log_w;
    let depth = row_len.trailing_zeros() as usize;
    assert!(depth >= 4, "the batched x prover assumes t' >= 4 (deployed: >= 6)");
    let s = p.s;
    let per = p.cols();
    let num_trees = n_claims * per;
    let s_batch = s + log_n;
    let one = Gf::one();

    // Per-claim tables — the single-claim ones, DEDUPED by τ-chain
    // identity: claims passing the same `pow2` slice (same-point claims,
    // and every zero-weight dummy) share one table set; the driver's
    // `tau_set` field selects per tree via `tab_of`.
    let q1 = row_len >> 2;
    let q2 = row_len >> 1;
    struct TauTables {
        leaf_tau: (Vec<Gf>, Vec<Gf>),
        te: Vec<Gf>,
        to: Vec<Gf>,
        t4: Vec<Gf>,
    }
    let mut uniq_pow2: Vec<(*const Vec<Gf>, usize)> = Vec::new();
    let tab_of: Vec<usize> = claims
        .iter()
        .map(|&(_, pow2)| {
            let key = (pow2.as_ptr(), pow2.len());
            match uniq_pow2.iter().position(|&k| k == key) {
                Some(u) => u,
                None => {
                    uniq_pow2.push(key);
                    uniq_pow2.len() - 1
                }
            }
        })
        .collect();
    let uniq_reps: Vec<usize> = {
        let mut reps = vec![usize::MAX; uniq_pow2.len()];
        for (n, &u) in tab_of.iter().enumerate() {
            if reps[u] == usize::MAX {
                reps[u] = n;
            }
        }
        reps
    };
    drop(uniq_pow2);
    let mut tabs: Vec<TauTables> = uniq_reps
        .iter()
        .map(|&n| {
            let pow2 = claims[n].1;
            let pair_tbl = layer1_pair_table(p, pow2, log_w, row_len);
            let leaf_tau = leaf_tau_halves(p, pow2, one, log_w, row_len);
            let v = |i: usize| -> Gf { pow2[i >> log_w][i & mask_w] };
            let build_cases = |base: usize| -> Vec<Gf> {
                let mut t = Vec::with_capacity(q1 << 2);
                for y in 0..q1 {
                    let lo = base + y;
                    t.push(one);
                    t.push(v(lo));
                    t.push(v(lo + q2));
                    t.push(pair_tbl[lo]);
                }
                t
            };
            let te = build_cases(0);
            let to = build_cases(q1);
            // Indexed collect + in-place flatten — the parallel-`flatten`
            // nested-iterator pathology (see the single prover's t4 build).
            let t4: Vec<Gf> = {
                let rows: Vec<[Gf; 16]> = cfg_into_iter!(0..q1, 1 << 10)
                    .map(|y| {
                        let mut row = [Gf::one(); 16];
                        for (c, slot) in row.iter_mut().enumerate() {
                            *slot = te[(y << 2) | (c >> 2)] * to[(y << 2) | (c & 3)];
                        }
                        row
                    })
                    .collect();
                rows.into_flattened()
            };
            TauTables { leaf_tau, te, to, t4 }
        })
        .collect();
    let mut col_bits_all: Vec<Option<Vec<(Vec<u64>, Vec<u64>)>>> = claims
        .iter()
        .map(|&(packed_cols, _)| Some(extract_column_bit_halves(packed_cols, per, row_len)))
        .collect();

    if depth == 4 || !l8 {
        // The L/4 schedule — the DEFAULT (and forced at depth 4, where
        // the deeper chain degenerates): stored chain tops at level d−3,
        // level d−2 JIT, Pair2Bits / Leaf2Bits.
        let h3 = q1 >> 1; // level d−3 positions = 2^{d−3}
        let (levels, roots) = build_levels(num_trees, depth - 3, |k| {
            let (n, c) = (k >> s, k & (per - 1));
            let t4 = &tabs[tab_of[n]].t4;
            let cb = col_bits_all[n].as_ref().expect("leaf bits alive for the build");
            let (lb, rb) = &cb[c];
            let full = t4_level_values(lb, rb, t4, q1);
            let v3 = |y: usize| -> Gf { full[y] * full[y + h3] };
            let hh = h3 >> 1;
            ((0..hh).map(v3).collect(), (hh..h3).map(v3).collect())
        });
        let bit_layer = |ell: usize| -> Option<BitLayer> {
            if ell == depth - 3 {
                // JIT: regenerate level d−2 (Dense) per tree — alive
                // only while this layer runs.
                let hh = q1 >> 1;
                let bufs: Vec<GroupBufs<Gf>> = cfg_into_iter!(0..num_trees)
                    .map(|k| {
                        let (n, c) = (k >> s, k & (per - 1));
                        let t4 = &tabs[tab_of[n]].t4;
                        let cb = col_bits_all[n]
                            .as_ref()
                            .expect("leaf bits alive for the JIT regen");
                        let (lb, rb) = &cb[c];
                        // Exact-capacity halves: a split_off would carry
                        // 2× allocation through the layer.
                        let full = t4_level_values(lb, rb, t4, q1);
                        GroupBufs::Dense(vec![(full[..hh].to_vec(), full[hh..].to_vec())])
                    })
                    .collect();
                for tab in tabs.iter_mut() {
                    tab.t4 = Vec::new();
                }
                Some(BitLayer {
                    bufs,
                    tau_sets: Vec::new(),
                    pair_tau_sets: Vec::new(),
                    t4_sets: Vec::new(),
                })
            } else if ell == depth - 2 {
                Some(BitLayer {
                    bufs: (0..num_trees)
                        .map(|k| {
                            let (n, c) = (k >> s, k & (per - 1));
                            let cb =
                                col_bits_all[n].as_ref().expect("bits alive for pair layer");
                            GroupBufs::Pair2Bits {
                                lbits: cb[c].0.clone(),
                                rbits: cb[c].1.clone(),
                                tau_set: tab_of[n],
                            }
                        })
                        .collect(),
                    tau_sets: Vec::new(),
                    pair_tau_sets: tabs
                        .iter()
                        .map(|t| Pair2TauSet { te: t.te.clone(), to: t.to.clone() })
                        .collect(),
                    t4_sets: Vec::new(),
                })
            } else if ell == depth - 1 {
                let per_claim_bits: Vec<Vec<(Vec<u64>, Vec<u64>)>> = col_bits_all
                    .iter_mut()
                    .map(|t| t.take().expect("leaf bits consumed once"))
                    .collect();
                // Two bit-driven rounds (k = d−1 = 3): dense buffers only
                // after round 2.
                Some(BitLayer {
                    bufs: per_claim_bits
                        .into_iter()
                        .enumerate()
                        .flat_map(|(n, cb)| {
                            let ts = tab_of[n];
                            cb.into_iter().map(move |(lbits, rbits)| GroupBufs::Leaf2Bits {
                                lbits,
                                rbits,
                                tau_set: ts,
                            })
                        })
                        .collect(),
                    tau_sets: tabs.iter().map(|t| t.leaf_tau.clone()).collect(),
                    pair_tau_sets: Vec::new(),
                    t4_sets: Vec::new(),
                })
            } else {
                None
            }
        };
        return drive_grouped(transcript, roots, levels, bit_layer, depth, s_batch);
    }

    // depth ≥ 5 — the L/8 schedule (see the single prover): stored chain
    // tops at level d−4, layer d−4 JIT-regenerates level d−3, layers
    // d−3 / d−2 / d−1 run T4Bits / Pair3Bits / Leaf3Bits. T4 moves into
    // the T4Bits layer's tau sets (zero-copy) and dies with it.
    let h3 = q1 >> 1; // level d−3 positions = 2^{d−3}
    let h4 = q1 >> 2; // level d−4 positions = 2^{d−4}
    let (levels, roots) = build_levels(num_trees, depth - 4, |k| {
        let (n, c) = (k >> s, k & (per - 1));
        let t4 = &tabs[tab_of[n]].t4;
        let cb = col_bits_all[n].as_ref().expect("leaf bits alive for the build");
        let (lb, rb) = &cb[c];
        let full = t4_level_values(lb, rb, t4, q1);
        let v4 =
            |y: usize| -> Gf { (full[y] * full[y + h3]) * (full[y + h4] * full[y + h4 + h3]) };
        let hh = h4 >> 1;
        ((0..hh).map(v4).collect(), (hh..h4).map(v4).collect())
    });

    let bit_layer = |ell: usize| -> Option<BitLayer> {
        if ell == depth - 4 {
            // JIT: regenerate level d−3 (Dense, exact-capacity halves)
            // per tree — transient ≈ L/8.
            let hh = h3 >> 1;
            let bufs: Vec<GroupBufs<Gf>> = cfg_into_iter!(0..num_trees)
                .map(|k| {
                    let (n, c) = (k >> s, k & (per - 1));
                    let t4 = &tabs[tab_of[n]].t4;
                    let cb =
                        col_bits_all[n].as_ref().expect("leaf bits alive for the JIT regen");
                    let (lb, rb) = &cb[c];
                    let full = t4_level_values(lb, rb, t4, q1);
                    let e: Vec<Gf> = (0..hh).map(|y| full[y] * full[y + h3]).collect();
                    let o: Vec<Gf> = (hh..h3).map(|y| full[y] * full[y + h3]).collect();
                    GroupBufs::Dense(vec![(e, o)])
                })
                .collect();
            Some(BitLayer {
                bufs,
                tau_sets: Vec::new(),
                pair_tau_sets: Vec::new(),
                t4_sets: Vec::new(),
            })
        } else if ell == depth - 3 {
            // One bit-driven round straight off T4 (k = d−3 ≥ 2).
            let bufs: Vec<GroupBufs<Gf>> = (0..num_trees)
                .map(|k| {
                    let (n, c) = (k >> s, k & (per - 1));
                    let cb = col_bits_all[n].as_ref().expect("bits alive for the T4 layer");
                    GroupBufs::T4Bits {
                        lbits: cb[c].0.clone(),
                        rbits: cb[c].1.clone(),
                        tau_set: tab_of[n],
                    }
                })
                .collect();
            Some(BitLayer {
                bufs,
                tau_sets: Vec::new(),
                pair_tau_sets: Vec::new(),
                t4_sets: tabs.iter_mut().map(|t| core::mem::take(&mut t.t4)).collect(),
            })
        } else if ell == depth - 2 {
            // Two bit-driven rounds (k = d−2 ≥ 3).
            Some(BitLayer {
                bufs: (0..num_trees)
                    .map(|k| {
                        let (n, c) = (k >> s, k & (per - 1));
                        let cb = col_bits_all[n].as_ref().expect("bits alive for pair layer");
                        GroupBufs::Pair3Bits {
                            lbits: cb[c].0.clone(),
                            rbits: cb[c].1.clone(),
                            tau_set: tab_of[n],
                        }
                    })
                    .collect(),
                tau_sets: Vec::new(),
                pair_tau_sets: tabs
                    .iter()
                    .map(|t| Pair2TauSet { te: t.te.clone(), to: t.to.clone() })
                    .collect(),
                t4_sets: Vec::new(),
            })
        } else if ell == depth - 1 {
            let per_claim_bits: Vec<Vec<(Vec<u64>, Vec<u64>)>> = col_bits_all
                .iter_mut()
                .map(|t| t.take().expect("leaf bits consumed once"))
                .collect();
            // Three bit-driven rounds (k = d−1 ≥ 4): leaf set ≈ L/8.
            Some(BitLayer {
                bufs: per_claim_bits
                    .into_iter()
                    .enumerate()
                    .flat_map(|(n, cb)| {
                        let ts = tab_of[n];
                        cb.into_iter().map(move |(lbits, rbits)| GroupBufs::Leaf3Bits {
                            lbits,
                            rbits,
                            tau_set: ts,
                        })
                    })
                    .collect(),
                tau_sets: tabs.iter().map(|t| t.leaf_tau.clone()).collect(),
                pair_tau_sets: Vec::new(),
                t4_sets: Vec::new(),
            })
        } else {
            None
        }
    };
    drive_grouped(transcript, roots, levels, bit_layer, depth, s_batch)
}

/// Verify; returns `(exit_point, exit_eval)`. The caller supplies the roots
/// (they are part of the enclosing proof and get absorbed here).
#[allow(clippy::arithmetic_side_effects)]
pub fn verify_merged_forest(
    transcript: &mut impl Transcript,
    roots: &[Gf],
    proof: &MergedForestProof,
    depth: usize,
    s: usize,
) -> Result<(Vec<Gf>, Gf), MergedForestError> {
    if roots.len() != 1usize << s || proof.layers.len() != depth {
        return Err(MergedForestError::Shape);
    }
    let one = Gf::one();
    absorb_gfs(transcript, 0x30, roots);
    let zeta: Vec<Gf> = transcript.get_field_challenges(s, &());
    let mut claim = mle_at(roots, &zeta);

    let mut z_x: Vec<Gf> = Vec::new();
    let mut z_c: Vec<Gf> = zeta;
    for (ell, layer) in proof.layers.iter().enumerate() {
        // Phase A: the ℓ in-tree variables. Its claimed sum must be the
        // running layer claim; its expected evaluation factors as
        // eq(r_x, z_x) · (phase B's claimed sum).
        let r_x = if ell == 0 {
            if layer.sc_x.is_some() {
                return Err(MergedForestError::Shape);
            }
            if layer.sc_c.claimed_sum != claim {
                return Err(MergedForestError::LayerClaim { layer: ell });
            }
            Vec::new()
        } else {
            let sc_x = layer.sc_x.as_ref().ok_or(MergedForestError::Shape)?;
            if sc_x.claimed_sum != claim {
                return Err(MergedForestError::LayerClaim { layer: ell });
            }
            let sub = MLSumcheck::<Gf>::verify_as_subprotocol(transcript, ell, 3, sc_x, &())
                .map_err(|_| MergedForestError::LayerClaim { layer: ell })?;
            let eqx = eq_eval(&sub.point, &z_x, one)
                .map_err(|_| MergedForestError::Shape)?;
            if sub.expected_evaluation != eqx * layer.sc_c.claimed_sum {
                return Err(MergedForestError::LayerClaim { layer: ell });
            }
            sub.point
        };

        // Phase B: the s tree-index variables, closing on the child pair.
        let sub_c = MLSumcheck::<Gf>::verify_as_subprotocol(transcript, s, 3, &layer.sc_c, &())
            .map_err(|_| MergedForestError::LayerClaim { layer: ell })?;
        let (p_, q_) = layer.pair;
        let eqc = eq_eval(&sub_c.point, &z_c, one).map_err(|_| MergedForestError::Shape)?;
        if sub_c.expected_evaluation != eqc * p_ * q_ {
            return Err(MergedForestError::LayerClaim { layer: ell });
        }

        absorb_gfs(transcript, 0x32, &[p_, q_]);
        let mu: Gf = transcript.get_field_challenge(&());
        claim = p_ + mu * (p_ + q_);
        let mut nx = r_x;
        nx.push(mu);
        z_x = nx;
        z_c = sub_c.point;
    }
    let mut z = z_x;
    z.extend_from_slice(&z_c);
    Ok((z, claim))
}

/// Proof bytes: the per-layer sumcheck messages + closing pair.
#[allow(clippy::arithmetic_side_effects)]
pub fn merged_forest_proof_size_bytes(proof: &MergedForestProof) -> usize {
    use crate::transcript::traits::Transcribable;
    let mut n = 0usize;
    for l in &proof.layers {
        if let Some(sc) = &l.sc_x {
            n += sc.get_num_bytes();
        }
        n += l.sc_c.get_num_bytes();
        n += 2 * 16;
    }
    n
}

#[cfg(test)]
#[allow(clippy::arithmetic_side_effects)]
mod tests {
    use super::*;
    use crate::transcript::Blake3Transcript;

    fn sample(seed: u64) -> Gf {
        let hi = seed.wrapping_mul(0x9E37_79B9_7F4A_7C15).rotate_left(29) ^ 0x1234_5678_9ABC_DEF0;
        Gf::from_words([seed ^ 0xA5A5_5A5A_0F0F_F0F0, hi])
    }

    #[test]
    fn merged_forest_roundtrips() {
        for (depth, s) in [(1usize, 2usize), (3, 2), (5, 3), (6, 4)] {
            let leaves: Vec<Gf> =
                (0..(1usize << (depth + s))).map(|i| sample(0x9000 + i as u64)).collect();
            let mut pt = Blake3Transcript::new();
            let (roots, proof, z_p, e_p) = prove_merged_forest(&mut pt, &leaves, depth, s);
            // Roots are the per-tree products.
            for c in 0..(1usize << s) {
                let prod = leaves[c << depth..(c + 1) << depth]
                    .iter()
                    .fold(Gf::one(), |a, &b| a * b);
                assert_eq!(roots[c], prod, "root {c}");
            }
            let mut vt = Blake3Transcript::new();
            let (z_v, e_v) =
                verify_merged_forest(&mut vt, &roots, &proof, depth, s).expect("verify");
            assert_eq!((&z_p, e_p), (&z_v, e_v), "exit claims agree");
            // Exit claim is the leaf MLE at the exit point.
            assert_eq!(mle_at(&leaves, &z_v), e_v, "exit eval");

            // Tampers.
            if depth >= 2 {
                let mut bad = proof.clone();
                if let Some(sc) = &mut bad.layers[1].sc_x {
                    sc.claimed_sum += Gf::one();
                }
                let mut vt = Blake3Transcript::new();
                assert!(verify_merged_forest(&mut vt, &roots, &bad, depth, s).is_err());
            }
            let mut bad = proof.clone();
            bad.layers[0].pair.0 += Gf::one();
            let mut vt = Blake3Transcript::new();
            assert!(verify_merged_forest(&mut vt, &roots, &bad, depth, s).is_err());
            let mut bad = proof.clone();
            bad.layers[depth - 1].sc_c.claimed_sum += Gf::one();
            let mut vt = Blake3Transcript::new();
            assert!(verify_merged_forest(&mut vt, &roots, &bad, depth, s).is_err());
        }
    }

    /// The multi-claim batched prover must be byte-identical to the eager
    /// prover over the concatenated per-claim leaf tables (tree index
    /// `(n ≪ s) | c`, claim high). The last claim's τ chains are all 1 —
    /// the zero-weight DUMMY-padding shape (leaves identically 1).
    #[test]
    fn lazy_multi_matches_eager() {
        use crate::ligerito::pack_columns_from_rows;
        // Depths 4..=8: 4 = the multi prover's minimum (degenerate JIT
        // edge — stored chain tops at level 1), 5 = shallow chain, 6/7 =
        // the deployed x shapes (scalar t4-gen), 8 = the word-wise
        // t4-gen path (q1 = 64).
        for (t, s, n_claims) in
            [(4usize, 2usize, 2usize), (5, 1, 2), (6, 2, 2), (7, 3, 4), (6, 1, 4), (8, 1, 2)]
        {
            let p = IntEvalParams { t, s, word_bits: 1 };
            let row_len = p.rows();
            let depth = t;
            let log_n = n_claims.trailing_zeros() as usize;
            let words = row_len.div_ceil(64);
            let mut rows_all = Vec::with_capacity(n_claims);
            let mut pow2_all = Vec::with_capacity(n_claims);
            for n in 0..n_claims {
                let rows: Vec<Vec<u64>> = (0..p.cols())
                    .map(|c| {
                        (0..words)
                            .map(|wd| {
                                (c as u64 + 7 * n as u64 + 1)
                                    .wrapping_mul(0x9E37_79B9_7F4A_7C15)
                                    .wrapping_add(wd as u64)
                                    .rotate_left((c + 5 * wd + n) as u32 & 63)
                            })
                            .collect()
                    })
                    .collect();
                let pow2: Vec<Vec<Gf>> = (0..p.rows())
                    .map(|b| {
                        if n == n_claims - 1 {
                            vec![Gf::one()] // the dummy-padding shape
                        } else {
                            vec![sample(0xB0B + (n * p.rows() + b) as u64)]
                        }
                    })
                    .collect();
                rows_all.push(rows);
                pow2_all.push(pow2);
            }
            // SAME-WEIGHT sharing: claim 1 uses claim 0's τ chains, passed
            // as the SAME slice reference — the multi prover's table dedup
            // must stay byte-identical to the eager run over those values.
            if n_claims >= 3 {
                pow2_all[1] = pow2_all[0].clone();
            }
            let packed_all: Vec<Vec<Vec<u64>>> =
                rows_all.iter().map(|r| pack_columns_from_rows(&p, r)).collect();

            // Dense concatenated leaves, tree index (n ≪ s) | c.
            let dense: Vec<Gf> = (0..(row_len << (s + log_n)))
                .map(|idx| {
                    let k = idx >> depth;
                    let i = idx & (row_len - 1);
                    let (n, c) = (k >> s, k & ((1 << s) - 1));
                    if (rows_all[n][c][i >> 6] >> (i & 63)) & 1 == 1 {
                        pow2_all[n][i][0]
                    } else {
                        Gf::one()
                    }
                })
                .collect();

            let mut t_eager = Blake3Transcript::new();
            let eager = prove_merged_forest(&mut t_eager, &dense, depth, s + log_n);
            let claim_refs: Vec<(&[Vec<u64>], &[Vec<Gf>])> = packed_all
                .iter()
                .enumerate()
                .map(|(n, pc)| {
                    let pw = if n == 1 && n_claims >= 3 { &pow2_all[0] } else { &pow2_all[n] };
                    (&pc[..], &pw[..])
                })
                .collect();
            let ce: Gf = t_eager.get_field_challenge(&());
            // BOTH schedules must be byte-identical to the eager run
            // (l8 = false → the L/4 default; true → the opt-in L/8,
            // which silently falls back to L/4 at depth 4).
            for l8 in [false, true] {
                let mut t_multi = Blake3Transcript::new();
                let multi =
                    prove_merged_forest_lazy_multi_sched(&mut t_multi, &p, &claim_refs, l8);

                assert_eq!(eager.0, multi.0, "roots (t={t},s={s},N={n_claims},l8={l8})");
                assert_eq!(eager.2, multi.2, "exit point (l8={l8})");
                assert_eq!(eager.3, multi.3, "exit eval (l8={l8})");
                for (k, (le, lm)) in
                    eager.1.layers.iter().zip(multi.1.layers.iter()).enumerate()
                {
                    assert_eq!(le.sc_x, lm.sc_x, "layer {k} sc_x (l8={l8})");
                    assert_eq!(le.sc_c, lm.sc_c, "layer {k} sc_c (l8={l8})");
                    assert_eq!(le.pair, lm.pair, "layer {k} pair (l8={l8})");
                }
                let cm: Gf = t_multi.get_field_challenge(&());
                assert_eq!(ce, cm, "transcript states diverged (l8={l8})");
                // Dummy-shaped claim: roots of the last claim's trees are
                // the products of its ALL-ONES leaves.
                for c in 0..1usize << s {
                    assert_eq!(
                        multi.0[((n_claims - 1) << s) | c],
                        Gf::one(),
                        "dummy root {c} (l8={l8})"
                    );
                }
                let mut vt = Blake3Transcript::new();
                let (z_v, e_v) =
                    verify_merged_forest(&mut vt, &multi.0, &multi.1, depth, s + log_n)
                        .expect("verify multi");
                assert_eq!(z_v, multi.2);
                assert_eq!(e_v, multi.3);
            }
        }
    }

    /// The lazy bit-affine prover must be byte-identical to the eager one
    /// over the same `bit ? α-power : 1` leaves: same roots, same proof
    /// values, same exit claim, same transcript state.
    #[test]
    fn lazy_matches_eager() {
        use crate::ligerito::pack_columns_from_rows;
        // Depths 3..=9 (row_len = 2^t·W): 3 = LeafBits-only fallback; 4 =
        // the degenerate JIT edge (stored chain tops at level 1, Pair2Bits
        // k=2, Leaf2Bits k=3); 5/6 = shallow JIT chains (scalar t4-gen
        // path, q1 < 64); 7 = deep scalar; 8/9 = the WORD-wise t4-gen
        // path (q1 = 64 / 128, one / two block strides).
        for (t, s, w) in [
            (3usize, 1usize, 1usize),
            (4, 2, 1),
            (5, 1, 1),
            (6, 2, 1),
            (7, 3, 1),
            (5, 2, 4),
            (8, 1, 1),
            (9, 2, 1),
        ] {
            let p = IntEvalParams { t, s, word_bits: w };
            let log_w = w.trailing_zeros() as usize;
            let row_len = p.rows() << log_w;
            let depth = row_len.trailing_zeros() as usize;
            // Arbitrary per-row α-power chains (shape only; values free).
            let pow2: Vec<Vec<Gf>> = (0..p.rows())
                .map(|b| (0..w).map(|j| sample(0xA11CE + (b * w + j) as u64)).collect())
                .collect();
            let words = row_len.div_ceil(64);
            let rows: Vec<Vec<u64>> = (0..p.cols())
                .map(|c| {
                    (0..words)
                        .map(|wd| {
                            (c as u64 + 1)
                                .wrapping_mul(0x9E37_79B9_7F4A_7C15)
                                .wrapping_add(wd as u64)
                                .rotate_left((c + 3 * wd) as u32 & 63)
                        })
                        .collect()
                })
                .collect();
            let packed_cols = pack_columns_from_rows(&p, &rows);
            let mask_w = w.wrapping_sub(1);
            let dense: Vec<Gf> = (0..(row_len << s))
                .map(|idx| {
                    let (c, i) = (idx >> depth, idx & (row_len - 1));
                    if (rows[c][i >> 6] >> (i & 63)) & 1 == 1 {
                        pow2[i >> log_w][i & mask_w]
                    } else {
                        Gf::one()
                    }
                })
                .collect();

            let mut t_eager = Blake3Transcript::new();
            let eager = prove_merged_forest(&mut t_eager, &dense, depth, s);
            let ce: Gf = t_eager.get_field_challenge(&());
            // BOTH schedules must be byte-identical (l8 = false → the L/4
            // default; true → opt-in L/8, falling back to L/4 at d ≤ 4).
            let mut last = None;
            for l8 in [false, true] {
                let mut t_lazy = Blake3Transcript::new();
                let lazy =
                    prove_merged_forest_lazy_sched(&mut t_lazy, &p, &packed_cols, &pow2, l8);

                assert_eq!(eager.0, lazy.0, "roots (t={t},s={s},W={w},l8={l8})");
                assert_eq!(eager.2, lazy.2, "exit point (l8={l8})");
                assert_eq!(eager.3, lazy.3, "exit eval (l8={l8})");
                for (k, (le, ll)) in eager.1.layers.iter().zip(lazy.1.layers.iter()).enumerate()
                {
                    assert_eq!(le.sc_x, ll.sc_x, "layer {k} sc_x (l8={l8})");
                    assert_eq!(le.sc_c, ll.sc_c, "layer {k} sc_c (l8={l8})");
                    assert_eq!(le.pair, ll.pair, "layer {k} pair (l8={l8})");
                }
                let cl: Gf = t_lazy.get_field_challenge(&());
                assert_eq!(ce, cl, "transcript states diverged (l8={l8})");
                last = Some(lazy);
            }
            let lazy = last.expect("both schedules ran");

            // And the verifier accepts the lazy proof.
            let mut vt = Blake3Transcript::new();
            let (z_v, e_v) =
                verify_merged_forest(&mut vt, &lazy.0, &lazy.1, depth, s).expect("verify lazy");
            assert_eq!(z_v, lazy.2);
            assert_eq!(e_v, lazy.3);
        }
    }
}

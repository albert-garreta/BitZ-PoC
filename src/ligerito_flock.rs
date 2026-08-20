//! Flock-backed RS opening for the integer-MLE-eval protocol — the
//! **performance backend** of [`crate::f2_int_ligerito`] (feature
//! `flock-pcs`).
//!
//! Everything hot runs flock-core's optimized code (succinctlabs/flock,
//! MIT OR Apache-2.0): the NEON/cache-blocked additive NTT, the SHA-256
//! Merkle commit with octopus multi-proofs, and `pcs::basefold` (at M2:
//! `pcs::ligerito`). zinc keeps the protocol layers around it — the forest
//! GKR, the pre-sumcheck, the (thin) ring-switch orchestration — and the ONE
//! Fiat–Shamir chain is preserved by driving flock's `Challenger` trait from
//! zinc's [`Transcript`] ([`ZincChallenger`]).
//!
//! The two `GF(2^128)` representations are bit-identical (monomial/LSB-first,
//! GHASH reduction `0x87`): `Gf ↔ F128` conversion is a word copy, pinned by
//! the `ntt_matches_flock` differential test, which also serves as the
//! cross-implementation oracle for the in-repo scalar reference.
//!
//! Division of labour per claim `M̂(point) = μ` (from the shared
//! pre-sumcheck):
//! * zinc [`crate::ligerito::ring_switch_prove`]/`_verify` handle the
//!   `s_v` message and the r″ recombination (O(2^{m_p}) — not hot), emitting
//!   the weight table `B(y) = Φ_{r″}(eq(r_hi, y))` and the target `β₀`.
//! * flock `basefold::prove`/`verify` prove `Σ_y P(y)·B(y) = β₀` against
//!   flock's commitment, with `a` = the packed witness (codeword side) and
//!   `b` = the weight table, exactly flock's own PCS wiring
//!   (`pcs.rs::open`). The closing `final_b` check is
//!   [`crate::ligerito::tensor_eq_phi_eval`] — the succinct
//!   tensor-algebra evaluation.
//!
//! Query counts are flock's soundness-pinned `default_fri_queries(rate)`
//! (243 at rate 1/2, 148 at rate 1/4); the `RsOpenConfig::num_queries` knob
//! does not apply on this backend. Note flock's Merkle is SHA-256 without
//! leaf/node domain separation (flagged upstream as non-production) — carried
//! as-is for now; recorded in the ledger.

use flock_core::challenger::Challenger;
use flock_core::field::F128;
use flock_core::ntt::additive_ntt_f128::AdditiveNttF128;
use flock_core::pcs::basefold::{
    self, BaseFoldProof as FlockBaseFoldProof, VerifyError as FlockVerifyError,
    default_fri_queries,
};
use flock_core::pcs::commit::{Commitment, PcsParams, ProverData, commit};
use flock_core::pcs::ligerito::{
    self, LigeritoProof, LigeritoSecurityConfig, ProverConfig as LigProverConfig,
    SoundnessRegime, VerifierConfig as LigVerifierConfig,
};

use crate::piop::lookup::gkr_product::ProductForestProof;
use crate::piop::sumcheck::multi_degree::MultiDegreeSumcheckProof;
use crate::poly::univariate::binary_gf128::BinaryFieldGF128 as Gf;
use crate::transcript::traits::Transcript;

use crate::pcs::{IntEvalParams, ShaF2Layout, final_eval_ring};
use crate::ligerito::{
    IntEvalRsError, LOG_PACKING, PackedBits, RingSwitchProof, RsOpenConfig, RsOpenError,
    packed_vars, phi_byte_tables, phi_from_words, prove_int_eval_common,
    prove_int_eval_merged_common, prove_x_claims_batched_common, repack_leaf_bits,
    residual_b_evals, ring_switch_prove, ring_switch_verify, row_bit_vars, rs_fast,
    sv_fold_mfr, tensor_eq_phi_eval, verify_int_eval_common, verify_int_eval_merged_common,
    verify_x_claims_batched_common,
};
use crate::merged_forest::MergedForestProof;
use crate::taps::TapOp;
use crate::utils::{cfg_chunks_mut, cfg_into_iter, cfg_iter};

#[cfg(feature = "parallel")]
use rayon::prelude::*;

// ---------------------------------------------------------------------
// Field bridging (bit-identical representations)
// ---------------------------------------------------------------------

#[inline]
pub fn gf_to_f128(g: Gf) -> F128 {
    let w = g.words();
    F128 { lo: w[0], hi: w[1] }
}

#[inline]
pub fn f128_to_gf(f: F128) -> Gf {
    Gf::from_words([f.lo, f.hi])
}

/// Word view for the shared fold kernels — flock's packed message is
/// bit-compatible with `Gf` (same GHASH bit convention), so the kernels read
/// it in place, no conversion pass.
impl PackedBits for F128 {
    #[inline(always)]
    fn bit_words(&self) -> [u64; 2] {
        [self.lo, self.hi]
    }
}

fn gf_slice_to_f128(v: &[Gf]) -> Vec<F128> {
    v.iter().map(|&g| gf_to_f128(g)).collect()
}

// ---------------------------------------------------------------------
// Challenger bridge: one Fiat–Shamir chain across zinc + flock layers
// ---------------------------------------------------------------------

/// Drives flock's `Challenger` from a zinc [`Transcript`], so the flock PCS
/// stages chain into the same Fiat–Shamir state as the forest GKR and the
/// pre-sumcheck. Absorbs are framed through `absorb_slice`; challenges come
/// from `get_field_challenge::<Gf>`.
pub struct ZincChallenger<'a, T: Transcript + Send>(pub &'a mut T);

impl<T: Transcript + Send> Challenger for ZincChallenger<'_, T> {
    fn observe_label(&mut self, label: &[u8]) {
        self.0.absorb_slice(label);
    }

    fn observe_f128(&mut self, value: F128) {
        let mut bytes = [0u8; 16];
        bytes[..8].copy_from_slice(&value.lo.to_le_bytes());
        bytes[8..].copy_from_slice(&value.hi.to_le_bytes());
        self.0.absorb_slice(&bytes);
    }

    #[allow(clippy::arithmetic_side_effects)]
    fn observe_f128_slice(&mut self, values: &[F128]) {
        let mut bytes = Vec::with_capacity(values.len() * 16);
        for v in values {
            bytes.extend_from_slice(&v.lo.to_le_bytes());
            bytes.extend_from_slice(&v.hi.to_le_bytes());
        }
        self.0.absorb_slice(&bytes);
    }

    fn observe_bytes(&mut self, bytes: &[u8]) {
        self.0.absorb_slice(bytes);
    }

    fn sample_f128(&mut self) -> F128 {
        let g: Gf = self.0.get_field_challenge(&());
        gf_to_f128(g)
    }

    fn grind_pow(&mut self, bits: u32) -> u64 {
        let seed = self.pow_seed();
        let mut nonce = 0u64;
        loop {
            if pow_ok(&seed, nonce, bits) {
                self.0.absorb_slice(&nonce.to_le_bytes());
                return nonce;
            }
            nonce = nonce.wrapping_add(1);
        }
    }

    fn verify_pow(&mut self, nonce: u64, bits: u32) -> bool {
        let seed = self.pow_seed();
        let ok = pow_ok(&seed, nonce, bits);
        // Absorb regardless, keeping the transcript in lockstep with the
        // prover; an honest verifier rejects on `false` anyway.
        self.0.absorb_slice(&nonce.to_le_bytes());
        ok
    }
}

impl<T: Transcript + Send> ZincChallenger<'_, T> {
    /// PoW seed: one squeezed field element binds the grind to the current
    /// transcript state (prover and verifier squeeze identically).
    fn pow_seed(&mut self) -> [u8; 16] {
        let g: Gf = self.0.get_field_challenge(&());
        let w = g.words();
        let mut seed = [0u8; 16];
        seed[..8].copy_from_slice(&w[0].to_le_bytes());
        seed[8..].copy_from_slice(&w[1].to_le_bytes());
        seed
    }
}

/// `blake3(seed || nonce)` has at least `bits` leading zero bits.
fn pow_ok(seed: &[u8; 16], nonce: u64, bits: u32) -> bool {
    let mut buf = [0u8; 24];
    buf[..16].copy_from_slice(seed);
    buf[16..].copy_from_slice(&nonce.to_le_bytes());
    let hash = blake3::hash(&buf);
    leading_zero_bits(hash.as_bytes()) >= bits
}

fn leading_zero_bits(bytes: &[u8]) -> u32 {
    let mut acc = 0u32;
    for &b in bytes {
        if b == 0 {
            acc = acc.wrapping_add(8);
        } else {
            return acc.wrapping_add(b.leading_zeros());
        }
    }
    acc
}

// ---------------------------------------------------------------------
// Commit
// ---------------------------------------------------------------------

/// Prover-side state of the flock-backed commitment.
pub struct FlockCommitHint {
    /// Per-column bit rows (shared layout with the zinc backend).
    rows: Vec<Vec<u64>>,
    /// Column-lane packing for the branch-native bit-affine forest,
    /// built once at commit.
    packed_cols: Vec<Vec<u64>>,
    /// The packed message in flock representation.
    p_msg: Vec<F128>,
    pub commitment: Commitment,
    prover_data: ProverData,
}

impl FlockCommitHint {
    /// The published root.
    pub fn root(&self) -> &flock_core::merkle::Hash {
        &self.commitment.root
    }

    /// The committed per-column bit rows (row `c` = `2^{t+log₂W}` bits, 64
    /// per word) — e.g. for virtual-XOR row extraction or expected-value
    /// computations in tests/benches.
    pub fn rows(&self) -> &[Vec<u64>] {
        &self.rows
    }
}

impl core::fmt::Debug for FlockCommitHint {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("FlockCommitHint")
            .field("root", &self.commitment.root)
            .field("params", &self.commitment.params)
            .finish_non_exhaustive()
    }
}

/// Shared commit tail: build the packed `F128` message from the per-column
/// bit rows (low 7 row-bit coordinates in-pack, row-bit-high then column
/// bits above) and run flock's PCS commit (NEON interleaved NTT + SHA-256
/// Merkle).
#[allow(clippy::arithmetic_side_effects)]
fn commit_rs_flock_from_rows(
    p: &IntEvalParams,
    rows: Vec<Vec<u64>>,
    packed_cols: Vec<Vec<u64>>,
    log_inv_rate: usize,
    log_batch: usize,
) -> FlockCommitHint {
    let t_w = row_bit_vars(p);
    assert!(t_w >= LOG_PACKING, "packing needs t + log2(W) >= 7");
    let hi_count = 1usize << (t_w - LOG_PACKING);
    let mut p_msg = Vec::with_capacity(hi_count << p.s);
    for row in rows.iter().take(p.cols()) {
        for i_hi in 0..hi_count {
            p_msg.push(F128 { lo: row[2 * i_hi], hi: row[2 * i_hi + 1] });
        }
    }

    let m_p = packed_vars(p);
    assert!(log_batch < m_p, "log_batch must leave at least one position variable");
    let params = PcsParams {
        m: m_p + LOG_PACKING,
        log_inv_rate,
        log_batch_size: log_batch,
        profile: Default::default(),
    };
    let (commitment, prover_data) = commit(&p_msg, &params);
    FlockCommitHint { rows, packed_cols, p_msg, commitment, prover_data }
}

/// Commit the bit data with flock's PCS commit at an explicit
/// `(log_inv_rate, log_batch)` shape, starting from the flat `u128` cell
/// tensor.
pub fn commit_rs_flock_with(
    p: &IntEvalParams,
    data: &[u128],
    log_inv_rate: usize,
    log_batch: usize,
) -> FlockCommitHint {
    let rows = repack_leaf_bits(p, data);
    let packed_cols = crate::ligerito::pack_columns_from_rows(p, &rows);
    commit_rs_flock_from_rows(p, rows, packed_cols, log_inv_rate, log_batch)
}

/// Commit starting from per-column bit rows (the [`repack_leaf_bits`]
/// layout: bit `i = (b<<log₂W)|j` of row `c` = bit `j` of cell `(b,c)`,
/// 64 bits per word) — the column-lane packing is built here and the
/// `u128` cell tensor never exists. This is the memory-honest entry for
/// harnesses/hosts that can produce bits directly: peak stays at the
/// packed scale (`2^n/8` bytes per store) instead of 16 B per cell.
pub fn commit_rs_ligerito_rows(
    p: &IntEvalParams,
    rows: Vec<Vec<u64>>,
    pc: &LigProverConfig,
) -> FlockCommitHint {
    let packed_cols = crate::ligerito::pack_columns_from_rows(p, &rows);
    commit_rs_flock_from_rows(p, rows, packed_cols, pc.log_inv_rates[0], pc.initial_k)
}

/// Commit starting from the 64-column-lane packed store (the layout
/// `sha_f2_packed_cols` builds on the host SHA path) — the per-column rows
/// are rebuilt by 64×64 bit-transposes; the `u128` cell tensor never
/// exists. Shape from the Ligerito prover config.
pub fn commit_rs_ligerito_packed(
    p: &IntEvalParams,
    packed_cols: Vec<Vec<u64>>,
    pc: &LigProverConfig,
) -> FlockCommitHint {
    let rows = crate::ligerito::rows_from_packed_cols(p, &packed_cols);
    commit_rs_flock_from_rows(p, rows, packed_cols, pc.log_inv_rates[0], pc.initial_k)
}

/// The Ligerito config pair for a SHA-shaped opening at `m_p` packed
/// variables. For the deployed regime (`m = m_p + 7 = 22..=35`, i.e. every
/// deployed SHA size `nv ≥ 12`) the default is the best rate-1/2 config we
/// found: a validator-gated Johnson-regime config at base rate 1/2 (`r0 = 1`)
/// with `initial_k = 4` — 183 L0 queries + 16-bit query grinding, a smaller
/// proof than the k=6 Fast profile at the same rate. Below `m = 22` it falls
/// back to the ad-hoc `default_config` (UDR, unaudited — test shapes only).
/// Prover and verifier both derive their config here.
pub fn sha_lig_configs(m_p: usize) -> Result<(LigProverConfig, LigVerifierConfig), String> {
    let m = m_p + LOG_PACKING;
    if m < 22 {
        // Ad-hoc UDR config for small/test shapes (unaudited).
        return lig_configs(m_p, LigConfig::Adhoc { log_batch: 2, log_inv_rate: 2 });
    }
    // Default (m = 22..=35): the best rate-1/2 config we found — Johnson-regime,
    // base rate 1/2 (r0 = 1), initial_k = 4 (183 L0 queries + 16-bit query
    // grinding), built via flock's validator-gated `custom_johnson_config` from
    // the embedded slim TOML template. f2z-pcs reads `initial_k` from the config,
    // so the k = 4 batch needs no extra wiring.
    if ligerito::embedded_security_config(m, ligerito::LigeritoProfile::Slim).is_none() {
        return Err(format!("no embedded ligerito template for m={m}"));
    }
    custom_johnson_config(m, 1, 4).to_prover_verifier_configs()
}

/// [`commit_rs_flock_with`] at the shape in `cfg` (the BaseFold backend's
/// entry point; the Ligerito path derives its shape from the level config —
/// see [`lig_configs`] + [`commit_rs_ligerito`]).
pub fn commit_rs_flock(p: &IntEvalParams, data: &[u128], cfg: &RsOpenConfig) -> FlockCommitHint {
    commit_rs_flock_with(p, data, cfg.log_inv_rate, cfg.log_batch)
}

// ---------------------------------------------------------------------
// Ligerito configs
// ---------------------------------------------------------------------

/// Where the Ligerito level parameters come from.
#[derive(Clone, Copy, Debug)]
pub enum LigConfig {
    /// The audited embedded security config for `m = m_p + 7` at the given
    /// profile (fast/slim/secure). Exists for m = 22..=35 — exactly the
    /// deployed f2-int sizes at W=1 (`m = n`). The commit shape
    /// (`log_inv_rate`, `log_batch = initial_k`) comes from the config.
    Embedded(ligerito::LigeritoProfile),
    /// `ligerito::default_config` for ad-hoc/test shapes (UDR query counts,
    /// no grinding/OOD; per-level parameters not audited).
    Adhoc { log_batch: usize, log_inv_rate: usize },
}

/// Resolve `(ProverConfig, VerifierConfig)` for `m_p` packed variables.
pub fn lig_configs(
    m_p: usize,
    cfg: LigConfig,
) -> Result<(LigProverConfig, LigVerifierConfig), String> {
    match cfg {
        LigConfig::Embedded(profile) => {
            let m = m_p.wrapping_add(LOG_PACKING);
            let toml = ligerito::embedded_security_config(m, profile)
                .ok_or_else(|| format!("no embedded ligerito config for m={m}"))?;
            let sec = LigeritoSecurityConfig::from_toml_str(toml)?;
            sec.to_prover_verifier_configs()
        }
        LigConfig::Adhoc { log_batch, log_inv_rate } => {
            let pc = ligerito::default_config(m_p, log_batch, log_inv_rate)
                .map_err(|e| e.to_string())?;
            let vc = LigVerifierConfig {
                log_inv_rates: pc.log_inv_rates.clone(),
                recursive_steps: pc.recursive_steps,
                initial_log_msg_cols: pc.initial_log_msg_cols,
                initial_log_num_interleaved: pc.initial_log_num_interleaved,
                initial_k: pc.initial_k,
                recursive_log_msg_cols: pc.recursive_log_msg_cols.clone(),
                recursive_ks: pc.recursive_ks.clone(),
                queries: pc.queries.clone(),
                grinding_bits: pc.grinding_bits.clone(),
                fold_grinding_bits: pc.fold_grinding_bits.clone(),
                ood_samples: pc.ood_samples.clone(),
            };
            Ok((pc, vc))
        }
    }
}

/// Build a Johnson-regime Ligerito security config for `(m, base rate
/// `2^-r0`, L0 interleave `2^k0`)` at the embedded profiles' per-level
/// target and query-grinding convention, using flock's own machinery end
/// to end: the embedded slim config as the field template (header strings,
/// `eta`, grinding, target), `scripts/soundness.py`'s ladder rule (rate +1
/// per level, 3-bit folds until the residual is ≤ 5), queries /
/// fold-grinding / OOD solved against
/// [`LigeritoLevelConfig::paper_predicted_bits`] /
/// [`paper_predicted_ood_bits`] — the exact formulas
/// [`LigeritoSecurityConfig::validate`] re-checks — and the whole config
/// gated by `validate()` before it is returned. Nothing hand-picked.
///
/// Used by the bench's `F2Z_LIG_PROFILE=custom:<r0>:<k0>` and by
/// `examples/gen_lig_configs.rs` (which regenerates flock's embedded slim
/// TOMLs at a chosen geometry).
///
/// [`LigeritoLevelConfig::paper_predicted_bits`]: flock_core::pcs::ligerito::LigeritoLevelConfig::paper_predicted_bits
/// [`paper_predicted_ood_bits`]: flock_core::pcs::ligerito::LigeritoLevelConfig::paper_predicted_ood_bits
#[allow(clippy::arithmetic_side_effects, clippy::missing_panics_doc)]
pub fn custom_johnson_config(m: usize, r0: usize, k0: usize) -> LigeritoSecurityConfig {
    custom_johnson_config_bits(m, r0, k0, None)
}

/// [`custom_johnson_config`] with an explicit round-by-round security
/// target (bits). `None` keeps the slim template's target (100). The
/// target is flock's round-by-round notion — total security is the
/// MINIMUM over rounds, the quantity that governs Fiat–Shamir security —
/// and the existing per-level solvers adapt to it unchanged: the query
/// count grows to cover `target − grinding_bits`, `fold_grinding_bits`
/// absorbs the proximity-gap shortfall, and OOD samples escalate until
/// they clear the target on their own. Everything stays gated by flock's
/// `validate()`. Exposed on the CLI/bench as
/// `custom:<log_inv_rate>:<initial_k>:<bits>`.
///
/// Ceiling: the challenge field is `GF(2^128)`, so per-round error terms
/// are floored near `2^-128` minus list-size/length slack — targets much
/// above ~128 fail validation rather than silently degrade.
#[allow(clippy::arithmetic_side_effects, clippy::missing_panics_doc)]
pub fn custom_johnson_config_bits(
    m: usize,
    r0: usize,
    k0: usize,
    target_bits: Option<usize>,
) -> LigeritoSecurityConfig {
    let slim = ligerito::embedded_security_config(m, ligerito::LigeritoProfile::Slim)
        .unwrap_or_else(|| panic!("no embedded slim template for m={m}"));
    let mut cfg = LigeritoSecurityConfig::from_toml_str(slim).expect("slim template validates");
    let log_n = cfg.log_n;
    assert!(k0 >= 1 && k0 < log_n, "custom initial_k out of range");
    if let Some(bits) = target_bits {
        cfg.target_security_bits = bits;
    }
    let tmpl = cfg.levels[0].clone();

    // derive_ladder: (log_msg_cols, log_num_interleaved, k_recursive, rate).
    let mut shapes = vec![(log_n - k0, k0, k0, r0)];
    let mut n_run = log_n - k0;
    let mut rate = r0;
    while n_run > 5 {
        let kr = 3.min(n_run);
        rate += 1;
        shapes.push((n_run - kr, kr, kr, rate));
        n_run -= kr;
    }
    cfg.initial_k = k0;
    cfg.final_block.yr_log_n = n_run;
    cfg.levels = shapes
        .iter()
        .enumerate()
        .map(|(i, &(mc, il, kr, r))| {
            let mut lv = tmpl.clone();
            if let Some(bits) = target_bits {
                lv.target_security_bits = bits;
            }
            lv.log_inv_rate = r;
            lv.log_msg_cols = mc;
            lv.log_num_interleaved = il;
            lv.k_recursive = kr;
            lv.ood_samples = if i == 0 { 0 } else { 1 };
            // Queries: smallest Q whose predicted query-phase bits cover
            // target − query-grinding (validate()'s own gate).
            let need_q = (lv.target_security_bits - lv.grinding_bits) as f64;
            lv.queries = (1..=10_000)
                .find(|&q| {
                    lv.queries = q;
                    lv.paper_predicted_bits().1 + 1e-3 >= need_q
                })
                .expect("query search converges");
            let (pg, qb) = lv.paper_predicted_bits();
            lv.fold_grinding_bits = (lv.target_security_bits as f64 - pg).ceil().max(0.0) as usize;
            lv.expected_eps_pg_bits = pg;
            lv.expected_eps_query_bits = qb;
            // OOD must clear the target on its own. Deeper levels escalate
            // samples; L0 CANNOT (its s = 0 implicit post-commit binding is
            // fixed at `128 − log₂(list) − log₂(μ)` bits — the hard,
            // field-limited ceiling on the round-by-round target). Record
            // L0's bits as-is and let `validate()` report honestly when a
            // requested target exceeds them.
            if i == 0 {
                lv.expected_eps_ood_bits =
                    Some(lv.paper_predicted_ood_bits().expect("johnson_ood prediction"));
            } else {
                loop {
                    let ood = lv.paper_predicted_ood_bits().expect("johnson_ood prediction");
                    if ood + 1e-3 >= lv.target_security_bits as f64 {
                        lv.expected_eps_ood_bits = Some(ood);
                        break;
                    }
                    lv.ood_samples += 1;
                }
            }
            lv
        })
        .collect();
    cfg.validate().expect("custom config passes flock's validator");
    cfg
}

/// Queries-only security: a **UDR-regime** config at the
/// [`custom_johnson_config`] ladder geometry with ZERO grinding of either
/// kind — no query-phase PoW, no fold-challenge PoW — and no OOD samples
/// (the unique-decoding list has size 1, so nothing needs binding). The
/// entire target is paid in codeword queries at the UDR radius
/// `γ = δ/2 − 3/(δ·n)` (≈0.83 bits/query at rate 1/8), so proofs grow
/// where the Johnson configs would instead grind.
///
/// Ceiling: the UDR fold error is `128 − log₂(γ·len + 1)` per level with
/// NOTHING to recover it (that is the point — recovering it is what
/// `fold_grinding_bits` does), so the max round-by-round target is set by
/// the LONGEST codeword (L0): at r0 = 3 ≈115 bits at n = 22, ≈109 at
/// n = 28, shrinking one bit per witness doubling. A target above the
/// ceiling fails flock's `validate()` with the exact shortfall.
///
/// Unlike the Johnson regime, LOWERING the inverse rate RAISES the
/// ceiling (shorter codeword → smaller exceptional set): r0 = 1
/// (rate 1/2) measures 118 bits at n = 22 / 112 at n = 28, with a
/// cheaper commit (2× expansion instead of 8×) and lower peak, paid in
/// per-query bits (0.41 vs 0.83) → ~2× the queries → larger proof.
/// `udr:1:4:<max>` is the highest-security zero-grinding configuration
/// this analysis supports. Exposed on the CLI/bench as
/// `udr:<log_inv_rate>:<initial_k>[:<bits>]`.
#[allow(clippy::arithmetic_side_effects, clippy::missing_panics_doc)]
pub fn custom_udr_config_bits(
    m: usize,
    r0: usize,
    k0: usize,
    target_bits: Option<usize>,
) -> LigeritoSecurityConfig {
    udr_config_impl(m, r0, k0, target_bits, false)
}

/// [`custom_udr_config_bits`] with **fold-grinding allowed**: the per-level
/// proximity-gap shortfall `target − eps_pg` is recovered by PoW on each
/// fold challenge (flock's `fold_grinding_bits`), lifting the UDR ceiling
/// all the way to targets the queries can pay for — including 128. This is
/// CHEAP in UDR, unlike Johnson: the UDR exceptional set is only
/// `γ·len + 1`, so `eps_pg` sits at 112–119 bits at our shapes and the
/// grind is 9–16 bits per fold (µs–ms of hashing), where the Johnson pg
/// (~100 bits) would demand 2^28-class grinds. Queries still cover the
/// FULL target (no query-phase grinding), and there are still no OOD
/// samples. Exposed on the CLI/bench as
/// `udrg:<log_inv_rate>:<initial_k>[:<bits>]`; `udrg:1:4:128` is the
/// 128-bit configuration.
///
/// Honest scope: 128 here means every term flock TRACKS (proximity gap +
/// grind, query phase) clears 2^-128 round-by-round. Untracked
/// field-limited rounds (each degree-d sumcheck message, error ≈ d/2^128)
/// sit at ~126–127 bits — the GF(2^128) floor no parameter escapes.
#[allow(clippy::arithmetic_side_effects, clippy::missing_panics_doc)]
pub fn custom_udr_grind_config_bits(
    m: usize,
    r0: usize,
    k0: usize,
    target_bits: Option<usize>,
) -> LigeritoSecurityConfig {
    udr_config_impl(m, r0, k0, target_bits, true)
}

#[allow(clippy::arithmetic_side_effects)]
fn udr_config_impl(
    m: usize,
    r0: usize,
    k0: usize,
    target_bits: Option<usize>,
    fold_grind: bool,
) -> LigeritoSecurityConfig {
    let slim = ligerito::embedded_security_config(m, ligerito::LigeritoProfile::Slim)
        .unwrap_or_else(|| panic!("no embedded slim template for m={m}"));
    let mut cfg = LigeritoSecurityConfig::from_toml_str(slim).expect("slim template validates");
    let log_n = cfg.log_n;
    assert!(k0 >= 1 && k0 < log_n, "custom initial_k out of range");
    if let Some(bits) = target_bits {
        cfg.target_security_bits = bits;
    }
    let tmpl = cfg.levels[0].clone();

    // Same ladder as `custom_johnson_config`.
    let mut shapes = vec![(log_n - k0, k0, k0, r0)];
    let mut n_run = log_n - k0;
    let mut rate = r0;
    while n_run > 5 {
        let kr = 3.min(n_run);
        rate += 1;
        shapes.push((n_run - kr, kr, kr, rate));
        n_run -= kr;
    }
    cfg.initial_k = k0;
    cfg.final_block.yr_log_n = n_run;
    cfg.levels = shapes
        .iter()
        .map(|&(mc, il, kr, r)| {
            let mut lv = tmpl.clone();
            if let Some(bits) = target_bits {
                lv.target_security_bits = bits;
            }
            lv.log_inv_rate = r;
            lv.log_msg_cols = mc;
            lv.log_num_interleaved = il;
            lv.k_recursive = kr;
            // UDR: no query-phase grinding, no OOD.
            lv.regime = SoundnessRegime::Udr;
            lv.eta = None;
            lv.proximity_loss = Some(0.0);
            lv.grinding_bits = 0;
            lv.fold_grinding_bits = 0;
            lv.ood_samples = 0;
            lv.expected_eps_ood_bits = None;
            // Queries cover the FULL target (no query grinding to offset).
            let need_q = lv.target_security_bits as f64;
            lv.queries = (1..=100_000)
                .find(|&q| {
                    lv.queries = q;
                    lv.paper_predicted_bits().1 + 1e-3 >= need_q
                })
                .expect("query search converges");
            let (pg, qb) = lv.paper_predicted_bits();
            // udrg only: recover the pg shortfall with per-fold PoW
            // (cheap here — pg is 112–119, so the grind is 9–16 bits).
            if fold_grind {
                lv.fold_grinding_bits =
                    (lv.target_security_bits as f64 - pg).ceil().max(0.0) as usize;
            }
            lv.expected_eps_pg_bits = pg;
            lv.expected_eps_query_bits = qb;
            lv
        })
        .collect();
    cfg.validate().expect("custom UDR config passes flock's validator");
    cfg
}

/// Commit at the shape the Ligerito config dictates
/// (`log_inv_rate = log_inv_rates[0]`, `log_batch = initial_k`).
pub fn commit_rs_ligerito(
    p: &IntEvalParams,
    data: &[u128],
    pc: &LigProverConfig,
) -> FlockCommitHint {
    commit_rs_flock_with(p, data, pc.log_inv_rates[0], pc.initial_k)
}

// ---------------------------------------------------------------------
// Open (ring-switch on zinc side, BaseFold on flock side)
// ---------------------------------------------------------------------

/// Proof of one bit-MLE claim through the flock backend.
#[derive(Clone, Debug)]
pub struct FlockRsOpenProof {
    pub ring: RingSwitchProof,
    pub basefold: FlockBaseFoldProof,
}

/// Errors of the flock-backed opening / end-to-end verification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FlockRsError {
    /// A backend-independent stage failed (forest, binding, pre-sumcheck,
    /// read-off — see [`IntEvalRsError`]).
    Common(IntEvalRsError),
    /// The zinc-side ring-switch rejected.
    RingSwitch(RsOpenError),
    /// flock's BaseFold verifier rejected.
    Basefold(FlockVerifyError),
    /// `final_b` disagrees with the succinct weight evaluation
    /// `B̂(challenges)` (the tensor-algebra check).
    FinalWeight,
    /// flock's Ligerito succinct verifier rejected (boolean API — the
    /// failing stage is not surfaced).
    LigeritoReject,
    /// A sent chunk fold failed the free range check `u < 2^{c_w+t+W}`.
    ChunkRange { chunk: usize, col: usize },
    /// The extension-field Step-1 fold table has the wrong shape
    /// (`ext_deg · L₁` chunk-fold vectors of at most `2^s` entries).
    ExtShape,
    /// An extension-field Step-1 coefficient fold failed the free range
    /// check `μ < 2^{c_w+t+W}` (coordinate `coeff`, chunk `chunk`).
    ExtChunkRange { coeff: usize, chunk: usize, col: usize },
    /// The GKR-certified folds disagree with the sent Step-1 polynomials at
    /// the sampled projection point: `⟨bits_c, γ⟩ ≢ μ_c(α') (mod q')`.
    ExtCongruence { col: usize },
    /// The extension-field read-off failed:
    /// `Σ_c v_c^{(2)}·π_canon(μ_c) ≠ μ` over the evaluation field `K`.
    ExtReadOff,
}

/// Prove `M̂(point) = μ` (μ implied by the transcript) through ring-switch +
/// flock BaseFold.
pub fn prove_rs_open_flock(
    transcript: &mut (impl Transcript + Send),
    hint: &FlockCommitHint,
    point: &[Gf],
) -> FlockRsOpenProof {
    let r_hi = &point[LOG_PACKING..];
    // zinc-side ring-switch over the packed message (converted view).
    let p_msg_gf: Vec<Gf> = hint.p_msg.iter().map(|&f| f128_to_gf(f)).collect();
    let (ring, b_tbl, beta0) = ring_switch_prove(transcript, &p_msg_gf, r_hi);

    let params = &hint.commitment.params;
    let ntt = AdditiveNttF128::standard(params.k_code());
    let basefold = basefold::prove(
        &hint.p_msg,
        gf_slice_to_f128(&b_tbl),
        gf_to_f128(beta0),
        &hint.prover_data.codeword,
        &hint.prover_data.merkle_tree,
        &ntt,
        params.log_inv_rate,
        params.log_batch_size,
        default_fri_queries(params.log_inv_rate),
        &mut ZincChallenger(transcript),
    );
    FlockRsOpenProof { ring, basefold }
}

/// Verify `M̂(point) = μ` against the flock commitment.
pub fn verify_rs_open_flock(
    transcript: &mut (impl Transcript + Send),
    commitment: &Commitment,
    mu: Gf,
    point: &[Gf],
    proof: &FlockRsOpenProof,
) -> Result<(), FlockRsError> {
    if point.len() != commitment.params.m {
        return Err(FlockRsError::RingSwitch(RsOpenError::Shape));
    }
    let (r_lo, r_hi) = point.split_at(LOG_PACKING);
    let (eq_r2, beta0) =
        ring_switch_verify(transcript, &proof.ring, mu, r_lo).map_err(FlockRsError::RingSwitch)?;

    let params = &commitment.params;
    let ntt = AdditiveNttF128::standard(params.k_code());
    let challenges = basefold::verify(
        gf_to_f128(beta0),
        &proof.basefold,
        &commitment.root,
        &ntt,
        params.log_inv_rate,
        params.log_batch_size,
        &mut ZincChallenger(transcript),
    )
    .map_err(FlockRsError::Basefold)?;

    // Closing check: final_b == B̂(challenges), succinctly.
    let chals_gf: Vec<Gf> = challenges.iter().map(|&f| f128_to_gf(f)).collect();
    let expected_b = tensor_eq_phi_eval(&chals_gf, r_hi, &eq_r2);
    if f128_to_gf(proof.basefold.final_b) != expected_b {
        return Err(FlockRsError::FinalWeight);
    }
    Ok(())
}

// ---------------------------------------------------------------------
// Ligerito opening (M2): tapered levels, induced bases, grinding, OOD
// ---------------------------------------------------------------------

/// Proof of one bit-MLE claim through ring-switch + flock Ligerito.
#[derive(Clone, Debug)]
pub struct LigOpenProof {
    pub ring: RingSwitchProof,
    pub lig: LigeritoProof,
}

/// Prove `M̂(point) = μ` through ring-switch + flock's recursive Ligerito
/// (`recursive_prover_with_basis` — the arbitrary-basis entry point).
pub fn prove_rs_open_ligerito(
    transcript: &mut (impl Transcript + Send),
    hint: &FlockCommitHint,
    point: &[Gf],
    pc: &LigProverConfig,
) -> LigOpenProof {
    let r_hi = &point[LOG_PACKING..];
    let p_msg_gf: Vec<Gf> = hint.p_msg.iter().map(|&f| f128_to_gf(f)).collect();
    let (ring, b_tbl, beta0) = ring_switch_prove(transcript, &p_msg_gf, r_hi);

    let lig = ligerito::recursive_prover_with_basis(
        pc,
        hint.p_msg.clone(),
        gf_slice_to_f128(&b_tbl),
        gf_to_f128(beta0),
        &hint.prover_data.codeword,
        &hint.prover_data.merkle_tree,
        &mut ZincChallenger(transcript),
    );
    LigOpenProof { ring, lig }
}

/// Verify `M̂(point) = μ` against the flock commitment through the succinct
/// Ligerito verifier. The initial-basis residual evaluations come from
/// [`residual_b_evals`] (shared tensor prefix + boolean tails).
pub fn verify_rs_open_ligerito(
    transcript: &mut (impl Transcript + Send),
    commitment: &Commitment,
    mu: Gf,
    point: &[Gf],
    proof: &LigOpenProof,
    vc: &LigVerifierConfig,
) -> Result<(), FlockRsError> {
    if point.len() != commitment.params.m {
        return Err(FlockRsError::RingSwitch(RsOpenError::Shape));
    }
    let (r_lo, r_hi) = point.split_at(LOG_PACKING);
    let (eq_r2, beta0) =
        ring_switch_verify(transcript, &proof.ring, mu, r_lo).map_err(FlockRsError::RingSwitch)?;

    let m_p = r_hi.len();
    let r_hi_gf: Vec<Gf> = r_hi.to_vec();
    let eval_b = |ris: &[F128], yr_log_n: usize| -> Vec<F128> {
        let ris_gf: Vec<Gf> = ris.iter().map(|&f| f128_to_gf(f)).collect();
        residual_b_evals(&ris_gf, yr_log_n, &r_hi_gf, &eq_r2)
            .into_iter()
            .map(gf_to_f128)
            .collect()
    };
    let ok = ligerito::recursive_verifier_with_basis_succinct(
        vc,
        &proof.lig,
        m_p,
        gf_to_f128(beta0),
        &commitment.root,
        eval_b,
        &mut ZincChallenger(transcript),
    );
    if !ok {
        return Err(FlockRsError::LigeritoReject);
    }
    Ok(())
}

// ---------------------------------------------------------------------
// End-to-end
// ---------------------------------------------------------------------

/// End-to-end proof with the flock-backed opening.
pub struct IntEvalRsFlockProof {
    pub forest: ProductForestProof<Gf>,
    pub v: Vec<u128>,
    pub presum: MultiDegreeSumcheckProof<Gf>,
    pub open: FlockRsOpenProof,
}

/// End-to-end proof with the Ligerito opening. The forest is the MERGED
/// product forest (payload O(Σ(s+k)) K-elements instead of the per-tree
/// `2·2^s·d` child-eval block — the former dominant proof term). The
/// roots are NOT carried: they are `α^{v_c}` by construction, so the
/// verifier derives them from `v` (2^s K-elements saved).
pub struct IntEvalRsLigProof {
    pub mf: MergedForestProof,
    pub v: Vec<u128>,
    pub presum: MultiDegreeSumcheckProof<Gf>,
    pub open: LigOpenProof,
}

/// Prove with the Ligerito opening (merged forest + `v` + pre-sumcheck,
/// then the recursive opening). Reads the committed bits straight from
/// `hint.rows` — no `u128` data tensor.
pub fn prove_rs_ligerito(
    transcript: &mut (impl Transcript + Send),
    hint: &FlockCommitHint,
    p: &IntEvalParams,
    row_weights: &[u128],
    alpha: Gf,
    pc: &LigProverConfig,
) -> IntEvalRsLigProof {
    let (mf, v, presum, point) = prove_int_eval_merged_common(
        transcript,
        p,
        &hint.rows,
        Some(&hint.packed_cols),
        row_weights,
        alpha,
    );
    let open = prove_rs_open_ligerito(transcript, hint, &point, pc);
    IntEvalRsLigProof { mf, v, presum, open }
}

/// Verify with the Ligerito opening.
#[allow(clippy::too_many_arguments)] // mirrors `f2_int_eval::verify`'s surface
pub fn verify_rs_ligerito<R>(
    transcript: &mut (impl Transcript + Send),
    commitment: &Commitment,
    proof: &IntEvalRsLigProof,
    p: &IntEvalParams,
    row_weights: &[u128],
    col_weights: &[R],
    g_r: R,
    alpha: Gf,
    claimed_eval: R,
    vc: &LigVerifierConfig,
) -> Result<(), FlockRsError>
where
    R: Copy + PartialEq + From<u128> + core::ops::Add<Output = R> + core::ops::Mul<Output = R>,
{
    let (point, mu) = verify_int_eval_merged_common(
        transcript,
        &proof.mf,
        &proof.v,
        &proof.presum,
        p,
        row_weights,
        alpha,
    )
    .map_err(FlockRsError::Common)?;

    verify_rs_open_ligerito(transcript, commitment, mu, &point, &proof.open, vc)?;

    let computed = final_eval_ring(&proof.v, col_weights, g_r);
    if computed != claimed_eval {
        return Err(FlockRsError::Common(IntEvalRsError::ReadOff));
    }
    Ok(())
}

// ---------------------------------------------------------------------
// Batched Ligerito opening: L polys, own points, ONE commitment + ONE
// recursion. Slice ℓ of the concatenated packed polynomial (poly index =
// high variables) carries poly ℓ; claim ℓ's big point is
// (r*_ℓ, ξ_ℓ, bits(ℓ)). The L eq-bases are slice-supported, so the
// η-combined basis is built slice-by-slice; the verifier's residual
// closure is Σ_ℓ η_ℓ·residual_b_evals(·, r_hi_ℓ ++ bits(ℓ), eq_r2).
// ---------------------------------------------------------------------

/// Prover-side state of the batched commitment (`L` a power of two).
pub struct FlockBatchCommitHint {
    rows: Vec<Vec<Vec<u64>>>,
    p_msg: Vec<F128>,
    pub commitment: Commitment,
    prover_data: ProverData,
}

/// Commit `L` same-shape polys as one packed vector (slice ℓ at offset
/// `ℓ·2^{m_p}`), at the shape the Ligerito config dictates.
#[allow(clippy::arithmetic_side_effects)]
pub fn commit_rs_ligerito_batch(
    p: &IntEvalParams,
    datas: &[Vec<u128>],
    pc: &LigProverConfig,
) -> FlockBatchCommitHint {
    let l = datas.len();
    assert!(l.is_power_of_two() && l >= 2, "batch size must be a power of two >= 2");
    let t_w = row_bit_vars(p);
    assert!(t_w >= LOG_PACKING);
    let hi_count = 1usize << (t_w - LOG_PACKING);
    let m_p = packed_vars(p);
    let mut rows_all = Vec::with_capacity(l);
    let mut p_msg = Vec::with_capacity(l << m_p);
    for data in datas {
        let rows = repack_leaf_bits(p, data);
        for row in rows.iter().take(p.cols()) {
            for i_hi in 0..hi_count {
                p_msg.push(F128 { lo: row[2 * i_hi], hi: row[2 * i_hi + 1] });
            }
        }
        rows_all.push(rows);
    }
    let log_l = l.trailing_zeros() as usize;
    let params = PcsParams {
        m: m_p + log_l + LOG_PACKING,
        log_inv_rate: pc.log_inv_rates[0],
        log_batch_size: pc.initial_k,
        profile: Default::default(),
    };
    let (commitment, prover_data) = commit(&p_msg, &params);
    FlockBatchCommitHint { rows: rows_all, p_msg, commitment, prover_data }
}

/// End-to-end batched proof (merged forests; roots derived from `vs`).
pub struct IntEvalRsLigBatchProof {
    pub mfs: Vec<MergedForestProof>,
    pub vs: Vec<Vec<u128>>,
    pub presums: Vec<MultiDegreeSumcheckProof<Gf>>,
    pub rings: Vec<RingSwitchProof>,
    pub lig: LigeritoProof,
}

/// The big claim point's suffix for slice ℓ: `r_hi_ℓ ++ bits(ℓ)`
/// (ℓ's bit j at coordinate `m_p − 7 + j`).
#[allow(clippy::arithmetic_side_effects)]
fn big_r_hi(point: &[Gf], ell: usize, log_l: usize) -> Vec<Gf> {
    let mut v = point[LOG_PACKING..].to_vec();
    for j in 0..log_l {
        v.push(if (ell >> j) & 1 == 1 { Gf::one() } else { Gf::zero() });
    }
    v
}

/// Prove `L` integer-MLE evaluations (own points) with one batched Ligerito
/// opening.
#[allow(clippy::arithmetic_side_effects)]
pub fn prove_rs_ligerito_batch(
    transcript: &mut (impl Transcript + Send),
    hint: &FlockBatchCommitHint,
    p: &IntEvalParams,
    row_weights: &[Vec<u128>],
    alpha: Gf,
    pc: &LigProverConfig,
) -> IntEvalRsLigBatchProof {
    let l = hint.rows.len();
    let m_p = packed_vars(p);
    let log_l = l.trailing_zeros() as usize;
    let slice = 1usize << m_p;

    let mut mfs = Vec::with_capacity(l);
    let mut vs = Vec::with_capacity(l);
    let mut presums = Vec::with_capacity(l);
    let mut points = Vec::with_capacity(l);
    for ell in 0..l {
        let (mf, v, ps, pt) = prove_int_eval_merged_common(
            transcript, p, &hint.rows[ell], None, &row_weights[ell], alpha,
        );
        mfs.push(mf);
        vs.push(v);
        presums.push(ps);
        points.push(pt);
    }

    // Per-slice ring-switch messages under ONE later-drawn r″.
    let mut rings = Vec::with_capacity(l);
    let mut eq_his = Vec::with_capacity(l);
    for ell in 0..l {
        let r_hi = &points[ell][LOG_PACKING..];
        let eq_hi = crate::poly::utils::build_eq_x_r_vec(r_hi, &()).expect("r_hi");
        let s = dense_ring_sv(&hint.p_msg[ell * slice..(ell + 1) * slice], &eq_hi);
        crate::ligerito::absorb_sv(transcript, &s);
        rings.push(RingSwitchProof { s_v: s });
        eq_his.push(eq_hi);
    }
    let r2: Vec<Gf> = transcript.get_field_challenges(LOG_PACKING, &());
    let eq_r2 = crate::poly::utils::build_eq_x_r_vec(&r2, &()).expect("r2");
    let etas: Vec<Gf> = transcript.get_field_challenges(l, &());

    // Combined basis (slice-supported) + combined target.
    let mut b_comb = vec![F128::ZERO; l << m_p];
    if rs_fast() {
        for ell in 0..l {
            let tables = phi_byte_tables(&eq_r2, etas[ell]);
            let eq_hi = &eq_his[ell];
            let dst = &mut b_comb[ell * slice..(ell + 1) * slice];
            const CHUNK: usize = 1 << 12;
            cfg_chunks_mut!(dst, CHUNK).enumerate().for_each(|(ci, chunk)| {
                let base = ci * CHUNK;
                for (off, slot) in chunk.iter_mut().enumerate() {
                    *slot = gf_to_f128(phi_from_words(*eq_hi[base + off].words(), &tables));
                }
            });
        }
    } else {
        for ell in 0..l {
            for (y, &ev) in eq_his[ell].iter().enumerate() {
                b_comb[ell * slice + y] = gf_to_f128(etas[ell] * phi_r2(ev, &eq_r2));
            }
        }
    }
    let mut target = Gf::zero();
    for ell in 0..l {
        let s_u = crate::ligerito::transpose_bits_128(&rings[ell].s_v);
        let beta_ell =
            s_u.iter().zip(eq_r2.iter()).fold(Gf::zero(), |a, (su, e)| a + *su * *e);
        target += etas[ell] * beta_ell;
    }

    let lig = ligerito::recursive_prover_with_basis(
        pc,
        hint.p_msg.clone(),
        b_comb,
        gf_to_f128(target),
        &hint.prover_data.codeword,
        &hint.prover_data.merkle_tree,
        &mut ZincChallenger(transcript),
    );
    let _ = log_l;
    IntEvalRsLigBatchProof { mfs, vs, presums, rings, lig }
}

/// Verify `L` integer-MLE evaluations against the batched commitment.
#[allow(clippy::arithmetic_side_effects)]
#[allow(clippy::too_many_arguments)]
pub fn verify_rs_ligerito_batch<R>(
    transcript: &mut (impl Transcript + Send),
    commitment: &Commitment,
    proof: &IntEvalRsLigBatchProof,
    p: &IntEvalParams,
    row_weights: &[Vec<u128>],
    col_weights: &[Vec<R>],
    g_r: &[R],
    alpha: Gf,
    claimed_evals: &[R],
    vc: &LigVerifierConfig,
) -> Result<(), FlockRsError>
where
    R: Copy + PartialEq + From<u128> + core::ops::Add<Output = R> + core::ops::Mul<Output = R>,
{
    let l = proof.mfs.len();
    if !(l.is_power_of_two() && l >= 2)
        || proof.vs.len() != l
        || proof.presums.len() != l
        || proof.rings.len() != l
        || row_weights.len() != l
        || claimed_evals.len() != l
    {
        return Err(FlockRsError::RingSwitch(RsOpenError::Shape));
    }
    let m_p = packed_vars(p);
    let log_l = l.trailing_zeros() as usize;

    let mut points = Vec::with_capacity(l);
    let mut mus = Vec::with_capacity(l);
    for ell in 0..l {
        let (pt, mu) = verify_int_eval_merged_common(
            transcript,
            &proof.mfs[ell],
            &proof.vs[ell],
            &proof.presums[ell],
            p,
            &row_weights[ell],
            alpha,
        )
        .map_err(FlockRsError::Common)?;
        points.push(pt);
        mus.push(mu);
    }

    for ell in 0..l {
        let ring = &proof.rings[ell];
        if ring.s_v.len() != 128 {
            return Err(FlockRsError::RingSwitch(RsOpenError::Shape));
        }
        let eq_lo = crate::poly::utils::build_eq_x_r_vec(&points[ell][..LOG_PACKING], &())
            .expect("r_lo");
        let claim =
            ring.s_v.iter().zip(eq_lo.iter()).fold(Gf::zero(), |a, (s, e)| a + *s * *e);
        if claim != mus[ell] {
            return Err(FlockRsError::RingSwitch(RsOpenError::RingSwitchClaim));
        }
        crate::ligerito::absorb_sv(transcript, &ring.s_v);
    }
    let r2: Vec<Gf> = transcript.get_field_challenges(LOG_PACKING, &());
    let eq_r2 = crate::poly::utils::build_eq_x_r_vec(&r2, &()).expect("r2");
    let etas: Vec<Gf> = transcript.get_field_challenges(l, &());

    let mut target = Gf::zero();
    for ell in 0..l {
        let s_u = crate::ligerito::transpose_bits_128(&proof.rings[ell].s_v);
        let beta_ell =
            s_u.iter().zip(eq_r2.iter()).fold(Gf::zero(), |a, (su, e)| a + *su * *e);
        target += etas[ell] * beta_ell;
    }

    let r_his: Vec<Vec<Gf>> = (0..l).map(|ell| big_r_hi(&points[ell], ell, log_l)).collect();
    let eval_b = |ris: &[F128], yr_log_n: usize| -> Vec<F128> {
        let ris_gf: Vec<Gf> = ris.iter().map(|&f| f128_to_gf(f)).collect();
        let mut out = vec![Gf::zero(); 1usize << yr_log_n];
        for ell in 0..l {
            let blk = residual_b_evals(&ris_gf, yr_log_n, &r_his[ell], &eq_r2);
            for (o, x) in out.iter_mut().zip(blk.iter()) {
                *o += etas[ell] * *x;
            }
        }
        out.into_iter().map(gf_to_f128).collect()
    };
    let ok = ligerito::recursive_verifier_with_basis_succinct(
        vc,
        &proof.lig,
        m_p + log_l,
        gf_to_f128(target),
        &commitment.root,
        eval_b,
        &mut ZincChallenger(transcript),
    );
    if !ok {
        return Err(FlockRsError::LigeritoReject);
    }

    for ell in 0..l {
        let computed = final_eval_ring(&proof.vs[ell], &col_weights[ell], g_r[ell]);
        if computed != claimed_evals[ell] {
            return Err(FlockRsError::Common(IntEvalRsError::ReadOff));
        }
    }
    Ok(())
}

/// `Φ_{r″}` — the F₂-linear batching map on the bit representation:
/// `β_u ↦ eq_r2[u]`, applied to a K element by summing over its set bits.
#[allow(clippy::arithmetic_side_effects)]
#[inline]
fn phi_r2(ev: Gf, eq_r2: &[Gf]) -> Gf {
    let w = ev.words();
    let mut acc = Gf::zero();
    for wi in 0..2usize {
        let mut bits = w[wi];
        while bits != 0 {
            let t = bits.trailing_zeros() as usize;
            acc += eq_r2[(wi << 6) | t];
            bits &= bits.wrapping_sub(1);
        }
    }
    acc
}

/// Dense in-pack marginal `s_v[j] = Σ_y eq_hi[y]·bit_j(P[y])`,
/// chunk-parallel with per-chunk accumulators. Default: the
/// method-of-four-Russians kernel ([`sv_fold_mfr`]); `F2Z_RS_FAST=0`
/// restores the scalar bit scan (byte-identical either way).
#[allow(clippy::arithmetic_side_effects)]
fn dense_ring_sv(p_msg: &[F128], eq_hi: &[Gf]) -> Vec<Gf> {
    if rs_fast() {
        return sv_fold_mfr(p_msg, eq_hi);
    }
    const CHUNK: usize = 1 << 12;
    let n_chunks = p_msg.len().div_ceil(CHUNK).max(1);
    let partials: Vec<Vec<Gf>> = cfg_into_iter!(0..n_chunks)
        .map(|c| {
            let lo = c * CHUNK;
            let hi = (lo + CHUNK).min(p_msg.len());
            let mut s = vec![Gf::zero(); 128];
            for y in lo..hi {
                let e = eq_hi[y];
                let pe = p_msg[y];
                for wi in 0..2usize {
                    let mut bits = if wi == 0 { pe.lo } else { pe.hi };
                    while bits != 0 {
                        let t = bits.trailing_zeros() as usize;
                        s[(wi << 6) | t] += e;
                        bits &= bits.wrapping_sub(1);
                    }
                }
            }
            s
        })
        .collect();
    let mut s = vec![Gf::zero(); 128];
    for part in &partials {
        for (a, b) in s.iter_mut().zip(part.iter()) {
            *a += *b;
        }
    }
    s
}

/// Overwrite `b[y] = Σ_l η_l·Φ_{r″}(eq_his[l][y])` — the η-combined
/// Ligerito basis of the dense (main-chunk) claims — parallel over `y`.
/// Default: η-premultiplied byte-table subset sums ([`phi_byte_tables`],
/// 16 gathers/element, no per-element η multiply); `F2Z_RS_FAST=0` restores
/// the scalar bit scan (byte-identical either way).
#[allow(clippy::arithmetic_side_effects)]
fn fill_phi_basis(b: &mut [F128], eq_his: &[Vec<Gf>], etas: &[Gf], eq_r2: &[Gf]) {
    const CHUNK: usize = 1 << 12;
    if rs_fast() {
        let tables: Vec<Vec<Gf>> = eq_his
            .iter()
            .enumerate()
            .map(|(l, _)| phi_byte_tables(eq_r2, etas[l]))
            .collect();
        cfg_chunks_mut!(b, CHUNK).enumerate().for_each(|(ci, chunk)| {
            let base = ci * CHUNK;
            for (off, slot) in chunk.iter_mut().enumerate() {
                let y = base + off;
                let mut acc = Gf::zero();
                for (l, eq_hi) in eq_his.iter().enumerate() {
                    acc += phi_from_words(*eq_hi[y].words(), &tables[l]);
                }
                *slot = gf_to_f128(acc);
            }
        });
        return;
    }
    cfg_chunks_mut!(b, CHUNK).enumerate().for_each(|(ci, chunk)| {
        let base = ci * CHUNK;
        for (off, slot) in chunk.iter_mut().enumerate() {
            let y = base + off;
            let mut acc = Gf::zero();
            for (l, eq_hi) in eq_his.iter().enumerate() {
                acc += etas[l] * phi_r2(eq_hi[y], eq_r2);
            }
            *slot = gf_to_f128(acc);
        }
    });
}

/// [`fill_phi_basis`] fused with the Ligerito **round-0** sumcheck message:
/// while writing `b`, accumulates `(u_0, u_2) = (Σ_j f[2j]·b[2j],
/// Σ_j (f[2j]+f[2j+1])·(b[2j]+b[2j+1]))` over adjacent pairs — exactly the
/// `(f, b)` message flock's `SumcheckProver::new` (`round_msg_lsb`) would
/// recompute with its own full read pass, which
/// `recursive_prover_with_basis_precomputed_round0` then skips. Products are
/// accumulated with deferred reduction (one reduction per accumulator per
/// chunk; `F₂`-linear, so the values — and every transcript byte — are
/// identical to the unfused entry point).
#[allow(clippy::arithmetic_side_effects)]
fn fill_phi_basis_round0(
    b: &mut [F128],
    f: &[F128],
    eq_his: &[Vec<Gf>],
    etas: &[Gf],
    eq_r2: &[Gf],
) -> (Gf, Gf) {
    use crate::utils::wide_mul::WideMulAcc;
    assert_eq!(b.len(), f.len());
    assert!(b.len() >= 2 && b.len().is_multiple_of(2));
    const CHUNK: usize = 1 << 12; // even ⇒ (2j, 2j+1) pairs never straddle chunks
    let tables: Vec<Vec<Gf>> = eq_his
        .iter()
        .enumerate()
        .map(|(l, _)| phi_byte_tables(eq_r2, etas[l]))
        .collect();
    let partials: Vec<(Gf, Gf)> = cfg_chunks_mut!(b, CHUNK)
        .enumerate()
        .map(|(ci, chunk)| {
            let base = ci * CHUNK;
            for (off, slot) in chunk.iter_mut().enumerate() {
                let y = base + off;
                let mut acc = Gf::zero();
                for (l, eq_hi) in eq_his.iter().enumerate() {
                    acc += phi_from_words(*eq_hi[y].words(), &tables[l]);
                }
                *slot = gf_to_f128(acc);
            }
            let zero = Gf::zero();
            let mut u0 = <Gf as WideMulAcc>::wide_zero(&zero);
            let mut u2 = <Gf as WideMulAcc>::wide_zero(&zero);
            let mut j = 0usize;
            while j + 1 < chunk.len() {
                let b0 = f128_to_gf(chunk[j]);
                let b1 = f128_to_gf(chunk[j + 1]);
                let f0 = f128_to_gf(f[base + j]);
                let f1 = f128_to_gf(f[base + j + 1]);
                <Gf as WideMulAcc>::wide_add_assign(
                    &mut u0,
                    &<Gf as WideMulAcc>::mul_wide(&f0, &b0),
                );
                <Gf as WideMulAcc>::wide_add_assign(
                    &mut u2,
                    &<Gf as WideMulAcc>::mul_wide(&(f0 + f1), &(b0 + b1)),
                );
                j += 2;
            }
            (
                <Gf as WideMulAcc>::from_wide(u0),
                <Gf as WideMulAcc>::from_wide(u2),
            )
        })
        .collect();
    let mut u0 = Gf::zero();
    let mut u2 = Gf::zero();
    for (p0, p2) in &partials {
        u0 += *p0;
        u2 += *p2;
    }
    (u0, u2)
}

// ---------------------------------------------------------------------
// Mod-q MLE evaluation through the Ligerito opener (X-note §6): the ~100-bit
// row weights are chunked into base-2^{c_w} limbs; each chunk l yields its
// own forest + pre-sumcheck claim on the SAME committed P; the L claims are
// η-RLC'd into one Ligerito call; the verifier range-checks every chunk fold
// (u < 2^{c_w+t+W}, which with the α-generator binding pins it) and
// recombines y = Σ_c w′_c·Σ_l 2^{c_w·l}·u_c^{(l)} in 𝔽_q.
// ---------------------------------------------------------------------

/// End-to-end mod-q proof (chunks share one commitment; merged forests;
/// per-chunk roots derived from `us`).
#[derive(Clone)]
pub struct IntEvalRsLigModQProof {
    pub mfs: Vec<MergedForestProof>,
    /// `us[l]` = the 2^s chunk folds `u_c^{(l)}`.
    pub us: Vec<Vec<u128>>,
    pub presums: Vec<MultiDegreeSumcheckProof<Gf>>,
    pub rings: Vec<RingSwitchProof>,
    pub lig: LigeritoProof,
}

/// Prove `MLE[INT(D)](r) = y ∈ 𝔽_q` with the Ligerito opening.
/// `row_weights_q[b] = eq(b, r₁) mod q ∈ [0, q)`. Reads the committed bits
/// straight from `hint.rows` — no `u128` data tensor.
#[allow(clippy::arithmetic_side_effects)]
pub fn prove_mle_eval_mod_q_ligerito(
    transcript: &mut (impl Transcript + Send),
    hint: &FlockCommitHint,
    p: &IntEvalParams,
    row_weights_q: &[u128],
    q_bits: usize,
    alpha: Gf,
    pc: &LigProverConfig,
) -> IntEvalRsLigModQProof {
    use crate::pcs::{chunk_row_weights, mod_q_chunk_width, mod_q_num_chunks};
    let c_w = mod_q_chunk_width(p);
    let lch = mod_q_num_chunks(p, q_bits);
    let chunks = chunk_row_weights(row_weights_q, c_w, lch);

    let mut mfs = Vec::with_capacity(lch);
    let mut us = Vec::with_capacity(lch);
    let mut presums = Vec::with_capacity(lch);
    let mut points = Vec::with_capacity(lch);
    for w_l in &chunks {
        let (mf, u, ps, pt) = prove_int_eval_merged_common(
            transcript,
            p,
            &hint.rows,
            Some(&hint.packed_cols),
            w_l,
            alpha,
        );
        mfs.push(mf);
        us.push(u);
        presums.push(ps);
        points.push(pt);
    }

    // L claims on the SAME packed P: per-claim s_v under one shared r″.
    let mut rings = Vec::with_capacity(lch);
    let mut eq_his = Vec::with_capacity(lch);
    let _g_r = crate::utils::prof::scope("mq:rings");
    for pt in &points {
        let eq_hi = crate::poly::utils::build_eq_x_r_vec(&pt[LOG_PACKING..], &()).expect("r_hi");
        let s = dense_ring_sv(&hint.p_msg, &eq_hi);
        crate::ligerito::absorb_sv(transcript, &s);
        rings.push(RingSwitchProof { s_v: s });
        eq_his.push(eq_hi);
    }
    drop(_g_r);
    let _g_b = crate::utils::prof::scope("mq:bcomb");
    let r2: Vec<Gf> = transcript.get_field_challenges(LOG_PACKING, &());
    let eq_r2 = crate::poly::utils::build_eq_x_r_vec(&r2, &()).expect("r2");
    let etas: Vec<Gf> = transcript.get_field_challenges(lch, &());

    let m_p = packed_vars(p);
    let mut b_comb = vec![F128::ZERO; 1usize << m_p];
    // Fast path: basis fill fused with the Ligerito round-0 message, so the
    // prover entry point skips its own full `(f, b)` read pass.
    let round0 = if rs_fast() {
        Some(fill_phi_basis_round0(&mut b_comb, &hint.p_msg, &eq_his, &etas, &eq_r2))
    } else {
        fill_phi_basis(&mut b_comb, &eq_his, &etas, &eq_r2);
        None
    };
    let mut target = Gf::zero();
    for l in 0..lch {
        let s_u = crate::ligerito::transpose_bits_128(&rings[l].s_v);
        let beta = s_u.iter().zip(eq_r2.iter()).fold(Gf::zero(), |a, (su, e)| a + *su * *e);
        target += etas[l] * beta;
    }
    drop(_g_b);

    let _g_l = crate::utils::prof::scope("mq:lig");
    let lig = match round0 {
        Some((u0, u2)) => ligerito::recursive_prover_with_basis_precomputed_round0(
            pc,
            hint.p_msg.clone(),
            b_comb,
            gf_to_f128(target),
            &hint.prover_data.codeword,
            &hint.prover_data.merkle_tree,
            (gf_to_f128(u0), gf_to_f128(u2)),
            &mut ZincChallenger(transcript),
        ),
        None => ligerito::recursive_prover_with_basis(
            pc,
            hint.p_msg.clone(),
            b_comb,
            gf_to_f128(target),
            &hint.prover_data.codeword,
            &hint.prover_data.merkle_tree,
            &mut ZincChallenger(transcript),
        ),
    };
    drop(_g_l);
    IntEvalRsLigModQProof { mfs, us, presums, rings, lig }
}

/// Verify `MLE[INT(D)](r) = claimed ∈ R` (R char ≠ 2, e.g. 𝔽_q).
/// `col_weights[c] = eq(c, r₂) ∈ R`.
#[allow(clippy::arithmetic_side_effects)]
#[allow(clippy::too_many_arguments)]
pub fn verify_mle_eval_mod_q_ligerito<R>(
    transcript: &mut (impl Transcript + Send),
    commitment: &Commitment,
    proof: &IntEvalRsLigModQProof,
    p: &IntEvalParams,
    row_weights_q: &[u128],
    col_weights: &[R],
    alpha: Gf,
    claimed: R,
    q_bits: usize,
    vc: &LigVerifierConfig,
) -> Result<(), FlockRsError>
where
    R: Copy + PartialEq + From<u128> + core::ops::Add<Output = R> + core::ops::Mul<Output = R>,
{
    use crate::pcs::{mod_q_chunk_width, mod_q_num_chunks, recombine_read_off};
    let us = verify_mod_q_lig_core(
        transcript,
        commitment,
        proof,
        p,
        row_weights_q,
        alpha,
        q_bits,
        vc,
    )?;
    let c_w = mod_q_chunk_width(p);
    let lch = mod_q_num_chunks(p, q_bits);

    // Recombine in R: y = Σ_c w′_c · Σ_l 2^{c_w·l}·u_c^{(l)}.
    let v_flat: Vec<u128> = us.iter().flat_map(|u| u.iter().copied()).collect();
    let y = recombine_read_off(p, &v_flat, 0, col_weights, c_w, lch);
    if y != claimed {
        return Err(FlockRsError::Common(IntEvalRsError::ReadOff));
    }
    Ok(())
}

/// The claim-agnostic core of [`verify_mle_eval_mod_q_ligerito`]: everything
/// that binds the transmitted chunk folds `us` to the committed bits under
/// the row weights (per-chunk range checks, merged forests, pre-sumchecks,
/// ring-switch, the ONE η-batched Ligerito call) — but NOT the final
/// read-off, which differs between the prime-field claim (recombine in `R`
/// against `claimed`) and the extension-field claim (congruence against the
/// Step-1 polynomials mod `q'` plus the `K`-side recombination). Returns
/// the chunk folds re-padded to the full `2^s`.
#[allow(clippy::arithmetic_side_effects)]
#[allow(clippy::too_many_arguments)]
fn verify_mod_q_lig_core(
    transcript: &mut (impl Transcript + Send),
    commitment: &Commitment,
    proof: &IntEvalRsLigModQProof,
    p: &IntEvalParams,
    row_weights_q: &[u128],
    alpha: Gf,
    q_bits: usize,
    vc: &LigVerifierConfig,
) -> Result<Vec<Vec<u128>>, FlockRsError> {
    use crate::pcs::{chunk_row_weights, mod_q_chunk_width, mod_q_num_chunks};
    let c_w = mod_q_chunk_width(p);
    let lch = mod_q_num_chunks(p, q_bits);
    if proof.mfs.len() != lch
        || proof.us.len() != lch
        || proof.presums.len() != lch
        || proof.rings.len() != lch
    {
        return Err(FlockRsError::RingSwitch(RsOpenError::Shape));
    }
    let chunks = chunk_row_weights(row_weights_q, c_w, lch);

    // Re-pad the transmitted chunk folds to the full `2^s`: a padded
    // witness's trailing all-zero columns fold to 0 and are not sent (see
    // `to_bytes`). Forcing those entries to zero is exactly what the
    // prover committed to — the forest binds `root_c = α^{u_c} = 1` for
    // them, and the Ligerito opening then binds the committed bits, so a
    // prover who under-declares the live prefix simply fails.
    let cols = p.cols();
    let mut us: Vec<Vec<u128>> = Vec::with_capacity(lch);
    for u in &proof.us {
        if u.len() > cols {
            return Err(FlockRsError::RingSwitch(RsOpenError::Shape));
        }
        let mut u = u.clone();
        u.resize(cols, 0);
        us.push(u);
    }

    // Per-chunk range bound 2^{c_w+t+W} (c_w+t+W = 127 by construction).
    let range_shift = c_w.wrapping_add(p.t).wrapping_add(p.word_bits);
    let bound = 1u128 << range_shift;

    let mut points = Vec::with_capacity(lch);
    let mut mus = Vec::with_capacity(lch);
    for l in 0..lch {
        for (k, &u) in us[l].iter().enumerate() {
            if u >= bound {
                let _ = k;
                return Err(FlockRsError::ChunkRange { chunk: l, col: k });
            }
        }
        let (pt, mu) = verify_int_eval_merged_common(
            transcript,
            &proof.mfs[l],
            &us[l],
            &proof.presums[l],
            p,
            &chunks[l],
            alpha,
        )
        .map_err(FlockRsError::Common)?;
        points.push(pt);
        mus.push(mu);
    }

    for l in 0..lch {
        let ring = &proof.rings[l];
        if ring.s_v.len() != 128 {
            return Err(FlockRsError::RingSwitch(RsOpenError::Shape));
        }
        let eq_lo =
            crate::poly::utils::build_eq_x_r_vec(&points[l][..LOG_PACKING], &()).expect("r_lo");
        let claim = ring.s_v.iter().zip(eq_lo.iter()).fold(Gf::zero(), |a, (s, e)| a + *s * *e);
        if claim != mus[l] {
            return Err(FlockRsError::RingSwitch(RsOpenError::RingSwitchClaim));
        }
        crate::ligerito::absorb_sv(transcript, &ring.s_v);
    }
    let r2: Vec<Gf> = transcript.get_field_challenges(LOG_PACKING, &());
    let eq_r2 = crate::poly::utils::build_eq_x_r_vec(&r2, &()).expect("r2");
    let etas: Vec<Gf> = transcript.get_field_challenges(lch, &());

    let mut target = Gf::zero();
    for l in 0..lch {
        let s_u = crate::ligerito::transpose_bits_128(&proof.rings[l].s_v);
        let beta = s_u.iter().zip(eq_r2.iter()).fold(Gf::zero(), |a, (su, e)| a + *su * *e);
        target += etas[l] * beta;
    }

    let m_p = packed_vars(p);
    let r_his: Vec<Vec<Gf>> = points.iter().map(|pt| pt[LOG_PACKING..].to_vec()).collect();
    let eval_b = |ris: &[F128], yr_log_n: usize| -> Vec<F128> {
        let ris_gf: Vec<Gf> = ris.iter().map(|&f| f128_to_gf(f)).collect();
        let mut out = vec![Gf::zero(); 1usize << yr_log_n];
        for l in 0..lch {
            let blk = residual_b_evals(&ris_gf, yr_log_n, &r_his[l], &eq_r2);
            for (o, x) in out.iter_mut().zip(blk.iter()) {
                *o += etas[l] * *x;
            }
        }
        out.into_iter().map(gf_to_f128).collect()
    };
    let ok = ligerito::recursive_verifier_with_basis_succinct(
        vc,
        &proof.lig,
        m_p,
        gf_to_f128(target),
        &commitment.root,
        eval_b,
        &mut ZincChallenger(transcript),
    );
    if !ok {
        return Err(FlockRsError::LigeritoReject);
    }

    Ok(us)
}

// ---------------------------------------------------------------------
// EXTENSION-FIELD evaluation claims (paper `c:core_iop`, Steps 1–3): open
// `⟨π_q(bits), v⟩ = μ ∈ K` for an extension field `K = F_q[X]/(h(X))` of
// degree `e ≥ 2`. The row weights' canonical lift `π_canon^{-1}(v^{(1)})`
// is now a vector of integer POLYNOMIALS (degree < e, coefficients in
// `[0, q)`), so the claim cannot ride one exponent fold directly. Instead:
//
//   Step 1  the prover sends, per basis coordinate `d`, the exact
//           integer chunk folds
//           `μ_{c,d}^{(l)} = ⟨bits_c, chunk_l(coords_d)⟩ < 2^{c_w+t+W}`
//           (the coefficients of `μ_c ∈ ℤ[X]` in base-2^{c_w} digits) —
//           NOT GKR-certified, just absorbed into the transcript;
//   Step 3  both sides sample a random prime `q'` and a point
//           `α' ∈ F_{q'}` from the transcript and project the weights,
//           `γ = (Σ_d coords_d·α'^d) mod q'` — the extension-field
//           replacement for the plain `π_q^{-1}(v^{(1)})` lift;
//   Steps 4–6  the ORDINARY mod-q' opening runs on `γ` (same forests,
//           pre-sumchecks, ring-switch, one Ligerito call).
//
// The verifier accepts iff the mod-q' core accepts AND
//   (A) per column: `Σ_l 2^{c_w·l}·u_c^{(l)} ≡ μ_c(α') (mod q')` — the
//       certified folds pin the sent polynomials via Schwartz–Zippel over
//       the random `(q', α')` (paper `l:reduction_lemma`); and
//   (B) `Σ_c v_c^{(2)}·π_canon(μ_c) = μ` over `K` — the Step-2 read-off,
//       computed through the generic evaluation ring `R` with a
//       caller-supplied image of the module basis.
// ---------------------------------------------------------------------

/// The extension-field opening: the Step-1 per-coefficient chunk folds plus
/// the ordinary mod-`q'` proof for the projected claim.
#[derive(Clone)]
pub struct IntEvalRsLigExtProof {
    /// `mus[d·L₁ + l][c] = ⟨bits_c, chunk_l(coords_d)⟩` — coordinate-major,
    /// `ext_deg · L₁` vectors of `2^s` folds (`L₁ = ⌈q_bits/c_w⌉`). The
    /// codec trims each vector's all-zero tail exactly like the base
    /// proof's `us`.
    pub mus: Vec<Vec<u128>>,
    /// The mod-`q'` opening of the projected claim at the row weights `γ`.
    pub base: IntEvalRsLigModQProof,
}

/// Absorb the Step-1 folds into the transcript (16-byte little-endian
/// values, coordinate-major, full `2^s` per vector) — they must be bound
/// BEFORE the projection prime and point are squeezed.
fn absorb_ext_step1_folds(transcript: &mut impl Transcript, mus: &[Vec<u128>]) {
    let total: usize = mus.iter().map(|m| m.len().wrapping_mul(16)).sum();
    let mut buf = Vec::with_capacity(total);
    for m in mus {
        for &u in m {
            buf.extend_from_slice(&u.to_le_bytes());
        }
    }
    transcript.absorb_slice(&buf);
}

/// Prove `⟨π_q(bits), v^{(1)} ⊗ v^{(2)}⟩ = μ` over an extension field
/// `K = F_q[X]/(h(X))` of degree `ext_deg ≥ 2` with the Ligerito opening.
///
/// `weight_coords[d][b] ∈ [0, 2^{q_bits})` is coordinate `d` of the
/// canonical integer lift `π_canon^{-1}(v^{(1)}_b)` (coordinate-major, one
/// vector of length `2^t` per basis element `1, X, …, X^{e−1}`). For a
/// prime field (`ext_deg = 1`) use [`prove_mle_eval_mod_q_ligerito`] — the
/// projection step is the identity there and this entry point rejects it.
#[allow(clippy::arithmetic_side_effects)]
pub fn prove_mle_eval_ext_ligerito(
    transcript: &mut (impl Transcript + Send),
    hint: &FlockCommitHint,
    p: &IntEvalParams,
    weight_coords: &[Vec<u128>],
    q_bits: usize,
    proj: &crate::ext_proj::ExtProjParams,
    alpha: Gf,
    pc: &LigProverConfig,
) -> IntEvalRsLigExtProof {
    use crate::ext_proj::{projected_row_weights, sample_proj_point, sample_proj_prime};
    use crate::pcs::{chunk_row_weights, mod_q_chunk_width, mod_q_num_chunks};
    let ext_deg = weight_coords.len();
    assert!(
        ext_deg >= 2,
        "extension degree must be ≥ 2 (use prove_mle_eval_mod_q_ligerito for prime fields)"
    );
    assert!((1..=126).contains(&q_bits), "q_bits must be in [1, 126]");
    proj.validate();
    let coord_bound = 1u128 << q_bits;
    for wc in weight_coords {
        assert_eq!(wc.len(), p.rows(), "one weight coordinate per row");
        assert!(wc.iter().all(|&x| x < coord_bound), "weight coordinates must be < 2^q_bits");
    }
    let c_w = mod_q_chunk_width(p);
    let l1 = mod_q_num_chunks(p, q_bits);

    // Step 1: the exact integer folds of every coordinate's chunked lift —
    // all `e·L₁` weight sets in ONE pass over the committed bit rows.
    let mus: Vec<Vec<u128>> = {
        let _g = crate::utils::prof::scope("ext:step1_folds");
        let chunked: Vec<Vec<u128>> = weight_coords
            .iter()
            .flat_map(|wc| chunk_row_weights(wc, c_w, l1))
            .collect();
        let sets: Vec<&[u128]> = chunked.iter().map(|w| &w[..]).collect();
        crate::ligerito::fold_values_bits_multi(p, &hint.rows, &sets)
    };
    absorb_ext_step1_folds(transcript, &mus);

    // Step 3: random prime + point, then the projected row weights γ.
    let q_proj = sample_proj_prime(transcript, proj);
    let alpha_proj = sample_proj_point(transcript, q_proj);
    let gamma = projected_row_weights(weight_coords, q_proj, alpha_proj);

    // Steps 4–6: the ordinary mod-q' opening at γ.
    let base =
        prove_mle_eval_mod_q_ligerito(transcript, hint, p, &gamma, proj.prime_bits, alpha, pc);
    IntEvalRsLigExtProof { mus, base }
}

/// Verify an extension-field evaluation claim `claimed ∈ R ≅ K`.
///
/// `weight_coords` as in [`prove_mle_eval_ext_ligerito`]; `col_weights[c] =
/// v_c^{(2)} ∈ K`; `basis[d]` is the image in `K` of the `d`-th module
/// basis element (for `K = F_q[X]/(h)` in the power basis:
/// `basis = [1, X, …, X^{e−1}]` as `R`-elements). `R::from(u128)` must be
/// the ring embedding `ℤ → K` (reduction mod `q` into the prime subfield).
#[allow(clippy::arithmetic_side_effects)]
#[allow(clippy::too_many_arguments)]
pub fn verify_mle_eval_ext_ligerito<R>(
    transcript: &mut (impl Transcript + Send),
    commitment: &Commitment,
    proof: &IntEvalRsLigExtProof,
    p: &IntEvalParams,
    weight_coords: &[Vec<u128>],
    col_weights: &[R],
    basis: &[R],
    alpha: Gf,
    claimed: R,
    q_bits: usize,
    proj: &crate::ext_proj::ExtProjParams,
    vc: &LigVerifierConfig,
) -> Result<(), FlockRsError>
where
    R: Copy + PartialEq + From<u128> + core::ops::Add<Output = R> + core::ops::Mul<Output = R>,
{
    use crate::ext_proj::{
        ProjArith, projected_row_weights, sample_proj_point, sample_proj_prime,
    };
    use crate::pcs::{mod_q_chunk_width, mod_q_num_chunks, recombine_read_off};
    let ext_deg = weight_coords.len();
    assert!(
        ext_deg >= 2,
        "extension degree must be ≥ 2 (use verify_mle_eval_mod_q_ligerito for prime fields)"
    );
    assert!((1..=126).contains(&q_bits), "q_bits must be in [1, 126]");
    assert_eq!(basis.len(), ext_deg, "one basis image per weight coordinate");
    proj.validate();
    let coord_bound = 1u128 << q_bits;
    for wc in weight_coords {
        assert_eq!(wc.len(), p.rows(), "one weight coordinate per row");
        assert!(wc.iter().all(|&x| x < coord_bound), "weight coordinates must be < 2^q_bits");
    }
    let c_w = mod_q_chunk_width(p);
    let l1 = mod_q_num_chunks(p, q_bits);
    let cols = p.cols();

    // Step-1 folds: shape, re-pad (the codec trims all-zero tails), and the
    // free range check at the honest per-chunk bound — chunk `l` of a
    // coordinate's lift has width `w_l = min(c_w, q_bits − c_w·l)` bits, so
    // an honest fold is `< 2^{t+W+w_l}` (≤ the old `2^{c_w+t+W} = 2^127`).
    // Bounding the sent values bounds the difference polynomial's
    // coefficients in the Schwartz–Zippel argument.
    if proof.mus.len() != ext_deg.wrapping_mul(l1) {
        return Err(FlockRsError::ExtShape);
    }
    let mut mus: Vec<Vec<u128>> = Vec::with_capacity(proof.mus.len());
    for (k, m) in proof.mus.iter().enumerate() {
        if m.len() > cols {
            return Err(FlockRsError::ExtShape);
        }
        let w_l = c_w.min(q_bits.wrapping_sub(c_w.wrapping_mul(k % l1)));
        let bound = 1u128 << w_l.wrapping_add(p.t).wrapping_add(p.word_bits);
        for (c, &x) in m.iter().enumerate() {
            if x >= bound {
                return Err(FlockRsError::ExtChunkRange { coeff: k / l1, chunk: k % l1, col: c });
            }
        }
        let mut m = m.clone();
        m.resize(cols, 0);
        mus.push(m);
    }
    absorb_ext_step1_folds(transcript, &mus);

    // Step 3: the same transcript sampling and projection as the prover.
    let q_proj = sample_proj_prime(transcript, proj);
    let alpha_proj = sample_proj_point(transcript, q_proj);
    let gamma = projected_row_weights(weight_coords, q_proj, alpha_proj);

    // Steps 4–6: the mod-q' core binds `us[l][c] = ⟨bits_c, chunk_l(γ)⟩`.
    let us = verify_mod_q_lig_core(
        transcript,
        commitment,
        &proof.base,
        p,
        &gamma,
        alpha,
        proj.prime_bits,
        vc,
    )?;

    // (A) Per-column congruence mod q': the certified folds must equal the
    // sent polynomials evaluated at α'. Both sides are recombined from
    // their base-2^{c_w} digits modulo q' — the digit place values
    // (`2^{c_w·l}` for the lhs, the fused `α'^d·2^{c_w·l}` for the rhs)
    // are prepared ONCE as Montgomery factors, so each fold costs one
    // Montgomery multiplication (plain×monty) and a modular add.
    let _g_checks = crate::utils::prof::scope("ext:checks");
    let zq = ProjArith::new(q_proj);
    let l2 = mod_q_num_chunks(p, proj.prime_bits);
    let chunk_base = zq.reduce(1u128 << c_w);
    let base_pows = zq.powers(chunk_base, l1.max(l2));
    let alpha_pows = zq.powers(alpha_proj, ext_deg);
    let lhs_m: Vec<_> = base_pows[..l2].iter().map(|&b| zq.monty_factor(b)).collect();
    let rhs_m: Vec<_> = (0..ext_deg)
        .flat_map(|d| {
            base_pows[..l1]
                .iter()
                .map(|&b| zq.monty_factor(zq.mul(alpha_pows[d], b)))
                .collect::<Vec<_>>()
        })
        .collect();
    for c in 0..cols {
        let mut lhs = 0u128;
        for (u_l, m) in us.iter().zip(lhs_m.iter()) {
            lhs = zq.add(lhs, zq.mul_plain_by(u_l[c], m));
        }
        let mut rhs = 0u128;
        for (mu_k, m) in mus.iter().zip(rhs_m.iter()) {
            rhs = zq.add(rhs, zq.mul_plain_by(mu_k[c], m));
        }
        if lhs != rhs {
            return Err(FlockRsError::ExtCongruence { col: c });
        }
    }

    // (B) The K-side read-off: Σ_c v_c^{(2)}·π_canon(μ_c) = μ, coordinate
    // by coordinate through the generic ring — coordinate d contributes
    // basis[d]·Σ_c w′_c·(Σ_l 2^{c_w·l}·μ_{c,d}^{(l)} mod q).
    let mut y = R::from(0u128);
    for (d, &b_d) in basis.iter().enumerate() {
        let flat: Vec<u128> = mus[d.wrapping_mul(l1)..d.wrapping_add(1).wrapping_mul(l1)]
            .iter()
            .flat_map(|m| m.iter().copied())
            .collect();
        let y_d = recombine_read_off(p, &flat, 0, col_weights, c_w, l1);
        y = y + b_d * y_d;
    }
    if y != claimed {
        return Err(FlockRsError::ExtReadOff);
    }
    Ok(())
}

// ---------------------------------------------------------------------
// Mod-q MLE evaluation + VIRTUAL XOR claims (no new commitment): prove
// `MLE[INT(x)](r') = y' ∈ 𝔽_q` for `x = ⊕_k committed columns`, alongside
// the main mod-q opening. The x bits are F₂-derivable from the committed
// rows (wordwise-XOR slice extraction), so the x-claim runs its OWN small
// merged forest — same `2^s` trees, folded width `t' = t − log_cols`, i.e.
// `2^{-log_cols}` of the main forest's leaves — and its residual claim
// `M̂_x(z') = μ'` expands BY CHAR-2 LINEARITY into `k` claims on the
// committed bit-MLE at points whose column coordinates are boolean:
// `M̂_x(z') = Σ_k M̂(z'_{row}, bits(i_k), z'_{bit}, z'_{clear})`. Those ride
// the ONE existing Ligerito call as extra η-RLC'd ring-switch entries —
// no new commitment, no new opened rows, no new recursion.
// ---------------------------------------------------------------------

/// One virtual-XOR evaluation claim (prover side): the committed UAIR
/// columns being XORed, an optional constant word pattern, an optional
/// UNCOMMITTED external term, and the mod-q row weights over the x
/// tensor's `2^{t'}` folded positions (`w'_b = eq(b, r'_fold) mod q` for a
/// genuine MLE claim at `r'`; any `[0, q)` weights are accepted). The
/// virtual vector is `x = (⊕_k cols[k]) ⊕ constant ⊕ external`.
pub struct VirtualXorClaim<'a> {
    /// Committed UAIR column indices (each `< layout.num_cols`); may be
    /// empty when `constant`/`external_rows` carry the claim.
    pub cols: &'a [usize],
    /// Constant word pattern XORed into every trace row (0 = absent);
    /// e.g. the all-ones word turns `a ⊕ W` into `a ⊕ ¬W`. FREE for the
    /// verifier (closed-form bit-MLE, no proof bytes).
    pub constant: u128,
    /// Uncommitted external term, given in the x layout (`2^s` rows of
    /// `2^{t'}` bits). Its per-chunk residual `μ_e = ê(z')` rides the
    /// proof UNVERIFIED and is returned as a [`VirtualXorObligation`] —
    /// the CALLER must bind it by other means (it is NOT proven against
    /// the F₂ commitment here).
    pub external_rows: Option<&'a [Vec<u64>]>,
    /// Row weights over `b' = (j ≪ tw) | row_hi`, length `2^{t'}`.
    pub row_weights_q: &'a [u128],
}

/// A virtual-XOR claim's verifier inputs: the statement-side fields plus
/// the clear-axis column weights `w'_c ∈ R` and the claimed evaluation.
pub struct VirtualXorVerifyClaim<'a, R> {
    /// Committed UAIR column indices (each `< layout.num_cols`).
    pub cols: &'a [usize],
    /// Constant word pattern XORed into every trace row (0 = absent).
    pub constant: u128,
    /// Whether the claim carries an uncommitted external term.
    pub has_external: bool,
    /// Row weights over `b' = (j ≪ tw) | row_hi`, length `2^{t'}`.
    pub row_weights_q: &'a [u128],
    /// Clear-axis weights `w'_c = eq(c, r'_clear) ∈ R`, length `2^s`.
    pub col_weights: &'a [R],
    /// The claimed evaluation `y' = Σ_c w'_c · Σ_{b'} w'_{b'} · x[(b',c)]`.
    pub claimed: R,
}

/// An UNVERIFIED external-term residual the verifier hands back to the
/// caller: the claim `ê(point) = value` (the external's bit-MLE over the
/// x index space) must be discharged by the surrounding protocol — e.g.
/// the integer-side machinery that owns the uncommitted column.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VirtualXorObligation {
    /// Which entry of `xors` the obligation belongs to.
    pub xor_index: usize,
    /// The weight chunk it arose in.
    pub chunk: usize,
    /// The x-tensor residual point (`t' + s` coordinates).
    pub point: Vec<Gf>,
    /// The claimed external bit-MLE value at `point`.
    pub value: Gf,
}

/// Per virtual-XOR claim proof parts: one small merged forest per weight
/// chunk (same `2^s` trees, depth `t'`; roots derived from `us`). The
/// ring-switch messages live in the shared flat list
/// ([`IntEvalRsLigModQXorProof::rings`]).
pub struct VirtXorSide {
    /// `us[l]` = the 2^s chunk folds `u'^{(l)}_c`, range-checked `< 2^127`.
    /// (The claim's forest/presum live in the SHARED batched
    /// [`IntEvalRsLigModQXorProof::x_mfs`]/`x_presums`.)
    pub us: Vec<Vec<u128>>,
    /// Per-chunk external residuals `μ_e = ê(z'_l)` (empty when the claim
    /// has no external term). Absorbed, NOT verified — returned as
    /// [`VirtualXorObligation`]s.
    pub externals: Vec<Gf>,
}

/// End-to-end mod-q proof with virtual-XOR side claims. `rings` is flat in
/// η order: the main chunks' claims first, then per (xor, chunk, BUCKET) —
/// embedded points that share their in-pack coordinates (columns with
/// equal low `min(7−tw, log_cols)` index bits) share one summed ring.
pub struct IntEvalRsLigModQXorProof {
    pub mfs: Vec<MergedForestProof>,
    pub us: Vec<Vec<u128>>,
    pub presums: Vec<MultiDegreeSumcheckProof<Gf>>,
    /// Per x weight chunk: ONE batched merged forest over ALL claims'
    /// trees (`N·2^s`, padded to a power of two) + ONE N-group
    /// pre-sumcheck — every claim exits at the chunk's SHARED point.
    pub x_mfs: Vec<MergedForestProof>,
    pub x_presums: Vec<MultiDegreeSumcheckProof<Gf>>,
    pub xors: Vec<VirtXorSide>,
    pub rings: Vec<RingSwitchProof>,
    pub lig: LigeritoProof,
}

/// Bucket the XORed columns by their IN-PACK boolean coordinate pattern
/// (the low `min(7−tw, log_cols)` bits of the column index): embedded
/// points within a bucket share `eq_lo`, so their in-pack marginals sum
/// into ONE ring message. Returns `(pattern, member positions)` in first-
/// occurrence order — prover and verifier derive identical buckets.
fn xor_ring_buckets(layout: &ShaF2Layout, cols: &[usize]) -> Vec<(usize, Vec<usize>)> {
    let nb_lo = LOG_PACKING.saturating_sub(layout.tw).min(layout.log_cols);
    let pat_mask = (1usize << nb_lo).wrapping_sub(1);
    let mut out: Vec<(usize, Vec<usize>)> = Vec::new();
    for (ki, &i) in cols.iter().enumerate() {
        let pat = i & pat_mask;
        match out.iter_mut().find(|(p, _)| *p == pat) {
            Some((_, members)) => members.push(ki),
            None => out.push((pat, vec![ki])),
        }
    }
    out
}

/// Committed-matrix full bit index for x-tensor full index `idx_x` with the
/// column coordinates pinned to committed column `i_col`: parse
/// `idx_x = (c ≪ t') | (j ≪ tw) | row_hi` and re-insert `i_col` between
/// `row_hi` and `j`.
#[allow(clippy::arithmetic_side_effects)] // bounded index math over the layout
pub(crate) fn embed_xor_index(layout: &ShaF2Layout, idx_x: usize, i_col: usize) -> usize {
    let tw = layout.tw;
    let t_x = layout.bit_vars + tw;
    let row_hi = idx_x & ((1usize << tw) - 1);
    let j = (idx_x >> tw) & ((1usize << layout.bit_vars) - 1);
    let c = idx_x >> t_x;
    row_hi | (i_col << tw) | (j << (tw + layout.log_cols)) | (c << layout.p.t)
}

/// The committed-matrix residual point for x-claim exit point `pt_x`
/// (`t' + s` coordinates) and column `i_col`: coordinates
/// `[pt_x[0..tw], bits(i_col), pt_x[tw..]]` — the column coordinates are
/// boolean, everything else is shared with the x claim.
#[allow(clippy::arithmetic_side_effects)]
fn embed_xor_point(layout: &ShaF2Layout, pt_x: &[Gf], i_col: usize) -> Vec<Gf> {
    let tw = layout.tw;
    let mut out = Vec::with_capacity(pt_x.len() + layout.log_cols);
    out.extend_from_slice(&pt_x[..tw]);
    for m in 0..layout.log_cols {
        out.push(if (i_col >> m) & 1 == 1 { Gf::one() } else { Gf::zero() });
    }
    out.extend_from_slice(&pt_x[tw..]);
    out
}

/// First `pt_x` coordinate whose embedded position clears the 128-bit pack.
/// The embedded claim's hi-part eq factorizes as
/// `eq(pt_x[p0..], ŷ) · [boolean column bits match i_col]`, so the sparse
/// support walk enumerates `ŷ` over `2^{t'+s−p0}` entries and the eq table
/// over `pt_x[p0..]` is SHARED by all k embedded claims.
fn xor_support_prefix(layout: &ShaF2Layout) -> usize {
    let nb_lo = LOG_PACKING.saturating_sub(layout.tw).min(layout.log_cols);
    LOG_PACKING.saturating_sub(nb_lo)
}

/// `ê(pt)` — an external term's bit-MLE at the x residual point, from its
/// x-layout bit rows (eq tables over the folded and clear coordinates).
#[allow(clippy::arithmetic_side_effects)]
fn external_residual(p_x: &IntEvalParams, e_rows: &[Vec<u64>], pt: &[Gf]) -> Gf {
    let (bx, c_part) = pt.split_at(p_x.t);
    let eq_bx = crate::poly::utils::build_eq_x_r_vec(bx, &()).expect("t' >= 1");
    let eq_c = crate::poly::utils::build_eq_x_r_vec(c_part, &()).expect("s >= 1");
    let mut acc = Gf::zero();
    for (c, row) in e_rows.iter().enumerate() {
        let mut rc = Gf::zero();
        for (wi, &word) in row.iter().enumerate() {
            let mut bits = word;
            while bits != 0 {
                let t = bits.trailing_zeros() as usize;
                rc += eq_bx[(wi << 6) | t];
                bits &= bits.wrapping_sub(1);
            }
        }
        acc += eq_c[c] * rc;
    }
    acc
}

/// `Ĉ(pt)` — the constant pattern's bit-MLE at the x residual point. The
/// pattern repeats on every trace row, so the row and clear eq-sums
/// collapse to 1 and only the bit-position coordinates survive:
/// `Ĉ(pt) = Σ_j bit_j(pattern) · eq(bits(j), pt[tw..tw+bit_vars])`.
/// FREE for the verifier — no proof bytes, no trust.
#[allow(clippy::arithmetic_side_effects)]
fn constant_residual(layout: &ShaF2Layout, constant: u128, pt: &[Gf]) -> Gf {
    if constant == 0 {
        return Gf::zero();
    }
    let one = Gf::one();
    let coords = &pt[layout.tw..layout.tw + layout.bit_vars];
    let mut acc = Gf::zero();
    for j in 0..(1usize << layout.bit_vars) {
        if (constant >> j) & 1 == 1 {
            let mut e = one;
            for (m, r) in coords.iter().enumerate() {
                e = e * (if (j >> m) & 1 == 1 { *r } else { one + *r });
            }
            acc += e;
        }
    }
    acc
}

/// Prove the main mod-q opening PLUS a list of virtual-XOR claims against
/// the same commitment. Layout/weight conventions as
/// [`prove_mle_eval_mod_q_ligerito`]; the x-claim weights come per claim.
/// Complement claims (same columns/row-weights, constants differing by the
/// all-ones pattern, no externals — [`complement_elision`]) are ELIDED:
/// they get no forest trees, folds or rings, and their proof side stays
/// empty — the verifier derives their value from the base claim's.
#[allow(clippy::too_many_arguments)]
pub fn prove_mle_eval_mod_q_ligerito_with_virtual_xors(
    transcript: &mut (impl Transcript + Send),
    hint: &FlockCommitHint,
    layout: &ShaF2Layout,
    row_weights_q: &[u128],
    q_bits: usize,
    xors: &[VirtualXorClaim<'_>],
    alpha: Gf,
    pc: &LigProverConfig,
) -> IntEvalRsLigModQXorProof {
    prove_mod_q_lig_xor_impl(transcript, hint, layout, Some(row_weights_q), q_bits, xors, alpha, pc)
}

/// ALL-CLAIMS-VIRTUAL mode: prove a claim set with NO main claim — every
/// evaluation, including single committed columns (a k=1 claim), runs as a
/// virtual claim at depth `t' = t − log_cols` and joins the ONE batched
/// x-forest. The Ligerito basis is built from the x-claim rings alone: the
/// commitment is bound by the Ligerito call (proximity + the η-combined
/// basis functional), not by which claim is "main" — each claim's chain
/// (range-checked folds → derived roots → batched forest → presum →
/// residual rings → basis) is self-contained. Compared to routing a
/// committed column through the main claim (depth `t`, weights supported
/// on the column's slice), the k=1 virtual claim runs at depth `t'` —
/// `2^{-log_cols}` of the leaf work — with an identical soundness chain.
#[allow(clippy::too_many_arguments)]
pub fn prove_mle_eval_mod_q_ligerito_claims_only(
    transcript: &mut (impl Transcript + Send),
    hint: &FlockCommitHint,
    layout: &ShaF2Layout,
    q_bits: usize,
    xors: &[VirtualXorClaim<'_>],
    alpha: Gf,
    pc: &LigProverConfig,
) -> IntEvalRsLigModQXorProof {
    assert!(!xors.is_empty(), "claims-only mode needs at least one virtual claim");
    prove_mod_q_lig_xor_impl(transcript, hint, layout, None, q_bits, xors, alpha, pc)
}

#[allow(clippy::arithmetic_side_effects)]
#[allow(clippy::too_many_arguments)]
fn prove_mod_q_lig_xor_impl(
    transcript: &mut (impl Transcript + Send),
    hint: &FlockCommitHint,
    layout: &ShaF2Layout,
    main_rw: Option<&[u128]>,
    q_bits: usize,
    xors: &[VirtualXorClaim<'_>],
    alpha: Gf,
    pc: &LigProverConfig,
) -> IntEvalRsLigModQXorProof {
    use crate::pcs::{
        chunk_row_weights, complement_elision, extract_virtual_xor_rows, mod_q_chunk_width,
        mod_q_num_chunks, virtual_xor_params,
    };
    let p = &layout.p;
    assert_eq!(p.word_bits, 1, "virtual-XOR claims assume the W=1 SHA layout");
    let lch = main_rw.map_or(0, |_| mod_q_num_chunks(p, q_bits));
    let chunks = main_rw
        .map_or_else(Vec::new, |rw| chunk_row_weights(rw, mod_q_chunk_width(p), lch));

    // Main chunks — identical to `prove_mle_eval_mod_q_ligerito`.
    let _g_main = crate::utils::prof::scope("mq:main_chunks");
    let mut mfs = Vec::with_capacity(lch);
    let mut us = Vec::with_capacity(lch);
    let mut presums = Vec::with_capacity(lch);
    let mut points = Vec::with_capacity(lch);
    for w_l in &chunks {
        let (mf, u, ps, pt) = prove_int_eval_merged_common(
            transcript,
            p,
            &hint.rows,
            Some(&hint.packed_cols),
            w_l,
            alpha,
        );
        mfs.push(mf);
        us.push(u);
        presums.push(ps);
        points.push(pt);
    }
    drop(_g_main);

    // Complement elision: derived claims get NO machinery of their own.
    let elide = complement_elision(
        layout,
        &xors
            .iter()
            .map(|cl| (cl.cols, cl.constant, cl.external_rows.is_some(), cl.row_weights_q))
            .collect::<Vec<_>>(),
    );
    let active: Vec<usize> = (0..xors.len()).filter(|&j| elide[j].is_none()).collect();

    // Virtual-XOR side: extract each ACTIVE claim's rows, then run ONE
    // batched merged forest per weight chunk of the x tensor.
    let p_x = virtual_xor_params(layout);
    let (c_w_x, lch_x) = if active.is_empty() {
        (0usize, 0usize)
    } else {
        assert!(
            row_bit_vars(&p_x) >= 6,
            "x-claim pre-sumcheck needs t' ≥ 6 (whole-word rows); got t'={}",
            p_x.t
        );
        (mod_q_chunk_width(&p_x), mod_q_num_chunks(&p_x, q_bits))
    };
    let mut xor_sides: Vec<VirtXorSide> = xors
        .iter()
        .map(|_| VirtXorSide { us: Vec::with_capacity(lch_x), externals: Vec::new() })
        .collect();
    let mut x_mfs = Vec::with_capacity(lch_x);
    let mut x_presums = Vec::with_capacity(lch_x);
    let mut x_points: Vec<Vec<Gf>> = Vec::with_capacity(lch_x);
    if !active.is_empty() {
        let x_rows_all: Vec<Vec<Vec<u64>>> = {
            let _g = crate::utils::prof::scope("vx:extract");
            active
                .iter()
                .map(|&j| {
                    let cl = &xors[j];
                    assert_eq!(cl.row_weights_q.len(), p_x.rows(), "x row-weight length");
                    extract_virtual_xor_rows(
                        layout,
                        &hint.rows,
                        cl.cols,
                        cl.constant,
                        cl.external_rows,
                    )
                })
                .collect()
        };
        let x_chunks_all: Vec<Vec<Vec<u128>>> = active
            .iter()
            .map(|&j| chunk_row_weights(xors[j].row_weights_q, c_w_x, lch_x))
            .collect();
        let rows_refs: Vec<&[Vec<u64>]> = x_rows_all.iter().map(|r| &r[..]).collect();
        let _g_vx = crate::utils::prof::scope("vx:common");
        for l in 0..lch_x {
            let w_refs: Vec<&[u128]> = x_chunks_all.iter().map(|ch| &ch[l][..]).collect();
            let (mf, us_per_claim, presum, pt) =
                prove_x_claims_batched_common(transcript, &p_x, &rows_refs, &w_refs, alpha);
            x_mfs.push(mf);
            x_presums.push(presum);
            x_points.push(pt);
            for (k, u) in us_per_claim.into_iter().enumerate() {
                xor_sides[active[k]].us.push(u);
            }
        }
        drop(_g_vx);
        for (n, cl) in xors.iter().enumerate() {
            if let Some(e_rows) = cl.external_rows {
                for pt in &x_points {
                    xor_sides[n].externals.push(external_residual(&p_x, e_rows, pt));
                }
                crate::ligerito::absorb_externals(transcript, &xor_sides[n].externals);
            }
        }
    }

    // Ring-switch messages. Main chunks: dense in-pack marginals of the
    // packed message, as before.
    let _g_rm = crate::utils::prof::scope("mq:rings_main");
    let mut rings = Vec::new();
    let mut eq_his = Vec::with_capacity(lch);
    for pt in &points {
        let eq_hi = crate::poly::utils::build_eq_x_r_vec(&pt[LOG_PACKING..], &()).expect("r_hi");
        let s = dense_ring_sv(&hint.p_msg, &eq_hi);
        crate::ligerito::absorb_sv(transcript, &s);
        rings.push(RingSwitchProof { s_v: s });
        eq_his.push(eq_hi);
    }
    drop(_g_rm);
    // X-claim rings: `μ' = Σ_k M̂(pt_k)` by char-2 linearity. Each XORed
    // column gets an in-pack marginal `s_v` at its embedded point, computed
    // on the support of the boolean column coordinates only (`2^{t'+s−p0}`
    // entries; the non-boolean eq table is shared across k, and the k walks
    // run in parallel). Columns sharing their in-pack coordinates share
    // `eq_lo`, so their marginals SUM into one ring message per bucket.
    let _g_rx = crate::utils::prof::scope("vx:rings");
    let p0 = xor_support_prefix(layout);
    // One eq table per chunk — the batched claims SHARE their exit point.
    let xor_eq_ns: Vec<Vec<Gf>> = x_points
        .iter()
        .map(|pt| crate::poly::utils::build_eq_x_r_vec(&pt[p0..], &()).expect("x support"))
        .collect();
    for &n in &active {
        let cl = &xors[n];
        let buckets = xor_ring_buckets(layout, cl.cols);
        for eq_ns in &xor_eq_ns {
            let cols_owned: Vec<usize> = cl.cols.to_vec();
            let svs: Vec<Vec<Gf>> = cfg_into_iter!(cols_owned)
                .map(|i_col| {
                    let mut s = vec![Gf::zero(); 128];
                    for (yx, &e) in eq_ns.iter().enumerate() {
                        let y = embed_xor_index(layout, yx << p0, i_col) >> LOG_PACKING;
                        let pe = hint.p_msg[y];
                        for wi in 0..2usize {
                            let mut bits = if wi == 0 { pe.lo } else { pe.hi };
                            while bits != 0 {
                                let t = bits.trailing_zeros() as usize;
                                s[(wi << 6) | t] += e;
                                bits &= bits.wrapping_sub(1);
                            }
                        }
                    }
                    s
                })
                .collect();
            for (_pat, members) in &buckets {
                let mut st = vec![Gf::zero(); 128];
                for &ki in members {
                    for (a, b) in st.iter_mut().zip(svs[ki].iter()) {
                        *a += *b;
                    }
                }
                crate::ligerito::absorb_sv(transcript, &st);
                rings.push(RingSwitchProof { s_v: st });
            }
        }
    }
    drop(_g_rx);

    let r2: Vec<Gf> = transcript.get_field_challenges(LOG_PACKING, &());
    let eq_r2 = crate::poly::utils::build_eq_x_r_vec(&r2, &()).expect("r2");
    let etas: Vec<Gf> = transcript.get_field_challenges(rings.len(), &());

    // Combined basis + target over the flat ring list.
    let m_p = packed_vars(p);
    let mut b_comb = vec![F128::ZERO; 1usize << m_p];
    let _g_bm = crate::utils::prof::scope("mq:bcomb_main");
    fill_phi_basis(&mut b_comb, &eq_his, &etas[..lch], &eq_r2);
    drop(_g_bm);
    let mut target = Gf::zero();
    let _g_bx = crate::utils::prof::scope("vx:bcomb");
    // Φ images of the per-chunk eq tables, shared across the claims.
    let phi_ns_all: Vec<Vec<Gf>> = xor_eq_ns
        .iter()
        .map(|eq_ns| cfg_iter!(eq_ns).map(|&e| phi_r2(e, &eq_r2)).collect())
        .collect();
    let mut ring_idx = lch;
    for &n in &active {
        let cl = &xors[n];
        let buckets = xor_ring_buckets(layout, cl.cols);
        for phi_ns in &phi_ns_all {
            for (_pat, members) in &buckets {
                let eta = etas[ring_idx];
                for &ki in members {
                    let i_col = cl.cols[ki];
                    for (yx, &ph) in phi_ns.iter().enumerate() {
                        let y = embed_xor_index(layout, yx << p0, i_col) >> LOG_PACKING;
                        b_comb[y] = b_comb[y] + gf_to_f128(eta * ph);
                    }
                }
                ring_idx += 1;
            }
        }
    }
    drop(_g_bx);
    for (i, ring) in rings.iter().enumerate() {
        let s_u = crate::ligerito::transpose_bits_128(&ring.s_v);
        let beta = s_u.iter().zip(eq_r2.iter()).fold(Gf::zero(), |a, (su, e)| a + *su * *e);
        target += etas[i] * beta;
    }

    let _g_lig = crate::utils::prof::scope("mq:lig");
    let lig = ligerito::recursive_prover_with_basis(
        pc,
        hint.p_msg.clone(),
        b_comb,
        gf_to_f128(target),
        &hint.prover_data.codeword,
        &hint.prover_data.merkle_tree,
        &mut ZincChallenger(transcript),
    );
    IntEvalRsLigModQXorProof { mfs, us, presums, x_mfs, x_presums, xors: xor_sides, rings, lig }
}

/// Verify the main mod-q opening PLUS the virtual-XOR claims. Complement
/// claims ([`complement_elision`]) are checked as free derivations of
/// their base claim (`y_j + y_i = (Σ_b w_b)·(Σ_c w′_c)`); their proof
/// sides must be empty.
#[allow(clippy::too_many_arguments)]
pub fn verify_mle_eval_mod_q_ligerito_with_virtual_xors<R>(
    transcript: &mut (impl Transcript + Send),
    commitment: &Commitment,
    proof: &IntEvalRsLigModQXorProof,
    layout: &ShaF2Layout,
    row_weights_q: &[u128],
    col_weights: &[R],
    alpha: Gf,
    claimed: R,
    q_bits: usize,
    xors: &[VirtualXorVerifyClaim<'_, R>],
    vc: &LigVerifierConfig,
) -> Result<Vec<VirtualXorObligation>, FlockRsError>
where
    R: Copy + PartialEq + From<u128> + core::ops::Add<Output = R> + core::ops::Mul<Output = R>,
{
    verify_mod_q_lig_xor_impl(
        transcript,
        commitment,
        proof,
        layout,
        Some((row_weights_q, col_weights, claimed)),
        alpha,
        q_bits,
        xors,
        vc,
    )
}

/// Verify an ALL-CLAIMS-VIRTUAL proof
/// ([`prove_mle_eval_mod_q_ligerito_claims_only`]): no main claim — the
/// proof's main-chunk fields must be empty and the Ligerito basis is the
/// x-claim rings alone.
#[allow(clippy::too_many_arguments)]
pub fn verify_mle_eval_mod_q_ligerito_claims_only<R>(
    transcript: &mut (impl Transcript + Send),
    commitment: &Commitment,
    proof: &IntEvalRsLigModQXorProof,
    layout: &ShaF2Layout,
    alpha: Gf,
    q_bits: usize,
    xors: &[VirtualXorVerifyClaim<'_, R>],
    vc: &LigVerifierConfig,
) -> Result<Vec<VirtualXorObligation>, FlockRsError>
where
    R: Copy + PartialEq + From<u128> + core::ops::Add<Output = R> + core::ops::Mul<Output = R>,
{
    if xors.is_empty() {
        return Err(FlockRsError::RingSwitch(RsOpenError::Shape));
    }
    verify_mod_q_lig_xor_impl(transcript, commitment, proof, layout, None, alpha, q_bits, xors, vc)
}

#[allow(clippy::arithmetic_side_effects)]
#[allow(clippy::too_many_arguments)]
fn verify_mod_q_lig_xor_impl<R>(
    transcript: &mut (impl Transcript + Send),
    commitment: &Commitment,
    proof: &IntEvalRsLigModQXorProof,
    layout: &ShaF2Layout,
    main: Option<(&[u128], &[R], R)>,
    alpha: Gf,
    q_bits: usize,
    xors: &[VirtualXorVerifyClaim<'_, R>],
    vc: &LigVerifierConfig,
) -> Result<Vec<VirtualXorObligation>, FlockRsError>
where
    R: Copy + PartialEq + From<u128> + core::ops::Add<Output = R> + core::ops::Mul<Output = R>,
{
    use crate::pcs::{
        chunk_row_weights, complement_elision, mod_q_chunk_width, mod_q_num_chunks,
        recombine_read_off, virtual_xor_params,
    };
    let p = &layout.p;
    if p.word_bits != 1 {
        return Err(FlockRsError::RingSwitch(RsOpenError::Shape));
    }
    let c_w = mod_q_chunk_width(p);
    let lch = main.map_or(0, |_| mod_q_num_chunks(p, q_bits));
    let p_x = virtual_xor_params(layout);

    // Complement elision — the same statement-level partition the prover
    // computed; derived claims carry no machinery.
    let elide = complement_elision(
        layout,
        &xors
            .iter()
            .map(|cl| (cl.cols, cl.constant, cl.has_external, cl.row_weights_q))
            .collect::<Vec<_>>(),
    );
    let active: Vec<usize> = (0..xors.len()).filter(|&j| elide[j].is_none()).collect();

    let (c_w_x, lch_x) = if active.is_empty() {
        (0usize, 0usize)
    } else {
        (mod_q_chunk_width(&p_x), mod_q_num_chunks(&p_x, q_bits))
    };
    let num_xor_rings: usize = active
        .iter()
        .map(|&j| xor_ring_buckets(layout, xors[j].cols).len() * lch_x)
        .sum();
    if proof.mfs.len() != lch
        || proof.us.len() != lch
        || proof.presums.len() != lch
        || proof.xors.len() != xors.len()
        || proof.rings.len() != lch + num_xor_rings
    {
        return Err(FlockRsError::RingSwitch(RsOpenError::Shape));
    }
    if proof.x_mfs.len() != lch_x || proof.x_presums.len() != lch_x {
        return Err(FlockRsError::RingSwitch(RsOpenError::Shape));
    }
    for (j, (cl, xs)) in xors.iter().zip(proof.xors.iter()).enumerate() {
        let (expected_us, expected_externals) = if elide[j].is_some() {
            (0usize, 0usize)
        } else if cl.has_external {
            (lch_x, lch_x)
        } else {
            (lch_x, 0usize)
        };
        if (cl.cols.is_empty() && cl.constant == 0 && !cl.has_external)
            || cl.cols.iter().any(|&i| i >= layout.num_cols)
            || cl.row_weights_q.len() != p_x.rows()
            || cl.col_weights.len() != p_x.cols()
            || xs.us.len() != expected_us
            || xs.externals.len() != expected_externals
        {
            return Err(FlockRsError::RingSwitch(RsOpenError::Shape));
        }
    }

    // Derived (complement) claims: the base's column weights must match,
    // and `y_j + y_i = (Σ_b w_b)·(Σ_c w′_c)` in R — nothing else to check.
    for (j, base) in elide.iter().enumerate() {
        let Some(i) = *base else { continue };
        let (cl_j, cl_i) = (&xors[j], &xors[i]);
        if cl_j.col_weights.len() != cl_i.col_weights.len()
            || cl_j.col_weights.iter().zip(cl_i.col_weights.iter()).any(|(a, b)| *a != *b)
        {
            return Err(FlockRsError::RingSwitch(RsOpenError::Shape));
        }
        let s_row = cl_j.row_weights_q.iter().fold(R::from(0u128), |a, &w| a + R::from(w));
        let s_col = cl_j.col_weights.iter().fold(R::from(0u128), |a, &w| a + w);
        if cl_j.claimed + cl_i.claimed != s_row * s_col {
            return Err(FlockRsError::Common(IntEvalRsError::ReadOff));
        }
    }

    // Main chunks: range checks + merged-common verify, as before.
    let range_shift = c_w.wrapping_add(p.t).wrapping_add(p.word_bits);
    let bound = 1u128 << range_shift;
    let mut points = Vec::with_capacity(lch);
    let mut mus = Vec::with_capacity(lch);
    if let Some((row_weights_q, _, _)) = main {
        let chunks = chunk_row_weights(row_weights_q, c_w, lch);
        for l in 0..lch {
            for (k, &u) in proof.us[l].iter().enumerate() {
                if u >= bound {
                    let _ = k;
                    return Err(FlockRsError::ChunkRange { chunk: l, col: k });
                }
            }
            let (pt, mu) = verify_int_eval_merged_common(
                transcript,
                &proof.mfs[l],
                &proof.us[l],
                &proof.presums[l],
                p,
                &chunks[l],
                alpha,
            )
            .map_err(FlockRsError::Common)?;
            points.push(pt);
            mus.push(mu);
        }
    }

    // X-claim chunks: ONE batched forest + N-group presum per chunk; the
    // ACTIVE claims share the chunk's exit point.
    let mut x_points: Vec<Vec<Gf>> = Vec::with_capacity(lch_x);
    let mut x_mus: Vec<Vec<Gf>> = Vec::with_capacity(lch_x); // [chunk][active pos]
    if !active.is_empty() {
        let x_chunks_all: Vec<Vec<Vec<u128>>> = active
            .iter()
            .map(|&j| chunk_row_weights(xors[j].row_weights_q, c_w_x, lch_x))
            .collect();
        let range_shift_x = c_w_x.wrapping_add(p_x.t).wrapping_add(p_x.word_bits);
        let bound_x = 1u128 << range_shift_x;
        for l in 0..lch_x {
            for &j in &active {
                for (k, &u) in proof.xors[j].us[l].iter().enumerate() {
                    if u >= bound_x {
                        let _ = k;
                        return Err(FlockRsError::ChunkRange { chunk: l, col: k });
                    }
                }
            }
            let us_refs: Vec<&[u128]> =
                active.iter().map(|&j| &proof.xors[j].us[l][..]).collect();
            let w_refs: Vec<&[u128]> = x_chunks_all.iter().map(|ch| &ch[l][..]).collect();
            let (pt, mus_l) = verify_x_claims_batched_common(
                transcript,
                &proof.x_mfs[l],
                &us_refs,
                &proof.x_presums[l],
                &p_x,
                &w_refs,
                alpha,
            )
            .map_err(FlockRsError::Common)?;
            x_points.push(pt);
            x_mus.push(mus_l);
        }
        for (cl, xs) in xors.iter().zip(proof.xors.iter()) {
            if cl.has_external {
                crate::ligerito::absorb_externals(transcript, &xs.externals);
            }
        }
    }

    // Main rings: in-pack read-off must reproduce each chunk residual.
    // `r_hi_groups[i]` collects the hi-part points the i-th ring binds
    // (one for a main ring; a bucket's members for an x ring).
    let mut r_hi_groups: Vec<Vec<Vec<Gf>>> = Vec::with_capacity(proof.rings.len());
    for l in 0..lch {
        let ring = &proof.rings[l];
        if ring.s_v.len() != 128 {
            return Err(FlockRsError::RingSwitch(RsOpenError::Shape));
        }
        let eq_lo =
            crate::poly::utils::build_eq_x_r_vec(&points[l][..LOG_PACKING], &()).expect("r_lo");
        let claim = ring.s_v.iter().zip(eq_lo.iter()).fold(Gf::zero(), |a, (s, e)| a + *s * *e);
        if claim != mus[l] {
            return Err(FlockRsError::RingSwitch(RsOpenError::RingSwitchClaim));
        }
        crate::ligerito::absorb_sv(transcript, &ring.s_v);
        r_hi_groups.push(vec![points[l][LOG_PACKING..].to_vec()]);
    }
    // X-claim rings: the bucket sums of the embedded in-pack read-offs,
    // plus the closed-form constant term and the (unverified) external
    // residual, must reproduce the claim's residual `μ_n` at the chunk's
    // SHARED point (char-2 linearity of bit-MLEs; embedded points within
    // a bucket share `eq_lo`).
    let mut obligations: Vec<VirtualXorObligation> = Vec::new();
    let mut ring_idx = lch;
    for (pos, &x) in active.iter().enumerate() {
        let cl = &xors[x];
        let buckets = xor_ring_buckets(layout, cl.cols);
        for (l, pt_x) in x_points.iter().enumerate() {
            let mut sum = constant_residual(layout, cl.constant, pt_x);
            if cl.has_external {
                let value = proof.xors[x].externals[l];
                sum += value;
                obligations.push(VirtualXorObligation {
                    xor_index: x,
                    chunk: l,
                    point: pt_x.clone(),
                    value,
                });
            }
            for (_pat, members) in &buckets {
                let ring = &proof.rings[ring_idx];
                if ring.s_v.len() != 128 {
                    return Err(FlockRsError::RingSwitch(RsOpenError::Shape));
                }
                let group: Vec<Vec<Gf>> = members
                    .iter()
                    .map(|&ki| {
                        embed_xor_point(layout, pt_x, cl.cols[ki])[LOG_PACKING..].to_vec()
                    })
                    .collect();
                // eq_lo is shared within the bucket: build it from the
                // first member's embedded point.
                let pt_k0 = embed_xor_point(layout, pt_x, cl.cols[members[0]]);
                let eq_lo = crate::poly::utils::build_eq_x_r_vec(&pt_k0[..LOG_PACKING], &())
                    .expect("r_lo");
                sum += ring
                    .s_v
                    .iter()
                    .zip(eq_lo.iter())
                    .fold(Gf::zero(), |a, (s, e)| a + *s * *e);
                crate::ligerito::absorb_sv(transcript, &ring.s_v);
                r_hi_groups.push(group);
                ring_idx += 1;
            }
            if sum != x_mus[l][pos] {
                return Err(FlockRsError::RingSwitch(RsOpenError::RingSwitchClaim));
            }
        }
    }

    let r2: Vec<Gf> = transcript.get_field_challenges(LOG_PACKING, &());
    let eq_r2 = crate::poly::utils::build_eq_x_r_vec(&r2, &()).expect("r2");
    let etas: Vec<Gf> = transcript.get_field_challenges(proof.rings.len(), &());

    let mut target = Gf::zero();
    for (i, ring) in proof.rings.iter().enumerate() {
        let s_u = crate::ligerito::transpose_bits_128(&ring.s_v);
        let beta = s_u.iter().zip(eq_r2.iter()).fold(Gf::zero(), |a, (su, e)| a + *su * *e);
        target += etas[i] * beta;
    }

    let m_p = packed_vars(p);
    let eval_b = |ris: &[F128], yr_log_n: usize| -> Vec<F128> {
        let ris_gf: Vec<Gf> = ris.iter().map(|&f| f128_to_gf(f)).collect();
        let mut out = vec![Gf::zero(); 1usize << yr_log_n];
        for (i, group) in r_hi_groups.iter().enumerate() {
            for r_hi in group {
                let blk = residual_b_evals(&ris_gf, yr_log_n, r_hi, &eq_r2);
                for (o, x) in out.iter_mut().zip(blk.iter()) {
                    *o += etas[i] * *x;
                }
            }
        }
        out.into_iter().map(gf_to_f128).collect()
    };
    let ok = ligerito::recursive_verifier_with_basis_succinct(
        vc,
        &proof.lig,
        m_p,
        gf_to_f128(target),
        &commitment.root,
        eval_b,
        &mut ZincChallenger(transcript),
    );
    if !ok {
        return Err(FlockRsError::LigeritoReject);
    }

    // Read-offs in R: the main claim (when present), then each ACTIVE
    // x claim from its own folds (derived claims were checked above).
    if let Some((_, col_weights, claimed)) = main {
        let v_flat: Vec<u128> = proof.us.iter().flat_map(|u| u.iter().copied()).collect();
        let y = recombine_read_off(p, &v_flat, 0, col_weights, c_w, lch);
        if y != claimed {
            return Err(FlockRsError::Common(IntEvalRsError::ReadOff));
        }
    }
    for &x in &active {
        let (cl, xs) = (&xors[x], &proof.xors[x]);
        let vx_flat: Vec<u128> = xs.us.iter().flat_map(|u| u.iter().copied()).collect();
        let yx = recombine_read_off(&p_x, &vx_flat, 0, cl.col_weights, c_w_x, lch_x);
        if yx != cl.claimed {
            return Err(FlockRsError::Common(IntEvalRsError::ReadOff));
        }
    }
    Ok(obligations)
}

/// Itemized `(zinc-side, flock LigeritoProof)` bytes of a virtual-XOR mod-q
/// proof: main chunks + every x-claim's small forests + the flat ring list.
#[allow(clippy::arithmetic_side_effects)]
pub fn mle_eval_mod_q_lig_xor_size_breakdown(
    proof: &IntEvalRsLigModQXorProof,
) -> (ZincSideSizeBreakdown, usize) {
    let mut b = ZincSideSizeBreakdown::default();
    for l in 0..proof.mfs.len() {
        b.accumulate(&zinc_side_size_breakdown_merged(
            &proof.mfs[l],
            &proof.us[l],
            &proof.presums[l],
        ));
    }
    // Batched x side: ONE forest + N-group presum per chunk (shared by
    // the claims), per-claim folds + external residuals.
    for (mf, presum) in proof.x_mfs.iter().zip(proof.x_presums.iter()) {
        b.accumulate(&zinc_side_size_breakdown_merged(mf, &[], presum));
    }
    for xs in &proof.xors {
        for us in &xs.us {
            b.v += us.len() * 16;
        }
        b.v += xs.externals.len() * 16;
    }
    // The merged breakdown assumes one ring per forest; the flat ring list
    // (one per bucket per x chunk) is counted directly instead.
    b.s_v = proof.rings.len() * 128 * 16;
    (b, proof.lig.size_bytes())
}

// ---------------------------------------------------------------------
// EXPERIMENTAL — structured-tap virtual claims
// (docs/rlc-structured-taps-prompt.md, docs/rlc-structured-taps-phase0.md).
//
// Claims `MLE[INT(x)](r) = y ∈ R` for `x = ⊕_taps op(col)` with
// `op = ROT^r / SHIFT^r · off^o` ([`crate::taps::TapOp`]): the tapped rows
// are extracted ([`crate::taps::extract_virtual_tap_rows`]) and run the
// SAME batched x-forest + N-group presum as the claims-only virtual-XOR
// path; each claim's residual `x̂(ζ) = Σ_taps ŝ_tap(ζ)` then expands by
// char-2 linearity into TRANSLATED-EQ committed openings — per (tap,
// class) one sparse in-pack marginal against the class's translated
// slice-of-eq weight tables, bucketed by A-table equality — and the
// closing Ligerito residual is evaluated succinctly by the
// matrix-product-state closure ([`crate::taps::residual_b_evals_tap`]).
// The identity tap reproduces the plain embedded-claim reduction.
//
// Fiat–Shamir chain: absorb statement (tag 0x42: root, layout, q_bits,
// every claim's taps + constant + row weights) → per chunk the batched
// forest + presum → per (claim, chunk, bucket) the ring `s_v` messages →
// r″ + ring η's → ONE `recursive_prover_with_basis` call. Claimed values
// and clear-axis weights are R-valued read-off inputs (as in the
// virtual-XOR path); the caller binds them if its context requires.
// NOT wired into `proof_codec`; the API is experimental.
// ---------------------------------------------------------------------

/// One structured-tap claim (prover side).
pub struct TapClaim<'a> {
    /// The tap terms; `x = ⊕ op(col)`.
    pub taps: &'a [TapOp],
    /// Row weights over the x tensor's `2^{t'}` folded positions.
    pub row_weights_q: &'a [u128],
}

/// One structured-tap claim (verifier side).
pub struct TapVerifyClaim<'a, R> {
    /// The tap terms.
    pub taps: &'a [TapOp],
    /// Row weights over `2^{t'}` folded positions.
    pub row_weights_q: &'a [u128],
    /// Clear-axis weights `w'_c ∈ R`, length `2^s`.
    pub col_weights: &'a [R],
    /// The claimed evaluation.
    pub claimed: R,
}

/// End-to-end proof of a structured-tap claim set (EXPERIMENTAL).
pub struct IntEvalRsLigModQTapProof {
    /// Per x weight chunk: ONE batched merged forest over all claims'
    /// trees + ONE N-group pre-sumcheck.
    pub x_mfs: Vec<MergedForestProof>,
    pub x_presums: Vec<MultiDegreeSumcheckProof<Gf>>,
    /// `tap_us[claim][chunk]` = the `2^s` chunk folds, range-checked.
    pub tap_us: Vec<Vec<Vec<u128>>>,
    /// Ring-switch messages, flat in (claim, chunk, bucket) order.
    pub rings: Vec<RingSwitchProof>,
    pub lig: LigeritoProof,
}

/// Absorb the tap-claim statement (domain tag 0x42): root, layout shape,
/// `q_bits`, and every claim's taps + constant + row-weight vector —
/// everything the challenges depend on precedes them.
#[allow(clippy::arithmetic_side_effects)]
fn absorb_tap_statement(
    transcript: &mut impl Transcript,
    root: &flock_core::merkle::Hash,
    layout: &ShaF2Layout,
    q_bits: usize,
    claims: &[(&[TapOp], &[u128])],
) {
    let mut bytes = Vec::new();
    bytes.push(0x42u8);
    bytes.extend_from_slice(root);
    for v in [
        layout.p.t,
        layout.p.s,
        layout.p.word_bits,
        layout.num_cols,
        layout.log_cols,
        layout.bit_vars,
        layout.num_vars,
        layout.tw,
        layout.x_fold_extra,
        q_bits,
        claims.len(),
    ] {
        bytes.extend_from_slice(&(v as u64).to_le_bytes());
    }
    for (taps, rw) in claims {
        bytes.extend_from_slice(&(taps.len() as u64).to_le_bytes());
        for tap in *taps {
            for v in [
                tap.col as u64,
                tap.grp_log2 as u64,
                tap.bit_amt as u64,
                u64::from(tap.bit_dropout),
                tap.off as u64,
            ] {
                bytes.extend_from_slice(&v.to_le_bytes());
            }
        }
        for &x in *rw {
            bytes.extend_from_slice(&x.to_le_bytes());
        }
    }
    transcript.absorb_slice(&bytes);
}

/// Non-panicking mirror of [`crate::taps::assert_tap`] for verifier
/// shape checks.
fn tap_shape_ok(layout: &ShaF2Layout, t: &TapOp) -> bool {
    t.col < layout.num_cols
        && t.grp_log2 <= layout.p.s
        && (!(t.bit_amt > 0 || t.bit_dropout) || t.bit_amt < (1usize << t.grp_log2))
        && t.off < (1usize << (layout.p.s - t.grp_log2))
}

// ---------------------------------------------------------------------
// EXPERIMENTAL — the single-tap shared-point COLLAPSE
// (docs/rlc-structured-taps-phase0.md; the "weight transform" route).
//
// k claims, each on a SINGLE tap of a committed column (no XOR mixing),
// all at ONE shared evaluation point, collapse to at most
// `#columns × 2` PLAIN single-column claims through the deployed
// claims-only virtual path — no streams, no carry channels, no
// discharge, no translated-eq rings. The identity, per claim `i` with
// tap `(col, g, r, dropout, off)` and forward index map `σ`:
//
//   Σ_p w[p]·tap(a)[p] = Σ_{p'} w[σ(p')]·a[p']·[valid]
//     = Σ_{β∈{0,1}} Σ_{(b,c)} w_row^{(β)}[b]·e^{(β)}[c]·a[(b,c)],
//
// where the group translation acts inside the CLEAR axis (`g ≤ s`), so
// the tensor split survives with the row side shared: branch β = the
// word-carry at the clear/fold boundary, `w_row^{(0)} = w_row`,
// `w_row^{(1)}` = `w_row` advanced one step in `row_hi` (zero at the
// top — the word-overflow dropout), and `e^{(β)}` = the carry-branch-
// masked, group-translated column weights. The γ-RLC then merges all
// claims per (column, branch): `Σ_i γ_i·w'_i = Σ_{(col,β)}
// w_row^{(β)} ⊗ E_{col,β}` with `E_{col,β} = Σ_{i on col} γ_i·e_i^{(β)}
// mod q`. The verifier derives each branch value `y_{col,β}` from the
// proof's own (forest-bound) fold vectors and checks
// `Σ y_{col,β} = T = Σ_i γ_i·c_i`; soundness = 1/q (the γ-combination)
// + the inner path's errors. Fiat–Shamir: statement tag 0x44 (root,
// layout, taps, claimed values, both weight vectors) → γ's → the inner
// claims-only protocol. Evaluation field fixed q = 2^100 − 15.
// ---------------------------------------------------------------------

/// One uniform-op claim at the shared point:
/// `MLE[INT(op(⊕_{i∈cols} a_i))](r) = claimed` — a single op applied
/// OUTSIDE an XOR set of committed columns (`|cols| = 1` recovers the
/// single-tap case; identity ops allowed).
#[derive(Clone, Copy, Debug)]
pub struct TapPointClaim<'a> {
    /// The XOR set of committed columns (nonempty after canonical
    /// pair-cancellation, [`crate::pcs::xor_canonical_cols`]).
    pub cols: &'a [usize],
    /// The uniform op applied outside the XOR.
    pub op: crate::taps::TapUniOp,
    /// The claimed evaluation, canonical in `[0, q)`.
    pub claimed: u128,
}

/// Absorb the collapse statement (domain tag 0x44).
#[allow(clippy::arithmetic_side_effects)]
fn absorb_tap_collapse_statement(
    transcript: &mut impl Transcript,
    root: &flock_core::merkle::Hash,
    layout: &ShaF2Layout,
    row_weights_q: &[u128],
    col_weights: &[crate::pcs::Fq],
    claims: &[TapPointClaim<'_>],
) {
    let mut bytes = Vec::new();
    bytes.push(0x44u8);
    bytes.extend_from_slice(root);
    for v in [
        layout.p.t,
        layout.p.s,
        layout.p.word_bits,
        layout.num_cols,
        layout.log_cols,
        layout.bit_vars,
        layout.num_vars,
        layout.tw,
        layout.x_fold_extra,
        claims.len(),
    ] {
        bytes.extend_from_slice(&(v as u64).to_le_bytes());
    }
    bytes.extend_from_slice(&crate::pcs::FQ_MOD.to_le_bytes());
    for cl in claims {
        bytes.extend_from_slice(&(cl.cols.len() as u64).to_le_bytes());
        for &c in cl.cols {
            bytes.extend_from_slice(&(c as u64).to_le_bytes());
        }
        for v in [
            cl.op.grp_log2 as u64,
            cl.op.bit_amt as u64,
            u64::from(cl.op.bit_dropout),
            cl.op.off as u64,
        ] {
            bytes.extend_from_slice(&v.to_le_bytes());
        }
        bytes.extend_from_slice(&cl.claimed.to_le_bytes());
    }
    for &x in row_weights_q {
        bytes.extend_from_slice(&x.to_le_bytes());
    }
    for w in col_weights {
        bytes.extend_from_slice(&w.0.to_le_bytes());
    }
    transcript.absorb_slice(&bytes);
}

/// The canonical inner-claim plan: (canonical XOR set, carry branch)
/// pairs, sets in lexicographic order, branch 0 before branch 1; branch
/// 1 present iff some claim on the set has a word offset. STRUCTURAL
/// (independent of table values), so prover and verifier derive
/// identical shapes.
fn tap_collapse_plan(claims: &[TapPointClaim<'_>]) -> Vec<(Vec<usize>, usize)> {
    use crate::pcs::xor_canonical_cols;
    let mut sets: Vec<Vec<usize>> =
        claims.iter().map(|c| xor_canonical_cols(c.cols)).collect();
    sets.sort_unstable();
    sets.dedup();
    let mut plan = Vec::new();
    for set in sets {
        let has_off = claims
            .iter()
            .any(|c| xor_canonical_cols(c.cols) == set && c.op.off > 0);
        plan.push((set.clone(), 0));
        if has_off {
            plan.push((set, 1));
        }
    }
    plan
}

/// The v1 δ-envelope for collapse OUTER ops under `x_fold_extra = δ`:
/// the fold cut must sit inside the op's translation-invariant bottom
/// — `δ ≤ g` and `2^δ | bit_amt` (identity ops pass trivially) — so
/// the group/word action never touches the folded low-δ clear bits and
/// the branch structure stays the plain (γ) split with the effective
/// clear-axis fields `g' = g − δ`, `amt' = amt ≫ δ`, `off` unchanged.
fn collapse_op_delta_ok(layout: &ShaF2Layout, op: &crate::taps::TapUniOp) -> bool {
    let delta = layout.x_fold_extra;
    let ident = op.bit_amt == 0 && op.off == 0;
    delta == 0
        || ident
        || (delta <= op.grp_log2 && op.bit_amt.is_multiple_of(1usize << delta))
}

/// Panicking prover-side mirror of [`collapse_op_delta_ok`].
fn assert_collapse_op_delta(layout: &ShaF2Layout, op: &crate::taps::TapUniOp) {
    assert!(
        collapse_op_delta_ok(layout, op),
        "collapse op {op:?} outside the δ-envelope (δ ≤ g and 2^δ | amt) at x_fold_extra = {}",
        layout.x_fold_extra
    );
}

/// Branch-β row weights: β = 0 is `w_row` itself; β = 1 advances the
/// `row_hi` field by one (per block of the variables above it — the
/// word-bit axis and, under `x_fold_extra`, the folded low-δ clear
/// bits, both untouched by the carry), zero at the top — the
/// word-overflow dropout.
#[allow(clippy::arithmetic_side_effects)]
fn tap_collapse_row_weights(layout: &ShaF2Layout, w_row: &[u128], beta: usize) -> Vec<u128> {
    if beta == 0 {
        return w_row.to_vec();
    }
    let tw = layout.tw;
    let mut out = vec![0u128; w_row.len()];
    for hi in 0..w_row.len() >> tw {
        for rh in 0..(1usize << tw) - 1 {
            out[(hi << tw) | rh] = w_row[(hi << tw) | (rh + 1)];
        }
    }
    out
}

/// One claim's carry-branch column-weight tables `e^{(0)}, e^{(1)}`:
/// `e^{(β)}[c] = [carry(c) = β]·[group valid]·e_col[c_out(c)]`, over
/// the clear axis (`row_lo ≫ δ` under `x_fold_extra = δ`, where the
/// δ-envelope reduces the op to `g' = g − δ`, `amt' = amt ≫ δ`).
#[allow(clippy::arithmetic_side_effects)]
fn tap_collapse_col_weights(
    layout: &ShaF2Layout,
    op: &crate::taps::TapUniOp,
    e_col: &[crate::pcs::Fq],
) -> [Vec<u128>; 2] {
    let delta = layout.x_fold_extra;
    debug_assert!(collapse_op_delta_ok(layout, op));
    let s = layout.p.s - delta;
    let (g, amt) = if delta <= op.grp_log2 {
        (op.grp_log2 - delta, op.bit_amt >> delta)
    } else {
        (0, 0) // identity op (the envelope guarantees amt = off = 0)
    };
    let n_g = 1usize << g;
    let n_w = 1usize << (s - g);
    let mut out = [vec![0u128; 1 << s], vec![0u128; 1 << s]];
    for c in 0..1usize << s {
        let j = c & (n_g - 1);
        let wlo = c >> g;
        let jout = if op.bit_dropout {
            if j + amt >= n_g {
                continue;
            }
            j + amt
        } else {
            (j + amt) & (n_g - 1)
        };
        let (wout, beta) = if wlo + op.off >= n_w {
            (wlo + op.off - n_w, 1usize)
        } else {
            (wlo + op.off, 0usize)
        };
        out[beta][c] = e_col[(wout << g) | jout].0;
    }
    out
}

/// The γ-combined per-(set, branch) column weights, in plan order.
#[allow(clippy::arithmetic_side_effects)]
fn tap_collapse_combined_cols(
    layout: &ShaF2Layout,
    claims: &[TapPointClaim<'_>],
    gammas: &[u128],
    e_col: &[crate::pcs::Fq],
    plan: &[(Vec<usize>, usize)],
) -> Vec<Vec<crate::pcs::Fq>> {
    use crate::pcs::{Fq, fq_add, fq_mul, xor_canonical_cols};
    let mut acc: Vec<Vec<u128>> = plan.iter().map(|_| vec![0u128; e_col.len()]).collect();
    for (cl, &gam) in claims.iter().zip(gammas.iter()) {
        let branches = tap_collapse_col_weights(layout, &cl.op, e_col);
        let set = xor_canonical_cols(cl.cols);
        for (pi, (pset, beta)) in plan.iter().enumerate() {
            if *pset != set {
                continue;
            }
            for (a, &e) in acc[pi].iter_mut().zip(branches[*beta].iter()) {
                *a = fq_add(*a, fq_mul(gam, e));
            }
        }
    }
    acc.into_iter().map(|v| v.into_iter().map(Fq::from).collect()).collect()
}

/// Prove k uniform-op claims (`op(⊕ cols)`) at ONE shared point by the
/// weight-transform collapse: at most `#distinct-XOR-sets × 2` plain
/// virtual-XOR claims through
/// [`prove_mle_eval_mod_q_ligerito_claims_only`] (EXPERIMENTAL; see the
/// section comment). Returns the inner claims-only proof.
#[allow(clippy::arithmetic_side_effects)]
pub fn prove_mle_eval_mod_q_ligerito_tap_collapse(
    transcript: &mut (impl Transcript + Send),
    hint: &FlockCommitHint,
    layout: &ShaF2Layout,
    row_weights_q: &[u128],
    col_weights: &[crate::pcs::Fq],
    claims: &[TapPointClaim<'_>],
    alpha: Gf,
    pc: &LigProverConfig,
) -> IntEvalRsLigModQXorProof {
    use crate::pcs::{FQ_BITS, FQ_MOD, fq_challenge, virtual_xor_params, xor_canonical_cols};
    use crate::taps::{assert_tap_layout, assert_tap_op};
    assert_tap_layout(layout);
    assert!(!claims.is_empty(), "need at least one claim");
    let p_x = virtual_xor_params(layout);
    assert_eq!(row_weights_q.len(), p_x.rows(), "shared row-weight length");
    assert_eq!(col_weights.len(), p_x.cols(), "shared col-weight length");
    for cl in claims {
        assert_tap_op(layout, &cl.op);
        assert_collapse_op_delta(layout, &cl.op);
        assert!(
            !xor_canonical_cols(cl.cols).is_empty(),
            "claim's XOR set cancels to the zero vector"
        );
        for &c in cl.cols {
            assert!(c < layout.num_cols, "claim column {c} out of range");
        }
        assert!(cl.claimed < FQ_MOD, "claimed value must be canonical");
    }
    absorb_tap_collapse_statement(
        transcript,
        hint.root(),
        layout,
        row_weights_q,
        col_weights,
        claims,
    );
    // γ's are drawn for transcript parity; the prover's inner claims are
    // γ-independent (the combination lives in the verifier's read-off).
    let _gammas: Vec<u128> = (0..claims.len()).map(|_| fq_challenge(transcript)).collect();
    let plan = tap_collapse_plan(claims);
    let branch_rows: Vec<Vec<u128>> = plan
        .iter()
        .map(|(_, beta)| tap_collapse_row_weights(layout, row_weights_q, *beta))
        .collect();
    let vx: Vec<VirtualXorClaim<'_>> = plan
        .iter()
        .enumerate()
        .map(|(pi, (set, _))| VirtualXorClaim {
            cols: set,
            constant: 0,
            external_rows: None,
            row_weights_q: &branch_rows[pi],
        })
        .collect();
    prove_mle_eval_mod_q_ligerito_claims_only(transcript, hint, layout, FQ_BITS, &vx, alpha, pc)
}

/// Verify a uniform-op shared-point collapse (EXPERIMENTAL): derives the
/// branch values from the proof's own fold vectors, checks their sum
/// against `T = Σ γ_i·c_i`, and runs the inner claims-only verifier.
#[allow(clippy::arithmetic_side_effects)]
#[allow(clippy::too_many_arguments)]
pub fn verify_mle_eval_mod_q_ligerito_tap_collapse(
    transcript: &mut (impl Transcript + Send),
    commitment: &Commitment,
    proof: &IntEvalRsLigModQXorProof,
    layout: &ShaF2Layout,
    row_weights_q: &[u128],
    col_weights: &[crate::pcs::Fq],
    claims: &[TapPointClaim<'_>],
    alpha: Gf,
    vc: &LigVerifierConfig,
) -> Result<(), FlockRsError> {
    use crate::pcs::{
        FQ_BITS, FQ_MOD, Fq, fq_add, fq_challenge, fq_mul, mod_q_chunk_width, mod_q_num_chunks,
        recombine_read_off, virtual_xor_params, xor_canonical_cols,
    };
    if layout.p.word_bits != 1
        || layout.x_fold_extra >= layout.p.s
        || (layout.x_fold_extra > 0 && layout.bit_vars.wrapping_add(layout.tw) < 6)
        || layout.tw + layout.log_cols < 7
    {
        return Err(FlockRsError::RingSwitch(RsOpenError::Shape));
    }
    let p_x = virtual_xor_params(layout);
    if claims.is_empty()
        || row_weights_q.len() != p_x.rows()
        || col_weights.len() != p_x.cols()
        || claims.iter().any(|cl| {
            !tap_shape_ok(layout, &cl.op.with_col(0))
                || !collapse_op_delta_ok(layout, &cl.op)
                || cl.claimed >= FQ_MOD
                || xor_canonical_cols(cl.cols).is_empty()
                || cl.cols.iter().any(|&c| c >= layout.num_cols)
        })
    {
        return Err(FlockRsError::RingSwitch(RsOpenError::Shape));
    }
    absorb_tap_collapse_statement(
        transcript,
        &commitment.root,
        layout,
        row_weights_q,
        col_weights,
        claims,
    );
    let gammas: Vec<u128> = (0..claims.len()).map(|_| fq_challenge(transcript)).collect();
    let target = claims
        .iter()
        .zip(gammas.iter())
        .fold(0u128, |acc, (cl, &g)| fq_add(acc, fq_mul(g, cl.claimed)));
    let plan = tap_collapse_plan(claims);
    let combined = tap_collapse_combined_cols(layout, claims, &gammas, col_weights, &plan);

    // Branch values from the proof's (forest-bound) fold vectors; their
    // sum must reproduce the combined target.
    let c_w_x = mod_q_chunk_width(&p_x);
    let lch_x = mod_q_num_chunks(&p_x, FQ_BITS);
    if proof.xors.len() != plan.len()
        || proof.xors.iter().any(|xs| xs.us.len() != lch_x)
    {
        return Err(FlockRsError::RingSwitch(RsOpenError::Shape));
    }
    let ys: Vec<Fq> = (0..plan.len())
        .map(|pi| {
            let flat: Vec<u128> =
                proof.xors[pi].us.iter().flat_map(|u| u.iter().copied()).collect();
            recombine_read_off(&p_x, &flat, 0, &combined[pi], c_w_x, lch_x)
        })
        .collect();
    let total = ys.iter().fold(0u128, |acc, y| fq_add(acc, y.0));
    if total != target {
        return Err(FlockRsError::Common(IntEvalRsError::ReadOff));
    }

    let branch_rows: Vec<Vec<u128>> = plan
        .iter()
        .map(|(_, beta)| tap_collapse_row_weights(layout, row_weights_q, *beta))
        .collect();
    let vx: Vec<VirtualXorVerifyClaim<'_, Fq>> = plan
        .iter()
        .enumerate()
        .map(|(pi, (set, _))| VirtualXorVerifyClaim {
            cols: set,
            constant: 0,
            has_external: false,
            row_weights_q: &branch_rows[pi],
            col_weights: &combined[pi],
            claimed: ys[pi],
        })
        .collect();
    let obligations = verify_mle_eval_mod_q_ligerito_claims_only(
        transcript,
        commitment,
        proof,
        layout,
        alpha,
        FQ_BITS,
        &vx,
        vc,
    )?;
    debug_assert!(obligations.is_empty(), "no external terms in the collapse");
    Ok(())
}

// ---------------------------------------------------------------------
// EXPERIMENTAL — the COMPOSED collapse: uniform outer ops over MIXED
// (XOR-of-taps) sources at ONE shared point
// (docs/rlc-structured-taps-phase0.md §6).
//
// The 0x44 collapse identity never used that its source is a plain
// column XOR — only that the inner claim `⟨w∘σ, x⟩` is provable. Here
// the source is a fixed XOR-of-taps combination
// `x_S = ⊕_{t∈S} op_t(a_{i_t})` (extractable; provable by the 0x42
// tap-claims path), and k claims `OUTER_i(x_{S_i})` at ONE shared point
// γ-collapse to at most `#distinct-sources × 2` INNER TAP CLAIMS —
// independent of k. The branch split of the transformed weight depends
// only on `(OUTER, layout)`, never on the source: branch 0 keeps the
// row weights, branch 1 advances `row_hi` one step (zero at the top),
// and the column side is the branch-masked, group-translated table
// (`tap_collapse_row_weights` / `tap_collapse_col_weights`, verbatim).
// The verifier derives each branch value from the proof's own
// forest-bound fold vectors with the γ-combined column weights
// `E_{S,β} = Σ_{i on S} γ_i·e_i^{(β)} mod q` and checks
// `Σ y_{S,β} = T = Σ_i γ_i·c_i`; soundness = 1/q (the γ-combination)
// + the inner tap path's errors. Shift-invariant (schedule-shaped)
// workloads — every claim a word-offset `off^t` of ONE mixed
// combination — cost TWO inner bodies total versus one padded forest
// body per claim on the batched path. The outer op's envelope
// (`off < 2^{s−g}`) stands ALONE — it does not compound with the source
// taps' offsets (the transform treats `x_S` as a black box).
// Fiat–Shamir: statement tag 0x45 (root, layout, sources + outer ops +
// claimed values, both weight vectors) → γ's → the inner 0x42 protocol.
// Evaluation field fixed q = 2^100 − 15.
// ---------------------------------------------------------------------

/// One composed claim at the shared point:
/// `MLE[INT(outer(⊕_{t∈source} op_t(a_{col_t})))](r) = claimed` — a
/// uniform op applied OUTSIDE a fixed XOR-of-taps source combination.
/// An all-identity source recovers [`TapPointClaim`]; prefer the 0x44
/// path there (its inner bodies are plain claims with no rings).
#[derive(Clone, Copy, Debug)]
pub struct TapComposedClaim<'a> {
    /// The source combination's tap terms (nonempty after
    /// [`crate::taps::tap_canonical_ops`]).
    pub source: &'a [TapOp],
    /// The uniform op applied outside the source.
    pub outer: crate::taps::TapUniOp,
    /// The claimed evaluation, canonical in `[0, q)`.
    pub claimed: u128,
}

/// Absorb the composed-collapse statement (domain tag 0x45).
#[allow(clippy::arithmetic_side_effects)]
fn absorb_tap_composed_statement(
    transcript: &mut impl Transcript,
    root: &flock_core::merkle::Hash,
    layout: &ShaF2Layout,
    row_weights_q: &[u128],
    col_weights: &[crate::pcs::Fq],
    claims: &[TapComposedClaim<'_>],
) {
    let mut bytes = Vec::new();
    bytes.push(0x45u8);
    bytes.extend_from_slice(root);
    for v in [
        layout.p.t,
        layout.p.s,
        layout.p.word_bits,
        layout.num_cols,
        layout.log_cols,
        layout.bit_vars,
        layout.num_vars,
        layout.tw,
        layout.x_fold_extra,
        claims.len(),
    ] {
        bytes.extend_from_slice(&(v as u64).to_le_bytes());
    }
    bytes.extend_from_slice(&crate::pcs::FQ_MOD.to_le_bytes());
    for cl in claims {
        bytes.extend_from_slice(&(cl.source.len() as u64).to_le_bytes());
        for tap in cl.source {
            for v in [
                tap.col as u64,
                tap.grp_log2 as u64,
                tap.bit_amt as u64,
                u64::from(tap.bit_dropout),
                tap.off as u64,
            ] {
                bytes.extend_from_slice(&v.to_le_bytes());
            }
        }
        for v in [
            cl.outer.grp_log2 as u64,
            cl.outer.bit_amt as u64,
            u64::from(cl.outer.bit_dropout),
            cl.outer.off as u64,
        ] {
            bytes.extend_from_slice(&v.to_le_bytes());
        }
        bytes.extend_from_slice(&cl.claimed.to_le_bytes());
    }
    for &x in row_weights_q {
        bytes.extend_from_slice(&x.to_le_bytes());
    }
    for w in col_weights {
        bytes.extend_from_slice(&w.0.to_le_bytes());
    }
    transcript.absorb_slice(&bytes);
}

/// The composed inner-claim plan: (canonical source, carry branch)
/// pairs, sources in lexicographic tap order, branch 0 before branch 1;
/// branch 1 present iff some claim on the source has an outer word
/// offset. STRUCTURAL (independent of table values), so prover and
/// verifier derive identical shapes.
fn tap_composed_plan(claims: &[TapComposedClaim<'_>]) -> Vec<(Vec<TapOp>, usize)> {
    use crate::taps::{tap_canonical_ops, tap_sort_key};
    let mut sources: Vec<Vec<TapOp>> =
        claims.iter().map(|c| tap_canonical_ops(c.source)).collect();
    sources.sort_unstable_by(|a, b| a.iter().map(tap_sort_key).cmp(b.iter().map(tap_sort_key)));
    sources.dedup();
    let mut plan = Vec::new();
    for src in sources {
        let has_off = claims
            .iter()
            .any(|c| tap_canonical_ops(c.source) == src && c.outer.off > 0);
        plan.push((src.clone(), 0));
        if has_off {
            plan.push((src, 1));
        }
    }
    plan
}

/// Prove k composed claims (`outer(⊕ source-taps)`) at ONE shared point
/// by the weight-transform collapse: at most `#distinct-sources × 2`
/// inner tap claims through
/// [`prove_mle_eval_mod_q_ligerito_tap_claims`] (EXPERIMENTAL; see the
/// section comment). Returns the inner tap-claims proof.
#[allow(clippy::arithmetic_side_effects)]
pub fn prove_mle_eval_mod_q_ligerito_tap_composed(
    transcript: &mut (impl Transcript + Send),
    hint: &FlockCommitHint,
    layout: &ShaF2Layout,
    row_weights_q: &[u128],
    col_weights: &[crate::pcs::Fq],
    claims: &[TapComposedClaim<'_>],
    alpha: Gf,
    pc: &LigProverConfig,
) -> IntEvalRsLigModQTapProof {
    use crate::pcs::{FQ_BITS, FQ_MOD, fq_challenge, virtual_xor_params};
    use crate::taps::{assert_tap, assert_tap_layout, assert_tap_op, tap_canonical_ops};
    assert_tap_layout(layout);
    assert!(!claims.is_empty(), "need at least one claim");
    let p_x = virtual_xor_params(layout);
    assert_eq!(row_weights_q.len(), p_x.rows(), "shared row-weight length");
    assert_eq!(col_weights.len(), p_x.cols(), "shared col-weight length");
    for cl in claims {
        assert_tap_op(layout, &cl.outer);
        assert_collapse_op_delta(layout, &cl.outer);
        for tap in cl.source {
            assert_tap(layout, tap);
        }
        assert!(
            !tap_canonical_ops(cl.source).is_empty(),
            "claim's source combination cancels to the zero vector"
        );
        assert!(cl.claimed < FQ_MOD, "claimed value must be canonical");
    }
    absorb_tap_composed_statement(
        transcript,
        hint.root(),
        layout,
        row_weights_q,
        col_weights,
        claims,
    );
    // γ's are drawn for transcript parity; the prover's inner claims are
    // γ-independent (the combination lives in the verifier's read-off).
    let _gammas: Vec<u128> = (0..claims.len()).map(|_| fq_challenge(transcript)).collect();
    let plan = tap_composed_plan(claims);
    let branch_rows: Vec<Vec<u128>> = plan
        .iter()
        .map(|(_, beta)| tap_collapse_row_weights(layout, row_weights_q, *beta))
        .collect();
    let inner: Vec<TapClaim<'_>> = plan
        .iter()
        .enumerate()
        .map(|(pi, (src, _))| TapClaim { taps: src, row_weights_q: &branch_rows[pi] })
        .collect();
    prove_mle_eval_mod_q_ligerito_tap_claims(transcript, hint, layout, FQ_BITS, &inner, alpha, pc)
}

/// Verify a composed collapse (EXPERIMENTAL): derives the per-(source,
/// branch) values from the proof's own fold vectors, checks their sum
/// against `T = Σ γ_i·c_i`, and runs the inner tap-claims verifier.
#[allow(clippy::arithmetic_side_effects)]
#[allow(clippy::too_many_arguments)]
pub fn verify_mle_eval_mod_q_ligerito_tap_composed(
    transcript: &mut (impl Transcript + Send),
    commitment: &Commitment,
    proof: &IntEvalRsLigModQTapProof,
    layout: &ShaF2Layout,
    row_weights_q: &[u128],
    col_weights: &[crate::pcs::Fq],
    claims: &[TapComposedClaim<'_>],
    alpha: Gf,
    vc: &LigVerifierConfig,
) -> Result<(), FlockRsError> {
    use crate::pcs::{
        FQ_BITS, FQ_MOD, Fq, fq_add, fq_challenge, fq_mul, mod_q_chunk_width, mod_q_num_chunks,
        recombine_read_off, virtual_xor_params,
    };
    use crate::taps::tap_canonical_ops;
    if layout.p.word_bits != 1
        || layout.x_fold_extra >= layout.p.s
        || (layout.x_fold_extra > 0 && layout.bit_vars.wrapping_add(layout.tw) < 6)
        || layout.tw + layout.log_cols < 7
    {
        return Err(FlockRsError::RingSwitch(RsOpenError::Shape));
    }
    let p_x = virtual_xor_params(layout);
    if claims.is_empty()
        || row_weights_q.len() != p_x.rows()
        || col_weights.len() != p_x.cols()
        || claims.iter().any(|cl| {
            !tap_shape_ok(layout, &cl.outer.with_col(0))
                || !collapse_op_delta_ok(layout, &cl.outer)
                || cl.source.iter().any(|t| !tap_shape_ok(layout, t))
                || cl.claimed >= FQ_MOD
                || tap_canonical_ops(cl.source).is_empty()
        })
    {
        return Err(FlockRsError::RingSwitch(RsOpenError::Shape));
    }
    absorb_tap_composed_statement(
        transcript,
        &commitment.root,
        layout,
        row_weights_q,
        col_weights,
        claims,
    );
    let gammas: Vec<u128> = (0..claims.len()).map(|_| fq_challenge(transcript)).collect();
    let target = claims
        .iter()
        .zip(gammas.iter())
        .fold(0u128, |acc, (cl, &g)| fq_add(acc, fq_mul(g, cl.claimed)));
    let plan = tap_composed_plan(claims);
    // The γ-combined per-(source, branch) column weights, in plan order.
    let mut acc: Vec<Vec<u128>> =
        plan.iter().map(|_| vec![0u128; col_weights.len()]).collect();
    for (cl, &gam) in claims.iter().zip(gammas.iter()) {
        let branches = tap_collapse_col_weights(layout, &cl.outer, col_weights);
        let src = tap_canonical_ops(cl.source);
        for (pi, (psrc, beta)) in plan.iter().enumerate() {
            if *psrc != src {
                continue;
            }
            for (a, &e) in acc[pi].iter_mut().zip(branches[*beta].iter()) {
                *a = fq_add(*a, fq_mul(gam, e));
            }
        }
    }
    let combined: Vec<Vec<Fq>> =
        acc.into_iter().map(|v| v.into_iter().map(Fq::from).collect()).collect();

    // Branch values from the proof's (forest-bound) fold vectors; their
    // sum must reproduce the combined target.
    let c_w_x = mod_q_chunk_width(&p_x);
    let lch_x = mod_q_num_chunks(&p_x, FQ_BITS);
    if proof.tap_us.len() != plan.len() || proof.tap_us.iter().any(|us| us.len() != lch_x) {
        return Err(FlockRsError::RingSwitch(RsOpenError::Shape));
    }
    let ys: Vec<Fq> = (0..plan.len())
        .map(|pi| {
            let flat: Vec<u128> =
                proof.tap_us[pi].iter().flat_map(|u| u.iter().copied()).collect();
            recombine_read_off(&p_x, &flat, 0, &combined[pi], c_w_x, lch_x)
        })
        .collect();
    let total = ys.iter().fold(0u128, |acc, y| fq_add(acc, y.0));
    if total != target {
        return Err(FlockRsError::Common(IntEvalRsError::ReadOff));
    }

    let branch_rows: Vec<Vec<u128>> = plan
        .iter()
        .map(|(_, beta)| tap_collapse_row_weights(layout, row_weights_q, *beta))
        .collect();
    let inner: Vec<TapVerifyClaim<'_, Fq>> = plan
        .iter()
        .enumerate()
        .map(|(pi, (src, _))| TapVerifyClaim {
            taps: src,
            row_weights_q: &branch_rows[pi],
            col_weights: &combined[pi],
            claimed: ys[pi],
        })
        .collect();
    verify_mle_eval_mod_q_ligerito_tap_claims(
        transcript, commitment, proof, layout, alpha, FQ_BITS, &inner, vc,
    )
}

// ---------------------------------------------------------------------
// EXPERIMENTAL — the MULTIWEIGHT collapse: per-claim COLUMN weights
// (docs/blake3-taps-design.md, the P-LIN layer).
//
// The 0x44 collapse fixes ONE shared (row, column) weight pair and lets
// claims differ only by their op. Its verifier, however, already
// γ-combines PER-CLAIM branch tables (`E_{set,β} = Σ γᵢ·eᵢ^{(β)}`) —
// nothing in the argument needs the eᵢ to be transforms of one shared
// vector. Generalization: each claim carries its OWN column-weight
// vector (masks, place values, public permutations — arbitrary), while
// the ROW weights stay shared per branch; k claims still γ-collapse to
// at most `#distinct-XOR-sets × 2` plain inner bodies. This is the
// missing piece that lets EVERY integer-linear relation of a system
// (additions with carries, boundary reads, permuted schedule reads)
// ride the same per-(column, branch) bodies: word-level linear
// identities are Schwartz–Zippel zero-checks whose per-column reads
// differ only in column weights. Soundness: the 0x44 chain verbatim
// (γ drawn after the statement absorbs every weight vector; 1/q + the
// inner claims-only path's errors). Fiat–Shamir: statement tag 0x46
// (root, layout, per-claim set + op + col_weights + claimed, shared
// row weights) → γ's → the inner claims-only protocol. Evaluation
// field fixed q = 2^100 − 15.
// ---------------------------------------------------------------------

/// One weighted uniform-op claim at the shared ROW point:
/// `Σ_p (w_row ⊗ e_i)[p]·op(⊕_{c∈cols} a_c)[p] = claimed` — as
/// [`TapPointClaim`] but with per-claim column weights `e_i`.
#[derive(Clone, Copy, Debug)]
pub struct TapWeightedClaim<'a> {
    /// The XOR set of committed columns (nonempty after
    /// [`crate::pcs::xor_canonical_cols`]).
    pub cols: &'a [usize],
    /// The uniform op applied outside the XOR.
    pub op: crate::taps::TapUniOp,
    /// This claim's OWN column weights, length `2^{s−δ}`.
    pub col_weights: &'a [crate::pcs::Fq],
    /// The claimed evaluation, canonical in `[0, q)`.
    pub claimed: u128,
}

/// Absorb the multiweight-collapse statement (domain tag 0x46).
#[allow(clippy::arithmetic_side_effects)]
fn absorb_tap_multiweight_statement(
    transcript: &mut impl Transcript,
    root: &flock_core::merkle::Hash,
    layout: &ShaF2Layout,
    row_weights_q: &[u128],
    claims: &[TapWeightedClaim<'_>],
) {
    let mut bytes = Vec::new();
    bytes.push(0x46u8);
    bytes.extend_from_slice(root);
    for v in [
        layout.p.t,
        layout.p.s,
        layout.p.word_bits,
        layout.num_cols,
        layout.log_cols,
        layout.bit_vars,
        layout.num_vars,
        layout.tw,
        layout.x_fold_extra,
        claims.len(),
    ] {
        bytes.extend_from_slice(&(v as u64).to_le_bytes());
    }
    bytes.extend_from_slice(&crate::pcs::FQ_MOD.to_le_bytes());
    for cl in claims {
        bytes.extend_from_slice(&(cl.cols.len() as u64).to_le_bytes());
        for &c in cl.cols {
            bytes.extend_from_slice(&(c as u64).to_le_bytes());
        }
        for v in [
            cl.op.grp_log2 as u64,
            cl.op.bit_amt as u64,
            u64::from(cl.op.bit_dropout),
            cl.op.off as u64,
        ] {
            bytes.extend_from_slice(&v.to_le_bytes());
        }
        for w in cl.col_weights {
            bytes.extend_from_slice(&w.0.to_le_bytes());
        }
        bytes.extend_from_slice(&cl.claimed.to_le_bytes());
    }
    for &x in row_weights_q {
        bytes.extend_from_slice(&x.to_le_bytes());
    }
    transcript.absorb_slice(&bytes);
}

/// The multiweight plan — identical shape law to [`tap_collapse_plan`]
/// (per canonical XOR set, branch 0 then 1; branch 1 iff some claim on
/// the set has a word offset).
fn tap_multiweight_plan(claims: &[TapWeightedClaim<'_>]) -> Vec<(Vec<usize>, usize)> {
    use crate::pcs::xor_canonical_cols;
    let mut sets: Vec<Vec<usize>> =
        claims.iter().map(|c| xor_canonical_cols(c.cols)).collect();
    sets.sort_unstable();
    sets.dedup();
    let mut plan = Vec::new();
    for set in sets {
        let has_off = claims
            .iter()
            .any(|c| xor_canonical_cols(c.cols) == set && c.op.off > 0);
        plan.push((set.clone(), 0));
        if has_off {
            plan.push((set, 1));
        }
    }
    plan
}

/// Prove k weighted uniform-op claims at ONE shared ROW point
/// (EXPERIMENTAL; see the section comment): at most
/// `#distinct-XOR-sets × 2` plain inner claims through
/// [`prove_mle_eval_mod_q_ligerito_claims_only`]. The prover never
/// touches the column weights (the combination lives in the verifier's
/// read-off), so the body is the 0x44 prover under the 0x46 statement.
#[allow(clippy::arithmetic_side_effects)]
pub fn prove_mle_eval_mod_q_ligerito_tap_multiweight(
    transcript: &mut (impl Transcript + Send),
    hint: &FlockCommitHint,
    layout: &ShaF2Layout,
    row_weights_q: &[u128],
    claims: &[TapWeightedClaim<'_>],
    alpha: Gf,
    pc: &LigProverConfig,
) -> IntEvalRsLigModQXorProof {
    use crate::pcs::{FQ_BITS, FQ_MOD, fq_challenge, virtual_xor_params, xor_canonical_cols};
    use crate::taps::{assert_tap_layout, assert_tap_op};
    assert_tap_layout(layout);
    assert!(!claims.is_empty(), "need at least one claim");
    let p_x = virtual_xor_params(layout);
    assert_eq!(row_weights_q.len(), p_x.rows(), "shared row-weight length");
    for cl in claims {
        assert_tap_op(layout, &cl.op);
        assert_collapse_op_delta(layout, &cl.op);
        assert_eq!(cl.col_weights.len(), p_x.cols(), "claim col-weight length");
        assert!(
            !xor_canonical_cols(cl.cols).is_empty(),
            "claim's XOR set cancels to the zero vector"
        );
        for &c in cl.cols {
            assert!(c < layout.num_cols, "claim column {c} out of range");
        }
        assert!(cl.claimed < FQ_MOD, "claimed value must be canonical");
    }
    absorb_tap_multiweight_statement(transcript, hint.root(), layout, row_weights_q, claims);
    // γ's for transcript parity; the inner claims are γ-independent.
    let _gammas: Vec<u128> = (0..claims.len()).map(|_| fq_challenge(transcript)).collect();
    let plan = tap_multiweight_plan(claims);
    let branch_rows: Vec<Vec<u128>> = plan
        .iter()
        .map(|(_, beta)| tap_collapse_row_weights(layout, row_weights_q, *beta))
        .collect();
    let vx: Vec<VirtualXorClaim<'_>> = plan
        .iter()
        .enumerate()
        .map(|(pi, (set, _))| VirtualXorClaim {
            cols: set,
            constant: 0,
            external_rows: None,
            row_weights_q: &branch_rows[pi],
        })
        .collect();
    prove_mle_eval_mod_q_ligerito_claims_only(transcript, hint, layout, FQ_BITS, &vx, alpha, pc)
}

/// Verify a multiweight collapse (EXPERIMENTAL): γ-combines each
/// claim's OWN branch column-weight tables per (set, branch), derives
/// the branch values from the proof's fold vectors, checks their sum
/// against `T = Σ γᵢ·cᵢ`, and runs the inner claims-only verifier.
#[allow(clippy::arithmetic_side_effects)]
#[allow(clippy::too_many_arguments)]
pub fn verify_mle_eval_mod_q_ligerito_tap_multiweight(
    transcript: &mut (impl Transcript + Send),
    commitment: &Commitment,
    proof: &IntEvalRsLigModQXorProof,
    layout: &ShaF2Layout,
    row_weights_q: &[u128],
    claims: &[TapWeightedClaim<'_>],
    alpha: Gf,
    vc: &LigVerifierConfig,
) -> Result<(), FlockRsError> {
    use crate::pcs::{
        FQ_BITS, FQ_MOD, Fq, fq_add, fq_challenge, fq_mul, mod_q_chunk_width, mod_q_num_chunks,
        recombine_read_off, virtual_xor_params, xor_canonical_cols,
    };
    if layout.p.word_bits != 1
        || layout.x_fold_extra >= layout.p.s
        || (layout.x_fold_extra > 0 && layout.bit_vars.wrapping_add(layout.tw) < 6)
        || layout.tw + layout.log_cols < 7
    {
        return Err(FlockRsError::RingSwitch(RsOpenError::Shape));
    }
    let p_x = virtual_xor_params(layout);
    if claims.is_empty()
        || row_weights_q.len() != p_x.rows()
        || claims.iter().any(|cl| {
            !tap_shape_ok(layout, &cl.op.with_col(0))
                || !collapse_op_delta_ok(layout, &cl.op)
                || cl.col_weights.len() != p_x.cols()
                || cl.claimed >= FQ_MOD
                || xor_canonical_cols(cl.cols).is_empty()
                || cl.cols.iter().any(|&c| c >= layout.num_cols)
        })
    {
        return Err(FlockRsError::RingSwitch(RsOpenError::Shape));
    }
    absorb_tap_multiweight_statement(transcript, &commitment.root, layout, row_weights_q, claims);
    let gammas: Vec<u128> = (0..claims.len()).map(|_| fq_challenge(transcript)).collect();
    let target = claims
        .iter()
        .zip(gammas.iter())
        .fold(0u128, |acc, (cl, &g)| fq_add(acc, fq_mul(g, cl.claimed)));
    let plan = tap_multiweight_plan(claims);
    // Per-(set, branch) combined column weights from each claim's OWN
    // weight vector.
    let mut acc: Vec<Vec<u128>> =
        plan.iter().map(|_| vec![0u128; p_x.cols()]).collect();
    for (cl, &gam) in claims.iter().zip(gammas.iter()) {
        let branches = tap_collapse_col_weights(layout, &cl.op, cl.col_weights);
        let set = xor_canonical_cols(cl.cols);
        for (pi, (pset, beta)) in plan.iter().enumerate() {
            if *pset != set {
                continue;
            }
            for (a, &e) in acc[pi].iter_mut().zip(branches[*beta].iter()) {
                *a = fq_add(*a, fq_mul(gam, e));
            }
        }
    }
    let combined: Vec<Vec<Fq>> =
        acc.into_iter().map(|v| v.into_iter().map(Fq::from).collect()).collect();

    let c_w_x = mod_q_chunk_width(&p_x);
    let lch_x = mod_q_num_chunks(&p_x, FQ_BITS);
    if proof.xors.len() != plan.len() || proof.xors.iter().any(|xs| xs.us.len() != lch_x) {
        return Err(FlockRsError::RingSwitch(RsOpenError::Shape));
    }
    let ys: Vec<Fq> = (0..plan.len())
        .map(|pi| {
            let flat: Vec<u128> =
                proof.xors[pi].us.iter().flat_map(|u| u.iter().copied()).collect();
            recombine_read_off(&p_x, &flat, 0, &combined[pi], c_w_x, lch_x)
        })
        .collect();
    let total = ys.iter().fold(0u128, |acc, y| fq_add(acc, y.0));
    if total != target {
        return Err(FlockRsError::Common(IntEvalRsError::ReadOff));
    }

    let branch_rows: Vec<Vec<u128>> = plan
        .iter()
        .map(|(_, beta)| tap_collapse_row_weights(layout, row_weights_q, *beta))
        .collect();
    let vx: Vec<VirtualXorVerifyClaim<'_, Fq>> = plan
        .iter()
        .enumerate()
        .map(|(pi, (set, _))| VirtualXorVerifyClaim {
            cols: set,
            constant: 0,
            has_external: false,
            row_weights_q: &branch_rows[pi],
            col_weights: &combined[pi],
            claimed: ys[pi],
        })
        .collect();
    let obligations = verify_mle_eval_mod_q_ligerito_claims_only(
        transcript,
        commitment,
        proof,
        layout,
        alpha,
        FQ_BITS,
        &vx,
        vc,
    )?;
    debug_assert!(obligations.is_empty(), "no external terms in the collapse");
    Ok(())
}

/// The deterministic ring plan of one claim at one exit point: members =
/// (tap index, class) in canonical order, grouped into buckets by
/// in-pack-table equality (first-occurrence order). Both sides derive it.
struct TapRingPlan {
    /// Per member: (tap index, class, A table).
    members: Vec<(usize, crate::taps::TapClass, Vec<Gf>)>,
    /// Buckets: member positions sharing one ring message.
    buckets: Vec<Vec<usize>>,
}

fn tap_ring_plan(layout: &ShaF2Layout, taps: &[TapOp], pt_x: &[Gf]) -> TapRingPlan {
    use crate::taps::{tap_classes, tap_inpack_table};
    let mut members = Vec::new();
    for (ti, tap) in taps.iter().enumerate() {
        for cl in tap_classes(layout, tap) {
            let a = tap_inpack_table(layout, tap, pt_x, cl);
            members.push((ti, cl, a));
        }
    }
    let mut buckets: Vec<Vec<usize>> = Vec::new();
    for (mi, (_, _, a)) in members.iter().enumerate() {
        match buckets.iter_mut().find(|b| members[b[0]].2 == *a) {
            Some(b) => b.push(mi),
            None => buckets.push(vec![mi]),
        }
    }
    TapRingPlan { members, buckets }
}

/// One member's sparse in-pack marginal: `s[v] = Σ_y B_β(y)·bit_v(P[y])`
/// over the tap's translated support (band-restricted; zero weights
/// skipped).
#[allow(clippy::arithmetic_side_effects)]
fn tap_ring_walk(
    layout: &ShaF2Layout,
    p_msg: &[F128],
    tap: &TapOp,
    sup: &crate::taps::TapSupportTables,
) -> Vec<Gf> {
    let p0 = xor_support_prefix(layout);
    let bv = layout.bit_vars;
    let w = 1usize << bv;
    let hs = sup.hs;
    let mut s = vec![Gf::zero(); 128];
    for lo in sup.lo_band.0..sup.lo_band.1 {
        let vlo = sup.t_lo[lo];
        if vlo.is_zero() {
            continue;
        }
        for j in 0..w {
            let vj = vlo * sup.t_mid[j];
            if vj.is_zero() {
                continue;
            }
            let base = (lo << (bv + hs)) | (j << hs);
            for h in 0..1usize << hs {
                let e = vj * sup.t_hi[h];
                if e.is_zero() {
                    continue;
                }
                let y = embed_xor_index(layout, (base | h) << p0, tap.col) >> LOG_PACKING;
                let pe = p_msg[y];
                for wi in 0..2usize {
                    let mut bits = if wi == 0 { pe.lo } else { pe.hi };
                    while bits != 0 {
                        let t = bits.trailing_zeros() as usize;
                        s[(wi << 6) | t] += e;
                        bits &= bits.wrapping_sub(1);
                    }
                }
            }
        }
    }
    s
}

/// One member's basis-fill: `b[y] += η·Φ_{r″}(B_β(y))` over the support
/// (the `phi_tables` carry the η premultiplied).
#[allow(clippy::arithmetic_side_effects)]
fn tap_fill_basis(
    layout: &ShaF2Layout,
    b: &mut [F128],
    tap: &TapOp,
    sup: &crate::taps::TapSupportTables,
    phi_tables: &[Gf],
) {
    let p0 = xor_support_prefix(layout);
    let bv = layout.bit_vars;
    let w = 1usize << bv;
    let hs = sup.hs;
    for lo in sup.lo_band.0..sup.lo_band.1 {
        let vlo = sup.t_lo[lo];
        if vlo.is_zero() {
            continue;
        }
        for j in 0..w {
            let vj = vlo * sup.t_mid[j];
            if vj.is_zero() {
                continue;
            }
            let base = (lo << (bv + hs)) | (j << hs);
            for h in 0..1usize << hs {
                let e = vj * sup.t_hi[h];
                if e.is_zero() {
                    continue;
                }
                let y = embed_xor_index(layout, (base | h) << p0, tap.col) >> LOG_PACKING;
                let add = crate::ligerito::phi_from_words(*e.words(), phi_tables);
                b[y] += gf_to_f128(add);
            }
        }
    }
}

/// Tap-path forest block cap (tree-sets per batched-common call), part
/// of the proof shape. Measured 2026-07-27 (the k=6 instance and the
/// k=48 schedule baseline, n=22–28): 2-set blocks are best-or-tie at
/// every shape — the merged forest's marginal round-sharing saturates
/// at two tree-sets (the classic two-column shape the lazy kernels are
/// tuned on) while wider merges pay the cache regime (a 4-set block
/// costs +27 % at n=28) — and they flatten the batched path's
/// working-set wall: peak ≈ 2 tree-sets for ANY claim count, where the
/// padded path scaled with 2^⌈log₂k⌉.
const TAP_CLAIM_BLOCK_CAP: usize = 2;

/// Claim blocks for the batched tap path (STRUCTURAL — both sides
/// derive them): pairs plus an optional trailing singleton
/// ([`TAP_CLAIM_BLOCK_CAP`]-capped binary decomposition). The batched
/// common pads each CALL's tree-sets to a power of two, so blocking
/// removes the pad entirely (k=6: 8 padded sets → 2+2+2; k=48: 64 →
/// 24×2). Blocks share the statement, the ring basis, and the ONE
/// closing Ligerito call; each block gets its own forest + presum
/// absorbs and residual exit point (sequential Fiat–Shamir composition
/// — every block's challenges are drawn after the preceding blocks'
/// messages). k ≤ 2 keeps the pre-blocking transcript byte-identical.
fn tap_claim_blocks(k: usize) -> Vec<core::ops::Range<usize>> {
    let mut out = Vec::new();
    let mut start = 0usize;
    while start < k {
        let b = (1usize << (k.wrapping_sub(start)).ilog2()).min(TAP_CLAIM_BLOCK_CAP);
        out.push(start..start.wrapping_add(b));
        start = start.wrapping_add(b);
    }
    out
}

/// Prove a structured-tap claim set against the commitment (EXPERIMENTAL;
/// see the section comment for the protocol). Claims-only style: every
/// claim runs at depth `t' = t − log_cols` in a batched x forest —
/// claims are partitioned into binary blocks ([`tap_claim_blocks`]), one
/// padless forest + presum per (block, chunk); claims in a block share
/// their residual exit point.
#[allow(clippy::arithmetic_side_effects)]
pub fn prove_mle_eval_mod_q_ligerito_tap_claims(
    transcript: &mut (impl Transcript + Send),
    hint: &FlockCommitHint,
    layout: &ShaF2Layout,
    q_bits: usize,
    claims: &[TapClaim<'_>],
    alpha: Gf,
    pc: &LigProverConfig,
) -> IntEvalRsLigModQTapProof {
    use crate::pcs::{chunk_row_weights, mod_q_chunk_width, mod_q_num_chunks, virtual_xor_params};
    use crate::taps::{assert_tap, assert_tap_layout, extract_virtual_tap_rows, tap_support_tables};
    assert_tap_layout(layout);
    assert!(!claims.is_empty(), "need at least one tap claim");
    let p_x = virtual_xor_params(layout);
    assert!(row_bit_vars(&p_x) >= 6, "x-claim pre-sumcheck needs t' ≥ 6");
    for cl in claims {
        for tap in cl.taps {
            assert_tap(layout, tap);
        }
        assert!(!cl.taps.is_empty(), "tap claim needs at least one term");
        assert_eq!(cl.row_weights_q.len(), p_x.rows(), "tap claim row-weight length");
    }
    let stmt: Vec<(&[TapOp], &[u128])> =
        claims.iter().map(|cl| (cl.taps, cl.row_weights_q)).collect();
    absorb_tap_statement(transcript, hint.root(), layout, q_bits, &stmt);

    let c_w_x = mod_q_chunk_width(&p_x);
    let lch_x = mod_q_num_chunks(&p_x, q_bits);

    // Extraction + the batched forests/presums, per binary claim block
    // (one padless forest + presum per (block, chunk); extraction is
    // per block and freed before the next block runs).
    let blocks = tap_claim_blocks(claims.len());
    let x_chunks_all: Vec<Vec<Vec<u128>>> =
        claims.iter().map(|cl| chunk_row_weights(cl.row_weights_q, c_w_x, lch_x)).collect();
    let mut x_mfs = Vec::with_capacity(blocks.len().wrapping_mul(lch_x));
    let mut x_presums = Vec::with_capacity(blocks.len().wrapping_mul(lch_x));
    // Per-claim per-chunk exit points (claims in a block share theirs).
    let mut x_points: Vec<Vec<Vec<Gf>>> = vec![Vec::with_capacity(lch_x); claims.len()];
    let mut tap_us: Vec<Vec<Vec<u128>>> = claims.iter().map(|_| Vec::with_capacity(lch_x)).collect();
    for blk in &blocks {
        let x_rows_blk: Vec<Vec<Vec<u64>>> = {
            let _g = crate::utils::prof::scope("tap:extract");
            claims[blk.clone()]
                .iter()
                .map(|cl| extract_virtual_tap_rows(layout, &hint.rows, cl.taps))
                .collect()
        };
        let rows_refs: Vec<&[Vec<u64>]> = x_rows_blk.iter().map(|r| &r[..]).collect();
        let _g = crate::utils::prof::scope("tap:common");
        for l in 0..lch_x {
            let w_refs: Vec<&[u128]> =
                x_chunks_all[blk.clone()].iter().map(|ch| &ch[l][..]).collect();
            let (mf, us_per_claim, presum, pt) =
                crate::ligerito::prove_x_claims_batched_common(
                    transcript, &p_x, &rows_refs, &w_refs, alpha,
                );
            x_mfs.push(mf);
            x_presums.push(presum);
            for (k, u) in us_per_claim.into_iter().enumerate() {
                x_points[blk.start + k].push(pt.clone());
                tap_us[blk.start + k].push(u);
            }
        }
    }

    // Rings: per (claim, chunk, bucket) the summed member walks.
    let _g_r = crate::utils::prof::scope("tap:rings");
    let mut rings = Vec::new();
    // Per (claim, chunk): the plan + each member's support tables
    // (built once here, reused by the basis-fill phase).
    let mut plans: Vec<Vec<TapRingPlan>> = Vec::with_capacity(claims.len());
    let mut sups: Vec<Vec<Vec<crate::taps::TapSupportTables>>> =
        Vec::with_capacity(claims.len());
    for (n, cl) in claims.iter().enumerate() {
        let mut per_chunk = Vec::with_capacity(lch_x);
        let mut sup_chunk = Vec::with_capacity(lch_x);
        for pt in &x_points[n] {
            let plan = tap_ring_plan(layout, cl.taps, pt);
            let member_sups: Vec<crate::taps::TapSupportTables> = plan
                .members
                .iter()
                .map(|(ti, clss, _)| tap_support_tables(layout, &cl.taps[*ti], pt, *clss))
                .collect();
            per_chunk.push(plan);
            sup_chunk.push(member_sups);
        }
        plans.push(per_chunk);
        sups.push(sup_chunk);
    }
    for (n, cl) in claims.iter().enumerate() {
        for l in 0..lch_x {
            let plan = &plans[n][l];
            let member_svs: Vec<Vec<Gf>> = cfg_into_iter!(0..plan.members.len())
                .map(|mi| {
                    let (ti, _, _) = &plan.members[mi];
                    tap_ring_walk(layout, &hint.p_msg, &cl.taps[*ti], &sups[n][l][mi])
                })
                .collect();
            for bucket in &plan.buckets {
                let mut s = vec![Gf::zero(); 128];
                for &mi in bucket {
                    for (a, b) in s.iter_mut().zip(member_svs[mi].iter()) {
                        *a += *b;
                    }
                }
                crate::ligerito::absorb_sv(transcript, &s);
                rings.push(RingSwitchProof { s_v: s });
            }
        }
    }
    drop(_g_r);

    let r2: Vec<Gf> = transcript.get_field_challenges(LOG_PACKING, &());
    let eq_r2 = crate::poly::utils::build_eq_x_r_vec(&r2, &()).expect("r2");
    let etas: Vec<Gf> = transcript.get_field_challenges(rings.len(), &());

    // Combined basis + target.
    let m_p = packed_vars(&layout.p);
    let mut b_comb = vec![F128::ZERO; 1usize << m_p];
    {
        let _g = crate::utils::prof::scope("tap:bcomb");
        let mut ring_idx = 0usize;
        for (n, cl) in claims.iter().enumerate() {
            for l in 0..lch_x {
                let plan = &plans[n][l];
                for bucket in &plan.buckets {
                    let phi_tables = crate::ligerito::phi_byte_tables(&eq_r2, etas[ring_idx]);
                    for &mi in bucket {
                        let (ti, _, _) = &plan.members[mi];
                        tap_fill_basis(
                            layout,
                            &mut b_comb,
                            &cl.taps[*ti],
                            &sups[n][l][mi],
                            &phi_tables,
                        );
                    }
                    ring_idx += 1;
                }
            }
        }
    }
    let mut target = Gf::zero();
    for (i, ring) in rings.iter().enumerate() {
        let s_u = crate::ligerito::transpose_bits_128(&ring.s_v);
        let beta = s_u.iter().zip(eq_r2.iter()).fold(Gf::zero(), |a, (su, e)| a + *su * *e);
        target += etas[i] * beta;
    }

    let _g_lig = crate::utils::prof::scope("tap:lig");
    let lig = ligerito::recursive_prover_with_basis(
        pc,
        hint.p_msg.clone(),
        b_comb,
        gf_to_f128(target),
        &hint.prover_data.codeword,
        &hint.prover_data.merkle_tree,
        &mut ZincChallenger(transcript),
    );
    IntEvalRsLigModQTapProof { x_mfs, x_presums, tap_us, rings, lig }
}

/// Verify a structured-tap claim set (EXPERIMENTAL).
#[allow(clippy::arithmetic_side_effects)]
#[allow(clippy::too_many_arguments)]
pub fn verify_mle_eval_mod_q_ligerito_tap_claims<R>(
    transcript: &mut (impl Transcript + Send),
    commitment: &Commitment,
    proof: &IntEvalRsLigModQTapProof,
    layout: &ShaF2Layout,
    alpha: Gf,
    q_bits: usize,
    claims: &[TapVerifyClaim<'_, R>],
    vc: &LigVerifierConfig,
) -> Result<(), FlockRsError>
where
    R: Copy + PartialEq + From<u128> + core::ops::Add<Output = R> + core::ops::Mul<Output = R>,
{
    use crate::pcs::{
        chunk_row_weights, mod_q_chunk_width, mod_q_num_chunks, recombine_read_off,
        virtual_xor_params,
    };
    use crate::taps::{residual_b_evals_tap, tap_closure_desc};
    let p = &layout.p;
    if p.word_bits != 1
        || layout.x_fold_extra >= p.s
        || (layout.x_fold_extra > 0 && layout.bit_vars.wrapping_add(layout.tw) < 6)
        || layout.tw + layout.log_cols < 7
    {
        return Err(FlockRsError::RingSwitch(RsOpenError::Shape));
    }
    if claims.is_empty() {
        return Err(FlockRsError::RingSwitch(RsOpenError::Shape));
    }
    let p_x = virtual_xor_params(layout);
    let c_w_x = mod_q_chunk_width(&p_x);
    let lch_x = mod_q_num_chunks(&p_x, q_bits);
    for cl in claims {
        if cl.taps.iter().any(|t| !tap_shape_ok(layout, t))
            || cl.taps.is_empty()
            || cl.row_weights_q.len() != p_x.rows()
            || cl.col_weights.len() != p_x.cols()
        {
            return Err(FlockRsError::RingSwitch(RsOpenError::Shape));
        }
    }
    let stmt: Vec<(&[TapOp], &[u128])> =
        claims.iter().map(|cl| (cl.taps, cl.row_weights_q)).collect();
    absorb_tap_statement(transcript, &commitment.root, layout, q_bits, &stmt);

    let blocks = tap_claim_blocks(claims.len());
    if proof.x_mfs.len() != blocks.len().wrapping_mul(lch_x)
        || proof.x_presums.len() != blocks.len().wrapping_mul(lch_x)
        || proof.tap_us.len() != claims.len()
        || proof.tap_us.iter().any(|us| us.len() != lch_x)
    {
        return Err(FlockRsError::RingSwitch(RsOpenError::Shape));
    }

    // Batched forests/presums per binary claim block; claims in a block
    // share each chunk's exit point.
    let x_chunks_all: Vec<Vec<Vec<u128>>> =
        claims.iter().map(|cl| chunk_row_weights(cl.row_weights_q, c_w_x, lch_x)).collect();
    let range_shift_x = c_w_x.wrapping_add(p_x.t).wrapping_add(p_x.word_bits);
    let bound_x = 1u128 << range_shift_x;
    for l in 0..lch_x {
        for us in proof.tap_us.iter() {
            for (k, &u) in us[l].iter().enumerate() {
                if u >= bound_x {
                    let _ = k;
                    return Err(FlockRsError::ChunkRange { chunk: l, col: k });
                }
            }
        }
    }
    // Per-claim per-chunk exit points and residuals.
    let mut x_points: Vec<Vec<Vec<Gf>>> = vec![Vec::with_capacity(lch_x); claims.len()];
    let mut x_mus: Vec<Vec<Gf>> = vec![vec![Gf::zero(); claims.len()]; lch_x];
    for (bi, blk) in blocks.iter().enumerate() {
        for l in 0..lch_x {
            let us_refs: Vec<&[u128]> =
                proof.tap_us[blk.clone()].iter().map(|us| &us[l][..]).collect();
            let w_refs: Vec<&[u128]> =
                x_chunks_all[blk.clone()].iter().map(|ch| &ch[l][..]).collect();
            let (pt, mus_l) = crate::ligerito::verify_x_claims_batched_common(
                transcript,
                &proof.x_mfs[bi.wrapping_mul(lch_x).wrapping_add(l)],
                &us_refs,
                &proof.x_presums[bi.wrapping_mul(lch_x).wrapping_add(l)],
                &p_x,
                &w_refs,
                alpha,
            )
            .map_err(FlockRsError::Common)?;
            for (k, m) in mus_l.into_iter().enumerate() {
                x_points[blk.start.wrapping_add(k)].push(pt.clone());
                x_mus[l][blk.start.wrapping_add(k)] = m;
            }
        }
    }

    // Rings: per (claim, chunk) the bucketed A-table read-offs plus the
    // closed-form constant term must reproduce the claim's residual.
    let mut plans: Vec<Vec<TapRingPlan>> = Vec::with_capacity(claims.len());
    for (n, cl) in claims.iter().enumerate() {
        let mut per_chunk = Vec::with_capacity(lch_x);
        for pt in &x_points[n] {
            per_chunk.push(tap_ring_plan(layout, cl.taps, pt));
        }
        plans.push(per_chunk);
    }
    let num_rings: usize =
        plans.iter().map(|pc| pc.iter().map(|pl| pl.buckets.len()).sum::<usize>()).sum();
    if proof.rings.len() != num_rings {
        return Err(FlockRsError::RingSwitch(RsOpenError::Shape));
    }
    let mut ring_idx = 0usize;
    for n in 0..claims.len() {
        for l in 0..lch_x {
            let plan = &plans[n][l];
            let mut sum = Gf::zero();
            for bucket in &plan.buckets {
                let ring = &proof.rings[ring_idx];
                if ring.s_v.len() != 128 {
                    return Err(FlockRsError::RingSwitch(RsOpenError::Shape));
                }
                let a_tbl = &plan.members[bucket[0]].2;
                sum += ring
                    .s_v
                    .iter()
                    .zip(a_tbl.iter())
                    .fold(Gf::zero(), |a, (s, e)| a + *s * *e);
                crate::ligerito::absorb_sv(transcript, &ring.s_v);
                ring_idx += 1;
            }
            if sum != x_mus[l][n] {
                return Err(FlockRsError::RingSwitch(RsOpenError::RingSwitchClaim));
            }
        }
    }

    let r2: Vec<Gf> = transcript.get_field_challenges(LOG_PACKING, &());
    let eq_r2 = crate::poly::utils::build_eq_x_r_vec(&r2, &()).expect("r2");
    let etas: Vec<Gf> = transcript.get_field_challenges(proof.rings.len(), &());

    let mut target = Gf::zero();
    for (i, ring) in proof.rings.iter().enumerate() {
        let s_u = crate::ligerito::transpose_bits_128(&ring.s_v);
        let beta = s_u.iter().zip(eq_r2.iter()).fold(Gf::zero(), |a, (su, e)| a + *su * *e);
        target += etas[i] * beta;
    }

    // Succinct residual closure: per ring, the η-scaled MPS evaluations of
    // its members' translated bases.
    let m_p = packed_vars(p);
    let mut ring_descs: Vec<Vec<Vec<crate::taps::TapCoord>>> = Vec::with_capacity(num_rings);
    for (n, cl) in claims.iter().enumerate() {
        for (l, pt) in x_points[n].iter().enumerate() {
            let plan = &plans[n][l];
            for bucket in &plan.buckets {
                let descs: Vec<Vec<crate::taps::TapCoord>> = bucket
                    .iter()
                    .map(|&mi| {
                        let (ti, clss, _) = &plan.members[mi];
                        tap_closure_desc(layout, &cl.taps[*ti], pt, *clss)
                    })
                    .collect();
                ring_descs.push(descs);
            }
        }
    }
    let eval_b = |ris: &[F128], yr_log_n: usize| -> Vec<F128> {
        let ris_gf: Vec<Gf> = ris.iter().map(|&f| f128_to_gf(f)).collect();
        let mut out = vec![Gf::zero(); 1usize << yr_log_n];
        for (i, descs) in ring_descs.iter().enumerate() {
            for desc in descs {
                let blk = residual_b_evals_tap(&ris_gf, yr_log_n, desc, &eq_r2);
                for (o, x) in out.iter_mut().zip(blk.iter()) {
                    *o += etas[i] * *x;
                }
            }
        }
        out.into_iter().map(gf_to_f128).collect()
    };
    let ok = ligerito::recursive_verifier_with_basis_succinct(
        vc,
        &proof.lig,
        m_p,
        gf_to_f128(target),
        &commitment.root,
        eval_b,
        &mut ZincChallenger(transcript),
    );
    if !ok {
        return Err(FlockRsError::LigeritoReject);
    }

    // Read-offs in R, per claim.
    for (cl, us) in claims.iter().zip(proof.tap_us.iter()) {
        let flat: Vec<u128> = us.iter().flat_map(|u| u.iter().copied()).collect();
        let y = recombine_read_off(&p_x, &flat, 0, cl.col_weights, c_w_x, lch_x);
        if y != cl.claimed {
            return Err(FlockRsError::Common(IntEvalRsError::ReadOff));
        }
    }
    Ok(())
}

/// Itemized `(zinc-side, flock LigeritoProof)` bytes of a tap-claim proof.
#[allow(clippy::arithmetic_side_effects)]
pub fn mle_eval_mod_q_lig_tap_size_breakdown(
    proof: &IntEvalRsLigModQTapProof,
) -> (ZincSideSizeBreakdown, usize) {
    let mut b = ZincSideSizeBreakdown::default();
    for (mf, presum) in proof.x_mfs.iter().zip(proof.x_presums.iter()) {
        b.accumulate(&zinc_side_size_breakdown_merged(mf, &[], presum));
    }
    for us in &proof.tap_us {
        for u in us {
            b.v += u.len() * 16;
        }
    }
    b.s_v = proof.rings.len() * 128 * 16;
    (b, proof.lig.size_bytes())
}

// ---------------------------------------------------------------------
// EXPERIMENTAL — RLC families over tap STREAMS
// (docs/rlc-structured-taps-phase0.md §2–3).
//
// The stream reduction: claims on tapped convolutions `b_i = ⊕ op(col)`
// are per-position XORs of the deduped tap STREAMS, so the RLC family
// construction applies verbatim with the streams as the family's base
// columns (`j_eff` = #streams). Clusters keep the active-channel count in
// budget (the monolithic instance has ~900 channels; the pinned clusters
// 27 + 31); each cluster runs ONE eager `2^{j_eff}`-case forest per weight
// chunk + its active-channel presum (monomial rows FUSED into the scan —
// never materialised) and its own 2-level leaf-bit discharge cascade (the
// eq-factored driver shares suffix tensors across bit-selected groups, so
// each cascade level needs ONE exit point — per-cluster levels; the
// single-chunk regime `lch = 1` is asserted, as in every deployed shape).
// Every residual stream opening — singleton channel exits, level-1 Col
// sides at ρ, level-2 exits at ρ′ — closes through the translated-eq
// machinery of [`crate::taps`] (per (stream, class) rings, claim checked
// as `μ = Σ_β ⟨A_β, s_β⟩`). ONE recursive Ligerito call closes everything.
//
// Fiat–Shamir chain: absorb statement (tag 0x43) → per cluster γ's →
// per cluster per chunk forest + presum → per cluster cascade
// (η's → level 1 → ω's → η″'s → level 2 → ω″'s) → rings → r″ + ring η's
// → Ligerito. Evaluation field fixed q = 2^100 − 15.
// ---------------------------------------------------------------------

/// One cluster of a stream family: the deduped tap streams plus the
/// claims whose forms are bitmasks over them. All claims (across ALL
/// clusters of one call) share the column point.
pub struct TapFamilyCluster<'a> {
    /// The family base streams, `1 ≤ j_eff ≤ 7`.
    pub streams: &'a [TapOp],
    /// Claims: `form` over the streams, per-claim row weights, claimed
    /// values (canonical `[0, q)`).
    pub claims: &'a [RlcFamilyClaim<'a>],
}

/// Per-cluster proof parts of a stream family.
pub struct TapFamilyClusterSide {
    /// Per weight chunk: the eager `2^{j_eff}`-case forest.
    pub mfs: Vec<MergedForestProof>,
    /// `us[l][c]` = the combined case-weight folds, range-checked.
    pub us: Vec<Vec<u128>>,
    /// Per chunk: the active-channel presum.
    pub presums: Vec<MultiDegreeSumcheckProof<Gf>>,
    /// The cluster's cascade level 1 (its |S| ≥ 2 channels; None when all
    /// monomial channels elided).
    pub discharge_eqf: Option<RlcDischargeEqf>,
    /// Level-1 side openings at the cluster's ρ, canonical side order.
    pub omegas: Vec<Gf>,
    /// The cluster's cascade level 2 (its AND sides).
    pub discharge_eqf2: Option<RlcDischargeEqf>,
    /// Level-2 stream openings at the cluster's ρ′, ascending stream order.
    pub omegas2: Vec<Gf>,
}

/// End-to-end proof of a clustered stream family (EXPERIMENTAL).
pub struct IntEvalRsLigTapFamilyProof {
    pub clusters: Vec<TapFamilyClusterSide>,
    /// Twisted ring messages, flat in (cluster, group, stream, class) order.
    pub rings: Vec<RingSwitchProof>,
    pub lig: LigeritoProof,
}

/// Absorb the stream-family statement (domain tag 0x43): root, layout,
/// q, and per cluster the stream descriptors, forms, claimed values and
/// row-weight vectors — everything the γ's depend on precedes them.
#[allow(clippy::arithmetic_side_effects)]
fn absorb_tap_family_statement(
    transcript: &mut impl Transcript,
    root: &flock_core::merkle::Hash,
    layout: &ShaF2Layout,
    clusters: &[TapFamilyCluster<'_>],
) {
    let mut bytes = Vec::new();
    bytes.push(0x43u8);
    bytes.extend_from_slice(root);
    for v in [
        layout.p.t,
        layout.p.s,
        layout.p.word_bits,
        layout.num_cols,
        layout.log_cols,
        layout.bit_vars,
        layout.num_vars,
        layout.tw,
        layout.x_fold_extra,
        clusters.len(),
    ] {
        bytes.extend_from_slice(&(v as u64).to_le_bytes());
    }
    bytes.extend_from_slice(&crate::pcs::FQ_MOD.to_le_bytes());
    for cl in clusters {
        bytes.extend_from_slice(&(cl.streams.len() as u64).to_le_bytes());
        for st in cl.streams {
            for v in [
                st.col as u64,
                st.grp_log2 as u64,
                st.bit_amt as u64,
                u64::from(st.bit_dropout),
                st.off as u64,
            ] {
                bytes.extend_from_slice(&v.to_le_bytes());
            }
        }
        bytes.extend_from_slice(&(cl.claims.len() as u64).to_le_bytes());
        for c in cl.claims {
            bytes.extend_from_slice(&(c.form as u64).to_le_bytes());
            bytes.extend_from_slice(&c.claimed.to_le_bytes());
            for &x in c.row_weights_q {
                bytes.extend_from_slice(&x.to_le_bytes());
            }
        }
    }
    transcript.absorb_slice(&bytes);
}

/// Validate a stream-family statement (shared prover asserts / verifier
/// shape checks).
fn tap_family_check(layout: &ShaF2Layout, clusters: &[TapFamilyCluster<'_>]) -> bool {
    use crate::pcs::{FQ_MOD, virtual_xor_params};
    let p_x = virtual_xor_params(layout);
    if clusters.is_empty()
        || layout.p.word_bits != 1
        || layout.x_fold_extra != 0
        || layout.tw + layout.log_cols < 7
        || row_bit_vars(&p_x) < 6
    {
        return false;
    }
    for cl in clusters {
        let j = cl.streams.len();
        if !(1..=7).contains(&j) || cl.claims.is_empty() {
            return false;
        }
        for st in cl.streams {
            if !tap_shape_ok(layout, st) {
                return false;
            }
        }
        for c in cl.claims {
            if c.form == 0
                || c.form >= (1usize << j)
                || c.row_weights_q.len() != p_x.rows()
                || c.claimed >= FQ_MOD
            {
                return false;
            }
        }
        for fi in 0..j {
            if !cl.claims.iter().any(|c| (c.form >> fi) & 1 == 1) {
                return false;
            }
        }
    }
    true
}

/// One cluster's cascade shape, derived from its active channels on both
/// sides: the level-1 (chunk, channel) pairs, the canonical side list,
/// the level-2 AND masks and their member streams.
struct TapCascadeShape {
    l1_pairs: Vec<(usize, usize)>,
    side_list: Vec<RlcSide>,
    and_masks: Vec<usize>,
    omega2_fis: Vec<usize>,
}

fn tap_cascade_shape(j: usize, actives: &[Vec<usize>]) -> TapCascadeShape {
    let l1_pairs: Vec<(usize, usize)> = {
        let mut v = Vec::new();
        for (l, act) in actives.iter().enumerate() {
            for &ch in act {
                if ch.count_ones() >= 2 {
                    v.push((l, ch));
                }
            }
        }
        v
    };
    let side_list: Vec<RlcSide> = {
        let mut v: Vec<RlcSide> = Vec::new();
        for &(_, ch) in &l1_pairs {
            let (a, b) = rlc_channel_sides(ch);
            for sd in [a, b] {
                if !v.contains(&sd) {
                    v.push(sd);
                }
            }
        }
        v.sort_unstable();
        v
    };
    let and_masks: Vec<usize> = side_list
        .iter()
        .filter_map(|&sd| match sd {
            RlcSide::And(mask) => Some(mask),
            RlcSide::Col(_) => None,
        })
        .collect();
    let omega2_fis: Vec<usize> = {
        let mut fis: Vec<usize> = and_masks
            .iter()
            .flat_map(|&mask| (0..j).filter(move |fi| (mask >> fi) & 1 == 1))
            .collect();
        fis.sort_unstable();
        fis.dedup();
        fis
    };
    TapCascadeShape { l1_pairs, side_list, and_masks, omega2_fis }
}

/// Prove a clustered stream family (EXPERIMENTAL; see the section
/// comment). All clusters' claims share the column point.
#[allow(clippy::arithmetic_side_effects)]
pub fn prove_mle_eval_mod_q_ligerito_tap_family(
    transcript: &mut (impl Transcript + Send),
    hint: &FlockCommitHint,
    layout: &ShaF2Layout,
    clusters: &[TapFamilyCluster<'_>],
    alpha: Gf,
    pc: &LigProverConfig,
) -> IntEvalRsLigTapFamilyProof {
    use crate::merged_forest::prove_merged_forest;
    use crate::pcs::{
        FQ_BITS, fq_challenge, mod_q_chunk_width, mod_q_num_chunks, rlc_case_pow_table,
        rlc_case_weights, rlc_chunk_case_weights, rlc_tau_tables, virtual_xor_params,
    };
    use crate::piop::sumcheck::multi_degree::{MultiDegreeSumcheck, MultiDegreeSumcheckGroup};
    use crate::poly::mle::DenseMultilinearExtension;
    use crate::poly::utils::build_eq_x_r_vec;
    use crate::taps::{extract_virtual_tap_rows, tap_classes, tap_support_tables};
    use crypto_primitives::Field;

    assert!(tap_family_check(layout, clusters), "invalid stream-family statement");
    let p_x = virtual_xor_params(layout);
    let t_x = row_bit_vars(&p_x);
    let c_w_x = mod_q_chunk_width(&p_x);
    let lch_x = mod_q_num_chunks(&p_x, FQ_BITS);
    assert_eq!(
        lch_x, 1,
        "the leaf-bit cascade shares one exit point per level; multi-chunk shapes are unbuilt"
    );

    // (1)–(2) Statement → per-cluster γ's → case chunks.
    absorb_tap_family_statement(transcript, hint.root(), layout, clusters);
    let case_chunks_all: Vec<Vec<Vec<Vec<u128>>>> = clusters
        .iter()
        .map(|cl| {
            let gammas: Vec<u128> =
                (0..cl.claims.len()).map(|_| fq_challenge(transcript)).collect();
            let forms: Vec<usize> = cl.claims.iter().map(|c| c.form).collect();
            let w_refs: Vec<&[u128]> = cl.claims.iter().map(|c| c.row_weights_q).collect();
            let _g = crate::utils::prof::scope("tapf:casew");
            let case_w = rlc_case_weights(&w_refs, &gammas, &forms, cl.streams.len());
            rlc_chunk_case_weights(&case_w, c_w_x, lch_x)
        })
        .collect();

    // Stream rows per cluster.
    let x_rows_all: Vec<Vec<Vec<Vec<u64>>>> = {
        let _g = crate::utils::prof::scope("tapf:extract");
        clusters
            .iter()
            .map(|cl| {
                cl.streams
                    .iter()
                    .map(|st| extract_virtual_tap_rows(layout, &hint.rows, &[*st]))
                    .collect()
            })
            .collect()
    };

    // (3)–(4) Per cluster: forests + presums, then the cluster's cascade.
    let one = Gf::one();
    let zero_inner = Gf::zero().into_inner();
    let mut sides_out: Vec<TapFamilyClusterSide> = Vec::with_capacity(clusters.len());
    // Ring surfaces collected per cluster: (point, streams) with cluster id.
    struct RingSurface {
        ci: usize,
        point: Vec<Gf>,
        streams: Vec<usize>,
    }
    let mut surfaces: Vec<RingSurface> = Vec::new();
    for (ci, cl) in clusters.iter().enumerate() {
        let x_rows = &x_rows_all[ci];
        let j = cl.streams.len();
        let mut mfs = Vec::with_capacity(lch_x);
        let mut us = Vec::with_capacity(lch_x);
        let mut presums = Vec::with_capacity(lch_x);
        let mut points = Vec::with_capacity(lch_x);
        let mut actives = Vec::with_capacity(lch_x);
        for chunk in &case_chunks_all[ci] {
            let _g = crate::utils::prof::scope("tapf:chunk");
            let case_pow = {
                let _g = crate::utils::prof::scope("tapf:pows");
                rlc_case_pow_table(chunk, alpha)
            };
            let (leaves, u) = {
                let _g = crate::utils::prof::scope("tapf:leaves");
                rlc_leaves_and_folds(&p_x, x_rows, chunk, &case_pow)
            };
            let (_roots, mf, z, e_d) = {
                let _g = crate::utils::prof::scope("tapf:forest");
                prove_merged_forest(transcript, &leaves, t_x, p_x.s)
            };
            let _g_ps = crate::utils::prof::scope("tapf:presum");
            let (z_bj, z_c) = z.split_at(t_x);
            let eq_zbj = build_eq_x_r_vec(z_bj, &()).expect("t' >= 1");
            let eq_zc = build_eq_x_r_vec(z_c, &()).expect("s >= 1");
            let taus = rlc_tau_tables(&case_pow);
            let active = rlc_active_channels(&taus);
            assert!(!active.is_empty(), "degenerate cluster: every presum channel vanished");
            for &s in &active {
                assert!(
                    s.count_ones() <= 4,
                    "active channel {s:#b} exceeds the |S| ≤ 4 cascade — re-cluster"
                );
            }
            let m_tbls: Vec<Vec<Gf>> = active
                .iter()
                .map(|&s| {
                    let members: Vec<&[Vec<u64>]> = (0..j)
                        .filter(|fi| (s >> fi) & 1 == 1)
                        .map(|fi| &x_rows[fi][..])
                        .collect();
                    crate::ligerito::xi_combined_rows_and(&p_x, &members, &eq_zc)
                })
                .collect();
            let to_mle = |tbl: Vec<Gf>| {
                DenseMultilinearExtension::from_evaluations_vec(
                    t_x,
                    tbl.into_iter().map(|g| g.into_inner()).collect(),
                    zero_inner,
                )
            };
            let groups: Vec<MultiDegreeSumcheckGroup<Gf>> = active
                .iter()
                .enumerate()
                .map(|(gi, &s)| {
                    let r_tbl: Vec<Gf> =
                        eq_zbj.iter().zip(taus[s].iter()).map(|(&e, &t)| e * t).collect();
                    MultiDegreeSumcheckGroup::new(
                        2,
                        vec![to_mle(r_tbl), to_mle(m_tbls[gi].clone())],
                        Box::new(|vals: &[Gf]| vals[0] * vals[1]),
                    )
                })
                .collect();
            let (presum, states) =
                MultiDegreeSumcheck::<Gf>::prove_as_subprotocol(transcript, groups, t_x, &());
            debug_assert_eq!(
                presum.claimed_sums().iter().fold(Gf::zero(), |a, &b| a + b),
                e_d + one,
                "presum channels must sum to the forest exit claim"
            );
            let r_star = states[0].randomness.clone();
            let point: Vec<Gf> = r_star.iter().chain(z_c.iter()).copied().collect();
            mfs.push(mf);
            us.push(u);
            presums.push(presum);
            points.push(point);
            actives.push(active);
        }
        // Chunk-exit ring surfaces (active singleton streams).
        for (l, pt) in points.iter().enumerate() {
            surfaces.push(RingSurface {
                ci,
                point: pt.clone(),
                streams: (0..j)
                    .filter(|&fi| actives[l].contains(&(1usize << fi)))
                    .collect(),
            });
        }
        // The cluster's cascade.
        let shape = tap_cascade_shape(j, &actives);
        let (d1, omegas, d2, omegas2) = if shape.l1_pairs.is_empty() {
            (None, Vec::new(), None, Vec::new())
        } else {
            let _g = crate::utils::prof::scope("tapf:discharge");
            let etas_dis: Vec<Gf> =
                transcript.get_field_challenges(shape.l1_pairs.len(), &());
            let _g_t = crate::utils::prof::scope("tapf:dis_tbls");
            let side_not_rows = |sd: RlcSide| -> Vec<Vec<u64>> {
                match sd {
                    RlcSide::Col(fi) => rlc_not_rows(&x_rows[fi]),
                    RlcSide::And(mask) => {
                        let mut bits = (0..j).filter(|fi| (mask >> fi) & 1 == 1);
                        let (a, b) =
                            (bits.next().expect("2 bits"), bits.next().expect("2 bits"));
                        x_rows[a]
                            .iter()
                            .zip(x_rows[b].iter())
                            .map(|(ra, rb)| {
                                ra.iter().zip(rb.iter()).map(|(&x, &y)| !(x & y)).collect()
                            })
                            .collect()
                    }
                }
            };
            let not_cache: Vec<Vec<Vec<u64>>> =
                shape.side_list.iter().map(|&sd| side_not_rows(sd)).collect();
            let not_of = |sd: RlcSide| -> &Vec<Vec<u64>> {
                &not_cache[shape.side_list.iter().position(|&x| x == sd).expect("side")]
            };
            let specs: Vec<RlcEqfSpec<'_>> = shape
                .l1_pairs
                .iter()
                .enumerate()
                .map(|(i, &(l, ch))| {
                    let (a, b) = rlc_channel_sides(ch);
                    (&points[l][..], etas_dis[i], &not_of(a)[..], &not_of(b)[..])
                })
                .collect();
            drop(_g_t);
            let _g_r = crate::utils::prof::scope("tapf:dis_run");
            let (d1, rho, vals) = rlc_prove_eqf_level(transcript, &p_x, t_x, &specs);
            let omegas: Vec<Gf> = shape
                .side_list
                .iter()
                .map(|&sd| {
                    for (i, &(_, ch)) in shape.l1_pairs.iter().enumerate() {
                        let (a, b) = rlc_channel_sides(ch);
                        if a == sd {
                            return vals[i].0;
                        }
                        if b == sd {
                            return vals[i].1;
                        }
                    }
                    unreachable!("side_list derives from l1_pairs")
                })
                .collect();
            crate::ligerito::absorb_rlc_omegas(transcript, &omegas);
            surfaces.push(RingSurface {
                ci,
                point: rho.clone(),
                streams: shape
                    .side_list
                    .iter()
                    .filter_map(|&sd| match sd {
                        RlcSide::Col(fi) => Some(fi),
                        RlcSide::And(_) => None,
                    })
                    .collect(),
            });
            if shape.and_masks.is_empty() {
                (Some(d1), omegas, None, Vec::new())
            } else {
                let etas2: Vec<Gf> =
                    transcript.get_field_challenges(shape.and_masks.len(), &());
                let not_singles: Vec<(usize, Vec<Vec<u64>>)> = shape
                    .omega2_fis
                    .iter()
                    .map(|&fi| (fi, rlc_not_rows(&x_rows[fi])))
                    .collect();
                let not_single = |fi: usize| -> &[Vec<u64>] {
                    &not_singles.iter().find(|(f, _)| *f == fi).expect("member cached").1
                };
                let specs2: Vec<RlcEqfSpec<'_>> = shape
                    .and_masks
                    .iter()
                    .enumerate()
                    .map(|(i, &mask)| {
                        let mut bits = (0..j).filter(|fi| (mask >> fi) & 1 == 1);
                        let (a, b) =
                            (bits.next().expect("2 bits"), bits.next().expect("2 bits"));
                        (&rho[..], etas2[i], not_single(a), not_single(b))
                    })
                    .collect();
                let (d2, rho2, vals2) = rlc_prove_eqf_level(transcript, &p_x, t_x, &specs2);
                let omegas2: Vec<Gf> = shape
                    .omega2_fis
                    .iter()
                    .map(|&fi| {
                        for (i, &mask) in shape.and_masks.iter().enumerate() {
                            let mut bits = (0..j).filter(|f| (mask >> f) & 1 == 1);
                            let (a, b) =
                                (bits.next().expect("2 bits"), bits.next().expect("2 bits"));
                            if a == fi {
                                return vals2[i].0;
                            }
                            if b == fi {
                                return vals2[i].1;
                            }
                        }
                        unreachable!("omega2 fis derive from and_masks")
                    })
                    .collect();
                crate::ligerito::absorb_rlc_omegas(transcript, &omegas2);
                surfaces.push(RingSurface {
                    ci,
                    point: rho2.clone(),
                    streams: shape.omega2_fis.clone(),
                });
                (Some(d1), omegas, Some(d2), omegas2)
            }
        };
        sides_out.push(TapFamilyClusterSide {
            mfs,
            us,
            presums,
            discharge_eqf: d1,
            omegas,
            discharge_eqf2: d2,
            omegas2,
        });
    }

    // (5) Twisted rings over all surfaces, per (stream, class).
    let _g_r = crate::utils::prof::scope("tapf:rings");
    let mut ring_walks: Vec<(TapOp, usize, crate::taps::TapClass)> = Vec::new();
    for (si, surf) in surfaces.iter().enumerate() {
        for &fi in &surf.streams {
            let tap = clusters[surf.ci].streams[fi];
            for cls in tap_classes(layout, &tap) {
                ring_walks.push((tap, si, cls));
            }
        }
    }
    let svs: Vec<Vec<Gf>> = cfg_into_iter!(0..ring_walks.len())
        .map(|i| {
            let (tap, si, cls) = &ring_walks[i];
            let sup = tap_support_tables(layout, tap, &surfaces[*si].point, *cls);
            tap_ring_walk(layout, &hint.p_msg, tap, &sup)
        })
        .collect();
    let mut rings = Vec::with_capacity(svs.len());
    for sv in svs {
        crate::ligerito::absorb_sv(transcript, &sv);
        rings.push(RingSwitchProof { s_v: sv });
    }
    drop(_g_r);

    // (6) r″ + ring η's → combined basis + target → ONE Ligerito call.
    let r2: Vec<Gf> = transcript.get_field_challenges(LOG_PACKING, &());
    let eq_r2 = crate::poly::utils::build_eq_x_r_vec(&r2, &()).expect("r2");
    let etas: Vec<Gf> = transcript.get_field_challenges(rings.len(), &());
    let m_p = packed_vars(&layout.p);
    let mut b_comb = vec![F128::ZERO; 1usize << m_p];
    {
        let _g = crate::utils::prof::scope("tapf:bcomb");
        for (i, (tap, si, cls)) in ring_walks.iter().enumerate() {
            let phi_tables = crate::ligerito::phi_byte_tables(&eq_r2, etas[i]);
            let sup = tap_support_tables(layout, tap, &surfaces[*si].point, *cls);
            tap_fill_basis(layout, &mut b_comb, tap, &sup, &phi_tables);
        }
    }
    let mut target = Gf::zero();
    for (i, ring) in rings.iter().enumerate() {
        let s_u = crate::ligerito::transpose_bits_128(&ring.s_v);
        let beta = s_u.iter().zip(eq_r2.iter()).fold(Gf::zero(), |a, (su, e)| a + *su * *e);
        target += etas[i] * beta;
    }
    let _g_l = crate::utils::prof::scope("tapf:lig");
    let lig = ligerito::recursive_prover_with_basis(
        pc,
        hint.p_msg.clone(),
        b_comb,
        gf_to_f128(target),
        &hint.prover_data.codeword,
        &hint.prover_data.merkle_tree,
        &mut ZincChallenger(transcript),
    );
    IntEvalRsLigTapFamilyProof { clusters: sides_out, rings, lig }
}

/// Verify a clustered stream family (EXPERIMENTAL). `col_weights` is the
/// SHARED clear-axis weight vector (one column point across all clusters);
/// per cluster the recombination is checked against `T = Σ_i γ_i·c_i`.
#[allow(clippy::arithmetic_side_effects)]
#[allow(clippy::too_many_arguments)]
pub fn verify_mle_eval_mod_q_ligerito_tap_family(
    transcript: &mut (impl Transcript + Send),
    commitment: &Commitment,
    proof: &IntEvalRsLigTapFamilyProof,
    layout: &ShaF2Layout,
    clusters: &[TapFamilyCluster<'_>],
    col_weights: &[crate::pcs::Fq],
    alpha: Gf,
    vc: &LigVerifierConfig,
) -> Result<(), FlockRsError> {
    use crate::merged_forest::verify_merged_forest;
    use crate::pcs::{
        FQ_BITS, FixedBasePow, Fq, fq_add, fq_challenge, fq_mul, is_generator,
        mod_q_chunk_width, mod_q_num_chunks, recombine_read_off, rlc_case_pow_table,
        rlc_case_weights, rlc_chunk_case_weights, rlc_tau_tables, virtual_xor_params,
    };
    use crate::piop::sumcheck::multi_degree::MultiDegreeSumcheck;
    use crate::poly::utils::build_eq_x_r_vec;
    use crate::taps::{residual_b_evals_tap, tap_classes, tap_closure_desc, tap_inpack_table};

    if !tap_family_check(layout, clusters) {
        return Err(FlockRsError::RingSwitch(RsOpenError::Shape));
    }
    if !is_generator(alpha) {
        return Err(FlockRsError::Common(IntEvalRsError::ChallengeNotGenerator));
    }
    let p_x = virtual_xor_params(layout);
    let t_x = row_bit_vars(&p_x);
    let c_w_x = mod_q_chunk_width(&p_x);
    let lch_x = mod_q_num_chunks(&p_x, FQ_BITS);
    if lch_x != 1
        || proof.clusters.len() != clusters.len()
        || col_weights.len() != p_x.cols()
        || proof.clusters.iter().any(|s| {
            s.mfs.len() != lch_x || s.us.len() != lch_x || s.presums.len() != lch_x
        })
    {
        return Err(FlockRsError::RingSwitch(RsOpenError::Shape));
    }

    // (1)–(2) Statement → γ's → case chunks + targets.
    absorb_tap_family_statement(transcript, &commitment.root, layout, clusters);
    let mut case_chunks_all = Vec::with_capacity(clusters.len());
    let mut targets: Vec<Fq> = Vec::with_capacity(clusters.len());
    for cl in clusters {
        let gammas: Vec<u128> =
            (0..cl.claims.len()).map(|_| fq_challenge(transcript)).collect();
        let t = cl
            .claims
            .iter()
            .zip(gammas.iter())
            .fold(0u128, |acc, (c, &g)| fq_add(acc, fq_mul(g, c.claimed)));
        targets.push(Fq::from(t));
        let forms: Vec<usize> = cl.claims.iter().map(|c| c.form).collect();
        let w_refs: Vec<&[u128]> = cl.claims.iter().map(|c| c.row_weights_q).collect();
        let case_w = rlc_case_weights(&w_refs, &gammas, &forms, cl.streams.len());
        case_chunks_all.push(rlc_chunk_case_weights(&case_w, c_w_x, lch_x));
    }

    // (3)–(5) Per cluster: forest + presum residuals, cascade, and the
    // ring surfaces (checked after all clusters, in surface order).
    let one = Gf::one();
    let comb = FixedBasePow::new(alpha, 128, 8);
    let range_shift = c_w_x.wrapping_add(p_x.t).wrapping_add(p_x.word_bits);
    let bound = 1u128 << range_shift;
    struct VSurface {
        ci: usize,
        point: Vec<Gf>,
        // (stream, expected residual) pairs.
        streams: Vec<(usize, Gf)>,
    }
    let mut surfaces: Vec<VSurface> = Vec::new();
    for (ci, cl) in clusters.iter().enumerate() {
        let side = &proof.clusters[ci];
        let j = cl.streams.len();
        let mut points = Vec::with_capacity(lch_x);
        let mut actives = Vec::with_capacity(lch_x);
        let mut mus_s1 = Vec::with_capacity(lch_x);
        let mut mus_ge2 = Vec::with_capacity(lch_x);
        for (l, chunk_w) in case_chunks_all[ci].iter().enumerate() {
            for (c, &u) in side.us[l].iter().enumerate() {
                if u >= bound {
                    return Err(FlockRsError::ChunkRange { chunk: l, col: c });
                }
            }
            let roots: Vec<Gf> = side.us[l].iter().map(|&u| comb.pow(u)).collect();
            let (z, e_d) = verify_merged_forest(transcript, &roots, &side.mfs[l], t_x, p_x.s)
                .map_err(|_| FlockRsError::Common(IntEvalRsError::Forest))?;
            let subclaims = MultiDegreeSumcheck::<Gf>::verify_as_subprotocol(
                transcript,
                t_x,
                &side.presums[l],
                &(),
            )
            .map_err(|_| FlockRsError::Common(IntEvalRsError::PreSumcheck))?;
            let case_pow = rlc_case_pow_table(chunk_w, alpha);
            let taus = rlc_tau_tables(&case_pow);
            let active = rlc_active_channels(&taus);
            let sums = side.presums[l].claimed_sums();
            if active.is_empty()
                || sums.len() != active.len()
                || active.iter().any(|s| s.count_ones() > 4)
            {
                return Err(FlockRsError::RingSwitch(RsOpenError::Shape));
            }
            if sums.iter().fold(Gf::zero(), |a, &b| a + b) != e_d + one {
                return Err(FlockRsError::Common(IntEvalRsError::PreSumcheck));
            }
            let (z_bj, z_c) = z.split_at(t_x);
            let r_star = subclaims.point().to_vec();
            let eq_zbj = build_eq_x_r_vec(z_bj, &()).expect("t' >= 1");
            let eq_rstar = build_eq_x_r_vec(&r_star, &()).expect("t' >= 1");
            let eq_prod: Vec<Gf> =
                eq_zbj.iter().zip(eq_rstar.iter()).map(|(&a, &b)| a * b).collect();
            let mut mu_s1 = vec![None; j];
            let mut mu_ge2 = Vec::with_capacity(active.len());
            for (g, &s) in active.iter().enumerate() {
                let r_hat = eq_prod
                    .iter()
                    .zip(taus[s].iter())
                    .fold(Gf::zero(), |acc, (&e, &t)| acc + e * t);
                if r_hat.is_zero() {
                    return Err(FlockRsError::Common(IntEvalRsError::RHatZero));
                }
                let mu = subclaims.expected_evaluations()[g] * r_hat.inverse();
                if s.count_ones() == 1 {
                    mu_s1[s.trailing_zeros() as usize] = Some(mu);
                } else {
                    mu_ge2.push(mu);
                }
            }
            points.push(
                r_star.iter().chain(z_c.iter()).copied().collect::<Vec<Gf>>(),
            );
            actives.push(active);
            mus_s1.push(mu_s1);
            mus_ge2.push(mu_ge2);
        }
        for (l, pt) in points.iter().enumerate() {
            surfaces.push(VSurface {
                ci,
                point: pt.clone(),
                streams: (0..j)
                    .filter(|&fi| actives[l].contains(&(1usize << fi)))
                    .map(|fi| {
                        (fi, mus_s1[l][fi].expect("active singleton has a residual"))
                    })
                    .collect(),
            });
        }
        let shape = tap_cascade_shape(j, &actives);
        if side.discharge_eqf.is_some() == shape.l1_pairs.is_empty()
            || side.omegas.len() != shape.side_list.len()
            || side.discharge_eqf2.is_some() == shape.and_masks.is_empty()
            || side.omegas2.len() != shape.omega2_fis.len()
        {
            return Err(FlockRsError::RingSwitch(RsOpenError::Shape));
        }
        if !shape.l1_pairs.is_empty() {
            let etas_dis: Vec<Gf> =
                transcript.get_field_challenges(shape.l1_pairs.len(), &());
            let d = side.discharge_eqf.as_ref().expect("shape-checked above");
            let specs: Vec<(&[Gf], Gf)> = shape
                .l1_pairs
                .iter()
                .enumerate()
                .map(|(i, &(l, _))| (&points[l][..], etas_dis[i]))
                .collect();
            let claimed: Vec<Gf> = shape
                .l1_pairs
                .iter()
                .enumerate()
                .map(|(i, &(l, ch))| {
                    let gi = actives[l]
                        .iter()
                        .filter(|x| x.count_ones() >= 2)
                        .position(|&x| x == ch)
                        .expect("pair derived from actives");
                    etas_dis[i] * mus_ge2[l][gi]
                })
                .collect();
            let side_pos = |sd: RlcSide| -> usize {
                shape.side_list.iter().position(|&x| x == sd).expect("side in list")
            };
            let side_vals: Vec<(Gf, Gf)> = shape
                .l1_pairs
                .iter()
                .map(|&(_, ch)| {
                    let (a, b) = rlc_channel_sides(ch);
                    (side.omegas[side_pos(a)], side.omegas[side_pos(b)])
                })
                .collect();
            let rho =
                rlc_verify_eqf_level(transcript, t_x, p_x.s, d, &specs, &claimed, &side_vals)?;
            crate::ligerito::absorb_rlc_omegas(transcript, &side.omegas);
            surfaces.push(VSurface {
                ci,
                point: rho.clone(),
                streams: shape
                    .side_list
                    .iter()
                    .enumerate()
                    .filter_map(|(pos, &sd)| match sd {
                        RlcSide::Col(fi) => Some((fi, side.omegas[pos])),
                        RlcSide::And(_) => None,
                    })
                    .collect(),
            });
            if !shape.and_masks.is_empty() {
                let etas2: Vec<Gf> =
                    transcript.get_field_challenges(shape.and_masks.len(), &());
                let d2 = side.discharge_eqf2.as_ref().expect("shape-checked above");
                let specs2: Vec<(&[Gf], Gf)> = shape
                    .and_masks
                    .iter()
                    .enumerate()
                    .map(|(i, _)| (&rho[..], etas2[i]))
                    .collect();
                let claimed2: Vec<Gf> = shape
                    .and_masks
                    .iter()
                    .enumerate()
                    .map(|(i, &mask)| etas2[i] * side.omegas[side_pos(RlcSide::And(mask))])
                    .collect();
                let fi_pos = |fi: usize| -> usize {
                    shape.omega2_fis.iter().position(|&x| x == fi).expect("member")
                };
                let side_vals2: Vec<(Gf, Gf)> = shape
                    .and_masks
                    .iter()
                    .map(|&mask| {
                        let mut bits = (0..j).filter(|fi| (mask >> fi) & 1 == 1);
                        let (a, b) =
                            (bits.next().expect("2 bits"), bits.next().expect("2 bits"));
                        (side.omegas2[fi_pos(a)], side.omegas2[fi_pos(b)])
                    })
                    .collect();
                let rho2 = rlc_verify_eqf_level(
                    transcript, t_x, p_x.s, d2, &specs2, &claimed2, &side_vals2,
                )?;
                crate::ligerito::absorb_rlc_omegas(transcript, &side.omegas2);
                surfaces.push(VSurface {
                    ci,
                    point: rho2,
                    streams: shape
                        .omega2_fis
                        .iter()
                        .zip(side.omegas2.iter())
                        .map(|(&fi, &om)| (fi, om))
                        .collect(),
                });
            }
        }
    }

    // (5b) Twisted rings: each (stream, class) set reproduces its residual.
    let mut ring_descs: Vec<Vec<crate::taps::TapCoord>> = Vec::new();
    let mut ring_idx = 0usize;
    for surf in &surfaces {
        for &(fi, expect) in &surf.streams {
            let tap = clusters[surf.ci].streams[fi];
            let mut acc = Gf::zero();
            for cls in tap_classes(layout, &tap) {
                let Some(ring) = proof.rings.get(ring_idx) else {
                    return Err(FlockRsError::RingSwitch(RsOpenError::Shape));
                };
                if ring.s_v.len() != 128 {
                    return Err(FlockRsError::RingSwitch(RsOpenError::Shape));
                }
                let a_tbl = tap_inpack_table(layout, &tap, &surf.point, cls);
                acc += ring
                    .s_v
                    .iter()
                    .zip(a_tbl.iter())
                    .fold(Gf::zero(), |a, (s, e)| a + *s * *e);
                crate::ligerito::absorb_sv(transcript, &ring.s_v);
                ring_descs.push(tap_closure_desc(layout, &tap, &surf.point, cls));
                ring_idx += 1;
            }
            if acc != expect {
                return Err(FlockRsError::RingSwitch(RsOpenError::RingSwitchClaim));
            }
        }
    }
    if ring_idx != proof.rings.len() {
        return Err(FlockRsError::RingSwitch(RsOpenError::Shape));
    }

    // (6) r″ + ring η's → target → succinct closure → Ligerito.
    let r2: Vec<Gf> = transcript.get_field_challenges(LOG_PACKING, &());
    let eq_r2 = build_eq_x_r_vec(&r2, &()).expect("r2");
    let etas: Vec<Gf> = transcript.get_field_challenges(proof.rings.len(), &());
    let mut target = Gf::zero();
    for (i, ring) in proof.rings.iter().enumerate() {
        let s_u = crate::ligerito::transpose_bits_128(&ring.s_v);
        let beta = s_u.iter().zip(eq_r2.iter()).fold(Gf::zero(), |a, (su, e)| a + *su * *e);
        target += etas[i] * beta;
    }
    let m_p = packed_vars(&layout.p);
    let eval_b = |ris: &[F128], yr_log_n: usize| -> Vec<F128> {
        let ris_gf: Vec<Gf> = ris.iter().map(|&f| f128_to_gf(f)).collect();
        let mut out = vec![Gf::zero(); 1usize << yr_log_n];
        for (i, desc) in ring_descs.iter().enumerate() {
            let blk = residual_b_evals_tap(&ris_gf, yr_log_n, desc, &eq_r2);
            for (o, x) in out.iter_mut().zip(blk.iter()) {
                *o += etas[i] * *x;
            }
        }
        out.into_iter().map(gf_to_f128).collect()
    };
    let ok = ligerito::recursive_verifier_with_basis_succinct(
        vc,
        &proof.lig,
        m_p,
        gf_to_f128(target),
        &commitment.root,
        eval_b,
        &mut ZincChallenger(transcript),
    );
    if !ok {
        return Err(FlockRsError::LigeritoReject);
    }

    // (7) Read-off per cluster: Σ_c e_c·Σ_l 2^{c_w·l}·u^{(l)}_c mod q = T.
    for (ci, side) in proof.clusters.iter().enumerate() {
        let us_flat: Vec<u128> = side.us.iter().flat_map(|u| u.iter().copied()).collect();
        let y: crate::pcs::Fq = recombine_read_off(&p_x, &us_flat, 0, col_weights, c_w_x, lch_x);
        if y != targets[ci] {
            return Err(FlockRsError::Common(IntEvalRsError::ReadOff));
        }
    }
    Ok(())
}

/// Total proof bytes of a stream-family proof.
#[allow(clippy::arithmetic_side_effects)]
pub fn mle_eval_mod_q_lig_tap_family_size_breakdown(
    proof: &IntEvalRsLigTapFamilyProof,
) -> (ZincSideSizeBreakdown, usize) {
    use crate::transcript::traits::Transcribable;
    let mut b = ZincSideSizeBreakdown::default();
    let eqf_bytes = |d: &RlcDischargeEqf| {
        d.sc_a.get_num_bytes() + d.betas.len() * 16 + d.sc_b.get_num_bytes()
    };
    for side in &proof.clusters {
        for l in 0..side.mfs.len() {
            b.accumulate(&zinc_side_size_breakdown_merged(
                &side.mfs[l],
                &side.us[l],
                &side.presums[l],
            ));
        }
        if let Some(d) = &side.discharge_eqf {
            b.presum += eqf_bytes(d);
        }
        if let Some(d) = &side.discharge_eqf2 {
            b.presum += eqf_bytes(d);
        }
        b.presum += (side.omegas.len() + side.omegas2.len()) * 16;
    }
    b.s_v = proof.rings.len() * 128 * 16;
    (b, proof.lig.size_bytes())
}

// ---------------------------------------------------------------------
// EXPERIMENTAL — mod-q RLC claim families (docs/rlc-family-note-prompt.md).
//
// k claims `MLE[INT(a_i)](r_i) = c_i ∈ 𝔽_q` on F₂-linear forms
// `a_i = L_i(m_1, …, m_j)` of j committed UAIR columns, all claims sharing
// the COLUMN point. One γ-RLC collapses the k weight functions into a
// 2^j-CASE weight `W_b(m) = Σ_i γ_i·w_{i,b}·L_i(m) mod q`; ONE forest per
// weight chunk binds `α^{W_b^{(l)}(m(pos))}` (2^j-case leaf select on the
// committed bits — the derived vectors never materialise); the presum runs
// 2^j − 1 channels `R_S = eq ⊙ τ_S` against the monomials `Π_{i∈S} m_i`;
// the |S| ≥ 2 residuals discharge through ONE η-batched degree-(j+1)
// eq-sumcheck over the n' = t' + s x-tensor variables, exiting at committed
// openings `M̂_i(ρ)`; every residual opening rides the ONE recursive
// Ligerito call. The verifier recombines
// `Σ_c e_c·Σ_l 2^{c_w·l}·u'^{(l)}_c mod q ?= T = Σ_i γ_i·c_i`.
//
// Fiat–Shamir chain (prover and verifier in lockstep):
//   1. absorb root + statement (layout, family cols, forms, all (c_i, w_i));
//   2. draw γ_1..γ_k ∈ 𝔽_q (256-bit reduction, [`crate::pcs::fq_challenge`]);
//   3. per chunk l: merged forest (absorbs the α^{u} roots), then the
//      (2^j−1)-group presum;
//   4. draw the discharge η's, run the discharge, absorb the ω openings;
//   5. absorb the ring `s_v` messages (per chunk × family column at the
//      chunk's exit point, then per family column at the discharge exit ρ);
//   6. draw r″ + the ring η's, ONE `recursive_prover_with_basis` call.
//
// The evaluation field is the crate's fixed `q = 2^100 − 15`
// ([`crate::pcs::FQ_MOD`]) — the γ arithmetic is protocol-internal, so this
// family is NOT generic over the evaluation ring. NOT wired into
// `proof_codec`; the API is experimental.
// ---------------------------------------------------------------------

/// One claim of an RLC family: `MLE[INT(⊕_{i'∈form} m_{i'})](r) = claimed`.
/// Statement data — identical on the prover and verifier side.
pub struct RlcFamilyClaim<'a> {
    /// Nonzero bitmask over the family columns: bit `i'` set ⇔ `m_{i'}`
    /// participates in this claim's XOR.
    pub form: usize,
    /// Row weights over the x tensor's `2^{t'}` folded positions, reduced
    /// mod q (`w_b = eq(b, r_rows) mod q` for a genuine MLE claim; any
    /// `[0, q)` weights are accepted). Row points may differ per claim —
    /// only the COLUMN point is shared.
    pub row_weights_q: &'a [u128],
    /// The claimed evaluation `Σ_c e_c·Σ_b w_b·a[(b,c)] mod q`, canonical.
    pub claimed: u128,
}

/// End-to-end proof of an RLC claim family (EXPERIMENTAL).
/// One family's transcript slice of a merged MULTI-family proof
/// (EXPERIMENTAL) — the single-family proof minus the shared Ligerito
/// closure.
pub struct RlcFamilyPartProof {
    /// Per weight chunk: the family's merged 2^j-case forest.
    pub mfs: Vec<MergedForestProof>,
    /// `us[l][c]` — the combined case-weight chunk folds, range-checked.
    pub us: Vec<Vec<u128>>,
    /// Per chunk: the active-channel presum.
    pub presums: Vec<MultiDegreeSumcheckProof<Gf>>,
    /// The discharge cascade (as in the single-family proof).
    pub discharge_eqf: Option<RlcDischargeEqf>,
    pub omegas: Vec<Gf>,
    pub discharge_eqf2: Option<RlcDischargeEqf>,
    pub omegas2: Vec<Gf>,
    /// This family's ring messages, in its canonical ring order.
    pub rings: Vec<RingSwitchProof>,
}

/// One family of a merged multi-family statement: its columns and its
/// shared-point claims.
#[derive(Clone, Copy)]
pub struct RlcFamilySpec<'a> {
    /// The family's committed columns (j = len ∈ [1, 4]).
    pub family_cols: &'a [usize],
    /// Shared-point claims (form, claimed) over those columns.
    pub claims: &'a [RlcSharedClaim],
}

/// A merged multi-family proof: k families' fronts share ONE transcript,
/// one r″/η draw, and ONE closing Ligerito call (EXPERIMENTAL, tag 0x47).
pub struct IntEvalRsLigRlcFamiliesProof {
    /// Per family, in statement order.
    pub parts: Vec<RlcFamilyPartProof>,
    /// The ONE shared recursive opening.
    pub lig: LigeritoProof,
}

pub struct IntEvalRsLigRlcFamilyProof {
    /// Per weight chunk: ONE merged forest over the `2^s` x-tensor trees
    /// with 2^j-case leaves.
    pub mfs: Vec<MergedForestProof>,
    /// `us[l][c]` = the combined case-weight chunk folds
    /// `Σ_b W_b^{(l)}(m(b,c))`, range-checked `< 2^{c_w+t'+1}`.
    pub us: Vec<Vec<u128>>,
    /// Per chunk: the (2^j − 1)-channel presum (groups in ascending-S
    /// order; `Σ_S σ_S = e_d + 1`).
    pub presums: Vec<MultiDegreeSumcheckProof<Gf>>,
    /// Level 1 of the discharge CASCADE (j ≥ 2; absent when j = 1 or every
    /// |S| ≥ 2 channel elided): each active monomial channel factors into
    /// a PAIR of sides — committed columns and (for |S| ≥ 3) 2-bit AND
    /// intermediates, [`rlc_channel_sides`] — and `M̂ = 1 + ¬(side bits)`
    /// puts every pair in the driver's leaf-bit-affine shape (complement
    /// bit streams, all-ones τ). Two phases (rows then columns) with the
    /// absorbed per-spec entry sums β; the phase-B finals ARE the side
    /// openings.
    pub discharge_eqf: Option<RlcDischargeEqf>,
    /// Level-1 side openings at ρ, in canonical side order (committed
    /// columns ascending, then AND masks ascending). Column sides ring;
    /// AND sides are discharged by level 2.
    pub omegas: Vec<Gf>,
    /// Level 2 of the cascade (present iff level 1 has AND sides): the
    /// AND openings `M̂_a∧b(ρ)`, η'-batched at the single point ρ, each
    /// channel the pair of its two committed columns — same two-phase
    /// leaf-bit form, exiting at committed openings at ρ'.
    pub discharge_eqf2: Option<RlcDischargeEqf>,
    /// Level-2 committed openings at ρ' (ascending column order).
    pub omegas2: Vec<Gf>,
    /// Ring-switch messages, flat: per chunk × family column at the
    /// chunk's exit point, then per family column at ρ.
    pub rings: Vec<RingSwitchProof>,
    pub lig: LigeritoProof,
}

/// The two-phase j = 2 discharge (see
/// [`IntEvalRsLigRlcFamilyProof::discharge_eqf`]).
#[derive(Clone, Debug)]
pub struct RlcDischargeEqf {
    /// Phase A: the t'-variable leaf-bit sumcheck over the per-column
    /// groups (degree 3; claimed sum = `Σ_l η_l·μ_l`).
    pub sc_a: crate::piop::sumcheck::SumcheckProof<Gf>,
    /// Per-chunk phase-B entry sums `β_l = η_l·Σ_c eq(pt_l⁺, c)·F₁(c)·F₂(c)`
    /// (absorbed between the phases; phase A closes against
    /// `Σ_l eq(r_A, pt_l⁻)·β_l`, phase B opens at `Σ_l β_l`).
    pub betas: Vec<Gf>,
    /// Phase B: the s-variable Dense pair sumcheck over the per-column
    /// finals (degree 3; closes against `Σ_l η_l·eq(r_B, pt_l⁺)·ω₁·ω₂`).
    pub sc_b: crate::piop::sumcheck::SumcheckProof<Gf>,
}

/// Absorb the RLC-family statement (domain tag 0x40): commitment root,
/// layout shape, q, the family columns, and every claim's (form, claimed
/// value, row-weight vector). Everything the case weights are derived from
/// is in the transcript BEFORE the γ's are drawn.
#[allow(clippy::arithmetic_side_effects)]
fn absorb_rlc_family_statement(
    transcript: &mut impl Transcript,
    root: &flock_core::merkle::Hash,
    layout: &ShaF2Layout,
    family_cols: &[usize],
    forms: &[usize],
    claim_cs: &[u128],
    claim_weights: &[&[u128]],
) {
    let w_bytes: usize = claim_weights.iter().map(|w| w.len() * 16).sum();
    let mut bytes = Vec::with_capacity(1 + 32 + 12 * 8 + 16 + claim_cs.len() * 16 + w_bytes);
    bytes.push(0x40u8);
    bytes.extend_from_slice(root);
    for v in [
        layout.p.t,
        layout.p.s,
        layout.p.word_bits,
        layout.num_cols,
        layout.log_cols,
        layout.bit_vars,
        layout.num_vars,
        layout.tw,
        layout.x_fold_extra,
        family_cols.len(),
        claim_cs.len(),
    ] {
        bytes.extend_from_slice(&(v as u64).to_le_bytes());
    }
    bytes.extend_from_slice(&crate::pcs::FQ_MOD.to_le_bytes());
    for &c in family_cols {
        bytes.extend_from_slice(&(c as u64).to_le_bytes());
    }
    for &f in forms {
        bytes.extend_from_slice(&(f as u64).to_le_bytes());
    }
    for &c in claim_cs {
        bytes.extend_from_slice(&c.to_le_bytes());
    }
    for w in claim_weights {
        for &x in *w {
            bytes.extend_from_slice(&x.to_le_bytes());
        }
    }
    transcript.absorb_slice(&bytes);
}

/// One claim of a SHARED-POINT family: only the form and the claimed
/// value — the row-weight vector (and the column point) is shared across
/// the family, passed once to the `..._shared_point` entries.
#[derive(Clone, Copy, Debug)]
pub struct RlcSharedClaim {
    /// Nonzero bitmask over the family columns (as [`RlcFamilyClaim::form`]).
    pub form: usize,
    /// The claimed evaluation at the shared point, canonical in `[0, q)`.
    pub claimed: u128,
}

/// Canonicalize a shared-point claim list: keep the FIRST occurrence of
/// each form in caller order; a repeated form must carry the same claimed
/// value — at one point it is literally the SAME claim — else the
/// offending claim index is returned. The canonical (deduped) list is
/// what the transcript binds, so `k ≤ 2^j − 1` after this.
fn rlc_shared_point_canonical(
    claims: &[RlcSharedClaim],
) -> Result<(Vec<usize>, Vec<u128>), usize> {
    let mut forms: Vec<usize> = Vec::with_capacity(claims.len());
    let mut cs: Vec<u128> = Vec::with_capacity(claims.len());
    for (i, cl) in claims.iter().enumerate() {
        match forms.iter().position(|&f| f == cl.form) {
            None => {
                forms.push(cl.form);
                cs.push(cl.claimed);
            }
            Some(pos) if cs[pos] == cl.claimed => {}
            Some(_) => return Err(i),
        }
    }
    Ok((forms, cs))
}

/// Absorb the COLLAPSED shared-point statement (domain tag 0x41 — a
/// deliberately different transcript from the general family's 0x40):
/// commitment root, layout shape, q, the family columns, the canonical
/// (form, claimed) list, and the ONE shared row-weight vector — `2^{t'}`
/// weight words instead of the general absorb's `k·2^{t'}`. Everything
/// the case weights are derived from precedes the γ draw.
#[allow(clippy::arithmetic_side_effects)]
fn absorb_rlc_shared_point_statement(
    transcript: &mut impl Transcript,
    root: &flock_core::merkle::Hash,
    layout: &ShaF2Layout,
    family_cols: &[usize],
    forms: &[usize],
    claim_cs: &[u128],
    row_weights_q: &[u128],
) {
    let mut bytes = Vec::with_capacity(
        1 + 32
            + 12 * 8
            + 16
            + (family_cols.len() + forms.len()) * 8
            + (claim_cs.len() + row_weights_q.len()) * 16,
    );
    bytes.push(0x41u8);
    bytes.extend_from_slice(root);
    for v in [
        layout.p.t,
        layout.p.s,
        layout.p.word_bits,
        layout.num_cols,
        layout.log_cols,
        layout.bit_vars,
        layout.num_vars,
        layout.tw,
        layout.x_fold_extra,
        family_cols.len(),
        claim_cs.len(),
    ] {
        bytes.extend_from_slice(&(v as u64).to_le_bytes());
    }
    bytes.extend_from_slice(&crate::pcs::FQ_MOD.to_le_bytes());
    for &c in family_cols {
        bytes.extend_from_slice(&(c as u64).to_le_bytes());
    }
    for &f in forms {
        bytes.extend_from_slice(&(f as u64).to_le_bytes());
    }
    for &c in claim_cs {
        bytes.extend_from_slice(&c.to_le_bytes());
    }
    for &x in row_weights_q {
        bytes.extend_from_slice(&x.to_le_bytes());
    }
    transcript.absorb_slice(&bytes);
}

/// The ACTIVE presum channels of one chunk: the S whose τ_S table is not
/// identically zero. Structurally-zero channels exist for legitimate
/// degenerate families — e.g. a pure-XOR family (every claim on the same
/// ⊕-combination) has α^{W(m)} factoring through the XOR, killing the
/// AND channel — and MUST be elided for completeness (an included zero
/// channel would divide by R̂_S = 0). Public data: both sides derive the
/// same set from the case weights, per chunk.
fn rlc_active_channels(taus: &[Vec<Gf>]) -> Vec<usize> {
    (1..taus.len()).filter(|&s| taus[s].iter().any(|t| !t.is_zero())).collect()
}

/// One side of a discharge-cascade channel: a committed family column, or
/// a 2-bit AND intermediate (discharged by cascade level 2).
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
enum RlcSide {
    /// Family column index.
    Col(usize),
    /// AND of exactly two family columns (bitmask over [j], popcount 2).
    And(usize),
}

/// The two pair-factors of a |S| ≥ 2 channel: |S| = 2 → its two columns;
/// |S| = 3 → (AND of the two lowest bits, the highest column); |S| = 4 →
/// (AND of the two lowest, AND of the two highest). Deterministic — both
/// sides derive it; the AND intermediates are exactly the level-2
/// channels, so the cascade depth is 2 for every j ≤ 4.
fn rlc_channel_sides(s: usize) -> (RlcSide, RlcSide) {
    let bits: Vec<usize> = (0..8).filter(|b| (s >> b) & 1 == 1).collect();
    match bits.len() {
        2 => (RlcSide::Col(bits[0]), RlcSide::Col(bits[1])),
        3 => (RlcSide::And((1 << bits[0]) | (1 << bits[1])), RlcSide::Col(bits[2])),
        4 => (
            RlcSide::And((1 << bits[0]) | (1 << bits[1])),
            RlcSide::And((1 << bits[2]) | (1 << bits[3])),
        ),
        _ => unreachable!("cascade channels have 2..=4 members"),
    }
}

/// One spec of a cascade level: (exit point, η, ¬side-A rows, ¬side-B rows).
type RlcEqfSpec<'a> = (&'a [Gf], Gf, &'a [Vec<u64>], &'a [Vec<u64>]);

/// Complement a side's per-tree bit rows (`M̂ = 1 + ¬bits·1`).
fn rlc_not_rows(rows: &[Vec<u64>]) -> Vec<Vec<u64>> {
    rows.iter().map(|r| r.iter().map(|&w| !w).collect()).collect()
}

/// Prove one level of the leaf-bit discharge cascade:
/// `Σ_g η_g·Σ_x eq(pt_g, x)·Â_g(x)·B̂_g(x)` over the n' x-tensor
/// variables, each side's MLE in the leaf-bit-affine form
/// `1 + ¬(side bits)·1`. Phase A binds the t' row variables (per-(spec,
/// column) `Leaf3Bits` groups — nothing dense is materialised), phase B
/// the s column variables (per-spec Dense pairs over the per-column
/// finals), joined by the absorbed per-spec entry sums β. Returns the
/// proof part, the exit ρ, and each spec's side openings `(Â(ρ), B̂(ρ))`.
#[allow(clippy::arithmetic_side_effects)]
#[allow(clippy::type_complexity)]
fn rlc_prove_eqf_level(
    transcript: &mut (impl Transcript + Send),
    p_x: &IntEvalParams,
    t_x: usize,
    specs: &[RlcEqfSpec<'_>],
) -> (RlcDischargeEqf, Vec<Gf>, Vec<(Gf, Gf)>) {
    use crate::piop::sumcheck::eq_factored::{
        EqInnerGroupMixed, GroupBufs, prove_eq_inner_sumcheck_mixed,
    };
    use crate::poly::utils::build_eq_x_r_vec;
    let cols = p_x.cols();
    let ones_tau = vec![Gf::one(); p_x.rows()];
    let tau_sets = vec![(ones_tau.clone(), ones_tau)];
    let eq_tops: Vec<Vec<Gf>> = specs
        .iter()
        .map(|(pt, ..)| build_eq_x_r_vec(&pt[t_x..], &()).expect("s >= 1"))
        .collect();
    let deep = t_x >= 4 && crate::merged_forest::forest_lut3();
    let mut groups_a: Vec<EqInnerGroupMixed<Gf>> = Vec::with_capacity(specs.len() * cols);
    for (si, (pt, eta, na, nb)) in specs.iter().enumerate() {
        for c in 0..cols {
            let (lbits, rbits) = (na[c].clone(), nb[c].clone());
            groups_a.push(EqInnerGroupMixed {
                q: pt[..t_x].to_vec(),
                scale: *eta * eq_tops[si][c],
                bufs: if deep {
                    GroupBufs::Leaf3Bits { lbits, rbits, tau_set: 0 }
                } else {
                    GroupBufs::Leaf2Bits { lbits, rbits, tau_set: 0 }
                },
            });
        }
    }
    let (sc_a, r_a, finals_a) =
        prove_eq_inner_sumcheck_mixed(transcript, groups_a, &tau_sets, &[], &[], &());
    // Per-spec per-column finals: F_A[c] = Â(r_A, c), F_B[c] = B̂(r_A, c).
    let f_pairs: Vec<(Vec<Gf>, Vec<Gf>)> = (0..specs.len())
        .map(|si| {
            let base = si * cols;
            (
                (0..cols).map(|c| finals_a[base + c][0].0).collect(),
                (0..cols).map(|c| finals_a[base + c][0].1).collect(),
            )
        })
        .collect();
    let betas: Vec<Gf> = specs
        .iter()
        .enumerate()
        .map(|(si, (_, eta, ..))| {
            let (fa, fb) = &f_pairs[si];
            (0..cols).fold(Gf::zero(), |a, c| a + *eta * eq_tops[si][c] * fa[c] * fb[c])
        })
        .collect();
    crate::ligerito::absorb_rlc_betas(transcript, &betas);
    let mut groups_b: Vec<EqInnerGroupMixed<Gf>> = Vec::with_capacity(specs.len());
    for ((pt, eta, ..), (fa, fb)) in specs.iter().zip(f_pairs) {
        groups_b.push(EqInnerGroupMixed {
            q: pt[t_x..].to_vec(),
            scale: *eta,
            bufs: GroupBufs::Dense(vec![(fa, fb)]),
        });
    }
    let (sc_b, r_b, finals_b) =
        prove_eq_inner_sumcheck_mixed(transcript, groups_b, &[], &[], &[], &());
    let side_vals: Vec<(Gf, Gf)> = (0..specs.len()).map(|si| finals_b[si][0]).collect();
    let rho: Vec<Gf> = r_a.iter().chain(r_b.iter()).copied().collect();
    (RlcDischargeEqf { sc_a, betas, sc_b }, rho, side_vals)
}

/// Verify one cascade level: phase A's claimed sum must equal the
/// η-weighted targets, its closing `Σ_g eq(r_A, pt_g⁻)·β_g`; phase B
/// opens at `Σ β` and closes at `Σ_g η_g·eq(r_B, pt_g⁺)·ωa_g·ωb_g` with
/// the resolved side openings. Returns ρ.
#[allow(clippy::arithmetic_side_effects)]
fn rlc_verify_eqf_level(
    transcript: &mut (impl Transcript + Send),
    t_x: usize,
    s_vars: usize,
    d: &RlcDischargeEqf,
    specs: &[(&[Gf], Gf)],
    claimed: &[Gf],
    side_vals: &[(Gf, Gf)],
) -> Result<Vec<Gf>, FlockRsError> {
    use crate::piop::sumcheck::MLSumcheck;
    use crate::poly::utils::eq_eval;
    let one = Gf::one();
    if d.betas.len() != specs.len() {
        return Err(FlockRsError::RingSwitch(RsOpenError::Shape));
    }
    let want_sum = claimed.iter().fold(Gf::zero(), |a, &b| a + b);
    if d.sc_a.claimed_sum != want_sum {
        return Err(FlockRsError::Common(IntEvalRsError::Discharge));
    }
    let sub_a = MLSumcheck::<Gf>::verify_as_subprotocol(transcript, t_x, 3, &d.sc_a, &())
        .map_err(|_| FlockRsError::Common(IntEvalRsError::Discharge))?;
    let expected_a = sub_a.expected_evaluation;
    let r_a = sub_a.point;
    let want_a = specs.iter().zip(d.betas.iter()).try_fold(
        Gf::zero(),
        |a, ((pt, _), &beta)| {
            eq_eval(&r_a, &pt[..t_x], one)
                .map(|e| a + e * beta)
                .map_err(|_| FlockRsError::RingSwitch(RsOpenError::Shape))
        },
    )?;
    if expected_a != want_a {
        return Err(FlockRsError::Common(IntEvalRsError::Discharge));
    }
    crate::ligerito::absorb_rlc_betas(transcript, &d.betas);
    let beta_sum = d.betas.iter().fold(Gf::zero(), |a, &b| a + b);
    if d.sc_b.claimed_sum != beta_sum {
        return Err(FlockRsError::Common(IntEvalRsError::Discharge));
    }
    let sub_b = MLSumcheck::<Gf>::verify_as_subprotocol(transcript, s_vars, 3, &d.sc_b, &())
        .map_err(|_| FlockRsError::Common(IntEvalRsError::Discharge))?;
    let expected_b = sub_b.expected_evaluation;
    let r_b = sub_b.point;
    let want_b = specs.iter().zip(side_vals.iter()).try_fold(
        Gf::zero(),
        |a, ((pt, eta), &(oa, ob))| {
            eq_eval(&r_b, &pt[t_x..], one)
                .map(|e| a + *eta * e * oa * ob)
                .map_err(|_| FlockRsError::RingSwitch(RsOpenError::Shape))
        },
    )?;
    if expected_b != want_b {
        return Err(FlockRsError::Common(IntEvalRsError::Discharge));
    }
    Ok(r_a.into_iter().chain(r_b).collect())
}

/// One chunk's eager 2^j-case forest leaves and per-column folds: leaf at
/// x-position `(b, c)` is `case_pow[b][m(b,c)]` and
/// `u_c = Σ_b W_b^{(l)}(m(b,c))`, with `m(b,c)` gathered from the j family
/// columns' x-tensor bit rows. Flat leaf order `(c ≪ t') | b` — the
/// [`prove_merged_forest`] layout. Eager: the leaf table is materialised
/// (16 B per position) — the reference / j ≥ 3 fallback path.
#[allow(clippy::arithmetic_side_effects)]
fn rlc_leaves_and_folds(
    p_x: &IntEvalParams,
    x_rows: &[Vec<Vec<u64>>],
    case_w: &[Vec<u128>],
    case_pow: &[Vec<Gf>],
) -> (Vec<Gf>, Vec<u128>) {
    let rows = p_x.rows();
    let per_col: Vec<(Vec<Gf>, u128)> = cfg_into_iter!(0..p_x.cols())
        .map(|c| {
            let mut leaf_col = Vec::with_capacity(rows);
            let mut u = 0u128;
            for i in 0..rows {
                let mut m = 0usize;
                for (fi, xr) in x_rows.iter().enumerate() {
                    m |= (((xr[c][i >> 6] >> (i & 63)) & 1) as usize) << fi;
                }
                leaf_col.push(case_pow[i][m]);
                // < 2^{c_w}·2^{t'} < 2^127: no overflow.
                u += case_w[i][m];
            }
            (leaf_col, u)
        })
        .collect();
    let mut leaves = Vec::with_capacity(rows << p_x.s);
    let mut us = Vec::with_capacity(p_x.cols());
    for (leaf_col, u) in per_col {
        leaves.extend_from_slice(&leaf_col);
        us.push(u);
    }
    (leaves, us)
}

/// The per-column case-weight folds alone (`u_c = Σ_b W_b^{(l)}(m(b,c))`)
/// — the lazy forest paths compute the folds without materialising leaves.
#[allow(clippy::arithmetic_side_effects)]
fn rlc_folds(
    p_x: &IntEvalParams,
    x_rows: &[Vec<Vec<u64>>],
    case_w: &[Vec<u128>],
) -> Vec<u128> {
    let rows = p_x.rows();
    cfg_into_iter!(0..p_x.cols())
        .map(|c| {
            let mut u = 0u128;
            for i in 0..rows {
                let mut m = 0usize;
                for (fi, xr) in x_rows.iter().enumerate() {
                    m |= (((xr[c][i >> 6] >> (i & 63)) & 1) as usize) << fi;
                }
                u += case_w[i][m];
            }
            u
        })
        .collect()
}

/// `F2Z_RLC_EAGER=1` forces the materialised-leaf forest on the RLC-family
/// prover (A/B / diagnostic); unset, j ≤ 2 run the lazy bit-driven paths.
/// Byte-identical proofs either way. Read once per prove call.
fn rlc_eager_forced() -> bool {
    std::env::var("F2Z_RLC_EAGER").is_ok_and(|v| v == "1")
}

/// Prover-side output of the discharge cascade.
struct RlcDischargeOut {
    d1: Option<RlcDischargeEqf>,
    omegas: Vec<Gf>,
    side_list: Vec<RlcSide>,
    rho: Vec<Gf>,
    d2: Option<RlcDischargeEqf>,
    omegas2: Vec<Gf>,
    omega2_fis: Vec<usize>,
    rho2: Vec<Gf>,
}

/// Prove k RLC-family claims against the commitment (EXPERIMENTAL — see
/// the module-section comment for the protocol and Fiat–Shamir chain).
/// `family_cols` are the j committed UAIR columns `m_1..m_j`; every
/// claim's form is a bitmask over them. Layout/weight conventions as
/// [`prove_mle_eval_mod_q_ligerito_with_virtual_xors`]; the evaluation
/// field is the fixed `q = 2^100 − 15`.
#[allow(clippy::arithmetic_side_effects)]
pub fn prove_mle_eval_mod_q_ligerito_rlc_family(
    transcript: &mut (impl Transcript + Send),
    hint: &FlockCommitHint,
    layout: &ShaF2Layout,
    family_cols: &[usize],
    claims: &[RlcFamilyClaim<'_>],
    alpha: Gf,
    pc: &LigProverConfig,
) -> IntEvalRsLigRlcFamilyProof {
    use crate::pcs::{
        FQ_BITS, FQ_MOD, fq_challenge, mod_q_chunk_width, mod_q_num_chunks, rlc_case_weights,
        rlc_chunk_case_weights, virtual_xor_params,
    };

    let p = &layout.p;
    assert_eq!(p.word_bits, 1, "RLC-family claims assume the W=1 SHA layout");
    let j = family_cols.len();
    assert!((1..=4).contains(&j), "family size j must be in [1, 4]");
    for &i in family_cols {
        assert!(i < layout.num_cols, "family column {i} out of range (< {})", layout.num_cols);
    }
    let k = claims.len();
    assert!(k >= 1, "need at least one claim");
    let p_x = virtual_xor_params(layout);
    let t_x = row_bit_vars(&p_x);
    assert!(t_x >= 6, "RLC-family presum needs t' ≥ 6 (whole-word x rows); got t'={t_x}");
    for cl in claims {
        assert!(
            cl.form != 0 && cl.form < (1usize << j),
            "claim form must be a nonzero bitmask over [j]"
        );
        assert_eq!(cl.row_weights_q.len(), p_x.rows(), "claim row-weight length");
        assert!(cl.claimed < FQ_MOD, "claimed value must be a canonical 𝔽_q representative");
    }

    // (1)–(2) Statement → γ's.
    let forms: Vec<usize> = claims.iter().map(|cl| cl.form).collect();
    for fi in 0..j {
        assert!(
            forms.iter().any(|f| (f >> fi) & 1 == 1),
            "family column index {fi} appears in no claim form"
        );
    }
    let claim_cs: Vec<u128> = claims.iter().map(|cl| cl.claimed).collect();
    let w_refs: Vec<&[u128]> = claims.iter().map(|cl| cl.row_weights_q).collect();
    absorb_rlc_family_statement(
        transcript,
        hint.root(),
        layout,
        family_cols,
        &forms,
        &claim_cs,
        &w_refs,
    );
    let gammas: Vec<u128> = (0..k).map(|_| fq_challenge(transcript)).collect();

    // Case weights + chunking (c_w over the x geometry).
    let case_chunks = {
        let _g = crate::utils::prof::scope("rlc:casew");
        let case_w = rlc_case_weights(&w_refs, &gammas, &forms, j);
        rlc_chunk_case_weights(&case_w, mod_q_chunk_width(&p_x), mod_q_num_chunks(&p_x, FQ_BITS))
    };
    prove_rlc_family_core(transcript, hint, layout, family_cols, &case_chunks, alpha, pc)
}

/// Prove a SHARED-POINT RLC claim family (EXPERIMENTAL): all `k` claims at
/// ONE evaluation point — one shared `row_weights_q` (and, as for every
/// family, one column point) — so two claims with the same form are the
/// SAME claim (deduped here; a repeated form with a different claimed
/// value is rejected) and the maximal family is the full XOR-closure of
/// the `j` columns, `k ≤ 2^j − 1`. The Fiat–Shamir statement absorbs the
/// ONE weight vector plus the (form, value) list — a deliberately
/// DIFFERENT (smaller) transcript than the same claims through
/// [`prove_mle_eval_mod_q_ligerito_rlc_family`] — and the case-weight
/// table is built rank-1, `W_b(m) = w_b·Γ(m) mod q`
/// ([`rlc_case_weights_shared_point`]). Everything downstream of the γ
/// draw is the general family core, byte for byte.
#[allow(clippy::arithmetic_side_effects)]
#[allow(clippy::too_many_arguments)]
pub fn prove_mle_eval_mod_q_ligerito_rlc_family_shared_point(
    transcript: &mut (impl Transcript + Send),
    hint: &FlockCommitHint,
    layout: &ShaF2Layout,
    family_cols: &[usize],
    row_weights_q: &[u128],
    claims: &[RlcSharedClaim],
    alpha: Gf,
    pc: &LigProverConfig,
) -> IntEvalRsLigRlcFamilyProof {
    use crate::pcs::{
        FQ_BITS, FQ_MOD, fq_challenge, mod_q_chunk_width, mod_q_num_chunks, rlc_case_weights_shared_point,
        rlc_chunk_case_weights, rlc_gamma_cases, virtual_xor_params,
    };

    let p = &layout.p;
    assert_eq!(p.word_bits, 1, "RLC-family claims assume the W=1 SHA layout");
    let j = family_cols.len();
    assert!((1..=4).contains(&j), "family size j must be in [1, 4]");
    for &i in family_cols {
        assert!(i < layout.num_cols, "family column {i} out of range (< {})", layout.num_cols);
    }
    assert!(!claims.is_empty(), "need at least one claim");
    let p_x = virtual_xor_params(layout);
    let t_x = row_bit_vars(&p_x);
    assert!(t_x >= 6, "RLC-family presum needs t' ≥ 6 (whole-word x rows); got t'={t_x}");
    assert_eq!(row_weights_q.len(), p_x.rows(), "shared row-weight length");
    for cl in claims {
        assert!(
            cl.form != 0 && cl.form < (1usize << j),
            "claim form must be a nonzero bitmask over [j]"
        );
        assert!(cl.claimed < FQ_MOD, "claimed value must be a canonical 𝔽_q representative");
    }

    // (1)–(2) Canonical statement (deduped) → γ's.
    let (forms, claim_cs) = rlc_shared_point_canonical(claims).unwrap_or_else(|i| {
        panic!("claim {i} repeats an earlier form with a DIFFERENT claimed value")
    });
    for fi in 0..j {
        assert!(
            forms.iter().any(|f| (f >> fi) & 1 == 1),
            "family column index {fi} appears in no claim form"
        );
    }
    absorb_rlc_shared_point_statement(
        transcript,
        hint.root(),
        layout,
        family_cols,
        &forms,
        &claim_cs,
        row_weights_q,
    );
    let gammas: Vec<u128> = (0..forms.len()).map(|_| fq_challenge(transcript)).collect();

    // Rank-1 case weights + chunking.
    let case_chunks = {
        let _g = crate::utils::prof::scope("rlc:casew");
        let case_w =
            rlc_case_weights_shared_point(row_weights_q, &rlc_gamma_cases(&gammas, &forms, j));
        rlc_chunk_case_weights(&case_w, mod_q_chunk_width(&p_x), mod_q_num_chunks(&p_x, FQ_BITS))
    };
    prove_rlc_family_core(transcript, hint, layout, family_cols, &case_chunks, alpha, pc)
}

/// The family prover body shared by the general and shared-point entries:
/// everything downstream of the γ draw — chunked case weights in, proof
/// out. Byte-identical to the pre-split general prover from this point on.
#[allow(clippy::arithmetic_side_effects)]
fn prove_rlc_family_core(
    transcript: &mut (impl Transcript + Send),
    hint: &FlockCommitHint,
    layout: &ShaF2Layout,
    family_cols: &[usize],
    case_chunks: &[Vec<Vec<u128>>],
    alpha: Gf,
    pc: &LigProverConfig,
) -> IntEvalRsLigRlcFamilyProof {
    let (part, ring_data) =
        prove_rlc_family_front(transcript, hint, layout, family_cols, case_chunks, alpha);
    let lig =
        prove_rlc_families_closure(transcript, hint, layout, &[(&part, &ring_data)], pc);
    IntEvalRsLigRlcFamilyProof {
        mfs: part.mfs,
        us: part.us,
        presums: part.presums,
        discharge_eqf: part.discharge_eqf,
        omegas: part.omegas,
        discharge_eqf2: part.discharge_eqf2,
        omegas2: part.omegas2,
        rings: part.rings,
        lig,
    }
}

/// One family's FRONT: extraction, per-chunk case forests + presums, the
/// discharge cascade, and the ring messages — everything absorbed, no
/// closure. Returns the part proof and the (eq table, columns) data each
/// ring spec contributes to the shared basis.
#[allow(clippy::arithmetic_side_effects)]
#[allow(clippy::type_complexity)]
fn prove_rlc_family_front(
    transcript: &mut (impl Transcript + Send),
    hint: &FlockCommitHint,
    layout: &ShaF2Layout,
    family_cols: &[usize],
    case_chunks: &[Vec<Vec<u128>>],
    alpha: Gf,
) -> (RlcFamilyPartProof, Vec<(Vec<Gf>, Vec<usize>)>) {
    use crate::merged_forest::prove_merged_forest;
    use crate::pcs::{extract_virtual_xor_rows, rlc_case_pow_table, rlc_tau_tables, virtual_xor_params};
    use crate::piop::sumcheck::multi_degree::{MultiDegreeSumcheck, MultiDegreeSumcheckGroup};
    use crate::poly::mle::DenseMultilinearExtension;
    use crate::poly::utils::build_eq_x_r_vec;
    use crypto_primitives::Field;

    let j = family_cols.len();
    let p_x = virtual_xor_params(layout);
    let t_x = row_bit_vars(&p_x);
    let lch_x = case_chunks.len();

    // Family-column x rows + the 2^j − 1 monomial (AND) row sets, built once.
    let x_rows: Vec<Vec<Vec<u64>>> = {
        let _g = crate::utils::prof::scope("rlc:extract");
        family_cols
            .iter()
            .map(|&i| {
                extract_virtual_xor_rows(layout, hint.rows(), core::slice::from_ref(&i), 0, None)
            })
            .collect()
    };
    let and_rows: Vec<Vec<Vec<u64>>> = (1..1usize << j)
        .map(|s| {
            let members: Vec<usize> = (0..j).filter(|&fi| (s >> fi) & 1 == 1).collect();
            let mut rows = x_rows[members[0]].clone();
            for &fi in &members[1..] {
                for (rw, xw) in rows.iter_mut().zip(x_rows[fi].iter()) {
                    for (a, b) in rw.iter_mut().zip(xw.iter()) {
                        *a &= *b;
                    }
                }
            }
            rows
        })
        .collect();

    // (3) Per chunk: eager 2^j-case forest + (2^j − 1)-channel presum.
    let one = Gf::one();
    let zero_inner = Gf::zero().into_inner();
    let mut mfs = Vec::with_capacity(lch_x);
    let mut us = Vec::with_capacity(lch_x);
    let mut presums = Vec::with_capacity(lch_x);
    let mut points: Vec<Vec<Gf>> = Vec::with_capacity(lch_x);
    let mut actives: Vec<Vec<usize>> = Vec::with_capacity(lch_x);
    // Forest dispatch (every arm byte-identical): j = 2 runs the lazy
    // 4-case Pair/T4 schedule ([`prove_merged_forest_lazy_rlc2`]); j = 1
    // leaves are bit-affine (`1 + m·(A−1)` with `A = case_pow[b][1]`),
    // exactly the base scheme's lazy forest; j = 3, 4 default to the
    // EAGER forest — measured faster than the Dense-JIT form at n = 24–28
    // (the 8/16-case leaf-ROUND kernels remain the open lever) —
    // with `F2Z_RLC_J34_LAZY=1` opting into the low-peak-memory JIT form
    // ([`prove_merged_forest_lazy_rlc_general`], ~⅓ the peak).
    let lazy = !rlc_eager_forced();
    let j34_lazy = std::env::var("F2Z_RLC_J34_LAZY").is_ok_and(|v| v == "1");
    let packed_x1 = if lazy && j == 1 {
        Some(crate::ligerito::pack_columns_from_rows(&p_x, &x_rows[0]))
    } else {
        None
    };
    for chunk in case_chunks {
        let _g = crate::utils::prof::scope("rlc:chunk");
        let case_pow = {
            let _g = crate::utils::prof::scope("rlc:pows");
            rlc_case_pow_table(chunk, alpha)
        };
        let (u, mf, z, e_d) = if lazy && j == 2 {
            let u = {
                let _g = crate::utils::prof::scope("rlc:folds");
                rlc_folds(&p_x, &x_rows, chunk)
            };
            let (_roots, mf, z, e_d) = {
                let _g = crate::utils::prof::scope("rlc:forest");
                crate::merged_forest::prove_merged_forest_lazy_rlc2(
                    transcript, &p_x, &x_rows[0], &x_rows[1], &case_pow,
                )
            };
            (u, mf, z, e_d)
        } else if lazy && j >= 3 && j34_lazy {
            let u = {
                let _g = crate::utils::prof::scope("rlc:folds");
                rlc_folds(&p_x, &x_rows, chunk)
            };
            let row_refs: Vec<&[Vec<u64>]> = x_rows.iter().map(|r| &r[..]).collect();
            let (_roots, mf, z, e_d) = {
                let _g = crate::utils::prof::scope("rlc:forest");
                crate::merged_forest::prove_merged_forest_lazy_rlc_general(
                    transcript, &p_x, &row_refs, &case_pow,
                )
            };
            (u, mf, z, e_d)
        } else if let Some(packed) = &packed_x1 {
            let u = {
                let _g = crate::utils::prof::scope("rlc:folds");
                rlc_folds(&p_x, &x_rows, chunk)
            };
            let pow2: Vec<Vec<Gf>> = case_pow.iter().map(|r| vec![r[1]]).collect();
            let (_roots, mf, z, e_d) = {
                let _g = crate::utils::prof::scope("rlc:forest");
                // Derived x-channels, not the witness layout — no column
                // elision here (the tail is not generally zero).
                let live = p_x.cols();
                crate::merged_forest::prove_merged_forest_lazy(transcript, &p_x, packed, &pow2, live)
            };
            (u, mf, z, e_d)
        } else {
            let (leaves, u) = {
                let _g = crate::utils::prof::scope("rlc:leaves");
                rlc_leaves_and_folds(&p_x, &x_rows, chunk, &case_pow)
            };
            let (_roots, mf, z, e_d) = {
                let _g = crate::utils::prof::scope("rlc:forest");
                prove_merged_forest(transcript, &leaves, t_x, p_x.s)
            };
            (u, mf, z, e_d)
        };

        let _g_ps = crate::utils::prof::scope("rlc:presum");
        let (z_bj, z_c) = z.split_at(t_x);
        let eq_zbj = build_eq_x_r_vec(z_bj, &()).expect("t' >= 1");
        let eq_zc = build_eq_x_r_vec(z_c, &()).expect("s >= 1");
        let taus = rlc_tau_tables(&case_pow);
        // Zero channels (legitimate degenerate families) are ELIDED; both
        // sides derive the active set from the public case weights.
        let active = rlc_active_channels(&taus);
        assert!(!active.is_empty(), "degenerate statement: every presum channel vanished");
        let m_tbls: Vec<Vec<Gf>> = active
            .iter()
            .map(|&s| crate::ligerito::xi_combined_rows(&p_x, &and_rows[s - 1], &eq_zc))
            .collect();
        let to_mle = |tbl: Vec<Gf>| {
            DenseMultilinearExtension::from_evaluations_vec(
                t_x,
                tbl.into_iter().map(|g| g.into_inner()).collect(),
                zero_inner,
            )
        };
        let groups: Vec<MultiDegreeSumcheckGroup<Gf>> = active
            .iter()
            .enumerate()
            .map(|(gi, &s)| {
                let r_tbl: Vec<Gf> =
                    eq_zbj.iter().zip(taus[s].iter()).map(|(&e, &t)| e * t).collect();
                MultiDegreeSumcheckGroup::new(
                    2,
                    vec![to_mle(r_tbl), to_mle(m_tbls[gi].clone())],
                    Box::new(|vals: &[Gf]| vals[0] * vals[1]),
                )
            })
            .collect();
        let (presum, states) =
            MultiDegreeSumcheck::<Gf>::prove_as_subprotocol(transcript, groups, t_x, &());
        debug_assert_eq!(
            presum.claimed_sums().iter().fold(Gf::zero(), |a, &b| a + b),
            e_d + one,
            "presum channels must sum to the forest exit claim"
        );
        let r_star = states[0].randomness.clone();
        actives.push(active);
        let point: Vec<Gf> = r_star.iter().chain(z_c.iter()).copied().collect();
        mfs.push(mf);
        us.push(u);
        presums.push(presum);
        points.push(point);
    }

    // (4) The discharge CASCADE. Level 1: every ACTIVE |S| ≥ 2 channel of
    // every chunk factors into a pair of sides ([`rlc_channel_sides`] —
    // committed columns and, for |S| ≥ 3, 2-bit AND intermediates built
    // from the already-extracted AND rows) and the whole batch runs as ONE
    // two-phase leaf-bit sumcheck; the side openings ω ride the proof.
    // Level 2 (present iff AND sides exist): the AND openings at ρ are
    // η'-batched and discharged the same way, exiting at committed
    // openings at ρ'. Cascade depth is 2 for every j ≤ 4.
    let l1_pairs: Vec<(usize, usize)> = {
        let mut v = Vec::new();
        for (l, act) in actives.iter().enumerate() {
            for &ch in act {
                if ch.count_ones() >= 2 {
                    v.push((l, ch));
                }
            }
        }
        v
    };
    let dis = if l1_pairs.is_empty() {
        None
    } else {
        let _g = crate::utils::prof::scope("rlc:discharge");
        let etas_dis: Vec<Gf> = transcript.get_field_challenges(l1_pairs.len(), &());
        let _g_t = crate::utils::prof::scope("rlc:dis_tbls");
        // Canonical side list (columns ascending, then AND masks) and the
        // complemented bit rows per distinct side.
        let side_list: Vec<RlcSide> = {
            let mut v: Vec<RlcSide> = Vec::new();
            for &(_, ch) in &l1_pairs {
                let (a, b) = rlc_channel_sides(ch);
                for sd in [a, b] {
                    if !v.contains(&sd) {
                        v.push(sd);
                    }
                }
            }
            v.sort_unstable();
            v
        };
        let side_rows = |sd: RlcSide| -> &Vec<Vec<u64>> {
            match sd {
                RlcSide::Col(fi) => &x_rows[fi],
                RlcSide::And(mask) => &and_rows[mask - 1],
            }
        };
        let not_cache: Vec<Vec<Vec<u64>>> =
            side_list.iter().map(|&sd| rlc_not_rows(side_rows(sd))).collect();
        let not_of = |sd: RlcSide| -> &Vec<Vec<u64>> {
            &not_cache[side_list.iter().position(|&x| x == sd).expect("side in list")]
        };
        let specs: Vec<RlcEqfSpec<'_>> = l1_pairs
            .iter()
            .enumerate()
            .map(|(i, &(l, ch))| {
                let (a, b) = rlc_channel_sides(ch);
                (&points[l][..], etas_dis[i], &not_of(a)[..], &not_of(b)[..])
            })
            .collect();
        drop(_g_t);
        let _g_r = crate::utils::prof::scope("rlc:dis_run");
        let (d1, rho, vals) = rlc_prove_eqf_level(transcript, &p_x, t_x, &specs);
        // One ω per distinct side (first-occurrence value).
        let omegas: Vec<Gf> = side_list
            .iter()
            .map(|&sd| {
                for (i, &(_, ch)) in l1_pairs.iter().enumerate() {
                    let (a, b) = rlc_channel_sides(ch);
                    if a == sd {
                        return vals[i].0;
                    }
                    if b == sd {
                        return vals[i].1;
                    }
                }
                unreachable!("side_list derives from l1_pairs")
            })
            .collect();
        crate::ligerito::absorb_rlc_omegas(transcript, &omegas);
        // Level 2: discharge the AND openings.
        let and_masks: Vec<usize> = side_list
            .iter()
            .filter_map(|&sd| match sd {
                RlcSide::And(mask) => Some(mask),
                RlcSide::Col(_) => None,
            })
            .collect();
        let out = if and_masks.is_empty() {
            RlcDischargeOut {
                d1: Some(d1),
                omegas,
                side_list,
                rho,
                d2: None,
                omegas2: Vec::new(),
                omega2_fis: Vec::new(),
                rho2: Vec::new(),
            }
        } else {
            let etas2: Vec<Gf> = transcript.get_field_challenges(and_masks.len(), &());
            let not_singles: Vec<(usize, Vec<Vec<u64>>)> = {
                let mut fis: Vec<usize> = and_masks
                    .iter()
                    .flat_map(|&mask| (0..j).filter(move |fi| (mask >> fi) & 1 == 1))
                    .collect();
                fis.sort_unstable();
                fis.dedup();
                fis.iter().map(|&fi| (fi, rlc_not_rows(&x_rows[fi]))).collect()
            };
            let not_single = |fi: usize| -> &[Vec<u64>] {
                &not_singles.iter().find(|(f, _)| *f == fi).expect("member cached").1
            };
            let specs2: Vec<RlcEqfSpec<'_>> = and_masks
                .iter()
                .enumerate()
                .map(|(i, &mask)| {
                    let mut bits = (0..j).filter(|fi| (mask >> fi) & 1 == 1);
                    let (a, b) = (bits.next().expect("2 bits"), bits.next().expect("2 bits"));
                    (&rho[..], etas2[i], not_single(a), not_single(b))
                })
                .collect();
            let (d2, rho2, vals2) = rlc_prove_eqf_level(transcript, &p_x, t_x, &specs2);
            let omega2_fis: Vec<usize> = not_singles.iter().map(|(fi, _)| *fi).collect();
            let omegas2: Vec<Gf> = omega2_fis
                .iter()
                .map(|&fi| {
                    for (i, &mask) in and_masks.iter().enumerate() {
                        let mut bits = (0..j).filter(|f| (mask >> f) & 1 == 1);
                        let (a, b) =
                            (bits.next().expect("2 bits"), bits.next().expect("2 bits"));
                        if a == fi {
                            return vals2[i].0;
                        }
                        if b == fi {
                            return vals2[i].1;
                        }
                    }
                    unreachable!("omega2 columns derive from and_masks")
                })
                .collect();
            crate::ligerito::absorb_rlc_omegas(transcript, &omegas2);
            RlcDischargeOut {
                d1: Some(d1),
                omegas,
                side_list,
                rho,
                d2: Some(d2),
                omegas2,
                omega2_fis,
                rho2,
            }
        };
        drop(_g_r);
        Some(out)
    };
    let dis = dis.unwrap_or(RlcDischargeOut {
        d1: None,
        omegas: Vec::new(),
        side_list: Vec::new(),
        rho: Vec::new(),
        d2: None,
        omegas2: Vec::new(),
        omega2_fis: Vec::new(),
        rho2: Vec::new(),
    });

    // (5) Rings: per chunk, the ACTIVE singleton channels' columns at the
    // chunk's exit; then the level-1 COLUMN sides at ρ; then the level-2
    // committed exits at ρ'. (AND sides never ring — level 2 discharges
    // them.)
    let _g_r = crate::utils::prof::scope("rlc:rings");
    let mut ring_specs: Vec<(&[Gf], Vec<usize>)> = Vec::with_capacity(lch_x + 2);
    for (l, pt) in points.iter().enumerate() {
        let cols: Vec<usize> = (0..j)
            .filter(|&fi| actives[l].contains(&(1usize << fi)))
            .map(|fi| family_cols[fi])
            .collect();
        ring_specs.push((&pt[..], cols));
    }
    if dis.d1.is_some() {
        ring_specs.push((
            &dis.rho,
            dis.side_list
                .iter()
                .filter_map(|&sd| match sd {
                    RlcSide::Col(fi) => Some(family_cols[fi]),
                    RlcSide::And(_) => None,
                })
                .collect(),
        ));
    }
    if dis.d2.is_some() {
        ring_specs.push((
            &dis.rho2,
            dis.omega2_fis.iter().map(|&fi| family_cols[fi]).collect(),
        ));
    }
    let p0 = xor_support_prefix(layout);
    let eq_ns_all: Vec<Vec<Gf>> = ring_specs
        .iter()
        .map(|(pt, _)| build_eq_x_r_vec(&pt[p0..], &()).expect("x support"))
        .collect();
    let mut rings = Vec::new();
    for ((_, spec_cols), eq_ns) in ring_specs.iter().zip(eq_ns_all.iter()) {
        let cols_owned: Vec<usize> = spec_cols.clone();
        let svs: Vec<Vec<Gf>> = cfg_into_iter!(cols_owned)
            .map(|i_col| {
                let mut s = vec![Gf::zero(); 128];
                for (yx, &e) in eq_ns.iter().enumerate() {
                    let y = embed_xor_index(layout, yx << p0, i_col) >> LOG_PACKING;
                    let pe = hint.p_msg[y];
                    for wi in 0..2usize {
                        let mut bits = if wi == 0 { pe.lo } else { pe.hi };
                        while bits != 0 {
                            let t = bits.trailing_zeros() as usize;
                            s[(wi << 6) | t] += e;
                            bits &= bits.wrapping_sub(1);
                        }
                    }
                }
                s
            })
            .collect();
        for sv in svs {
            crate::ligerito::absorb_sv(transcript, &sv);
            rings.push(RingSwitchProof { s_v: sv });
        }
    }
    let ring_data: Vec<(Vec<Gf>, Vec<usize>)> = eq_ns_all
        .into_iter()
        .zip(ring_specs.into_iter().map(|(_, cols)| cols))
        .collect();
    drop(_g_r);
    (
        RlcFamilyPartProof {
            mfs,
            us,
            presums,
            discharge_eqf: dis.d1,
            omegas: dis.omegas,
            discharge_eqf2: dis.d2,
            omegas2: dis.omegas2,
            rings,
        },
        ring_data,
    )
}

/// The shared closure of one-or-many family fronts: r″ + ring η's over
/// EVERY ring in front order → one combined basis + target → ONE
/// Ligerito call. Byte-identical to the pre-split single-family tail
/// when called with one front.
#[allow(clippy::arithmetic_side_effects)]
fn prove_rlc_families_closure(
    transcript: &mut (impl Transcript + Send),
    hint: &FlockCommitHint,
    layout: &ShaF2Layout,
    parts: &[(&RlcFamilyPartProof, &[(Vec<Gf>, Vec<usize>)])],
    pc: &LigProverConfig,
) -> LigeritoProof {
    let p = &layout.p;
    let p0 = xor_support_prefix(layout);
    let n_rings: usize = parts.iter().map(|(pt, _)| pt.rings.len()).sum();
    // (6) r″ + ring η's → combined basis + target → ONE Ligerito call.
    let r2: Vec<Gf> = transcript.get_field_challenges(LOG_PACKING, &());
    let eq_r2 = crate::poly::utils::build_eq_x_r_vec(&r2, &()).expect("r2");
    let etas: Vec<Gf> = transcript.get_field_challenges(n_rings, &());

    let _g_b = crate::utils::prof::scope("rlc:bcomb");
    let m_p = packed_vars(p);
    let mut b_comb = vec![F128::ZERO; 1usize << m_p];
    let mut ring_idx = 0usize;
    for (_, ring_data) in parts {
        let phi_ns_all: Vec<Vec<Gf>> = ring_data
            .iter()
            .map(|(eq_ns, _)| cfg_iter!(eq_ns).map(|&e| phi_r2(e, &eq_r2)).collect())
            .collect();
        for ((_, spec_cols), phi_ns) in ring_data.iter().zip(phi_ns_all.iter()) {
            for &i_col in spec_cols {
                let eta = etas[ring_idx];
                for (yx, &ph) in phi_ns.iter().enumerate() {
                    let y = embed_xor_index(layout, yx << p0, i_col) >> LOG_PACKING;
                    b_comb[y] += gf_to_f128(eta * ph);
                }
                ring_idx = ring_idx.wrapping_add(1);
            }
        }
    }
    let mut target = Gf::zero();
    let mut ri = 0usize;
    for (part, _) in parts {
        for ring in &part.rings {
            let s_u = crate::ligerito::transpose_bits_128(&ring.s_v);
            let beta =
                s_u.iter().zip(eq_r2.iter()).fold(Gf::zero(), |a, (su, e)| a + *su * *e);
            target += etas[ri] * beta;
            ri = ri.wrapping_add(1);
        }
    }
    drop(_g_b);

    let _g_l = crate::utils::prof::scope("rlc:lig");
    ligerito::recursive_prover_with_basis(
        pc,
        hint.p_msg.clone(),
        b_comb,
        gf_to_f128(target),
        &hint.prover_data.codeword,
        &hint.prover_data.merkle_tree,
        &mut ZincChallenger(transcript),
    )
}

/// Verify an RLC claim family (EXPERIMENTAL). `col_weights[c] = eq(c,
/// r_cols) ∈ 𝔽_q` — the SHARED clear-axis weights (the family requires one
/// column point). Checks, per chunk: the free range bound on every fold,
/// the merged forest against the recomputed roots `α^{u}`, the
/// (2^j−1)-channel presum against `e_d + 1` with the per-channel
/// `R̂_S(r*)` from the O(2^j·2^{t'}) case-power tables; then the η-batched
/// discharge against the |S| ≥ 2 residuals with the sent ω closings; each
/// ring against its residual; the ONE succinct Ligerito call; and the
/// recombination `Σ_c e_c·Σ_l 2^{c_w·l}·u^{(l)}_c mod q = Σ_i γ_i·c_i`.
#[allow(clippy::arithmetic_side_effects)]
#[allow(clippy::too_many_arguments)]
pub fn verify_mle_eval_mod_q_ligerito_rlc_family(
    transcript: &mut (impl Transcript + Send),
    commitment: &Commitment,
    proof: &IntEvalRsLigRlcFamilyProof,
    layout: &ShaF2Layout,
    family_cols: &[usize],
    claims: &[RlcFamilyClaim<'_>],
    col_weights: &[crate::pcs::Fq],
    alpha: Gf,
    vc: &LigVerifierConfig,
) -> Result<(), FlockRsError> {
    use crate::pcs::{
        FQ_BITS, FQ_MOD, Fq, fq_challenge, mod_q_chunk_width, mod_q_num_chunks,
        rlc_case_weights, rlc_chunk_case_weights, virtual_xor_params,
    };

    let p = &layout.p;
    let j = family_cols.len();
    let k = claims.len();
    let p_x = virtual_xor_params(layout);
    let t_x = row_bit_vars(&p_x);
    // Statement shape.
    if p.word_bits != 1
        || !(1..=4).contains(&j)
        || family_cols.iter().any(|&i| i >= layout.num_cols)
        || k == 0
        || t_x < 6
        || col_weights.len() != p_x.cols()
        || claims.iter().any(|cl| {
            cl.form == 0
                || cl.form >= (1usize << j)
                || cl.row_weights_q.len() != p_x.rows()
                || cl.claimed >= FQ_MOD
        })
        || (0..j).any(|fi| claims.iter().all(|cl| (cl.form >> fi) & 1 == 0))
    {
        return Err(FlockRsError::RingSwitch(RsOpenError::Shape));
    }
    let c_w_x = mod_q_chunk_width(&p_x);
    let lch_x = mod_q_num_chunks(&p_x, FQ_BITS);
    // Proof shape (the elision-dependent counts — rings, discharge
    // presence, ω's — are checked after the per-chunk active sets are
    // derived from the case weights).
    if proof.mfs.len() != lch_x
        || proof.us.len() != lch_x
        || proof.presums.len() != lch_x
        || proof.us.iter().any(|u| u.len() != p_x.cols())
    {
        return Err(FlockRsError::RingSwitch(RsOpenError::Shape));
    }

    // (1)–(2) Statement → γ's (mirrors the prover byte for byte).
    let forms: Vec<usize> = claims.iter().map(|cl| cl.form).collect();
    let claim_cs: Vec<u128> = claims.iter().map(|cl| cl.claimed).collect();
    let w_refs: Vec<&[u128]> = claims.iter().map(|cl| cl.row_weights_q).collect();
    absorb_rlc_family_statement(
        transcript,
        &commitment.root,
        layout,
        family_cols,
        &forms,
        &claim_cs,
        &w_refs,
    );
    let gammas: Vec<u128> = (0..k).map(|_| fq_challenge(transcript)).collect();
    let case_w = rlc_case_weights(&w_refs, &gammas, &forms, j);
    let case_chunks = rlc_chunk_case_weights(&case_w, c_w_x, lch_x);
    drop(case_w);
    let t_target = claims
        .iter()
        .zip(gammas.iter())
        .fold(Fq::from(0u128), |a, (cl, &g)| a + Fq::from(g) * Fq::from(cl.claimed));
    verify_rlc_family_core(
        transcript, commitment, proof, layout, family_cols, &case_chunks, col_weights, t_target,
        alpha, vc,
    )
}

/// Verify a SHARED-POINT RLC claim family (EXPERIMENTAL) — the verify side
/// of [`prove_mle_eval_mod_q_ligerito_rlc_family_shared_point`]: ONE
/// shared `row_weights_q`, claims as canonical (form, value) pairs.
/// Duplicate forms are deduped exactly as on the prover (first occurrence
/// wins) and a repeated form with a DIFFERENT claimed value rejects — the
/// statement would otherwise silently drop a claim the caller believes is
/// being verified. The absorb is the collapsed 0x41 statement; the case
/// weights are the rank-1 build; everything downstream is the general
/// family core.
#[allow(clippy::arithmetic_side_effects)]
#[allow(clippy::too_many_arguments)]
pub fn verify_mle_eval_mod_q_ligerito_rlc_family_shared_point(
    transcript: &mut (impl Transcript + Send),
    commitment: &Commitment,
    proof: &IntEvalRsLigRlcFamilyProof,
    layout: &ShaF2Layout,
    family_cols: &[usize],
    row_weights_q: &[u128],
    claims: &[RlcSharedClaim],
    col_weights: &[crate::pcs::Fq],
    alpha: Gf,
    vc: &LigVerifierConfig,
) -> Result<(), FlockRsError> {
    use crate::pcs::{
        FQ_BITS, FQ_MOD, Fq, fq_challenge, mod_q_chunk_width, mod_q_num_chunks,
        rlc_case_weights_shared_point, rlc_chunk_case_weights, rlc_gamma_cases,
        virtual_xor_params,
    };

    let p = &layout.p;
    let j = family_cols.len();
    let p_x = virtual_xor_params(layout);
    let t_x = row_bit_vars(&p_x);
    // Statement shape (incl. the canonical dedupe).
    if p.word_bits != 1
        || !(1..=4).contains(&j)
        || family_cols.iter().any(|&i| i >= layout.num_cols)
        || claims.is_empty()
        || t_x < 6
        || col_weights.len() != p_x.cols()
        || row_weights_q.len() != p_x.rows()
        || claims
            .iter()
            .any(|cl| cl.form == 0 || cl.form >= (1usize << j) || cl.claimed >= FQ_MOD)
    {
        return Err(FlockRsError::RingSwitch(RsOpenError::Shape));
    }
    let (forms, claim_cs) = rlc_shared_point_canonical(claims)
        .map_err(|_| FlockRsError::RingSwitch(RsOpenError::Shape))?;
    if (0..j).any(|fi| forms.iter().all(|f| (f >> fi) & 1 == 0)) {
        return Err(FlockRsError::RingSwitch(RsOpenError::Shape));
    }
    let c_w_x = mod_q_chunk_width(&p_x);
    let lch_x = mod_q_num_chunks(&p_x, FQ_BITS);
    // Proof shape (elision-dependent counts checked in the core).
    if proof.mfs.len() != lch_x
        || proof.us.len() != lch_x
        || proof.presums.len() != lch_x
        || proof.us.iter().any(|u| u.len() != p_x.cols())
    {
        return Err(FlockRsError::RingSwitch(RsOpenError::Shape));
    }

    // (1)–(2) Collapsed statement → γ's → rank-1 case weights (mirrors the
    // prover byte for byte).
    absorb_rlc_shared_point_statement(
        transcript,
        &commitment.root,
        layout,
        family_cols,
        &forms,
        &claim_cs,
        row_weights_q,
    );
    let gammas: Vec<u128> = (0..forms.len()).map(|_| fq_challenge(transcript)).collect();
    let case_w =
        rlc_case_weights_shared_point(row_weights_q, &rlc_gamma_cases(&gammas, &forms, j));
    let case_chunks = rlc_chunk_case_weights(&case_w, c_w_x, lch_x);
    drop(case_w);
    let t_target = claim_cs
        .iter()
        .zip(gammas.iter())
        .fold(Fq::from(0u128), |a, (&c, &g)| a + Fq::from(g) * Fq::from(c));
    verify_rlc_family_core(
        transcript, commitment, proof, layout, family_cols, &case_chunks, col_weights, t_target,
        alpha, vc,
    )
}

/// The family verifier body shared by the general and shared-point
/// entries: everything downstream of the γ draw — chunked case weights
/// and the combined target `T = Σ_i γ_i·c_i` in, accept/reject out.
/// Byte-identical to the pre-split general verifier from this point on.
#[allow(clippy::arithmetic_side_effects)]
#[allow(clippy::too_many_arguments)]
fn verify_rlc_family_core(
    transcript: &mut (impl Transcript + Send),
    commitment: &Commitment,
    proof: &IntEvalRsLigRlcFamilyProof,
    layout: &ShaF2Layout,
    family_cols: &[usize],
    case_chunks: &[Vec<Vec<u128>>],
    col_weights: &[crate::pcs::Fq],
    t_target: crate::pcs::Fq,
    alpha: Gf,
    vc: &LigVerifierConfig,
) -> Result<(), FlockRsError> {
    let part = RlcPartView {
        mfs: &proof.mfs,
        us: &proof.us,
        presums: &proof.presums,
        discharge_eqf: &proof.discharge_eqf,
        omegas: &proof.omegas,
        discharge_eqf2: &proof.discharge_eqf2,
        omegas2: &proof.omegas2,
        rings: &proof.rings,
    };
    let r_his =
        verify_rlc_family_front(transcript, &part, layout, family_cols, case_chunks, alpha)?;
    verify_rlc_families_closure(
        transcript,
        commitment,
        &proof.lig,
        layout,
        &[(&proof.rings, &r_his)],
        vc,
    )?;
    use crate::pcs::{Fq, mod_q_chunk_width, recombine_read_off, virtual_xor_params};
    let p_x = virtual_xor_params(layout);
    let c_w_x = mod_q_chunk_width(&p_x);
    let lch_x = case_chunks.len();
    // (7) Read-off: Σ_c e_c·Σ_l 2^{c_w·l}·u^{(l)}_c mod q = T = Σ_i γ_i·c_i.
    let us_flat: Vec<u128> = proof.us.iter().flat_map(|u| u.iter().copied()).collect();
    let y: Fq = recombine_read_off(&p_x, &us_flat, 0, col_weights, c_w_x, lch_x);
    if y != t_target {
        return Err(FlockRsError::Common(IntEvalRsError::ReadOff));
    }
    Ok(())
}

/// Borrowed view of one family's part (the single proof and the merged
/// parts share the front verifier).
struct RlcPartView<'a> {
    mfs: &'a [MergedForestProof],
    us: &'a [Vec<u128>],
    presums: &'a [MultiDegreeSumcheckProof<Gf>],
    discharge_eqf: &'a Option<RlcDischargeEqf>,
    omegas: &'a [Gf],
    discharge_eqf2: &'a Option<RlcDischargeEqf>,
    omegas2: &'a [Gf],
    rings: &'a [RingSwitchProof],
}

/// One family's FRONT verifier: forests against recomputed roots,
/// presums, the discharge cascade, and every ring residual — all absorbs
/// mirrored, no closure. Returns the rings' embedded high-points for the
/// shared basis.
#[allow(clippy::arithmetic_side_effects)]
fn verify_rlc_family_front(
    transcript: &mut (impl Transcript + Send),
    part: &RlcPartView<'_>,
    layout: &ShaF2Layout,
    family_cols: &[usize],
    case_chunks: &[Vec<Vec<u128>>],
    alpha: Gf,
) -> Result<Vec<Vec<Gf>>, FlockRsError> {

    use crate::merged_forest::verify_merged_forest;
    use crate::pcs::{
        FixedBasePow, is_generator, mod_q_chunk_width, rlc_case_pow_table,
        rlc_tau_tables, virtual_xor_params,
    };
    use crate::piop::sumcheck::multi_degree::MultiDegreeSumcheck;
    use crate::poly::utils::build_eq_x_r_vec;

    let j = family_cols.len();
    let p_x = virtual_xor_params(layout);
    let t_x = row_bit_vars(&p_x);
    let c_w_x = mod_q_chunk_width(&p_x);
    let lch_x = case_chunks.len();

    if !is_generator(alpha) {
        return Err(FlockRsError::Common(IntEvalRsError::ChallengeNotGenerator));
    }
    // Per-chunk range bound 2^{c_w+t'+W} (= 2^127 by construction).
    let range_shift = c_w_x.wrapping_add(p_x.t).wrapping_add(p_x.word_bits);
    let bound = 1u128 << range_shift;

    // (3) Per chunk: forest against the recomputed roots + presum; extract
    // the per-channel residuals via R̂_S(r*).
    let one = Gf::one();
    let comb = FixedBasePow::new(alpha, 128, 8);
    let mut points: Vec<Vec<Gf>> = Vec::with_capacity(lch_x);
    let mut mus_s1: Vec<Vec<Option<Gf>>> = Vec::with_capacity(lch_x); // [l][family col]
    let mut mus_ge2: Vec<Vec<Gf>> = Vec::with_capacity(lch_x); // [l][active-ge2 idx]
    let mut actives: Vec<Vec<usize>> = Vec::with_capacity(lch_x);
    for (l, chunk_w) in case_chunks.iter().enumerate() {
        for (c, &u) in part.us[l].iter().enumerate() {
            if u >= bound {
                return Err(FlockRsError::ChunkRange { chunk: l, col: c });
            }
        }
        let roots: Vec<Gf> = {
            let _g = crate::utils::prof::scope("rlcv:roots");
            part.us[l].iter().map(|&u| comb.pow(u)).collect()
        };
        let (z, e_d) = verify_merged_forest(transcript, &roots, &part.mfs[l], t_x, p_x.s)
            .map_err(|_| FlockRsError::Common(IntEvalRsError::Forest))?;
        let subclaims =
            MultiDegreeSumcheck::<Gf>::verify_as_subprotocol(transcript, t_x, &part.presums[l], &())
                .map_err(|_| FlockRsError::Common(IntEvalRsError::PreSumcheck))?;
        // The O(2^j·2^{t'}) step: case powers → τ_S → the ACTIVE channels
        // (zero channels are elided on both sides) → R̂_S(r*).
        let case_pow = {
            let _g = crate::utils::prof::scope("rlcv:pows");
            rlc_case_pow_table(chunk_w, alpha)
        };
        let taus = rlc_tau_tables(&case_pow);
        let active = rlc_active_channels(&taus);
        let sums = part.presums[l].claimed_sums();
        if active.is_empty() || sums.len() != active.len() {
            return Err(FlockRsError::RingSwitch(RsOpenError::Shape));
        }
        if sums.iter().fold(Gf::zero(), |a, &b| a + b) != e_d + one {
            return Err(FlockRsError::Common(IntEvalRsError::PreSumcheck));
        }
        let (z_bj, z_c) = z.split_at(t_x);
        let r_star = subclaims.point().to_vec();
        let eq_zbj = build_eq_x_r_vec(z_bj, &()).expect("t' >= 1");
        let eq_rstar = build_eq_x_r_vec(&r_star, &()).expect("t' >= 1");
        let eq_prod: Vec<Gf> =
            eq_zbj.iter().zip(eq_rstar.iter()).map(|(&a, &b)| a * b).collect();
        let mut mu_s1 = vec![None; j];
        let mut mu_ge2 = Vec::with_capacity(active.len());
        for (g, &s) in active.iter().enumerate() {
            let r_hat = eq_prod
                .iter()
                .zip(taus[s].iter())
                .fold(Gf::zero(), |acc, (&e, &t)| acc + e * t);
            if r_hat.is_zero() {
                return Err(FlockRsError::Common(IntEvalRsError::RHatZero));
            }
            let mu = subclaims.expected_evaluations()[g] * r_hat.inverse();
            if s.count_ones() == 1 {
                mu_s1[s.trailing_zeros() as usize] = Some(mu);
            } else {
                mu_ge2.push(mu);
            }
        }
        mus_s1.push(mu_s1);
        mus_ge2.push(mu_ge2);
        actives.push(active);
        points.push(r_star.iter().chain(z_c.iter()).copied().collect());
    }

    // Elision-derived proof shape: level-1 (chunk, channel) pairs, the
    // canonical side list, the level-2 AND set, ring counts.
    let l1_pairs: Vec<(usize, usize)> = {
        let mut v = Vec::new();
        for (l, act) in actives.iter().enumerate() {
            for &ch in act {
                if ch.count_ones() >= 2 {
                    v.push((l, ch));
                }
            }
        }
        v
    };
    let side_list: Vec<RlcSide> = {
        let mut v: Vec<RlcSide> = Vec::new();
        for &(_, ch) in &l1_pairs {
            let (a, b) = rlc_channel_sides(ch);
            for sd in [a, b] {
                if !v.contains(&sd) {
                    v.push(sd);
                }
            }
        }
        v.sort_unstable();
        v
    };
    let and_masks: Vec<usize> = side_list
        .iter()
        .filter_map(|&sd| match sd {
            RlcSide::And(mask) => Some(mask),
            RlcSide::Col(_) => None,
        })
        .collect();
    let omega2_fis: Vec<usize> = {
        let mut fis: Vec<usize> = and_masks
            .iter()
            .flat_map(|&mask| (0..j).filter(move |fi| (mask >> fi) & 1 == 1))
            .collect();
        fis.sort_unstable();
        fis.dedup();
        fis
    };
    let l1_ring_cols = side_list
        .iter()
        .filter(|sd| matches!(sd, RlcSide::Col(_)))
        .count();
    let singleton_rings: usize = actives
        .iter()
        .map(|a| a.iter().filter(|s| s.count_ones() == 1).count())
        .sum();
    if part.rings.len() != singleton_rings + l1_ring_cols + omega2_fis.len()
        || part.discharge_eqf.is_some() == l1_pairs.is_empty()
        || part.omegas.len() != side_list.len()
        || part.discharge_eqf2.is_some() == and_masks.is_empty()
        || part.omegas2.len() != omega2_fis.len()
    {
        return Err(FlockRsError::RingSwitch(RsOpenError::Shape));
    }

    // (4) The discharge cascade. Level 1: claimed sums are the η-weighted
    // active |S| ≥ 2 residuals; the closing uses the sent side openings.
    // Level 2: claimed sums are the η'-weighted AND openings; the closing
    // uses the sent committed openings at ρ'.
    let mut rho: Vec<Gf> = Vec::new();
    let mut rho2: Vec<Gf> = Vec::new();
    if !l1_pairs.is_empty() {
        let etas_dis: Vec<Gf> = transcript.get_field_challenges(l1_pairs.len(), &());
        let d = part.discharge_eqf.as_ref().expect("shape-checked above");
        let specs: Vec<(&[Gf], Gf)> = l1_pairs
            .iter()
            .enumerate()
            .map(|(i, &(l, _))| (&points[l][..], etas_dis[i]))
            .collect();
        let claimed: Vec<Gf> = l1_pairs
            .iter()
            .enumerate()
            .map(|(i, &(l, ch))| {
                let gi = actives[l]
                    .iter()
                    .filter(|x| x.count_ones() >= 2)
                    .position(|&x| x == ch)
                    .expect("pair derived from actives");
                etas_dis[i] * mus_ge2[l][gi]
            })
            .collect();
        let side_pos = |sd: RlcSide| -> usize {
            side_list.iter().position(|&x| x == sd).expect("side in canonical list")
        };
        let side_vals: Vec<(Gf, Gf)> = l1_pairs
            .iter()
            .map(|&(_, ch)| {
                let (a, b) = rlc_channel_sides(ch);
                (part.omegas[side_pos(a)], part.omegas[side_pos(b)])
            })
            .collect();
        rho = rlc_verify_eqf_level(transcript, t_x, p_x.s, d, &specs, &claimed, &side_vals)?;
        crate::ligerito::absorb_rlc_omegas(transcript, &part.omegas);
        if !and_masks.is_empty() {
            let etas2: Vec<Gf> = transcript.get_field_challenges(and_masks.len(), &());
            let d2 = part.discharge_eqf2.as_ref().expect("shape-checked above");
            let specs2: Vec<(&[Gf], Gf)> = and_masks
                .iter()
                .enumerate()
                .map(|(i, _)| (&rho[..], etas2[i]))
                .collect();
            let claimed2: Vec<Gf> = and_masks
                .iter()
                .enumerate()
                .map(|(i, &mask)| etas2[i] * part.omegas[side_pos(RlcSide::And(mask))])
                .collect();
            let fi_pos = |fi: usize| -> usize {
                omega2_fis.iter().position(|&x| x == fi).expect("member in omega2 set")
            };
            let side_vals2: Vec<(Gf, Gf)> = and_masks
                .iter()
                .map(|&mask| {
                    let mut bits = (0..j).filter(|fi| (mask >> fi) & 1 == 1);
                    let (a, b) = (bits.next().expect("2 bits"), bits.next().expect("2 bits"));
                    (part.omegas2[fi_pos(a)], part.omegas2[fi_pos(b)])
                })
                .collect();
            rho2 = rlc_verify_eqf_level(
                transcript, t_x, p_x.s, d2, &specs2, &claimed2, &side_vals2,
            )?;
            crate::ligerito::absorb_rlc_omegas(transcript, &part.omegas2);
        }
    }

    // (5) Rings: each in-pack read-off must reproduce its residual — per
    // chunk the ACTIVE singleton channels' columns, then the level-1
    // COLUMN sides at ρ, then the level-2 exits at ρ'.
    // (embedded column, expected residual) pairs per ring point
    type RingCols = Vec<(usize, Gf)>;
    let mut ring_specs: Vec<(&[Gf], RingCols)> = Vec::with_capacity(lch_x + 2);
    for (l, pt) in points.iter().enumerate() {
        let cols: RingCols = (0..j)
            .filter(|&fi| actives[l].contains(&(1usize << fi)))
            .map(|fi| {
                (family_cols[fi], mus_s1[l][fi].expect("active singleton has a residual"))
            })
            .collect();
        ring_specs.push((&pt[..], cols));
    }
    if !l1_pairs.is_empty() {
        ring_specs.push((
            &rho,
            side_list
                .iter()
                .enumerate()
                .filter_map(|(pos, &sd)| match sd {
                    RlcSide::Col(fi) => Some((family_cols[fi], part.omegas[pos])),
                    RlcSide::And(_) => None,
                })
                .collect(),
        ));
    }
    if !and_masks.is_empty() {
        ring_specs.push((
            &rho2,
            omega2_fis
                .iter()
                .zip(part.omegas2.iter())
                .map(|(&fi, &om)| (family_cols[fi], om))
                .collect(),
        ));
    }
    let mut r_his: Vec<Vec<Gf>> = Vec::with_capacity(part.rings.len());
    let mut ring_idx = 0usize;
    for (pt, cols) in &ring_specs {
        for &(i_col, expect) in cols {
            let ring = &part.rings[ring_idx];
            if ring.s_v.len() != 128 {
                return Err(FlockRsError::RingSwitch(RsOpenError::Shape));
            }
            let pt_e = embed_xor_point(layout, pt, i_col);
            let eq_lo = build_eq_x_r_vec(&pt_e[..LOG_PACKING], &()).expect("r_lo");
            let claim =
                ring.s_v.iter().zip(eq_lo.iter()).fold(Gf::zero(), |a, (s, e)| a + *s * *e);
            if claim != expect {
                return Err(FlockRsError::RingSwitch(RsOpenError::RingSwitchClaim));
            }
            crate::ligerito::absorb_sv(transcript, &ring.s_v);
            r_his.push(pt_e[LOG_PACKING..].to_vec());
            ring_idx = ring_idx.wrapping_add(1);
        }
    }
    Ok(r_his)
}

/// The shared closure verifier: r″ + η's over every ring in front order,
/// the combined succinct basis, ONE recursive Ligerito check.
#[allow(clippy::arithmetic_side_effects)]
fn verify_rlc_families_closure(
    transcript: &mut (impl Transcript + Send),
    commitment: &Commitment,
    lig: &LigeritoProof,
    layout: &ShaF2Layout,
    parts: &[(&[RingSwitchProof], &[Vec<Gf>])],
    vc: &LigVerifierConfig,
) -> Result<(), FlockRsError> {
    use crate::poly::utils::build_eq_x_r_vec;
    let n_rings: usize = parts.iter().map(|(r, _)| r.len()).sum();
    let r2: Vec<Gf> = transcript.get_field_challenges(LOG_PACKING, &());
    let eq_r2 = build_eq_x_r_vec(&r2, &()).expect("r2");
    let etas: Vec<Gf> = transcript.get_field_challenges(n_rings, &());
    let mut target = Gf::zero();
    let mut ri = 0usize;
    for (rings, _) in parts {
        for ring in rings.iter() {
            let s_u = crate::ligerito::transpose_bits_128(&ring.s_v);
            let beta =
                s_u.iter().zip(eq_r2.iter()).fold(Gf::zero(), |a, (su, e)| a + *su * *e);
            target += etas[ri] * beta;
            ri = ri.wrapping_add(1);
        }
    }
    let m_p = packed_vars(&layout.p);
    let all_r_his: Vec<&Vec<Gf>> =
        parts.iter().flat_map(|(_, rh)| rh.iter()).collect();
    let eval_b = |ris: &[F128], yr_log_n: usize| -> Vec<F128> {
        let ris_gf: Vec<Gf> = ris.iter().map(|&f| f128_to_gf(f)).collect();
        let mut out = vec![Gf::zero(); 1usize << yr_log_n];
        for (i, r_hi) in all_r_his.iter().enumerate() {
            let blk = residual_b_evals(&ris_gf, yr_log_n, r_hi, &eq_r2);
            for (o, x) in out.iter_mut().zip(blk.iter()) {
                *o += etas[i] * *x;
            }
        }
        out.into_iter().map(gf_to_f128).collect()
    };
    let ok = ligerito::recursive_verifier_with_basis_succinct(
        vc,
        lig,
        m_p,
        gf_to_f128(target),
        &commitment.root,
        eval_b,
        &mut ZincChallenger(transcript),
    );
    if !ok {
        return Err(FlockRsError::LigeritoReject);
    }
    Ok(())
}

// ---------------------------------------------------------------------
// EXPERIMENTAL — MERGED multi-family shared-point proofs (tag 0x47).
//
// k disjoint-or-overlapping families (j ≤ 4 each), ALL claims at one
// shared (row, column) point, in ONE proof: one statement absorb, then
// per family (γ's → rank-1 case weights → the family FRONT: forests,
// presums, cascade, rings — sequential Fiat–Shamir composition), then
// ONE r″/η draw over every ring and ONE closing Ligerito call. This
// removes the per-family recursive-opening tails that dominated the
// fam-route byte cost (k families cost ONE blob instead of k) and their
// fixed prove cost. Soundness: the single-family chain per front,
// sequentially composed; the shared closure is the same η-batched basis
// argument over the concatenated ring list.
// ---------------------------------------------------------------------

/// Absorb the merged multi-family statement (domain tag 0x47):
/// root, layout, every family's columns + canonical (form, value)
/// pairs, and the ONE shared row-weight vector.
#[allow(clippy::arithmetic_side_effects)]
fn absorb_rlc_families_statement(
    transcript: &mut impl Transcript,
    root: &flock_core::merkle::Hash,
    layout: &ShaF2Layout,
    fams: &[(&[usize], Vec<usize>, Vec<u128>)],
    row_weights_q: &[u128],
) {
    let mut bytes = Vec::new();
    bytes.push(0x47u8);
    bytes.extend_from_slice(root);
    for v in [
        layout.p.t,
        layout.p.s,
        layout.p.word_bits,
        layout.num_cols,
        layout.log_cols,
        layout.bit_vars,
        layout.num_vars,
        layout.tw,
        layout.x_fold_extra,
        fams.len(),
    ] {
        bytes.extend_from_slice(&(v as u64).to_le_bytes());
    }
    bytes.extend_from_slice(&crate::pcs::FQ_MOD.to_le_bytes());
    for (cols, forms, cs) in fams {
        bytes.extend_from_slice(&(cols.len() as u64).to_le_bytes());
        for &c in cols.iter() {
            bytes.extend_from_slice(&(c as u64).to_le_bytes());
        }
        bytes.extend_from_slice(&(forms.len() as u64).to_le_bytes());
        for (&f, &c) in forms.iter().zip(cs.iter()) {
            bytes.extend_from_slice(&(f as u64).to_le_bytes());
            bytes.extend_from_slice(&c.to_le_bytes());
        }
    }
    for &x in row_weights_q {
        bytes.extend_from_slice(&x.to_le_bytes());
    }
    transcript.absorb_slice(&bytes);
}

/// Prove k merged shared-point families in ONE proof (EXPERIMENTAL; see
/// the section comment). Per family: γ's are drawn immediately before
/// its front (sequential composition); ONE closing Ligerito call.
#[allow(clippy::arithmetic_side_effects)]
pub fn prove_mle_eval_mod_q_ligerito_rlc_families_shared_point(
    transcript: &mut (impl Transcript + Send),
    hint: &FlockCommitHint,
    layout: &ShaF2Layout,
    families: &[RlcFamilySpec<'_>],
    row_weights_q: &[u128],
    alpha: Gf,
    pc: &LigProverConfig,
) -> IntEvalRsLigRlcFamiliesProof {
    use crate::pcs::{
        FQ_BITS, FQ_MOD, fq_challenge, mod_q_chunk_width, mod_q_num_chunks,
        rlc_case_weights_shared_point, rlc_chunk_case_weights, rlc_gamma_cases,
        virtual_xor_params,
    };
    let p = &layout.p;
    assert_eq!(p.word_bits, 1, "RLC-family claims assume the W=1 SHA layout");
    assert!(!families.is_empty(), "need at least one family");
    let p_x = virtual_xor_params(layout);
    let t_x = row_bit_vars(&p_x);
    assert!(t_x >= 6, "RLC-family presum needs t' ≥ 6");
    assert_eq!(row_weights_q.len(), p_x.rows(), "shared row-weight length");
    let mut canon: Vec<(&[usize], Vec<usize>, Vec<u128>)> = Vec::with_capacity(families.len());
    for fam in families {
        let j = fam.family_cols.len();
        assert!((1..=4).contains(&j), "family size j must be in [1, 4]");
        for &i in fam.family_cols {
            assert!(i < layout.num_cols, "family column {i} out of range");
        }
        assert!(!fam.claims.is_empty(), "family needs at least one claim");
        for cl in fam.claims {
            assert!(cl.form != 0 && cl.form < (1usize << j), "claim form out of range");
            assert!(cl.claimed < FQ_MOD, "claimed value must be canonical");
        }
        let (forms, cs) = rlc_shared_point_canonical(fam.claims)
            .unwrap_or_else(|i| panic!("claim {i} repeats a form with a different value"));
        for fi in 0..j {
            assert!(
                forms.iter().any(|f| (f >> fi) & 1 == 1),
                "family column index {fi} appears in no claim form"
            );
        }
        canon.push((fam.family_cols, forms, cs));
    }
    absorb_rlc_families_statement(transcript, hint.root(), layout, &canon, row_weights_q);

    let c_w_x = mod_q_chunk_width(&p_x);
    let lch_x = mod_q_num_chunks(&p_x, FQ_BITS);
    let mut parts: Vec<RlcFamilyPartProof> = Vec::with_capacity(families.len());
    let mut ring_datas: Vec<Vec<(Vec<Gf>, Vec<usize>)>> = Vec::with_capacity(families.len());
    for (cols, forms, _cs) in &canon {
        let gammas: Vec<u128> = (0..forms.len()).map(|_| fq_challenge(transcript)).collect();
        let case_chunks = {
            let _g = crate::utils::prof::scope("rlc:casew");
            let case_w = rlc_case_weights_shared_point(
                row_weights_q,
                &rlc_gamma_cases(&gammas, forms, cols.len()),
            );
            rlc_chunk_case_weights(&case_w, c_w_x, lch_x)
        };
        let (part, rd) =
            prove_rlc_family_front(transcript, hint, layout, cols, &case_chunks, alpha);
        parts.push(part);
        ring_datas.push(rd);
    }
    let part_refs: Vec<(&RlcFamilyPartProof, &[(Vec<Gf>, Vec<usize>)])> =
        parts.iter().zip(ring_datas.iter()).map(|(pt, rd)| (pt, &rd[..])).collect();
    let lig = prove_rlc_families_closure(transcript, hint, layout, &part_refs, pc);
    IntEvalRsLigRlcFamiliesProof { parts, lig }
}

/// Verify a merged multi-family proof (EXPERIMENTAL): per family the
/// front chain + the mod-q read-off against `T = Σ γᵢcᵢ`, then the ONE
/// shared closure.
#[allow(clippy::arithmetic_side_effects)]
#[allow(clippy::too_many_arguments)]
pub fn verify_mle_eval_mod_q_ligerito_rlc_families_shared_point(
    transcript: &mut (impl Transcript + Send),
    commitment: &Commitment,
    proof: &IntEvalRsLigRlcFamiliesProof,
    layout: &ShaF2Layout,
    families: &[RlcFamilySpec<'_>],
    row_weights_q: &[u128],
    col_weights: &[crate::pcs::Fq],
    alpha: Gf,
    vc: &LigVerifierConfig,
) -> Result<(), FlockRsError> {
    use crate::pcs::{
        FQ_BITS, FQ_MOD, Fq, fq_challenge, mod_q_chunk_width, mod_q_num_chunks,
        recombine_read_off, rlc_case_weights_shared_point, rlc_chunk_case_weights,
        rlc_gamma_cases, virtual_xor_params,
    };
    let p = &layout.p;
    let p_x = virtual_xor_params(layout);
    let t_x = row_bit_vars(&p_x);
    if p.word_bits != 1
        || families.is_empty()
        || proof.parts.len() != families.len()
        || t_x < 6
        || col_weights.len() != p_x.cols()
        || row_weights_q.len() != p_x.rows()
    {
        return Err(FlockRsError::RingSwitch(RsOpenError::Shape));
    }
    let c_w_x = mod_q_chunk_width(&p_x);
    let lch_x = mod_q_num_chunks(&p_x, FQ_BITS);
    let mut canon: Vec<(&[usize], Vec<usize>, Vec<u128>)> = Vec::with_capacity(families.len());
    for (fam, part) in families.iter().zip(proof.parts.iter()) {
        let j = fam.family_cols.len();
        if !(1..=4).contains(&j)
            || fam.family_cols.iter().any(|&i| i >= layout.num_cols)
            || fam.claims.is_empty()
            || fam
                .claims
                .iter()
                .any(|cl| cl.form == 0 || cl.form >= (1usize << j) || cl.claimed >= FQ_MOD)
            || part.mfs.len() != lch_x
            || part.us.len() != lch_x
            || part.presums.len() != lch_x
            || part.us.iter().any(|u| u.len() != p_x.cols())
        {
            return Err(FlockRsError::RingSwitch(RsOpenError::Shape));
        }
        let (forms, cs) = rlc_shared_point_canonical(fam.claims)
            .map_err(|_| FlockRsError::RingSwitch(RsOpenError::Shape))?;
        if (0..j).any(|fi| forms.iter().all(|f| (f >> fi) & 1 == 0)) {
            return Err(FlockRsError::RingSwitch(RsOpenError::Shape));
        }
        canon.push((fam.family_cols, forms, cs));
    }
    absorb_rlc_families_statement(transcript, &commitment.root, layout, &canon, row_weights_q);

    let mut r_his_all: Vec<Vec<Vec<Gf>>> = Vec::with_capacity(families.len());
    for ((cols, forms, cs), part) in canon.iter().zip(proof.parts.iter()) {
        let gammas: Vec<u128> = (0..forms.len()).map(|_| fq_challenge(transcript)).collect();
        let case_chunks = {
            let case_w = rlc_case_weights_shared_point(
                row_weights_q,
                &rlc_gamma_cases(&gammas, forms, cols.len()),
            );
            rlc_chunk_case_weights(&case_w, c_w_x, lch_x)
        };
        let view = RlcPartView {
            mfs: &part.mfs,
            us: &part.us,
            presums: &part.presums,
            discharge_eqf: &part.discharge_eqf,
            omegas: &part.omegas,
            discharge_eqf2: &part.discharge_eqf2,
            omegas2: &part.omegas2,
            rings: &part.rings,
        };
        let r_his =
            verify_rlc_family_front(transcript, &view, layout, cols, &case_chunks, alpha)?;
        // Per-family mod-q read-off.
        let t_target = cs
            .iter()
            .zip(gammas.iter())
            .fold(Fq::from(0u128), |a, (&c, &g)| a + Fq::from(g) * Fq::from(c));
        let us_flat: Vec<u128> = part.us.iter().flat_map(|u| u.iter().copied()).collect();
        let y: Fq = recombine_read_off(&p_x, &us_flat, 0, col_weights, c_w_x, lch_x);
        if y != t_target {
            return Err(FlockRsError::Common(IntEvalRsError::ReadOff));
        }
        r_his_all.push(r_his);
    }
    let closure_parts: Vec<(&[RingSwitchProof], &[Vec<Gf>])> = proof
        .parts
        .iter()
        .zip(r_his_all.iter())
        .map(|(pt, rh)| (&pt.rings[..], &rh[..]))
        .collect();
    verify_rlc_families_closure(transcript, commitment, &proof.lig, layout, &closure_parts, vc)
}

/// Total proof bytes of a merged multi-family proof.
#[allow(clippy::arithmetic_side_effects)]
pub fn mle_eval_mod_q_lig_rlc_families_proof_size_bytes(
    proof: &IntEvalRsLigRlcFamiliesProof,
) -> usize {
    use crate::transcript::traits::Transcribable;
    let mut total = proof.lig.size_bytes();
    for part in &proof.parts {
        let mut b = ZincSideSizeBreakdown::default();
        for l in 0..part.mfs.len() {
            b.accumulate(&zinc_side_size_breakdown_merged(
                &part.mfs[l],
                &part.us[l],
                &part.presums[l],
            ));
        }
        b.s_v = part.rings.len() * 128 * 16;
        let eqf_bytes = |d: &RlcDischargeEqf| {
            d.sc_a.get_num_bytes() + d.betas.len() * 16 + d.sc_b.get_num_bytes()
        };
        total += b.total()
            + part.discharge_eqf.as_ref().map_or(0, eqf_bytes)
            + part.discharge_eqf2.as_ref().map_or(0, eqf_bytes)
            + (part.omegas.len() + part.omegas2.len()) * 16;
    }
    total
}

/// Total proof bytes of an RLC-family proof (zinc side + flock Ligerito).
#[allow(clippy::arithmetic_side_effects)]
pub fn mle_eval_mod_q_lig_rlc_family_proof_size_bytes(
    proof: &IntEvalRsLigRlcFamilyProof,
) -> usize {
    use crate::transcript::traits::Transcribable;
    let mut b = ZincSideSizeBreakdown::default();
    for l in 0..proof.mfs.len() {
        b.accumulate(&zinc_side_size_breakdown_merged(
            &proof.mfs[l],
            &proof.us[l],
            &proof.presums[l],
        ));
    }
    // One ring per (point, family column) — override the per-forest count.
    b.s_v = proof.rings.len() * 128 * 16;
    let eqf_bytes = |d: &RlcDischargeEqf| {
        d.sc_a.get_num_bytes() + d.betas.len() * 16 + d.sc_b.get_num_bytes()
    };
    let discharge = proof.discharge_eqf.as_ref().map_or(0, eqf_bytes)
        + proof.discharge_eqf2.as_ref().map_or(0, eqf_bytes);
    b.total() + discharge + (proof.omegas.len() + proof.omegas2.len()) * 16
        + proof.lig.size_bytes()
}

/// Total proof bytes of a virtual-XOR mod-q Ligerito-opened proof.
pub fn mle_eval_mod_q_lig_xor_proof_size_bytes(proof: &IntEvalRsLigModQXorProof) -> usize {
    let (b, lig) = mle_eval_mod_q_lig_xor_size_breakdown(proof);
    b.total().saturating_add(lig)
}

// ---------------------------------------------------------------------
// Proof-size accounting
// ---------------------------------------------------------------------

/// Per-component byte breakdown of the zinc-side proof parts (everything
/// the end-to-end proof carries besides the flock `LigeritoProof`).
#[derive(Clone, Copy, Debug, Default)]
pub struct ZincSideSizeBreakdown {
    /// Forest per-tree roots (`2^s` K-elements per forest).
    pub forest_roots: usize,
    /// Forest per-layer shared sumcheck messages.
    pub forest_sumchecks: usize,
    /// Forest per-layer per-tree `(left, right)` child-eval pairs — the
    /// `2·2^s·(t+log₂W)` K-element block.
    pub forest_evals: usize,
    /// The sent integers `v` (or the chunk folds `u`).
    pub v: usize,
    /// De-black-boxing pre-sumcheck messages.
    pub presum: usize,
    /// Ring-switch `s_v` messages (128 K-elements per claim).
    pub s_v: usize,
}

impl ZincSideSizeBreakdown {
    /// Sum of every component.
    #[allow(clippy::arithmetic_side_effects)]
    pub fn total(&self) -> usize {
        self.forest_roots
            + self.forest_sumchecks
            + self.forest_evals
            + self.v
            + self.presum
            + self.s_v
    }

    /// Component-wise accumulate (for multi-chunk / multi-poly proofs).
    #[allow(clippy::arithmetic_side_effects)]
    pub fn accumulate(&mut self, o: &ZincSideSizeBreakdown) {
        self.forest_roots += o.forest_roots;
        self.forest_sumchecks += o.forest_sumchecks;
        self.forest_evals += o.forest_evals;
        self.v += o.v;
        self.presum += o.presum;
        self.s_v += o.s_v;
    }

    /// Print one line per component to stderr.
    pub fn print(&self, label: &str) {
        eprintln!("  {label} zinc-side breakdown:");
        eprintln!("    forest roots            {:>9} B", self.forest_roots);
        eprintln!("    forest sumcheck msgs    {:>9} B", self.forest_sumchecks);
        eprintln!("    forest child-eval pairs {:>9} B", self.forest_evals);
        eprintln!("    v / chunk folds         {:>9} B", self.v);
        eprintln!("    pre-sumcheck            {:>9} B", self.presum);
        eprintln!("    ring-switch s_v         {:>9} B", self.s_v);
        eprintln!("    zinc-side total         {:>9} B", self.total());
    }
}

/// Itemize the zinc-side proof components shared by every backend: forest
/// (roots + per-layer sumchecks + child evals) + `v` + pre-sumcheck + the
/// ring-switch `s_v` message (one 128-element claim per forest).
#[allow(clippy::arithmetic_side_effects)]
pub fn zinc_side_size_breakdown(
    forest: &ProductForestProof<Gf>,
    v: &[u128],
    presum: &MultiDegreeSumcheckProof<Gf>,
) -> ZincSideSizeBreakdown {
    use crate::transcript::traits::Transcribable;
    let gf = 16usize;
    let mut b = ZincSideSizeBreakdown {
        forest_roots: forest.roots.len() * gf,
        v: v.len() * gf,
        presum: presum.get_num_bytes(),
        s_v: 128 * gf,
        ..Default::default()
    };
    for layer in &forest.layers {
        if let Some(sc) = &layer.sumcheck_proof {
            b.forest_sumchecks += sc.get_num_bytes();
        }
        b.forest_evals += layer.evals.len() * 2 * gf;
    }
    b
}

/// Bytes of the zinc-side proof components ([`zinc_side_size_breakdown`]'s
/// total).
pub fn zinc_side_proof_size_bytes(
    forest: &ProductForestProof<Gf>,
    v: &[u128],
    presum: &MultiDegreeSumcheckProof<Gf>,
) -> usize {
    zinc_side_size_breakdown(forest, v, presum).total()
}

/// Itemize the zinc-side components of a MERGED-forest proof: the per-layer
/// per-tree child-eval block collapses to one `(left, right)` pair per
/// layer, the per-layer degree-3 round messages replace the shared
/// eq-factored sumchecks, and the roots cost NOTHING (derived from `v`).
#[allow(clippy::arithmetic_side_effects)]
pub fn zinc_side_size_breakdown_merged(
    mf: &MergedForestProof,
    v: &[u128],
    presum: &MultiDegreeSumcheckProof<Gf>,
) -> ZincSideSizeBreakdown {
    use crate::transcript::traits::Transcribable;
    let gf = 16usize;
    let mut b = ZincSideSizeBreakdown {
        forest_roots: 0, // recomputed by the verifier as α^{v_c}
        v: v.len() * gf,
        presum: presum.get_num_bytes(),
        s_v: 128 * gf,
        ..Default::default()
    };
    for layer in &mf.layers {
        if let Some(sc) = &layer.sc_x {
            b.forest_sumchecks += sc.get_num_bytes();
        }
        b.forest_sumchecks += layer.sc_c.get_num_bytes();
        b.forest_evals += 2 * gf; // the closing (left, right) pair
        if layer.pair2.is_some() {
            b.forest_evals += 2 * gf; // quad layers close on four values
        }
    }
    b
}

/// Total proof bytes of an end-to-end Ligerito-opened proof.
pub fn int_eval_rs_lig_proof_size_bytes(proof: &IntEvalRsLigProof) -> usize {
    zinc_side_size_breakdown_merged(&proof.mf, &proof.v, &proof.presum)
        .total()
        .saturating_add(proof.open.lig.size_bytes())
}

/// Total proof bytes of an end-to-end mod-q Ligerito-opened proof
/// (per-chunk zinc sides + the shared `LigeritoProof`).
pub fn mle_eval_mod_q_lig_proof_size_bytes(proof: &IntEvalRsLigModQProof) -> usize {
    mle_eval_mod_q_lig_size_breakdown(proof).0.total().saturating_add(proof.lig.size_bytes())
}

/// Itemized `(zinc-side summed over chunks, flock LigeritoProof)` bytes of a
/// mod-q Ligerito-opened proof.
pub fn mle_eval_mod_q_lig_size_breakdown(
    proof: &IntEvalRsLigModQProof,
) -> (ZincSideSizeBreakdown, usize) {
    let mut b = ZincSideSizeBreakdown::default();
    for l in 0..proof.mfs.len() {
        // Only the transmitted prefix of `us` is on the wire.
        let u = &proof.us[l];
        b.accumulate(&zinc_side_size_breakdown_merged(
            &proof.mfs[l],
            &u[..transmitted_us_len(u)],
            &proof.presums[l],
        ));
    }
    (b, proof.lig.size_bytes())
}

/// How many leading chunk folds go on the wire: everything up to and
/// including the last non-zero one. The trailing zeros belong to the
/// elided all-zero columns of a padded witness and are re-derived by the
/// verifier (see [`IntEvalRsLigModQProof::to_bytes`]).
pub fn transmitted_us_len(u: &[u128]) -> usize {
    u.iter().rposition(|&x| x != 0).map_or(0, |i| i.wrapping_add(1))
}

// ---------------------------------------------------------------------
// Host proof-stream (de)serialization of `IntEvalRsLigModQProof`
// ---------------------------------------------------------------------

impl IntEvalRsLigModQProof {
    /// Serialize the complete F2Z proof into the host proof stream: the
    /// zinc-side parts field by field (merged forests, chunk folds `u`,
    /// pre-sumchecks, ring-switch `s_v` messages) via [`crate::proof_codec`],
    /// then the flock [`LigeritoProof`] as a length-prefixed `bincode` 1.3
    /// blob. Mirrors [`Self::from_bytes`].
    #[allow(clippy::arithmetic_side_effects)]
    pub fn to_bytes(&self) -> Vec<u8> {
        use crate::proof_codec::Writer;
        let mut w = Writer::new();
        let lch = self.mfs.len();
        w.len(lch);
        for l in 0..lch {
            w.len(self.mfs[l].layers.len());
            for layer in &self.mfs[l].layers {
                // Flag bits: 1 = sc_x present, 2 = quad (pair2 present).
                // Arity-2 layers keep the legacy 0/1 values — their byte
                // streams are unchanged.
                let flag = layer.sc_x.is_some() as usize
                    | ((layer.pair2.is_some() as usize) << 1);
                w.len(flag);
                if let Some(sc) = &layer.sc_x {
                    w.transcribable(sc);
                }
                w.transcribable(&layer.sc_c);
                w.gf(&layer.pair.0);
                w.gf(&layer.pair.1);
                if let Some(p2) = &layer.pair2 {
                    w.gf(&p2.0);
                    w.gf(&p2.1);
                }
            }
            // Publicly-zero chunk folds are NOT transmitted. A witness of
            // N ≠ 2^n cells is zero-padded into whole trailing columns
            // (the column index is the high-order MLE index), whose folds
            // are `u_c = 0` and whose roots are `α^0 = 1`. The verifier
            // re-pads to `2^s`, so the decoded `us` is bit-for-bit the
            // prover's — this is a shorter ENCODING of the same proof
            // object, with the same soundness surface (the adversary could
            // always have sent those zeros explicitly). Canonical: the
            // written prefix never ends in a zero, and `from_bytes`
            // rejects any non-minimal encoding.
            let u_l = &self.us[l];
            let n_u = transmitted_us_len(u_l);
            w.len(n_u);
            for &u in &u_l[..n_u] {
                w.u128(u);
            }
            w.transcribable(&self.presums[l]);
            w.len(self.rings[l].s_v.len());
            for g in &self.rings[l].s_v {
                w.gf(g);
            }
        }
        let lig_bytes = bincode::serialize(&self.lig).expect("LigeritoProof bincode encode");
        w.len(lig_bytes.len());
        w.bytes(&lig_bytes);
        w.into_vec()
    }

    /// Deserialize the complete F2Z proof from the host proof stream.
    /// Mirrors [`Self::to_bytes`]. The embedded `LigeritoProof` is decoded
    /// from its length-prefixed `bincode` blob.
    #[allow(clippy::arithmetic_side_effects)]
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, crate::proof_codec::CodecError> {
        use crate::merged_forest::{MergedForestProof, MergedLayer};
        use crate::proof_codec::{CodecError, Reader};
        let mut r = Reader::new(bytes);
        let lch = r.len()?;
        let mut mfs = Vec::with_capacity(lch);
        let mut us = Vec::with_capacity(lch);
        let mut presums = Vec::with_capacity(lch);
        let mut rings = Vec::with_capacity(lch);
        for _ in 0..lch {
            let n_layers = r.len()?;
            let mut layers = Vec::with_capacity(n_layers);
            for _ in 0..n_layers {
                let flag = r.len()?;
                let sc_x = if flag & 1 == 1 {
                    Some(r.transcribable::<crate::piop::sumcheck::SumcheckProof<Gf>>()?)
                } else {
                    None
                };
                let sc_c = r.transcribable::<crate::piop::sumcheck::SumcheckProof<Gf>>()?;
                let p0 = r.gf()?;
                let p1 = r.gf()?;
                let pair2 = if flag & 2 == 2 {
                    let p2 = r.gf()?;
                    let p3 = r.gf()?;
                    Some((p2, p3))
                } else {
                    None
                };
                layers.push(MergedLayer { sc_x, sc_c, pair: (p0, p1), pair2 });
            }
            mfs.push(MergedForestProof { layers });
            let n_u = r.len()?;
            // Bound the attacker-controlled count by what is actually left
            // before reserving (a tampered prefix must cost a `Truncated`,
            // not a huge speculative allocation).
            let mut u = Vec::with_capacity(n_u.min(r.remaining() / 16));
            for _ in 0..n_u {
                u.push(r.u128()?);
            }
            // The zero tail is implied by the shape, never transmitted.
            if u.last() == Some(&0) {
                return Err(CodecError::NonCanonical);
            }
            us.push(u);
            presums.push(r.transcribable::<MultiDegreeSumcheckProof<Gf>>()?);
            let n_sv = r.len()?;
            let mut s_v = Vec::with_capacity(n_sv);
            for _ in 0..n_sv {
                s_v.push(r.gf()?);
            }
            rings.push(RingSwitchProof { s_v });
        }
        let n_bytes = r.len()?;
        let lig_bytes = r.take(n_bytes)?;
        let lig: LigeritoProof =
            bincode::deserialize(lig_bytes).map_err(|e| CodecError::Bincode(e.to_string()))?;
        Ok(IntEvalRsLigModQProof { mfs, us, presums, rings, lig })
    }
}

/// Minimal little-endian byte width of `x` (`0 → 1`; `< 2^{8w} → ≤ w`).
fn min_byte_width(x: u128) -> usize {
    ((128usize.wrapping_sub(x.leading_zeros() as usize)).div_ceil(8)).max(1)
}

impl IntEvalRsLigExtProof {
    /// Serialize the extension-field proof: the Step-1 fold vectors (each
    /// with its all-zero tail trimmed, exactly like the base proof's `us`,
    /// then packed at the vector's **minimal byte width** — honest folds
    /// are `~(t+W+q_bits)`-bit, so a width byte plus tight little-endian
    /// values beats fixed 16-byte cells by ~1/3, and sparse witnesses
    /// shrink further), then the base mod-`q'` proof as one
    /// length-prefixed blob. Canonical and tamper-rejecting like the base
    /// codec.
    #[allow(clippy::arithmetic_side_effects)]
    pub fn to_bytes(&self) -> Vec<u8> {
        use crate::proof_codec::Writer;
        let mut w = Writer::new();
        w.len(self.mus.len());
        for m in &self.mus {
            let n = transmitted_us_len(m);
            w.len(n);
            if n > 0 {
                let width = m[..n].iter().map(|&u| min_byte_width(u)).max().expect("n > 0");
                w.bytes(&[width as u8]);
                for &u in &m[..n] {
                    w.bytes(&u.to_le_bytes()[..width]);
                }
            }
        }
        let base = self.base.to_bytes();
        w.len(base.len());
        w.bytes(&base);
        w.into_vec()
    }

    /// Deserialize the extension-field proof. Mirrors [`Self::to_bytes`]:
    /// rejects a non-minimal width byte, a trailing zero fold, and any
    /// framing damage.
    #[allow(clippy::arithmetic_side_effects)]
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, crate::proof_codec::CodecError> {
        use crate::proof_codec::{CodecError, Reader};
        let mut r = Reader::new(bytes);
        let n_mus = r.len()?;
        let mut mus = Vec::with_capacity(n_mus.min(r.remaining() / 8));
        for _ in 0..n_mus {
            let n_u = r.len()?;
            let mut m: Vec<u128> = Vec::with_capacity(n_u.min(r.remaining()));
            if n_u > 0 {
                let width = usize::from(r.take(1)?[0]);
                if !(1..=16).contains(&width) {
                    return Err(CodecError::NonCanonical);
                }
                let mut max_seen = 0u128;
                for _ in 0..n_u {
                    let b = r.take(width)?;
                    let mut buf = [0u8; 16];
                    buf[..width].copy_from_slice(b);
                    let v = u128::from_le_bytes(buf);
                    max_seen = max_seen.max(v);
                    m.push(v);
                }
                // Canonical: the width must be necessary for the largest
                // value, and the zero tail is implied, never transmitted.
                if min_byte_width(max_seen) != width || m.last() == Some(&0) {
                    return Err(CodecError::NonCanonical);
                }
            }
            mus.push(m);
        }
        let n_bytes = r.len()?;
        let base_bytes = r.take(n_bytes)?;
        let base = IntEvalRsLigModQProof::from_bytes(base_bytes)?;
        Ok(IntEvalRsLigExtProof { mus, base })
    }
}

/// Prove an integer-MLE evaluation with the flock-backed opening. The
/// forest GKR, `v` message, and pre-sumcheck are shared with the zinc
/// backend ([`prove_int_eval_common`]).
pub fn prove_rs_flock(
    transcript: &mut (impl Transcript + Send),
    hint: &FlockCommitHint,
    p: &IntEvalParams,
    data: &[u128],
    row_weights: &[u128],
    alpha: Gf,
) -> IntEvalRsFlockProof {
    let (forest, v, presum, point) =
        prove_int_eval_common(transcript, p, data, row_weights, alpha, &hint.rows, Some(&hint.packed_cols));
    let open = prove_rs_open_flock(transcript, hint, &point);
    IntEvalRsFlockProof { forest, v, presum, open }
}

/// Verify an integer-MLE evaluation with the flock-backed opening.
#[allow(clippy::too_many_arguments)] // mirrors `f2_int_eval::verify`'s surface
pub fn verify_rs_flock<R>(
    transcript: &mut (impl Transcript + Send),
    commitment: &Commitment,
    proof: &IntEvalRsFlockProof,
    p: &IntEvalParams,
    row_weights: &[u128],
    col_weights: &[R],
    g_r: R,
    alpha: Gf,
    claimed_eval: R,
) -> Result<(), FlockRsError>
where
    R: Copy + PartialEq + From<u128> + core::ops::Add<Output = R> + core::ops::Mul<Output = R>,
{
    let (point, mu) = verify_int_eval_common(
        transcript,
        &proof.forest,
        &proof.v,
        &proof.presum,
        p,
        row_weights,
        alpha,
    )
    .map_err(FlockRsError::Common)?;

    verify_rs_open_flock(transcript, commitment, mu, &point, &proof.open)?;

    let computed = final_eval_ring(&proof.v, col_weights, g_r);
    if computed != claimed_eval {
        return Err(FlockRsError::Common(IntEvalRsError::ReadOff));
    }
    Ok(())
}

// ---------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::arithmetic_side_effects)]
mod tests {
    use super::*;
    use crate::pcs::smallest_generator;
    use crate::transcript::Blake3Transcript;

    fn sample(seed: u64) -> Gf {
        let hi = seed.wrapping_mul(0x9E37_79B9_7F4A_7C15).rotate_left(29) ^ 0x1234_5678_9ABC_DEF0;
        Gf::from_words([seed ^ 0xA5A5_5A5A_0F0F_F0F0, hi])
    }

    /// Serializes the test that MUTATES the process-global `F2Z_QUAD` env
    /// var against quad-eligible (row_len ≥ 256) prove/verify pairs that
    /// must see a stable value across their whole run.
    static QUAD_ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    /// Field bridging is the identity on words, and multiplication agrees —
    /// the two `GF(2^128)` implementations are the same field in the same
    /// representation.
    #[test]
    fn field_bridge_is_bit_identical() {
        for i in 0..64u64 {
            let a = sample(0x77 + i);
            let b = sample(0x1000 + i);
            let fa = gf_to_f128(a);
            let fb = gf_to_f128(b);
            assert_eq!(f128_to_gf(fa), a);
            assert_eq!(f128_to_gf(fa * fb), a * b, "mul mismatch at {i}");
            assert_eq!(f128_to_gf(fa + fb), a + b, "add mismatch at {i}");
        }
        assert_eq!(LOG_PACKING, flock_core::pcs::pack::LOG_PACKING);
    }

    /// Mod-q MLE evaluation through the Ligerito opener: 1-chunk (W=1) and
    /// 2-chunk (W=32) regimes, with a local 𝔽_q (q = 2^100 − 15).
    #[test]
    fn mle_eval_mod_q_ligerito_roundtrips() {
        use crate::pcs::{mod_q_chunk_width, mod_q_num_chunks};
        let _env = QUAD_ENV_LOCK.lock().unwrap_or_else(|p| p.into_inner());
        const Q: u128 = (1u128 << 100) - 15;
        #[derive(Clone, Copy, PartialEq, Debug)]
        struct Fq(u128);
        impl From<u128> for Fq {
            fn from(v: u128) -> Self {
                Fq(v % Q)
            }
        }
        impl core::ops::Add for Fq {
            type Output = Fq;
            fn add(self, o: Fq) -> Fq {
                let s = self.0 + o.0; // both < Q < 2^100: no overflow
                Fq(if s >= Q { s - Q } else { s })
            }
        }
        impl core::ops::Mul for Fq {
            type Output = Fq;
            fn mul(self, o: Fq) -> Fq {
                // Russian-peasant: doubles stay < 2^101.
                let (mut a, mut b, mut acc) = (self.0, o.0, 0u128);
                while b != 0 {
                    if b & 1 == 1 {
                        let s = acc + a;
                        acc = if s >= Q { s - Q } else { s };
                    }
                    let d = a << 1;
                    a = if d >= Q { d - Q } else { d };
                    b >>= 1;
                }
                Fq(acc)
            }
        }

        let alpha = smallest_generator();
        let q_bits = 100usize;
        for (t, s_vars, w) in [(10usize, 5usize, 1usize), (4, 8, 32)] {
            let p = IntEvalParams { t, s: s_vars, word_bits: w };
            let m_p = packed_vars(&p);
            let lch = mod_q_num_chunks(&p, q_bits);
            let c_w = mod_q_chunk_width(&p);
            let (pc, vc) = lig_configs(m_p, LigConfig::Adhoc { log_batch: 2, log_inv_rate: 2 })
                .expect("cfg");

            let mask = if w == 128 { u128::MAX } else { (1u128 << w) - 1 };
            let data: Vec<u128> = (0..p.cells())
                .map(|i| (i as u128).wrapping_mul(0x9E37_79B9_7F4A_7C15) & mask)
                .collect();
            // ~100-bit row weights in [0, q).
            let rw_q: Vec<u128> = (0..p.rows())
                .map(|b| {
                    (b as u128)
                        .wrapping_mul(0xDEAD_BEEF_CAFE_F00D_1234_5678_9ABC_DEF1)
                        .wrapping_add(7)
                        % Q
                })
                .collect();
            let cw_small: Vec<u128> =
                (0..p.cols()).map(|c| ((c as u128).wrapping_mul(5) & 7).wrapping_add(1)).collect();
            let col_w: Vec<Fq> = cw_small.iter().map(|&x| Fq::from(x)).collect();
            // Expected y in F_q, computed directly.
            let mut y = Fq::from(0u128);
            for c in 0..p.cols() {
                let mut vc = Fq::from(0u128);
                for b in 0..p.rows() {
                    vc = vc + Fq::from(rw_q[b]) * Fq::from(data[p.cell_index(b, c)]);
                }
                y = y + col_w[c] * vc;
            }

            let hint = commit_rs_flock_with(&p, &data, pc.log_inv_rates[0], pc.initial_k);
            let mut pt = Blake3Transcript::new();
            let proof =
                prove_mle_eval_mod_q_ligerito(&mut pt, &hint, &p, &rw_q, q_bits, alpha, &pc);
            assert_eq!(proof.us.len(), lch, "chunk count (c_w={c_w})");

            let mut vt = Blake3Transcript::new();
            verify_mle_eval_mod_q_ligerito(
                &mut vt, &hint.commitment, &proof, &p, &rw_q, &col_w, alpha, y, q_bits, &vc,
            )
            .unwrap_or_else(|e| panic!("mod-q (t={t},W={w},L={lch}) failed: {e:?}"));

            // Wrong claim.
            let mut vt = Blake3Transcript::new();
            assert_eq!(
                verify_mle_eval_mod_q_ligerito(
                    &mut vt, &hint.commitment, &proof, &p, &rw_q, &col_w, alpha,
                    y + Fq::from(1u128), q_bits, &vc,
                ),
                Err(FlockRsError::Common(IntEvalRsError::ReadOff)),
            );

            // Out-of-range chunk fold.
            let mut bad = IntEvalRsLigModQProof {
                mfs: proof.mfs.clone(),
                us: proof.us.clone(),
                presums: proof.presums.clone(),
                rings: proof.rings.clone(),
                lig: proof.lig.clone(),
            };
            bad.us[0][0] = u128::MAX - 1;
            let mut vt = Blake3Transcript::new();
            assert!(matches!(
                verify_mle_eval_mod_q_ligerito(
                    &mut vt, &hint.commitment, &bad, &p, &rw_q, &col_w, alpha, y, q_bits, &vc,
                ),
                Err(FlockRsError::ChunkRange { .. })
            ));

            // Non-generator α is rejected before any proof processing.
            let mut vt = Blake3Transcript::new();
            assert_eq!(
                verify_mle_eval_mod_q_ligerito(
                    &mut vt, &hint.commitment, &proof, &p, &rw_q, &col_w, Gf::one(), y, q_bits, &vc,
                ),
                Err(FlockRsError::Common(IntEvalRsError::ChallengeNotGenerator)),
            );

            // QUAD forest (`F2Z_QUAD=1` — arity-4 region layers, K
            // challenges, its own transcript shape; both test shapes have
            // row_len ≥ 256, and (4, 8, 32) exercises the odd-depth
            // parity bridge): roundtrip, wrong-claim rejection, and the
            // codec round-trips the quad layers (pair2 flag).
            unsafe { std::env::set_var("F2Z_QUAD", "1") };
            let mut pt = Blake3Transcript::new();
            let proof_q =
                prove_mle_eval_mod_q_ligerito(&mut pt, &hint, &p, &rw_q, q_bits, alpha, &pc);
            let mut vt = Blake3Transcript::new();
            verify_mle_eval_mod_q_ligerito(
                &mut vt, &hint.commitment, &proof_q, &p, &rw_q, &col_w, alpha, y, q_bits, &vc,
            )
            .unwrap_or_else(|e| panic!("quad mod-q (t={t},W={w}) failed: {e:?}"));
            let mut vt = Blake3Transcript::new();
            assert_eq!(
                verify_mle_eval_mod_q_ligerito(
                    &mut vt, &hint.commitment, &proof_q, &p, &rw_q, &col_w, alpha,
                    y + Fq::from(1u128), q_bits, &vc,
                ),
                Err(FlockRsError::Common(IntEvalRsError::ReadOff)),
                "quad wrong claim must be rejected (t={t},W={w})"
            );
            let rt = IntEvalRsLigModQProof::from_bytes(&proof_q.to_bytes())
                .expect("quad proof codec roundtrip");
            let mut vt = Blake3Transcript::new();
            verify_mle_eval_mod_q_ligerito(
                &mut vt, &hint.commitment, &rt, &p, &rw_q, &col_w, alpha, y, q_bits, &vc,
            )
            .expect("decoded quad proof verifies");
            // The restructured degree-5 bodies (w-prefold + Karatsuba-3
            // cross stage + folded node conversion) are value-exact
            // re-associations: the proof stream must be byte-identical
            // to the naive bodies'.
            unsafe { std::env::set_var("F2Z_QUAD_KERNEL", "0") };
            let mut pt = Blake3Transcript::new();
            let proof_q_naive =
                prove_mle_eval_mod_q_ligerito(&mut pt, &hint, &p, &rw_q, q_bits, alpha, &pc);
            unsafe { std::env::remove_var("F2Z_QUAD_KERNEL") };
            assert_eq!(
                proof_q.to_bytes(),
                proof_q_naive.to_bytes(),
                "quad kernel bodies must be transcript-identical (t={t},W={w})"
            );
            // BOTTOM MERGE (`F2Z_QUAD=2` — the pair and leaf layers as
            // ONE arity-4 bit-driven layer, `prove_quad_bottom_sumcheck`):
            // roundtrip, wrong-claim rejection, codec, and the v1/v2
            // plans are mutually incompatible (layer counts differ).
            unsafe { std::env::set_var("F2Z_QUAD", "2") };
            let mut pt = Blake3Transcript::new();
            let proof_q2 =
                prove_mle_eval_mod_q_ligerito(&mut pt, &hint, &p, &rw_q, q_bits, alpha, &pc);
            let mut vt = Blake3Transcript::new();
            verify_mle_eval_mod_q_ligerito(
                &mut vt, &hint.commitment, &proof_q2, &p, &rw_q, &col_w, alpha, y, q_bits,
                &vc,
            )
            .unwrap_or_else(|e| panic!("quad-v2 mod-q (t={t},W={w}) failed: {e:?}"));
            let mut vt = Blake3Transcript::new();
            assert_eq!(
                verify_mle_eval_mod_q_ligerito(
                    &mut vt, &hint.commitment, &proof_q2, &p, &rw_q, &col_w, alpha,
                    y + Fq::from(1u128), q_bits, &vc,
                ),
                Err(FlockRsError::Common(IntEvalRsError::ReadOff)),
                "quad-v2 wrong claim must be rejected (t={t},W={w})"
            );
            let rt2 = IntEvalRsLigModQProof::from_bytes(&proof_q2.to_bytes())
                .expect("quad-v2 proof codec roundtrip");
            let mut vt = Blake3Transcript::new();
            verify_mle_eval_mod_q_ligerito(
                &mut vt, &hint.commitment, &rt2, &p, &rw_q, &col_w, alpha, y, q_bits, &vc,
            )
            .expect("decoded quad-v2 proof verifies");
            // A v1 proof must not pass under the v2 plan.
            let mut vt = Blake3Transcript::new();
            assert!(
                verify_mle_eval_mod_q_ligerito(
                    &mut vt, &hint.commitment, &proof_q, &p, &rw_q, &col_w, alpha, y, q_bits,
                    &vc,
                )
                .is_err(),
                "a v1 quad proof must be rejected under the v2 plan (t={t},W={w})"
            );
            unsafe { std::env::remove_var("F2Z_QUAD") };
            // A quad proof must NOT pass the arity-2 dispatch (different
            // transcript shape — the quad layers' pair2 rejects).
            let mut vt = Blake3Transcript::new();
            assert!(
                verify_mle_eval_mod_q_ligerito(
                    &mut vt, &hint.commitment, &proof_q, &p, &rw_q, &col_w, alpha, y, q_bits,
                    &vc,
                )
                .is_err(),
                "quad proof must be rejected by the arity-2 verifier (t={t},W={w})"
            );
        }
    }


    /// Extension-field evaluation (paper `c:core_iop` Steps 1–3) over
    /// `K = Goldilocks[X]/(X² − 7)`: honest roundtrip at three shapes
    /// (covering L₁ = 1/2 Step-1 chunking and L₂ = 1/2 projected chunking),
    /// wrong-claim rejection, Step-1 tamper rejection, shape/range
    /// rejection, and the codec roundtrip.
    #[test]
    fn mle_eval_ext_ligerito_roundtrips() {
        use crate::ext_proj::ExtProjParams;
        use crate::pcs::{mod_q_chunk_width, mod_q_num_chunks};
        let _env = QUAD_ENV_LOCK.lock().unwrap_or_else(|p| p.into_inner());
        // Goldilocks p = 2^64 − 2^32 + 1; K = F_p[X]/(X² − 7) (the Plonky2
        // quadratic extension — 7 is a non-residue mod p).
        const P: u128 = 0xFFFF_FFFF_0000_0001;
        #[derive(Clone, Copy, PartialEq, Debug)]
        struct Fp2 {
            c0: u128,
            c1: u128,
        }
        impl From<u128> for Fp2 {
            fn from(v: u128) -> Self {
                Fp2 { c0: v % P, c1: 0 }
            }
        }
        impl core::ops::Add for Fp2 {
            type Output = Fp2;
            fn add(self, o: Fp2) -> Fp2 {
                // Coordinates < P < 2^64: sums stay far below 2^128.
                Fp2 { c0: (self.c0 + o.c0) % P, c1: (self.c1 + o.c1) % P }
            }
        }
        impl core::ops::Mul for Fp2 {
            type Output = Fp2;
            fn mul(self, o: Fp2) -> Fp2 {
                // Products of < 2^64 values fit u128 exactly.
                let m = |a: u128, b: u128| (a * b) % P;
                Fp2 {
                    c0: (m(self.c0, o.c0) + m(7, m(self.c1, o.c1))) % P,
                    c1: (m(self.c0, o.c1) + m(self.c1, o.c0)) % P,
                }
            }
        }
        let basis = [Fp2 { c0: 1, c1: 0 }, Fp2 { c0: 0, c1: 1 }];

        let alpha = smallest_generator();
        let q_bits = 64usize; // ⌈log₂ p⌉ for Goldilocks
        let ext_deg = 2usize;
        let proj = ExtProjParams::default();
        for (t, s_vars, w) in [(10usize, 5usize, 1usize), (4, 8, 32), (6, 6, 64)] {
            let p = IntEvalParams { t, s: s_vars, word_bits: w };
            let m_p = packed_vars(&p);
            let c_w = mod_q_chunk_width(&p);
            let l1 = mod_q_num_chunks(&p, q_bits);
            let l2 = mod_q_num_chunks(&p, proj.prime_bits);
            let (pc, vc) = lig_configs(m_p, LigConfig::Adhoc { log_batch: 2, log_inv_rate: 2 })
                .expect("cfg");

            let mask = if w == 128 { u128::MAX } else { (1u128 << w) - 1 };
            let data: Vec<u128> = (0..p.cells())
                .map(|i| (i as u128).wrapping_mul(0x9E37_79B9_7F4A_7C15) & mask)
                .collect();
            // Coordinate-major canonical lift of v⁽¹⁾ ∈ K^{2^t}: two
            // arbitrary coordinate vectors in [0, p).
            let coords: Vec<Vec<u128>> = (0..ext_deg)
                .map(|d| {
                    (0..p.rows())
                        .map(|b| {
                            (b as u128)
                                .wrapping_mul(0xDEAD_BEEF_CAFE_F00D_1234_5678_9ABC_DEF1)
                                .wrapping_add(d as u128 + 7)
                                % P
                        })
                        .collect()
                })
                .collect();
            let cw_small: Vec<u128> = (0..p.cols())
                .map(|c| ((c as u128).wrapping_mul(11) & 15).wrapping_add(1))
                .collect();
            let col_w: Vec<Fp2> = cw_small
                .iter()
                .enumerate()
                .map(|(c, &x)| Fp2 { c0: x, c1: (c as u128).wrapping_mul(3) % P })
                .collect();
            // Expected μ ∈ K, computed directly: Σ_c w′_c·Σ_b v⁽¹⁾_b·INT(D).
            let mut y = Fp2::from(0u128);
            for c in 0..p.cols() {
                let mut vc_acc = Fp2::from(0u128);
                for b in 0..p.rows() {
                    let v1 = Fp2 { c0: coords[0][b], c1: coords[1][b] };
                    vc_acc = vc_acc + v1 * Fp2::from(data[p.cell_index(b, c)]);
                }
                y = y + col_w[c] * vc_acc;
            }

            let hint = commit_rs_flock_with(&p, &data, pc.log_inv_rates[0], pc.initial_k);
            let mut pt = Blake3Transcript::new();
            let proof =
                prove_mle_eval_ext_ligerito(&mut pt, &hint, &p, &coords, q_bits, &proj, alpha, &pc);
            assert_eq!(proof.mus.len(), ext_deg * l1, "step-1 fold count (c_w={c_w})");
            assert_eq!(proof.base.us.len(), l2, "projected chunk count (c_w={c_w})");

            let mut vt = Blake3Transcript::new();
            verify_mle_eval_ext_ligerito(
                &mut vt, &hint.commitment, &proof, &p, &coords, &col_w, &basis, alpha, y, q_bits,
                &proj, &vc,
            )
            .unwrap_or_else(|e| panic!("ext (t={t},W={w},L1={l1},L2={l2}) failed: {e:?}"));

            // Wrong claim → the K-side read-off rejects.
            let mut vt = Blake3Transcript::new();
            assert_eq!(
                verify_mle_eval_ext_ligerito(
                    &mut vt, &hint.commitment, &proof, &p, &coords, &col_w, &basis, alpha,
                    y + Fp2::from(1u128), q_bits, &proj, &vc,
                ),
                Err(FlockRsError::ExtReadOff),
                "wrong claim must be rejected (t={t},W={w})"
            );

            // Tampered Step-1 fold (within range): the transcript diverges
            // before the projection sampling, so verification must fail.
            let mut bad = proof.clone();
            bad.mus[0][0] ^= 1;
            let mut vt = Blake3Transcript::new();
            assert!(
                verify_mle_eval_ext_ligerito(
                    &mut vt, &hint.commitment, &bad, &p, &coords, &col_w, &basis, alpha, y,
                    q_bits, &proj, &vc,
                )
                .is_err(),
                "tampered step-1 fold must be rejected (t={t},W={w})"
            );

            // Out-of-range Step-1 fold.
            let mut bad = proof.clone();
            bad.mus[0][0] = u128::MAX - 1;
            let mut vt = Blake3Transcript::new();
            assert!(
                matches!(
                    verify_mle_eval_ext_ligerito(
                        &mut vt, &hint.commitment, &bad, &p, &coords, &col_w, &basis, alpha, y,
                        q_bits, &proj, &vc,
                    ),
                    Err(FlockRsError::ExtChunkRange { coeff: 0, chunk: 0, col: 0 })
                ),
                "out-of-range step-1 fold must be rejected (t={t},W={w})"
            );

            // Wrong fold-table shape.
            let mut bad = proof.clone();
            bad.mus.pop();
            let mut vt = Blake3Transcript::new();
            assert_eq!(
                verify_mle_eval_ext_ligerito(
                    &mut vt, &hint.commitment, &bad, &p, &coords, &col_w, &basis, alpha, y,
                    q_bits, &proj, &vc,
                ),
                Err(FlockRsError::ExtShape),
                "missing fold vector must be rejected (t={t},W={w})"
            );

            // Codec: canonical roundtrip, and the decoded proof verifies.
            let bytes = proof.to_bytes();
            let rt = IntEvalRsLigExtProof::from_bytes(&bytes).expect("ext codec roundtrip");
            assert_eq!(bytes, rt.to_bytes(), "ext codec is canonical");
            let mut vt = Blake3Transcript::new();
            verify_mle_eval_ext_ligerito(
                &mut vt, &hint.commitment, &rt, &p, &coords, &col_w, &basis, alpha, y, q_bits,
                &proj, &vc,
            )
            .expect("decoded ext proof verifies");
        }
    }

    /// The complete F2Z proof object round-trips through the host byte stream
    /// (zinc parts field-by-field + a length-prefixed `bincode` `LigeritoProof`
    /// blob), and any single tampered byte is rejected — the stream fails to
    /// decode, or the reconstructed proof fails verification.
    #[test]
    fn mod_q_ligerito_proof_serialization_roundtrips() {
        const Q: u128 = (1u128 << 100) - 15;
        #[derive(Clone, Copy, PartialEq, Debug)]
        struct Fq(u128);
        impl From<u128> for Fq {
            fn from(v: u128) -> Self {
                Fq(v % Q)
            }
        }
        impl core::ops::Add for Fq {
            type Output = Fq;
            fn add(self, o: Fq) -> Fq {
                let s = self.0 + o.0;
                Fq(if s >= Q { s - Q } else { s })
            }
        }
        impl core::ops::Mul for Fq {
            type Output = Fq;
            fn mul(self, o: Fq) -> Fq {
                let (mut a, mut b, mut acc) = (self.0, o.0, 0u128);
                while b != 0 {
                    if b & 1 == 1 {
                        let s = acc + a;
                        acc = if s >= Q { s - Q } else { s };
                    }
                    let d = a << 1;
                    a = if d >= Q { d - Q } else { d };
                    b >>= 1;
                }
                Fq(acc)
            }
        }
        let alpha = smallest_generator();
        let q_bits = 100usize;
        let p = IntEvalParams { t: 10, s: 5, word_bits: 1 };
        let m_p = packed_vars(&p);
        let (pc, vc) =
            lig_configs(m_p, LigConfig::Adhoc { log_batch: 2, log_inv_rate: 2 }).expect("cfg");
        let data: Vec<u128> =
            (0..p.cells()).map(|i| (i as u128).wrapping_mul(0x9E37_79B9_7F4A_7C15) & 1).collect();
        let rw_q: Vec<u128> = (0..p.rows())
            .map(|b| {
                (b as u128)
                    .wrapping_mul(0xDEAD_BEEF_CAFE_F00D_1234_5678_9ABC_DEF1)
                    .wrapping_add(7)
                    % Q
            })
            .collect();
        let cw_small: Vec<u128> =
            (0..p.cols()).map(|c| ((c as u128).wrapping_mul(5) & 7).wrapping_add(1)).collect();
        let col_w: Vec<Fq> = cw_small.iter().map(|&x| Fq::from(x)).collect();
        let mut y = Fq::from(0u128);
        for c in 0..p.cols() {
            let mut vc_acc = Fq::from(0u128);
            for b in 0..p.rows() {
                vc_acc = vc_acc + Fq::from(rw_q[b]) * Fq::from(data[p.cell_index(b, c)]);
            }
            y = y + col_w[c] * vc_acc;
        }
        let hint = commit_rs_flock_with(&p, &data, pc.log_inv_rates[0], pc.initial_k);
        let mut pt = Blake3Transcript::new();
        let proof = prove_mle_eval_mod_q_ligerito(&mut pt, &hint, &p, &rw_q, q_bits, alpha, &pc);

        // Round-trip: serialize -> deserialize -> verify passes.
        let bytes = proof.to_bytes();
        let proof2 = IntEvalRsLigModQProof::from_bytes(&bytes).expect("deserialize");
        let mut vt = Blake3Transcript::new();
        verify_mle_eval_mod_q_ligerito(
            &mut vt, &hint.commitment, &proof2, &p, &rw_q, &col_w, alpha, y, q_bits, &vc,
        )
        .expect("deserialized proof verifies");
        // Canonical: re-serialization is byte-identical.
        assert_eq!(bytes, proof2.to_bytes(), "codec is canonical");

        // Tamper: flipping any single byte is rejected.
        for &pos in &[0usize, bytes.len() / 3, bytes.len() / 2, bytes.len() - 1] {
            let mut bad = bytes.clone();
            bad[pos] ^= 0x01;
            let rejected = match IntEvalRsLigModQProof::from_bytes(&bad) {
                Err(_) => true,
                Ok(bad_proof) => {
                    let mut vt = Blake3Transcript::new();
                    verify_mle_eval_mod_q_ligerito(
                        &mut vt, &hint.commitment, &bad_proof, &p, &rw_q, &col_w, alpha, y, q_bits,
                        &vc,
                    )
                    .is_err()
                }
            };
            assert!(rejected, "tampered byte at {pos} not rejected");
        }
    }

    /// A zero-padded witness must not pay for its padding on the wire: the
    /// trailing all-zero columns' chunk folds are re-derived by the
    /// verifier, not transmitted. The encoding stays canonical (a
    /// non-minimal `us` block is rejected) and the decoded proof still
    /// verifies against the un-changed verifier surface.
    #[test]
    fn mod_q_ligerito_padded_witness_trims_us() {
        use crate::pcs::{FQ_BITS, Fq};
        let alpha = smallest_generator();
        let p = IntEvalParams { t: 10, s: 5, word_bits: 1 };
        let m_p = packed_vars(&p);
        let (pc, vc) =
            lig_configs(m_p, LigConfig::Adhoc { log_batch: 2, log_inv_rate: 2 }).expect("cfg");
        // φ ≈ 0.55: columns `live..32` are the zero padding of a witness
        // of N = live·2^t cells.
        let live = 18usize;
        assert!(live < p.cols());
        let data: Vec<u128> = (0..p.cells())
            .map(|i| {
                let c = i & (p.cols() - 1);
                if c >= live { 0 } else { (i as u128).wrapping_mul(0x9E37_79B9_7F4A_7C15) & 1 }
            })
            .collect();
        let rw_q: Vec<u128> = (0..p.rows())
            .map(|b| {
                (b as u128)
                    .wrapping_mul(0xDEAD_BEEF_CAFE_F00D_1234_5678_9ABC_DEF1)
                    .wrapping_add(7)
                    % crate::pcs::FQ_MOD
            })
            .collect();
        let col_w: Vec<Fq> =
            (0..p.cols()).map(|c| Fq::from(((c as u128).wrapping_mul(5) & 7) + 1)).collect();
        let mut y = Fq::from(0u128);
        for c in 0..p.cols() {
            let mut acc = Fq::from(0u128);
            for b in 0..p.rows() {
                acc = acc + Fq::from(rw_q[b]) * Fq::from(data[p.cell_index(b, c)]);
            }
            y = y + col_w[c] * acc;
        }
        let hint = commit_rs_flock_with(&p, &data, pc.log_inv_rates[0], pc.initial_k);
        let mut pt = Blake3Transcript::new();
        let proof = prove_mle_eval_mod_q_ligerito(&mut pt, &hint, &p, &rw_q, FQ_BITS, alpha, &pc);

        // Prover-side `us` is full width; the wire carries only the live
        // prefix (the last live column is non-zero by construction).
        assert_eq!(proof.us[0].len(), p.cols(), "prover holds all 2^s folds");
        assert!(proof.us[0][live..].iter().all(|&u| u == 0), "padding folds are zero");
        let bytes = proof.to_bytes();
        let proof2 = IntEvalRsLigModQProof::from_bytes(&bytes).expect("deserialize");
        assert_eq!(proof2.us[0].len(), live, "only the live folds are transmitted");

        // …and it still verifies, with no verifier-side change.
        let mut vt = Blake3Transcript::new();
        verify_mle_eval_mod_q_ligerito(
            &mut vt, &hint.commitment, &proof2, &p, &rw_q, &col_w, alpha, y, FQ_BITS, &vc,
        )
        .expect("trimmed proof verifies");
        assert_eq!(bytes, proof2.to_bytes(), "codec is canonical");

        // A non-minimal `us` block (one transmitted trailing zero) decodes
        // to the same proof object, so the codec must reject it.
        let mut needle = Vec::new();
        for &u in &proof2.us[0] {
            needle.extend_from_slice(&u.to_le_bytes());
        }
        let pos = bytes.windows(needle.len()).position(|w| w == needle).expect("us block");
        let mut bad = Vec::new();
        bad.extend_from_slice(&bytes[..pos - 8]);
        bad.extend_from_slice(&(live as u64 + 1).to_le_bytes());
        bad.extend_from_slice(&needle);
        bad.extend_from_slice(&[0u8; 16]);
        bad.extend_from_slice(&bytes[pos + needle.len()..]);
        assert!(
            matches!(
                IntEvalRsLigModQProof::from_bytes(&bad),
                Err(crate::proof_codec::CodecError::NonCanonical)
            ),
            "non-minimal us encoding accepted"
        );

        // An over-wide `us` (more folds than columns) is a shape error.
        let mut wide = IntEvalRsLigModQProof::from_bytes(&bytes).expect("deserialize");
        wide.us[0].resize(p.cols() + 1, 0);
        wide.us[0][p.cols()] = 7;
        let mut vt = Blake3Transcript::new();
        assert!(
            verify_mle_eval_mod_q_ligerito(
                &mut vt,
                &hint.commitment,
                &wide,
                &p,
                &rw_q,
                &col_w,
                alpha,
                y,
                FQ_BITS,
                &vc
            )
            .is_err(),
            "over-wide us accepted"
        );
    }

    // ── RLC claim-family tests (EXPERIMENTAL API) ────────────────────────

    use crate::pcs::{FQ_BITS, FQ_MOD, Fq, extract_virtual_xor_rows, virtual_xor_params};

    /// Synthetic W=1 layout for family tests: 4 UAIR columns of 8 bit
    /// positions over 2^10 trace rows; t = 9, s = 6 (n = 15); the x tensor
    /// is t' = 7, s = 6 (n' = 13).
    fn rlc_test_layout() -> ShaF2Layout {
        ShaF2Layout {
            p: IntEvalParams { t: 9, s: 6, word_bits: 1 },
            num_cols: 4,
            log_cols: 2,
            bit_vars: 3,
            num_vars: 10,
            tw: 4,
            x_fold_extra: 0,
        }
    }

    /// Pseudorandom committed bit tensor + commitment for the test layout.
    fn rlc_test_commit(layout: &ShaF2Layout) -> (FlockCommitHint, LigProverConfig, LigVerifierConfig)
    {
        let p = &layout.p;
        let m_p = packed_vars(p);
        let (pc, vc) =
            lig_configs(m_p, LigConfig::Adhoc { log_batch: 2, log_inv_rate: 2 }).expect("cfg");
        let data: Vec<u128> = (0..p.cells())
            .map(|i| u128::from((i as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) >> 37) & 1)
            .collect();
        let hint = commit_rs_flock_with(p, &data, pc.log_inv_rates[0], pc.initial_k);
        (hint, pc, vc)
    }

    /// Pseudorandom `[0, q)` row-weight vector over the x tensor.
    fn rlc_test_row_weights(p_x: &IntEvalParams, seed: u128) -> Vec<u128> {
        (0..p_x.rows())
            .map(|b| {
                (b as u128)
                    .wrapping_mul(0xDEAD_BEEF_CAFE_F00D_1234_5678_9ABC_DEF1)
                    .wrapping_add(seed.wrapping_mul(0x1234_5677))
                    % FQ_MOD
            })
            .collect()
    }

    /// Direct 𝔽_q evaluation `Σ_c e_c·Σ_b w_b·a[(b,c)]` of the claim
    /// vector `a = ⊕_{i'∈form} m_{i'}` from the committed rows.
    fn rlc_expected_claim(
        layout: &ShaF2Layout,
        rows: &[Vec<u64>],
        family_cols: &[usize],
        form: usize,
        rw: &[u128],
        colw: &[Fq],
    ) -> u128 {
        let cols: Vec<usize> = (0..family_cols.len())
            .filter(|&fi| (form >> fi) & 1 == 1)
            .map(|fi| family_cols[fi])
            .collect();
        let a_rows = extract_virtual_xor_rows(layout, rows, &cols, 0, None);
        let mut y = Fq::from(0u128);
        for (c, row) in a_rows.iter().enumerate() {
            let mut acc = Fq::from(0u128);
            for (wi, &word) in row.iter().enumerate() {
                let mut bits = word;
                while bits != 0 {
                    let t = bits.trailing_zeros() as usize;
                    acc = acc + Fq::from(rw[(wi << 6) | t]);
                    bits &= bits.wrapping_sub(1);
                }
            }
            y = y + colw[c] * acc;
        }
        y.0
    }

    /// Shared clear-axis weights for the tests.
    fn rlc_test_col_weights(p_x: &IntEvalParams) -> Vec<Fq> {
        (0..p_x.cols())
            .map(|c| Fq::from((c as u128).wrapping_mul(0xABCD_EF01_2345).wrapping_add(3)))
            .collect()
    }

    /// Flip one byte of a multi-degree sumcheck proof through its
    /// transcription codec (offset from the END — the tail is the
    /// claimed-sums region).
    fn rlc_tamper_mds(
        p: &MultiDegreeSumcheckProof<Gf>,
        back_off: usize,
    ) -> MultiDegreeSumcheckProof<Gf> {
        use crate::transcript::traits::{GenTranscribable, Transcribable};
        let mut buf = vec![0u8; p.get_num_bytes()];
        p.write_transcription_bytes_exact(&mut buf);
        let idx = buf.len() - 1 - back_off;
        buf[idx] ^= 1;
        MultiDegreeSumcheckProof::<Gf>::read_transcription_bytes_exact(&buf)
    }

    /// Flip one byte of a plain sumcheck proof through its transcription
    /// codec (offset from the END).
    fn rlc_tamper_sc(
        p: &crate::piop::sumcheck::SumcheckProof<Gf>,
        back_off: usize,
    ) -> crate::piop::sumcheck::SumcheckProof<Gf> {
        use crate::transcript::traits::{GenTranscribable, Transcribable};
        let mut buf = vec![0u8; p.get_num_bytes()];
        p.write_transcription_bytes_exact(&mut buf);
        let idx = buf.len() - 1 - back_off;
        buf[idx] ^= 1;
        crate::piop::sumcheck::SumcheckProof::<Gf>::read_transcription_bytes_exact(&buf)
    }

    /// The lazy 4-case forest ([`crate::merged_forest::prove_merged_forest_lazy_rlc2`])
    /// is byte-identical to the eager reference over the same leaves: same
    /// roots, exit point, exit claim, and post-forest transcript state; and
    /// the roots bind the case-weight folds (`α^{u_c}`).
    #[test]
    fn rlc2_lazy_forest_matches_eager() {
        use crate::pcs::{
            gf_pow, mod_q_chunk_width, rlc_case_pow_table, rlc_case_weights,
            rlc_chunk_case_weights,
        };
        let layout = rlc_test_layout();
        let p_x = virtual_xor_params(&layout);
        let (hint, _pc, _vc) = rlc_test_commit(&layout);
        let alpha = smallest_generator();
        let family_cols = [0usize, 1];
        let x_rows: Vec<Vec<Vec<u64>>> = family_cols
            .iter()
            .map(|&i| {
                extract_virtual_xor_rows(&layout, hint.rows(), core::slice::from_ref(&i), 0, None)
            })
            .collect();
        let rws: Vec<Vec<u128>> =
            (0..3).map(|i| rlc_test_row_weights(&p_x, 91 + i as u128)).collect();
        let w_refs: Vec<&[u128]> = rws.iter().map(|w| &w[..]).collect();
        let case_w = rlc_case_weights(&w_refs, &[5, 9, 13], &[0b01, 0b10, 0b11], 2);
        let c_w_x = mod_q_chunk_width(&p_x);
        let chunks = rlc_chunk_case_weights(&case_w, c_w_x, 1);
        let case_pow = rlc_case_pow_table(&chunks[0], alpha);
        let (leaves, us) = rlc_leaves_and_folds(&p_x, &x_rows, &chunks[0], &case_pow);
        let t_x = row_bit_vars(&p_x);

        let mut t1 = Blake3Transcript::new();
        let (roots_e, _mf_e, z_e, ed_e) =
            crate::merged_forest::prove_merged_forest(&mut t1, &leaves, t_x, p_x.s);
        let mut t2 = Blake3Transcript::new();
        let (roots_l, _mf_l, z_l, ed_l) = crate::merged_forest::prove_merged_forest_lazy_rlc2(
            &mut t2, &p_x, &x_rows[0], &x_rows[1], &case_pow,
        );
        assert_eq!(roots_e, roots_l, "roots");
        assert_eq!(z_e, z_l, "exit point");
        assert_eq!(ed_e, ed_l, "exit claim");
        let c1: Gf = t1.get_field_challenge(&());
        let c2: Gf = t2.get_field_challenge(&());
        assert_eq!(c1, c2, "lazy rlc2 forest must be byte-identical to the eager reference");
        for (c, &u) in us.iter().enumerate() {
            assert_eq!(gf_pow(alpha, u), roots_l[c], "root {c} binds α^u");
        }
    }

    /// The general (j = 3) Dense-JIT lazy forest is byte-identical to the
    /// eager reference over the same leaves.
    #[test]
    fn rlc_general_lazy_forest_matches_eager() {
        use crate::pcs::{
            mod_q_chunk_width, rlc_case_pow_table, rlc_case_weights, rlc_chunk_case_weights,
        };
        let layout = rlc_test_layout();
        let p_x = virtual_xor_params(&layout);
        let (hint, _pc, _vc) = rlc_test_commit(&layout);
        let alpha = smallest_generator();
        let family_cols = [0usize, 1, 2];
        let x_rows: Vec<Vec<Vec<u64>>> = family_cols
            .iter()
            .map(|&i| {
                extract_virtual_xor_rows(&layout, hint.rows(), core::slice::from_ref(&i), 0, None)
            })
            .collect();
        let rws: Vec<Vec<u128>> =
            (0..4).map(|i| rlc_test_row_weights(&p_x, 171 + i as u128)).collect();
        let w_refs: Vec<&[u128]> = rws.iter().map(|w| &w[..]).collect();
        let case_w = rlc_case_weights(&w_refs, &[5, 9, 13, 21], &[0b001, 0b010, 0b100, 0b111], 3);
        let chunks = rlc_chunk_case_weights(&case_w, mod_q_chunk_width(&p_x), 1);
        let case_pow = rlc_case_pow_table(&chunks[0], alpha);
        let (leaves, _us) = rlc_leaves_and_folds(&p_x, &x_rows, &chunks[0], &case_pow);
        let t_x = row_bit_vars(&p_x);

        let mut t1 = Blake3Transcript::new();
        let (roots_e, _mf_e, z_e, ed_e) =
            crate::merged_forest::prove_merged_forest(&mut t1, &leaves, t_x, p_x.s);
        let mut t2 = Blake3Transcript::new();
        let row_refs: Vec<&[Vec<u64>]> = x_rows.iter().map(|r| &r[..]).collect();
        let (roots_l, _mf_l, z_l, ed_l) =
            crate::merged_forest::prove_merged_forest_lazy_rlc_general(
                &mut t2, &p_x, &row_refs, &case_pow,
            );
        assert_eq!(roots_e, roots_l, "roots");
        assert_eq!(z_e, z_l, "exit point");
        assert_eq!(ed_e, ed_l, "exit claim");
        let c1: Gf = t1.get_field_challenge(&());
        let c2: Gf = t2.get_field_challenge(&());
        assert_eq!(c1, c2, "general lazy forest must be byte-identical to eager");
    }

    /// The XOR triple (k = 3, j = 2, a₃ = a₁ ⊕ a₂, W = 1) — the primary
    /// target: roundtrip with per-claim row points, roundtrip with a shared
    /// row point, and rejection of every tampered component.
    #[test]
    fn rlc_family_xor_triple_roundtrips() {
        let layout = rlc_test_layout();
        let p_x = virtual_xor_params(&layout);
        let alpha = smallest_generator();
        let (hint, pc, vc) = rlc_test_commit(&layout);
        let family_cols = [0usize, 1];
        let forms = [0b01usize, 0b10, 0b11];
        let colw = rlc_test_col_weights(&p_x);

        for same_point in [false, true] {
            let rws: Vec<Vec<u128>> = (0..3)
                .map(|i| rlc_test_row_weights(&p_x, if same_point { 7 } else { 11 + i as u128 }))
                .collect();
            let cs: Vec<u128> = forms
                .iter()
                .zip(rws.iter())
                .map(|(&f, rw)| {
                    rlc_expected_claim(&layout, hint.rows(), &family_cols, f, rw, &colw)
                })
                .collect();
            let claims: Vec<RlcFamilyClaim<'_>> = (0..3)
                .map(|i| RlcFamilyClaim {
                    form: forms[i],
                    row_weights_q: &rws[i],
                    claimed: cs[i],
                })
                .collect();

            let mut pt = Blake3Transcript::new();
            let proof = prove_mle_eval_mod_q_ligerito_rlc_family(
                &mut pt, &hint, &layout, &family_cols, &claims, alpha, &pc,
            );
            assert_eq!(proof.mfs.len(), 1, "L = 1 at this shape");
            assert!(proof.discharge_eqf.is_some() && proof.discharge_eqf2.is_none());
            assert_eq!(proof.omegas.len(), 2);
            assert_eq!(proof.rings.len(), 4, "(L + 1) chunks-and-discharge × j rings");

            let mut vt = Blake3Transcript::new();
            verify_mle_eval_mod_q_ligerito_rlc_family(
                &mut vt, &hint.commitment, &proof, &layout, &family_cols, &claims, &colw, alpha,
                &vc,
            )
            .unwrap_or_else(|e| panic!("XOR triple (same_point={same_point}) failed: {e:?}"));

            if same_point {
                continue; // tamper suite once, on the per-claim-points variant
            }

            // Tampered claimed value: γ's change, the transcript diverges.
            let mut cs_bad = cs.clone();
            cs_bad[1] = (cs_bad[1] + 1) % FQ_MOD;
            let claims_bad: Vec<RlcFamilyClaim<'_>> = (0..3)
                .map(|i| RlcFamilyClaim {
                    form: forms[i],
                    row_weights_q: &rws[i],
                    claimed: cs_bad[i],
                })
                .collect();
            let mut vt = Blake3Transcript::new();
            assert!(
                verify_mle_eval_mod_q_ligerito_rlc_family(
                    &mut vt, &hint.commitment, &proof, &layout, &family_cols, &claims_bad, &colw,
                    alpha, &vc,
                )
                .is_err(),
                "tampered claimed value accepted"
            );

            // Tampered chunk fold (in range): the recomputed roots diverge.
            let rebuild = |proof: &IntEvalRsLigRlcFamilyProof,
                           f: &dyn Fn(&mut IntEvalRsLigRlcFamilyProof)| {
                let mut p2 = IntEvalRsLigRlcFamilyProof {
                    mfs: proof.mfs.clone(),
                    us: proof.us.clone(),
                    presums: proof.presums.clone(),
                    discharge_eqf: proof.discharge_eqf.clone(),
                    omegas: proof.omegas.clone(),
                    discharge_eqf2: proof.discharge_eqf2.clone(),
                    omegas2: proof.omegas2.clone(),
                    rings: proof.rings.clone(),
                    lig: proof.lig.clone(),
                };
                f(&mut p2);
                p2
            };
            let check_rejected = |p2: &IntEvalRsLigRlcFamilyProof, what: &str| {
                let mut vt = Blake3Transcript::new();
                assert!(
                    verify_mle_eval_mod_q_ligerito_rlc_family(
                        &mut vt, &hint.commitment, p2, &layout, &family_cols, &claims, &colw,
                        alpha, &vc,
                    )
                    .is_err(),
                    "{what} accepted"
                );
            };
            check_rejected(&rebuild(&proof, &|p2| p2.us[0][0] ^= 1), "tampered u fold");

            // Out-of-range fold hits the free range check.
            let p2 = rebuild(&proof, &|p2| p2.us[0][0] = 1u128 << 127);
            let mut vt = Blake3Transcript::new();
            assert!(
                matches!(
                    verify_mle_eval_mod_q_ligerito_rlc_family(
                        &mut vt, &hint.commitment, &p2, &layout, &family_cols, &claims, &colw,
                        alpha, &vc,
                    ),
                    Err(FlockRsError::ChunkRange { .. })
                ),
                "out-of-range fold not caught by the range check"
            );

            // Tampered presum: a claimed sum (tail byte) and a round
            // message (interior byte).
            check_rejected(
                &rebuild(&proof, &|p2| p2.presums[0] = rlc_tamper_mds(&p2.presums[0], 0)),
                "tampered presum claimed sum",
            );
            check_rejected(
                &rebuild(&proof, &|p2| {
                    let mid = {
                        use crate::transcript::traits::Transcribable;
                        p2.presums[0].get_num_bytes() / 2
                    };
                    p2.presums[0] = rlc_tamper_mds(&p2.presums[0], mid);
                }),
                "tampered presum round message",
            );

            // Tampered discharge: a phase-A round byte, a β, a phase-B
            // round byte — and below, the ω closing.
            check_rejected(
                &rebuild(&proof, &|p2| {
                    let d = p2.discharge_eqf.as_mut().expect("j = 2 discharge");
                    d.sc_a = rlc_tamper_sc(&d.sc_a, 0);
                }),
                "tampered discharge phase A",
            );
            check_rejected(
                &rebuild(&proof, &|p2| {
                    let d = p2.discharge_eqf.as_mut().expect("j = 2 discharge");
                    d.betas[0] += Gf::one();
                }),
                "tampered discharge β",
            );
            check_rejected(
                &rebuild(&proof, &|p2| {
                    let d = p2.discharge_eqf.as_mut().expect("j = 2 discharge");
                    d.sc_b = rlc_tamper_sc(&d.sc_b, 0);
                }),
                "tampered discharge phase B",
            );
            let p2 = rebuild(&proof, &|p2| p2.omegas[0] += Gf::one());
            let mut vt = Blake3Transcript::new();
            assert_eq!(
                verify_mle_eval_mod_q_ligerito_rlc_family(
                    &mut vt, &hint.commitment, &p2, &layout, &family_cols, &claims, &colw, alpha,
                    &vc,
                ),
                Err(FlockRsError::Common(IntEvalRsError::Discharge)),
                "tampered ω must fail the discharge closing"
            );

            // Tampered ring message.
            let p2 = rebuild(&proof, &|p2| p2.rings[0].s_v[0] += Gf::one());
            let mut vt = Blake3Transcript::new();
            assert_eq!(
                verify_mle_eval_mod_q_ligerito_rlc_family(
                    &mut vt, &hint.commitment, &p2, &layout, &family_cols, &claims, &colw, alpha,
                    &vc,
                ),
                Err(FlockRsError::RingSwitch(RsOpenError::RingSwitchClaim)),
                "tampered ring must fail its residual check"
            );

            // Wrong column weights: the recombination misses T.
            let mut colw_bad = colw.clone();
            colw_bad[0] = colw_bad[0] + Fq::from(1u128);
            let mut vt = Blake3Transcript::new();
            assert_eq!(
                verify_mle_eval_mod_q_ligerito_rlc_family(
                    &mut vt, &hint.commitment, &proof, &layout, &family_cols, &claims, &colw_bad,
                    alpha, &vc,
                ),
                Err(FlockRsError::Common(IntEvalRsError::ReadOff)),
                "wrong column weights must fail the read-off"
            );
        }
    }

    /// A PURE-XOR family (k = 1, j = 2, form = 11 — or several claims on
    /// the same XOR): the γ-combined weight factors through m₁ ⊕ m₂, so
    /// the AND channel's τ is IDENTICALLY zero and must be elided — the
    /// completeness edge of the construction's degenerate case.
    #[test]
    fn rlc_family_pure_xor_family_roundtrips() {
        let layout = rlc_test_layout();
        let p_x = virtual_xor_params(&layout);
        let alpha = smallest_generator();
        let (hint, pc, vc) = rlc_test_commit(&layout);
        let family_cols = [0usize, 1];
        let colw = rlc_test_col_weights(&p_x);
        // k = 1 single XOR claim, and k = 2 multi-point on the same XOR.
        for k in [1usize, 2] {
            let forms: Vec<usize> = vec![0b11; k];
            let rws: Vec<Vec<u128>> =
                (0..k).map(|i| rlc_test_row_weights(&p_x, 130 + i as u128)).collect();
            let cs: Vec<u128> = forms
                .iter()
                .zip(rws.iter())
                .map(|(&f, rw)| {
                    rlc_expected_claim(&layout, hint.rows(), &family_cols, f, rw, &colw)
                })
                .collect();
            let claims: Vec<RlcFamilyClaim<'_>> = (0..k)
                .map(|i| RlcFamilyClaim {
                    form: forms[i],
                    row_weights_q: &rws[i],
                    claimed: cs[i],
                })
                .collect();
            let mut pt = Blake3Transcript::new();
            let proof = prove_mle_eval_mod_q_ligerito_rlc_family(
                &mut pt, &hint, &layout, &family_cols, &claims, alpha, &pc,
            );
            let mut vt = Blake3Transcript::new();
            verify_mle_eval_mod_q_ligerito_rlc_family(
                &mut vt, &hint.commitment, &proof, &layout, &family_cols, &claims, &colw,
                alpha, &vc,
            )
            .unwrap_or_else(|e| panic!("pure-XOR family (k={k}) failed: {e:?}"));
        }
    }

    /// SHARED-POINT maximal families — the full XOR-closure of j columns
    /// at ONE point (j = 2: k = 3, j = 3: k = 7, j = 4: k = 15) —
    /// roundtrip through the collapsed-absorb API; the collapsed statement
    /// binds (tampered claimed value, tampered shared weight vector); and
    /// the shared-point transcript deliberately DIVERGES from the general
    /// API on the same claims (cross-verification rejects both ways).
    #[test]
    fn rlc_family_shared_point_maximal_roundtrips() {
        let layout = rlc_test_layout();
        let p_x = virtual_xor_params(&layout);
        let alpha = smallest_generator();
        let (hint, pc, vc) = rlc_test_commit(&layout);
        let colw = rlc_test_col_weights(&p_x);
        let rw = rlc_test_row_weights(&p_x, 57);

        for j in [2usize, 3, 4] {
            let family_cols: Vec<usize> = (0..j).collect();
            let forms: Vec<usize> = (1..1usize << j).collect(); // maximal: every nonzero form
            let cs: Vec<u128> = forms
                .iter()
                .map(|&f| rlc_expected_claim(&layout, hint.rows(), &family_cols, f, &rw, &colw))
                .collect();
            let claims: Vec<RlcSharedClaim> = forms
                .iter()
                .zip(cs.iter())
                .map(|(&form, &claimed)| RlcSharedClaim { form, claimed })
                .collect();

            let mut pt = Blake3Transcript::new();
            let proof = prove_mle_eval_mod_q_ligerito_rlc_family_shared_point(
                &mut pt, &hint, &layout, &family_cols, &rw, &claims, alpha, &pc,
            );
            let mut vt = Blake3Transcript::new();
            verify_mle_eval_mod_q_ligerito_rlc_family_shared_point(
                &mut vt, &hint.commitment, &proof, &layout, &family_cols, &rw, &claims, &colw,
                alpha, &vc,
            )
            .unwrap_or_else(|e| panic!("maximal shared-point family j={j} failed: {e:?}"));

            // Generic γ's keep every channel alive: full presum width,
            // full cascade (level 2 iff j ≥ 3).
            assert_eq!(proof.presums[0].claimed_sums().len(), (1 << j) - 1, "j={j} channels");
            assert_eq!(proof.discharge_eqf2.is_some(), j >= 3, "j={j} cascade depth");

            // The collapsed absorb binds the claimed values...
            let mut claims_bad = claims.clone();
            claims_bad[1].claimed = (claims_bad[1].claimed + 1) % FQ_MOD;
            let mut vt = Blake3Transcript::new();
            assert!(
                verify_mle_eval_mod_q_ligerito_rlc_family_shared_point(
                    &mut vt, &hint.commitment, &proof, &layout, &family_cols, &rw, &claims_bad,
                    &colw, alpha, &vc,
                )
                .is_err(),
                "tampered claimed value accepted (j={j})"
            );
            // ...and the ONE shared weight vector.
            let mut rw_bad = rw.clone();
            rw_bad[3] = (rw_bad[3] + 1) % FQ_MOD;
            let mut vt = Blake3Transcript::new();
            assert!(
                verify_mle_eval_mod_q_ligerito_rlc_family_shared_point(
                    &mut vt, &hint.commitment, &proof, &layout, &family_cols, &rw_bad, &claims,
                    &colw, alpha, &vc,
                )
                .is_err(),
                "tampered shared weight vector accepted (j={j})"
            );

            // Same claims through the GENERAL API (k copies of the weight
            // vector): same downstream proof shape, but a different
            // transcript — cross-verification must reject both ways.
            let gen_claims: Vec<RlcFamilyClaim<'_>> = forms
                .iter()
                .zip(cs.iter())
                .map(|(&form, &claimed)| RlcFamilyClaim {
                    form,
                    row_weights_q: &rw,
                    claimed,
                })
                .collect();
            let mut pt = Blake3Transcript::new();
            let gen_proof = prove_mle_eval_mod_q_ligerito_rlc_family(
                &mut pt, &hint, &layout, &family_cols, &gen_claims, alpha, &pc,
            );
            assert_eq!(gen_proof.rings.len(), proof.rings.len(), "same core shape (j={j})");
            assert_eq!(gen_proof.omegas.len(), proof.omegas.len(), "same core shape (j={j})");
            let mut vt = Blake3Transcript::new();
            assert!(
                verify_mle_eval_mod_q_ligerito_rlc_family_shared_point(
                    &mut vt, &hint.commitment, &gen_proof, &layout, &family_cols, &rw, &claims,
                    &colw, alpha, &vc,
                )
                .is_err(),
                "general-API proof accepted by the shared-point verifier (j={j})"
            );
            let mut vt = Blake3Transcript::new();
            assert!(
                verify_mle_eval_mod_q_ligerito_rlc_family(
                    &mut vt, &hint.commitment, &proof, &layout, &family_cols, &gen_claims, &colw,
                    alpha, &vc,
                )
                .is_err(),
                "shared-point proof accepted by the general verifier (j={j})"
            );
        }
    }

    /// Shared-point dedupe: duplicate forms with EQUAL claimed values are
    /// canonicalized away (a duplicated list proves and verifies
    /// interchangeably with the canonical list — same transcript), and a
    /// duplicate with a DIFFERENT claimed value is rejected by the
    /// verifier. Plus the degenerate shared-point families: the pure-XOR
    /// singleton (AND channel elided) and j = 1 (all claims dedupe to one).
    #[test]
    /// The shared-point family under `x_fold_extra`: the
    /// p_x-parameterized core (extraction, folds, forests, presum,
    /// cascade, rings) re-splits end to end. Cross-split consistency:
    /// with product-form weights (`colw0 = f ⊗ g` over the low-δ/high
    /// split, `rw_δ = rw0 ⊗ f`), the δ path must prove the SAME claimed
    /// values as the δ = 0 reading of the same flat weight function —
    /// a silent mis-split fails the read-off.
    #[test]
    fn rlc_family_shared_point_delta_roundtrips() {
        let layout0 = rlc_test_layout();
        let p_x0 = virtual_xor_params(&layout0);
        let t_x0 = p_x0.rows().trailing_zeros() as usize;
        let alpha = smallest_generator();
        let (hint, pc, vc) = rlc_test_commit(&layout0);
        let rw0 = rlc_test_row_weights(&p_x0, 461);
        for delta in [1usize, 2] {
            let mut layout = rlc_test_layout();
            layout.x_fold_extra = delta;
            let p_x = virtual_xor_params(&layout);
            let f: Vec<u128> = (0..1usize << delta)
                .map(|i| (i as u128).wrapping_mul(0x1234_5679).wrapping_add(11) % FQ_MOD)
                .collect();
            let g: Vec<Fq> = (0..p_x.cols())
                .map(|c| Fq::from((c as u128).wrapping_mul(0xABC_DEF).wrapping_add(5)))
                .collect();
            let colw0: Vec<Fq> = (0..p_x0.cols())
                .map(|c| Fq::from(f[c & ((1 << delta) - 1)]) * g[c >> delta])
                .collect();
            let rw: Vec<u128> = (0..p_x.rows())
                .map(|i| {
                    let b = i & (p_x0.rows() - 1);
                    let lo = i >> t_x0;
                    (Fq::from(rw0[b]) * Fq::from(f[lo])).0
                })
                .collect();
            for j in [2usize, 3] {
                let family_cols: Vec<usize> = (0..j).collect();
                let forms: Vec<usize> = (1..1usize << j).collect();
                // Claimed values at the FLAT (δ = 0) reading.
                let cs: Vec<u128> = forms
                    .iter()
                    .map(|&fm| {
                        rlc_expected_claim(&layout0, hint.rows(), &family_cols, fm, &rw0, &colw0)
                    })
                    .collect();
                let claims: Vec<RlcSharedClaim> = forms
                    .iter()
                    .zip(cs.iter())
                    .map(|(&form, &claimed)| RlcSharedClaim { form, claimed })
                    .collect();
                let mut pt = Blake3Transcript::new();
                let proof = prove_mle_eval_mod_q_ligerito_rlc_family_shared_point(
                    &mut pt, &hint, &layout, &family_cols, &rw, &claims, alpha, &pc,
                );
                let mut vt = Blake3Transcript::new();
                verify_mle_eval_mod_q_ligerito_rlc_family_shared_point(
                    &mut vt, &hint.commitment, &proof, &layout, &family_cols, &rw, &claims, &g,
                    alpha, &vc,
                )
                .unwrap_or_else(|e| panic!("δ={delta} j={j} family failed: {e:?}"));
                assert_eq!(proof.discharge_eqf2.is_some(), j >= 3, "cascade depth at δ");
                // A wrong value must still be caught at δ.
                let mut bad = claims.clone();
                bad[0].claimed = (bad[0].claimed + 1) % FQ_MOD;
                let mut vt = Blake3Transcript::new();
                assert!(
                    verify_mle_eval_mod_q_ligerito_rlc_family_shared_point(
                        &mut vt, &hint.commitment, &proof, &layout, &family_cols, &rw, &bad,
                        &g, alpha, &vc,
                    )
                    .is_err(),
                    "tampered value accepted at δ={delta} j={j}"
                );
            }
        }
    }

    /// The MERGED multi-family proof (0x47): three families — two j=2
    /// pairs and a j=1 singleton — share one transcript and ONE closing
    /// Ligerito call; roundtrips at δ = 0 and 2; tampered values and
    /// tampered part rings reject.
    #[test]
    fn rlc_families_merged_roundtrips() {
        for delta in [0usize, 2] {
            let mut layout = rlc_test_layout();
            layout.x_fold_extra = delta;
            let p_x = virtual_xor_params(&layout);
            let alpha = smallest_generator();
            let (hint, pc, vc) = rlc_test_commit(&layout);
            let colw = rlc_test_col_weights(&p_x);
            let rw = rlc_test_row_weights(&p_x, 509);
            let f1_cols = [0usize, 1];
            let f2_cols = [2usize, 3];
            let f3_cols = [0usize];
            let mk = |cols: &[usize], forms: &[usize]| -> Vec<RlcSharedClaim> {
                forms
                    .iter()
                    .map(|&form| RlcSharedClaim {
                        form,
                        claimed: rlc_expected_claim(
                            &layout,
                            hint.rows(),
                            cols,
                            form,
                            &rw,
                            &colw,
                        ),
                    })
                    .collect()
            };
            let c1 = mk(&f1_cols, &[1, 2, 3]);
            let c2 = mk(&f2_cols, &[1, 2, 3]);
            let c3 = mk(&f3_cols, &[1]);
            let fams = [
                RlcFamilySpec { family_cols: &f1_cols, claims: &c1 },
                RlcFamilySpec { family_cols: &f2_cols, claims: &c2 },
                RlcFamilySpec { family_cols: &f3_cols, claims: &c3 },
            ];
            let mut pt = Blake3Transcript::new();
            let proof = prove_mle_eval_mod_q_ligerito_rlc_families_shared_point(
                &mut pt, &hint, &layout, &fams, &rw, alpha, &pc,
            );
            assert_eq!(proof.parts.len(), 3, "one part per family");
            let mut vt = Blake3Transcript::new();
            verify_mle_eval_mod_q_ligerito_rlc_families_shared_point(
                &mut vt, &hint.commitment, &proof, &layout, &fams, &rw, &colw, alpha, &vc,
            )
            .unwrap_or_else(|e| panic!("merged families δ={delta} failed: {e:?}"));
            // Tampered claimed value in family 1.
            let mut c1_bad = c1.clone();
            c1_bad[2].claimed = (c1_bad[2].claimed + 1) % FQ_MOD;
            let fams_bad = [
                RlcFamilySpec { family_cols: &f1_cols, claims: &c1_bad },
                RlcFamilySpec { family_cols: &f2_cols, claims: &c2 },
                RlcFamilySpec { family_cols: &f3_cols, claims: &c3 },
            ];
            let mut vt = Blake3Transcript::new();
            assert!(
                verify_mle_eval_mod_q_ligerito_rlc_families_shared_point(
                    &mut vt, &hint.commitment, &proof, &layout, &fams_bad, &rw, &colw,
                    alpha, &vc,
                )
                .is_err(),
                "tampered value accepted at δ={delta}"
            );
            // Tampered ring inside part 1.
            let mut pt = Blake3Transcript::new();
            let mut proof2 = prove_mle_eval_mod_q_ligerito_rlc_families_shared_point(
                &mut pt, &hint, &layout, &fams, &rw, alpha, &pc,
            );
            proof2.parts[1].rings[0].s_v[5] += Gf::one();
            let mut vt = Blake3Transcript::new();
            assert!(
                verify_mle_eval_mod_q_ligerito_rlc_families_shared_point(
                    &mut vt, &hint.commitment, &proof2, &layout, &fams, &rw, &colw, alpha,
                    &vc,
                )
                .is_err(),
                "tampered part ring accepted at δ={delta}"
            );
        }
    }

    #[test]
    fn rlc_family_shared_point_dedupe_and_degenerate() {
        let layout = rlc_test_layout();
        let p_x = virtual_xor_params(&layout);
        let alpha = smallest_generator();
        let (hint, pc, vc) = rlc_test_commit(&layout);
        let colw = rlc_test_col_weights(&p_x);
        let rw = rlc_test_row_weights(&p_x, 91);
        let family_cols = [0usize, 1];
        let forms = [0b01usize, 0b10, 0b11];
        let cs: Vec<u128> = forms
            .iter()
            .map(|&f| rlc_expected_claim(&layout, hint.rows(), &family_cols, f, &rw, &colw))
            .collect();
        let canonical: Vec<RlcSharedClaim> = forms
            .iter()
            .zip(cs.iter())
            .map(|(&form, &claimed)| RlcSharedClaim { form, claimed })
            .collect();
        // The same statement with duplicates interleaved.
        let duplicated: Vec<RlcSharedClaim> = vec![
            canonical[0],
            canonical[1],
            canonical[0], // repeat of form 01, equal c
            canonical[2],
            canonical[2], // repeat of form 11, equal c
        ];

        let mut pt = Blake3Transcript::new();
        let proof_canon = prove_mle_eval_mod_q_ligerito_rlc_family_shared_point(
            &mut pt, &hint, &layout, &family_cols, &rw, &canonical, alpha, &pc,
        );
        let mut pt = Blake3Transcript::new();
        let proof_dup = prove_mle_eval_mod_q_ligerito_rlc_family_shared_point(
            &mut pt, &hint, &layout, &family_cols, &rw, &duplicated, alpha, &pc,
        );
        // Same canonical statement ⇒ same transcript ⇒ same folds.
        assert_eq!(proof_canon.us, proof_dup.us, "dedupe must canonicalize the transcript");
        // Either claim list verifies either proof.
        for (pr, cl) in
            [(&proof_canon, &duplicated), (&proof_dup, &canonical), (&proof_dup, &duplicated)]
        {
            let mut vt = Blake3Transcript::new();
            verify_mle_eval_mod_q_ligerito_rlc_family_shared_point(
                &mut vt, &hint.commitment, pr, &layout, &family_cols, &rw, cl, &colw, alpha,
                &vc,
            )
            .expect("deduped claim lists are interchangeable");
        }
        // A duplicate form with a DIFFERENT claimed value: unsatisfiable
        // statement, rejected outright (Shape) — not silently deduped.
        let mut conflicted = duplicated.clone();
        conflicted[2].claimed = (conflicted[2].claimed + 1) % FQ_MOD;
        let mut vt = Blake3Transcript::new();
        assert!(
            matches!(
                verify_mle_eval_mod_q_ligerito_rlc_family_shared_point(
                    &mut vt, &hint.commitment, &proof_canon, &layout, &family_cols, &rw,
                    &conflicted, &colw, alpha, &vc,
                ),
                Err(FlockRsError::RingSwitch(RsOpenError::Shape))
            ),
            "conflicting duplicate form must reject as Shape"
        );

        // Pure-XOR shared-point singleton: Γ(01) = Γ(10), Γ(11) = 0 — the
        // AND channel's τ vanishes identically and is elided.
        let c_xor = rlc_expected_claim(&layout, hint.rows(), &family_cols, 0b11, &rw, &colw);
        let xor_claims = vec![RlcSharedClaim { form: 0b11, claimed: c_xor }];
        let mut pt = Blake3Transcript::new();
        let proof_xor = prove_mle_eval_mod_q_ligerito_rlc_family_shared_point(
            &mut pt, &hint, &layout, &family_cols, &rw, &xor_claims, alpha, &pc,
        );
        assert!(proof_xor.discharge_eqf.is_none(), "elided AND channel ⇒ no discharge");
        let mut vt = Blake3Transcript::new();
        verify_mle_eval_mod_q_ligerito_rlc_family_shared_point(
            &mut vt, &hint.commitment, &proof_xor, &layout, &family_cols, &rw, &xor_claims,
            &colw, alpha, &vc,
        )
        .expect("shared-point pure-XOR singleton verifies");

        // j = 1: any number of claims on the one column dedupe to a single
        // claim — the base-protocol degeneration.
        let single_col = [0usize];
        let c0 = rlc_expected_claim(&layout, hint.rows(), &single_col, 0b1, &rw, &colw);
        let j1_claims = vec![
            RlcSharedClaim { form: 0b1, claimed: c0 },
            RlcSharedClaim { form: 0b1, claimed: c0 },
        ];
        let mut pt = Blake3Transcript::new();
        let proof_j1 = prove_mle_eval_mod_q_ligerito_rlc_family_shared_point(
            &mut pt, &hint, &layout, &single_col, &rw, &j1_claims, alpha, &pc,
        );
        assert!(proof_j1.discharge_eqf.is_none(), "j = 1 has no monomial channels");
        let mut vt = Blake3Transcript::new();
        verify_mle_eval_mod_q_ligerito_rlc_family_shared_point(
            &mut vt, &hint.commitment, &proof_j1, &layout, &single_col, &rw, &j1_claims, &colw,
            alpha, &vc,
        )
        .expect("j = 1 shared-point dedupe verifies");
    }

    /// The j = 3 family (k = 4, a₄ = a₁ ⊕ a₂ ⊕ a₃): 7 presum channels,
    /// 4 discharge groups up to degree 4. And the j = 1 multi-point
    /// corollary (k = 2 claims on the SAME column at different row points):
    /// 2 cases, NO discharge, one forest.
    #[test]
    fn rlc_family_j3_and_multipoint_corollary() {
        let layout = rlc_test_layout();
        let p_x = virtual_xor_params(&layout);
        let alpha = smallest_generator();
        let (hint, pc, vc) = rlc_test_commit(&layout);
        let colw = rlc_test_col_weights(&p_x);

        // j = 3, k = 4.
        {
            let family_cols = [0usize, 1, 2];
            let forms = [0b001usize, 0b010, 0b100, 0b111];
            let rws: Vec<Vec<u128>> =
                (0..4).map(|i| rlc_test_row_weights(&p_x, 31 + i as u128)).collect();
            let cs: Vec<u128> = forms
                .iter()
                .zip(rws.iter())
                .map(|(&f, rw)| {
                    rlc_expected_claim(&layout, hint.rows(), &family_cols, f, rw, &colw)
                })
                .collect();
            let claims: Vec<RlcFamilyClaim<'_>> = (0..4)
                .map(|i| RlcFamilyClaim {
                    form: forms[i],
                    row_weights_q: &rws[i],
                    claimed: cs[i],
                })
                .collect();
            let mut pt = Blake3Transcript::new();
            let proof = prove_mle_eval_mod_q_ligerito_rlc_family(
                &mut pt, &hint, &layout, &family_cols, &claims, alpha, &pc,
            );
            assert_eq!(proof.presums[0].claimed_sums().len(), 7, "2^3 − 1 channels");
            // Level-1 sides: the three columns + the AND intermediate of
            // the |S| = 3 channel; level 2 discharges the AND.
            assert_eq!(proof.omegas.len(), 4);
            assert!(proof.discharge_eqf.is_some() && proof.discharge_eqf2.is_some());
            assert_eq!(proof.omegas2.len(), 2);
            let mut vt = Blake3Transcript::new();
            verify_mle_eval_mod_q_ligerito_rlc_family(
                &mut vt, &hint.commitment, &proof, &layout, &family_cols, &claims, &colw, alpha,
                &vc,
            )
            .unwrap_or_else(|e| panic!("j=3 family failed: {e:?}"));
        }

        // j = 1, k = 2 — the multi-point batching corollary.
        {
            let family_cols = [2usize];
            let forms = [0b1usize, 0b1];
            let rws: Vec<Vec<u128>> =
                (0..2).map(|i| rlc_test_row_weights(&p_x, 51 + i as u128)).collect();
            let cs: Vec<u128> = forms
                .iter()
                .zip(rws.iter())
                .map(|(&f, rw)| {
                    rlc_expected_claim(&layout, hint.rows(), &family_cols, f, rw, &colw)
                })
                .collect();
            let claims: Vec<RlcFamilyClaim<'_>> = (0..2)
                .map(|i| RlcFamilyClaim {
                    form: forms[i],
                    row_weights_q: &rws[i],
                    claimed: cs[i],
                })
                .collect();
            let mut pt = Blake3Transcript::new();
            let proof = prove_mle_eval_mod_q_ligerito_rlc_family(
                &mut pt, &hint, &layout, &family_cols, &claims, alpha, &pc,
            );
            assert!(proof.discharge_eqf.is_none() && proof.omegas.is_empty(), "j=1: no discharge");
            assert_eq!(proof.mfs.len(), 1, "ONE forest for both points");
            let mut vt = Blake3Transcript::new();
            verify_mle_eval_mod_q_ligerito_rlc_family(
                &mut vt, &hint.commitment, &proof, &layout, &family_cols, &claims, &colw, alpha,
                &vc,
            )
            .unwrap_or_else(|e| panic!("multi-point corollary failed: {e:?}"));
        }

        // j = 4, k = 5 (a₅ = a₁⊕a₂⊕a₃⊕a₄): the |S| = 4 channel pairs two
        // AND intermediates — full cascade depth. Also tamper the level-2
        // pieces.
        {
            let family_cols = [0usize, 1, 2, 3];
            let forms = [0b0001usize, 0b0010, 0b0100, 0b1000, 0b1111];
            let rws: Vec<Vec<u128>> =
                (0..5).map(|i| rlc_test_row_weights(&p_x, 61 + i as u128)).collect();
            let cs: Vec<u128> = forms
                .iter()
                .zip(rws.iter())
                .map(|(&f, rw)| {
                    rlc_expected_claim(&layout, hint.rows(), &family_cols, f, rw, &colw)
                })
                .collect();
            let claims: Vec<RlcFamilyClaim<'_>> = (0..5)
                .map(|i| RlcFamilyClaim {
                    form: forms[i],
                    row_weights_q: &rws[i],
                    claimed: cs[i],
                })
                .collect();
            let mut pt = Blake3Transcript::new();
            let proof = prove_mle_eval_mod_q_ligerito_rlc_family(
                &mut pt, &hint, &layout, &family_cols, &claims, alpha, &pc,
            );
            assert_eq!(proof.presums[0].claimed_sums().len(), 15, "2^4 − 1 channels");
            assert!(proof.discharge_eqf.is_some() && proof.discharge_eqf2.is_some());
            let mut vt = Blake3Transcript::new();
            verify_mle_eval_mod_q_ligerito_rlc_family(
                &mut vt, &hint.commitment, &proof, &layout, &family_cols, &claims, &colw,
                alpha, &vc,
            )
            .unwrap_or_else(|e| panic!("j=4 family failed: {e:?}"));

            // Tampered level-2 ω and level-2 phase-B message.
            let mut p2 = IntEvalRsLigRlcFamilyProof {
                mfs: proof.mfs.clone(),
                us: proof.us.clone(),
                presums: proof.presums.clone(),
                discharge_eqf: proof.discharge_eqf.clone(),
                omegas: proof.omegas.clone(),
                discharge_eqf2: proof.discharge_eqf2.clone(),
                omegas2: proof.omegas2.clone(),
                rings: proof.rings.clone(),
                lig: proof.lig.clone(),
            };
            p2.omegas2[0] += Gf::one();
            let mut vt = Blake3Transcript::new();
            assert!(
                verify_mle_eval_mod_q_ligerito_rlc_family(
                    &mut vt, &hint.commitment, &p2, &layout, &family_cols, &claims, &colw,
                    alpha, &vc,
                )
                .is_err(),
                "tampered level-2 omega accepted"
            );
            let mut p2 = IntEvalRsLigRlcFamilyProof {
                mfs: proof.mfs.clone(),
                us: proof.us.clone(),
                presums: proof.presums.clone(),
                discharge_eqf: proof.discharge_eqf.clone(),
                omegas: proof.omegas.clone(),
                discharge_eqf2: proof.discharge_eqf2.clone(),
                omegas2: proof.omegas2.clone(),
                rings: proof.rings.clone(),
                lig: proof.lig.clone(),
            };
            {
                let d2 = p2.discharge_eqf2.as_mut().expect("level 2 present");
                d2.sc_b = rlc_tamper_sc(&d2.sc_b, 0);
            }
            let mut vt = Blake3Transcript::new();
            assert!(
                verify_mle_eval_mod_q_ligerito_rlc_family(
                    &mut vt, &hint.commitment, &p2, &layout, &family_cols, &claims, &colw,
                    alpha, &vc,
                )
                .is_err(),
                "tampered level-2 discharge accepted"
            );
        }
    }

    /// The RLC family and the virtual-XOR path accept the SAME statement
    /// (same commitment, claims, weights, column weights) — the family is a
    /// drop-in for the batched-vx API on shared-column-point XOR families.
    #[test]
    fn rlc_family_matches_virtual_xor_path() {
        let layout = rlc_test_layout();
        let p_x = virtual_xor_params(&layout);
        let alpha = smallest_generator();
        let (hint, pc, vc) = rlc_test_commit(&layout);
        let family_cols = [0usize, 1];
        let forms = [0b01usize, 0b10, 0b11];
        let colw = rlc_test_col_weights(&p_x);
        let rws: Vec<Vec<u128>> =
            (0..3).map(|i| rlc_test_row_weights(&p_x, 71 + i as u128)).collect();
        let cs: Vec<u128> = forms
            .iter()
            .zip(rws.iter())
            .map(|(&f, rw)| rlc_expected_claim(&layout, hint.rows(), &family_cols, f, rw, &colw))
            .collect();

        // RLC-family proof.
        let claims: Vec<RlcFamilyClaim<'_>> = (0..3)
            .map(|i| RlcFamilyClaim { form: forms[i], row_weights_q: &rws[i], claimed: cs[i] })
            .collect();
        let mut pt = Blake3Transcript::new();
        let rlc_proof = prove_mle_eval_mod_q_ligerito_rlc_family(
            &mut pt, &hint, &layout, &family_cols, &claims, alpha, &pc,
        );
        let mut vt = Blake3Transcript::new();
        verify_mle_eval_mod_q_ligerito_rlc_family(
            &mut vt, &hint.commitment, &rlc_proof, &layout, &family_cols, &claims, &colw, alpha,
            &vc,
        )
        .expect("RLC family verifies");

        // Virtual-XOR (claims-only) proof of the same statement.
        let col_lists: Vec<Vec<usize>> = forms
            .iter()
            .map(|&f| {
                (0..family_cols.len())
                    .filter(|&fi| (f >> fi) & 1 == 1)
                    .map(|fi| family_cols[fi])
                    .collect()
            })
            .collect();
        let xors: Vec<VirtualXorClaim<'_>> = (0..3)
            .map(|i| VirtualXorClaim {
                cols: &col_lists[i],
                constant: 0,
                external_rows: None,
                row_weights_q: &rws[i],
            })
            .collect();
        let mut pt = Blake3Transcript::new();
        let vx_proof = prove_mle_eval_mod_q_ligerito_claims_only(
            &mut pt, &hint, &layout, FQ_BITS, &xors, alpha, &pc,
        );
        let vx_claims: Vec<VirtualXorVerifyClaim<'_, Fq>> = (0..3)
            .map(|i| VirtualXorVerifyClaim {
                cols: &col_lists[i],
                constant: 0,
                has_external: false,
                row_weights_q: &rws[i],
                col_weights: &colw,
                claimed: Fq::from(cs[i]),
            })
            .collect();
        let mut vt = Blake3Transcript::new();
        let obligations = verify_mle_eval_mod_q_ligerito_claims_only(
            &mut vt, &hint.commitment, &vx_proof, &layout, alpha, FQ_BITS, &vx_claims, &vc,
        )
        .expect("virtual-XOR path verifies the same statement");
        assert!(obligations.is_empty());

        // Cross-check is non-vacuous: a wrong claimed value fails BOTH paths.
        let mut cs_bad = cs.clone();
        cs_bad[2] = (cs_bad[2] + 1) % FQ_MOD;
        let claims_bad: Vec<RlcFamilyClaim<'_>> = (0..3)
            .map(|i| RlcFamilyClaim { form: forms[i], row_weights_q: &rws[i], claimed: cs_bad[i] })
            .collect();
        let mut vt = Blake3Transcript::new();
        assert!(
            verify_mle_eval_mod_q_ligerito_rlc_family(
                &mut vt, &hint.commitment, &rlc_proof, &layout, &family_cols, &claims_bad, &colw,
                alpha, &vc,
            )
            .is_err()
        );
        let vx_claims_bad: Vec<VirtualXorVerifyClaim<'_, Fq>> = (0..3)
            .map(|i| VirtualXorVerifyClaim {
                cols: &col_lists[i],
                constant: 0,
                has_external: false,
                row_weights_q: &rws[i],
                col_weights: &colw,
                claimed: Fq::from(cs_bad[i]),
            })
            .collect();
        let mut vt = Blake3Transcript::new();
        assert!(
            verify_mle_eval_mod_q_ligerito_claims_only(
                &mut vt, &hint.commitment, &vx_proof, &layout, alpha, FQ_BITS, &vx_claims_bad,
                &vc,
            )
            .is_err()
        );
    }

    /// The fused basis-fill + round-0 message equals the scalar `η·Φ` fill
    /// and the naive `(u_0, u_2)` pair sums (flock's `round_msg_lsb`
    /// convention) exactly.
    #[test]
    fn fill_phi_basis_round0_matches_unfused() {
        let n = 64usize;
        let lch = 2usize;
        let p_msg: Vec<F128> = (0..n).map(|i| gf_to_f128(sample(0xE000 + i as u64))).collect();
        let eq_his: Vec<Vec<Gf>> = (0..lch)
            .map(|l| (0..n).map(|y| sample(0xF000 + (l * n + y) as u64)).collect())
            .collect();
        let etas: Vec<Gf> = (0..lch).map(|l| sample(0x1_0000 + l as u64)).collect();
        let eq_r2: Vec<Gf> = (0..128).map(|i| sample(0x2_0000 + i as u64)).collect();

        // Scalar reference: η·Φ per slot, then plain-mul pair sums.
        let expect_b: Vec<F128> = (0..n)
            .map(|y| {
                let mut acc = Gf::zero();
                for l in 0..lch {
                    acc += etas[l] * phi_r2(eq_his[l][y], &eq_r2);
                }
                gf_to_f128(acc)
            })
            .collect();
        let mut exp_u0 = Gf::zero();
        let mut exp_u2 = Gf::zero();
        for j in 0..n / 2 {
            let f0 = f128_to_gf(p_msg[2 * j]);
            let f1 = f128_to_gf(p_msg[2 * j + 1]);
            let b0 = f128_to_gf(expect_b[2 * j]);
            let b1 = f128_to_gf(expect_b[2 * j + 1]);
            exp_u0 += f0 * b0;
            exp_u2 += (f0 + f1) * (b0 + b1);
        }

        let mut b = vec![F128::ZERO; n];
        let (u0, u2) = fill_phi_basis_round0(&mut b, &p_msg, &eq_his, &etas, &eq_r2);
        for (y, (got, want)) in b.iter().zip(expect_b.iter()).enumerate() {
            assert_eq!((got.lo, got.hi), (want.lo, want.hi), "basis slot {y}");
        }
        assert_eq!(u0, exp_u0, "u_0");
        assert_eq!(u2, exp_u2, "u_2");
    }
    // ── Structured-tap claim tests (EXPERIMENTAL API) ────────────────────

    use crate::taps::{TapOp, extract_virtual_tap_rows};

    /// W=1 tap test layout, `tw = 6` (pack cut at the column bit): 2 UAIR
    /// bit-columns over 2^14 trace rows; x tensor t' = 6, s = 8; group
    /// width g = 3 (8-bit words along the entry axis).
    fn tap_test_layout_tw6() -> ShaF2Layout {
        ShaF2Layout {
            p: IntEvalParams { t: 7, s: 8, word_bits: 1 },
            num_cols: 2,
            log_cols: 1,
            bit_vars: 0,
            num_vars: 14,
            tw: 6,
            x_fold_extra: 0,
        }
    }

    /// `tw = 9 > 7` (three-class offset splits): 2 bit-columns over 2^16
    /// trace rows; x tensor t' = 9, s = 7; g = 5 (32-bit words,
    /// off < 2^{s−g} = 4).
    fn tap_test_layout_tw9() -> ShaF2Layout {
        ShaF2Layout {
            p: IntEvalParams { t: 10, s: 7, word_bits: 1 },
            num_cols: 2,
            log_cols: 1,
            bit_vars: 0,
            num_vars: 16,
            tw: 9,
            x_fold_extra: 0,
        }
    }

    /// The layout's tap group width for the tests.
    fn tap_grp(layout: &ShaF2Layout) -> usize {
        if layout.tw == 9 { 5 } else { 3 }
    }

    /// The j = 2, k = 6 target instance of the structured-taps prompt
    /// (corrected semantics: 2^g-bit words along the entry axis of W=1
    /// bit-vectors): identity claims on both columns, two three-tap
    /// single-column rotation convolutions, a cross-column mix, and a
    /// lossy-SHIFT claim.
    fn tap_instance_claims(g: usize) -> Vec<Vec<TapOp>> {
        let rot =
            |col, amt, off| TapOp { col, grp_log2: g, bit_amt: amt, bit_dropout: false, off };
        let shl =
            |col, amt, off| TapOp { col, grp_log2: g, bit_amt: amt, bit_dropout: true, off };
        vec![
            vec![TapOp::ident(0)],
            vec![TapOp::ident(1)],
            vec![rot(0, 1, 0), rot(0, 2, 1), rot(0, 3, 2)],
            vec![rot(1, 2, 0), rot(1, 5, 1), rot(1, 7, 2)],
            vec![rot(0, 1, 0), rot(1, 4, 0), rot(0, 6, 1)],
            vec![shl(0, 3, 0), shl(1, 5, 1), rot(1, 2, 2)],
        ]
    }

    /// Direct 𝔽_q evaluation of a tapped claim from the committed rows.
    fn tap_expected_claim(
        layout: &ShaF2Layout,
        rows: &[Vec<u64>],
        taps: &[TapOp],
        rw: &[u128],
        colw: &[Fq],
    ) -> u128 {
        let a_rows = extract_virtual_tap_rows(layout, rows, taps);
        let mut y = Fq::from(0u128);
        for (c, row) in a_rows.iter().enumerate() {
            let mut acc = Fq::from(0u128);
            for (wi, &word) in row.iter().enumerate() {
                let mut bits = word;
                while bits != 0 {
                    let t = bits.trailing_zeros() as usize;
                    acc = acc + Fq::from(rw[(wi << 6) | t]);
                    bits &= bits.wrapping_sub(1);
                }
            }
            y = y + colw[c] * acc;
        }
        y.0
    }

    /// The j = 2, k = 6 structured-tap instance roundtrips on both
    /// supported pack-cut geometries, with per-claim row points.
    #[test]
    fn tap_claims_instance_roundtrips() {
        for layout in [tap_test_layout_tw6(), tap_test_layout_tw9()] {
            let p_x = virtual_xor_params(&layout);
            let alpha = smallest_generator();
            let (hint, pc, vc) = rlc_test_commit(&layout);
            let colw = rlc_test_col_weights(&p_x);
            let taps_all = tap_instance_claims(tap_grp(&layout));
            let rws: Vec<Vec<u128>> = (0..taps_all.len())
                .map(|i| rlc_test_row_weights(&p_x, 31 + i as u128))
                .collect();
            let claims: Vec<TapClaim<'_>> = taps_all
                .iter()
                .zip(rws.iter())
                .map(|(taps, rw)| TapClaim { taps, row_weights_q: rw })
                .collect();
            let mut pt = Blake3Transcript::new();
            let proof = prove_mle_eval_mod_q_ligerito_tap_claims(
                &mut pt, &hint, &layout, FQ_BITS, &claims, alpha, &pc,
            );
            let vclaims: Vec<TapVerifyClaim<'_, Fq>> = taps_all
                .iter()
                .zip(rws.iter())
                .map(|(taps, rw)| TapVerifyClaim {
                    taps,
                    row_weights_q: rw,
                    col_weights: &colw,
                    claimed: Fq::from(tap_expected_claim(&layout, hint.rows(), taps, rw, &colw)),
                })
                .collect();
            let mut vt = Blake3Transcript::new();
            verify_mle_eval_mod_q_ligerito_tap_claims(
                &mut vt, &hint.commitment, &proof, &layout, alpha, FQ_BITS, &vclaims, &vc,
            )
            .expect("tap instance verifies");
        }
    }

    /// Pure-ROT sub-instance (the rank-1 opening regime) on the tw = 9
    /// layout.
    #[test]
    fn tap_claims_pure_rot_roundtrips() {
        let layout = tap_test_layout_tw9();
        let g = tap_grp(&layout);
        let p_x = virtual_xor_params(&layout);
        let alpha = smallest_generator();
        let (hint, pc, vc) = rlc_test_commit(&layout);
        let colw = rlc_test_col_weights(&p_x);
        let rot =
            |col, amt| TapOp { col, grp_log2: g, bit_amt: amt, bit_dropout: false, off: 0 };
        let taps_all: Vec<Vec<TapOp>> =
            vec![vec![rot(0, 1), rot(0, 4)], vec![rot(1, 3), rot(0, 7)]];
        let rws: Vec<Vec<u128>> =
            (0..taps_all.len()).map(|i| rlc_test_row_weights(&p_x, 77 + i as u128)).collect();
        let claims: Vec<TapClaim<'_>> = taps_all
            .iter()
            .zip(rws.iter())
            .map(|(taps, rw)| TapClaim { taps, row_weights_q: rw })
            .collect();
        let mut pt = Blake3Transcript::new();
        let proof = prove_mle_eval_mod_q_ligerito_tap_claims(
            &mut pt, &hint, &layout, FQ_BITS, &claims, alpha, &pc,
        );
        let vclaims: Vec<TapVerifyClaim<'_, Fq>> = taps_all
            .iter()
            .zip(rws.iter())
            .map(|(taps, rw)| TapVerifyClaim {
                taps,
                row_weights_q: rw,
                col_weights: &colw,
                claimed: Fq::from(tap_expected_claim(&layout, hint.rows(), taps, rw, &colw)),
            })
            .collect();
        let mut vt = Blake3Transcript::new();
        verify_mle_eval_mod_q_ligerito_tap_claims(
            &mut vt, &hint.commitment, &proof, &layout, alpha, FQ_BITS, &vclaims, &vc,
        )
        .expect("pure-ROT taps verify");
    }

    /// 64-bit words (g = 6) with wide rotation amounts (≥ 32)
    /// roundtrip end-to-end — the tap machinery is width-generic
    /// (chains are O(g); the class structure is width-independent).
    #[test]
    fn tap_claims_g6_wide_amounts_roundtrip() {
        let layout = tap_test_layout_tw6(); // s = 8 ≥ g + 2
        let g = 6usize;
        let p_x = virtual_xor_params(&layout);
        let alpha = smallest_generator();
        let (hint, pc, vc) = rlc_test_commit(&layout);
        let colw = rlc_test_col_weights(&p_x);
        let rot =
            |col, amt, off| TapOp { col, grp_log2: g, bit_amt: amt, bit_dropout: false, off };
        let shl =
            |col, amt, off| TapOp { col, grp_log2: g, bit_amt: amt, bit_dropout: true, off };
        let taps_all: Vec<Vec<TapOp>> = vec![
            vec![rot(0, 33, 0)],
            vec![rot(0, 47, 1), rot(1, 9, 0)],
            vec![shl(1, 40, 0), rot(0, 63, 2)],
        ];
        let rws: Vec<Vec<u128>> = (0..taps_all.len())
            .map(|i| rlc_test_row_weights(&p_x, 421 + i as u128))
            .collect();
        let claims: Vec<TapClaim<'_>> = taps_all
            .iter()
            .zip(rws.iter())
            .map(|(taps, rw)| TapClaim { taps, row_weights_q: rw })
            .collect();
        let mut pt = Blake3Transcript::new();
        let proof = prove_mle_eval_mod_q_ligerito_tap_claims(
            &mut pt, &hint, &layout, FQ_BITS, &claims, alpha, &pc,
        );
        let vclaims: Vec<TapVerifyClaim<'_, Fq>> = taps_all
            .iter()
            .zip(rws.iter())
            .map(|(taps, rw)| TapVerifyClaim {
                taps,
                row_weights_q: rw,
                col_weights: &colw,
                claimed: Fq::from(tap_expected_claim(&layout, hint.rows(), taps, rw, &colw)),
            })
            .collect();
        let mut vt = Blake3Transcript::new();
        verify_mle_eval_mod_q_ligerito_tap_claims(
            &mut vt, &hint.commitment, &proof, &layout, alpha, FQ_BITS, &vclaims, &vc,
        )
        .expect("g=6 wide-amount taps verify");
    }

    /// Every tampered component of a tap proof is rejected.
    #[test]
    fn tap_claims_tampered_rejected() {
        let layout = tap_test_layout_tw9();
        let p_x = virtual_xor_params(&layout);
        let alpha = smallest_generator();
        let (hint, pc, vc) = rlc_test_commit(&layout);
        let colw = rlc_test_col_weights(&p_x);
        let taps_all = tap_instance_claims(tap_grp(&layout));
        let rws: Vec<Vec<u128>> =
            (0..taps_all.len()).map(|i| rlc_test_row_weights(&p_x, 131 + i as u128)).collect();
        let claims: Vec<TapClaim<'_>> = taps_all
            .iter()
            .zip(rws.iter())
            .map(|(taps, rw)| TapClaim { taps, row_weights_q: rw })
            .collect();
        let mut pt = Blake3Transcript::new();
        let proof = prove_mle_eval_mod_q_ligerito_tap_claims(
            &mut pt, &hint, &layout, FQ_BITS, &claims, alpha, &pc,
        );
        let good: Vec<u128> = taps_all
            .iter()
            .zip(rws.iter())
            .map(|(taps, rw)| tap_expected_claim(&layout, hint.rows(), taps, rw, &colw))
            .collect();
        let verify = |proof: &IntEvalRsLigModQTapProof, cs: &[u128], taps_all: &[Vec<TapOp>]| {
            let vclaims: Vec<TapVerifyClaim<'_, Fq>> = taps_all
                .iter()
                .zip(rws.iter())
                .zip(cs.iter())
                .map(|((taps, rw), &c)| TapVerifyClaim {
                    taps,
                    row_weights_q: rw,
                    col_weights: &colw,
                    claimed: Fq::from(c),
                })
                .collect();
            let mut vt = Blake3Transcript::new();
            verify_mle_eval_mod_q_ligerito_tap_claims(
                &mut vt, &hint.commitment, proof, &layout, alpha, FQ_BITS, &vclaims, &vc,
            )
        };
        assert!(verify(&proof, &good, &taps_all).is_ok(), "honest proof verifies");

        // Claimed value off by one.
        let mut bad = good.clone();
        bad[3] = (bad[3] + 1) % FQ_MOD;
        assert!(verify(&proof, &bad, &taps_all).is_err(), "wrong claimed value");

        // Statement mismatch: one tap's offset changed on the verifier side.
        let mut taps_bad = taps_all.clone();
        taps_bad[2][1].off = 2;
        assert!(verify(&proof, &good, &taps_bad).is_err(), "tap statement mismatch");

        // Tampered fold value.
        {
            let mut p2 = IntEvalRsLigModQTapProof {
                x_mfs: proof.x_mfs.clone(),
                x_presums: proof.x_presums.clone(),
                tap_us: proof.tap_us.clone(),
                rings: proof.rings.iter().map(|r| RingSwitchProof { s_v: r.s_v.clone() }).collect(),
                lig: proof.lig.clone(),
            };
            p2.tap_us[2][0][1] ^= 1;
            assert!(verify(&p2, &good, &taps_all).is_err(), "tampered fold");
        }
        // Tampered ring element (an offset-class ring: the last bucket of
        // claim 2's chunk-0 plan).
        {
            let mut p2 = IntEvalRsLigModQTapProof {
                x_mfs: proof.x_mfs.clone(),
                x_presums: proof.x_presums.clone(),
                tap_us: proof.tap_us.clone(),
                rings: proof.rings.iter().map(|r| RingSwitchProof { s_v: r.s_v.clone() }).collect(),
                lig: proof.lig.clone(),
            };
            let last = p2.rings.len() - 1;
            p2.rings[last].s_v[7] += Gf::one();
            assert!(verify(&p2, &good, &taps_all).is_err(), "tampered ring");
        }
        // Tampered presum.
        {
            let mut p2 = IntEvalRsLigModQTapProof {
                x_mfs: proof.x_mfs.clone(),
                x_presums: proof.x_presums.clone(),
                tap_us: proof.tap_us.clone(),
                rings: proof.rings.iter().map(|r| RingSwitchProof { s_v: r.s_v.clone() }).collect(),
                lig: proof.lig.clone(),
            };
            p2.x_presums[0] = rlc_tamper_mds(&p2.x_presums[0], 3);
            assert!(verify(&p2, &good, &taps_all).is_err(), "tampered presum");
        }
    }

    // ── Composed-collapse tests (EXPERIMENTAL API, tag 0x45) ─────────────

    /// Direct 𝔽_q evaluation of `outer(⊕ source-taps)`: extract the
    /// source, then walk the OUTER index map per output trace position —
    /// independent of the collapse's branch algebra. δ-aware: under
    /// `x_fold_extra` the extraction rows are re-split and the low-δ
    /// clear bits index the ROW weights.
    fn composed_expected_claim(
        layout: &ShaF2Layout,
        rows: &[Vec<u64>],
        source: &[TapOp],
        outer: &crate::taps::TapUniOp,
        rw: &[u128],
        colw: &[Fq],
    ) -> u128 {
        let x_rows = extract_virtual_tap_rows(layout, rows, source);
        let s = layout.p.s;
        let tw = layout.tw;
        let bv = layout.bit_vars;
        let delta = layout.x_fold_extra;
        let dmask = (1usize << delta) - 1;
        let g = outer.grp_log2;
        let n_g = 1usize << g;
        let mut y = Fq::from(0u128);
        for p in 0..1usize << layout.num_vars {
            let (k, j) = (p >> g, p & (n_g - 1));
            if k < outer.off {
                continue;
            }
            let src_j = if outer.bit_dropout {
                if j < outer.bit_amt {
                    continue;
                }
                j - outer.bit_amt
            } else if g == 0 {
                0
            } else {
                (j + n_g - outer.bit_amt) & (n_g - 1)
            };
            let q_trace = ((k - outer.off) << g) | src_j;
            let (q_hi, q_lo) = (q_trace >> s, q_trace & ((1 << s) - 1));
            let (p_hi, p_lo) = (p >> s, p & ((1 << s) - 1));
            for jm in 0..1usize << bv {
                let bitpos = ((q_lo & dmask) << (bv + tw)) | (jm << tw) | q_hi;
                if (x_rows[q_lo >> delta][bitpos >> 6] >> (bitpos & 63)) & 1 == 1 {
                    let widx = ((p_lo & dmask) << (bv + tw)) | (jm << tw) | p_hi;
                    y = y + Fq::from(rw[widx]) * colw[p_lo >> delta];
                }
            }
        }
        y.0
    }

    /// Schedule-shaped composed claims — `off^t` powers of ONE mixed
    /// source plus two claims on a second source — collapse to 2 + 2
    /// inner tap bodies and roundtrip on both pack-cut geometries.
    #[test]
    fn tap_composed_schedule_roundtrips() {
        for layout in [tap_test_layout_tw6(), tap_test_layout_tw9()] {
            let g = tap_grp(&layout);
            let p_x = virtual_xor_params(&layout);
            let alpha = smallest_generator();
            let (hint, pc, vc) = rlc_test_commit(&layout);
            let colw = rlc_test_col_weights(&p_x);
            let rw = rlc_test_row_weights(&p_x, 191);
            let rot =
                |col, amt, off| TapOp { col, grp_log2: g, bit_amt: amt, bit_dropout: false, off };
            let shl =
                |col, amt, off| TapOp { col, grp_log2: g, bit_amt: amt, bit_dropout: true, off };
            let uni = |amt: usize, dropout: bool, off: usize| crate::taps::TapUniOp {
                grp_log2: g,
                bit_amt: amt,
                bit_dropout: dropout,
                off,
            };
            // σ-style mixed source (rot + shifted + cross-column) and a
            // second two-tap rotation source.
            let src_a = vec![rot(0, 1, 0), rot(0, 3, 1), shl(0, 2, 0), rot(1, 0, 0)];
            let src_b = vec![rot(1, 2, 0), rot(0, 5, 0)];
            // The schedule: off^0..off^3 of src_a (off < 2^{s−g} = 4 on
            // the tw9 layout), a SHIFT-outer claim on src_a, and a
            // rot-outer + an off-outer claim on src_b.
            let shapes: Vec<(&[TapOp], crate::taps::TapUniOp)> = vec![
                (&src_a, uni(0, false, 0)),
                (&src_a, uni(0, false, 1)),
                (&src_a, uni(0, false, 2)),
                (&src_a, uni(0, false, 3)),
                (&src_a, uni(2, true, 0)),
                (&src_b, uni(4, false, 0)),
                (&src_b, uni(0, false, 1)),
            ];
            let claims: Vec<TapComposedClaim<'_>> = shapes
                .iter()
                .map(|(src, outer)| TapComposedClaim {
                    source: src,
                    outer: *outer,
                    claimed: composed_expected_claim(
                        &layout,
                        hint.rows(),
                        src,
                        outer,
                        &rw,
                        &colw,
                    ),
                })
                .collect();
            let mut pt = Blake3Transcript::new();
            let proof = prove_mle_eval_mod_q_ligerito_tap_composed(
                &mut pt, &hint, &layout, &rw, &colw, &claims, alpha, &pc,
            );
            // 7 claims, 2 sources, both with an offset claim → 4 inner
            // tap bodies (the schedule's whole point).
            assert_eq!(proof.tap_us.len(), 4, "2 sources × 2 branches");
            let mut vt = Blake3Transcript::new();
            verify_mle_eval_mod_q_ligerito_tap_composed(
                &mut vt, &hint.commitment, &proof, &layout, &rw, &colw, &claims, alpha, &vc,
            )
            .expect("composed schedule verifies");
        }
    }

    /// Distributed-extraction cross-check: the composed-outer evaluation
    /// equals the evaluation of the outer op FOLDED into each source tap
    /// (offsets add on the word field; rotation amounts add mod 2^g on
    /// rot-only sources), and the composed path proves values derived
    /// through that independent extraction route.
    #[test]
    fn tap_composed_matches_folded_taps() {
        let layout = tap_test_layout_tw9();
        let g = tap_grp(&layout);
        let p_x = virtual_xor_params(&layout);
        let alpha = smallest_generator();
        let (hint, pc, vc) = rlc_test_commit(&layout);
        let colw = rlc_test_col_weights(&p_x);
        let rw = rlc_test_row_weights(&p_x, 223);
        let rot =
            |col, amt, off| TapOp { col, grp_log2: g, bit_amt: amt, bit_dropout: false, off };
        let shl =
            |col, amt, off| TapOp { col, grp_log2: g, bit_amt: amt, bit_dropout: true, off };
        let uni = |amt: usize, dropout: bool, off: usize| crate::taps::TapUniOp {
            grp_log2: g,
            bit_amt: amt,
            bit_dropout: dropout,
            off,
        };
        // Pure-off outer over a mixed source: fold as off += o.
        let src_mixed = vec![rot(0, 7, 0), shl(0, 3, 0), rot(1, 0, 1)];
        for o in [1usize, 2] {
            let folded: Vec<TapOp> = src_mixed
                .iter()
                .map(|t| {
                    let mut t = *t;
                    t.off += o;
                    t
                })
                .collect();
            let via_fold = tap_expected_claim(&layout, hint.rows(), &folded, &rw, &colw);
            let via_outer = composed_expected_claim(
                &layout,
                hint.rows(),
                &src_mixed,
                &uni(0, false, o),
                &rw,
                &colw,
            );
            assert_eq!(via_fold, via_outer, "off^{o} composition");
        }
        // Rot-outer over a rot-only source: fold as amt += c (mod 2^g).
        let src_rot = vec![rot(0, 4, 0), rot(1, 9, 1)];
        let c = 3usize;
        let folded_rot: Vec<TapOp> = src_rot
            .iter()
            .map(|t| {
                let mut t = *t;
                t.bit_amt = (t.bit_amt + c) & ((1 << g) - 1);
                t
            })
            .collect();
        let via_fold = tap_expected_claim(&layout, hint.rows(), &folded_rot, &rw, &colw);
        let via_outer = composed_expected_claim(
            &layout,
            hint.rows(),
            &src_rot,
            &uni(c, false, 0),
            &rw,
            &colw,
        );
        assert_eq!(via_fold, via_outer, "rot^{c} composition");
        // End-to-end: prove the composed claims with the FOLDED-route
        // values.
        let shapes: Vec<(&[TapOp], crate::taps::TapUniOp, u128)> = vec![
            (
                &src_mixed,
                uni(0, false, 1),
                tap_expected_claim(
                    &layout,
                    hint.rows(),
                    &src_mixed
                        .iter()
                        .map(|t| {
                            let mut t = *t;
                            t.off += 1;
                            t
                        })
                        .collect::<Vec<_>>(),
                    &rw,
                    &colw,
                ),
            ),
            (&src_rot, uni(c, false, 0), via_fold),
        ];
        let claims: Vec<TapComposedClaim<'_>> = shapes
            .iter()
            .map(|&(source, outer, claimed)| TapComposedClaim { source, outer, claimed })
            .collect();
        let mut pt = Blake3Transcript::new();
        let proof = prove_mle_eval_mod_q_ligerito_tap_composed(
            &mut pt, &hint, &layout, &rw, &colw, &claims, alpha, &pc,
        );
        assert_eq!(proof.tap_us.len(), 3, "src_mixed both branches + src_rot branch 0");
        let mut vt = Blake3Transcript::new();
        verify_mle_eval_mod_q_ligerito_tap_composed(
            &mut vt, &hint.commitment, &proof, &layout, &rw, &colw, &claims, alpha, &vc,
        )
        .expect("folded-value composed claims verify");
    }

    /// The batched tap path under `x_fold_extra`: the k = 6 instance at
    /// δ = 1 and 2 — the fold vectors shrink 2^δ×; the translated-eq
    /// machinery is untouched (flat geometry).
    #[test]
    fn tap_claims_delta_roundtrips() {
        for delta in [1usize, 2] {
            let mut layout = tap_test_layout_tw6();
            layout.x_fold_extra = delta;
            let p_x = virtual_xor_params(&layout);
            let alpha = smallest_generator();
            let (hint, pc, vc) = rlc_test_commit(&layout);
            let colw = rlc_test_col_weights(&p_x);
            let taps_all = tap_instance_claims(tap_grp(&layout));
            let rws: Vec<Vec<u128>> = (0..taps_all.len())
                .map(|i| rlc_test_row_weights(&p_x, 313 + i as u128))
                .collect();
            let claims: Vec<TapClaim<'_>> = taps_all
                .iter()
                .zip(rws.iter())
                .map(|(taps, rw)| TapClaim { taps, row_weights_q: rw })
                .collect();
            let mut pt = Blake3Transcript::new();
            let proof = prove_mle_eval_mod_q_ligerito_tap_claims(
                &mut pt, &hint, &layout, FQ_BITS, &claims, alpha, &pc,
            );
            assert_eq!(
                proof.tap_us[0][0].len(),
                1usize << (layout.p.s - delta),
                "fold vectors shrink 2^δ×"
            );
            let vclaims: Vec<TapVerifyClaim<'_, Fq>> = taps_all
                .iter()
                .zip(rws.iter())
                .map(|(taps, rw)| TapVerifyClaim {
                    taps,
                    row_weights_q: rw,
                    col_weights: &colw,
                    claimed: Fq::from(tap_expected_claim(&layout, hint.rows(), taps, rw, &colw)),
                })
                .collect();
            let mut vt = Blake3Transcript::new();
            verify_mle_eval_mod_q_ligerito_tap_claims(
                &mut vt, &hint.commitment, &proof, &layout, alpha, FQ_BITS, &vclaims, &vc,
            )
            .unwrap_or_else(|e| panic!("δ={delta} tap instance verifies: {e:?}"));
        }
    }

    /// The composed collapse under `x_fold_extra = 2`: pure-off
    /// schedule outers plus a `2^δ | amt` rot outer roundtrip (fold
    /// vectors 4× smaller); an outer amount off the δ-envelope is
    /// rejected at the shape gate.
    #[test]
    fn tap_composed_delta_roundtrips() {
        let mut layout = tap_test_layout_tw6();
        layout.x_fold_extra = 2;
        let g = tap_grp(&layout);
        let p_x = virtual_xor_params(&layout);
        let alpha = smallest_generator();
        let (hint, pc, vc) = rlc_test_commit(&layout);
        let colw = rlc_test_col_weights(&p_x);
        let rw = rlc_test_row_weights(&p_x, 349);
        let rot =
            |col, amt, off| TapOp { col, grp_log2: g, bit_amt: amt, bit_dropout: false, off };
        let shl =
            |col, amt, off| TapOp { col, grp_log2: g, bit_amt: amt, bit_dropout: true, off };
        let uni = |amt: usize, off: usize| crate::taps::TapUniOp {
            grp_log2: g,
            bit_amt: amt,
            bit_dropout: false,
            off,
        };
        let src = vec![rot(0, 1, 0), shl(1, 5, 1)];
        // δ = 2, g = 3: outer amounts must be multiples of 4.
        let outers = [uni(0, 0), uni(0, 1), uni(0, 3), uni(4, 0)];
        let claims: Vec<TapComposedClaim<'_>> = outers
            .iter()
            .map(|outer| TapComposedClaim {
                source: &src,
                outer: *outer,
                claimed: composed_expected_claim(
                    &layout,
                    hint.rows(),
                    &src,
                    outer,
                    &rw,
                    &colw,
                ),
            })
            .collect();
        let mut pt = Blake3Transcript::new();
        let proof = prove_mle_eval_mod_q_ligerito_tap_composed(
            &mut pt, &hint, &layout, &rw, &colw, &claims, alpha, &pc,
        );
        assert_eq!(proof.tap_us.len(), 2, "one source, both branches");
        assert_eq!(proof.tap_us[0][0].len(), 1usize << (layout.p.s - 2));
        let mut vt = Blake3Transcript::new();
        verify_mle_eval_mod_q_ligerito_tap_composed(
            &mut vt, &hint.commitment, &proof, &layout, &rw, &colw, &claims, alpha, &vc,
        )
        .expect("δ=2 composed schedule verifies");
        // amt = 2 is not a multiple of 2^δ = 4 → shape-gate rejection.
        let mut bad = claims.clone();
        bad[3].outer.bit_amt = 2;
        let mut vt = Blake3Transcript::new();
        assert!(
            verify_mle_eval_mod_q_ligerito_tap_composed(
                &mut vt, &hint.commitment, &proof, &layout, &rw, &colw, &bad, alpha, &vc,
            )
            .is_err(),
            "off-envelope outer amount rejected"
        );
    }

    /// Every tampered component of a composed-collapse statement or
    /// proof is rejected.
    #[test]
    fn tap_composed_tampered_rejected() {
        let layout = tap_test_layout_tw6();
        let g = tap_grp(&layout);
        let p_x = virtual_xor_params(&layout);
        let alpha = smallest_generator();
        let (hint, pc, vc) = rlc_test_commit(&layout);
        let colw = rlc_test_col_weights(&p_x);
        let rw = rlc_test_row_weights(&p_x, 251);
        let rot =
            |col, amt, off| TapOp { col, grp_log2: g, bit_amt: amt, bit_dropout: false, off };
        let uni = |amt: usize, off: usize| crate::taps::TapUniOp {
            grp_log2: g,
            bit_amt: amt,
            bit_dropout: false,
            off,
        };
        let src = vec![rot(0, 2, 0), rot(1, 5, 1)];
        let outers = [uni(0, 0), uni(0, 1), uni(3, 0)];
        let claims: Vec<TapComposedClaim<'_>> = outers
            .iter()
            .map(|outer| TapComposedClaim {
                source: &src,
                outer: *outer,
                claimed: composed_expected_claim(
                    &layout,
                    hint.rows(),
                    &src,
                    outer,
                    &rw,
                    &colw,
                ),
            })
            .collect();
        let mut pt = Blake3Transcript::new();
        let proof = prove_mle_eval_mod_q_ligerito_tap_composed(
            &mut pt, &hint, &layout, &rw, &colw, &claims, alpha, &pc,
        );
        let verify = |proof: &IntEvalRsLigModQTapProof, claims: &[TapComposedClaim<'_>]| {
            let mut vt = Blake3Transcript::new();
            verify_mle_eval_mod_q_ligerito_tap_composed(
                &mut vt, &hint.commitment, proof, &layout, &rw, &colw, claims, alpha, &vc,
            )
        };
        assert!(verify(&proof, &claims).is_ok(), "honest proof verifies");

        // Claimed value off by one.
        let mut bad = claims.clone();
        bad[1].claimed = (bad[1].claimed + 1) % FQ_MOD;
        assert!(verify(&proof, &bad).is_err(), "wrong claimed value");

        // Outer op changed on the verifier side.
        let mut bad = claims.clone();
        bad[2].outer.bit_amt = 4;
        assert!(verify(&proof, &bad).is_err(), "outer-op statement mismatch");

        // Source tap changed on the verifier side.
        let src_bad = vec![rot(0, 3, 0), rot(1, 5, 1)];
        let mut bad = claims.clone();
        bad[0].source = &src_bad;
        assert!(verify(&proof, &bad).is_err(), "source statement mismatch");

        // Tampered fold value.
        {
            let mut p2 = IntEvalRsLigModQTapProof {
                x_mfs: proof.x_mfs.clone(),
                x_presums: proof.x_presums.clone(),
                tap_us: proof.tap_us.clone(),
                rings: proof
                    .rings
                    .iter()
                    .map(|r| RingSwitchProof { s_v: r.s_v.clone() })
                    .collect(),
                lig: proof.lig.clone(),
            };
            p2.tap_us[1][0][2] ^= 1;
            assert!(verify(&p2, &claims).is_err(), "tampered fold");
        }
        // Tampered ring element.
        {
            let mut p2 = IntEvalRsLigModQTapProof {
                x_mfs: proof.x_mfs.clone(),
                x_presums: proof.x_presums.clone(),
                tap_us: proof.tap_us.clone(),
                rings: proof
                    .rings
                    .iter()
                    .map(|r| RingSwitchProof { s_v: r.s_v.clone() })
                    .collect(),
                lig: proof.lig.clone(),
            };
            p2.rings[0].s_v[19] += Gf::one();
            assert!(verify(&p2, &claims).is_err(), "tampered ring");
        }
    }

    // ── Stream-family tests (EXPERIMENTAL API) ───────────────────────────

    /// The pinned two-cluster split of the k = 6 instance:
    /// `{b1, b3, b5}` over 6 streams and `{b2, b4, b6}` over 7.
    #[allow(clippy::type_complexity)]
    fn tap_family_instance(g: usize) -> (Vec<Vec<TapOp>>, Vec<Vec<usize>>, Vec<Vec<usize>>) {
        let rot =
            |col, amt, off| TapOp { col, grp_log2: g, bit_amt: amt, bit_dropout: false, off };
        let shl =
            |col, amt, off| TapOp { col, grp_log2: g, bit_amt: amt, bit_dropout: true, off };
        // Cluster streams (deduped: S1 = rot(0,1,0) shared by b3/b5).
        let streams1 =
            vec![TapOp::ident(0), rot(0, 1, 0), rot(0, 2, 1), rot(0, 3, 2), rot(1, 4, 0), rot(0, 6, 1)];
        let streams2 = vec![
            TapOp::ident(1),
            rot(1, 2, 0),
            rot(1, 5, 1),
            rot(1, 7, 2),
            shl(0, 3, 0),
            shl(1, 5, 1),
            rot(1, 2, 2),
        ];
        // Forms over the cluster streams; claim order [b1, b3, b5] / [b2, b4, b6].
        let forms1 = vec![0b000001usize, 0b001110, 0b110010];
        let forms2 = vec![0b0000001usize, 0b0001110, 0b1110000];
        (vec![streams1, streams2], vec![forms1, forms2], vec![vec![0, 2, 4], vec![1, 3, 5]])
    }

    /// The clustered stream family proves EXACTLY the instance's claim
    /// values (cross-checked against the extraction-based evaluation the
    /// tap-claims baseline proves), on both pack-cut geometries.
    #[test]
    fn tap_family_instance_roundtrips() {
        for layout in [tap_test_layout_tw6(), tap_test_layout_tw9()] {
            let p_x = virtual_xor_params(&layout);
            let alpha = smallest_generator();
            let (hint, pc, vc) = rlc_test_commit(&layout);
            let colw = rlc_test_col_weights(&p_x);
            let claim_taps = tap_instance_claims(tap_grp(&layout));
            let rws: Vec<Vec<u128>> = (0..claim_taps.len())
                .map(|i| rlc_test_row_weights(&p_x, 57 + i as u128))
                .collect();
            let expected: Vec<u128> = claim_taps
                .iter()
                .zip(rws.iter())
                .map(|(taps, rw)| tap_expected_claim(&layout, hint.rows(), taps, rw, &colw))
                .collect();
            let (streams, forms, members) = tap_family_instance(tap_grp(&layout));
            let cluster_claims: Vec<Vec<RlcFamilyClaim<'_>>> = (0..2)
                .map(|ci| {
                    forms[ci]
                        .iter()
                        .zip(members[ci].iter())
                        .map(|(&form, &bi)| RlcFamilyClaim {
                            form,
                            row_weights_q: &rws[bi],
                            claimed: expected[bi],
                        })
                        .collect()
                })
                .collect();
            let clusters: Vec<TapFamilyCluster<'_>> = (0..2)
                .map(|ci| TapFamilyCluster { streams: &streams[ci], claims: &cluster_claims[ci] })
                .collect();
            let mut pt = Blake3Transcript::new();
            let proof = prove_mle_eval_mod_q_ligerito_tap_family(
                &mut pt, &hint, &layout, &clusters, alpha, &pc,
            );
            let mut vt = Blake3Transcript::new();
            verify_mle_eval_mod_q_ligerito_tap_family(
                &mut vt, &hint.commitment, &proof, &layout, &clusters, &colw, alpha, &vc,
            )
            .expect("stream family verifies");
        }
    }

    /// A pure-XOR stream cluster (every claim one XOR form): the AND
    /// channel vanishes identically and must be elided — no discharge, two
    /// singleton rings, honest proof accepted.
    #[test]
    fn tap_family_pure_xor_elides() {
        let layout = tap_test_layout_tw9();
        let p_x = virtual_xor_params(&layout);
        let alpha = smallest_generator();
        let (hint, pc, vc) = rlc_test_commit(&layout);
        let colw = rlc_test_col_weights(&p_x);
        let g = tap_grp(&layout);
        let streams = [
            TapOp { col: 0, grp_log2: g, bit_amt: 1, bit_dropout: false, off: 0 },
            TapOp { col: 1, grp_log2: g, bit_amt: 3, bit_dropout: false, off: 1 },
        ];
        let taps: Vec<TapOp> = streams.to_vec();
        let rw = rlc_test_row_weights(&p_x, 213);
        let claimed = tap_expected_claim(&layout, hint.rows(), &taps, &rw, &colw);
        let claims = [RlcFamilyClaim { form: 0b11, row_weights_q: &rw, claimed }];
        let clusters = [TapFamilyCluster { streams: &streams, claims: &claims }];
        let mut pt = Blake3Transcript::new();
        let proof =
            prove_mle_eval_mod_q_ligerito_tap_family(&mut pt, &hint, &layout, &clusters, alpha, &pc);
        assert!(
            proof.clusters[0].discharge_eqf.is_none(),
            "pure-XOR cluster has no monomial channels"
        );
        assert_eq!(proof.rings.len(), 1 + 3, "two streams: 1 class (off 0) + 3 (off 1)");
        let mut vt = Blake3Transcript::new();
        verify_mle_eval_mod_q_ligerito_tap_family(
            &mut vt, &hint.commitment, &proof, &layout, &clusters, &colw, alpha, &vc,
        )
        .expect("pure-XOR stream family verifies");
    }

    /// k uniform-op claims (`op(⊕ cols)`) at ONE shared point collapse to
    /// ≤ #distinct-XOR-sets × 2 plain virtual-XOR claims; values are
    /// cross-checked against the extraction-based evaluation of the
    /// DISTRIBUTED tap lists (`op(⊕ a_i) = ⊕ op(a_i)` — which
    /// independently pins the weight-transform identity), on both
    /// pack-cut geometries.
    #[test]
    fn tap_collapse_roundtrips() {
        use crate::taps::TapUniOp;
        for layout in [tap_test_layout_tw6(), tap_test_layout_tw9()] {
            let g = tap_grp(&layout);
            let p_x = virtual_xor_params(&layout);
            let alpha = smallest_generator();
            let (hint, pc, vc) = rlc_test_commit(&layout);
            let colw = rlc_test_col_weights(&p_x);
            let rw = rlc_test_row_weights(&p_x, 91);
            let rot =
                |amt, off| TapUniOp { grp_log2: g, bit_amt: amt, bit_dropout: false, off };
            let shl = |amt, off| TapUniOp { grp_log2: g, bit_amt: amt, bit_dropout: true, off };
            // (XOR set, op): singleton sets + ROT^c(a₀⊕a₁)-style claims.
            let spec: Vec<(Vec<usize>, TapUniOp)> = vec![
                (vec![0], TapUniOp::ident()),
                (vec![1], TapUniOp::ident()),
                (vec![0], rot(1, 0)),
                (vec![1], rot(3, 0)),
                (vec![0], shl(2, 0)),
                (vec![0, 1], rot(4, 0)),
                (vec![0], rot(5, 1)),
                (vec![1], rot(2, 2)),
                (vec![1], rot(0, 3)),
                (vec![0, 1], rot(6, 1)),
            ];
            let mk_claims = |count: usize| -> Vec<TapPointClaim<'_>> {
                spec[..count]
                    .iter()
                    .map(|(set, op)| {
                        let taps: Vec<TapOp> =
                            set.iter().map(|&c| op.with_col(c)).collect();
                        TapPointClaim {
                            cols: set,
                            op: *op,
                            claimed: tap_expected_claim(
                                &layout,
                                hint.rows(),
                                &taps,
                                &rw,
                                &colw,
                            ),
                        }
                    })
                    .collect()
            };
            let claims = mk_claims(spec.len());
            let mut pt = Blake3Transcript::new();
            let proof = prove_mle_eval_mod_q_ligerito_tap_collapse(
                &mut pt, &hint, &layout, &rw, &colw, &claims, alpha, &pc,
            );
            assert_eq!(proof.xors.len(), 6, "3 XOR sets × 2 branches (offsets on all)");
            let mut vt = Blake3Transcript::new();
            verify_mle_eval_mod_q_ligerito_tap_collapse(
                &mut vt, &hint.commitment, &proof, &layout, &rw, &colw, &claims, alpha, &vc,
            )
            .expect("collapse verifies");

            // No-offset subset: one inner claim per XOR set.
            let claims2 = mk_claims(6);
            let mut pt = Blake3Transcript::new();
            let proof2 = prove_mle_eval_mod_q_ligerito_tap_collapse(
                &mut pt, &hint, &layout, &rw, &colw, &claims2, alpha, &pc,
            );
            assert_eq!(proof2.xors.len(), 3, "no offsets: branch 0 only per XOR set");
            let mut vt = Blake3Transcript::new();
            verify_mle_eval_mod_q_ligerito_tap_collapse(
                &mut vt, &hint.commitment, &proof2, &layout, &rw, &colw, &claims2, alpha, &vc,
            )
            .expect("no-offset collapse verifies");
        }
    }

    /// The multiweight collapse (0x46): per-claim column weights —
    /// masks, place values, permuted patterns — γ-merge into the same
    /// per-(set, branch) bodies; two claims on the SAME set with
    /// different weights share one body.
    #[test]
    fn tap_multiweight_roundtrips() {
        use crate::taps::TapUniOp;
        for layout in [tap_test_layout_tw6(), tap_test_layout_tw9()] {
            let g = tap_grp(&layout);
            let p_x = virtual_xor_params(&layout);
            let alpha = smallest_generator();
            let (hint, pc, vc) = rlc_test_commit(&layout);
            let rw = rlc_test_row_weights(&p_x, 401);
            let w_eq = rlc_test_col_weights(&p_x);
            // Place-value-flavored, parity-masked, and permuted weight
            // vectors — the P-LIN layer's read patterns.
            let w_pv: Vec<Fq> =
                (0..p_x.cols()).map(|c| Fq::from(1u128 << (c % 20))).collect();
            let w_mask: Vec<Fq> = (0..p_x.cols())
                .map(|c| if c % 2 == 0 { w_eq[c] } else { Fq::from(0u128) })
                .collect();
            let w_perm: Vec<Fq> =
                (0..p_x.cols()).map(|c| w_eq[(c + 3) % p_x.cols()]).collect();
            let rot =
                |amt, off| TapUniOp { grp_log2: g, bit_amt: amt, bit_dropout: false, off };
            let spec: Vec<(Vec<usize>, TapUniOp, &[Fq])> = vec![
                (vec![0], TapUniOp::ident(), &w_eq),
                (vec![0], TapUniOp::ident(), &w_pv),
                (vec![0], rot(0, 1), &w_mask),
                (vec![1], TapUniOp::ident(), &w_perm),
                (vec![0, 1], rot(4, 0), &w_eq),
            ];
            let claims: Vec<TapWeightedClaim<'_>> = spec
                .iter()
                .map(|(set, op, cw)| {
                    let taps: Vec<TapOp> = set.iter().map(|&c| op.with_col(c)).collect();
                    TapWeightedClaim {
                        cols: set,
                        op: *op,
                        col_weights: cw,
                        claimed: tap_expected_claim(&layout, hint.rows(), &taps, &rw, cw),
                    }
                })
                .collect();
            let mut pt = Blake3Transcript::new();
            let proof = prove_mle_eval_mod_q_ligerito_tap_multiweight(
                &mut pt, &hint, &layout, &rw, &claims, alpha, &pc,
            );
            assert_eq!(
                proof.xors.len(),
                4,
                "{{0}}×2 branches + {{1}} + {{0,1}} — same-set claims share bodies"
            );
            let mut vt = Blake3Transcript::new();
            verify_mle_eval_mod_q_ligerito_tap_multiweight(
                &mut vt, &hint.commitment, &proof, &layout, &rw, &claims, alpha, &vc,
            )
            .expect("multiweight collapse verifies");

            // Tampers: swapped weight vectors between two same-shape
            // claims, and a wrong claimed value.
            let mut bad = claims.clone();
            bad[0].col_weights = &w_pv;
            bad[1].col_weights = &w_eq;
            let mut vt = Blake3Transcript::new();
            assert!(
                verify_mle_eval_mod_q_ligerito_tap_multiweight(
                    &mut vt, &hint.commitment, &proof, &layout, &rw, &bad, alpha, &vc,
                )
                .is_err(),
                "swapped per-claim weights rejected"
            );
            let mut bad = claims.clone();
            bad[3].claimed = (bad[3].claimed + 1) % FQ_MOD;
            let mut vt = Blake3Transcript::new();
            assert!(
                verify_mle_eval_mod_q_ligerito_tap_multiweight(
                    &mut vt, &hint.commitment, &proof, &layout, &rw, &bad, alpha, &vc,
                )
                .is_err(),
                "wrong claimed value rejected"
            );
        }
    }

    /// Collapse tampers: wrong claimed value, tampered fold, and a
    /// statement tap mismatch are all rejected.
    #[test]
    fn tap_collapse_tampered_rejected() {
        let layout = tap_test_layout_tw9();
        let g = tap_grp(&layout);
        let p_x = virtual_xor_params(&layout);
        let alpha = smallest_generator();
        let (hint, pc, vc) = rlc_test_commit(&layout);
        let colw = rlc_test_col_weights(&p_x);
        let rw = rlc_test_row_weights(&p_x, 143);
        use crate::taps::TapUniOp;
        let rot = |amt, off| TapUniOp { grp_log2: g, bit_amt: amt, bit_dropout: false, off };
        let spec: Vec<(Vec<usize>, TapUniOp)> = vec![
            (vec![0], TapUniOp::ident()),
            (vec![0], rot(3, 1)),
            (vec![0, 1], rot(6, 0)),
            (vec![1], rot(1, 2)),
        ];
        let claims: Vec<TapPointClaim<'_>> = spec
            .iter()
            .map(|(set, op)| {
                let taps: Vec<TapOp> = set.iter().map(|&c| op.with_col(c)).collect();
                TapPointClaim {
                    cols: set,
                    op: *op,
                    claimed: tap_expected_claim(&layout, hint.rows(), &taps, &rw, &colw),
                }
            })
            .collect();
        let mut pt = Blake3Transcript::new();
        let proof = prove_mle_eval_mod_q_ligerito_tap_collapse(
            &mut pt, &hint, &layout, &rw, &colw, &claims, alpha, &pc,
        );
        let verify = |proof: &IntEvalRsLigModQXorProof, claims: &[TapPointClaim<'_>]| {
            let mut vt = Blake3Transcript::new();
            verify_mle_eval_mod_q_ligerito_tap_collapse(
                &mut vt, &hint.commitment, proof, &layout, &rw, &colw, claims, alpha, &vc,
            )
        };
        assert!(verify(&proof, &claims).is_ok(), "honest proof verifies");

        let mut bad = claims.clone();
        bad[1].claimed = (bad[1].claimed + 1) % FQ_MOD;
        assert!(verify(&proof, &bad).is_err(), "wrong claimed value");

        let mut bad = claims.clone();
        bad[3].op.off = 1;
        assert!(verify(&proof, &bad).is_err(), "op statement mismatch");

        let mut bad = claims.clone();
        bad[2].cols = &spec[3].0;
        assert!(verify(&proof, &bad).is_err(), "XOR-set statement mismatch");

        {
            let mut p2 = IntEvalRsLigModQXorProof {
                mfs: Vec::new(),
                us: Vec::new(),
                presums: Vec::new(),
                x_mfs: proof.x_mfs.clone(),
                x_presums: proof.x_presums.clone(),
                xors: proof
                    .xors
                    .iter()
                    .map(|xs| VirtXorSide { us: xs.us.clone(), externals: xs.externals.clone() })
                    .collect(),
                rings: proof
                    .rings
                    .iter()
                    .map(|r| RingSwitchProof { s_v: r.s_v.clone() })
                    .collect(),
                lig: proof.lig.clone(),
            };
            p2.xors[2].us[0][1] ^= 1;
            assert!(verify(&p2, &claims).is_err(), "tampered fold");
        }
    }

    /// Every tampered component of a stream-family proof is rejected.
    #[test]
    fn tap_family_tampered_rejected() {
        let layout = tap_test_layout_tw9();
        let p_x = virtual_xor_params(&layout);
        let alpha = smallest_generator();
        let (hint, pc, vc) = rlc_test_commit(&layout);
        let colw = rlc_test_col_weights(&p_x);
        let claim_taps = tap_instance_claims(tap_grp(&layout));
        let rws: Vec<Vec<u128>> = (0..claim_taps.len())
            .map(|i| rlc_test_row_weights(&p_x, 87 + i as u128))
            .collect();
        let expected: Vec<u128> = claim_taps
            .iter()
            .zip(rws.iter())
            .map(|(taps, rw)| tap_expected_claim(&layout, hint.rows(), taps, rw, &colw))
            .collect();
        let (streams, forms, members) = tap_family_instance(tap_grp(&layout));
        let mk_claims = |vals: &[u128]| -> Vec<Vec<RlcFamilyClaim<'_>>> {
            (0..2)
                .map(|ci| {
                    forms[ci]
                        .iter()
                        .zip(members[ci].iter())
                        .map(|(&form, &bi)| RlcFamilyClaim {
                            form,
                            row_weights_q: &rws[bi],
                            claimed: vals[bi],
                        })
                        .collect()
                })
                .collect()
        };
        let good_claims = mk_claims(&expected);
        let clusters: Vec<TapFamilyCluster<'_>> = (0..2)
            .map(|ci| TapFamilyCluster { streams: &streams[ci], claims: &good_claims[ci] })
            .collect();
        let mut pt = Blake3Transcript::new();
        let proof =
            prove_mle_eval_mod_q_ligerito_tap_family(&mut pt, &hint, &layout, &clusters, alpha, &pc);
        let clone_proof = |p: &IntEvalRsLigTapFamilyProof| IntEvalRsLigTapFamilyProof {
            clusters: p
                .clusters
                .iter()
                .map(|s| TapFamilyClusterSide {
                    mfs: s.mfs.clone(),
                    us: s.us.clone(),
                    presums: s.presums.clone(),
                    discharge_eqf: s.discharge_eqf.clone(),
                    omegas: s.omegas.clone(),
                    discharge_eqf2: s.discharge_eqf2.clone(),
                    omegas2: s.omegas2.clone(),
                })
                .collect(),
            rings: p.rings.iter().map(|r| RingSwitchProof { s_v: r.s_v.clone() }).collect(),
            lig: p.lig.clone(),
        };
        let verify_with = |p: &IntEvalRsLigTapFamilyProof, vals: &[u128]| {
            let cl = mk_claims(vals);
            let cls: Vec<TapFamilyCluster<'_>> = (0..2)
                .map(|ci| TapFamilyCluster { streams: &streams[ci], claims: &cl[ci] })
                .collect();
            let mut vt = Blake3Transcript::new();
            verify_mle_eval_mod_q_ligerito_tap_family(
                &mut vt, &hint.commitment, p, &layout, &cls, &colw, alpha, &vc,
            )
        };
        assert!(verify_with(&proof, &expected).is_ok(), "honest proof verifies");

        let mut bad_vals = expected.clone();
        bad_vals[4] = (bad_vals[4] + 1) % FQ_MOD;
        assert!(verify_with(&proof, &bad_vals).is_err(), "wrong claimed value");

        {
            let mut p2 = clone_proof(&proof);
            p2.clusters[1].us[0][2] ^= 1;
            assert!(verify_with(&p2, &expected).is_err(), "tampered fold");
        }
        {
            let mut p2 = clone_proof(&proof);
            let last = p2.rings.len() - 1;
            p2.rings[last].s_v[19] += Gf::one();
            assert!(verify_with(&p2, &expected).is_err(), "tampered ring");
        }
        {
            let mut p2 = clone_proof(&proof);
            p2.clusters[0].omegas[1] += Gf::one();
            assert!(verify_with(&p2, &expected).is_err(), "tampered omega");
        }
        {
            let mut p2 = clone_proof(&proof);
            p2.clusters[1].omegas2[0] += Gf::one();
            assert!(verify_with(&p2, &expected).is_err(), "tampered omega2");
        }
        {
            let mut p2 = clone_proof(&proof);
            p2.clusters[0].presums[0] = rlc_tamper_mds(&p2.clusters[0].presums[0], 5);
            assert!(verify_with(&p2, &expected).is_err(), "tampered presum");
        }
        {
            let mut p2 = clone_proof(&proof);
            let d = p2.clusters[0].discharge_eqf.as_mut().expect("cascade present");
            d.sc_a = rlc_tamper_sc(&d.sc_a, 2);
            assert!(verify_with(&p2, &expected).is_err(), "tampered discharge A");
        }
        {
            let mut p2 = clone_proof(&proof);
            let d = p2.clusters[1].discharge_eqf2.as_mut().expect("level 2 present");
            d.betas[0] += Gf::one();
            assert!(verify_with(&p2, &expected).is_err(), "tampered level-2 betas");
        }
    }
}

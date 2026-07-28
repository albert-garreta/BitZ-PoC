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
use flock_core::merkle::Hash;
use flock_core::ntt::additive_ntt_f128::AdditiveNttF128;
use flock_core::pcs::basefold::{
    self, BaseFoldProof as FlockBaseFoldProof, VerifyError as FlockVerifyError,
    default_fri_queries,
};
use flock_core::pcs::commit::{Commitment, PcsParams, ProverData, commit};
use flock_core::pcs::ligerito::{
    self, LigeritoProof, LigeritoSecurityConfig, ProverConfig as LigProverConfig,
    VerifierConfig as LigVerifierConfig,
};

use crate::piop::lookup::gkr_product::ProductForestProof;
use crate::piop::sumcheck::multi_degree::MultiDegreeSumcheckProof;
use crate::poly::univariate::binary_gf128::BinaryFieldGF128 as Gf;
use crate::transcript::traits::Transcript;

use crate::pcs::{IntEvalParams, ShaF2Layout, final_eval_ring};
use crate::ligerito::{
    IntEvalRsError, LOG_PACKING, RingSwitchProof, RsOpenConfig, RsOpenError, packed_vars,
    prove_int_eval_common, prove_int_eval_merged_common, prove_x_claims_batched_common,
    repack_leaf_bits, residual_b_evals, ring_switch_prove, ring_switch_verify, row_bit_vars,
    tensor_eq_phi_eval, verify_int_eval_common, verify_int_eval_merged_common,
    verify_x_claims_batched_common,
};
use crate::merged_forest::MergedForestProof;
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

fn gf_slice_to_f128(v: &[Gf]) -> Vec<F128> {
    v.iter().map(|&g| gf_to_f128(g)).collect()
}

const MLE_EVAL_MOD_Q_TRANSCRIPT_LABEL: &[u8] = b"f2z-pcs/mle-eval-mod-q/v1";

fn absorb_mle_eval_mod_q_root(transcript: &mut impl Transcript, root: &Hash) {
    transcript.absorb_slice(MLE_EVAL_MOD_Q_TRANSCRIPT_LABEL);
    transcript.absorb_slice(root);
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
    let slim = ligerito::embedded_security_config(m, ligerito::LigeritoProfile::Slim)
        .unwrap_or_else(|| panic!("no embedded slim template for m={m}"));
    let mut cfg = LigeritoSecurityConfig::from_toml_str(slim).expect("slim template validates");
    let log_n = cfg.log_n;
    assert!(k0 >= 1 && k0 < log_n, "custom initial_k out of range");
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
            // OOD must clear the target on its own (L0 uses the implicit
            // post-commit binding, s = 0; deeper levels escalate samples).
            loop {
                let ood = lv.paper_predicted_ood_bits().expect("johnson_ood prediction");
                if ood + 1e-3 >= lv.target_security_bits as f64 {
                    lv.expected_eps_ood_bits = Some(ood);
                    break;
                }
                lv.ood_samples += 1;
            }
            lv
        })
        .collect();
    cfg.validate().expect("custom config passes flock's validator");
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
    let mut target = Gf::zero();
    for ell in 0..l {
        for (y, &ev) in eq_his[ell].iter().enumerate() {
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
            b_comb[ell * slice + y] = gf_to_f128(etas[ell] * acc);
        }
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
/// chunk-parallel with per-chunk accumulators.
#[allow(clippy::arithmetic_side_effects)]
fn dense_ring_sv(p_msg: &[F128], eq_hi: &[Gf]) -> Vec<Gf> {
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
#[allow(clippy::arithmetic_side_effects)]
fn fill_phi_basis(b: &mut [F128], eq_his: &[Vec<Gf>], etas: &[Gf], eq_r2: &[Gf]) {
    const CHUNK: usize = 1 << 12;
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

    absorb_mle_eval_mod_q_root(transcript, hint.root());
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
    fill_phi_basis(&mut b_comb, &eq_his, &etas, &eq_r2);
    let mut target = Gf::zero();
    for l in 0..lch {
        let s_u = crate::ligerito::transpose_bits_128(&rings[l].s_v);
        let beta = s_u.iter().zip(eq_r2.iter()).fold(Gf::zero(), |a, (su, e)| a + *su * *e);
        target += etas[l] * beta;
    }
    drop(_g_b);

    let _g_l = crate::utils::prof::scope("mq:lig");
    let lig = ligerito::recursive_prover_with_basis(
        pc,
        hint.p_msg.clone(),
        b_comb,
        gf_to_f128(target),
        &hint.prover_data.codeword,
        &hint.prover_data.merkle_tree,
        &mut ZincChallenger(transcript),
    );
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
    use crate::pcs::{
        chunk_row_weights, mod_q_chunk_width, mod_q_num_chunks, recombine_read_off,
    };

    absorb_mle_eval_mod_q_root(transcript, &commitment.root);
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

    // Per-chunk range bound 2^{c_w+t+W} (c_w+t+W = 127 by construction).
    let range_shift = c_w.wrapping_add(p.t).wrapping_add(p.word_bits);
    let bound = 1u128 << range_shift;

    let mut points = Vec::with_capacity(lch);
    let mut mus = Vec::with_capacity(lch);
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

    // Recombine in R: y = Σ_c w′_c · Σ_l 2^{c_w·l}·u_c^{(l)}.
    let v_flat: Vec<u128> = proof.us.iter().flat_map(|u| u.iter().copied()).collect();
    let y = recombine_read_off(p, &v_flat, 0, col_weights, c_w, lch);
    if y != claimed {
        return Err(FlockRsError::Common(IntEvalRsError::ReadOff));
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
fn embed_xor_index(layout: &ShaF2Layout, idx_x: usize, i_col: usize) -> usize {
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
        b.accumulate(&zinc_side_size_breakdown_merged(
            &proof.mfs[l],
            &proof.us[l],
            &proof.presums[l],
        ));
    }
    (b, proof.lig.size_bytes())
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
                match &layer.sc_x {
                    None => w.len(0),
                    Some(sc) => {
                        w.len(1);
                        w.transcribable(sc);
                    }
                }
                w.transcribable(&layer.sc_c);
                w.gf(&layer.pair.0);
                w.gf(&layer.pair.1);
            }
            w.len(self.us[l].len());
            for &u in &self.us[l] {
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
                let has_x = r.len()?;
                let sc_x = if has_x == 1 {
                    Some(r.transcribable::<crate::piop::sumcheck::SumcheckProof<Gf>>()?)
                } else {
                    None
                };
                let sc_c = r.transcribable::<crate::piop::sumcheck::SumcheckProof<Gf>>()?;
                let p0 = r.gf()?;
                let p1 = r.gf()?;
                layers.push(MergedLayer { sc_x, sc_c, pair: (p0, p1) });
            }
            mfs.push(MergedForestProof { layers });
            let n_u = r.len()?;
            let mut u = Vec::with_capacity(n_u);
            for _ in 0..n_u {
                u.push(r.u128()?);
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

    #[test]
    fn mod_q_root_prefix_encoding_is_stable_and_root_sensitive() {
        let root_a = core::array::from_fn(|i| i as u8);
        let mut via_helper = Blake3Transcript::new();
        absorb_mle_eval_mod_q_root(&mut via_helper, &root_a);
        let challenge_a: Gf = via_helper.get_field_challenge(&());

        let mut explicit = Blake3Transcript::new();
        explicit.absorb_slice(MLE_EVAL_MOD_Q_TRANSCRIPT_LABEL);
        explicit.absorb_slice(&root_a);
        let explicit_challenge: Gf = explicit.get_field_challenge(&());
        assert_eq!(
            challenge_a, explicit_challenge,
            "root-prefix framing changed"
        );
        assert_eq!(
            *challenge_a.words(),
            [17_415_676_208_349_607_439, 9_887_866_971_790_199_839],
            "update only for an intentional protocol version"
        );

        let mut root_b = root_a;
        root_b[31] ^= 1;
        let mut changed_root = Blake3Transcript::new();
        absorb_mle_eval_mod_q_root(&mut changed_root, &root_b);
        let challenge_b: Gf = changed_root.get_field_challenge(&());
        assert_ne!(
            challenge_a, challenge_b,
            "the first challenge must depend on the root"
        );
    }

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
}

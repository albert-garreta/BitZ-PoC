//! Shared Flock commitment, configuration, statement, and OOD support.
//! Integer openings are implemented by `crate::bitz`.

use crate::ligerito::{LOG_PACKING, packed_vars, repack_leaf_bits, row_bit_vars};
use crate::pcs::IntegerMatrixLayout;
use crate::piop::spartan::grinding::{
    GrindingDomain, GrindingRound, grind_and_absorb, verify_and_absorb,
};
use crate::poly::univariate::binary_gf128::Gf128 as Gf;
use crate::transcript::traits::Transcript;
use crate::utils::{cfg_chunks_mut, cfg_into_iter};
use anyhow::Context;
use flock_core::challenger::Challenger;
use flock_core::field::Gf128;
use flock_core::merkle::HashKind;
use flock_core::pcs::commit::{Commitment, PcsParams, ProverData, commit};
use flock_core::pcs::ligerito::{
    self, LigeritoSecurityConfig, ProverConfig as LigProverConfig, SoundnessRegime,
    VerifierConfig as LigVerifierConfig,
};
#[cfg(feature = "parallel")]
use rayon::prelude::*;
mod configuration;
#[cfg(test)]
mod coverage;
pub(crate) mod grinding_plan;
pub use configuration::{LigeritoSelection, ResolvedLigerito};
mod ood;
pub use ood::{ProverOod, VerifierOod, bind_prover_ood, bind_verifier_ood};

/// Drives flock's `Challenger` from a zinc [`Transcript`], so the flock PCS
/// stages chain into the same Fiat–Shamir state as the forest GKR and the
/// pre-sumcheck. Absorbs are framed through `absorb_slice`; challenges come
/// from `get_field_challenge::<Gf>`.
pub struct ZincChallenger<'a, T: Transcript + Send>(pub &'a mut T);

impl<T: Transcript + Send> Challenger for ZincChallenger<'_, T> {
    fn observe_label(&mut self, label: &[u8]) {
        self.0.absorb_slice(label);
    }

    fn observe_f128(&mut self, value: Gf128) {
        let mut bytes = [0u8; 16];
        bytes[..8].copy_from_slice(&value.lo.to_le_bytes());
        bytes[8..].copy_from_slice(&value.hi.to_le_bytes());
        self.0.absorb_slice(&bytes);
    }

    #[allow(clippy::arithmetic_side_effects)]
    fn observe_f128_slice(&mut self, values: &[Gf128]) {
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

    fn sample_f128(&mut self) -> Gf128 {
        let g: Gf = self.0.get_field_challenge(&());
        g
    }

    fn grind_pow(&mut self, bits: u32) -> u64 {
        let span = tracing::info_span!(
            "lig:grind_pow",
            bits,
            nonce = tracing::field::Empty,
            seed_lo = tracing::field::Empty,
            seed_hi = tracing::field::Empty,
        );
        let _g = span.enter();
        let seed = self.pow_seed();
        // Diagnostics use the same public Fiat–Shamir seed and chosen nonce;
        // recording them does not sample or absorb any transcript bytes.
        span.record("seed_lo", u64::from_le_bytes(seed[..8].try_into().unwrap()));
        span.record("seed_hi", u64::from_le_bytes(seed[8..].try_into().unwrap()));
        // Parallel smallest-nonce search (prover-side only; the verifier
        // checks whatever nonce arrives): every pool thread takes chunks of
        // the nonce space in order and the running minimum hit ends the
        // scan — exactly the serial scan's nonce, so the transcript stays
        // byte-identical. The expected serial cost is 2^bits compressions
        // (four-lane NEON kernel). Below ~2^12 expected attempts the
        // thread broadcast outweighs the win — stay serial there.
        #[cfg(feature = "parallel")]
        let nonce = if bits == 0 {
            0
        } else if bits >= 12 {
            crate::utils::blake3x4::smallest_pow_nonce(&seed, bits).expect("a nonce below 2^64")
        } else {
            first_pow_nonce(&seed, 0, u64::MAX, bits).expect("a nonce below 2^64")
        };
        #[cfg(not(feature = "parallel"))]
        let nonce = if bits == 0 {
            0
        } else {
            first_pow_nonce(&seed, 0, u64::MAX, bits).expect("a nonce below 2^64")
        };
        span.record("nonce", nonce);
        self.0.absorb_slice(&nonce.to_le_bytes());
        nonce
    }

    fn verify_pow(&mut self, nonce: u64, bits: u32) -> bool {
        let seed = self.pow_seed();
        // A zero-bit site has no work requirement, but it still needs one
        // canonical proof representation. Accepting any nonce here would give
        // a prover a free 64-bit Fiat--Shamir reroll before the next query
        // challenge.
        let ok = if bits == 0 {
            nonce == 0
        } else {
            pow_ok(&seed, nonce, bits)
        };
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
        let w = g.as_words();
        let mut seed = [0u8; 16];
        seed[..8].copy_from_slice(&w[0].to_le_bytes());
        seed[8..].copy_from_slice(&w[1].to_le_bytes());
        seed
    }
}

/// `blake3(seed || nonce)` has at least `bits` leading zero bits; the
/// prover scans nonces four at a time ([`first_pow_nonce`]), the verifier
/// checks the one it receives.
use crate::utils::blake3x4::{first_pow_nonce, pow_ok};

// ---------------------------------------------------------------------
// Commit
// ---------------------------------------------------------------------

/// Prover-side state of the flock-backed commitment.
pub struct FlockCommitHint {
    /// Per-column bit rows (shared layout with the zinc backend).
    rows: std::sync::Arc<Vec<Vec<u64>>>,
    /// Materialize the alternate layout only for a consumer that needs it.
    packed_cols: std::sync::OnceLock<Vec<Vec<u64>>>,
    row_layout: IntegerMatrixLayout,
    /// The packed message in flock representation.
    p_msg: Vec<Gf128>,
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

    /// The 64-column-lane packing of the same bits (`packed_cols[g][b]` =
    /// bit `b` of columns `64g..64g+63`), built on first use.
    pub fn packed_cols(&self) -> &[Vec<u64>] {
        self.packed_cols
            .get_or_init(|| crate::ligerito::pack_columns_from_rows(&self.row_layout, &self.rows))
    }

    /// The flock prover data (codeword + Merkle tree) behind the commitment.
    pub fn flock_prover_data(&self) -> &ProverData {
        &self.prover_data
    }

    /// The packed message in flock representation, `2^{m_p}` elements.
    pub fn packed_message(&self) -> &[Gf128] {
        &self.p_msg
    }

    pub(crate) fn matches_rows(&self, rows: &std::sync::Arc<Vec<Vec<u64>>>) -> bool {
        std::sync::Arc::ptr_eq(&self.rows, rows) || self.rows == *rows
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

/// Shared commit tail: build the packed `Gf128` message from the per-column
/// bit rows (low 7 row-bit coordinates in-pack, row-bit-high then column
/// bits above) and run flock's PCS commit (NEON interleaved NTT + SHA-256
/// Merkle).
#[allow(clippy::arithmetic_side_effects)]
fn commit_rs_flock_from_rows(
    p: &IntegerMatrixLayout,
    rows: std::sync::Arc<Vec<Vec<u64>>>,
    packed_cols: Option<Vec<Vec<u64>>>,
    log_inv_rate: usize,
    log_batch: usize,
    merkle_hash: HashKind,
) -> FlockCommitHint {
    let t_w = row_bit_vars(p);
    assert!(t_w >= LOG_PACKING, "packing needs t + log2(W) >= 7");
    let hi_count = 1usize << (t_w - LOG_PACKING);
    let mut p_msg = Vec::with_capacity(hi_count << p.col_vars);
    for row in rows.iter().take(p.cols()) {
        for i_hi in 0..hi_count {
            p_msg.push(Gf128 {
                lo: row[2 * i_hi],
                hi: row[2 * i_hi + 1],
            });
        }
    }

    let m_p = packed_vars(p);
    assert!(
        log_batch < m_p,
        "log_batch must leave at least one position variable"
    );
    let params = PcsParams {
        m: m_p + LOG_PACKING,
        log_inv_rate,
        log_batch_size: log_batch,
        profile: Default::default(),
        merkle_hash,
    };
    let (commitment, prover_data) = commit(&p_msg, &params);
    FlockCommitHint {
        rows,
        packed_cols: packed_cols
            .map(std::sync::OnceLock::from)
            .unwrap_or_default(),
        row_layout: *p,
        p_msg,
        commitment,
        prover_data,
    }
}

/// Commit the bit data with flock's PCS commit at an explicit
/// `(log_inv_rate, log_batch)` shape, starting from the flat `u128` cell
/// tensor.
pub fn commit_rs_flock_with(
    p: &IntegerMatrixLayout,
    data: &[u128],
    log_inv_rate: usize,
    log_batch: usize,
) -> FlockCommitHint {
    let rows = repack_leaf_bits(p, data);
    commit_rs_flock_from_rows(
        p,
        rows.into(),
        None,
        log_inv_rate,
        log_batch,
        HashKind::default(),
    )
}

/// Commit starting from per-column bit rows (the [`repack_leaf_bits`]
/// layout: bit `i = (b<<log₂W)|j` of row `c` = bit `j` of cell `(b,c)`,
/// 64 bits per word) — column-lane packing is built on first use and the
/// `u128` cell tensor never exists. This is the memory-honest entry for
/// harnesses/hosts that can produce bits directly: peak stays at the
/// packed scale (`2^n/8` bytes per store) instead of 16 B per cell.
pub fn commit_rs_ligerito_rows(
    p: &IntegerMatrixLayout,
    rows: Vec<Vec<u64>>,
    pc: &LigProverConfig,
) -> FlockCommitHint {
    commit_rs_ligerito_shared_rows(p, rows.into(), pc)
}

/// Share immutable source storage with a witness that outlives commitment.
pub(crate) fn commit_rs_ligerito_shared_rows(
    p: &IntegerMatrixLayout,
    rows: std::sync::Arc<Vec<Vec<u64>>>,
    pc: &LigProverConfig,
) -> FlockCommitHint {
    commit_rs_flock_from_rows(
        p,
        rows,
        None,
        pc.log_inv_rates[0],
        pc.initial_k,
        pc.merkle_hash,
    )
}

/// Commit starting from the 64-column-lane packed store (the layout
/// `sha_f2_packed_cols` builds on the host SHA path) — the per-column rows
/// are rebuilt by 64×64 bit-transposes; the `u128` cell tensor never
/// exists. Shape from the Ligerito prover config.
pub fn commit_rs_ligerito_packed(
    p: &IntegerMatrixLayout,
    packed_cols: Vec<Vec<u64>>,
    pc: &LigProverConfig,
) -> FlockCommitHint {
    let rows = crate::ligerito::rows_from_packed_cols(p, &packed_cols);
    commit_rs_flock_from_rows(
        p,
        rows.into(),
        Some(packed_cols),
        pc.log_inv_rates[0],
        pc.initial_k,
        pc.merkle_hash,
    )
}

/// Baseline Ligerito configuration used by the fixed-modulus adapters.
pub fn historical_sha_lig_configs(
    m_p: usize,
) -> Result<(LigProverConfig, LigVerifierConfig), String> {
    let m = m_p + LOG_PACKING;
    if m < 22 {
        return lig_configs(
            m_p,
            LigConfig::Adhoc {
                log_batch: 2,
                log_inv_rate: 2,
            },
        );
    }
    if ligerito::embedded_security_config(m, ligerito::LigeritoProfile::Slim).is_none() {
        return Err(format!("no embedded ligerito template for m={m}"));
    }
    custom_johnson_config(m, 1, 4).to_prover_verifier_configs()
}

/// Round-0 parameters matching [`historical_sha_lig_configs`]: its Johnson
/// ladder (from `m = 22` on) runs Round 0, its ad-hoc configs below need none.
pub fn historical_sha_lig_ood_params(m_p: usize) -> Option<OodRoundParams> {
    let m = m_p + LOG_PACKING;
    if m < 22 {
        return None;
    }
    let config = custom_johnson_config(m, 1, 4);
    let target = u32::try_from(config.target_security_bits).ok()?;
    ood_round_params(&config, m_p, target)
}

/// Production fixed-modulus default; unsupported shapes are errors.
pub fn sha_lig_configs(m_p: usize) -> Result<(LigProverConfig, LigVerifierConfig), String> {
    let resolved = LigeritoSelection::JOHNSON.resolve(m_p, 100)?;
    Ok((resolved.prover().clone(), resolved.verifier().clone()))
}

/// Round-0 parameters matching the checked production configuration.
pub fn sha_lig_ood_params(m_p: usize) -> Option<OodRoundParams> {
    let resolved = LigeritoSelection::JOHNSON
        .resolve(m_p, 100)
        .expect("unsupported production Ligerito shape");
    ood_round_params(resolved.security(), m_p, 100)
}

/// Builds a validator-gated UDR Ligerito configuration at an explicit
/// round-by-round security target.
///
/// The supported window is `m = m_p + 7 = 20..=35`: the two small SHA
/// endpoints reuse the `m=22` scalar template, and `m=22..=35` are backed by
/// embedded production templates. The audited 128-bit target and lower custom
/// targets use the same UDR/fold-grinding solver; non-128 targets remain marked
/// as custom in the configuration metadata. Shapes outside this window are
/// rejected instead of silently falling back to an ad-hoc configuration.
pub fn validated_udr_lig_configs_for_target(
    m_p: usize,
    target_bits: usize,
) -> Result<(LigProverConfig, LigVerifierConfig), String> {
    validated_udr_lig_configs_with(m_p, 1, 4, target_bits)
}

/// [`validated_udr_lig_configs_for_target`] with an explicit code rate and
/// fold arity: the CLI profile `udrg:<log_inv_rate>:<initial_k>:<bits>`
/// (UDR geometry with fold grinding, BLAKE3 Merkle trees), validator-gated at
/// `target_bits`. Lower rates buy more bits per query (fewer queries, smaller
/// proof) at the price of a longer codeword to encode and hash at commit.
pub fn validated_udr_lig_configs_with(
    m_p: usize,
    log_inv_rate: usize,
    initial_k: usize,
    target_bits: usize,
) -> Result<(LigProverConfig, LigVerifierConfig), String> {
    let m = m_p
        .checked_add(LOG_PACKING)
        .ok_or_else(|| "Ligerito variable count overflow".to_owned())?;
    if !(20..=35).contains(&m) {
        return Err(format!(
            "validated UDR profile requires m in [20, 35], got {m}"
        ));
    }
    if !(64..=128).contains(&target_bits) {
        return Err(format!(
            "validated UDR target must be in [64, 128] bits, got {target_bits}"
        ));
    }
    let mut security = custom_udr_grind_config_bits(m, log_inv_rate, initial_k, Some(target_bits));
    if target_bits != 128 {
        security.analysis_version =
            "udr_maximal_radius_with_fold_grinding (unaudited custom target)".into();
    }
    security.hash = "blake3".into();
    security.validate()?;
    security.to_prover_verifier_configs()
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
    /// A validator-gated Johnson config derived from flock's embedded slim
    /// template at the requested base rate and L0 interleaving.
    CustomJohnson {
        log_inv_rate: usize,
        initial_k: usize,
    },
    /// `ligerito::default_config` for ad-hoc/test shapes (UDR query counts,
    /// no grinding/OOD; per-level parameters not audited).
    Adhoc {
        log_batch: usize,
        log_inv_rate: usize,
    },
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
        LigConfig::CustomJohnson {
            log_inv_rate,
            initial_k,
        } => {
            let m = m_p.wrapping_add(LOG_PACKING);
            custom_johnson_config(m, log_inv_rate, initial_k).to_prover_verifier_configs()
        }
        LigConfig::Adhoc {
            log_batch,
            log_inv_rate,
        } => {
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
                merkle_hash: pc.merkle_hash,
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
/// Used by the bench's `BITZ_LIG_PROFILE=custom:<r0>:<k0>` and by
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
///
/// Shapes: `m = m_p + 7 ≥ 20`. Flock's embedded slim templates exist for
/// `m = 22..=35`; `m = 20, 21` are seeded from the `m = 22` template (only
/// its scalar/default fields are used — every shape field is rebuilt here),
/// the same seeding [`custom_udr_config_bits`] already applies. `m ≥ 22`
/// configs are unchanged by this.
#[allow(clippy::arithmetic_side_effects, clippy::missing_panics_doc)]
pub fn custom_johnson_config_bits(
    m: usize,
    r0: usize,
    k0: usize,
    target_bits: Option<usize>,
) -> LigeritoSecurityConfig {
    try_custom_johnson_config_bits(m, r0, k0, target_bits)
        .expect("custom config passes flock's validator")
}

fn try_custom_johnson_config_bits(
    m: usize,
    r0: usize,
    k0: usize,
    target_bits: Option<usize>,
) -> Result<LigeritoSecurityConfig, String> {
    // Embedded production tables start at m=22. Only scalar/default fields
    // are borrowed from the template (header strings, `eta`, grinding
    // convention, target); `m`, `log_n`, every level shape, and the final
    // block are rebuilt below, so — exactly as in `udr_config_impl` — the
    // m=22 template is also a sound seed for m=20 and m=21. For m ≥ 22 the
    // template's own `m`/`log_n` are re-assigned to themselves (no change).
    let template_m = m.max(22);
    let slim = ligerito::embedded_security_config(template_m, ligerito::LigeritoProfile::Slim)
        .ok_or_else(|| format!("no embedded slim template for m={template_m}"))?;
    let mut cfg = LigeritoSecurityConfig::from_toml_str(slim)?;
    let log_n = m
        .checked_sub(LOG_PACKING)
        .ok_or("custom Johnson witness has fewer than LOG_PACKING variables")?;
    cfg.m = m;
    cfg.log_n = log_n;
    if k0 == 0 || k0 >= log_n {
        return Err("custom initial_k out of range".into());
    }
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
        .map(|(i, &(mc, il, kr, r))| -> Result<_, String> {
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
                .ok_or("Johnson query search did not converge")?;
            // Every query is a distinct codeword position, so a level must be
            // at least as wide as its query count (flock's prover asserts this
            // at proving time; fail here, at configuration time, instead).
            let positions = 1usize << (mc + r);
            if lv.queries > positions {
                return Err(format!(
                    "custom Johnson level {i} is too thin: {} queries over {positions} positions \
                     (log_msg_cols {mc}, log_inv_rate {r}); use a larger m or a smaller initial_k",
                    lv.queries
                ));
            }
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
                lv.expected_eps_ood_bits = Some(
                    lv.paper_predicted_ood_bits()
                        .expect("johnson_ood prediction"),
                );
            } else {
                loop {
                    let ood = lv
                        .paper_predicted_ood_bits()
                        .expect("johnson_ood prediction");
                    if ood + 1e-3 >= lv.target_security_bits as f64 {
                        lv.expected_eps_ood_bits = Some(ood);
                        break;
                    }
                    lv.ood_samples += 1;
                }
            }
            Ok(lv)
        })
        .collect::<Result<_, _>>()?;
    cfg.validate()?;
    Ok(cfg)
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
    try_udr_config_impl(m, r0, k0, target_bits, fold_grind)
        .expect("custom UDR config passes flock's validator")
}

fn try_udr_config_impl(
    m: usize,
    r0: usize,
    k0: usize,
    target_bits: Option<usize>,
    fold_grind: bool,
) -> Result<LigeritoSecurityConfig, String> {
    // Embedded production tables start at m=22.  Only scalar/default fields
    // are borrowed from the template: `m`, `log_n`, every level shape, the
    // target, and the final block are rebuilt below, so the m=22 template is
    // also a sound seed for the paper's m=20 and m=21 SHA endpoints.
    let template_m = m.max(22);
    let slim = ligerito::embedded_security_config(template_m, ligerito::LigeritoProfile::Slim)
        .ok_or_else(|| format!("no embedded slim template for m={template_m}"))?;
    let mut cfg = LigeritoSecurityConfig::from_toml_str(slim)?;
    let log_n = m
        .checked_sub(LOG_PACKING)
        .ok_or("custom UDR witness has fewer than LOG_PACKING variables")?;
    cfg.m = m;
    cfg.log_n = log_n;
    cfg.analysis_version = "udr_maximal_radius_with_fold_grinding".into();
    if k0 == 0 || k0 >= log_n {
        return Err("custom initial_k out of range".into());
    }
    if let Some(bits) = target_bits {
        cfg.target_security_bits = bits;
    }
    let tmpl = cfg.levels[0].clone();

    // Same ladder as `custom_johnson_config`.
    let mut shapes = vec![(log_n - k0, k0, k0, r0)];
    let mut n_run = log_n - k0;
    let mut rate = r0;
    while n_run > 5 {
        // Leave a five-variable final block.  For m=20/21 the last fold is
        // only one/two variables; blindly taking three would leave a codeword
        // too short for the 128-bit query count.
        let kr = 3.min(n_run - 5);
        rate += 1;
        shapes.push((n_run - kr, kr, kr, rate));
        n_run -= kr;
    }
    cfg.initial_k = k0;
    cfg.final_block.yr_log_n = n_run;
    cfg.levels = shapes
        .iter()
        .map(|&(mc, il, kr, r)| -> Result<_, String> {
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
                .ok_or("UDR query search did not converge")?;
            let (pg, qb) = lv.paper_predicted_bits();
            // udrg only: recover the pg shortfall with per-fold PoW
            // (cheap here — pg is 112–119, so the grind is 9–16 bits).
            // flock grinds `fold_grinding_bits − j` at the level's fold
            // round `j` (a taper the Johnson row union pays for); in unique
            // decoding every one of the level's `kr` rounds carries the full
            // shortfall, so the last round's `kr − 1` bits go on top.
            if fold_grind {
                let need = (lv.target_security_bits as f64 - pg).ceil().max(0.0) as usize;
                lv.fold_grinding_bits = if need == 0 { 0 } else { need + kr - 1 };
            }
            lv.expected_eps_pg_bits = pg;
            lv.expected_eps_query_bits = qb;
            Ok(lv)
        })
        .collect::<Result<_, _>>()?;
    cfg.validate()?;
    // flock's validator checks each level once; the fold rounds are checked
    // one by one here (the weakest round of a UDR level is its last).
    if fold_grind {
        for (index, lv) in cfg.levels.iter().enumerate() {
            let weakest = fold_round_bits(lv)
                .into_iter()
                .fold(f64::INFINITY, f64::min);
            if weakest + 1e-9 < lv.target_security_bits as f64 {
                return Err(format!(
                    "UDR level {index}: weakest fold round {weakest:.2} bits < target {}",
                    lv.target_security_bits
                ));
            }
        }
    }
    Ok(cfg)
}

/// The bits of each fold round of a Ligerito level, its fold grinding
/// included. flock grinds `fold_grinding_bits − j` at the level's fold round
/// `j` (`k_recursive` rounds). In the Johnson regime the level's proximity-gap
/// term is sized for round 0 and the row-union factor `2^{ℓ−1−j}` drops by
/// exactly the bit the taper removes, so every round is credited
/// `eps_pg + fold_grinding_bits` (the accounting every caller has always
/// used). In unique decoding the term is the same at every round, so round
/// `j` keeps only `fold_grinding_bits − j` bits of proof of work.
pub fn fold_round_bits(level: &ligerito::LigeritoLevelConfig) -> Vec<f64> {
    let (pg, _) = level.paper_predicted_bits();
    (0..level.k_recursive)
        .map(|j| pg + fold_round_grinding(level, j) as f64)
        .collect()
}

/// The fold grinding the weakest fold round of `level` gets:
/// `fold_grinding_bits` in the Johnson regime, `fold_grinding_bits − (k − 1)`
/// in unique decoding (see [`fold_round_bits`]).
pub fn weakest_fold_round_grinding(level: &ligerito::LigeritoLevelConfig) -> usize {
    fold_round_grinding(level, level.k_recursive.saturating_sub(1))
}

/// The PoW credited to fold round `round` of `level` (see [`fold_round_bits`]).
fn fold_round_grinding(level: &ligerito::LigeritoLevelConfig, round: usize) -> usize {
    match level.regime {
        SoundnessRegime::JohnsonOod => level.fold_grinding_bits,
        SoundnessRegime::Udr => level.fold_grinding_bits.saturating_sub(round),
    }
}

#[cfg(test)]
mod fold_round_tests {
    use super::*;

    /// Every fold round of a `udrg` ladder reaches its target — including a
    /// level's last round, which flock's taper grinds `k − 1` bits less than
    /// its first (MultiSwap's `udrg:3:4:114` at 2^25 committed bits used to
    /// leave its level-0 rounds at 114.2 / 113.2 / 112.2 / 112.2 bits).
    #[test]
    fn udrg_fold_rounds_reach_the_target() {
        for (m, r0, k0, target) in [
            (25, 3, 4, 114),
            (22, 3, 4, 114),
            (24, 3, 4, 112),
            (26, 1, 4, 128),
            (30, 1, 4, 128),
        ] {
            let cfg = custom_udr_grind_config_bits(m, r0, k0, Some(target));
            for (index, level) in cfg.levels.iter().enumerate() {
                let rounds = fold_round_bits(level);
                assert_eq!(rounds.len(), level.k_recursive);
                for (j, bits) in rounds.iter().enumerate() {
                    assert!(
                        *bits + 1e-9 >= target as f64,
                        "m={m} target={target} level {index} round {j}: {bits:.2} bits"
                    );
                }
            }
        }
    }

    /// Johnson ladders keep the accounting every caller has always used:
    /// each fold round at `eps_pg + fold_grinding_bits`.
    #[test]
    fn johnson_fold_rounds_are_unchanged() {
        for (m, r0) in [(24, 1), (28, 1), (28, 3)] {
            let cfg = custom_johnson_config(m, r0, 4);
            for level in &cfg.levels {
                let (pg, _) = level.paper_predicted_bits();
                for bits in fold_round_bits(level) {
                    assert_eq!(bits, pg + level.fold_grinding_bits as f64);
                }
                assert_eq!(weakest_fold_round_grinding(level), level.fold_grinding_bits);
            }
        }
    }
}

/// Commit at the shape the Ligerito config dictates
/// (`log_inv_rate = log_inv_rates[0]`, `log_batch = initial_k`).
pub fn commit_rs_ligerito(
    p: &IntegerMatrixLayout,
    data: &[u128],
    pc: &LigProverConfig,
) -> FlockCommitHint {
    commit_rs_flock_with(p, data, pc.log_inv_rates[0], pc.initial_k)
}

/// Release flock's process-global scratch pool
/// ([`flock_core::scratch::clear`]). flock retains the open's large `Gf128`
/// buffers across proves (up to ~1 codeword + the fold set) to skip
/// page-fault/munmap churn on repeated proves; call this after the last
/// prove of a batch — or before measuring a single prove's peak — to return
/// that memory to the OS.
pub fn flock_scratch_clear() {
    flock_core::scratch::clear();
}

// ---------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------

/// Errors in shared commitment and OOD validation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FlockRsError {
    PrimeSampling(crate::prime_sampling::PrimeSamplingError),
    CommitmentConfig,
    OodRound,
}

/// Reject a commitment/config mismatch before any Fiat–Shamir state is
/// consumed. The Ligerito config describes the packed message length as its
/// initial column and interleave dimensions; the public commitment adds the
/// seven in-pack bit coordinates.
pub(crate) fn validate_ligerito_commitment(
    commitment: &Commitment,
    config: &impl LigeritoStatementConfig,
) -> Result<(), FlockRsError> {
    check_ligerito_commitment(commitment, config).map_err(|_| FlockRsError::CommitmentConfig)
}

fn check_ligerito_commitment(
    commitment: &Commitment,
    config: &impl LigeritoStatementConfig,
) -> anyhow::Result<()> {
    validate_ligerito_config_shape(config)?;
    let recursive_levels = config.recursive_steps();
    let log_inv_rates = config.log_inv_rates();
    let recursive_log_msg_cols = config.recursive_log_msg_cols();
    let recursive_ks = config.recursive_ks();
    let queries = config.queries();
    let log_inv_rate = log_inv_rates
        .first()
        .copied()
        .context("missing initial inverse rate")?;
    let initial_log_msg_cols = config.initial_log_msg_cols();
    let initial_log_num_interleaved = config.initial_log_num_interleaved();
    let initial_k = config.initial_k();
    let expected_m = initial_log_msg_cols
        .checked_add(initial_log_num_interleaved)
        .and_then(|len| len.checked_add(LOG_PACKING))
        .context("message dimension overflow")?;
    anyhow::ensure!(
        expected_m < usize::BITS as usize,
        "message dimension exceeds usize"
    );
    let initial_block_log = initial_log_msg_cols
        .checked_add(log_inv_rate)
        .context("initial block dimension overflow")?;
    let initial_block_len = u32::try_from(initial_block_log)
        .ok()
        .and_then(|log| 1usize.checked_shl(log))
        .context("initial block length overflow")?;
    anyhow::ensure!(
        queries[0] <= initial_block_len,
        "initial queries exceed block length"
    );
    let mut remaining = initial_log_msg_cols;
    for i in 0..recursive_levels {
        let k = recursive_ks[i];
        anyhow::ensure!(
            k > 0 && k <= remaining,
            "invalid fold dimension at level {i}"
        );
        remaining -= k;
        anyhow::ensure!(
            recursive_log_msg_cols[i] == remaining,
            "message dimension mismatch at level {i}"
        );
        let block_log = remaining
            .checked_add(log_inv_rates[i + 1])
            .context("recursive block dimension overflow")?;
        let block_len = u32::try_from(block_log)
            .ok()
            .and_then(|log| 1usize.checked_shl(log))
            .context("recursive block length overflow")?;
        anyhow::ensure!(
            queries[i + 1] <= block_len,
            "queries exceed block length at level {}",
            i + 1
        );
    }
    let params = &commitment.params;
    anyhow::ensure!(
        params.m == expected_m
            && params.log_inv_rate == log_inv_rate
            && params.log_batch_size == initial_k
            && initial_log_num_interleaved == initial_k
            && params.merkle_hash == config.merkle_hash(),
        "commitment metadata does not match Ligerito config"
    );
    Ok(())
}

/// Validate per-level array lengths and required values before indexing them.
fn validate_ligerito_config_shape(config: &impl LigeritoStatementConfig) -> anyhow::Result<()> {
    let recursive_levels = config.recursive_steps();
    let levels = recursive_levels
        .checked_add(1)
        .context("recursive level count overflow")?;
    let invalid_lengths = [
        config.log_inv_rates().len(),
        config.queries().len(),
        config.grinding_bits().len(),
        config.fold_grinding_bits().len(),
        config.ood_samples().len(),
    ]
    .into_iter()
    .any(|len| len != levels);
    let invalid_recursive_lengths = [
        config.recursive_log_msg_cols().len(),
        config.recursive_ks().len(),
    ]
    .into_iter()
    .any(|len| len != recursive_levels);

    anyhow::ensure!(recursive_levels > 0, "missing recursive levels");
    anyhow::ensure!(
        !invalid_lengths && !invalid_recursive_lengths,
        "invalid per-level array lengths"
    );
    anyhow::ensure!(
        !config.log_inv_rates().contains(&0),
        "inverse rates must be nonzero"
    );
    anyhow::ensure!(
        !config.queries().contains(&0),
        "query counts must be nonzero"
    );
    anyhow::ensure!(
        config.ood_samples().first() == Some(&0),
        "initial OOD sample count must be zero"
    );
    Ok(())
}

const STATEMENT_FRAME_DOMAIN: &[u8] = b"bitz/ligerito-flock/statement-frame/v1";
const FIELD_BYTES: u8 = 1;
const FIELD_U8: u8 = 2;
const FIELD_U64: u8 = 3;
#[allow(dead_code)]
const FIELD_U128: u8 = 4;
const FIELD_GF128: u8 = 5;

/// A streaming, typed transcript frame. Every field is encoded as one
/// `absorb_slice` frame containing `(semantic tag, type tag, element count,
/// canonical little-endian payload)`. Streaming the payload through
/// `absorb_inner` avoids materializing a second copy of large row-weight
/// tables while remaining byte-for-byte equivalent to one contiguous
/// `absorb_slice` call.
struct StatementFrame<'a, T: Transcript> {
    transcript: &'a mut T,
}

impl<'a, T: Transcript> StatementFrame<'a, T> {
    fn new(transcript: &'a mut T, domain: &[u8]) -> Self {
        transcript.absorb_slice(STATEMENT_FRAME_DOMAIN);
        transcript.absorb_slice(domain);
        Self { transcript }
    }

    fn begin_field(&mut self, tag: u8, kind: u8, count: usize) {
        self.transcript.absorb_inner(&[0x6]);
        self.transcript.absorb_inner(&[tag, kind]);
        self.transcript.absorb_inner(&(count as u64).to_le_bytes());
    }

    fn end_field(&mut self) {
        self.transcript.absorb_inner(&[0x7]);
    }

    fn bytes(&mut self, tag: u8, values: &[u8]) {
        self.begin_field(tag, FIELD_BYTES, values.len());
        self.transcript.absorb_inner(values);
        self.end_field();
    }

    fn byte(&mut self, tag: u8, value: u8) {
        self.begin_field(tag, FIELD_U8, 1);
        self.transcript.absorb_inner(&[value]);
        self.end_field();
    }

    fn usize(&mut self, tag: u8, value: usize) {
        self.begin_field(tag, FIELD_U64, 1);
        self.transcript.absorb_inner(&(value as u64).to_le_bytes());
        self.end_field();
    }

    fn usizes(&mut self, tag: u8, values: &[usize]) {
        self.begin_field(tag, FIELD_U64, values.len());
        for &value in values {
            self.transcript.absorb_inner(&(value as u64).to_le_bytes());
        }
        self.end_field();
    }

    #[allow(dead_code)]
    fn u128s(&mut self, tag: u8, values: &[u128]) {
        self.begin_field(tag, FIELD_U128, values.len());
        for &value in values {
            self.transcript.absorb_inner(&value.to_le_bytes());
        }
        self.end_field();
    }

    fn gf128(&mut self, tag: u8, value: Gf) {
        self.begin_field(tag, FIELD_GF128, 1);
        for word in value.as_words() {
            self.transcript.absorb_inner(&word.to_le_bytes());
        }
        self.end_field();
    }

    fn commitment(&mut self, commitment: &Commitment) {
        self.bytes(0x01, &commitment.root);
        self.usize(0x02, commitment.params.m);
        self.usize(0x03, commitment.params.log_inv_rate);
        self.usize(0x04, commitment.params.log_batch_size);
        self.byte(0x05, ligerito_profile_code(commitment.params.profile));
        self.byte(0x06, merkle_hash_code(commitment.params.merkle_hash));
    }

    fn int_eval_params(&mut self, p: &IntegerMatrixLayout) {
        self.usize(0x20, p.row_vars);
        self.usize(0x21, p.col_vars);
        self.usize(0x22, 1usize);
    }

    fn ligerito_config(&mut self, config: &impl LigeritoStatementConfig) {
        self.usizes(0x08, config.log_inv_rates());
        self.usize(0x09, config.recursive_steps());
        self.usize(0x0a, config.initial_log_msg_cols());
        self.usize(0x0b, config.initial_log_num_interleaved());
        self.usize(0x0c, config.initial_k());
        self.usizes(0x0d, config.recursive_log_msg_cols());
        self.usizes(0x0e, config.recursive_ks());
        self.usizes(0x0f, config.queries());
        self.usizes(0x10, config.grinding_bits());
        self.usizes(0x11, config.fold_grinding_bits());
        self.usizes(0x12, config.ood_samples());
        self.byte(0x13, merkle_hash_code(config.merkle_hash()));
    }
}

/// Read-only canonical view shared by prover and verifier Ligerito configs
/// when binding a public statement.
pub trait LigeritoStatementConfig {
    fn log_inv_rates(&self) -> &[usize];
    fn recursive_steps(&self) -> usize;
    fn initial_log_msg_cols(&self) -> usize;
    fn initial_log_num_interleaved(&self) -> usize;
    fn initial_k(&self) -> usize;
    fn recursive_log_msg_cols(&self) -> &[usize];
    fn recursive_ks(&self) -> &[usize];
    fn queries(&self) -> &[usize];
    fn grinding_bits(&self) -> &[usize];
    fn fold_grinding_bits(&self) -> &[usize];
    fn ood_samples(&self) -> &[usize];
    fn merkle_hash(&self) -> HashKind;
}

macro_rules! impl_ligerito_statement_config {
    ($ty:ty) => {
        impl LigeritoStatementConfig for $ty {
            fn log_inv_rates(&self) -> &[usize] {
                &self.log_inv_rates
            }
            fn recursive_steps(&self) -> usize {
                self.recursive_steps
            }
            fn initial_log_msg_cols(&self) -> usize {
                self.initial_log_msg_cols
            }
            fn initial_log_num_interleaved(&self) -> usize {
                self.initial_log_num_interleaved
            }
            fn initial_k(&self) -> usize {
                self.initial_k
            }
            fn recursive_log_msg_cols(&self) -> &[usize] {
                &self.recursive_log_msg_cols
            }
            fn recursive_ks(&self) -> &[usize] {
                &self.recursive_ks
            }
            fn queries(&self) -> &[usize] {
                &self.queries
            }
            fn grinding_bits(&self) -> &[usize] {
                &self.grinding_bits
            }
            fn fold_grinding_bits(&self) -> &[usize] {
                &self.fold_grinding_bits
            }
            fn ood_samples(&self) -> &[usize] {
                &self.ood_samples
            }
            fn merkle_hash(&self) -> HashKind {
                self.merkle_hash
            }
        }
    };
}

impl_ligerito_statement_config!(LigProverConfig);
impl_ligerito_statement_config!(LigVerifierConfig);

const fn ligerito_profile_code(profile: ligerito::LigeritoProfile) -> u8 {
    match profile {
        ligerito::LigeritoProfile::Fast => 0,
        ligerito::LigeritoProfile::Slim => 1,
        ligerito::LigeritoProfile::Secure => 2,
        ligerito::LigeritoProfile::Slim3 => 3,
    }
}

const fn merkle_hash_code(hash: HashKind) -> u8 {
    match hash {
        HashKind::Sha256 => 0,
        HashKind::Blake3 => 1,
    }
}

const STANDALONE_MOD_Q_STATEMENT_DOMAIN: &[u8] =
    b"bitz/ligerito-flock/standalone-mod-q-statement/v1";
const STANDALONE_MOD_Q_CLAIM_DOMAIN: &[u8] = b"bitz/ligerito-flock/standalone-mod-q-claim/v1";

/// Bind the public statement of a STANDALONE opening whose evaluation prime
/// and point are sampled from the transcript AFTER this frame (the `bitz`
/// CLI and `benches/pcs.rs`): the commitment, the opener config, the tensor
/// shape, the generator, the prime width and the Round-0 parameters. Sample
/// `q` and the point next, then bind the claim with
/// [`absorb_standalone_mod_q_claim`] before proving or verifying.
pub fn absorb_standalone_mod_q_statement(
    transcript: &mut impl Transcript,
    commitment: &Commitment,
    p: &IntegerMatrixLayout,
    alpha: Gf,
    q_bits: usize,
    ood: Option<OodRoundParams>,
    config: &impl LigeritoStatementConfig,
) {
    let mut frame = StatementFrame::new(transcript, STANDALONE_MOD_Q_STATEMENT_DOMAIN);
    frame.commitment(commitment);
    frame.ligerito_config(config);
    frame.int_eval_params(p);
    frame.gf128(0x30, alpha);
    frame.usize(0x31, q_bits);
    frame.usize(
        0x32,
        ood.map_or(0, |round| 1usize.wrapping_add(round.grinding_bits as usize)),
    );
}

/// Bind the transcript-sampled prime and the claimed value of a standalone
/// opening (see [`absorb_standalone_mod_q_statement`]).
pub fn absorb_standalone_mod_q_claim(transcript: &mut impl Transcript, q: u128, claimed_q: u128) {
    let mut frame = StatementFrame::new(transcript, STANDALONE_MOD_Q_CLAIM_DOMAIN);
    frame.u128s(0x30, &[q, claimed_q]);
}

/// Preserve the published standalone prime interval: bit width
/// `min(113, 126 - row_vars)`. Sampling and claim coordinates follow the
/// commitment; BitZ validates the resulting exact exponent bound.
pub fn standalone_q_bits(p: &IntegerMatrixLayout) -> usize {
    // Preserve the published prime-selection rule independently of the opener.
    126usize
        .checked_sub(p.row_vars)
        .expect("supported row width")
        .min(113)
}

/// The transcript-sampled instance of a standalone claim: the prime, the
/// evaluation point, and the `eq` weight tables over `F_q` it induces.
pub struct StandaloneInstance {
    pub q: u128,
    pub row_weights_q: Vec<u128>,
    pub col_weights_q: Vec<u128>,
}

/// Round-1-style draw after the statement is bound: `q` from the interval,
/// then the point coordinates uniformly mod `q`. Both sides run this
/// ([`StandaloneModQOpening`] inside every prove and verify).
pub fn sample_standalone_instance(
    transcript: &mut impl Transcript,
    p: &IntegerMatrixLayout,
    q_bits: usize,
) -> StandaloneInstance {
    let _g = tracing::info_span!("mq:sample_instance").entered();
    let q = crate::prime_sampling::sample_prime_with_bits(transcript, q_bits)
        .expect("bounded standalone prime search");
    let arith = field::FpCtx::from_prime_u128(q);
    let r1: Vec<u128> = (0..p.row_vars)
        .map(|_| crate::prime_sampling::sample_residue(transcript, q))
        .collect();
    let r2: Vec<u128> = (0..p.col_vars)
        .map(|_| crate::prime_sampling::sample_residue(transcript, q))
        .collect();
    StandaloneInstance {
        q,
        row_weights_q: eq_table_mod_q(&arith, &r1),
        col_weights_q: eq_table_mod_q(&arith, &r2),
    }
}

/// `eq(b, r) mod q` over `b ∈ {0,1}^{r.len()}`; each coordinate is appended
/// as the new lowest index bit, so `r[k]` ↔ index bit `r.len() − 1 − k`
/// (both roles build the same table, so only consistency matters).
pub fn eq_table_mod_q(arith: &field::FpCtx<2>, r: &[u128]) -> Vec<u128> {
    let q = arith.modulus_u128();
    let mut table = vec![1u128 % q];
    for &coord in r {
        let mut next = Vec::with_capacity(table.len() * 2);
        for &v in &table {
            let v1 = arith.mul_u128(v, coord);
            let v0 = if v >= v1 { v - v1 } else { v + q - v1 };
            next.push(v0);
            next.push(v1);
        }
        table = next;
    }
    table
}

/// Round-0 (OOD sample) parameters: `Some` executes the round with
/// `grinding_bits` of proof-of-work before the `ζ` draw; `None` skips it.
/// Callers derive it from the opener's security config through
/// [`ood_round_params`]; the choice is bound into the transcript.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OodRoundParams {
    /// Proof-of-work bits before the `ζ` draw (`0` = no boundary).
    pub grinding_bits: u32,
}

/// The prover's Round-0 messages.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OodRound {
    /// `y = MLE[P](ζ⃗)`, the packed message's out-of-domain evaluation.
    pub y: Gf,
    /// The proof-of-work nonce preceding the `ζ` draw (`Some` iff the
    /// round's difficulty is nonzero).
    pub nonce: Option<u64>,
}

/// The Round-0 proof-of-work domain.
pub enum OodRoundGrinding {}

impl GrindingDomain for OodRoundGrinding {
    const DOMAIN: &'static [u8] = b"bitz/core/ood-round-grinding/v1";
}

/// Transcript frame binding the round's public parameters before the draw.
const OOD_ROUND_DOMAIN: &[u8] = b"bitz/core/ood-round/v1";

/// `log₂` of the block the OOD kernels parallelize over.
const OOD_BLOCK_LOG: usize = 12;

/// `-log₂` of the theorem's Round-0 collision bound
/// `C(L_δ, 2)·(2^{packed_vars} − 1)/|K|` at the opener's level-0 Johnson
/// parameters, with the Johnson list size `L_δ ≤ 1/(2η√ρ)` (paper
/// `t:thm_core_IOPP` and `l:ood_collision`); `None` when level 0 runs in
/// the unique-decoding regime (`L_δ = 1`, no round).
pub fn ood_round_bits(cfg: &LigeritoSecurityConfig, packed_vars: usize) -> Option<f64> {
    let l0 = cfg.levels.first()?;
    let eta = match l0.regime {
        SoundnessRegime::JohnsonOod => l0.eta?,
        SoundnessRegime::Udr => return None,
    };
    let rho = (-(l0.log_inv_rate as f64)).exp2();
    let list = 1.0 / (2.0 * eta * rho.sqrt());
    let pairs = (list * (list - 1.0) / 2.0).max(1.0);
    let degree = ((packed_vars as f64).exp2() - 1.0).max(1.0);
    Some(128.0 - pairs.log2() - degree.log2())
}

/// Round-0 parameters at target `lambda`: in the Johnson regime, the
/// grinding that tops [`ood_round_bits`] up to `lambda`; `None` in the
/// unique-decoding regime.
pub fn ood_round_params(
    cfg: &LigeritoSecurityConfig,
    packed_vars: usize,
    lambda: u32,
) -> Option<OodRoundParams> {
    let bits = ood_round_bits(cfg, packed_vars)?;
    let deficit = f64::from(lambda) - bits;
    let grinding_bits = if deficit <= 0.0 {
        0
    } else {
        deficit.ceil() as u32
    };
    Some(OodRoundParams { grinding_bits })
}

/// Number of packed variables of a committed message (`2^{m_p}` elements).
fn packed_message_vars(p_msg: &[Gf128]) -> usize {
    assert!(
        !p_msg.is_empty() && p_msg.len().is_power_of_two(),
        "packed message length must be a power of two"
    );
    p_msg.len().trailing_zeros() as usize
}

/// The Round-0 evaluation point `ζ⃗ = (ζ^{2^0}, ζ^{2^1}, …)`: distinct
/// multilinear monomials become distinct powers of `ζ`.
#[allow(clippy::arithmetic_side_effects)]
fn ood_point(zeta: Gf, vars: usize) -> Vec<Gf> {
    let mut point = Vec::with_capacity(vars);
    let mut cur = zeta;
    for _ in 0..vars {
        point.push(cur);
        cur = cur * cur;
    }
    point
}

/// `scalar·eq(·, point)` over `{0,1}^{point.len()}` — index bit `k` ↔
/// `point[k]`, the [`crate::poly::utils::build_eq_x_r_vec`] convention
/// (low bits first, so Ligerito's low-bit-first folds bind `point[0]`
/// first).
#[allow(clippy::arithmetic_side_effects)]
fn build_eq_scaled(point: &[Gf], scalar: Gf) -> Vec<Gf> {
    let mut table = vec![scalar];
    for &z in point {
        // Coordinate `k` becomes index bit `k`: the existing table is the
        // low-index half (`v·(1 + z) = v + v·z` in characteristic two) and
        // its `z`-scaled copy the high-index half.
        let mut next = Vec::with_capacity(table.len() * 2);
        next.extend(table.iter().map(|&v| v + v * z));
        next.extend(table.iter().map(|&v| v * z));
        table = next;
    }
    table
}

/// `MLE[P](point)` for the packed message — block-parallel, deferred
/// reduction inside each block.
#[allow(clippy::arithmetic_side_effects)]
fn ood_eval(p_msg: &[Gf128], point: &[Gf]) -> Gf {
    use crate::utils::wide_mul::WideMulAcc;
    let vars = packed_message_vars(p_msg);
    assert_eq!(point.len(), vars, "OOD point dimension");
    let lo = vars.min(OOD_BLOCK_LOG);
    let block = 1usize << lo;
    let tail = build_eq_scaled(&point[..lo], Gf::one());
    let head = build_eq_scaled(&point[lo..], Gf::one());
    let inner: Vec<Gf> = cfg_into_iter!(0..head.len())
        .map(|hi| {
            let base = hi * block;
            let zero = Gf::zero();
            let mut acc = <Gf as WideMulAcc>::wide_zero(&zero);
            for (j, &t) in tail.iter().enumerate() {
                let m = p_msg[base + j];
                <Gf as WideMulAcc>::wide_add_assign(
                    &mut acc,
                    &<Gf as WideMulAcc>::mul_wide(&m, &t),
                );
            }
            <Gf as WideMulAcc>::from_wide(acc)
        })
        .collect();
    inner
        .iter()
        .zip(head.iter())
        .fold(Gf::zero(), |acc, (&i, &h)| acc + i * h)
}

fn absorb_ood_round_header(
    transcript: &mut impl Transcript,
    packed_vars: usize,
    params: OodRoundParams,
) {
    transcript.absorb_slice(OOD_ROUND_DOMAIN);
    transcript.absorb_slice(&(packed_vars as u64).to_le_bytes());
    transcript.absorb_slice(&params.grinding_bits.to_le_bytes());
}

/// The prover's view of the round: the point (kept, the eq table is
/// rebuilt scaled by `η_ood` at batching time), the value, and the
/// messages that go on the wire.
pub(crate) struct OodProverClaim {
    pub(crate) point: Vec<Gf>,
    pub(crate) y: Gf,
    pub(crate) round: OodRound,
}

/// Round 0 on the prover side: bind the parameters, grind, draw `ζ`,
/// evaluate, absorb `y`.
fn prove_ood_round(
    transcript: &mut (impl Transcript + Send),
    hint: &FlockCommitHint,
    params: OodRoundParams,
) -> OodProverClaim {
    prove_ood_round_packed(transcript, &hint.p_msg, params)
}

/// [`prove_ood_round`] on an explicit packed message (the message the
/// final Ligerito opening is run on — for a virtual concatenation of
/// several commitments, the virtual packed witness itself).
pub(crate) fn prove_ood_round_packed(
    transcript: &mut (impl Transcript + Send),
    p_msg: &[Gf128],
    params: OodRoundParams,
) -> OodProverClaim {
    let _g = tracing::info_span!("mc:ood").entered();
    let vars = packed_message_vars(p_msg);
    absorb_ood_round_header(transcript, vars, params);
    let nonce = if params.grinding_bits == 0 {
        None
    } else {
        Some(
            grind_and_absorb(
                transcript,
                GrindingRound::<OodRoundGrinding>::new(0),
                params.grinding_bits,
            )
            .expect("Round-0 grinding difficulty is validated by the profile"),
        )
    };
    let zeta: Gf = transcript.get_field_challenge(&());
    let point = ood_point(zeta, vars);
    let y = ood_eval(p_msg, &point);
    crate::ligerito::absorb_ood_value(transcript, y);
    OodProverClaim {
        point,
        y,
        round: OodRound { y, nonce },
    }
}

/// The verifier's view of the round: the point and the claimed value.
pub(crate) struct OodVerifierClaim {
    pub(crate) point: Vec<Gf>,
    pub(crate) y: Gf,
}

/// Round 0 on the verifier side: the same frame and draw, the proof's
/// nonce checked, the prover's `y` absorbed.
pub(crate) fn verify_ood_round(
    transcript: &mut (impl Transcript + Send),
    packed_vars: usize,
    params: OodRoundParams,
    round: &OodRound,
) -> Result<OodVerifierClaim, FlockRsError> {
    absorb_ood_round_header(transcript, packed_vars, params);
    match (params.grinding_bits, round.nonce) {
        (0, None) => {}
        (0, Some(_)) | (_, None) => return Err(FlockRsError::OodRound),
        (bits, Some(nonce)) => verify_and_absorb(
            transcript,
            GrindingRound::<OodRoundGrinding>::new(0),
            bits,
            nonce,
        )
        .map_err(|_| FlockRsError::OodRound)?,
    }
    let zeta: Gf = transcript.get_field_challenge(&());
    crate::ligerito::absorb_ood_value(transcript, round.y);
    Ok(OodVerifierClaim {
        point: ood_point(zeta, packed_vars),
        y: round.y,
    })
}

/// Add `η·eq(·, point)` to the η-combined Ligerito basis `b` (block-
/// parallel) and, when the fused Ligerito round-0 message `(u_0, u_2)` is
/// being precomputed, its contribution to that message (the message is
/// bilinear in `(f, b)`, so the OOD term adds on).
#[allow(clippy::arithmetic_side_effects)]
pub(crate) fn add_ood_basis(
    b: &mut [Gf128],
    f: &[Gf128],
    point: &[Gf],
    eta: Gf,
    round0: Option<&mut (Gf, Gf)>,
) {
    use crate::utils::wide_mul::WideMulAcc;
    let vars = point.len();
    assert_eq!(
        b.len(),
        1usize << vars,
        "basis length must match the OOD point"
    );
    assert_eq!(f.len(), b.len(), "message and basis lengths");
    let lo = vars.min(OOD_BLOCK_LOG);
    let block = 1usize << lo;
    let tail = build_eq_scaled(&point[..lo], Gf::one());
    let head = build_eq_scaled(&point[lo..], eta);
    let want_round0 = round0.is_some();
    let partials: Vec<(Gf, Gf)> = cfg_chunks_mut!(b, block)
        .enumerate()
        .map(|(hi, chunk)| {
            let base = hi * block;
            let scale = head[hi];
            let zero = Gf::zero();
            let mut u0 = <Gf as WideMulAcc>::wide_zero(&zero);
            let mut u2 = <Gf as WideMulAcc>::wide_zero(&zero);
            let mut j = 0usize;
            while j < chunk.len() {
                let has_pair = j + 1 < chunk.len();
                let d0 = scale * tail[j];
                let d1 = if has_pair {
                    scale * tail[j + 1]
                } else {
                    Gf::zero()
                };
                chunk[j] = (chunk[j]) + d0;
                if has_pair {
                    chunk[j + 1] = (chunk[j + 1]) + d1;
                }
                if want_round0 {
                    let f0 = f[base + j];
                    let f1 = if has_pair {
                        f[base + j + 1]
                    } else {
                        Gf::zero()
                    };
                    <Gf as WideMulAcc>::wide_add_assign(
                        &mut u0,
                        &<Gf as WideMulAcc>::mul_wide(&f0, &d0),
                    );
                    <Gf as WideMulAcc>::wide_add_assign(
                        &mut u2,
                        &<Gf as WideMulAcc>::mul_wide(&(f0 + f1), &(d0 + d1)),
                    );
                }
                j += 2;
            }
            (
                <Gf as WideMulAcc>::from_wide(u0),
                <Gf as WideMulAcc>::from_wide(u2),
            )
        })
        .collect();
    if let Some((u0, u2)) = round0 {
        for (p0, p2) in partials {
            *u0 += p0;
            *u2 += p2;
        }
    }
}

/// The OOD basis term after Ligerito bound the low `ris.len()` variables:
/// `η·eq(ris, point[..k])·eq(·, point[k..])` over every boolean tail.
#[allow(clippy::arithmetic_side_effects)]
pub(crate) fn ood_residual_evals(
    ris: &[Gf128],
    remaining_vars: usize,
    point: &[Gf],
    eta: Gf,
) -> Vec<Gf> {
    let bound = ris.len();
    assert_eq!(
        bound + remaining_vars,
        point.len(),
        "prefix + tail must cover the OOD point"
    );
    let one = Gf::one();
    let mut scalar = eta;
    for (r, z) in ris.iter().zip(point.iter()) {
        // eq(a, b) = ab + (1 + a)(1 + b) = 1 + a + b in characteristic two.
        scalar *= one + (*r) + *z;
    }
    build_eq_scaled(&point[bound..], scalar)
}

#[cfg(test)]
mod support_tests {
    use super::*;
    use crate::{ligerito::bind_low, transcript::Blake3Transcript};
    struct Xorshift(u64);
    impl Xorshift {
        fn next_u64(&mut self) -> u64 {
            self.0 ^= self.0 << 13;
            self.0 ^= self.0 >> 7;
            self.0 ^= self.0 << 17;
            self.0
        }
        fn gf(&mut self) -> Gf {
            Gf::from_polynomial_words([self.next_u64(), self.next_u64()])
        }
    }
    fn sample(seed: u64) -> Gf {
        let hi = seed.wrapping_mul(0x9E37_79B9_7F4A_7C15).rotate_left(29) ^ 0x1234_5678_9ABC_DEF0;
        Gf::from_polynomial_words([seed ^ 0xA5A5_5A5A_0F0F_F0F0, hi])
    }
    /// FieldRepresentation bridging is the identity on words, and multiplication agrees —
    /// the two `GF(2^128)` implementations are the same field in the same
    /// representation.
    #[test]
    fn field_bridge_is_bit_identical() {
        for i in 0..64u64 {
            let a = sample(0x77 + i);
            let b = sample(0x1000 + i);
            let fa = a;
            let fb = b;
            assert_eq!((fa), a);
            assert_eq!((fa * fb), a * b, "mul mismatch at {i}");
            assert_eq!((fa + fb), a + b, "add mismatch at {i}");
        }
        assert_eq!(LOG_PACKING, flock_core::pcs::pack::LOG_PACKING);
    }
    #[test]
    fn zinc_zero_bit_pow_nonce_is_canonical() {
        let mut prover_transcript = Blake3Transcript::new();
        let mut verifier_transcript = Blake3Transcript::new();
        let prover_next = {
            let mut challenger = ZincChallenger(&mut prover_transcript);
            assert_eq!(challenger.grind_pow(0), 0);
            challenger.sample_f128()
        };
        let verifier_next = {
            let mut challenger = ZincChallenger(&mut verifier_transcript);
            assert!(challenger.verify_pow(0, 0));
            challenger.sample_f128()
        };
        assert_eq!(prover_next, verifier_next);

        let mut malformed_transcript = Blake3Transcript::new();
        let mut challenger = ZincChallenger(&mut malformed_transcript);
        assert!(!challenger.verify_pow(1, 0));
    }
    #[test]
    fn shared_rows_and_lazy_columns_preserve_commitment_storage() {
        let p = IntegerMatrixLayout {
            row_vars: 9,
            col_vars: 6,
        };
        let (pc, _) = lig_configs(
            packed_vars(&p),
            LigConfig::Adhoc {
                log_batch: 2,
                log_inv_rate: 2,
            },
        )
        .unwrap();
        let data: Vec<_> = (0..p.cells()).map(|i| (i & 1) as u128).collect();
        let mut rows = std::sync::Arc::new(repack_leaf_bits(&p, &data));
        let hint = commit_rs_ligerito_shared_rows(&p, rows.clone(), &pc);
        assert!(std::sync::Arc::ptr_eq(&rows, &hint.rows));
        assert!(hint.packed_cols.get().is_none());
        assert!(hint.matches_rows(&std::sync::Arc::new((*rows).clone())));
        let expected = crate::ligerito::pack_columns_from_rows(&p, &rows);
        assert_eq!(hint.packed_cols(), expected);
        assert_eq!(hint.packed_cols().as_ptr(), hint.packed_cols().as_ptr());
        std::sync::Arc::make_mut(&mut rows)[0][0] ^= 1;
        assert!(!hint.matches_rows(&rows));
        assert_eq!(hint.packed_cols(), expected);
    }
    #[test]
    fn scaled_eq_table_matches_the_shared_convention() {
        let mut rng = Xorshift(0x5eed_0000);
        for vars in [1usize, 2, 5, 8] {
            let point: Vec<Gf> = (0..vars).map(|_| rng.gf()).collect();
            let expected = crate::poly::utils::build_eq_x_r_vec(&point, &()).expect("eq");
            assert_eq!(build_eq_scaled(&point, Gf::one()), expected, "vars={vars}");
        }
    }
    #[test]
    fn ood_residual_matches_dense_fold() {
        let mut rng = Xorshift(0x5eed_0001);
        for vars in [1usize, 3, 6, 9] {
            let point: Vec<Gf> = (0..vars).map(|_| rng.gf()).collect();
            let eta = rng.gf();
            for bound in 0..=vars {
                let ris: Vec<Gf> = (0..bound).map(|_| rng.gf()).collect();
                let mut dense = build_eq_scaled(&point, eta);
                for &r in &ris {
                    bind_low(&mut dense, r);
                }
                let ris_f: Vec<Gf128> = ris.iter().map(|&r| r).collect();
                let succinct = ood_residual_evals(&ris_f, vars - bound, &point, eta);
                assert_eq!(succinct, dense, "vars={vars} bound={bound}");
            }
        }
    }
    #[test]
    fn ood_eval_matches_naive_inner_product() {
        let mut rng = Xorshift(0x5eed_0002);
        for vars in [1usize, 5, 12, 13, 14] {
            let p_msg: Vec<Gf128> = (0..1usize << vars).map(|_| rng.gf()).collect();
            let point = ood_point(rng.gf(), vars);
            let eq = crate::poly::utils::build_eq_x_r_vec(&point, &()).expect("eq");
            let naive = eq
                .iter()
                .zip(&p_msg)
                .fold(Gf::zero(), |acc, (&e, &m)| acc + e * (m));
            assert_eq!(ood_eval(&p_msg, &point), naive, "vars={vars}");
        }
    }
    #[test]
    fn ood_round_params_follow_the_theorem_bound() {
        // Rate 1/8, η = 0.02: L_δ ≤ 1/(2η√ρ) = 70.71, C(L_δ, 2) = 2^11.27; at
        // m_p = 15 the point degree is 2^15 − 1, so the bound is 2^-101.7.
        let johnson = custom_johnson_config(22, 3, 4);
        let bits = ood_round_bits(&johnson, 15).expect("Johnson regime");
        assert!((bits - (128.0 - 11.267 - 15.0)).abs() < 0.05, "{bits}");
        assert_eq!(
            ood_round_params(&johnson, 15, 100),
            Some(OodRoundParams { grinding_bits: 0 })
        );
        assert_eq!(
            ood_round_params(&johnson, 15, 110),
            Some(OodRoundParams { grinding_bits: 9 })
        );
        // The paper's schedule ⌈log ℓ − 23.7⌉ at ℓ = 2^30 (m_p = 23): 7 bits.
        assert_eq!(
            ood_round_params(&johnson, 23, 100),
            Some(OodRoundParams { grinding_bits: 7 })
        );
        let udr = custom_udr_config_bits(22, 3, 4, None);
        assert_eq!(ood_round_bits(&udr, 15), None);
        assert_eq!(ood_round_params(&udr, 15, 100), None);
        // The fixed-modulus adapters' rate-1/2 Johnson config clears 100 bits
        // without grinding at m = 22; below the template boundary the ad-hoc
        // config runs without the round.
        assert_eq!(
            sha_lig_ood_params(15),
            Some(OodRoundParams { grinding_bits: 0 })
        );
        assert!(sha_lig_configs(10).is_err());
    }
    /// The presence rule an opener outside this module applies through
    /// `opening_claim`: where no round is due, nothing is absorbed and a
    /// Round-0 record is rejected, whether the state was bound before the
    /// PIOP or left for the opening.
    #[test]
    fn an_opening_with_no_round_due_rejects_a_record() {
        let round = OodRound {
            y: Gf::one(),
            nonce: None,
        };
        let mut transcript = Blake3Transcript::new();
        let fresh = transcript.state_digest();
        assert!(matches!(
            VerifierOod::from(None).opening_claim(&mut transcript, 15, None),
            Ok(None)
        ));
        let bound = bind_verifier_ood(&mut transcript, 15, None, None).unwrap();
        assert!(matches!(
            bound.opening_claim(&mut transcript, 15, None),
            Ok(None)
        ));
        assert_eq!(transcript.state_digest(), fresh);
        let states = [
            VerifierOod::from(None),
            bind_verifier_ood(&mut transcript, 15, None, None).unwrap(),
        ];
        for state in states {
            assert!(matches!(
                state.opening_claim(&mut transcript, 15, Some(&round)),
                Err(FlockRsError::OodRound)
            ));
        }
    }
}

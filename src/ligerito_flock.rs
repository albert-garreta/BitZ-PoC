//! Flock-backed RS opening for the integer-MLE-eval protocol — the
//! **performance backend** of [`crate::f2_int_ligerito`] (feature
//! `flock-pcs`).
//!
//! Everything hot runs flock-core's optimized code (succinctlabs/flock,
//! MIT OR Apache-2.0): the NEON/cache-blocked additive NTT, the SHA-256
//! Merkle commit with octopus multi-proofs, and `pcs::ligerito` (the
//! recursive prover/verifier). zinc keeps the protocol layers around it — the forest
//! GKR, the pre-sumcheck, the (thin) ring-switch orchestration — and the ONE
//! Fiat–Shamir chain is preserved by driving flock's `Challenger` trait from
//! zinc's [`Transcript`] ([`ZincChallenger`]).
//!
//! The two `GF(2^128)` representations are bit-identical (monomial/LSB-first,
//! GHASH reduction `0x87`): `Gf ↔ Gf128` conversion is a word copy, pinned by
//! the `ntt_matches_flock` differential test, which also serves as the
//! cross-implementation oracle for the in-repo scalar reference.
//!
//! Division of labour per claim `M̂(point) = μ` (from the shared
//! pre-sumcheck):
//! * zinc [`crate::ligerito::ring_switch_prove`]/`_verify` handle the
//!   `s_v` message and the r″ recombination (O(2^{m_p}) — not hot), emitting
//!   the weight table `B(y) = Φ_{r″}(eq(r_hi, y))` and the target `β₀`.
//! * flock `ligerito::recursive_prover_with_basis`/
//!   `recursive_verifier_with_basis_succinct` prove `Σ_y P(y)·B(y) = β₀`
//!   against flock's commitment, with `a` = the packed witness (codeword
//!   side) and `b` = the weight table. The closing residual check evaluates
//!   the weight basis succinctly at the recursion's challenges.
//!
//! Query counts, fold grinding, and the BLAKE3 Merkle hash come from the
//! validator-gated Ligerito security configs ([`sha_lig_configs`]); the
//! `RsOpenConfig::num_queries` knob does not apply on this backend.

#[cfg(test)]
use circuit::linear_map::CscMatrix;

use anyhow::Context;
use flock_core::challenger::Challenger;
mod configuration;
#[cfg(test)]
mod coverage;
pub(crate) mod grinding;
pub use configuration::{LigeritoSelection, ResolvedLigerito};
mod ood;
pub use ood::{ProverOod, VerifierOod, bind_prover_ood, bind_verifier_ood};

use flock_core::field::Gf128;
use flock_core::merkle::HashKind;
use flock_core::pcs::commit::{Commitment, PcsParams, ProverData, commit};
use flock_core::pcs::ligerito::{
    self, LigeritoProof, LigeritoSecurityConfig, ProverConfig as LigProverConfig, SoundnessRegime,
    VerifierConfig as LigVerifierConfig,
};

use crate::cfg_iter_mut;
use crate::piop::lookup::gkr_product::ProductForestProof;
use crate::piop::spartan::grinding::{
    ForestRoundGrinding, GrindingDomain, GrindingRound, ProverGrindingTranscript,
    VerifierGrindingTranscript, grind_and_absorb, verify_and_absorb,
};
use crate::piop::sumcheck::multi_degree::MultiDegreeSumcheckProof;
use crate::poly::univariate::binary_gf128::Gf128 as Gf;
use crate::transcript::traits::Transcript;
use circuit::linear_map::binary_adjoint::{
    AffineTailWeights, BinaryAdjoint, DenseWeightCorrection,
};
#[cfg(test)]
use circuit::linear_map::binary_adjoint::BinaryRowWeights;

use crate::ligerito::{
    IntEvalRsError, LOG_PACKING, RingSwitchProof, RsOpenConfig, RsOpenError, packed_vars,
    phi_bit_sum, phi_byte_tables, phi_from_words, prove_int_eval_merged_common,
    repack_leaf_bits, residual_b_evals, ring_switch_prove,
    ring_switch_verify, row_bit_vars, rs_fast, sv_fold_mfr, verify_int_eval_merged_common,
};
use crate::merged_forest::MergedForestProof;
use crate::pcs::{
    IntegerMatrixLayout, ModQWeightChunks, ModQWeightSource, final_eval_ring,
};
use crate::utils::{cfg_chunks, cfg_chunks_mut, cfg_into_iter, cfg_iter};
use crate::virt_batch::{AffineTailPlanes, RhoTables};

#[cfg(feature = "parallel")]
use rayon::prelude::*;

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
        let _g = tracing::info_span!("lig:grind_pow").entered();
        let seed = self.pow_seed();
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

/// [`commit_rs_flock_with`] at the shape in `cfg` (the BaseFold backend's
/// entry point; the Ligerito path derives its shape from the level config —
/// see [`lig_configs`] + [`commit_rs_ligerito`]).
pub fn commit_rs_flock(
    p: &IntegerMatrixLayout,
    data: &[u128],
    cfg: &RsOpenConfig,
) -> FlockCommitHint {
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
            let weakest = fold_round_bits(lv).into_iter().fold(f64::INFINITY, f64::min);
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

/// Errors of the flock-backed opening / end-to-end verification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FlockRsError {
    PrimeSampling(crate::ext_proj::PrimeSamplingError),

    /// A backend-independent stage failed (forest, binding, pre-sumcheck,
    /// read-off — see [`IntEvalRsError`]).
    Common(IntEvalRsError),
    /// The zinc-side ring-switch rejected.
    RingSwitch(RsOpenError),
    /// The public commitment metadata does not describe the L0 code and
    /// Merkle tree selected by the supplied Ligerito config.
    CommitmentConfig,
    /// `final_b` disagrees with the succinct weight evaluation
    /// `B̂(challenges)` (the tensor-algebra check).
    FinalWeight,
    /// flock's Ligerito succinct verifier rejected (boolean API — the
    /// failing stage is not surfaced).
    LigeritoReject,
    /// A sent chunk fold failed the free range check `u < 2^{c_w+t+W}`.
    ChunkRange {
        chunk: usize,
        col: usize,
    },

    /// A forest/opening per-round grinding nonce is missing, invalid, or
    /// left over (the B.6 proof-of-work hooks).
    ForestGrinding,
    /// The virtual opening's batching message failed the
    /// coefficient-projection check `Σ_i c₀(h_i)·X^i = Σ_l η_l·μ_l`
    /// (paper batching-protocol step 3).
    VirtualBatch,
    /// Round 0 (the out-of-domain sample) is malformed: the proof carries
    /// the round while the parameters skip it (or vice versa), or its
    /// proof-of-work nonce is missing, present at difficulty 0, or invalid.
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

/// Checked verifier-side dimensions for one integer-evaluation instance.
///
/// The protocol's older shape helpers intentionally use wrapping arithmetic
/// and unchecked shifts because their prover callers have already committed
/// to a valid layout. Public verifiers must not feed adversarial parameters
/// into those helpers before rejecting them.
#[derive(Clone, Copy)]
struct IntEvalGeometry {
    rows: usize,
    cols: usize,
    row_bit_vars: usize,
}

fn checked_int_eval_geometry(p: &IntegerMatrixLayout) -> Result<IntEvalGeometry, FlockRsError> {
    let shape = || FlockRsError::RingSwitch(RsOpenError::Shape);
    if !p.word_bits.is_power_of_two() || p.word_bits > u128::BITS as usize {
        return Err(shape());
    }
    let log_word_bits = p.word_bits.trailing_zeros() as usize;
    let Some(row_bit_vars) = p.row_vars.checked_add(log_word_bits) else {
        return Err(shape());
    };
    if row_bit_vars >= usize::BITS as usize {
        return Err(shape());
    }
    let Some(rows) = u32::try_from(p.row_vars)
        .ok()
        .and_then(|t| 1usize.checked_shl(t))
    else {
        return Err(shape());
    };
    let Some(cols) = u32::try_from(p.col_vars)
        .ok()
        .and_then(|s| 1usize.checked_shl(s))
    else {
        return Err(shape());
    };
    Ok(IntEvalGeometry {
        rows,
        cols,
        row_bit_vars,
    })
}

fn validate_int_eval_geometry(
    commitment: &Commitment,
    p: &IntegerMatrixLayout,
    extra_commitment_vars: usize,
) -> Result<IntEvalGeometry, FlockRsError> {
    let shape = || FlockRsError::RingSwitch(RsOpenError::Shape);
    let geometry = checked_int_eval_geometry(p)?;
    if geometry.row_bit_vars < LOG_PACKING {
        return Err(shape());
    }
    let Some(expected_m) = geometry
        .row_bit_vars
        .checked_add(p.col_vars)
        .and_then(|m| m.checked_add(extra_commitment_vars))
    else {
        return Err(shape());
    };
    if expected_m >= usize::BITS as usize || commitment.params.m != expected_m {
        return Err(shape());
    }
    Ok(geometry)
}

fn q_weight_bound(q_bits: usize) -> Option<u128> {
    (1..=126).contains(&q_bits).then(|| 1u128 << q_bits)
}

fn checked_mod_q_geometry(
    p: &IntegerMatrixLayout,
    q_bits: usize,
) -> Result<(IntEvalGeometry, usize, usize), FlockRsError> {
    let shape = || FlockRsError::RingSwitch(RsOpenError::Shape);
    let geometry = checked_int_eval_geometry(p)?;
    if q_weight_bound(q_bits).is_none() {
        return Err(shape());
    }
    let Some(tw) = p.row_vars.checked_add(p.word_bits) else {
        return Err(shape());
    };
    if tw > 126 {
        return Err(shape());
    }
    let c_w = 127usize - tw;
    Ok((geometry, c_w, q_bits.div_ceil(c_w)))
}

fn checked_mod_q_weight_source_geometry<S>(
    p: &IntegerMatrixLayout,
    source: &S,
    q_bits: usize,
) -> Result<(IntEvalGeometry, usize, usize), FlockRsError>
where
    S: ModQWeightSource + ?Sized,
{
    let shape = || FlockRsError::RingSwitch(RsOpenError::Shape);
    let (geometry, chunk_width, chunk_count) = match source.padding_bound() {
        None => checked_mod_q_geometry(p, q_bits)?,
        Some(bound) => {
            if !bound.matches_layout(p) || q_weight_bound(q_bits).is_none() {
                return Err(shape());
            }
            let geometry = checked_int_eval_geometry(p)?;
            let tw = p
                .row_vars
                .checked_add(bound.value_bits())
                .ok_or_else(shape)?;
            if tw > 126 {
                return Err(shape());
            }
            let width = 127 - tw;
            (geometry, width, q_bits.div_ceil(width))
        }
    };
    if source.chunk_count() == 0
        || source.row_count() != geometry.rows
        || source.chunk_width() != chunk_width
        || source.chunk_count() != chunk_count
        || source.q_bits() != q_bits
    {
        return Err(shape());
    }
    Ok((geometry, chunk_width, chunk_count))
}

fn checked_mod_q_weight_chunks_geometry(
    p: &IntegerMatrixLayout,
    chunks: &ModQWeightChunks,
    q_bits: usize,
) -> Result<(IntEvalGeometry, usize, usize), FlockRsError> {
    checked_mod_q_weight_source_geometry(p, chunks, q_bits)
}

fn checked_mod_q_shape(
    commitment: &Commitment,
    proof: &IntEvalRsLigModQProof,
    p: &IntegerMatrixLayout,
    q_bits: usize,
) -> Result<(IntEvalGeometry, usize, usize), FlockRsError> {
    let shape = || FlockRsError::RingSwitch(RsOpenError::Shape);
    let commitment_geometry = validate_int_eval_geometry(commitment, p, 0)?;
    let (geometry, c_w, lch) = checked_mod_q_geometry(p, q_bits)?;
    debug_assert_eq!(geometry.rows, commitment_geometry.rows);
    debug_assert_eq!(geometry.cols, commitment_geometry.cols);
    if proof.mfs.len() != lch
        || proof.us.len() != lch
        || proof.presums.len() != lch
        || proof.rings.len() != lch
        || proof.us.iter().any(|u| u.len() > geometry.cols)
        || proof.rings.iter().any(|ring| ring.s_v.len() != 128)
    {
        return Err(shape());
    }
    if proof
        .presums
        .iter()
        .any(|presum| !presum.has_shape(geometry.row_bit_vars, &[2]))
    {
        return Err(FlockRsError::Common(IntEvalRsError::PreSumcheck));
    }
    Ok((geometry, c_w, lch))
}

// ---------------------------------------------------------------------
// Canonical public-statement binding
// ---------------------------------------------------------------------

#[allow(dead_code)]
const RS_OPEN_STATEMENT_DOMAIN: &[u8] = b"bitz/ligerito-flock/rs-open/v1";
#[allow(dead_code)]
const RS_EVAL_STATEMENT_DOMAIN: &[u8] = b"bitz/ligerito-flock/rs-eval/v1";
#[allow(dead_code)]
const RS_EVAL_BATCH_STATEMENT_DOMAIN: &[u8] = b"bitz/ligerito-flock/rs-eval-batch/v1";
const MOD_Q_STATEMENT_DOMAIN: &[u8] = b"bitz/ligerito-flock/mod-q/v1";
const U32_MOD_Q_WEIGHT_CHUNKS_STATEMENT_DOMAIN: &[u8] = b"bitz/spartan-bitz/u32-mod-q-opening/v2";
const U64_MOD_Q_WEIGHT_CHUNKS_STATEMENT_DOMAIN: &[u8] = b"bitz/spartan-bitz/u64-mod-q-opening/v1";
const U128_MOD_Q_WEIGHT_CHUNKS_STATEMENT_DOMAIN: &[u8] = b"bitz/spartan-bitz/u128-mod-q-opening/v1";
const BABY_BEAR_MOD_Q_WEIGHT_CHUNKS_STATEMENT_DOMAIN: &[u8] =
    b"bitz/spartan-baby-bear-bitz/mod-q-opening/v2";

/// Application relation whose statement domain binds a chunked-weight mod-q
/// opening. The enum is crate-private so callers cannot supply arbitrary
/// transcript-domain bytes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ModQOpeningKind {
    U32Mul,
    BabyBearMul,
    U64Mul,
    U128Mul,
}

impl ModQOpeningKind {
    const fn statement_domain(self) -> &'static [u8] {
        match self {
            Self::U32Mul => U32_MOD_Q_WEIGHT_CHUNKS_STATEMENT_DOMAIN,
            Self::BabyBearMul => BABY_BEAR_MOD_Q_WEIGHT_CHUNKS_STATEMENT_DOMAIN,
            Self::U64Mul => U64_MOD_Q_WEIGHT_CHUNKS_STATEMENT_DOMAIN,
            Self::U128Mul => U128_MOD_Q_WEIGHT_CHUNKS_STATEMENT_DOMAIN,
        }
    }
}

const STATEMENT_FRAME_DOMAIN: &[u8] = b"bitz/ligerito-flock/statement-frame/v1";
const FIELD_BYTES: u8 = 1;
const FIELD_U8: u8 = 2;
const FIELD_U64: u8 = 3;
#[allow(dead_code)]
const FIELD_U128: u8 = 4;
const FIELD_GF128: u8 = 5;
const FIELD_U128_ROWS: u8 = 6;

/// A streaming, typed transcript frame. Every field is encoded as one
/// `absorb_slice` frame containing `(semantic tag, type tag, element count,
/// canonical little-endian payload)`. Streaming the payload through
/// `absorb_inner` avoids materializing a second copy of large row-weight
/// tables while remaining byte-for-byte equivalent to one contiguous
/// `absorb_slice` call.
struct StatementFrame<'a, T: Transcript> {
    transcript: &'a mut T,
}

/// Capability proving that a canonical public statement was absorbed before
/// entering a mod-q after-statement core.
///
/// Its constructor and field are private to this module. Keeping the token
/// affine (neither `Copy` nor `Clone`) makes every core invocation consume one
/// concrete binding operation rather than relying on a call-site comment.
#[must_use = "a bound mod-q statement must be consumed by an after-statement core"]
pub(crate) struct BoundModQStatement {
    _private: (),
}

impl BoundModQStatement {
    const fn new() -> Self {
        Self { _private: () }
    }
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

    #[allow(dead_code)]
    fn gf128s(&mut self, tag: u8, values: &[Gf]) {
        self.begin_field(tag, FIELD_GF128, values.len());
        for value in values {
            for word in value.as_words() {
                self.transcript.absorb_inner(&word.to_le_bytes());
            }
        }
        self.end_field();
    }

    fn u128_rows(&mut self, tag: u8, rows: &[Vec<u128>]) {
        self.begin_field(tag, FIELD_U128_ROWS, rows.len());
        for row in rows {
            self.transcript
                .absorb_inner(&(row.len() as u64).to_le_bytes());
            for &value in row {
                self.transcript.absorb_inner(&value.to_le_bytes());
            }
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
        self.usize(0x22, p.word_bits);
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

#[allow(dead_code)]
fn absorb_rs_open_statement(
    transcript: &mut impl Transcript,
    commitment: &Commitment,
    point: &[Gf],
    config: &impl LigeritoStatementConfig,
) {
    let mut frame = StatementFrame::new(transcript, RS_OPEN_STATEMENT_DOMAIN);
    frame.commitment(commitment);
    frame.ligerito_config(config);
    frame.gf128s(0x30, point);
}

#[allow(dead_code)]
fn absorb_rs_eval_statement(
    transcript: &mut impl Transcript,
    commitment: &Commitment,
    p: &IntegerMatrixLayout,
    row_weights: &[u128],
    alpha: Gf,
    config: &impl LigeritoStatementConfig,
) {
    let mut frame = StatementFrame::new(transcript, RS_EVAL_STATEMENT_DOMAIN);
    frame.commitment(commitment);
    frame.ligerito_config(config);
    frame.int_eval_params(p);
    frame.u128s(0x30, row_weights);
    frame.gf128(0x31, alpha);
}

#[allow(dead_code)]
fn absorb_rs_eval_batch_statement(
    transcript: &mut impl Transcript,
    commitment: &Commitment,
    p: &IntegerMatrixLayout,
    row_weights: &[Vec<u128>],
    alpha: Gf,
    config: &impl LigeritoStatementConfig,
) {
    let mut frame = StatementFrame::new(transcript, RS_EVAL_BATCH_STATEMENT_DOMAIN);
    frame.commitment(commitment);
    frame.ligerito_config(config);
    frame.int_eval_params(p);
    frame.u128_rows(0x30, row_weights);
    frame.gf128(0x31, alpha);
}

/// Bind the statement of the public mod-q opening
/// ([`prove_mle_eval_mod_q_ligerito`], [`verify_mle_eval_mod_q_ligerito`]
/// and their variants) before its first challenge: the commitment, the
/// ladder, the tensor shape, every row weight, the prime width and the
/// generator. The column weights and the claimed value need no frame: they
/// enter only the final read-off, which recombines folds the protocol has
/// already tied to the committed bits under these row weights.
fn absorb_mod_q_statement(
    transcript: &mut impl Transcript,
    commitment: &Commitment,
    p: &IntegerMatrixLayout,
    row_weights_q: &[u128],
    q_bits: usize,
    alpha: Gf,
    config: &impl LigeritoStatementConfig,
) -> BoundModQStatement {
    let mut frame = StatementFrame::new(transcript, MOD_Q_STATEMENT_DOMAIN);
    frame.commitment(commitment);
    frame.ligerito_config(config);
    frame.int_eval_params(p);
    frame.u128s(0x30, row_weights_q);
    frame.usize(0x31, q_bits);
    frame.gf128(0x32, alpha);
    BoundModQStatement::new()
}

/// Whether a ladder is beyond unique decoding (the Johnson regime), whose
/// first level's list only the outer Round 0 binds: such ladders sample out
/// of domain at every level but the first (the crate's solvers and flock's
/// embedded profiles alike), unique-decoding ones at none.
fn needs_round0(config: &impl LigeritoStatementConfig) -> bool {
    config
        .ood_samples()
        .iter()
        .skip(1)
        .any(|&samples| samples > 0)
}

/// Bind a compact statement digest instead of a dense row-weight table while
/// retaining application-level domain separation.
#[allow(clippy::too_many_arguments)]
pub(crate) fn absorb_mod_q_weight_chunks_statement(
    transcript: &mut impl Transcript,
    opening_kind: ModQOpeningKind,
    commitment: &Commitment,
    p: &IntegerMatrixLayout,
    statement_digest: &[u8; 32],
    q_bits: usize,
    alpha: Gf,
    config: &impl LigeritoStatementConfig,
) -> BoundModQStatement {
    let mut frame = StatementFrame::new(transcript, opening_kind.statement_domain());
    frame.commitment(commitment);
    frame.ligerito_config(config);
    frame.int_eval_params(p);
    frame.bytes(0x30, statement_digest);
    frame.usize(0x31, q_bits);
    frame.gf128(0x32, alpha);
    BoundModQStatement::new()
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

/// The evaluation prime of a standalone claim is SAMPLED from the
/// transcript after the commitment, uniformly among the primes of the
/// widest admissible dyadic interval `[2^(b−1), 2^b)`: the paper's
/// Strategy-1 field policy (`b ≤ 113`) capped by the one-chunk exponent-
/// fold width `c_w = 127 − t − W` (so the fold integers never wrap and the
/// forest runs once) — exactly the width rule of the Spartan security
/// profile's derived interval. The claim `⟨eq(·, r₁) ⊗ eq(·, r₂), f⟩ = μ`
/// then uses a transcript-sampled point `(r₁, r₂) ∈ F_q^{t+s}`.
pub fn standalone_q_bits(p: &IntegerMatrixLayout) -> usize {
    crate::pcs::mod_q_chunk_width(p).min(113)
}

/// Miller–Rabin rounds of the transcript prime sampler (the library
/// default: a composite survives with probability `≈ 2^-128`).
pub fn standalone_prime_sampler(q_bits: usize) -> crate::ext_proj::ExtProjParams {
    crate::ext_proj::ExtProjParams {
        prime_bits: q_bits,
        ..crate::ext_proj::ExtProjParams::default()
    }
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
    let q = crate::ext_proj::sample_proj_prime(transcript, &standalone_prime_sampler(q_bits))
        .expect("bounded standalone prime search");
    let arith = field::FpCtx::from_prime_u128(q);
    let r1: Vec<u128> = (0..p.row_vars)
        .map(|_| crate::ext_proj::sample_proj_point(transcript, q))
        .collect();
    let r2: Vec<u128> = (0..p.col_vars)
        .map(|_| crate::ext_proj::sample_proj_point(transcript, q))
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

/// The standalone opening the `bitz` CLI and `benches/pcs.rs` measure: the
/// claim `⟨eq(·, r₁) ⊗ eq(·, r₂), f⟩ = μ` over a transcript-sampled prime and
/// point, with every public input bound before the challenges that depend
/// on it — the statement frame ([`absorb_standalone_mod_q_statement`]), the
/// ladder's policy, Round 0 when the ladder needs it, the prime and point
/// draws ([`sample_standalone_instance`]) and the claim frame
/// ([`absorb_standalone_mod_q_claim`]) — then the opening on that transcript.
/// Every call starts its own transcript, so nothing is left for a caller to
/// bind.
#[derive(Clone, Copy, Debug)]
pub struct StandaloneModQOpening<'a> {
    layout: &'a IntegerMatrixLayout,
    alpha: Gf,
    q_bits: usize,
    ood: Option<OodRoundParams>,
    ligerito: &'a ResolvedLigerito,
}

impl<'a> StandaloneModQOpening<'a> {
    /// The opening of `layout` under `ligerito`, with Round-0 parameters
    /// `ood` (present exactly when the ladder is beyond unique decoding: see
    /// [`ood_round_params`]) and `q_bits`-bit evaluation primes
    /// ([`standalone_q_bits`]).
    pub fn new(
        layout: &'a IntegerMatrixLayout,
        alpha: Gf,
        q_bits: usize,
        ood: Option<OodRoundParams>,
        ligerito: &'a ResolvedLigerito,
    ) -> Result<Self, FlockRsError> {
        if ood.is_some() != ligerito.ood_bits().is_some() {
            return Err(FlockRsError::OodRound);
        }
        Ok(Self {
            layout,
            alpha,
            q_bits,
            ood,
            ligerito,
        })
    }

    fn bind_statement(&self, transcript: &mut impl Transcript, commitment: &Commitment) {
        absorb_standalone_mod_q_statement(
            transcript,
            commitment,
            self.layout,
            self.alpha,
            self.q_bits,
            self.ood,
            self.ligerito.verifier(),
        );
        self.ligerito.bind(transcript);
    }

    /// The instance the transcript samples for `hint`'s commitment, replayed
    /// as [`Self::prove`] runs it (outside any timer: the prover learns its
    /// claim's point from it).
    pub fn instance(&self, hint: &FlockCommitHint) -> StandaloneInstance {
        let mut transcript = crate::transcript::Blake3Transcript::new();
        self.bind_statement(&mut transcript, &hint.commitment);
        let _ = bind_prover_ood(&mut transcript, hint, self.ood);
        sample_standalone_instance(&mut transcript, self.layout, self.q_bits)
    }

    /// Proves the claim with value `claimed_q` on the committed bits of
    /// `hint`.
    pub fn prove(&self, hint: &FlockCommitHint, claimed_q: u128) -> IntEvalRsLigModQProof {
        let mut transcript = crate::transcript::Blake3Transcript::new();
        self.bind_statement(&mut transcript, &hint.commitment);
        let ood = bind_prover_ood(&mut transcript, hint, self.ood);
        let instance = sample_standalone_instance(&mut transcript, self.layout, self.q_bits);
        absorb_standalone_mod_q_claim(&mut transcript, instance.q, claimed_q);
        let chunks = prover_mod_q_chunks(hint, self.layout, &instance.row_weights_q, self.q_bits);
        // The weights are the eq tables of a point drawn after the
        // statement, so every input of the opening is bound by now.
        prove_mle_eval_mod_q_ligerito_after_statement(
            &mut transcript,
            hint,
            self.layout,
            &chunks,
            self.alpha,
            self.ligerito.prover(),
            BoundModQStatement::new(),
            0,
            ood,
        )
    }

    /// Verifies a [`Self::prove`] proof of `claimed_q` against `commitment`.
    pub fn verify(
        &self,
        commitment: &Commitment,
        proof: &IntEvalRsLigModQProof,
        claimed_q: u128,
    ) -> Result<(), FlockRsError> {
        let mut transcript = crate::transcript::Blake3Transcript::new();
        self.bind_statement(&mut transcript, commitment);
        let ood = bind_verifier_ood(
            &mut transcript,
            packed_vars(self.layout),
            self.ood,
            proof.ood.as_ref(),
        )?;
        let instance = sample_standalone_instance(&mut transcript, self.layout, self.q_bits);
        absorb_standalone_mod_q_claim(&mut transcript, instance.q, claimed_q);
        let (chunks, c_w, lch) = runtime_mod_q_chunks(
            commitment,
            proof,
            self.layout,
            &instance.row_weights_q,
            &instance.col_weights_q,
            claimed_q,
            instance.q,
            self.q_bits,
            self.ligerito.verifier(),
        )?;
        verify_mod_q_runtime_after_statement(
            &mut transcript,
            commitment,
            proof,
            self.layout,
            &chunks,
            &instance.col_weights_q,
            claimed_q,
            instance.q,
            (c_w, lch),
            self.alpha,
            self.ligerito.verifier(),
            BoundModQStatement::new(),
            ood,
        )
    }
}

// ---------------------------------------------------------------------
// Ligerito opening: tapered levels, induced bases, grinding, OOD
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
    // The ring switch reads flock's packed words in place (bit-compatible
    // with `Gf`): no 2^{m_p}-element conversion copy.
    let (ring, b_tbl, beta0) = ring_switch_prove(transcript, &hint.p_msg, r_hi);

    let lig = ligerito::recursive_prover_with_basis(
        pc,
        hint.p_msg.as_slice(),
        b_tbl,
        beta0,
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
    let eval_b = |ris: &[Gf128], yr_log_n: usize| -> Vec<Gf128> {
        let ris_gf = ris;
        residual_b_evals(&ris_gf, yr_log_n, &r_hi_gf, &eq_r2)
            .into_iter()
            .collect()
    };
    let ok = ligerito::recursive_verifier_with_basis_succinct(
        vc,
        &proof.lig,
        m_p,
        beta0,
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
    p: &IntegerMatrixLayout,
    row_weights: &[u128],
    alpha: Gf,
    pc: &LigProverConfig,
) -> IntEvalRsLigProof {
    let (mf, v, presum, point) = prove_int_eval_merged_common(
        transcript,
        p,
        &hint.rows,
        Some(hint.packed_cols()),
        row_weights,
        alpha,
    );
    let open = prove_rs_open_ligerito(transcript, hint, &point, pc);
    IntEvalRsLigProof {
        mf,
        v,
        presum,
        open,
    }
}

/// Verify with the Ligerito opening.
#[allow(clippy::too_many_arguments)] // mirrors `f2_int_eval::verify`'s surface
pub fn verify_rs_ligerito<R>(
    transcript: &mut (impl Transcript + Send),
    commitment: &Commitment,
    proof: &IntEvalRsLigProof,
    p: &IntegerMatrixLayout,
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
    p_msg: Vec<Gf128>,
    pub commitment: Commitment,
    prover_data: ProverData,
}

/// Commit `L` same-shape polys as one packed vector (slice ℓ at offset
/// `ℓ·2^{m_p}`), at the shape the Ligerito config dictates.
#[allow(clippy::arithmetic_side_effects)]
pub fn commit_rs_ligerito_batch(
    p: &IntegerMatrixLayout,
    datas: &[Vec<u128>],
    pc: &LigProverConfig,
) -> FlockBatchCommitHint {
    let l = datas.len();
    assert!(
        l.is_power_of_two() && l >= 2,
        "batch size must be a power of two >= 2"
    );
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
                p_msg.push(Gf128 {
                    lo: row[2 * i_hi],
                    hi: row[2 * i_hi + 1],
                });
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
        merkle_hash: pc.merkle_hash,
    };
    let (commitment, prover_data) = commit(&p_msg, &params);
    FlockBatchCommitHint {
        rows: rows_all,
        p_msg,
        commitment,
        prover_data,
    }
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
        v.push(if (ell >> j) & 1 == 1 {
            Gf::one()
        } else {
            Gf::zero()
        });
    }
    v
}

/// Prove `L` integer-MLE evaluations (own points) with one batched Ligerito
/// opening.
#[allow(clippy::arithmetic_side_effects)]
pub fn prove_rs_ligerito_batch(
    transcript: &mut (impl Transcript + Send),
    hint: &FlockBatchCommitHint,
    p: &IntegerMatrixLayout,
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
            transcript,
            p,
            &hint.rows[ell],
            None,
            &row_weights[ell],
            alpha,
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
    let mut b_comb = vec![Gf128::ZERO; l << m_p];
    if rs_fast() {
        for ell in 0..l {
            let tables = phi_byte_tables(&eq_r2, etas[ell]);
            let eq_hi = &eq_his[ell];
            let dst = &mut b_comb[ell * slice..(ell + 1) * slice];
            const CHUNK: usize = 1 << 12;
            cfg_chunks_mut!(dst, CHUNK)
                .enumerate()
                .for_each(|(ci, chunk)| {
                    let base = ci * CHUNK;
                    for (off, slot) in chunk.iter_mut().enumerate() {
                        *slot = phi_from_words(*eq_hi[base + off].as_words(), &tables);
                    }
                });
        }
    } else {
        for ell in 0..l {
            for (y, &ev) in eq_his[ell].iter().enumerate() {
                b_comb[ell * slice + y] = etas[ell] * phi_bit_sum(ev, &eq_r2);
            }
        }
    }
    let mut target = Gf::zero();
    for ell in 0..l {
        let s_u = crate::ligerito::transpose_bits_128(&rings[ell].s_v);
        let beta_ell = s_u
            .iter()
            .zip(eq_r2.iter())
            .fold(Gf::zero(), |a, (su, e)| a + *su * *e);
        target += etas[ell] * beta_ell;
    }

    let lig = ligerito::recursive_prover_with_basis(
        pc,
        hint.p_msg.as_slice(),
        b_comb,
        target,
        &hint.prover_data.codeword,
        &hint.prover_data.merkle_tree,
        &mut ZincChallenger(transcript),
    );
    let _ = log_l;
    IntEvalRsLigBatchProof {
        mfs,
        vs,
        presums,
        rings,
        lig,
    }
}

/// Verify `L` integer-MLE evaluations against the batched commitment.
#[allow(clippy::arithmetic_side_effects)]
#[allow(clippy::too_many_arguments)]
pub fn verify_rs_ligerito_batch<R>(
    transcript: &mut (impl Transcript + Send),
    commitment: &Commitment,
    proof: &IntEvalRsLigBatchProof,
    p: &IntegerMatrixLayout,
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
        let eq_lo =
            crate::poly::utils::build_eq_x_r_vec(&points[ell][..LOG_PACKING], &()).expect("r_lo");
        let claim = ring
            .s_v
            .iter()
            .zip(eq_lo.iter())
            .fold(Gf::zero(), |a, (s, e)| a + *s * *e);
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
        let beta_ell = s_u
            .iter()
            .zip(eq_r2.iter())
            .fold(Gf::zero(), |a, (su, e)| a + *su * *e);
        target += etas[ell] * beta_ell;
    }

    let r_his: Vec<Vec<Gf>> = (0..l)
        .map(|ell| big_r_hi(&points[ell], ell, log_l))
        .collect();
    let eval_b = |ris: &[Gf128], yr_log_n: usize| -> Vec<Gf128> {
        let ris_gf = ris;
        let mut out = vec![Gf::zero(); 1usize << yr_log_n];
        for ell in 0..l {
            let blk = residual_b_evals(&ris_gf, yr_log_n, &r_his[ell], &eq_r2);
            for (o, x) in out.iter_mut().zip(blk.iter()) {
                *o += etas[ell] * *x;
            }
        }
        out
    };
    let ok = ligerito::recursive_verifier_with_basis_succinct(
        vc,
        &proof.lig,
        m_p + log_l,
        target,
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

/// Dense in-pack marginal `s_v[j] = Σ_y eq_hi[y]·bit_j(P[y])`,
/// chunk-parallel with per-chunk accumulators. Default: the
/// method-of-four-Russians kernel ([`sv_fold_mfr`]); `BITZ_RS_FAST=0`
/// restores the scalar bit scan (byte-identical either way).
#[allow(clippy::arithmetic_side_effects)]
fn dense_ring_sv(p_msg: &[Gf128], eq_hi: &[Gf]) -> Vec<Gf> {
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
/// 16 gathers/element, no per-element η multiply); `BITZ_RS_FAST=0` restores
/// the scalar bit scan (byte-identical either way).
#[allow(clippy::arithmetic_side_effects)]
fn fill_phi_basis(b: &mut [Gf128], eq_his: &[Vec<Gf>], etas: &[Gf], eq_r2: &[Gf]) {
    const CHUNK: usize = 1 << 12;
    if rs_fast() {
        let tables: Vec<Vec<Gf>> = eq_his
            .iter()
            .enumerate()
            .map(|(l, _)| phi_byte_tables(eq_r2, etas[l]))
            .collect();
        cfg_chunks_mut!(b, CHUNK)
            .enumerate()
            .for_each(|(ci, chunk)| {
                let base = ci * CHUNK;
                for (off, slot) in chunk.iter_mut().enumerate() {
                    let y = base + off;
                    let mut acc = Gf::zero();
                    for (l, eq_hi) in eq_his.iter().enumerate() {
                        acc += phi_from_words(*eq_hi[y].as_words(), &tables[l]);
                    }
                    *slot = acc;
                }
            });
        return;
    }
    cfg_chunks_mut!(b, CHUNK)
        .enumerate()
        .for_each(|(ci, chunk)| {
            let base = ci * CHUNK;
            for (off, slot) in chunk.iter_mut().enumerate() {
                let y = base + off;
                let mut acc = Gf::zero();
                for (l, eq_hi) in eq_his.iter().enumerate() {
                    acc += etas[l] * phi_bit_sum(eq_hi[y], eq_r2);
                }
                *slot = acc;
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
    b: &mut [Gf128],
    f: &[Gf128],
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
                    acc += phi_from_words(*eq_hi[y].as_words(), &tables[l]);
                }
                *slot = acc;
            }
            let zero = Gf::zero();
            let mut u0 = <Gf as WideMulAcc>::wide_zero(&zero);
            let mut u2 = <Gf as WideMulAcc>::wide_zero(&zero);
            let mut j = 0usize;
            while j + 1 < chunk.len() {
                let b0 = chunk[j];
                let b1 = chunk[j + 1];
                let f0 = f[base + j];
                let f1 = f[base + j + 1];
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

// ---------------------------------------------------------------------
// Round 0 of the paper's `c:core_iop`: the out-of-domain (OOD) sample.
//
// Executed whenever the opener's proximity parameter sits beyond the unique
// decoding radius (flock's Johnson-regime Ligerito configs), skipped in the
// unique-decoding regime (`L_δ = 1`, where the theorem voids `κ_OOD`).
// Right after the commitment is bound, the verifier draws `ζ ∈ K` (behind an
// optional proof-of-work boundary) and the prover answers with
//
//     y = MLE[P](ζ⃗),   ζ⃗ = (ζ^{2^0}, ζ^{2^1}, …, ζ^{2^{m_p−1}}),
//
// an evaluation of the PACKED message `P ∈ K^{2^{m_p}}`. That pins the
// prover to one element of the `δ`-list before any further challenge
// (paper `l:ood_collision`: two distinct list elements agree on `ζ⃗` with
// probability at most `(2^{m_p} − 1)/|K|`, union-bounded over `C(L_δ, 2)`
// pairs; the grinding tops that bound up to the target). The claim is a
// plain `K`-linear claim on `P`, so it rides the final Ligerito opening for
// free: one extra batching draw `η_ood` adds `η_ood·eq(·, ζ⃗)` to the basis
// and `η_ood·y` to the target, and the verifier folds that term succinctly
// (`eq(·, ζ⃗)` is a product, so its partial evaluation is a scalar times
// the tail's eq table).
// ---------------------------------------------------------------------

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
    /// Per-challenge forest/opening grinding nonces, in draw order — one
    /// per challenge drawn between the first forest message and the
    /// batching draws, when the security profile sets a nonzero
    /// difficulty. Empty (and absent from the codec) at difficulty 0.
    pub grinding_nonces: Vec<u64>,
    /// Round 0 (the out-of-domain sample): `Some` iff the round was
    /// executed (see [`OodRoundParams`]).
    pub ood: Option<OodRound>,
}

/// Borrowed common body shared by the direct and virtual mod-q proof formats.
#[derive(Clone, Copy)]
struct ModQLigProofView<'a> {
    mfs: &'a [MergedForestProof],
    us: &'a [Vec<u128>],
    presums: &'a [MultiDegreeSumcheckProof<Gf>],
    lig: &'a LigeritoProof,
    grinding_nonces: &'a [u64],
    ood: Option<&'a OodRound>,
}

impl<'a> From<&'a IntEvalRsLigModQProof> for ModQLigProofView<'a> {
    fn from(proof: &'a IntEvalRsLigModQProof) -> Self {
        Self {
            mfs: &proof.mfs,
            us: &proof.us,
            presums: &proof.presums,
            lig: &proof.lig,
            grinding_nonces: &proof.grinding_nonces,
            ood: proof.ood.as_ref(),
        }
    }
}

/// Common output of the unified prover core. The outer proof formats choose
/// their own reduction-message representation while sharing every other field.
struct ModQLigCoreProof<R> {
    mfs: Vec<MergedForestProof>,
    us: Vec<Vec<u128>>,
    presums: Vec<MultiDegreeSumcheckProof<Gf>>,
    reduction: R,
    lig: LigeritoProof,
    grinding_nonces: Vec<u64>,
    ood: Option<OodRound>,
}

struct PreparedProverLigeritoClaim<R> {
    reduction: R,
    target: Gf,
    basis: Vec<Gf128>,
    precomputed_round0: Option<(Gf, Gf)>,
    grinding_nonces: Vec<u64>,
}

/// The one branch-specific phase between the common pre-sumchecks and the
/// common Ligerito opening.
trait ModQLigProverReduction {
    type Proof;

    fn prepare<T: Transcript + Send>(
        self,
        grinder: ProverGrindingTranscript<'_, T, ForestRoundGrinding>,
        points: &[Vec<Gf>],
        hint: &FlockCommitHint,
        ood: Option<&OodProverClaim>,
    ) -> PreparedProverLigeritoClaim<Self::Proof>;
}

/// Direct and virtual-identity commitment bridge.
struct EqProverReduction {
    packed_vars: usize,
}

impl ModQLigProverReduction for EqProverReduction {
    type Proof = Vec<RingSwitchProof>;

    #[allow(clippy::arithmetic_side_effects)]
    fn prepare<T: Transcript + Send>(
        self,
        mut grinder: ProverGrindingTranscript<'_, T, ForestRoundGrinding>,
        points: &[Vec<Gf>],
        hint: &FlockCommitHint,
        ood: Option<&OodProverClaim>,
    ) -> PreparedProverLigeritoClaim<Self::Proof> {
        let mut rings = Vec::with_capacity(points.len());
        let mut eq_his = Vec::with_capacity(points.len());
        let _g_r = tracing::info_span!("mq:rings").entered();
        for pt in points {
            let eq_hi =
                crate::poly::utils::build_eq_x_r_vec(&pt[LOG_PACKING..], &()).expect("r_hi");
            let s_v = dense_ring_sv(&hint.p_msg, &eq_hi);
            crate::ligerito::absorb_sv(&mut grinder, &s_v);
            rings.push(RingSwitchProof { s_v });
            eq_his.push(eq_hi);
        }
        drop(_g_r);

        let r2: Vec<Gf> = grinder.get_field_challenges(LOG_PACKING, &());
        let eq_r2 = crate::poly::utils::build_eq_x_r_vec(&r2, &()).expect("r2");
        let etas: Vec<Gf> = grinder.get_field_challenges(points.len(), &());
        let eta_ood: Option<Gf> = ood.map(|_| grinder.get_field_challenge(&()));
        let grinding_nonces = grinder.finish();

        let _g_b = tracing::info_span!("mq:bcomb").entered();
        let mut basis = vec![Gf128::ZERO; 1usize << self.packed_vars];
        let mut precomputed_round0 = if rs_fast() {
            Some(fill_phi_basis_round0(
                &mut basis,
                &hint.p_msg,
                &eq_his,
                &etas,
                &eq_r2,
            ))
        } else {
            fill_phi_basis(&mut basis, &eq_his, &etas, &eq_r2);
            None
        };
        let mut target = Gf::zero();
        for (eta, ring) in etas.iter().zip(&rings) {
            let s_u = crate::ligerito::transpose_bits_128(&ring.s_v);
            let beta = s_u
                .iter()
                .zip(eq_r2.iter())
                .fold(Gf::zero(), |acc, (su, e)| acc + *su * *e);
            target += *eta * beta;
        }
        if let (Some(claim), Some(eta)) = (ood, eta_ood) {
            let _g_o = tracing::info_span!("mq:ood_basis").entered();
            add_ood_basis(
                &mut basis,
                &hint.p_msg,
                &claim.point,
                eta,
                precomputed_round0.as_mut(),
            );
            target += eta * claim.y;
        }
        drop(_g_b);

        PreparedProverLigeritoClaim {
            reduction: rings,
            target,
            basis,
            precomputed_round0,
            grinding_nonces,
        }
    }
}

fn prove_prepared_mod_q_ligerito_with_security(
    transcript: &mut (impl Transcript + Send),
    hint: &FlockCommitHint,
    pc: &LigProverConfig,
    basis: Vec<Gf128>,
    target: Gf,
    precomputed_round0: Option<(Gf, Gf)>,
    security: Option<&mut grinding::GrindingContext<'_>>,
) -> LigeritoProof {
    let _g_l = tracing::info_span!("mq:lig").entered();
    if let Some(security) = security {
        let mut challenger = grinding::GrindingChallenger::new(transcript, security);
        let proof = match precomputed_round0 {
            Some((u0, u2)) => ligerito::recursive_prover_with_basis_precomputed_round0(
                pc,
                hint.p_msg.as_slice(),
                basis,
                target,
                &hint.prover_data.codeword,
                &hint.prover_data.merkle_tree,
                ((u0), (u2)),
                None,
                &mut challenger,
            ),
            None => ligerito::recursive_prover_with_basis(
                pc,
                hint.p_msg.as_slice(),
                basis,
                target,
                &hint.prover_data.codeword,
                &hint.prover_data.merkle_tree,
                &mut challenger,
            ),
        };
        assert!(
            challenger.finish(),
            "Flock prover diverged from the grinding plan"
        );
        return proof;
    }
    match precomputed_round0 {
        Some((u0, u2)) => ligerito::recursive_prover_with_basis_precomputed_round0(
            pc,
            hint.p_msg.as_slice(),
            basis,
            target,
            &hint.prover_data.codeword,
            &hint.prover_data.merkle_tree,
            ((u0), (u2)),
            None,
            &mut ZincChallenger(transcript),
        ),
        None => ligerito::recursive_prover_with_basis(
            pc,
            hint.p_msg.as_slice(),
            basis,
            target,
            &hint.prover_data.codeword,
            &hint.prover_data.merkle_tree,
            &mut ZincChallenger(transcript),
        ),
    }
}

/// Transcript-neutral prover shared by direct, virtual-identity, and general
/// virtual openings. The relation rows may be derived `h`, while `hint`
/// always identifies the committed source opened by the final Ligerito call.
#[allow(clippy::arithmetic_side_effects)]
#[allow(clippy::too_many_arguments)]
fn prove_mod_q_lig_core<S, R>(
    transcript: &mut (impl Transcript + Send),
    hint: &FlockCommitHint,
    relation_params: &IntegerMatrixLayout,
    relation_rows: &[Vec<u64>],
    relation_packed_cols: Option<&[Vec<u64>]>,
    chunks: &S,
    alpha: Gf,
    pc: &LigProverConfig,
    forest_grinding_bits: u32,
    ood: impl Into<ProverOod>,
    reduction: R,
) -> ModQLigCoreProof<R::Proof>
where
    S: ModQWeightSource + ?Sized,
    R: ModQLigProverReduction,
{
    prove_mod_q_lig_core_with_security(
        transcript,
        hint,
        relation_params,
        relation_rows,
        relation_packed_cols,
        chunks,
        alpha,
        pc,
        forest_grinding_bits,
        ood,
        reduction,
        None,
    )
}

fn prove_mod_q_lig_core_with_security<S, R>(
    transcript: &mut (impl Transcript + Send),
    hint: &FlockCommitHint,
    relation_params: &IntegerMatrixLayout,
    relation_rows: &[Vec<u64>],
    relation_packed_cols: Option<&[Vec<u64>]>,
    chunks: &S,
    alpha: Gf,
    pc: &LigProverConfig,
    forest_grinding_bits: u32,
    ood: impl Into<ProverOod>,
    reduction: R,
    security: Option<&mut grinding::GrindingContext<'_>>,
) -> ModQLigCoreProof<R::Proof>
where
    S: ModQWeightSource + ?Sized,
    R: ModQLigProverReduction,
{
    let lch = chunks.chunk_count();
    // Round 0 precedes every forest message: it binds the committed
    // message's list element before any further challenge.
    let ood_claim = ood.into().claim(transcript, hint);
    let mut grinder: ProverGrindingTranscript<_, ForestRoundGrinding> =
        ProverGrindingTranscript::new(transcript, forest_grinding_bits);
    let mut mfs = Vec::with_capacity(lch);
    let mut us = Vec::with_capacity(lch);
    let mut presums = Vec::with_capacity(lch);
    let mut points = Vec::with_capacity(lch);
    for chunk_index in 0..lch {
        let (mf, u, presum, point) = chunks
            .with_chunk(chunk_index, |weights| {
                crate::ligerito::prove_int_eval_merged_bounded(
                    &mut grinder,
                    relation_params,
                    relation_rows,
                    relation_packed_cols,
                    weights,
                    alpha,
                    chunks
                        .padding_bound()
                        .map_or(relation_params.word_bits, |b| b.value_bits()),
                )
            })
            .expect("validated weight source must materialize every chunk");
        mfs.push(mf);
        us.push(u);
        presums.push(presum);
        points.push(point);
    }

    let prepared = reduction.prepare(grinder, &points, hint, ood_claim.as_ref());
    let lig = prove_prepared_mod_q_ligerito_with_security(
        transcript,
        hint,
        pc,
        prepared.basis,
        prepared.target,
        prepared.precomputed_round0,
        security,
    );
    ModQLigCoreProof {
        mfs,
        us,
        presums,
        reduction: prepared.reduction,
        lig,
        grinding_nonces: prepared.grinding_nonces,
        ood: ood_claim.map(|claim| claim.round),
    }
}

#[allow(clippy::too_many_arguments)]
fn prove_mod_q_lig_after_statement<S, R>(
    transcript: &mut (impl Transcript + Send),
    hint: &FlockCommitHint,
    relation_params: &IntegerMatrixLayout,
    relation_rows: &[Vec<u64>],
    relation_packed_cols: Option<&[Vec<u64>]>,
    chunks: &S,
    alpha: Gf,
    pc: &LigProverConfig,
    _bound_statement: BoundModQStatement,
    forest_grinding_bits: u32,
    ood: impl Into<ProverOod>,
    reduction: R,
) -> ModQLigCoreProof<R::Proof>
where
    S: ModQWeightSource + ?Sized,
    R: ModQLigProverReduction,
{
    prove_mod_q_lig_core(
        transcript,
        hint,
        relation_params,
        relation_rows,
        relation_packed_cols,
        chunks,
        alpha,
        pc,
        forest_grinding_bits,
        ood,
        reduction,
    )
}

fn into_direct_mod_q_proof(core: ModQLigCoreProof<Vec<RingSwitchProof>>) -> IntEvalRsLigModQProof {
    IntEvalRsLigModQProof {
        mfs: core.mfs,
        us: core.us,
        presums: core.presums,
        rings: core.reduction,
        lig: core.lig,
        grinding_nonces: core.grinding_nonces,
        ood: core.ood,
    }
}

/// Prove `MLE[INT(D)](r) = y ∈ 𝔽_q` with the Ligerito opening.
/// `row_weights_q[b] = eq(b, r₁) mod q ∈ [0, q)`. Reads the committed bits
/// straight from `hint.rows` — no `u128` data tensor. Binds its statement
/// itself (as [`prove_mle_eval_mod_q_ligerito_with_ood`]). Round 0 (the
/// out-of-domain sample) is skipped, so a Johnson-regime ladder is refused;
/// see [`prove_mle_eval_mod_q_ligerito_with_ood`].
#[allow(clippy::arithmetic_side_effects)]
pub fn prove_mle_eval_mod_q_ligerito(
    transcript: &mut (impl Transcript + Send),
    hint: &FlockCommitHint,
    p: &IntegerMatrixLayout,
    row_weights_q: &[u128],
    q_bits: usize,
    alpha: Gf,
    pc: &LigProverConfig,
) -> IntEvalRsLigModQProof {
    prove_mle_eval_mod_q_ligerito_with_ood(
        transcript,
        hint,
        p,
        row_weights_q,
        q_bits,
        alpha,
        None,
        pc,
    )
}

/// [`prove_mle_eval_mod_q_ligerito`] with Round 0 (the out-of-domain
/// sample) executed when `ood` is `Some` — required whenever `pc` is a
/// Johnson-regime (beyond unique decoding) Ligerito config, which is refused
/// without it; derive `ood` with [`ood_round_params`]. Binds its statement
/// first (the commitment, the ladder, the shape, the row weights, `q_bits`
/// and `alpha`), so a caller need not; anything a caller absorbs before is
/// bound as well.
#[allow(clippy::arithmetic_side_effects)]
#[allow(clippy::too_many_arguments)]
pub fn prove_mle_eval_mod_q_ligerito_with_ood(
    transcript: &mut (impl Transcript + Send),
    hint: &FlockCommitHint,
    p: &IntegerMatrixLayout,
    row_weights_q: &[u128],
    q_bits: usize,
    alpha: Gf,
    ood: impl Into<ProverOod>,
    pc: &LigProverConfig,
) -> IntEvalRsLigModQProof {
    let chunks = prover_mod_q_chunks(hint, p, row_weights_q, q_bits);
    let ood = ood.into();
    assert!(
        ood.runs_round0() || !needs_round0(pc),
        "a Johnson-regime Ligerito config needs Round 0: pass its OodRoundParams"
    );
    let bound_statement = absorb_mod_q_statement(
        transcript,
        &hint.commitment,
        p,
        row_weights_q,
        q_bits,
        alpha,
        pc,
    );
    prove_mle_eval_mod_q_ligerito_after_statement(
        transcript,
        hint,
        p,
        &chunks,
        alpha,
        pc,
        bound_statement,
        0,
        ood,
    )
}

/// The checks and weight chunks of a mod-q prover opening, before any
/// transcript work (invalid inputs panic, as the public prover always has).
fn prover_mod_q_chunks(
    hint: &FlockCommitHint,
    p: &IntegerMatrixLayout,
    row_weights_q: &[u128],
    q_bits: usize,
) -> ModQWeightChunks {
    let geometry = validate_int_eval_geometry(&hint.commitment, p, 0)
        .expect("valid integer-evaluation commitment geometry");
    checked_mod_q_geometry(p, q_bits).expect("valid mod-q geometry");
    assert_eq!(row_weights_q.len(), geometry.rows, "row-weight length");
    let _g = tracing::info_span!("mq:chunking").entered();
    ModQWeightChunks::from_dense(p, row_weights_q, q_bits)
        .expect("q_bits must be in [1, 126] and every row weight must be < 2^q_bits")
}

/// Prove a mod-q MLE evaluation whose row weights are already represented as
/// validated base-2^c_w chunks. All static checks complete before the
/// application-domain-separated statement frame mutates the transcript.
#[allow(clippy::arithmetic_side_effects)]
#[allow(clippy::too_many_arguments)]
pub(crate) fn prove_mle_eval_mod_q_ligerito_with_weight_chunks(
    transcript: &mut (impl Transcript + Send),
    opening_kind: ModQOpeningKind,
    hint: &FlockCommitHint,
    p: &IntegerMatrixLayout,
    chunks: &ModQWeightChunks,
    statement_digest: &[u8; 32],
    q_bits: usize,
    alpha: Gf,
    forest_grinding_bits: u32,
    ood: impl Into<ProverOod>,
    pc: &LigProverConfig,
) -> Result<IntEvalRsLigModQProof, FlockRsError> {
    validate_ligerito_commitment(&hint.commitment, pc)?;
    if chunks.padding_bound().is_some() {
        return Err(FlockRsError::RingSwitch(RsOpenError::Shape));
    }
    let commitment_geometry = validate_int_eval_geometry(&hint.commitment, p, 0)?;
    let (geometry, _, _) = checked_mod_q_weight_chunks_geometry(p, chunks, q_bits)?;
    let expected_words = 1usize
        .checked_shl(
            u32::try_from(geometry.row_bit_vars)
                .map_err(|_| FlockRsError::RingSwitch(RsOpenError::Shape))?,
        )
        .and_then(|bits| bits.checked_div(u64::BITS as usize))
        .ok_or(FlockRsError::RingSwitch(RsOpenError::Shape))?;
    if geometry.rows != commitment_geometry.rows
        || geometry.cols != commitment_geometry.cols
        || hint.rows.len() != geometry.cols
        || hint.rows.iter().any(|row| row.len() != expected_words)
    {
        return Err(FlockRsError::RingSwitch(RsOpenError::Shape));
    }

    let bound_statement = absorb_mod_q_weight_chunks_statement(
        transcript,
        opening_kind,
        &hint.commitment,
        p,
        statement_digest,
        q_bits,
        alpha,
        pc,
    );
    Ok(prove_mle_eval_mod_q_ligerito_after_statement(
        transcript,
        hint,
        p,
        chunks,
        alpha,
        pc,
        bound_statement,
        forest_grinding_bits,
        ood,
    ))
}

/// Mod-q prover after the surrounding protocol has already bound a statement
/// containing the commitment root and row-weight claim.
#[allow(clippy::arithmetic_side_effects)]
#[allow(clippy::too_many_arguments)]
fn prove_mle_eval_mod_q_ligerito_after_statement<S>(
    transcript: &mut (impl Transcript + Send),
    hint: &FlockCommitHint,
    p: &IntegerMatrixLayout,
    chunks: &S,
    alpha: Gf,
    pc: &LigProverConfig,
    bound_statement: BoundModQStatement,
    forest_grinding_bits: u32,
    ood: impl Into<ProverOod>,
) -> IntEvalRsLigModQProof
where
    S: ModQWeightSource + ?Sized,
{
    into_direct_mod_q_proof(prove_mod_q_lig_after_statement(
        transcript,
        hint,
        p,
        &hint.rows,
        Some(hint.packed_cols()),
        chunks,
        alpha,
        pc,
        bound_statement,
        forest_grinding_bits,
        ood,
        EqProverReduction {
            packed_vars: packed_vars(p),
        },
    ))
}

/// Transcript-neutral mod-q prover core with no statement of its own (the
/// tests' handle on the bare opening; every entry point binds one first).
///
/// At a nonzero `forest_grinding_bits`, every challenge drawn between the
/// first forest message and the r″/η batching draws is preceded by one
/// [`ForestRoundGrinding`] proof-of-work boundary (the B.6 hooks); the
/// nonces ride the proof. At difficulty 0 not one transcript byte moves.
#[cfg(test)]
#[allow(clippy::arithmetic_side_effects)]
fn prove_mle_eval_mod_q_ligerito_raw<S>(
    transcript: &mut (impl Transcript + Send),
    hint: &FlockCommitHint,
    p: &IntegerMatrixLayout,
    chunks: &S,
    alpha: Gf,
    pc: &LigProverConfig,
    forest_grinding_bits: u32,
    ood: impl Into<ProverOod>,
) -> IntEvalRsLigModQProof
where
    S: ModQWeightSource + ?Sized,
{
    into_direct_mod_q_proof(prove_mod_q_lig_core(
        transcript,
        hint,
        p,
        &hint.rows,
        Some(hint.packed_cols()),
        chunks,
        alpha,
        pc,
        forest_grinding_bits,
        ood,
        EqProverReduction {
            packed_vars: packed_vars(p),
        },
    ))
}

/// Re-padded chunk folds in the chunk-major order consumed by all read-off
/// implementations. Keeping one flat buffer avoids rebuilding it in every
/// direct, runtime-prime, and virtual wrapper.
struct PaddedChunkFolds {
    values: Vec<u128>,
    cols: usize,
}

impl PaddedChunkFolds {
    fn chunk(&self, index: usize) -> &[u128] {
        let start = index * self.cols;
        &self.values[start..start + self.cols]
    }

    fn chunks(&self) -> impl ExactSizeIterator<Item = &[u128]> {
        self.values.chunks_exact(self.cols)
    }
}

fn verify_mod_q_lig_preflight<S, C>(
    proof: ModQLigProofView<'_>,
    p: &IntegerMatrixLayout,
    chunks: &S,
    read_off: C,
) -> Result<PaddedChunkFolds, FlockRsError>
where
    S: ModQWeightSource + ?Sized,
    C: FnOnce(&[u128], usize, usize) -> Result<(), FlockRsError>,
{
    let shape = || FlockRsError::RingSwitch(RsOpenError::Shape);
    let geometry = checked_int_eval_geometry(p)?;
    let chunk_count = chunks.chunk_count();
    if proof.mfs.len() != chunk_count
        || proof.us.len() != chunk_count
        || proof.presums.len() != chunk_count
    {
        return Err(shape());
    }
    if proof
        .presums
        .iter()
        .any(|presum| !presum.has_shape(geometry.row_bit_vars, &[2]))
    {
        return Err(FlockRsError::Common(IntEvalRsError::PreSumcheck));
    }

    let range_shift = chunks
        .chunk_width()
        .checked_add(p.row_vars)
        .and_then(|shift| {
            shift.checked_add(
                chunks
                    .padding_bound()
                    .map_or(p.word_bits, |b| b.value_bits()),
            )
        })
        .ok_or_else(shape)?;
    let shift = u32::try_from(range_shift).map_err(|_| shape())?;
    let bound = 1u128.checked_shl(shift).ok_or_else(shape)?;
    let total = geometry.cols.checked_mul(chunk_count).ok_or_else(shape)?;
    let mut values = vec![0u128; total];
    for (chunk, transmitted) in proof.us.iter().enumerate() {
        if transmitted.len() > geometry.cols {
            return Err(shape());
        }
        for (col, &value) in transmitted.iter().enumerate() {
            if value >= bound {
                return Err(FlockRsError::ChunkRange { chunk, col });
            }
        }
        let start = chunk * geometry.cols;
        values[start..start + transmitted.len()].copy_from_slice(transmitted);
    }

    read_off(&values, chunks.chunk_width(), chunk_count)?;
    Ok(PaddedChunkFolds {
        values,
        cols: geometry.cols,
    })
}

enum PreparedLigeritoBasis {
    Eq {
        r_his: Vec<Vec<Gf>>,
        eq_r2: Vec<Gf>,
        etas: Vec<Gf>,
    },
    Dense {
        a_prime: Vec<Gf>,
    },
}

impl PreparedLigeritoBasis {
    #[allow(clippy::arithmetic_side_effects)]
    fn evaluate(&self, ris: &[Gf128], remaining_vars: usize) -> Vec<Gf128> {
        match self {
            Self::Eq { r_his, eq_r2, etas } => {
                let ris_gf = ris;
                let mut out = vec![Gf::zero(); 1usize << remaining_vars];
                for (r_hi, eta) in r_his.iter().zip(etas) {
                    let block = residual_b_evals(&ris_gf, remaining_vars, r_hi, eq_r2);
                    for (output, value) in out.iter_mut().zip(block) {
                        *output += *eta * value;
                    }
                }
                out
            }
            Self::Dense { a_prime } => {
                let mut table = a_prime.clone();
                for &challenge in ris {
                    crate::ligerito::bind_low(&mut table, challenge);
                }
                debug_assert_eq!(table.len(), 1usize << remaining_vars);
                table
            }
        }
    }
}

struct PreparedLigeritoClaim {
    packed_vars: usize,
    target: Gf,
    basis: PreparedLigeritoBasis,
    /// The Round-0 term `η_ood·eq(·, ζ⃗)` of the basis, folded succinctly.
    ood: Option<(Vec<Gf>, Gf)>,
}

trait ModQLigVerifierReduction {
    fn validate_shape(&self, chunk_count: usize) -> Result<(), FlockRsError>;

    /// Packed variables of the committed source the final opening runs on.
    fn packed_vars(&self) -> usize;

    fn prepare<T: Transcript + Send>(
        self,
        grinder: VerifierGrindingTranscript<'_, '_, T, ForestRoundGrinding>,
        points: &[Vec<Gf>],
        mus: &[Gf],
        ood: Option<&OodVerifierClaim>,
    ) -> Result<PreparedLigeritoClaim, FlockRsError>;
}

struct EqVerifierReduction<'a> {
    rings: &'a [RingSwitchProof],
    packed_vars: usize,
}

impl ModQLigVerifierReduction for EqVerifierReduction<'_> {
    fn validate_shape(&self, chunk_count: usize) -> Result<(), FlockRsError> {
        if self.rings.len() != chunk_count || self.rings.iter().any(|ring| ring.s_v.len() != 128) {
            return Err(FlockRsError::RingSwitch(RsOpenError::Shape));
        }
        Ok(())
    }

    fn packed_vars(&self) -> usize {
        self.packed_vars
    }

    #[allow(clippy::arithmetic_side_effects)]
    fn prepare<T: Transcript + Send>(
        self,
        mut grinder: VerifierGrindingTranscript<'_, '_, T, ForestRoundGrinding>,
        points: &[Vec<Gf>],
        mus: &[Gf],
        ood: Option<&OodVerifierClaim>,
    ) -> Result<PreparedLigeritoClaim, FlockRsError> {
        for ((point, mu), ring) in points.iter().zip(mus).zip(self.rings) {
            let eq_lo =
                crate::poly::utils::build_eq_x_r_vec(&point[..LOG_PACKING], &()).expect("r_lo");
            let claim = ring
                .s_v
                .iter()
                .zip(eq_lo.iter())
                .fold(Gf::zero(), |acc, (s, e)| acc + *s * *e);
            if claim != *mu {
                return Err(FlockRsError::RingSwitch(RsOpenError::RingSwitchClaim));
            }
            crate::ligerito::absorb_sv(&mut grinder, &ring.s_v);
        }
        let r2: Vec<Gf> = grinder.get_field_challenges(LOG_PACKING, &());
        let eq_r2 = crate::poly::utils::build_eq_x_r_vec(&r2, &()).expect("r2");
        let etas: Vec<Gf> = grinder.get_field_challenges(points.len(), &());
        let eta_ood: Option<Gf> = ood.map(|_| grinder.get_field_challenge(&()));
        grinder.finish().map_err(|_| FlockRsError::ForestGrinding)?;

        let mut target = Gf::zero();
        for (eta, ring) in etas.iter().zip(self.rings) {
            let s_u = crate::ligerito::transpose_bits_128(&ring.s_v);
            let beta = s_u
                .iter()
                .zip(eq_r2.iter())
                .fold(Gf::zero(), |acc, (su, e)| acc + *su * *e);
            target += *eta * beta;
        }
        let ood_term = ood.zip(eta_ood).map(|(claim, eta)| {
            target += eta * claim.y;
            (claim.point.clone(), eta)
        });
        let r_his = points
            .iter()
            .map(|point| point[LOG_PACKING..].to_vec())
            .collect();
        Ok(PreparedLigeritoClaim {
            packed_vars: self.packed_vars,
            target,
            basis: PreparedLigeritoBasis::Eq { r_his, eq_r2, etas },
            ood: ood_term,
        })
    }
}

fn verify_prepared_mod_q_ligerito_with_security(
    transcript: &mut (impl Transcript + Send),
    commitment: &Commitment,
    proof: &LigeritoProof,
    vc: &LigVerifierConfig,
    prepared: PreparedLigeritoClaim,
    security: Option<&mut grinding::GrindingContext<'_>>,
) -> Result<(), FlockRsError> {
    let eval_b = |ris: &[Gf128], remaining_vars: usize| {
        let mut out = prepared.basis.evaluate(ris, remaining_vars);
        if let Some((point, eta)) = &prepared.ood {
            let add = ood_residual_evals(ris, remaining_vars, point, *eta);
            for (slot, term) in out.iter_mut().zip(add) {
                *slot = (*slot) + term;
            }
        }
        out
    };
    if let Some(security) = security {
        let mut challenger = grinding::GrindingChallenger::new(transcript, security);
        let ok = ligerito::recursive_verifier_with_basis_succinct(
            vc,
            proof,
            prepared.packed_vars,
            prepared.target,
            &commitment.root,
            eval_b,
            &mut challenger,
        );
        return if ok && challenger.finish() {
            Ok(())
        } else {
            Err(FlockRsError::LigeritoReject)
        };
    }
    let ok = ligerito::recursive_verifier_with_basis_succinct(
        vc,
        proof,
        prepared.packed_vars,
        prepared.target,
        &commitment.root,
        eval_b,
        &mut ZincChallenger(transcript),
    );
    if !ok {
        return Err(FlockRsError::LigeritoReject);
    }
    Ok(())
}

/// Verify `MLE[INT(D)](r) = claimed ∈ R` (R char ≠ 2, e.g. 𝔽_q).
/// `col_weights[c] = eq(c, r₂) ∈ R`. Binds its statement itself (as
/// [`prove_mle_eval_mod_q_ligerito_with_ood`]). Round 0 skipped, so a
/// Johnson-regime ladder is rejected; see
/// [`verify_mle_eval_mod_q_ligerito_with_ood`].
#[allow(clippy::arithmetic_side_effects)]
#[allow(clippy::too_many_arguments)]
pub fn verify_mle_eval_mod_q_ligerito<R>(
    transcript: &mut (impl Transcript + Send),
    commitment: &Commitment,
    proof: &IntEvalRsLigModQProof,
    p: &IntegerMatrixLayout,
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
    verify_mle_eval_mod_q_ligerito_with_ood(
        transcript,
        commitment,
        proof,
        p,
        row_weights_q,
        col_weights,
        alpha,
        claimed,
        q_bits,
        None,
        vc,
    )
}

/// Verify a standalone opening under a transcript-sampled (runtime) prime
/// `q`: every weight and the claim are canonical integers in `[0, q)` and
/// the read-off is recombined modulo `q` with [`field::FpCtx<2>`].
/// `ood` must equal the prover's ([`ood_round_params`]); a Johnson-regime
/// ladder without Round 0 is rejected. Binds its statement first, as
/// [`prove_mle_eval_mod_q_ligerito_with_ood`] does.
#[allow(clippy::arithmetic_side_effects)]
#[allow(clippy::too_many_arguments)]
pub fn verify_mle_eval_mod_q_ligerito_runtime(
    transcript: &mut (impl Transcript + Send),
    commitment: &Commitment,
    proof: &IntEvalRsLigModQProof,
    p: &IntegerMatrixLayout,
    row_weights_q: &[u128],
    col_weights_q: &[u128],
    alpha: Gf,
    claimed_q: u128,
    q: u128,
    q_bits: usize,
    ood: impl Into<VerifierOod>,
    vc: &LigVerifierConfig,
) -> Result<(), FlockRsError> {
    let (chunks, c_w, lch) = runtime_mod_q_chunks(
        commitment,
        proof,
        p,
        row_weights_q,
        col_weights_q,
        claimed_q,
        q,
        q_bits,
        vc,
    )?;
    let ood = ood.into();
    if !ood.runs_round0() && needs_round0(vc) {
        return Err(FlockRsError::OodRound);
    }
    let bound_statement =
        absorb_mod_q_statement(transcript, commitment, p, row_weights_q, q_bits, alpha, vc);
    verify_mod_q_runtime_after_statement(
        transcript,
        commitment,
        proof,
        p,
        &chunks,
        col_weights_q,
        claimed_q,
        q,
        (c_w, lch),
        alpha,
        vc,
        bound_statement,
        ood,
    )
}

/// The checks and weight chunks of a runtime-prime verifier opening, before
/// any transcript work.
#[allow(clippy::too_many_arguments)]
fn runtime_mod_q_chunks(
    commitment: &Commitment,
    proof: &IntEvalRsLigModQProof,
    p: &IntegerMatrixLayout,
    row_weights_q: &[u128],
    col_weights_q: &[u128],
    claimed_q: u128,
    q: u128,
    q_bits: usize,
    vc: &LigVerifierConfig,
) -> Result<(ModQWeightChunks, usize, usize), FlockRsError> {
    validate_runtime_q(q, q_bits, row_weights_q)?;
    if claimed_q >= q || col_weights_q.iter().any(|&weight| weight >= q) {
        return Err(FlockRsError::RingSwitch(RsOpenError::Shape));
    }
    validate_ligerito_commitment(commitment, vc)?;
    let (geometry, c_w, lch) = checked_mod_q_shape(commitment, proof, p, q_bits)?;
    if row_weights_q.len() != geometry.rows || col_weights_q.len() != geometry.cols {
        return Err(FlockRsError::RingSwitch(RsOpenError::Shape));
    }
    let _g = tracing::info_span!("mv:chunking").entered();
    let chunks = ModQWeightChunks::from_dense(p, row_weights_q, q_bits)
        .map_err(|()| FlockRsError::RingSwitch(RsOpenError::Shape))?;
    Ok((chunks, c_w, lch))
}

/// The runtime-prime verifier once its statement is bound: the opening
/// core, then the read-off recombined modulo `q`.
#[allow(clippy::too_many_arguments)]
fn verify_mod_q_runtime_after_statement(
    transcript: &mut (impl Transcript + Send),
    commitment: &Commitment,
    proof: &IntEvalRsLigModQProof,
    p: &IntegerMatrixLayout,
    chunks: &ModQWeightChunks,
    col_weights_q: &[u128],
    claimed_q: u128,
    q: u128,
    (c_w, lch): (usize, usize),
    alpha: Gf,
    vc: &LigVerifierConfig,
    bound_statement: BoundModQStatement,
    ood: VerifierOod,
) -> Result<(), FlockRsError> {
    let arithmetic = field::FpCtx::from_prime_u128(q);
    verify_mod_q_lig_after_statement(
        transcript,
        commitment,
        proof.into(),
        p,
        chunks,
        alpha,
        vc,
        bound_statement,
        0,
        ood,
        EqVerifierReduction {
            rings: &proof.rings,
            packed_vars: packed_vars(p),
        },
        |values, _, _| {
            if recombine_read_off_runtime(p, values, col_weights_q, c_w, lch, &arithmetic)
                != claimed_q
            {
                return Err(FlockRsError::Common(IntEvalRsError::ReadOff));
            }
            Ok(())
        },
    )?;
    Ok(())
}

/// [`verify_mle_eval_mod_q_ligerito`] with Round 0 (the out-of-domain
/// sample) verified when `ood` is `Some`; `ood` must equal the prover's, and
/// a Johnson-regime ladder without it is rejected. Binds its statement
/// first, as [`prove_mle_eval_mod_q_ligerito_with_ood`] does.
#[allow(clippy::arithmetic_side_effects)]
#[allow(clippy::too_many_arguments)]
pub fn verify_mle_eval_mod_q_ligerito_with_ood<R>(
    transcript: &mut (impl Transcript + Send),
    commitment: &Commitment,
    proof: &IntEvalRsLigModQProof,
    p: &IntegerMatrixLayout,
    row_weights_q: &[u128],
    col_weights: &[R],
    alpha: Gf,
    claimed: R,
    q_bits: usize,
    ood: impl Into<VerifierOod>,
    vc: &LigVerifierConfig,
) -> Result<(), FlockRsError>
where
    R: Copy + PartialEq + From<u128> + core::ops::Add<Output = R> + core::ops::Mul<Output = R>,
{
    validate_ligerito_commitment(commitment, vc)?;
    let (geometry, c_w, lch) = checked_mod_q_shape(commitment, proof, p, q_bits)?;
    if row_weights_q.len() != geometry.rows || col_weights.len() != geometry.cols {
        return Err(FlockRsError::RingSwitch(RsOpenError::Shape));
    }
    let chunks = {
        let _g = tracing::info_span!("mv:chunking").entered();
        ModQWeightChunks::from_dense(p, row_weights_q, q_bits)
            .map_err(|()| FlockRsError::RingSwitch(RsOpenError::Shape))?
    };
    let ood = ood.into();
    if !ood.runs_round0() && needs_round0(vc) {
        return Err(FlockRsError::OodRound);
    }
    let bound_statement =
        absorb_mod_q_statement(transcript, commitment, p, row_weights_q, q_bits, alpha, vc);
    use crate::pcs::recombine_read_off;
    verify_mod_q_lig_after_statement(
        transcript,
        commitment,
        proof.into(),
        p,
        &chunks,
        alpha,
        vc,
        bound_statement,
        0,
        ood,
        EqVerifierReduction {
            rings: &proof.rings,
            packed_vars: packed_vars(p),
        },
        |values, _, _| {
            let y = recombine_read_off(p, values, 0, col_weights, c_w, lch);
            if y != claimed {
                return Err(FlockRsError::Common(IntEvalRsError::ReadOff));
            }
            Ok(())
        },
    )?;
    Ok(())
}

/// Verify a chunked-weight opening under a runtime modulus. The final read-off
/// uses explicit canonical mod-q arithmetic because a transcript-sampled prime
/// cannot be represented by a compile-time modulus on a generic ring type.
#[allow(clippy::arithmetic_side_effects)]
#[allow(clippy::too_many_arguments)]
pub(crate) fn verify_mle_eval_mod_q_ligerito_with_weight_chunks_runtime(
    transcript: &mut (impl Transcript + Send),
    opening_kind: ModQOpeningKind,
    commitment: &Commitment,
    proof: &IntEvalRsLigModQProof,
    p: &IntegerMatrixLayout,
    chunks: &ModQWeightChunks,
    col_weights_q: &[u128],
    statement_digest: &[u8; 32],
    alpha: Gf,
    claimed_q: u128,
    q: u128,
    q_bits: usize,
    forest_grinding_bits: u32,
    ood: impl Into<VerifierOod>,
    vc: &LigVerifierConfig,
) -> Result<(), FlockRsError> {
    validate_runtime_q_source(q, q_bits, chunks)?;
    validate_ligerito_commitment(commitment, vc)?;
    let (geometry, chunk_width, chunk_count) = checked_mod_q_shape(commitment, proof, p, q_bits)?;
    let (chunk_geometry, expected_chunk_width, expected_chunk_count) =
        checked_mod_q_weight_chunks_geometry(p, chunks, q_bits)?;
    if geometry.rows != chunk_geometry.rows
        || geometry.cols != chunk_geometry.cols
        || chunk_width != expected_chunk_width
        || chunk_count != expected_chunk_count
        || col_weights_q.len() != geometry.cols
        || claimed_q >= q
        || col_weights_q.iter().any(|&weight| weight >= q)
    {
        return Err(FlockRsError::RingSwitch(RsOpenError::Shape));
    }

    let bound_statement = absorb_mod_q_weight_chunks_statement(
        transcript,
        opening_kind,
        commitment,
        p,
        statement_digest,
        q_bits,
        alpha,
        vc,
    );
    let arithmetic = field::FpCtx::from_prime_u128(q);
    verify_mod_q_lig_after_statement(
        transcript,
        commitment,
        proof.into(),
        p,
        chunks,
        alpha,
        vc,
        bound_statement,
        forest_grinding_bits,
        ood,
        EqVerifierReduction {
            rings: &proof.rings,
            packed_vars: packed_vars(p),
        },
        |values, _, _| {
            if recombine_read_off_runtime(
                p,
                values,
                col_weights_q,
                chunk_width,
                chunk_count,
                &arithmetic,
            ) != claimed_q
            {
                return Err(FlockRsError::Common(IntEvalRsError::ReadOff));
            }
            Ok(())
        },
    )?;
    Ok(())
}

/// Verify a mod-q MLE evaluation whose row weights are already represented as
/// validated base-2^c_w chunks, including the final read-off in `R`.
#[allow(clippy::arithmetic_side_effects)]
#[allow(clippy::too_many_arguments)]
pub(crate) fn verify_mle_eval_mod_q_ligerito_with_weight_chunks<R>(
    transcript: &mut (impl Transcript + Send),
    opening_kind: ModQOpeningKind,
    commitment: &Commitment,
    proof: &IntEvalRsLigModQProof,
    p: &IntegerMatrixLayout,
    chunks: &ModQWeightChunks,
    col_weights: &[R],
    statement_digest: &[u8; 32],
    alpha: Gf,
    claimed: R,
    q_bits: usize,
    forest_grinding_bits: u32,
    ood: impl Into<VerifierOod>,
    vc: &LigVerifierConfig,
) -> Result<(), FlockRsError>
where
    R: Copy + PartialEq + From<u128> + core::ops::Add<Output = R> + core::ops::Mul<Output = R>,
{
    validate_ligerito_commitment(commitment, vc)?;
    let (geometry, chunk_width, chunk_count) = checked_mod_q_shape(commitment, proof, p, q_bits)?;
    let (chunk_geometry, expected_chunk_width, expected_chunk_count) =
        checked_mod_q_weight_chunks_geometry(p, chunks, q_bits)?;
    if geometry.rows != chunk_geometry.rows
        || geometry.cols != chunk_geometry.cols
        || chunk_width != expected_chunk_width
        || chunk_count != expected_chunk_count
        || col_weights.len() != geometry.cols
    {
        return Err(FlockRsError::RingSwitch(RsOpenError::Shape));
    }

    let bound_statement = absorb_mod_q_weight_chunks_statement(
        transcript,
        opening_kind,
        commitment,
        p,
        statement_digest,
        q_bits,
        alpha,
        vc,
    );
    use crate::pcs::recombine_read_off;
    verify_mod_q_lig_after_statement(
        transcript,
        commitment,
        proof.into(),
        p,
        chunks,
        alpha,
        vc,
        bound_statement,
        forest_grinding_bits,
        ood,
        EqVerifierReduction {
            rings: &proof.rings,
            packed_vars: packed_vars(p),
        },
        |values, _, _| {
            let y = recombine_read_off(p, values, 0, col_weights, chunk_width, chunk_count);
            if y != claimed {
                return Err(FlockRsError::Common(IntEvalRsError::ReadOff));
            }
            Ok(())
        },
    )?;
    Ok(())
}

/// Transcript-neutral verifier shared by direct, virtual-identity, and general
/// virtual openings. Public read-off is checked before the first forest
/// challenge; the selected reduction then converts the residual claims to one
/// Ligerito opening against `commitment`.
#[allow(clippy::arithmetic_side_effects)]
#[allow(clippy::too_many_arguments)]
fn verify_mod_q_lig_core<S, R, C>(
    transcript: &mut (impl Transcript + Send),
    commitment: &Commitment,
    proof: ModQLigProofView<'_>,
    p: &IntegerMatrixLayout,
    chunks: &S,
    alpha: Gf,
    vc: &LigVerifierConfig,
    forest_grinding_bits: u32,
    ood: impl Into<VerifierOod>,
    reduction: R,
    read_off: C,
) -> Result<PaddedChunkFolds, FlockRsError>
where
    S: ModQWeightSource + ?Sized,
    R: ModQLigVerifierReduction,
    C: FnOnce(&[u128], usize, usize) -> Result<(), FlockRsError>,
{
    verify_mod_q_lig_core_with_security(
        transcript,
        commitment,
        proof,
        p,
        chunks,
        alpha,
        vc,
        forest_grinding_bits,
        ood,
        reduction,
        read_off,
        None,
    )
}

fn verify_mod_q_lig_core_with_security<S, R, C>(
    transcript: &mut (impl Transcript + Send),
    commitment: &Commitment,
    proof: ModQLigProofView<'_>,
    p: &IntegerMatrixLayout,
    chunks: &S,
    alpha: Gf,
    vc: &LigVerifierConfig,
    forest_grinding_bits: u32,
    ood: impl Into<VerifierOod>,
    reduction: R,
    read_off: C,
    security: Option<&mut grinding::GrindingContext<'_>>,
) -> Result<PaddedChunkFolds, FlockRsError>
where
    S: ModQWeightSource + ?Sized,
    R: ModQLigVerifierReduction,
    C: FnOnce(&[u128], usize, usize) -> Result<(), FlockRsError>,
{
    validate_ligerito_commitment(commitment, vc)?;
    let lch = chunks.chunk_count();
    reduction.validate_shape(lch)?;
    let folds = {
        let _g = tracing::info_span!("mv:readoff").entered();
        verify_mod_q_lig_preflight(proof, p, chunks, read_off)?
    };

    // Round 0 precedes every forest challenge (its presence must match
    // the parameters exactly: the round is part of the protocol version).
    let ood_claim = ood
        .into()
        .claim(transcript, reduction.packed_vars(), proof.ood)?;

    let mut grinder: VerifierGrindingTranscript<_, ForestRoundGrinding> =
        VerifierGrindingTranscript::new(transcript, forest_grinding_bits, proof.grinding_nonces);
    let mut points = Vec::with_capacity(lch);
    let mut mus = Vec::with_capacity(lch);
    for l in 0..lch {
        let verified = chunks
            .with_chunk(l, |w_l| {
                verify_int_eval_merged_common(
                    &mut grinder,
                    &proof.mfs[l],
                    folds.chunk(l),
                    &proof.presums[l],
                    p,
                    w_l,
                    alpha,
                )
            })
            .map_err(|()| FlockRsError::RingSwitch(RsOpenError::Shape))?;
        let (pt, mu) = verified.map_err(FlockRsError::Common)?;
        points.push(pt);
        mus.push(mu);
    }

    let prepared = {
        let _g = tracing::info_span!("mv:rswitch").entered();
        reduction.prepare(grinder, &points, &mus, ood_claim.as_ref())?
    };
    {
        let _g = tracing::info_span!("mv:lig").entered();
        verify_prepared_mod_q_ligerito_with_security(
            transcript, commitment, proof.lig, vc, prepared, security,
        )?;
    }
    Ok(folds)
}

#[allow(clippy::too_many_arguments)]
fn verify_mod_q_lig_after_statement<S, R, C>(
    transcript: &mut (impl Transcript + Send),
    commitment: &Commitment,
    proof: ModQLigProofView<'_>,
    p: &IntegerMatrixLayout,
    chunks: &S,
    alpha: Gf,
    vc: &LigVerifierConfig,
    _bound_statement: BoundModQStatement,
    forest_grinding_bits: u32,
    ood: impl Into<VerifierOod>,
    reduction: R,
    read_off: C,
) -> Result<PaddedChunkFolds, FlockRsError>
where
    S: ModQWeightSource + ?Sized,
    R: ModQLigVerifierReduction,
    C: FnOnce(&[u128], usize, usize) -> Result<(), FlockRsError>,
{
    verify_mod_q_lig_core(
        transcript,
        commitment,
        proof,
        p,
        chunks,
        alpha,
        vc,
        forest_grinding_bits,
        ood,
        reduction,
        read_off,
    )
}

// ---------------------------------------------------------------------
/// [`prove_mle_eval_mod_q_ligerito_virtual_with_ood`] with Round 0 (the
/// out-of-domain sample) skipped.
#[allow(clippy::too_many_arguments)]
pub fn prove_mle_eval_mod_q_ligerito_virtual<M>(
    transcript: &mut (impl Transcript + Send),
    hint_f: &FlockCommitHint,
    h_rows: &[Vec<u64>],
    h_layout: &IntegerMatrixLayout,
    f_layout: &IntegerMatrixLayout,
    map: &M,
    row_weights_q: &[u128],
    q_bits: usize,
    alpha: Gf,
    pc: &LigProverConfig,
) -> IntEvalRsLigVirtProof
where
    M: circuit::linear_map::binary::VirtualMap,
{
    prove_mle_eval_mod_q_ligerito_virtual_with_ood(
        transcript,
        hint_f,
        h_rows,
        h_layout,
        f_layout,
        map,
        row_weights_q,
        q_bits,
        alpha,
        None,
        pc,
    )
}

/// [`verify_mle_eval_mod_q_ligerito_virtual_with_ood`] with Round 0 skipped.
#[allow(clippy::too_many_arguments)]
pub fn verify_mle_eval_mod_q_ligerito_virtual<R, M>(
    transcript: &mut (impl Transcript + Send),
    commitment_f: &Commitment,
    proof: &IntEvalRsLigVirtProof,
    h_layout: &IntegerMatrixLayout,
    f_layout: &IntegerMatrixLayout,
    map: &M,
    row_weights_q: &[u128],
    col_weights: &[R],
    alpha: Gf,
    claimed: R,
    q_bits: usize,
    vc: &LigVerifierConfig,
) -> Result<(), FlockRsError>
where
    R: Copy + PartialEq + From<u128> + core::ops::Add<Output = R> + core::ops::Mul<Output = R>,
    M: circuit::linear_map::binary::VirtualMap,
{
    verify_mle_eval_mod_q_ligerito_virtual_with_ood(
        transcript,
        commitment_f,
        proof,
        h_layout,
        f_layout,
        map,
        row_weights_q,
        col_weights,
        alpha,
        claimed,
        q_bits,
        None,
        vc,
    )
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
    mle_eval_mod_q_lig_size_breakdown(proof)
        .0
        .total()
        .saturating_add(proof.lig.size_bytes())
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
    u.iter()
        .rposition(|&x| x != 0)
        .map_or(0, |i| i.wrapping_add(1))
}

// ---------------------------------------------------------------------
// Host proof-stream (de)serialization of `IntEvalRsLigModQProof`
// ---------------------------------------------------------------------

#[allow(clippy::arithmetic_side_effects)]
fn write_mod_q_chunk_record(
    writer: &mut crate::proof_codec::Writer,
    forest: &MergedForestProof,
    folds: &[u128],
    presum: &MultiDegreeSumcheckProof<Gf>,
) {
    writer.len(forest.layers.len());
    for layer in &forest.layers {
        // Flag bits: 1 = sc_x present, 2 = quad (pair2 present).
        // Arity-2 layers keep the legacy 0/1 values, preserving bytes.
        let flag = layer.sc_x.is_some() as usize | ((layer.pair2.is_some() as usize) << 1);
        writer.len(flag);
        if let Some(sumcheck) = &layer.sc_x {
            writer.transcribable(sumcheck);
        }
        writer.transcribable(&layer.sc_c);
        writer.gf(&layer.pair.0);
        writer.gf(&layer.pair.1);
        if let Some(pair) = &layer.pair2 {
            writer.gf(&pair.0);
            writer.gf(&pair.1);
        }
    }

    let transmitted = transmitted_us_len(folds);
    writer.len(transmitted);
    for &fold in &folds[..transmitted] {
        writer.u128(fold);
    }
    writer.transcribable(presum);
}

fn read_mod_q_chunk_record(
    reader: &mut crate::proof_codec::Reader<'_>,
) -> Result<
    (MergedForestProof, Vec<u128>, MultiDegreeSumcheckProof<Gf>),
    crate::proof_codec::CodecError,
> {
    use crate::merged_forest::MergedLayer;
    use crate::proof_codec::CodecError;

    let layer_count = reader.len()?;
    let mut layers = Vec::with_capacity(layer_count.min(64));
    for _ in 0..layer_count {
        let flag = reader.len()?;
        if flag & !0b11 != 0 {
            return Err(CodecError::NonCanonical);
        }
        let sc_x = if flag & 1 == 1 {
            Some(reader.transcribable::<crate::piop::sumcheck::SumcheckProof<Gf>>()?)
        } else {
            None
        };
        let sc_c = reader.transcribable::<crate::piop::sumcheck::SumcheckProof<Gf>>()?;
        let pair = (reader.gf()?, reader.gf()?);
        let pair2 = if flag & 2 == 2 {
            Some((reader.gf()?, reader.gf()?))
        } else {
            None
        };
        layers.push(MergedLayer {
            sc_x,
            sc_c,
            pair,
            pair2,
        });
    }

    let fold_count = reader.len()?;
    // Bound attacker-controlled capacity by the remaining wire bytes.
    let mut folds = Vec::with_capacity(fold_count.min(reader.remaining() / 16));
    for _ in 0..fold_count {
        folds.push(reader.u128()?);
    }
    if folds.last() == Some(&0) {
        return Err(CodecError::NonCanonical);
    }
    let presum = reader.transcribable::<MultiDegreeSumcheckProof<Gf>>()?;
    Ok((MergedForestProof { layers }, folds, presum))
}

fn write_ligerito_blob(writer: &mut crate::proof_codec::Writer, proof: &LigeritoProof) {
    let bytes = bincode::serialize(proof).expect("LigeritoProof bincode encode");
    writer.len(bytes.len());
    writer.bytes(&bytes);
}

fn read_ligerito_blob(
    reader: &mut crate::proof_codec::Reader<'_>,
) -> Result<LigeritoProof, crate::proof_codec::CodecError> {
    use crate::proof_codec::CodecError;
    let byte_count = reader.len()?;
    bincode::deserialize(reader.take(byte_count)?)
        .map_err(|error| CodecError::Bincode(error.to_string()))
}

impl IntEvalRsLigModQProof {
    /// Serialize the complete BitZ proof into the host proof stream: the
    /// zinc-side parts field by field (merged forests, chunk folds `u`,
    /// pre-sumchecks, ring-switch `s_v` messages) via [`crate::proof_codec`],
    /// then the flock [`LigeritoProof`] as a length-prefixed `bincode` 1.3
    /// blob. Mirrors [`Self::from_bytes`].
    #[allow(clippy::arithmetic_side_effects)]
    pub fn to_bytes(&self) -> Vec<u8> {
        use crate::proof_codec::Writer;
        let mut w = Writer::new();
        w.bytes(b"BITZM002");
        let lch = self.mfs.len();
        w.len(lch);
        for l in 0..lch {
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
            write_mod_q_chunk_record(&mut w, &self.mfs[l], &self.us[l], &self.presums[l]);
            w.len(self.rings[l].s_v.len());
            for g in &self.rings[l].s_v {
                w.gf(g);
            }
        }
        write_ligerito_blob(&mut w, &self.lig);
        write_proof_trailer(&mut w, &self.grinding_nonces, self.ood.as_ref());
        w.into_vec()
    }

    /// Deserialize the complete BitZ proof from the host proof stream.
    /// Mirrors [`Self::to_bytes`]. The embedded `LigeritoProof` is decoded
    /// from its length-prefixed `bincode` blob.
    #[allow(clippy::arithmetic_side_effects)]
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, crate::proof_codec::CodecError> {
        use crate::proof_codec::Reader;
        let mut r = Reader::new(bytes);
        if r.take(8)? != b"BITZM002" {
            return Err(crate::proof_codec::CodecError::NonCanonical);
        }
        let lch = r.len()?;
        let mut mfs = Vec::with_capacity(lch.min(64));
        let mut us = Vec::with_capacity(lch.min(64));
        let mut presums = Vec::with_capacity(lch.min(64));
        let mut rings = Vec::with_capacity(lch.min(64));
        for _ in 0..lch {
            let (forest, folds, presum) = read_mod_q_chunk_record(&mut r)?;
            mfs.push(forest);
            us.push(folds);
            presums.push(presum);
            let n_sv = r.len()?;
            let mut s_v = Vec::with_capacity(n_sv.min(r.remaining() / 16));
            for _ in 0..n_sv {
                s_v.push(r.gf()?);
            }
            rings.push(RingSwitchProof { s_v });
        }
        let lig = read_ligerito_blob(&mut r)?;
        let (grinding_nonces, ood) = read_proof_trailer(&mut r)?;
        Ok(IntEvalRsLigModQProof {
            mfs,
            us,
            presums,
            rings,
            lig,
            grinding_nonces,
            ood,
        })
    }
}

/// The word that opens the Round-0 (OOD) trailer section. A grinding-nonce
/// count can never take this value (it would mean a `2^67`-byte proof), so
/// the two optional sections stay distinguishable without a flag word and
/// every pre-Round-0 proof stream keeps its exact bytes.
const OOD_TRAILER_MARKER: usize = usize::MAX;

/// The optional proof trailer shared by the mod-q and virtual codecs:
/// ABSENT when there is nothing to carry (ungrinded, Round-0-less proofs are
/// byte-identical to the pre-grinding stream); otherwise, in this order,
/// the OOD section when Round 0 ran — [`OOD_TRAILER_MARKER`], `y` (16
/// bytes), a 0/1 nonce-presence word and the 8-byte LE nonce when present —
/// and the grinding-nonce section when any nonce exists — a length prefix
/// plus 8-byte LE nonces. An empty nonce section, a presence word outside
/// `{0, 1}` and trailing bytes are all non-canonical.
fn read_proof_trailer(
    r: &mut crate::proof_codec::Reader<'_>,
) -> Result<(Vec<u64>, Option<OodRound>), crate::proof_codec::CodecError> {
    use crate::proof_codec::CodecError;
    if r.remaining() == 0 {
        return Ok((Vec::new(), None));
    }
    let mut head = r.len()?;
    let ood = if head == OOD_TRAILER_MARKER {
        let y = r.gf()?;
        let nonce = match r.len()? {
            0 => None,
            1 => {
                let bytes = r.take(8)?;
                Some(u64::from_le_bytes(bytes.try_into().expect("8-byte take")))
            }
            _ => return Err(CodecError::NonCanonical),
        };
        if r.remaining() == 0 {
            return Ok((Vec::new(), Some(OodRound { y, nonce })));
        }
        head = r.len()?;
        Some(OodRound { y, nonce })
    } else {
        None
    };
    let count = head;
    if count == 0 || count == OOD_TRAILER_MARKER {
        return Err(CodecError::NonCanonical);
    }
    let mut nonces = Vec::with_capacity(count.min(r.remaining() / 8));
    for _ in 0..count {
        let bytes = r.take(8)?;
        nonces.push(u64::from_le_bytes(bytes.try_into().expect("8-byte take")));
    }
    if r.remaining() != 0 {
        return Err(CodecError::NonCanonical);
    }
    Ok((nonces, ood))
}

/// Writer twin of [`read_proof_trailer`].
fn write_proof_trailer(w: &mut crate::proof_codec::Writer, nonces: &[u64], ood: Option<&OodRound>) {
    if let Some(round) = ood {
        w.len(OOD_TRAILER_MARKER);
        w.gf(&round.y);
        match round.nonce {
            None => w.len(0),
            Some(nonce) => {
                w.len(1);
                w.bytes(&nonce.to_le_bytes());
            }
        }
    }
    if !nonces.is_empty() {
        w.len(nonces.len());
        for &nonce in nonces {
            w.bytes(&nonce.to_le_bytes());
        }
    }
}

// ---------------------------------------------------------------------
// F₂-VIRTUALIZATION (paper `s:to_f2_virtual` / `s:virtualization`,
// construction `c:virtual_iop`): open a mod-q claim about the DERIVED
// vector `h = M·f` over `F₂` against the commitment to `f` alone. `M` is
// a public sparse [`PreparedVirtualMap`](circuit::linear_map::binary::PreparedVirtualMap)
// between the two bit-cell grids; `h` is
// never committed.
//
// Pipeline (the paper's "run `c:core_iop` until Phase 3, then transpose",
// closed by the ring switch for ARBITRARY inner products — the batching
// protocol of appendix `a:ring_switch_remco` §"Extension openings",
// instantiated with the §"Coefficient projection" embedding):
//
//   1. Synthesis supplies `h`'s bit rows alongside `f`; the prover runs
//      the ORDINARY per-chunk machinery on `h` — integer chunk folds `us`, merged
//      product forests, de-black-boxing pre-sumchecks
//      ([`prove_int_eval_merged_common`]) — with `h_layout` geometry. Nothing
//      here touches the oracle: the verifier recomputes the roots from
//      the sent `us` and is left with per-chunk residual claims
//      `ĥ(pt_l) = μ_l` about the (uncommitted) derived bit-MLE.
//   2. Both sides draw η's and transpose through `Mᵀ` at the commitment
//      field (char 2, where XOR is addition): `h := Σ_l η_l·μ_l =
//      ⟨W, f⟩_K` with `W := Σ_l η_l·Mᵀ eq(pt_l)` over `f`'s cells —
//      the appendix's `⟨w, a⟩_E = h` with `F = F₂`, `E = K`, `w = f`,
//      `a = W`. The embedding triple is `W = Id` (the commitment already
//      packs cells in the MONOMIAL basis, bit `v` ↔ `X^v`), `H = c₀`,
//      and `A` = the `μ_H`-dual basis ([`crate::dual_basis`]:
//      reversal of the `v ≥ 1` coordinates + seven GHASH corrections).
//   3. Batching protocol: the prover sends the d = 128 dual-packed plane
//      inner products `h_i = ⟨pack(f), A(a_i)⟩_K` (`a_i` = bit-plane `i`
//      of the weights; tag 0x48). The verifier checks step 3,
//      `Σ_i c₀(h_i)·X^i = h`, draws the zero-evader `ρ` (LOG_PACKING
//      challenges, eq-expanded to `K^128` — the ring-switch `r″`
//      convention, RBR error ≤ LOG_PACKING/|K|), and both sides reduce
//      to the ONE native Ligerito inner product
//      `⟨pack(f), a′⟩_K = h′ := Σ_i ρ_i·h_i`, with
//      `a′(y) = Σ_v Φ_ρ(W_{(v,y)})·A(e_v)`. No bridge sumcheck, no
//      point opening, no division anywhere (`f₀ = 1`).
//
// Both sides exploit the same char-2 reassociations (exact — XOR is
// K-addition). For source cell `j`, let
// `W_j := Σ_{r:M[r,j]=1} E_r`, where
// `E_r := Σ_l η_l·eq_{bits(r)}(pt_l)`. Then
//
//      h_i = Σ_j bit_i(W_j)·pack(f)[y_j]·A(e_{v_j}),
//      a′(y) = Σ_{j:y_j=y} Φ_ρ(W_j)·A(e_{v_j}),
//
// so neither `W` nor any `f`-side table is materialized: both sides stream
// the canonical CSC source columns. The prover and verifier call the SAME
// pack-owned `a′` builder, and the verifier answers the Ligerito residual
// hook by MLE-folding its result.
//
// IDENTITY FAST PATH: when `M` is the identity and both grids share one
// row layout ([`virtual_id_fast_eligible`]), `h`'s bit rows ARE `f`'s
// and every per-chunk claim is a claim on `f`'s own flat bit-MLE — the
// prover ignores the supplied derived rows and skips both batching passes,
// running
// the BASE opening ([`prove_mle_eval_mod_q_ligerito`]'s reduction: per-chunk
// eq ring-switch `s_v`, shared `r″`, η-batched Ligerito) after the same
// v2 statement absorb, emitting the [`VirtualReductionProof::Eq`] reduction. The
// switch `BITZ_VIRT_ID_FAST` (default ON, `=0` disables) is PROVER-side
// only: on an eligible statement the verifier accepts either reduction (each
// is an individually sound reduction of the same claim — with `h = f`
// the eq-tensor weights are exactly the base path's); on any other
// statement the Eq reduction is rejected as a shape error, since there the
// base verification would bind `f̂(pt_l)` where the claim is
// `(M·f)ˆ(pt_l)`.
//
// Soundness chain (informal; mirrors the base path plus two fresh
// terms): the forests bind the sent `us` as exponent folds of whatever
// row data underlies the leaves, and each pre-sumcheck + `R̂(r*)`
// division pins `μ_l` as that data's bit-MLE value at the random exit
// point `pt_l` — exactly as in the base path. The Ligerito call binds
// `⟨pack(f), a′⟩ = h′` for the COMMITTED `f` with `a′` derived from the
// statement alone, so if any sent `h_i` differs from its true value the
// ρ-batch accepts with probability ≤ LOG_PACKING/|K| (the eq-tensor
// zero-evader ε of the batching protocol — account it next to the
// η-batch error `L/|K|`). With all `h_i` true, step 3 IS
// `Σ_l η_l μ_l = ⟨W, f⟩` (the bilinear-embedding identity
// `⟨a_i, f⟩_{F₂} = c₀(h_i)` blockwise, paper theorem "Structure of
// bilinear embeddings"), i.e. `μ_l = (M·f)ˆ(pt_l)` for every chunk
// except with probability `≈ (L + LOG_PACKING)/|K|`. From there the base
// path's argument applies verbatim with `h := M·f`. The statement
// (commitment root, both geometries, `M`'s digest, the row weights,
// `q_bits`, α) is digest-absorbed before any challenge, so the new API
// is self-binding.
// ---------------------------------------------------------------------

/// The virtual opening's commitment bridge after the per-chunk claims.
#[derive(Clone)]
pub enum VirtualReductionProof {
    /// General `M`: the dual-basis batching message
    /// `hs[i] = ⟨pack(f), A(a_i)⟩_K` for bit-plane `i` of the transposed
    /// weights (always 128 elements), followed by the ρ-batched call.
    AdjointBatch { hs: Box<[Gf; 128]> },
    /// Identity-`M` fast path (`h = f` cell for cell, same row layout):
    /// the BASE path's per-chunk eq ring-switch messages — no `h`
    /// materialization, no batching passes. The verifier accepts this
    /// reduction only when the statement is eligible (see
    /// [`virtual_id_fast_eligible`]).
    Eq { rings: Vec<RingSwitchProof> },
}

/// End-to-end proof of a mod-q claim on the derived vector `h = M·f`:
/// per-chunk forests/folds/pre-sumchecks on `h` (`h_layout` geometry), then
/// one of the two commitment bridges ([`VirtualReductionProof`]) and the ONE Ligerito
/// call on `f`'s commitment.
#[derive(Clone)]
pub struct IntEvalRsLigVirtProof {
    /// Per weight chunk: the merged product forest on `h`.
    pub mfs: Vec<MergedForestProof>,
    /// `us[l]` = the `2^{s_h}` chunk folds `u_c^{(l)}` of `h`.
    pub us: Vec<Vec<u128>>,
    /// Per weight chunk: the de-black-boxing pre-sumcheck on `h`.
    pub presums: Vec<MultiDegreeSumcheckProof<Gf>>,
    /// The commitment-bridge messages (general dual-basis batch, or the
    /// identity-`M` eq ring switch).
    pub reduction: VirtualReductionProof,
    /// The Ligerito opening (`⟨pack(f), a′⟩ = h′` for AdjointBatch;
    /// the base path's η-batched call for Eq).
    pub lig: LigeritoProof,
    /// Per-challenge forest/opening grinding nonces, in draw order (empty
    /// — and absent from the codec — at difficulty 0).
    pub grinding_nonces: Vec<u64>,
    /// Round 0 (the out-of-domain sample on the committed source): `Some`
    /// iff the round was executed.
    pub ood: Option<OodRound>,
}

impl<'a> From<&'a IntEvalRsLigVirtProof> for ModQLigProofView<'a> {
    fn from(proof: &'a IntEvalRsLigVirtProof) -> Self {
        Self {
            mfs: &proof.mfs,
            us: &proof.us,
            presums: &proof.presums,
            lig: &proof.lig,
            grinding_nonces: &proof.grinding_nonces,
            ood: proof.ood.as_ref(),
        }
    }
}

struct AdjointBatchVerifierReduction<'proof, 'map, M> {
    map: &'map M,
    hs: &'proof [Gf; 128],
    derived_row_bits: usize,
    source_packed_vars: usize,
}

impl<M> ModQLigVerifierReduction for AdjointBatchVerifierReduction<'_, '_, M>
where
    M: circuit::linear_map::binary::VirtualMap,
{
    fn validate_shape(&self, _chunk_count: usize) -> Result<(), FlockRsError> {
        Ok(())
    }

    fn packed_vars(&self) -> usize {
        self.source_packed_vars
    }

    #[allow(clippy::arithmetic_side_effects)]
    fn prepare<T: Transcript + Send>(
        self,
        mut grinder: VerifierGrindingTranscript<'_, '_, T, ForestRoundGrinding>,
        points: &[Vec<Gf>],
        mus: &[Gf],
        ood: Option<&OodVerifierClaim>,
    ) -> Result<PreparedLigeritoClaim, FlockRsError> {
        let etas: Vec<Gf> = grinder.get_field_challenges(points.len(), &());
        let combined_mu = etas
            .iter()
            .zip(mus)
            .fold(Gf::zero(), |acc, (&eta, &mu)| acc + eta * mu);
        let mut assembled = [0u64; 2];
        for (index, value) in self.hs.iter().enumerate() {
            assembled[index >> 6] |= crate::dual_basis::c0_bit(*value) << (index & 63);
        }
        if Gf::from_polynomial_words(assembled) != combined_mu {
            return Err(FlockRsError::VirtualBatch);
        }
        crate::ligerito::absorb_hs(&mut grinder, self.hs);

        let r2: Vec<Gf> = grinder.get_field_challenges(LOG_PACKING, &());
        let eta_ood: Option<Gf> = ood.map(|_| grinder.get_field_challenge(&()));
        grinder.finish().map_err(|_| FlockRsError::ForestGrinding)?;
        let rho = crate::poly::utils::build_eq_x_r_vec(&r2, &()).expect("r2");
        let mut target = rho
            .iter()
            .zip(self.hs)
            .fold(Gf::zero(), |acc, (&weight, &value)| acc + weight * value);
        let ood_term = ood.zip(eta_ood).map(|(claim, eta)| {
            target += eta * claim.y;
            (claim.point.clone(), eta)
        });

        let weights = {
            let _g = tracing::info_span!("mqv:vwprep").entered();
            BinaryAdjoint::new_factored_tail(self.map, points, &etas, self.derived_row_bits)
        };
        let a_prime = {
            let _g = tracing::info_span!("mqv:vaprime").entered();
            verifier_a_prime(self.map, &weights, &rho, 1usize << self.source_packed_vars)
        };
        Ok(PreparedLigeritoClaim {
            packed_vars: self.source_packed_vars,
            target,
            basis: PreparedLigeritoBasis::Dense { a_prime },
            ood: ood_term,
        })
    }
}

enum VirtualVerifierReduction<'proof, 'map, M> {
    Eq(EqVerifierReduction<'proof>),
    AdjointBatch(AdjointBatchVerifierReduction<'proof, 'map, M>),
}

impl<M> ModQLigVerifierReduction for VirtualVerifierReduction<'_, '_, M>
where
    M: circuit::linear_map::binary::VirtualMap,
{
    fn validate_shape(&self, chunk_count: usize) -> Result<(), FlockRsError> {
        match self {
            Self::Eq(reduction) => reduction.validate_shape(chunk_count),
            Self::AdjointBatch(reduction) => reduction.validate_shape(chunk_count),
        }
    }

    fn packed_vars(&self) -> usize {
        match self {
            Self::Eq(reduction) => reduction.packed_vars(),
            Self::AdjointBatch(reduction) => reduction.packed_vars(),
        }
    }

    fn prepare<T: Transcript + Send>(
        self,
        grinder: VerifierGrindingTranscript<'_, '_, T, ForestRoundGrinding>,
        points: &[Vec<Gf>],
        mus: &[Gf],
        ood: Option<&OodVerifierClaim>,
    ) -> Result<PreparedLigeritoClaim, FlockRsError> {
        match self {
            Self::Eq(reduction) => reduction.prepare(grinder, points, mus, ood),
            Self::AdjointBatch(reduction) => reduction.prepare(grinder, points, mus, ood),
        }
    }
}

/// Whether the STATEMENT admits the identity fast path: `M` is the
/// identity and both grids share one row layout (`t + log₂W` and `s`
/// equal), so `h`'s bit rows ARE `f`'s and every per-chunk claim is a
/// claim on `f`'s own flat bit-MLE. Deterministic in the statement —
/// prover and verifier need no coordination.
pub fn virtual_id_fast_eligible<M>(
    map: &M,
    h_layout: &IntegerMatrixLayout,
    f_layout: &IntegerMatrixLayout,
) -> bool
where
    M: circuit::linear_map::binary::VirtualMap,
{
    use crate::f2map::cell_row_bits;
    map.is_identity()
        && cell_row_bits(h_layout) == cell_row_bits(f_layout)
        && h_layout.col_vars == f_layout.col_vars
}

/// The identity fast-path switch (default ON; `BITZ_VIRT_ID_FAST=0`
/// disables). PROVER-side only: it selects which reduction is produced on an
/// eligible statement; the verifier accepts either reduction there (both are
/// sound), so no cross-process agreement is needed. Read per call so
/// tests can toggle it.
/// The packed-source plane engine (`BITZ_VIRT_PLANES`, default on; `0`
/// restores the per-cell batching kernels — diagnostic / A-B measurement;
/// byte-identical proofs either way). Read once per process.
fn virt_planes() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| std::env::var("BITZ_VIRT_PLANES").map_or(true, |v| v != "0"))
}

fn virt_id_fast() -> bool {
    std::env::var("BITZ_VIRT_ID_FAST").map_or(true, |v| v != "0")
}

/// Digest-absorbs the virtual opening's complete statement before any
/// challenge is drawn: commitment root and geometry, both cell shapes,
/// the map digest, the claimed row weights, the full modulus `q`, `q_bits`,
/// and `α`.
#[allow(clippy::too_many_arguments)]
fn absorb_virtual_statement<M, S>(
    transcript: &mut impl Transcript,
    commitment: &Commitment,
    h_layout: &IntegerMatrixLayout,
    f_layout: &IntegerMatrixLayout,
    map: &M,
    row_weights: &S,
    q: u128,
    q_bits: usize,
    alpha: Gf,
) -> BoundModQStatement
where
    M: circuit::linear_map::binary::VirtualMap,
    S: ModQWeightSource + ?Sized,
{
    let mut hash = blake3::Hasher::new();
    // v3 additionally binds the full runtime modulus rather than only its
    // bit length.  Distinct primes with one bit length must never share a
    // subprotocol statement.
    if let Some(bound) = row_weights.padding_bound() {
        hash.update(b"bitz/mod-q-virtual-statement/v4");
        hash.update(&(bound.value_bits() as u64).to_le_bytes());
    } else {
        hash.update(b"bitz/mod-q-virtual-statement/v3");
    }
    hash.update(&commitment.root);
    for v in [
        commitment.params.m,
        commitment.params.log_inv_rate,
        commitment.params.log_batch_size,
        h_layout.row_vars,
        h_layout.col_vars,
        h_layout.word_bits,
        f_layout.row_vars,
        f_layout.col_vars,
        f_layout.word_bits,
        q_bits,
        row_weights.row_count(),
    ] {
        hash.update(&(v as u64).to_le_bytes());
    }
    hash.update(&q.to_le_bytes());
    hash.update(&map.digest());
    // Reconstruct row-major canonical values lazily. This is byte-for-byte
    // identical to hashing the dense input accepted by `from_dense`, while
    // avoiding a second `2^t`-element allocation for direct chunk callers.
    for row in 0..row_weights.row_count() {
        let weight = row_weights
            .canonical_weight(row)
            .expect("validated weight source must contain every canonical row");
        hash.update(&weight.to_le_bytes());
    }
    let aw = alpha.as_words();
    hash.update(&aw[0].to_le_bytes());
    hash.update(&aw[1].to_le_bytes());
    let mut frame = Vec::with_capacity(33);
    frame.push(0x56u8); // 'V'
    frame.extend_from_slice(hash.finalize().as_bytes());
    transcript.absorb_slice(&frame);
    BoundModQStatement::new()
}

#[cfg(test)]
#[test]
fn virtual_row_coefficients_match_direct_equality_weights() {
    use crate::poly::utils::build_eq_x_r_vec;

    const BITS: usize = 10;
    for count in [0usize, 1, 2, 7] {
        let points: Vec<Vec<_>> = (0..count)
            .map(|point| {
                (0..BITS)
                    .map(|bit| {
                        Gf::from_polynomial_words([
                            17 + (point * BITS + bit) as u64,
                            29 + point as u64,
                        ])
                    })
                    .collect()
            })
            .collect();
        let etas: Vec<_> = (0..count)
            .map(|point| Gf::from_polynomial_words([73 + point as u64, 13]))
            .collect();
        // Build full equality tables independently of the factored row/column
        // representation, using ordinary reduced field products throughout.
        let full: Vec<_> = points
            .iter()
            .map(|point| build_eq_x_r_vec(point, &()).unwrap())
            .collect();
        for row_bits in [1, 4, BITS] {
            let actual = BinaryRowWeights::new(&points, &etas, row_bits, binary_equality);
            for row in 0..1usize << BITS {
                let expected = full
                    .iter()
                    .zip(&etas)
                    .fold(Gf::zero(), |sum, (table, &eta)| sum + eta * table[row]);
                assert_eq!(
                    actual.coeff(row),
                    expected,
                    "chunks {count}, split {row_bits}, row {row}"
                );
            }
        }
    }
}

/// One 16-row block of the `h_i` scatter, method-of-four-Russians: four
/// 16-entry subset-sum tables over the `G_r` values, then per byte
/// position two 8×8 bit transposes of the `E_r` patterns and per output
/// bit four lookups + three adds + ONE accumulator RMW — halving the
/// per-bit RMW count of the 8-row block at the same table-build cost
/// ([`sv_fold_mfr`]'s kernel widened; exact field sums either way).
#[allow(clippy::arithmetic_side_effects)]
#[inline]
fn hs_scatter_block16(s: &mut [Gf; 128], wits: &[[u64; 2]; 16], vals: &[Gf; 16]) {
    use crate::ligerito::{subset_sums_4, transpose_8x8_bits};
    let t0 = subset_sums_4([vals[0], vals[1], vals[2], vals[3]]);
    let t1 = subset_sums_4([vals[4], vals[5], vals[6], vals[7]]);
    let t2 = subset_sums_4([vals[8], vals[9], vals[10], vals[11]]);
    let t3 = subset_sums_4([vals[12], vals[13], vals[14], vals[15]]);
    let mut m_bytes = [[0u8; 16]; 16];
    for (e, slot) in m_bytes.iter_mut().enumerate() {
        slot[..8].copy_from_slice(&wits[e][0].to_le_bytes());
        slot[8..].copy_from_slice(&wits[e][1].to_le_bytes());
    }
    for r_byte in 0..16 {
        let lo8: u64 = (m_bytes[0][r_byte] as u64)
            | ((m_bytes[1][r_byte] as u64) << 8)
            | ((m_bytes[2][r_byte] as u64) << 16)
            | ((m_bytes[3][r_byte] as u64) << 24)
            | ((m_bytes[4][r_byte] as u64) << 32)
            | ((m_bytes[5][r_byte] as u64) << 40)
            | ((m_bytes[6][r_byte] as u64) << 48)
            | ((m_bytes[7][r_byte] as u64) << 56);
        let hi8: u64 = (m_bytes[8][r_byte] as u64)
            | ((m_bytes[9][r_byte] as u64) << 8)
            | ((m_bytes[10][r_byte] as u64) << 16)
            | ((m_bytes[11][r_byte] as u64) << 24)
            | ((m_bytes[12][r_byte] as u64) << 32)
            | ((m_bytes[13][r_byte] as u64) << 40)
            | ((m_bytes[14][r_byte] as u64) << 48)
            | ((m_bytes[15][r_byte] as u64) << 56);
        let tb_lo = transpose_8x8_bits(lo8).to_le_bytes();
        let tb_hi = transpose_8x8_bits(hi8).to_le_bytes();
        let base = r_byte * 8;
        for p in 0..8usize {
            let m0 = tb_lo[p];
            let m1 = tb_hi[p];
            s[base + p] += (t0[(m0 & 0x0F) as usize] + t1[(m0 >> 4) as usize])
                + (t2[(m1 & 0x0F) as usize] + t3[(m1 >> 4) as usize]);
        }
    }
}

#[cfg(all(test, feature = "ecdsa"))]
#[test]
fn chained_compact_tail_weights_and_planes_match_generic() {
    use {
        crate::piop::spartan::ecdsa_sha256::{OuterMode, prepare_sha256_ecdsa},
        circuit::linear_map::binary::VirtualMap,
    };
    let prepared = prepare_sha256_ecdsa(7, 100, OuterMode::Split).unwrap();
    let map = prepared.map();
    assert!(map.chained_packed_source_tail().unwrap().map.is_identity());
    let points: Vec<Vec<_>> = [17, 41]
        .into_iter()
        .map(|offset| {
            (0..map.rows().ilog2())
                .map(|i| Gf::from_polynomial_words([offset + u64::from(i), 29]))
                .collect()
        })
        .collect();
    let etas = [
        Gf::from_polynomial_words([73, 13]),
        Gf::from_polynomial_words([89, 37]),
    ];
    let weights = BinaryAdjoint::new(map, &points, &etas, prepared.assignment_params().row_vars);
    assert!(matches!(
        &weights,
        BinaryAdjoint::PackedSourceRepeated { corrections, .. } if !corrections.is_empty()
    ));
    let generic = BinaryAdjoint::Generic {
        map,
        coeffs: BinaryRowWeights::new(
            &points,
            &etas,
            prepared.assignment_params().row_vars,
            binary_equality,
        ),
    };
    let mut actual = [Gf::zero(); 128];
    let mut expected = actual;
    for pack in 0..(map.cols() >> LOG_PACKING) {
        weights.pack_weights(pack, &mut actual);
        generic.pack_weights(pack, &mut expected);
        assert_eq!(actual, expected, "source pack {pack}");
    }
    let p_msg: Vec<_> = (0..map.cols() >> LOG_PACKING)
        .map(|i| Gf128 {
            lo: i as u64 ^ 0xabcdef,
            hi: !(i as u64),
        })
        .collect();
    let a_cols = crate::dual_basis::dual_basis_cols();
    if let Some(planes) = weights.packed_source_planes() {
        let mut hs = planes.hs_fold(&p_msg);
        weights.add_extra_hs(&mut hs, &p_msg, &a_cols);
        assert_eq!(hs, virtual_hs_fold(map, &weights, &p_msg, &a_cols));
        let rho = crate::poly::utils::build_eq_x_r_vec(&points[0][..7], &()).unwrap();
        let (mut basis, mut round0) = planes.a_prime(&rho, &p_msg);
        weights.add_extra_a_prime(&mut basis, &mut round0, &rho, &p_msg);
        let expected = virtual_a_prime(map, &weights, &rho, &a_cols, p_msg.len());
        assert!(basis.iter().zip(expected).all(|(&a, b)| (a) == b));
    }
}

/// The verifier's factored weights and plane-engine basis on the real
/// SHA-256 + ECDSA map are the prover's folded weights and the streamed
/// basis: at 2^3 (tail phase 8 — straddling packs) and 2^7 (phase 0), both
/// outer modes, one and two weight chunks, random points and a random ρ
/// as well as the protocol's eq-tensor ρ. (No modulus enters: the weights
/// live in GF(2^128).)
#[cfg(all(test, feature = "ecdsa"))]
#[test]
fn verifier_basis_matches_streamed_basis_on_the_ecdsa_map() {
    use {
        crate::piop::spartan::ecdsa_sha256::{OuterMode, prepare_sha256_ecdsa},
        circuit::linear_map::binary::VirtualMap,
    };
    let splitmix = |x: u64| {
        let mut z = x.wrapping_add(0x9E37_79B9_7F4A_7C15);
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    };
    let sample = |seed: u64| Gf::from_polynomial_words([splitmix(seed), splitmix(seed ^ 0xD1CE)]);
    for (log_n, mode) in [
        (3usize, OuterMode::Split),
        (3, OuterMode::AllRows),
        (7, OuterMode::Split),
        (7, OuterMode::AllRows),
    ] {
        let prepared = prepare_sha256_ecdsa(log_n, 100, mode).unwrap();
        let map = prepared.map();
        let t_wh = prepared.assignment_params().row_vars;
        let vars = map.rows().ilog2() as usize;
        let n_packs = map.cols() >> LOG_PACKING;
        for chunks in [1usize, 2] {
            let points: Vec<Vec<Gf>> = (0..chunks as u64)
                .map(|l| {
                    (0..vars as u64)
                        .map(|i| sample(0x7A00 + (log_n as u64) * 977 + l * 131 + i))
                        .collect()
                })
                .collect();
            let etas: Vec<Gf> = (0..chunks as u64).map(|l| sample(0x9B00 + l)).collect();
            let dense = BinaryAdjoint::new(map, &points, &etas, t_wh);
            let factored = BinaryAdjoint::new_factored_tail(map, &points, &etas, t_wh);
            assert!(
                matches!(
                    factored,
                    BinaryAdjoint::PackedSourceRepeated {
                        affine_tail: Some(_),
                        ..
                    }
                ),
                "the P-256 tail is an identity map and must stay factored"
            );
            let mut expected = [Gf::zero(); 128];
            let mut actual = expected;
            for pack in 0..n_packs {
                let live = dense.pack_weights(pack, &mut expected);
                assert_eq!(
                    factored.pack_weights(pack, &mut actual),
                    live,
                    "pack {pack}"
                );
                assert_eq!(
                    actual, expected,
                    "2^{log_n} {mode:?} chunks {chunks} pack {pack}"
                );
            }
            let a_cols = crate::dual_basis::dual_basis_cols();
            let rhos = [
                (0..128u64).map(|i| sample(0xC000 + i)).collect::<Vec<_>>(),
                crate::poly::utils::build_eq_x_r_vec(&points[0][..LOG_PACKING], &()).unwrap(),
            ];
            for rho in &rhos {
                let streamed = virtual_a_prime(map, &dense, rho, &a_cols, n_packs);
                let engines = verifier_a_prime(map, &factored, rho, n_packs);
                assert_eq!(engines, streamed, "2^{log_n} {mode:?} chunks {chunks}");
            }
        }
    }
}

trait VirtualOpeningWeights<'a, M: circuit::linear_map::binary::VirtualMap>: Sized {
    fn new(map: &'a M, points: &[Vec<Gf>], etas: &[Gf], t_wh: usize) -> Self;
    fn new_factored_tail(map: &'a M, points: &[Vec<Gf>], etas: &[Gf], t_wh: usize) -> Self;
    fn packed_source_planes(&self) -> Option<crate::virt_batch::PackedSourcePlanes>;
    fn packed_source_planes_with(
        &self,
        plane_major: bool,
    ) -> Option<crate::virt_batch::PackedSourcePlanes>;
    fn add_extra_hs(&self, hs: &mut [Gf; 128], p_msg: &[Gf128], a_cols: &[Gf; 128]);
    fn extra_a_prime_deltas(&self, phi_tables: &[Gf]) -> Vec<(usize, Gf)>;
    fn add_extra_a_prime(
        &self,
        basis: &mut [Gf],
        round0: &mut (Gf, Gf),
        rho: &[Gf],
        p_msg: &[Gf128],
    );
}
fn binary_equality(point: &[Gf], cfg: &()) -> Result<Vec<Gf>, ()> {
    crate::poly::utils::build_eq_x_r_vec(point, cfg).map_err(|_| ())
}
impl<'a, M: circuit::linear_map::binary::VirtualMap> VirtualOpeningWeights<'a, M>
    for BinaryAdjoint<'a, M>
{
    /// The weights with every compact-tail column folded (the prover's form:
    /// its batching message reads the tail weights per cell).
    fn new(map: &'a M, points: &[Vec<Gf>], etas: &[Gf], t_wh: usize) -> Self {
        Self::new_with_tail(
            map,
            points,
            etas,
            t_wh,
            false,
            binary_equality,
            cfg!(feature = "parallel"),
        )
    }

    /// The verifier's form: an identity compact tail stays factored
    /// ([`AffineTailWeights`]) — its alias columns still fold, the dense
    /// remainder is never materialized — so the ρ-batched basis can take it
    /// through the affine-tail plane engine. Every weight is the same value
    /// as in [`Self::new`] (pinned by
    /// `verifier_basis_matches_streamed_basis_on_the_ecdsa_map`).
    fn new_factored_tail(map: &'a M, points: &[Vec<Gf>], etas: &[Gf], t_wh: usize) -> Self {
        Self::new_with_tail(
            map,
            points,
            etas,
            t_wh,
            true,
            binary_equality,
            cfg!(feature = "parallel"),
        )
    }

    /// The plane engine ([`crate::virt_batch`]) for a packed-source
    /// repetition whose shape pays for it; `None` keeps the per-cell
    /// kernels. `BITZ_VIRT_PLANES=0` opts out (A/B; bit-identical messages
    /// either way).
    fn packed_source_planes(&self) -> Option<crate::virt_batch::PackedSourcePlanes<'_>> {
        self.packed_source_planes_with(true)
    }

    /// [`Self::packed_source_planes`] with or without the plane-major tables
    /// (only the batching message `h` reads them; the verifier's basis does not).
    fn packed_source_planes_with(
        &self,
        plane_major: bool,
    ) -> Option<crate::virt_batch::PackedSourcePlanes<'_>> {
        let Self::PackedSourceRepeated {
            local_width,
            eq_inst_gf,
            instances,
            s,
            constant_weight,
            ..
        } = self
        else {
            return None;
        };
        if !virt_planes()
            || !crate::virt_batch::PackedSourcePlanes::eligible(*local_width, *instances, s.len())
        {
            return None;
        }
        let build = if plane_major {
            crate::virt_batch::PackedSourcePlanes::new
        } else {
            crate::virt_batch::PackedSourcePlanes::new_basis_only
        };
        Some(build(
            *local_width,
            *instances,
            eq_inst_gf,
            s,
            *constant_weight,
        ))
    }

    /// Adds the extra terms' contribution to the batching message `hs`
    /// computed by the plane engine for the plain part (exact by
    /// linearity: `bit_b` and the field sum are additive over the weights).
    #[allow(clippy::arithmetic_side_effects)]
    fn add_extra_hs(&self, hs: &mut [Gf; 128], p_msg: &[Gf128], a_cols: &[Gf; 128]) {
        let packs = self.extra_packs();
        if packs.is_empty() {
            return;
        }
        const PACKS_PER_CHUNK: usize = 1 << 9;
        let partials: Vec<[Gf; 128]> = cfg_chunks!(packs, PACKS_PER_CHUNK)
            .map(|chunk| {
                let mut partial = [Gf::zero(); 128];
                let mut wits = [[0u64; 2]; 16];
                let mut vals = [Gf::zero(); 16];
                let mut fill = 0usize;
                let mut pack_w = [Gf::zero(); 128];
                for &pack in chunk {
                    if !self.pack_weights_extra(pack, &mut pack_w) {
                        continue;
                    }
                    let p_val = p_msg[pack];
                    for (slot, &weight) in pack_w.iter().enumerate() {
                        if weight == Gf::zero() {
                            continue;
                        }
                        wits[fill] = *weight.as_words();
                        vals[fill] = p_val * a_cols[slot];
                        fill += 1;
                        if fill == 16 {
                            hs_scatter_block16(&mut partial, &wits, &vals);
                            fill = 0;
                        }
                    }
                }
                for index in 0..fill {
                    crate::ligerito::sv_scalar_accum(&mut partial, wits[index], vals[index]);
                }
                partial
            })
            .collect();
        for partial in partials {
            for (target, value) in hs.iter_mut().zip(partial) {
                *target += value;
            }
        }
    }

    /// The extra terms' (chained nonconstant terms and corrections)
    /// contribution to the ρ-batched basis `a′`, per touched pack: the
    /// pack's extra weights through `Φ_ρ` (`phi_tables`) and the dual-basis
    /// combination — exactly the per-cell kernel restricted to those packs.
    #[allow(clippy::arithmetic_side_effects)]
    fn extra_a_prime_deltas(&self, phi_tables: &[Gf]) -> Vec<(usize, Gf)> {
        let packs = self.extra_packs();
        cfg_iter!(packs)
            .map(|&pack| {
                let mut pack_w = [Gf::zero(); 128];
                if !self.pack_weights_extra(pack, &mut pack_w) {
                    return (pack, Gf::zero());
                }
                for value in &mut pack_w {
                    *value = phi_from_words(*value.as_words(), phi_tables);
                }
                (
                    pack,
                    crate::dual_basis::dual_basis_linear_combination(&pack_w),
                )
            })
            .collect()
    }

    /// Adds the extra terms' contribution to the ρ-batched basis `a′` and
    /// to flock's round-0 pair computed by the plane engine for the plain
    /// part (both are linear in `a′`).
    #[allow(clippy::arithmetic_side_effects)]
    fn add_extra_a_prime(
        &self,
        basis: &mut [Gf],
        round0: &mut (Gf, Gf),
        rho: &[Gf],
        p_msg: &[Gf128],
    ) {
        let phi_tables = phi_byte_tables(rho, Gf::one());
        let deltas = self.extra_a_prime_deltas(&phi_tables);
        if deltas.is_empty() {
            return;
        }
        for &(pack, delta) in &deltas {
            basis[pack] += delta;
        }
        // Round-0 pair over the aligned pack pairs `(2j, 2j + 1)`.
        let mut index = 0;
        while index < deltas.len() {
            let (pack, delta) = deltas[index];
            let pair = pack & !1;
            let (d0, d1) = if pack == pair {
                let d1 = if index + 1 < deltas.len() && deltas[index + 1].0 == pair + 1 {
                    index += 1;
                    deltas[index].1
                } else {
                    Gf::zero()
                };
                (delta, d1)
            } else {
                (Gf::zero(), delta)
            };
            if pair + 1 < p_msg.len() {
                let f0 = p_msg[pair];
                let f1 = p_msg[pair + 1];
                round0.0 += f0 * d0;
                round0.1 += (f0 + f1) * (d0 + d1);
            }
            index += 1;
        }
    }
}
trait TailOpeningPlanes {
    fn planes(&self) -> AffineTailPlanes;
}
impl TailOpeningPlanes for circuit::linear_map::binary_adjoint::AffineTailWeights {
    fn planes(&self) -> AffineTailPlanes {
        AffineTailPlanes::new(
            self.source_start,
            self.len,
            self.row_start,
            &self.coeffs.eq_rs,
            &self.coeffs.scaled_zc,
        )
    }
}
/// The batching message computed by streaming source columns of the CSC map.
/// For source cell `j`, `W_j = Σ_{r:M[r,j]=1} E_r`; bit plane `i`
/// contributes `bit_i(W_j) · pack(f)[j>>7] · A(e_{j&127})`.
#[allow(clippy::arithmetic_side_effects)]
fn virtual_hs_fold<M>(
    map: &M,
    weights: &BinaryAdjoint<'_, M>,
    p_msg: &[Gf128],
    a_cols: &[Gf; 128],
) -> Box<[Gf; 128]>
where
    M: circuit::linear_map::binary::VirtualMap,
{
    // Keep the per-chunk 128-element accumulator comfortably below the
    // production source vector: at the 2^16 SHA batch this bounds the merge
    // buffer at 16 MiB instead of 128 MiB while retaining thousands of
    // tasks. 2^9 packs = the previous 2^16-column chunk boundaries, so the
    // nonzero-column accumulation order is unchanged.
    const PACKS_PER_CHUNK: usize = 1 << 9;
    let n_packs = map.cols() >> LOG_PACKING;
    let n_chunks = n_packs.div_ceil(PACKS_PER_CHUNK).max(1);
    let partials: Vec<[Gf; 128]> = cfg_into_iter!(0..n_chunks)
        .map(|chunk| {
            let lo = chunk * PACKS_PER_CHUNK;
            let hi = (lo + PACKS_PER_CHUNK).min(n_packs);
            let mut hs = [Gf::zero(); 128];
            let mut wits = [[0u64; 2]; 16];
            let mut vals = [Gf::zero(); 16];
            let mut fill = 0usize;
            let mut pack_w = [Gf::zero(); 128];

            for pack in lo..hi {
                if !weights.pack_weights(pack, &mut pack_w) {
                    continue;
                }
                let p_val = p_msg[pack];
                for (slot, &weight) in pack_w.iter().enumerate() {
                    if weight == Gf::zero() {
                        continue;
                    }
                    wits[fill] = *weight.as_words();
                    vals[fill] = p_val * a_cols[slot];
                    fill += 1;
                    if fill == 16 {
                        hs_scatter_block16(&mut hs, &wits, &vals);
                        fill = 0;
                    }
                }
            }
            for index in 0..fill {
                crate::ligerito::sv_scalar_accum(&mut hs, wits[index], vals[index]);
            }
            hs
        })
        .collect();

    let mut hs = Box::new([Gf::zero(); 128]);
    for partial in partials {
        for (target, value) in hs.iter_mut().zip(partial) {
            *target += value;
        }
    }
    hs
}

/// The ρ-batched dual-basis Ligerito basis, computed pack-by-pack from CSC
/// source columns. Each worker owns one output pack, so no partial dense
/// vectors or scatter synchronization are needed.
#[allow(clippy::arithmetic_side_effects)]
fn virtual_a_prime<M>(
    map: &M,
    weights: &BinaryAdjoint<'_, M>,
    rho: &[Gf],
    a_cols: &[Gf; 128],
    n_packs: usize,
) -> Vec<Gf>
where
    M: circuit::linear_map::binary::VirtualMap,
{
    debug_assert_eq!(map.cols(), n_packs << LOG_PACKING);
    let phi_tables = phi_byte_tables(rho, Gf::one());
    let mut result = vec![Gf::ZERO; n_packs];
    let live_packs = match weights {
        BinaryAdjoint::PackedSourceRepeated {
            live_cols,
            corrections,
            affine_tail,
            ..
        } => corrections
            .iter()
            .map(DenseWeightCorrection::end)
            .chain(affine_tail.iter().map(AffineTailWeights::end))
            .fold(*live_cols, usize::max)
            .div_ceil(1usize << LOG_PACKING)
            .min(n_packs),
        _ => n_packs,
    };
    let dense_packed_source = matches!(weights, BinaryAdjoint::PackedSourceRepeated { .. });
    cfg_iter_mut!(&mut result[..live_packs])
        .enumerate()
        .for_each(|(pack, output)| {
            let mut pack_w = [Gf::zero(); 128];
            if !weights.pack_weights(pack, &mut pack_w) {
                // Φ_ρ(0)·A(e_v) = 0: an all-zero pack contributes nothing.
                return;
            }
            let value = if dense_packed_source {
                for value in &mut pack_w {
                    *value = phi_from_words(*value.as_words(), &phi_tables);
                }
                crate::dual_basis::dual_basis_linear_combination(&pack_w)
            } else {
                let mut acc = Gf::zero();
                for (slot, &weight) in pack_w.iter().enumerate() {
                    if weight == Gf::zero() {
                        continue;
                    }
                    let phi = phi_from_words(*weight.as_words(), &phi_tables);
                    acc += phi * a_cols[slot];
                }
                acc
            };
            *output = value;
        });
    result
}

/// Packs per parallel task of the verifier's basis engines: a 2^21-cell
/// source is 64 tasks, enough to balance ten threads (the prover keeps its
/// 2^11-pack tasks; the per-task cost is one duplicated instance read-off).
const VERIFIER_TASK_PACKS: usize = 1 << 8;

/// The verifier's ρ-batched Ligerito basis `a′`: the packed-source plane
/// engine for the plain repetition (with the constant column), the
/// affine-tail engine for an identity compact tail, and the per-pack kernel
/// for the chained terms and the alias corrections — three exact
/// rearrangements of [`virtual_a_prime`]'s per-cell sum
/// `a′(y) = Σ_v Φ_ρ(W_{(y,v)})·A(e_v)` over the three parts of the weights
/// (`W = W_plain + W_tail + W_extra` cell for cell, and both `Φ_ρ` and the
/// dual-basis combination are additive), so the basis is the same field
/// vector [`virtual_a_prime`] returns on the folded weights (pinned by
/// `verifier_basis_matches_streamed_basis_on_the_ecdsa_map`). Any other
/// shape, or the plane engine opted out, takes [`virtual_a_prime`] itself.
fn verifier_a_prime<M>(
    map: &M,
    weights: &BinaryAdjoint<'_, M>,
    rho: &[Gf],
    n_packs: usize,
) -> Vec<Gf>
where
    M: circuit::linear_map::binary::VirtualMap,
{
    let planes = {
        let _g = tracing::info_span!("mqv:vplanes").entered();
        weights.packed_source_planes_with(false)
    };
    let Some(planes) = planes else {
        let a_cols = crate::dual_basis::dual_basis_cols();
        return virtual_a_prime(map, weights, rho, &a_cols, n_packs);
    };
    let rho_tables = phi_byte_tables(rho, Gf::one());
    let coefficient_tables = {
        let _g = tracing::info_span!("mqv:vrho").entered();
        RhoTables::new(rho)
    };
    let mut basis = vec![Gf::zero(); n_packs];
    {
        let _g = tracing::info_span!("mqv:vaprime_plain").entered();
        planes.add_a_prime(
            &coefficient_tables,
            &rho_tables,
            &mut basis,
            VERIFIER_TASK_PACKS,
        );
    }
    if let BinaryAdjoint::PackedSourceRepeated {
        affine_tail: Some(tail),
        ..
    } = weights
    {
        let _g = tracing::info_span!("mqv:vaprime_tail").entered();
        tail.planes()
            .add_a_prime(&coefficient_tables, &mut basis, VERIFIER_TASK_PACKS);
    }
    {
        let _g = tracing::info_span!("mqv:vaprime_extra").entered();
        for (pack, delta) in weights.extra_a_prime_deltas(&rho_tables) {
            basis[pack] += delta;
        }
    }
    basis
}

/// General virtual commitment bridge: apply `M^T` to the batched residual
/// weights, send the 128 dual-basis plane openings, and reduce them to one
/// native Ligerito inner-product claim on the committed source.
struct AdjointBatchProverReduction<'a, M> {
    map: &'a M,
    derived_row_bits: usize,
    source_packed_vars: usize,
}

impl<M> ModQLigProverReduction for AdjointBatchProverReduction<'_, M>
where
    M: circuit::linear_map::binary::VirtualMap,
{
    type Proof = Box<[Gf; 128]>;

    #[allow(clippy::arithmetic_side_effects)]
    fn prepare<T: Transcript + Send>(
        self,
        mut grinder: ProverGrindingTranscript<'_, T, ForestRoundGrinding>,
        points: &[Vec<Gf>],
        hint: &FlockCommitHint,
        ood: Option<&OodProverClaim>,
    ) -> PreparedProverLigeritoClaim<Self::Proof> {
        let etas: Vec<Gf> = grinder.get_field_challenges(points.len(), &());
        let weights = {
            let _g = tracing::info_span!("mqv:wprep").entered();
            BinaryAdjoint::new(self.map, points, &etas, self.derived_row_bits)
        };
        let a_cols = crate::dual_basis::dual_basis_cols();
        debug_assert_eq!(hint.p_msg.len(), 1usize << self.source_packed_vars);
        let planes = {
            let _g = tracing::info_span!("mqv:planes").entered();
            weights.packed_source_planes()
        };

        let hs = {
            let _g = tracing::info_span!("mqv:hs").entered();
            match &planes {
                Some(planes) => {
                    let mut hs = planes.hs_fold(&hint.p_msg);
                    weights.add_extra_hs(&mut hs, &hint.p_msg, &a_cols);
                    hs
                }
                None => virtual_hs_fold(self.map, &weights, &hint.p_msg, &a_cols),
            }
        };
        crate::ligerito::absorb_hs(&mut grinder, hs.as_ref());

        let r2: Vec<Gf> = grinder.get_field_challenges(LOG_PACKING, &());
        let eta_ood: Option<Gf> = ood.map(|_| grinder.get_field_challenge(&()));
        let grinding_nonces = grinder.finish();
        let rho = crate::poly::utils::build_eq_x_r_vec(&r2, &()).expect("r2");
        let mut target = rho
            .iter()
            .zip(hs.iter())
            .fold(Gf::zero(), |acc, (&r, &h)| acc + r * h);
        let (mut basis, mut precomputed_round0): (Vec<Gf128>, Option<(Gf, Gf)>) = {
            let _g = tracing::info_span!("mqv:aprime").entered();
            match &planes {
                Some(planes) => {
                    let (mut basis, mut round0) = planes.a_prime(&rho, &hint.p_msg);
                    weights.add_extra_a_prime(&mut basis, &mut round0, &rho, &hint.p_msg);
                    (basis.into_iter().collect(), rs_fast().then_some(round0))
                }
                None => (
                    virtual_a_prime(self.map, &weights, &rho, &a_cols, hint.p_msg.len()),
                    None,
                ),
            }
        };
        if let (Some(claim), Some(eta)) = (ood, eta_ood) {
            let _g = tracing::info_span!("mqv:ood_basis").entered();
            add_ood_basis(
                &mut basis,
                &hint.p_msg,
                &claim.point,
                eta,
                precomputed_round0.as_mut(),
            );
            target += eta * claim.y;
        }

        PreparedProverLigeritoClaim {
            reduction: hs,
            target,
            basis,
            precomputed_round0,
            grinding_nonces,
        }
    }
}

/// Prove `Σ_c w'_c·(Σ_b rw[b]·h_{b,c}) = y ∈ 𝔽_q` for the derived vector
/// `h = M·f`, against the commitment to `f` (`hint_f`). Same claim shape
/// as [`prove_mle_eval_mod_q_ligerito`], with `h` in `h_layout` geometry —
/// `row_weights_q[b] ∈ [0, 2^q_bits)` over `h`'s `2^{t_h}` rows. The
/// correctly shaped `h_rows` are supplied by synthesis; proving never
/// computes a forward `M f` product.
///
/// # Panics
///
/// Panics before transcript absorption if trusted prover inputs have invalid
/// geometry, including malformed `h_rows`, a map/commitment shape mismatch,
/// fewer than two derived columns, or out-of-range mod-q weights.
#[allow(clippy::arithmetic_side_effects)]
pub fn prove_mle_eval_mod_q_ligerito_virtual_with_ood<M>(
    transcript: &mut (impl Transcript + Send),
    hint_f: &FlockCommitHint,
    h_rows: &[Vec<u64>],
    h_layout: &IntegerMatrixLayout,
    f_layout: &IntegerMatrixLayout,
    map: &M,
    row_weights_q: &[u128],
    q_bits: usize,
    alpha: Gf,
    ood: impl Into<ProverOod>,
    pc: &LigProverConfig,
) -> IntEvalRsLigVirtProof
where
    M: circuit::linear_map::binary::VirtualMap,
{
    let chunks = ModQWeightChunks::from_dense(h_layout, row_weights_q, q_bits)
        .expect("q_bits must be in [1, 126] and every row weight must be < 2^q_bits");
    prove_mle_eval_mod_q_ligerito_virtual_with_weight_chunks_and_modulus(
        transcript,
        hint_f,
        h_rows,
        h_layout,
        f_layout,
        map,
        &chunks,
        crate::pcs::FQ_MOD,
        q_bits,
        alpha,
        0,
        ood,
        pc,
    )
}

/// Runtime-prime form of [`prove_mle_eval_mod_q_ligerito_virtual`].
///
/// `q` is transcript-derived by the statement-owning protocol.  It is bound
/// into this subprotocol's statement in full; `q_bits` controls chunk geometry
/// and must be the actual bit length of `q`.
#[allow(clippy::arithmetic_side_effects)]
#[allow(clippy::too_many_arguments)]
pub fn prove_mle_eval_mod_q_ligerito_virtual_runtime<M>(
    transcript: &mut (impl Transcript + Send),
    hint_f: &FlockCommitHint,
    h_rows: &[Vec<u64>],
    h_layout: &IntegerMatrixLayout,
    f_layout: &IntegerMatrixLayout,
    map: &M,
    row_weights_q: &[u128],
    q: u128,
    q_bits: usize,
    alpha: Gf,
    forest_grinding_bits: u32,
    ood: impl Into<ProverOod>,
    pc: &LigProverConfig,
) -> Result<IntEvalRsLigVirtProof, FlockRsError>
where
    M: circuit::linear_map::binary::VirtualMap,
{
    validate_runtime_q(q, q_bits, row_weights_q)?;
    let chunks = ModQWeightChunks::from_dense(h_layout, row_weights_q, q_bits)
        .map_err(|()| FlockRsError::RingSwitch(RsOpenError::Shape))?;
    Ok(
        prove_mle_eval_mod_q_ligerito_virtual_with_weight_chunks_and_modulus(
            transcript,
            hint_f,
            h_rows,
            h_layout,
            f_layout,
            map,
            &chunks,
            q,
            q_bits,
            alpha,
            forest_grinding_bits,
            ood,
            pc,
        ),
    )
}

/// Runtime-prime virtual opening with row weights already decomposed into
/// validated chunk-major form. Direct callers can populate the chunks in
/// bounded ranges and avoid retaining a dense `2^t` canonical-weight vector.
#[allow(clippy::arithmetic_side_effects)]
#[allow(clippy::too_many_arguments)]
#[allow(dead_code)]
pub(crate) fn prove_mle_eval_mod_q_ligerito_virtual_with_weight_chunks_runtime<M>(
    transcript: &mut (impl Transcript + Send),
    hint_f: &FlockCommitHint,
    h_rows: &[Vec<u64>],
    h_layout: &IntegerMatrixLayout,
    f_layout: &IntegerMatrixLayout,
    map: &M,
    chunks: &ModQWeightChunks,
    q: u128,
    q_bits: usize,
    alpha: Gf,
    forest_grinding_bits: u32,
    ood: impl Into<ProverOod>,
    pc: &LigProverConfig,
) -> Result<IntEvalRsLigVirtProof, FlockRsError>
where
    M: circuit::linear_map::binary::VirtualMap,
{
    validate_runtime_q_source(q, q_bits, chunks)?;
    checked_mod_q_weight_chunks_geometry(h_layout, chunks, q_bits)?;
    Ok(
        prove_mle_eval_mod_q_ligerito_virtual_with_weight_chunks_and_modulus(
            transcript,
            hint_f,
            h_rows,
            h_layout,
            f_layout,
            map,
            chunks,
            q,
            q_bits,
            alpha,
            forest_grinding_bits,
            ood,
            pc,
        ),
    )
}

/// Runtime-prime virtual opening from a canonical row-weight generator.
///
/// At most one `2^t`-row base-`2^c_w` limb is live at once.  The ordinary
/// chunk-backed API above implements the same source interface and therefore
/// exercises exactly the same transcript/proof core.
#[allow(clippy::arithmetic_side_effects)]
#[allow(clippy::too_many_arguments)]
pub(crate) fn prove_mle_eval_mod_q_ligerito_virtual_with_weight_source_runtime<M, S>(
    transcript: &mut (impl Transcript + Send),
    hint_f: &FlockCommitHint,
    h_rows: &[Vec<u64>],
    h_layout: &IntegerMatrixLayout,
    f_layout: &IntegerMatrixLayout,
    map: &M,
    source: &S,
    q: u128,
    q_bits: usize,
    alpha: Gf,
    forest_grinding_bits: u32,
    ood: impl Into<ProverOod>,
    pc: &LigProverConfig,
) -> Result<IntEvalRsLigVirtProof, FlockRsError>
where
    M: circuit::linear_map::binary::VirtualMap,
    S: ModQWeightSource + ?Sized,
{
    validate_runtime_q_source(q, q_bits, source)?;
    checked_mod_q_weight_source_geometry(h_layout, source, q_bits)?;
    Ok(
        prove_mle_eval_mod_q_ligerito_virtual_with_weight_chunks_and_modulus(
            transcript,
            hint_f,
            h_rows,
            h_layout,
            f_layout,
            map,
            source,
            q,
            q_bits,
            alpha,
            forest_grinding_bits,
            ood,
            pc,
        ),
    )
}

#[allow(clippy::arithmetic_side_effects)]
#[allow(clippy::too_many_arguments)]
fn prove_mle_eval_mod_q_ligerito_virtual_with_weight_chunks_and_modulus<M, S>(
    transcript: &mut (impl Transcript + Send),
    hint_f: &FlockCommitHint,
    h_rows: &[Vec<u64>],
    h_layout: &IntegerMatrixLayout,
    f_layout: &IntegerMatrixLayout,
    map: &M,
    chunks: &S,
    q: u128,
    q_bits: usize,
    alpha: Gf,
    forest_grinding_bits: u32,
    ood: impl Into<ProverOod>,
    pc: &LigProverConfig,
) -> IntEvalRsLigVirtProof
where
    M: circuit::linear_map::binary::VirtualMap,
    S: ModQWeightSource + ?Sized,
{
    prove_mle_eval_mod_q_ligerito_virtual_with_weight_chunks_and_modulus_with_security(
        transcript,
        hint_f,
        h_rows,
        h_layout,
        f_layout,
        map,
        chunks,
        q,
        q_bits,
        alpha,
        forest_grinding_bits,
        ood,
        pc,
        None,
    )
}

pub(crate) fn prove_mle_eval_mod_q_ligerito_virtual_with_weight_chunks_and_modulus_with_security<
    M,
    S,
>(
    transcript: &mut (impl Transcript + Send),
    hint_f: &FlockCommitHint,
    h_rows: &[Vec<u64>],
    h_layout: &IntegerMatrixLayout,
    f_layout: &IntegerMatrixLayout,
    map: &M,
    chunks: &S,
    q: u128,
    q_bits: usize,
    alpha: Gf,
    forest_grinding_bits: u32,
    ood: impl Into<ProverOod>,
    pc: &LigProverConfig,
    security: Option<&mut grinding::GrindingContext<'_>>,
) -> IntEvalRsLigVirtProof
where
    M: circuit::linear_map::binary::VirtualMap,
    S: ModQWeightSource + ?Sized,
{
    assert!(
        chunks
            .padding_bound()
            .is_none_or(|b| b.matches_map(h_layout, map)),
        "padding bound must match the virtual statement"
    );
    let (h_geometry, _, _) = checked_mod_q_weight_source_geometry(h_layout, chunks, q_bits)
        .expect("h_layout, q_bits, and chunks must define valid mod-q geometry");
    let f_geometry = validate_int_eval_geometry(&hint_f.commitment, f_layout, 0)
        .expect("commitment geometry must match f_layout");
    assert!(
        h_geometry.row_bit_vars >= LOG_PACKING,
        "h rows must contain at least one 128-bit pack"
    );
    let h_cells = h_geometry
        .rows
        .checked_mul(h_layout.word_bits)
        .and_then(|count| count.checked_mul(h_geometry.cols))
        .expect("h_layout cell count must fit usize");
    let f_cells = f_geometry
        .rows
        .checked_mul(f_layout.word_bits)
        .and_then(|count| count.checked_mul(f_geometry.cols))
        .expect("f_layout cell count must fit usize");
    let h_words_per_row = h_geometry
        .rows
        .checked_mul(h_layout.word_bits)
        .expect("h row width must fit usize")
        / u64::BITS as usize;
    let t_wh = h_geometry.row_bit_vars;
    assert_eq!(map.rows(), h_cells, "map rows must match h_layout cells");
    assert_eq!(map.cols(), f_cells, "map cols must match f_layout cells");
    assert_eq!(h_rows.len(), h_geometry.cols, "h_rows column count");
    assert!(
        h_rows.iter().all(|row| row.len() == h_words_per_row),
        "h_rows word count"
    );
    validate_ligerito_commitment(&hint_f.commitment, pc)
        .expect("commitment metadata must match the Ligerito config");

    let _bound_statement = {
        let _g = tracing::info_span!("mqv:stmt").entered();
        absorb_virtual_statement(
            transcript,
            &hint_f.commitment,
            h_layout,
            f_layout,
            map,
            chunks,
            q,
            q_bits,
            alpha,
        )
    };

    // Identity fast path: `h = f` (same cells, same row layout), so the
    // whole derived-vector machinery — packing `h`, the `h_i` fold, and the
    // `a′` build — is skipped and the BASE opening runs on `f`'s own
    // rows under `h_layout`'s claim shape (the flat bit-MLE is
    // layout-agnostic, and the layouts coincide here anyway).
    if virtual_id_fast_eligible(map, h_layout, f_layout) && virt_id_fast() {
        let _g = tracing::info_span!("mqv:idfast").entered();
        let core = prove_mod_q_lig_core_with_security(
            transcript,
            hint_f,
            h_layout,
            &hint_f.rows,
            Some(hint_f.packed_cols()),
            chunks,
            alpha,
            pc,
            forest_grinding_bits,
            ood,
            EqProverReduction {
                packed_vars: packed_vars(f_layout),
            },
            security,
        );
        return IntEvalRsLigVirtProof {
            mfs: core.mfs,
            us: core.us,
            presums: core.presums,
            reduction: VirtualReductionProof::Eq {
                rings: core.reduction,
            },
            lig: core.lig,
            grinding_nonces: core.grinding_nonces,
            ood: core.ood,
        };
    }

    // The common prefix operates on synthesized `h`; the selected reduction
    // alone knows about `M`, and the common suffix opens committed `f`.
    let h_packed = {
        let _g = tracing::info_span!("mqv:pack").entered();
        crate::ligerito::pack_columns_from_rows(h_layout, h_rows)
    };
    let core = prove_mod_q_lig_core_with_security(
        transcript,
        hint_f,
        h_layout,
        h_rows,
        Some(&h_packed),
        chunks,
        alpha,
        pc,
        forest_grinding_bits,
        ood,
        AdjointBatchProverReduction {
            map,
            derived_row_bits: t_wh,
            source_packed_vars: packed_vars(f_layout),
        },
        security,
    );
    IntEvalRsLigVirtProof {
        mfs: core.mfs,
        us: core.us,
        presums: core.presums,
        reduction: VirtualReductionProof::AdjointBatch { hs: core.reduction },
        lig: core.lig,
        grinding_nonces: core.grinding_nonces,
        ood: core.ood,
    }
}

/// Verify a virtual mod-q claim `Σ_c w'_c·(Σ_b rw[b]·h_{b,c}) = claimed`
/// for `h = M·f` against `f`'s commitment. `col_weights[c] ∈ R` over
/// `h`'s `2^{s_h}` columns. The verifier's `M`-dependent cost is
/// `O(L·nnz(M) + #cols(M))` field operations, plus the dense source-pack
/// basis that the Ligerito residual hook MLE-folds.
#[allow(clippy::arithmetic_side_effects)]
#[allow(clippy::too_many_arguments)]
pub fn verify_mle_eval_mod_q_ligerito_virtual_with_ood<R, M>(
    transcript: &mut (impl Transcript + Send),
    commitment_f: &Commitment,
    proof: &IntEvalRsLigVirtProof,
    h_layout: &IntegerMatrixLayout,
    f_layout: &IntegerMatrixLayout,
    map: &M,
    row_weights_q: &[u128],
    col_weights: &[R],
    alpha: Gf,
    claimed: R,
    q_bits: usize,
    ood: impl Into<VerifierOod>,
    vc: &LigVerifierConfig,
) -> Result<(), FlockRsError>
where
    R: Copy + PartialEq + From<u128> + core::ops::Add<Output = R> + core::ops::Mul<Output = R>,
    M: circuit::linear_map::binary::VirtualMap,
{
    let chunks = ModQWeightChunks::from_dense(h_layout, row_weights_q, q_bits)
        .map_err(|()| FlockRsError::RingSwitch(RsOpenError::Shape))?;
    verify_mle_eval_mod_q_ligerito_virtual_with_weight_chunks_and_read_off(
        transcript,
        commitment_f,
        proof,
        h_layout,
        f_layout,
        map,
        &chunks,
        crate::pcs::FQ_MOD,
        q_bits,
        alpha,
        0,
        ood,
        vc,
        col_weights.len(),
        |v, c_w, lch| {
            crate::pcs::recombine_read_off(h_layout, v, 0, col_weights, c_w, lch) == claimed
        },
    )
}

/// Runtime-prime verifier for the virtual mod-`q` opening.
///
/// All row/column weights and the claim are canonical integers in `[0, q)`.
/// Recombination uses [`field::FpCtx<2>`] under the supplied
/// modulus, matching the SHA integer-to-field projection.
#[allow(clippy::arithmetic_side_effects)]
#[allow(clippy::too_many_arguments)]
pub fn verify_mle_eval_mod_q_ligerito_virtual_runtime<M>(
    transcript: &mut (impl Transcript + Send),
    commitment_f: &Commitment,
    proof: &IntEvalRsLigVirtProof,
    h_layout: &IntegerMatrixLayout,
    f_layout: &IntegerMatrixLayout,
    map: &M,
    row_weights_q: &[u128],
    col_weights_q: &[u128],
    alpha: Gf,
    claimed_q: u128,
    q: u128,
    q_bits: usize,
    forest_grinding_bits: u32,
    ood: impl Into<VerifierOod>,
    vc: &LigVerifierConfig,
) -> Result<(), FlockRsError>
where
    M: circuit::linear_map::binary::VirtualMap,
{
    validate_runtime_q(q, q_bits, row_weights_q)?;
    if claimed_q >= q || col_weights_q.iter().any(|&weight| weight >= q) {
        return Err(FlockRsError::RingSwitch(RsOpenError::Shape));
    }
    let chunks = ModQWeightChunks::from_dense(h_layout, row_weights_q, q_bits)
        .map_err(|()| FlockRsError::RingSwitch(RsOpenError::Shape))?;
    let arithmetic = field::FpCtx::from_prime_u128(q);
    verify_mle_eval_mod_q_ligerito_virtual_with_weight_chunks_and_read_off(
        transcript,
        commitment_f,
        proof,
        h_layout,
        f_layout,
        map,
        &chunks,
        q,
        q_bits,
        alpha,
        forest_grinding_bits,
        ood,
        vc,
        col_weights_q.len(),
        |v, c_w, lch| {
            recombine_read_off_runtime(h_layout, v, col_weights_q, c_w, lch, &arithmetic)
                == claimed_q
        },
    )
}

/// Runtime-prime virtual verifier for row weights already represented as
/// validated chunk-major limbs. The canonical statement is reconstructed
/// lazily from `chunks`, in the same row order as the dense API.
#[allow(clippy::arithmetic_side_effects)]
#[allow(clippy::too_many_arguments)]
#[allow(dead_code)]
pub(crate) fn verify_mle_eval_mod_q_ligerito_virtual_with_weight_chunks_runtime<M>(
    transcript: &mut (impl Transcript + Send),
    commitment_f: &Commitment,
    proof: &IntEvalRsLigVirtProof,
    h_layout: &IntegerMatrixLayout,
    f_layout: &IntegerMatrixLayout,
    map: &M,
    chunks: &ModQWeightChunks,
    col_weights_q: &[u128],
    alpha: Gf,
    claimed_q: u128,
    q: u128,
    q_bits: usize,
    forest_grinding_bits: u32,
    ood: impl Into<VerifierOod>,
    vc: &LigVerifierConfig,
) -> Result<(), FlockRsError>
where
    M: circuit::linear_map::binary::VirtualMap,
{
    validate_runtime_q_source(q, q_bits, chunks)?;
    checked_mod_q_weight_chunks_geometry(h_layout, chunks, q_bits)?;
    if claimed_q >= q || col_weights_q.iter().any(|&weight| weight >= q) {
        return Err(FlockRsError::RingSwitch(RsOpenError::Shape));
    }
    let arithmetic = field::FpCtx::from_prime_u128(q);
    verify_mle_eval_mod_q_ligerito_virtual_with_weight_chunks_and_read_off(
        transcript,
        commitment_f,
        proof,
        h_layout,
        f_layout,
        map,
        chunks,
        q,
        q_bits,
        alpha,
        forest_grinding_bits,
        ood,
        vc,
        col_weights_q.len(),
        |v, c_w, lch| {
            recombine_read_off_runtime(h_layout, v, col_weights_q, c_w, lch, &arithmetic)
                == claimed_q
        },
    )
}

/// Runtime-prime virtual verifier from the same canonical streaming source as
/// the prover.  Canonical statement hashing and chunk materialization share
/// one source, so a generated source cannot silently bind one weight sequence
/// and prove another.
#[allow(clippy::arithmetic_side_effects)]
#[allow(clippy::too_many_arguments)]
pub(crate) fn verify_mle_eval_mod_q_ligerito_virtual_with_weight_source_runtime<M, S>(
    transcript: &mut (impl Transcript + Send),
    commitment_f: &Commitment,
    proof: &IntEvalRsLigVirtProof,
    h_layout: &IntegerMatrixLayout,
    f_layout: &IntegerMatrixLayout,
    map: &M,
    source: &S,
    col_weights_q: &[u128],
    alpha: Gf,
    claimed_q: u128,
    q: u128,
    q_bits: usize,
    forest_grinding_bits: u32,
    ood: impl Into<VerifierOod>,
    vc: &LigVerifierConfig,
) -> Result<(), FlockRsError>
where
    M: circuit::linear_map::binary::VirtualMap,
    S: ModQWeightSource + ?Sized,
{
    validate_runtime_q_source(q, q_bits, source)?;
    checked_mod_q_weight_source_geometry(h_layout, source, q_bits)?;
    if claimed_q >= q || col_weights_q.iter().any(|&weight| weight >= q) {
        return Err(FlockRsError::RingSwitch(RsOpenError::Shape));
    }
    let arithmetic = field::FpCtx::from_prime_u128(q);
    verify_mle_eval_mod_q_ligerito_virtual_with_weight_chunks_and_read_off(
        transcript,
        commitment_f,
        proof,
        h_layout,
        f_layout,
        map,
        source,
        q,
        q_bits,
        alpha,
        forest_grinding_bits,
        ood,
        vc,
        col_weights_q.len(),
        |v, c_w, lch| {
            recombine_read_off_runtime(h_layout, v, col_weights_q, c_w, lch, &arithmetic)
                == claimed_q
        },
    )
}

#[allow(clippy::arithmetic_side_effects)]
#[allow(clippy::too_many_arguments)]
fn verify_mle_eval_mod_q_ligerito_virtual_with_weight_chunks_and_read_off<M, S, C>(
    transcript: &mut (impl Transcript + Send),
    commitment_f: &Commitment,
    proof: &IntEvalRsLigVirtProof,
    h_layout: &IntegerMatrixLayout,
    f_layout: &IntegerMatrixLayout,
    map: &M,
    chunks: &S,
    q: u128,
    q_bits: usize,
    alpha: Gf,
    forest_grinding_bits: u32,
    ood: impl Into<VerifierOod>,
    vc: &LigVerifierConfig,
    col_weight_count: usize,
    read_off_accepts: C,
) -> Result<(), FlockRsError>
where
    M: circuit::linear_map::binary::VirtualMap,
    S: ModQWeightSource + ?Sized,
    C: Fn(&[u128], usize, usize) -> bool,
{
    verify_mle_eval_mod_q_ligerito_virtual_with_weight_chunks_and_read_off_with_security(
        transcript,
        commitment_f,
        proof,
        h_layout,
        f_layout,
        map,
        chunks,
        q,
        q_bits,
        alpha,
        forest_grinding_bits,
        ood,
        vc,
        col_weight_count,
        read_off_accepts,
        None,
    )
}

pub(crate) fn verify_mle_eval_mod_q_ligerito_virtual_with_weight_chunks_and_read_off_with_security<
    M,
    S,
    C,
>(
    transcript: &mut (impl Transcript + Send),
    commitment_f: &Commitment,
    proof: &IntEvalRsLigVirtProof,
    h_layout: &IntegerMatrixLayout,
    f_layout: &IntegerMatrixLayout,
    map: &M,
    chunks: &S,
    q: u128,
    q_bits: usize,
    alpha: Gf,
    forest_grinding_bits: u32,
    ood: impl Into<VerifierOod>,
    vc: &LigVerifierConfig,
    col_weight_count: usize,
    read_off_accepts: C,
    security: Option<&mut grinding::GrindingContext<'_>>,
) -> Result<(), FlockRsError>
where
    M: circuit::linear_map::binary::VirtualMap,
    S: ModQWeightSource + ?Sized,
    C: Fn(&[u128], usize, usize) -> bool,
{
    let shape = || FlockRsError::RingSwitch(RsOpenError::Shape);
    if chunks
        .padding_bound()
        .is_some_and(|b| !b.matches_map(h_layout, map))
    {
        return Err(shape());
    }
    let (h_geometry, _, _) = checked_mod_q_weight_source_geometry(h_layout, chunks, q_bits)?;
    let f_geometry = validate_int_eval_geometry(commitment_f, f_layout, 0)?;
    let h_cells = h_geometry
        .rows
        .checked_mul(h_layout.word_bits)
        .and_then(|count| count.checked_mul(h_geometry.cols))
        .ok_or_else(shape)?;
    let f_cells = f_geometry
        .rows
        .checked_mul(f_layout.word_bits)
        .and_then(|count| count.checked_mul(f_geometry.cols))
        .ok_or_else(shape)?;
    let t_wh = h_geometry.row_bit_vars;
    if t_wh < LOG_PACKING
        || map.rows() != h_cells
        || map.cols() != f_cells
        || col_weight_count != h_geometry.cols
    {
        return Err(FlockRsError::RingSwitch(RsOpenError::Shape));
    }
    validate_ligerito_commitment(commitment_f, vc)?;

    let _bound_statement = {
        let _g = tracing::info_span!("mqv:stmt").entered();
        absorb_virtual_statement(
            transcript,
            commitment_f,
            h_layout,
            f_layout,
            map,
            chunks,
            q,
            q_bits,
            alpha,
        )
    };

    // The proof selects messages, never authorization: Eq is constructed
    // only after the statement-derived identity/layout gate succeeds.
    let reduction = match &proof.reduction {
        VirtualReductionProof::Eq { rings } => {
            if !virtual_id_fast_eligible(map, h_layout, f_layout) {
                return Err(FlockRsError::RingSwitch(RsOpenError::Shape));
            }
            VirtualVerifierReduction::Eq(EqVerifierReduction {
                rings,
                packed_vars: packed_vars(f_layout),
            })
        }
        VirtualReductionProof::AdjointBatch { hs } => {
            VirtualVerifierReduction::AdjointBatch(AdjointBatchVerifierReduction {
                map,
                hs: hs.as_ref(),
                derived_row_bits: t_wh,
                source_packed_vars: packed_vars(f_layout),
            })
        }
    };
    verify_mod_q_lig_core_with_security(
        transcript,
        commitment_f,
        proof.into(),
        h_layout,
        chunks,
        alpha,
        vc,
        forest_grinding_bits,
        ood,
        reduction,
        |values, chunk_width, chunk_count| {
            if !read_off_accepts(values, chunk_width, chunk_count) {
                return Err(FlockRsError::Common(IntEvalRsError::ReadOff));
            }
            Ok(())
        },
        security,
    )?;
    Ok(())
}

fn validate_runtime_q(q: u128, q_bits: usize, row_weights_q: &[u128]) -> Result<(), FlockRsError> {
    validate_runtime_modulus(q, q_bits)?;
    if row_weights_q.iter().any(|&weight| weight >= q) {
        return Err(FlockRsError::RingSwitch(RsOpenError::Shape));
    }
    Ok(())
}

fn validate_runtime_q_source<S>(q: u128, q_bits: usize, chunks: &S) -> Result<(), FlockRsError>
where
    S: ModQWeightSource + ?Sized,
{
    validate_runtime_modulus(q, q_bits)?;
    if chunks.q_bits() != q_bits
        || (0..chunks.row_count()).any(|row| {
            chunks
                .canonical_weight(row)
                .is_none_or(|weight| weight >= q)
        })
    {
        return Err(FlockRsError::RingSwitch(RsOpenError::Shape));
    }
    Ok(())
}

fn validate_runtime_modulus(q: u128, q_bits: usize) -> Result<(), FlockRsError> {
    let actual_bits = (u128::BITS - q.leading_zeros()) as usize;
    if q <= 2
        || q & 1 == 0
        || q >= (1_u128 << 126)
        || q_bits != actual_bits
        || !field::is_probable_prime_public(&field::Uint::from(q))
    {
        return Err(FlockRsError::RingSwitch(RsOpenError::Shape));
    }
    Ok(())
}

#[allow(clippy::arithmetic_side_effects)]
fn recombine_read_off_runtime(
    p: &IntegerMatrixLayout,
    values: &[u128],
    col_weights_q: &[u128],
    chunk_width: usize,
    chunk_count: usize,
    arithmetic: &field::FpCtx<2>,
) -> u128 {
    let chunk_base = arithmetic.reduce_u128(1_u128 << chunk_width);
    let mut result = 0_u128;
    for (column, &column_weight) in col_weights_q.iter().enumerate() {
        let mut column_value = 0_u128;
        let mut place = 1_u128;
        for chunk in 0..chunk_count {
            let index = (chunk << p.col_vars) + column;
            column_value =
                arithmetic.add_u128(column_value, arithmetic.mul_u128(place, values[index]));
            place = arithmetic.mul_u128(place, chunk_base);
        }
        result = arithmetic.add_u128(result, arithmetic.mul_u128(column_weight, column_value));
    }
    result
}

// ---------------------------------------------------------------------
// Host proof-stream (de)serialization of `IntEvalRsLigVirtProof`
// ---------------------------------------------------------------------

impl IntEvalRsLigVirtProof {
    /// Serialize the virtual-opening proof into the host proof stream:
    /// per chunk the merged forest, the (canonically zero-tail-trimmed)
    /// chunk folds, and the pre-sumcheck — each encoded EXACTLY like the
    /// base [`IntEvalRsLigModQProof::to_bytes`] — then ONE reduction tag byte
    /// (0 = AdjointBatch, 1 = identity-fast Eq), the reduction (AdjointBatch: the 128
    /// `h_i`; eq: per chunk the 128-element `s_v` — both fixed counts,
    /// no length prefixes), and the flock [`LigeritoProof`] as a
    /// length-prefixed `bincode` 1.3 blob. Mirrors [`Self::from_bytes`].
    #[allow(clippy::arithmetic_side_effects)]
    pub fn to_bytes(&self) -> Vec<u8> {
        use crate::proof_codec::Writer;
        let mut w = Writer::new();
        w.bytes(b"BITZV002");
        let lch = self.mfs.len();
        w.len(lch);
        for l in 0..lch {
            write_mod_q_chunk_record(&mut w, &self.mfs[l], &self.us[l], &self.presums[l]);
        }
        match &self.reduction {
            VirtualReductionProof::AdjointBatch { hs } => {
                w.bytes(&[0u8]);
                for g in hs.iter() {
                    w.gf(g);
                }
            }
            VirtualReductionProof::Eq { rings } => {
                assert_eq!(rings.len(), lch, "one ring-switch message per chunk");
                w.bytes(&[1u8]);
                for ring in rings {
                    assert_eq!(ring.s_v.len(), 128, "s_v is always 128 elements");
                    for g in &ring.s_v {
                        w.gf(g);
                    }
                }
            }
        }
        write_ligerito_blob(&mut w, &self.lig);
        write_proof_trailer(&mut w, &self.grinding_nonces, self.ood.as_ref());
        w.into_vec()
    }

    /// Deserialize the virtual-opening proof from the host proof stream.
    /// Mirrors [`Self::to_bytes`]; rejects non-minimal chunk-fold
    /// encodings exactly like the base codec.
    #[allow(clippy::arithmetic_side_effects)]
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, crate::proof_codec::CodecError> {
        use crate::proof_codec::{CodecError, Reader};
        let mut r = Reader::new(bytes);
        if r.take(8)? != b"BITZV002" {
            return Err(crate::proof_codec::CodecError::NonCanonical);
        }
        let lch = r.len()?;
        let mut mfs = Vec::with_capacity(lch.min(64));
        let mut us = Vec::with_capacity(lch.min(64));
        let mut presums = Vec::with_capacity(lch.min(64));
        for _ in 0..lch {
            let (forest, folds, presum) = read_mod_q_chunk_record(&mut r)?;
            mfs.push(forest);
            us.push(folds);
            presums.push(presum);
        }
        let reduction = match r.take(1)?[0] {
            0 => {
                let mut hs = [Gf::zero(); 128];
                for h in &mut hs {
                    *h = r.gf()?;
                }
                VirtualReductionProof::AdjointBatch { hs: Box::new(hs) }
            }
            1 => {
                let mut rings = Vec::with_capacity(lch.min(64));
                for _ in 0..lch {
                    let mut s_v = Vec::with_capacity(128);
                    for _ in 0..128 {
                        s_v.push(r.gf()?);
                    }
                    rings.push(RingSwitchProof { s_v });
                }
                VirtualReductionProof::Eq { rings }
            }
            _ => return Err(CodecError::NonCanonical),
        };
        let lig = read_ligerito_blob(&mut r)?;
        let (grinding_nonces, ood) = read_proof_trailer(&mut r)?;
        Ok(IntEvalRsLigVirtProof {
            mfs,
            us,
            presums,
            reduction,
            lig,
            grinding_nonces,
            ood,
        })
    }
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

    struct CountingTranscript {
        inner: Blake3Transcript,
        challenges: usize,
    }

    impl CountingTranscript {
        fn new() -> Self {
            Self {
                inner: Blake3Transcript::new(),
                challenges: 0,
            }
        }
    }

    impl Transcript for CountingTranscript {
        fn begin_sampling(&mut self) {
            self.challenges += 1;
            self.inner.begin_sampling();
        }
        fn fill_sampling_bytes(&mut self, output: &mut [u8]) {
            self.inner.fill_sampling_bytes(output);
        }

        fn get_challenge<T: crate::transcript::traits::ConstTranscribable>(&mut self) -> T {
            self.challenges = self.challenges.wrapping_add(1);
            self.inner.get_challenge()
        }

        fn absorb_inner(&mut self, value: &[u8]) {
            self.inner.absorb_inner(value);
        }
    }

    /// Test-only wrapper that forces the ordinary virtual tail for an identity
    /// matrix, so source-parity coverage exercises the one-limb-at-a-time
    /// general path as well as the identity shortcut used in production.
    struct GeneralPathMap(circuit::linear_map::binary::PreparedVirtualMap);

    impl circuit::linear_map::binary::VirtualMap for GeneralPathMap {
        type ColumnRows<'a>
            = <circuit::linear_map::binary::PreparedVirtualMap as circuit::linear_map::binary::VirtualMap>::ColumnRows<'a>
        where
            Self: 'a;

        fn rows(&self) -> usize {
            self.0.rows()
        }

        fn cols(&self) -> usize {
            self.0.cols()
        }

        fn nnz(&self) -> usize {
            self.0.nnz()
        }

        fn digest(&self) -> [u8; 32] {
            self.0.digest()
        }

        fn is_identity(&self) -> bool {
            false
        }

        fn column_rows(&self, column: usize) -> Option<Self::ColumnRows<'_>> {
            circuit::linear_map::binary::VirtualMap::column_rows(&self.0, column)
        }
    }

    fn sample(seed: u64) -> Gf {
        let hi = seed.wrapping_mul(0x9E37_79B9_7F4A_7C15).rotate_left(29) ^ 0x1234_5678_9ABC_DEF0;
        Gf::from_polynomial_words([seed ^ 0xA5A5_5A5A_0F0F_F0F0, hi])
    }

    /// Serializes the test that MUTATES the process-global `BITZ_QUAD` env
    /// var against quad-eligible (row_len ≥ 256) prove/verify pairs that
    /// must see a stable value across their whole run.
    use crate::utils::QUAD_ENV_LOCK;

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
            row_vars: 4,
            col_vars: 6,
            word_bits: 32,
        };
        let (pc, _) = lig_configs(
            packed_vars(&p),
            LigConfig::Adhoc {
                log_batch: 2,
                log_inv_rate: 2,
            },
        )
        .unwrap();
        let data: Vec<_> = (0..p.cells()).map(|i| i as u128).collect();
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

    /// Supplying canonical row weights densely, as validated chunk-major
    /// limbs, or through an on-demand generator must produce the same proof
    /// and Fiat–Shamir continuation.  The W=32 shape exercises two chunks
    /// under the fixed 100-bit test modulus.
    #[test]
    fn virtual_runtime_dense_and_weight_chunks_are_transcript_identical() {
        use {
            crate::{
                f2map::cell_count,
                pcs::{FQ_BITS, FQ_MOD, GeneratedModQWeightSource, fq_add, fq_mul},
            },
            circuit::linear_map::binary::PreparedVirtualMap,
        };

        let _env = QUAD_ENV_LOCK.lock().unwrap_or_else(|p| p.into_inner());
        let p = IntegerMatrixLayout {
            row_vars: 4,
            col_vars: 6,
            word_bits: 32,
        };
        let (pc, vc) = lig_configs(
            packed_vars(&p),
            LigConfig::Adhoc {
                log_batch: 2,
                log_inv_rate: 2,
            },
        )
        .unwrap();
        let data = (0..p.cells())
            .map(|cell| (cell as u128).wrapping_mul(0x9E37_79B9) & u32::MAX as u128)
            .collect::<Vec<_>>();
        let hint = commit_rs_ligerito(&p, &data, &pc);
        let cells = cell_count(&p);
        let map = GeneralPathMap(
            PreparedVirtualMap::from_implicit(
                CscMatrix::try_from_binary_csc(cells, (0..=cells).collect(), (0..cells).collect())
                    .unwrap(),
            )
            .unwrap(),
        );
        assert!(!circuit::linear_map::binary::VirtualMap::is_identity(&map));

        let row_weights = (0..p.rows())
            .map(|row| {
                (((row as u128 + 1) << 92) | ((row as u128 + 3) << 37) | row as u128) % FQ_MOD
            })
            .collect::<Vec<_>>();
        let chunks = ModQWeightChunks::from_dense(&p, &row_weights, FQ_BITS).unwrap();
        let generated =
            GeneratedModQWeightSource::new(&p, FQ_BITS, |row| row_weights.get(row).copied())
                .unwrap();
        assert_eq!(
            chunks.len(),
            2,
            "W=32 must exercise multi-chunk reconstruction"
        );
        let col_weights = (0..p.cols())
            .map(|column| column as u128 + 1)
            .collect::<Vec<_>>();
        let mut claimed = 0_u128;
        for column in 0..p.cols() {
            let mut column_value = 0_u128;
            for row in 0..p.rows() {
                column_value = fq_add(
                    column_value,
                    fq_mul(row_weights[row], data[p.cell_index(row, column)]),
                );
            }
            claimed = fq_add(claimed, fq_mul(col_weights[column], column_value));
        }

        let alpha = smallest_generator();
        let mut dense_transcript = Blake3Transcript::new();
        let dense_proof = prove_mle_eval_mod_q_ligerito_virtual_runtime(
            &mut dense_transcript,
            &hint,
            hint.rows(),
            &p,
            &p,
            &map,
            &row_weights,
            FQ_MOD,
            FQ_BITS,
            alpha,
            0,
            None,
            &pc,
        )
        .unwrap();
        let mut chunk_transcript = Blake3Transcript::new();
        let chunk_proof = prove_mle_eval_mod_q_ligerito_virtual_with_weight_chunks_runtime(
            &mut chunk_transcript,
            &hint,
            hint.rows(),
            &p,
            &p,
            &map,
            &chunks,
            FQ_MOD,
            FQ_BITS,
            alpha,
            0,
            None,
            &pc,
        )
        .unwrap();
        let mut generated_transcript = Blake3Transcript::new();
        let generated_proof = prove_mle_eval_mod_q_ligerito_virtual_with_weight_source_runtime(
            &mut generated_transcript,
            &hint,
            hint.rows(),
            &p,
            &p,
            &map,
            &generated,
            FQ_MOD,
            FQ_BITS,
            alpha,
            0,
            None,
            &pc,
        )
        .unwrap();
        assert_eq!(dense_proof.to_bytes(), chunk_proof.to_bytes());
        assert_eq!(dense_proof.to_bytes(), generated_proof.to_bytes());
        let dense_next = dense_transcript.get_challenge::<u128>();
        assert_eq!(dense_next, chunk_transcript.get_challenge::<u128>());
        assert_eq!(dense_next, generated_transcript.get_challenge::<u128>());

        let mut dense_verifier = Blake3Transcript::new();
        verify_mle_eval_mod_q_ligerito_virtual_runtime(
            &mut dense_verifier,
            &hint.commitment,
            &chunk_proof,
            &p,
            &p,
            &map,
            &row_weights,
            &col_weights,
            alpha,
            claimed,
            FQ_MOD,
            FQ_BITS,
            0,
            None,
            &vc,
        )
        .unwrap();
        let mut chunk_verifier = Blake3Transcript::new();
        verify_mle_eval_mod_q_ligerito_virtual_with_weight_chunks_runtime(
            &mut chunk_verifier,
            &hint.commitment,
            &dense_proof,
            &p,
            &p,
            &map,
            &chunks,
            &col_weights,
            alpha,
            claimed,
            FQ_MOD,
            FQ_BITS,
            0,
            None,
            &vc,
        )
        .unwrap();
        let mut generated_verifier = Blake3Transcript::new();
        verify_mle_eval_mod_q_ligerito_virtual_with_weight_source_runtime(
            &mut generated_verifier,
            &hint.commitment,
            &chunk_proof,
            &p,
            &p,
            &map,
            &generated,
            &col_weights,
            alpha,
            claimed,
            FQ_MOD,
            FQ_BITS,
            0,
            None,
            &vc,
        )
        .unwrap();
        let dense_next = dense_verifier.get_challenge::<u128>();
        assert_eq!(dense_next, chunk_verifier.get_challenge::<u128>());
        assert_eq!(dense_next, generated_verifier.get_challenge::<u128>());
    }

    /// The streaming row-factored batching passes equal the cell-wise
    /// definitions (weights `W = Σ_l η_l·Mᵀ eq(pt_l)` materialized, then
    /// scanned) on a small map with empty rows and cross-row duplicate
    /// sources — pinning the char-2 reassociations
    /// `bit_i(Σ_r E_r) = ⊕_r bit_i(E_r)` and `Φ_ρ(Σ_r E_r) = Σ_r Φ_ρ(E_r)`
    /// the prover and verifier both rely on.
    /// The factored tensor-repetition weight engine equals the streamed
    /// per-nonzero fold bit-for-bit — on both the hoisted (k ≥ 7) and
    /// small-instance (k < 7) paths — so engine selection cannot move a
    /// transcript. Also pins the pack-skip contract: `live == false`
    /// implies an all-zero pack.
    #[test]
    fn virtual_pack_weights_match_generic() {
        use circuit::linear_map::binary::{PreparedVirtualMap, RepeatedVirtualMap};

        let local_rows = 32usize;
        let local_cols = 16usize;
        // An empty local column, a dense one, and small pseudo-random ones.
        let columns: Vec<Vec<(usize, bool)>> = (0..local_cols)
            .map(|c| {
                if c == 3 {
                    return Vec::new();
                }
                if c == 5 {
                    return (0..local_rows).map(|r| (r, true)).collect();
                }
                let mut rows: Vec<usize> = (0..=(c % 4))
                    .map(|k| (c * 7 + k * 11 + 3) % local_rows)
                    .collect();
                rows.sort_unstable();
                rows.dedup();
                rows.into_iter().map(|r| (r, true)).collect()
            })
            .collect();
        let local =
            PreparedVirtualMap::new(CscMatrix::try_from_columns(local_rows, columns).unwrap())
                .unwrap();

        for instances in [16usize, 256] {
            let repeated = RepeatedVirtualMap::new(local.clone(), instances).unwrap();
            let vars =
                circuit::linear_map::binary::VirtualMap::rows(&repeated).trailing_zeros() as usize;
            let t_wh = vars / 2;
            let points: Vec<Vec<Gf>> = (0..2u64)
                .map(|l| {
                    (0..vars)
                        .map(|k| sample(0x7000 + l * 64 + k as u64))
                        .collect()
                })
                .collect();
            let etas = vec![sample(0xE1), sample(0xE2)];
            let structured = BinaryAdjoint::new(&repeated, &points, &etas, t_wh);
            assert!(
                matches!(structured, BinaryAdjoint::Repeated { .. }),
                "power-of-two repetition must take the factored path"
            );
            let generic = BinaryAdjoint::Generic {
                map: &repeated,
                coeffs: BinaryRowWeights::new(&points, &etas, t_wh, binary_equality),
            };
            let n_packs = circuit::linear_map::binary::VirtualMap::cols(&repeated) >> LOG_PACKING;
            assert!(n_packs >= 2);
            for pack in 0..n_packs {
                let mut fast = [Gf::zero(); 128];
                let mut slow = [Gf::zero(); 128];
                let live = structured.pack_weights(pack, &mut fast);
                let _ = generic.pack_weights(pack, &mut slow);
                assert_eq!(fast, slow, "pack {pack} (instances {instances})");
                if !live {
                    assert!(fast.iter().all(|w| *w == Gf::zero()));
                }
            }
        }
    }

    /// The SHA/product-layout source repetition has a different column order
    /// from `RepeatedVirtualMap`: one shared constant followed by contiguous
    /// nonconstant columns for each instance. Its factored engine must still
    /// match the generic CSC walk across instance boundaries and padding.
    #[test]
    fn virtual_packed_source_weights_match_generic() {
        use circuit::linear_map::binary::{PackedSourceRepeatedVirtualMap, PreparedVirtualMap};

        let local_rows = 29usize;
        let local_cols = 16usize; // 15-wide instance runs cross 128-cell packs.
        let columns: Vec<Vec<(usize, bool)>> = (0..local_cols)
            .map(|column| {
                if column == 3 {
                    return Vec::new();
                }
                if column == 5 {
                    return (0..local_rows).map(|row| (row, true)).collect();
                }
                let count = if column == 0 { 5 } else { 1 + column % 5 };
                let mut rows: Vec<usize> = (0..count)
                    .map(|index| (column * 11 + index * 7 + 2) % local_rows)
                    .collect();
                rows.sort_unstable();
                rows.dedup();
                rows.into_iter().map(|row| (row, true)).collect()
            })
            .collect();
        let local =
            PreparedVirtualMap::new(CscMatrix::try_from_columns(local_rows, columns).unwrap())
                .unwrap();

        use circuit::linear_map::binary::PackedSourceOrder;
        for (instances, order) in [
            (4usize, PackedSourceOrder::LocalMajor),
            (256, PackedSourceOrder::LocalMajor),
            (4, PackedSourceOrder::InstanceMajor),
            (256, PackedSourceOrder::InstanceMajor),
        ] {
            let rows = match order {
                PackedSourceOrder::LocalMajor => (local_rows * instances).next_power_of_two(),
                PackedSourceOrder::InstanceMajor => local_rows.next_power_of_two() * instances,
            };
            let live_cols = 1 + (local_cols - 1) * instances;
            let cols = live_cols.next_power_of_two().max(128);
            let map = PackedSourceRepeatedVirtualMap::new_with_order(
                local.clone(),
                instances,
                rows,
                cols,
                order,
            )
            .unwrap();
            let vars = rows.trailing_zeros() as usize;
            let k = instances.trailing_zeros() as usize;
            for t_wh in [k - 1, k + 1] {
                let points: Vec<Vec<Gf>> = (0..2u64)
                    .map(|claim| {
                        (0..vars)
                            .map(|bit| sample(0x7A00 + claim * 64 + bit as u64))
                            .collect()
                    })
                    .collect();
                let etas = vec![sample(0xEA), sample(0xEB)];
                let structured = BinaryAdjoint::new(&map, &points, &etas, t_wh);
                assert!(
                    matches!(structured, BinaryAdjoint::PackedSourceRepeated { .. }),
                    "packed source repetition must take its factored path ({order:?})"
                );
                let generic = BinaryAdjoint::Generic {
                    map: &map,
                    coeffs: BinaryRowWeights::new(&points, &etas, t_wh, binary_equality),
                };
                let n_packs = cols >> LOG_PACKING;
                for pack in 0..n_packs {
                    let mut fast = [Gf::zero(); 128];
                    let mut slow = [Gf::zero(); 128];
                    let live = structured.pack_weights(pack, &mut fast);
                    let slow_live = generic.pack_weights(pack, &mut slow);
                    assert_eq!(
                        fast, slow,
                        "pack {pack}, instances {instances}, t_wh {t_wh}, {order:?}"
                    );
                    assert_eq!(live, slow_live);
                    if !live {
                        assert!(fast.iter().all(|weight| *weight == Gf::zero()));
                    }
                }

                // Exercise both complete kernels on the mixed source layout.
                // The factored path must equal the generic sparse reference,
                // and changing only the padded message suffix must not affect h.
                let p_msg: Vec<Gf128> = (0..n_packs)
                    .map(|pack| sample(0xB000 + pack as u64))
                    .collect();
                let live_packs = live_cols.div_ceil(1usize << LOG_PACKING);
                let mut zero_padded_msg = p_msg.clone();
                zero_padded_msg[live_packs..].fill(Gf128::ZERO);
                let a_cols = crate::dual_basis::dual_basis_cols();
                let hs_fast = virtual_hs_fold(&map, &structured, &p_msg, &a_cols);
                assert_eq!(
                    hs_fast,
                    virtual_hs_fold(&map, &structured, &zero_padded_msg, &a_cols),
                    "h must ignore the structural padding suffix"
                );
                assert_eq!(
                    hs_fast,
                    virtual_hs_fold(&map, &generic, &p_msg, &a_cols),
                    "factored and generic h kernels"
                );

                let rho: Vec<Gf> = (0..128).map(|bit| sample(0xC000 + bit as u64)).collect();
                let a_fast = virtual_a_prime(&map, &structured, &rho, &a_cols, n_packs);
                let a_f128: Vec<Gf> = virtual_a_prime(&map, &structured, &rho, &a_cols, n_packs)
                    .into_iter()
                    .collect();
                assert_eq!(a_f128, a_fast, "direct Gf128 output");
                assert_eq!(
                    a_fast,
                    virtual_a_prime(&map, &generic, &rho, &a_cols, n_packs),
                    "factored and generic a-prime kernels"
                );
                assert!(
                    a_fast[live_packs..]
                        .iter()
                        .all(|value| *value == Gf::zero())
                );
            }
        }
    }

    /// The packed-source plane engine reproduces the per-cell `h` and `a′`
    /// kernels bit-for-bit: narrow (many instances per pack) and wide
    /// (instances straddling packs at drifting phases, odd and even
    /// widths) local layouts, one and two chunks, padded suffixes.
    #[test]
    fn virtual_planes_match_cellwise() {
        use circuit::linear_map::binary::{PackedSourceRepeatedVirtualMap, PreparedVirtualMap};

        use crate::virt_batch::PackedSourcePlanes;

        for (local_rows, local_width, instances) in [
            (29usize, 15usize, 256usize),
            (29, 15, 4),
            (61, 200, 16),
            (61, 201, 64),
            (97, 1000, 8),
            (97, 1024, 8),
            (37, 300, 2),
        ] {
            let local_cols = local_width + 1;
            let columns: Vec<Vec<(usize, bool)>> = (0..local_cols)
                .map(|column| {
                    if column % 97 == 3 {
                        return Vec::new();
                    }
                    let count = if column == 0 { 5 } else { 1 + column % 5 };
                    let mut rows: Vec<usize> = (0..count)
                        .map(|index| (column * 11 + index * 7 + 2) % local_rows)
                        .collect();
                    rows.sort_unstable();
                    rows.dedup();
                    rows.into_iter().map(|row| (row, true)).collect()
                })
                .collect();
            let local =
                PreparedVirtualMap::new(CscMatrix::try_from_columns(local_rows, columns).unwrap())
                    .unwrap();
            let rows = (local_rows * instances).next_power_of_two();
            let live_cols = 1 + local_width * instances;
            let cols = live_cols.next_power_of_two().max(128);
            let map =
                PackedSourceRepeatedVirtualMap::new(local.clone(), instances, rows, cols).unwrap();
            let vars = rows.trailing_zeros() as usize;
            let k = instances.trailing_zeros() as usize;
            let t_wh = k + 1;
            let n_packs = cols >> LOG_PACKING;
            for chunks in [1usize, 2] {
                let points: Vec<Vec<Gf>> = (0..chunks as u64)
                    .map(|claim| {
                        (0..vars)
                            .map(|bit| {
                                sample(0x5A00 + claim * 64 + bit as u64 + local_width as u64)
                            })
                            .collect()
                    })
                    .collect();
                let etas: Vec<Gf> = (0..chunks as u64).map(|c| sample(0xE0 + c)).collect();
                let structured = BinaryAdjoint::new(&map, &points, &etas, t_wh);
                let BinaryAdjoint::PackedSourceRepeated {
                    eq_inst_gf,
                    s,
                    constant_weight,
                    ..
                } = &structured
                else {
                    panic!("packed source repetition must take its factored path");
                };
                let planes = PackedSourcePlanes::new(
                    local_width,
                    instances,
                    eq_inst_gf,
                    s,
                    *constant_weight,
                );
                let generic = BinaryAdjoint::Generic {
                    map: &map,
                    coeffs: BinaryRowWeights::new(&points, &etas, t_wh, binary_equality),
                };
                let p_msg: Vec<Gf128> = (0..n_packs)
                    .map(|pack| sample(0xB100 + pack as u64))
                    .collect();
                let a_cols = crate::dual_basis::dual_basis_cols();
                assert_eq!(
                    planes.hs_fold(&p_msg),
                    virtual_hs_fold(&map, &generic, &p_msg, &a_cols),
                    "h: width {local_width}, instances {instances}, chunks {chunks}"
                );
                let rho: Vec<Gf> = (0..128).map(|bit| sample(0xC100 + bit as u64)).collect();
                let (a_prime, (u0, u2)) = planes.a_prime(&rho, &p_msg);
                assert_eq!(
                    a_prime,
                    virtual_a_prime(&map, &generic, &rho, &a_cols, n_packs),
                    "a′: width {local_width}, instances {instances}, chunks {chunks}"
                );
                // The fused round-0 pair equals the direct pairwise sums.
                let mut expect_u0 = Gf::zero();
                let mut expect_u2 = Gf::zero();
                for j in (0..n_packs.saturating_sub(1)).step_by(2) {
                    let f0 = p_msg[j];
                    let f1 = p_msg[j + 1];
                    expect_u0 += f0 * a_prime[j];
                    expect_u2 += (f0 + f1) * (a_prime[j] + a_prime[j + 1]);
                }
                assert_eq!((u0, u2), (expect_u0, expect_u2), "round-0 pair");
            }
        }
    }

    /// A chained packed-source repetition — the plain repetition plus the
    /// rotated chain link and the one-instance boundary maps — takes the
    /// factored path, and its weights, `h` and `a′` (plane engine plus the
    /// extra terms, and the per-pack kernels) all equal the generic CSC walk.
    #[test]
    fn virtual_chained_weights_match_generic() {
        use circuit::linear_map::binary::{ChainedPackedSourceMap, PreparedVirtualMap};

        use crate::virt_batch::PackedSourcePlanes;

        let local_rows = 40usize;
        let width = 600usize; // ≥ 512: the plane engine is eligible.
        let local_cols = width + 1;
        let prepared = |columns: Vec<Vec<(usize, bool)>>| {
            PreparedVirtualMap::new(CscMatrix::try_from_columns(local_rows, columns).unwrap())
                .unwrap()
        };
        let rows_of = |seed: usize, count: usize, lo: usize, hi: usize| -> Vec<(usize, bool)> {
            let mut rows: Vec<usize> = (0..count)
                .map(|index| lo + (seed * 11 + index * 7 + 2) % (hi - lo))
                .collect();
            rows.sort_unstable();
            rows.dedup();
            rows.into_iter().map(|row| (row, true)).collect()
        };
        // `local`: the constant row from column 0, rows < 30 elsewhere.
        let local = prepared(
            (0..local_cols)
                .map(|column| {
                    if column == 0 {
                        vec![(0, true)]
                    } else if column % 97 == 3 {
                        Vec::new()
                    } else {
                        rows_of(column, 1 + column % 4, 1, 30)
                    }
                })
                .collect(),
        );
        // `prev`: a band of columns [200, 264), any nonconstant rows.
        let prev = prepared(
            (0..local_cols)
                .map(|column| {
                    if (200..264).contains(&column) {
                        rows_of(column + 5, 1 + column % 3, 1, 40)
                    } else {
                        Vec::new()
                    }
                })
                .collect(),
        );
        // `first`: the constant column only, rows `local` never uses.
        let first = prepared(
            (0..local_cols)
                .map(|column| {
                    if column == 0 {
                        rows_of(9, 6, 30, 40)
                    } else {
                        Vec::new()
                    }
                })
                .collect(),
        );
        // `last`: a tail band [560, 600) plus the constant, rows ≥ 30.
        let last = prepared(
            (0..local_cols)
                .map(|column| {
                    if column == 0 {
                        vec![(35, true)]
                    } else if column >= 561 {
                        rows_of(column + 1, 1 + column % 2, 30, 40)
                    } else {
                        Vec::new()
                    }
                })
                .collect(),
        );
        for instances in [2usize, 8] {
            let rows = (local_rows * instances).next_power_of_two();
            let live_cols = 1 + width * instances;
            let cols = live_cols.next_power_of_two().max(128);
            let map = ChainedPackedSourceMap::new(
                local.clone(),
                prev.clone(),
                first.clone(),
                last.clone(),
                instances,
                rows,
                cols,
            )
            .unwrap();
            let vars = rows.trailing_zeros() as usize;
            let k = instances.trailing_zeros() as usize;
            let t_wh = k + 1;
            let n_packs = cols >> LOG_PACKING;
            for chunks in [1usize, 2] {
                let points: Vec<Vec<Gf>> = (0..chunks as u64)
                    .map(|claim| {
                        (0..vars)
                            .map(|bit| sample(0x3C00 + claim * 64 + bit as u64 + instances as u64))
                            .collect()
                    })
                    .collect();
                let etas: Vec<Gf> = (0..chunks as u64).map(|c| sample(0xE8 + c)).collect();
                let structured = BinaryAdjoint::new(&map, &points, &etas, t_wh);
                let BinaryAdjoint::PackedSourceRepeated {
                    eq_inst_gf,
                    s,
                    constant_weight,
                    extra,
                    ..
                } = &structured
                else {
                    panic!("chained repetition must take the factored path");
                };
                // `first` reads only the constant column: its weight lives
                // in `constant_weight`, so only `prev` and `last` remain.
                assert_eq!(extra.len(), 2, "prev and last");
                let generic = BinaryAdjoint::Generic {
                    map: &map,
                    coeffs: BinaryRowWeights::new(&points, &etas, t_wh, binary_equality),
                };
                for pack in 0..n_packs {
                    let mut fast = [Gf::zero(); 128];
                    let mut slow = [Gf::zero(); 128];
                    let live = structured.pack_weights(pack, &mut fast);
                    let slow_live = generic.pack_weights(pack, &mut slow);
                    assert_eq!(
                        fast, slow,
                        "pack {pack}, instances {instances}, chunks {chunks}"
                    );
                    assert_eq!(live, slow_live);
                }
                let p_msg: Vec<Gf128> = (0..n_packs)
                    .map(|pack| sample(0xB300 + pack as u64))
                    .collect();
                let a_cols = crate::dual_basis::dual_basis_cols();
                let planes =
                    PackedSourcePlanes::new(width, instances, eq_inst_gf, s, *constant_weight);
                let expect_hs = virtual_hs_fold(&map, &generic, &p_msg, &a_cols);
                assert_eq!(
                    virtual_hs_fold(&map, &structured, &p_msg, &a_cols),
                    expect_hs,
                    "per-pack h kernel (instances {instances}, chunks {chunks})"
                );
                let mut hs = planes.hs_fold(&p_msg);
                structured.add_extra_hs(&mut hs, &p_msg, &a_cols);
                assert_eq!(
                    hs, expect_hs,
                    "planes + extra h (instances {instances}, chunks {chunks})"
                );

                let rho: Vec<Gf> = (0..128).map(|bit| sample(0xC300 + bit as u64)).collect();
                let expect_a = virtual_a_prime(&map, &generic, &rho, &a_cols, n_packs);
                assert_eq!(
                    virtual_a_prime(&map, &structured, &rho, &a_cols, n_packs),
                    expect_a,
                    "per-pack a′ kernel (instances {instances}, chunks {chunks})"
                );
                let (mut a_prime, mut round0) = planes.a_prime(&rho, &p_msg);
                structured.add_extra_a_prime(&mut a_prime, &mut round0, &rho, &p_msg);
                assert_eq!(
                    a_prime, expect_a,
                    "planes + extra a′ (instances {instances}, chunks {chunks})"
                );
                let mut expect_u0 = Gf::zero();
                let mut expect_u2 = Gf::zero();
                for j in (0..n_packs.saturating_sub(1)).step_by(2) {
                    let f0 = p_msg[j];
                    let f1 = p_msg[j + 1];
                    expect_u0 += f0 * expect_a[j];
                    expect_u2 += (f0 + f1) * (expect_a[j] + expect_a[j + 1]);
                }
                assert_eq!(
                    round0,
                    (expect_u0, expect_u2),
                    "round-0 pair after the extra terms"
                );
            }
        }
    }

    #[test]
    fn virtual_hs_and_a_prime_match_cellwise() {
        use {
            crate::f2map::{cell_count, cell_row_bits},
            circuit::linear_map::binary::PreparedVirtualMap,
        };

        let f_layout = IntegerMatrixLayout {
            row_vars: 8,
            col_vars: 2,
            word_bits: 1,
        }; // 2^10 cells, 8 packs
        let h_layout = IntegerMatrixLayout {
            row_vars: 7,
            col_vars: 3,
            word_bits: 1,
        }; // 2^10 derived cells
        let n_f = cell_count(&f_layout);
        let n_h = cell_count(&h_layout);
        let t_wh = cell_row_bits(&h_layout);
        let lists: Vec<Vec<usize>> = (0..n_h)
            .map(|i| {
                if i % 5 == 4 {
                    return Vec::new(); // empty rows
                }
                // Deliberate cross-row duplicates: nearby rows share cells.
                let a = (i * 7 + 3) % n_f;
                let b = (i / 2 * 13 + 11) % n_f;
                let mut l = vec![a.min(b), a.max(b)];
                l.dedup();
                l
            })
            .collect();
        let matrix = CscMatrix::try_from_rows(
            n_f,
            lists
                .into_iter()
                .map(|row| row.into_iter().map(|column| (column, true)).collect())
                .collect(),
        )
        .unwrap();
        let map = PreparedVirtualMap::new(matrix).unwrap();

        let points: Vec<Vec<Gf>> = (0..2)
            .map(|l| {
                (0..t_wh + h_layout.col_vars)
                    .map(|k| sample(0x9000 + (l * 64 + k) as u64))
                    .collect()
            })
            .collect();
        let etas = vec![sample(0xA1), sample(0xA2)];
        let coeffs = BinaryRowWeights::new(&points, &etas, t_wh, binary_equality);
        let weights = BinaryAdjoint::new(&map, &points, &etas, t_wh);
        let a_cols = crate::dual_basis::dual_basis_cols();
        let n_packs = n_f >> LOG_PACKING;
        let p_msg: Vec<Gf128> = (0..n_packs).map(|y| sample(0xB000 + y as u64)).collect();

        // Materialized weights (the old `mqv:wcoef` + `mqv:wtbl`).
        let mut w_tbl = vec![Gf::zero(); n_f];
        for (j, column) in map.matrix().columns().enumerate() {
            for &row in column.indices() {
                w_tbl[j] += coeffs.coeff(row);
            }
        }

        // h_i: streaming vs cell-wise plane scan.
        let hs = virtual_hs_fold(&map, &weights, &p_msg, &a_cols);
        let mut expect = vec![Gf::zero(); 128];
        for (j, wj) in w_tbl.iter().enumerate() {
            let w = wj.as_words();
            let g = (p_msg[j >> LOG_PACKING]) * a_cols[j & 127];
            for (i, e) in expect.iter_mut().enumerate() {
                if (w[i >> 6] >> (i & 63)) & 1 == 1 {
                    *e += g;
                }
            }
        }
        assert_eq!(hs.as_slice(), expect, "h_i fold");

        // a′: the shared prover/verifier build equals the cell-wise Φ_ρ scan.
        let rho: Vec<Gf> = (0..128).map(|i| sample(0xC000 + i as u64)).collect();
        let a = virtual_a_prime(&map, &weights, &rho, &a_cols, n_packs);
        let mut expect_a = vec![Gf::zero(); n_packs];
        for (j, wj) in w_tbl.iter().enumerate() {
            let w = wj.as_words();
            let mut phi = Gf::zero();
            for (i, r) in rho.iter().enumerate() {
                if (w[i >> 6] >> (i & 63)) & 1 == 1 {
                    phi += *r;
                }
            }
            expect_a[j >> LOG_PACKING] += phi * a_cols[j & 127];
        }
        assert_eq!(a, expect_a, "a' build");
    }

    /// Mod-q MLE evaluation through the Ligerito opener: 1-chunk (W=1) and
    /// 2-chunk (W=32) regimes, with a local 𝔽_q (q = 2^100 − 15).
    #[test]
    fn mle_eval_mod_q_ligerito_roundtrips() {
        // This test changes process-wide protocol dispatch settings. A mutex
        // cannot protect other tests that simply read those settings.
        const CHILD: &str = "BITZ_QUAD_TEST_CHILD";
        if std::env::var_os(CHILD).is_none() {
            let status = std::process::Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    "ligerito_flock::tests::mle_eval_mod_q_ligerito_roundtrips",
                ])
                .env(CHILD, "1")
                // Quad is an L4-only experiment; auto may choose a binary schedule.
                .env("F2_FOREST_SCHEDULE", "l4")
                .status()
                .unwrap();
            assert!(status.success(), "isolated quad test failed");
            return;
        }
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
            let p = IntegerMatrixLayout {
                row_vars: t,
                col_vars: s_vars,
                word_bits: w,
            };
            let m_p = packed_vars(&p);
            let lch = mod_q_num_chunks(&p, q_bits);
            let c_w = mod_q_chunk_width(&p);
            let (pc, vc) = lig_configs(
                m_p,
                LigConfig::Adhoc {
                    log_batch: 2,
                    log_inv_rate: 2,
                },
            )
            .expect("cfg");

            let mask = if w == 128 {
                u128::MAX
            } else {
                (1u128 << w) - 1
            };
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
            let cw_small: Vec<u128> = (0..p.cols())
                .map(|c| ((c as u128).wrapping_mul(5) & 7).wrapping_add(1))
                .collect();
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
                &mut vt,
                &hint.commitment,
                &proof,
                &p,
                &rw_q,
                &col_w,
                alpha,
                y,
                q_bits,
                &vc,
            )
            .unwrap_or_else(|e| panic!("mod-q (t={t},W={w},L={lch}) failed: {e:?}"));

            // Wrong claim.
            let mut vt = CountingTranscript::new();
            assert_eq!(
                verify_mle_eval_mod_q_ligerito(
                    &mut vt,
                    &hint.commitment,
                    &proof,
                    &p,
                    &rw_q,
                    &col_w,
                    alpha,
                    y + Fq::from(1u128),
                    q_bits,
                    &vc,
                ),
                Err(FlockRsError::Common(IntEvalRsError::ReadOff)),
            );
            assert_eq!(
                vt.challenges, 0,
                "an invalid read-off must reject before any forest challenge"
            );

            // Out-of-range chunk fold.
            let mut bad = IntEvalRsLigModQProof {
                mfs: proof.mfs.clone(),
                us: proof.us.clone(),
                presums: proof.presums.clone(),
                rings: proof.rings.clone(),
                lig: proof.lig.clone(),
                grinding_nonces: proof.grinding_nonces.clone(),
                ood: proof.ood,
            };
            bad.us[0][0] = u128::MAX - 1;
            let mut vt = Blake3Transcript::new();
            assert!(matches!(
                verify_mle_eval_mod_q_ligerito(
                    &mut vt,
                    &hint.commitment,
                    &bad,
                    &p,
                    &rw_q,
                    &col_w,
                    alpha,
                    y,
                    q_bits,
                    &vc,
                ),
                Err(FlockRsError::ChunkRange { .. })
            ));

            // Non-generator α is rejected before any proof processing.
            let mut vt = Blake3Transcript::new();
            assert_eq!(
                verify_mle_eval_mod_q_ligerito(
                    &mut vt,
                    &hint.commitment,
                    &proof,
                    &p,
                    &rw_q,
                    &col_w,
                    Gf::one(),
                    y,
                    q_bits,
                    &vc,
                ),
                Err(FlockRsError::Common(IntEvalRsError::ChallengeNotGenerator)),
            );

            // QUAD forest (`BITZ_QUAD=1` — arity-4 region layers, K
            // challenges, its own transcript shape; both test shapes have
            // row_len ≥ 256, and (4, 8, 32) exercises the odd-depth
            // parity bridge): roundtrip, wrong-claim rejection, and the
            // codec round-trips the quad layers (pair2 flag).
            unsafe { std::env::set_var("BITZ_QUAD", "1") };
            let mut pt = Blake3Transcript::new();
            let proof_q =
                prove_mle_eval_mod_q_ligerito(&mut pt, &hint, &p, &rw_q, q_bits, alpha, &pc);
            let mut vt = Blake3Transcript::new();
            verify_mle_eval_mod_q_ligerito(
                &mut vt,
                &hint.commitment,
                &proof_q,
                &p,
                &rw_q,
                &col_w,
                alpha,
                y,
                q_bits,
                &vc,
            )
            .unwrap_or_else(|e| panic!("quad mod-q (t={t},W={w}) failed: {e:?}"));
            let mut vt = Blake3Transcript::new();
            assert_eq!(
                verify_mle_eval_mod_q_ligerito(
                    &mut vt,
                    &hint.commitment,
                    &proof_q,
                    &p,
                    &rw_q,
                    &col_w,
                    alpha,
                    y + Fq::from(1u128),
                    q_bits,
                    &vc,
                ),
                Err(FlockRsError::Common(IntEvalRsError::ReadOff)),
                "quad wrong claim must be rejected (t={t},W={w})"
            );
            let rt = IntEvalRsLigModQProof::from_bytes(&proof_q.to_bytes())
                .expect("quad proof codec roundtrip");
            let mut vt = Blake3Transcript::new();
            verify_mle_eval_mod_q_ligerito(
                &mut vt,
                &hint.commitment,
                &rt,
                &p,
                &rw_q,
                &col_w,
                alpha,
                y,
                q_bits,
                &vc,
            )
            .expect("decoded quad proof verifies");
            // The restructured degree-5 bodies (w-prefold + Karatsuba-3
            // cross stage + folded node conversion) are value-exact
            // re-associations: the proof stream must be byte-identical
            // to the naive bodies'.
            unsafe { std::env::set_var("BITZ_QUAD_KERNEL", "0") };
            let mut pt = Blake3Transcript::new();
            let proof_q_naive =
                prove_mle_eval_mod_q_ligerito(&mut pt, &hint, &p, &rw_q, q_bits, alpha, &pc);
            unsafe { std::env::remove_var("BITZ_QUAD_KERNEL") };
            assert_eq!(
                proof_q.to_bytes(),
                proof_q_naive.to_bytes(),
                "quad kernel bodies must be transcript-identical (t={t},W={w})"
            );
            // BOTTOM MERGE (`BITZ_QUAD=2` — the pair and leaf layers as
            // ONE arity-4 bit-driven layer, `prove_quad_bottom_sumcheck`):
            // roundtrip, wrong-claim rejection, codec, and the v1/v2
            // plans are mutually incompatible (layer counts differ).
            unsafe { std::env::set_var("BITZ_QUAD", "2") };
            let mut pt = Blake3Transcript::new();
            let proof_q2 =
                prove_mle_eval_mod_q_ligerito(&mut pt, &hint, &p, &rw_q, q_bits, alpha, &pc);
            let mut vt = Blake3Transcript::new();
            verify_mle_eval_mod_q_ligerito(
                &mut vt,
                &hint.commitment,
                &proof_q2,
                &p,
                &rw_q,
                &col_w,
                alpha,
                y,
                q_bits,
                &vc,
            )
            .unwrap_or_else(|e| panic!("quad-v2 mod-q (t={t},W={w}) failed: {e:?}"));
            let mut vt = Blake3Transcript::new();
            assert_eq!(
                verify_mle_eval_mod_q_ligerito(
                    &mut vt,
                    &hint.commitment,
                    &proof_q2,
                    &p,
                    &rw_q,
                    &col_w,
                    alpha,
                    y + Fq::from(1u128),
                    q_bits,
                    &vc,
                ),
                Err(FlockRsError::Common(IntEvalRsError::ReadOff)),
                "quad-v2 wrong claim must be rejected (t={t},W={w})"
            );
            let rt2 = IntEvalRsLigModQProof::from_bytes(&proof_q2.to_bytes())
                .expect("quad-v2 proof codec roundtrip");
            let mut vt = Blake3Transcript::new();
            verify_mle_eval_mod_q_ligerito(
                &mut vt,
                &hint.commitment,
                &rt2,
                &p,
                &rw_q,
                &col_w,
                alpha,
                y,
                q_bits,
                &vc,
            )
            .expect("decoded quad-v2 proof verifies");
            // A v1 proof must not pass under the v2 plan.
            let mut vt = Blake3Transcript::new();
            assert!(
                verify_mle_eval_mod_q_ligerito(
                    &mut vt,
                    &hint.commitment,
                    &proof_q,
                    &p,
                    &rw_q,
                    &col_w,
                    alpha,
                    y,
                    q_bits,
                    &vc,
                )
                .is_err(),
                "a v1 quad proof must be rejected under the v2 plan (t={t},W={w})"
            );
            unsafe { std::env::remove_var("BITZ_QUAD") };
            // A quad proof must NOT pass the arity-2 dispatch (different
            // transcript shape — the quad layers' pair2 rejects).
            let mut vt = Blake3Transcript::new();
            assert!(
                verify_mle_eval_mod_q_ligerito(
                    &mut vt,
                    &hint.commitment,
                    &proof_q,
                    &p,
                    &rw_q,
                    &col_w,
                    alpha,
                    y,
                    q_bits,
                    &vc,
                )
                .is_err(),
                "quad proof must be rejected by the arity-2 verifier (t={t},W={w})"
            );
        }
    }

    #[test]
    fn chunked_weight_openings_bind_public_inputs_and_domains() {
        use crate::pcs::{FQ_BITS, FQ_MOD, Q100Element};

        let _env = QUAD_ENV_LOCK.lock().unwrap_or_else(|p| p.into_inner());
        let p = IntegerMatrixLayout {
            row_vars: 7,
            col_vars: 5,
            word_bits: 8,
        };
        let (pc, vc) = lig_configs(
            packed_vars(&p),
            LigConfig::Adhoc {
                log_batch: 2,
                log_inv_rate: 2,
            },
        )
        .expect("ad-hoc config");
        let data = (0..p.cells())
            .map(|cell| ((cell as u128).wrapping_mul(17).wrapping_add(3)) & 0xff)
            .collect::<Vec<_>>();
        let row_weights = (0..p.rows()).map(|row| row as u128 + 1).collect::<Vec<_>>();
        let col_weights = (0..p.cols())
            .map(|column| Q100Element::from(column as u128 + 1))
            .collect::<Vec<_>>();
        let mut claimed = Q100Element::from(0_u128);
        for column in 0..p.cols() {
            let mut folded = Q100Element::from(0_u128);
            for row in 0..p.rows() {
                folded = folded
                    + Q100Element::from(row_weights[row])
                        * Q100Element::from(data[p.cell_index(row, column)]);
            }
            claimed = claimed + col_weights[column] * folded;
        }
        let col_weights_q = col_weights
            .iter()
            .map(|weight| weight.canonical_u128())
            .collect::<Vec<_>>();

        let hint = commit_rs_ligerito(&p, &data, &pc);
        let chunks = ModQWeightChunks::from_dense(&p, &row_weights, FQ_BITS).unwrap();
        let bridge_digest = [0x6du8; 32];
        let alpha = smallest_generator();
        let mut prover_transcript = Blake3Transcript::new();
        let proof = prove_mle_eval_mod_q_ligerito_with_weight_chunks(
            &mut prover_transcript,
            ModQOpeningKind::U32Mul,
            &hint,
            &p,
            &chunks,
            &bridge_digest,
            FQ_BITS,
            alpha,
            0,
            None,
            &pc,
        )
        .unwrap();

        let verify = |chunks: &ModQWeightChunks, cols: &[u128], digest: &[u8; 32], value: u128| {
            let mut transcript = Blake3Transcript::new();
            verify_mle_eval_mod_q_ligerito_with_weight_chunks_runtime(
                &mut transcript,
                ModQOpeningKind::U32Mul,
                &hint.commitment,
                &proof,
                &p,
                chunks,
                cols,
                digest,
                alpha,
                value,
                FQ_MOD,
                FQ_BITS,
                0,
                None,
                &vc,
            )
        };
        verify(
            &chunks,
            &col_weights_q,
            &bridge_digest,
            claimed.canonical_u128(),
        )
        .unwrap();

        let composite_q = (1_u128 << (FQ_BITS - 1)) + 1;
        let mut invalid_modulus_transcript = Blake3Transcript::new();
        let mut untouched = invalid_modulus_transcript.clone();
        assert_eq!(
            verify_mle_eval_mod_q_ligerito_with_weight_chunks_runtime(
                &mut invalid_modulus_transcript,
                ModQOpeningKind::U32Mul,
                &hint.commitment,
                &proof,
                &p,
                &chunks,
                &col_weights_q,
                &bridge_digest,
                alpha,
                claimed.canonical_u128(),
                composite_q,
                FQ_BITS,
                0,
                None,
                &vc,
            ),
            Err(FlockRsError::RingSwitch(RsOpenError::Shape))
        );
        assert_eq!(
            invalid_modulus_transcript.get_challenge::<u128>(),
            untouched.get_challenge::<u128>(),
            "invalid modulus must reject before statement absorption"
        );

        let mut wrong_digest = bridge_digest;
        wrong_digest[0] ^= 1;
        assert!(
            verify(
                &chunks,
                &col_weights_q,
                &wrong_digest,
                claimed.canonical_u128()
            )
            .is_err()
        );

        let mut wrong_rows = row_weights.clone();
        wrong_rows[0] += 1;
        let wrong_chunks = ModQWeightChunks::from_dense(&p, &wrong_rows, FQ_BITS).unwrap();
        assert!(
            verify(
                &wrong_chunks,
                &col_weights_q,
                &bridge_digest,
                claimed.canonical_u128()
            )
            .is_err()
        );

        let mut wrong_cols = col_weights_q.clone();
        wrong_cols[0] += 1;
        assert!(
            verify(
                &chunks,
                &wrong_cols,
                &bridge_digest,
                claimed.canonical_u128()
            )
            .is_err()
        );
        assert!(
            verify(
                &chunks,
                &col_weights_q,
                &bridge_digest,
                (claimed.canonical_u128() + 1) % FQ_MOD,
            )
            .is_err()
        );

        // Exercise the Baby Bear statement domain in ordinary CI and prove
        // that it cannot accept a u32 proof (or vice versa), even when every
        // other public input is identical.
        let mut baby_bear_prover_transcript = Blake3Transcript::new();
        let baby_bear_proof = prove_mle_eval_mod_q_ligerito_with_weight_chunks(
            &mut baby_bear_prover_transcript,
            ModQOpeningKind::BabyBearMul,
            &hint,
            &p,
            &chunks,
            &bridge_digest,
            FQ_BITS,
            alpha,
            0,
            None,
            &pc,
        )
        .unwrap();
        let mut baby_bear_verifier_transcript = Blake3Transcript::new();
        verify_mle_eval_mod_q_ligerito_with_weight_chunks(
            &mut baby_bear_verifier_transcript,
            ModQOpeningKind::BabyBearMul,
            &hint.commitment,
            &baby_bear_proof,
            &p,
            &chunks,
            &col_weights,
            &bridge_digest,
            alpha,
            claimed,
            FQ_BITS,
            0,
            None,
            &vc,
        )
        .unwrap();

        let mut wrong_u32_domain = Blake3Transcript::new();
        assert!(
            verify_mle_eval_mod_q_ligerito_with_weight_chunks_runtime(
                &mut wrong_u32_domain,
                ModQOpeningKind::U32Mul,
                &hint.commitment,
                &baby_bear_proof,
                &p,
                &chunks,
                &col_weights_q,
                &bridge_digest,
                alpha,
                claimed.canonical_u128(),
                FQ_MOD,
                FQ_BITS,
                0,
                None,
                &vc,
            )
            .is_err()
        );
        let mut wrong_baby_bear_domain = Blake3Transcript::new();
        assert!(
            verify_mle_eval_mod_q_ligerito_with_weight_chunks(
                &mut wrong_baby_bear_domain,
                ModQOpeningKind::BabyBearMul,
                &hint.commitment,
                &proof,
                &p,
                &chunks,
                &col_weights,
                &bridge_digest,
                alpha,
                claimed,
                FQ_BITS,
                0,
                None,
                &vc,
            )
            .is_err()
        );
    }

    /// The complete BitZ proof object round-trips through the host byte stream
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
        let p = IntegerMatrixLayout {
            row_vars: 10,
            col_vars: 5,
            word_bits: 1,
        };
        let m_p = packed_vars(&p);
        let (pc, vc) = lig_configs(
            m_p,
            LigConfig::Adhoc {
                log_batch: 2,
                log_inv_rate: 2,
            },
        )
        .expect("cfg");
        let data: Vec<u128> = (0..p.cells())
            .map(|i| (i as u128).wrapping_mul(0x9E37_79B9_7F4A_7C15) & 1)
            .collect();
        let rw_q: Vec<u128> = (0..p.rows())
            .map(|b| {
                (b as u128)
                    .wrapping_mul(0xDEAD_BEEF_CAFE_F00D_1234_5678_9ABC_DEF1)
                    .wrapping_add(7)
                    % Q
            })
            .collect();
        let cw_small: Vec<u128> = (0..p.cols())
            .map(|c| ((c as u128).wrapping_mul(5) & 7).wrapping_add(1))
            .collect();
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
            &mut vt,
            &hint.commitment,
            &proof2,
            &p,
            &rw_q,
            &col_w,
            alpha,
            y,
            q_bits,
            &vc,
        )
        .expect("deserialized proof verifies");
        // Canonical: re-serialization is byte-identical.
        assert_eq!(bytes, proof2.to_bytes(), "codec is canonical");

        // Only the low two forest-layer flag bits are defined. Accepting a
        // high bit would give the same proof object multiple byte encodings.
        let mut high_flag = bytes.clone();
        let flag_offset = 8 + 2 * core::mem::size_of::<u64>();
        high_flag[flag_offset] |= 0x04;
        assert!(matches!(
            IntEvalRsLigModQProof::from_bytes(&high_flag),
            Err(crate::proof_codec::CodecError::NonCanonical)
        ));

        // Tamper: flipping any single byte is rejected.
        for &pos in &[0usize, bytes.len() / 3, bytes.len() / 2, bytes.len() - 1] {
            let mut bad = bytes.clone();
            bad[pos] ^= 0x01;
            let rejected = match IntEvalRsLigModQProof::from_bytes(&bad) {
                Err(_) => true,
                Ok(bad_proof) => {
                    let mut vt = Blake3Transcript::new();
                    verify_mle_eval_mod_q_ligerito(
                        &mut vt,
                        &hint.commitment,
                        &bad_proof,
                        &p,
                        &rw_q,
                        &col_w,
                        alpha,
                        y,
                        q_bits,
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
        use crate::pcs::{FQ_BITS, Q100Element};
        let alpha = smallest_generator();
        let p = IntegerMatrixLayout {
            row_vars: 10,
            col_vars: 5,
            word_bits: 1,
        };
        let m_p = packed_vars(&p);
        let (pc, vc) = lig_configs(
            m_p,
            LigConfig::Adhoc {
                log_batch: 2,
                log_inv_rate: 2,
            },
        )
        .expect("cfg");
        // φ ≈ 0.55: columns `live..32` are the zero padding of a witness
        // of N = live·2^t cells.
        let live = 18usize;
        assert!(live < p.cols());
        let data: Vec<u128> = (0..p.cells())
            .map(|i| {
                let c = i & (p.cols() - 1);
                if c >= live {
                    0
                } else {
                    (i as u128).wrapping_mul(0x9E37_79B9_7F4A_7C15) & 1
                }
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
        let col_w: Vec<Q100Element> = (0..p.cols())
            .map(|c| Q100Element::from(((c as u128).wrapping_mul(5) & 7) + 1))
            .collect();
        let mut y = Q100Element::from(0u128);
        for c in 0..p.cols() {
            let mut acc = Q100Element::from(0u128);
            for b in 0..p.rows() {
                acc =
                    acc + Q100Element::from(rw_q[b]) * Q100Element::from(data[p.cell_index(b, c)]);
            }
            y = y + col_w[c] * acc;
        }
        let hint = commit_rs_flock_with(&p, &data, pc.log_inv_rates[0], pc.initial_k);
        let mut pt = Blake3Transcript::new();
        let proof = prove_mle_eval_mod_q_ligerito(&mut pt, &hint, &p, &rw_q, FQ_BITS, alpha, &pc);

        // Prover-side `us` is full width; the wire carries only the live
        // prefix (the last live column is non-zero by construction).
        assert_eq!(proof.us[0].len(), p.cols(), "prover holds all 2^s folds");
        assert!(
            proof.us[0][live..].iter().all(|&u| u == 0),
            "padding folds are zero"
        );
        let bytes = proof.to_bytes();
        let proof2 = IntEvalRsLigModQProof::from_bytes(&bytes).expect("deserialize");
        assert_eq!(
            proof2.us[0].len(),
            live,
            "only the live folds are transmitted"
        );

        // …and it still verifies, with no verifier-side change.
        let mut vt = Blake3Transcript::new();
        verify_mle_eval_mod_q_ligerito(
            &mut vt,
            &hint.commitment,
            &proof2,
            &p,
            &rw_q,
            &col_w,
            alpha,
            y,
            FQ_BITS,
            &vc,
        )
        .expect("trimmed proof verifies");
        assert_eq!(bytes, proof2.to_bytes(), "codec is canonical");

        // A non-minimal `us` block (one transmitted trailing zero) decodes
        // to the same proof object, so the codec must reject it.
        let mut needle = Vec::new();
        for &u in &proof2.us[0] {
            needle.extend_from_slice(&u.to_le_bytes());
        }
        let pos = bytes
            .windows(needle.len())
            .position(|w| w == needle)
            .expect("us block");
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


    /// The fused basis-fill + round-0 message equals the scalar `η·Φ` fill
    /// and the naive `(u_0, u_2)` pair sums (flock's `round_msg_lsb`
    /// convention) exactly.
    #[test]
    fn fill_phi_basis_round0_matches_unfused() {
        let n = 64usize;
        let lch = 2usize;
        let p_msg: Vec<Gf128> = (0..n).map(|i| sample(0xE000 + i as u64)).collect();
        let eq_his: Vec<Vec<Gf>> = (0..lch)
            .map(|l| {
                (0..n)
                    .map(|y| sample(0xF000 + (l * n + y) as u64))
                    .collect()
            })
            .collect();
        let etas: Vec<Gf> = (0..lch).map(|l| sample(0x1_0000 + l as u64)).collect();
        let eq_r2: Vec<Gf> = (0..128).map(|i| sample(0x2_0000 + i as u64)).collect();

        // Scalar reference: η·Φ per slot, then plain-mul pair sums.
        let expect_b: Vec<Gf128> = (0..n)
            .map(|y| {
                let mut acc = Gf::zero();
                for l in 0..lch {
                    acc += etas[l] * phi_bit_sum(eq_his[l][y], &eq_r2);
                }
                acc
            })
            .collect();
        let mut exp_u0 = Gf::zero();
        let mut exp_u2 = Gf::zero();
        for j in 0..n / 2 {
            let f0 = p_msg[2 * j];
            let f1 = p_msg[2 * j + 1];
            let b0 = expect_b[2 * j];
            let b1 = expect_b[2 * j + 1];
            exp_u0 += f0 * b0;
            exp_u2 += (f0 + f1) * (b0 + b1);
        }

        let mut b = vec![Gf128::ZERO; n];
        let (u0, u2) = fill_phi_basis_round0(&mut b, &p_msg, &eq_his, &etas, &eq_r2);
        for (y, (got, want)) in b.iter().zip(expect_b.iter()).enumerate() {
            assert_eq!((got.lo, got.hi), (want.lo, want.hi), "basis slot {y}");
        }
        assert_eq!(u0, exp_u0, "u_0");
        assert_eq!(u2, exp_u2, "u_2");
    }





}

#[cfg(test)]
mod ood_round_tests {
    //! Round 0 (the out-of-domain sample): the succinct residual against the
    //! dense fold, the prover's evaluation kernel, the theorem-bound
    //! accounting, and end-to-end acceptance / rejection / codec behaviour
    //! of the direct opening with the round executed; the public opening's
    //! own statement frame, its refusal of a Johnson ladder without the
    //! round, and the standalone protocol of the `bitz` CLI.
    use super::*;
    use crate::ext_proj::{ExtProjParams, sample_proj_point, sample_proj_prime};
    use crate::ligerito::bind_low;
    use crate::pcs::{IntegerMatrixLayout, mod_q_chunk_width, smallest_generator};
    use crate::transcript::Blake3Transcript;

    struct Xorshift(u64);

    impl Xorshift {
        fn next_u64(&mut self) -> u64 {
            self.0 ^= self.0 << 13;
            self.0 ^= self.0 >> 7;
            self.0 ^= self.0 << 17;
            self.0
        }

        fn gf(&mut self) -> Gf {
            let lo = self.next_u64();
            let hi = self.next_u64();
            Gf::from_polynomial_words([lo, hi])
        }
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

    /// `eq(b, r) mod q` over `b ∈ {0,1}^{r.len()}` (index bit `k` ↔ `r[k]`).
    fn eq_table_mod_q(arith: &field::FpCtx<2>, r: &[u128]) -> Vec<u128> {
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

    /// The claimed `Σ_c w_c Σ_b rw_b · cell(b, c)` mod `q` from the committed
    /// bit rows (row `c`: bit `(b << log₂W) | j` = bit `j` of cell `(b, c)`).
    fn claim_from_rows(
        p: &IntegerMatrixLayout,
        rows: &[Vec<u64>],
        rw: &[u128],
        cw: &[u128],
        arith: &field::FpCtx<2>,
    ) -> u128 {
        let log_w = p.word_bits.trailing_zeros() as usize;
        let pow2: Vec<u128> = (0..p.word_bits)
            .map(|j| arith.reduce_u128(1u128 << j))
            .collect();
        let mut y = 0u128;
        for (c, row) in rows.iter().enumerate() {
            let mut acc = 0u128;
            for (wi, &word) in row.iter().enumerate() {
                let mut bits = word;
                while bits != 0 {
                    let bit = bits.trailing_zeros() as usize;
                    bits &= bits - 1;
                    let i = (wi << 6) | bit;
                    let (b, j) = (i >> log_w, i & (p.word_bits - 1));
                    let term = if j == 0 {
                        rw[b]
                    } else {
                        arith.mul_u128(rw[b], pow2[j])
                    };
                    acc = arith.add_u128(acc, term);
                }
            }
            y = arith.add_u128(y, arith.mul_u128(cw[c], acc));
        }
        y
    }

    /// The standalone protocol of the `bitz` CLI at one tiny shape: statement,
    /// transcript-sampled prime and point, claim, then the opening with the
    /// requested Round-0 parameters.
    fn standalone_roundtrip(t: usize, s: usize, w: usize, ood: Option<OodRoundParams>) {
        let alpha = smallest_generator();
        let p = IntegerMatrixLayout {
            row_vars: t,
            col_vars: s,
            word_bits: w,
        };
        let q_bits = mod_q_chunk_width(&p).min(113);
        let (pc, vc) = lig_configs(
            packed_vars(&p),
            LigConfig::Adhoc {
                log_batch: 2,
                log_inv_rate: 2,
            },
        )
        .expect("cfg");
        let mask = if w == 128 {
            u128::MAX
        } else {
            (1u128 << w) - 1
        };
        let data: Vec<u128> = (0..p.cells())
            .map(|i| (i as u128).wrapping_mul(0x9E37_79B9_7F4A_7C15) & mask)
            .collect();
        let hint = commit_rs_flock_with(&p, &data, pc.log_inv_rates[0], pc.initial_k);

        let statement = |transcript: &mut Blake3Transcript| {
            absorb_standalone_mod_q_statement(
                transcript,
                &hint.commitment,
                &p,
                alpha,
                q_bits,
                ood,
                &vc,
            );
            let proj = ExtProjParams {
                prime_bits: q_bits,
                ..ExtProjParams::default()
            };
            let q = sample_proj_prime(transcript, &proj).unwrap();
            let arith = field::FpCtx::from_prime_u128(q);
            let r1: Vec<u128> = (0..p.row_vars)
                .map(|_| sample_proj_point(transcript, q))
                .collect();
            let r2: Vec<u128> = (0..p.col_vars)
                .map(|_| sample_proj_point(transcript, q))
                .collect();
            (q, eq_table_mod_q(&arith, &r1), eq_table_mod_q(&arith, &r2))
        };
        let (q, rw, cw) = {
            let mut st = Blake3Transcript::new();
            statement(&mut st)
        };
        let y = claim_from_rows(&p, hint.rows(), &rw, &cw, &field::FpCtx::from_prime_u128(q));

        let mut pt = Blake3Transcript::new();
        let (q_p, rw_p, _) = statement(&mut pt);
        assert_eq!(q_p, q);
        absorb_standalone_mod_q_claim(&mut pt, q, y);
        let proof = prove_mle_eval_mod_q_ligerito_with_ood(
            &mut pt, &hint, &p, &rw_p, q_bits, alpha, ood, &pc,
        );
        assert_eq!(proof.ood.is_some(), ood.is_some());
        assert_eq!(
            proof.ood.and_then(|round| round.nonce).is_some(),
            ood.is_some_and(|params| params.grinding_bits > 0)
        );

        let verify =
            |proof: &IntEvalRsLigModQProof, params: Option<OodRoundParams>, claimed: u128| {
                let mut vt = Blake3Transcript::new();
                let (q_v, rw_v, cw_v) = statement(&mut vt);
                absorb_standalone_mod_q_claim(&mut vt, q_v, claimed);
                verify_mle_eval_mod_q_ligerito_runtime(
                    &mut vt,
                    &hint.commitment,
                    proof,
                    &p,
                    &rw_v,
                    &cw_v,
                    alpha,
                    claimed,
                    q_v,
                    q_bits,
                    params,
                    &vc,
                )
            };
        verify(&proof, ood, y).unwrap_or_else(|e| panic!("t={t} s={s} W={w} ood={ood:?}: {e:?}"));
        assert!(
            verify(&proof, ood, (y + 1) % q).is_err(),
            "a wrong claim must be rejected"
        );

        // The codec carries the round canonically.
        let bytes = proof.to_bytes();
        let decoded = IntEvalRsLigModQProof::from_bytes(&bytes).expect("codec");
        assert_eq!(decoded.ood, proof.ood);
        assert_eq!(decoded.to_bytes(), bytes);
        let mut trailing = bytes.clone();
        trailing.push(0);
        assert!(IntEvalRsLigModQProof::from_bytes(&trailing).is_err());
        verify(&decoded, ood, y).expect("decoded proof verifies");

        if let Some(params) = ood {
            let round = proof.ood.expect("round present");
            // Parameters and proof must agree on the round's presence.
            let mut without = proof.clone();
            without.ood = None;
            assert_eq!(verify(&without, ood, y).err(), Some(FlockRsError::OodRound));
            assert_eq!(verify(&proof, None, y).err(), Some(FlockRsError::OodRound));
            // A wrong out-of-domain value breaks the batched opening.
            let mut wrong_y = proof.clone();
            wrong_y.ood = Some(OodRound {
                y: round.y + Gf::one(),
                nonce: round.nonce,
            });
            assert!(
                verify(&wrong_y, ood, y).is_err(),
                "a wrong OOD value must be rejected"
            );
            // The nonce must be present exactly at a nonzero difficulty.
            let mut nonce_flip = proof.clone();
            nonce_flip.ood = Some(OodRound {
                y: round.y,
                nonce: if params.grinding_bits == 0 {
                    Some(0)
                } else {
                    None
                },
            });
            assert_eq!(
                verify(&nonce_flip, ood, y).err(),
                Some(FlockRsError::OodRound)
            );
            if params.grinding_bits > 0 {
                // A different difficulty invalidates the transcript prefix.
                let other = Some(OodRoundParams {
                    grinding_bits: params.grinding_bits + 1,
                });
                assert!(verify(&proof, other, y).is_err());
            }
        }
    }

    #[test]
    fn standalone_direct_opening_with_round0() {
        standalone_roundtrip(10, 5, 1, None);
        standalone_roundtrip(10, 5, 1, Some(OodRoundParams { grinding_bits: 0 }));
        standalone_roundtrip(10, 5, 1, Some(OodRoundParams { grinding_bits: 6 }));
        standalone_roundtrip(4, 8, 32, Some(OodRoundParams { grinding_bits: 2 }));
    }

    #[test]
    fn trailer_carries_forest_nonces_and_the_round_together() {
        let alpha = smallest_generator();
        let p = IntegerMatrixLayout {
            row_vars: 10,
            col_vars: 5,
            word_bits: 1,
        };
        let q_bits = 100usize;
        let (pc, _vc) = lig_configs(
            packed_vars(&p),
            LigConfig::Adhoc {
                log_batch: 2,
                log_inv_rate: 2,
            },
        )
        .expect("cfg");
        let data: Vec<u128> = (0..p.cells()).map(|i| (i as u128 * 7) & 1).collect();
        let hint = commit_rs_flock_with(&p, &data, pc.log_inv_rates[0], pc.initial_k);
        let rw_q: Vec<u128> = (0..p.rows())
            .map(|b| (b as u128 + 3) * 0x1234_5678_9abc)
            .collect();
        let chunks = ModQWeightChunks::from_dense(&p, &rw_q, q_bits).expect("chunks");
        for (forest_bits, ood) in [
            (2u32, Some(OodRoundParams { grinding_bits: 1 })),
            (2, Some(OodRoundParams { grinding_bits: 0 })),
            (2, None),
            (0, Some(OodRoundParams { grinding_bits: 3 })),
            (0, None),
        ] {
            let mut pt = Blake3Transcript::new();
            let proof = prove_mle_eval_mod_q_ligerito_raw(
                &mut pt,
                &hint,
                &p,
                &chunks,
                alpha,
                &pc,
                forest_bits,
                ood,
            );
            assert_eq!(proof.grinding_nonces.is_empty(), forest_bits == 0);
            assert_eq!(proof.ood.is_some(), ood.is_some());
            let bytes = proof.to_bytes();
            let decoded = IntEvalRsLigModQProof::from_bytes(&bytes).expect("codec");
            assert_eq!(decoded.grinding_nonces, proof.grinding_nonces);
            assert_eq!(decoded.ood, proof.ood);
            assert_eq!(decoded.to_bytes(), bytes);
        }
    }

    /// Random bit rows in the committed layout (row `c`: bit
    /// `(b << log₂W) | j` = bit `j` of cell `(b, c)`).
    fn random_bit_rows(p: &IntegerMatrixLayout, seed: u64) -> Vec<Vec<u64>> {
        let mut rng = Xorshift(seed);
        let bits = p.rows() * p.word_bits;
        (0..p.cols())
            .map(|_| {
                let mut row: Vec<u64> = (0..bits.div_ceil(64)).map(|_| rng.next_u64()).collect();
                if bits % 64 != 0 {
                    *row.last_mut().expect("words") &= (1u64 << (bits % 64)) - 1;
                }
                row
            })
            .collect()
    }

    /// A prime and eq weight tables of `q_bits` bits, drawn off a transcript
    /// the openings below never see.
    fn fixed_instance(p: &IntegerMatrixLayout, q_bits: usize) -> StandaloneInstance {
        let mut transcript = Blake3Transcript::new();
        transcript.absorb_slice(b"mod-q binding test instance");
        sample_standalone_instance(&mut transcript, p, q_bits)
    }

    /// The public opening binds its own statement: every input of the frame
    /// (one row weight, the prime width, the generator, the commitment)
    /// moves the transcript, prover and verifier configurations absorb
    /// alike, and a proof of the bare core on a fresh transcript (no frame)
    /// is rejected.
    #[test]
    fn public_mod_q_opening_binds_its_statement() {
        let alpha = smallest_generator();
        let p = IntegerMatrixLayout {
            row_vars: 10,
            col_vars: 5,
            word_bits: 1,
        };
        let q_bits = 100;
        let (pc, vc) = lig_configs(
            packed_vars(&p),
            LigConfig::Adhoc {
                log_batch: 2,
                log_inv_rate: 2,
            },
        )
        .expect("cfg");
        let hint = commit_rs_ligerito_rows(&p, random_bit_rows(&p, 0x5eed_0101), &pc);
        let other = commit_rs_ligerito_rows(&p, random_bit_rows(&p, 0x5eed_0102), &pc);
        let instance = fixed_instance(&p, q_bits);
        let (q, rw, cw) = (instance.q, &instance.row_weights_q, &instance.col_weights_q);

        let frame = |commitment: &Commitment, rw: &[u128], q_bits: usize, alpha: Gf| {
            let mut transcript = Blake3Transcript::new();
            let _ = absorb_mod_q_statement(&mut transcript, commitment, &p, rw, q_bits, alpha, &vc);
            transcript.state_digest()
        };
        let base = frame(&hint.commitment, rw, q_bits, alpha);
        let mut last = rw.clone();
        *last.last_mut().expect("rows") ^= 1;
        assert_ne!(frame(&hint.commitment, &last, q_bits, alpha), base);
        assert_ne!(frame(&hint.commitment, rw, q_bits - 1, alpha), base);
        assert_ne!(frame(&hint.commitment, rw, q_bits, alpha * alpha), base);
        assert_ne!(frame(&other.commitment, rw, q_bits, alpha), base);
        let mut transcript = Blake3Transcript::new();
        let _ = absorb_mod_q_statement(
            &mut transcript,
            &hint.commitment,
            &p,
            rw,
            q_bits,
            alpha,
            &pc,
        );
        assert_eq!(
            transcript.state_digest(),
            base,
            "prover and verifier frames agree"
        );

        let y = claim_from_rows(&p, hint.rows(), rw, cw, &field::FpCtx::from_prime_u128(q));
        let verify = |proof: &IntEvalRsLigModQProof| {
            verify_mle_eval_mod_q_ligerito_runtime(
                &mut Blake3Transcript::new(),
                &hint.commitment,
                proof,
                &p,
                rw,
                cw,
                alpha,
                y,
                q,
                q_bits,
                None,
                &vc,
            )
        };
        let bound = prove_mle_eval_mod_q_ligerito(
            &mut Blake3Transcript::new(),
            &hint,
            &p,
            rw,
            q_bits,
            alpha,
            &pc,
        );
        verify(&bound).expect("the self-bound opening verifies");
        let chunks = ModQWeightChunks::from_dense(&p, rw, q_bits).expect("chunks");
        let unbound = prove_mle_eval_mod_q_ligerito_raw(
            &mut Blake3Transcript::new(),
            &hint,
            &p,
            &chunks,
            alpha,
            &pc,
            0,
            None,
        );
        assert!(
            verify(&unbound).is_err(),
            "a proof without the statement frame must be rejected"
        );
    }

    /// Only Round 0 binds a Johnson ladder's first-level list, so the public
    /// opening refuses such a ladder without it: the prover panics, and the
    /// verifier rejects even the proof the round-less opening would produce
    /// on exactly its transcript.
    #[test]
    fn johnson_ladder_without_round0_is_refused() {
        let alpha = smallest_generator();
        let p = IntegerMatrixLayout {
            row_vars: 12,
            col_vars: 8,
            word_bits: 1,
        };
        let resolved = LigeritoSelection::JOHNSON
            .resolve(packed_vars(&p), 100)
            .expect("m = 20 Johnson ladder");
        let ood = resolved
            .round0(100)
            .expect("Round 0 under the grinding cap");
        assert!(
            ood.is_some() && needs_round0(resolved.prover()) && needs_round0(resolved.verifier())
        );
        let hint = commit_rs_ligerito_rows(&p, random_bit_rows(&p, 0x5eed_0201), resolved.prover());
        let q_bits = standalone_q_bits(&p);
        let instance = fixed_instance(&p, q_bits);
        let (q, rw, cw) = (instance.q, &instance.row_weights_q, &instance.col_weights_q);
        let y = claim_from_rows(&p, hint.rows(), rw, cw, &field::FpCtx::from_prime_u128(q));
        let verify = |proof: &IntEvalRsLigModQProof, params: Option<OodRoundParams>| {
            verify_mle_eval_mod_q_ligerito_runtime(
                &mut Blake3Transcript::new(),
                &hint.commitment,
                proof,
                &p,
                rw,
                cw,
                alpha,
                y,
                q,
                q_bits,
                params,
                resolved.verifier(),
            )
        };

        let proof = prove_mle_eval_mod_q_ligerito_with_ood(
            &mut Blake3Transcript::new(),
            &hint,
            &p,
            rw,
            q_bits,
            alpha,
            ood,
            resolved.prover(),
        );
        verify(&proof, ood).expect("the ladder verifies with Round 0");
        assert_eq!(verify(&proof, None).err(), Some(FlockRsError::OodRound));

        let mut transcript = Blake3Transcript::new();
        let _ = absorb_mod_q_statement(
            &mut transcript,
            &hint.commitment,
            &p,
            rw,
            q_bits,
            alpha,
            resolved.prover(),
        );
        let chunks = ModQWeightChunks::from_dense(&p, rw, q_bits).expect("chunks");
        let roundless = prove_mle_eval_mod_q_ligerito_raw(
            &mut transcript,
            &hint,
            &p,
            &chunks,
            alpha,
            resolved.prover(),
            0,
            None,
        );
        assert!(roundless.ood.is_none());
        assert_eq!(verify(&roundless, None).err(), Some(FlockRsError::OodRound));

        let refused = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            prove_mle_eval_mod_q_ligerito(
                &mut Blake3Transcript::new(),
                &hint,
                &p,
                rw,
                q_bits,
                alpha,
                resolved.prover(),
            )
        }))
        .err()
        .expect("the prover refuses a Johnson ladder without Round 0");
        let message = refused
            .downcast_ref::<&str>()
            .map(|message| message.to_string())
            .or_else(|| refused.downcast_ref::<String>().cloned())
            .unwrap_or_default();
        assert!(message.contains("needs Round 0"), "{message}");
    }

    /// [`StandaloneModQOpening`] is, byte for byte, the transcript the
    /// `bitz` CLI and `benches/pcs.rs` spelled out before it (statement,
    /// policy, Round 0, prime and point, claim, then the bare core); it
    /// verifies its proofs, rejects a wrong claim, and takes Round 0
    /// exactly when the ladder is beyond unique decoding.
    #[test]
    fn standalone_opening_keeps_the_cli_transcript() {
        let alpha = smallest_generator();
        let p = IntegerMatrixLayout {
            row_vars: 12,
            col_vars: 8,
            word_bits: 1,
        };
        let q_bits = standalone_q_bits(&p);
        for selection in [LigeritoSelection::JOHNSON, LigeritoSelection::MATCHED_UDR] {
            let name = selection.name();
            let resolved = selection
                .resolve(packed_vars(&p), 100)
                .expect("m = 20 ladder");
            let ood = resolved
                .round0(100)
                .expect("Round 0 under the grinding cap");
            assert_eq!(
                ood.is_some(),
                selection == LigeritoSelection::JOHNSON,
                "{name}"
            );
            let mismatched = match ood {
                Some(_) => None,
                None => Some(OodRoundParams { grinding_bits: 0 }),
            };
            assert_eq!(
                StandaloneModQOpening::new(&p, alpha, q_bits, mismatched, &resolved).err(),
                Some(FlockRsError::OodRound),
                "{name}"
            );
            let opening = StandaloneModQOpening::new(&p, alpha, q_bits, ood, &resolved)
                .expect("Round 0 matches");
            let hint =
                commit_rs_ligerito_rows(&p, random_bit_rows(&p, 0x5eed_0301), resolved.prover());
            let instance = opening.instance(&hint);
            let q = instance.q;
            let y = claim_from_rows(
                &p,
                hint.rows(),
                &instance.row_weights_q,
                &instance.col_weights_q,
                &field::FpCtx::from_prime_u128(q),
            );
            let proof = opening.prove(&hint, y);

            let mut transcript = Blake3Transcript::new();
            absorb_standalone_mod_q_statement(
                &mut transcript,
                &hint.commitment,
                &p,
                alpha,
                q_bits,
                ood,
                resolved.verifier(),
            );
            resolved.bind(&mut transcript);
            let bound = bind_prover_ood(&mut transcript, &hint, ood);
            let sampled = sample_standalone_instance(&mut transcript, &p, q_bits);
            assert_eq!(sampled.q, q, "{name}");
            absorb_standalone_mod_q_claim(&mut transcript, q, y);
            let chunks =
                ModQWeightChunks::from_dense(&p, &sampled.row_weights_q, q_bits).expect("chunks");
            let manual = prove_mle_eval_mod_q_ligerito_raw(
                &mut transcript,
                &hint,
                &p,
                &chunks,
                alpha,
                resolved.prover(),
                0,
                bound,
            );
            assert_eq!(proof.to_bytes(), manual.to_bytes(), "{name}");

            opening
                .verify(&hint.commitment, &proof, y)
                .unwrap_or_else(|error| panic!("{name}: {error:?}"));
            assert!(
                opening
                    .verify(&hint.commitment, &proof, (y + 1) % q)
                    .is_err(),
                "{name}: a wrong claim must be rejected"
            );
        }
    }
}

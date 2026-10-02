//! BitZ logarithmic (proof of concept): the scheme of the note
//! `paper/notes/bitz_logarithmic.tex`, with one recursive call.
//!
//! Standard BitZ (§2.1 of the paper, Step 4) sends the `2^s` integer column
//! folds `μ_j = Σ_i γ_i f_ij` (`γ = π_q^{-1}(u^(1))`), and the verifier
//! checks `Σ_j π_q(μ_j)·u^(2)_j = μ` and exponentiates every fold for the
//! grand products. Here the prover commits to the bits of the folds
//! instead, and
//!
//! 1. a second batched product tree over those bits,
//!    `Π_k ((h_k − 1)·μ_{jk} + 1) = g^{μ_j}` with `h_k = g^{2^k}`, meets the
//!    first one (`Π_i ((g^{γ_i} − 1)·f_ij + 1)`) at one random point `ζ` of
//!    the shared output layer: the prover sends `e₀ = MLE[g^μ](ζ)` and both
//!    GKRs start from it;
//! 2. the recombination `Σ_j π_q(μ_j)·u^(2)_j = μ` is proved by one
//!    standard (recursive) BitZ instance over the committed bits, with
//!    weights `(2^k mod q)·u^(2)_j` (tensor-split again);
//! 3. the claim on `f` is opened as before, and the two claims on the folds'
//!    bits (the second tree's exit, the recursive instance's exit) are
//!    batched into one opening of the second commitment.
//!
//! Only 127 of the 128 bit slots of a fold carry weight (`h_127 = 1`, its
//! recursive weight zero), so a committed exponent stays below
//! `2^127 < ord(g)` and `g^{μ_j}` determines `μ_j`: no range check.
//!
//! The verifier never reads a column fold. What remains linear in a
//! dimension is the multilinear extension of `(g^{γ_i})_i` at the
//! sumcheck's row point (the note's `(⋆)`: the `2^t` exponentiations and
//! the `eq` table of `r₁` mod `q`), and the recursive instance's own
//! `O(2^{t'} + 2^{s'})` work (its folds, and the same `(⋆)` term for its
//! row weights). Every phase is traced so a caller can report them apart.
//!
//! Native challenges run under the same zero-work schedules as BitZ, one per
//! sub-protocol; the second tree shares the fold point and has no schedule
//! of its own, so its draws are unscheduled (and refused unless the policy
//! grinds nothing).

#[cfg(feature = "parallel")]
use rayon::prelude::*;

use flock_core::pcs::ligerito::{
    BranchLevel0, LigeritoProfile, LigeritoSecurityConfig, ProverConfig, SoundnessRegime,
    VerifierConfig,
};

use super::fold::{self, Fold};
use super::forest;
use super::gkr::{eq_table, gpgkr_verify};
use super::grinding::{Geometry, Policy, Schedule, Stage};
use super::params::{BitZParams, LinearClaim, Root, Shape, SumClaimGf, reference_log_rows};
use super::pcs::{MergePlan, OpeningQuery, Pcs, StatementBinding};
use super::reduce;
use super::transcript::{ProverState, VerifierState, build_prover, build_verifier};
use super::{BitZProver, FixedBasePow, WINDOW, trace};
use crate::cfg_into_iter;
use crate::ligerito::{mle_eval, pack_columns_from_rows};
use crate::ligerito_flock::{
    FlockCommitHint, LigeritoSelection, OodRoundParams, eq_table_mod_q, ood_eval, ood_point,
    ood_round_params,
};
use crate::pcs::IntegerMatrixLayout;
use crate::piop::spartan::protocol::bitz_opener::{BitZLigerito, BitZOpener, BitZOpeningProof};
use crate::transcript::traits::Transcript;
use crate::utils::wide_mul::WideMulAcc;
use field::Gf128 as Gf;

/// Bit slots per fold: one packed element of the second commitment.
pub const MU_SLOTS: usize = 128;
/// The slots that carry weight; the top one is dead (exponents `< 2^127`).
pub const MU_LIVE: usize = 127;
/// `log₂ MU_SLOTS`: the second tree's row variables.
const MU_ROW_VARS: usize = 7;
/// The smallest committed size the crate's Ligerito selections resolve at
/// (the default floor of the second commitment; explicit ladders go lower).
pub const MIN_MU_LOG_BITS: usize = 20;
/// The session tag of the standalone opening's fork.
const SESSION: &[u8] = b"bitz/bitz-log/standalone/v1";
/// The inner transcript's frame labels.
const STATEMENT_LABEL: &[u8] = b"bitz/log/statement/v1";
const FOLD_COMMITMENT_LABEL: &[u8] = b"bitz/log/fold-commitment/v1";
const BATCH_LABEL: &[u8] = b"bitz/log/fold-opening-batch/v1";
const FOLD_OOD_GRINDING_LABEL: &[u8] = b"bitz/log/fold-round0-grinding/v1";

/// What fails in the logarithmic scheme.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LogError {
    /// The geometry or a parameter gate rejects the instance.
    Config(String),
    /// The committed rows are not the opener's shape.
    Witness,
    /// A proof message is missing or malformed.
    Malformed,
    /// A check failed.
    Rejected(&'static str),
    /// The opening of `f` or of the folds' bits failed.
    Opening(String),
}

/// The recursion's geometry: the committed grid `2^t × 2^s`, the second
/// tree's `2^7 × 2^s` view of the folds' bits, the recursive instance's
/// `2^{7+a} × 2^{s−a}` view of the same bits (the low `a` column variables
/// join its rows), and the second commitment's `2^{7+a} × 2^{s_com}` grid
/// (zero columns pad it to the smallest size a ladder resolves at).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LogGeometry {
    pub t: usize,
    pub s: usize,
    pub a: usize,
    pub s_com: usize,
}

impl LogGeometry {
    /// `a = None` takes `t' = ⌈0.6 (s + 7)⌉` (the reference split plus one
    /// row variable: the recursive instance's folds are proof bytes, its
    /// rows only `2^{t'}` verifier exponentiations), at least 7, at most
    /// `t`; the second
    /// commitment is padded with zero columns to at least `2^min_log_bits`
    /// bits.
    pub fn new(t: usize, s: usize, a: Option<usize>, min_log_bits: usize) -> Result<Self, LogError> {
        let a = a
            .unwrap_or_else(|| (reference_log_rows(s + MU_ROW_VARS) + 1).saturating_sub(MU_ROW_VARS))
            .min(s)
            .min(t.saturating_sub(MU_ROW_VARS));
        if MU_ROW_VARS + a > t {
            return Err(LogError::Config(format!(
                "recursive rows 7 + a = {} exceed t = {t}: its folds would overflow the exponent",
                MU_ROW_VARS + a
            )));
        }
        let s_com = (s - a).max(min_log_bits.saturating_sub(MU_ROW_VARS + a));
        Ok(Self { t, s, a, s_com })
    }

    pub fn shape(&self) -> Result<Shape, LogError> {
        Shape::new(self.t, self.s).map_err(|e| LogError::Config(format!("shape: {e:?}")))
    }

    /// The second tree's grid: one row per bit slot, one column per fold.
    pub fn tree_shape(&self) -> Result<Shape, LogError> {
        Shape::new(MU_ROW_VARS, self.s).map_err(|e| LogError::Config(format!("tree shape: {e:?}")))
    }

    pub fn rec_shape(&self) -> Result<Shape, LogError> {
        Shape::new(MU_ROW_VARS + self.a, self.s - self.a)
            .map_err(|e| LogError::Config(format!("recursive shape: {e:?}")))
    }

    pub fn commitment_shape(&self) -> Result<Shape, LogError> {
        Shape::new(MU_ROW_VARS + self.a, self.s_com)
            .map_err(|e| LogError::Config(format!("fold commitment shape: {e:?}")))
    }

    fn frame(&self) -> [u8; 32] {
        let mut out = [0u8; 32];
        for (i, v) in [self.t, self.s, self.a, self.s_com].into_iter().enumerate() {
            out[8 * i..8 * i + 8].copy_from_slice(&(v as u64).to_le_bytes());
        }
        out
    }
}

/// The second commitment's Ligerito ladder.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MuLadder {
    /// A validated crate selection (`custom:r:k`, `udr:r:k`, …), resolved at
    /// the commitment's size (`m ≥ 20`).
    Selection(LigeritoSelection),
    /// A Johnson ladder with an explicit shape, `lad:r0:k0:k_rec:final`:
    /// rate `2^{−r0}`, initial fold `k0`, recursive folds of `k_rec`
    /// variables until at most `final` remain (the crate's selections use
    /// `k_rec = 3`, `final = 5`). Also below `m = 20` (unaudited regime).
    /// A single run executes its level 0 only: it keeps `r0`, chooses `k0`
    /// and never runs the tail ([`MuLadder::level_0`]).
    Ladder { r0: usize, k0: usize, k_rec: usize, final_log: usize },
}

impl MuLadder {
    pub fn parse(request: &str, target: usize) -> Result<Self, String> {
        let parts: Vec<&str> = request.trim().split(':').collect();
        if parts.first() == Some(&"lad") {
            if parts.len() != 5 {
                return Err(format!("expected lad:r0:k0:k_rec:final, got {request:?}"));
            }
            let v = |i: usize| parts[i].parse::<usize>().map_err(|_| format!("bad ladder {request:?}"));
            return Ok(Self::Ladder { r0: v(1)?, k0: v(2)?, k_rec: v(3)?, final_log: v(4)? });
        }
        LigeritoSelection::parse(request, target).map(Self::Selection)
    }

    /// The default floor of the second commitment: the crate's selections
    /// resolve from `m = 20`, explicit ladders go down to `m = 14`.
    pub fn default_min_log_bits(&self) -> usize {
        match self {
            Self::Selection(_) => MIN_MU_LOG_BITS,
            Self::Ladder { .. } => 14,
        }
    }

    /// The fold bits' ladder in a single run at `packed_vars`: level 0 at
    /// rate `2^{−r0}` and interleaving `k0`, then the shortest tail flock
    /// accepts (one fold of one variable). The tail is never executed (the
    /// levels after level 0 are the merged chain's), so it must not decide
    /// which level-0 shapes are admissible.
    fn level_0(r0: usize, k0: usize, packed_vars: usize) -> Self {
        Self::Ladder { r0, k0, k_rec: 1, final_log: packed_vars.saturating_sub(k0 + 1) }
    }

    pub fn name(&self) -> String {
        match self {
            Self::Selection(selection) => selection.name(),
            Self::Ladder { r0, k0, k_rec, final_log } => format!("lad:{r0}:{k0}:{k_rec}:{final_log}"),
        }
    }

    /// The validated security configuration and its digest at `packed_vars`.
    fn resolve(&self, packed_vars: usize, target: usize) -> Result<(LigeritoSecurityConfig, [u8; 32]), String> {
        match self {
            Self::Selection(selection) => {
                let resolved = selection.resolve(packed_vars, target)?;
                Ok((resolved.security().clone(), resolved.digest()))
            }
            &Self::Ladder { r0, k0, k_rec, final_log } => {
                if !(1..=8).contains(&r0) {
                    return Err("lad: rate exponent 1..8".into());
                }
                if !(64..=128).contains(&target) {
                    return Err("lad: target bits 64..=128".into());
                }
                // At least one recursive level (flock needs one): a small
                // commitment stops folding earlier.
                let final_log = final_log.min(packed_vars.saturating_sub(k0 + 1));
                let mut security = crate::ligerito_flock::try_custom_johnson_ladder_bits(
                    packed_vars + MU_ROW_VARS,
                    r0,
                    k0,
                    k_rec,
                    final_log,
                    Some(target),
                )?;
                security.hash = "blake3".into();
                security.validate()?;
                let digest = *blake3::hash(&bincode::serialize(&security).map_err(|e| e.to_string())?).as_bytes();
                Ok((security, digest))
            }
        }
    }
}

/// The opener: BitZ's opener for the committed bits `f`, and the PCS of the
/// folds' bits under its own ladder.
#[derive(Clone, Debug)]
pub struct LogOpener {
    base: BitZOpener,
    geometry: LogGeometry,
    base_ood: Option<OodRoundParams>,
    mu_pcs: Pcs,
    mu_security: LigeritoSecurityConfig,
    mu_ladder: MuLadder,
    mu_digest: [u8; 32],
    mu_ood: Option<OodRoundParams>,
    policy: Policy,
    /// The single-run opening (the fold bits join `f`'s Ligerito ladder);
    /// `None` opens the two commitments in two runs.
    merge: Option<MergePlan>,
}

impl LogOpener {
    /// `layout` is the committed grid (`t × s`), `ladder` the opener of
    /// `f`, `mu_ladder` the second commitment's (resolved at its size), `a`
    /// the recursive split and `min_mu_log_bits` the second commitment's
    /// floor (see [`LogGeometry::new`]).
    pub fn new(
        layout: IntegerMatrixLayout,
        ladder: BitZLigerito,
        mu_ladder: MuLadder,
        a: Option<usize>,
        min_mu_log_bits: Option<usize>,
        single_run: bool,
        target_bits: usize,
    ) -> Result<Self, LogError> {
        let base = BitZOpener::new(layout, ladder, target_bits)
            .map_err(|e| LogError::Config(format!("base opener: {e:?}")))?;
        let floor = min_mu_log_bits.unwrap_or_else(|| mu_ladder.default_min_log_bits());
        let mut geometry = LogGeometry::new(layout.row_vars, layout.col_vars, a, floor)?;
        // One run: the fold bits' interleaving is chosen so their folded
        // message meets a level of `f`'s ladder (cheapest level-0 opening);
        // when no level fits their natural size, they are padded with zero
        // columns (a larger floor) until one does.
        let mu_ladder = if single_run {
            let MuLadder::Ladder { r0, .. } = mu_ladder else {
                return Err(LogError::Config("a single run needs an explicit lad: ladder".into()));
            };
            if !all_johnson(base.security()) {
                return Err(LogError::Config("a single run needs a Johnson ladder for f".into()));
            }
            let mut chosen = None;
            let mut refusal = None;
            for extra in 0..=6 {
                let candidate = LogGeometry::new(layout.row_vars, layout.col_vars, a, floor + extra)?;
                if extra > 0 && candidate == geometry {
                    continue;
                }
                let packed = candidate.commitment_shape()?.log_packed_len();
                match choose_branch_interleave(base.security(), packed, r0, target_bits) {
                    Ok(k0) => {
                        chosen = Some((candidate, MuLadder::level_0(r0, k0, packed)));
                        break;
                    }
                    // The refusal at the natural size says the most.
                    Err(e) => {
                        refusal.get_or_insert(e);
                    }
                }
            }
            let Some((candidate, ladder)) = chosen else {
                return Err(refusal.unwrap_or_else(|| {
                    LogError::Config("no level of f's ladder meets the fold bits".into())
                }));
            };
            geometry = candidate;
            ladder
        } else {
            mu_ladder
        };
        let mu_shape = geometry.commitment_shape()?;
        let mu_packed_vars = mu_shape.log_packed_len();
        let (mu_security, mu_digest) = mu_ladder
            .resolve(mu_packed_vars, target_bits)
            .map_err(|e| LogError::Config(format!("fold commitment ladder: {e}")))?;
        let policy = Policy::new(Some(target_bits as u32), 0, 0)
            .map_err(|_| LogError::Config("native policy".into()))?;
        // The second tree's draws are unscheduled: nothing may be ground.
        if policy.work(Stage::GkrRound, 1) != Ok(0) || policy.work(Stage::GkrClose, 1) != Ok(0) {
            return Err(LogError::Config("the second tree needs a zero-work native policy".into()));
        }
        let mu_pcs = Pcs::with_security(&mu_shape, &mu_security, LigeritoProfile::Fast)
            .map_err(|e| LogError::Config(format!("fold commitment pcs: {e:?}")))?
            .with_native_policy(policy);
        let mu_ood = ood_round_params(
            &mu_security,
            mu_packed_vars,
            mu_security.target_security_bits as u32,
        );
        // A Johnson level 0 needs the commitment's Round 0 (as
        // `LigeritoSelection::resolve` insists for the crate's selections).
        let johnson = mu_security
            .levels
            .first()
            .is_some_and(|level| matches!(level.regime, SoundnessRegime::JohnsonOod));
        if johnson != mu_ood.is_some() {
            return Err(LogError::Config("fold commitment: Johnson regime without its Round 0".into()));
        }
        if mu_ood.is_some_and(|params| params.grinding_bits > super::grinding::MAX_GRINDING_BITS) {
            return Err(LogError::Config("the fold commitment's Round 0 needs too much grinding".into()));
        }
        let packed_vars = layout.row_vars + layout.col_vars - MU_ROW_VARS;
        let base_ood = ood_round_params(
            base.security(),
            packed_vars,
            base.security().target_security_bits as u32,
        );
        let johnson_f = base
            .security()
            .levels
            .first()
            .is_some_and(|level| matches!(level.regime, SoundnessRegime::JohnsonOod));
        if johnson_f != base_ood.is_some() {
            return Err(LogError::Config("f: Johnson regime without its Round 0".into()));
        }
        let merge = if single_run {
            Some(merged_plan(base.security(), &mu_security, target_bits)?)
        } else {
            None
        };
        Ok(Self {
            base,
            geometry,
            base_ood,
            mu_pcs,
            mu_security,
            mu_ladder,
            mu_digest,
            mu_ood,
            policy,
            merge,
        })
    }

    /// The single-run plan, when the openings share one Ligerito run.
    pub fn merge_plan(&self) -> Option<&MergePlan> {
        self.merge.as_ref()
    }

    /// The opening mode, bound with the statement: one run (tag 1, then the
    /// digest of the merged chain it executes) or two runs (all zero).
    fn mode_frame(&self) -> [u8; 33] {
        let mut out = [0u8; 33];
        if let Some(plan) = &self.merge {
            out[0] = 1;
            out[1..].copy_from_slice(&plan.digest());
        }
        out
    }

    pub fn base(&self) -> &BitZOpener {
        &self.base
    }

    pub fn geometry(&self) -> &LogGeometry {
        &self.geometry
    }

    pub fn mu_security(&self) -> &LigeritoSecurityConfig {
        &self.mu_security
    }

    pub fn mu_ladder(&self) -> MuLadder {
        self.mu_ladder
    }

    /// Whether the second commitment runs its own Round 0.
    pub fn mu_round_0(&self) -> bool {
        self.mu_ood.is_some()
    }

    /// The proof-of-work bits of the second commitment's Round 0.
    pub fn mu_round_0_grinding(&self) -> u32 {
        self.mu_ood.map_or(0, |params| params.grinding_bits)
    }

    /// Commits `f` (the base opener's commitment).
    pub fn commit(&self, rows: Vec<Vec<u64>>) -> Result<FlockCommitHint, LogError> {
        self.base
            .commit(rows)
            .map_err(|e| LogError::Config(format!("commit: {e:?}")))
    }

    fn mu_packed_vars(&self) -> usize {
        self.mu_pcs.params().m - MU_ROW_VARS
    }
}

/// The message sizes of a ladder: `[log_n, after L0's folds, after each
/// recursive iteration's folds…]` (the last is the residual).
fn ladder_dims(config: &LigeritoSecurityConfig) -> Vec<usize> {
    let mut dims = vec![config.log_n, config.log_n - config.initial_k];
    for level in config.levels.iter().skip(1) {
        let last = *dims.last().expect("dims");
        dims.push(last.saturating_sub(level.k_recursive));
    }
    dims
}

/// The fold bits' level-0 interleaving `k` for a single run: their folded
/// message (`log_n_b − k` variables) must equal `f`'s message after one of
/// its recursive iterations other than the last, and among those the
/// cheapest level-0 opening wins (rows `Q·2^k·16` B plus Merkle paths
/// ≈ `Q·(log₂ positions − log₂ Q)·32` B).
fn choose_branch_interleave(
    a: &LigeritoSecurityConfig,
    log_n_b: usize,
    r0: usize,
    target: usize,
) -> Result<usize, LogError> {
    let dims = ladder_dims(a);
    let recursive = a.levels.len() - 1;
    let mut best: Option<(f64, usize)> = None;
    let mut refused: Option<String> = None;
    for iter in 0..recursive.saturating_sub(1) {
        let d = dims[iter + 2];
        if d >= log_n_b {
            continue;
        }
        let k = log_n_b - d;
        if k == 0 || k > 8 {
            continue;
        }
        let security = match MuLadder::level_0(r0, k, log_n_b).resolve(log_n_b, target) {
            Ok((security, _)) => security,
            Err(e) => {
                refused = Some(format!("k_B = {k}: {e}"));
                continue;
            }
        };
        let queries = security.levels[0].queries as f64;
        let positions_log = (log_n_b - k + r0) as f64;
        let rows = queries * (1u64 << k) as f64 * 16.0;
        let paths = queries * (positions_log - queries.log2()).max(0.0) * 32.0;
        let cost = rows + paths;
        if best.is_none_or(|(c, _)| cost < c) {
            best = Some((cost, k));
        }
    }
    best.map(|(_, k)| k).ok_or_else(|| {
        let refused = refused.map_or(String::new(), |why| format!("; a matching level was refused ({why})"));
        LogError::Config(format!(
            "no level of f's ladder (message sizes {dims:?}) meets the fold bits (2^{log_n_b}){refused}"
        ))
    })
}

/// Every level in the Johnson regime (each with its out-of-domain samples).
fn all_johnson(config: &LigeritoSecurityConfig) -> bool {
    config
        .levels
        .iter()
        .all(|level| matches!(level.regime, SoundnessRegime::JohnsonOod))
}

/// The merged chain's plan: `f`'s ladder up to the level where the fold
/// bits join, then the same levels with one more message variable (each
/// re-solved by the crate's per-level Johnson solver), the fold bits'
/// level-0 parameters, and the residual size.
fn merged_plan(
    a: &LigeritoSecurityConfig,
    b: &LigeritoSecurityConfig,
    target: usize,
) -> Result<MergePlan, LogError> {
    // The post-merge levels are sized by the Johnson solver.
    if !all_johnson(a) {
        return Err(LogError::Config("the merged ladder needs a Johnson ladder for f".into()));
    }
    let dims = ladder_dims(a);
    let recursive = a.levels.len() - 1;
    let joined = b.log_n - b.initial_k;
    let merge_iter = (0..recursive.saturating_sub(1))
        .find(|&iter| dims[iter + 2] == joined)
        .ok_or_else(|| LogError::Config("the fold bits meet no level of f's ladder".into()))?;
    let mut levels = a.levels[..merge_iter + 2].to_vec();
    for (index, level) in a.levels.iter().enumerate().skip(merge_iter + 2) {
        let shape = (
            level.log_msg_cols + 1,
            level.log_num_interleaved,
            level.k_recursive,
            level.log_inv_rate,
        );
        let solved = crate::ligerito_flock::solve_custom_johnson_level(level, index, shape, Some(target))
            .map_err(|e| LogError::Config(format!("merged level {index}: {e}")))?;
        levels.push(solved);
    }
    // The chain: each level's message is the previous one's folded message,
    // one variable larger from the merged level on.
    let mut dim_in = a.log_n;
    for (index, level) in levels.iter().enumerate() {
        if index == merge_iter + 2 {
            dim_in += 1;
        }
        if level.log_msg_cols + level.log_num_interleaved != dim_in {
            return Err(LogError::Config(format!("merged level {index}: shape mismatch")));
        }
        dim_in -= level.k_recursive;
    }
    let final_log_n = a.final_block.yr_log_n + 1;
    if dim_in != final_log_n {
        return Err(LogError::Config("merged ladder: residual mismatch".into()));
    }
    let merkle_hash = a.merkle_hash().map_err(LogError::Config)?;
    if b.merkle_hash().map_err(LogError::Config)? != merkle_hash {
        return Err(LogError::Config("the two commitments use different hashes".into()));
    }
    let log_inv_rates: Vec<usize> = levels.iter().map(|lv| lv.log_inv_rate).collect();
    let recursive_ks: Vec<usize> = levels.iter().skip(1).map(|lv| lv.k_recursive).collect();
    let recursive_log_msg_cols: Vec<usize> = levels.iter().skip(1).map(|lv| lv.log_msg_cols).collect();
    let queries: Vec<usize> = levels.iter().map(|lv| lv.queries).collect();
    let grinding_bits: Vec<usize> = levels.iter().map(|lv| lv.grinding_bits).collect();
    let fold_grinding_bits: Vec<usize> = levels.iter().map(|lv| lv.fold_grinding_bits).collect();
    let ood_samples: Vec<usize> = levels.iter().map(|lv| lv.ood_samples).collect();
    let prover_config = ProverConfig {
        log_inv_rates: log_inv_rates.clone(),
        recursive_steps: recursive_ks.len(),
        initial_log_msg_cols: levels[0].log_msg_cols,
        initial_log_num_interleaved: a.initial_k,
        initial_k: a.initial_k,
        recursive_log_msg_cols: recursive_log_msg_cols.clone(),
        recursive_ks: recursive_ks.clone(),
        queries: queries.clone(),
        grinding_bits: grinding_bits.clone(),
        fold_grinding_bits: fold_grinding_bits.clone(),
        ood_samples: ood_samples.clone(),
        merkle_hash,
    };
    let verifier_config = VerifierConfig {
        log_inv_rates,
        recursive_steps: recursive_ks.len(),
        initial_log_msg_cols: levels[0].log_msg_cols,
        initial_log_num_interleaved: a.initial_k,
        initial_k: a.initial_k,
        recursive_log_msg_cols,
        recursive_ks,
        queries,
        grinding_bits,
        fold_grinding_bits,
        ood_samples,
        merkle_hash,
    };
    let b0 = &b.levels[0];
    Ok(MergePlan {
        prover_config,
        verifier_config,
        branch: BranchLevel0 {
            log_inv_rate: b0.log_inv_rate,
            initial_k: b.initial_k,
            queries: b0.queries,
            grinding_bits: b0.grinding_bits,
            fold_grinding_bits: b0.fold_grinding_bits,
        },
        merge_iter,
        final_log_n,
    })
}

/// The claim `⟨eq(·, r₁) ⊗ eq(·, r₂), f⟩ = target (mod q)` by its point:
/// the verifier never expands `eq(·, r₂)`, and expands `eq(·, r₁)` only for
/// `(⋆)`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LogStatement {
    pub q: u128,
    pub r1: Vec<u128>,
    pub r2: Vec<u128>,
    pub target: u128,
}

impl LogStatement {
    fn frame(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(16 * (3 + self.r1.len() + self.r2.len()) + 16);
        out.extend_from_slice(&(self.r1.len() as u64).to_le_bytes());
        out.extend_from_slice(&(self.r2.len() as u64).to_le_bytes());
        out.extend_from_slice(&self.q.to_le_bytes());
        out.extend_from_slice(&self.target.to_le_bytes());
        for r in self.r1.iter().chain(&self.r2) {
            out.extend_from_slice(&r.to_le_bytes());
        }
        out
    }
}

/// Bytes of each part of a proof, as the prover wrote them (narg, hints).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LogSizes {
    /// The second commitment's root, its Round-0 value and `e₀`.
    pub head: usize,
    /// The first tree's GKR (over `f`).
    pub gkr_f: usize,
    /// The second tree's GKR (over the folds' bits).
    pub gkr_mu: usize,
    /// The recursive instance's folds.
    pub rec_folds: usize,
    /// The recursive instance's GKR.
    pub rec_gkr: usize,
    /// The opening of `f`: sumcheck and ring switch (narg) and Ligerito
    /// (hints). With one run, its reduction only.
    pub open_f: (usize, usize),
    /// The opening of the folds' bits, likewise. With one run, their
    /// reduction (narg) and their own tree's level-0 opening (hints).
    pub open_mu: (usize, usize),
    /// With one run, the Ligerito run both openings share.
    pub open_chain: (usize, usize),
}

impl LogSizes {
    pub fn total(&self) -> usize {
        self.head
            + self.gkr_f
            + self.gkr_mu
            + self.rec_folds
            + self.rec_gkr
            + self.open_f.0
            + self.open_f.1
            + self.open_mu.0
            + self.open_mu.1
            + self.open_chain.0
            + self.open_chain.1
    }
}

/// The pool of the scheme's small sub-protocols (the fold bits' commitment,
/// second tree, recursive instance and opening: a few MB of work each, where
/// the caller's full pool loses more to fork-join than it gains).
/// `BITZ_LOG_SMALL_THREADS` sets its size (0 = the caller's pool). Default 4:
/// at n = 28 / 30, 10 threads, those phases took 5.8 / 11.3 ms on 4 threads
/// against 7.8–9.9 / 14.6 on the caller's 10 (and 9.2 / 15.8 on 2).
#[cfg(feature = "parallel")]
fn small_pool() -> Option<&'static rayon::ThreadPool> {
    static POOL: std::sync::OnceLock<Option<rayon::ThreadPool>> = std::sync::OnceLock::new();
    POOL.get_or_init(|| {
        let threads = std::env::var("BITZ_LOG_SMALL_THREADS")
            .ok()
            .and_then(|v| v.parse::<usize>().ok())
            .unwrap_or(4)
            // Never more than the caller's pool (a 1-thread run stays one).
            .min(rayon::current_num_threads());
        (threads > 0).then(|| {
            rayon::ThreadPoolBuilder::new()
                .num_threads(threads)
                .build()
                .expect("small sub-protocol pool")
        })
    })
    .as_ref()
}

/// Runs `work` on [`small_pool`] (inline without the `parallel` feature).
fn on_small<R: Send>(work: impl FnOnce() -> R + Send) -> R {
    #[cfg(feature = "parallel")]
    if let Some(pool) = small_pool() {
        return pool.install(work);
    }
    work()
}

/// `h_k = g^{2^k}` for the live slots, `1` for the dead one.
pub fn slot_images(g: Gf) -> Vec<Gf> {
    let mut out = Vec::with_capacity(MU_SLOTS);
    let mut h = g;
    for _ in 0..MU_LIVE {
        out.push(h);
        h = h * h;
    }
    out.resize(MU_SLOTS, Gf::one());
    out
}

/// `(2^k mod q)` for the live slots, `0` for the dead one.
fn slot_weights(q: u128) -> Vec<u128> {
    let mut out = Vec::with_capacity(MU_SLOTS);
    let mut w = 1u128 % q;
    for _ in 0..MU_LIVE {
        out.push(w);
        // q < 2^126, so 2w < 2^127 fits.
        w <<= 1;
        if w >= q {
            w -= q;
        }
    }
    out.push(0);
    out
}

/// The second commitment's rows: column `c` holds the bits of folds
/// `c·2^a .. (c+1)·2^a`, one 128-bit slot each (slot 127 cleared); columns
/// past `2^{s−a}` are zero.
fn fold_bit_rows(geometry: &LogGeometry, folds: &[u128]) -> Vec<Vec<u64>> {
    let per = 1usize << geometry.a;
    let live = 1usize << (geometry.s - geometry.a);
    (0..1usize << geometry.s_com)
        .map(|c| {
            let mut row = vec![0u64; 2 * per];
            if c < live {
                for i in 0..per {
                    let m = folds[c * per + i];
                    row[2 * i] = m as u64;
                    row[2 * i + 1] = ((m >> 64) as u64) & !(1u64 << 63);
                }
            }
            row
        })
        .collect()
}

/// The recursive instance's tables: `eq(·, r₂)` mod `q` factors as
/// `A[j mod 2^a]·B[j ≫ a]` (the low `a` index bits are `r₂`'s last `a`
/// coordinates), and its row weights are `(2^k mod q)·A[j_low]` at row
/// `j_low·128 + k`.
fn recursive_weights(geometry: &LogGeometry, statement: &LogStatement) -> (Vec<u128>, Vec<u128>) {
    let arith = field::FpCtx::from_prime_u128(statement.q);
    let split = geometry.s - geometry.a;
    let low = eq_table_mod_q(&arith, &statement.r2[split..]);
    let high = eq_table_mod_q(&arith, &statement.r2[..split]);
    let slots = slot_weights(statement.q);
    let mut rows = Vec::with_capacity(low.len() * MU_SLOTS);
    for &a in &low {
        for &w in &slots {
            rows.push(arith.mul_u128(a, w));
        }
    }
    (rows, high)
}

/// `Σ_b eq(ρ, b)·eq(α, b)·(y_b − 1)`: the first tree's exit row weights
/// `(y_b − 1)·eq(b, α)` at the sumcheck's row point `ρ` — the multilinear
/// extension of `(g^{γ_b})_b` at one point, up to a known scalar and an
/// `O(t)` correction. `2^t` products with deferred reduction.
pub fn star_mle(y: &[Gf], alpha: &[Gf], rho: &[Gf]) -> Gf {
    assert_eq!(alpha.len(), rho.len());
    assert_eq!(y.len(), 1usize << alpha.len());
    let one = Gf::one();
    let factors: Vec<(Gf, Gf)> = alpha
        .iter()
        .zip(rho)
        .map(|(&a, &r)| ((one + a) * (one + r), a * r))
        .collect();
    let total = factors.iter().fold(one, |acc, &(f0, f1)| acc * (f0 + f1));
    let lo = factors.len().min(10);
    let w_lo = tensor_table(&factors[..lo]);
    let w_hi = tensor_table(&factors[lo..]);
    let block = 1usize << lo;
    let partial: Vec<Gf> = cfg_into_iter!(0..w_hi.len(), 1)
        .map(|h| {
            let zero = Gf::zero();
            let mut acc = <Gf as WideMulAcc>::wide_zero(&zero);
            let base = h * block;
            for (l, &w) in w_lo.iter().enumerate() {
                <Gf as WideMulAcc>::wide_add_assign(
                    &mut acc,
                    &<Gf as WideMulAcc>::mul_wide(&w, &y[base + l]),
                );
            }
            <Gf as WideMulAcc>::from_wide(acc) * w_hi[h]
        })
        .collect();
    partial.into_iter().fold(Gf::zero(), |acc, v| acc + v) - total
}

/// `Π_i f_i(b_i)` over `b`, coordinate `i` ↔ index bit `i`.
fn tensor_table(factors: &[(Gf, Gf)]) -> Vec<Gf> {
    let mut table = vec![Gf::one()];
    for &(f0, f1) in factors {
        let mut next = Vec::with_capacity(table.len() * 2);
        next.extend(table.iter().map(|&v| v * f0));
        next.extend(table.iter().map(|&v| v * f1));
        table = next;
    }
    table
}

/// The two claims on the folds' bits in the commitment's grid: the second
/// tree's exit (`u1` over the 128 slots, its column point over the `s` fold
/// index bits, split `a | s − a`) and the recursive exit (already in the
/// recursive grid), batched by `β`; zero columns past `2^{s−a}`.
#[allow(clippy::too_many_arguments)]
fn fold_bits_claim(
    geometry: &LogGeometry,
    tree_u1: &[Gf],
    tree_columns: &[Gf],
    tree_target: Gf,
    rec_u1: &[Gf],
    rec_u2: &[Gf],
    rec_target: Gf,
    beta: Gf,
) -> Result<SumClaimGf, LogError> {
    let a = geometry.a;
    let low = eq_table(&tree_columns[..a]);
    let high = eq_table(&tree_columns[a..]);
    let mut rows_tree = Vec::with_capacity(low.len() * tree_u1.len());
    for &e in &low {
        rows_tree.extend(tree_u1.iter().map(|&u| u * e));
    }
    let width = 1usize << geometry.s_com;
    let pad = |v: &[Gf]| {
        let mut out = v.to_vec();
        out.resize(width, Gf::zero());
        out
    };
    let rows_rec: Vec<Gf> = rec_u1.iter().map(|&u| u * beta).collect();
    SumClaimGf::from_shape(
        &geometry.commitment_shape()?,
        vec![(rows_tree, pad(&high)), (rows_rec, pad(rec_u2))],
        tree_target + beta * rec_target,
        Vec::new(),
    )
    .map_err(|e| LogError::Config(format!("fold-bits claim: {e:?}")))
}

fn bind(
    transcript: &mut impl super::transcript::PublicTranscript,
    opener: &LogOpener,
    params: &BitZParams,
    root: &[u8; 32],
    statement: &LogStatement,
) {
    transcript.public_message(STATEMENT_LABEL);
    transcript.public_message(root);
    transcript.public_message(params);
    transcript.public_message(&opener.geometry.frame());
    transcript.public_message(&opener.mu_digest);
    transcript.public_message(&opener.mode_frame());
    transcript.public_message(statement.frame().as_slice());
}

#[cfg(test)]
thread_local! {
    /// Test-only cheating provers: 1 commits `μ_0 + q` (congruent mod `q`)
    /// and claims the honest `e₀`; 2 commits and claims with `μ_0 + 1`; 3
    /// commits the honest bits and claims `e₀` with `μ_0 + 1`.
    static CHEAT: std::cell::Cell<u8> = const { std::cell::Cell::new(0) };
}

/// The live slots carry every honest fold: `(q − 1)·2^t < 2^127` (the
/// standalone prime rule `q < 2^{min(113, 126 − t)}` keeps it).
fn folds_fit_live_slots(q: u128, t: usize) -> bool {
    q.checked_sub(1)
        .and_then(|q_minus_one| q_minus_one.checked_mul(1u128 << t))
        .is_some_and(|bound| bound < 1u128 << MU_LIVE)
}

/// The prover of one claim on the committed bits of `hint`. `ood` is `f`'s
/// Round-0 claim, batched into its opening as in [`super::BitZProver::prove`].
/// The transcript binds the root, the parameters, the geometry, the fold
/// commitment's ladder and the statement; a composing caller binds `f`'s
/// ladder and Round 0 before `r₁, r₂` (as [`prove_standalone`] does).
pub fn prove(
    opener: &LogOpener,
    statement: &LogStatement,
    hint: &FlockCommitHint,
    transcript: &mut ProverState,
    ood: Option<(&[Gf], Gf)>,
) -> Result<LogSizes, LogError> {
    let geometry = opener.geometry;
    let (t, s) = (geometry.t, geometry.s);
    let shape = geometry.shape()?;
    let rows = hint.rows();
    if rows.len() != shape.columns() || rows.iter().any(|row| row.len() * 64 != shape.rows()) {
        return Err(LogError::Witness);
    }
    if statement.r1.len() != t || statement.r2.len() != s || statement.target >= statement.q {
        return Err(LogError::Config("statement does not fit the shape".into()));
    }
    if !folds_fit_live_slots(statement.q, t) {
        return Err(LogError::Config("(q − 1)·2^t reaches 2^127: the folds do not fit the live slots".into()));
    }
    if ood.is_some() != opener.base_ood.is_some() {
        return Err(LogError::Config("f's Round 0 does not match its ladder".into()));
    }
    let g = crate::piop::spartan::protocol::bitz_generator();
    let params = BitZParams::new(shape, statement.q, g)
        .map_err(|e| LogError::Config(format!("parameters: {e:?}")))?;
    let rec_shape = geometry.rec_shape()?;
    let rec_params = BitZParams::new(rec_shape, statement.q, g)
        .map_err(|e| LogError::Config(format!("recursive parameters: {e:?}")))?;
    let mut sizes = LogSizes::default();
    let mut mark = transcript.written();
    let mut take = |transcript: &ProverState| {
        let now = transcript.written();
        let delta = (now.0 - mark.0, now.1 - mark.1);
        mark = now;
        delta
    };

    let started = std::time::Instant::now();
    bind(transcript, opener, &params, hint.root(), statement);
    let comb = FixedBasePow::new(g, 128, WINDOW);
    let arith = field::FpCtx::from_prime_u128(statement.q);
    let gamma = eq_table_mod_q(&arith, &statement.r1);
    trace("log: bind+weights", started);

    // The folds, then a commitment to their bits.
    let started = std::time::Instant::now();
    let nibble = forest::nibble_rows_for(t, s, hint.packed_cols());
    let folds = match &nibble {
        Some(nibble) => nibble.column_folds(t, &gamma, shape.columns()),
        None => fold::fold_columns(&shape, rows, &gamma),
    };
    trace("log: folds", started);
    #[cfg(test)]
    let (folds, claimed_folds) = {
        let mut committed = folds.clone();
        let mut claimed = folds;
        match CHEAT.get() {
            1 => committed[0] += statement.q,
            2 => {
                committed[0] += 1;
                claimed[0] += 1;
            }
            3 => claimed[0] += 1,
            _ => {}
        }
        (committed, claimed)
    };
    #[cfg(not(test))]
    let claimed_folds = &folds;
    let started = std::time::Instant::now();
    let mu_shape = geometry.commitment_shape()?;
    let mu_rows = fold_bit_rows(&geometry, &folds);
    let (mu_root, mu_hint) = on_small(|| opener.mu_pcs.commit(&mu_shape, mu_rows))
        .map_err(|e| LogError::Config(format!("fold commitment: {e:?}")))?;
    transcript.public_message(FOLD_COMMITMENT_LABEL);
    transcript.prover_message(&mu_root.0);
    let mu_ood = match opener.mu_ood {
        Some(params) => {
            // The round's collision bound tops up to the target with
            // proof of work before `ζ`, as `f`'s Round 0 does.
            if params.grinding_bits > 0 {
                transcript.public_message(FOLD_OOD_GRINDING_LABEL);
                transcript.public_message(&params.grinding_bits.to_le_bytes());
                let seed = super::codec::gf_to_bytes(transcript.verifier_message::<Gf>());
                let nonce = super::grinding::grind(&seed, params.grinding_bits);
                transcript.prover_message(&nonce.to_le_bytes());
            }
            let zeta: Gf = transcript.verifier_message();
            let point = ood_point(zeta, opener.mu_packed_vars());
            let y = ood_eval(mu_hint.packed_message(), &point);
            transcript.prover_message(&y);
            Some((point, y))
        }
        None => None,
    };
    trace("log: commit fold bits", started);

    // The shared output point and the claimed value of both trees there.
    let started = std::time::Instant::now();
    let top = Schedule::new(opener.policy, Geometry::prefix(t, s))
        .map_err(|_| LogError::Config("top schedule".into()))?;
    transcript.start_native(top).map_err(|_| LogError::Config("top schedule".into()))?;
    let zeta = transcript.native_vector(Stage::FoldPoint, s);
    let images = fold::column_images(&comb, &claimed_folds);
    let e0 = mle_eval(&images, &zeta);
    transcript.prover_message(&e0);
    let row_images = fold::row_images(&comb, &gamma);
    trace("log: images", started);
    sizes.head = take(transcript).0;

    // The first tree, over `f`.
    let started = std::time::Instant::now();
    let fold_f = Fold {
        folds: Vec::new(),
        images: Vec::new(),
        row_images,
        zeta: zeta.clone(),
        e0,
    };
    let query_f = reduce::gkr_reduce_prove_with(transcript, &fold_f, &shape, hint, nibble)
        .map_err(|e| LogError::Config(format!("first tree: {e:?}")))?;
    transcript.end_native().map_err(|_| LogError::Config("top schedule incomplete".into()))?;
    trace("log: gkr f", started);
    sizes.gkr_f = take(transcript).0;

    // The second tree, over the folds' bits (128 slot rows, 2^s columns).
    let started = std::time::Instant::now();
    let tree_shape = geometry.tree_shape()?;
    let tree_rows: Vec<Vec<u64>> = folds
        .iter()
        .map(|&m| vec![m as u64, ((m >> 64) as u64) & !(1u64 << 63)])
        .collect();
    let tree_packed = pack_columns_from_rows(&tree_shape.layout(), &tree_rows);
    let fold_mu = Fold {
        folds: Vec::new(),
        images: Vec::new(),
        row_images: slot_images(g),
        zeta,
        e0,
    };
    transcript.set_unscheduled(true);
    let (tree_u1, tree_columns, tree_target) = on_small(|| {
        reduce::gkr_exit_prove(transcript, &fold_mu, &tree_shape, &tree_packed)
    });
    transcript.set_unscheduled(false);
    trace("log: gkr fold bits", started);
    sizes.gkr_mu = take(transcript).0;

    // The recursive instance: standard BitZ on the folds' bits.
    let started = std::time::Instant::now();
    let (rec_rows_w, rec_cols_w) = recursive_weights(&geometry, statement);
    let rec_claim = LinearClaim::new(&rec_params, rec_rows_w, rec_cols_w, statement.target)
        .map_err(|e| LogError::Config(format!("recursive claim: {e:?}")))?;
    let rec_bit_rows: Vec<Vec<u64>> = mu_hint.rows()[..rec_shape.columns()].to_vec();
    let rec = Schedule::new(opener.policy, Geometry::prefix(rec_shape.log_rows(), rec_shape.log_columns()))
        .map_err(|_| LogError::Config("recursive schedule".into()))?;
    transcript.start_native(rec).map_err(|_| LogError::Config("recursive schedule".into()))?;
    let rec_prover = BitZProver::new(rec_params, WINDOW);
    let fold_rec = on_small(|| rec_prover.send_fold(&rec_claim, &rec_bit_rows, transcript))
        .map_err(|e| LogError::Config(format!("recursive fold: {e:?}")))?;
    trace("log: rec fold", started);
    sizes.rec_folds = take(transcript).0;
    let started = std::time::Instant::now();
    let query_rec = on_small(|| {
        let rec_packed = pack_columns_from_rows(&rec_shape.layout(), &rec_bit_rows);
        reduce::gkr_reduce_prove_packed(transcript, &fold_rec, &rec_shape, &rec_packed)
    })
    .map_err(|e| LogError::Config(format!("recursive tree: {e:?}")))?;
    transcript.end_native().map_err(|_| LogError::Config("recursive schedule incomplete".into()))?;
    trace("log: rec gkr", started);
    sizes.rec_gkr = take(transcript).0;
    let OpeningQuery::InnerProduct { claim: rec_exit } = query_rec else {
        return Err(LogError::Config("recursive exit".into()));
    };

    if let Some(plan) = &opener.merge {
        // One Ligerito run for both commitments.
        let started = std::time::Instant::now();
        transcript.public_message(BATCH_LABEL);
        let beta: Gf = transcript.verifier_message();
        let claim = fold_bits_claim(
            &geometry,
            &tree_u1,
            &tree_columns,
            tree_target,
            rec_exit.row_weights(),
            rec_exit.column_weights(),
            rec_exit.target(),
            beta,
        )?;
        let pair = opener
            .base
            .pcs()
            .prove_lin_pair(
                hint,
                &query_f,
                ood,
                &opener.mu_pcs,
                &mu_hint,
                &OpeningQuery::InnerProductSum { claim },
                mu_ood.as_ref().map(|(point, y)| (point.as_slice(), *y)),
                plan,
                transcript,
            )
            .map_err(|e| LogError::Opening(format!("single run: {e:?}")))?;
        trace("log: open (one run)", started);
        // The batching challenge's label and draw write nothing.
        sizes.open_f = pair.reduce_a;
        sizes.open_mu = (pair.reduce_b.0 + pair.branch.0, pair.reduce_b.1 + pair.branch.1);
        sizes.open_chain = pair.chain;
        return Ok(sizes);
    }

    // The opening of `f`.
    let started = std::time::Instant::now();
    opener
        .base
        .pcs()
        .prove_lin_rows(
            hint,
            hint.rows(),
            hint.packed_cols(),
            &query_f,
            StatementBinding::AlreadyBound,
            transcript,
            ood,
        )
        .map_err(|e| LogError::Opening(format!("f: {e:?}")))?;
    opener.base.pcs().replay_verifier_tail(transcript);
    transcript.end_native().map_err(|_| LogError::Config("opening schedule".into()))?;
    trace("log: open f", started);
    sizes.open_f = take(transcript);

    // One opening of the folds' bits for both of their claims.
    let started = std::time::Instant::now();
    transcript.public_message(BATCH_LABEL);
    let beta: Gf = transcript.verifier_message();
    let claim = fold_bits_claim(
        &geometry,
        &tree_u1,
        &tree_columns,
        tree_target,
        rec_exit.row_weights(),
        rec_exit.column_weights(),
        rec_exit.target(),
        beta,
    )?;
    let query = OpeningQuery::InnerProductSum { claim };
    on_small(|| {
        opener.mu_pcs.prove_lin_rows(
            &mu_hint,
            mu_hint.rows(),
            mu_hint.packed_cols(),
            &query,
            StatementBinding::AlreadyBound,
            transcript,
            mu_ood.as_ref().map(|(point, y)| (point.as_slice(), *y)),
        )
    })
    .map_err(|e| LogError::Opening(format!("fold bits: {e:?}")))?;
    transcript.end_native().map_err(|_| LogError::Config("opening schedule".into()))?;
    trace("log: open fold bits", started);
    sizes.open_mu = take(transcript);
    Ok(sizes)
}

/// The verifier, consuming the transcript. Phase labels starting with
/// `v: (*)` are the note's `(⋆)` term (the top-level row weights); `v: (*')`
/// is the same term of the recursive instance.
pub fn verify(
    opener: &LogOpener,
    statement: &LogStatement,
    root: &[u8; 32],
    mut transcript: VerifierState<'_>,
    ood: Option<(&[Gf], Gf)>,
) -> Result<(), LogError> {
    let geometry = opener.geometry;
    let (t, s) = (geometry.t, geometry.s);
    let shape = geometry.shape()?;
    if statement.r1.len() != t || statement.r2.len() != s || statement.target >= statement.q {
        return Err(LogError::Config("statement does not fit the shape".into()));
    }
    if !folds_fit_live_slots(statement.q, t) {
        return Err(LogError::Config("(q − 1)·2^t reaches 2^127: the folds do not fit the live slots".into()));
    }
    if ood.is_some() != opener.base_ood.is_some() {
        return Err(LogError::Config("f's Round 0 does not match its ladder".into()));
    }
    let g = crate::piop::spartan::protocol::bitz_generator();
    let params = BitZParams::new(shape, statement.q, g)
        .map_err(|e| LogError::Config(format!("parameters: {e:?}")))?;
    let rec_shape = geometry.rec_shape()?;
    let rec_params = BitZParams::new(rec_shape, statement.q, g)
        .map_err(|e| LogError::Config(format!("recursive parameters: {e:?}")))?;

    let started = std::time::Instant::now();
    bind(&mut transcript, opener, &params, root, statement);
    let comb = FixedBasePow::new(g, 128, WINDOW);
    transcript.public_message(FOLD_COMMITMENT_LABEL);
    let mu_root: [u8; 32] = transcript.prover_message().map_err(|_| LogError::Malformed)?;
    let mu_ood = match opener.mu_ood {
        Some(params) => {
            if params.grinding_bits > 0 {
                transcript.public_message(FOLD_OOD_GRINDING_LABEL);
                transcript.public_message(&params.grinding_bits.to_le_bytes());
                let seed = super::codec::gf_to_bytes(transcript.verifier_message::<Gf>());
                let nonce = u64::from_le_bytes(
                    transcript.prover_message::<[u8; 8]>().map_err(|_| LogError::Malformed)?,
                );
                if !super::grinding::valid(&seed, nonce, params.grinding_bits) {
                    return Err(LogError::Rejected("fold commitment round 0 grinding"));
                }
            }
            let zeta: Gf = transcript.verifier_message();
            let point = ood_point(zeta, opener.mu_packed_vars());
            let y: Gf = transcript.prover_message().map_err(|_| LogError::Malformed)?;
            Some((point, y))
        }
        None => None,
    };
    let top = Schedule::new(opener.policy, Geometry::prefix(t, s))
        .map_err(|_| LogError::Config("top schedule".into()))?;
    transcript.start_native(top).map_err(|_| LogError::Config("top schedule".into()))?;
    let zeta = transcript.native_vector(Stage::FoldPoint, s).map_err(|_| LogError::Malformed)?;
    let e0: Gf = transcript.prover_message().map_err(|_| LogError::Malformed)?;
    trace("v: head", started);

    // The first tree.
    let started = std::time::Instant::now();
    let (mut point_f, claim_f) =
        gpgkr_verify(&mut transcript, e0, &zeta, t as u32).ok_or(LogError::Rejected("first tree"))?;
    transcript.end_native().map_err(|_| LogError::Malformed)?;
    let alpha_b = point_f.split_off(s);
    let alpha_c = point_f;
    trace("v: gkr f", started);

    // The second tree, from the same claim.
    let started = std::time::Instant::now();
    transcript.set_unscheduled(true);
    let (mut point_mu, claim_mu) = gpgkr_verify(&mut transcript, e0, &zeta, MU_ROW_VARS as u32)
        .ok_or(LogError::Rejected("second tree"))?;
    transcript.set_unscheduled(false);
    let slot_b = point_mu.split_off(s);
    let tree_columns = point_mu;
    let tree_u1: Vec<Gf> = slot_images(g)
        .iter()
        .zip(eq_table(&slot_b))
        .map(|(&h, e)| (h - Gf::one()) * e)
        .collect();
    trace("v: gkr fold bits", started);

    // The recursive instance.
    let started = std::time::Instant::now();
    let (rec_rows_w, rec_cols_w) = recursive_weights(&geometry, statement);
    let rec_claim = LinearClaim::new(&rec_params, rec_rows_w, rec_cols_w, statement.target)
        .map_err(|e| LogError::Config(format!("recursive claim: {e:?}")))?;
    let rec = Schedule::new(opener.policy, Geometry::prefix(rec_shape.log_rows(), rec_shape.log_columns()))
        .map_err(|_| LogError::Config("recursive schedule".into()))?;
    transcript.start_native(rec).map_err(|_| LogError::Config("recursive schedule".into()))?;
    let rec_folds = (0..rec_shape.columns())
        .map(|_| {
            transcript
                .prover_message::<[u8; 16]>()
                .map(u128::from_le_bytes)
                .map_err(|_| LogError::Malformed)
        })
        .collect::<Result<Vec<_>, _>>()?;
    if rec_folds.iter().any(|&fold| fold > rec_params.fold_bound()) {
        return Err(LogError::Rejected("recursive fold out of range"));
    }
    if fold::reconstruct(&rec_claim, &rec_folds, statement.q) != statement.target {
        return Err(LogError::Rejected("recursive reconstruction"));
    }
    let rec_zeta = transcript
        .native_vector(Stage::FoldPoint, rec_shape.log_columns())
        .map_err(|_| LogError::Malformed)?;
    let rec_images = fold::column_images(&comb, &rec_folds);
    let rec_e0 = mle_eval(&rec_images, &rec_zeta);
    trace("v: rec fold", started);
    let started = std::time::Instant::now();
    let rec_row_images = fold::row_images(&comb, rec_claim.row_exponents());
    trace("v: (*') rec row images", started);
    let started = std::time::Instant::now();
    let (rec_point, rec_claim_value) =
        gpgkr_verify(&mut transcript, rec_e0, &rec_zeta, rec_shape.log_rows() as u32)
            .ok_or(LogError::Rejected("recursive tree"))?;
    transcript.end_native().map_err(|_| LogError::Malformed)?;
    let fold_rec = Fold {
        folds: rec_folds,
        images: rec_images,
        row_images: rec_row_images,
        zeta: rec_zeta,
        e0: rec_e0,
    };
    let (rec_u1, rec_columns, rec_target) = reduce::exit_from_terminal(&fold_rec, rec_point, rec_claim_value);
    let rec_u2 = eq_table(&rec_columns);
    trace("v: rec gkr", started);

    // The opening of `f`, its row weights evaluated once at the end: (⋆).
    let started = std::time::Instant::now();
    let arith = field::FpCtx::from_prime_u128(statement.q);
    let gamma = eq_table_mod_q(&arith, &statement.r1);
    trace("v: (*) eq table r1", started);
    let started = std::time::Instant::now();
    let row_images = fold::row_images(&comb, &gamma);
    trace("v: (*) row images", started);
    if let Some(plan) = &opener.merge {
        // One Ligerito run for both commitments.
        let started = std::time::Instant::now();
        transcript.public_message(BATCH_LABEL);
        let beta: Gf = transcript.verifier_message();
        let claim = fold_bits_claim(
            &geometry,
            &tree_u1,
            &tree_columns,
            claim_mu - Gf::one(),
            &rec_u1,
            &rec_u2,
            rec_target,
            beta,
        )?;
        let mut star_time = std::time::Duration::ZERO;
        let result = opener.base.pcs().verify_lin_pair_deferred(
            &Root(*root),
            t,
            &alpha_c,
            claim_f - Gf::one(),
            |rho| {
                let at = std::time::Instant::now();
                let value = star_mle(&row_images, &alpha_b, rho);
                star_time = at.elapsed();
                value
            },
            ood,
            &opener.mu_pcs,
            &Root(mu_root),
            &OpeningQuery::InnerProductSum { claim },
            mu_ood.as_ref().map(|(point, y)| (point.as_slice(), *y)),
            plan,
            &mut transcript,
        );
        let total = started.elapsed();
        record_duration("v: (*) mle", star_time);
        record_duration("v: open (one run)", total.saturating_sub(star_time));
        result.map_err(|e| LogError::Opening(format!("single run: {e:?}")))?;
        transcript.end_native().map_err(|_| LogError::Malformed)?;
        transcript.check_eof().map_err(|_| LogError::Rejected("trailing data"))?;
        return Ok(());
    }
    let started = std::time::Instant::now();
    let mut star_time = std::time::Duration::ZERO;
    let result = opener.base.pcs().verify_lin_deferred(
        &Root(*root),
        t,
        &alpha_c,
        claim_f - Gf::one(),
        |rho| {
            let at = std::time::Instant::now();
            let value = star_mle(&row_images, &alpha_b, rho);
            star_time = at.elapsed();
            value
        },
        &mut transcript,
        ood,
    );
    let total = started.elapsed();
    record_duration("v: (*) mle", star_time);
    record_duration("v: open f", total.saturating_sub(star_time));
    result.map_err(|e| LogError::Opening(format!("f: {e:?}")))?;
    transcript.end_native().map_err(|_| LogError::Malformed)?;

    // One opening of the folds' bits.
    let started = std::time::Instant::now();
    transcript.public_message(BATCH_LABEL);
    let beta: Gf = transcript.verifier_message();
    let claim = fold_bits_claim(
        &geometry,
        &tree_u1,
        &tree_columns,
        claim_mu - Gf::one(),
        &rec_u1,
        &rec_u2,
        rec_target,
        beta,
    )?;
    opener
        .mu_pcs
        .verify_lin(
            &Root(mu_root),
            &OpeningQuery::InnerProductSum { claim },
            StatementBinding::AlreadyBound,
            &mut transcript,
            mu_ood.as_ref().map(|(point, y)| (point.as_slice(), *y)),
        )
        .map_err(|e| LogError::Opening(format!("fold bits: {e:?}")))?;
    transcript.end_native().map_err(|_| LogError::Malformed)?;
    trace("v: open fold bits", started);
    transcript.check_eof().map_err(|_| LogError::Rejected("trailing data"))?;
    Ok(())
}

/// Records a phase measured by the caller under `label` (as [`trace`] would).
fn record_duration(label: &str, elapsed: std::time::Duration) {
    super::record_phase(label, elapsed);
}

// ---------------------------------------------------------------------
// The standalone claim (the `bitz` CLI's raw claim), as BitZ's
// `prove_standalone` runs it: statement, Round 0 of `f`, prime and point,
// claim, then the scheme on a forked transcript.
// ---------------------------------------------------------------------

fn bind_standalone<T: Transcript>(
    transcript: &mut T,
    opener: &LogOpener,
    commitment: &flock_core::pcs::commit::Commitment,
    q_bits: usize,
) {
    crate::ligerito_flock::absorb_standalone_mod_q_statement(
        transcript,
        commitment,
        opener.base.layout(),
        crate::piop::spartan::protocol::bitz_generator(),
        q_bits,
        opener.base_ood,
        opener.base.pcs().verifier_config(),
    );
    transcript.absorb_slice(SESSION);
    transcript.absorb_slice(&opener.base.digest());
    transcript.absorb_slice(&opener.mu_digest);
    transcript.absorb_slice(&opener.geometry.frame());
    transcript.absorb_slice(&opener.mode_frame());
}

/// The prime and the point, drawn as the standalone instance draws them,
/// without the `eq` tables.
fn sample_statement<T: Transcript>(transcript: &mut T, layout: &IntegerMatrixLayout, q_bits: usize) -> (u128, Vec<u128>, Vec<u128>) {
    let q = crate::prime_sampling::sample_prime_with_bits(transcript, q_bits)
        .expect("bounded standalone prime search");
    let r1 = (0..layout.row_vars)
        .map(|_| crate::prime_sampling::sample_residue(transcript, q))
        .collect();
    let r2 = (0..layout.col_vars)
        .map(|_| crate::prime_sampling::sample_residue(transcript, q))
        .collect();
    (q, r1, r2)
}

/// The honest value of the standalone claim on `hint`'s bits (computed once,
/// outside any timer).
pub fn standalone_evaluation(opener: &LogOpener, hint: &FlockCommitHint) -> Result<u128, LogError> {
    let layout = *opener.base.layout();
    let q_bits = crate::ligerito_flock::standalone_q_bits(&layout);
    let mut transcript = crate::transcript::Blake3Transcript::new();
    bind_standalone(&mut transcript, opener, &hint.commitment, q_bits);
    let _ = crate::ligerito_flock::bind_prover_ood(&mut transcript, hint, opener.base_ood);
    let (q, r1, r2) = sample_statement(&mut transcript, &layout, q_bits);
    let arith = field::FpCtx::from_prime_u128(q);
    let shape = opener.geometry.shape()?;
    let gamma = eq_table_mod_q(&arith, &r1);
    let u2 = eq_table_mod_q(&arith, &r2);
    let folds = fold::fold_columns(&shape, hint.rows(), &gamma);
    let params = BitZParams::new(shape, q, crate::piop::spartan::protocol::bitz_generator())
        .map_err(|e| LogError::Config(format!("parameters: {e:?}")))?;
    let claim = LinearClaim::new(&params, gamma, u2, 0)
        .map_err(|e| LogError::Config(format!("claim: {e:?}")))?;
    Ok(fold::reconstruct(&claim, &folds, q))
}

/// Proves the standalone claim `⟨eq(·, r₁) ⊗ eq(·, r₂), f⟩ = claimed` with
/// the logarithmic scheme; returns the proof (BitZ's proof container: the
/// forked transcript and `f`'s Round 0) and the prover's size accounting.
pub fn prove_standalone(
    opener: &LogOpener,
    hint: &FlockCommitHint,
    claimed: u128,
) -> Result<(BitZOpeningProof, LogSizes), LogError> {
    let layout = *opener.base.layout();
    crate::piop::spartan::protocol::validate_bit_rows(&layout, hint.rows())
        .map_err(|_| LogError::Witness)?;
    let q_bits = crate::ligerito_flock::standalone_q_bits(&layout);
    let mut transcript = crate::transcript::Blake3Transcript::new();
    bind_standalone(&mut transcript, opener, &hint.commitment, q_bits);
    let ood = crate::ligerito_flock::bind_prover_ood(&mut transcript, hint, opener.base_ood)
        .opening_claim(&mut transcript, hint);
    let (q, r1, r2) = sample_statement(&mut transcript, &layout, q_bits);
    if claimed >= q {
        return Err(LogError::Config("claimed value not reduced".into()));
    }
    crate::ligerito_flock::absorb_standalone_mod_q_claim(&mut transcript, q, claimed);
    let tag = crate::piop::spartan::protocol::bitz_opener::fork_tag(&mut transcript);
    let mut state = build_prover(SESSION, &tag);
    let statement = LogStatement { q, r1, r2, target: claimed };
    let sizes = prove(
        opener,
        &statement,
        hint,
        &mut state,
        ood.as_ref().map(|claim| (claim.point.as_slice(), claim.y)),
    )?;
    Ok((
        BitZOpeningProof {
            transcript: state.finish(),
            ood: ood.map(|claim| claim.round),
        },
        sizes,
    ))
}

/// Verifies a [`prove_standalone`] proof of `claimed` against `commitment`.
pub fn verify_standalone(
    opener: &LogOpener,
    commitment: &flock_core::pcs::commit::Commitment,
    claimed: u128,
    proof: &BitZOpeningProof,
) -> Result<(), LogError> {
    let layout = *opener.base.layout();
    crate::piop::spartan::protocol::validate_commitment(&layout, commitment, opener.base.pcs().prover_config())
        .map_err(|e| LogError::Config(format!("commitment: {e:?}")))?;
    let started = std::time::Instant::now();
    let q_bits = crate::ligerito_flock::standalone_q_bits(&layout);
    let packed_vars = layout.row_vars + layout.col_vars - MU_ROW_VARS;
    let mut transcript = crate::transcript::Blake3Transcript::new();
    bind_standalone(&mut transcript, opener, commitment, q_bits);
    let ood = crate::ligerito_flock::bind_verifier_ood(
        &mut transcript,
        packed_vars,
        opener.base_ood,
        proof.ood.as_ref(),
    )
    .map_err(|_| LogError::Rejected("round 0"))?
    .opening_claim(&mut transcript, packed_vars, proof.ood.as_ref())
    .map_err(|_| LogError::Rejected("round 0"))?;
    let (q, r1, r2) = sample_statement(&mut transcript, &layout, q_bits);
    if claimed >= q {
        return Err(LogError::Rejected("claimed value not reduced"));
    }
    crate::ligerito_flock::absorb_standalone_mod_q_claim(&mut transcript, q, claimed);
    let tag = crate::piop::spartan::protocol::bitz_opener::fork_tag(&mut transcript);
    trace("v: statement", started);
    let statement = LogStatement { q, r1, r2, target: claimed };
    verify(
        opener,
        &statement,
        &commitment.root,
        build_verifier(SESSION, &tag, &proof.transcript),
        ood.as_ref().map(|claim| (claim.point.as_slice(), claim.y)),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn xorshift(state: &mut u64) -> u64 {
        *state ^= *state << 13;
        *state ^= *state >> 7;
        *state ^= *state << 17;
        *state
    }

    fn random_gf(state: &mut u64) -> Gf {
        Gf::from_polynomial_words([xorshift(state), xorshift(state)])
    }

    #[test]
    fn star_mle_matches_the_folded_row_weights() {
        let mut state = 0x1234_5678_9abc_def1u64;
        for t in [1usize, 3, 7, 11, 12] {
            let y: Vec<Gf> = (0..1 << t).map(|_| random_gf(&mut state)).collect();
            let alpha: Vec<Gf> = (0..t).map(|_| random_gf(&mut state)).collect();
            let rho: Vec<Gf> = (0..t).map(|_| random_gf(&mut state)).collect();
            let u1: Vec<Gf> = y
                .iter()
                .zip(eq_table(&alpha))
                .map(|(&v, e)| (v - Gf::one()) * e)
                .collect();
            assert_eq!(star_mle(&y, &alpha, &rho), mle_eval(&u1, &rho), "t = {t}");
        }
    }

    #[test]
    fn recursive_weights_factor_the_column_table() {
        let q = (1u128 << 100) - 15;
        let arith = field::FpCtx::from_prime_u128(q);
        let mut state = 7u64;
        for (s, a) in [(9usize, 0usize), (9, 2), (12, 4), (5, 5)] {
            let r2: Vec<u128> = (0..s).map(|_| (u128::from(xorshift(&mut state)) << 30) % q).collect();
            let geometry = LogGeometry { t: 20, s, a, s_com: s - a };
            let statement = LogStatement { q, r1: Vec::new(), r2: r2.clone(), target: 0 };
            let (rows, cols) = recursive_weights(&geometry, &statement);
            let full = eq_table_mod_q(&arith, &r2);
            let slots = slot_weights(q);
            for j in 0..1usize << s {
                let (low, high) = (j & ((1 << a) - 1), j >> a);
                for k in 0..MU_SLOTS {
                    let want = arith.mul_u128(full[j], slots[k]);
                    let got = arith.mul_u128(rows[low * MU_SLOTS + k], cols[high]);
                    assert_eq!(got, want, "s {s} a {a} j {j} k {k}");
                }
            }
        }
    }

    thread_local! {
        /// Whether `instance` builds single-run openers (`lad:` ladders).
        static SINGLE_RUN: std::cell::Cell<bool> = const { std::cell::Cell::new(true) };
    }

    fn instance(n: usize, a: Option<usize>, mu: &str) -> (LogOpener, FlockCommitHint) {
        instance_at(Shape::reference(n).unwrap(), a, mu)
    }

    fn instance_at(shape: Shape, a: Option<usize>, mu: &str) -> (LogOpener, FlockCommitHint) {
        let n = shape.log_bits();
        let opener = LogOpener::new(
            shape.layout(),
            BitZLigerito::parse("custom:1:4", 100).unwrap(),
            MuLadder::parse(mu, 100).unwrap(),
            a,
            None,
            mu.starts_with("lad") && SINGLE_RUN.get(),
            100,
        )
        .unwrap();
        let mut state = 0x9E37_79B9_7F4A_7C15u64 ^ n as u64;
        let words = shape.rows() / 64;
        let rows = (0..shape.columns())
            .map(|_| (0..words).map(|_| xorshift(&mut state)).collect())
            .collect();
        let hint = opener.commit(rows).unwrap();
        (opener, hint)
    }

    #[test]
    #[ignore]
    fn print_fold_ladders() {
        for m in 20..=23usize {
            for r in 1..=4usize {
                for k in 1..=5usize {
                    let name = format!("custom:{r}:{k}");
                    let sel = LigeritoSelection::parse(&name, 100).unwrap();
                    match sel.resolve(m - 7, 100) {
                        Ok(res) => {
                            let q: Vec<usize> = res.security().levels.iter().map(|l| l.queries).collect();
                            println!("m {m} {name}: ok, queries {q:?}");
                        }
                        Err(e) => println!("m {m} {name}: {e}"),
                    }
                }
            }
        }
    }

    /// Bytes of every part of both Ligerito proofs (`LOG_BREAKDOWN_N`,
    /// optional `LOG_BREAKDOWN_T`): `cargo test … proof_breakdown -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn proof_breakdown() {
        use bincode::Options;
        use flock_core::pcs::ligerito::LigeritoProof;
        let n: usize = std::env::var("LOG_BREAKDOWN_N").ok().and_then(|v| v.parse().ok()).unwrap_or(24);
        let shape = match std::env::var("LOG_BREAKDOWN_T").ok().and_then(|v| v.parse::<usize>().ok()) {
            Some(t) => Shape::new(t, n - t).unwrap(),
            None => Shape::reference(n).unwrap(),
        };
        let (opener, hint) = instance_at(shape, None, "lad:5:3:3:7");
        let claimed = standalone_evaluation(&opener, &hint).unwrap();
        let (proof, sizes) = prove_standalone(&opener, &hint, claimed).unwrap();
        let opts = bincode::DefaultOptions::new().with_fixint_encoding();
        let mut rest: &[u8] = &proof.transcript.hints;
        let names: &[&str] = if opener.merge_plan().is_some() { &["one run"] } else { &["f", "fold bits"] };
        for &which in names {
            let len = u32::from_le_bytes(rest[..4].try_into().unwrap()) as usize;
            let p: LigeritoProof = opts.deserialize(&rest[4..4 + len]).unwrap();
            rest = &rest[4 + len..];
            let size = |v: &dyn erased::Ser| v.bytes(&opts);
            let level = |rows: &Vec<Vec<Gf>>, merkle: &Vec<flock_core::merkle::Hash>| {
                let elems: usize = rows.iter().map(|r| r.len()).sum();
                (rows.len(), elems * 16, merkle.len() * 32)
            };
            let (q0, r0, m0) = level(&p.initial_proof.opened_rows, &p.initial_proof.merkle_proof);
            println!("{which}: total {len} B; level 0: {q0} queries, rows {r0} B, merkle {m0} B");
            for (i, rp) in p.recursive_proofs.iter().enumerate() {
                let (q, r, m) = level(&rp.opened_rows, &rp.merkle_proof);
                println!("{which}: level {}: {q} queries, rows {r} B, merkle {m} B", i + 1);
            }
            let (qf, rf, mf) = level(&p.final_proof.opened_rows, &p.final_proof.merkle_proof);
            println!(
                "{which}: last level: {qf} queries, rows {rf} B, merkle {mf} B; yr {} B; roots {} B; sumcheck {} msgs; ood values {} B",
                p.final_proof.yr.len() * 16,
                32 * (1 + p.recursive_roots.len()),
                p.sumcheck_transcript.len(),
                size(&p.ood_values),
            );
        }
        if !rest.is_empty() {
            let len = u32::from_le_bytes(rest[..4].try_into().unwrap()) as usize;
            let b: flock_core::pcs::ligerito::RecursiveProof = opts.deserialize(&rest[4..4 + len]).unwrap();
            let elems: usize = b.opened_rows.iter().map(|r| r.len()).sum();
            println!(
                "fold bits level 0 (branch): {len} B; {} queries, rows {} B, merkle {} B",
                b.opened_rows.len(),
                elems * 16,
                b.merkle_proof.len() * 32
            );
        }
        println!("narg parts: {sizes:?}");
    }

    mod erased {
        pub trait Ser {
            fn bytes(&self, opts: &bincode::config::WithOtherIntEncoding<bincode::DefaultOptions, bincode::config::FixintEncoding>) -> usize;
        }
        impl<T: serde::Serialize> Ser for T {
            fn bytes(&self, opts: &bincode::config::WithOtherIntEncoding<bincode::DefaultOptions, bincode::config::FixintEncoding>) -> usize {
                use bincode::Options;
                opts.serialize(self).map(|v| v.len()).unwrap_or(0)
            }
        }
    }

    #[test]
    fn standalone_claims_prove_and_verify() {
        // The `lad:` instances run both in one run and in two.
        for (n, a, mu, single_run) in [
            (20usize, None, "custom:2:4", true),
            (22, None, "custom:3:3", true),
            (22, Some(0), "custom:2:4", true),
            (22, None, "lad:4:2:3:6", true),
            (22, None, "lad:4:2:3:6", false),
            (23, Some(1), "lad:5:1:2:4", true),
            (23, Some(1), "lad:5:1:2:4", false),
        ] {
            SINGLE_RUN.set(single_run);
            let (opener, hint) = instance(n, a, mu);
            assert_eq!(opener.merge_plan().is_some(), single_run && mu.starts_with("lad"), "n {n} {mu}");
            let claimed = standalone_evaluation(&opener, &hint).unwrap();
            let (proof, sizes) = prove_standalone(&opener, &hint, claimed).unwrap();
            assert_eq!(sizes.total(), proof.transcript.narg_string.len() + proof.transcript.hints.len(), "n {n}");
            verify_standalone(&opener, &hint.commitment, claimed, &proof).unwrap();
            assert!(verify_standalone(&opener, &hint.commitment, claimed + 1, &proof).is_err());
            // Every part of the narg string and the hints is checked.
            let narg = proof.transcript.narg_string.len();
            let mut offset = 0usize;
            for (part, len) in [
                ("head", sizes.head),
                ("gkr f", sizes.gkr_f),
                ("gkr mu", sizes.gkr_mu),
                ("rec folds", sizes.rec_folds),
                ("rec gkr", sizes.rec_gkr),
                ("open f", sizes.open_f.0),
                ("open mu", sizes.open_mu.0),
                ("open chain", sizes.open_chain.0),
            ] {
                if len > 0 {
                    let mut bad = proof.clone();
                    bad.transcript.narg_string[offset + len / 2] ^= 0x10;
                    assert!(verify_standalone(&opener, &hint.commitment, claimed, &bad).is_err(), "n {n} part {part}");
                }
                offset += len;
            }
            assert_eq!(offset, narg);
            let hints = proof.transcript.hints.len();
            for at in [7usize, (sizes.open_f.1 + sizes.open_chain.1) / 2, hints - 40, hints - 3] {
                let mut bad = proof.clone();
                bad.transcript.hints[at] ^= 0x01;
                assert!(verify_standalone(&opener, &hint.commitment, claimed, &bad).is_err(), "n {n} hint {at}");
            }
        }
        SINGLE_RUN.set(true);
    }

    #[test]
    fn a_large_fold_commitment_grinds_its_round_0() {
        // t = 11, s = 15: the folds' bits are 2^22 bits, whose Round 0 at
        // rate 1/32 falls a few bits short of 100 and is ground.
        let shape = Shape::new(11, 15).unwrap();
        let opener = LogOpener::new(
            shape.layout(),
            BitZLigerito::parse("custom:1:4", 100).unwrap(),
            MuLadder::parse("lad:5:3:3:7", 100).unwrap(),
            None,
            None,
            false,
            100,
        )
        .unwrap();
        assert!(opener.mu_round_0_grinding() > 0, "expected a ground Round 0");
        let mut state = 0xDEAD_BEEF_u64;
        let rows = (0..shape.columns())
            .map(|_| (0..shape.rows() / 64).map(|_| xorshift(&mut state)).collect())
            .collect();
        let hint = opener.commit(rows).unwrap();
        let claimed = standalone_evaluation(&opener, &hint).unwrap();
        let (proof, sizes) = prove_standalone(&opener, &hint, claimed).unwrap();
        verify_standalone(&opener, &hint.commitment, claimed, &proof).unwrap();
        // The nonce sits right after the second root: flip it.
        let mut bad = proof.clone();
        bad.transcript.narg_string[32] ^= 1;
        assert!(verify_standalone(&opener, &hint.commitment, claimed, &bad).is_err());
        assert!(sizes.head >= 32 + 8 + 16 + 16);
    }

    #[test]
    fn edge_geometries_prove_and_verify() {
        // s' = 0 (every column variable in the recursive rows), a = 0, and a
        // tiny top-level column count.
        for (t, s, a) in [(14usize, 6usize, Some(6usize)), (13, 9, Some(0)), (17, 3, None)] {
            let (opener, hint) = instance_at(Shape::new(t, s).unwrap(), a, "lad:5:3:3:7");
            let claimed = standalone_evaluation(&opener, &hint).unwrap();
            let (proof, _) = prove_standalone(&opener, &hint, claimed).unwrap();
            verify_standalone(&opener, &hint.commitment, claimed, &proof)
                .unwrap_or_else(|e| panic!("t {t} s {s} a {a:?}: {e:?}"));
        }
    }

    #[test]
    fn one_run_and_two_runs_both_verify_and_one_run_is_smaller() {
        for n in [22usize, 24] {
            SINGLE_RUN.set(false);
            let (two, hint) = instance(n, None, "lad:5:3:3:7");
            SINGLE_RUN.set(true);
            let (one, _) = instance(n, None, "lad:5:3:3:7");
            assert!(one.merge_plan().is_some() && two.merge_plan().is_none());
            let claimed_two = standalone_evaluation(&two, &hint).unwrap();
            let claimed_one = standalone_evaluation(&one, &hint).unwrap();
            let (proof_two, _) = prove_standalone(&two, &hint, claimed_two).unwrap();
            let (proof_one, _) = prove_standalone(&one, &hint, claimed_one).unwrap();
            verify_standalone(&two, &hint.commitment, claimed_two, &proof_two).unwrap();
            verify_standalone(&one, &hint.commitment, claimed_one, &proof_one).unwrap();
            // The modes are bound: neither verifier takes the other's proof.
            assert!(verify_standalone(&one, &hint.commitment, claimed_two, &proof_two).is_err());
            assert!(verify_standalone(&two, &hint.commitment, claimed_one, &proof_one).is_err());
            assert!(
                proof_one.to_bytes().len() < proof_two.to_bytes().len(),
                "n {n}: one run {} B, two runs {} B",
                proof_one.to_bytes().len(),
                proof_two.to_bytes().len()
            );
        }
    }

    #[test]
    fn single_run_pads_small_fold_commitments_and_refuses_udr() {
        // t = 17, s = 7 at n = 24: the fold bits' natural 2^7 packed message
        // meets no in-loop level of f's ladder; the opener pads it.
        let shape = Shape::new(17, 7).unwrap();
        let (opener, hint) = instance_at(shape, None, "lad:5:3:3:7");
        assert!(opener.merge_plan().is_some());
        let claimed = standalone_evaluation(&opener, &hint).unwrap();
        let (proof, _) = prove_standalone(&opener, &hint, claimed).unwrap();
        verify_standalone(&opener, &hint.commitment, claimed, &proof).unwrap();
        let udr = LogOpener::new(
            Shape::reference(22).unwrap().layout(),
            BitZLigerito::parse("udr:1:4", 100).unwrap(),
            MuLadder::parse("lad:5:3:3:7", 100).unwrap(),
            None,
            None,
            true,
            100,
        );
        assert!(matches!(udr, Err(LogError::Config(_))));
    }

    #[test]
    fn a_false_claim_is_rejected() {
        let (opener, hint) = instance(20, None, "lad:5:3:3:7");
        let claimed = standalone_evaluation(&opener, &hint).unwrap();
        // The honest folds reconstruct the true value, so the recursive
        // instance's reconstruction check fails for any other target.
        let wrong = claimed ^ 1;
        if let Ok((proof, _)) = prove_standalone(&opener, &hint, wrong) {
            assert_eq!(
                verify_standalone(&opener, &hint.commitment, wrong, &proof),
                Err(LogError::Rejected("recursive reconstruction"))
            );
        }
    }

    #[test]
    fn cheating_fold_bits_are_rejected() {
        let (opener, hint) = instance(20, None, "custom:2:4");
        let claimed = standalone_evaluation(&opener, &hint).unwrap();
        for mode in [1u8, 2, 3] {
            CHEAT.set(mode);
            let proved = prove_standalone(&opener, &hint, claimed);
            CHEAT.set(0);
            // The cheating prover completes (it never checks `e₀`); the
            // verifier rejects in the tree whose output disagrees.
            let (proof, _) = proved.expect("the cheating prover completes");
            let verdict = verify_standalone(&opener, &hint.commitment, claimed, &proof);
            let expected = match mode {
                1 => LogError::Rejected("second tree"),
                _ => LogError::Rejected("first tree"),
            };
            assert_eq!(verdict, Err(expected), "cheat {mode}");
        }
    }

    #[test]
    fn slot_images_are_powers_and_the_top_slot_is_dead() {
        let g = crate::piop::spartan::protocol::bitz_generator();
        let h = slot_images(g);
        assert_eq!(h.len(), MU_SLOTS);
        assert_eq!(h[MU_LIVE], Gf::one());
        let comb = FixedBasePow::new(g, 128, WINDOW);
        for k in [0usize, 1, 5, 63, 64, 100, 126] {
            assert_eq!(h[k], comb.pow(1u128 << k), "k {k}");
        }
    }
}

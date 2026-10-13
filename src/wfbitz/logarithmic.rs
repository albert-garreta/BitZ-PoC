//! BitZ logarithmic on a block-layout commitment (proof of concept): the
//! scheme of `paper/notes/bitz_logarithmic.tex` with one recursive call,
//! as a drop-in for [`super::BitZProver::prove_block`].
//!
//! Standard BitZ sends the `2^s` integer column folds `μ_j = Σ_i w_i·h_ij`
//! and the verifier checks `Σ_j v_j·π_q(μ_j) = target` and exponentiates
//! every fold for the grand products. Here the prover commits to the bits
//! of the folds instead, and
//!
//! 1. a second batched product tree over those bits,
//!    `Π_k ((h_k − 1)·μ_{jk} + 1) = g^{μ_j}` with `h_k = g^{2^k}`, meets the
//!    first one (`Π_i ((g^{w_i} − 1)·h_ij + 1)`) at one random point `ζ` of
//!    the shared output layer: the prover sends `e₀ = MLE[g^μ](ζ)` and both
//!    GKRs start from it;
//! 2. the recombination `Σ_j v_j·π_q(μ_j) = target` is proved by one
//!    standard (recursive) BitZ instance over the committed bits, with
//!    weights `(2^k mod q)·v_j` (tensor-split again: the column weights of
//!    the block claim are an `eq` table over the high instance bits, so they
//!    factor over any split of the fold index);
//! 3. the first tree's exit claim on the derived grid is transposed through
//!    the block map and opened on the commitment as before, and the two
//!    claims on the folds' bits (the second tree's exit, the recursive
//!    instance's exit) are batched into one opening of the second
//!    commitment (a second Ligerito run; the merged run is not ported yet).
//!
//! Only 127 of the 128 bit slots of a fold carry weight (`h_127 = 1`, its
//! recursive weight zero), so a committed exponent stays below
//! `2^127 < ord(g)` and `g^{μ_j}` determines `μ_j`: no range check.
//!
//! The verifier never reads a column fold. What remains linear in a
//! dimension is `(⋆)`: the `2^t` row images `g^{w_i}` the first tree's exit
//! claim needs (the standard opener computes them too), plus the recursive
//! instance's own `O(2^{t'} + 2^{s'})` work. Every phase is traced.

use circuit::linear_map::binary::VirtualMap;
use flock_core::pcs::ligerito::{LigeritoProfile, LigeritoSecurityConfig, SoundnessRegime};
use spongefish::Encoding;

use super::block::BlockStatement;
use super::fold::{self, Fold};
use super::forest;
use super::gkr::eq_table;
use super::params::{BitZParams, LinearClaim, Root, Shape, SumClaimGf, reference_log_rows};
use super::pcs::{MergePlan, OpeningQuery, Pcs, StatementBinding};
use super::reduce;
use super::transcript::{ProverState, VerifierState};
use super::{BitZProver, BitZVerifier, FixedBasePow, WINDOW, trace};
use crate::ligerito::{mle_eval, pack_columns_from_rows};
use crate::ligerito_flock::{
    FlockCommitHint, LigeritoSelection, OodRoundParams, ood_eval, ood_point, ood_round_params,
};
use crate::pcs::IntegerMatrixLayout;
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
/// The inner transcript's frame labels.
const STATEMENT_LABEL: &[u8] = b"bitz/log-block-statement/v1";
const FOLD_COMMITMENT_LABEL: &[u8] = b"bitz/log/fold-commitment/v1";
const BATCH_LABEL: &[u8] = b"bitz/log/fold-opening-batch/v1";
const FOLD_OOD_GRINDING_LABEL: &[u8] = b"bitz/log/fold-round0-grinding/v1";

/// What fails in the logarithmic scheme.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LogError {
    /// The geometry or a parameter gate rejects the instance.
    Config(String),
    /// The derived rows are not the claim's shape.
    Witness,
    /// A proof message is missing or malformed.
    Malformed,
    /// A check failed.
    Rejected(&'static str),
    /// The opening of `h` or of the folds' bits failed.
    Opening(String),
}

/// The recursion's geometry: the claim grid `2^t × 2^s`, the second tree's
/// `2^7 × 2^s` view of the folds' bits, the recursive instance's
/// `2^{7+a} × 2^{s−a}` view of the same bits (the low `a` fold-index bits
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
    /// `t`; the second commitment is padded with zero columns to at least
    /// `2^min_log_bits` bits.
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

    pub fn name(&self) -> String {
        match self {
            Self::Selection(selection) => selection.name(),
            Self::Ladder { r0, k0, k_rec, final_log } => format!("lad:{r0}:{k0}:{k_rec}:{final_log}"),
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

/// The opener of the folds' bits: their commitment's ladder, Round 0 and
/// PCS, and the recursion's geometry for one claim grid. The committed
/// vector `h` is opened by the caller's own [`Pcs`] (the block opener's).
#[derive(Clone, Debug)]
pub struct LogOpener {
    geometry: LogGeometry,
    mu_pcs: Pcs,
    mu_security: LigeritoSecurityConfig,
    mu_ladder: MuLadder,
    mu_digest: [u8; 32],
    mu_ood: Option<OodRoundParams>,
    target_bits: usize,
    /// The single-run opening (the fold bits join `h`'s Ligerito ladder);
    /// `None` opens the two commitments in two runs.
    merge: Option<MergePlan>,
}

impl LogOpener {
    /// `derived` is the claim grid (`t × s`), `mu_ladder` the second
    /// commitment's ladder (resolved at its size), `a` the recursive split
    /// and `min_mu_log_bits` the second commitment's floor (see
    /// [`LogGeometry::new`]).
    /// `f_security` is `h`'s (the caller's) Ligerito ladder when the two
    /// commitments are to be opened in ONE run: the fold bits' interleaving
    /// is then chosen so their folded message meets a level of that ladder
    /// (cheapest level-0 opening), padding them with zero columns (a larger
    /// floor) when no level fits their natural size; `None` opens them in
    /// two runs under `mu_ladder`.
    pub fn new(
        derived: Shape,
        mu_ladder: MuLadder,
        a: Option<usize>,
        min_mu_log_bits: Option<usize>,
        f_security: Option<&LigeritoSecurityConfig>,
        target_bits: usize,
    ) -> Result<Self, LogError> {
        let floor = min_mu_log_bits.unwrap_or_else(|| mu_ladder.default_min_log_bits());
        let mut geometry = LogGeometry::new(derived.log_rows(), derived.log_columns(), a, floor)?;
        let mu_ladder = if let Some(f_security) = f_security {
            let MuLadder::Ladder { r0, .. } = mu_ladder else {
                return Err(LogError::Config("a single run needs an explicit lad: ladder".into()));
            };
            if !all_johnson(f_security) {
                return Err(LogError::Config("a single run needs a Johnson ladder for h".into()));
            }
            let mut chosen = None;
            let mut refusal = None;
            for extra in 0..=6 {
                let candidate = LogGeometry::new(derived.log_rows(), derived.log_columns(), a, floor + extra)?;
                if extra > 0 && candidate == geometry {
                    continue;
                }
                let packed = candidate.commitment_shape()?.log_packed_len();
                match choose_branch_interleave(f_security, packed, r0, target_bits) {
                    Ok(k0) => {
                        chosen = Some((candidate, MuLadder::level_0(r0, k0, packed)));
                        break;
                    }
                    // A resolver's reason outranks a size mismatch; among
                    // equals the natural size's error is kept.
                    Err(e) => {
                        let specific = |e: &LogError| matches!(e, LogError::Config(m) if m.contains("was refused"));
                        if refusal.as_ref().is_none_or(|kept| !specific(kept) && specific(&e)) {
                            refusal = Some(e);
                        }
                    }
                }
            }
            let Some((candidate, ladder)) = chosen else {
                return Err(refusal.unwrap_or_else(|| {
                    LogError::Config("no level of h's ladder meets the fold bits".into())
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
        let mu_pcs = Pcs::with_security(&mu_shape, &mu_security, LigeritoProfile::Fast)
            .map_err(|e| LogError::Config(format!("fold commitment pcs: {e:?}")))?;
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
        if mu_ood.is_some_and(|params| params.grinding_bits > 24) {
            return Err(LogError::Config("the fold commitment's Round 0 needs too much grinding".into()));
        }
        let merge = match f_security {
            Some(f_security) => Some(merged_plan(f_security, &mu_security, target_bits)?),
            None => None,
        };
        Ok(Self {
            geometry,
            mu_pcs,
            mu_security,
            mu_ladder,
            mu_digest,
            mu_ood,
            target_bits,
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

    pub fn geometry(&self) -> &LogGeometry {
        &self.geometry
    }

    pub fn mu_security(&self) -> &LigeritoSecurityConfig {
        &self.mu_security
    }

    pub fn mu_ladder(&self) -> &MuLadder {
        &self.mu_ladder
    }

    pub fn target_bits(&self) -> usize {
        self.target_bits
    }

    /// Whether the second commitment runs its Round 0 (OOD sample).
    pub fn mu_round_0(&self) -> bool {
        self.mu_ood.is_some()
    }

    /// The proof-of-work bits of the second commitment's Round 0.
    pub fn mu_round_0_grinding(&self) -> u32 {
        self.mu_ood.map_or(0, |params| params.grinding_bits)
    }

    /// What the outer statement binds before the fork: the geometry, the
    /// second commitment's ladder and the opening mode (two runs).
    pub fn digest(&self) -> [u8; 32] {
        let mut h = blake3::Hasher::new();
        h.update(b"bitz/log-block-opener/v1");
        h.update(&self.geometry.frame());
        h.update(&self.mu_digest);
        h.update(&self.mode_frame());
        *h.finalize().as_bytes()
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
/// message (`log_n_b − k` variables) must equal `h`'s message after one of
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
            "no level of h's ladder (message sizes {dims:?}) meets the fold bits (2^{log_n_b}){refused}"
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

/// The merged chain's plan: `h`'s ladder up to the level where the fold
/// bits join, then the same levels with one more message variable (each
/// re-solved by the crate's per-level Johnson solver), the fold bits'
/// level-0 parameters, and the residual size.
fn merged_plan(
    a: &LigeritoSecurityConfig,
    b: &LigeritoSecurityConfig,
    target: usize,
) -> Result<MergePlan, LogError> {
    use flock_core::pcs::ligerito::{BranchLevel0, ProverConfig, VerifierConfig};
    // The post-merge levels are sized by the Johnson solver.
    if !all_johnson(a) {
        return Err(LogError::Config("the merged ladder needs a Johnson ladder for h".into()));
    }
    let dims = ladder_dims(a);
    let recursive = a.levels.len() - 1;
    let joined = b.log_n - b.initial_k;
    let merge_iter = (0..recursive.saturating_sub(1))
        .find(|&iter| dims[iter + 2] == joined)
        .ok_or_else(|| LogError::Config("the fold bits meet no level of h's ladder".into()))?;
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

/// Bytes of each part of a proof, as the prover wrote them (narg, hints).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LogSizes {
    /// The second commitment's root, its Round-0 value and `e₀`.
    pub head: usize,
    /// The first tree's GKR (over `h`).
    pub gkr_f: usize,
    /// The second tree's GKR (over the folds' bits).
    pub gkr_mu: usize,
    /// The recursive instance's folds.
    pub rec_folds: usize,
    /// The recursive instance's GKR.
    pub rec_gkr: usize,
    /// The opening of `h` on the commitment: sumcheck and ring switch
    /// (narg) and Ligerito (hints).
    pub open_f: (usize, usize),
    /// The opening of the folds' bits, likewise (in one run: its reduction
    /// and its level-0 opening).
    pub open_mu: (usize, usize),
    /// In one run: the merged Ligerito chain (both commitments' folds).
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

/// The sizes of the last [`prove_block_log`] proof on this process, for a
/// bench to read back (the opener's API returns only the proof).
static LAST_SIZES: std::sync::Mutex<Option<LogSizes>> = std::sync::Mutex::new(None);

/// Takes the sizes of the last [`prove_block_log`] proof, if any.
pub fn take_last_sizes() -> Option<LogSizes> {
    LAST_SIZES.lock().ok().and_then(|mut slot| slot.take())
}

/// The pool of the scheme's small sub-protocols (the fold bits' commitment,
/// second tree, recursive instance and opening: a few MB of work each, where
/// the caller's full pool loses more to fork-join than it gains).
/// `BITZ_LOG_SMALL_THREADS` sets its size (0 = the caller's pool). Default 4.
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

/// The column weights `v` factored over the split `a | s − a` of the fold
/// index, `v[l + h·2^a]·v[0] = A[l]·B[h]` with `A[l] = v[l]` and
/// `B[h] = v[h·2^a]`: exact when `v` is a tensor over that split (an `eq`
/// table is, over any split), rejected otherwise. The recursive instance
/// then proves `Σ A[l]·B[h]·π_q(μ_{l+h·2^a}) = target·v[0]`.
fn column_tensor(cols: &[u128], q: u128, a: usize) -> Result<(Vec<u128>, Vec<u128>, u128), LogError> {
    let arith = field::FpCtx::from_prime_u128(q);
    let per = 1usize << a;
    if cols.is_empty() || cols.len() % per != 0 {
        return Err(LogError::Config("column weights do not split".into()));
    }
    if cols[0] == 0 {
        return Err(LogError::Config("column weights vanish at index 0: no tensor split".into()));
    }
    let low: Vec<u128> = cols[..per].to_vec();
    let high: Vec<u128> = cols.iter().step_by(per).copied().collect();
    let scale = arith.prepare_multiplier_u128(cols[0]);
    for (h, &b) in high.iter().enumerate() {
        let b = arith.prepare_multiplier_u128(b);
        for (l, &a_l) in low.iter().enumerate() {
            let expected = arith.mul_canonical_u128(cols[h * per + l], &scale);
            if arith.mul_canonical_u128(a_l, &b) != expected {
                return Err(LogError::Config("column weights are not a tensor over the recursive split".into()));
            }
        }
    }
    Ok((low, high, cols[0]))
}

/// The recursive instance's tables: row weights `(2^k mod q)·A[j_low]` at
/// row `j_low·128 + k`, column weights `B`.
fn recursive_weights(q: u128, low: &[u128]) -> Vec<u128> {
    let arith = field::FpCtx::from_prime_u128(q);
    let slots = slot_weights(q);
    let mut rows = Vec::with_capacity(low.len() * MU_SLOTS);
    for &a in low {
        let a = arith.prepare_multiplier_u128(a);
        for &w in &slots {
            rows.push(arith.mul_canonical_u128(w, &a));
        }
    }
    rows
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

/// The bytes both roles bind after the root: the block statement's frame
/// (claim parameters, committed shape, map digest) and the opener's digest.
fn frame(statement: &BlockStatement<'_>, opener: &LogOpener) -> Vec<u8> {
    let committed = statement.committed_shape();
    let mut out = Vec::with_capacity(48 + 16 + 32 + 32);
    out.extend_from_slice(Encoding::<[u8]>::encode(statement.claim_params()).as_ref());
    out.extend_from_slice(&(committed.log_rows() as u64).to_le_bytes());
    out.extend_from_slice(&(committed.log_columns() as u64).to_le_bytes());
    out.extend_from_slice(&statement.map().digest());
    out.extend_from_slice(&opener.digest());
    out
}

/// The live slots carry every honest fold: `(q − 1)·2^t < 2^127`.
fn folds_fit_live_slots(q: u128, t: usize) -> bool {
    q.checked_sub(1)
        .and_then(|q_minus_one| q_minus_one.checked_mul(1u128 << t))
        .is_some_and(|bound| bound < 1u128 << MU_LIVE)
}

/// The claim grid, the recursive instance's parameters and the recursive
/// claim, shared by both roles.
struct Instance {
    shape: Shape,
    rec_shape: Shape,
    rec_params: BitZParams,
    rec_claim: LinearClaim,
    q: u128,
    g: Gf,
}

fn instance(opener: &LogOpener, statement: &BlockStatement<'_>) -> Result<Instance, LogError> {
    let geometry = opener.geometry;
    let shape = geometry.shape()?;
    let params = statement.claim_params();
    if *params.shape() != shape {
        return Err(LogError::Config(format!(
            "the claim grid {:?} is not the opener's {:?}",
            params.shape(),
            shape
        )));
    }
    let q = params.q();
    let g = params.generator();
    if !folds_fit_live_slots(q, geometry.t) {
        return Err(LogError::Config("(q − 1)·2^t reaches 2^127: the folds do not fit the live slots".into()));
    }
    let rec_shape = geometry.rec_shape()?;
    let rec_params = BitZParams::new(rec_shape, q, g)
        .map_err(|e| LogError::Config(format!("recursive parameters: {e:?}")))?;
    let (low, high, scale) = column_tensor(statement.claim().column_weights(), q, geometry.a)?;
    let rec_rows = recursive_weights(q, &low);
    let arith = field::FpCtx::from_prime_u128(q);
    let rec_target = arith.mul_u128(statement.claim().target(), scale);
    let rec_claim = LinearClaim::new(&rec_params, rec_rows, high, rec_target)
        .map_err(|e| LogError::Config(format!("recursive claim: {e:?}")))?;
    Ok(Instance { shape, rec_shape, rec_params, rec_claim, q, g })
}

/// Proves `statement`'s claim on the derived grid with the folds' bits
/// committed, and opens the transposed exit claim on the block commitment
/// (`hint`, under `pcs`) and the folds' bits on their own commitment.
/// `derived_rows` are the per-column bit rows of `h` in the claim grid's
/// shape; `ood` is the commitment's Round-0 claim.
pub fn prove_block_log(
    opener: &LogOpener,
    statement: &BlockStatement<'_>,
    pcs: &Pcs,
    hint: &FlockCommitHint,
    derived_rows: &[Vec<u64>],
    transcript: &mut ProverState,
    ood: Option<(&[Gf], Gf)>,
) -> Result<LogSizes, LogError> {
    let geometry = opener.geometry;
    let (t, s) = (geometry.t, geometry.s);
    let Instance { shape, rec_shape, rec_params, rec_claim, q: _, g } = instance(opener, statement)?;
    let committed = statement.committed_shape();
    if pcs.bit_len() != 1usize << committed.log_bits()
        || hint.rows().len() != committed.columns()
        || hint.rows().iter().any(|row| row.len() * 64 != committed.rows())
    {
        return Err(LogError::Config("the commitment is not the statement's committed grid".into()));
    }
    if derived_rows.len() != shape.columns() || derived_rows.iter().any(|row| row.len() * 64 != shape.rows()) {
        return Err(LogError::Witness);
    }
    let claim = statement.claim();
    let mut sizes = LogSizes::default();
    let mut mark = transcript.written();
    let mut take = |transcript: &ProverState| {
        let now = transcript.written();
        let delta = (now.0 - mark.0, now.1 - mark.1);
        mark = now;
        delta
    };

    super::trace_start();
    let started = std::time::Instant::now();
    let root = Root(*hint.root());
    transcript.public_message(STATEMENT_LABEL);
    transcript.public_message(&root.0);
    transcript.public_message(frame(statement, opener).as_slice());
    transcript.public_message(claim);
    let comb = FixedBasePow::new(g, 128, WINDOW);
    trace("log: bind", started);

    // The derived grid's packing and the forest's nibble rows come first:
    // the column folds read the nibble rows too (as the block opener's).
    let started = std::time::Instant::now();
    let layout = IntegerMatrixLayout { row_vars: t, col_vars: s, word_bits: 1 };
    let packed = pack_columns_from_rows(&layout, derived_rows);
    let nibble = forest::nibble_rows_for(t, s, &packed);
    let folds = match &nibble {
        Some(nibble) => nibble.column_folds(t, claim.row_exponents(), shape.columns()),
        None => fold::fold_columns(&shape, derived_rows, claim.row_exponents()),
    };
    trace("log: folds", started);
    #[cfg(test)]
    let (folds, claimed_folds) = {
        let mut committed = folds.clone();
        let mut claimed = folds;
        match CHEAT.get() {
            1 => committed[0] += q_of(statement),
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

    // A commitment to the folds' bits, and its Round 0.
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
            // proof of work before `ζ`, as the commitment's Round 0 does.
            if params.grinding_bits > 0 {
                transcript.public_message(FOLD_OOD_GRINDING_LABEL);
                transcript.public_message(&params.grinding_bits.to_le_bytes());
                let seed = super::codec::gf_to_bytes(transcript.verifier_message::<Gf>());
                let nonce = super::pcs::find_pow(&seed, params.grinding_bits);
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
    let zeta: Vec<Gf> = (0..s).map(|_| transcript.verifier_message::<Gf>()).collect();
    let images = fold::column_images(&comb, &claimed_folds);
    let e0 = mle_eval(&images, &zeta);
    transcript.prover_message(&e0);
    let row_images = fold::row_images(&comb, claim.row_exponents());
    trace("log: images", started);
    sizes.head = take(transcript).0;

    // The first tree, over the derived rows.
    let started = std::time::Instant::now();
    let fold_f = Fold {
        folds: Vec::new(),
        images: Vec::new(),
        row_images,
        zeta: zeta.clone(),
        e0,
    };
    let query_f = reduce::gkr_reduce_prove_packed_with(transcript, &fold_f, &shape, &packed, nibble)
        .map_err(|e| LogError::Config(format!("first tree: {e:?}")))?;
    drop(packed);
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
    let (tree_u1, tree_columns, tree_target) =
        on_small(|| reduce::gkr_exit_prove(transcript, &fold_mu, &tree_shape, &tree_packed));
    trace("log: gkr fold bits", started);
    sizes.gkr_mu = take(transcript).0;

    // The recursive instance: standard BitZ on the folds' bits.
    let started = std::time::Instant::now();
    let rec_bit_rows: Vec<Vec<u64>> = mu_hint.rows()[..rec_shape.columns()].to_vec();
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
    trace("log: rec gkr", started);
    sizes.rec_gkr = take(transcript).0;
    let OpeningQuery::InnerProduct { claim: rec_exit } = query_rec else {
        return Err(LogError::Config("recursive exit".into()));
    };

    // The opening of `h`: the exit claim transposed through the block map.
    let started = std::time::Instant::now();
    let query = statement
        .transpose_query(query_f)
        .map_err(|e| LogError::Config(format!("transpose: {e:?}")))?;
    trace("log: transpose", started);

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
        let pair = pcs
            .prove_lin_pair(
                hint,
                &query,
                StatementBinding::Bind,
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
        if let Ok(mut slot) = LAST_SIZES.lock() {
            *slot = Some(sizes);
        }
        return Ok(sizes);
    }

    let started = std::time::Instant::now();
    pcs.prove_lin(hint, &query, StatementBinding::Bind, transcript, ood)
        .map_err(|e| LogError::Opening(format!("h: {e:?}")))?;
    pcs.replay_verifier_tail(transcript);
    trace("log: open h", started);
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
    trace("log: open fold bits", started);
    sizes.open_mu = take(transcript);
    if let Ok(mut slot) = LAST_SIZES.lock() {
        *slot = Some(sizes);
    }
    Ok(sizes)
}

#[cfg(test)]
fn q_of(statement: &BlockStatement<'_>) -> u128 {
    statement.claim_params().q()
}

/// The verifier, consuming the transcript. The phase `v: (*) row images` is
/// the note's `(⋆)` term (the `2^t` row images of the claim).
pub fn verify_block_log(
    opener: &LogOpener,
    statement: &BlockStatement<'_>,
    pcs: &Pcs,
    com: Root,
    mut transcript: VerifierState<'_>,
    ood: Option<(&[Gf], Gf)>,
) -> Result<(), LogError> {
    let geometry = opener.geometry;
    let (t, s) = (geometry.t, geometry.s);
    let Instance { shape, rec_shape, rec_params, rec_claim, q: _, g } = instance(opener, statement)?;
    let committed = statement.committed_shape();
    if pcs.bit_len() != 1usize << committed.log_bits() {
        return Err(LogError::Config("the pcs is not the statement's committed grid".into()));
    }
    let claim = statement.claim();

    let started = std::time::Instant::now();
    transcript.public_message(STATEMENT_LABEL);
    transcript.public_message(&com.0);
    transcript.public_message(frame(statement, opener).as_slice());
    transcript.public_message(claim);
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
                if !super::pcs::pow_valid(&seed, nonce, params.grinding_bits) {
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
    let zeta: Vec<Gf> = (0..s).map(|_| transcript.verifier_message::<Gf>()).collect();
    let e0: Gf = transcript.prover_message().map_err(|_| LogError::Malformed)?;
    trace("v: head", started);

    // (⋆): the row images of the claim, the first tree's leaf images.
    let started = std::time::Instant::now();
    let row_images = fold::row_images(&comb, claim.row_exponents());
    trace("v: (*) row images", started);

    // The first tree.
    let started = std::time::Instant::now();
    let fold_f = Fold {
        folds: Vec::new(),
        images: Vec::new(),
        row_images,
        zeta: zeta.clone(),
        e0,
    };
    let query_f = reduce::gkr_reduce_verify(&mut transcript, &fold_f, &shape)
        .map_err(|_| LogError::Rejected("first tree"))?;
    trace("v: gkr f", started);

    // The second tree, from the same claim.
    let started = std::time::Instant::now();
    let tree_shape = geometry.tree_shape()?;
    let fold_mu = Fold {
        folds: Vec::new(),
        images: Vec::new(),
        row_images: slot_images(g),
        zeta,
        e0,
    };
    let (tree_u1, tree_columns, tree_target) =
        reduce::gkr_exit_verify(&mut transcript, &fold_mu, &tree_shape)
            .map_err(|_| LogError::Rejected("second tree"))?;
    trace("v: gkr fold bits", started);

    // The recursive instance (its folds are checked against its claim and
    // the bound `2^{t'}(q − 1)` by `receive_fold`).
    let started = std::time::Instant::now();
    let rec_verifier = BitZVerifier::new(rec_params, WINDOW);
    let fold_rec = rec_verifier
        .receive_fold(&rec_claim, &mut transcript)
        .map_err(|e| match e {
            fold::ReceiveError::MalformedProof => LogError::Malformed,
            _ => LogError::Rejected("recursive fold"),
        })?;
    trace("v: rec fold", started);
    let started = std::time::Instant::now();
    let query_rec = reduce::gkr_reduce_verify(&mut transcript, &fold_rec, &rec_shape)
        .map_err(|_| LogError::Rejected("recursive tree"))?;
    let OpeningQuery::InnerProduct { claim: rec_exit } = query_rec else {
        return Err(LogError::Config("recursive exit".into()));
    };
    trace("v: rec gkr", started);

    // The opening of `h`: the exit claim transposed through the block map.
    let started = std::time::Instant::now();
    let query = statement
        .transpose_query(query_f)
        .map_err(|e| LogError::Config(format!("transpose: {e:?}")))?;
    trace("v: transpose", started);

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
        pcs.verify_lin_pair(
            &com,
            &query,
            StatementBinding::Bind,
            ood,
            &opener.mu_pcs,
            &Root(mu_root),
            &OpeningQuery::InnerProductSum { claim },
            mu_ood.as_ref().map(|(point, y)| (point.as_slice(), *y)),
            plan,
            &mut transcript,
        )
        .map_err(|e| LogError::Opening(format!("single run: {e:?}")))?;
        trace("v: open (one run)", started);
        transcript.check_eof().map_err(|_| LogError::Rejected("trailing data"))?;
        return Ok(());
    }

    let started = std::time::Instant::now();
    pcs.verify_lin(&com, &query, StatementBinding::Bind, &mut transcript, ood)
        .map_err(|e| LogError::Opening(format!("h: {e:?}")))?;
    trace("v: open h", started);

    // One opening of the folds' bits.
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
    trace("v: open fold bits", started);
    transcript.check_eof().map_err(|_| LogError::Rejected("trailing data"))?;
    Ok(())
}

#[cfg(test)]
thread_local! {
    /// Test-only cheating provers: 1 commits `μ_0 + q` (congruent mod `q`)
    /// and claims the honest `e₀`; 2 commits and claims with `μ_0 + 1`; 3
    /// commits the honest bits and claims `e₀` with `μ_0 + 1`.
    pub(crate) static CHEAT: std::cell::Cell<u8> = const { std::cell::Cell::new(0) };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slot_images_are_powers_and_the_top_slot_is_dead() {
        let g = crate::piop::spartan::protocol::bitz_generator();
        let images = slot_images(g);
        assert_eq!(images.len(), MU_SLOTS);
        let mut h = g;
        for &image in &images[..MU_LIVE] {
            assert_eq!(image, h);
            h = h * h;
        }
        assert_eq!(images[MU_LIVE], Gf::one());
    }

    #[test]
    fn column_tensor_splits_an_eq_table_and_rejects_a_non_tensor() {
        let q: u128 = (1u128 << 61) - 1;
        let arith = field::FpCtx::from_prime_u128(q);
        let point: Vec<u128> = [3u128, 7, 11, 13, 17].iter().map(|&x| x * 0x1234_5678_9abc % q).collect();
        // eq(b, point) with coordinate i on index bit i.
        let mut table = vec![1u128];
        for &p in &point {
            let mut next = Vec::with_capacity(table.len() * 2);
            for &v in &table {
                next.push(arith.mul_u128(v, (1 + q - p) % q));
            }
            for &v in &table {
                next.push(arith.mul_u128(v, p));
            }
            table = next;
        }
        for a in 0..=5 {
            let (low, high, scale) = column_tensor(&table, q, a).unwrap();
            assert_eq!(scale, table[0]);
            for (j, &v) in table.iter().enumerate() {
                let (l, h) = (j & ((1 << a) - 1), j >> a);
                assert_eq!(arith.mul_u128(low[l], high[h]), arith.mul_u128(v, scale), "a {a} j {j}");
            }
        }
        let mut broken = table.clone();
        broken[5] = (broken[5] + 1) % q;
        assert!(column_tensor(&broken, q, 2).is_err());
        assert!(column_tensor(&broken, q, 0).is_ok(), "a = 0 splits anything");
    }

    #[test]
    fn recursive_weights_factor_the_slots() {
        let q: u128 = (1u128 << 61) - 1;
        let arith = field::FpCtx::from_prime_u128(q);
        let low = vec![5u128, 9, 1, 0];
        let rows = recursive_weights(q, &low);
        assert_eq!(rows.len(), low.len() * MU_SLOTS);
        let slots = slot_weights(q);
        for (l, &a) in low.iter().enumerate() {
            for (k, &w) in slots.iter().enumerate() {
                assert_eq!(rows[l * MU_SLOTS + k], arith.mul_u128(a, w));
            }
            assert_eq!(rows[l * MU_SLOTS + MU_LIVE], 0);
        }
    }
}

//! BitZ logarithmic (the note `paper/notes/bitz_logarithmic.tex`) for a
//! virtual opening: the claim lives on a derived grid `h = M·f` and `f` is
//! committed. One recursive call, two Ligerito runs.
//!
//! Standard BitZ (§2.1 of the paper, Step 4) sends the `2^s` integer column
//! folds `μ_j = Σ_i γ_i h_ij` (`γ = π_q^{-1}(u^(1))`), and the verifier
//! checks `Σ_j π_q(μ_j)·u^(2)_j = μ` and exponentiates every fold for the
//! grand products. Here the prover commits to the bits of the folds
//! instead, and
//!
//! 1. a second batched product tree over those bits,
//!    `Π_k ((h_k − 1)·μ_{jk} + 1) = g^{μ_j}` with `h_k = g^{2^k}`, meets the
//!    first one (`Π_i ((g^{γ_i} − 1)·h_ij + 1)`) at one random point `ζ` of
//!    the shared output layer: the prover sends `e₀ = MLE[g^μ](ζ)` and both
//!    GKRs start from it;
//! 2. the recombination `Σ_j π_q(μ_j)·u^(2)_j = μ` is proved by one
//!    standard (recursive) BitZ instance over the committed bits, with
//!    weights `(2^k mod q)·u^(2)_j` (tensor-split again);
//! 3. the first tree's exit on `h` is opened through the commitment to `f`
//!    as the standard virtual opening does it (structured weights when the
//!    map has them, the dense transpose otherwise), and the two claims on
//!    the folds' bits (the second tree's exit, the recursive instance's
//!    exit) are batched into one opening of the second commitment.
//!
//! Only 127 of the 128 bit slots of a fold carry weight (`h_127 = 1`, its
//! recursive weight zero), so a committed exponent stays below
//! `2^127 < ord(g)` and `g^{μ_j}` determines `μ_j`: no range check, but the
//! folds must fit, `(q − 1)·2^t < 2^127`.
//!
//! Every native challenge runs under a schedule of the caller's policy, one
//! per sub-protocol: the first tree with the fold point, the second tree
//! from the same point (a schedule without the fold-point block), the
//! recursive instance, and the two openings. Ported from the SHA+ECDSA
//! proof-size line (itself from branch `bitz-logarithmic`), the virtual
//! two-run path only; there the second tree was unscheduled, which refused
//! any policy that grinds (128 bits).

use flock_core::pcs::ligerito::{LigeritoProfile, LigeritoSecurityConfig, SoundnessRegime};

use super::fold::{self, Fold};
use super::grinding::{Geometry, Policy, Schedule, Stage};
use super::params::{BitZParams, LinearClaim, Root, Shape, SumClaimGf, reference_log_rows};
use super::pcs::{OpeningQuery, Pcs, StatementBinding};
use super::reduce;
use super::transcript::{ProverState, PublicTranscript, VerifierState};
use super::virt::VirtualStatement;
use super::{BitZProver, BitZVerifier, FixedBasePow, WINDOW, trace, trace_bytes};
use circuit::linear_map::binary::VirtualMap;
use crate::ligerito::{mle_eval, pack_columns_from_rows};
use crate::ligerito_flock::{
    FlockCommitHint, LigeritoSelection, OodRoundParams, eq_table_mod_q, ood_eval, ood_point,
    ood_round_params,
};
use field::Gf128 as Gf;

/// Bit slots per fold: one packed element of the second commitment.
pub const MU_SLOTS: usize = 128;
/// The slots that carry weight; the top one is dead (exponents `< 2^127`).
pub const MU_LIVE: usize = 127;
/// `log₂ MU_SLOTS`: the second tree's row variables.
const MU_ROW_VARS: usize = 7;
/// The smallest committed size the crate's Ligerito selections resolve at
/// (the default floor of the second commitment).
pub const MIN_MU_LOG_BITS: usize = 20;
/// The inner transcript's frame labels.
const STATEMENT_LABEL: &[u8] = b"bitz/log/statement/v1";
const VIRTUAL_LABEL: &[u8] = b"bitz/log/virtual-statement/v1";
const FOLD_COMMITMENT_LABEL: &[u8] = b"bitz/log/fold-commitment/v1";
const BATCH_LABEL: &[u8] = b"bitz/log/fold-opening-batch/v1";
const FOLD_OOD_GRINDING_LABEL: &[u8] = b"bitz/log/fold-round0-grinding/v1";

/// What fails in the logarithmic scheme.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LogError {
    /// The geometry or a parameter gate rejects the instance.
    Config(String),
    /// The derived rows are not the opener's shape.
    Witness,
    /// A proof message is missing or malformed.
    Malformed,
    /// A check failed.
    Rejected(&'static str),
    /// The opening of `f` or of the folds' bits failed.
    Opening(String),
}

/// The recursion's geometry: the claim grid `2^t × 2^s`, the second tree's
/// `2^7 × 2^s` view of the folds' bits, the recursive instance's
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

/// The opener's fixed part: the geometry, the second commitment's PCS and
/// ladder (resolved at its size), and the native policy every schedule runs
/// under. `f`'s PCS is the caller's.
#[derive(Clone, Debug)]
pub struct LogOpener {
    geometry: LogGeometry,
    mu_pcs: Pcs,
    mu_security: LigeritoSecurityConfig,
    mu_digest: [u8; 32],
    mu_ood: Option<OodRoundParams>,
    policy: Policy,
}

impl LogOpener {
    /// `claim` is the derived grid (`t × s`), `mu` the second commitment's
    /// Ligerito selection (resolved at its size and `target_bits`), `a` the
    /// recursive split and `min_mu_log_bits` the commitment's floor (see
    /// [`LogGeometry::new`]); `policy` grinds every native challenge.
    pub(crate) fn new(
        claim: &Shape,
        mu: LigeritoSelection,
        a: Option<usize>,
        min_mu_log_bits: Option<usize>,
        target_bits: usize,
        policy: Policy,
    ) -> Result<Self, LogError> {
        let floor = min_mu_log_bits.unwrap_or(MIN_MU_LOG_BITS);
        let geometry = LogGeometry::new(claim.log_rows(), claim.log_columns(), a, floor)?;
        let mu_shape = geometry.commitment_shape()?;
        let mu_packed_vars = mu_shape.log_bits() - MU_ROW_VARS;
        let resolved = mu
            .resolve(mu_packed_vars, target_bits)
            .map_err(|e| LogError::Config(format!("fold commitment ladder: {e}")))?;
        let mu_security = resolved.security().clone();
        let mu_pcs = Pcs::with_security(&mu_shape, &mu_security, LigeritoProfile::Fast)
            .map_err(|e| LogError::Config(format!("fold commitment pcs: {e:?}")))?
            .with_native_policy(policy);
        let mu_ood = ood_round_params(&mu_security, mu_packed_vars, mu_security.target_security_bits as u32);
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
        Ok(Self { geometry, mu_pcs, mu_security, mu_digest: resolved.digest(), mu_ood, policy })
    }

    pub fn geometry(&self) -> &LogGeometry {
        &self.geometry
    }

    pub fn mu_security(&self) -> &LigeritoSecurityConfig {
        &self.mu_security
    }

    /// The opener's frame (geometry and the second commitment's ladder), for
    /// a caller's statement digest.
    pub fn frame(&self) -> Vec<u8> {
        let mut out = self.geometry.frame().to_vec();
        out.extend_from_slice(&self.mu_digest);
        out
    }

    fn mu_packed_vars(&self) -> usize {
        self.mu_pcs.params().m - MU_ROW_VARS
    }
}

/// The claim `⟨eq(·, r₁) ⊗ scale·eq(·, r₂), h⟩ = target (mod q)` by its
/// point: the verifier never expands `eq(·, r₂)`, and expands `eq(·, r₁)`
/// only for the first tree's row images (the note's `(⋆)`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LogStatement {
    pub q: u128,
    pub r1: Vec<u128>,
    pub r2: Vec<u128>,
    /// A residue on the column weights, `scale·eq(·, r₂)` (`1`: none).
    pub scale: u128,
    pub target: u128,
}

impl LogStatement {
    fn frame(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(16 * (4 + self.r1.len() + self.r2.len()));
        out.extend_from_slice(&(self.r1.len() as u64).to_le_bytes());
        out.extend_from_slice(&(self.r2.len() as u64).to_le_bytes());
        out.extend_from_slice(&self.q.to_le_bytes());
        out.extend_from_slice(&self.target.to_le_bytes());
        for r in self.r1.iter().chain(&self.r2) {
            out.extend_from_slice(&r.to_le_bytes());
        }
        out.extend_from_slice(&self.scale.to_le_bytes());
        out
    }
}

/// Bytes of each part of a proof, as the prover wrote them.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LogSizes {
    /// The second commitment's root, its Round-0 value and `e₀`.
    pub head: usize,
    /// The first tree's GKR (over the derived grid).
    pub gkr_h: usize,
    /// The second tree's GKR (over the folds' bits).
    pub gkr_mu: usize,
    /// The recursive instance's folds.
    pub rec_folds: usize,
    /// The recursive instance's GKR.
    pub rec_gkr: usize,
    /// The opening of `f` through the map.
    pub open_f: usize,
    /// The opening of the folds' bits.
    pub open_mu: usize,
}

impl LogSizes {
    pub fn total(&self) -> usize {
        self.head + self.gkr_h + self.gkr_mu + self.rec_folds + self.rec_gkr + self.open_f + self.open_mu
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

/// The recursive instance's tables: `scale·eq(·, r₂)` mod `q` factors as
/// `A[j mod 2^a]·B[j ≫ a]` (the low `a` index bits are `r₂`'s last `a`
/// coordinates), and its row weights are `(2^k mod q)·A[j_low]` at row
/// `j_low·128 + k`.
fn recursive_weights(geometry: &LogGeometry, statement: &LogStatement) -> (Vec<u128>, Vec<u128>) {
    let arith = field::FpCtx::from_prime_u128(statement.q);
    let split = geometry.s - geometry.a;
    let low = eq_table_mod_q(&arith, &statement.r2[split..]);
    let mut high = eq_table_mod_q(&arith, &statement.r2[..split]);
    if statement.scale != 1 {
        for h in &mut high {
            *h = arith.mul_u128(*h, statement.scale);
        }
    }
    let slots = slot_weights(statement.q);
    let mut rows = Vec::with_capacity(low.len() * MU_SLOTS);
    for &a in &low {
        for &w in &slots {
            rows.push(arith.mul_u128(a, w));
        }
    }
    (rows, high)
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
    let low = super::gkr::eq_table(&tree_columns[..a]);
    let high = super::gkr::eq_table(&tree_columns[a..]);
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

/// The live slots carry every honest fold: `(q − 1)·2^t < 2^127`.
fn folds_fit_live_slots(q: u128, t: usize) -> bool {
    q.checked_sub(1)
        .and_then(|q_minus_one| q_minus_one.checked_mul(1u128 << t))
        .is_some_and(|bound| bound < 1u128 << MU_LIVE)
}

/// The statement, the geometry and the second commitment's ladder, then the
/// map statement's frame and `f`'s PCS parameters.
fn bind<M: VirtualMap>(
    transcript: &mut impl PublicTranscript,
    opener: &LogOpener,
    params: &BitZParams,
    root: &[u8; 32],
    statement: &LogStatement,
    map: &VirtualStatement<'_, M>,
    pcs: &Pcs,
) {
    transcript.public_message(STATEMENT_LABEL);
    transcript.public_message(root);
    transcript.public_message(params);
    transcript.public_message(&opener.geometry.frame());
    transcript.public_message(&opener.mu_digest);
    transcript.public_message(statement.frame().as_slice());
    transcript.public_message(VIRTUAL_LABEL);
    transcript.public_message(map.frame().as_slice());
    transcript.public_message(pcs);
}

/// The gates both roles check before reading anything.
fn check<M: VirtualMap>(
    opener: &LogOpener,
    statement: &LogStatement,
    map: &VirtualStatement<'_, M>,
    pcs: &Pcs,
    shape: &Shape,
) -> Result<(), LogError> {
    let (t, s) = (opener.geometry.t, opener.geometry.s);
    if statement.r1.len() != t
        || statement.r2.len() != s
        || statement.target >= statement.q
        || statement.scale >= statement.q
    {
        return Err(LogError::Config("statement does not fit the shape".into()));
    }
    if !folds_fit_live_slots(statement.q, t) {
        return Err(LogError::Config("(q − 1)·2^t reaches 2^127: the folds do not fit the live slots".into()));
    }
    let claim_params = map.claim_params();
    if claim_params.shape() != shape || claim_params.q() != statement.q {
        return Err(LogError::Config("the virtual statement's claim grid or modulus differs".into()));
    }
    if pcs.bit_len() != 1usize << map.committed_shape().log_bits() {
        return Err(LogError::Config("f's commitment is not the statement's committed grid".into()));
    }
    Ok(())
}

fn schedule(policy: Policy, rows: usize, columns: usize) -> Result<Schedule, LogError> {
    Schedule::new(policy, Geometry::prefix(rows, columns)).map_err(|e| LogError::Config(format!("native schedule: {e}")))
}

fn native<E>(result: Result<(), E>) -> Result<(), LogError> {
    result.map_err(|_| LogError::Config("native challenge schedule".into()))
}

/// Opens the first tree's exit on the derived grid through the commitment to
/// `f`: structured weights when the map has them, else the dense transpose.
fn open_f<M: VirtualMap>(
    map: &VirtualStatement<'_, M>,
    pcs: &Pcs,
    hint: &FlockCommitHint,
    query: OpeningQuery,
    transcript: &mut ProverState,
    ood: Option<(&[Gf], Gf)>,
) -> Result<(), LogError> {
    let shape = *map.claim_params().shape();
    let opening = |e| LogError::Opening(format!("f: {e:?}"));
    if let OpeningQuery::InnerProduct { claim } = &query
        && claim.row_weights().len() == shape.rows()
        && claim.column_weights().len() == shape.columns()
        && std::env::var_os("BITZ_DENSE_PROVE").is_none()
        && let Some(weights) = map.map().structured_weights(claim.row_weights(), claim.column_weights(), shape.log_rows())
    {
        return pcs.prove_lin_structured(hint, weights, claim.target(), transcript, ood).map_err(opening);
    }
    let query = map.transpose_query(query).map_err(|e| LogError::Opening(format!("transpose: {e:?}")))?;
    let flat = super::virt::flatten_rows(hint.rows());
    pcs.prove_lin_rows(hint, core::slice::from_ref(&flat), &[], &query, StatementBinding::AlreadyBound, transcript, ood)
        .map_err(opening)
}

/// The prover of the claim on `map`'s derived grid (`derived_rows`, per
/// column bit rows in the claim grid's shape), opened through `hint`
/// (committed under `pcs`). `ood` is `f`'s Round-0 claim, batched into its
/// opening as in the standard virtual opening.
#[allow(clippy::too_many_arguments)]
pub(crate) fn prove<M: VirtualMap>(
    opener: &LogOpener,
    statement: &LogStatement,
    map: &VirtualStatement<'_, M>,
    pcs: &Pcs,
    hint: &FlockCommitHint,
    derived_rows: &[Vec<u64>],
    transcript: &mut ProverState,
    ood: Option<(&[Gf], Gf)>,
) -> Result<LogSizes, LogError> {
    let geometry = opener.geometry;
    let (t, s) = (geometry.t, geometry.s);
    let shape = geometry.shape()?;
    if derived_rows.len() != shape.columns() || derived_rows.iter().any(|row| row.len() * 64 != shape.rows()) {
        return Err(LogError::Witness);
    }
    check(opener, statement, map, pcs, &shape)?;
    let g = crate::piop::spartan::protocol::bitz_generator();
    let params = BitZParams::new(shape, statement.q, g).map_err(|e| LogError::Config(format!("parameters: {e:?}")))?;
    let rec_shape = geometry.rec_shape()?;
    let rec_params =
        BitZParams::new(rec_shape, statement.q, g).map_err(|e| LogError::Config(format!("recursive parameters: {e:?}")))?;
    let mut sizes = LogSizes::default();
    let mut mark = transcript.bytes_written();
    let mut take = |transcript: &ProverState, label: &str| {
        let now = transcript.bytes_written();
        trace_bytes(label, mark, now);
        let delta = now - mark;
        mark = now;
        delta
    };

    let started = std::time::Instant::now();
    bind(transcript, opener, &params, hint.root(), statement, map, pcs);
    let comb = FixedBasePow::new(g, 128, WINDOW);
    let arith = field::FpCtx::from_prime_u128(statement.q);
    let gamma = eq_table_mod_q(&arith, &statement.r1);
    trace("log: bind+weights", started);

    // The folds, then a commitment to their bits.
    let started = std::time::Instant::now();
    let folds = fold::fold_columns(&shape, derived_rows, &gamma);
    trace("log: folds", started);
    let started = std::time::Instant::now();
    let mu_shape = geometry.commitment_shape()?;
    let (mu_root, mu_hint) = on_small(|| opener.mu_pcs.commit(&mu_shape, fold_bit_rows(&geometry, &folds)))
        .map_err(|e| LogError::Config(format!("fold commitment: {e:?}")))?;
    transcript.public_message(FOLD_COMMITMENT_LABEL);
    transcript.prover_message(&mu_root.0);
    let mu_ood = match opener.mu_ood {
        Some(params) => {
            // The round's collision bound tops up to the target with proof
            // of work before `ζ`, as `f`'s Round 0 does.
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

    // The shared output point and the claimed value of both trees there,
    // then the first tree, over the derived grid.
    let started = std::time::Instant::now();
    native(transcript.start_native(schedule(opener.policy, t, s)?))?;
    let zeta = transcript.native_vector(Stage::FoldPoint, s);
    let images = fold::column_images(&comb, &folds);
    let e0 = mle_eval(&images, &zeta);
    transcript.prover_message(&e0);
    let row_images = fold::row_images(&comb, &gamma);
    trace("log: images", started);
    sizes.head = take(transcript, "log head");
    let started = std::time::Instant::now();
    let fold_h = Fold { folds: Vec::new(), images: Vec::new(), row_images, zeta: zeta.clone(), e0 };
    let packed = pack_columns_from_rows(&shape.layout(), derived_rows);
    let query_h = reduce::gkr_reduce_prove_packed(transcript, &fold_h, &shape, &packed)
        .map_err(|e| LogError::Config(format!("first tree: {e:?}")))?;
    drop(packed);
    native(transcript.end_native())?;
    trace("log: gkr h", started);
    sizes.gkr_h = take(transcript, "log gkr h");

    // The second tree, over the folds' bits (128 slot rows, 2^s columns),
    // from the same point.
    let started = std::time::Instant::now();
    let tree_shape = geometry.tree_shape()?;
    let tree_rows: Vec<Vec<u64>> = folds.iter().map(|&m| vec![m as u64, ((m >> 64) as u64) & !(1u64 << 63)]).collect();
    let tree_packed = pack_columns_from_rows(&tree_shape.layout(), &tree_rows);
    let fold_mu = Fold { folds: Vec::new(), images: Vec::new(), row_images: slot_images(g), zeta, e0 };
    native(transcript.start_native(schedule(opener.policy, MU_ROW_VARS, s)?.sharing_fold_point()))?;
    let (tree_u1, tree_columns, tree_target) =
        on_small(|| reduce::gkr_exit_prove(transcript, &fold_mu, &tree_shape, &tree_packed));
    native(transcript.end_native())?;
    trace("log: gkr fold bits", started);
    sizes.gkr_mu = take(transcript, "log gkr fold bits");

    // The recursive instance: standard BitZ on the folds' bits.
    let started = std::time::Instant::now();
    let (rec_rows_w, rec_cols_w) = recursive_weights(&geometry, statement);
    let rec_claim = LinearClaim::new(&rec_params, rec_rows_w, rec_cols_w, statement.target)
        .map_err(|e| LogError::Config(format!("recursive claim: {e:?}")))?;
    let rec_bit_rows: Vec<Vec<u64>> = mu_hint.rows()[..rec_shape.columns()].to_vec();
    native(transcript.start_native(schedule(opener.policy, rec_shape.log_rows(), rec_shape.log_columns())?))?;
    let rec_prover = BitZProver::new(rec_params, WINDOW);
    let fold_rec = on_small(|| rec_prover.send_fold(&rec_claim, &rec_bit_rows, transcript))
        .map_err(|e| LogError::Config(format!("recursive fold: {e:?}")))?;
    sizes.rec_folds = take(transcript, "log rec folds");
    let query_rec = on_small(|| {
        let rec_packed = pack_columns_from_rows(&rec_shape.layout(), &rec_bit_rows);
        reduce::gkr_reduce_prove_packed(transcript, &fold_rec, &rec_shape, &rec_packed)
    })
    .map_err(|e| LogError::Config(format!("recursive tree: {e:?}")))?;
    native(transcript.end_native())?;
    trace("log: recursive instance", started);
    sizes.rec_gkr = take(transcript, "log rec gkr");
    let OpeningQuery::InnerProduct { claim: rec_exit } = query_rec else {
        return Err(LogError::Config("recursive exit".into()));
    };

    // The opening of `f` (its own schedule, the PCS's), then the challenges
    // its verifier draws after the prover stops.
    let started = std::time::Instant::now();
    open_f(map, pcs, hint, query_h, transcript, ood)?;
    pcs.replay_verifier_tail(transcript);
    native(transcript.end_native())?;
    trace("log: open f", started);
    sizes.open_f = take(transcript, "log open f");

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
    native(transcript.end_native())?;
    trace("log: open fold bits", started);
    sizes.open_mu = take(transcript, "log open fold bits");
    Ok(sizes)
}

/// The verifier of [`prove`], consuming the transcript. It forms the first
/// tree's row images (the `2^t` exponentiations of the note's `(⋆)`) and
/// checks `f`'s opening lazily when the map evaluates its transpose.
#[allow(clippy::too_many_arguments)]
pub(crate) fn verify<M: VirtualMap>(
    opener: &LogOpener,
    statement: &LogStatement,
    map: &VirtualStatement<'_, M>,
    pcs: &Pcs,
    root: &[u8; 32],
    mut transcript: VerifierState<'_>,
    ood: Option<(&[Gf], Gf)>,
) -> Result<(), LogError> {
    let geometry = opener.geometry;
    let (t, s) = (geometry.t, geometry.s);
    let shape = geometry.shape()?;
    check(opener, statement, map, pcs, &shape)?;
    let g = crate::piop::spartan::protocol::bitz_generator();
    let params = BitZParams::new(shape, statement.q, g).map_err(|e| LogError::Config(format!("parameters: {e:?}")))?;
    let rec_shape = geometry.rec_shape()?;
    let rec_params =
        BitZParams::new(rec_shape, statement.q, g).map_err(|e| LogError::Config(format!("recursive parameters: {e:?}")))?;
    let malformed = |_| LogError::Malformed;

    let started = std::time::Instant::now();
    bind(&mut transcript, opener, &params, root, statement, map, pcs);
    let comb = FixedBasePow::new(g, 128, WINDOW);
    transcript.public_message(FOLD_COMMITMENT_LABEL);
    let mu_root: [u8; 32] = transcript.prover_message().map_err(malformed)?;
    let mu_ood = match opener.mu_ood {
        Some(params) => {
            if params.grinding_bits > 0 {
                transcript.public_message(FOLD_OOD_GRINDING_LABEL);
                transcript.public_message(&params.grinding_bits.to_le_bytes());
                let seed = super::codec::gf_to_bytes(transcript.verifier_message::<Gf>());
                let nonce = u64::from_le_bytes(transcript.prover_message::<[u8; 8]>().map_err(malformed)?);
                if !super::grinding::valid(&seed, nonce, params.grinding_bits) {
                    return Err(LogError::Rejected("fold commitment round 0 grinding"));
                }
            }
            let zeta: Gf = transcript.verifier_message();
            let point = ood_point(zeta, opener.mu_packed_vars());
            let y: Gf = transcript.prover_message().map_err(malformed)?;
            Some((point, y))
        }
        None => None,
    };
    native(transcript.start_native(schedule(opener.policy, t, s)?))?;
    let zeta = transcript.native_vector(Stage::FoldPoint, s).map_err(malformed)?;
    let e0: Gf = transcript.prover_message().map_err(malformed)?;
    trace("v: log head", started);

    // The first tree.
    let started = std::time::Instant::now();
    let arith = field::FpCtx::from_prime_u128(statement.q);
    let gamma = eq_table_mod_q(&arith, &statement.r1);
    let fold_h = Fold { folds: Vec::new(), images: Vec::new(), row_images: fold::row_images(&comb, &gamma), zeta: zeta.clone(), e0 };
    trace("v: (*) row images", started);
    let started = std::time::Instant::now();
    let query_h = reduce::gkr_reduce_verify(&mut transcript, &fold_h, &shape).map_err(|_| LogError::Rejected("first tree"))?;
    native(transcript.end_native())?;
    trace("v: gkr h", started);

    // The second tree, from the same claim.
    let started = std::time::Instant::now();
    let tree_shape = geometry.tree_shape()?;
    let fold_mu = Fold { folds: Vec::new(), images: Vec::new(), row_images: slot_images(g), zeta, e0 };
    native(transcript.start_native(schedule(opener.policy, MU_ROW_VARS, s)?.sharing_fold_point()))?;
    let (tree_u1, tree_columns, tree_target) =
        reduce::gkr_exit_verify(&mut transcript, &fold_mu, &tree_shape).map_err(|_| LogError::Rejected("second tree"))?;
    native(transcript.end_native())?;
    trace("v: gkr fold bits", started);

    // The recursive instance.
    let started = std::time::Instant::now();
    let (rec_rows_w, rec_cols_w) = recursive_weights(&geometry, statement);
    let rec_claim = LinearClaim::new(&rec_params, rec_rows_w, rec_cols_w, statement.target)
        .map_err(|e| LogError::Config(format!("recursive claim: {e:?}")))?;
    native(transcript.start_native(schedule(opener.policy, rec_shape.log_rows(), rec_shape.log_columns())?))?;
    let rec_verifier = BitZVerifier::new(rec_params, WINDOW);
    let fold_rec = rec_verifier
        .receive_fold(&rec_claim, &mut transcript)
        .map_err(|_| LogError::Rejected("recursive fold"))?;
    let query_rec =
        reduce::gkr_reduce_verify(&mut transcript, &fold_rec, &rec_shape).map_err(|_| LogError::Rejected("recursive tree"))?;
    native(transcript.end_native())?;
    let OpeningQuery::InnerProduct { claim: rec_exit } = query_rec else {
        return Err(LogError::Config("recursive exit".into()));
    };
    trace("v: recursive instance", started);

    // The first tree's exit, on `f`.
    let started = std::time::Instant::now();
    let lazy = map.map().has_transposed_mle() && std::env::var_os("BITZ_DENSE_VERIFY").is_none();
    let opening = |e| LogError::Opening(format!("f: {e:?}"));
    match (&query_h, lazy) {
        (OpeningQuery::InnerProduct { claim }, true) => {
            if claim.row_weights().len() != shape.rows() || claim.column_weights().len() != shape.columns() {
                return Err(LogError::Rejected("first tree exit shape"));
            }
            let (row_w, col_w) = (claim.row_weights(), claim.column_weights());
            let inner = map.map();
            pcs.verify_lin_lazy(
                &Root(*root),
                claim.target(),
                |point| inner.transposed_mle(row_w, col_w, t, point).expect("the map evaluates its transpose"),
                &mut transcript,
                ood,
            )
            .map_err(opening)?;
        }
        _ => {
            let query_f = map.transpose_query(query_h).map_err(|e| LogError::Opening(format!("transpose: {e:?}")))?;
            pcs.verify_lin(&Root(*root), &query_f, StatementBinding::AlreadyBound, &mut transcript, ood).map_err(opening)?;
        }
    }
    native(transcript.end_native())?;
    trace("v: open f", started);

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
    native(transcript.end_native())?;
    trace("v: open fold bits", started);
    transcript.check_eof().map_err(|_| LogError::Rejected("trailing data"))
}

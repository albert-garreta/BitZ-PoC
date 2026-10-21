//! The unsplit bridge with BitZ's logarithmic column sums (a bench-only
//! experiment, `BITZ_FALCON_BRIDGE=polylog`).
//!
//! The standard bridge sends the `2^s` integer column folds
//! `S[c] = Σ_r w_r·b[r, c]` in the clear, 16 bytes each. Here the prover
//! commits `T[c] = S[c] + 1` instead, one GF(2^128) word per column, and
//!
//! 1. sends `R = Σ_c eq(ζ, c)·g^{S[c]}` at the usual column point `ζ`; the
//!    source forest proves `R` against the source bits, as the standard
//!    bridge proves the root its verifier computes from `S`;
//! 2. a second product tree over `T`'s bits (rows `k < 128`, images
//!    `g^{2^k}`) proves `Σ_c eq(ζ, c)·g^{T[c]} = g·R`. Every committed word
//!    is below `2^128` and every honest `S[c] + 1` lies in `[1, 2^128 − 1)`,
//!    so `g^{T[c]} = g^{S[c] + 1}` forces `T[c] = S[c] + 1`: the shift removes
//!    the alias `0 ≡ 2^128 − 1` that an unshifted commitment would allow;
//! 3. the prime read-off `Σ_c cw[c]·S[c] ≡ v (mod p)` becomes
//!    `Σ_c cw[c]·T[c] ≡ v + 1`, because `Σ_c eq_p(·, c) = 1`. A standard
//!    unsplit bridge on `T`'s bits, reshaped to rows `(k, c_lo)` with weights
//!    `2^k·eq_p(c_lo)` and `2^{s−a}` columns, proves it; its few column sums
//!    go in the clear;
//! 4. `T`'s two binary claims (steps 2 and 3) are batched, reduced by a joint
//!    sumcheck and opened by `T`'s own Ligerito run, after its Round 0 under
//!    a Johnson profile.
//!
//! The source claim of step 1 returns to the caller, as in the standard
//! bridge. Grinding: the bridge's per-block difficulty covers both trees and
//! the read-off (their per-round numerators do not exceed the source
//! forest's); the fold sumcheck, opening and Round 0 are sized per site.
use super::{
    Cfg, F, ForestProver, ForestVerifier, checked_weight_sum, err, generator_comb, prove_unsplit,
    unsplit_binary_claim, unsplit_message_count, unsplit_root_point, unsplit_shape,
    validate_point, validate_row_weights, verify_unsplit, weights,
};
use crate::{
    bitz::{fold::fold_columns, forest::Forest, gkr::gpgkr_verify},
    hybrid::{
        BinaryClaim,
        block_grinding::{ProverBlockGrindingTranscript, VerifierBlockGrindingTranscript},
        joint_sumcheck as joint,
        opening::{
            self as shared, Geometry,
            grinding::{GrindingContext, GrindingNonces},
        },
        sumcheck::Scratch,
    },
    ligerito::pack_columns_from_rows,
    ligerito_flock::{
        LigeritoSelection, OodRound, OodRoundParams, ResolvedLigerito, grinding_plan::GrindingPlan,
    },
    pcs::IntegerMatrixLayout,
    piop::spartan::{
        SpartanField,
        falcon::FalconError,
        grinding::{GrindingDomain, site_grinding_bits},
    },
    poly::{univariate::binary_gf128::Gf128 as Gf, utils::build_eq_x_r_vec},
    transcript::{Blake3Transcript, traits::Transcript},
};
use field::{RingOps, Uint};
use flock_core::merkle::Hash;

struct PolylogGrinding;
impl GrindingDomain for PolylogGrinding {
    const DOMAIN: &'static [u8] = b"bitz/falcon/polylog-bridge/grinding/v1";
}

struct PolylogOpeningGrinding;
impl GrindingDomain for PolylogOpeningGrinding {
    const DOMAIN: &'static [u8] = b"bitz/falcon/polylog-bridge/opening-grinding/v1";
}

fn hybrid(error: crate::hybrid::Error) -> FalconError {
    FalconError::Piop(error.to_string())
}

/// The fold commitment's opener: `2^s` words (one per bridge column) in a
/// one-source joint geometry, its Ligerito profile and per-site work.
pub(crate) struct Prepared {
    layout: IntegerMatrixLayout,
    geometry: Geometry<1>,
    ligerito: ResolvedLigerito,
    plan: GrindingPlan,
    round0: Option<OodRoundParams>,
    joint_bits: u32,
    opening_bits: u32,
    /// The fold opening's terminal over GF(2^256) (the main opening's).
    terminal: shared::Terminal,
}

impl Prepared {
    pub(crate) fn new(
        layout: IntegerMatrixLayout,
        selection: LigeritoSelection,
        target_bits: usize,
        terminal: shared::Terminal,
    ) -> Result<Self, FalconError> {
        let s = layout.col_vars;
        // The fold commitment is a joint source of 2^s words (s >= 7); the
        // read-off moves at least one column bit into its rows (t >= 8).
        if layout.row_vars < 8 || s < 7 {
            return Err(err("polylog bridge needs t >= 8 and s >= 7"));
        }
        let geometry = Geometry::with_allocation_logs([s], [s.max(12)]).map_err(hybrid)?;
        let ligerito = selection
            .resolve(geometry.packed_log(), target_bits)
            .map_err(FalconError::Piop)?;
        let mut plan =
            GrindingPlan::resolve(ligerito.security(), target_bits as u32).map_err(FalconError::Piop)?;
        for block in &mut plan.blocks {
            let needed = (target_bits as f64 + block.raw_error.log2()).ceil().max(0.0) as u32;
            block.bits = needed.max(block.native_bits.unwrap_or(0));
        }
        let round0 = ligerito.ood_bits().map(|ood| OodRoundParams {
            grinding_bits: (target_bits as f64 - ood).ceil().max(0.0) as u32,
            extension: ligerito.extension(),
        });
        if round0.is_some_and(|params| {
            params.grinding_bits > crate::piop::spartan::profile::MAX_DERIVED_GRINDING_BITS
        }) {
            return Err(err("polylog fold Round 0 exceeds the derived grinding cap"));
        }
        Ok(Self {
            layout,
            geometry,
            ligerito,
            plan,
            round0,
            joint_bits: site_grinding_bits(target_bits, 2, 128),
            opening_bits: site_grinding_bits(target_bits, 256, 128),
            terminal,
        })
    }

    #[cfg(feature = "bench-internals")]
    pub(crate) fn ligerito(&self) -> &ResolvedLigerito {
        &self.ligerito
    }

    #[cfg(feature = "bench-internals")]
    pub(crate) fn plan(&self) -> &GrindingPlan {
        &self.plan
    }

    #[cfg(feature = "bench-internals")]
    pub(crate) fn round0(&self) -> Option<OodRoundParams> {
        self.round0
    }

    /// The fold tree's and the read-off's shapes, whose sites share the
    /// bridge's per-block difficulty.
    pub(crate) fn layouts(&self) -> [IntegerMatrixLayout; 2] {
        [fold_layout(&self.layout), readoff_layout(&self.layout).1]
    }

    /// Per-site errors before work of the fold sumcheck and the opening's
    /// ring switch, at their difficulties; then Round 0 and the PCS blocks.
    pub(crate) fn site_errors(&self) -> Vec<f64> {
        let binary = |n: usize, bits: u32| n as f64 * 2f64.powi(-128 - bits as i32);
        let mut errors = vec![binary(2, self.joint_bits), binary(256, self.opening_bits)];
        if let (Some(ood), Some(params)) = (self.ligerito.ood_bits(), self.round0) {
            errors.push(2f64.powf(-ood - f64::from(params.grinding_bits)));
        }
        errors.extend(
            self.plan
                .blocks
                .iter()
                .map(|block| block.raw_error * 2f64.powi(-(block.bits as i32))),
        );
        errors
    }

    /// Transcript binding of the experiment's public choices.
    pub(crate) fn bind(&self, h: &mut blake3::Hasher) {
        h.update(b"bridge:polylog/shifted-folds/v1");
        h.update(&self.ligerito.digest());
        for bits in [self.joint_bits, self.opening_bits] {
            h.update(&bits.to_le_bytes());
        }
        for block in &self.plan.blocks {
            h.update(&block.bits.to_le_bytes());
        }
    }
}

/// `T`'s bits as a bridge matrix: rows `k < 128`, one column per word.
fn fold_layout(p: &IntegerMatrixLayout) -> IntegerMatrixLayout {
    IntegerMatrixLayout {
        row_vars: 7,
        col_vars: p.col_vars,
    }
}

/// The read-off matrix: `a` column bits moved into rows `(k, c_lo)`, keeping
/// `2^{7+a} ≤ 2^t` rows so the bridge's prime family still bounds its sums.
fn readoff_layout(p: &IntegerMatrixLayout) -> (usize, IntegerMatrixLayout) {
    let a = (p.row_vars - 7).min(p.col_vars - 1);
    (
        a,
        IntegerMatrixLayout {
            row_vars: 7 + a,
            col_vars: p.col_vars - a,
        },
    )
}

/// Read-off row weights `2^k·eq_p(column_point[..a], c_lo)` at row
/// `k + 128·c_lo`, canonical below `p`.
fn readoff_weights(a: usize, column_point: &[F], field: &Cfg) -> Vec<u128> {
    let low = weights(&column_point[..a], field);
    let powers: Vec<u128> = (0..128).map(|k| field.reduce_u128(1u128 << k)).collect();
    low.iter()
        .flat_map(|&weight| powers.iter().map(move |&power| field.mul_u128(power, weight)))
        .collect()
}

fn bind_folds(t: &mut impl Transcript, p: &IntegerMatrixLayout, root: &Hash) {
    t.absorb_slice(b"bitz/falcon/polylog-bridge/shifted-folds/v1");
    t.absorb_slice(&(p.row_vars as u64).to_le_bytes());
    t.absorb_slice(&(p.col_vars as u64).to_le_bytes());
    t.absorb_slice(root);
}

const ROOT_VALUE_LABEL: &[u8] = b"bitz/falcon/polylog-bridge/root-value/v1";
const FOLD_CLAIMS_LABEL: &[u8] = b"bitz/falcon/polylog-bridge/fold-claims/v1";

fn bind_root_value(t: &mut impl Transcript, value: &Gf) {
    t.absorb_slice(ROOT_VALUE_LABEL);
    t.absorb_slice(&value.to_bytes());
}

/// One batching draw over `T`'s two claims, after both are bound.
fn batch_fold_claims(
    t: &mut impl Transcript,
    fold: BinaryClaim,
    readoff: BinaryClaim,
) -> (joint::Coefficients, Gf) {
    t.absorb_slice(FOLD_CLAIMS_LABEL);
    t.absorb_slice(&fold.value.to_bytes());
    t.absorb_slice(&readoff.value.to_bytes());
    let xi: Gf = t.get_field_challenge(&());
    let target = fold.value + xi * readoff.value;
    (batched_coefficients(fold, readoff, xi), target)
}

/// The batched claims' coefficients at the draw `xi`: the fold claim's
/// tensor, then the read-off's scaled by `xi`.
fn batched_coefficients(fold: BinaryClaim, readoff: BinaryClaim, xi: Gf) -> joint::Coefficients {
    let mut low = readoff.low;
    for value in &mut low {
        *value *= xi;
    }
    joint::Coefficients {
        tensors: vec![
            joint::Tensor {
                low: fold.low,
                high_point: fold.high_point,
                marginals: None,
            },
            joint::Tensor {
                low,
                high_point: readoff.high_point,
                marginals: None,
            },
        ],
        gathers: Vec::new(),
    }
}

/// `T`'s columns as the bridge's column-major bit vectors: word `c` holds
/// rows `k < 128` of column `c`.
fn fold_columns_of(words: &[Gf]) -> Vec<Vec<u64>> {
    words.iter().map(|word| vec![word.lo, word.hi]).collect()
}

/// The read-off's columns: column `c_hi` stacks words `c_hi·2^a .. +2^a`.
fn readoff_columns_of(words: &[Gf], a: usize) -> Vec<Vec<u64>> {
    words
        .chunks_exact(1 << a)
        .map(|chunk| chunk.iter().flat_map(|word| [word.lo, word.hi]).collect())
        .collect()
}

#[derive(Clone, Debug)]
pub(crate) struct Proof {
    /// The fold commitment and its Round 0 (Johnson profiles only).
    pub root: Hash,
    pub ood: Option<OodRound>,
    /// `R = Σ_c eq(ζ, c)·g^{S[c]}`.
    pub root_value: Gf,
    pub forest: Vec<[Gf; 2]>,
    pub fold_forest: Vec<[Gf; 2]>,
    pub nonces: Vec<u64>,
    /// The prime read-off on the reshaped folds.
    pub readoff: super::Proof,
    pub joint: joint::Proof,
    pub opening: shared::JointOpening,
    pub opening_nonces: Vec<u64>,
    pub pcs_nonces: Vec<u64>,
}

impl Proof {
    /// Bytes per part; the parts sum to the bridge's payload.
    pub(crate) fn size_breakdown(&self) -> Vec<(&'static str, usize)> {
        let round0 = self.ood.as_ref().map_or(0, |round| {
            16 * (1 + usize::from(round.y_ext.is_some())) + 8 * usize::from(round.nonce.is_some())
        });
        let (ring, ligerito) = match &self.opening {
            shared::JointOpening::Base(opening) => (
                opening.ring.s_v.len(),
                bincode::serialized_size(&opening.ligerito),
            ),
            shared::JointOpening::Ext(opening) => (opening.ring.s_v.len(), opening.ligerito.serialized_size()),
        };
        let ligerito = ligerito.expect("proof serialization size") as usize;
        vec![
            ("fold_commitment", 32 + round0),
            (
                "source_forest",
                16 + 32 * self.forest.len() + 8 * self.nonces.len(),
            ),
            ("fold_forest", 32 * self.fold_forest.len()),
            (
                "readoff",
                self.readoff.sums.encoded_len()
                    + 32 * self.readoff.forest.len()
                    + 8 * self.readoff.nonces.len(),
            ),
            (
                "fold_sumcheck",
                16 * (2 * self.joint.rounds.len() + 1) + 8 * self.joint.nonces.len(),
            ),
            (
                "fold_opening",
                16 * ring + ligerito + 8 * (self.opening_nonces.len() + self.pcs_nonces.len()),
            ),
        ]
    }
}

/// Prove the bridge claim `Σ_c eq_p(column_point, c)·Σ_r w_r·b[r, c] ≡ v`
/// (mod `modulus`) without sending the column folds; returns the source's
/// binary claim for the enclosing PCS. `rows` are the bridge's column-major
/// bit vectors, as in [`super::prove_weighted`].
#[allow(clippy::too_many_arguments)]
pub(crate) fn prove<T: Transcript + Send>(
    t: &mut T,
    prepared: &Prepared,
    rows: &[Vec<u64>],
    column_point: &[F],
    modulus: u128,
    grinding_bits: u32,
    row_weights: &[u128],
) -> Result<(Proof, BinaryClaim), FalconError> {
    let p = &prepared.layout;
    let field = F::make_cfg(&Uint::from(modulus)).map_err(|_| err("bridge modulus"))?;
    if rows.len() != p.cols() || rows.iter().any(|column| column.len() != p.rows() / 64) {
        return Err(err("bridge source shape"));
    }
    validate_point(column_point, p.col_vars, &field)?;
    validate_row_weights(p, row_weights, modulus)?;
    let shape = unsplit_shape(p, modulus)?;
    // Every fold is at most this bound, below 2^128 - 1: the shift fits.
    checked_weight_sum(row_weights)?;
    let fold_span = tracing::info_span!("falcon_bridge:integer_folds").entered();
    let sums = fold_columns(&shape, rows, row_weights);
    drop(fold_span);
    let commit_span = tracing::info_span!("falcon_bridge:polylog_commit").entered();
    let mut words = vec![Gf::ZERO; 1 << prepared.geometry.physical_logs[0]];
    for (word, &sum) in words.iter_mut().zip(&sums) {
        *word = Gf::from_polynomial_bits(sum + 1);
    }
    let (root, data) = shared::commit_sources(
        &prepared.geometry,
        [&words],
        prepared.ligerito.prover().log_inv_rates[0],
    )
    .map_err(hybrid)?;
    drop(commit_span);
    bind_folds(t, p, &root);
    let ood = prepared.round0.and_then(|params| {
        shared::prove_ood(t, Some(params), &prepared.geometry.virtual_packed([&words]))
    });

    let comb = generator_comb();
    let images: Vec<_> = row_weights.iter().map(|&weight| comb.pow(weight)).collect();
    let fold_images: Vec<Gf> = (0..128).map(|k| comb.pow(1u128 << k)).collect();
    let packed_cols = pack_columns_from_rows(p, rows);
    let folds = fold_layout(p);
    let fold_cols = pack_columns_from_rows(&folds, &fold_columns_of(&words[..p.cols()]));
    let mut grinder = ProverBlockGrindingTranscript::<_, PolylogGrinding>::new(t, grinding_bits);
    let zeta = unsplit_root_point(&mut grinder, p);
    let eq = build_eq_x_r_vec(&zeta, &()).expect("nonempty column point");
    let root_value = sums
        .iter()
        .zip(&eq)
        .fold(Gf::zero(), |acc, (&sum, &weight)| acc + weight * comb.pow(sum));
    bind_root_value(&mut grinder, &root_value);
    let forest_span = tracing::info_span!("falcon_bridge:wfbitz_forest").entered();
    let mut state = ForestProver {
        transcript: &mut grinder,
        messages: Vec::with_capacity(unsplit_message_count(p)),
    };
    let (point, value) = Forest::new(p.row_vars, p.col_vars, &packed_cols, &images)
        .prove_depth(&mut state, &zeta, p.row_vars);
    let forest = state.messages;
    drop(forest_span);
    let claim = unsplit_binary_claim(p, &images, &point, value)?;
    let fold_span = tracing::info_span!("falcon_bridge:polylog_fold_forest").entered();
    let mut state = ForestProver {
        transcript: &mut grinder,
        messages: Vec::with_capacity(unsplit_message_count(&folds)),
    };
    let (fold_point, fold_value) = Forest::new(folds.row_vars, folds.col_vars, &fold_cols, &fold_images)
        .prove_depth(&mut state, &zeta, folds.row_vars);
    let fold_forest = state.messages;
    let fold_claim = unsplit_binary_claim(&folds, &fold_images, &fold_point, fold_value)?;
    let nonces = grinder.finish();
    drop(fold_span);

    let readoff_span = tracing::info_span!("falcon_bridge:polylog_readoff").entered();
    let (a, readoff) = readoff_layout(p);
    let (readoff_proof, readoff_claim) = prove_unsplit(
        t,
        &readoff,
        &readoff_columns_of(&words[..p.cols()], a),
        modulus,
        grinding_bits,
        &readoff_weights(a, column_point, &field),
    )?;
    drop(readoff_span);

    let open_span = tracing::info_span!("falcon_bridge:polylog_open").entered();
    let (coefficients, target) = batch_fold_claims(t, fold_claim, readoff_claim);
    let mut scratch = Scratch::default();
    let (joint, fold_point) = joint::prove(
        t,
        &prepared.geometry,
        [&words],
        [&coefficients],
        target,
        prepared.joint_bits,
        &mut scratch,
    )
    .map_err(hybrid)?;
    let mut pcs_nonces = Vec::new();
    let mut pcs = GrindingContext {
        plan: &prepared.plan,
        nonces: GrindingNonces::Prove(&mut pcs_nonces),
    };
    let mut opening_t =
        ProverBlockGrindingTranscript::<_, PolylogOpeningGrinding>::new(t, prepared.opening_bits);
    let opening = shared::prove_joint_opening(
        &mut opening_t,
        &prepared.geometry,
        &root,
        [&words],
        ood.as_ref(),
        &prepared.ligerito,
        &data,
        &fold_point,
        prepared.terminal,
        Some(&mut pcs),
    )
    .map_err(hybrid)?;
    let opening_nonces = opening_t.finish();
    drop(open_span);
    Ok((
        Proof {
            root,
            ood: ood.map(|claim| claim.round),
            root_value,
            forest,
            fold_forest,
            nonces,
            readoff: readoff_proof,
            joint,
            opening,
            opening_nonces,
            pcs_nonces,
        },
        claim,
    ))
}

/// Verify [`prove`], returning the source's binary claim.
#[allow(clippy::too_many_arguments)]
pub(crate) fn verify<T: Transcript + Send>(
    t: &mut T,
    prepared: &Prepared,
    column_point: &[F],
    value: F,
    modulus: u128,
    proof: &Proof,
    grinding_bits: u32,
    row_weights: &[u128],
) -> Result<BinaryClaim, FalconError> {
    let p = &prepared.layout;
    let field = F::make_cfg(&Uint::from(modulus)).map_err(|_| err("bridge modulus"))?;
    validate_point(column_point, p.col_vars, &field)?;
    value
        .validate_element(&F::canonical_modulus_encoding(&field))
        .map_err(|_| err("bridge value element"))?;
    validate_row_weights(p, row_weights, modulus)?;
    unsplit_shape(p, modulus)?;
    checked_weight_sum(row_weights)?;
    let folds = fold_layout(p);
    if proof.forest.len() != unsplit_message_count(p)
        || proof.fold_forest.len() != unsplit_message_count(&folds)
    {
        return Err(err("polylog bridge forest message count"));
    }
    bind_folds(t, p, &proof.root);
    let ood = shared::verify_ood(t, &prepared.geometry, prepared.round0, proof.ood.as_ref())
        .map_err(hybrid)?;

    let comb = generator_comb();
    let mut grinder = VerifierBlockGrindingTranscript::<_, PolylogGrinding>::new(
        t,
        grinding_bits,
        &proof.nonces,
    );
    let zeta = unsplit_root_point(&mut grinder, p);
    bind_root_value(&mut grinder, &proof.root_value);
    let mut state = ForestVerifier {
        transcript: &mut grinder,
        messages: &proof.forest,
        cursor: 0,
    };
    let (point, claimed) = gpgkr_verify(&mut state, proof.root_value, &zeta, p.row_vars as u32)
        .ok_or(err("bridge forest"))?;
    if state.cursor != state.messages.len() {
        return Err(err("bridge forest trailing messages"));
    }
    let images: Vec<_> = row_weights.iter().map(|&weight| comb.pow(weight)).collect();
    let claim = unsplit_binary_claim(p, &images, &point, claimed)?;
    // The fold tree's root is g·R: every column carries the +1 shift.
    let fold_images: Vec<Gf> = (0..128).map(|k| comb.pow(1u128 << k)).collect();
    let mut state = ForestVerifier {
        transcript: &mut grinder,
        messages: &proof.fold_forest,
        cursor: 0,
    };
    let (fold_point, fold_value) = gpgkr_verify(
        &mut state,
        comb.pow(1) * proof.root_value,
        &zeta,
        folds.row_vars as u32,
    )
    .ok_or(err("polylog fold forest"))?;
    if state.cursor != state.messages.len() {
        return Err(err("polylog fold forest trailing messages"));
    }
    let fold_claim = unsplit_binary_claim(&folds, &fold_images, &fold_point, fold_value)?;
    grinder.finish().map_err(|_| err("bridge grinding"))?;

    let (a, readoff) = readoff_layout(p);
    let shifted = field.add(&value, &F::one_with_cfg(&field));
    let readoff_claim = verify_unsplit(
        t,
        &readoff,
        &column_point[a..],
        shifted,
        &field,
        &proof.readoff,
        grinding_bits,
        &readoff_weights(a, column_point, &field),
    )?;

    let (coefficients, target) = batch_fold_claims(t, fold_claim, readoff_claim);
    let fold_point = joint::verify(
        t,
        &prepared.geometry,
        [&coefficients],
        target,
        prepared.joint_bits,
        &proof.joint,
    )
    .map_err(hybrid)?;
    let mut pcs = GrindingContext {
        plan: &prepared.plan,
        nonces: GrindingNonces::Verify {
            values: &proof.pcs_nonces,
            cursor: 0,
        },
    };
    let mut opening_t = VerifierBlockGrindingTranscript::<_, PolylogOpeningGrinding>::new(
        t,
        prepared.opening_bits,
        &proof.opening_nonces,
    );
    shared::verify_joint_opening(
        &mut opening_t,
        &prepared.geometry,
        &proof.root,
        &proof.root,
        &fold_point,
        proof.joint.value,
        ood.as_ref(),
        &prepared.ligerito,
        &proof.opening,
        prepared.terminal,
        Some(&mut pcs),
    )
    .map_err(hybrid)?;
    opening_t
        .finish()
        .map_err(|_| err("polylog fold opening grinding"))?;
    Ok(claim)
}

/// The bridge's native pieces as the recursion circuit
/// ([`crate::recursion::falcon_verifier`]) runs them on public data: the
/// labels and domains it absorbs, the trees' and read-off's shapes, weights
/// and images, the batching coefficients, the fold opener's configuration.
pub(crate) mod recursion {
    use super::*;

    pub(crate) const GRINDING_DOMAIN: &[u8] = <PolylogGrinding as GrindingDomain>::DOMAIN;
    pub(crate) const OPENING_GRINDING_DOMAIN: &[u8] = <PolylogOpeningGrinding as GrindingDomain>::DOMAIN;
    pub(crate) const ROOT_VALUE_LABEL: &[u8] = super::ROOT_VALUE_LABEL;
    pub(crate) const FOLD_CLAIMS_LABEL: &[u8] = super::FOLD_CLAIMS_LABEL;

    /// The public checks [`super::verify`] makes before its transcript: the
    /// column point, the row weights, the shape against the modulus.
    pub(crate) fn validate(p: &IntegerMatrixLayout, column_point: &[F], field: &Cfg, row_weights: &[u128]) -> Result<(), FalconError> {
        validate_point(column_point, p.col_vars, field)?;
        validate_row_weights(p, row_weights, field.modulus_u128())?;
        unsplit_shape(p, field.modulus_u128())?;
        checked_weight_sum(row_weights)?;
        Ok(())
    }

    pub(crate) fn bind_folds(t: &mut impl Transcript, p: &IntegerMatrixLayout, root: &Hash) {
        super::bind_folds(t, p, root);
    }

    pub(crate) fn fold_layout(p: &IntegerMatrixLayout) -> IntegerMatrixLayout {
        super::fold_layout(p)
    }

    pub(crate) fn readoff_layout(p: &IntegerMatrixLayout) -> (usize, IntegerMatrixLayout) {
        super::readoff_layout(p)
    }

    pub(crate) fn readoff_weights(a: usize, column_point: &[F], field: &Cfg) -> Vec<u128> {
        super::readoff_weights(a, column_point, field)
    }

    /// `g`, the fold tree's root factor (`g^T = g·g^S`).
    pub(crate) fn generator() -> Gf {
        generator_comb().pow(1)
    }

    /// The fold tree's binary claim at its exit `point` (value zero: the
    /// circuit forms it), images `g^{2^k}`.
    pub(crate) fn fold_claim(p: &IntegerMatrixLayout, point: &[Gf]) -> Result<BinaryClaim, FalconError> {
        let comb = generator_comb();
        let images: Vec<Gf> = (0..128).map(|k| comb.pow(1u128 << k)).collect();
        unsplit_binary_claim(&fold_layout(p), &images, point, Gf::ZERO)
    }

    pub(crate) fn batched_coefficients(fold: BinaryClaim, readoff: BinaryClaim, xi: Gf) -> joint::Coefficients {
        super::batched_coefficients(fold, readoff, xi)
    }

    impl Prepared {
        pub(crate) fn layout(&self) -> &IntegerMatrixLayout {
            &self.layout
        }

        pub(crate) fn geometry(&self) -> &Geometry<1> {
            &self.geometry
        }

        pub(crate) fn resolved(&self) -> &ResolvedLigerito {
            &self.ligerito
        }

        pub(crate) fn pcs_plan(&self) -> &GrindingPlan {
            &self.plan
        }

        pub(crate) fn round0_params(&self) -> Option<OodRoundParams> {
            self.round0
        }

        pub(crate) fn joint_bits(&self) -> u32 {
            self.joint_bits
        }

        pub(crate) fn opening_bits(&self) -> u32 {
            self.opening_bits
        }

        pub(crate) fn terminal(&self) -> shared::Terminal {
            self.terminal
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bitz::column_sums::ColumnSums;
    use flock_core::field::Gf256;

    /// Column-major bits with all-zero and all-one columns: zero folds are
    /// where an unshifted commitment could alias `0` with `2^128 - 1`.
    fn source(p: &IntegerMatrixLayout) -> Vec<Vec<u64>> {
        let mut state = 0x9e37_79b9_7f4a_7c15u64;
        (0..p.cols())
            .map(|column| {
                (0..p.rows() / 64)
                    .map(|_| {
                        state ^= state << 13;
                        state ^= state >> 7;
                        state ^= state << 17;
                        match column % 5 {
                            0 => 0,
                            1 => u64::MAX,
                            _ => state,
                        }
                    })
                    .collect()
            })
            .collect()
    }

    fn prime_value(rows: &[Vec<u64>], row_weights: &[u128], column_point: &[F], field: &Cfg) -> F {
        let cols = weights(column_point, field);
        let value = rows.iter().zip(cols).fold(0, |acc, (bits, column)| {
            let sum = row_weights.iter().enumerate().fold(0, |sum, (row, &weight)| {
                if bits[row / 64] >> (row % 64) & 1 == 1 {
                    field.add_u128(sum, weight)
                } else {
                    sum
                }
            });
            field.add_u128(acc, field.mul_u128(sum, column))
        });
        F::from_with_cfg(Uint::from(value), field)
    }

    fn binary_value(rows: &[Vec<u64>], claim: &BinaryClaim) -> Gf {
        let cols = build_eq_x_r_vec(&claim.high_point, &()).unwrap();
        rows.iter().zip(cols).fold(Gf::zero(), |acc, (bits, column)| {
            let sum = claim.low.iter().enumerate().fold(Gf::zero(), |sum, (row, &weight)| {
                if bits[row / 64] >> (row % 64) & 1 == 1 {
                    sum + weight
                } else {
                    sum
                }
            });
            acc + sum * column
        })
    }

    #[test]
    fn polylog_bridge_roundtrip_matches_claims_and_rejects_tampering() {
        let p = IntegerMatrixLayout {
            row_vars: 10,
            col_vars: 8,
        };
        let modulus = (1u128 << 107) - 1;
        let field = F::make_cfg(&Uint::from(modulus)).unwrap();
        let rows = source(&p);
        let row_weights: Vec<u128> = (0..p.rows())
            .map(|row| match row % 3 {
                0 => modulus - 1 - row as u128,
                1 => (row as u128 * 0x1234_5678_9abc_def1) % modulus,
                _ => 0,
            })
            .collect();
        let column_point: Vec<F> = (0..p.col_vars)
            .map(|i| F::from_with_cfg(Uint::from(3 + 5 * i as u128), &field))
            .collect();
        let value = prime_value(&rows, &row_weights, &column_point, &field);
        for profile in ["ext:3:4", "udrg:1:4"] {
            let selection = LigeritoSelection::parse(profile, 128).unwrap();
            let prepared = Prepared::new(p, selection, 128, shared::Terminal::Compact).unwrap();
            let mut pt = Blake3Transcript::new();
            let (proof, claim) =
                prove(&mut pt, &prepared, &rows, &column_point, modulus, 1, &row_weights).unwrap();
            let mut vt = Blake3Transcript::new();
            let verified = verify(
                &mut vt,
                &prepared,
                &column_point,
                value,
                modulus,
                &proof,
                1,
                &row_weights,
            )
            .unwrap();
            assert_eq!(claim.low, verified.low);
            assert_eq!(claim.high_point, verified.high_point);
            assert_eq!(claim.value, verified.value);
            assert_eq!(binary_value(&rows, &verified), verified.value);
            assert_eq!(pt.get_challenge::<u128>(), vt.get_challenge::<u128>());
            assert_eq!(proof.ood.is_some(), profile.starts_with("ext:"));
            let ColumnSums::Unsplit(sums) = &proof.readoff.sums else {
                panic!("the read-off is an unsplit bridge")
            };
            // a = min(t - 7, s - 1) = 3 column bits move into the read-off's rows.
            assert_eq!(sums.len(), 1 << (p.col_vars - 3));

            let rejects = |bad: &Proof, value: F| {
                verify(
                    &mut Blake3Transcript::new(),
                    &prepared,
                    &column_point,
                    value,
                    modulus,
                    bad,
                    1,
                    &row_weights,
                )
                .is_err()
            };
            assert!(rejects(&proof, field.add(&value, &F::one_with_cfg(&field))));
            for mutation in 0..8 {
                let mut bad = proof.clone();
                match mutation {
                    0 => bad.root_value += Gf::one(),
                    1 => bad.root[0] ^= 1,
                    2 => bad.forest[0][0] += Gf::one(),
                    3 => bad.fold_forest[0][0] += Gf::one(),
                    4 => bad.readoff.forest[0][1] += Gf::one(),
                    5 => {
                        if let ColumnSums::Unsplit(sums) = &mut bad.readoff.sums {
                            sums[0] += 1;
                        }
                    }
                    6 => bad.joint.value += Gf::one(),
                    _ => match &mut bad.opening {
                        shared::JointOpening::Base(opening) => {
                            opening.ligerito.final_proof[0] += Gf::one()
                        }
                        shared::JointOpening::Ext(opening) => match &mut opening.ligerito {
                            shared::ExtLigerito::Compact(lig) => lig.final_proof[0] += Gf256::new(Gf::ZERO, Gf::ONE),
                            shared::ExtLigerito::Standard(lig) => lig.final_proof.yr[0] += Gf256::new(Gf::ZERO, Gf::ONE),
                        },
                    },
                }
                assert!(rejects(&bad, value), "{profile} mutation {mutation}");
            }
        }
    }

    /// `g` has order `2^128 - 1`: an unshifted fold commitment could carry
    /// the all-ones word for a zero column. The shifted folds never do.
    #[test]
    fn the_shift_avoids_the_exponent_alias() {
        let comb = generator_comb();
        assert_eq!(comb.pow(u128::MAX), comb.pow(0));
        assert_ne!(comb.pow(u128::MAX - 1), comb.pow(0));
    }
}

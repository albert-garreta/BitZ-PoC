//! The native pieces of the algebraic Falcon verifier its recursion circuit
//! ([`crate::recursion::falcon_verifier`]) runs on public data: the statement
//! binding, prime sampling, the keyed-ring instance polynomials and weights,
//! every challenge-derived weight, and the shapes of the proof.
//!
//! Each hook is the native verifier's own code (or a thin view of its
//! state), so the circuit's public computation is byte-identical to
//! [`PreparedFalconAlgebraic::verify`]. The circuit covers one
//! configuration: keyed ring, word binder, small prime with the unsplit
//! bridge, no compact binding ([`check_configuration`]).

use super::super::arithmetic::recursion as arith;
use super::super::keyed_ring;
use super::*;
use crate::hybrid::joint_sumcheck::Coefficients;
use crate::hybrid::opening::Geometry;

pub(crate) type FieldConfig = Cfg;
pub(crate) const QUOTIENT_LABEL: &[u8] = keyed_ring::recursion::QUOTIENT_LABEL;
pub(crate) const QUOTIENT_LEN: usize = keyed_ring::QUOTIENT_LEN;
pub(crate) const BATCHING_COEFFICIENTS: usize = keyed_ring::BATCHING_COEFFICIENTS;
pub(crate) const POINT_GRINDING_DOMAIN: &[u8] = keyed_ring::recursion::POINT_GRINDING_DOMAIN;
pub(crate) const RING_MODULUS: u64 = Q as u64;
pub(crate) const OPENING_GRINDING_DOMAIN: &[u8] = <OpeningGrinding as GrindingDomain>::DOMAIN;

pub(crate) const NORM_DOMAIN: &[u8] = arith::NORM_DOMAIN;
pub(crate) const NORM_INSTANCE_DOMAIN: &[u8] = arith::NORM_INSTANCE_DOMAIN;
pub(crate) const NORM_ROUND_DOMAIN: &[u8] = arith::NORM_ROUND_DOMAIN;
pub(crate) const MERGE_DOMAIN: &[u8] = arith::MERGE_DOMAIN;
pub(crate) const MERGE_GRINDING_DOMAIN: &[u8] = arith::MERGE_GRINDING_DOMAIN;
pub(crate) const BINDING_ROUND_DOMAIN: &[u8] = arith::BINDING_ROUND_DOMAIN;
pub(crate) const BINDER_DOMAIN: &[u8] = arith::BINDER_DOMAIN;
pub(crate) const BINDER_TERMINALS: &[u8] = arith::BINDER_TERMINALS;
pub(crate) const TERMINAL_MERGE_DOMAIN: &[u8] = arith::TERMINAL_MERGE_DOMAIN;
pub(crate) const SOURCE_OPENING_LABEL: &[u8] = arith::SOURCE_OPENING_LABEL;

/// The prepared configuration as the circuit reads it.
pub fn describe(prepared: &PreparedFalconAlgebraic) -> String {
    let params = bridge_params(&prepared.layout);
    let vc = prepared.ligerito.verifier();
    let mut out = format!(
        "batch {} capacity {} row_vars {} bridge {}x{} packed_log {} ring {:?} keyed_points {}\n",
        prepared.layout.batch(),
        prepared.layout.capacity(),
        prepared.layout.row_vars(),
        params.row_vars,
        params.col_vars,
        prepared.geometry.packed_log(),
        prepared.ring_mode,
        super::keyed_points(),
    );
    out += &format!(
        "schedule {:?}\ngrinding bridge {:?} merge {} joint {} opening {} carry {}\n",
        prepared.schedule,
        prepared.grinding.bridge,
        prepared.grinding.bridge_merge,
        prepared.grinding.joint,
        prepared.grinding.opening,
        prepared.carry_grinding(),
    );
    out += &format!(
        "ligerito initial_k {} ks {:?} log_msg_cols {:?} rates {:?} queries {:?} grinding {:?} fold {:?} ood {:?}\n",
        vc.initial_k,
        vc.recursive_ks,
        vc.recursive_log_msg_cols,
        vc.log_inv_rates,
        vc.queries,
        vc.grinding_bits,
        vc.fold_grinding_bits,
        vc.ood_samples,
    );
    for block in &prepared.pcs_grinding.blocks {
        out += &format!("  pcs block {} native {:?} bits {}\n", block.label, block.native_bits, block.bits);
    }
    match &prepared.geometry {
        SourceGeometry::Single(g) => out += &format!("geometry single {g:?}\n"),
        SourceGeometry::Split(g) => out += &format!("geometry split {g:?}\n"),
    }
    out
}

/// The configuration the circuit implements.
pub(crate) fn check_configuration(prepared: &PreparedFalconAlgebraic) -> Result<(), FalconError> {
    if prepared.ring_mode != RingMode::Keyed
        || !prepared.schedule.word_binding
        || prepared.uses_compact_binding()
        || prepared.uses_legacy_transcript()
        || bridge_mode() != BridgeMode::Unsplit
    {
        return Err(error(
            "the Falcon verifier circuit needs the keyed-ring, word-binder, unsplit-bridge configuration",
        ));
    }
    // Unique decoding, or Johnson over GF(2^256) with the standard terminal
    // (the main opening and the polylog bridge's fold opening alike).
    let supported = |resolved: &ResolvedLigerito, terminal: crate::hybrid::opening::Terminal| {
        resolved.ood_bits().is_none() || (resolved.extension() && terminal == crate::hybrid::opening::Terminal::Standard)
    };
    if !supported(&prepared.ligerito, prepared.terminal())
        || prepared.polylog.as_ref().is_some_and(|pl| !supported(pl.resolved(), pl.terminal()))
    {
        return Err(error(
            "the Falcon verifier circuit opens unique-decoding or ext: profiles with the standard terminal",
        ));
    }
    Ok(())
}

/// Every prime-field difficulty of the argument.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Difficulties {
    pub point: u32,
    pub norm_instance: u32,
    pub norm_round: u32,
    pub merge: u32,
    pub binding: u32,
    pub terminal_merge: u32,
    pub bridge: u32,
    pub joint: u32,
    pub opening: u32,
}

pub(crate) fn difficulties(prepared: &PreparedFalconAlgebraic) -> Difficulties {
    let s = prepared.schedule;
    Difficulties {
        point: prepared.carry_grinding(),
        norm_instance: s.norm_instance_bits,
        norm_round: s.norm_round_bits,
        merge: s.merge_bits,
        binding: s.binding_bits,
        terminal_merge: s.terminal_merge_bits,
        bridge: prepared.grinding.bridge[0],
        joint: prepared.grinding.joint,
        opening: prepared.grinding.opening,
    }
}

/// Round counts: the norm sumcheck's instance variables and rounds, and the
/// binder's rounds; whether the batch is padded (a third binder terminal).
pub(crate) fn rounds(prepared: &PreparedFalconAlgebraic) -> (usize, usize, usize, bool) {
    let layout = &prepared.layout;
    (
        arith::instance_vars(layout),
        arith::norm_rounds(layout),
        arith::binder_rounds(layout),
        layout.batch() < layout.capacity(),
    )
}

pub(crate) fn keyed_points_count() -> usize {
    super::keyed_points()
}

/// `PreparedFalconAlgebraic::transcript` onto `t`: validates the statement
/// and exceptions, absorbs the protocol identifier, the statement digest and
/// the opening configuration. Returns the digest.
pub(crate) fn bind_statement(
    prepared: &PreparedFalconAlgebraic,
    public: &FalconAlgebraicStatement,
    root: &[u8; 32],
    exceptions: &[Vec<u16>],
    t: &mut impl Transcript,
) -> Result<[u8; 32], FalconError> {
    prepared.transcript_into(public, root, exceptions, t)
}

pub(crate) fn sample_field(t: &mut impl Transcript) -> Result<Cfg, FalconError> {
    super::sample_field(t)
}

pub(crate) fn quotient_bound(prepared: &PreparedFalconAlgebraic) -> u128 {
    keyed_ring::quotient_bound(prepared.layout.batch())
}

/// The keyed ring up to the quotient: checks, binding, instance polynomials.
pub(crate) fn keyed_begin(
    prepared: &PreparedFalconAlgebraic,
    public: &FalconAlgebraicStatement,
    field: &Cfg,
    t: &mut impl Transcript,
) -> Result<Vec<[u16; BATCHING_COEFFICIENTS]>, FalconError> {
    keyed_ring::recursion::begin(t, &prepared.layout, public, field, prepared.carry_grinding(), super::keyed_points())
}

pub(crate) fn sample_keyed_points(t: &mut impl Transcript, field: &Cfg) -> Result<(Vec<F>, Vec<F>), FalconError> {
    keyed_ring::recursion::points(t, field, super::keyed_points())
}

/// The keyed ring's verifier claim without the quotient's contribution.
pub(crate) struct KeyedClaim(keyed_ring::VerifierClaim);

impl KeyedClaim {
    pub(crate) fn target(&self) -> F {
        self.0.target
    }
}

pub(crate) fn keyed_claim(
    public: &FalconAlgebraicStatement,
    polynomials: &[[u16; BATCHING_COEFFICIENTS]],
    points: Vec<F>,
    coefficients: &[F],
    field: &Cfg,
) -> KeyedClaim {
    KeyedClaim(keyed_ring::recursion::claim_without_quotient(public, polynomials, points, coefficients, field))
}

pub(crate) fn validate_field(prepared: &PreparedFalconAlgebraic, field: &Cfg) -> Result<(), FalconError> {
    arith::validate(&prepared.layout, field)
}

pub(crate) fn norm_target(prepared: &PreparedFalconAlgebraic, instance_point: &[F], field: &Cfg) -> Result<F, FalconError> {
    arith::norm_target(&prepared.layout, instance_point, field)
}

pub(crate) fn norm_terminal_factor(instance_point: &[F], point: &[F], field: &Cfg) -> F {
    arith::norm_terminal_factor(instance_point, point, field)
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn binder_terminal_weights(
    prepared: &PreparedFalconAlgebraic,
    field: &Cfg,
    ring: &KeyedClaim,
    public: &FalconAlgebraicStatement,
    instance_point: &[F],
    norm_point: &[F],
    eta: F,
    binder_point: &[F],
) -> Result<Vec<F>, FalconError> {
    arith::binder_terminal_weights(
        &prepared.layout,
        field,
        &ring.0,
        &public.public_keys,
        instance_point,
        norm_point,
        eta,
        binder_point,
    )
}

/// The bridge matrix and the word binder's source claim on its rows (the
/// value is the circuit's): column point and row weights.
pub(crate) fn bridge_claim(
    prepared: &PreparedFalconAlgebraic,
    binder_point: Vec<F>,
    zeta: F,
    field: &Cfg,
) -> Result<(IntegerMatrixLayout, Vec<F>, Vec<u128>), FalconError> {
    let (column_point, row_weights) = arith::bridge_claim(&prepared.layout, binder_point, zeta, field)?;
    let params = bridge_params(&prepared.layout);
    let claim = arithmetic::BridgeClaim {
        column_point,
        row_weights,
        modulus: field.modulus_u128(),
        value: field.zero(),
    };
    let claim = widen_bridge_claim(claim, params.row_vars - SOURCE_ROW_VARS, field)?;
    Ok((params, claim.column_point, claim.row_weights))
}

pub(crate) fn coefficients(
    prepared: &PreparedFalconAlgebraic,
    params: &IntegerMatrixLayout,
    exceptions: &[Vec<u16>],
    claim: crate::hybrid::BinaryClaim,
) -> Result<[Coefficients; 3], FalconError> {
    compact::coefficients(&prepared.layout, params, exceptions, claim)
}

/// The shared opening's source geometry.
pub(crate) enum GeometryRef<'a> {
    Single(&'a Geometry<1>),
    Split(&'a Geometry<3>),
}

pub(crate) fn geometry(prepared: &PreparedFalconAlgebraic) -> GeometryRef<'_> {
    match &prepared.geometry {
        SourceGeometry::Single(g) => GeometryRef::Single(g),
        SourceGeometry::Split(g) => GeometryRef::Split(g),
    }
}

pub(crate) fn ligerito(prepared: &PreparedFalconAlgebraic) -> &ResolvedLigerito {
    &prepared.ligerito
}

pub(crate) fn pcs_plan(prepared: &PreparedFalconAlgebraic) -> &GrindingPlan {
    &prepared.pcs_grinding
}

/// The logarithmic bridge's preparation (its fold opener's configuration).
pub(crate) type PolylogPrepared = bridge::polylog::Prepared;

/// The logarithmic bridge's fold opener (`BITZ_FALCON_BRIDGE=polylog`).
pub(crate) fn polylog(prepared: &PreparedFalconAlgebraic) -> Option<&bridge::polylog::Prepared> {
    prepared.polylog.as_ref()
}

/// Round 0's parameters (Johnson profiles), right after the statement.
pub(crate) fn round0(prepared: &PreparedFalconAlgebraic) -> Option<crate::ligerito_flock::OodRoundParams> {
    prepared.round0
}

pub(crate) fn terminal(prepared: &PreparedFalconAlgebraic) -> crate::hybrid::opening::Terminal {
    prepared.terminal()
}

/// An opening's parts as the circuit reads them: the ring switch's `s_v`,
/// the GF(2^128) Ligerito proof or the GF(2^256) one (standard terminal),
/// and its grinding nonces.
pub(crate) struct OpeningView<'a> {
    pub s_v: &'a [Gf],
    pub ligerito: Option<&'a flock_core::pcs::ligerito::LigeritoProof<Vec<Gf>>>,
    pub ligerito_ext: Option<&'a flock_core::pcs::ligerito::ext::LigeritoProofExt>,
    pub opening_nonces: &'a [u64],
    pub pcs_nonces: &'a [u64],
}

fn opening_view<'a>(
    opening: &'a crate::hybrid::opening::JointOpening,
    opening_nonces: &'a [u64],
    pcs_nonces: &'a [u64],
) -> Result<OpeningView<'a>, FalconError> {
    use crate::hybrid::opening::{ExtLigerito, JointOpening};
    let (s_v, ligerito, ligerito_ext) = match opening {
        JointOpening::Base(opening) => (&opening.ring.s_v[..], Some(&opening.ligerito), None),
        JointOpening::Ext(opening) => match &opening.ligerito {
            ExtLigerito::Standard(lig) => (&opening.ring.s_v[..], None, Some(lig)),
            ExtLigerito::Compact(_) => {
                return Err(error("the Falcon verifier circuit reads the standard extension terminal"));
            }
        },
    };
    Ok(OpeningView { s_v, ligerito, ligerito_ext, opening_nonces, pcs_nonces })
}

/// The logarithmic bridge's parts as the circuit reads them.
pub(crate) struct PolylogView<'a> {
    pub root: &'a [u8; 32],
    pub round0: Option<&'a crate::ligerito_flock::OodRound>,
    pub root_value: Gf,
    pub forest: &'a [[Gf; 2]],
    pub fold_forest: &'a [[Gf; 2]],
    pub nonces: &'a [u64],
    pub readoff_sums: &'a [u128],
    pub readoff_forest: &'a [[Gf; 2]],
    pub readoff_nonces: &'a [u64],
    pub joint_rounds: &'a [[Gf; 2]],
    pub joint_value: Gf,
    pub joint_nonces: &'a [u64],
    pub opening: OpeningView<'a>,
}

/// A proof's parts as the circuit reads them.
pub(crate) struct ProofView<'a> {
    pub quotient: &'a [i64],
    pub point_nonce: Option<u64>,
    pub norm_instance_nonce: Option<u64>,
    pub slack: F,
    pub norm_rounds: &'a [[F; 4]],
    pub norm_terminal: F,
    pub norm_nonces: &'a [u64],
    pub merge_nonce: Option<u64>,
    pub binding_rounds: &'a [[F; 3]],
    pub binding_terminal: &'a [F],
    pub binding_nonces: &'a [u64],
    pub terminal_merge_nonce: Option<u64>,
    pub sums: &'a [u128],
    pub forest: &'a [[Gf; 2]],
    pub bridge_nonces: &'a [u64],
    pub joint_rounds: &'a [[Gf; 2]],
    pub joint_value: Gf,
    pub joint_nonces: &'a [u64],
    pub s_v: &'a [Gf],
    /// The GF(2^128) opening, or (`ext:` profiles, standard terminal) the
    /// GF(2^256) one.
    pub ligerito: Option<&'a flock_core::pcs::ligerito::LigeritoProof<Vec<Gf>>>,
    pub ligerito_ext: Option<&'a flock_core::pcs::ligerito::ext::LigeritoProofExt>,
    /// Round 0 of the Johnson profiles.
    pub round0: Option<&'a crate::ligerito_flock::OodRound>,
    pub opening_nonces: &'a [u64],
    pub pcs_nonces: &'a [u64],
    /// The logarithmic bridge (its folds are committed; `sums`, `forest`
    /// and `bridge_nonces` are then empty).
    pub polylog: Option<PolylogView<'a>>,
}

pub(crate) fn view(proof: &FalconAlgebraicProof) -> Result<ProofView<'_>, FalconError> {
    let (RingProof::Keyed(ring), ArithmeticProof::Raw(arithmetic)) = (&proof.ring, &proof.arithmetic) else {
        return Err(error("the Falcon verifier circuit reads keyed-ring raw-arithmetic proofs"));
    };
    if !proof.bridge_merge_nonces.is_empty() {
        return Err(error("raw bridge shape"));
    }
    fn unsplit(sums: &crate::bitz::column_sums::ColumnSums) -> Result<&[u128], FalconError> {
        match sums {
            crate::bitz::column_sums::ColumnSums::Unsplit(sums) => Ok(sums.as_slice()),
            _ => Err(error("unsplit bridge sum shape")),
        }
    }
    let (sums, forest, bridge_nonces, polylog): (&[u128], &[[Gf; 2]], &[u64], _) =
        match (proof.bridges.as_slice(), &proof.polylog_bridge) {
            ([bridge], None) => (unsplit(&bridge.sums)?, &bridge.forest, &bridge.nonces, None),
            ([], Some(pl)) => (
                &[],
                &[],
                &[],
                Some(PolylogView {
                    root: &pl.root,
                    round0: pl.ood.as_ref(),
                    root_value: pl.root_value,
                    forest: &pl.forest,
                    fold_forest: &pl.fold_forest,
                    nonces: &pl.nonces,
                    readoff_sums: unsplit(&pl.readoff.sums)?,
                    readoff_forest: &pl.readoff.forest,
                    readoff_nonces: &pl.readoff.nonces,
                    joint_rounds: &pl.joint.rounds,
                    joint_value: pl.joint.value,
                    joint_nonces: &pl.joint.nonces,
                    opening: opening_view(&pl.opening, &pl.opening_nonces, &pl.pcs_nonces)?,
                }),
            ),
            _ => return Err(error("raw bridge shape")),
        };
    let main = opening_view(&proof.opening, &proof.opening_nonces, &proof.pcs_nonces)?;
    let (s_v, ligerito, ligerito_ext) = (main.s_v, main.ligerito, main.ligerito_ext);
    Ok(ProofView {
        quotient: &ring.quotient,
        point_nonce: ring.point_nonce,
        norm_instance_nonce: arithmetic.norm.instance_nonce,
        slack: arithmetic.norm.slack,
        norm_rounds: &arithmetic.norm.sumcheck.round_polynomials,
        norm_terminal: arithmetic.norm.terminal,
        norm_nonces: &arithmetic.norm.grinding_nonces,
        merge_nonce: arithmetic.merge_nonce,
        binding_rounds: &arithmetic.binding.round_polynomials,
        binding_terminal: &arithmetic.binding_terminal,
        binding_nonces: &arithmetic.binding_nonces,
        terminal_merge_nonce: arithmetic.terminal_merge_nonce,
        sums,
        forest,
        bridge_nonces,
        joint_rounds: &proof.joint.rounds,
        joint_value: proof.joint.value,
        joint_nonces: &proof.joint.nonces,
        s_v,
        ligerito,
        ligerito_ext,
        round0: proof.ood.as_ref(),
        opening_nonces: &proof.opening_nonces,
        pcs_nonces: &proof.pcs_nonces,
        polylog,
    })
}

impl ProofView<'_> {
    /// The main opening's parts.
    pub(crate) fn opening(&self) -> OpeningView<'_> {
        OpeningView {
            s_v: self.s_v,
            ligerito: self.ligerito,
            ligerito_ext: self.ligerito_ext,
            opening_nonces: self.opening_nonces,
            pcs_nonces: self.pcs_nonces,
        }
    }
}

/// The exceptions the statement digest binds (the circuit's public input
/// next to the root), and the bytes of their sparse encoding: a count, then
/// `(signature, count, indices)` for each signature that has any.
pub fn exceptions(proof: &FalconAlgebraicProof) -> (&[Vec<u16>], usize) {
    let sparse = 4 + proof
        .exceptions
        .iter()
        .filter(|e| !e.is_empty())
        .map(|e| 4 + 2 + 2 * e.len())
        .sum::<usize>();
    (&proof.exceptions, sparse)
}

pub fn source_root(proof: &FalconAlgebraicProof) -> &[u8; 32] {
    &proof.source_root
}

/// The parts [`tamper`] alters, each to a different well-formed value.
#[cfg(test)]
pub(crate) const TAMPERED_PARTS: &[&str] = &[
    "quotient",
    "norm-round",
    "norm-terminal",
    "norm-nonce",
    "binding-round",
    "binding-terminal",
    "fold",
    "forest",
    "joint-round",
    "joint-value",
    "ring-switch",
    "initial-row",
    "initial-path",
    "recursive-row",
    "recursive-root",
    "final-message",
    "ligerito-message",
    "fold-nonce",
    "opening-nonce",
];

/// The parts [`tamper`] alters in `proof`: an extension opening has no fold
/// nonces, and adds Round 0, an OOD value and the standard terminal's rows
/// and paths.
#[cfg(test)]
pub(crate) fn tampered_parts(proof: &FalconAlgebraicProof) -> Vec<&'static str> {
    let mut parts = match &proof.opening {
        crate::hybrid::opening::JointOpening::Base(_) => TAMPERED_PARTS.to_vec(),
        crate::hybrid::opening::JointOpening::Ext(_) => TAMPERED_PARTS
            .iter()
            .copied()
            .filter(|&part| part != "fold-nonce")
            .chain(["round0", "ood-value", "final-row", "final-path"])
            .collect(),
    };
    // The logarithmic bridge commits its folds: its own parts in place of the
    // standard bridge's sums and forest.
    if proof.polylog_bridge.is_some() {
        parts.retain(|&part| part != "fold" && part != "forest");
        parts.extend(POLYLOG_PARTS);
    }
    parts
}

/// The logarithmic bridge's parts [`tamper`] alters.
#[cfg(test)]
pub(crate) const POLYLOG_PARTS: &[&str] = &[
    "polylog-root",
    "polylog-round0",
    "polylog-root-value",
    "polylog-forest",
    "polylog-fold-forest",
    "polylog-readoff-sum",
    "polylog-readoff-forest",
    "polylog-joint-round",
    "polylog-joint-value",
    "polylog-ring-switch",
    "polylog-initial-row",
    "polylog-initial-path",
    "polylog-final-message",
    "polylog-opening-nonce",
];

#[cfg(test)]
pub(crate) fn tamper(proof: &mut FalconAlgebraicProof, part: &str) {
    if let Some(part) = part.strip_prefix("polylog-") {
        tamper_polylog(proof, part);
        return;
    }
    if tamper_opening(proof, part) {
        return;
    }
    let (RingProof::Keyed(ring), ArithmeticProof::Raw(arithmetic)) = (&mut proof.ring, &mut proof.arithmetic) else {
        panic!("a keyed-ring raw-arithmetic proof");
    };
    match part {
        "quotient" => ring.quotient[0] += Q,
        "norm-round" => arithmetic.norm.sumcheck.round_polynomials[0].swap(1, 2),
        "norm-terminal" => arithmetic.norm.terminal = arithmetic.norm.slack,
        "norm-nonce" => arithmetic.norm.grinding_nonces[3] += 1,
        "binding-round" => arithmetic.binding.round_polynomials[2].swap(0, 1),
        "binding-terminal" => arithmetic.binding_terminal.swap(0, 1),
        "fold" => {
            let crate::bitz::column_sums::ColumnSums::Unsplit(sums) = &mut proof.bridges[0].sums else {
                panic!("unsplit sums")
            };
            sums[1] += 1;
        }
        "forest" => proof.bridges[0].forest[5][0] += Gf::ONE,
        "joint-round" => proof.joint.rounds[2][1] += Gf::ONE,
        "joint-value" => proof.joint.value += Gf::ONE,
        "opening-nonce" => proof.opening_nonces[0] ^= 1,
        _ => panic!("unknown part {part}"),
    }
}

/// [`tamper`]'s logarithmic-bridge parts.
#[cfg(test)]
fn tamper_polylog(proof: &mut FalconAlgebraicProof, part: &str) {
    use crate::hybrid::opening::{ExtLigerito, JointOpening};
    use flock_core::field::Gf256;
    let pl = proof.polylog_bridge.as_mut().expect("a polylog bridge proof");
    match part {
        "root" => pl.root[5] ^= 1,
        "round0" => *pl.ood.as_mut().and_then(|r| r.y_ext.as_mut()).expect("a fold Round 0") += Gf::ONE,
        "root-value" => pl.root_value += Gf::ONE,
        "forest" => pl.forest[5][0] += Gf::ONE,
        "fold-forest" => pl.fold_forest[3][1] += Gf::ONE,
        "readoff-sum" => {
            let crate::bitz::column_sums::ColumnSums::Unsplit(sums) = &mut pl.readoff.sums else {
                panic!("unsplit read-off sums")
            };
            sums[0] += 1;
        }
        "readoff-forest" => pl.readoff.forest[2][0] += Gf::ONE,
        "joint-round" => pl.joint.rounds[1][0] += Gf::ONE,
        "joint-value" => pl.joint.value += Gf::ONE,
        "opening-nonce" => pl.opening_nonces[0] ^= 1,
        _ => match &mut pl.opening {
            JointOpening::Ext(opening) => {
                let ExtLigerito::Standard(lig) = &mut opening.ligerito else {
                    panic!("a standard-terminal fold opening");
                };
                match part {
                    "ring-switch" => opening.ring.s_v[3] += Gf::ONE,
                    "initial-row" => lig.initial_proof.opened_rows[0][0] += Gf::ONE,
                    "initial-path" => lig.initial_proof.merkle_proof[0][0] ^= 1,
                    "final-message" => lig.final_proof.yr[1] += Gf256::new(Gf::ZERO, Gf::ONE),
                    _ => panic!("unknown polylog part {part}"),
                }
            }
            JointOpening::Base(opening) => match part {
                "ring-switch" => opening.ring.s_v[3] += Gf::ONE,
                "initial-row" => opening.ligerito.initial_proof.opened_rows[0][0] += Gf::ONE,
                "initial-path" => opening.ligerito.initial_proof.merkle_proof[0][0] ^= 1,
                "final-message" => opening.ligerito.final_proof[3] += Gf::ONE,
                _ => panic!("unknown polylog part {part}"),
            },
        },
    }
}

/// [`tamper`]'s opening parts; `false` for any other part.
#[cfg(test)]
fn tamper_opening(proof: &mut FalconAlgebraicProof, part: &str) -> bool {
    use crate::hybrid::opening::{ExtLigerito, JointOpening};
    use flock_core::field::Gf256;
    match &mut proof.opening {
        JointOpening::Base(opening) => {
            let lig = &mut opening.ligerito;
            match part {
                "ring-switch" => opening.ring.s_v[3] += Gf::ONE,
                "initial-row" => lig.initial_proof.opened_rows[0][0] += Gf::ONE,
                "initial-path" => lig.initial_proof.merkle_proof[0][0] ^= 1,
                "recursive-row" => lig.recursive_proofs[0].opened_rows[1][0] += Gf::ONE,
                "recursive-root" => lig.recursive_roots[1][0] ^= 1,
                "final-message" => lig.final_proof[3] += Gf::ONE,
                "ligerito-message" => lig.sumcheck_transcript[2].u_0 += Gf::ONE,
                "fold-nonce" => lig.fold_grinding_nonces[0] += 1,
                _ => return false,
            }
        }
        JointOpening::Ext(opening) => {
            let ExtLigerito::Standard(lig) = &mut opening.ligerito else {
                panic!("a standard-terminal extension opening");
            };
            let y = Gf256::new(Gf::ZERO, Gf::ONE);
            match part {
                "ring-switch" => opening.ring.s_v[3] += Gf::ONE,
                "round0" => *proof.ood.as_mut().and_then(|r| r.y_ext.as_mut()).expect("an extension Round 0") += Gf::ONE,
                "initial-row" => lig.initial_proof.opened_rows[0][0] += Gf::ONE,
                "initial-path" => lig.initial_proof.merkle_proof[0][0] ^= 1,
                "recursive-row" => lig.recursive_proofs[0].opened_rows[1][0] += y,
                "recursive-root" => lig.recursive_roots[1][0] ^= 1,
                "final-message" => lig.final_proof.yr[1] += y,
                "ligerito-message" => lig.sumcheck_transcript[2].u_0 += y,
                "ood-value" => lig.ood_values[0] += Gf256::ONE,
                "final-row" => lig.final_proof.opened_rows[0][0] += Gf256::ONE,
                "final-path" => lig.final_proof.merkle_proof[0][0] ^= 1,
                _ => return false,
            }
        }
    }
    true
}

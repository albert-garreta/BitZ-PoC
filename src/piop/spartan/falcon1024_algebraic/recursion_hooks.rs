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
        || prepared.ligerito.ood_bits().is_some()
    {
        return Err(error(
            "the Falcon verifier circuit needs the keyed-ring, word-binder, unsplit-bridge configuration",
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
    pub ligerito: &'a flock_core::pcs::ligerito::LigeritoProof<Vec<Gf>>,
    pub opening_nonces: &'a [u64],
    pub pcs_nonces: &'a [u64],
}

pub(crate) fn view(proof: &FalconAlgebraicProof) -> Result<ProofView<'_>, FalconError> {
    let (RingProof::Keyed(ring), ArithmeticProof::Raw(arithmetic)) = (&proof.ring, &proof.arithmetic) else {
        return Err(error("the Falcon verifier circuit reads keyed-ring raw-arithmetic proofs"));
    };
    if proof.bridges.len() != 1 || !proof.bridge_merge_nonces.is_empty() {
        return Err(error("raw bridge shape"));
    }
    let bridge = &proof.bridges[0];
    let crate::bitz::column_sums::ColumnSums::Unsplit(sums) = &bridge.sums else {
        return Err(error("unsplit bridge sum shape"));
    };
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
        forest: &bridge.forest,
        bridge_nonces: &bridge.nonces,
        joint_rounds: &proof.joint.rounds,
        joint_value: proof.joint.value,
        joint_nonces: &proof.joint.nonces,
        s_v: &proof.opening.ring.s_v,
        ligerito: &proof.opening.ligerito,
        opening_nonces: &proof.opening_nonces,
        pcs_nonces: &proof.pcs_nonces,
    })
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

#[cfg(test)]
pub(crate) fn tamper(proof: &mut FalconAlgebraicProof, part: &str) {
    let (RingProof::Keyed(ring), ArithmeticProof::Raw(arithmetic)) = (&mut proof.ring, &mut proof.arithmetic) else {
        panic!("a keyed-ring raw-arithmetic proof");
    };
    let bridge = &mut proof.bridges[0];
    let lig = &mut proof.opening.ligerito;
    match part {
        "quotient" => ring.quotient[0] += Q,
        "norm-round" => arithmetic.norm.sumcheck.round_polynomials[0].swap(1, 2),
        "norm-terminal" => arithmetic.norm.terminal = arithmetic.norm.slack,
        "norm-nonce" => arithmetic.norm.grinding_nonces[3] += 1,
        "binding-round" => arithmetic.binding.round_polynomials[2].swap(0, 1),
        "binding-terminal" => arithmetic.binding_terminal.swap(0, 1),
        "fold" => {
            let crate::bitz::column_sums::ColumnSums::Unsplit(sums) = &mut bridge.sums else {
                panic!("unsplit sums")
            };
            sums[1] += 1;
        }
        "forest" => bridge.forest[5][0] += Gf::ONE,
        "joint-round" => proof.joint.rounds[2][1] += Gf::ONE,
        "joint-value" => proof.joint.value += Gf::ONE,
        "ring-switch" => proof.opening.ring.s_v[3] += Gf::ONE,
        "initial-row" => lig.initial_proof.opened_rows[0][0] += Gf::ONE,
        "initial-path" => lig.initial_proof.merkle_proof[0][0] ^= 1,
        "recursive-row" => lig.recursive_proofs[0].opened_rows[1][0] += Gf::ONE,
        "recursive-root" => lig.recursive_roots[1][0] ^= 1,
        "final-message" => lig.final_proof[3] += Gf::ONE,
        "ligerito-message" => lig.sumcheck_transcript[2].u_0 += Gf::ONE,
        "fold-nonce" => lig.fold_grinding_nonces[0] += 1,
        "opening-nonce" => proof.opening_nonces[0] ^= 1,
        _ => panic!("unknown part {part}"),
    }
}

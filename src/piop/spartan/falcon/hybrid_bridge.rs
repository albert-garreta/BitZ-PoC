// Falcon layout adapter for the shared BitZ integer bridge.
use super::{FalconError, FalconSourceLayout, FalconSourceWitness};
use crate::{
    hybrid::{BinaryClaim, integer_bridge},
    piop::spartan::SpartanBitzField as F,
    transcript::traits::Transcript,
};

pub(super) use integer_bridge::{BridgeMode, Proof, validate_layout};

pub(super) fn error_numerator(layout: &FalconSourceLayout, mode: BridgeMode) -> usize {
    integer_bridge::error_numerator(&layout.bitz_params(), mode)
}

/// Grinding boundaries of the bridge; a work weight for the allocation.
pub(super) fn grinding_sites(layout: &FalconSourceLayout) -> usize {
    integer_bridge::message_count(&layout.bitz_params())
}

pub(super) fn prove(
    t: &mut impl Transcript,
    source: &FalconSourceWitness,
    mode: BridgeMode,
    point: &[F],
    modulus: u128,
    grinding_bits: u32,
    row_weights: &[u128],
) -> Result<(Proof, BinaryClaim), FalconError> {
    integer_bridge::prove(
        t,
        &source.layout().bitz_params(),
        source.rows(),
        mode,
        point,
        modulus,
        grinding_bits,
        row_weights,
    )
}

pub(super) fn verify(
    t: &mut impl Transcript,
    layout: &FalconSourceLayout,
    mode: BridgeMode,
    point: &[F],
    value: F,
    modulus: u128,
    proof: &Proof,
    grinding_bits: u32,
) -> Result<BinaryClaim, FalconError> {
    integer_bridge::verify(
        t,
        &layout.bitz_params(),
        mode,
        point,
        value,
        modulus,
        proof,
        grinding_bits,
    )
}

//! Optional, round-zero grinding shared by the Falcon integer reductions.
use super::FalconError;
use crate::{
    piop::spartan::grinding::{GrindingDomain, GrindingRound, grind_and_absorb, verify_and_absorb},
    transcript::traits::Transcript,
};

pub(crate) fn prove_nonce<D: GrindingDomain>(
    transcript: &mut impl Transcript,
    bits: u32,
) -> Result<Option<u64>, FalconError> {
    if bits == 0 {
        return Ok(None);
    }
    grind_and_absorb(transcript, GrindingRound::<D>::new(0), bits)
        .map(Some)
        .map_err(|error| FalconError::Piop(error.to_string()))
}

pub(crate) fn verify_nonce<D: GrindingDomain>(
    transcript: &mut impl Transcript,
    bits: u32,
    nonce: Option<u64>,
) -> Result<(), FalconError> {
    match (bits, nonce) {
        (0, None) => Ok(()),
        (1.., Some(nonce)) => {
            verify_and_absorb(transcript, GrindingRound::<D>::new(0), bits, nonce)
                .map_err(|error| FalconError::Piop(error.to_string()))
        }
        _ => Err(FalconError::Piop(
            "invalid Falcon grinding nonce shape".into(),
        )),
    }
}

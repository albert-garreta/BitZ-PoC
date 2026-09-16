//! Shared quadratic round schedule. Arithmetic and transcript codecs are static callbacks.
use field::RingOps;

/// q(X) = c0 + (claim - 2*c0 - c2) X + c2 X².
/// All messages precede the shared challenge. `fold` consumes that challenge,
/// writes the smaller state, and prepares the next round in the same traversal.
#[allow(clippy::too_many_arguments)]
pub(crate) fn drive<F: RingOps, T, Error>(
    field: &F,
    transcript: &mut T,
    rounds: usize,
    point: &mut Vec<F::Elem>,
    claims: &mut [F::Elem],
    coefficients: &mut [[F::Elem; 2]],
    messages: &mut [[F::Elem; 3]],
    mut sample: impl FnMut(&mut T, usize, &[[F::Elem; 3]]) -> Result<F::Elem, Error>,
    mut fold: impl FnMut(usize, &F::Elem, &mut [[F::Elem; 2]]) -> Result<(), Error>,
) -> Result<(), Error> {
    debug_assert_eq!(claims.len(), coefficients.len());
    debug_assert_eq!(claims.len(), messages.len());
    for round in 0..rounds {
        for ((claim, &[c0, c2]), message) in claims.iter().zip(&*coefficients).zip(&mut *messages) {
            *message = [
                c0,
                field.sub(&field.sub(claim, &field.add(&c0, &c0)), &c2),
                c2,
            ];
        }
        let challenge = sample(transcript, round, messages)?;
        for (claim, &[c0, c1, c2]) in claims.iter_mut().zip(&*messages) {
            *claim = field.add(
                &c0,
                &field.mul(&challenge, &field.add(&c1, &field.mul(&challenge, &c2))),
            );
        }
        point.push(challenge);
        fold(round, &challenge, coefficients)?;
    }
    Ok(())
}

//! Registered ring-challenge fields, implemented by the field backend.
pub use field::q12289::Q12289Extension as FalconExtension;

#[cfg(feature = "falcon-hybrid")]
use crate::transcript::traits::Transcript;

/// Draw canonical coordinates from the caller's current sampling stream.
#[cfg(feature = "falcon-hybrid")]
pub(crate) fn sample_coordinates<const K: usize>(
    transcript: &mut impl Transcript,
) -> Option<[u16; K]> {
    let q = field::q12289::Q as u16;
    let mut coordinates = [0; K];
    for coordinate in &mut coordinates {
        *coordinate = (0..128).find_map(|_| {
            let mut bytes = [0; 2];
            transcript.fill_sampling_bytes(&mut bytes);
            let candidate = u16::from_le_bytes(bytes);
            (candidate < 5 * q).then_some(candidate % q)
        })?;
    }
    Some(coordinates)
}

#[cfg(feature = "falcon-hybrid")]
pub(crate) fn absorb_extensions<const K: usize>(
    transcript: &mut impl Transcript,
    values: &[FalconExtension<K>],
) {
    let mut bytes = Vec::with_capacity(8 + values.len() * K * 2);
    bytes.extend_from_slice(&(values.len() as u64).to_le_bytes());
    for value in values {
        for coordinate in value.0 {
            bytes.extend_from_slice(&coordinate.to_le_bytes());
        }
    }
    transcript.absorb_slice(&bytes);
}

#[cfg(feature = "falcon-hybrid")]
pub(crate) fn equality_weights<const K: usize>(
    point: &[FalconExtension<K>],
) -> Vec<FalconExtension<K>> {
    let mut weights = vec![FalconExtension::<K>::ONE];
    for &r in point.iter().rev() {
        let mut next = Vec::with_capacity(2 * weights.len());
        for weight in weights {
            next.push(weight.mul(FalconExtension::<K>::ONE.sub(r)));
            next.push(weight.mul(r));
        }
        weights = next;
    }
    weights
}

/// Combine live witness rows; padded equality weights contribute zero.
#[cfg(feature = "falcon-hybrid")]
pub(crate) fn certificate<const K: usize, const COEFFICIENTS: usize, W: Sync>(
    witnesses: &[W],
    lambda: &[FalconExtension<K>],
    read: impl Fn(&W, usize) -> u16 + Sync,
) -> Box<[FalconExtension<K>; COEFFICIENTS]> {
    let coefficient = |j: usize| {
        let mut sum = [0u64; K];
        for (witness, weight) in witnesses.iter().zip(lambda) {
            let scalar = u64::from(read(witness, j));
            for (total, &coordinate) in sum.iter_mut().zip(&weight.0) {
                *total += scalar * u64::from(coordinate);
            }
        }
        FalconExtension::new(sum.map(|x| (x % field::q12289::Q as u64) as u16))
    };
    let mut result = Box::new([FalconExtension::ZERO; COEFFICIENTS]);
    if witnesses.len() >= 8 {
        use rayon::prelude::*;
        result
            .par_iter_mut()
            .enumerate()
            .for_each(|(j, value)| *value = coefficient(j));
    } else {
        for (j, value) in result.iter_mut().enumerate() {
            *value = coefficient(j);
        }
    }
    result
}

#[cfg(feature = "falcon-hybrid")]
pub(crate) fn powers<const K: usize, const LEN: usize>(
    base: FalconExtension<K>,
) -> Box<[FalconExtension<K>; LEN]> {
    let mut result = Box::new([FalconExtension::ONE; LEN]);
    for j in 1..LEN {
        result[j] = result[j - 1].mul(base);
    }
    result
}

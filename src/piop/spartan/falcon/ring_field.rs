//! Ring certificate and transcript operations shared by the Falcon reduction.
use super::{FalconError, FalconVerificationTrace, N, Q};
use crate::transcript::traits::Transcript;
#[cfg(feature = "parallel")]
use rayon::prelude::*;

pub(super) const EXTENSION_DEGREE: usize = super::EXTENSION;
pub(super) type Ext = crate::piop::spartan::falcon_extension::FalconExtension<EXTENSION_DEGREE>;

pub(super) fn sample_extension(transcript: &mut impl Transcript) -> Result<Ext, FalconError> {
    transcript.begin_sampling();
    let mut coordinates = [0u16; EXTENSION_DEGREE];
    for coordinate in &mut coordinates {
        let mut accepted = false;
        for _ in 0..128 {
            let mut bytes = [0u8; 2];
            transcript.fill_sampling_bytes(&mut bytes);
            let candidate = u16::from_le_bytes(bytes);
            if candidate < 5 * Q as u16 {
                *coordinate = candidate % Q as u16;
                accepted = true;
                break;
            }
        }
        if !accepted {
            return Err(FalconError::Piop("ring field sampling exhausted".into()));
        }
    }
    let result = Ext::new(coordinates);
    absorb_extensions(transcript, &[result]);
    Ok(result)
}

pub(super) fn absorb_extensions(transcript: &mut impl Transcript, values: &[Ext]) {
    let mut bytes = Vec::with_capacity(8 + values.len() * EXTENSION_DEGREE * 2);
    bytes.extend_from_slice(&(values.len() as u64).to_le_bytes());
    for value in values {
        for coordinate in value.0 {
            bytes.extend_from_slice(&coordinate.to_le_bytes());
        }
    }
    transcript.absorb_slice(&bytes);
}

pub(super) fn equality_weights(point: &[Ext]) -> Vec<Ext> {
    let mut weights = vec![Ext::ONE];
    for &r in point.iter().rev() {
        let mut next = Vec::with_capacity(2 * weights.len());
        for weight in weights {
            next.push(weight.mul(Ext::ONE.sub(r)));
            next.push(weight.mul(r));
        }
        weights = next;
    }
    weights
}

pub(super) fn certificate(traces: &[FalconVerificationTrace], lambda: &[Ext]) -> Vec<Ext> {
    let coefficient = |j: usize| {
        let mut sum = [0u64; EXTENSION_DEGREE];
        for (trace, weight) in traces.iter().zip(lambda) {
            let scalar = u64::from(trace.ring_quotient[j]);
            for (total, &coordinate) in sum.iter_mut().zip(&weight.0) {
                *total += scalar * u64::from(coordinate);
            }
        }
        Ext::new(sum.map(|x| (x % Q as u64) as u16))
    };
    #[cfg(feature = "parallel")]
    if traces.len() >= 8 {
        return (0..N - 1).into_par_iter().map(coefficient).collect();
    }
    (0..N - 1).map(coefficient).collect()
}

falcon_tests! {
mod tests {
    use super::*;
    use crate::transcript::traits::ConstTranscribable;
    use std::collections::VecDeque;

    struct ScriptedTranscript {
        candidates: VecDeque<u16>,
        boundaries: usize,
        draws: usize,
        absorbed: Vec<Vec<u8>>,
    }

    impl ScriptedTranscript {
        fn new(candidates: impl IntoIterator<Item = u16>) -> Self {
            Self {
                candidates: candidates.into_iter().collect(),
                boundaries: 0,
                draws: 0,
                absorbed: Vec::new(),
            }
        }
    }

    impl Transcript for ScriptedTranscript {
        fn get_challenge<T: ConstTranscribable>(&mut self) -> T {
            panic!("extension sampling must use its rejection stream");
        }

        fn begin_sampling(&mut self) {
            self.boundaries += 1;
        }

        fn fill_sampling_bytes(&mut self, output: &mut [u8]) {
            assert_eq!(output.len(), 2);
            assert_eq!(self.boundaries, 1);
            assert!(self.absorbed.is_empty(), "rejected candidates must not be absorbed");
            let candidate = self.candidates.pop_front().expect("script exhausted");
            self.draws += 1;
            output.copy_from_slice(&candidate.to_le_bytes());
        }

        fn absorb_inner(&mut self, bytes: &[u8]) {
            self.absorbed.push(bytes.to_vec());
        }
    }

    #[test]
    fn extension_sampler_checks_cutoff_and_absorbs_only_canonical_coordinates() {
        let q = Q as u16;
        let expected: [u16; EXTENSION_DEGREE] = std::array::from_fn(|i| match i {
            0 | 2 => q - 1,
            1 => 0,
            _ => i as u16,
        });
        let mut candidates = vec![5 * q, u16::MAX, 5 * q - 1, q, q - 1];
        candidates.extend_from_slice(&expected[3..]);
        let mut transcript = ScriptedTranscript::new(candidates);
        assert_eq!(sample_extension(&mut transcript).unwrap(), Ext::new(expected));
        assert_eq!(transcript.boundaries, 1);
        assert_eq!(transcript.draws, EXTENSION_DEGREE + 2);
        assert!(transcript.candidates.is_empty());
        let mut payload = 1u64.to_le_bytes().to_vec();
        for coordinate in expected {
            payload.extend_from_slice(&coordinate.to_le_bytes());
        }
        assert_eq!(
            transcript.absorbed,
            vec![vec![0x6], (payload.len() as u64).to_le_bytes().to_vec(), payload, vec![0x7]],
        );

        // SharedPrime samples the whole extension, including zero and base-field elements.
        for scalar in [0, 19] {
            let mut coordinates = [0; EXTENSION_DEGREE];
            coordinates[0] = scalar;
            let mut transcript = ScriptedTranscript::new(coordinates);
            assert_eq!(sample_extension(&mut transcript).unwrap(), Ext::new(coordinates));
            assert_eq!(transcript.draws, EXTENSION_DEGREE);
        }
    }

    #[test]
    fn extension_sampler_enforces_the_per_coordinate_retry_limit() {
        let mut candidates = vec![u16::MAX; 127];
        candidates.extend([0; EXTENSION_DEGREE]);
        let mut transcript = ScriptedTranscript::new(candidates);
        assert_eq!(sample_extension(&mut transcript).unwrap(), Ext::ZERO);
        assert_eq!(transcript.draws, 127 + EXTENSION_DEGREE);

        for accepted_coordinates in [0, 1] {
            let mut candidates = vec![42; accepted_coordinates];
            candidates.extend([5 * Q as u16; 128]);
            candidates.push(7); // Must remain unread after the failed coordinate.
            let mut transcript = ScriptedTranscript::new(candidates);
            assert!(sample_extension(&mut transcript).is_err());
            assert_eq!(transcript.boundaries, 1);
            assert_eq!(transcript.draws, accepted_coordinates + 128);
            assert_eq!(transcript.candidates, VecDeque::from([7]));
            assert!(transcript.absorbed.is_empty());
        }
    }

    fn element(seed: usize) -> Ext {
        Ext::new(std::array::from_fn(|i| {
            ((137 * seed + 499 * i + 19 * seed * i) % Q as usize) as u16
        }))
    }

    #[test]
    fn equality_weights_match_direct_products_and_sum_to_one() {
        for point in [
            vec![],
            vec![Ext::ZERO],
            vec![Ext::ONE],
            vec![element(3), element(7), element(11)],
            vec![element(3), Ext::ZERO, Ext::ONE, element(11)],
        ] {
            let weights = equality_weights(&point);
            assert_eq!(weights.len(), 1 << point.len());
            for (index, &weight) in weights.iter().enumerate() {
                let expected = point.iter().enumerate().fold(Ext::ONE, |acc, (bit, &r)| {
                    acc.mul(if index >> bit & 1 == 1 { r } else { Ext::ONE.sub(r) })
                });
                assert_eq!(weight, expected);
            }
            assert_eq!(weights.into_iter().fold(Ext::ZERO, Ext::add), Ext::ONE);
        }
    }

    #[test]
    fn certificate_matches_extension_arithmetic_with_heterogeneous_quotients() {
        let fixture = super::super::verification_trace(
            include_bytes!("fixtures/public_key.bin"),
            include_bytes!("fixtures/message.bin"),
            include_bytes!("fixtures/signature_ct.bin"),
        ).unwrap();
        // Batch 3 has an unused equality weight; batch 8 selects the parallel path.
        for batch in [1usize, 3, 8] {
            let mut traces = vec![fixture.clone(); batch];
            for (s, trace) in traces.iter_mut().enumerate() {
                for (j, coefficient) in trace.ring_quotient.iter_mut().enumerate() {
                    *coefficient = ((137 * (s + 1) + 73 * j + 19 * s * j) % Q as usize) as u16;
                }
            }
            let point: Vec<_> = (0..batch.next_power_of_two().ilog2())
                .map(|i| element(7 + i as usize))
                .collect();
            let mut weights = equality_weights(&point);
            let actual = certificate(&traces, &weights);
            assert_eq!(actual.len(), N - 1);
            for (j, &coefficient) in actual.iter().enumerate() {
                let expected = traces.iter().zip(&weights).fold(Ext::ZERO, |sum, (trace, &weight)| {
                    let mut scalar = [0; EXTENSION_DEGREE];
                    scalar[0] = trace.ring_quotient[j];
                    sum.add(weight.mul(Ext::new(scalar)))
                });
                assert_eq!(coefficient, expected, "batch={batch}, coefficient={j}");
                assert!(coefficient.canonical());
            }
            weights[batch..].fill(element(101));
            assert_eq!(certificate(&traces, &weights), actual);
        }
    }
}
}

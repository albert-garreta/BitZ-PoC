//! Experimental binary-word HashToPoint circuit and standalone proof.
//!
//! The circuit proves reduction, rejection, and stable selection of Falcon's
//! first 1024 accepted samples. The standalone proof exposes the exact samples
//! and output words as public inputs; it does not prove SHAKE or a signature.
//! [`add_hash_to_point`] also accepts private wires for a future shared-PCS
//! composition, which must authenticate their equality to the SHAKE and Falcon
//! branches. The current Falcon hybrid proof is deliberately unchanged.

use super::{FalconError, HASH_TO_POINT_SAMPLES, N, Q};
use binius_core::{constraint_system::ValueVec, word::Word};
use binius_frontend::{Circuit, CircuitBuilder, CircuitStat, Wire};
use binius_hash::Blake3HashSuite;
use binius_prover::{OptimalPackedB128, Prover};
use binius_transcript::{ProverTranscript, VerifierTranscript, fiat_shamir::HasherChallenger};
use binius_verifier::Verifier;
use std::time::Instant;

const DOMAIN: &[u8] = b"bitz/falcon1024-ct/binary-hash-to-point-prototype/v1";
type Challenger = HasherChallenger<blake3::Hasher>;

fn error(e: impl std::fmt::Display) -> FalconError {
    FalconError::Piop(e.to_string())
}

fn select_word(builder: &CircuitBuilder, condition: Wire, yes: Wire, no: Wire) -> Wire {
    // The pinned frontend's select gate lowers to BMUL. An explicit MSB mask
    // keeps this circuit in the XOR/AND/shift reduction with one source oracle.
    let mask = builder.sar(condition, 63);
    builder.bxor(no, builder.band(mask, builder.bxor(yes, no)))
}

/// Constrain 1311 canonical 16-bit samples and their first 1024 accepted
/// residues, in sample order. This gadget allocates only internal wires.
///
/// A sample is accepted exactly when it is less than `5 * 12289`; its residue
/// is computed with three conditional subtractions. Underflow is rejected by
/// requiring every selected output to be a canonical 14-bit word.
pub fn add_hash_to_point(
    builder: &CircuitBuilder,
    samples: &[Wire; HASH_TO_POINT_SAMPLES],
    point: &[Wire; N],
) {
    add_selection(builder, samples, point);
}

fn add_selection(builder: &CircuitBuilder, samples: &[Wire], point: &[Wire]) {
    assert!(!point.is_empty() && point.len() <= samples.len());
    assert!(samples.len() <= HASH_TO_POINT_SAMPLES);
    let zero = builder.add_constant(Word(0));
    let one = builder.add_constant(Word(1));
    let word_mask = builder.add_constant(Word(u16::MAX as u64));
    let mut rejected_before = zero;
    let mut slots = Vec::with_capacity(samples.len());
    for &sample in samples {
        builder.assert_zero("sample_u16", builder.shr(sample, 16));
        let mut residue = sample;
        for multiple in [2 * Q as u64, 2 * Q as u64, Q as u64] {
            let constant = builder.add_constant(Word(multiple));
            let (difference, borrow) = builder.isub_bin_bout(residue, constant, zero);
            // The subtraction's MSB borrow means residue < multiple.
            residue = select_word(builder, borrow, residue, difference);
        }
        let limit = builder.add_constant(Word(5 * Q as u64));
        let (_, accepted) = builder.isub_bin_bout(sample, limit, zero);
        let value = select_word(builder, accepted, residue, word_mask);
        // One record carries the 16-bit value (0xffff for a rejection) and
        // its original displacement: the number of preceding rejections.
        slots.push(builder.bxor(value, builder.shl(rejected_before, 16)));
        let rejected = builder.bxor(builder.shr(accepted, 63), one);
        rejected_before = builder.iadd(rejected_before, rejected).0;
    }

    // A valid item at original index u has target u - rejected_before[u].
    // After processing lower displacement bits it has already moved by those
    // bits' sum. The next pass moves it by 2^bit iff that original bit is set.
    // Increasing source order guarantees a moved item is not visited twice in
    // a pass. The destination is a rejected slot, so valid order is preserved.
    // If enough samples are accepted, displacement <= samples.len()-point.len().
    let max_rejections = samples.len() - point.len();
    for bit in 0..usize::BITS as usize {
        let step = 1usize << bit;
        if step > max_rejections {
            break;
        }
        for source in step..slots.len() {
            let current = slots[source];
            let destination = slots[source - step];
            // Both operands place their deciding bit at bit 63. Lower bits
            // are immaterial because the arithmetic shift reads only the MSB.
            let valid = builder.bnot(builder.shl(current, 48));
            let needs_move = builder.shl(current, (47 - bit) as u32);
            let mask = builder.sar(builder.band(valid, needs_move), 63);
            let delta = builder.band(mask, builder.bxor(current, destination));
            slots[source] = builder.bxor(current, delta);
            slots[source - step] = builder.bxor(destination, delta);
        }
    }
    for (&actual, &expected) in slots.iter().zip(point) {
        builder.assert_zero("point_u14", builder.shr(expected, 14));
        builder.assert_eq(
            "stable_hash_to_point",
            builder.band(actual, word_mask),
            expected,
        );
    }
}

/// Structural costs before committing or proving. Counts include the whole batch.
#[derive(Clone, Copy, Debug, serde::Serialize)]
pub struct HashToPointStats {
    pub batch: usize,
    pub and_constraints: usize,
    pub zero_constraints: usize,
    pub integer_multiplications: usize,
    pub binary_multiplications: usize,
    pub and_padded: usize,
    pub committed_words: usize,
    pub live_hidden_words: usize,
    pub public_words: usize,
}

/// Measured online costs; `proof` includes commitment and the complete opening.
#[derive(Clone, Copy, Debug, serde::Serialize)]
pub struct HashToPointTimings {
    pub witness_ms: f64,
    pub prove_ms: f64,
}

/// Complete standalone proof bytes, with samples and point supplied separately.
#[derive(Clone, Debug)]
pub struct BinaryHashToPointProof {
    pub bytes: Vec<u8>,
}

/// A prepared Binius64 experiment with explicit public sample/output boundaries.
///
/// The configured security parameter controls the FRI query target only. It
/// does not certify 128-bit soundness of the other Binius reductions or of a
/// future Falcon composition; that requires separate soundness accounting.
pub struct BinaryHashToPoint {
    circuit: Circuit,
    samples: Vec<[Wire; HASH_TO_POINT_SAMPLES]>,
    points: Vec<[Wire; N]>,
    verifier: Verifier<Blake3HashSuite>,
    prover: Prover<OptimalPackedB128, Blake3HashSuite>,
    fri_security_bits: usize,
}

impl BinaryHashToPoint {
    /// Compile a fixed-size batch. Setup is reusable and excluded from timings.
    pub fn new(batch: usize, fri_security_bits: usize) -> Result<Self, FalconError> {
        if !(1..=1024).contains(&batch) || !(1..=128).contains(&fri_security_bits) {
            return Err(error("binary HashToPoint batch or FRI security parameter"));
        }
        let builder = CircuitBuilder::new();
        let mut samples = Vec::with_capacity(batch);
        let mut points = Vec::with_capacity(batch);
        for _ in 0..batch {
            let input = std::array::from_fn(|_| builder.add_inout());
            let output = std::array::from_fn(|_| builder.add_inout());
            add_hash_to_point(&builder, &input, &output);
            samples.push(input);
            points.push(output);
        }
        let circuit = builder.build();
        let cs = circuit.constraint_system();
        cs.validate().map_err(error)?;
        if !cs.imul_constraints.is_empty() || !cs.bmul_constraints.is_empty() {
            return Err(error(
                "HashToPoint unexpectedly allocated multiplication oracles",
            ));
        }
        let verifier =
            Verifier::<Blake3HashSuite>::setup_with_security_bits(cs.clone(), 1, fri_security_bits)
                .map_err(error)?;
        let prover = Prover::setup(verifier.clone()).map_err(error)?;
        Ok(Self {
            circuit,
            samples,
            points,
            verifier,
            prover,
            fri_security_bits,
        })
    }

    pub fn circuit(&self) -> &Circuit {
        &self.circuit
    }

    /// Maps each input sample to its compiled witness index via `circuit()`.
    pub fn sample_wires(&self) -> &[[Wire; HASH_TO_POINT_SAMPLES]] {
        &self.samples
    }

    /// Maps each output coefficient to its compiled witness index via `circuit()`.
    pub fn point_wires(&self) -> &[[Wire; N]] {
        &self.points
    }

    pub fn stats(&self) -> HashToPointStats {
        let stat = CircuitStat::collect(&self.circuit);
        HashToPointStats {
            batch: self.samples.len(),
            and_constraints: stat.n_and_constraints,
            zero_constraints: stat.n_zero_constraints,
            integer_multiplications: stat.n_imul_constraints,
            binary_multiplications: stat.n_bmul_constraints,
            and_padded: stat.and_allocated,
            committed_words: stat.committed_allocated,
            live_hidden_words: stat.n_witness + stat.n_internal,
            public_words: stat.n_inout,
        }
    }

    fn check_shape(
        &self,
        samples: &[[u16; HASH_TO_POINT_SAMPLES]],
        points: &[[u16; N]],
    ) -> Result<(), FalconError> {
        if samples.len() != self.samples.len() || points.len() != self.points.len() {
            return Err(error("binary HashToPoint statement shape"));
        }
        Ok(())
    }

    pub fn populate(
        &self,
        samples: &[[u16; HASH_TO_POINT_SAMPLES]],
        points: &[[u16; N]],
    ) -> Result<ValueVec, FalconError> {
        self.check_shape(samples, points)?;
        let mut filler = self.circuit.new_witness_filler();
        for (input_wires, input) in self.samples.iter().zip(samples) {
            for (&wire, &word) in input_wires.iter().zip(input) {
                filler[wire] = Word(word as u64);
            }
        }
        for (output_wires, output) in self.points.iter().zip(points) {
            for (&wire, &word) in output_wires.iter().zip(output) {
                filler[wire] = Word(word as u64);
            }
        }
        self.circuit
            .populate_wire_witness(&mut filler)
            .map_err(error)?;
        Ok(filler.into_value_vec())
    }

    /// Prove the entire circuit, including witness commitment and opening.
    pub fn prove(
        &self,
        samples: &[[u16; HASH_TO_POINT_SAMPLES]],
        points: &[[u16; N]],
    ) -> Result<(BinaryHashToPointProof, HashToPointTimings), FalconError> {
        let start = Instant::now();
        let witness = self.populate(samples, points)?;
        let witness_time = start.elapsed();
        let start = Instant::now();
        let mut transcript = ProverTranscript::new(Challenger::default());
        transcript.observe().write_bytes(DOMAIN);
        transcript
            .observe()
            .write_bytes(&(self.samples.len() as u64).to_le_bytes());
        transcript
            .observe()
            .write_bytes(&(self.fri_security_bits as u64).to_le_bytes());
        self.prover
            .prove(&witness, &mut transcript)
            .map_err(error)?;
        let proof = BinaryHashToPointProof {
            bytes: transcript.finalize(),
        };
        Ok((
            proof,
            HashToPointTimings {
                witness_ms: witness_time.as_secs_f64() * 1000.0,
                prove_ms: start.elapsed().as_secs_f64() * 1000.0,
            },
        ))
    }

    pub fn verify(
        &self,
        samples: &[[u16; HASH_TO_POINT_SAMPLES]],
        points: &[[u16; N]],
        proof: &BinaryHashToPointProof,
    ) -> Result<(), FalconError> {
        self.check_shape(samples, points)?;
        // Inout allocation is [samples, point] per signature, in that order.
        let public: Vec<_> = samples
            .iter()
            .zip(points)
            .flat_map(|(input, output)| input.iter().chain(output).map(|&x| Word(x as u64)))
            .collect();
        let mut transcript = VerifierTranscript::new(Challenger::default(), proof.bytes.clone());
        transcript.observe().write_bytes(DOMAIN);
        transcript
            .observe()
            .write_bytes(&(self.samples.len() as u64).to_le_bytes());
        transcript
            .observe()
            .write_bytes(&(self.fri_security_bits as u64).to_le_bytes());
        self.verifier
            .verify(&public, &mut transcript)
            .map_err(error)?;
        transcript.finalize().map_err(error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(
        circuit: &Circuit,
        samples: &[Wire],
        point: &[Wire],
        input: &[u64],
        output: &[u64],
    ) -> bool {
        let mut filler = circuit.new_witness_filler();
        for (&wire, &value) in samples.iter().chain(point).zip(input.iter().chain(output)) {
            filler[wire] = Word(value);
        }
        let populated = circuit.populate_wire_witness(&mut filler).is_ok();
        let constrained = circuit
            .constraint_system()
            .verify(&filler.into_value_vec())
            .is_ok();
        assert_eq!(
            populated, constrained,
            "compiled constraints disagree with the evaluator"
        );
        constrained
    }

    #[test]
    fn binary_selection_exhausts_small_acceptance_patterns() {
        let builder = CircuitBuilder::new();
        let samples: Vec<_> = (0..8).map(|_| builder.add_witness()).collect();
        let point: Vec<_> = (0..4).map(|_| builder.add_witness()).collect();
        add_selection(&builder, &samples, &point);
        let circuit = builder.build();
        for acceptance in 0u32..256 {
            let input: Vec<_> = (0..8)
                .map(|i| {
                    if acceptance & (1 << i) != 0 {
                        Q as u64 * (i % 5) + i
                    } else {
                        5 * Q as u64 + i
                    }
                })
                .collect();
            let expected: Vec<_> = input
                .iter()
                .filter(|&&w| w < 5 * Q as u64)
                .map(|&w| w % Q as u64)
                .take(4)
                .collect();
            let mut output = expected.clone();
            output.resize(4, 0);
            assert_eq!(
                run(&circuit, &samples, &point, &input, &output),
                expected.len() == 4
            );
            if expected.len() == 4 {
                output.swap(0, 1);
                assert!(!run(&circuit, &samples, &point, &input, &output));
            }
        }
    }

    #[test]
    fn binary_selection_checks_reduction_boundaries_and_word_ranges() {
        let builder = CircuitBuilder::new();
        let samples: Vec<_> = (0..8).map(|_| builder.add_witness()).collect();
        let point: Vec<_> = (0..4).map(|_| builder.add_witness()).collect();
        add_selection(&builder, &samples, &point);
        let circuit = builder.build();
        let input = [0, 12288, 12289, 61444, 61445, 65535, 24578, 36867];
        let output = [0, 12288, 0, 12288];
        assert!(run(&circuit, &samples, &point, &input, &output));
        let mut wrong = output;
        wrong[0] = 1;
        assert!(!run(&circuit, &samples, &point, &input, &wrong));
        wrong[0] = 1 << 14;
        assert!(!run(&circuit, &samples, &point, &input, &wrong));
        let mut wide = input;
        wide[4] += 1 << 16;
        assert!(!run(&circuit, &samples, &point, &wide, &output));
    }

    #[test]
    fn binary_hash_to_point_matches_shake_and_extreme_compaction() {
        let builder = CircuitBuilder::new();
        let samples = std::array::from_fn(|_| builder.add_witness());
        let point = std::array::from_fn(|_| builder.add_witness());
        add_hash_to_point(&builder, &samples, &point);
        let circuit = builder.build();
        let stats = CircuitStat::collect(&circuit);
        assert_eq!(stats.n_imul_constraints, 0);
        assert_eq!(stats.n_bmul_constraints, 0);
        let trace = super::super::hash_to_point_ct(&[7; 40], &[3; 32]).unwrap();
        let input: Vec<_> = trace.words.iter().map(|&x| x as u64).collect();
        let output: Vec<_> = trace.point.iter().map(|&x| x as u64).collect();
        assert!(run(&circuit, &samples, &point, &input, &output));
        let mut input = vec![65535; HASH_TO_POINT_SAMPLES];
        for (i, slot) in input[HASH_TO_POINT_SAMPLES - N..].iter_mut().enumerate() {
            *slot = i as u64;
        }
        let output: Vec<_> = (0..N as u64).collect();
        assert!(run(&circuit, &samples, &point, &input, &output));
        input[HASH_TO_POINT_SAMPLES - N] = 65535;
        assert!(!run(&circuit, &samples, &point, &input, &output));
    }

    #[test]
    fn binary_hash_to_point_proof_authenticates_boundary() {
        let relation = BinaryHashToPoint::new(1, 100).unwrap();
        assert_eq!(relation.stats().integer_multiplications, 0);
        assert_eq!(relation.stats().binary_multiplications, 0);
        let trace = super::super::hash_to_point_ct(&[7; 40], &[3; 32]).unwrap();
        let samples = [*trace.words];
        let points = [*trace.point];
        let (proof, _) = relation.prove(&samples, &points).unwrap();
        relation.verify(&samples, &points, &proof).unwrap();
        let mut wrong = points;
        wrong[0].swap(0, 1);
        assert!(relation.verify(&samples, &wrong, &proof).is_err());
        let mut wrong = samples;
        wrong[0][0] ^= 1;
        assert!(relation.verify(&wrong, &points, &proof).is_err());
        let mut trailing = proof;
        trailing.bytes.push(0);
        assert!(relation.verify(&samples, &points, &trailing).is_err());
    }
}

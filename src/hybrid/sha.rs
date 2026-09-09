//! Sequential SHA-256 compression, with a fixed IV and one public final state.
use super::{
    BinaryClaim, Error,
    channel::{ProverChannel, VerifierChannel},
};
use crate::transcript::Blake3Transcript;
use binius_circuits::sha256::compress::{State, ref_compress, sha256_compress_2x_seq};
use binius_core::{constraint_system::ValueVec, word::Word};
use binius_frontend::{Circuit, CircuitBuilder, Wire};
use binius_iop::channel::OracleSpec;
use binius_prover::{IOPProver, protocols::shift::build_key_collection};
use binius_verifier::{IOPVerifier, config::B128};
use flock_core::field::F128;

pub const SHA256_IV: [u32; 8] = [
    0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19,
];

/// Each block contains sixteen SHA words, in the standard big-endian word order.
/// This is a compression chain; callers supply any desired SHA padding themselves.
pub fn chaining_value(blocks: &[[u32; 16]]) -> [u32; 8] {
    blocks
        .iter()
        .fold(SHA256_IV, |state, block| ref_compress(state, *block))
}

pub(super) struct ShaRelation {
    pub circuit: Circuit,
    pub blocks: Vec<[Wire; 16]>,
    output: [Wire; 8],
    pub verifier: IOPVerifier,
    prover: IOPProver,
}

impl ShaRelation {
    pub fn new(count: usize) -> Result<Self, Error> {
        tracing::info!(compressions = count, "building chained SHA circuit");
        let builder = CircuitBuilder::new();
        let blocks: Vec<[Wire; 16]> = (0..count)
            .map(|_| std::array::from_fn(|_| builder.add_witness()))
            .collect();
        let output = std::array::from_fn(|_| builder.add_inout());
        let mut state = State::iv(&builder);
        for pair in blocks.chunks_exact(2) {
            state = sha256_compress_2x_seq(&builder, state, [pair[0], pair[1]]);
        }
        let mask = builder.add_constant(Word(u32::MAX as u64));
        for (actual, expected) in state.0.into_iter().zip(output) {
            builder.assert_eq(
                "final_sha_chaining_value",
                builder.band(actual, mask),
                expected,
            );
        }
        tracing::info!("compiling chained SHA circuit");
        let circuit = builder.build();
        let cs = circuit.constraint_system();
        cs.validate().map_err(|e| Error::Binius(e.to_string()))?;
        if !cs.imul_constraints.is_empty() || !cs.bmul_constraints.is_empty() {
            return Err(Error::Invalid(
                "SHA circuit unexpectedly requires auxiliary oracles",
            ));
        }
        let verifier = IOPVerifier::new(cs.clone(), cs.log_public_words());
        tracing::info!("preparing SHA shift keys");
        let prover = IOPProver::new(verifier.clone(), build_key_collection(cs));
        Ok(Self {
            circuit,
            blocks,
            output,
            verifier,
            prover,
        })
    }

    pub fn public(&self, final_state: [u32; 8]) -> Vec<Word> {
        let cs = self.verifier.constraint_system();
        let mut public = vec![Word::ZERO; cs.n_public_words()];
        public[..cs.constants.len()].copy_from_slice(&cs.constants);
        for (wire, val) in self.output.iter().zip(final_state) {
            public[self.circuit.witness_index(*wire).0 as usize] = Word(val as u64);
        }
        public
    }

    pub fn populate(&self, blocks: &[[u32; 16]], final_state: [u32; 8]) -> Result<ValueVec, Error> {
        if blocks.len() != self.blocks.len() {
            return Err(Error::Invalid("SHA block count"));
        }
        let mut filler = self.circuit.new_witness_filler();
        for (wires, block) in self.blocks.iter().zip(blocks) {
            for (&wire, &word) in wires.iter().zip(block) {
                filler[wire] = Word(word as u64);
            }
        }
        for (&wire, word) in self.output.iter().zip(final_state) {
            filler[wire] = Word(word as u64);
        }
        self.circuit
            .populate_wire_witness(&mut filler)
            .map_err(|e| Error::Binius(e.to_string()))?;
        Ok(filler.into_value_vec())
    }

    pub fn pack(&self, witness: &ValueVec) -> Vec<F128> {
        let mut packed = vec![F128::ZERO; 1 << self.verifier.log_witness_elems()];
        for (dst, words) in packed.iter_mut().zip(witness.non_public().chunks(2)) {
            *dst = F128 {
                lo: words[0].0,
                hi: words.get(1).map_or(0, |w| w.0),
            };
        }
        packed
    }

    pub fn prove(
        &self,
        t: &mut Blake3Transcript,
        witness: &ValueVec,
    ) -> Result<(Vec<u128>, BinaryClaim), Error> {
        let mut channel = ProverChannel {
            transcript: t,
            messages: Vec::new(),
            spec: vec![OracleSpec::new(self.verifier.log_witness_elems())],
        };
        let alloc = binius_compute::GlobalAllocator;
        let (_, _, point, value) = self
            .prover
            .prove_to_evaluation::<_, binius_prover::OptimalPackedB128, _>(
                witness,
                &mut channel,
                &alloc,
            )
            .map_err(|e| Error::Binius(e.to_string()))?;
        if !channel.spec.is_empty() {
            return Err(Error::Invalid("unconsumed SHA oracle"));
        }
        Ok((channel.messages, evaluation_claim(&point, value)))
    }

    pub fn verify(
        &self,
        t: &mut Blake3Transcript,
        public: &[Word],
        messages: &[u128],
    ) -> Result<BinaryClaim, Error> {
        let mut channel = VerifierChannel {
            transcript: t,
            messages,
            spec: vec![OracleSpec::new(self.verifier.log_witness_elems())],
        };
        let (_, point, value) = self
            .verifier
            .verify_to_evaluation(public, &mut channel)
            .map_err(|e| Error::Binius(e.to_string()))?;
        if !channel.messages.is_empty() || !channel.spec.is_empty() {
            return Err(Error::Invalid("trailing SHA prefix data"));
        }
        Ok(evaluation_claim(&point, value))
    }
}

fn evaluation_claim(point: &[B128], value: B128) -> BinaryClaim {
    let convert = |x: B128| F128 {
        lo: u128::from(x.val()) as u64,
        hi: (u128::from(x.val()) >> 64) as u64,
    };
    let r: Vec<_> = point.iter().copied().map(convert).collect();
    BinaryClaim {
        low: super::sumcheck::eq_table(&r[..7]),
        high_point: r[7..].to_vec(),
        value: convert(value),
    }
}

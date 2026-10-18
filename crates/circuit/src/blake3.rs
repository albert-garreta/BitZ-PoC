//! BLAKE3 compression in the BitZ circuit language.
//!
//! Words, the adder and the lift are the SHA-256 module's: every modular
//! addition is a hinted wide sum plus one linear row, and XOR and rotations
//! are F2-linear, so they cost nothing. A word is lifted to the integer side
//! only when it enters a sum, and only once.

use std::array;

use crate::sha256::{ShaZ, UInt, UInt32, Word, ZValues, sum_32, uint_constant, uint_from_word};
use crate::{BoolWitness, Circuit};

/// BLAKE3's initial chaining value (the SHA-256 IV).
pub const IV: [u32; 8] = [
    0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19,
];

/// The message schedule permutation applied between rounds.
pub const MSG_PERMUTATION: [usize; 16] = [2, 6, 3, 10, 7, 0, 4, 13, 1, 11, 12, 5, 9, 14, 15, 8];

pub const CHUNK_START: u32 = 1;
pub const CHUNK_END: u32 = 2;
pub const PARENT: u32 = 4;
pub const ROOT: u32 = 8;

/// A compression input word: a public constant or a Boolean word.
#[derive(Clone)]
pub enum Input<BW: BoolWitness> {
    Const(u32),
    Word(Word<BW, 32>),
}

impl<BW: BoolWitness> Input<BW> {
    fn word(&self) -> Word<BW, 32> {
        match self {
            Self::Const(value) => Word::constant(u64::from(*value)),
            Self::Word(word) => word.clone(),
        }
    }
}

/// One state lane: its bits, and its integer value once it has been lifted.
struct Lane<CS: Circuit> {
    word: Word<CS::Bool, 32>,
    constant: Option<u32>,
    lifted: Option<ShaZ<CS>>,
}

impl<CS: Circuit> Lane<CS> {
    fn from_input(input: &Input<CS::Bool>) -> Self {
        Self {
            word: input.word(),
            constant: match input {
                Input::Const(value) => Some(*value),
                Input::Word(_) => None,
            },
            lifted: None,
        }
    }

    fn bits(word: Word<CS::Bool, 32>) -> Self {
        Self {
            word,
            constant: None,
            lifted: None,
        }
    }

    fn from_sum(sum: UInt32<CS>) -> Self {
        Self {
            word: sum.word,
            constant: None,
            lifted: Some(sum.z_values.low_32),
        }
    }

    fn uint(&mut self, circuit: &mut CS) -> UInt32<CS> {
        if let Some(value) = self.constant {
            return uint_constant::<CS>(u64::from(value));
        }
        let z = match &self.lifted {
            Some(z) => z.clone(),
            None => {
                let z = uint_from_word(circuit, self.word.clone()).int_value();
                self.lifted = Some(z.clone());
                z
            }
        };
        UInt {
            word: self.word.clone(),
            z_values: ZValues {
                full: z.clone(),
                low_32: z,
            },
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn g<CS: Circuit>(
    circuit: &mut CS,
    state: &mut [Lane<CS>; 16],
    message: &mut [Lane<CS>; 16],
    (a, b, c, d): (usize, usize, usize, usize),
    x: usize,
    y: usize,
) {
    let terms = [
        state[a].uint(circuit),
        state[b].uint(circuit),
        message[x].uint(circuit),
    ];
    state[a] = Lane::from_sum(sum_32::<34, 3, _>(circuit, terms));
    state[d] = Lane::bits(state[d].word.xor(circuit, &state[a].word).rotate_right(16));
    let terms = [state[c].uint(circuit), state[d].uint(circuit)];
    state[c] = Lane::from_sum(sum_32::<33, 2, _>(circuit, terms));
    state[b] = Lane::bits(state[b].word.xor(circuit, &state[c].word).rotate_right(12));
    let terms = [
        state[a].uint(circuit),
        state[b].uint(circuit),
        message[y].uint(circuit),
    ];
    state[a] = Lane::from_sum(sum_32::<34, 3, _>(circuit, terms));
    state[d] = Lane::bits(state[d].word.xor(circuit, &state[a].word).rotate_right(8));
    let terms = [state[c].uint(circuit), state[d].uint(circuit)];
    state[c] = Lane::from_sum(sum_32::<33, 2, _>(circuit, terms));
    state[b] = Lane::bits(state[b].word.xor(circuit, &state[c].word).rotate_right(7));
}

fn rounds<CS: Circuit>(
    circuit: &mut CS,
    mut state: [Lane<CS>; 16],
    mut message: [Lane<CS>; 16],
    cv_words: &[Word<CS::Bool, 32>; 8],
) -> [Word<CS::Bool, 32>; 16] {
    let mut schedule: [usize; 16] = array::from_fn(|i| i);
    for round in 0..7 {
        let m = schedule;
        g(circuit, &mut state, &mut message, (0, 4, 8, 12), m[0], m[1]);
        g(circuit, &mut state, &mut message, (1, 5, 9, 13), m[2], m[3]);
        g(circuit, &mut state, &mut message, (2, 6, 10, 14), m[4], m[5]);
        g(circuit, &mut state, &mut message, (3, 7, 11, 15), m[6], m[7]);
        g(circuit, &mut state, &mut message, (0, 5, 10, 15), m[8], m[9]);
        g(circuit, &mut state, &mut message, (1, 6, 11, 12), m[10], m[11]);
        g(circuit, &mut state, &mut message, (2, 7, 8, 13), m[12], m[13]);
        g(circuit, &mut state, &mut message, (3, 4, 9, 14), m[14], m[15]);
        if round + 1 < 7 {
            schedule = array::from_fn(|i| m[MSG_PERMUTATION[i]]);
        }
    }
    array::from_fn(|i| {
        if i < 8 {
            state[i].word.xor(circuit, &state[i + 8].word)
        } else {
            state[i].word.xor(circuit, &cv_words[i - 8])
        }
    })
}

/// The block kind of [`compress_block`].
pub const BLOCK_KIND: &str = "blake3-compression";

/// Input slots of a compression block: the chaining value (256 bits), the
/// message block (512), the counter (64), the block length and the flags
/// (32 each), in that order, words least-significant bit first.
pub const BLOCK_SLOTS: usize = 896;

fn input_bits<BW: BoolWitness>(input: &Input<BW>) -> impl Iterator<Item = BW> + '_ {
    (0..32).map(move |b| match input {
        Input::Const(v) => BW::from((v >> b) & 1 == 1),
        Input::Word(w) => w.bit(b),
    })
}

/// [`compress`] as one circuit block: every input, the counter, length and
/// flags included, is a slot, so all compressions record the same local
/// relation (constants become wiring). The output is the full 16-word state.
///
/// Output words 4..8 are b/d-lane XOR chains (~56 committed bits each) that
/// also carry the input CV's words 4..8: a caller chaining compressions
/// through the CV without bound (a transcript) must re-commit them, or its
/// supports grow with the chain.
pub fn compress_block<CS: Circuit>(
    circuit: &mut CS,
    cv: &[Input<CS::Bool>; 8],
    block: &[Input<CS::Bool>; 16],
    counter: u64,
    block_len: u32,
    flags: u32,
) -> [Word<CS::Bool, 32>; 16] {
    let tail = [Input::Const(counter as u32), Input::Const((counter >> 32) as u32), Input::Const(block_len), Input::Const(flags)];
    let bits: Vec<CS::Bool> = cv
        .iter()
        .chain(block.iter())
        .chain(tail.iter())
        .flat_map(|w| input_bits(w).collect::<Vec<_>>())
        .collect();
    debug_assert_eq!(bits.len(), BLOCK_SLOTS);
    let bits = match circuit.try_replay_block(BLOCK_KIND, bits) {
        Ok(back) => return array::from_fn(|k| Word::new(array::from_fn(|b| back[32 * k + b].clone()))),
        Err(bits) => bits,
    };
    let slots = circuit.begin_block(BLOCK_KIND, bits);
    let word = |k: usize| Word::<CS::Bool, 32>::new(array::from_fn(|b| slots[32 * k + b].clone()));
    let cv_words: [Word<CS::Bool, 32>; 8] = array::from_fn(word);
    let state: [Lane<CS>; 16] = array::from_fn(|i| match i {
        0..8 => Lane::bits(cv_words[i].clone()),
        8..12 => Lane::from_input(&Input::Const(IV[i - 8])),
        _ => Lane::bits(word(24 + i - 12)),
    });
    let message: [Lane<CS>; 16] = array::from_fn(|i| Lane::bits(word(8 + i)));
    let out = rounds(circuit, state, message, &cv_words);
    let flat: Vec<CS::Bool> = out.iter().flat_map(|w| (0..32).map(move |b| w.bit(b))).collect();
    let back = circuit.end_block(flat);
    array::from_fn(|k| Word::new(array::from_fn(|b| back[32 * k + b].clone())))
}

/// One BLAKE3 compression; returns the full 16-word output state. The
/// chaining value is `out[0..8]`, the extended output `out[8..16]`.
pub fn compress<CS: Circuit>(
    circuit: &mut CS,
    cv: &[Input<CS::Bool>; 8],
    block: &[Input<CS::Bool>; 16],
    counter: u64,
    block_len: u32,
    flags: u32,
) -> [Word<CS::Bool, 32>; 16] {
    let tail = [
        counter as u32,
        (counter >> 32) as u32,
        block_len,
        flags,
    ];
    let state: [Lane<CS>; 16] = array::from_fn(|i| match i {
        0..8 => Lane::from_input(&cv[i]),
        8..12 => Lane::from_input(&Input::Const(IV[i - 8])),
        _ => Lane::from_input(&Input::Const(tail[i - 12])),
    });
    // Message words keep their lifts across rounds: the schedule only permutes.
    let message: [Lane<CS>; 16] = array::from_fn(|i| Lane::from_input(&block[i]));
    let cv_words: [Word<CS::Bool, 32>; 8] = array::from_fn(|i| cv[i].word());
    rounds(circuit, state, message, &cv_words)
}

/// Native reference compression (BLAKE3 spec), for tests and witness checks.
pub fn compress_native(
    cv: &[u32; 8],
    block: &[u32; 16],
    counter: u64,
    block_len: u32,
    flags: u32,
) -> [u32; 16] {
    fn g(s: &mut [u32; 16], (a, b, c, d): (usize, usize, usize, usize), x: u32, y: u32) {
        s[a] = s[a].wrapping_add(s[b]).wrapping_add(x);
        s[d] = (s[d] ^ s[a]).rotate_right(16);
        s[c] = s[c].wrapping_add(s[d]);
        s[b] = (s[b] ^ s[c]).rotate_right(12);
        s[a] = s[a].wrapping_add(s[b]).wrapping_add(y);
        s[d] = (s[d] ^ s[a]).rotate_right(8);
        s[c] = s[c].wrapping_add(s[d]);
        s[b] = (s[b] ^ s[c]).rotate_right(7);
    }
    let mut s = [
        cv[0],
        cv[1],
        cv[2],
        cv[3],
        cv[4],
        cv[5],
        cv[6],
        cv[7],
        IV[0],
        IV[1],
        IV[2],
        IV[3],
        counter as u32,
        (counter >> 32) as u32,
        block_len,
        flags,
    ];
    let mut m = *block;
    for round in 0..7 {
        g(&mut s, (0, 4, 8, 12), m[0], m[1]);
        g(&mut s, (1, 5, 9, 13), m[2], m[3]);
        g(&mut s, (2, 6, 10, 14), m[4], m[5]);
        g(&mut s, (3, 7, 11, 15), m[6], m[7]);
        g(&mut s, (0, 5, 10, 15), m[8], m[9]);
        g(&mut s, (1, 6, 11, 12), m[10], m[11]);
        g(&mut s, (2, 7, 8, 13), m[12], m[13]);
        g(&mut s, (3, 4, 9, 14), m[14], m[15]);
        if round + 1 < 7 {
            m = array::from_fn(|i| m[MSG_PERMUTATION[i]]);
        }
    }
    for i in 0..8 {
        s[i] ^= s[i + 8];
        s[i + 8] ^= cv[i];
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::constraints::ConstraintGenerator;
    use crate::stats::{Dummy, Stats};
    use crate::witgen::Witgen;

    fn word_value(word: &Word<bool, 32>) -> u32 {
        (0..32).fold(0u32, |v, i| v | (u32::from(word.bit(i)) << i))
    }

    fn sample(seed: u64) -> ([u32; 8], [u32; 16]) {
        let mut state = seed;
        let mut next = || {
            state = state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            (state >> 32) as u32
        };
        (array::from_fn(|_| next()), array::from_fn(|_| next()))
    }

    #[test]
    fn native_reference_matches_the_blake3_crate() {
        // One 64-byte root chunk: CHUNK_START | CHUNK_END | ROOT, counter 0.
        let bytes: Vec<u8> = (0..64u8).collect();
        let block: [u32; 16] =
            array::from_fn(|i| u32::from_le_bytes(bytes[4 * i..4 * i + 4].try_into().unwrap()));
        let out = compress_native(&IV, &block, 0, 64, CHUNK_START | CHUNK_END | ROOT);
        let expected = blake3::hash(&bytes);
        let got: Vec<u8> = out[..8].iter().flat_map(|w| w.to_le_bytes()).collect();
        assert_eq!(got.as_slice(), expected.as_bytes());
    }

    #[test]
    fn circuit_compression_matches_native_and_satisfies_its_matrices() {
        for seed in 0..3u64 {
            let (cv, block) = sample(seed);
            let counter = 0x1234_5678_9abc_def0u64.rotate_left(seed as u32);
            let (len, flags) = (64, CHUNK_START | PARENT);
            let expected = compress_native(&cv, &block, counter, len, flags);

            // Inputs: the eight CV words, then the sixteen block words.
            let inputs: Vec<bool> = cv
                .iter()
                .chain(block.iter())
                .flat_map(|w| (0..32).map(move |i| (w >> i) & 1 == 1))
                .collect();
            let mut witgen = Witgen::with_inputs(&inputs);
            let word = |w: u32| Input::Word(Word::<bool, 32>::new(array::from_fn(|i| (w >> i) & 1 == 1)));
            let out = compress(
                &mut witgen,
                &cv.map(word),
                &block.map(word),
                counter,
                len,
                flags,
            );
            assert_eq!(out.map(|w| word_value(&w)), expected, "seed {seed}");

            let mut generator = ConstraintGenerator::new(inputs.len());
            let symbolic: Vec<_> = (0..inputs.len()).map(|i| generator.input(i)).collect();
            let sym_word = |w: usize| {
                Input::Word(Word::new(array::from_fn(|i| symbolic[32 * w + i].clone())))
            };
            let _ = compress(
                &mut generator,
                &array::from_fn(sym_word),
                &array::from_fn(|i| sym_word(8 + i)),
                counter,
                len,
                flags,
            );
            let matrices = generator.into_matrices();
            matrices
                .check_witness(witgen.witness())
                .expect("BLAKE3 witness satisfies M/A/B/C");
        }
    }

    #[test]
    fn block_compression_matches_native() {
        use crate::stats::{Dummy, Stats};
        for seed in 0..3u64 {
            let (cv, block) = sample(seed);
            let counter = 0x0fed_cba9_8765_4321u64.rotate_left(seed as u32);
            let expected = compress_native(&cv, &block, counter, 33, CHUNK_END | PARENT);
            let mut witgen = Witgen::new();
            let word = |w: u32| Input::Word(Word::<bool, 32>::new(array::from_fn(|i| (w >> i) & 1 == 1)));
            let cv_in: [Input<bool>; 8] = array::from_fn(|i| if i == 2 { Input::Const(cv[i]) } else { word(cv[i]) });
            let out = compress_block(&mut witgen, &cv_in, &block.map(word), counter, 33, CHUNK_END | PARENT);
            assert_eq!(out.map(|w| word_value(&w)), expected, "seed {seed}");
        }
        let mut stats = Stats::new(0);
        let cv: [Input<Dummy>; 8] = array::from_fn(|_| Input::Word(Word::new([Dummy; 32])));
        let block: [Input<Dummy>; 16] = array::from_fn(|_| Input::Word(Word::new([Dummy; 32])));
        let _ = compress_block(&mut stats, &cv, &block, 0, 64, 0);
        println!("BLAKE3 block: {stats:?}");
    }

    #[test]
    fn compression_cost() {
        let mut stats = Stats::new(768);
        let cv: [Input<Dummy>; 8] = array::from_fn(|_| Input::Word(Word::new([Dummy; 32])));
        let block: [Input<Dummy>; 16] = array::from_fn(|_| Input::Word(Word::new([Dummy; 32])));
        let _ = compress(&mut stats, &cv, &block, 0, 64, 0);
        // 56 G functions × (34 + 33 + 34 + 33) hinted sum bits.
        assert_eq!(stats.witnesses - 768, 56 * 134);
        // Sum outputs 134/G, four XOR-word lifts per G except the two that
        // only feed the output XOR in the last round (b, d of the last G of
        // each lane), the 16 message lifts and the 8 CV lifts.
        println!("BLAKE3 compression: {stats:?}");
        assert!(stats.bitz_calls < 16_000);
    }
}

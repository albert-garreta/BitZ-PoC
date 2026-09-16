//! Their `tooling/cli/src/sha256.rs` (d6b637e): the public SHA-256
//! compression and raw compression-chain statements, on this crate's
//! vendored circuit trait (`f2z` where theirs says `bitz`). Blocks and the
//! chaining value are public; no padding is added. Plus the seeded
//! derivation the parity probes share (SplitMix64, 8 bytes per output,
//! little-endian, 64 bytes per block) and the inverse of `public_bytes`.

use circuit::Circuit;
use circuit::sha256::{INITIAL_STATE, Word, compress};

use super::e2e::{CircuitStatement, Error};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sha256Circuit {
    Compression,
    Chain,
}

impl Sha256Circuit {
    pub fn name(self) -> &'static str {
        match self {
            Self::Compression => "sha256-compression",
            Self::Chain => "sha256-chain",
        }
    }

    pub fn parse(name: &str) -> Option<Self> {
        match name {
            "sha256-compression" => Some(Self::Compression),
            "sha256-chain" => Some(Self::Chain),
            _ => None,
        }
    }
}

/// Every block and the final chaining value are public. Compression accepts
/// a public initial state; Chain starts at the standard IV.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Sha256Statement {
    pub circuit: Sha256Circuit,
    pub blocks: Vec<[u32; 16]>,
    pub initial_state: [u32; 8],
    pub digest: [u32; 8],
}

impl Sha256Statement {
    /// `blocks` 64-byte blocks from `seed`, the chaining value after them.
    pub fn seeded(circuit: Sha256Circuit, blocks: usize, seed: u64) -> Self {
        let mut state = seed;
        let mut digest = INITIAL_STATE;
        let blocks = (0..blocks)
            .map(|_| {
                let mut bytes = [0u8; 64];
                for chunk in bytes.chunks_exact_mut(8) {
                    chunk.copy_from_slice(&splitmix64(&mut state).to_le_bytes());
                }
                sha256_compress(&mut digest, &bytes);
                std::array::from_fn(|i| {
                    u32::from_be_bytes(std::array::from_fn(|j| bytes[4 * i + j]))
                })
            })
            .collect();
        Self {
            circuit,
            blocks,
            initial_state: INITIAL_STATE,
            digest,
        }
    }

    /// The inverse of [`CircuitStatement::public_bytes`]: `u64` block count,
    /// the initial state, the blocks, the digest, all `u32` LE.
    pub fn from_public_bytes(circuit: Sha256Circuit, public: &[u8]) -> Option<Self> {
        if public.len() < 8 {
            return None;
        }
        let count = u64::from_le_bytes(public[..8].try_into().ok()?) as usize;
        let words: Vec<u32> = public[8..]
            .chunks_exact(4)
            .map(|c| u32::from_le_bytes(c.try_into().expect("4 bytes")))
            .collect();
        if words.len() != 8 + 16 * count + 8 || (public.len() - 8) % 4 != 0 {
            return None;
        }
        Some(Self {
            circuit,
            initial_state: words[..8].try_into().ok()?,
            blocks: (0..count)
                .map(|b| words[8 + 16 * b..8 + 16 * (b + 1)].try_into().expect("16 words"))
                .collect(),
            digest: words[8 + 16 * count..].try_into().ok()?,
        })
    }

    /// The input bits: every block word, least-significant bit first.
    pub fn input(&self) -> Vec<bool> {
        self.blocks
            .iter()
            .flatten()
            .flat_map(|word| (0..32).map(move |bit| word >> bit & 1 != 0))
            .collect()
    }

    fn validate(&self) -> Result<(), Error> {
        if self.blocks.is_empty() {
            return Err(Error::Input("SHA chain must contain at least one block"));
        }
        match self.circuit {
            Sha256Circuit::Compression if self.blocks.len() != 1 => {
                Err(Error::Input("compression requires one block"))
            }
            Sha256Circuit::Chain if self.initial_state != INITIAL_STATE => {
                Err(Error::Input("chain requires the standard IV"))
            }
            _ => Ok(()),
        }
    }
}

impl CircuitStatement for Sha256Statement {
    fn domain(&self) -> &'static [u8] {
        match self.circuit {
            Sha256Circuit::Compression => b"sha256-compression/v1",
            Sha256Circuit::Chain => b"sha256-chain/v1",
        }
    }

    fn public_bytes(&self) -> Vec<u8> {
        let mut bytes = (self.blocks.len() as u64).to_le_bytes().to_vec();
        bytes.extend(
            self.initial_state
                .iter()
                .chain(self.blocks.iter().flatten())
                .chain(self.digest.iter())
                .flat_map(|word| word.to_le_bytes()),
        );
        bytes
    }

    fn input_bits(&self) -> usize {
        self.blocks.len() * 512
    }

    fn synthesize<CS: Circuit>(&self, cs: &mut CS, inputs: &[CS::Bool]) -> Result<(), Error> {
        self.validate()?;
        if inputs.len() != self.input_bits() {
            return Err(Error::Input("wrong SHA input length"));
        }
        for (bit, expected) in inputs.iter().zip(self.input()) {
            constrain_bit(cs, bit.clone(), expected);
        }
        let mut state = self
            .initial_state
            .map(|word| Word::constant(u64::from(word)));
        for bits in inputs.chunks_exact(512) {
            let block = std::array::from_fn(|word| {
                Word::new(std::array::from_fn(|bit| bits[word * 32 + bit].clone()))
            });
            state = compress(cs, block, state).map(|value| value.word);
        }
        for (word, expected) in state.iter().zip(self.digest) {
            for bit in 0..32 {
                constrain_bit(cs, word.bit(bit), expected >> bit & 1 != 0);
            }
        }
        Ok(())
    }
}

fn constrain_bit<CS: Circuit>(cs: &mut CS, bit: CS::Bool, expected: bool) {
    let value = cs.f2z::<1>(bit);
    let expected = CS::Z::<1>::from(CS::Coefficient::<1>::from(u64::from(expected)));
    cs.assert_r1c::<1>(
        CS::Z::<1>::from(CS::Coefficient::<1>::from(1u64)),
        value,
        expected,
    );
}

/// The probes' seed derivation.
pub fn splitmix64(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// FIPS 180-4 compression of one block (no padding), for the expected
/// chaining value.
pub fn sha256_compress(state: &mut [u32; 8], block: &[u8; 64]) {
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5,
        0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
        0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
        0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
        0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
        0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
        0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
    ];
    let mut w = [0u32; 64];
    for i in 0..16 {
        w[i] = u32::from_be_bytes([block[4 * i], block[4 * i + 1], block[4 * i + 2], block[4 * i + 3]]);
    }
    for i in 16..64 {
        let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
        let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
        w[i] = w[i - 16].wrapping_add(s0).wrapping_add(w[i - 7]).wrapping_add(s1);
    }
    let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut h] = *state;
    for i in 0..64 {
        let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
        let ch = (e & f) ^ (!e & g);
        let t1 = h.wrapping_add(s1).wrapping_add(ch).wrapping_add(K[i]).wrapping_add(w[i]);
        let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
        let maj = (a & b) ^ (a & c) ^ (b & c);
        let t2 = s0.wrapping_add(maj);
        h = g;
        g = f;
        f = e;
        e = d.wrapping_add(t1);
        d = c;
        c = b;
        b = a;
        a = t1.wrapping_add(t2);
    }
    for (s, v) in state.iter_mut().zip([a, b, c, d, e, f, g, h]) {
        *s = s.wrapping_add(v);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_compression_matches_the_fips_abc_vector() {
        let mut state = INITIAL_STATE;
        let mut block = [0u8; 64];
        block[..3].copy_from_slice(b"abc");
        block[3] = 0x80;
        block[63] = 0x18;
        sha256_compress(&mut state, &block);
        assert_eq!(
            state,
            [0xba7816bf, 0x8f01cfea, 0x414140de, 0x5dae2223, 0xb00361a3, 0x96177a9c, 0xb410ff61, 0xf20015ad]
        );
    }

    #[test]
    fn public_bytes_round_trip() {
        let statement = Sha256Statement::seeded(Sha256Circuit::Chain, 3, 7);
        let public = statement.public_bytes();
        assert_eq!(public.len(), 8 + 4 * (8 + 16 * 3 + 8));
        assert_eq!(Sha256Statement::from_public_bytes(Sha256Circuit::Chain, &public), Some(statement));
    }
}

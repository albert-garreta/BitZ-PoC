//! Byte strings of the in-circuit transcripts: every byte public or eight F2
//! combinations (bit `i` of weight `2^i`), and the SHA-256 word views of
//! them (big-endian words, least-significant bit first in the circuit).

use std::array;

use circuit::sha256::Word;
use circuit::{BoolRepresentation, BoolWitness, Circuit, HintResult, PackedBits};

use super::val::{G, u128_of};

/// One transcript byte.
#[derive(Clone)]
pub enum Byte<B: BoolWitness> {
    P(u8),
    V([B; 8]),
}

impl<B: BoolWitness> Byte<B> {
    pub fn is_public(&self) -> bool {
        matches!(self, Self::P(_))
    }

    pub fn bit(&self, i: usize) -> B {
        match self {
            Self::P(v) => B::from((v >> i) & 1 == 1),
            Self::V(bits) => bits[i].clone(),
        }
    }
}

/// Public bytes.
pub fn public_bytes<B: BoolWitness>(bytes: &[u8]) -> Vec<Byte<B>> {
    bytes.iter().map(|&b| Byte::P(b)).collect()
}

/// The little-endian bytes of `bits.len() / 8` bytes' worth of bits.
pub fn bytes_of_bits<B: BoolWitness>(bits: &[B]) -> Vec<Byte<B>> {
    assert!(bits.len().is_multiple_of(8));
    bits.chunks(8)
        .map(|c| Byte::V(array::from_fn(|i| c[i].clone())))
        .collect()
}

/// The 16 wire bytes of a GF(2^128) value (`lo` then `hi`, little-endian:
/// bit `i` of the element is bit `i % 8` of byte `i / 8`).
pub fn gf_bytes<B: BoolWitness>(g: &G<B>) -> Vec<Byte<B>> {
    match g {
        G::P(v) => public_bytes(&u128_of(*v).to_le_bytes()),
        G::V(bits) => bytes_of_bits(&bits[..]),
    }
}

/// Commits `bytes` as proof data: fresh committed bits, one hint per 16
/// bytes.
pub fn witness_bytes<CS: Circuit>(cs: &mut CS, bytes: &[u8]) -> Vec<Byte<CS::Bool>> {
    let mut out = Vec::with_capacity(bytes.len());
    for chunk in bytes.chunks(16) {
        let mut padded = [0u8; 16];
        padded[..chunk.len()].copy_from_slice(chunk);
        let value = u128::from_le_bytes(padded);
        let packed = cs.hint::<1, 128, 2, _>(move |_| {
            HintResult::Ok(PackedBits::from_words([value as u64, (value >> 64) as u64]))
        });
        for j in 0..chunk.len() {
            out.push(Byte::V(array::from_fn(|i| packed.bit(8 * j + i))));
        }
    }
    out
}

/// A big-endian SHA-256 word from four bytes.
pub fn word_of_bytes<B: BoolWitness>(bytes: &[Byte<B>]) -> Word<B, 32> {
    assert_eq!(bytes.len(), 4);
    // Word bit k (LSB first) is bit k % 8 of byte 3 − k / 8.
    Word::new(array::from_fn(|k| bytes[3 - k / 8].bit(k % 8)))
}

/// The 16 words of a 64-byte block.
pub fn block_words<B: BoolWitness>(block: &[Byte<B>]) -> [Word<B, 32>; 16] {
    assert_eq!(block.len(), 64);
    array::from_fn(|w| word_of_bytes(&block[4 * w..4 * w + 4]))
}

/// The four big-endian bytes of a word.
pub fn bytes_of_word<B: BoolWitness>(word: &Word<B, 32>) -> [Byte<B>; 4] {
    array::from_fn(|j| Byte::V(array::from_fn(|i| word.bit(8 * (3 - j) + i))))
}

/// The 32 big-endian bytes of eight words (the SHA-256 digest order).
pub fn bytes_of_state<B: BoolWitness>(state: &[Word<B, 32>; 8]) -> Vec<Byte<B>> {
    state.iter().flat_map(|w| bytes_of_word(w)).collect()
}

/// The bits of a byte string, bit `8j + i` = bit `i` of byte `j`.
pub fn bits_of_bytes<B: BoolWitness>(bytes: &[Byte<B>]) -> Vec<B> {
    bytes.iter().flat_map(|b| (0..8).map(move |i| b.bit(i))).collect()
}

/// The 64 little-endian bits of a committed `u64` (a nonce).
pub fn witness_u64<CS: Circuit>(cs: &mut CS, value: u64) -> [CS::Bool; 64] {
    let packed = cs.hint::<1, 64, 1, _>(move |_| HintResult::Ok(PackedBits::from_u64(value)));
    array::from_fn(|i| packed.bit(i))
}

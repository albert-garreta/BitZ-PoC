//! Sixteen independent single-block BLAKE3 compressions in AVX-512 lanes.
//!
//! Only AVX512F is required: native word rotates replace the AVX2 byte
//! shuffles and shift pairs. All entry points require a runtime feature check.

use super::{FLAGS, IV, MSG_SCHEDULE};
use core::arch::x86_64::*;

const LANES: usize = 16;

#[inline(always)]
unsafe fn g(v: &mut [__m512i; 16], a: usize, b: usize, c: usize, d: usize, x: __m512i, y: __m512i) {
    // SAFETY: this helper is only called by the AVX512F-enabled compression.
    unsafe {
        v[a] = _mm512_add_epi32(_mm512_add_epi32(v[a], v[b]), x);
        v[d] = _mm512_ror_epi32::<16>(_mm512_xor_si512(v[d], v[a]));
        v[c] = _mm512_add_epi32(v[c], v[d]);
        v[b] = _mm512_ror_epi32::<12>(_mm512_xor_si512(v[b], v[c]));
        v[a] = _mm512_add_epi32(_mm512_add_epi32(v[a], v[b]), y);
        v[d] = _mm512_ror_epi32::<8>(_mm512_xor_si512(v[d], v[a]));
        v[c] = _mm512_add_epi32(v[c], v[d]);
        v[b] = _mm512_ror_epi32::<7>(_mm512_xor_si512(v[b], v[c]));
    }
}

/// A public, fixed BLAKE3 message schedule lets the compiler resolve every
/// message index before entering the nonce scan.
#[inline(always)]
unsafe fn round<const ROUND: usize>(state: &mut [__m512i; 16], message: &[__m512i; 16]) {
    let word = |i: usize| message[MSG_SCHEDULE[ROUND][i]];
    // SAFETY: callers enable AVX512F; this is the existing BLAKE3 G map.
    unsafe {
        g(state, 0, 4, 8, 12, word(0), word(1));
        g(state, 1, 5, 9, 13, word(2), word(3));
        g(state, 2, 6, 10, 14, word(4), word(5));
        g(state, 3, 7, 11, 15, word(6), word(7));
        g(state, 0, 5, 10, 15, word(8), word(9));
        g(state, 1, 6, 11, 12, word(10), word(11));
        g(state, 2, 7, 8, 13, word(12), word(13));
        g(state, 3, 4, 9, 14, word(14), word(15));
    }
}

/// First hash words in nonce order. The caller supplies at most 14 prefix
/// words and a base whose sixteen nonces fit in u64.
#[inline(always)]
unsafe fn compress_first_words(prefix: &[u32], base: u64) -> __m512i {
    // SAFETY: callers enable AVX512F and bound both the prefix and nonce range.
    unsafe {
        let zero = _mm512_setzero_si512();
        let mut message = [zero; 16];
        let mut state = [zero; 16];
        for (slot, &word) in message.iter_mut().zip(prefix) {
            *slot = _mm512_set1_epi32(word as i32);
        }
        let base_low = _mm512_set1_epi32(base as i32);
        let low = _mm512_add_epi32(
            base_low,
            _mm512_setr_epi32(0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15),
        );
        // Carry into the high word independently for each lane when a batch
        // crosses 2^32. The caller guarantees that no complete nonce overflows.
        let carry = _mm512_cmplt_epu32_mask(low, base_low);
        let high = _mm512_set1_epi32((base >> 32) as i32);
        message[prefix.len()] = low;
        message[prefix.len() + 1] = _mm512_mask_add_epi32(high, carry, high, _mm512_set1_epi32(1));
        for (slot, &word) in state.iter_mut().zip(IV.iter().chain(&IV[..4])) {
            *slot = _mm512_set1_epi32(word as i32);
        }
        state[14] = _mm512_set1_epi32((4 * (prefix.len() + 2)) as i32);
        state[15] = _mm512_set1_epi32(FLAGS as i32);
        for schedule in &MSG_SCHEDULE {
            let word = |i: usize| message[schedule[i]];
            g(&mut state, 0, 4, 8, 12, word(0), word(1));
            g(&mut state, 1, 5, 9, 13, word(2), word(3));
            g(&mut state, 2, 6, 10, 14, word(4), word(5));
            g(&mut state, 3, 7, 11, 15, word(6), word(7));
            g(&mut state, 0, 5, 10, 15, word(8), word(9));
            g(&mut state, 1, 6, 11, 12, word(10), word(11));
            g(&mut state, 2, 7, 8, 13, word(12), word(13));
            g(&mut state, 3, 4, 9, 14, word(14), word(15));
        }
        _mm512_xor_si512(state[0], state[8])
    }
}

/// Seed-dependent state before the first nonce-dependent G operation. All
/// sixteen lanes share the seed; only the two nonce message words vary.
struct Prepared {
    message: [__m512i; 16],
    state: [__m512i; 16],
}

impl Prepared {
    #[inline(always)]
    unsafe fn new<const WORDS: usize>(prefix: &[u32]) -> Self {
        debug_assert!(matches!(WORDS, 4 | 8) && prefix.len() == WORDS);
        // SAFETY: callers enable AVX512F and choose one of the two seed sizes.
        unsafe {
            let zero = _mm512_setzero_si512();
            let mut message = [zero; 16];
            let mut state = [zero; 16];
            for (slot, &word) in message.iter_mut().zip(prefix) {
                *slot = _mm512_set1_epi32(word as i32);
            }
            for (slot, &word) in state.iter_mut().zip(IV.iter().chain(&IV[..4])) {
                *slot = _mm512_set1_epi32(word as i32);
            }
            state[14] = _mm512_set1_epi32((4 * (WORDS + 2)) as i32);
            state[15] = _mm512_set1_epi32(FLAGS as i32);
            g(&mut state, 0, 4, 8, 12, message[0], message[1]);
            g(&mut state, 1, 5, 9, 13, message[2], message[3]);
            g(&mut state, 3, 7, 11, 15, message[6], message[7]);
            if WORDS == 8 {
                g(&mut state, 2, 6, 10, 14, message[4], message[5]);
                // Round-zero diagonals are disjoint. Only diagonal zero
                // contains the nonce for a 32-byte seed.
                g(&mut state, 1, 6, 11, 12, message[10], message[11]);
                g(&mut state, 2, 7, 8, 13, message[12], message[13]);
                g(&mut state, 3, 4, 9, 14, message[14], message[15]);
            }
            Self { message, state }
        }
    }

    #[inline(always)]
    unsafe fn first_words<const WORDS: usize>(&self, base: u64) -> __m512i {
        // SAFETY: AVX512F is enabled and the complete nonce batch fits in u64.
        unsafe {
            let mut message = self.message;
            let mut state = self.state;
            let base_low = _mm512_set1_epi32(base as i32);
            let low = _mm512_add_epi32(
                base_low,
                _mm512_setr_epi32(0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15),
            );
            let carry = _mm512_cmplt_epu32_mask(low, base_low);
            let high = _mm512_set1_epi32((base >> 32) as i32);
            message[WORDS] = low;
            message[WORDS + 1] = _mm512_mask_add_epi32(high, carry, high, _mm512_set1_epi32(1));
            if WORDS == 4 {
                g(&mut state, 2, 6, 10, 14, message[4], message[5]);
            }
            g(&mut state, 0, 5, 10, 15, message[8], message[9]);
            if WORDS == 4 {
                g(&mut state, 1, 6, 11, 12, message[10], message[11]);
                g(&mut state, 2, 7, 8, 13, message[12], message[13]);
                g(&mut state, 3, 4, 9, 14, message[14], message[15]);
            }
            round::<1>(&mut state, &message);
            round::<2>(&mut state, &message);
            round::<3>(&mut state, &message);
            round::<4>(&mut state, &message);
            round::<5>(&mut state, &message);
            // The first digest word is state[0] XOR state[8]. Final-round
            // diagonals one and three affect neither word; unused outputs of
            // the other two diagonals are dead after inlining. A >32-bit
            // difficulty still checks every candidate against the full hash.
            let word = |i: usize| message[MSG_SCHEDULE[6][i]];
            g(&mut state, 0, 4, 8, 12, word(0), word(1));
            g(&mut state, 1, 5, 9, 13, word(2), word(3));
            g(&mut state, 2, 6, 10, 14, word(4), word(5));
            g(&mut state, 3, 7, 11, 15, word(6), word(7));
            g(&mut state, 0, 5, 10, 15, word(8), word(9));
            g(&mut state, 2, 7, 8, 13, word(12), word(13));
            _mm512_xor_si512(state[0], state[8])
        }
    }
}

#[cfg(test)]
#[target_feature(enable = "avx512f")]
pub(super) unsafe fn first_words(prefix: &[u32], base: u64) -> [u32; LANES] {
    let mut output = [0u32; LANES];
    // SAFETY: AVX512F is enabled, the caller supplies bounded inputs, and
    // output contains exactly 64 writable bytes with no alignment requirement.
    unsafe {
        let words = match prefix.len() {
            4 => Prepared::new::<4>(prefix).first_words::<4>(base),
            8 => Prepared::new::<8>(prefix).first_words::<8>(base),
            _ => compress_first_words(prefix, base),
        };
        _mm512_storeu_si512(output.as_mut_ptr().cast(), words);
    }
    output
}

#[target_feature(enable = "avx512f")]
pub(super) unsafe fn first_pow_nonce(
    prefix: &[u8],
    start: u64,
    end: u64,
    bits: u32,
) -> Option<u64> {
    debug_assert!(prefix.len() <= super::MAX_PREFIX_LEN && prefix.len() % 4 == 0);
    let mut words = [0u32; super::MAX_PREFIX_LEN / 4];
    for (word, bytes) in words.iter_mut().zip(prefix.chunks_exact(4)) {
        *word = u32::from_le_bytes(bytes.try_into().expect("four-byte prefix word"));
    }
    let words = &words[..prefix.len() / 4];
    // Prepare once per scan chunk, not once per sixteen attempted nonces.
    // Other supported prefix lengths retain the general compression path.
    unsafe {
        match words.len() {
            4 => {
                let prepared = Prepared::new::<4>(words);
                scan(prefix, start, end, bits, |base| {
                    prepared.first_words::<4>(base)
                })
            }
            8 => {
                let prepared = Prepared::new::<8>(words);
                scan(prefix, start, end, bits, |base| {
                    prepared.first_words::<8>(base)
                })
            }
            _ => scan(prefix, start, end, bits, |base| {
                compress_first_words(words, base)
            }),
        }
    }
}

#[target_feature(enable = "avx512f")]
unsafe fn scan(
    prefix: &[u8],
    start: u64,
    end: u64,
    bits: u32,
    compress: impl Fn(u64) -> __m512i,
) -> Option<u64> {
    // Hash bytes are tested most-significant-bit first, but compression words
    // are little endian. Swap the zero-bit mask into compression word order.
    let mask = u32::MAX
        .checked_shl(32 - bits.min(32))
        .unwrap_or(0)
        .swap_bytes();
    let mask = _mm512_set1_epi32(mask as i32);
    let mut base = start;
    while end.saturating_sub(base) >= LANES as u64 {
        // SAFETY: AVX512F is enabled and a complete batch fits below end;
        // therefore every nonce addition is in range, including near u64::MAX.
        let hashes = compress(base);
        let mut candidates = _mm512_testn_epi32_mask(hashes, mask);
        while candidates != 0 {
            let nonce = base + u64::from(candidates.trailing_zeros());
            if bits <= 32 || super::pow_ok(prefix, nonce, bits) {
                return Some(nonce);
            }
            candidates &= candidates - 1;
        }
        base += LANES as u64;
    }
    // Includes empty/reversed ranges and the final zero to fifteen nonces.
    (base..end).find(|&nonce| super::pow_ok(prefix, nonce, bits))
}

/// Benchmark oracle: the original complete seven-round compression.
#[cfg(test)]
#[target_feature(enable = "avx512f")]
pub(super) unsafe fn first_pow_nonce_generic(
    prefix: &[u8],
    start: u64,
    end: u64,
    bits: u32,
) -> Option<u64> {
    let mut words = [0u32; super::MAX_PREFIX_LEN / 4];
    for (word, bytes) in words.iter_mut().zip(prefix.chunks_exact(4)) {
        *word = u32::from_le_bytes(bytes.try_into().unwrap());
    }
    let words = &words[..prefix.len() / 4];
    unsafe {
        scan(prefix, start, end, bits, |base| {
            compress_first_words(words, base)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn high_difficulty_candidates_use_the_complete_hash() {
        if !std::is_x86_feature_detected!("avx512f") {
            return;
        }
        for len in [16, 32] {
            let prefix: Vec<_> = (0..len).map(|i| (i * 37 + 11) as u8).collect();
            for bits in [33, 64, 128, 256, 257] {
                for (start, end) in [(3, 52), (u64::MAX - 49, u64::MAX)] {
                    let expected = (start..end).find(|&n| super::super::pow_ok(&prefix, n, bits));
                    // Force every SIMD lane through the full-hash fallback.
                    // SAFETY: AVX512F was checked and scan bounds each batch.
                    let actual =
                        unsafe { scan(&prefix, start, end, bits, |_| _mm512_setzero_si512()) };
                    assert_eq!(actual, expected, "len {len} bits {bits} start {start}");
                }
            }
        }
    }
}

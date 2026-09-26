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

#[cfg(test)]
#[target_feature(enable = "avx512f")]
pub(super) unsafe fn first_words(prefix: &[u32], base: u64) -> [u32; LANES] {
    let mut output = [0u32; LANES];
    // SAFETY: AVX512F is enabled, the caller supplies bounded inputs, and
    // output contains exactly 64 writable bytes with no alignment requirement.
    unsafe {
        _mm512_storeu_si512(
            output.as_mut_ptr().cast(),
            compress_first_words(prefix, base),
        );
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
        let hashes = unsafe { compress_first_words(words, base) };
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

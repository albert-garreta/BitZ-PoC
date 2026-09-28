//! AVX2 byte-shuffle selection from the sixteen-bit committed row entries.
//! The parent gates this module on AVX2; all loads and stores are unaligned.

use core::arch::x86_64::*;
use std::mem::MaybeUninit;

use super::Selector;

/// Four vectors of 32 nibble indices in their original column order.
#[inline(always)]
unsafe fn nibbles(src: *const u16) -> [__m256i; 4] {
    // SAFETY: the caller supplies 32 consecutive initialized entries.
    unsafe {
        let a = _mm256_loadu_si256(src.cast());
        let b = _mm256_loadu_si256(src.add(16).cast());
        let byte_mask = _mm256_set1_epi16(0xFF);
        // PACKUS works within 128-bit lanes. Restore the order of its four
        // eight-byte groups from [a0, b0, a1, b1] to [a0, a1, b0, b1].
        let lo = _mm256_permute4x64_epi64::<0xD8>(_mm256_packus_epi16(
            _mm256_and_si256(a, byte_mask),
            _mm256_and_si256(b, byte_mask),
        ));
        let hi = _mm256_permute4x64_epi64::<0xD8>(_mm256_packus_epi16(
            _mm256_srli_epi16::<8>(a),
            _mm256_srli_epi16::<8>(b),
        ));
        let mask = _mm256_set1_epi8(15);
        [
            _mm256_and_si256(lo, mask),
            _mm256_and_si256(_mm256_srli_epi16::<4>(lo), mask),
            _mm256_and_si256(hi, mask),
            _mm256_and_si256(_mm256_srli_epi16::<4>(hi), mask),
        ]
    }
}

#[inline(always)]
unsafe fn tables(sel: &Selector) -> [__m256i; 4] {
    // SAFETY: every table contains 16 initialized bytes. Replication puts
    // that same table in both independent byte-shuffle lanes.
    unsafe {
        std::array::from_fn(|i| {
            _mm256_broadcastsi128_si256(_mm_loadu_si128(sel.tables[i].as_ptr().cast()))
        })
    }
}

#[inline(always)]
unsafe fn apply(t: &[__m256i; 4], n: &[__m256i; 4]) -> __m256i {
    unsafe {
        _mm256_or_si256(
            _mm256_or_si256(
                _mm256_shuffle_epi8(t[0], n[0]),
                _mm256_shuffle_epi8(t[1], n[1]),
            ),
            _mm256_or_si256(
                _mm256_shuffle_epi8(t[2], n[2]),
                _mm256_shuffle_epi8(t[3], n[3]),
            ),
        )
    }
}

#[inline(always)]
pub(super) fn select<const K: usize>(
    block: &[u16; 64],
    sel: &[Selector; K],
    out: &mut [[u8; 64]; K],
) {
    // SAFETY: each half reads 32 of 64 entries and writes 32 of 64 bytes.
    // The module is compiled only when AVX2 is enabled.
    unsafe {
        let tables: [[__m256i; 4]; K] = std::array::from_fn(|i| tables(&sel[i]));
        for first in [0, 32] {
            let n = nibbles(block.as_ptr().add(first));
            for i in 0..K {
                _mm256_storeu_si256(out[i].as_mut_ptr().add(first).cast(), apply(&tables[i], &n));
            }
        }
    }
}

#[inline(always)]
pub(super) fn select_pair(a: &[u16; 64], b: &[u16; 64], sa: &Selector, sb: &Selector) -> [u8; 64] {
    let mut out = [0u8; 64];
    // SAFETY: as select; the two selections are combined before storing,
    // without materializing two additional intermediate byte arrays.
    unsafe {
        let ta = tables(sa);
        let tb = tables(sb);
        for first in [0, 32] {
            let va = apply(&ta, &nibbles(a.as_ptr().add(first)));
            let vb = apply(&tb, &nibbles(b.as_ptr().add(first)));
            _mm256_storeu_si256(out.as_mut_ptr().add(first).cast(), _mm256_or_si256(va, vb));
        }
    }
    out
}

#[inline(always)]
pub(super) fn interleave_bytes(lo: &[u8; 64], hi: &[u8; 64], out: &mut [MaybeUninit<u16>]) {
    assert_eq!(out.len(), 64);
    // SAFETY: each half reads 32 bytes per input and initializes 32 u16s.
    unsafe {
        for first in [0, 32] {
            let l = _mm256_loadu_si256(lo.as_ptr().add(first).cast());
            let h = _mm256_loadu_si256(hi.as_ptr().add(first).cast());
            let low = _mm256_unpacklo_epi8(l, h);
            let high = _mm256_unpackhi_epi8(l, h);
            _mm256_storeu_si256(
                out.as_mut_ptr().add(first).cast(),
                _mm256_permute2x128_si256::<0x20>(low, high),
            );
            _mm256_storeu_si256(
                out.as_mut_ptr().add(first + 16).cast(),
                _mm256_permute2x128_si256::<0x31>(low, high),
            );
        }
    }
}

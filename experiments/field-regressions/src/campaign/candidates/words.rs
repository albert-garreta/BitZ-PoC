//! Fixed-width primitive candidates; signed results use two's-complement bits.
use super::pass;
use crate::{Case, Rng, case_requested, measure};
use crypto_bigint::{Choice, CtSelect, Int, Uint};
use num_bigint::{BigInt, BigUint};
use num_traits::{CheckedAdd, CheckedSub, Zero};
use std::{array, hint::black_box};

#[cfg(test)]
#[path = "correctness/words.rs"]
mod tests;

#[inline(always)]
fn add<const L: usize>(a: &[u64; L], b: &[u64; L]) -> ([u64; L], bool) {
    let mut out = [0; L];
    let mut c = 0u128;
    for i in 0..L {
        c = a[i] as u128 + b[i] as u128 + c;
        out[i] = c as u64;
        c >>= 64;
    }
    (out, c != 0)
}
#[inline(always)]
fn sub<const L: usize>(a: &[u64; L], b: &[u64; L]) -> ([u64; L], bool) {
    let mut out = [0; L];
    let mut c = false;
    for i in 0..L {
        let (x, b0) = a[i].overflowing_sub(b[i]);
        let (x, b1) = x.overflowing_sub(c as u64);
        out[i] = x;
        c = b0 | b1;
    }
    (out, c)
}

/// Widening unsigned multiplication. O is public and must cover every output
/// word. The loops depend only on declared widths, including zero operands.
#[inline(always)]
fn wide<const A: usize, const B: usize, const O: usize>(a: &[u64; A], b: &[u64; B]) -> [u64; O] {
    assert!(O >= A + B);
    let mut out = [0; O];
    for i in 0..A {
        let mut c = 0u128;
        for j in 0..B {
            let s = a[i] as u128 * b[j] as u128 + out[i + j] as u128 + c;
            out[i + j] = s as u64;
            c = s >> 64;
        }
        out[i + B] = c as u64;
    }
    out
}
fn uint<const L: usize>(a: &[u64; L]) -> BigUint {
    BigUint::from_bytes_le(&a.iter().flat_map(|x| x.to_le_bytes()).collect::<Vec<_>>())
}
fn signed<const L: usize>(a: &[u64; L]) -> BigInt {
    BigInt::from_signed_bytes_le(&a.iter().flat_map(|x| x.to_le_bytes()).collect::<Vec<_>>())
}

// Compile the operation and signedness into the batch kernel. In particular,
// one/two-word values should use native overflow flags, not a generic u128
// carry loop plus per-element operation dispatch.
#[inline(always)]
fn native_op<const L: usize, const SUB: bool>(a: &[u64; L], b: &[u64; L]) -> ([u64; L], bool) {
    if L == 1 {
        let (v, overflow) = if SUB {
            a[0].overflowing_sub(b[0])
        } else {
            a[0].overflowing_add(b[0])
        };
        (array::from_fn(|_| v), overflow)
    } else if L == 2 {
        let a = a[0] as u128 | (a[1] as u128) << 64;
        let b = b[0] as u128 | (b[1] as u128) << 64;
        let (v, overflow) = if SUB {
            a.overflowing_sub(b)
        } else {
            a.overflowing_add(b)
        };
        (array::from_fn(|i| (v >> (i * 64)) as u64), overflow)
    } else if SUB {
        sub(a, b)
    } else {
        add(a, b)
    }
}

#[inline(always)]
fn wrapping_batch<const L: usize, const SUB: bool>(
    pairs: &[([u64; L], [u64; L])],
    out: &mut [Uint<L>],
) {
    for ((a, b), out) in pairs.iter().zip(out) {
        *out = Uint::from_words(native_op::<L, SUB>(a, b).0);
    }
}

#[inline(always)]
fn checked_batch<const L: usize, const SIGNED: bool, const SUB: bool>(
    pairs: &[([u64; L], [u64; L])],
    out: &mut [Option<Uint<L>>],
) {
    for ((a, b), out) in pairs.iter().zip(out) {
        let (r, carry) = native_op::<L, SUB>(a, b);
        let overflow = if SIGNED {
            let ab = a[L - 1] ^ b[L - 1];
            let ar = a[L - 1] ^ r[L - 1];
            ((if SUB { ab } else { !ab }) & ar) >> 63 != 0
        } else {
            carry
        };
        *out = if overflow {
            None
        } else {
            Some(Uint::from_words(r))
        };
    }
}
#[inline(always)]
fn limb_checked_batch<const L: usize, const SUB: bool>(
    pairs: &[([u64; L], [u64; L])],
    out: &mut [Option<Uint<L>>],
) {
    for ((a, b), out) in pairs.iter().zip(out) {
        let a = Uint::from_words(*a);
        let b = Uint::from_words(*b);
        let (r, flag) = if SUB {
            a.borrowing_sub(&b, crypto_bigint::Limb::ZERO)
        } else {
            a.carrying_add(&b, crypto_bigint::Limb::ZERO)
        };
        *out = if flag.0 == 0 { Some(r) } else { None };
    }
}

fn primitives<const L: usize>(samples: usize, rng: &mut Rng) {
    use crypto_primitives::{
        crypto_bigint_int::Int as OldInt, crypto_bigint_uint::Uint as OldUint,
    };
    for n in [16, 1024] {
        let size = format!("l{L}_n{n}");
        if ![
            "opt_uint_add",
            "opt_uint_sub",
            "opt_uint_checked_add",
            "opt_uint_checked_sub",
            "opt_int_checked_add",
            "opt_int_checked_sub",
        ]
        .iter()
        .any(|f| case_requested(f, &size))
        {
            continue;
        }
        let pairs: Vec<([u64; L], [u64; L])> = (0..n)
            .map(|i| match i % 6 {
                0 => ([u64::MAX; L], array::from_fn(|j| u64::from(j == 0))),
                1 => ([0; L], [u64::MAX; L]),
                2 => (
                    array::from_fn(|j| if j == L - 1 { 1 << 63 } else { 0 }),
                    [u64::MAX; L],
                ),
                3 => (
                    array::from_fn(|j| if j == L - 1 { u64::MAX >> 1 } else { u64::MAX }),
                    array::from_fn(|j| u64::from(j == 0)),
                ),
                _ => (
                    array::from_fn(|_| rng.next()),
                    array::from_fn(|_| rng.next()),
                ),
            })
            .collect();
        for subtract in [false, true] {
            let f = if subtract {
                "opt_uint_sub"
            } else {
                "opt_uint_add"
            };
            let expected: Vec<_> = pairs
                .iter()
                .map(|(a, b)| {
                    let a = circuit::witgen::Z::<L>::from_le_words(a);
                    let b = circuit::witgen::Z::<L>::from_le_words(b);
                    Uint::from_words(*(if subtract { a - b } else { a + b }).words())
                })
                .collect();
            let mut cases = vec![
                pass("circuit_z", n * L * 24, &pairs, &expected, |p, o| {
                    for ((a, b), o) in p.iter().zip(o) {
                        let a = circuit::witgen::Z::<L>::from_le_words(a);
                        let b = circuit::witgen::Z::<L>::from_le_words(b);
                        *o = Uint::from_words(*(if subtract { a - b } else { a + b }).words());
                    }
                }),
                pass("word_carry", n * L * 24, &pairs, &expected, |p, o| {
                    for ((a, b), o) in p.iter().zip(o) {
                        *o = Uint::from_words(if subtract { sub(a, b).0 } else { add(a, b).0 });
                    }
                }),
                if subtract {
                    pass("native_batch", n * L * 24, &pairs, &expected, |p, o| {
                        wrapping_batch::<L, true>(p, o)
                    })
                } else {
                    pass("native_batch", n * L * 24, &pairs, &expected, |p, o| {
                        wrapping_batch::<L, false>(p, o)
                    })
                },
            ];
            measure(f, &size, &mut cases, samples, rng);
            for signed in [false, true] {
                let family = match (signed, subtract) {
                    (false, false) => "opt_uint_checked_add",
                    (false, true) => "opt_uint_checked_sub",
                    (true, false) => "opt_int_checked_add",
                    (true, true) => "opt_int_checked_sub",
                };
                let baseline = |a: [u64; L], b: [u64; L]| -> Option<Uint<L>> {
                    if signed {
                        let a = OldInt::from_words(a);
                        let b = OldInt::from_words(b);
                        (if subtract {
                            a.checked_sub(&b)
                        } else {
                            a.checked_add(&b)
                        })
                        .map(|x| Uint::from_words(*x.inner().as_words()))
                    } else {
                        let a = OldUint::from_words(a);
                        let b = OldUint::from_words(b);
                        (if subtract {
                            a.checked_sub(&b)
                        } else {
                            a.checked_add(&b)
                        })
                        .map(|x| Uint::from_words(*x.as_words()))
                    }
                };
                let expected: Vec<_> = pairs.iter().map(|&(a, b)| baseline(a, b)).collect();
                let mut cases = vec![
                    pass("vendor_option", n * L * 24, &pairs, &expected, |p, o| {
                        for (&(a, b), o) in p.iter().zip(o) {
                            *o = baseline(a, b);
                        }
                    }),
                    pass("word_option", n * L * 24, &pairs, &expected, |p, o| {
                        for ((a, b), o) in p.iter().zip(o) {
                            let (r, carry) = if subtract { sub(a, b) } else { add(a, b) };
                            let overflow = if signed {
                                let ab = a[L - 1] ^ b[L - 1];
                                let ar = a[L - 1] ^ r[L - 1];
                                ((if subtract { ab } else { !ab }) & ar) >> 63 != 0
                            } else {
                                carry
                            };
                            *o = if overflow {
                                None
                            } else {
                                Some(Uint::from_words(r))
                            };
                        }
                    }),
                    match (signed, subtract) {
                        (false, false) => {
                            pass("native_option", n * L * 24, &pairs, &expected, |p, o| {
                                checked_batch::<L, false, false>(p, o)
                            })
                        }
                        (false, true) => {
                            pass("native_option", n * L * 24, &pairs, &expected, |p, o| {
                                checked_batch::<L, false, true>(p, o)
                            })
                        }
                        (true, false) => {
                            pass("native_option", n * L * 24, &pairs, &expected, |p, o| {
                                checked_batch::<L, true, false>(p, o)
                            })
                        }
                        (true, true) => {
                            pass("native_option", n * L * 24, &pairs, &expected, |p, o| {
                                checked_batch::<L, true, true>(p, o)
                            })
                        }
                    },
                ];
                if !signed {
                    cases.push(if subtract {
                        pass("limb_option", n * L * 24, &pairs, &expected, |p, o| {
                            limb_checked_batch::<L, true>(p, o)
                        })
                    } else {
                        pass("limb_option", n * L * 24, &pairs, &expected, |p, o| {
                            limb_checked_batch::<L, false>(p, o)
                        })
                    });
                }
                measure(family, &size, &mut cases, samples, rng);
            }
        }
    }
}

fn products<const A: usize, const B: usize, const O: usize>(samples: usize, rng: &mut Rng) {
    assert_eq!(O, A + B);
    for n in [16, 1024] {
        let size = format!("a{A}_b{B}_n{n}");
        if !case_requested("opt_exact_product", &size) {
            continue;
        }
        let pairs: Vec<([u64; A], [u64; B])> = (0..n)
            .map(|i| {
                if i % 7 == 0 {
                    ([u64::MAX; A], [u64::MAX; B])
                } else {
                    (
                        array::from_fn(|_| rng.next()),
                        array::from_fn(|_| rng.next()),
                    )
                }
            })
            .collect();
        let expected: Vec<_> = pairs
            .iter()
            .map(|(a, b)| {
                let (lo, hi) = Uint::<A>::from_words(*a).widening_mul(&Uint::<B>::from_words(*b));
                Uint::<O>::from_words(array::from_fn(|i| {
                    if i < A {
                        lo.as_words()[i]
                    } else {
                        hi.as_words()[i - A]
                    }
                }))
            })
            .collect();
        for ((a, b), r) in pairs.iter().zip(&expected) {
            assert_eq!(uint(r.as_words()), uint(a) * uint(b));
        }
        let mut cases = vec![
            pass(
                "crypto_bigint",
                n * (A + B + O) * 8,
                &pairs,
                &expected,
                |p, o| {
                    for ((a, b), o) in p.iter().zip(o) {
                        let (lo, hi) =
                            Uint::<A>::from_words(*a).widening_mul(&Uint::<B>::from_words(*b));
                        *o = Uint::<O>::from_words(array::from_fn(|i| {
                            if i < A {
                                lo.as_words()[i]
                            } else {
                                hi.as_words()[i - A]
                            }
                        }));
                    }
                },
            ),
            pass(
                "schoolbook",
                n * (A + B + O) * 8,
                &pairs,
                &expected,
                |p, o| {
                    for ((a, b), o) in p.iter().zip(o) {
                        *o = Uint::from_words(wide::<A, B, O>(a, b));
                    }
                },
            ),
        ];
        measure("opt_exact_product", &size, &mut cases, samples, rng);
    }
}

// Magnitude and sign conversion is included for the signed reference. The
// candidate accumulates in two's complement directly with fixed carry loops.
fn exact<const L: usize, const O: usize, const SIGNED: bool, const FUSED: bool>(
    a: &[[u64; L]],
    b: &[[u64; L]],
) -> [u64; O] {
    assert_eq!(O, 2 * L + 1);
    assert_eq!(a.len(), b.len());
    let mut out = [0; O];
    for (a, b) in a.iter().zip(b) {
        if FUSED {
            for i in 0..L {
                let mut c = 0u128;
                for j in 0..L {
                    let s = a[i] as u128 * b[j] as u128 + out[i + j] as u128 + c;
                    out[i + j] = s as u64;
                    c = s >> 64;
                }
                for x in &mut out[i + L..] {
                    let s = *x as u128 + c;
                    *x = s as u64;
                    c = s >> 64;
                }
            }
            if SIGNED {
                for (negative, operand) in [(a[L - 1] >> 63, b), (b[L - 1] >> 63, a)] {
                    let mask = 0u64.wrapping_sub(negative);
                    let mut borrow = false;
                    for j in 0..L {
                        let (x, b0) = out[L + j].overflowing_sub(operand[j] & mask);
                        let (x, b1) = x.overflowing_sub(borrow as u64);
                        out[L + j] = x;
                        borrow = b0 | b1;
                    }
                    out[2 * L] = out[2 * L].wrapping_sub(borrow as u64);
                }
                out[2 * L] = out[2 * L].wrapping_add((a[L - 1] >> 63) & (b[L - 1] >> 63));
            }
        } else {
            let words = if SIGNED {
                let (lo, hi, sign) =
                    Int::<L>::from_words(*a).widening_mul(&Int::<L>::from_words(*b));
                let magnitude = Uint::<O>::from_words(array::from_fn(|i| {
                    if i < L {
                        lo.as_words()[i]
                    } else if i < 2 * L {
                        hi.as_words()[i - L]
                    } else {
                        0
                    }
                }));
                magnitude.wrapping_neg_if(sign).to_words()
            } else {
                let (lo, hi) = Uint::<L>::from_words(*a).widening_mul(&Uint::<L>::from_words(*b));
                array::from_fn(|i| {
                    if i < L {
                        lo.as_words()[i]
                    } else if i < 2 * L {
                        hi.as_words()[i - L]
                    } else {
                        0
                    }
                })
            };
            out = add(&out, &words).0;
        }
    }
    out
}
fn exact_cases<const L: usize, const O: usize>(samples: usize, rng: &mut Rng) {
    for n in [0, 1, 3, 17, 16, 1024] {
        let size = format!("l{L}_n{n}");
        if [16, 1024].contains(&n)
            && !["opt_exact_signed_mac", "opt_exact_unsigned_mac"]
                .iter()
                .any(|f| case_requested(f, &size))
        {
            continue;
        }
        let a: Vec<_> = (0..n)
            .map(|i| {
                if i % 3 == 0 {
                    [u64::MAX; L]
                } else {
                    array::from_fn(|_| rng.next())
                }
            })
            .collect();
        let b: Vec<_> = (0..n)
            .map(|i| {
                if i % 5 == 0 {
                    array::from_fn(|j| if j == L - 1 { 1 << 63 } else { 0 })
                } else {
                    array::from_fn(|_| rng.next())
                }
            })
            .collect();
        macro_rules! run {
            ($signed:literal,$family:literal) => {{
                let expected = exact::<L, O, $signed, false>(&a, &b);
                assert_eq!(exact::<L, O, $signed, true>(&a, &b), expected);
                if $signed {
                    assert_eq!(exact_signed_columns::<L, O>(&a, &b), expected);
                }
                if !$signed {
                    assert_eq!(exact_columns::<L, O>(&a, &b), expected);
                }
                if $signed {
                    assert_eq!(
                        signed(&expected),
                        a.iter()
                            .zip(&b)
                            .fold(BigInt::zero(), |s, (a, b)| s + signed(a) * signed(b))
                    );
                } else {
                    assert_eq!(
                        uint(&expected),
                        a.iter()
                            .zip(&b)
                            .fold(BigUint::zero(), |s, (a, b)| s + uint(a) * uint(b))
                    );
                }
                if [16, 1024].contains(&n) {
                    let mut cases = vec![
                        Case::new("wide_then_add", n * L * 16, || {
                            black_box(exact::<L, O, $signed, false>(black_box(&a), black_box(&b)));
                        }),
                        Case::new("fused_exact", n * L * 16, || {
                            black_box(exact::<L, O, $signed, true>(black_box(&a), black_box(&b)));
                        }),
                    ];
                    if $signed {
                        cases.push(Case::new("signed_columns", n * L * 16, || {
                            black_box(exact_signed_columns::<L, O>(black_box(&a), black_box(&b)));
                        }));
                    }
                    if !$signed {
                        assert_eq!(exact_columns::<L, O>(&a, &b), expected);
                        cases.push(Case::new("column_exact", n * L * 16, || {
                            black_box(exact_columns::<L, O>(black_box(&a), black_box(&b)));
                        }));
                    }
                    measure($family, &size, &mut cases, samples, rng);
                }
            }};
        }
        run!(false, "opt_exact_unsigned_mac");
        run!(true, "opt_exact_signed_mac");
    }
}

// Each column receives at most 2L words per pair. The public term bound
// leaves one further word of room for the final carry from the prior column.
#[inline(never)]
fn exact_columns<const L: usize, const O: usize>(a: &[[u64; L]], b: &[[u64; L]]) -> [u64; O] {
    assert_eq!(O, 2 * L + 1);
    assert_eq!(a.len(), b.len());
    assert!(a.len() <= (u64::MAX / (2 * L + 1) as u64) as usize);
    let mut columns = [0u128; O];
    for (a, b) in a.iter().zip(b) {
        for i in 0..L {
            for j in 0..L {
                let p = a[i] as u128 * b[j] as u128;
                columns[i + j] += (p as u64) as u128;
                columns[i + j + 1] += p >> 64;
            }
        }
    }
    let mut carry = 0;
    array::from_fn(|i| {
        let v = columns[i] + carry;
        carry = v >> 64;
        v as u64
    })
}

// For radix B=2^64, a signed value is U - sign*B^L. Accumulate the
// unsigned product and both sign corrections in signed columns, then carry
// once. Each column gets at most 2L product words and two correction words
// per term. The public bound leaves i128 headroom, including the final carry.
// Input signs select masks only; every term executes the same limb schedule.
#[inline(never)]
fn exact_signed_columns<const L: usize, const O: usize>(
    a: &[[u64; L]],
    b: &[[u64; L]],
) -> [u64; O] {
    assert!(L > 0);
    assert_eq!(O, 2 * L + 1);
    assert_eq!(a.len(), b.len());
    assert!(a.len() <= (u64::MAX / (4 * L + 8) as u64) as usize);
    let mut columns = [0i128; O];
    for (a, b) in a.iter().zip(b) {
        for i in 0..L {
            for j in 0..L {
                let p = a[i] as u128 * b[j] as u128;
                columns[i + j] += (p as u64) as i128;
                columns[i + j + 1] += (p >> 64) as i128;
            }
        }
        let sa = a[L - 1] >> 63;
        let sb = b[L - 1] >> 63;
        // The compiler turned ordinary 0-sign masks into branches that
        // skipped correction limbs. CtSelect's backend keeps these masks
        // opaque to that optimization and emits conditional selection.
        let ma = 0u64.ct_select(&u64::MAX, Choice::from_u8_lsb(sa as u8));
        let mb = 0u64.ct_select(&u64::MAX, Choice::from_u8_lsb(sb as u8));
        for j in 0..L {
            columns[L + j] -= (b[j] & ma) as i128 + (a[j] & mb) as i128;
        }
        columns[2 * L] += (sa & sb) as i128;
    }
    let mut carry = 0i128;
    array::from_fn(|i| {
        let v = columns[i] + carry;
        carry = v >> 64;
        v as u64
    })
}
pub(super) fn run(samples: usize, rng: &mut Rng) {
    primitives::<1>(samples, rng);
    primitives::<2>(samples, rng);
    primitives::<4>(samples, rng);
    primitives::<9>(samples, rng);
    products::<1, 1, 2>(samples, rng);
    products::<2, 2, 4>(samples, rng);
    products::<4, 4, 8>(samples, rng);
    products::<9, 9, 18>(samples, rng);
    products::<2, 9, 11>(samples, rng);
    products::<32, 32, 64>(samples, rng);
    products::<64, 64, 128>(samples, rng);
    exact_cases::<2, 5>(samples, rng);
    exact_cases::<4, 9>(samples, rng);
    exact_cases::<9, 19>(samples, rng);
}

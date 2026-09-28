//! Cross sums for the four-variable leaf skip, with one direct bucket
//! scatter per column block. No pair-index blocks are materialized.

use std::time::{Duration, Instant};

#[cfg(feature = "parallel")]
use rayon::prelude::*;

use super::leaf_skip::byte_patterns;
use super::{Forest, Gf, bit_sums, eq_table, transposed_eq};

const BUCKETS: usize = 16 * 256;

#[derive(Clone, Copy, Default)]
struct Cost {
    clear: Duration,
    scatter: Duration,
    contract: Duration,
}

impl Cost {
    fn add(self, other: Self) -> Self {
        Self {
            clear: self.clear + other.clear,
            scatter: self.scatter + other.scatter,
            contract: self.contract + other.contract,
        }
    }
}

impl Forest<'_> {
    /// The same 16-by-16 cross matrix as `leaf_skip_cross`, with each
    /// column's eight nibbles and equality weight loaded once for all
    /// sixteen pair-bucket updates.
    pub(super) fn leaf_skip_cross_fast(&self, external: &[Gf]) -> [Gf; 256] {
        self.leaf_skip_cross_fast_impl::<false>(external).0
    }

    fn leaf_skip_cross_fast_impl<const PROFILE: bool>(&self, external: &[Gf]) -> ([Gf; 256], Cost) {
        assert!(self.t >= 6);
        assert_eq!(external.len(), self.t - 1 + self.s);
        let row_bits = self.t - 5;
        let rows = 1usize << row_bits;
        let groups = (1usize << self.s).div_ceil(64);
        let eq_t = transposed_eq(&eq_table(&external[..self.s]));
        let eq_y = eq_table(&external[self.s..self.s + row_bits]);
        let nibbles = self.nibbles();
        let row = |y: usize, buckets: &mut [Gf], acc: &mut [Gf; 256]| {
            let started = PROFILE.then(Instant::now);
            buckets.fill(Gf::zero());
            let cleared = PROFILE.then(Instant::now);
            for g in 0..groups {
                let patterns = byte_patterns(self, nibbles, y, g);
                scatter(buckets, &patterns, &eq_t[g * 64..(g + 1) * 64]);
            }
            let scattered = PROFILE.then(Instant::now);
            let delta = |p: usize| -> [Gf; 16] {
                std::array::from_fn(|a| {
                    self.images[y | (a << row_bits) | (p << (self.t - 1))] - Gf::one()
                })
            };
            contract_row(buckets, &delta(0), &delta(1), eq_y[y], acc);
            if PROFILE {
                Cost {
                    clear: cleared.unwrap() - started.unwrap(),
                    scatter: scattered.unwrap() - cleared.unwrap(),
                    contract: scattered.unwrap().elapsed(),
                }
            } else {
                Cost::default()
            }
        };
        let add = |(mut a, ca): ([Gf; 256], Cost), (b, cb): ([Gf; 256], Cost)| {
            for (x, y) in a.iter_mut().zip(b) {
                *x += y;
            }
            (a, ca.add(cb))
        };
        #[cfg(feature = "parallel")]
        let result = (0..rows)
            .into_par_iter()
            .with_min_len(1)
            .fold(
                || {
                    (
                        vec![Gf::zero(); BUCKETS],
                        [Gf::zero(); 256],
                        Cost::default(),
                    )
                },
                |(mut buckets, mut acc, cost), y| {
                    let current = row(y, &mut buckets, &mut acc);
                    (buckets, acc, cost.add(current))
                },
            )
            .map(|(_, acc, cost)| (acc, cost))
            .reduce(|| ([Gf::zero(); 256], Cost::default()), add);
        #[cfg(not(feature = "parallel"))]
        let result = {
            let _ = add;
            let mut buckets = vec![Gf::zero(); BUCKETS];
            let mut acc = [Gf::zero(); 256];
            let mut cost = Cost::default();
            for y in 0..rows {
                cost = cost.add(row(y, &mut buckets, &mut acc));
            }
            (acc, cost)
        };
        result
    }
}

#[inline(always)]
fn nibbles(patterns: &[[u8; 64]; 4], m: usize) -> ([usize; 4], [usize; 4]) {
    let e0 = patterns[0][m] as usize;
    let e1 = patterns[1][m] as usize;
    let o0 = patterns[2][m] as usize;
    let o1 = patterns[3][m] as usize;
    (
        [e0 & 15, e1 & 15, e0 >> 4, e1 >> 4],
        [o0 & 15, o1 & 15, o0 >> 4, o1 >> 4],
    )
}

#[cfg_attr(
    all(target_arch = "aarch64", target_feature = "neon"),
    allow(dead_code)
)]
fn scatter_portable(buckets: &mut [Gf], patterns: &[[u8; 64]; 4], weights: &[Gf]) {
    assert_eq!(buckets.len(), BUCKETS);
    assert_eq!(weights.len(), 64);
    for m in 0..64 {
        let (e, o) = nibbles(patterns, m);
        let weight = weights[m];
        for (i, &ep) in e.iter().enumerate() {
            let base = 1024 * i + 16 * ep;
            for (j, &op) in o.iter().enumerate() {
                buckets[base + 256 * j + op] += weight;
            }
        }
    }
}

#[inline]
fn scatter(buckets: &mut [Gf], patterns: &[[u8; 64]; 4], weights: &[Gf]) {
    #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
    neon::scatter(buckets, patterns, weights);
    #[cfg(not(all(target_arch = "aarch64", target_feature = "neon")))]
    scatter_portable(buckets, patterns, weights);
}

#[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
mod neon {
    use super::{BUCKETS, Gf, nibbles};
    use std::arch::aarch64::*;

    pub(super) fn scatter(buckets: &mut [Gf], patterns: &[[u8; 64]; 4], weights: &[Gf]) {
        assert_eq!(buckets.len(), BUCKETS);
        assert_eq!(weights.len(), 64);
        // SAFETY: Gf is repr(C, align(16)) with two u64 limbs. Every nibble
        // is below16; the largest field index is15*256+255=4095. Each
        // update completes before another update can alias its entry.
        unsafe {
            let base = buckets.as_mut_ptr().cast::<u64>();
            for m in 0..64 {
                let (e, o) = nibbles(patterns, m);
                let weight = vld1q_u64(weights.get_unchecked(m) as *const Gf as *const u64);
                for (i, &ep) in e.iter().enumerate() {
                    let row = base.add(2 * (1024 * i + 16 * ep));
                    let p0 = row.add(2 * o[0]);
                    let p1 = row.add(2 * (256 + o[1]));
                    let p2 = row.add(2 * (512 + o[2]));
                    let p3 = row.add(2 * (768 + o[3]));
                    vst1q_u64(p0, veorq_u64(vld1q_u64(p0), weight));
                    vst1q_u64(p1, veorq_u64(vld1q_u64(p1), weight));
                    vst1q_u64(p2, veorq_u64(vld1q_u64(p2), weight));
                    vst1q_u64(p3, veorq_u64(vld1q_u64(p3), weight));
                }
            }
        }
    }
}

#[inline(always)]
fn corner(group: usize, bit: usize) -> usize {
    8 * (group >> 1) + 2 * bit + (group & 1)
}

/// Contract each 4-by-4 corner block as soon as its joint marginals are
/// available. Only diagonal blocks compute single-bit marginals, and the
/// full 16-by-16 marginal matrix is never written to a temporary array.
fn contract_row(buckets: &[Gf], de: &[Gf; 16], do_: &[Gf; 16], weight: Gf, acc: &mut [Gf; 256]) {
    let mut single_e = [Gf::zero(); 16];
    let mut single_o = [Gf::zero(); 16];
    let weighted_e: [Gf; 16] = std::array::from_fn(|a| weight * de[a]);
    let mut total_weight = Gf::zero();
    for e in 0..4 {
        for o in 0..4 {
            let bucket = &buckets[(4 * e + o) * 256..(4 * e + o + 1) * 256];
            let mut by_o = [[Gf::zero(); 16]; 4];
            let mut total = [Gf::zero(); 16];
            for pattern in 0..16 {
                let (sum, bits) = bit_sums(&bucket[pattern * 16..(pattern + 1) * 16]);
                if e == o {
                    total[pattern] = sum;
                }
                for bit in 0..4 {
                    by_o[bit][pattern] = bits[bit];
                }
            }
            for (o_bit, column) in by_o.iter().enumerate() {
                let (sum, bits) = bit_sums(column);
                let b = corner(o, o_bit);
                if e == o {
                    single_o[b] = sum;
                }
                for (e_bit, &joint) in bits.iter().enumerate() {
                    let a = corner(e, e_bit);
                    acc[16 * a + b] += weighted_e[a] * do_[b] * joint;
                }
            }
            if e == o {
                let (sum, bits) = bit_sums(&total);
                total_weight = sum;
                for (bit, &marginal) in bits.iter().enumerate() {
                    single_e[corner(e, bit)] = marginal;
                }
            }
        }
    }
    let base = weight * total_weight;
    let left: [Gf; 16] = std::array::from_fn(|a| base + weighted_e[a] * single_e[a]);
    let right: [Gf; 16] = std::array::from_fn(|b| weight * do_[b] * single_o[b]);
    for a in 0..16 {
        for b in 0..16 {
            acc[16 * a + b] += left[a] + right[b];
        }
    }
}

/// An evaluation/extrapolation experiment for the same degree-30 Q.
/// This computes each factor at31 univariate nodes before multiplying,
/// so it does not replace their interpolation by a multivariate product.
#[cfg(test)]
mod evaluation {
    use super::*;
    use crate::cfg_into_iter;
    use crate::utils::wide_mul::WideMulAcc;
    use crate::wfbitz::leaf_skip::{SkipPolynomial, basis};
    use std::sync::OnceLock;

    struct Plan {
        weights: [[Gf; 16]; 31],
        inverse: [Gf; 32],
    }

    fn plan() -> &'static Plan {
        static PLAN: OnceLock<Plan> = OnceLock::new();
        PLAN.get_or_init(|| Plan {
            weights: std::array::from_fn(|i| basis(node(i))),
            inverse: std::array::from_fn(|i| {
                if i == 0 {
                    Gf::zero()
                } else {
                    Gf::one() / node(i)
                }
            }),
        })
    }

    fn node(i: usize) -> Gf {
        Gf::from_polynomial_words([i as u64, 0])
    }

    fn interpolate(mut divided: [Gf; 31]) -> SkipPolynomial {
        for order in 1..31 {
            for i in (order..31).rev() {
                // Polynomial-basis nodes subtract by XOR in characteristic2.
                divided[i] = (divided[i] - divided[i - 1]) * plan().inverse[i ^ (i - order)];
            }
        }
        let mut out = [Gf::zero(); 31];
        out[0] = divided[30];
        for (degree, i) in (0..30).rev().enumerate() {
            for j in (0..=degree + 1).rev() {
                let previous = if j == 0 { Gf::zero() } else { out[j - 1] };
                out[j] = previous - node(i) * out[j];
            }
            out[0] += divided[i];
        }
        out
    }

    impl Forest<'_> {
        pub(super) fn leaf_skip_polynomial_evaluated(&self, external: &[Gf]) -> SkipPolynomial {
            assert!(self.t >= 6);
            assert_eq!(external.len(), self.t - 1 + self.s);
            let row_bits = self.t - 5;
            let rows = 1usize << row_bits;
            let groups = (1usize << self.s).div_ceil(64);
            let eq_t = transposed_eq(&eq_table(&external[..self.s]));
            let eq_y = eq_table(&external[self.s..self.s + row_bits]);
            let nibbles = self.nibbles();
            let basis = &plan().weights;
            let partials: Vec<[Gf; 31]> = cfg_into_iter!(0..rows, 1)
                .map(|y| {
                    let mut tables = vec![Gf::zero(); 31 * 1024];
                    for z in 0..31 {
                        for p in 0..2 {
                            let coefficients: [Gf; 16] = std::array::from_fn(|a| {
                                basis[z][a]
                                    * (self.images[y | (a << row_bits) | (p << (self.t - 1))]
                                        - Gf::one())
                            });
                            for parity in 0..2 {
                                let at = z * 1024 + p * 512 + parity * 256;
                                tables[at] = if parity == 0 { Gf::one() } else { Gf::zero() };
                                for pattern in 1..256usize {
                                    let bit = pattern.trailing_zeros() as usize;
                                    tables[at + pattern] = tables[at + (pattern & (pattern - 1))]
                                        + coefficients[2 * bit + parity];
                                }
                            }
                        }
                    }
                    let mut sums = vec![<Gf as WideMulAcc>::wide_zero(&Gf::zero()); 31];
                    for g in 0..groups {
                        let patterns = byte_patterns(self, nibbles, y, g);
                        let weights = &eq_t[g * 64..(g + 1) * 64];
                        for z in 0..31 {
                            let table = &tables[z * 1024..(z + 1) * 1024];
                            for m in 0..64 {
                                let e = table[patterns[0][m] as usize]
                                    + table[256 + patterns[1][m] as usize];
                                let o = table[512 + patterns[2][m] as usize]
                                    + table[768 + patterns[3][m] as usize];
                                <Gf as WideMulAcc>::wide_add_assign(
                                    &mut sums[z],
                                    &<Gf as WideMulAcc>::mul_wide(&(weights[m] * e), &o),
                                );
                            }
                        }
                    }
                    std::array::from_fn(|z| {
                        eq_y[y] * <Gf as WideMulAcc>::from_wide(sums[z].clone())
                    })
                })
                .collect();
            let values = partials.into_iter().fold([Gf::zero(); 31], |mut a, b| {
                for i in 0..31 {
                    a[i] += b[i];
                }
                a
            });
            interpolate(values)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wfbitz::forest::PatternMode;
    use crate::wfbitz::leaf_skip::polynomial_from_cross;

    fn word(state: &mut u64) -> u64 {
        *state ^= *state << 13;
        *state ^= *state >> 7;
        *state ^= *state << 17;
        *state
    }

    fn field(state: &mut u64) -> Gf {
        Gf::from_polynomial_words([word(state), word(state)])
    }

    fn fixture(t: usize, s: usize) -> (Vec<Vec<u64>>, Vec<Gf>, Vec<Gf>) {
        let mut state = 0xAF67_109B_1C4D_F021u64 ^ ((t as u64) << 32) ^ s as u64;
        let packed = (0..(1usize << s).div_ceil(64))
            .map(|_| (0..1usize << t).map(|_| word(&mut state)).collect())
            .collect();
        let images = (0..1usize << t)
            .map(|i| {
                if i % 11 == 0 {
                    Gf::one()
                } else {
                    field(&mut state)
                }
            })
            .collect();
        let external = (0..t - 1 + s).map(|_| field(&mut state)).collect();
        (packed, images, external)
    }

    #[test]
    fn fused_cross_scatter_matches_portable_with_collisions() {
        let mut state = 0x713F_E859_04A1_D297u64;
        for mode in 0..3 {
            let mut expected = vec![Gf::zero(); BUCKETS];
            let mut actual = expected.clone();
            for _ in 0..3 {
                let patterns = std::array::from_fn(|_| {
                    std::array::from_fn(|_| match mode {
                        0 => word(&mut state) as u8,
                        1 => 0,
                        _ => 255,
                    })
                });
                let weights: Vec<Gf> = (0..64).map(|_| field(&mut state)).collect();
                scatter_portable(&mut expected, &patterns, &weights);
                scatter(&mut actual, &patterns, &weights);
                assert_eq!(actual, expected, "mode={mode}");
            }
        }
    }

    #[test]
    fn fast_cross_and_evaluated_polynomial_match_reference() {
        for t in [6, 7] {
            for s in [0, 3, 6, 7] {
                let (mut packed, images, mut external) = fixture(t, s);
                for mode in 0..3 {
                    if mode > 0 {
                        for group in &mut packed {
                            group.fill(if mode == 1 { 0 } else { u64::MAX });
                        }
                        external.fill(if mode == 1 { Gf::zero() } else { Gf::one() });
                    }
                    for nibble in [false, true] {
                        let forest =
                            Forest::new(t, s, &packed, &images).with_patterns(PatternMode {
                                nibble,
                                cache: false,
                                nibble_from: 0,
                            });
                        let reference = forest.leaf_skip_cross(&external);
                        assert_eq!(
                            forest.leaf_skip_cross_fast(&external),
                            reference,
                            "cross t={t} s={s} mode={mode} nibble={nibble}"
                        );
                        assert_eq!(
                            forest.leaf_skip_polynomial_evaluated(&external),
                            polynomial_from_cross(&reference),
                            "polynomial t={t} s={s} mode={mode} nibble={nibble}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn fast_cross_matches_direct_dense_leaf_products() {
        let (t, s) = (7, 3);
        let (packed, images, external) = fixture(t, s);
        let forest = Forest::new(t, s, &packed, &images).with_patterns(PatternMode {
            nibble: true,
            cache: false,
            nibble_from: 0,
        });
        let mut expected = [Gf::zero(); 256];
        let eq_c = eq_table(&external[..s]);
        let eq_y = eq_table(&external[s..s + t - 5]);
        for (y, &wy) in eq_y.iter().enumerate() {
            for (c, &wc) in eq_c.iter().enumerate() {
                let leaf = |p: usize, a: usize| {
                    let row = y | (a << (t - 5)) | (p << (t - 1));
                    if (packed[c >> 6][row] >> (c & 63)) & 1 == 0 {
                        Gf::one()
                    } else {
                        images[row]
                    }
                };
                for a in 0..16 {
                    for b in 0..16 {
                        expected[16 * a + b] += wy * wc * leaf(0, a) * leaf(1, b);
                    }
                }
            }
        }
        assert_eq!(forest.leaf_skip_cross_fast(&external), expected);
    }

    /// Select with WFBITZ_CROSS_PROBE=all|reference|fast|evaluation,
    /// WFBITZ_CROSS_T, WFBITZ_CROSS_S and WFBITZ_CROSS_REPS. Default grid
    /// 12:8 is intentionally bounded; this is an explicit diagnostic only.
    #[test]
    #[ignore = "explicit cross-sum benchmark; run without other CPU workloads"]
    fn leaf_cross_cost_probe() {
        let number = |key: &str, default: usize| {
            std::env::var(key)
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(default)
        };
        let (t, s) = (number("WFBITZ_CROSS_T", 12), number("WFBITZ_CROSS_S", 8));
        let reps = number("WFBITZ_CROSS_REPS", 3).max(1);
        let mode = std::env::var("WFBITZ_CROSS_PROBE").unwrap_or_else(|_| "all".to_owned());
        assert!(matches!(
            mode.as_str(),
            "all" | "reference" | "fast" | "evaluation"
        ));
        let (packed, images, external) = fixture(t, s);
        let forest = Forest::new(t, s, &packed, &images).with_patterns(PatternMode {
            nibble: true,
            cache: false,
            nibble_from: 0,
        });
        forest.nibbles();
        let expected = polynomial_from_cross(&forest.leaf_skip_cross(&external));
        for name in ["reference", "fast", "evaluation"] {
            if mode != "all" && mode != name {
                continue;
            }
            let mut times = Vec::new();
            for repetition in 0..=reps {
                let start = Instant::now();
                let polynomial = match name {
                    "reference" => polynomial_from_cross(&forest.leaf_skip_cross(&external)),
                    "fast" => polynomial_from_cross(&forest.leaf_skip_cross_fast(&external)),
                    _ => forest.leaf_skip_polynomial_evaluated(&external),
                };
                let elapsed = start.elapsed().as_secs_f64() * 1000.0;
                assert_eq!(std::hint::black_box(polynomial), expected);
                if repetition > 0 {
                    times.push(elapsed);
                }
            }
            times.sort_by(f64::total_cmp);
            eprintln!(
                "LEAF_CROSS mode={name} t={t} s={s} median_ms={:.3} samples={times:?}",
                times[times.len() / 2]
            );
        }
        let (_, cost) = forest.leaf_skip_cross_fast_impl::<true>(&external);
        eprintln!(
            "LEAF_CROSS worker_ms clear={:.3} patterns_and_scatter={:.3} contract={:.3}",
            cost.clear.as_secs_f64() * 1000.0,
            cost.scatter.as_secs_f64() * 1000.0,
            cost.contract.as_secs_f64() * 1000.0
        );
    }
}

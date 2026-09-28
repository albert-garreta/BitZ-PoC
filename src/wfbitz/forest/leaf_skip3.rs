//! An eight-corner univariate prefix followed by the existing JIT and dense rounds.

use super::{Forest, Gf, Point, ProverState, Tables};
use crate::cfg_chunks_mut;
use std::collections::VecDeque;

#[cfg(feature = "parallel")]
use rayon::prelude::*;

impl Forest<'_> {
    /// Skips the first three row variables, then resumes the same two
    /// table rounds and dense continuation as the ordinary leaf level.
    pub(super) fn prove_skipped_leaf3(
        &self,
        transcript: &mut ProverState,
        point: Point,
        arena: &mut Vec<Gf>,
        spare_tables: &mut Vec<Gf>,
    ) -> (Vec<Gf>, Gf, Vec<Gf>) {
        assert!(
            self.t >= 6,
            "three-variable skip needs two remaining row bits"
        );
        let external: Vec<Gf> = point.iter().rev().copied().collect();
        let started = std::time::Instant::now();
        let cross = self.cross_sums(0, 3, &external);
        super::super::trace("    L0 skip3 cross sums", started);
        let started = std::time::Instant::now();
        let weights = super::super::leaf_skip::prove_prefix3(transcript, &cross);
        super::super::trace("    L0 skip3 polynomial", started);
        let started = std::time::Instant::now();
        let tables = self.leaf_skip3_tables(&weights, std::mem::take(spare_tables));
        super::super::trace("    L0 skip3 tables", started);
        let (point, claim) = self.continue_jit_level(
            transcript,
            0,
            3,
            point,
            tables,
            Gf::one(),
            VecDeque::new(),
            spare_tables,
            arena,
            None,
            false,
        );
        (point.into_iter().rev().collect(), claim, weights.to_vec())
    }

    /// The same eight-bit pattern layout consumed by the ordinary leaf
    /// JIT kernels, but with Lagrange weights instead of three tensor weights.
    fn leaf_skip3_tables(&self, weights: &[Gf; 8], mut data: Vec<Gf>) -> Tables {
        let row_bits = self.t - 4;
        let rows = 1usize << row_bits;
        let positions = 2 * rows;
        let len = positions * 256;
        data.clear();
        data.reserve(len);
        let constant: Gf = weights.iter().copied().sum();
        cfg_chunks_mut!(&mut data.spare_capacity_mut()[..len], 256)
            .enumerate()
            .for_each(|(q, out)| {
                let (p, y) = (q >> row_bits, q & (rows - 1));
                let delta: [Gf; 8] = std::array::from_fn(|a| {
                    weights[a]
                        * (self.images[y | (a << row_bits) | (p << (self.t - 1))] - Gf::one())
                });
                let mut subset = [Gf::zero(); 256];
                subset[0] = constant;
                out[0].write(constant);
                for pattern in 1..256usize {
                    subset[pattern] =
                        subset[pattern & (pattern - 1)] + delta[pattern.trailing_zeros() as usize];
                    out[pattern].write(subset[pattern]);
                }
            });
        // SAFETY: all disjoint 256-entry position tables were initialized.
        unsafe { data.set_len(len) };
        Tables { data, len: 256 }
    }
}

#[cfg(test)]
mod tests {
    use super::super::{PatternMode, Weighing, eq_table};
    use super::*;
    use crate::wfbitz::gkr::prove_layer_from;
    use crate::wfbitz::leaf_skip::{prove_prefix3, verify_layer3};
    use crate::wfbitz::transcript::{build_prover, build_verifier};

    fn field(index: usize) -> Gf {
        let i = index as u64;
        Gf::from_polynomial_words([
            i.wrapping_mul(0x9e37_79b9_7f4a_7c15),
            i ^ 0x83bd_902e_7816_c3ab,
        ])
    }

    #[test]
    fn eight_corner_skip_reuses_jit_tail_and_matches_dense_protocol() {
        for (t, s) in [(6, 0), (6, 3), (7, 6), (7, 10)] {
            let cols = 1usize << s;
            let rows = 1usize << t;
            let mut state = 0xb32a_7816_ae02_349fu64;
            let packed: Vec<Vec<u64>> = (0..cols.div_ceil(64))
                .map(|_| {
                    (0..rows)
                        .map(|_| {
                            state ^= state << 13;
                            state ^= state >> 7;
                            state ^= state << 17;
                            state
                        })
                        .collect()
                })
                .collect();
            let images: Vec<Gf> = (0..rows).map(|i| field(i + 1)).collect();
            for boolean_columns in [false, true] {
                let external: Vec<Gf> = (0..t - 1 + s)
                    .map(|i| {
                        if boolean_columns && i < s {
                            if i & 1 == 0 { Gf::zero() } else { Gf::one() }
                        } else {
                            field(i + 101)
                        }
                    })
                    .collect();
                let point: Point = external.iter().rev().copied().collect();
                for nibble in [false, true] {
                    for weighing in [Weighing::Off, Weighing::On] {
                        let forest = Forest::new(t, s, &packed, &images)
                            .with_patterns(PatternMode {
                                nibble,
                                cache: false,
                                nibble_from: 0,
                            })
                            .with_weighing(weighing);
                        let mut direct = build_prover(b"skip3-tail-test", b"dense");
                        let cross = forest.cross_sums(0, 3, &external);
                        let weights = prove_prefix3(&mut direct, &cross);
                        let row_bits = t - 4;
                        let suffix_rows = 1usize << row_bits;
                        let value = |p: usize, y: usize, c: usize| -> Gf {
                            (0..8)
                                .map(|a| {
                                    let row = y | (a << row_bits) | (p << (t - 1));
                                    let leaf = if (packed[c >> 6][row] >> (c & 63)) & 1 == 0 {
                                        Gf::one()
                                    } else {
                                        images[row]
                                    };
                                    weights[a] * leaf
                                })
                                .sum()
                        };
                        let mut left: Vec<Gf> = (0..suffix_rows * cols)
                            .map(|i| value(0, i / cols, i % cols))
                            .collect();
                        let mut right: Vec<Gf> = (0..suffix_rows * cols)
                            .map(|i| value(1, i / cols, i % cols))
                            .collect();
                        let suffix = point.iter().skip(3).copied().collect();
                        let (want_point, want_claim) = prove_layer_from(
                            &mut direct,
                            suffix,
                            &mut left,
                            &mut right,
                            0,
                            Gf::one(),
                            VecDeque::new(),
                        );
                        let direct_proof = direct.finish();

                        let mut optimized = build_prover(b"skip3-tail-test", b"dense");
                        let mut arena = Vec::with_capacity(1usize << (t + s - 4));
                        let mut spare_tables = vec![Gf::zero(); (1usize << (t - 3)) * 256];
                        let capacity = spare_tables.capacity();
                        let (got_point, got_claim, got_weights) = forest.prove_skipped_leaf3(
                            &mut optimized,
                            point.clone(),
                            &mut arena,
                            &mut spare_tables,
                        );
                        assert_eq!(
                            spare_tables.capacity(),
                            capacity,
                            "reuse preceding level tables"
                        );
                        assert_eq!(got_weights, weights);
                        assert_eq!(got_point, want_point.into_iter().rev().collect::<Vec<_>>());
                        assert_eq!(got_claim, want_claim);
                        let proof = optimized.finish();
                        assert_eq!(
                            proof, direct_proof,
                            "t={t} s={s} nibble={nibble} boolean_columns={boolean_columns}"
                        );

                        let prefix = eq_table(&external[s + row_bits..]);
                        let initial_claim = (0..8).map(|a| prefix[a] * cross[8 * a + a]).sum();
                        let mut verifier = build_verifier(b"skip3-tail-test", b"dense", &proof);
                        let (binding, claim) =
                            verify_layer3(&mut verifier, initial_claim, point.clone())
                                .expect("valid skipped layer");
                        verifier.check_eof().expect("all messages consumed");
                        assert_eq!(claim, got_claim);
                        assert_eq!(binding.prefix_weights, got_weights);
                        assert_eq!(binding.suffix_point, got_point);
                        let row_weights = binding
                            .row_weights(&images, s)
                            .expect("terminal row weights");
                        let column_weights = eq_table(&got_point[..s]);
                        let mut opening_claim = Gf::zero();
                        for (row, &row_weight) in row_weights.iter().enumerate() {
                            for (column, &column_weight) in column_weights.iter().enumerate() {
                                if (packed[column >> 6][row] >> (column & 63)) & 1 != 0 {
                                    opening_claim += row_weight * column_weight;
                                }
                            }
                        }
                        assert_eq!(opening_claim, claim - Gf::one());
                    }
                }
            }
        }
    }
}

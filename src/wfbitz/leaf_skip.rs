//! Three or four row variables of the leaf GKR layer packed into one univariate.
//!
//! The two child factors are interpolated separately on eight or sixteen
//! distinct field points. Their product has degree at most fourteen or thirty. The
//! incoming claim is a known weighted sum of this polynomial at those
//! points; after its message, one full-field challenge binds the prefix.
//! The interpolation weights need not be a Boolean equality tensor, so
//! the terminal claim records them separately from its ordinary suffix.

use std::collections::VecDeque;
use std::sync::OnceLock;

use field::Gf128 as Gf;

use super::eq_factor;
use super::gkr::Point;
use super::transcript::{ProverState, VerifierState};
use crate::poly::utils::build_eq_x_r_vec;

pub(crate) const SKIP_VARS: usize = 4;
pub(crate) const CORNERS: usize = 1 << SKIP_VARS;
pub(crate) const COEFFICIENTS: usize = 2 * CORNERS - 1;
pub(crate) type CrossSums = [Gf; CORNERS * CORNERS];
pub(crate) type SkipPolynomial = [Gf; COEFFICIENTS];

const LABEL: &[u8] = b"wfbitz/leaf-univariate-skip4/v1";
const LABEL3: &[u8] = b"wfbitz/leaf-univariate-skip3/v1";

/// The actual polynomial-basis encodings 0 through 15. Distinctness is
/// sufficient; these points are not assumed to form a subfield.
fn node(index: usize) -> Gf {
    Gf::from_polynomial_words([index as u64, 0])
}

struct Interpolation<const N: usize, const M: usize> {
    basis: [[Gf; N]; N],
    /// `L_a L_b` for `a <= b`, in lexicographic pair order.
    products: Vec<[Gf; M]>,
}

fn prepare_interpolation<const N: usize, const M: usize>() -> Interpolation<N, M> {
    let mut basis = [[Gf::zero(); N]; N];
    for (i, polynomial) in basis.iter_mut().enumerate() {
        polynomial[0] = Gf::one();
        let mut degree = 0;
        let mut denominator = Gf::one();
        for j in 0..N {
            if j == i {
                continue;
            }
            let x = node(j);
            for k in (0..=degree + 1).rev() {
                let previous = if k == 0 {
                    Gf::zero()
                } else {
                    polynomial[k - 1]
                };
                polynomial[k] = previous - x * polynomial[k];
            }
            degree += 1;
            denominator *= node(i) - x;
        }
        // All nodes are distinct, so this public denominator is nonzero.
        let inverse = Gf::one() / denominator;
        for coefficient in polynomial {
            *coefficient *= inverse;
        }
    }
    let mut products = Vec::with_capacity(N * (N + 1) / 2);
    for a in 0..N {
        for b in a..N {
            let mut product = [Gf::zero(); M];
            for (i, &left) in basis[a].iter().enumerate() {
                for (j, &right) in basis[b].iter().enumerate() {
                    product[i + j] += left * right;
                }
            }
            products.push(product);
        }
    }
    Interpolation { basis, products }
}

fn interpolation() -> &'static Interpolation<CORNERS, COEFFICIENTS> {
    static PREPARED: OnceLock<Interpolation<CORNERS, COEFFICIENTS>> = OnceLock::new();
    PREPARED.get_or_init(prepare_interpolation)
}

fn interpolation3() -> &'static Interpolation<8, 15> {
    static PREPARED: OnceLock<Interpolation<8, 15>> = OnceLock::new();
    PREPARED.get_or_init(prepare_interpolation)
}

fn evaluate(coefficients: &[Gf], point: Gf) -> Gf {
    coefficients
        .iter()
        .rev()
        .fold(Gf::zero(), |value, &coefficient| {
            value * point + coefficient
        })
}

/// Lagrange weights in natural prefix-index order: the first skipped
/// (most significant) coordinate is bit 3 of the array index.
pub(crate) fn basis(challenge: Gf) -> [Gf; CORNERS] {
    std::array::from_fn(|i| evaluate(&interpolation().basis[i], challenge))
}

pub(crate) fn basis3(challenge: Gf) -> [Gf; 8] {
    std::array::from_fn(|i| evaluate(&interpolation3().basis[i], challenge))
}

/// `Q(T) = sum_{a,b} cross[a,b] L_a(T) L_b(T)`, in monomial order.
/// Opposite off-diagonal entries share the same basis product.
pub(crate) fn polynomial_from_cross(cross: &CrossSums) -> SkipPolynomial {
    polynomial_from_cross_with(cross, interpolation())
}

fn polynomial_from_cross_with<const N: usize, const M: usize>(
    cross: &[Gf],
    prepared: &Interpolation<N, M>,
) -> [Gf; M] {
    assert_eq!(cross.len(), N * N);
    let mut coefficients = [Gf::zero(); M];
    let mut products = prepared.products.iter();
    for a in 0..N {
        for b in a..N {
            let value = if a == b {
                cross[a * N + b]
            } else {
                cross[a * N + b] + cross[b * N + a]
            };
            let product = products
                .next()
                .expect("one public basis product per corner pair");
            for (coefficient, &weight) in coefficients.iter_mut().zip(product) {
                *coefficient += value * weight;
            }
        }
    }
    coefficients
}

/// Sends the packed round before learning its challenge. The caller uses
/// the returned weights to fold each factor directly from the packed bits.
pub(crate) fn prove_prefix(transcript: &mut ProverState, cross: &CrossSums) -> [Gf; CORNERS] {
    transcript.public_message(LABEL);
    transcript.prover_message(&polynomial_from_cross(cross));
    basis(transcript.verifier_message::<Gf>())
}

/// The eight-corner variant, with its own transcript domain and fifteen
/// coefficients. The four-variable wrapper keeps its existing wire format.
pub(crate) fn prove_prefix3(transcript: &mut ProverState, cross: &[Gf; 64]) -> [Gf; 8] {
    transcript.public_message(LABEL3);
    transcript.prover_message(&polynomial_from_cross_with(cross, interpolation3()));
    basis3(transcript.verifier_message::<Gf>())
}

/// The leaf exit after the skip and all ordinary suffix rounds.
#[derive(Clone, Debug)]
pub(crate) struct LeafBinding {
    pub(crate) prefix_weights: Vec<Gf>,
    /// External, little-endian coordinates: columns, remaining low row
    /// coordinates, then the child selector in the highest position.
    pub(crate) suffix_point: Vec<Gf>,
}

impl LeafBinding {
    /// The row factor of the opening claim. Its column factor is the
    /// equality table of `suffix_point[..column_vars]`, and its target is
    /// the terminal leaf claim minus one.
    pub(crate) fn row_weights(&self, images: &[Gf], column_vars: usize) -> Option<Vec<Gf>> {
        let skipped = match self.prefix_weights.len() {
            8 => 3,
            16 => 4,
            _ => return None,
        };
        if !images.len().is_power_of_two() {
            return None;
        }
        let row_vars = images.len().ilog2() as usize;
        if row_vars <= skipped
            || self.suffix_point.len() != row_vars.checked_add(column_vars)?.checked_sub(skipped)?
        {
            return None;
        }
        let suffix = build_eq_x_r_vec(&self.suffix_point[column_vars..], &()).ok()?;
        let low_bits = row_vars - skipped - 1;
        let low_mask = (1usize << low_bits) - 1;
        Some(
            images
                .iter()
                .enumerate()
                .map(|(row, &image)| {
                    let prefix = (row >> low_bits) & (self.prefix_weights.len() - 1);
                    let compact_row =
                        (row & low_mask) | ((row >> (low_bits + skipped)) << low_bits);
                    (image - Gf::one()) * self.prefix_weights[prefix] * suffix[compact_row]
                })
                .collect(),
        )
    }
}

/// Replays the packed prefix, then ordinary Gruen suffix rounds and the
/// child selector. `point` has the original layer's MSB-first order.
pub(crate) fn verify_layer(
    transcript: &mut VerifierState<'_>,
    claim: Gf,
    point: Point,
) -> Option<(LeafBinding, Gf)> {
    verify_layer_with(transcript, claim, point, SKIP_VARS, LABEL, interpolation())
}

pub(crate) fn verify_layer3(
    transcript: &mut VerifierState<'_>,
    claim: Gf,
    point: Point,
) -> Option<(LeafBinding, Gf)> {
    verify_layer_with(transcript, claim, point, 3, LABEL3, interpolation3())
}

fn verify_layer_with<const N: usize, const M: usize>(
    transcript: &mut VerifierState<'_>,
    mut claim: Gf,
    point: Point,
    skipped: usize,
    label: &[u8],
    prepared: &Interpolation<N, M>,
) -> Option<(LeafBinding, Gf)> {
    if point.len() < skipped {
        return None;
    }
    transcript.public_message(label);
    let polynomial: [Gf; M] = transcript.prover_message().ok()?;
    let mut expected = Gf::zero();
    for a in 0..N {
        let weight = point
            .iter()
            .take(skipped)
            .enumerate()
            .fold(Gf::one(), |weight, (i, &z)| {
                weight
                    * if (a >> (skipped - i - 1)) & 1 == 0 {
                        Gf::one() - z
                    } else {
                        z
                    }
            });
        expected += weight * evaluate(&polynomial, node(a));
    }
    if expected != claim {
        return None;
    }
    let challenge = transcript.verifier_message::<Gf>();
    let prefix_weights = prepared
        .basis
        .iter()
        .map(|p| evaluate(p, challenge))
        .collect();
    claim = evaluate(&polynomial, challenge);

    let one = Gf::one();
    let mut factor = one;
    let mut next_point = VecDeque::with_capacity(point.len() - skipped + 1);
    for z in point.into_iter().skip(skipped) {
        let [endpoint, leading]: [Gf; 2] = transcript.prover_message().ok()?;
        let (sum0, sum1) = if z == Gf::zero() {
            (claim, endpoint)
        } else {
            (endpoint, (claim - (one - z) * endpoint) / z)
        };
        let r = transcript.verifier_message::<Gf>();
        next_point.push_back(r);
        let equality = eq_factor(r, z);
        claim = equality * (sum0 + r * ((sum1 - sum0) + (r - one) * leading));
        factor *= equality;
    }
    let children: [Gf; 2] = transcript.prover_message().ok()?;
    if factor * children[0] * children[1] != claim {
        return None;
    }
    let r = transcript.verifier_message::<Gf>();
    next_point.push_front(r);
    claim = children[0] + r * (children[1] - children[0]);
    let suffix_point = next_point.into_iter().rev().collect();
    Some((
        LeafBinding {
            prefix_weights,
            suffix_point,
        },
        claim,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wfbitz::gkr::{eq_table, prove_layer_from};
    use crate::wfbitz::transcript::{Proof, build_prover, build_verifier};

    fn field(seed: u64) -> Gf {
        Gf::from_polynomial_words([
            seed.wrapping_mul(0x9e37_79b9_7f4a_7c15),
            seed ^ 0xd1b5_4a32_d192_ed03,
        ])
    }

    #[test]
    fn basis_interpolates_and_partitions_unity() {
        for a in 0..CORNERS {
            let weights = basis(node(a));
            for (b, weight) in weights.into_iter().enumerate() {
                assert_eq!(weight, if a == b { Gf::one() } else { Gf::zero() });
            }
        }
        for r in [Gf::zero(), Gf::one(), field(1), field(39)] {
            let weights = basis(r);
            assert_eq!(weights.iter().copied().sum::<Gf>(), Gf::one());
            for degree in 0..CORNERS {
                let at = |x: Gf| (0..degree).fold(Gf::one(), |power, _| power * x);
                let interpolated: Gf = weights
                    .iter()
                    .enumerate()
                    .map(|(a, &weight)| weight * at(node(a)))
                    .sum();
                assert_eq!(interpolated, at(r));
            }
        }
    }

    #[test]
    fn packed_polynomial_matches_factor_products() {
        let left: [Gf; CORNERS] = std::array::from_fn(|a| field(a as u64 + 1));
        let right: [Gf; CORNERS] = std::array::from_fn(|a| field(a as u64 + 101));
        let cross = std::array::from_fn(|i| left[i / CORNERS] * right[i % CORNERS]);
        let polynomial = polynomial_from_cross(&cross);
        for r in [node(0), node(7), node(15), field(29)] {
            let weights = basis(r);
            let e: Gf = left.iter().zip(weights).map(|(&e, w)| e * w).sum();
            let o: Gf = right.iter().zip(weights).map(|(&o, w)| o * w).sum();
            assert_eq!(evaluate(&polynomial, r), e * o);
        }
    }

    /// A dense oracle for the new protocol, independent of the bit kernels.
    fn dense_proof(prefix_is_boolean: bool) -> (Proof, Point, Gf, LeafBinding, Gf) {
        dense_proof_for(4, prefix_is_boolean)
    }

    fn dense_proof_for(
        skipped: usize,
        prefix_is_boolean: bool,
    ) -> (Proof, Point, Gf, LeafBinding, Gf) {
        const SUFFIX_VARS: usize = 3;
        const SUFFIX_LEN: usize = 1 << SUFFIX_VARS;
        let corners = 1usize << skipped;
        let mut point: Point = (0..skipped + SUFFIX_VARS)
            .map(|i| field(i as u64 + 23))
            .collect();
        if prefix_is_boolean {
            for i in 0..point.len() {
                point[i] = if i % 2 == 0 { Gf::zero() } else { Gf::one() };
            }
        }
        let external: Vec<Gf> = point.iter().rev().copied().collect();
        let suffix_weights = eq_table(&external[..SUFFIX_VARS]);
        let prefix_weights = eq_table(&external[SUFFIX_VARS..]);
        let left: Vec<[Gf; SUFFIX_LEN]> = (0..corners)
            .map(|a| std::array::from_fn(|u| field((a * SUFFIX_LEN + u + 1) as u64)))
            .collect();
        let right: Vec<[Gf; SUFFIX_LEN]> = (0..corners)
            .map(|a| std::array::from_fn(|u| field((a * SUFFIX_LEN + u + 301) as u64)))
            .collect();
        let cross: Vec<Gf> = (0..corners * corners)
            .map(|i| {
                (0..SUFFIX_LEN)
                    .map(|u| suffix_weights[u] * left[i / corners][u] * right[i % corners][u])
                    .sum()
            })
            .collect();
        let claim = (0..corners)
            .map(|a| prefix_weights[a] * cross[a * corners + a])
            .sum();
        let mut transcript = build_prover(b"leaf-skip-test", b"dense");
        let bound = if skipped == 3 {
            prove_prefix3(
                &mut transcript,
                &cross.try_into().expect("eight-corner matrix"),
            )
            .to_vec()
        } else {
            prove_prefix(
                &mut transcript,
                &cross.try_into().expect("sixteen-corner matrix"),
            )
            .to_vec()
        };
        let mut e: Vec<Gf> = (0..SUFFIX_LEN)
            .map(|u| (0..corners).map(|a| bound[a] * left[a][u]).sum())
            .collect();
        let mut o: Vec<Gf> = (0..SUFFIX_LEN)
            .map(|u| (0..corners).map(|a| bound[a] * right[a][u]).sum())
            .collect();
        let suffix = point.iter().skip(skipped).copied().collect();
        let (end_point, end_claim) = prove_layer_from(
            &mut transcript,
            suffix,
            &mut e,
            &mut o,
            0,
            Gf::one(),
            VecDeque::new(),
        );
        let binding = LeafBinding {
            prefix_weights: bound,
            suffix_point: end_point.into_iter().rev().collect(),
        };
        (transcript.finish(), point, claim, binding, end_claim)
    }

    #[test]
    fn skipped_layer_replays_dense_suffix_and_rejects_changes() {
        for boolean in [false, true] {
            let (proof, point, claim, want, want_claim) = dense_proof(boolean);
            let mut verifier = build_verifier(b"leaf-skip-test", b"dense", &proof);
            let (got, got_claim) =
                verify_layer(&mut verifier, claim, point.clone()).expect("valid packed layer");
            assert_eq!(got.prefix_weights, want.prefix_weights);
            assert_eq!(got.suffix_point, want.suffix_point);
            assert_eq!(got_claim, want_claim);
            verifier
                .check_eof()
                .expect("all packed-layer messages consumed");

            let mut verifier = build_verifier(b"leaf-skip-test", b"dense", &proof);
            assert!(verify_layer(&mut verifier, claim + Gf::one(), point.clone()).is_none());
            let mut changed = proof.clone();
            changed.narg_string[0] ^= 1; // Change Q's constant: its weighted sum changes by one.
            let mut verifier = build_verifier(b"leaf-skip-test", b"dense", &changed);
            assert!(verify_layer(&mut verifier, claim, point.clone()).is_none());
            let mut short = proof;
            short.narg_string.truncate(COEFFICIENTS * 16 - 1);
            let mut verifier = build_verifier(b"leaf-skip-test", b"dense", &short);
            assert!(verify_layer(&mut verifier, claim, point).is_none());
        }
    }

    #[test]
    fn node_preserving_polynomial_change_is_rejected_by_suffix() {
        let mut vanishing = [Gf::zero(); COEFFICIENTS];
        vanishing[0] = Gf::one();
        for a in 0..CORNERS {
            for degree in (0..=a + 1).rev() {
                let previous = if degree == 0 {
                    Gf::zero()
                } else {
                    vanishing[degree - 1]
                };
                vanishing[degree] = previous - node(a) * vanishing[degree];
            }
        }
        assert_eq!(vanishing[CORNERS], Gf::one());

        for boolean in [false, true] {
            let (mut proof, point, claim, _, _) = dense_proof(boolean);
            let original: SkipPolynomial = {
                let mut reader = build_verifier(b"leaf-skip-test", b"dense", &proof);
                reader.public_message(LABEL);
                reader.prover_message().expect("packed polynomial")
            };
            let altered: SkipPolynomial = std::array::from_fn(|i| original[i] + vanishing[i]);
            for a in 0..CORNERS {
                assert_eq!(evaluate(&altered, node(a)), evaluate(&original, node(a)));
            }

            // Preserve the original tail while replacing only Q. Its node
            // values, and hence every weighted initial-claim check, agree.
            // The challenge and the remaining factor claim must still bind Q.
            let mut prefix = build_prover(b"leaf-skip-test", b"dense");
            prefix.public_message(LABEL);
            prefix.prover_message(&altered);
            let replacement = prefix.finish().narg_string;
            proof.narg_string[..replacement.len()].copy_from_slice(&replacement);
            let mut verifier = build_verifier(b"leaf-skip-test", b"dense", &proof);
            assert!(verify_layer(&mut verifier, claim, point).is_none());
        }
    }

    #[test]
    fn eight_node_basis_and_product_polynomial_match_direct_evaluation() {
        for a in 0..8 {
            for (b, weight) in basis3(node(a)).into_iter().enumerate() {
                assert_eq!(weight, if a == b { Gf::one() } else { Gf::zero() });
            }
        }
        let left: [Gf; 8] = std::array::from_fn(|a| field(a as u64 + 11));
        let right: [Gf; 8] = std::array::from_fn(|a| field(a as u64 + 71));
        let cross: [Gf; 64] = std::array::from_fn(|i| left[i / 8] * right[i % 8]);
        let polynomial = polynomial_from_cross_with(&cross, interpolation3());
        for r in [Gf::zero(), Gf::one(), node(7), field(61)] {
            let weights = basis3(r);
            assert_eq!(weights.iter().copied().sum::<Gf>(), Gf::one());
            let e: Gf = left
                .iter()
                .zip(weights)
                .map(|(&value, weight)| value * weight)
                .sum();
            let o: Gf = right
                .iter()
                .zip(weights)
                .map(|(&value, weight)| value * weight)
                .sum();
            assert_eq!(evaluate(&polynomial, r), e * o);
        }
    }

    #[test]
    fn three_variable_skip_replays_and_binds_message_and_domain() {
        let mut vanishing = [Gf::zero(); 15];
        vanishing[0] = Gf::one();
        for a in 0..8 {
            for degree in (0..=a + 1).rev() {
                let previous = if degree == 0 {
                    Gf::zero()
                } else {
                    vanishing[degree - 1]
                };
                vanishing[degree] = previous - node(a) * vanishing[degree];
            }
        }
        for boolean in [false, true] {
            let (proof, point, claim, want, want_claim) = dense_proof_for(3, boolean);
            let mut verifier = build_verifier(b"leaf-skip-test", b"dense", &proof);
            let (got, got_claim) = verify_layer3(&mut verifier, claim, point.clone())
                .expect("valid three-variable prefix");
            verifier.check_eof().expect("all messages consumed");
            assert_eq!(got.prefix_weights, want.prefix_weights);
            assert_eq!(got.suffix_point, want.suffix_point);
            assert_eq!(got_claim, want_claim);

            let mut verifier = build_verifier(b"leaf-skip-test", b"dense", &proof);
            assert!(verify_layer3(&mut verifier, claim + Gf::one(), point.clone()).is_none());
            let mut verifier = build_verifier(b"leaf-skip-test", b"dense", &proof);
            assert!(
                verify_layer_with(
                    &mut verifier,
                    claim,
                    point.clone(),
                    3,
                    LABEL,
                    interpolation3()
                )
                .is_none(),
                "the skip4 domain cannot replace skip3's tag"
            );

            let original: [Gf; 15] = {
                let mut reader = build_verifier(b"leaf-skip-test", b"dense", &proof);
                reader.public_message(LABEL3);
                reader.prover_message().expect("packed polynomial")
            };
            let altered: [Gf; 15] = std::array::from_fn(|i| original[i] + vanishing[i]);
            for a in 0..8 {
                assert_eq!(evaluate(&altered, node(a)), evaluate(&original, node(a)));
            }
            let mut prefix = build_prover(b"leaf-skip-test", b"dense");
            prefix.public_message(LABEL3);
            prefix.prover_message(&altered);
            let replacement = prefix.finish().narg_string;
            let mut changed = proof.clone();
            changed.narg_string[..replacement.len()].copy_from_slice(&replacement);
            let mut verifier = build_verifier(b"leaf-skip-test", b"dense", &changed);
            assert!(verify_layer3(&mut verifier, claim, point.clone()).is_none());
            let mut short = proof;
            short.narg_string.truncate(15 * 16 - 1);
            let mut verifier = build_verifier(b"leaf-skip-test", b"dense", &short);
            assert!(verify_layer3(&mut verifier, claim, point).is_none());
        }
    }

    #[test]
    fn row_weights_reconstruct_the_leaf_linear_claim() {
        const ROW_VARS: usize = 6;
        const COLUMN_VARS: usize = 2;
        let images: Vec<Gf> = (0..1 << ROW_VARS).map(|i| field(i + 81)).collect();
        for skipped in [3, 4] {
            let binding = LeafBinding {
                prefix_weights: if skipped == 3 {
                    basis3(field(9)).to_vec()
                } else {
                    basis(field(9)).to_vec()
                },
                suffix_point: (0..ROW_VARS + COLUMN_VARS - skipped)
                    .map(|i| field(i as u64 + 171))
                    .collect(),
            };
            let weights = binding
                .row_weights(&images, COLUMN_VARS)
                .expect("valid row shape");
            let columns = eq_table(&binding.suffix_point[..COLUMN_VARS]);
            let remaining = eq_table(&binding.suffix_point[COLUMN_VARS..]);
            let mut leaf_claim = Gf::zero();
            let mut bit_claim = Gf::zero();
            let low_bits = ROW_VARS - skipped - 1;
            for row in 0..1 << ROW_VARS {
                let prefix = (row >> low_bits) & ((1usize << skipped) - 1);
                let compact =
                    (row & ((1usize << low_bits) - 1)) | ((row >> (ROW_VARS - 1)) << low_bits);
                for (column, &column_weight) in columns.iter().enumerate() {
                    let bit = ((row * 13 + column * 7) % 11) < 5;
                    let weight =
                        binding.prefix_weights[prefix] * remaining[compact] * column_weight;
                    leaf_claim += weight * if bit { images[row] } else { Gf::one() };
                    if bit {
                        bit_claim += weights[row] * column_weight;
                    }
                }
            }
            assert_eq!(leaf_claim - Gf::one(), bit_claim);
            assert!(
                binding
                    .row_weights(&images[..images.len() - 1], COLUMN_VARS)
                    .is_none()
            );
            assert!(binding.row_weights(&images, COLUMN_VARS + 1).is_none());
            assert!(binding.row_weights(&images[..16], COLUMN_VARS).is_none());
            let mut malformed = binding;
            malformed.prefix_weights.pop();
            assert!(malformed.row_weights(&images, COLUMN_VARS).is_none());
        }
    }
}

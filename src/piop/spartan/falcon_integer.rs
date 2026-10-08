//! One integer outer reduction for a weighted square sum and optional rejection rows.
//!
//! Row variables precede instance variables. Equality weights stay factored;
//! the first round reads signed machine integers and allocates only the folded
//! tables. Every returned endpoint still requires the caller's source projection.

use super::{
    SpartanBitzField as F, SpartanField, absorb_field_elements, matrix::eq_table, squeeze_field,
};
use crate::{
    sumcheck::{RoundBoundaryPolicy, SumcheckError, SumcheckProof},
    transcript::traits::Transcript,
};
use field::{FpProductAcc, PreparedLinearCombination, Reduce, RingOps, WideMul, Z};
#[cfg(feature = "parallel")]
use rayon::prelude::*;

type Cfg = <F as SpartanField>::Config;

pub(super) struct Output<const K: usize> {
    pub proof: SumcheckProof<F, 4>,
    pub point: Vec<F>,
    pub terminal: [F; K],
}

#[cfg(test)]
fn signed(value: i64, field: &Cfg) -> F {
    let x = F::from_with_cfg(value.unsigned_abs() as u128, field);
    if value < 0 { field.neg(&x) } else { x }
}

fn add(field: &Cfg, a: [F; 3], b: [F; 3]) -> [F; 3] {
    std::array::from_fn(|i| field.add(&a[i], &b[i]))
}

fn product(field: &Cfg, a: [F; 3], b: [F; 3]) -> [F; 3] {
    let d: [F; 3] = std::array::from_fn(|i| field.sub(&b[i], &a[i]));
    [
        field.sub(&field.mul(&a[0], &a[1]), &a[2]),
        field.sub(
            &field.add(&field.mul(&a[0], &d[1]), &field.mul(&a[1], &d[0])),
            &d[2],
        ),
        field.mul(&d[0], &d[1]),
    ]
}

fn linear_times(field: &Cfg, q: [F; 3], a: F, b: F, scale: F) -> [F; 4] {
    let a = field.mul(&a, &scale);
    let b = field.mul(&b, &scale);
    [
        field.mul(&a, &q[0]),
        field.add(&field.mul(&a, &q[1]), &field.mul(&b, &q[0])),
        field.add(&field.mul(&a, &q[2]), &field.mul(&b, &q[1])),
        field.mul(&b, &q[2]),
    ]
}

fn equality_line(field: &Cfg, t: F) -> [F; 2] {
    [
        field.sub(&field.one(), &t),
        field.sub(&field.add(&t, &t), &field.one()),
    ]
}

fn eq(field: &Cfg, a: &[F], b: &[F]) -> F {
    a.iter().zip(b).fold(field.one(), |v, (&a, b)| {
        let [c0, c1] = equality_line(field, a);
        field.mul(&v, &field.add(&c0, &field.mul(&c1, b)))
    })
}

pub(super) fn terminal<const K: usize>(
    field: &Cfg,
    row_vars: usize,
    rho: &[F],
    tau: Option<&[F]>,
    mix: F,
    point: &[F],
    values: &[F; K],
) -> F {
    let norm = field.mul(
        &eq(field, rho, &point[row_vars..]),
        &field.mul(&values[0], &values[0]),
    );
    if let Some(tau) = tau {
        let residual = field.sub(&field.mul(&values[1], &values[2]), &values[3]);
        field.add(
            &norm,
            &field.mul(&field.mul(&mix, &eq(field, tau, point)), &residual),
        )
    } else {
        norm
    }
}

/// K=1 proves the square sum; K=4 also proves equality-weighted A*B-C rows.
#[allow(clippy::too_many_arguments)]
pub(super) fn prove<const K: usize>(
    field: &Cfg,
    transcript: &mut impl Transcript,
    row_vars: usize,
    rho: &[F],
    tau: Option<&[F]>,
    mix: F,
    initial: F,
    read: impl Fn(usize) -> [i16; K] + Sync,
    boundary: &mut impl RoundBoundaryPolicy,
) -> Result<Output<K>, SumcheckError> {
    let rounds = row_vars + rho.len();
    // At most 2^19 native pairs per instance keep all i16 quadratic sums
    // below 2^52 before mixed multiplication by the instance weight.
    if row_vars == 0
        || row_vars > 20
        || rounds >= usize::BITS as usize
        || !matches!(K, 1 | 4)
        || (K == 4) != tau.is_some()
        || tau.is_some_and(|t| t.len() != rounds)
    {
        return Err(SumcheckError::InvalidProductDimensions);
    }
    boundary.validate(rounds)?;
    let zero = field.zero();
    let mut norm_scale = field.one();
    let mut rejection_scale = mix;
    let norm_instances = eq_table(rho, field)?;
    let rejection_instances = tau.map(|t| eq_table(&t[row_vars..], field)).transpose()?;
    let mut tables: [Vec<F>; K] = std::array::from_fn(|_| Vec::new());
    let mut scratch: [Vec<F>; K] = std::array::from_fn(|_| Vec::new());
    let mut proof = SumcheckProof {
        round_polynomials: Vec::with_capacity(rounds),
    };
    let mut point = Vec::with_capacity(rounds);
    let mut claim = initial;
    for round in 0..rounds {
        let coefficient_span = tracing::info_span!(
            "falcon_integer:coefficients",
            component = "falcon_integer_coefficients",
            tag_sumcheck = true,
            tag_constraint_proof = true,
            round_index = round as u64
        )
        .entered();
        let pairs = 1usize << (rounds - round - 1);
        let local_pairs = if round < row_vars {
            1usize << (row_vars - round - 1)
        } else {
            1
        };
        let norm_tail = if round >= row_vars {
            eq_table(&rho[round - row_vars + 1..], field)?
        } else {
            Vec::new()
        };
        let rejection_tail = tau
            .map(|t| {
                eq_table(
                    &t[round + 1..if round < row_vars { row_vars } else { rounds }],
                    field,
                )
            })
            .transpose()?;
        let groups = pairs / local_pairs;
        let group = |i: usize| {
            let mut native_n = [0i64; 3];
            // Each accumulator contains at most 2^19 products. Keep their
            // Montgomery R^2 scale until the complete row subtotal is ready.
            let mut norm_products = [FpProductAcc::<2>::default(); 3];
            let mut rejection_products = [FpProductAcc::<2>::default(); 3];
            let mut r = [zero; 3];
            for j in 0..local_pairs {
                let index = 2 * (i * local_pairs + j);
                if round == 0 {
                    let a = read(index);
                    let b = read(index + 1);
                    let x = i64::from(a[0]);
                    let d = i64::from(b[0]) - x;
                    native_n[0] += x * x;
                    native_n[1] += 2 * x * d;
                    native_n[2] += d * d;
                    if K == 4 {
                        let a: [i64; 3] = std::array::from_fn(|k| i64::from(a[k + 1]));
                        let d: [i64; 3] = std::array::from_fn(|k| i64::from(b[k + 1]) - a[k]);
                        let q = [
                            a[0] * a[1] - a[2],
                            a[0] * d[1] + a[1] * d[0] - d[2],
                            d[0] * d[1],
                        ];
                        let w = rejection_tail.as_ref().unwrap()[j];
                        for k in 0..3 {
                            // Native rejection residual coefficients are tiny.
                            let term = match q[k] {
                                0 => zero,
                                1 => w,
                                -1 => field.neg(&w),
                                v => field.reduce(field.mul_wide(&w, &v)),
                            };
                            r[k] = field.add(&r[k], &term);
                        }
                    }
                } else {
                    let x = tables[0][index];
                    let d = field.sub(&tables[0][index + 1], &x);
                    // Full Falcon pads S1 with zero rows. Their folded square
                    // is identically zero until the high row variable is bound.
                    if K != 4 || x != zero || d != zero {
                        norm_products[0].accumulate(&x, &x);
                        norm_products[1].accumulate(&x, &d);
                        norm_products[2].accumulate(&d, &d);
                    }
                    if K == 4 {
                        let a = std::array::from_fn(|k| tables[k + 1][index]);
                        let b = std::array::from_fn(|k| tables[k + 1][index + 1]);
                        // A=C=0 at both endpoints makes A(t)B(t)-C(t)
                        // identically zero, regardless of B or the challenge.
                        if a[0] == zero && b[0] == zero && a[2] == zero && b[2] == zero {
                            continue;
                        }
                        let q = product(field, a, b);
                        let w =
                            rejection_tail.as_ref().unwrap()[if round < row_vars { j } else { i }];
                        for k in 0..3 {
                            rejection_products[k].accumulate(&w, &q[k]);
                        }
                    }
                }
            }
            let nw = if round < row_vars {
                norm_instances[i]
            } else {
                norm_tail[i]
            };
            let n = if round == 0 {
                native_n.map(|v| field.reduce(field.mul_wide(&nw, &v)))
            } else {
                let mut n = norm_products.map(|sum| field.reduce(sum));
                n[1] = field.add(&n[1], &n[1]);
                if K == 4 {
                    r = rejection_products.map(|sum| field.reduce(sum));
                }
                n.map(|v| field.mul(&nw, &v))
            };
            if round < row_vars {
                if let Some(weights) = &rejection_instances {
                    r = r.map(|v| field.mul(&weights[i], &v));
                }
            }
            [n, r]
        };
        let combine =
            |a: [[F; 3]; 2], b: [[F; 3]; 2]| [add(field, a[0], b[0]), add(field, a[1], b[1])];
        #[cfg(feature = "parallel")]
        let sums = if pairs >= 4096 && groups > 1 {
            (0..groups)
                .into_par_iter()
                .map(group)
                .reduce(|| [[zero; 3]; 2], combine)
        } else {
            (0..groups).map(group).fold([[zero; 3]; 2], combine)
        };
        #[cfg(not(feature = "parallel"))]
        let sums = (0..groups).map(group).fold([[zero; 3]; 2], combine);
        let nl = if round < row_vars {
            [field.one(), zero]
        } else {
            equality_line(field, rho[round - row_vars])
        };
        let mut coefficients = linear_times(field, sums[0], nl[0], nl[1], norm_scale);
        let rl = tau.map(|t| equality_line(field, t[round]));
        if let Some([a, b]) = rl {
            let rejection = linear_times(field, sums[1], a, b, rejection_scale);
            coefficients = std::array::from_fn(|k| field.add(&coefficients[k], &rejection[k]));
        }
        let sum = coefficients
            .iter()
            .fold(coefficients[0], |s, v| field.add(&s, v));
        if sum != claim {
            return Err(SumcheckError::InvalidRoundClaim { round });
        }
        drop(coefficient_span);
        absorb_field_elements(transcript, &coefficients, field);
        boundary.after_round(transcript, round)?;
        let challenge = squeeze_field(transcript, field)?;
        claim = coefficients
            .iter()
            .rev()
            .fold(zero, |s, v| field.add(&field.mul(&s, &challenge), v));
        norm_scale = field.mul(
            &norm_scale,
            &field.add(&nl[0], &field.mul(&nl[1], &challenge)),
        );
        if let Some([a, b]) = rl {
            rejection_scale =
                field.mul(&rejection_scale, &field.add(&a, &field.mul(&b, &challenge)));
        }
        let fold_span = tracing::info_span!(
            "falcon_integer:fold",
            component = "falcon_integer_fold",
            tag_sumcheck = true,
            tag_constraint_proof = true,
            round_index = round as u64
        )
        .entered();
        let one = field.one();
        let complement = field.sub(&one, &challenge);
        // A native signed fold needs one mixed reduction, not two integer
        // embeddings followed by a field multiplication. Boolean row operands
        // (and zero padding) select their four possible results directly.
        let native_fold = (round == 0).then(|| {
            <Cfg as PreparedLinearCombination<Z<1>>>::prepare_linear_combination(
                field,
                [complement, challenge],
            )
        });
        for k in 0..K {
            let fold = |i: usize| {
                if round == 0 {
                    let pair = [read(2 * i)[k], read(2 * i + 1)[k]];
                    match pair {
                        [0, 0] => zero,
                        [1, 1] => one,
                        [0, 1] => challenge,
                        [1, 0] => complement,
                        _ => <Cfg as PreparedLinearCombination<Z<1>>>::linear_combination(
                            native_fold.as_ref().unwrap(),
                            |j| Z::from(pair[j]),
                        ),
                    }
                } else {
                    field.add(
                        &tables[k][2 * i],
                        &field.mul(
                            &challenge,
                            &field.sub(&tables[k][2 * i + 1], &tables[k][2 * i]),
                        ),
                    )
                }
            };
            // Initialize each output with its folded value. Zero-filling these
            // large buffers first adds a serial write and page-fault pass.
            #[cfg(feature = "parallel")]
            if pairs >= 4096 {
                (0..pairs)
                    .into_par_iter()
                    .map(fold)
                    .collect_into_vec(&mut scratch[k]);
            } else {
                scratch[k].clear();
                scratch[k].extend((0..pairs).map(fold));
            }
            #[cfg(not(feature = "parallel"))]
            {
                scratch[k].clear();
                scratch[k].extend((0..pairs).map(fold));
            }
        }
        std::mem::swap(&mut tables, &mut scratch);
        drop(fold_span);
        point.push(challenge);
        proof.round_polynomials.push(coefficients);
    }
    let values = std::array::from_fn(|k| tables[k][0]);
    if claim != terminal(field, row_vars, rho, tau, mix, &point, &values) {
        return Err(SumcheckError::InvalidTerminalClaim);
    }
    absorb_field_elements(transcript, &values, field);
    Ok(Output {
        proof,
        point,
        terminal: values,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{sumcheck::UngrindedRoundBoundary, transcript::Blake3Transcript};

    // Evaluate the defining polynomial from the original integer table, independently
    // of the prover's native first round, factored weights, and folded buffers.
    fn direct<const K: usize>(
        field: &Cfg,
        rows: &[[i16; K]],
        row_vars: usize,
        rho: &[F],
        tau: Option<&[F]>,
        mix: F,
        point: &[F],
    ) -> F {
        let weights = eq_table(point, field).unwrap();
        let values: [F; K] = std::array::from_fn(|k| {
            rows.iter().zip(&weights).fold(field.zero(), |s, (row, w)| {
                field.add(&s, &field.mul(w, &signed(i64::from(row[k]), field)))
            })
        });
        let equal = |a: &[F], b: &[F]| {
            a.iter().zip(b).fold(field.one(), |v, (a, b)| {
                let both_one = field.mul(a, b);
                let both_zero = field.mul(&field.sub(&field.one(), a), &field.sub(&field.one(), b));
                field.mul(&v, &field.add(&both_one, &both_zero))
            })
        };
        let mut result = field.mul(
            &equal(rho, &point[row_vars..]),
            &field.mul(&values[0], &values[0]),
        );
        if let Some(tau) = tau {
            result = field.add(
                &result,
                &field.mul(
                    &field.mul(&mix, &equal(tau, point)),
                    &field.sub(&field.mul(&values[1], &values[2]), &values[3]),
                ),
            );
        }
        result
    }

    fn check<const K: usize>() {
        let field = Cfg::from_prime_u128((1u128 << 127) - 1);
        let rho = [signed(3, &field), signed(5, &field)];
        let tau: Vec<_> = [7, 11, 13, 17, 19]
            .into_iter()
            .map(|v| signed(v, &field))
            .collect();
        let tau = (K == 4).then_some(tau.as_slice());
        let mix = signed(23, &field);
        let rows: Vec<[i16; K]> = (0..32)
            .map(|i| {
                std::array::from_fn(|k| match k {
                    0 => {
                        if i < 8 {
                            [i16::MIN, i16::MAX, -1, 0, 0, 1, 1, 0][i]
                        } else if i < 24 {
                            i as i16 * 37 - 509
                        } else {
                            0
                        }
                    }
                    1 => (i % 2) as i16,
                    2 => ((i / 2) % 2) as i16,
                    // Include nonzero rejection residuals: test the full aggregate identity.
                    _ => ((i / 3) % 2) as i16,
                })
            })
            .collect();
        let boolean = |index: usize, n| {
            (0..n)
                .map(|k| signed(((index >> k) & 1) as i64, &field))
                .collect::<Vec<_>>()
        };
        let initial = (0..32).fold(field.zero(), |s, i| {
            field.add(
                &s,
                &direct(&field, &rows, 3, &rho, tau, mix, &boolean(i, 5)),
            )
        });
        let mut prover = Blake3Transcript::new();
        let out = prove(
            &field,
            &mut prover,
            3,
            &rho,
            tau,
            mix,
            initial,
            |i| rows[i],
            &mut UngrindedRoundBoundary,
        )
        .unwrap();
        for round in 0..5 {
            for t in [0, 1, 2, 3] {
                let t = signed(t, &field);
                let expected = (0..1usize << (4 - round)).fold(field.zero(), |s, tail| {
                    let mut x = out.point[..round].to_vec();
                    x.push(t);
                    x.extend(boolean(tail, 4 - round));
                    field.add(&s, &direct(&field, &rows, 3, &rho, tau, mix, &x))
                });
                let actual = out.proof.round_polynomials[round]
                    .iter()
                    .rev()
                    .fold(field.zero(), |s, c| field.add(&field.mul(&s, &t), c));
                assert_eq!(actual, expected, "round {round}");
            }
        }
        let mut verifier = Blake3Transcript::new();
        let (point, claim) = out.proof.verify(&mut verifier, initial, 5, &field).unwrap();
        assert_eq!(claim, direct(&field, &rows, 3, &rho, tau, mix, &point));
        absorb_field_elements(&mut verifier, &out.terminal, &field);
        assert_eq!(
            prover.get_challenge::<field::Gf128>(),
            verifier.get_challenge::<field::Gf128>()
        );
        let mut bad = out.proof.clone();
        bad.round_polynomials[2][3] = field.add(&bad.round_polynomials[2][3], &field.one());
        assert!(
            bad.verify(&mut Blake3Transcript::new(), initial, 5, &field)
                .is_err()
        );
    }
    #[test]
    fn square_sum_matches_dense_polynomial() {
        check::<1>();
    }
    #[test]
    fn combined_sum_matches_dense_polynomial() {
        check::<4>();
    }
}

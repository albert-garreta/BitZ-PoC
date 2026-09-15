//! Arithmetic-only experiment. Candidates here are prototypes, not a migrated library.
use crate::{Case, Rng, arithmetic, case_requested, measure};
use f2z::{poly::univariate::binary_gf128::Gf128 as Gf, utils::wide_mul::WideMulAcc};
use std::hint::black_box;
mod binary_baselines;
mod binary_metrics;
mod bounded_product;
mod candidates;
mod integer;
mod integer_metrics;
mod prime;
mod prime_metrics;
mod two_limb_mac;
pub(crate) mod unified;

pub(crate) use candidates::tiled_ntt;

pub fn optimize(samples: usize, rng: &mut Rng) {
    candidates::run(samples, rng);
}

#[inline(never)]
fn f2z_dot(a: &[Gf], b: &[Gf]) -> Gf {
    assert_eq!(a.len(), b.len());
    let mut acc = Gf::wide_zero(&Gf::zero());
    for (a, b) in a.iter().zip(b) {
        Gf::wide_add_assign(&mut acc, &Gf::mul_wide(a, b));
    }
    Gf::from_wide(acc)
}

fn gf(samples: usize, rng: &mut Rng) {
    // Verify the actual production delayed kernel, including empty and ragged tails.
    for n in [0, 1, 2, 3, 7, 8, 9, 15, 17, 255, 1025] {
        let a = rng.values(n);
        let b = rng.values(n);
        let fa: Vec<_> = a
            .iter()
            .map(|v| Gf::from_polynomial_words([v.lo, v.hi]))
            .collect();
        let fb: Vec<_> = b
            .iter()
            .map(|v| Gf::from_polynomial_words([v.lo, v.hi]))
            .collect();
        let expected = arithmetic::dot::<arithmetic::Baseline>(&a, &b);
        assert_eq!(*f2z_dot(&fa, &fb).as_words(), [expected.lo, expected.hi]);
    }
    for n in [16, 1024, 65536, 1048576] {
        if !case_requested("gf_dot", &n.to_string()) {
            continue;
        }
        let a = rng.values(n);
        let b = rng.values(n);
        let fa: Vec<_> = a
            .iter()
            .map(|v| Gf::from_polynomial_words([v.lo, v.hi]))
            .collect();
        let fb: Vec<_> = b
            .iter()
            .map(|v| Gf::from_polynomial_words([v.lo, v.hi]))
            .collect();
        let expected = arithmetic::dot::<arithmetic::Baseline>(&a, &b);
        assert_eq!(*f2z_dot(&fa, &fb).as_words(), [expected.lo, expected.hi]);
        let mut cases = vec![
            Case::new("f2z_wide", n * 32, || {
                black_box(f2z_dot(black_box(&fa), black_box(&fb)));
            }),
            Case::new("flock_eager", n * 32, || {
                black_box(arithmetic::dot::<arithmetic::Baseline>(
                    black_box(&a),
                    black_box(&b),
                ));
            }),
        ];
        macro_rules! wide {
            ($k:literal,$name:literal) => {{
                assert_eq!(arithmetic::wide_dot::<$k>(&a, &b), expected);
                cases.push(Case::new($name, n * 32, || {
                    black_box(arithmetic::wide_dot::<$k>(black_box(&a), black_box(&b)));
                }));
            }};
        }
        wide!(1, "shared_wide1");
        wide!(2, "shared_wide2");
        wide!(4, "shared_wide4");
        wide!(8, "shared_wide8");
        measure("gf_dot", &n.to_string(), &mut cases, samples, rng);
    }
}

pub fn run(samples: usize, rng: &mut Rng) {
    gf(samples, rng);
    run_numeric(samples, rng);
}

pub fn run_numeric(samples: usize, rng: &mut Rng) {
    prime::run(samples, rng);
    integer::run(samples, rng);
}

pub fn integer_focus(samples: usize, rng: &mut Rng) {
    two_limb_mac::run(samples, rng);
    bounded_product::run(samples, rng);
}

pub fn operation_metrics(samples: usize, rng: &mut Rng) {
    binary_metrics::run(samples, rng);
    binary_baselines::run(samples, rng);
    integer_metrics::run(samples, rng);
    prime_metrics::run(samples, rng);
}

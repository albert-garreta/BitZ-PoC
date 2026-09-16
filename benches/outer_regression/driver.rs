//! Same inputs, timing boundary and verification on both sides of the refactor.
//! Only adapter.rs differs between the historical and current builds.
use super::{
    OuterSumcheckProof, R1csProductMles, SpartanField,
    raw_monty::{NativeProducts, NativeWideProducts},
    univariate_skip::UnivariateSkipOuterSumcheckProof,
};
use crate::{poly::mle::DenseMultilinearExtension, transcript::Blake3Transcript};
use field::{Fp, FpCtx, IntegerEmbedding, RingOps, Uint, WideMul};
use rand::{RngExt, SeedableRng, rngs::StdRng};
use rayon::prelude::*;
use std::{hint::black_box, time::Instant};
mod adapter;
type Elem = Fp<2>;
type Field = FpCtx<2>;

pub(super) enum Inputs {
    U32 {
        a: Vec<u64>,
        b: Vec<u64>,
        c: Vec<u64>,
        narrow_a: Vec<u32>,
        narrow_b: Vec<u32>,
    },
    U64 {
        a: Vec<u64>,
        b: Vec<u64>,
        lo: Vec<u64>,
        hi: Vec<u64>,
        c: Vec<u128>,
    },
    U128 {
        a: Vec<u128>,
        b: Vec<u128>,
        lo: Vec<u128>,
        hi: Vec<u128>,
        c: Vec<Uint<4>>,
    },
}
impl Inputs {
    fn new(bits: u32, n: usize) -> Self {
        let mut rng = StdRng::seed_from_u64(0xa345_0385 ^ n as u64 ^ u64::from(bits));
        // Include boundaries among otherwise full-width uniform operands.
        let a: Vec<u128> = (0..1usize << n)
            .map(|i| match i {
                0 => 0,
                1 => 1,
                2 => u128::MAX,
                _ => rng.random(),
            })
            .collect();
        let b: Vec<u128> = (0..a.len())
            .map(|i| if i < 3 { u128::MAX } else { rng.random() })
            .collect();
        match bits {
            32 => {
                let a: Vec<u64> = a.iter().map(|&v| v as u32 as u64).collect();
                let b: Vec<u64> = b.iter().map(|&v| v as u32 as u64).collect();
                Self::U32 {
                    c: a.iter().zip(&b).map(|(a, b)| a * b).collect(),
                    narrow_a: a.iter().map(|&v| v as u32).collect(),
                    narrow_b: b.iter().map(|&v| v as u32).collect(),
                    a,
                    b,
                }
            }
            64 => {
                let a: Vec<u64> = a.iter().map(|&v| v as u64).collect();
                let b: Vec<u64> = b.iter().map(|&v| v as u64).collect();
                let c: Vec<u128> = a
                    .iter()
                    .zip(&b)
                    .map(|(&a, &b)| a as u128 * b as u128)
                    .collect();
                Self::U64 {
                    lo: c.iter().map(|&v| v as u64).collect(),
                    hi: c.iter().map(|&v| (v >> 64) as u64).collect(),
                    a,
                    b,
                    c,
                }
            }
            128 => {
                let c: Vec<Uint<4>> = a
                    .iter()
                    .zip(&b)
                    .map(|(&a, &b)| {
                        *field::IntegerOps
                            .mul_wide(&Uint::<2>::from(a), &Uint::<2>::from(b))
                            .checked_resize_ct::<4>()
                            .value()
                    })
                    .collect();
                let lo = c
                    .iter()
                    .map(|v| v.as_words()[0] as u128 | ((v.as_words()[1] as u128) << 64))
                    .collect();
                let hi = c
                    .iter()
                    .map(|v| v.as_words()[2] as u128 | ((v.as_words()[3] as u128) << 64))
                    .collect();
                Self::U128 { a, b, c, lo, hi }
            }
            _ => panic!("unsupported width {bits}"),
        }
    }
    // The field-based skip entrypoint consumes owned projected tables. Include
    // this required work in the timer on BOTH versions; no fixture clones.
    fn project(&self, f: &Field, n: usize) -> R1csProductMles<Elem> {
        let mle = |v| DenseMultilinearExtension {
            num_vars: n,
            evaluations: v,
        };
        macro_rules! project {
            ($a:expr,$b:expr,$c:expr) => {
                R1csProductMles {
                    az: mle($a.par_iter().map(|v| f.from_integer(v)).collect()),
                    bz: mle($b.par_iter().map(|v| f.from_integer(v)).collect()),
                    cz: mle($c.par_iter().map(|v| f.from_integer(v)).collect()),
                }
            };
        }
        match self {
            Self::U32 { a, b, c, .. } => project!(a, b, c),
            Self::U64 { a, b, c, .. } => project!(a, b, c),
            Self::U128 { a, b, c, .. } => project!(a, b, c),
        }
    }
}

pub(super) enum Proof {
    Ordinary(OuterSumcheckProof<Elem>),
    Skip(UnivariateSkipOuterSumcheckProof<Elem>),
}
impl Proof {
    fn verify_and_hash(&self, field: &Field, tau: &[Elem], n: usize, digest: [u8; 32]) -> String {
        let mut verifier = Blake3Transcript::new();
        let mut hasher = blake3::Hasher::new();
        let mut hash = |x: &Elem| {
            hasher.update(&x.canonical_element_encoding(field));
        };
        let outer = match self {
            Self::Ordinary(p) => {
                p.verify(&mut verifier, field.zero(), tau, field).unwrap();
                p
            }
            Self::Skip(p) => {
                p.verify(&mut verifier, tau, n, field).unwrap();
                for x in p
                    .skip
                    .finite_q_evaluations
                    .iter()
                    .chain(std::iter::once(&p.skip.q_at_infinity))
                {
                    hash(x)
                }
                &p.tail
            }
        };
        assert_eq!(digest, verifier.state_digest());
        for x in outer.sumcheck.round_polynomials.iter().flatten().chain([
            &outer.az_mle_claim,
            &outer.bz_mle_claim,
            &outer.cz_mle_claim,
        ]) {
            hash(x)
        }
        hasher.update(&digest);
        hasher.finalize().to_hex().to_string()
    }
}
fn words(name: &str, default: &str) -> Vec<String> {
    std::env::var(name)
        .unwrap_or_else(|_| default.into())
        .split_whitespace()
        .map(str::to_owned)
        .collect()
}

pub fn run() {
    let reps = std::env::var("OUTER_REPS").map_or(5, |s| s.parse().unwrap());
    let field = Fp::<2>::make_cfg(&Uint::from((1u128 << 100) - 15)).unwrap();
    let variants = words("OUTER_VARIANTS", "production");
    for shape in words("OUTER_SHAPES", "12 15 17 19") {
        let n: usize = shape.parse().unwrap();
        for width in words("OUTER_WIDTHS", "32 64 128") {
            let bits: u32 = width.parse().unwrap();
            let input = Inputs::new(bits, n);
            for protocol in words("OUTER_PROTOCOLS", "ordinary skip-1 skip-2 skip-3 skip-4") {
                let k: usize = if protocol == "ordinary" {
                    0
                } else {
                    protocol.strip_prefix("skip-").unwrap().parse().unwrap()
                };
                if n < k {
                    continue;
                }
                let tau: Vec<_> = (0..n - k)
                    .map(|i| field.from_integer(&(i as u64 + 7)))
                    .collect();
                let prepared = adapter::prepare(&field, k);
                let mut expected = None;
                // First trial is an excluded warmup. Rotate in-process variants.
                for sample in 0..=reps {
                    for j in 0..variants.len() {
                        let variant = &variants[(j + sample) % variants.len()];
                        let mut transcript = Blake3Transcript::new();
                        let started = Instant::now();
                        let proof = match variant.as_str() {
                            "production" => {
                                adapter::production(&input, &field, &mut transcript, &tau, n, k)
                            }
                            "generic" => adapter::generic(
                                &input,
                                &field,
                                &mut transcript,
                                &tau,
                                k,
                                &prepared,
                            ),
                            _ => panic!("unknown variant {variant}"),
                        };
                        let ns = started.elapsed().as_nanos();
                        let digest = transcript.state_digest();
                        let fingerprint = proof.verify_and_hash(&field, &tau, n, digest);
                        if let Some(ref expected) = expected {
                            assert_eq!(&fingerprint, expected)
                        } else {
                            expected = Some(fingerprint.clone())
                        }
                        black_box(&proof);
                        println!(
                            "OUTER_SAMPLE {}",
                            serde_json::json!({"revision":adapter::REVISION,"variant":variant,"bits":bits,"rows":1usize<<n,"protocol":protocol,"threads":rayon::current_num_threads(),"sample":sample,"warmup":sample==0,"ns":ns.to_string(),"proof_digest":fingerprint,"verified":true})
                        );
                    }
                }
            }
        }
    }
}

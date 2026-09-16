//! Benchmark-only adapter: every arithmetic kernel comes unchanged from master.
use super::{SpartanField, raw_monty::{self as raw, NativeProducts, RawProducts, RawMontyCtx}, protocol::{FieldConfig, SpartanF2zField as Elem}};
use crate::{transcript::Blake3Transcript, utils::delayed_reduction::OptimizedMonty128Reducer};
use crypto_primitives::{FromWithConfig, PrimeField, crypto_bigint_uint::Uint};
use rand::{RngExt, SeedableRng, rngs::StdRng};
use std::{hint::black_box, time::Instant};

enum Inputs {
    U32 { a: Vec<u64>, b: Vec<u64>, c: Vec<u64> },
    U64 { a: Vec<u64>, b: Vec<u64>, lo: Vec<u64>, hi: Vec<u64> },
    U128 { a: Vec<u128>, b: Vec<u128>, lo: Vec<u128>, hi: Vec<u128> },
}
impl Inputs {
    fn new(bits: u32, n: usize) -> Self {
        // Identical RNG, order, seed and boundary rows to outer_regression.
        let mut rng = StdRng::seed_from_u64(0xa345_0385 ^ n as u64 ^ u64::from(bits));
        let a: Vec<u128> = (0..1usize<<n).map(|i| match i {0=>0,1=>1,2=>u128::MAX,_=>rng.random()}).collect();
        let b: Vec<u128> = (0..a.len()).map(|i| if i<3 {u128::MAX} else {rng.random()}).collect();
        match bits {
            32 => {
                let a: Vec<u64> = a.iter().map(|v| *v as u32 as u64).collect();
                let b: Vec<u64> = b.iter().map(|v| *v as u32 as u64).collect();
                let c = a.iter().zip(&b).map(|(a,b)| a*b).collect();
                Self::U32{a,b,c}
            }
            64 => {
                let a: Vec<u64> = a.iter().map(|v| *v as u64).collect();
                let b: Vec<u64> = b.iter().map(|v| *v as u64).collect();
                let c: Vec<u128> = a.iter().zip(&b).map(|(a,b)| *a as u128 * *b as u128).collect();
                let lo = c.iter().map(|v| *v as u64).collect();
                let hi = c.iter().map(|v| (*v>>64) as u64).collect();
                Self::U64{a,b,lo,hi}
            }
            128 => {
                use num_bigint::BigUint;
                let c: Vec<_> = a.iter().zip(&b).map(|(a,b)| BigUint::from(*a)*BigUint::from(*b)).collect();
                let mask = BigUint::from(u128::MAX);
                let lo = c.iter().map(|v| u128::try_from(v & &mask).unwrap()).collect();
                let hi = c.iter().map(|v| u128::try_from(v >> 128usize).unwrap()).collect();
                Self::U128{a,b,lo,hi}
            }
            _=>panic!("unsupported width"),
        }
    }
    fn fixture_digest(&self) -> String {
        let mut hash = blake3::Hasher::new();
        let mut row = |a: u128,b: u128,c: [u64;4]| {
            hash.update(&a.to_le_bytes()); hash.update(&b.to_le_bytes());
            for word in c { hash.update(&word.to_le_bytes()); }
        };
        match self {
            Self::U32{a,b,c} => for i in 0..a.len() { row(a[i] as u128,b[i] as u128,[c[i],0,0,0]); },
            Self::U64{a,b,lo,hi} => for i in 0..a.len() { row(a[i] as u128,b[i] as u128,[lo[i],hi[i],0,0]); },
            Self::U128{a,b,lo,hi} => for i in 0..a.len() { row(a[i],b[i],[lo[i] as u64,(lo[i]>>64) as u64,hi[i] as u64,(hi[i]>>64) as u64]); },
        }
        hash.finalize().to_hex().to_string()
    }
    fn prove(&self, t: &mut Blake3Transcript, f: &FieldConfig, tau: &[Elem], n: usize) -> super::OuterSumcheckProof<Elem> {
        let ctx = RawMontyCtx::new(f);
        let reducer = OptimizedMonty128Reducer::new(f).unwrap();
        let (low, high) = raw::make_equality_factors_raw(&ctx,tau);
        let zero = Elem::zero_with_cfg(f);
        if let Self::U32{a,b,c} = self {
            return raw::prove_outer_native_raw(t,&ctx,&reducer,zero,tau,low,high,NativeProducts{az:a,bz:b,cz:c}).unwrap().proof;
        }
        let products = match self {
            Self::U64{a,b,lo,hi} => RawProducts::from_native_limbs(&ctx,a,b,lo,hi,1<<n),
            Self::U128{a,b,lo,hi} => RawProducts::from_native_u128_halves(&ctx,a,b,lo,hi,1<<n),
            _=>unreachable!(),
        };
        raw::prove_outer_field_raw(t,&ctx,&reducer,zero,tau,low,high,products).unwrap().proof
    }
}
fn words(name: &str, default: &str) -> Vec<String> {
    std::env::var(name).unwrap_or_else(|_|default.into()).split_whitespace().map(str::to_owned).collect()
}
pub fn run() {
    let reps: usize = std::env::var("OUTER_REPS").map_or(21,|v|v.parse().unwrap());
    let f = Elem::make_cfg(&Uint::from((1u128<<100)-15)).unwrap();
    for shape in words("OUTER_SHAPES","12 15 17 19") {
        let n: usize = shape.parse().unwrap();
        for width in words("OUTER_WIDTHS","32 64 128") {
            let bits: u32 = width.parse().unwrap();
            let input = Inputs::new(bits,n);
            let fixture_digest = input.fixture_digest();
            let tau: Vec<_> = (0..n).map(|i|Elem::from_with_cfg(i as u64+7,&f)).collect();
            let mut expected = None;
            for sample in 0..=reps {
                let mut transcript = Blake3Transcript::new();
                let started = Instant::now();
                let proof = input.prove(&mut transcript,&f,&tau,n);
                let ns = started.elapsed().as_nanos();
                let mut verifier = Blake3Transcript::new();
                proof.verify(&mut verifier,Elem::zero_with_cfg(&f),&tau,&f).unwrap();
                assert_eq!(transcript.state_digest(),verifier.state_digest());
                let mut hash = blake3::Hasher::new();
                for x in proof.sumcheck.round_polynomials.iter().flatten().chain([&proof.az_mle_claim,&proof.bz_mle_claim,&proof.cz_mle_claim]) {
                    hash.update(&x.canonical_element_encoding());
                }
                hash.update(&transcript.state_digest());
                let digest = hash.finalize().to_hex().to_string();
                if let Some(ref expected) = expected { assert_eq!(&digest,expected); } else {expected=Some(digest.clone());}
                black_box(&proof);
                println!("OUTER_SAMPLE {}",serde_json::json!({"revision":"ac0aa44c5785db30f889fce8c5cdc98264a0e686","variant":"master","bits":bits,"rows":1usize<<n,"protocol":"ordinary","threads":rayon::current_num_threads(),"sample":sample,"warmup":sample==0,"ns":ns.to_string(),"proof_digest":digest,"fixture_digest":fixture_digest,"verified":true}));
            }
        }
    }
}

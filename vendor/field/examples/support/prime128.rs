use field::{FpCtx, PrimeSearchPolicy, PrimeSpec, PublicRandomSource, Uint};
use rand_core::{RngCore, SeedableRng};
use rand_pcg::Pcg64;
use std::{hint::black_box, time::Instant};

// The result of the seeded runtime search below, embedded for the static path.
// Limbs are little-endian. Changing the seed requires updating this declaration.
field::define_prime_field! {
    pub Random128 {
        limbs: 2,
        modulus: [0xe829_a5a1_22da_0c0d, 0xe8dc_0598_0109_8bb6],
        element: Random128Element,
        context: Random128Field,
    }
}

// Reproducible public benchmark randomness, not protocol randomness.
struct BenchmarkRandom(Pcg64);
impl PublicRandomSource for BenchmarkRandom {
    fn fill_bytes(&mut self, output: &mut [u8]) {
        self.0.fill_bytes(output);
    }
}

pub fn sample_field() -> FpCtx<2> {
    let mut rng = BenchmarkRandom(Pcg64::seed_from_u64(black_box(20260924)));
    let start = Instant::now();
    let dynamic = FpCtx::<2>::sample_prime_public(
        &mut rng,
        Uint::from(1u128 << 127)..=Uint::from(u128::MAX),
        &PrimeSearchPolicy::default(),
    )
    .expect("sample a 128-bit prime");
    let setup = start.elapsed();
    eprintln!("Sampled 128-bit p = {:#034x}", dynamic.modulus_u128());
    eprintln!("Runtime prime search + context setup: {setup:?}");
    assert_eq!(*dynamic.modulus(), Random128::MODULUS);
    dynamic
}

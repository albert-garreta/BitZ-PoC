use rand::{rngs::StdRng, seq::SliceRandom};
use rand_core::SeedableRng;

/// Reorder the elements in slice using the given randomness seed.
/// Vendored from zip-plus (`utils::shuffle_seeded`) — the RAA
/// permutation sampler.
pub fn shuffle_seeded<T>(slice: &mut [T], seed: u64) {
    let mut rng = StdRng::seed_from_u64(seed);
    slice.shuffle(&mut rng);
}

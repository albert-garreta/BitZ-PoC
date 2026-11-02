//! Proof bytes of BitZ's opening below Ligerito's ladders, `m = 10..=18`
//! committed bits at 100 bits: the clear opening (`--ladder clear`, the
//! GKR at its byte-optimal split plus the message) against the cheapest
//! sampled Johnson configuration flock's validator accepts at the size.
//!
//! The sampled estimate takes flock's own per-level query counts and
//! simulates the octopus multi-proof over random distinct positions
//! (shared Merkle nodes counted once); it assumes a Ligerito that may stop
//! after its lane fold when the ladder rule leaves no recursive level
//! (`m ≤ 19`), which the crate does not run. It is a floor on what a
//! sampled opening would cost at the target, not a measurement. The
//! Ligerito path also carries the inner-product sumcheck (`m` rounds) and
//! the ring switch (128 elements), counted in its total.
//!
//! ```text
//! cargo run --release --example tiny_opening_probe
//! ```

use bitz::ligerito_flock::try_custom_johnson_config_bits;

const ELEM: usize = 16;
const HASH: usize = 32;
const TARGET: usize = 100;

fn xorshift(state: &mut u64) -> u64 {
    *state ^= *state << 13;
    *state ^= *state >> 7;
    *state ^= *state << 17;
    *state
}

/// GKR bytes at `(t, s)`: two elements per sumcheck round, two per layer.
fn gkr_bytes(t: usize, s: usize) -> usize {
    2 * ELEM * (t * s + t * (t + 1) / 2)
}

/// The GKR plus the folds at the byte-optimal split of `m` bits.
fn best_gkr(m: usize) -> (usize, usize) {
    (7..=m)
        .map(|t| (ELEM * (1 << (m - t)) + gkr_bytes(t, m - t), t))
        .min()
        .expect("a split")
}

/// Nodes of the octopus multi-proof of `queries` random distinct leaves
/// among `2^depth`.
fn multiproof_nodes(depth: usize, queries: usize, seed: &mut u64) -> usize {
    let leaves = 1usize << depth;
    let queries = queries.min(leaves);
    let mut positions: Vec<usize> = Vec::with_capacity(queries);
    while positions.len() < queries {
        let p = (xorshift(seed) as usize) & (leaves - 1);
        if !positions.contains(&p) {
            positions.push(p);
        }
    }
    let mut occupied = positions;
    let mut nodes = 0;
    for _ in 0..depth {
        occupied.sort_unstable();
        occupied.dedup();
        nodes += occupied
            .iter()
            .filter(|&&node| occupied.binary_search(&(node ^ 1)).is_err())
            .count();
        occupied = occupied.iter().map(|node| node >> 1).collect();
    }
    nodes
}

fn main() {
    println!(
        "{:>2} | {:>8} {:>5} {:>6} | {:>8} {:>8} {:>8} {:>12}",
        "m", "clear B", "t", "msg B", "sampl. B", "lig. B", "levels", "L0 (r,k,Q)"
    );
    let mut seed = 0x9e37_79b9_7f4a_7c15u64;
    for m in 10..=18 {
        let log_n = m - 7;
        let message = ELEM * (1 << log_n) + 4;
        let (gkr, t) = best_gkr(m);
        let clear = gkr + message + 17;

        let mut best: Option<(usize, usize, usize, String)> = None;
        for r0 in 1..=12 {
            for k0 in 1..log_n {
                let Ok(cfg) = try_custom_johnson_config_bits(m, r0, k0, Some(TARGET)) else {
                    continue;
                };
                let mut lig = HASH;
                for (i, level) in cfg.levels.iter().enumerate() {
                    let depth = level.log_msg_cols + level.log_inv_rate;
                    lig += level.queries * (1 << level.log_num_interleaved) * ELEM;
                    lig += multiproof_nodes(depth, level.queries, &mut seed) * HASH;
                    lig += level.k_recursive * 3 * ELEM;
                    lig += level.ood_samples * ELEM;
                    lig += 8 + usize::from(level.fold_grinding_bits > 0) * 8 * level.k_recursive;
                    if i > 0 {
                        lig += HASH;
                    }
                }
                lig += ELEM << cfg.final_block.yr_log_n;
                let l0 = &cfg.levels[0];
                let label = format!("({r0},{k0},{})", l0.queries);
                let total = gkr + 3 * ELEM * m + 128 * ELEM + lig + 17;
                if best.as_ref().is_none_or(|b| total < b.0) {
                    best = Some((total, lig, cfg.levels.len(), label));
                }
            }
        }
        match best {
            Some((total, lig, levels, label)) => println!(
                "{m:>2} | {clear:>8} {t:>5} {message:>6} | {total:>8} {lig:>8} {levels:>8} {label:>12}"
            ),
            None => println!("{m:>2} | {clear:>8} {t:>5} {message:>6} | {:>8}", "none"),
        }
    }
}

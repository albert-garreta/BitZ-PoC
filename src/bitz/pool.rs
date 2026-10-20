//! Reused large `Gf` buffers. The virtual opening's transposed weights and
//! its sumcheck tables are `2^m`-entry vectors (1 GB at `m = 26`); fresh
//! allocations fault every page in on first touch (hundreds of ms per GB), so
//! the prover keeps them across proofs.

use std::sync::Mutex;

use field::Gf128 as Gf;

static POOL: Mutex<Vec<Vec<Gf>>> = Mutex::new(Vec::new());

/// A buffer of `len` initialized entries with ARBITRARY contents: callers
/// overwrite every entry they read.
pub fn take(len: usize) -> Vec<Gf> {
    let reused = POOL.lock().ok().and_then(|mut pool| {
        let best = pool
            .iter()
            .enumerate()
            .filter(|(_, v)| v.capacity() >= len)
            .min_by_key(|(_, v)| v.capacity())
            .map(|(i, _)| i)?;
        Some(pool.swap_remove(best))
    });
    match reused {
        Some(mut v) => {
            if v.len() >= len {
                v.truncate(len);
            } else {
                v.resize(len, Gf::zero());
            }
            v
        }
        None => vec![Gf::zero(); len],
    }
}

/// Returns a buffer to the pool (kept: at most four, the largest).
pub fn recycle(v: Vec<Gf>) {
    if v.capacity() < 1 << 20 {
        return;
    }
    if let Ok(mut pool) = POOL.lock() {
        pool.push(v);
        pool.sort_by_key(|b| std::cmp::Reverse(b.capacity()));
        pool.truncate(4);
    }
}

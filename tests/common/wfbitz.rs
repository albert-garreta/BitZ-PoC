//! Small deterministic algebra fixtures; their ad-hoc ladder makes no security claim.
#![allow(dead_code)]
use bitz::wfbitz::{Pcs, Shape};

pub fn next(state: &mut u64) -> u64 {
    *state ^= *state << 13;
    *state ^= *state >> 7;
    *state ^= *state << 17;
    *state
}
pub fn add(a: u128, b: u128, q: u128) -> u128 {
    let sum = a + b;
    if sum >= q { sum - q } else { sum }
}
/// Independent double-and-add reference; test moduli are below 2^127.
pub fn mul(mut a: u128, mut b: u128, q: u128) -> u128 {
    let mut result = 0;
    while b != 0 {
        if b & 1 != 0 {
            result = add(result, a, q);
        }
        b >>= 1;
        if b != 0 {
            a = add(a, a, q);
        }
    }
    result
}
pub fn eq(index: usize, point: &[u128], q: u128) -> u128 {
    point.iter().enumerate().fold(1, |acc, (i, &r)| {
        let factor = if index >> i & 1 == 1 {
            r
        } else {
            (q + 1 - r) % q
        };
        mul(acc, factor, q)
    })
}
pub fn bit(rows: &[Vec<u64>], shape: Shape, index: usize) -> bool {
    let row = index & (shape.rows() - 1);
    rows[index >> shape.log_rows()][row / 64] >> (row % 64) & 1 != 0
}
pub fn target(rows: &[Vec<u64>], shape: Shape, row: &[u128], col: &[u128], q: u128) -> u128 {
    let mut result = 0;
    for (c, &cw) in col.iter().enumerate() {
        for (r, &rw) in row.iter().enumerate() {
            if bit(rows, shape, c * shape.rows() + r) {
                result = add(result, mul(rw, cw, q), q);
            }
        }
    }
    result
}
pub fn pcs(shape: Shape) -> Pcs {
    let (pc, vc) = bitz::ligerito_flock::lig_configs(
        shape.log_packed_len(),
        bitz::ligerito_flock::LigConfig::Adhoc {
            log_batch: 2,
            log_inv_rate: 2,
        },
    )
    .unwrap();
    Pcs::from_configs(&shape, pc, vc).unwrap()
}

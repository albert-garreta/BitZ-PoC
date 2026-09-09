//! Degree-two sumcheck over a virtual, disjoint concatenation of bit witnesses.
//!
//! The first seven rounds use byte lookup tables: each round message is a
//! linear functional of the original packed bits. Only after those rounds do
//! we allocate field tables, at one element per packed word of the virtual
//! witness. In particular, no field table of the original bit domain exists.
use super::{BinaryClaim, Error, Gf, opening::Geometry};
use crate::ligerito_flock::{f128_to_gf, gf_to_f128};
use crate::transcript::{Blake3Transcript, traits::Transcript};
use flock_core::field::F128 as F;
#[cfg(feature = "parallel")]
use rayon::prelude::*;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Proof {
    pub rounds: Vec<[F; 2]>,
    pub value: F,
}

pub(crate) fn eq_table(point: &[F]) -> Vec<F> {
    let mut out = vec![F::ONE];
    for &r in point {
        let n = out.len();
        out.resize(2 * n, F::ZERO);
        for i in 0..n {
            let high = out[i] * r;
            out[n + i] = high;
            out[i] += high;
        }
    }
    out
}

pub(super) fn mle(table: &[F], point: &[F]) -> F {
    debug_assert_eq!(table.len(), 1 << point.len());
    table
        .iter()
        .zip(eq_table(point))
        .fold(F::ZERO, |s, (&v, e)| s + v * e)
}

fn eq_eval(a: &[F], b: &[F]) -> F {
    debug_assert_eq!(a.len(), b.len());
    a.iter()
        .zip(b)
        .fold(F::ONE, |p, (&x, &y)| p * (F::ONE + x + y))
}

fn fold(table: &mut Vec<F>, r: F) {
    for i in 0..table.len() / 2 {
        table[i] = table[2 * i] + r * (table[2 * i] + table[2 * i + 1]);
    }
    table.truncate(table.len() / 2);
}

fn observe(t: &mut Blake3Transcript, values: &[F]) {
    for &v in values {
        t.absorb_slice(&v.lo.to_le_bytes());
        t.absorb_slice(&v.hi.to_le_bytes());
    }
}

fn sample(t: &mut Blake3Transcript) -> F {
    gf_to_f128(t.get_field_challenge::<Gf>(&()))
}

fn evaluate_round([u0, u2]: [F; 2], sum: F, r: F) -> F {
    u0 + r * (sum + u2) + r * r * u2
}

/// LUT for the F2-linear map from a packed word to a field-valued bit sum.
fn byte_table(coefficients: &[F; 128]) -> Vec<F> {
    let mut table = vec![F::ZERO; 16 * 256];
    for byte in 0..16 {
        for val in 1usize..256 {
            table[byte * 256 + val] = table[byte * 256 + (val & (val - 1))]
                + coefficients[byte * 8 + val.trailing_zeros() as usize];
        }
    }
    table
}

fn apply(table: &[F], packed: F) -> F {
    let bytes = ((packed.lo as u128) | ((packed.hi as u128) << 64)).to_le_bytes();
    bytes
        .iter()
        .enumerate()
        .fold(F::ZERO, |s, (i, &b)| s + table[i * 256 + b as usize])
}

fn packed_round(packed: &[F], low: &[F], high: &[F], prefix_eq: &[F], round: usize) -> [F; 2] {
    let stride = 128 >> round;
    let low_blocks = low.len() / stride;
    #[cfg(feature = "parallel")]
    let parallel_blocks = low_blocks >= rayon::current_num_threads();
    let compute = |block: usize| {
        let mut c0 = [F::ZERO; 128];
        let mut c2 = [F::ZERO; 128];
        for bit in 0..128 {
            let j = block * stride + (bit >> (round + 1)) * 2;
            let prefix = prefix_eq[bit & ((1 << round) - 1)];
            if bit & (1 << round) == 0 {
                c0[bit] = prefix * low[j];
            }
            c2[bit] = prefix * (low[j] + low[j + 1]);
        }
        let tab0 = byte_table(&c0);
        let tab2 = byte_table(&c2);
        // SHA has one low block and a long scan: parallelize that scan.
        // Multiplication has many low blocks: parallelize whole blocks so
        // their LUT construction runs concurrently, without tiny nested jobs.
        let at = |i: usize| {
            let word = packed[i * low_blocks + block];
            [apply(&tab0, word) * high[i], apply(&tab2, word) * high[i]]
        };
        #[cfg(feature = "parallel")]
        if !parallel_blocks {
            return (0..high.len())
                .into_par_iter()
                .map(at)
                .reduce(|| [F::ZERO; 2], |a, b| [a[0] + b[0], a[1] + b[1]]);
        }
        (0..high.len())
            .map(at)
            .fold([F::ZERO; 2], |a, b| [a[0] + b[0], a[1] + b[1]])
    };
    #[cfg(feature = "parallel")]
    if parallel_blocks {
        return (0..low_blocks)
            .into_par_iter()
            .map(compute)
            .reduce(|| [F::ZERO; 2], |a, b| [a[0] + b[0], a[1] + b[1]]);
    }
    (0..low_blocks)
        .map(compute)
        .fold([F::ZERO; 2], |a, b| [a[0] + b[0], a[1] + b[1]])
}

fn bind_claims(t: &mut Blake3Transcript, a: &BinaryClaim, b: &BinaryClaim) -> F {
    t.absorb_slice(b"hybrid/joint-bit-sumcheck/v1");
    observe(t, &[a.value, b.value]);
    sample(t)
}

pub(super) fn prove(
    t: &mut Blake3Transcript,
    geometry: &Geometry,
    sources: [&[F]; 2],
    claims: [&BinaryClaim; 2],
) -> (Proof, Vec<Gf>) {
    let rho = bind_claims(t, claims[0], claims[1]);
    let scales = [F::ONE, rho];
    let mut low = [claims[0].low.clone(), claims[1].low.clone()];
    let high = [
        eq_table(&claims[0].high_point),
        eq_table(&claims[1].high_point),
    ];
    let mut value = claims[0].value + rho * claims[1].value;
    let mut point = Vec::with_capacity(geometry.bit_log());
    let mut rounds = Vec::with_capacity(geometry.bit_log());
    for round in 0..7 {
        let prefix_eq = eq_table(&point);
        let a = packed_round(sources[0], &low[0], &high[0], &prefix_eq, round);
        let b = packed_round(sources[1], &low[1], &high[1], &prefix_eq, round);
        let message = [a[0] + rho * b[0], a[1] + rho * b[1]];
        observe(t, &message);
        let r = sample(t);
        value = evaluate_round(message, value, r);
        point.push(r);
        rounds.push(message);
        for weights in &mut low {
            fold(weights, r);
        }
    }
    let bit_eq: [F; 128] = eq_table(&point).try_into().expect("seven coordinates");
    let bit_table = byte_table(&bit_eq);
    let mut witness = vec![F::ZERO; 1 << geometry.packed_log()];
    let mut weights = vec![F::ZERO; witness.len()];
    for branch in 0..2 {
        let nlow = low[branch].len();
        for (index, &word) in sources[branch].iter().enumerate() {
            let dst = geometry.embed(branch, index);
            witness[dst] = apply(&bit_table, word);
            weights[dst] = scales[branch] * low[branch][index % nlow] * high[branch][index / nlow];
        }
    }
    drop(low);
    drop(high);
    while witness.len() > 1 {
        let at = |i: usize| {
            let x0 = witness[2 * i];
            let dx = x0 + witness[2 * i + 1];
            let w0 = weights[2 * i];
            let dw = w0 + weights[2 * i + 1];
            [x0 * w0, dx * dw]
        };
        #[cfg(feature = "parallel")]
        let message = (0..witness.len() / 2)
            .into_par_iter()
            .map(at)
            .reduce(|| [F::ZERO; 2], |a, b| [a[0] + b[0], a[1] + b[1]]);
        #[cfg(not(feature = "parallel"))]
        let message = (0..witness.len() / 2)
            .map(at)
            .fold([F::ZERO; 2], |a, b| [a[0] + b[0], a[1] + b[1]]);
        observe(t, &message);
        let r = sample(t);
        value = evaluate_round(message, value, r);
        point.push(r);
        rounds.push(message);
        fold(&mut witness, r);
        fold(&mut weights, r);
    }
    debug_assert_eq!(value, witness[0] * weights[0]);
    observe(t, &witness);
    (
        Proof {
            rounds,
            value: witness[0],
        },
        point.into_iter().map(f128_to_gf).collect(),
    )
}

pub(super) fn verify(
    t: &mut Blake3Transcript,
    geometry: &Geometry,
    claims: [&BinaryClaim; 2],
    proof: &Proof,
) -> Result<Vec<Gf>, Error> {
    if proof.rounds.len() != geometry.bit_log() {
        return Err(Error::Invalid("joint sumcheck shape"));
    }
    let rho = bind_claims(t, claims[0], claims[1]);
    let mut value = claims[0].value + rho * claims[1].value;
    let mut point = Vec::with_capacity(proof.rounds.len());
    for &message in &proof.rounds {
        observe(t, &message);
        let r = sample(t);
        value = evaluate_round(message, value, r);
        point.push(r);
    }
    let eval = |branch: usize| {
        let (original, padding) = geometry.project_point(branch, &point);
        let claim = claims[branch];
        let nlow = claim.low.len().trailing_zeros() as usize;
        padding * mle(&claim.low, &original[..nlow]) * eq_eval(&claim.high_point, &original[nlow..])
    };
    if value != (eval(0) + rho * eval(1)) * proof.value {
        return Err(Error::Invalid("joint sumcheck terminal claim"));
    }
    observe(t, &[proof.value]);
    Ok(point.into_iter().map(f128_to_gf).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn streamed_rounds_equal_dense_sumcheck_in_both_lane_orders() {
        let mut seed = 0x123456789abcdef0u64;
        let mut next = || {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            F {
                lo: seed,
                hi: seed.rotate_left(29),
            }
        };
        for logs in [[9, 10], [10, 9], [9, 9]] {
            let geometry = Geometry::new(logs).unwrap();
            let packed: [Vec<F>; 2] =
                std::array::from_fn(|b| (0..1 << logs[b]).map(|_| next()).collect());
            let claims: [BinaryClaim; 2] = std::array::from_fn(|b| {
                // Exercise both scheduling paths: many LUT blocks for the
                // first source and one block with a long scan for the second.
                let low_log = if b == 0 { 11 } else { 7 };
                let low: Vec<F> = (0..1 << low_log).map(|_| next()).collect();
                let high_point: Vec<F> = (0..logs[b] + 7 - low_log).map(|_| next()).collect();
                let high = eq_table(&high_point);
                let mut value = F::ZERO;
                for (i, word) in packed[b].iter().enumerate() {
                    let bits = word.lo as u128 | ((word.hi as u128) << 64);
                    for bit in 0..128 {
                        if bits & (1 << bit) != 0 {
                            let index = i * 128 + bit;
                            value += low[index & ((1 << low_log) - 1)] * high[index >> low_log];
                        }
                    }
                }
                BinaryClaim {
                    low,
                    high_point,
                    value,
                }
            });
            let mut actual_t = Blake3Transcript::new();
            let (actual, point) = prove(
                &mut actual_t,
                &geometry,
                [&packed[0], &packed[1]],
                [&claims[0], &claims[1]],
            );
            let mut reference_t = Blake3Transcript::new();
            let rho = bind_claims(&mut reference_t, &claims[0], &claims[1]);
            let mut v = vec![F::ZERO; 1 << geometry.bit_log()];
            let mut w = v.clone();
            for b in 0..2 {
                let high = eq_table(&claims[b].high_point);
                for (i, word) in packed[b].iter().enumerate() {
                    let bits = word.lo as u128 | ((word.hi as u128) << 64);
                    for bit in 0..128 {
                        let index = i * 128 + bit;
                        let dst = geometry.embed(b, i) * 128 + bit;
                        v[dst] = if bits & (1 << bit) != 0 {
                            F::ONE
                        } else {
                            F::ZERO
                        };
                        w[dst] = claims[b].low[index % claims[b].low.len()]
                            * high[index / claims[b].low.len()]
                            * if b == 0 { F::ONE } else { rho };
                    }
                }
            }
            let mut rounds = Vec::new();
            let mut reference_point = Vec::new();
            while v.len() > 1 {
                let message = (0..v.len() / 2).fold([F::ZERO; 2], |mut sums, i| {
                    sums[0] += v[2 * i] * w[2 * i];
                    sums[1] += (v[2 * i] + v[2 * i + 1]) * (w[2 * i] + w[2 * i + 1]);
                    sums
                });
                observe(&mut reference_t, &message);
                let r = sample(&mut reference_t);
                rounds.push(message);
                reference_point.push(f128_to_gf(r));
                fold(&mut v, r);
                fold(&mut w, r);
            }
            observe(&mut reference_t, &v);
            assert_eq!(
                actual,
                Proof {
                    rounds,
                    value: v[0]
                }
            );
            assert_eq!(point, reference_point);
            assert_eq!(sample(&mut actual_t), sample(&mut reference_t));
            let verified = verify(
                &mut Blake3Transcript::new(),
                &geometry,
                [&claims[0], &claims[1]],
                &actual,
            )
            .unwrap();
            assert_eq!(point, verified);
        }
    }
}

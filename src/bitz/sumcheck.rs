//! Their `pcs::sumcheck`: the degree-2 sumcheck reducing a factored
//! inner-product claim over the committed bits to one MLE claim. Rounds
//! bind the lowest remaining coordinate (rows before columns) and send the
//! coefficients `[a0, a1, a2]`; the terminal message is the witness
//! evaluation. The prover runs it on the two small tables the bilinear
//! structure gives (see [`prove`]); the verifier replays it their way.

#[cfg(feature = "parallel")]
use rayon::prelude::*;

use super::params::{LinearClaimGf, SumClaimGf};
use super::pcs::{ProveError, VerifyError};
use super::transcript::{ProverState, VerifierState};
use crate::{cfg_chunks_mut, cfg_into_iter, cfg_iter_mut};
use crate::ligerito::{xi_combined_rows, xi_combined_rows_packed};

use crate::pcs::IntegerMatrixLayout;
use crate::poly::utils::build_eq_x_r_vec;
use crate::utils::wide_mul::WideMulAcc;
use field::Gf128 as Gf;

const PARALLEL_MIN_PAIRS: usize = 1 << 12;

/// A pending evaluation claim over the original committed bit polynomial.
pub(crate) struct MleClaim {
    /// Row coordinates before column coordinates, low-bit-first.
    pub(crate) point: Vec<Gf>,
    /// The witness MLE evaluation at `point`.
    pub(crate) target: Gf,
}

/// `Σ_b eq(b, point)·bit(c, b)` for every column `c`: the witness after its
/// row coordinates are bound to `point` (low-bit-first). The eq weight
/// factors over the coordinates, `eq(b, point) = eq(b mod 64, point[..6]) ·
/// eq(b div 64, point[6..])`, so a column's sum is `Σ_w eq_hi[w] · Σ_{j set
/// in word w} eq_lo[j]`: the inner sums come out of one 32 KB byte table
/// over `eq_lo` (eight lookups per word), the outer one is a multiply per
/// word. Exact: the same field products regrouped.
fn fold_rows_point(rows: &[Vec<u64>], point: &[Gf]) -> Vec<Gf> {
    const LOW: usize = 6;
    if point.len() < LOW {
        let eq = if point.is_empty() {
            vec![Gf::one()]
        } else {
            build_eq_x_r_vec(point, &()).expect("non-empty row point")
        };
        return fold_rows_eq(rows, &eq);
    }
    let eq_lo = build_eq_x_r_vec(&point[..LOW], &()).expect("six coordinates");
    let eq_hi = if point.len() == LOW {
        vec![Gf::one()]
    } else {
        build_eq_x_r_vec(&point[LOW..], &()).expect("high coordinates")
    };
    // table[pos][byte] = Σ_{bits b of byte} eq_lo[8·pos + b].
    let mut table = vec![[Gf::zero(); 256]; 8];
    for (pos, tb) in table.iter_mut().enumerate() {
        for byte in 1..256usize {
            tb[byte] = tb[byte & (byte - 1)] + eq_lo[(pos << 3) | byte.trailing_zeros() as usize];
        }
    }
    cfg_into_iter!(rows, 16)
        .map(|row| {
            debug_assert_eq!(row.len(), eq_hi.len());
            let zero = Gf::zero();
            let mut acc = <Gf as WideMulAcc>::wide_zero(&zero);
            for (word, &weight) in row.iter().zip(&eq_hi) {
                let mut inner = zero;
                for (pos, tb) in table.iter().enumerate() {
                    inner += tb[((word >> (8 * pos)) & 0xFF) as usize];
                }
                <Gf as WideMulAcc>::wide_add_assign(
                    &mut acc,
                    &<Gf as WideMulAcc>::mul_wide(&weight, &inner),
                );
            }
            <Gf as WideMulAcc>::from_wide(acc)
        })
        .collect()
}

/// One `Gf` per committed bit, bit `column * rows + row`, from the
/// per-column rows (64 bits per word, low bit first).
/// `Σ_b eq[b]·bit(c, b)` for every column `c`: the witness after its row
/// coordinates are bound to the point `eq` tabulates. A nibble table over
/// `eq` turns the per-bit adds into one table add per set nibble.
fn fold_rows_eq(rows: &[Vec<u64>], eq: &[Gf]) -> Vec<Gf> {
    let row_len = eq.len();
    // 2^t · 64 B of tables; past 2^22 rows fall back to per-bit adds.
    if row_len <= 1 << 22 {
        let groups = row_len / 4;
        let table: Vec<[Gf; 16]> = cfg_into_iter!(0..groups, 1 << 10)
            .map(|g| {
                let mut t = [Gf::zero(); 16];
                for nib in 1..16usize {
                    t[nib] = t[nib & (nib - 1)] + eq[(g << 2) | nib.trailing_zeros() as usize];
                }
                t
            })
            .collect();
        cfg_into_iter!(rows, 16)
            .map(|row| {
                let mut acc = Gf::zero();
                for (wi, &word) in row.iter().enumerate() {
                    let mut x = word;
                    let mut nib = 0usize;
                    while x != 0 {
                        let n = (x & 0xF) as usize;
                        if n != 0 {
                            acc += table[(wi << 4) | nib][n];
                        }
                        x >>= 4;
                        nib += 1;
                    }
                }
                acc
            })
            .collect()
    } else {
        cfg_into_iter!(rows, 16)
            .map(|row| {
                let mut acc = Gf::zero();
                for (wi, &word) in row.iter().enumerate() {
                    let mut x = word;
                    while x != 0 {
                        acc += eq[(wi << 6) | x.trailing_zeros() as usize];
                        x &= x - 1;
                    }
                }
                acc
            })
            .collect()
    }
}

/// Their `prove`, computed through the bilinear structure of the claim.
///
/// The weight of bit `(b, c)` is `row_weights[b]·column_weights[c]`, and
/// the rounds bind the row coordinates first. While a round binds a row
/// coordinate the column coordinates are only summed over, so its
/// polynomial is that of the `2^t`-entry table `M̃[b] = Σ_c w2[c]·bit(c, b)`
/// against `row_weights` alone; once the rows are bound the remaining
/// rounds run on the `2^s`-entry table `M'[c] = Σ_b eq(b, r_b)·bit(c, b)`
/// against `w1(r_b)·column_weights`. Every coefficient is the one the dense
/// pass over `2^m` bits produces — the two sums are the same field sum
/// regrouped — so the messages are byte-identical, without a 16-byte-per-bit
/// witness.
pub(crate) fn prove(
    claim: &LinearClaimGf,
    rows_bits: &[Vec<u64>],
    packed_cols: &[Vec<u64>],
    transcript: &mut ProverState,
) -> Result<MleClaim, ProveError> {
    let w1 = claim.row_weights();
    let w2 = claim.column_weights();
    if rows_bits.len() != w2.len() || rows_bits.iter().any(|row| row.len() * 64 != w1.len()) {
        return Err(ProveError::PackedWitnessLengthMismatch);
    }
    if w2.len() == 1 && packed_cols.is_empty() && w1.len() >= 1 << (BIT_ROUNDS + 1) {
        return prove_single_column_bits(claim, &rows_bits[0], transcript);
    }
    let layout = IntegerMatrixLayout {
        row_vars: w1.len().ilog2() as usize,
        col_vars: w2.len().ilog2() as usize,
    };
    let mut target = claim.target();
    let mut point = Vec::with_capacity(layout.row_vars + layout.col_vars);

    // Rows: the column-combined table against the row weights.
    let started = std::time::Instant::now();
    let mut combined = if packed_cols.is_empty() {
        xi_combined_rows(&layout, rows_bits, w2)
    } else {
        xi_combined_rows_packed(&layout, packed_cols, w2)
    };
    super::trace("    sc combine cols", started);
    let started = std::time::Instant::now();
    let mut rows = w1.to_vec();
    while combined.len() > 1 {
        let coefficients = round_polynomial(&combined, &rows);
        // In characteristic two, g(0) + g(1) = a1 + a2.
        if coefficients[1] + coefficients[2] != target {
            return Err(ProveError::InvalidClaim);
        }
        transcript.prover_message(&coefficients);
        let challenge = transcript.native_scalar(super::grinding::Stage::BinaryRound);
        target = evaluate_round(coefficients, challenge);
        point.push(challenge);
        fold(&mut combined, challenge);
        fold(&mut rows, challenge);
    }

    super::trace("    sc row rounds", started);

    // Columns: the row-folded table against `w1(r_b)·column_weights`.
    let started = std::time::Instant::now();
    let folded = fold_rows_point(rows_bits, &point);
    super::trace("    sc fold rows", started);
    let row_scalar = rows[0];
    let columns = w2.iter().map(|&w| row_scalar * w).collect();
    prove_columns(folded, columns, target, point, transcript)
}

/// Rounds the single-column prover runs on the witness bits' F2 codes
/// before switching to field tables.
const BIT_ROUNDS: usize = 3;

/// [`prove`] for a single-column claim (the virtual opening's transposed
/// weights over every committed bit): byte-identical messages, but the
/// witness is never a field table while it is still bits. After `k` folds a
/// witness entry is `c·Σ_{S⊆[k]} code[S]·Π_{i∈S} r_i` with an F2 code of
/// `2^k` bits, so the first rounds are XOR sums of weights selected by code
/// bits plus one fold of the weights; the field tables start `2^{BIT_ROUNDS}`
/// times smaller.
fn prove_single_column_bits(
    claim: &LinearClaimGf,
    bits: &[u64],
    transcript: &mut ProverState,
) -> Result<MleClaim, ProveError> {
    let c = claim.column_weights()[0];
    let w = claim.row_weights();
    let vars = w.len().ilog2() as usize;
    let mut target = claim.target();
    let mut point = Vec::with_capacity(vars);
    const CHUNK: usize = 1 << 13;
    let zero = Gf::zero();

    // Round 1: codes are the bits themselves.
    let pairs = w.len() / 2;
    let bit = |j: usize| (bits[j / 64] >> (j % 64)) & 1;
    let partials: Vec<[Gf; 3]> = cfg_into_iter!(0..pairs.div_ceil(CHUNK))
        .map(|chunk| {
            let (start, end) = (chunk * CHUNK, ((chunk + 1) * CHUNK).min(pairs));
            let mut acc = [zero; 3];
            for p in start..end {
                let (b0, b1) = (bit(2 * p), bit(2 * p + 1));
                let (w0, w1) = (w[2 * p], w[2 * p + 1]);
                if b0 == 1 {
                    acc[0] += w0;
                }
                if b1 == 1 {
                    acc[1] += w1;
                }
                if b0 != b1 {
                    acc[2] += w0 + w1;
                }
            }
            acc
        })
        .collect();
    let [s0, s1, sq] = partials.into_iter().fold([zero; 3], |[a, b, d], [x, y, z]| [a + x, b + y, d + z]);
    let coefficients = [c * s0, c * (s0 + s1 + sq), c * sq];
    if coefficients[1] + coefficients[2] != target {
        return Err(ProveError::InvalidClaim);
    }
    transcript.prover_message(&coefficients);
    let r = transcript.native_scalar(super::grinding::Stage::BinaryRound);
    target = evaluate_round(coefficients, r);
    point.push(r);
    let mut weights = super::pool::take(pairs);
    cfg_iter_mut!(weights).enumerate().for_each(|(p, dst)| *dst = w[2 * p] + r * (w[2 * p] + w[2 * p + 1]));
    let mut codes: Vec<u8> = cfg_into_iter!(0..pairs, PARALLEL_MIN_PAIRS)
        .map(|p| {
            let (b0, b1) = (bit(2 * p) as u8, bit(2 * p + 1) as u8);
            b0 | ((b0 ^ b1) << 1)
        })
        .collect();
    // products[S] = Π_{i∈S} r_i over the bound challenges.
    let mut products = vec![Gf::one(), r];

    // Rounds 2..=BIT_ROUNDS on the codes.
    for k in 1..BIT_ROUNDS {
        let width = 1usize << k;
        let pairs = weights.len() / 2;
        let partials: Vec<Vec<[Gf; 3]>> = cfg_into_iter!(0..pairs.div_ceil(CHUNK))
            .map(|chunk| {
                let (start, end) = (chunk * CHUNK, ((chunk + 1) * CHUNK).min(pairs));
                let mut acc = vec![[zero; 3]; width];
                for p in start..end {
                    let (c0, c1) = (codes[2 * p], codes[2 * p + 1]);
                    let (w0, w1) = (weights[2 * p], weights[2 * p + 1]);
                    let mut x = c0;
                    while x != 0 {
                        acc[x.trailing_zeros() as usize][0] += w0;
                        x &= x - 1;
                    }
                    let mut x = c1;
                    while x != 0 {
                        acc[x.trailing_zeros() as usize][1] += w1;
                        x &= x - 1;
                    }
                    let mut x = c0 ^ c1;
                    let sum = w0 + w1;
                    while x != 0 {
                        acc[x.trailing_zeros() as usize][2] += sum;
                        x &= x - 1;
                    }
                }
                acc
            })
            .collect();
        let mut totals = [zero; 3];
        for (subset, product) in products.iter().enumerate() {
            let mut sums = [zero; 3];
            for part in &partials {
                for t in 0..3 {
                    sums[t] += part[subset][t];
                }
            }
            for t in 0..3 {
                totals[t] += *product * sums[t];
            }
        }
        let [a0, a1, aq] = totals;
        let coefficients = [c * a0, c * (a0 + a1 + aq), c * aq];
        if coefficients[1] + coefficients[2] != target {
            return Err(ProveError::InvalidClaim);
        }
        transcript.prover_message(&coefficients);
        let r = transcript.native_scalar(super::grinding::Stage::BinaryRound);
        target = evaluate_round(coefficients, r);
        point.push(r);
        fold(&mut weights, r);
        codes = cfg_into_iter!(0..pairs, PARALLEL_MIN_PAIRS)
            .map(|p| {
                let (c0, c1) = (codes[2 * p], codes[2 * p + 1]);
                c0 | ((c0 ^ c1) << width)
            })
            .collect();
        let extended: Vec<Gf> = products.iter().map(|&x| x * r).collect();
        products.extend(extended);
    }

    // Field tables from here: entry = c·Σ_{S ∈ code} Π_{i∈S} r_i.
    let mut table = vec![zero; 1 << (1 << BIT_ROUNDS)];
    for code in 1..table.len() {
        let low = code & (code - 1);
        table[code] = table[low] + products[(code ^ low).trailing_zeros() as usize];
    }
    let mut combined = super::pool::take(codes.len());
    cfg_iter_mut!(combined).enumerate().for_each(|(j, dst)| *dst = c * table[codes[j] as usize]);
    drop(codes);
    while combined.len() > 1 {
        let coefficients = round_polynomial(&combined, &weights);
        if coefficients[1] + coefficients[2] != target {
            return Err(ProveError::InvalidClaim);
        }
        transcript.prover_message(&coefficients);
        let challenge = transcript.native_scalar(super::grinding::Stage::BinaryRound);
        target = evaluate_round(coefficients, challenge);
        point.push(challenge);
        fold(&mut combined, challenge);
        fold(&mut weights, challenge);
    }
    let folded = fold_rows_point(core::slice::from_ref(&bits.to_vec()), &point);
    let columns = vec![weights[0] * c];
    super::pool::recycle(weights);
    super::pool::recycle(combined);
    prove_columns(folded, columns, target, point, transcript)
}

/// [`prove`] for a single-column claim whose weights are
/// [`circuit::linear_map::binary::StructuredWeights`] over the committed grid
/// (`rows_bits[c]`: column `c`'s `2^t` bits). Byte-identical messages to the
/// dense prover over `w.dense()`; no `2^{t+s}` field table ever exists.
///
/// The low `b1` row rounds run on the parts' own tables: per row vector, its
/// row weights against the column-combined witness `A_k[r] = Σ_{c ∈ k}
/// weight[c]·f[c, r]`; the row box against the witness on its band; the
/// column box against the witness on its columns (the boxes' first round
/// straight from the bits, as [`prove_single_column_bits`] does). Then the
/// witness is folded once from its bits at the bound low rows (`2^{t−b1+s}`
/// entries), the weights are assembled on the same domain, and the remaining
/// rounds are the dense ones.
pub(crate) fn prove_structured(
    sw: circuit::linear_map::binary::StructuredWeights,
    rows_bits: &[Vec<u64>],
    target: Gf,
    transcript: &mut ProverState,
) -> Result<MleClaim, ProveError> {
    let circuit::linear_map::binary::StructuredWeights {
        t,
        s,
        rows,
        term,
        weight,
        term2,
        weight2,
        row_box_log: b1,
        row_boxes,
        col_box_start: c2,
        col_box_log: b2,
        col_box,
    } = sw;
    let mut starts: Vec<usize> = row_boxes.iter().map(|(start, _)| *start).collect();
    starts.sort_unstable();
    if rows_bits.len() != 1 << s
        || rows_bits.iter().any(|row| row.len() * 64 != 1 << t)
        || b1 > t
        || row_boxes
            .iter()
            .any(|(start, w)| start % (1 << b1) != 0 || start + (1 << b1) > 1 << t || w.len() != 1 << (b1 + s))
        || starts.windows(2).any(|pair| pair[0] == pair[1])
        || c2 % (1 << b2) != 0
        || c2 + (1 << b2) > 1 << s
        || term.len() != 1 << s
        || weight.len() != 1 << s
        || term2.len() != weight2.len()
        || !(term2.is_empty() || term2.len() == 1 << s)
        || rows.iter().any(|r| r.len() != 1 << t)
        || col_box.len() != 1 << (b2 + t)
    {
        return Err(ProveError::PackedWitnessLengthMismatch);
    }
    let zero = Gf::zero();
    let one = Gf::one();
    let bit = |c: usize, r: usize| (rows_bits[c][r / 64] >> (r % 64)) & 1 == 1;
    let mut target = target;
    let mut point = Vec::with_capacity(t + s);
    let send = |coefficients: [Gf; 3], target: &mut Gf, point: &mut Vec<Gf>, transcript: &mut ProverState| {
        if coefficients[1] + coefficients[2] != *target {
            return Err(ProveError::InvalidClaim);
        }
        transcript.prover_message(&coefficients);
        let challenge = transcript.native_scalar(super::grinding::Stage::BinaryRound);
        *target = evaluate_round(coefficients, challenge);
        point.push(challenge);
        Ok(challenge)
    };
    let add = |a: [Gf; 3], b: [Gf; 3]| [a[0] + b[0], a[1] + b[1], a[2] + b[2]];

    // Column-combined witness per row vector: each vector's columns (through
    // their first term, their second, or both), a vector read by many
    // columns (a rank-one term across the grid) in parallel column chunks.
    let started = std::time::Instant::now();
    let mut members: Vec<Vec<(usize, Gf)>> = vec![Vec::new(); rows.len()];
    for c in 0..1usize << s {
        let first = (term[c] != u32::MAX).then_some((term[c] as usize, weight[c]));
        let second = term2.get(c).filter(|&&k| k != u32::MAX).map(|&k| (k as usize, weight2[c]));
        match (first, second) {
            (Some((k1, w1)), Some((k2, w2))) if k1 == k2 => members[k1].push((c, w1 + w2)),
            _ => {
                for (k, w) in first.into_iter().chain(second) {
                    members[k].push((c, w));
                }
            }
        }
    }
    let add_columns = |a: &mut Vec<Gf>, columns: &[(usize, Gf)]| {
        for &(c, w) in columns {
            if w == zero {
                continue;
            }
            for (wi, &word) in rows_bits[c].iter().enumerate() {
                let mut x = word;
                while x != 0 {
                    a[64 * wi + x.trailing_zeros() as usize] += w;
                    x &= x - 1;
                }
            }
        }
    };
    let mut acc: Vec<Vec<Gf>> = cfg_into_iter!(0..rows.len())
        .map(|k| {
            let columns = &members[k];
            #[cfg(feature = "parallel")]
            if columns.len() > 64 {
                return columns
                    .par_chunks(32)
                    .fold(
                        || vec![zero; 1 << t],
                        |mut a, chunk| {
                            add_columns(&mut a, chunk);
                            a
                        },
                    )
                    .reduce(
                        || vec![zero; 1 << t],
                        |mut a, b| {
                            for (x, y) in a.iter_mut().zip(&b) {
                                *x += *y;
                            }
                            a
                        },
                    );
            }
            let mut a = vec![zero; 1 << t];
            add_columns(&mut a, columns);
            a
        })
        .collect();

    super::trace("    sc structured: row tables", started);
    let started = std::time::Instant::now();
    let mut rows = rows;
    // The row boxes (each its band's bits until the first fold) and the
    // column box, as (start, weights, witness).
    let (box_starts, mut box_w): (Vec<usize>, Vec<Vec<Gf>>) = row_boxes.into_iter().unzip();
    let mut box2_w = col_box;
    let box_bit = |j: usize, i: usize| bit(i >> b1, box_starts[j] + (i & ((1 << b1) - 1)));
    let box2_bit = |i: usize| bit(c2 + (i >> t), i & ((1 << t) - 1));

    // Low row rounds: every part on its own tables. The boxes' witness is
    // their bits until the first fold; each box fold also computes the next
    // round's polynomial in the same pass ([`fold_round`]).
    let mut box_f: Vec<Vec<Gf>> = box_starts.iter().map(|_| Vec::new()).collect();
    let mut box2_f: Vec<Gf> = Vec::new();
    let mut box_coefficients = if b1 > 0 {
        let mut c = bit_round(&box2_bit, &box2_w);
        for (j, w) in box_w.iter().enumerate() {
            c = add(c, bit_round(&|i: usize| box_bit(j, i), w));
        }
        c
    } else {
        [zero; 3]
    };
    for round in 0..b1 {
        let mut coefficients = box_coefficients;
        for (a, r) in acc.iter().zip(&rows) {
            coefficients = add(coefficients, round_polynomial(a, r));
        }
        let challenge = send(coefficients, &mut target, &mut point, transcript)?;
        let (next2, mut next_coefficients) = if round == 0 {
            let bit2 = |i: usize| if box2_bit(i) { one } else { zero };
            fold_round(bit2, &mut box2_w, challenge)
        } else {
            let src2 = &box2_f;
            fold_round(|i| src2[i], &mut box2_w, challenge)
        };
        super::pool::recycle(std::mem::replace(&mut box2_f, next2));
        for (j, (w, f)) in box_w.iter_mut().zip(box_f.iter_mut()).enumerate() {
            let (next, coefficients) = if round == 0 {
                fold_round(|i| if box_bit(j, i) { one } else { zero }, w, challenge)
            } else {
                let src = &*f;
                fold_round(|i| src[i], w, challenge)
            };
            super::pool::recycle(std::mem::replace(f, next));
            next_coefficients = add(next_coefficients, coefficients);
        }
        box_coefficients = next_coefficients;
        for (a, r) in acc.iter_mut().zip(rows.iter_mut()) {
            fold(a, challenge);
            fold(r, challenge);
        }
    }
    drop(acc);
    for f in box_f {
        super::pool::recycle(f);
    }
    super::pool::recycle(box2_f);
    super::trace("    sc structured: low rounds", started);
    let started = std::time::Instant::now();

    // The witness at the bound low rows, from its bits: byte tables over
    // `eq(point, ·)` on the low `b1` row bits.
    let high = t - b1;
    let eq = if b1 == 0 { vec![one] } else { build_eq_x_r_vec(&point, &()).expect("bound low rows") };
    let segment_words = (1usize << b1).div_ceil(64);
    let byte_tables: Vec<[Gf; 256]> = (0..(1usize << b1).div_ceil(8))
        .map(|q| {
            let mut table = [zero; 256];
            for byte in 1..256usize {
                let low = byte & (byte - 1);
                let r = 8 * q + (byte ^ low).trailing_zeros() as usize;
                table[byte] = table[low] + if r < eq.len() { eq[r] } else { zero };
            }
            table
        })
        .collect();
    let mut witness: Vec<Gf> = cfg_into_iter!(0..1usize << (high + s), 64)
        .map(|i| {
            let (c, h) = (i >> high, i & ((1 << high) - 1));
            let base = h << b1;
            if b1 < 6 {
                // Fewer than 64 low rows: bit by bit.
                (0..1usize << b1).fold(zero, |a, r| if bit(c, base + r) { a + eq[r] } else { a })
            } else {
                let words = &rows_bits[c][base / 64..base / 64 + segment_words];
                let mut a = zero;
                for (wi, &word) in words.iter().enumerate() {
                    for q in 0..8 {
                        a += byte_tables[8 * wi + q][((word >> (8 * q)) & 0xFF) as usize];
                    }
                }
                a
            }
        })
        .collect();
    // Each row box has collapsed to its band's high row.
    let box_high: Vec<usize> = box_starts.iter().map(|start| start >> b1).collect();
    let mut weights: Vec<Gf> = cfg_into_iter!(0..1usize << (high + s), 64)
        .map(|i| {
            let (c, h) = (i >> high, i & ((1 << high) - 1));
            let mut w = zero;
            if term[c] != u32::MAX {
                w += rows[term[c] as usize][h] * weight[c];
            }
            if let Some(&k) = term2.get(c)
                && k != u32::MAX
            {
                w += rows[k as usize][h] * weight2[c];
            }
            for (j, &h_j) in box_high.iter().enumerate() {
                if h == h_j {
                    w += box_w[j][c];
                }
            }
            if c >= c2 && c < c2 + (1 << b2) {
                w += box2_w[((c - c2) << high) | h];
            }
            w
        })
        .collect();
    drop(rows);
    for w in box_w {
        super::pool::recycle(w);
    }
    super::pool::recycle(box2_w);
    super::trace("    sc structured: fold to dense", started);

    // The remaining rounds, dense on `2^{t−b1+s}` entries.
    while witness.len() > 1 {
        let coefficients = round_polynomial(&witness, &weights);
        let challenge = send(coefficients, &mut target, &mut point, transcript)?;
        fold(&mut witness, challenge);
        fold(&mut weights, challenge);
    }
    let evaluation = witness[0];
    if target != evaluation * weights[0] {
        return Err(ProveError::InvalidClaim);
    }
    transcript.prover_message(&evaluation);
    Ok(MleClaim { point, target: evaluation })
}

/// [`fold`] of a witness (`witness(i)`, entries `0..weights.len()`) and of
/// `weights` at `challenge`, then the [`round_polynomial`] of the folded pair,
/// in one pass: the folded witness, the weights folded in place, and the
/// next round's coefficients (the same field sums, regrouped).
fn fold_round<S: Fn(usize) -> Gf + Sync>(witness: S, weights: &mut Vec<Gf>, challenge: Gf) -> (Vec<Gf>, [Gf; 3]) {
    let len = weights.len() / 2;
    let zero = Gf::zero();
    let mut folded = super::pool::take(len);
    let mut w_out = super::pool::take(len);
    let w_in: &[Gf] = weights;
    let fold1 = |a: Gf, b: Gf| a + challenge * (a + b);
    if len < 2 {
        for k in 0..len {
            folded[k] = fold1(witness(2 * k), witness(2 * k + 1));
            w_out[k] = fold1(w_in[2 * k], w_in[2 * k + 1]);
        }
        super::pool::recycle(std::mem::replace(weights, w_out));
        return (folded, [zero; 3]);
    }
    const PAIRS: usize = PARALLEL_MIN_PAIRS;
    let partials: Vec<[Gf; 3]> = cfg_chunks_mut!(folded, 2 * PAIRS)
        .zip(cfg_chunks_mut!(w_out, 2 * PAIRS))
        .enumerate()
        .map(|(chunk, (fo, wo))| {
            let mut at_zero = <Gf as WideMulAcc>::wide_zero(&zero);
            let mut at_one = <Gf as WideMulAcc>::wide_zero(&zero);
            let mut quadratic = <Gf as WideMulAcc>::wide_zero(&zero);
            for q in 0..fo.len() / 2 {
                let i = 4 * (chunk * PAIRS + q);
                let (f0, f1) = (fold1(witness(i), witness(i + 1)), fold1(witness(i + 2), witness(i + 3)));
                let (w0, w1) = (fold1(w_in[i], w_in[i + 1]), fold1(w_in[i + 2], w_in[i + 3]));
                fo[2 * q] = f0;
                fo[2 * q + 1] = f1;
                wo[2 * q] = w0;
                wo[2 * q + 1] = w1;
                <Gf as WideMulAcc>::wide_add_assign(&mut at_zero, &<Gf as WideMulAcc>::mul_wide(&f0, &w0));
                <Gf as WideMulAcc>::wide_add_assign(&mut at_one, &<Gf as WideMulAcc>::mul_wide(&f1, &w1));
                <Gf as WideMulAcc>::wide_add_assign(&mut quadratic, &<Gf as WideMulAcc>::mul_wide(&(f0 + f1), &(w0 + w1)));
            }
            [
                <Gf as WideMulAcc>::from_wide(at_zero),
                <Gf as WideMulAcc>::from_wide(at_one),
                <Gf as WideMulAcc>::from_wide(quadratic),
            ]
        })
        .collect();
    super::pool::recycle(std::mem::replace(weights, w_out));
    let [a0, a1, a2] = partials.into_iter().fold([zero; 3], |[a, b, c], [x, y, z]| [a + x, b + y, c + z]);
    (folded, [a0, a0 + a1 + a2, a2])
}

/// [`round_polynomial`] for a witness of bits (`bits(j)`): the products are
/// selections of the weights.
fn bit_round(bits: &(dyn Fn(usize) -> bool + Sync), weights: &[Gf]) -> [Gf; 3] {
    const CHUNK: usize = 1 << 13;
    let pairs = weights.len() / 2;
    let zero = Gf::zero();
    let partials: Vec<[Gf; 3]> = cfg_into_iter!(0..pairs.div_ceil(CHUNK))
        .map(|chunk| {
            let mut acc = [zero; 3];
            for p in chunk * CHUNK..((chunk + 1) * CHUNK).min(pairs) {
                let (b0, b1) = (bits(2 * p), bits(2 * p + 1));
                let (w0, w1) = (weights[2 * p], weights[2 * p + 1]);
                if b0 {
                    acc[0] += w0;
                }
                if b1 {
                    acc[1] += w1;
                }
                if b0 != b1 {
                    acc[2] += w0 + w1;
                }
            }
            acc
        })
        .collect();
    let [s0, s1, sq] = partials.into_iter().fold([zero; 3], |[a, b, d], [x, y, z]| [a + x, b + y, d + z]);
    [s0, s0 + s1 + sq, sq]
}

pub(crate) fn verify(
    claim: &LinearClaimGf,
    transcript: &mut VerifierState<'_>,
) -> Result<MleClaim, VerifyError> {
    let mut rows = claim.row_weights().to_vec();
    let mut columns = claim.column_weights().to_vec();
    let rounds = rows.len().ilog2() as usize + columns.len().ilog2() as usize;
    let mut point = Vec::with_capacity(rounds);
    let mut target = claim.target();
    for _ in 0..rounds {
        let coefficients = transcript
            .prover_message::<[Gf; 3]>()
            .map_err(|_| VerifyError::MalformedProof)?;
        if coefficients[1] + coefficients[2] != target {
            return Err(VerifyError::VerificationFailed);
        }
        let challenge = transcript
            .native_scalar(super::grinding::Stage::BinaryRound)
            .map_err(|_| VerifyError::MalformedProof)?;
        target = evaluate_round(coefficients, challenge);
        point.push(challenge);
        if rows.len() > 1 {
            fold(&mut rows, challenge);
        } else {
            fold(&mut columns, challenge);
        }
    }
    let evaluation = transcript
        .prover_message::<Gf>()
        .map_err(|_| VerifyError::MalformedProof)?;
    if target != evaluation * rows[0] * columns[0] {
        return Err(VerifyError::VerificationFailed);
    }
    Ok(MleClaim {
        point,
        target: evaluation,
    })
}

/// [`verify`] for a single-column claim with lazily evaluated weights.
pub(crate) fn verify_lazy(
    rounds: usize,
    target: Gf,
    weight_mle: impl FnOnce(&[Gf]) -> Gf,
    transcript: &mut VerifierState<'_>,
) -> Result<MleClaim, VerifyError> {
    let mut point = Vec::with_capacity(rounds);
    let mut target = target;
    for _ in 0..rounds {
        let coefficients = transcript
            .prover_message::<[Gf; 3]>()
            .map_err(|_| VerifyError::MalformedProof)?;
        if coefficients[1] + coefficients[2] != target {
            return Err(VerifyError::VerificationFailed);
        }
        let challenge = transcript
            .native_scalar(super::grinding::Stage::BinaryRound)
            .map_err(|_| VerifyError::MalformedProof)?;
        target = evaluate_round(coefficients, challenge);
        point.push(challenge);
    }
    let evaluation = transcript
        .prover_message::<Gf>()
        .map_err(|_| VerifyError::MalformedProof)?;
    if target != evaluation * weight_mle(&point) {
        return Err(VerifyError::VerificationFailed);
    }
    Ok(MleClaim { point, target: evaluation })
}

/// [`prove`] for a sum of factored claims: the row rounds run one
/// column-combined table per term against that term's row weights and send
/// the summed round polynomials; once the rows are bound every term is the
/// scalar `w1_k(r_b)`, so the column rounds run the one row-folded table
/// against `Σ_k w1_k(r_b)·cols_k`, exactly the single-term column rounds.
pub(crate) fn prove_sum(
    claim: &SumClaimGf,
    rows_bits: &[Vec<u64>],
    packed_cols: &[Vec<u64>],
    transcript: &mut ProverState,
) -> Result<MleClaim, ProveError> {
    let terms = claim.terms();
    let (n_rows, n_cols) = (claim.rows(), claim.columns());
    if rows_bits.len() != n_cols || rows_bits.iter().any(|row| row.len() * 64 != n_rows) {
        return Err(ProveError::PackedWitnessLengthMismatch);
    }
    let layout = IntegerMatrixLayout {
        row_vars: n_rows.ilog2() as usize,
        col_vars: n_cols.ilog2() as usize,
    };
    let mut target = claim.target();
    let mut point = Vec::with_capacity(layout.row_vars + layout.col_vars);

    let started = std::time::Instant::now();
    let mut combined: Vec<Vec<Gf>> = terms
        .iter()
        .map(|(_, w2)| {
            if packed_cols.is_empty() {
                xi_combined_rows(&layout, rows_bits, w2)
            } else {
                xi_combined_rows_packed(&layout, packed_cols, w2)
            }
        })
        .collect();
    super::trace("    sc combine cols (sum)", started);
    let started = std::time::Instant::now();
    let mut rows: Vec<Vec<Gf>> = terms.iter().map(|(w1, _)| w1.clone()).collect();
    while combined[0].len() > 1 {
        let mut coefficients = [Gf::zero(); 3];
        for (table, weights) in combined.iter().zip(&rows) {
            let part = round_polynomial(table, weights);
            for (sum, value) in coefficients.iter_mut().zip(part) {
                *sum += value;
            }
        }
        if coefficients[1] + coefficients[2] != target {
            return Err(ProveError::InvalidClaim);
        }
        transcript.prover_message(&coefficients);
        let challenge = transcript.native_scalar(super::grinding::Stage::BinaryRound);
        target = evaluate_round(coefficients, challenge);
        point.push(challenge);
        for table in &mut combined {
            fold(table, challenge);
        }
        for weights in &mut rows {
            fold(weights, challenge);
        }
    }
    super::trace("    sc row rounds (sum)", started);

    let started = std::time::Instant::now();
    let folded = fold_rows_point(rows_bits, &point);
    super::trace("    sc fold rows", started);
    let columns = merged_columns(terms, &rows);
    prove_columns(folded, columns, target, point, transcript)
}

/// `Σ_k w1_k(r_b)·cols_k`: the column weights once every term's row weights
/// are folded to the scalar `w1_k(r_b)`.
fn merged_columns(terms: &[(Vec<Gf>, Vec<Gf>)], bound_rows: &[Vec<Gf>]) -> Vec<Gf> {
    let mut columns = vec![Gf::zero(); terms[0].1.len()];
    for ((_, w2), bound) in terms.iter().zip(bound_rows) {
        let scalar = bound[0];
        for (sum, &weight) in columns.iter_mut().zip(w2) {
            *sum += scalar * weight;
        }
    }
    columns
}

pub(crate) fn verify_sum(
    claim: &SumClaimGf,
    transcript: &mut VerifierState<'_>,
) -> Result<MleClaim, VerifyError> {
    let terms = claim.terms();
    let row_rounds = claim.rows().ilog2() as usize;
    let col_rounds = claim.columns().ilog2() as usize;
    let mut rows: Vec<Vec<Gf>> = terms.iter().map(|(w1, _)| w1.clone()).collect();
    let mut columns = if row_rounds == 0 {
        merged_columns(terms, &rows)
    } else {
        Vec::new()
    };
    let mut point = Vec::with_capacity(row_rounds + col_rounds);
    let mut target = claim.target();
    for round in 0..row_rounds + col_rounds {
        let coefficients = transcript
            .prover_message::<[Gf; 3]>()
            .map_err(|_| VerifyError::MalformedProof)?;
        if coefficients[1] + coefficients[2] != target {
            return Err(VerifyError::VerificationFailed);
        }
        let challenge = transcript
            .native_scalar(super::grinding::Stage::BinaryRound)
            .map_err(|_| VerifyError::MalformedProof)?;
        target = evaluate_round(coefficients, challenge);
        point.push(challenge);
        if round < row_rounds {
            for weights in &mut rows {
                fold(weights, challenge);
            }
            if round + 1 == row_rounds {
                columns = merged_columns(terms, &rows);
            }
        } else {
            fold(&mut columns, challenge);
        }
    }
    let evaluation = transcript
        .prover_message::<Gf>()
        .map_err(|_| VerifyError::MalformedProof)?;
    if target != evaluation * columns[0] {
        return Err(VerifyError::VerificationFailed);
    }
    Ok(MleClaim {
        point,
        target: evaluation,
    })
}

/// Completes the sumcheck after the row coordinates are bound.
fn prove_columns(
    mut folded: Vec<Gf>,
    mut columns: Vec<Gf>,
    mut target: Gf,
    mut point: Vec<Gf>,
    transcript: &mut ProverState,
) -> Result<MleClaim, ProveError> {
    while folded.len() > 1 {
        let coefficients = round_polynomial(&folded, &columns);
        if coefficients[1] + coefficients[2] != target {
            return Err(ProveError::InvalidClaim);
        }
        transcript.prover_message(&coefficients);
        let challenge = transcript.native_scalar(super::grinding::Stage::BinaryRound);
        target = evaluate_round(coefficients, challenge);
        point.push(challenge);
        fold(&mut folded, challenge);
        fold(&mut columns, challenge);
    }

    let evaluation = folded[0];
    if target != evaluation * columns[0] {
        return Err(ProveError::InvalidClaim);
    }
    transcript.prover_message(&evaluation);
    Ok(MleClaim {
        point,
        target: evaluation,
    })
}

/// The coefficients `[a0, a1, a2]` of the round polynomial over pairs
/// `(2k, 2k+1)` of `witness` against the per-element `weights`, computed
/// their way (`a1 = Σ(w0·M0 + w1·M1) + a0 + a2` in characteristic two).
fn round_polynomial(witness: &[Gf], weights: &[Gf]) -> [Gf; 3] {
    let pairs = witness.len() / 2;
    let chunks = pairs.div_ceil(PARALLEL_MIN_PAIRS).max(1);
    let partials: Vec<[Gf; 3]> = cfg_into_iter!(0..chunks)
        .map(|chunk| {
            let start = chunk * PARALLEL_MIN_PAIRS;
            let end = (start + PARALLEL_MIN_PAIRS).min(pairs);
            let zero = Gf::zero();
            let mut at_zero = <Gf as WideMulAcc>::wide_zero(&zero);
            let mut at_one = <Gf as WideMulAcc>::wide_zero(&zero);
            let mut quadratic = <Gf as WideMulAcc>::wide_zero(&zero);
            for pair_index in start..end {
                let index = 2 * pair_index;
                let (m0, m1) = (witness[index], witness[index + 1]);
                let (w0, w1) = (weights[index], weights[index + 1]);
                <Gf as WideMulAcc>::wide_add_assign(
                    &mut at_zero,
                    &<Gf as WideMulAcc>::mul_wide(&m0, &w0),
                );
                <Gf as WideMulAcc>::wide_add_assign(
                    &mut at_one,
                    &<Gf as WideMulAcc>::mul_wide(&m1, &w1),
                );
                <Gf as WideMulAcc>::wide_add_assign(
                    &mut quadratic,
                    &<Gf as WideMulAcc>::mul_wide(&(m0 + m1), &(w0 + w1)),
                );
            }
            [
                <Gf as WideMulAcc>::from_wide(at_zero),
                <Gf as WideMulAcc>::from_wide(at_one),
                <Gf as WideMulAcc>::from_wide(quadratic),
            ]
        })
        .collect();
    let [at_zero, at_one, quadratic] = partials
        .into_iter()
        .fold([Gf::zero(); 3], |[a, b, c], [x, y, z]| {
            [a + x, b + y, c + z]
        });
    [at_zero, at_zero + at_one + quadratic, quadratic]
}

fn evaluate_round([constant, linear, quadratic]: [Gf; 3], challenge: Gf) -> Gf {
    constant + challenge * (linear + challenge * quadratic)
}

/// Fixes the lowest remaining coordinate: `v'[i] = v[2i] + r·(v[2i] + v[2i+1])`,
/// in place (the first half of the buffer holds the result).
fn fold(values: &mut Vec<Gf>, challenge: Gf) {
    let len = values.len() / 2;
    if len >= PARALLEL_MIN_PAIRS * 4 {
        // Chunks of pairs fold into a scratch half, then copy back: the pair
        // (2i, 2i+1) of a later index is still unread when index i is written
        // only when done serially, so the parallel path goes through scratch.
        let mut folded = super::pool::take(len);
        let source: &[Gf] = values;
        cfg_iter_mut!(folded, PARALLEL_MIN_PAIRS).enumerate().for_each(|(index, dst)| {
            let zero = source[2 * index];
            let one = source[2 * index + 1];
            *dst = zero + challenge * (zero + one);
        });
        values[..len].copy_from_slice(&folded);
        super::pool::recycle(folded);
    } else {
        for index in 0..len {
            let zero = values[2 * index];
            let one = values[2 * index + 1];
            values[index] = zero + challenge * (zero + one);
        }
    }
    values.truncate(len);
}

#[cfg(test)]
mod sum_tests {
    use super::*;
    use crate::bitz::params::{LinearClaimGf, Shape, SumClaimGf};
    use crate::bitz::{build_kernel_prover, build_kernel_verifier};

    fn xorshift(state: &mut u64) -> u64 {
        *state ^= *state << 13;
        *state ^= *state >> 7;
        *state ^= *state << 17;
        *state
    }

    fn random_gf(state: &mut u64) -> Gf {
        Gf::new(xorshift(state), xorshift(state))
    }

    /// `Σ_k Σ_{b,c} rows_k[b]·cols_k[c]·bit(b, c)` over the bits.
    fn dense_target(rows_bits: &[Vec<u64>], terms: &[(Vec<Gf>, Vec<Gf>)]) -> Gf {
        let mut total = Gf::zero();
        for (c, column) in rows_bits.iter().enumerate() {
            for (word_index, &word) in column.iter().enumerate() {
                let mut bits = word;
                while bits != 0 {
                    let b = word_index * 64 + bits.trailing_zeros() as usize;
                    bits &= bits - 1;
                    for (rows, cols) in terms {
                        total += rows[b] * cols[c];
                    }
                }
            }
        }
        total
    }

    fn grid(state: &mut u64, shape: &Shape) -> Vec<Vec<u64>> {
        (0..shape.columns())
            .map(|_| (0..shape.rows() / 64).map(|_| xorshift(state)).collect())
            .collect()
    }

    fn terms(state: &mut u64, shape: &Shape, count: usize) -> Vec<(Vec<Gf>, Vec<Gf>)> {
        (0..count)
            .map(|_| {
                (
                    (0..shape.rows()).map(|_| random_gf(state)).collect(),
                    (0..shape.columns()).map(|_| random_gf(state)).collect(),
                )
            })
            .collect()
    }

    #[test]
    fn a_sum_of_tensors_reduces_to_the_witness_evaluation() {
        let shape = Shape::new(8, 5).unwrap();
        let mut state = 0x9e37_79b9_7f4a_7c15u64;
        let rows_bits = grid(&mut state, &shape);
        let terms = terms(&mut state, &shape, 3);
        let target = dense_target(&rows_bits, &terms);
        let claim = SumClaimGf::from_shape(&shape, terms, target, b"sum-test".to_vec()).unwrap();
        let mut prover = build_kernel_prover(b"sum-test/v1", b"instance");
        let reduced = prove_sum(&claim, &rows_bits, &[], &mut prover).unwrap();
        let proof = prover.finish();
        let mut verifier = build_kernel_verifier(b"sum-test/v1", b"instance", &proof);
        let checked = verify_sum(&claim, &mut verifier).unwrap();
        assert_eq!(reduced.point, checked.point);
        assert_eq!(reduced.target, checked.target);
        // The reduced value is the witness MLE at the point: the dense sum of
        // eq weights over the set bits.
        let eq = crate::poly::utils::build_eq_x_r_vec(&reduced.point, &()).unwrap();
        let mut evaluation = Gf::zero();
        for (c, column) in rows_bits.iter().enumerate() {
            for (word_index, &word) in column.iter().enumerate() {
                let mut bits = word;
                while bits != 0 {
                    let b = word_index * 64 + bits.trailing_zeros() as usize;
                    bits &= bits - 1;
                    evaluation += eq[c * shape.rows() + b];
                }
            }
        }
        assert_eq!(reduced.target, evaluation);
        // A wrong target is caught by the prover's own consistency check.
        let wrong = SumClaimGf::from_shape(
            &shape,
            claim.terms().to_vec(),
            target + Gf::one(),
            Vec::new(),
        )
        .unwrap();
        let mut prover = build_kernel_prover(b"sum-test/v1", b"instance");
        assert!(prove_sum(&wrong, &rows_bits, &[], &mut prover).is_err());
    }

    /// The structured prover sends the dense single-column prover's bytes.
    #[test]
    fn structured_weights_match_the_dense_messages() {
        use circuit::linear_map::binary::StructuredWeights;
        let cases = [(7usize, 4usize, 3usize, 40usize, 1usize, 14usize), (7, 3, 6, 64, 0, 5), (8, 4, 7, 128, 2, 12)];
        let variants = [(false, false), (true, false), (false, true), (true, true)];
        for ((t, s, b1, r1, b2, c2), (second, several)) in cases.iter().flat_map(|&c| variants.map(|v| (c, v))) {
            let mut state = 0x1234_5678_9abc_def1u64 ^ (t as u64) << 32;
            let shape = Shape::new(t, s).unwrap();
            let rows_bits = grid(&mut state, &shape);
            let terms = 3usize;
            let (term2, weight2) = if second {
                (
                    (0..1u32 << s).map(|c| if c % 3 == 1 { u32::MAX } else { (c * 7 + 1) % terms as u32 }).collect(),
                    (0..1usize << s).map(|_| random_gf(&mut state)).collect(),
                )
            } else {
                (Vec::new(), Vec::new())
            };
            let sw = StructuredWeights {
                t,
                s,
                rows: (0..terms).map(|_| (0..1usize << t).map(|_| random_gf(&mut state)).collect()).collect(),
                term: (0..1u32 << s).map(|c| if c % 5 == 4 { u32::MAX } else { c % terms as u32 }).collect(),
                weight: (0..1usize << s).map(|_| random_gf(&mut state)).collect(),
                term2,
                weight2,
                row_box_log: b1,
                // One band at `r1`, or up to three disjoint aligned bands.
                row_boxes: {
                    let starts: Vec<usize> = if several {
                        (0..1usize << (t - b1)).map(|k| k << b1).filter(|&start| start != r1).take(2).chain([r1]).collect()
                    } else {
                        vec![r1]
                    };
                    starts.into_iter().map(|start| (start, (0..1usize << (b1 + s)).map(|_| random_gf(&mut state)).collect())).collect()
                },
                col_box_start: c2,
                col_box_log: b2,
                col_box: (0..1usize << (b2 + t)).map(|_| random_gf(&mut state)).collect(),
            };
            let dense = sw.dense();
            let flat: Vec<u64> = rows_bits.iter().flat_map(|r| r.iter().copied()).collect();
            let mut target = Gf::zero();
            for (j, w) in dense.iter().enumerate() {
                if (flat[j / 64] >> (j % 64)) & 1 == 1 {
                    target += *w;
                }
            }
            let single_shape = Shape::new(t + s, 0).unwrap();
            let claim = LinearClaimGf::from_shape(&single_shape, dense, vec![Gf::one()], target).unwrap();
            let mut a = build_kernel_prover(b"structured-test/v1", b"one");
            let dense_out = prove(&claim, core::slice::from_ref(&flat), &[], &mut a).unwrap();
            let mut b = build_kernel_prover(b"structured-test/v1", b"one");
            let structured_out = prove_structured(sw, &rows_bits, target, &mut b).unwrap();
            assert_eq!(a.finish().narg_string, b.finish().narg_string, "t={t} s={s}");
            assert_eq!(dense_out.point, structured_out.point);
            assert_eq!(dense_out.target, structured_out.target);
        }
    }

    #[test]
    fn one_term_matches_the_single_claim_messages() {
        for (row_vars, col_vars) in [(7, 0), (7, 6), (8, 7)] {
            let shape = Shape::new(row_vars, col_vars).unwrap();
            let mut state = 0x2545_f491_4f6c_dd1du64;
            let rows_bits = grid(&mut state, &shape);
            let mut terms = terms(&mut state, &shape, 1);
            let (rows, cols) = terms.pop().unwrap();
            let target = dense_target(&rows_bits, &[(rows.clone(), cols.clone())]);
            let single = LinearClaimGf::from_shape(&shape, rows.clone(), cols.clone(), target).unwrap();
            let sum = SumClaimGf::from_shape(&shape, vec![(rows, cols)], target, Vec::new()).unwrap();
            let mut a = build_kernel_prover(b"sum-test/v1", b"one");
            let reduced_single = prove(&single, &rows_bits, &[], &mut a).unwrap();
            let mut b = build_kernel_prover(b"sum-test/v1", b"one");
            let reduced_sum = prove_sum(&sum, &rows_bits, &[], &mut b).unwrap();
            assert_eq!(a.finish().narg_string, b.finish().narg_string);
            assert_eq!(reduced_single.point, reduced_sum.point);
            assert_eq!(reduced_single.target, reduced_sum.target);
        }
    }
}

//! Declared-width, borrowed u128/u256 assignment segments and mixed first rounds.
use super::*;
use crate::piop::spartan::raw_monty::RawFieldStorage;
use field::{CtOrd, CtSelect, Fp, FpLinearAcc, MergeAccumulator, RingOps, Uint};

#[derive(Clone, Copy)]
pub struct NativeU128Witness<'a> {
    block_len: usize,
    x: &'a [u128],
    y: &'a [u128],
    z_lo: &'a [u128],
    z_hi: &'a [u128],
}
impl<'a> NativeU128Witness<'a> {
    pub(crate) fn new(
        block_len: usize,
        x: &'a [u128],
        y: &'a [u128],
        z_lo: &'a [u128],
        z_hi: &'a [u128],
    ) -> Self {
        assert!(block_len.is_power_of_two());
        assert!(block_len.checked_mul(4).is_some());
        assert!(x.len() <= block_len);
        assert_eq!(x.len(), y.len());
        assert_eq!(x.len(), z_lo.len());
        assert_eq!(x.len(), z_hi.len());
        Self {
            block_len,
            x,
            y,
            z_lo,
            z_hi,
        }
    }
    pub(super) fn len(self) -> usize {
        4 * self.block_len
    }
    pub(super) fn block(self, index: usize) -> BlockValues<'a> {
        match index {
            0 => BlockValues::ConstantOne,
            1 => BlockValues::U128(self.x),
            2 => BlockValues::U128(self.y),
            3 => BlockValues::U256(self.z_lo, self.z_hi),
            _ => panic!("assignment block out of range"),
        }
    }
    // Dense fallback handles boundary-crossing pairs when block_len == 1.
    // Only scalar temporaries widen here; the borrowed segments retain widths.
    pub(super) fn read(self, index: usize) -> Uint<4> {
        assert!(index < self.len());
        let row = index % self.block_len;
        match self.block(index / self.block_len) {
            BlockValues::ConstantOne => Uint::from_u64((row == 0) as u64),
            BlockValues::U128(values) => read_u128(values, row).zero_extend(),
            BlockValues::U256(lo, hi) => read_u256(lo, hi, row),
            _ => unreachable!(),
        }
    }
}
/// A four-block assignment: implicit one, borrowed witness, borrowed quotient,
/// implicit zero. The source contract declares 32 limbs, independently of values.
#[derive(Clone, Copy)]
pub struct NativeLimbWitness<'a> {
    block_len: usize,
    witness: &'a [Uint<32>],
    quotient: &'a [Uint<32>],
}
impl<'a> NativeLimbWitness<'a> {
    pub(crate) fn new(block_len: usize, witness: &'a [Uint<32>], quotient: &'a [Uint<32>]) -> Self {
        assert!(block_len.is_power_of_two());
        assert!(block_len.checked_mul(4).is_some());
        assert!(witness.len() <= block_len && quotient.len() <= block_len);
        Self {
            block_len,
            witness,
            quotient,
        }
    }
    pub(super) fn len(self) -> usize {
        4 * self.block_len
    }
    pub(super) fn block(self, index: usize) -> BlockValues<'a> {
        match index {
            0 => BlockValues::ConstantOne,
            1 => BlockValues::Limbs(self.witness),
            2 => BlockValues::Limbs(self.quotient),
            3 => BlockValues::Zero,
            _ => panic!("assignment block out of range"),
        }
    }
    pub(crate) fn read(self, index: usize) -> Uint<32> {
        assert!(index < self.len());
        let row = index % self.block_len;
        match self.block(index / self.block_len) {
            BlockValues::ConstantOne => Uint::from_u64((row == 0) as u64),
            BlockValues::Limbs(values) => read_limbs(values, row),
            BlockValues::Zero => Uint::ZERO,
            _ => unreachable!(),
        }
    }
}
#[inline]
fn read_limbs(values: &[Uint<32>], index: usize) -> Uint<32> {
    values.get(index).copied().unwrap_or(Uint::ZERO)
}

#[inline]
fn read_u128(values: &[u128], i: usize) -> Uint<2> {
    Uint::from_words(raw_to_words(values.get(i).copied().unwrap_or(0)))
}
#[inline]
fn read_u256(lo: &[u128], hi: &[u128], i: usize) -> Uint<4> {
    let lo = lo.get(i).copied().unwrap_or(0);
    let hi = hi.get(i).copied().unwrap_or(0);
    Uint::from_words([lo as u64, (lo >> 64) as u64, hi as u64, (hi >> 64) as u64])
}

pub(super) fn wide_coefficients<const N: usize>(
    ctx: &field::FpCtx<2>,
    weights: &[Raw],
    read: impl Fn(usize) -> Uint<N> + Sync,
) -> [Raw; 2] {
    assert_eq!(weights.len() % 2, 0);
    let f = ctx;
    let zero = || [FpLinearAcc::<2, N>::zero(); 2];
    let merge = |mut a: [FpLinearAcc<2, N>; 2], b: [FpLinearAcc<2, N>; 2]| {
        for i in 0..2 {
            a[i].merge_assign(&b[i]);
        }
        a
    };
    let block = |start: usize, weights: &[Raw]| {
        let mut acc = zero();
        for (i, m) in weights.chunks_exact(2).enumerate() {
            let a = read(start + 2 * i);
            let b = read(start + 2 * i + 1);
            acc[0].accumulate(&shared_raw(f, m[0]), &a);
            let negative = b.ct_lt(&a);
            let difference = b.wrapping_sub(&a);
            let magnitude = Uint::ct_select(&difference, &difference.wrapping_neg(), negative);
            let delta = shared_raw(f, ctx.sub_raw(m[1], m[0]));
            acc[1].accumulate(&Fp::ct_select(&delta, &f.neg(&delta), negative), &magnitude);
        }
        acc
    };
    #[cfg(feature = "parallel")]
    if parallel(weights.len() / 2) {
        let acc = weights
            .par_chunks(2 * FOLD_BLOCK)
            .enumerate()
            .map(|(i, w)| block(i * 2 * FOLD_BLOCK, w))
            .reduce(zero, merge);
        return acc.map(|a| raw_shared(field::Reduce::reduce(f, a)));
    }
    block(0, weights).map(|a| raw_shared(field::Reduce::reduce(f, a)))
}

// FOLD_WEIGHTS is selected once by the dense/structured caller. Both forms
// read each borrowed integer once, write canonical folds, and accumulate the
// next message in the same traversal.
pub(super) fn wide_fold<const N: usize, const FOLD_WEIGHTS: bool>(
    ctx: &field::FpCtx<2>,
    weights: &[Raw],
    read: impl Fn(usize) -> Uint<N> + Sync,
    matrix_out: &mut [Raw],
    out: &mut [Uint<2>],
    challenge: Raw,
) -> [Raw; 2] {
    assert_eq!(out.len() % 2, 0);
    assert_eq!(weights.len(), out.len() * if FOLD_WEIGHTS { 2 } else { 1 });
    assert_eq!(matrix_out.len(), if FOLD_WEIGHTS { out.len() } else { 0 });
    let f = ctx;
    let challenge = shared_raw(f, challenge);
    let coefficients = [f.sub(&f.one(), &challenge), challenge];
    let zero = || [FpLinearAcc::<2, 2>::zero(); 2];
    let merge = |mut a: [FpLinearAcc<2, 2>; 2], b: [FpLinearAcc<2, 2>; 2]| {
        for i in 0..2 {
            a[i].merge_assign(&b[i]);
        }
        a
    };
    let block = |start: usize, mout: &mut [Raw], out: &mut [Uint<2>]| {
        let mut acc = zero();
        for (pair, z) in out.chunks_exact_mut(2).enumerate() {
            let mut m = [0; 2];
            for j in 0..2 {
                let i = start + 2 * pair + j;
                z[j] = f.weighted_pair_to_integer(&coefficients, &[read(2 * i), read(2 * i + 1)]);
                m[j] = if FOLD_WEIGHTS {
                    ctx.interpolate(weights[2 * i], weights[2 * i + 1], raw_shared(challenge))
                } else {
                    weights[i]
                };
                if FOLD_WEIGHTS {
                    mout[2 * pair + j] = m[j];
                }
            }
            acc[0].accumulate(&shared_raw(f, m[0]), &z[0]);
            acc[1].accumulate(
                &shared_raw(f, ctx.sub_raw(m[1], m[0])),
                &Uint::from_words(raw_to_words(
                    ctx.sub_raw(words_to_raw(z[1].as_words()), words_to_raw(z[0].as_words())),
                )),
            );
        }
        acc
    };
    let reduce = |a: [FpLinearAcc<2, 2>; 2]| a.map(|a| raw_shared(field::Reduce::reduce(f, a)));
    #[cfg(feature = "parallel")]
    if parallel(out.len() / 2) {
        let acc = if FOLD_WEIGHTS {
            matrix_out
                .par_chunks_mut(2 * FOLD_BLOCK)
                .zip(out.par_chunks_mut(2 * FOLD_BLOCK))
                .enumerate()
                .map(|(i, (m, z))| block(i * 2 * FOLD_BLOCK, m, z))
                .reduce(zero, merge)
        } else {
            out.par_chunks_mut(2 * FOLD_BLOCK)
                .enumerate()
                .map(|(i, z)| block(i * 2 * FOLD_BLOCK, &mut [], z))
                .reduce(zero, merge)
        };
        return reduce(acc);
    }
    reduce(block(0, matrix_out, out))
}

impl BlockValues<'_> {
    pub(super) fn coefficients(
        self,
        ctx: &field::FpCtx<2>,
        reducer: &field::FpCtx<2>,
        weights: &[Raw],
    ) -> [Raw; 2] {
        match self {
            Self::Native(v) => {
                inner_coefficients_native_raw(ctx, reducer, weights, &v[..weights.len()])
            }
            Self::Field(v) => {
                inner_coefficients_field_raw(ctx, reducer, weights, &v[..weights.len()])
            }
            Self::Zero => [0; 2],
            Self::Limbs(v) => wide_coefficients(ctx, weights, |i| read_limbs(v, i)),
            Self::U128(v) => wide_coefficients(ctx, weights, |i| read_u128(v, i)),
            Self::U256(lo, hi) => wide_coefficients(ctx, weights, |i| read_u256(lo, hi, i)),
            Self::ConstantOne => {
                wide_coefficients(ctx, weights, |i| Uint::<1>::from_u64((i == 0) as u64))
            }
        }
    }
    pub(super) fn folded_pair(
        self,
        ctx: &field::FpCtx<2>,
        reducer: &field::FpCtx<2>,
        challenge: Raw,
    ) -> Raw {
        let f = ctx;
        let c = shared_raw(f, challenge);
        let coefficients = [f.sub(&f.one(), &c), c];
        match self {
            Self::Native(v) => fold_native_pair(
                reducer,
                ctx.sub_raw(ctx.one_raw(), challenge),
                challenge,
                v[0],
                v[1],
            ),
            Self::Field(v) => ctx.interpolate(v[0], v[1], challenge),
            Self::Zero => 0,
            Self::Limbs(v) => {
                raw_shared(f.weighted_pair(&coefficients, &[read_limbs(v, 0), read_limbs(v, 1)]))
            }
            Self::U128(v) => {
                raw_shared(f.weighted_pair(&coefficients, &[read_u128(v, 0), read_u128(v, 1)]))
            }
            Self::U256(lo, hi) => raw_shared(
                f.weighted_pair(&coefficients, &[read_u256(lo, hi, 0), read_u256(lo, hi, 1)]),
            ),
            Self::ConstantOne => raw_shared(coefficients[0]),
        }
    }
    pub(super) fn fold_integer_block(
        self,
        ctx: &field::FpCtx<2>,
        reducer: &field::FpCtx<2>,
        weights: &[Raw],
        out: &mut [Uint<2>],
        challenge: Raw,
    ) -> [Raw; 2] {
        match self {
            Self::Native(v) => {
                fold_block_native_raw(ctx, reducer, weights, &v[..2 * out.len()], out, challenge)
            }
            Self::Field(_) => unreachable!("integer source dispatch"),
            Self::Zero => {
                out.fill(Uint::ZERO);
                [0; 2]
            }
            Self::Limbs(v) => {
                wide_fold::<32, false>(ctx, weights, |i| read_limbs(v, i), &mut [], out, challenge)
            }
            Self::U128(v) => {
                wide_fold::<2, false>(ctx, weights, |i| read_u128(v, i), &mut [], out, challenge)
            }
            Self::U256(lo, hi) => wide_fold::<4, false>(
                ctx,
                weights,
                |i| read_u256(lo, hi, i),
                &mut [],
                out,
                challenge,
            ),
            Self::ConstantOne => wide_fold::<1, false>(
                ctx,
                weights,
                |i| Uint::from_u64((i == 0) as u64),
                &mut [],
                out,
                challenge,
            ),
        }
    }
}

pub(super) fn weighted_wide<const N: usize>(
    ctx: &field::FpCtx<2>,
    eq: &[Raw],
    read: impl Fn(usize) -> Uint<N> + Sync,
) -> Raw {
    let f = ctx;
    let block = |start: usize, weights: &[Raw]| {
        let mut acc = FpLinearAcc::<2, N>::zero();
        for (i, &w) in weights.iter().enumerate() {
            acc.accumulate(&shared_raw(f, w), &read(start + i));
        }
        acc
    };
    #[cfg(feature = "parallel")]
    if parallel(eq.len()) {
        let acc = eq
            .par_chunks(2 * FOLD_BLOCK)
            .enumerate()
            .map(|(i, w)| block(i * 2 * FOLD_BLOCK, w))
            .reduce(FpLinearAcc::zero, |mut a, b| {
                a.merge_assign(&b);
                a
            });
        return raw_shared(field::Reduce::reduce(f, acc));
    }
    raw_shared(field::Reduce::reduce(f, block(0, eq)))
}
pub(super) fn weighted_wide_block(
    ctx: &field::FpCtx<2>,
    eq: &[Raw],
    block: BlockValues<'_>,
) -> Raw {
    match block {
        BlockValues::U128(v) => weighted_wide(ctx, eq, |i| read_u128(v, i)),
        BlockValues::U256(lo, hi) => weighted_wide(ctx, eq, |i| read_u256(lo, hi, i)),
        BlockValues::ConstantOne => eq[0],
        BlockValues::Zero => 0,
        BlockValues::Limbs(v) => weighted_wide(ctx, eq, |i| read_limbs(v, i)),
        _ => unreachable!(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transcript::Blake3Transcript;

    use rand::{RngExt, SeedableRng, rngs::StdRng};

    #[test]
    fn limb_witness_matches_projected_dense_and_structured_rounds() {
        let mut rng = StdRng::seed_from_u64(0x33326c696d62);
        for q in [(1u128 << 100) - 15, u128::MAX - 158] {
            let cfg = Field::make_cfg(&Uint::from_words(raw_to_words(q))).unwrap();
            let ctx = field_context(&cfg);
            for log in [0, 1, 3, 7] {
                let cap = 1usize << log;
                let live = cap.saturating_sub(1).max(1);
                let mut values = || {
                    (0..live)
                        .map(|i| {
                            Uint::<32>::from_words(core::array::from_fn(|_| match i {
                                0 => u64::MAX,
                                1 => 0,
                                _ => rng.random(),
                            }))
                        })
                        .collect::<Vec<_>>()
                };
                let witness = values();
                let quotient = values();
                let input = NativeLimbWitness::new(cap, &witness, &quotient);
                let radix = Field::from_with_cfg(1u128 << 64, &cfg);
                let projected: Vec<_> = (0..input.len())
                    .map(|i| {
                        let v = input.read(i);
                        let mut out = Field::zero_with_cfg(&cfg);
                        for &word in v.as_words().iter().rev() {
                            out = cfg.mul(&(out), &(&radix));
                            out = cfg.add(&(out), &(&Field::from_with_cfg(word, &cfg)));
                        }
                        ctx.raw(&out)
                    })
                    .collect();
                let weights: Vec<_> = (0..cap)
                    .map(|_| ctx.native_residue_u128(rng.random()))
                    .collect();
                for active in [[false, true, true, false], [true, false, true, true]] {
                    let scales: Vec<_> = active
                        .iter()
                        .map(|&v| v.then(|| ctx.native_residue_u128(rng.random())))
                        .collect();
                    let mut matrix = vec![0; 4 * cap];
                    for b in 0..4 {
                        if let Some(scale) = scales[b] {
                            for i in 0..cap {
                                matrix[b * cap + i] = ctx.mul_raw(scale, weights[i]);
                            }
                        }
                    }
                    let raw_claim = matrix
                        .iter()
                        .zip(&projected)
                        .fold(0, |a, (&m, &w)| ctx.add_raw(a, ctx.mul_raw(m, w)));
                    let claim = crate::utils::delayed_reduction::element(&cfg, raw_claim);
                    let expected = prove_inner_raw(
                        &mut Blake3Transcript::new(),
                        &ctx,
                        &ctx,
                        claim.clone(),
                        matrix.clone(),
                        RawWitness::Field(projected.clone()),
                        4 * cap,
                    )
                    .unwrap();
                    let got = prove_inner_raw(
                        &mut Blake3Transcript::new(),
                        &ctx,
                        &ctx,
                        claim.clone(),
                        matrix,
                        RawWitness::Limbs(input),
                        4 * cap,
                    )
                    .unwrap();
                    assert_eq!(got, expected, "dense log={log}");
                    if cap >= 2 {
                        let got = prove_inner_structured_raw(
                            &mut Blake3Transcript::new(),
                            &ctx,
                            &ctx,
                            claim,
                            &weights,
                            &BlockScales {
                                block_len: cap,
                                rows: cap,
                                scales,
                            },
                            RawWitness::Limbs(input),
                            4 * cap,
                            log + 2,
                        )
                        .unwrap();
                        assert_eq!(got, expected, "structured log={log}");
                    }
                }
            }
        }
    }

    #[test]
    fn segmented_witness_matches_dense_and_structured_projection() {
        let mut rng = StdRng::seed_from_u64(0x7769_6465_5f696e6e);
        for q in [(1u128 << 100) - 15, u128::MAX - 158] {
            let cfg = Field::make_cfg(&Uint::from_words(raw_to_words(q))).unwrap();
            let ctx = crate::piop::spartan::raw_monty::field_context(&cfg);
            let reducer = crate::utils::delayed_reduction::prepare_field(&cfg).unwrap();
            for log in [0, 1, 2, 6, 13] {
                let cap = 1usize << log;
                for live in [cap, cap.saturating_sub(1).max(1)] {
                    let mut values = || {
                        (0..live)
                            .map(|i| {
                                if i < 4 {
                                    [u128::MAX, 0, 1u128 << 127, 1][i]
                                } else {
                                    rng.random()
                                }
                            })
                            .collect::<Vec<u128>>()
                    };
                    let x = values();
                    let y = values();
                    let lo = values();
                    let hi = values();
                    let input = NativeU128Witness::new(cap, &x, &y, &lo, &hi);
                    let two128 = cfg.mul(
                        &(Field::from_with_cfg(1u128 << 127, &cfg)),
                        &(&Field::from_with_cfg(2u64, &cfg)),
                    );
                    let mut projected = vec![0; 4 * cap];
                    projected[0] = ctx.one_raw();
                    for i in 0..live {
                        projected[cap + i] = ctx.raw(&Field::from_with_cfg(x[i], &cfg));
                        projected[2 * cap + i] = ctx.raw(&Field::from_with_cfg(y[i], &cfg));
                        projected[3 * cap + i] = ctx.raw(
                            &(cfg.add(
                                &(Field::from_with_cfg(lo[i], &cfg)),
                                &(&(cfg.mul(
                                    &(two128.clone()),
                                    &(&Field::from_with_cfg(hi[i], &cfg)),
                                ))),
                            )),
                        );
                    }
                    let weights: Vec<_> = (0..cap)
                        .map(|_| ctx.native_residue_u128(rng.random()))
                        .collect();
                    for active in [[false, true, true, true], [true, false, true, false]] {
                        let scales: Vec<_> = active
                            .iter()
                            .map(|&v| v.then(|| ctx.native_residue_u128(rng.random())))
                            .collect();
                        let mut matrix = vec![0; 4 * cap];
                        for b in 0..4 {
                            if let Some(scale) = scales[b] {
                                for i in 0..cap {
                                    matrix[b * cap + i] = ctx.mul_raw(scale, weights[i]);
                                }
                            }
                        }
                        let claim = crate::utils::delayed_reduction::element(
                            &cfg,
                            matrix
                                .iter()
                                .zip(&projected)
                                .fold(0, |a, (&m, &w)| ctx.add_raw(a, ctx.mul_raw(m, w))),
                        );
                        let expected = prove_inner_raw(
                            &mut Blake3Transcript::new(),
                            &ctx,
                            &reducer,
                            claim.clone(),
                            matrix.clone(),
                            RawWitness::Field(projected.clone()),
                            4 * cap,
                        )
                        .unwrap();
                        let got = prove_inner_raw(
                            &mut Blake3Transcript::new(),
                            &ctx,
                            &reducer,
                            claim.clone(),
                            matrix,
                            RawWitness::Wide(input),
                            4 * cap,
                        )
                        .unwrap();
                        assert_eq!(got, expected, "dense log={log} live={live}");
                        if cap >= 2 {
                            let got = prove_inner_structured_raw(
                                &mut Blake3Transcript::new(),
                                &ctx,
                                &reducer,
                                claim,
                                &weights,
                                &BlockScales {
                                    block_len: cap,
                                    rows: cap,
                                    scales,
                                },
                                RawWitness::Wide(input),
                                4 * cap,
                                log + 2,
                            )
                            .unwrap();
                            assert_eq!(got, expected, "structured log={log} live={live}");
                        }
                    }
                }
            }
        }
    }
}

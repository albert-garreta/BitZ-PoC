//! Typed folded witness storage. Its type selects the unique MAC scale.
use super::*;
use crate::piop::spartan::raw_monty::RawFieldStorage;
use field::RingOps;
use field::{Fp, FpLinearAcc, FpProductAcc, MergeAccumulator, Uint};

pub(super) trait FoldedValue: Copy + Send + Sync {
    type Acc: Copy + Send + MergeAccumulator;
    fn zero(ctx: &field::FpCtx<2>) -> Self;
    fn encoding(self) -> Raw;
    fn from_encoding(ctx: &field::FpCtx<2>, value: Raw) -> Self;
    fn final_raw(self, ctx: &field::FpCtx<2>) -> Raw;
    fn accumulate(ctx: &field::FpCtx<2>, acc: &mut Self::Acc, weight: Raw, value: Self);
    fn reduce(ctx: &field::FpCtx<2>, acc: Self::Acc) -> Raw;
    fn fold_initial(
        ctx: &field::FpCtx<2>,
        reducer: &field::FpCtx<2>,
        block: BlockValues<'_>,
        weights: &[Raw],
        out: &mut [Self],
        challenge: Raw,
    ) -> [Raw; 2];
}
impl FoldedValue for Uint<2> {
    type Acc = FpLinearAcc<2, 2>;
    fn zero(_: &field::FpCtx<2>) -> Self {
        Self::ZERO
    }
    #[inline(always)]
    fn encoding(self) -> Raw {
        words_to_raw(self.as_words())
    }
    #[inline(always)]
    fn from_encoding(_: &field::FpCtx<2>, value: Raw) -> Self {
        Self::from_words(raw_to_words(value))
    }
    #[inline(always)]
    fn final_raw(self, ctx: &field::FpCtx<2>) -> Raw {
        ctx.plain_to_raw(self.encoding())
    }
    #[inline(always)]
    fn accumulate(ctx: &field::FpCtx<2>, acc: &mut Self::Acc, weight: Raw, value: Self) {
        acc.accumulate(&shared_raw(ctx, weight), &value);
    }
    #[inline(always)]
    fn reduce(ctx: &field::FpCtx<2>, acc: Self::Acc) -> Raw {
        raw_shared(field::Reduce::reduce(ctx, acc))
    }
    fn fold_initial(
        ctx: &field::FpCtx<2>,
        reducer: &field::FpCtx<2>,
        block: BlockValues<'_>,
        weights: &[Raw],
        out: &mut [Self],
        challenge: Raw,
    ) -> [Raw; 2] {
        block.fold_integer_block(ctx, reducer, weights, out, challenge)
    }
}
impl FoldedValue for Fp<2> {
    type Acc = FpProductAcc<2>;
    fn zero(ctx: &field::FpCtx<2>) -> Self {
        shared_raw(ctx, 0)
    }
    #[inline(always)]
    fn encoding(self) -> Raw {
        raw_shared(self)
    }
    #[inline(always)]
    fn from_encoding(ctx: &field::FpCtx<2>, value: Raw) -> Self {
        shared_raw(ctx, value)
    }
    #[inline(always)]
    fn final_raw(self, _: &field::FpCtx<2>) -> Raw {
        raw_shared(self)
    }
    #[inline(always)]
    fn accumulate(ctx: &field::FpCtx<2>, acc: &mut Self::Acc, weight: Raw, value: Self) {
        acc.accumulate(&shared_raw(ctx, weight), &value);
    }
    #[inline(always)]
    fn reduce(ctx: &field::FpCtx<2>, acc: Self::Acc) -> Raw {
        raw_shared(field::Reduce::reduce(ctx, acc))
    }
    fn fold_initial(
        ctx: &field::FpCtx<2>,
        _: &field::FpCtx<2>,
        block: BlockValues<'_>,
        weights: &[Raw],
        out: &mut [Self],
        challenge: Raw,
    ) -> [Raw; 2] {
        let BlockValues::Field(values) = block else {
            unreachable!("field source dispatch")
        };
        fold_round::<Self, false>(
            ctx,
            weights,
            |i| shared_raw(ctx, values[i]),
            &mut [],
            out,
            challenge,
        )
    }
}

pub(super) fn pair<W: FoldedValue>() -> [W::Acc; 2] {
    [W::Acc::zero(); 2]
}
pub(super) fn merge<W: FoldedValue>(mut a: [W::Acc; 2], b: [W::Acc; 2]) -> [W::Acc; 2] {
    for i in 0..2 {
        a[i].merge_assign(&b[i]);
    }
    a
}
pub(super) fn reduce<W: FoldedValue>(ctx: &field::FpCtx<2>, a: [W::Acc; 2]) -> [Raw; 2] {
    a.map(|a| W::reduce(ctx, a))
}

pub(super) fn fold_round<W: FoldedValue, const FOLD_WEIGHTS: bool>(
    ctx: &field::FpCtx<2>,
    weights: &[Raw],
    read: impl Fn(usize) -> W + Sync,
    matrix_out: &mut [Raw],
    out: &mut [W],
    challenge: Raw,
) -> [Raw; 2] {
    assert_eq!(out.len() % 2, 0);
    assert_eq!(weights.len(), out.len() * if FOLD_WEIGHTS { 2 } else { 1 });
    assert_eq!(matrix_out.len(), if FOLD_WEIGHTS { out.len() } else { 0 });
    let block = |start: usize, mout: &mut [Raw], out: &mut [W]| {
        let mut acc = pair::<W>();
        for (pair, z) in out.chunks_exact_mut(2).enumerate() {
            let mut m = [0; 2];
            for j in 0..2 {
                let i = start + 2 * pair + j;
                z[j] = W::from_encoding(
                    ctx,
                    ctx.interpolate(
                        read(2 * i).encoding(),
                        read(2 * i + 1).encoding(),
                        challenge,
                    ),
                );
                m[j] = if FOLD_WEIGHTS {
                    ctx.interpolate(weights[2 * i], weights[2 * i + 1], challenge)
                } else {
                    weights[i]
                };
                if FOLD_WEIGHTS {
                    mout[2 * pair + j] = m[j];
                }
            }
            W::accumulate(ctx, &mut acc[0], m[0], z[0]);
            W::accumulate(
                ctx,
                &mut acc[1],
                ctx.sub_raw(m[1], m[0]),
                W::from_encoding(ctx, ctx.sub_raw(z[1].encoding(), z[0].encoding())),
            );
        }
        acc
    };
    #[cfg(feature = "parallel")]
    if parallel(out.len() / 2) {
        let acc = if FOLD_WEIGHTS {
            matrix_out
                .par_chunks_mut(FOLD_BLOCK)
                .zip(out.par_chunks_mut(FOLD_BLOCK))
                .enumerate()
                .map(|(i, (m, z))| block(i * FOLD_BLOCK, m, z))
                .reduce(pair::<W>, merge::<W>)
        } else {
            out.par_chunks_mut(FOLD_BLOCK)
                .enumerate()
                .map(|(i, z)| block(i * FOLD_BLOCK, &mut [], z))
                .reduce(pair::<W>, merge::<W>)
        };
        return reduce::<W>(ctx, acc);
    }
    reduce::<W>(ctx, block(0, matrix_out, out))
}

pub(super) enum FoldedWitness {
    Integers(Vec<Uint<2>>),
    Field(Vec<Fp<2>>),
}

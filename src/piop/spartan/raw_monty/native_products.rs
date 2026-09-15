//! Borrowed native operands and split products. Width dispatch occurs before
//! any hot loop; integer projection is fused with the first outer MAC/fold.
use super::*;
use crate::piop::spartan::raw_monty::RawFieldStorage;
use crate::utils::delayed_reduction::EncodedMac;
use field::{CtMask, CtOrd, CtSelect, Fp, FpLinearAcc, MergeAccumulator, RingOps, Uint, WideMul};

#[derive(Clone, Copy)]
pub struct NativeWideProducts<'a, T> {
    az: &'a [T],
    bz: &'a [T],
    cz_lo: &'a [T],
    cz_hi: &'a [T],
    rows: usize,
}
impl<'a, T> NativeWideProducts<'a, T> {
    pub(crate) fn new(
        az: &'a [T],
        bz: &'a [T],
        cz_lo: &'a [T],
        cz_hi: &'a [T],
        rows: usize,
    ) -> Self {
        assert!(rows.is_power_of_two());
        assert!(az.len() <= rows);
        assert_eq!(az.len(), bz.len());
        assert_eq!(az.len(), cz_lo.len());
        assert_eq!(az.len(), cz_hi.len());
        Self {
            az,
            bz,
            cz_lo,
            cz_hi,
            rows,
        }
    }
}

pub(crate) trait NativeOuterInput: Copy + Sync {
    fn len(&self) -> usize;
    fn valid_shape(&self) -> bool;
    fn singleton(&self, ctx: &field::FpCtx<2>) -> [Raw; 3];
    fn round0(
        &self,
        ctx: &field::FpCtx<2>,
        reducer: &field::FpCtx<2>,
        weights: RawEqWeights<'_>,
        endpoint: FactoredEndpoint,
    ) -> [Raw; 2];
    fn fold(
        &self,
        ctx: &field::FpCtx<2>,
        reducer: &field::FpCtx<2>,
        out: &mut RawProducts,
        challenge: Raw,
        weights: Option<(RawEqWeights<'_>, FactoredEndpoint)>,
    ) -> [Raw; 2];
}
impl NativeOuterInput for NativeProducts<'_> {
    fn len(&self) -> usize {
        self.az.len()
    }
    fn valid_shape(&self) -> bool {
        self.az.len() == self.bz.len() && self.az.len() == self.cz.len()
    }
    fn singleton(&self, ctx: &field::FpCtx<2>) -> [Raw; 3] {
        [
            ctx.native_residue(self.az[0]),
            ctx.native_residue(self.bz[0]),
            ctx.native_residue(self.cz[0]),
        ]
    }
    fn round0(
        &self,
        ctx: &field::FpCtx<2>,
        reducer: &field::FpCtx<2>,
        weights: RawEqWeights<'_>,
        endpoint: FactoredEndpoint,
    ) -> [Raw; 2] {
        native_cofactor_evaluations_raw(ctx, reducer, *self, weights, endpoint)
    }
    fn fold(
        &self,
        ctx: &field::FpCtx<2>,
        reducer: &field::FpCtx<2>,
        out: &mut RawProducts,
        challenge: Raw,
        weights: Option<(RawEqWeights<'_>, FactoredEndpoint)>,
    ) -> [Raw; 2] {
        fold_native_products_and_cofactor_evaluations_raw(
            ctx, reducer, *self, out, challenge, weights,
        )
    }
}

// A prepared product table can share the outer driver with native inputs.
// Its first fold still fuses the next cofactor evaluation.
impl NativeOuterInput for &RawProducts {
    fn len(&self) -> usize {
        self.az.len()
    }
    fn valid_shape(&self) -> bool {
        self.az.len() == self.bz.len() && self.az.len() == self.cz.len()
    }
    fn singleton(&self, _: &field::FpCtx<2>) -> [Raw; 3] {
        [self.az[0], self.bz[0], self.cz[0]]
    }
    fn round0(
        &self,
        ctx: &field::FpCtx<2>,
        reducer: &field::FpCtx<2>,
        weights: RawEqWeights<'_>,
        endpoint: FactoredEndpoint,
    ) -> [Raw; 2] {
        cofactor_evaluations_raw(ctx, reducer, self, weights, endpoint)
    }
    fn fold(
        &self,
        ctx: &field::FpCtx<2>,
        reducer: &field::FpCtx<2>,
        out: &mut RawProducts,
        challenge: Raw,
        weights: Option<(RawEqWeights<'_>, FactoredEndpoint)>,
    ) -> [Raw; 2] {
        match weights {
            Some((weights, endpoint)) => fold_products_and_cofactor_evaluations_raw(
                ctx, reducer, self, out, challenge, weights, endpoint,
            ),
            None => {
                fold_products_raw(ctx, self, out, challenge);
                [0; 2]
            }
        }
    }
}

fn magnitude<const N: usize>(a: Uint<N>, b: Uint<N>) -> (Uint<N>, CtMask) {
    let negative = a.ct_lt(&b);
    let difference = a.wrapping_sub(&b);
    (
        Uint::ct_select(&difference, &difference.wrapping_neg(), negative),
        negative,
    )
}

trait WideSource<const N: usize>: Copy + Sync {
    fn len(self) -> usize;
    fn cofactor(self, pair: usize, endpoint: FactoredEndpoint) -> [(Uint<N>, CtMask); 2];
    fn folded(self, field: &field::FpCtx<2>, coefficients: &[Fp<2>; 2], index: usize) -> [Raw; 3];
}
macro_rules! wide_source {
    ($native:ty, $a:literal, $p:literal, $operand:expr, $product:expr) => {
        impl WideSource<$p> for NativeWideProducts<'_, $native> {
            fn len(self) -> usize {
                self.rows
            }
            #[inline]
            fn cofactor(self, pair: usize, endpoint: FactoredEndpoint) -> [(Uint<$p>, CtMask); 2] {
                let read =
                    |values: &[$native], i: usize| ($operand)(values.get(i).copied().unwrap_or(0));
                let [a0, a1] = [read(self.az, 2 * pair), read(self.az, 2 * pair + 1)];
                let [b0, b1] = [read(self.bz, 2 * pair), read(self.bz, 2 * pair + 1)];
                let e = match endpoint {
                    FactoredEndpoint::Zero => 0,
                    FactoredEndpoint::One => 1,
                };
                let (a, b) = if e == 0 { (a0, b0) } else { (a1, b1) };
                let c = ($product)(
                    self.cz_lo.get(2 * pair + e).copied().unwrap_or(0),
                    self.cz_hi.get(2 * pair + e).copied().unwrap_or(0),
                );
                let ab = *field::IntegerOps
                    .mul_wide(&a, &b)
                    .checked_resize_ct::<$p>()
                    .value();
                let residual = magnitude(ab, c);
                let (da, na) = magnitude(a1, a0);
                let (db, nb) = magnitude(b1, b0);
                let infinity = *field::IntegerOps
                    .mul_wide(&da, &db)
                    .checked_resize_ct::<$p>()
                    .value();
                [residual, (infinity, na ^ nb)]
            }
            #[inline]
            fn folded(
                self,
                field: &field::FpCtx<2>,
                coefficients: &[Fp<2>; 2],
                index: usize,
            ) -> [Raw; 3] {
                let fold_operand = |values: &[$native]| {
                    let v = [
                        ($operand)(values.get(index).copied().unwrap_or(0)),
                        ($operand)(values.get(index + 1).copied().unwrap_or(0)),
                    ];
                    raw_shared(field.weighted_pair(coefficients, &v))
                };
                let product = |i| {
                    ($product)(
                        self.cz_lo.get(i).copied().unwrap_or(0),
                        self.cz_hi.get(i).copied().unwrap_or(0),
                    )
                };
                [
                    fold_operand(self.az),
                    fold_operand(self.bz),
                    raw_shared(
                        field.weighted_pair(coefficients, &[product(index), product(index + 1)]),
                    ),
                ]
            }
        }
        impl NativeOuterInput for NativeWideProducts<'_, $native> {
            fn len(&self) -> usize {
                self.rows
            }
            fn valid_shape(&self) -> bool {
                true
            } // constructor validated all public shapes
            fn singleton(&self, ctx: &field::FpCtx<2>) -> [Raw; 3] {
                let f = ctx;
                self.folded(f, &[f.one(), f.zero()], 0)
            }
            fn round0(
                &self,
                ctx: &field::FpCtx<2>,
                reducer: &field::FpCtx<2>,
                weights: RawEqWeights<'_>,
                endpoint: FactoredEndpoint,
            ) -> [Raw; 2] {
                wide_round0::<$p, _>(ctx, reducer, *self, weights, endpoint)
            }
            fn fold(
                &self,
                ctx: &field::FpCtx<2>,
                reducer: &field::FpCtx<2>,
                out: &mut RawProducts,
                challenge: Raw,
                weights: Option<(RawEqWeights<'_>, FactoredEndpoint)>,
            ) -> [Raw; 2] {
                wide_fold::<$p, _>(ctx, reducer, *self, out, challenge, weights)
            }
        }
    };
}
wide_source!(
    u64,
    1,
    2,
    |v: u64| Uint::<1>::from_words([v]),
    |lo: u64, hi: u64| Uint::<2>::from_words([lo, hi])
);
wide_source!(
    u128,
    2,
    4,
    |v: u128| Uint::<2>::from_words(raw_to_words(v)),
    |lo: u128, hi: u128| Uint::<4>::from_words([
        lo as u64,
        (lo >> 64) as u64,
        hi as u64,
        (hi >> 64) as u64
    ])
);

fn wide_round0<const N: usize, P: WideSource<N>>(
    ctx: &field::FpCtx<2>,
    reducer: &field::FpCtx<2>,
    products: P,
    weights: RawEqWeights<'_>,
    endpoint: FactoredEndpoint,
) -> [Raw; 2] {
    let f = ctx;
    let count = products.len() / 2;
    assert_eq!(count, weights.pair_count());
    let zero = || [FpLinearAcc::<2, N>::zero(); 2];
    let merge = |mut a: [FpLinearAcc<2, N>; 2], b: [FpLinearAcc<2, N>; 2]| {
        for i in 0..2 {
            a[i].merge_assign(&b[i]);
        }
        a
    };
    let block = |start: usize, end: usize, low: Option<&[Raw]>| {
        let mut acc = zero();
        for i in start..end {
            let w = shared_raw(
                f,
                low.map_or_else(|| weights.pair_weight(ctx, i), |v| v[i - start]),
            );
            let neg = f.neg(&w);
            for (a, (value, negative)) in acc.iter_mut().zip(products.cofactor(i, endpoint)) {
                a.accumulate(&Fp::ct_select(&w, &neg, negative), &value);
            }
        }
        acc
    };
    let reduce = |a: [FpLinearAcc<2, N>; 2]| a.map(|a| raw_shared(field::Reduce::reduce(f, a)));
    if let Some(low) = weights.two_level_low() {
        let bucket = |hi: usize| {
            let inner = reduce(block(hi * low.len(), (hi + 1) * low.len(), Some(low)));
            let mut out = product_pair();
            for i in 0..2 {
                out[i].accumulate_encoded(ctx, weights.high[hi], inner[i]);
            }
            out
        };
        #[cfg(feature = "parallel")]
        if parallel(count) {
            return reduce_product_pair(
                (0..weights.high.len())
                    .into_par_iter()
                    .map(bucket)
                    .reduce(product_pair, merge_product_pair),
                reducer,
            );
        }
        return reduce_product_pair(
            (0..weights.high.len())
                .map(bucket)
                .fold(product_pair(), merge_product_pair),
            reducer,
        );
    }
    #[cfg(feature = "parallel")]
    if parallel(count) {
        return reduce(
            (0..count.div_ceil(FOLD_BLOCK))
                .into_par_iter()
                .map(|b| block(b * FOLD_BLOCK, ((b + 1) * FOLD_BLOCK).min(count), None))
                .reduce(zero, merge),
        );
    }
    reduce(block(0, count, None))
}

fn wide_fold<const N: usize, P: WideSource<N>>(
    ctx: &field::FpCtx<2>,
    reducer: &field::FpCtx<2>,
    products: P,
    out: &mut RawProducts,
    challenge: Raw,
    weights: Option<(RawEqWeights<'_>, FactoredEndpoint)>,
) -> [Raw; 2] {
    assert_eq!(products.len(), 2 * out.len());
    let f = ctx;
    let challenge = shared_raw(f, challenge);
    let coefficients = [f.sub(&f.one(), &challenge), challenge];
    let Some((weights, endpoint)) = weights else {
        assert_eq!(out.len(), 1);
        let [a, b, c] = products.folded(f, &coefficients, 0);
        out.az[0] = a;
        out.bz[0] = b;
        out.cz[0] = c;
        return [0, 0];
    };
    let pairs = out.len() / 2;
    assert_eq!(pairs, weights.pair_count());
    let process = |start: usize, a: &mut [Raw], b: &mut [Raw], c: &mut [Raw]| {
        let mut total = product_pair();
        let low_count = weights.two_level_low().map_or(a.len() / 2, |v| v.len());
        for base in (0..a.len() / 2).step_by(low_count) {
            let mut inner = product_pair();
            for j in 0..low_count {
                let pair = base + j;
                let i = 4 * (start + pair);
                let o = 2 * pair;
                let [a0, b0, c0] = products.folded(f, &coefficients, i);
                let [a1, b1, c1] = products.folded(f, &coefficients, i + 2);
                a[o] = a0;
                a[o + 1] = a1;
                b[o] = b0;
                b[o + 1] = b1;
                c[o] = c0;
                c[o + 1] = c1;
                let weight = weights
                    .two_level_low()
                    .map_or_else(|| weights.pair_weight(ctx, start + pair), |v| v[j]);
                accumulate_cofactor_raw(ctx, &mut inner, weight, endpoint, a0, a1, b0, b1, c0, c1);
            }
            if weights.two_level_low().is_some() {
                let inner = reduce_product_pair(inner, reducer);
                let high = weights.high[(start + base) / low_count];
                for i in 0..2 {
                    total[i].accumulate_encoded(ctx, high, inner[i]);
                }
            } else {
                total = merge_product_pair(total, inner);
            }
        }
        total
    };
    #[cfg(feature = "parallel")]
    if parallel(pairs) {
        let block = weights.two_level_low().map_or(FOLD_BLOCK / 2, |v| {
            (FOLD_BLOCK / 2).div_ceil(v.len()).max(1) * v.len()
        });
        let total = (
            out.az.par_chunks_mut(2 * block),
            out.bz.par_chunks_mut(2 * block),
            out.cz.par_chunks_mut(2 * block),
        )
            .into_par_iter()
            .enumerate()
            .map(|(i, (a, b, c))| process(i * block, a, b, c))
            .reduce(product_pair, merge_product_pair);
        return reduce_product_pair(total, reducer);
    }
    reduce_product_pair(process(0, &mut out.az, &mut out.bz, &mut out.cz), reducer)
}

#[cfg(test)]
mod tests {
    use super::*;

    use rand::{RngExt, SeedableRng, rngs::StdRng};

    #[test]
    fn native_wide_rounds_match_projected_full_width_and_padded_inputs() {
        let mut rng = StdRng::seed_from_u64(0x4f75_7465_725f_7769);
        for q in [(1u128 << 100) - 15, u128::MAX - 158] {
            let cfg = Field::make_cfg(&Uint::from_words(raw_to_words(q))).unwrap();
            let ctx = crate::piop::spartan::raw_monty::field_context(&cfg);
            let reducer = crate::utils::delayed_reduction::prepare_field(&cfg).unwrap();
            for log in [1, 2, 5, 14] {
                let rows = 1usize << log;
                for live in [rows, rows - 1] {
                    let mut values = || {
                        (0..live)
                            .map(|i| {
                                if i < 4 {
                                    [0, u128::MAX, 1, 1u128 << 127][i]
                                } else {
                                    rng.random()
                                }
                            })
                            .collect::<Vec<u128>>()
                    };
                    let a = values();
                    let b = values();
                    let lo = values();
                    let hi = values();
                    let native = NativeWideProducts::new(&a, &b, &lo, &hi, rows);
                    let two128 = cfg.mul(
                        &(Field::from_with_cfg(1u128 << 127, &cfg)),
                        &(&Field::from_with_cfg(2u64, &cfg)),
                    );
                    let project = |v: &[u128]| {
                        (0..rows)
                            .map(|i| {
                                ctx.raw(&Field::from_with_cfg(v.get(i).copied().unwrap_or(0), &cfg))
                            })
                            .collect()
                    };
                    let product = (0..rows)
                        .map(|i| {
                            ctx.raw(
                                &(cfg.add(
                                    &(Field::from_with_cfg(lo.get(i).copied().unwrap_or(0), &cfg)),
                                    &(&(cfg.mul(
                                        &(two128.clone()),
                                        &(&Field::from_with_cfg(
                                            hi.get(i).copied().unwrap_or(0),
                                            &cfg,
                                        )),
                                    ))),
                                )),
                            )
                        })
                        .collect();
                    let projected = RawProducts {
                        az: project(&a),
                        bz: project(&b),
                        cz: product,
                    };
                    let tau: Vec<_> = (0..log - 1)
                        .map(|_| ctx.native_residue_u128(rng.random()))
                        .collect();
                    let split = tau.len() / 2;
                    let low = eq_table_raw(&ctx, &tau[..split]);
                    let high = eq_table_raw(&ctx, &tau[split..]);
                    for endpoint in [FactoredEndpoint::Zero, FactoredEndpoint::One] {
                        let weights = weights_of(&low, &high);
                        assert_eq!(
                            native.round0(&ctx, &reducer, weights, endpoint),
                            cofactor_evaluations_raw(&ctx, &reducer, &projected, weights, endpoint)
                        );
                    }
                    let challenge = ctx.native_residue_u128(rng.random());
                    let mut expected = RawProducts::zeros(rows / 2);
                    let mut actual = RawProducts::zeros(rows / 2);
                    if rows == 2 {
                        native.fold(&ctx, &reducer, &mut actual, challenge, None);
                        fold_products_raw(&ctx, &projected, &mut expected, challenge);
                    } else {
                        let tau = &tau[..tau.len() - 1];
                        let split = tau.len() / 2;
                        let low = eq_table_raw(&ctx, &tau[..split]);
                        let high = eq_table_raw(&ctx, &tau[split..]);
                        let weights = weights_of(&low, &high);
                        let want = fold_products_and_cofactor_evaluations_raw(
                            &ctx,
                            &reducer,
                            &projected,
                            &mut expected,
                            challenge,
                            weights,
                            FactoredEndpoint::One,
                        );
                        let got = native.fold(
                            &ctx,
                            &reducer,
                            &mut actual,
                            challenge,
                            Some((weights, FactoredEndpoint::One)),
                        );
                        assert_eq!(got, want);
                    }
                    assert_eq!(actual.az, expected.az);
                    assert_eq!(actual.bz, expected.bz);
                    assert_eq!(actual.cz, expected.cz);
                }
            }
        }
    }
}

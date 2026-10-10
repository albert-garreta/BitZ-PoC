use field::*;

fn native<C>(field: &C)
where
    C: FieldOps
        + IntegerEmbedding<i64>
        + IntegerEmbedding<i128>
        + BatchMulAcc<C::Elem, i64>
        + BatchMulAcc<C::Elem, i128>
        + Reduce<<C as BatchMulAcc<C::Elem, i64>>::Accumulator, Output = C::Elem>
        + Reduce<<C as BatchMulAcc<C::Elem, i128>>::Accumulator, Output = C::Elem>,
    C::Elem: core::fmt::Debug + Eq,
{
    let weights = [
        field.one(),
        field.neg(&field.one()),
        field.from_integer(&19i64),
        field.one(),
    ];
    let small = [i64::MIN, i64::MAX, -17, 0];
    let wide = [i128::MIN, i128::MAX, -17, 0];
    let reference_small = weights.iter().zip(small).fold(field.zero(), |sum, (w, v)| {
        field.add(&sum, &field.mul(w, &field.from_integer(&v)))
    });
    let reference_wide = weights.iter().zip(wide).fold(field.zero(), |sum, (w, v)| {
        field.add(&sum, &field.mul(w, &field.from_integer(&v)))
    });
    assert_eq!(
        field.reduce(field.batch_mul_acc(&weights, &small)),
        reference_small
    );
    assert_eq!(
        field.reduce(field.batch_mul_acc(&weights, &wide)),
        reference_wide
    );
    assert_eq!(
        field.reduce(field.batch_mul_acc_map(4, |i| (weights[i], small[i]))),
        reference_small
    );
    assert_eq!(
        field.reduce(field.batch_mul_acc_map(4, |i| (weights[i], wide[i]))),
        reference_wide
    );
}

#[test]
fn native_signed_mac_preserves_runtime_and_static_scales() {
    native(&FpCtx::from_prime_u128((1u128 << 100) - 15));
    native(&FpCtx::from_prime_u128(u128::MAX - 158));
    native(&StaticFpOps::<Q100Prime, 2>::new());
}

#[test]
fn signed_words_preserve_declared_width_and_reject_narrowing() {
    let narrowed = Z::<1>::checked_from_twos_complement_words_ct(&[u64::MAX, u64::MAX]);
    assert!(narrowed.validity().declassify());
    assert_eq!(narrowed.value().as_words(), &[u64::MAX]);
    let minimum = [0, 1 << 63];
    for words in [minimum, [1 << 63, 0], [0, u64::MAX]] {
        assert!(
            !Z::<1>::checked_from_twos_complement_words_ct(&words)
                .validity()
                .declassify()
        );
    }
    assert_eq!(
        Z::<3>::checked_from_twos_complement_words_ct(&minimum)
            .value()
            .as_words(),
        &[0, 1 << 63, u64::MAX]
    );
}

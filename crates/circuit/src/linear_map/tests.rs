use super::*;
use crate::integer_storage::IntegerTable;
use field::{Gf128Ops, IntegerEmbedding, RingOps, Uint, Z, create_prime_field};

#[test]
fn integer_field_and_explicit_matrix_agree() {
    for p in [97u64, 101] {
        let f = create_prime_field(Uint::<1>::from_words([p]));
        let mut b = WengertBuilder::new(Vec::<i64>::new());
        let x = b.input();
        let y = b.input();
        let _unused = b.input();
        let a = b.linear_combination([(x, 3), (y, 2)]);
        let c = b.sub(x, y);
        b.output(a);
        b.output(c);
        b.output(a);
        let tape = b.finish();
        let w = [5u64, 7, 11].map(|v| f.from_integer(&v));
        let xs = [13u64, 17, 19].map(|v| f.from_integer(&v));
        let mut prepared = tape.prepare(&f);
        let mut out = vec![f.zero(); 3];
        prepared.adjoint_into(&w, &mut out).unwrap();
        assert_eq!(
            out,
            [f.from_integer(&55u64), f.from_integer(&25u64), f.zero()]
        );
        let forward = prepared
            .evaluate_bilinear(&w, &DenseColumns::new(&f, &xs))
            .unwrap();
        let dot = out
            .iter()
            .zip(xs)
            .fold(f.zero(), |s, (a, b)| f.add(&s, &f.mul(a, &b)));
        assert_eq!(forward, dot);
        let mut fb = WengertBuilder::new(FieldCoefficients::new(&f));
        let x = fb.input();
        let y = fb.input();
        fb.input();
        let a = fb.linear_combination([(x, f.from_integer(&3u64)), (y, f.from_integer(&2u64))]);
        let c = fb.sub(x, y);
        fb.output(a);
        fb.output(c);
        fb.output(a);
        let ft = fb.finish();
        ft.prepare().adjoint_into(&w, &mut out).unwrap();
        assert_eq!(
            forward,
            ft.prepare()
                .evaluate_bilinear(&w, &DenseColumns::new(&f, &xs))
                .unwrap()
        );
        prepared.adjoint_into(&[f.zero(); 3], &mut out).unwrap();
        assert_eq!(out, vec![f.zero(); 3]);
    }
}

#[test]
fn mixed_widths_and_nested_labels_do_not_wrap() {
    let mut b = WengertBuilder::new(IntegerTable::default());
    let x = b.input();
    let max = Z::<1>::from_twos_complement_words([i64::MAX as u64]);
    let wide = Z::<4>::from_twos_complement_words([5, 1, 0, 0]);
    let a = b.scale(x, max);
    let a = b.scale(a, Z::<1>::from(2u64));
    let c = b.scale(x, wide);
    let sum = b.add(a, c);
    b.output(sum);
    let tape = b.finish();
    assert_eq!(
        tape.coefficients()
            .iter()
            .map(|w| w.len())
            .collect::<Vec<_>>(),
        [1, 1, 4]
    );
    for p in [97u64, 101] {
        let f = create_prime_field(Uint::<1>::from_words([p]));
        let expected = f.add(
            &f.mul(&f.from_integer(&max), &f.from_integer(&2u64)),
            &f.from_integer(&wide),
        );
        let mut out = [f.zero()];
        tape.prepare(&f).adjoint_into(&[f.one()], &mut out).unwrap();
        assert_eq!(out, [expected]);
    }
}

#[test]
fn packed_full_low_zero_and_alias_boundaries() {
    let f = create_prime_field(Uint::<1>::from_words([97]));
    for bits in [1, 2, 7, 32, 33, 65, 4099, 8201] {
        for low in [0, 1, bits / 2, bits - 1, bits] {
            let mut b = WengertBuilder::new(Vec::<u64>::new());
            let unused = b.input();
            let packed = b.packed_inputs(bits, low);
            b.output(packed.full);
            b.output(packed.low);
            let zero = b.zero();
            b.output(zero);
            let _dead = b.scale(unused, 99);
            let tape = b.finish();
            let mut prepared = tape.prepare(&f);
            let w = [
                f.from_integer(&3u64),
                f.from_integer(&5u64),
                f.from_integer(&7u64),
            ];
            let mut out = vec![f.one(); bits + 1];
            prepared.adjoint_into(&w, &mut out).unwrap();
            assert_eq!(out[0], f.zero());
            let mut pow = f.one();
            for i in 0..bits {
                assert_eq!(
                    out[1 + i],
                    f.mul(&pow, &f.from_integer(&if i < low { 8u64 } else { 3u64 }))
                );
                pow = f.add(&pow, &pow);
            }
            let xs = (0..=bits)
                .map(|i| f.from_integer(&(i as u64 + 1)))
                .collect::<Vec<_>>();
            let expected = out
                .iter()
                .zip(&xs)
                .fold(f.zero(), |s, (a, b)| f.add(&s, &f.mul(a, b)));
            assert_eq!(
                prepared
                    .evaluate_bilinear(&w, &DenseColumns::new(&f, &xs))
                    .unwrap(),
                expected
            );
            for run in prepared.power_runs() {
                let mut value = run.base;
                for i in 0..run.len {
                    assert_eq!(out[run.first_column + i], value);
                    value = f.add(&value, &value);
                }
            }
        }
    }
}

#[test]
fn binary_field_coefficients_and_zero_domain() {
    let f = Gf128Ops;
    let mut b = WengertBuilder::new(FieldCoefficients::new(&f));
    let x = b.input();
    let y = b.scale(x, f.one());
    let z = b.sub(y, x);
    b.output(z);
    let tape = b.finish();
    let mut out = [f.one()];
    tape.prepare().adjoint_into(&[f.one()], &mut out).unwrap();
    assert_eq!(out, [f.zero()]);
    let b = WengertBuilder::new(Vec::<u64>::new());
    let tape = b.finish();
    let prime = create_prime_field(Uint::<1>::from_words([97]));
    let mut p = tape.prepare(&prime);
    p.adjoint_into(&[], &mut []).unwrap();
    assert_eq!(
        p.evaluate_bilinear(&[], &DenseColumns::new(&prime, &[]))
            .unwrap(),
        prime.zero()
    );
}

#[test]
#[should_panic(expected = "another Wengert builder")]
fn rejects_foreign_nodes() {
    let mut a = WengertBuilder::new(Vec::<u64>::new());
    let mut b = WengertBuilder::new(Vec::<u64>::new());
    b.output(a.input());
}

#[test]
fn rejects_wrong_shapes_before_mutation() {
    let f = create_prime_field(Uint::<1>::from_words([97]));
    let mut b = WengertBuilder::new(Vec::<u64>::new());
    let x = b.input();
    b.output(x);
    let tape = b.finish();
    let mut p = tape.prepare(&f);
    let mut out = [f.one()];
    assert!(p.adjoint_into(&[], &mut out).is_err());
    assert_eq!(out, [f.one()]);
    assert!(
        p.evaluate_bilinear(&[f.one()], &DenseColumns::new(&f, &[]))
            .is_err()
    );
}

#[test]
fn sparse_mixed_mac_matches_field_embedding_and_bilinear_identity() {
    use super::contraction::{PreparedSparse, signed_native_into};
    let f = create_prime_field(Uint::<2>::from(97u64));
    let rows = vec![vec![(0, 1u64), (2, u64::MAX)], vec![(0, 7), (1, 9)], vec![]];
    let matrix = SparseMatrix::try_from_rows(4, rows).unwrap();
    let embedded = SparseMatrix::from_topology(
        matrix.topology().clone(),
        matrix
            .coefficients()
            .iter()
            .map(|x| f.from_integer(x))
            .collect(),
    )
    .unwrap();
    let w = [2u64, 3, 5].map(|x| f.from_integer(&x));
    let x = [7u64, 11, 13, 17].map(|x| f.from_integer(&x));
    let mut out = [f.zero(); 4];
    let mut expected = out;
    PreparedSparse::new(&f, &matrix)
        .adjoint_into(&w, &mut out, false)
        .unwrap();
    PreparedSparse::new(&f, &embedded)
        .adjoint_into(&w, &mut expected, true)
        .unwrap();
    assert_eq!(out, expected);
    let dot = out
        .iter()
        .zip(x)
        .fold(f.zero(), |s, (a, b)| f.add(&s, &f.mul(a, &b)));
    assert_eq!(
        PreparedSparse::new(&f, &matrix)
            .evaluate_bilinear(&w, &DenseColumns::new(&f, &x))
            .unwrap(),
        dot
    );
    let signed = SparseMatrix::try_from_rows(
        4,
        vec![vec![(0, -1), (2, i64::MIN)], vec![(0, 1), (1, 9)], vec![]],
    )
    .unwrap();
    signed_native_into(&f, &signed, &w, &mut out, false);
    for (j, column) in signed.columns().enumerate() {
        let value = column.into_iter().fold(f.zero(), |s, (row, c)| {
            f.add(&s, &f.mul(&w[row], &f.from_integer(c)))
        });
        assert_eq!(out[j], value);
    }
    assert!(
        PreparedSparse::new(&f, &matrix)
            .adjoint_into(&w[..2], &mut out, false)
            .is_err()
    );
}

#[test]
fn delayed_graph_reduction_matches_immediate_and_chunks_large_nodes() {
    // Each output seeds one shared input. Fan-out exceeds one chunk so the
    // adjoint must reduce before accumulating the remaining contributions.
    for prime in [97u128, (1u128 << 127) - 1] {
        let f = create_prime_field(Uint::<2>::from(prime));
        let mut b = WengertBuilder::new(Vec::<u64>::new());
        let x = b.input();
        for i in 0..65543u64 {
            let v = b.scale(x, i + 2);
            b.output(v);
        }
        let tape = b.finish();
        let weights = f
            .zero_vec(tape.output_count())
            .into_iter()
            .enumerate()
            .map(|(i, _)| f.from_integer(&(i as u64 + 7)))
            .collect::<Vec<_>>();
        let mut p = tape.prepare(&f);
        let mut ordinary = [f.zero()];
        let mut delayed = ordinary;
        p.adjoint_into(&weights, &mut ordinary).unwrap();
        p.adjoint_delayed_map_storage_into(weights.len(), |i| weights[i], &mut delayed, |x| x)
            .unwrap();
        assert_eq!(ordinary, delayed);
    }
}

#[test]
fn binary_adjoint_streams_unaligned_ranges_without_materializing_weights() {
    use super::binary::PreparedVirtualMap;
    use super::binary_adjoint::BinaryAdjoint;
    let matrix =
        SparseMatrix::try_from_rows(256, (0..256).map(|i| vec![(i, true)]).collect()).unwrap();
    let map = PreparedVirtualMap::new(matrix.clone()).unwrap();
    assert_eq!(
        map,
        PreparedVirtualMap::from_topology(matrix.into_parts().0).unwrap()
    );
    let points = vec![
        (0..8)
            .map(|i| field::Gf128::from_polynomial_words([i + 2, 0]))
            .collect::<Vec<_>>(),
    ];
    let eta = [field::Gf128::one()];
    let equality = |p: &[field::Gf128], _: &()| {
        let mut v = vec![field::Gf128::zero(); 1 << p.len()];
        v[0] = field::Gf128::one();
        for (bit, r) in p.iter().enumerate() {
            for i in 0..1 << bit {
                let t = v[i] * r;
                v[i] += t;
                v[i + (1 << bit)] = t;
            }
        }
        Ok(v)
    };
    let weights = BinaryAdjoint::new_with_tail(&map, &points, &eta, 4, false, equality, false);
    let expected = equality(&points[0], &()).unwrap();
    assert_eq!(weights.column_count(), map.cols());
    for (first, len) in [(0, 256), (1, 254), (7, 130), (255, 1), (256, 0)] {
        let mut out = vec![field::Gf128::zero(); len];
        weights.fill_range(first, &mut out).unwrap();
        assert_eq!(out, expected[first..first + len]);
    }
    let mut out = [field::Gf128::one(); 2];
    assert!(weights.fill_range(255, &mut out).is_err());
    assert_eq!(out, [field::Gf128::one(); 2]);
}

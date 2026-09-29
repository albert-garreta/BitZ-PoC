use super::*;
use crate::{
    piop::spartan::protocol::{self, PreparedRelation, RelationSpec},
    transcript::Blake3Transcript,
};

fn check_packing<T: MulWord>(n: usize, shift: i8, row: MulRow<T>) {
    let layout = MulLayout::<T>::new(n)
        .unwrap()
        .with_split_shift(shift)
        .unwrap();
    let witness = MulWitness::from_row_fn(layout, |_| row).unwrap();
    let p = layout.bitz_params();
    let mut expected = vec![vec![0u64; p.rows() / 64]; p.cols()];
    let high = 1 << (layout.gate_vars() - layout.col_vars());
    for gate in 0..n {
        for (limb, value) in [row.x, row.y, row.lo, row.hi].into_iter().enumerate() {
            for bit in 0..T::BITS {
                let index = (limb * T::BITS + bit) * high + gate / p.cols();
                expected[gate % p.cols()][index / 64] |=
                    ((value.as_u128() >> bit & 1) as u64) << (index % 64);
            }
        }
    }
    assert_eq!(witness.bitz_bit_rows(), expected);
    let mut reused = expected.clone();
    for row in &mut reused {
        row.fill(u64::MAX);
    }
    witness.write_bitz_bit_rows(&mut reused).unwrap();
    assert_eq!(reused, expected);
    let zeros = MulWitness::from_fn_with_layout(layout, |_| (T::default(), T::default())).unwrap();
    zeros.write_bitz_bit_rows(&mut reused).unwrap();
    assert!(reused.iter().flatten().all(|&word| word == 0));
    reused[0].pop();
    let malformed = reused.clone();
    assert_eq!(
        witness.write_bitz_bit_rows(&mut reused),
        Err(MulError::InvalidRowShape)
    );
    assert_eq!(reused, malformed);
}

#[test]
fn native_packing_matches_reference_and_overwrites_reused_rows() {
    // Shifting across six high gate coordinates exercises both the fast
    // transpose and the necessary small-shape bitwise fallback.
    for n in [37, 257, 2053, 4097] {
        for shift in [-1, 0, 1, 2] {
            check_packing(
                n,
                shift,
                MulRow {
                    x: u32::MAX,
                    y: 0x80102041,
                    lo: 0xfefe1313,
                    hi: 0xfeedface,
                },
            );
            check_packing(
                n,
                shift,
                MulRow {
                    x: u64::MAX,
                    y: 0x8010204180808081,
                    lo: 0xfefe131341341234,
                    hi: 0xfeedface87654321,
                },
            );
            check_packing(
                n,
                shift,
                MulRow {
                    x: u128::MAX,
                    y: (1_u128 << 127) | 0x123456789abcdef,
                    lo: 0xfefe131341341234,
                    hi: (1_u128 << 120) | 0xfeedface87654321,
                },
            );
        }
    }
}

#[test]
fn packing_bounds_reject_overflow_and_invalid_splits() {
    assert!(MulLayout::<u64>::new(0).is_err());
    assert!(MulLayout::<u64>::new(usize::MAX).is_err());
    assert!(
        MulLayout::<u64>::new(1 << 24)
            .unwrap()
            .with_split_shift(30)
            .is_err()
    );
    assert!(
        MulLayout::<u64>::new(1 << 24)
            .unwrap()
            .with_split_shift(-30)
            .is_err()
    );
}
#[test]
fn large_partial_batches_preserve_limbs_and_zero_padding() {
    fn check<T: MulWord>(n: usize, x: T, y: T) {
        let witness = MulWitness::from_fn(n, |_| (x, y)).unwrap();
        let (lo, hi) = T::multiply(x, y);
        for (values, expected) in [
            (witness.x_values(), x),
            (witness.y_values(), y),
            (witness.z_lo_values(), lo),
            (witness.z_hi_values(), hi),
        ] {
            assert_eq!(values.len(), witness.layout().capacity());
            assert!(values[..n].iter().all(|&v| v == expected));
            assert!(values[n..].iter().all(|&v| v == T::default()));
        }
    }
    check((1 << 20) + 1, u32::MAX, u32::MAX);
    check((1 << 19) + 1, u64::MAX, u64::MAX);
    check((1 << 18) + 1, u128::MAX, u128::MAX);
}
fn roundtrip<T: MulWord>(input: impl FnMut(usize) -> (T, T))
where
    MulLayout<T>: RelationSpec<Witness = MulWitness<T>>,
{
    let witness = MulWitness::from_fn(1 << 15, input).unwrap();
    let prepared = PreparedRelation::new(*witness.layout()).unwrap();
    let hint = protocol::commit(&prepared, witness.bitz_bit_rows()).unwrap();
    let proof = protocol::prove(&mut Blake3Transcript::new(), &prepared, &witness, &hint).unwrap();
    protocol::verify(
        &mut Blake3Transcript::new(),
        &prepared,
        &hint.commitment,
        &proof,
    )
    .unwrap();
    let mut wrong_root = hint.commitment.clone();
    wrong_root.root[0] ^= 1;
    assert!(
        protocol::verify(&mut Blake3Transcript::new(), &prepared, &wrong_root, &proof).is_err()
    );
    let mut rows: Vec<_> = witness.rows().collect();
    rows[3].hi = T::default();
    let bad = MulWitness::from_rows_with_layout(*witness.layout(), &rows).unwrap();
    assert_ne!(bad, witness);
    let bad_hint = protocol::commit(&prepared, bad.bitz_bit_rows()).unwrap();
    if let Ok(bad_proof) = protocol::prove(&mut Blake3Transcript::new(), &prepared, &bad, &bad_hint)
    {
        assert!(
            protocol::verify(
                &mut Blake3Transcript::new(),
                &prepared,
                &bad_hint.commitment,
                &bad_proof
            )
            .is_err()
        );
    }
    let wrong_layout = MulLayout::<T>::new(1 << 15)
        .unwrap()
        .with_split_shift(1)
        .unwrap();
    let wrong_prepared = PreparedRelation::new(wrong_layout).unwrap();
    assert!(
        protocol::verify(
            &mut Blake3Transcript::new(),
            &wrong_prepared,
            &hint.commitment,
            &proof
        )
        .is_err()
    );
}
#[test]
fn direct_proofs_reject_wrong_roots_limbs_and_layouts() {
    roundtrip(|i| (u32::MAX - i as u32, u32::MAX));
    roundtrip(|i| (u64::MAX - i as u64, u64::MAX));
    roundtrip(|i| (u128::MAX - i as u128, u128::MAX));
}

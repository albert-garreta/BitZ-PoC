#![cfg(feature = "bitz-parity")]

//! The direct BitZ entry point must bind its initial claim before sampling the
//! fold batching point. Otherwise the prover can choose claim weights after
//! seeing that point and make false folds agree with the GKR root.

use bitz::pcs::smallest_generator;
use bitz::wfbitz::gkr::{GrandProductCircuit, gpgkr_prove};
use bitz::wfbitz::params::LinearClaimGf;
use bitz::wfbitz::{
    BitZParams, BitZProver, BitZVerifier, LinearClaim, OpeningQuery, Pcs, Shape, StatementBinding,
    WINDOW, build_prover, build_verifier,
};
use field::Gf128 as Gf;
use flock_core::merkle::HashKind;

fn eq_table(point: &[Gf]) -> Vec<Gf> {
    let mut result = vec![Gf::one()];
    for &z in point {
        let len = result.len();
        for i in 0..len {
            let old = result[i];
            result[i] = old * (Gf::one() - z);
            result.push(old * z);
        }
    }
    result
}

/// Return a nonzero binary relation among 256 elements of a 128-bit field.
fn first_relation(values: &[Gf]) -> [u64; 4] {
    assert_eq!(values.len(), 256);
    let mut basis: [Option<(u128, [u64; 4])>; 128] = [None; 128];
    for (i, &field) in values.iter().enumerate() {
        let mut value = u128::from_le_bytes(field.to_bytes());
        let mut mask = [0u64; 4];
        mask[i / 64] ^= 1u64 << (i % 64);
        while value != 0 {
            let pivot = value.ilog2() as usize;
            if let Some((b, m)) = basis[pivot] {
                value ^= b;
                for (a, x) in mask.iter_mut().zip(m) {
                    *a ^= x;
                }
            } else {
                basis[pivot] = Some((value, mask));
                break;
            }
        }
        if value == 0 {
            assert!(mask.iter().any(|&x| x != 0));
            return mask;
        }
    }
    unreachable!("256 vectors in a 128-dimensional space must be dependent")
}

#[test]
fn direct_bitz_rejects_claim_chosen_after_fold_challenge() {
    let shape = Shape::new(14, 8).unwrap();
    let params = BitZParams::new(shape, 17, smallest_generator().into()).unwrap();
    let pcs = Pcs::new(&shape, HashKind::Blake3).unwrap();
    let cols = shape.columns();
    let rows_count = shape.rows();

    // Bit (row c, column c) is set for each c in 0..256.
    let mut rows = vec![vec![0u64; rows_count / 64]; cols];
    for c in 0..cols {
        rows[c][c / 64] |= 1u64 << (c % 64);
    }
    let (root, hint) = pcs.commit(&shape, rows).unwrap();

    let session = b"direct-bit-z-false-fold" as &[u8];
    let instance = b"one" as &[u8];
    let mut malicious = build_prover(session, instance);
    malicious.public_message(&root.0);
    malicious.public_message(&params);
    for _ in 0..cols {
        malicious.prover_message(&0u128.to_le_bytes());
    }
    let zeta: Vec<Gf> = (0..shape.log_columns())
        .map(|_| malicious.verifier_message::<Gf>())
        .collect();
    let column_eq = eq_table(&zeta);
    let relation = first_relation(&column_eq);

    // The relation makes the honest batched GKR root equal the all-zero-fold
    // root, although selecting any set diagonal bit gives weighted sum one.
    let mut row_weights = vec![0u128; rows_count];
    let mut relation_sum = Gf::zero();
    let mut chosen = None;
    for c in 0..cols {
        if relation[c / 64] >> (c % 64) & 1 != 0 {
            row_weights[c] = 1;
            relation_sum += column_eq[c];
            chosen = Some(c);
        }
    }
    assert_eq!(relation_sum, Gf::zero());
    let chosen = chosen.unwrap();
    let mut column_weights = vec![0u128; cols];
    column_weights[chosen] = 1;
    assert_eq!(row_weights[chosen] * column_weights[chosen], 1);
    let false_claim =
        LinearClaim::new(&params, row_weights.clone(), column_weights.clone(), 0).unwrap();

    let generator = params.generator();
    let mut leaves = vec![Gf::one(); rows_count * cols];
    for c in 0..cols {
        if row_weights[c] == 1 {
            leaves[c * cols + c] = generator;
        }
    }
    let (roots, witnesses) = GrandProductCircuit::new(leaves).batched_eval(cols);
    let actual_e0: Gf = roots.iter().zip(&column_eq).map(|(&v, &e)| v * e).sum();
    assert_eq!(actual_e0, Gf::one());
    let (terminal_point, terminal_claim) = gpgkr_prove(&mut malicious, &zeta, witnesses);
    let (alpha_c, alpha_b) = terminal_point.split_at(shape.log_columns());
    let row_factor: Vec<Gf> = eq_table(alpha_b)
        .iter()
        .zip(&row_weights)
        .map(|(&weight, &b)| {
            if b == 1 {
                (generator - Gf::one()) * weight
            } else {
                Gf::zero()
            }
        })
        .collect();
    let query = LinearClaimGf::from_shape(
        &shape,
        row_factor,
        eq_table(alpha_c),
        terminal_claim - Gf::one(),
    )
    .unwrap();
    pcs.prove_lin(
        &hint,
        &OpeningQuery::InnerProduct { claim: query },
        StatementBinding::Bind,
        &mut malicious,
        None,
    )
    .unwrap();
    let forged_proof = malicious.finish();

    let verifier = BitZVerifier::new(params, WINDOW);
    assert!(
        verifier
            .verify(
                &false_claim,
                &pcs,
                root,
                build_verifier(session, instance, &forged_proof),
                None,
            )
            .is_err(),
        "a committed bit has weighted sum 1 but the claim says 0"
    );

    // A valid claim for exactly the same commitment must still roundtrip.
    let true_claim = LinearClaim::new(&params, row_weights, column_weights, 1).unwrap();
    let mut honest = build_prover(session, instance);
    BitZProver::new(params, WINDOW)
        .prove(&true_claim, &pcs, &hint, &mut honest, None)
        .unwrap();
    let honest_proof = honest.finish();
    verifier
        .verify(
            &true_claim,
            &pcs,
            root,
            build_verifier(session, instance, &honest_proof),
            None,
        )
        .unwrap();
}

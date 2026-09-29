//! The direct entry point binds every component of the initial linear claim.
//! A proof cannot be reused with other weights, even when both claims evaluate
//! to the same value on the committed witness.

use bitz::pcs::smallest_generator;
use bitz::wfbitz::{
    BitZParams, BitZProver, BitZVerifier, LinearClaim, Pcs, Shape, WINDOW, build_prover,
    build_verifier,
};
use flock_core::merkle::HashKind;

#[test]
fn direct_bitz_rejects_changed_initial_claim() {
    let shape = Shape::new(14, 8).unwrap();
    let params = BitZParams::new(shape, 17, smallest_generator().into()).unwrap();
    let pcs = Pcs::new(&shape, HashKind::Blake3).unwrap();
    let mut rows = vec![vec![0u64; shape.rows() / 64]; shape.columns()];
    for (column, row) in rows.iter_mut().enumerate() {
        row[column / 64] |= 1u64 << (column % 64);
    }
    let (root, hint) = pcs.commit(&shape, rows).unwrap();
    let row_weights = vec![1; shape.rows()];
    let mut column_weights = vec![0; shape.columns()];
    column_weights[0] = 1;
    let claim = LinearClaim::new(&params, row_weights.clone(), column_weights.clone(), 1).unwrap();
    let mut prover = build_prover(b"direct-claim-binding", b"diagonal");
    BitZProver::new(params, WINDOW)
        .prove(&claim, &pcs, &hint, &mut prover, None)
        .unwrap();
    let proof = prover.finish();
    let verifier = BitZVerifier::new(params, WINDOW);
    let verify = |claim: &LinearClaim| {
        verifier.verify(
            claim,
            &pcs,
            root,
            build_verifier(b"direct-claim-binding", b"diagonal", &proof),
            None,
        )
    };
    verify(&claim).unwrap();
    // These two statements are both true, but each has different weights.
    let mut other_rows = row_weights.clone();
    other_rows[1] = 2;
    let mut other_columns = column_weights.clone();
    other_columns.swap(0, 1);
    for changed in [
        LinearClaim::new(&params, other_rows, column_weights.clone(), 1).unwrap(),
        LinearClaim::new(&params, row_weights.clone(), other_columns, 1).unwrap(),
        LinearClaim::new(&params, row_weights, column_weights, 0).unwrap(),
    ] {
        assert!(verify(&changed).is_err());
    }
}

//! BitZ completeness against an independent modular MLE reference.
//! Small ad-hoc configurations test algebra, not security parameters.
#[path = "common/bitz.rs"]
mod reference;
use bitz::bitz::{
    BitZParams, BitZProver, BitZVerifier, LinearClaim, Shape, WINDOW, build_prover, build_verifier,
};
use reference::*;

#[test]
fn randomized_mle_claims_and_transcript_rejection() {
    let mut seed = 0x52da_71f3_aceb_9123;
    for (t, s) in [(7, 8), (8, 7), (15, 0), (10, 6), (7, 10)] {
        let shape = Shape::new(t, s).unwrap();
        let pcs = pcs(shape);
        for q in [3, 257, (1u128 << 61) - 1, (1u128 << 100) - 15] {
            for class in 0..3 {
                let rows: Vec<Vec<u64>> = (0..shape.columns())
                    .map(|_| {
                        (0..shape.rows() / 64)
                            .map(|_| match class {
                                0 => 0,
                                1 => u64::MAX,
                                _ => next(&mut seed),
                            })
                            .collect()
                    })
                    .collect();
                // Boolean points exercise the former zero-weight degeneracies.
                let point: Vec<u128> = (0..shape.log_bits())
                    .map(|_| {
                        let x = u128::from(next(&mut seed));
                        if class == 1 { x & 1 } else { x % q }
                    })
                    .collect();
                let rw: Vec<_> = (0..shape.rows()).map(|i| eq(i, &point[..t], q)).collect();
                let cw: Vec<_> = (0..shape.columns())
                    .map(|i| eq(i, &point[t..], q))
                    .collect();
                let y = target(&rows, shape, &rw, &cw, q);
                let direct = (0..1usize << shape.log_bits())
                    .filter(|&i| bit(&rows, shape, i))
                    .fold(0, |sum, i| add(sum, eq(i, &point, q), q));
                assert_eq!(y, direct);
                let params = BitZParams::new(shape, q, bitz::pcs::smallest_generator()).unwrap();
                let claim = LinearClaim::new(&params, rw.clone(), cw.clone(), y).unwrap();
                let (root, hint) = pcs.commit(&shape, rows).unwrap();
                let prover = BitZProver::new(params, WINDOW);
                let verifier = BitZVerifier::new(params, WINDOW);
                let mut state = build_prover("completeness", "seeded");
                prover.prove(&claim, &pcs, &hint, &mut state, None).unwrap();
                let proof = state.finish();
                let verify = |claim: &LinearClaim, proof: &bitz::bitz::Proof| {
                    verifier.verify(
                        claim,
                        &pcs,
                        root,
                        build_verifier("completeness", "seeded", proof),
                        None,
                    )
                };
                verify(&claim, &proof).unwrap();
                let wrong = LinearClaim::new(&params, rw.clone(), cw.clone(), (y + 1) % q).unwrap();
                assert!(verify(&wrong, &proof).is_err());
                let mut changed_rw = rw;
                changed_rw[0] = (changed_rw[0] + 1) % q;
                let changed = LinearClaim::new(&params, changed_rw, cw, y).unwrap();
                assert!(verify(&changed, &proof).is_err());
                let mut bad = proof.clone();
                bad.narg_string[0] ^= 1;
                assert!(verify(&claim, &bad).is_err());
                let mut bad = proof.clone();
                bad.narg_string.push(0);
                assert!(verify(&claim, &bad).is_err());
                let mut bad = proof.clone();
                bad.hints.push(0);
                assert!(verify(&claim, &bad).is_err());
                let mut wrong_root = root;
                wrong_root.0[0] ^= 1;
                assert!(
                    verifier
                        .verify(
                            &claim,
                            &pcs,
                            wrong_root,
                            build_verifier("completeness", "seeded", &proof),
                            None
                        )
                        .is_err()
                );
            }
        }
    }
}

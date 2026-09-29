//! Virtual Wfbitz claims checked against an independent application of M over F2.
#[path = "common/wfbitz.rs"]
mod reference;
use bitz::wfbitz::{
    BitZParams, BitZProver, BitZVerifier, LinearClaim, Shape, WINDOW, build_prover, build_verifier,
    virt::VirtualStatement,
};
use circuit::linear_map::{CscMatrix, binary::PreparedVirtualMap};
use reference::*;

#[test]
fn virtual_maps_roundtrip_and_reject_substitution() {
    const Q: u128 = (1 << 100) - 15;
    let committed = Shape::new(8, 7).unwrap();
    let cells = 1usize << committed.log_bits();
    let pcs = pcs(committed);
    let mut seed = 0x921a_1211_f493_7cc5;
    let rows: Vec<Vec<u64>> = (0..committed.columns())
        .map(|_| {
            (0..committed.rows() / 64)
                .map(|_| next(&mut seed))
                .collect()
        })
        .collect();
    let (root, hint) = pcs.commit(&committed, rows.clone()).unwrap();
    for kind in 0..5 {
        let derived = if kind == 0 {
            committed
        } else {
            Shape::new(7, 8).unwrap()
        };
        let mut h = vec![vec![0u64; derived.rows() / 64]; derived.columns()];
        let mut offsets = vec![0];
        let mut indices = Vec::new();
        for source in 0..cells {
            let mut destinations = match kind {
                0 => vec![source],
                1 => vec![(source * 13 + 7) % cells, source],
                2 if source == 0 => vec![0],
                3 => vec![],
                4 if source < cells - 1 => vec![source],
                _ => vec![],
            };
            destinations.sort_unstable();
            destinations.dedup();
            for &d in &destinations {
                if bit(&rows, committed, source) {
                    h[d >> derived.log_rows()][(d & (derived.rows() - 1)) / 64] ^= 1 << (d % 64);
                }
            }
            indices.extend(destinations);
            offsets.push(indices.len());
        }
        let map = PreparedVirtualMap::from_implicit(
            CscMatrix::try_from_binary_csc(cells, offsets, indices).unwrap(),
        )
        .unwrap();
        let params = BitZParams::new(derived, Q, bitz::pcs::smallest_generator()).unwrap();
        let rw: Vec<_> = (0..derived.rows())
            .map(|_| u128::from(next(&mut seed)) % Q)
            .collect();
        let cw: Vec<_> = (0..derived.columns())
            .map(|_| u128::from(next(&mut seed)) % Q)
            .collect();
        let y = target(&h, derived, &rw, &cw, Q);
        let claim = LinearClaim::new(&params, rw.clone(), cw.clone(), y).unwrap();
        let statement = VirtualStatement::new(params, committed, &map, &claim).unwrap();
        assert_eq!(statement.is_direct(), kind == 0);
        let prover = BitZProver::new(params, WINDOW);
        let verifier = BitZVerifier::new(params, WINDOW);
        let mut state = build_prover("virtual", "seeded");
        prover
            .prove_virtual(&statement, &pcs, &hint, &h, &mut state, None)
            .unwrap();
        let proof = state.finish();
        verifier
            .verify_virtual(
                &statement,
                &pcs,
                root,
                build_verifier("virtual", "seeded", &proof),
                None,
            )
            .unwrap();
        let wrong = LinearClaim::new(&params, rw, cw, (y + 1) % Q).unwrap();
        let wrong_statement = VirtualStatement::new(params, committed, &map, &wrong).unwrap();
        assert!(
            verifier
                .verify_virtual(
                    &wrong_statement,
                    &pcs,
                    root,
                    build_verifier("virtual", "seeded", &proof),
                    None
                )
                .is_err()
        );
        let substitute = PreparedVirtualMap::from_implicit(
            CscMatrix::try_from_binary_csc(
                cells,
                (0..=cells).collect(),
                (0..cells).map(|i| (i + 1) % cells).collect(),
            )
            .unwrap(),
        )
        .unwrap();
        let substituted = VirtualStatement::new(params, committed, &substitute, &claim).unwrap();
        assert!(
            verifier
                .verify_virtual(
                    &substituted,
                    &pcs,
                    root,
                    build_verifier("virtual", "seeded", &proof),
                    None
                )
                .is_err()
        );

        // Also prove the same target against a direct commitment to h.
        let direct_pcs = reference::pcs(derived);
        let (h_root, h_hint) = direct_pcs.commit(&derived, h).unwrap();
        let mut state = build_prover("direct-derived", "seeded");
        prover
            .prove(&claim, &direct_pcs, &h_hint, &mut state, None)
            .unwrap();
        let direct_proof = state.finish();
        verifier
            .verify(
                &claim,
                &direct_pcs,
                h_root,
                build_verifier("direct-derived", "seeded", &direct_proof),
                None,
            )
            .unwrap();
    }
}

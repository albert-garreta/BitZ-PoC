#![recursion_limit = "256"]
//! One recursion step over a batch of algebraic Falcon-512 proofs: the base
//! proof (the `sha256-fs` transcript), its verifier circuit
//! (`bitz::recursion::falcon_verifier`) proved with FlockRec, and the final
//! verifier, which rebuilds the outer relation from the public stream.
//!
//! Run with the configuration-G environment (`BITZ_FALCON_GRINDING_MODEL=
//! per-site BITZ_FALCON_PRIME=small BITZ_ARITHMETIC_RING=keyed
//! BITZ_FALCON_BRIDGE_ROWS=16 BITZ_ARITHMETIC_BINDING_STRATEGY=optimized`) and
//! `--features recursion,falcon-hybrid,bench-internals,unchecked`. Prints one
//! `TIMING {json}` line; medians over the repetitions.
//!
//! Knobs: `BITZ_REC_FALCON_BATCH` (1024), `BITZ_REC_REPS` (3),
//! `BITZ_REC_SECURITY` (128: the base and the outer proof), `BITZ_REC_LIG`
//! (the outer opening: `udrg:3:4` at 128, `custom:6:4` at 100), `BITZ_REC_T`
//! (outer opening rows: 18 at 128, 19 at 100), `BITZ_REC_SEED` (42) and
//! `BITZ_FALCON_CASE_CACHE`.

#[path = "common/falcon_degree_inputs.rs"]
mod falcon_degree_inputs;

use std::time::Instant;

fn env<T: std::str::FromStr>(name: &str, default: T) -> T {
    std::env::var(name).ok().and_then(|v| v.parse().ok()).unwrap_or(default)
}

fn median(mut xs: Vec<f64>) -> f64 {
    xs.sort_by(f64::total_cmp);
    xs[xs.len() / 2]
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    use bitz::piop::spartan::falcon512_algebraic::{FalconAlgebraicStatement, PreparedFalconAlgebraic};
    use bitz::piop::spartan::{
        falcon::{decode_public_key, decode_signature_ct},
        falcon_profiles::n512_k11::hash_to_point_ct,
    };
    use bitz::recursion::ecdsa_verifier::witness_and_record_circuit;
    use bitz::recursion::falcon_verifier as fv;
    use bitz::recursion::val::PublicStream;

    let batch = env("BITZ_REC_FALCON_BATCH", 1024usize);
    let reps = env("BITZ_REC_REPS", 3usize);
    let security = env("BITZ_REC_SECURITY", 128usize);
    let selection = std::env::var("BITZ_REC_LIG")
        .unwrap_or_else(|_| if security == 128 { "udrg:3:4" } else { "custom:6:4" }.into());
    let opening_t = Some(env("BITZ_REC_T", if security == 128 { 18usize } else { 19 }));
    let seed = env("BITZ_REC_SEED", 42u64);
    let threads = rayon::current_num_threads();

    let cases = falcon_degree_inputs::generate_cases(512, batch, seed)?;
    let mut statement = FalconAlgebraicStatement {
        public_keys: Vec::with_capacity(batch),
        targets: Vec::with_capacity(batch),
    };
    let mut s2 = Vec::with_capacity(batch);
    for case in &cases {
        let key = decode_public_key::<512>(&case.public_key)?;
        let signature = decode_signature_ct::<512>(&case.signature)?;
        let target = hash_to_point_ct(&signature.nonce, &case.message)?;
        statement.public_keys.push(*key.h);
        statement.targets.push(*target.point);
        s2.push(*signature.s2);
    }
    drop(cases);
    let started = Instant::now();
    let prepared = PreparedFalconAlgebraic::new(batch, security)?;
    let setup_s = started.elapsed().as_secs_f64();

    // Before: the base proof.
    let mut base = Vec::new();
    let mut last = None;
    for _ in 0..reps {
        let t0 = Instant::now();
        let committed = prepared.commit_from_s2(statement.clone(), s2.clone())?;
        let t1 = Instant::now();
        let proof = prepared.prove(committed)?;
        let t2 = Instant::now();
        prepared.verify(&statement, &proof)?;
        let t3 = Instant::now();
        base.push([t1 - t0, t2 - t1, t2 - t0, t3 - t2].map(|d| d.as_secs_f64()));
        last = Some(proof);
    }
    let proof = last.expect("one repetition");
    let base_bytes = proof.payload_size_bytes();
    let circuit = fv::FalconCircuit::new(&prepared, &statement, &proof);

    // The honest proof satisfies its verifier circuit (untimed check).
    let check_stream = PublicStream::discover();
    let mut check = circuit::witgen::CheckingWitgen::new();
    let counts = fv::verify_falcon_algebraic(
        &mut check,
        &prepared,
        &statement,
        &circuit.root,
        circuit.exceptions,
        Some(&proof),
        check_stream.clone(),
        None,
    );
    assert_eq!(check.first_failure, None, "the base proof satisfies its verifier circuit");
    assert_eq!(check_stream.borrow().failed, None);
    drop(check);
    let stream = check_stream.borrow().clone();
    let mut by_label = stream.bytes_by_label();
    by_label.sort_by_key(|&(_, _, b)| std::cmp::Reverse(b));

    // After: one recursion step, then the final verifier.
    let mut rec = Vec::new();
    let mut ver = Vec::new();
    let mut outer = None;
    for _ in 0..reps {
        let t0 = Instant::now();
        let passes = witness_and_record_circuit(&circuit, |g, blocks| {
            fv::early_commit(g, blocks, &selection, security).expect("early committed rows")
        })?;
        let t1 = Instant::now();
        assert_eq!(passes.stream.values, stream.values, "deterministic public stream");
        let shape = [
            passes.circuit.committed,
            passes.circuit.cells,
            passes.circuit.rows,
            passes.circuit.instances.len(),
        ];
        let rel = fv::outer_relation(
            passes.circuit,
            &statement,
            &circuit.root,
            circuit.exceptions,
            &passes.stream,
            &selection,
            opening_t,
            security,
        )?;
        let t2 = Instant::now();
        let (early, hint) = passes.early;
        assert!(rel.matches_early(&early), "the relation lays out the committed bits as the witness pass did");
        let wit = rel
            .witness_with_committed(early.rows, &passes.h)
            .map_err(|e| format!("{e:?}"))?;
        let t3 = Instant::now();
        let mut tp = bitz::transcript::Sha256Transcript::new();
        let pi = rel.prove(&mut tp, &wit, &hint).map_err(|e| format!("{e:?}"))?;
        let t4 = Instant::now();
        let bits = |l: &bitz::pcs::IntegerMatrixLayout| l.row_vars + l.col_vars;
        let outer_shape = [bits(rel.committed_layout()), bits(rel.derived_layout())];
        drop((wit, rel, passes.g, passes.h));
        rec.push([
            passes.witness_s,
            passes.record_s,
            passes.early_s,
            (t1 - t0).as_secs_f64(),
            (t2 - t1).as_secs_f64(),
            (t3 - t2).as_secs_f64(),
            (t4 - t3).as_secs_f64(),
            (t4 - t0).as_secs_f64(),
        ]);
        let v0 = Instant::now();
        let vrel = fv::replay_relation(
            &prepared,
            &statement,
            &circuit.root,
            circuit.exceptions,
            &stream,
            &selection,
            opening_t,
            security,
        )?;
        let mut tv = bitz::transcript::Sha256Transcript::new();
        vrel.verify(&mut tv, &hint.commitment, &pi).map_err(|e| format!("{e:?}"))?;
        ver.push(v0.elapsed().as_secs_f64());
        outer = Some((shape, outer_shape, hint.commitment, pi));
    }
    let (shape, outer_shape, commitment, pi) = outer.expect("one repetition");
    let pi_bytes = pi.bitz.to_bytes().len() + pi.piop_bytes() + bincode::serialize(&commitment)?.len();
    let colb = |i: usize| median(base.iter().map(|r| r[i]).collect());
    let colr = |i: usize| median(rec.iter().map(|r| r[i]).collect());
    let json = serde_json::json!({
        "batch": batch,
        "threads": threads,
        "reps": reps,
        "security": security,
        "outer_ligerito": selection,
        "outer_t": opening_t,
        "setup_s": setup_s,
        "base_commit_s": colb(0),
        "base_prove_s": colb(1),
        "base_prover_s": colb(2),
        "base_verify_s": colb(3),
        "base_proof_bytes": base_bytes,
        "rec_witness_s": colr(0),
        "rec_record_s": colr(1),
        "rec_early_commit_s": colr(2),
        "rec_passes_s": colr(3),
        "rec_relation_s": colr(4),
        "rec_outer_witness_s": colr(5),
        "rec_protocol_s": colr(6),
        "rec_prove_s": colr(7),
        "final_verify_s": median(ver.clone()),
        "pi_bytes": pi_bytes,
        "public_stream_bytes": stream.bytes(),
        "public_input_bytes": fv::public_input_bytes(&proof),
        "public_top": by_label.iter().take(8).map(|(l, n, b)| format!("{l}:{n}:{b}")).collect::<Vec<_>>(),
        "circuit": {"committed": shape[0], "cells": shape[1], "rows": shape[2], "blocks": shape[3], "outer_f_bits": outer_shape[0], "outer_h_bits": outer_shape[1]},
        "counts": format!("{counts:?}"),
        "base_runs": base,
        "rec_runs": rec,
        "verify_runs": ver,
    });
    println!("TIMING {json}");
    Ok(())
}

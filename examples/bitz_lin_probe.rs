//! FEASIBILITY PROBE (throwaway): the e2e's transposed query shape — a
//! single-column `LinearClaim<F128>` over `Shape::new(22, 0)` on 2^22
//! committed bits — re-proved through our `bitz::Pcs` from a `dump_lin`
//! dump (their `crates/tests/examples/dump_lin.rs`), bytes compared, both
//! proofs verified by our verifier.
//!
//! `bitz_lin_probe <dump-dir>`
use std::collections::HashMap;
use std::time::Instant;

use f2z::bitz::params::LinearClaimGf;
use f2z::bitz::{OpeningQuery, Pcs, Proof, Shape, StatementBinding, build_prover, build_verifier};
use f2z::poly::univariate::binary_gf128::BinaryFieldGF128 as Gf;
use flock_core::merkle::HashKind;

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn unhex(s: &str) -> Vec<u8> {
    (0..s.len() / 2)
        .map(|i| u8::from_str_radix(&s[2 * i..2 * i + 2], 16).expect("hex"))
        .collect()
}

fn first_mismatch(a: &[u8], b: &[u8]) -> String {
    if a == b {
        return "IDENTICAL".to_string();
    }
    let at = a.iter().zip(b).position(|(x, y)| x != y).unwrap_or(a.len().min(b.len()));
    format!("first mismatch at byte {at}")
}

fn gf(bytes: &[u8]) -> Gf {
    Gf::from_words([
        u64::from_le_bytes(bytes[..8].try_into().unwrap()),
        u64::from_le_bytes(bytes[8..16].try_into().unwrap()),
    ])
}

fn main() {
    let dir = std::path::PathBuf::from(std::env::args().nth(1).expect("dump dir"));
    let meta: HashMap<String, String> = std::fs::read_to_string(dir.join("meta.txt"))
        .expect("meta.txt")
        .lines()
        .filter_map(|l| l.split_once('='))
        .map(|(k, v)| (k.trim().to_string(), v.trim().to_string()))
        .collect();
    let t: usize = meta["t"].parse().unwrap();
    let s: usize = meta["s"].parse().unwrap();
    let session = meta["session"].clone();
    let instance = meta["instance"].clone();
    let their_root = unhex(&meta["root"]);

    // The packed witness, flat: element i holds bits 128i.. (lo, then hi).
    let bytes = std::fs::read(dir.join("witness.bin")).expect("witness.bin");
    let shape = Shape::new(t, s).expect("shape");
    assert_eq!(bytes.len() * 8, 1 << shape.log_bits());
    let rows_per_column = shape.rows() / 64;
    let rows: Vec<Vec<u64>> = (0..shape.columns())
        .map(|c| {
            (0..rows_per_column)
                .map(|w| {
                    let at = (c * rows_per_column + w) * 8;
                    u64::from_le_bytes(bytes[at..at + 8].try_into().unwrap())
                })
                .collect()
        })
        .collect();

    // claim.bin: u64 count, count x 16 B, u64 count, count x 16 B, 16 B target.
    let cb = std::fs::read(dir.join("claim.bin")).expect("claim.bin");
    let mut at = 0usize;
    let mut take_u64 = |at: &mut usize| {
        let v = u64::from_le_bytes(cb[*at..*at + 8].try_into().unwrap());
        *at += 8;
        v as usize
    };
    let n_rows = take_u64(&mut at);
    let row_weights: Vec<Gf> = (0..n_rows)
        .map(|i| gf(&cb[at + 16 * i..at + 16 * i + 16]))
        .collect();
    at += 16 * n_rows;
    let n_cols = take_u64(&mut at);
    let column_weights: Vec<Gf> = (0..n_cols)
        .map(|i| gf(&cb[at + 16 * i..at + 16 * i + 16]))
        .collect();
    at += 16 * n_cols;
    let target = gf(&cb[at..at + 16]);
    assert_eq!(at + 16, cb.len());
    assert_eq!((n_rows, n_cols), (shape.rows(), shape.columns()));

    let pcs = Pcs::new(&shape, HashKind::Blake3).expect("pcs");
    let started = Instant::now();
    let (root, hint) = pcs.commit(&shape, rows).expect("commit");
    println!(
        "shape=({t},{s}) commit {:.1?} root ours={} theirs={} {}",
        started.elapsed(),
        hex(&root.0),
        hex(&their_root),
        if root.0[..] == their_root[..] { "MATCH" } else { "MISMATCH" }
    );
    let claim = LinearClaimGf::from_shape(&shape, row_weights, column_weights, target).expect("claim");
    let query = OpeningQuery::InnerProduct { claim };

    let theirs = Proof {
        narg_string: std::fs::read(dir.join("narg.bin")).expect("narg.bin"),
        hints: std::fs::read(dir.join("hints.bin")).expect("hints.bin"),
    };
    let started = Instant::now();
    let mut transcript = build_prover(session.as_str(), instance.as_str());
    pcs.prove_lin(&hint, &query, StatementBinding::Bind, &mut transcript)
        .expect("our prove_lin");
    let ours = transcript.finish();
    println!("our prove_lin: {:.1?}", started.elapsed());
    println!(
        "narg:  ours={} B theirs={} B {}",
        ours.narg_string.len(),
        theirs.narg_string.len(),
        first_mismatch(&ours.narg_string, &theirs.narg_string)
    );
    println!(
        "hints: ours={} B theirs={} B {}",
        ours.hints.len(),
        theirs.hints.len(),
        first_mismatch(&ours.hints, &theirs.hints)
    );
    for (label, proof) in [("THEIR", &theirs), ("OUR", &ours)] {
        let started = Instant::now();
        let mut verifier = build_verifier(session.as_str(), instance.as_str(), proof);
        let result = pcs.verify_lin(&root, &query, StatementBinding::Bind, &mut verifier);
        let eof = verifier.check_eof();
        println!(
            "our verifier on {label} proof: {result:?} eof={} ({:.1?})",
            eof.is_ok(),
            started.elapsed()
        );
    }
    std::fs::write(dir.join("ours.narg.bin"), &ours.narg_string).unwrap();
    std::fs::write(dir.join("ours.hints.bin"), &ours.hints).unwrap();
}

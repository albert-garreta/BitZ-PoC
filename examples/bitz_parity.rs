//! BitZ transcript parity harness (our side).
//!
//! Reads an instance and proof dumped by f2z-benchmark's `dump_bitz`
//! example (`meta.txt`, `witness.bin`, `claim.bin`, `narg.bin`,
//! `hints.bin`), re-proves it here through `f2z::bitz`, and compares the
//! narg string and hint stream byte for byte. Also verifies their proof
//! with our verifier and ours with ours, and writes our proof next to
//! theirs (`ours.narg.bin`, `ours.hints.bin`) for their `verify_bitz`.
//!
//! Usage: `bitz_parity <dump-dir>`
use std::collections::HashMap;
use std::path::Path;

use f2z::bitz::{
    BitZParams, BitZProver, BitZVerifier, LinearClaim, Pcs, Proof, Shape, WINDOW, build_prover,
    build_verifier,
};
use flock_core::merkle::HashKind;
use f2z::poly::univariate::binary_gf128::BinaryFieldGF128 as Gf;

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn unhex(s: &str) -> Vec<u8> {
    (0..s.len() / 2)
        .map(|i| u8::from_str_radix(&s[2 * i..2 * i + 2], 16).expect("hex"))
        .collect()
}

fn first_mismatch(a: &[u8], b: &[u8]) -> Option<usize> {
    if a == b {
        return None;
    }
    Some(a.iter().zip(b).position(|(x, y)| x != y).unwrap_or(a.len().min(b.len())))
}

fn main() {
    let dir = std::env::args().nth(1).expect("usage: bitz_parity <dump-dir>");
    let dir = Path::new(&dir);
    let meta: HashMap<String, String> = std::fs::read_to_string(dir.join("meta.txt"))
        .expect("meta.txt")
        .lines()
        .filter_map(|l| l.split_once('='))
        .map(|(k, v)| (k.trim().to_string(), v.trim().to_string()))
        .collect();
    let t: usize = meta["t"].parse().unwrap();
    let s: usize = meta["s"].parse().unwrap();
    let q: u128 = meta["q"].parse().unwrap();
    let generator = {
        let b = unhex(&meta["generator"]);
        Gf::from_words([
            u64::from_le_bytes(b[..8].try_into().unwrap()),
            u64::from_le_bytes(b[8..].try_into().unwrap()),
        ])
    };
    let session = meta["session"].clone();
    let instance = meta["instance"].clone();
    let their_root = unhex(&meta["root"]);

    // Witness → per-column bit rows (their packed layout is ours).
    let bytes = std::fs::read(dir.join("witness.bin")).expect("witness.bin");
    let packed: Vec<(u64, u64)> = bytes
        .chunks_exact(16)
        .map(|c| {
            (
                u64::from_le_bytes(c[..8].try_into().unwrap()),
                u64::from_le_bytes(c[8..].try_into().unwrap()),
            )
        })
        .collect();
    let hi_count = 1usize << (t - 7);
    assert_eq!(packed.len(), hi_count << s, "witness length vs shape");
    let rows: Vec<Vec<u64>> = (0..1usize << s)
        .map(|c| {
            let mut row = Vec::with_capacity(2 * hi_count);
            for i_hi in 0..hi_count {
                let (lo, hi) = packed[c * hi_count + i_hi];
                row.push(lo);
                row.push(hi);
            }
            row
        })
        .collect();

    // Claim: u64 count, count × u128 LE, u64 count, count × u128 LE, u128 target.
    let cb = std::fs::read(dir.join("claim.bin")).expect("claim.bin");
    let mut at = 0usize;
    let take_u64 = |at: &mut usize| {
        let v = u64::from_le_bytes(cb[*at..*at + 8].try_into().unwrap());
        *at += 8;
        v
    };
    let take_u128 = |at: &mut usize| {
        let v = u128::from_le_bytes(cb[*at..*at + 16].try_into().unwrap());
        *at += 16;
        v
    };
    let n_rows = take_u64(&mut at) as usize;
    let row_weights: Vec<u128> = (0..n_rows).map(|_| take_u128(&mut at)).collect();
    let n_cols = take_u64(&mut at) as usize;
    let column_weights: Vec<u128> = (0..n_cols).map(|_| take_u128(&mut at)).collect();
    let target = take_u128(&mut at);
    assert_eq!(at, cb.len(), "claim.bin trailing bytes");

    let shape = Shape::new(t, s).expect("shape");
    let params = BitZParams::new(shape, q, generator).expect("params");
    let claim = LinearClaim::new(&params, row_weights, column_weights, target).expect("claim");
    let pcs = Pcs::new(&shape, HashKind::Blake3).expect("pcs");
    let (root, hint) = pcs.commit(&shape, rows).expect("commit");
    println!(
        "root: ours={} theirs={} {}",
        hex(&root.0),
        hex(&their_root),
        if root.0[..] == their_root[..] { "MATCH" } else { "MISMATCH" }
    );

    let theirs = Proof {
        narg_string: std::fs::read(dir.join("narg.bin")).expect("narg.bin"),
        hints: std::fs::read(dir.join("hints.bin")).expect("hints.bin"),
    };

    let prover = BitZProver::new(params, WINDOW);
    let started = std::time::Instant::now();
    let mut transcript = build_prover(session.as_str(), instance.as_str());
    prover
        .prove(&claim, &pcs, &hint, &mut transcript)
        .expect("our prover on their instance");
    let ours = transcript.finish();
    println!("our prove: {:.1?}", started.elapsed());

    println!(
        "narg:  ours={} B theirs={} B {}",
        ours.narg_string.len(),
        theirs.narg_string.len(),
        match first_mismatch(&ours.narg_string, &theirs.narg_string) {
            None => "IDENTICAL".to_string(),
            Some(i) => format!("first mismatch at byte {i}"),
        }
    );
    println!(
        "hints: ours={} B theirs={} B {}",
        ours.hints.len(),
        theirs.hints.len(),
        match first_mismatch(&ours.hints, &theirs.hints) {
            None => "IDENTICAL".to_string(),
            Some(i) => format!("first mismatch at byte {i}"),
        }
    );

    let verifier = BitZVerifier::new(params, WINDOW);
    let started = std::time::Instant::now();
    let on_theirs = verifier.verify(
        &claim,
        &pcs,
        root,
        build_verifier(session.as_str(), instance.as_str(), &theirs),
    );
    println!("our verifier on THEIR proof: {on_theirs:?} ({:.1?})", started.elapsed());
    let on_ours = verifier.verify(
        &claim,
        &pcs,
        root,
        build_verifier(session.as_str(), instance.as_str(), &ours),
    );
    println!("our verifier on OUR proof:   {on_ours:?}");

    std::fs::write(dir.join("ours.narg.bin"), &ours.narg_string).unwrap();
    std::fs::write(dir.join("ours.hints.bin"), &ours.hints).unwrap();
}

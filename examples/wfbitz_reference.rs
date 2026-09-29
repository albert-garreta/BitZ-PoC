//! Compare a `dump_bitz` fixture's commitment and verify a local proof.
//!
//! `cargo run --release --example wfbitz_reference -- <dump-dir>` reads
//! `meta.txt`, `witness.bin`, and `claim.bin`. The native challenge schedule
//! is versioned independently of upstream proof bytes.
use std::{collections::HashMap, path::Path};

use bitz::wfbitz::{
    BitZParams, BitZProver, BitZVerifier, LinearClaim, Pcs, Shape, WINDOW, build_prover,
    build_verifier,
};
use field::Gf128;
use flock_core::merkle::HashKind;

fn hex<const N: usize>(text: &str) -> Result<[u8; N], String> {
    if text.len() != N * 2 || !text.is_ascii() {
        return Err("wrong hexadecimal length".into());
    }
    let mut out = [0; N];
    for (i, byte) in out.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&text[i * 2..i * 2 + 2], 16).map_err(|e| e.to_string())?;
    }
    Ok(out)
}

fn take<const N: usize>(bytes: &mut &[u8]) -> Result<[u8; N], String> {
    let (value, tail) = bytes.split_at_checked(N).ok_or("truncated fixture")?;
    *bytes = tail;
    Ok(value.try_into().expect("checked field length"))
}

fn weights(bytes: &mut &[u8], expected: usize) -> Result<Vec<u128>, String> {
    if u64::from_le_bytes(take(bytes)?) != expected as u64 || bytes.len() < expected * 16 {
        return Err("claim dimensions disagree with the shape".into());
    }
    (0..expected)
        .map(|_| take(bytes).map(u128::from_le_bytes))
        .collect()
}

fn check(dir: &Path) -> Result<(), String> {
    let read = |name| std::fs::read(dir.join(name)).map_err(|e| format!("{name}: {e}"));
    let meta = String::from_utf8(read("meta.txt")?).map_err(|e| e.to_string())?;
    let meta: HashMap<_, _> = meta
        .lines()
        .filter_map(|l| l.split_once('='))
        .map(|(k, v)| (k.trim(), v.trim()))
        .collect();
    let value = |key| {
        meta.get(key)
            .copied()
            .ok_or_else(|| format!("missing {key}"))
    };
    let number = |key| value(key)?.parse::<usize>().map_err(|e| e.to_string());
    let shape = Shape::new(number("t")?, number("s")?).map_err(|e| format!("{e:?}"))?;
    let q = value("q")?
        .parse()
        .map_err(|e: std::num::ParseIntError| e.to_string())?;
    let generator = hex::<16>(value("generator")?)?;
    let generator = Gf128::from_polynomial_words([
        u64::from_le_bytes(generator[..8].try_into().unwrap()),
        u64::from_le_bytes(generator[8..].try_into().unwrap()),
    ]);
    let params = BitZParams::new(shape, q, generator).map_err(|e| format!("{e:?}"))?;
    let witness = read("witness.bin")?;
    if witness.len() != (1usize << shape.log_bits()) / 8 {
        return Err("witness dimensions disagree with the shape".into());
    }
    let rows = witness
        .chunks_exact(shape.rows() / 8)
        .map(|row| {
            row.chunks_exact(8)
                .map(|v| u64::from_le_bytes(v.try_into().unwrap()))
                .collect()
        })
        .collect();
    let claim = read("claim.bin")?;
    let mut bytes = claim.as_slice();
    let rows_weights = weights(&mut bytes, shape.rows())?;
    let columns_weights = weights(&mut bytes, shape.columns())?;
    let target = u128::from_le_bytes(take(&mut bytes)?);
    if !bytes.is_empty() {
        return Err("trailing claim bytes".into());
    }
    let claim = LinearClaim::new(&params, rows_weights, columns_weights, target)
        .map_err(|e| format!("{e:?}"))?;
    let pcs = Pcs::new(&shape, HashKind::Blake3).map_err(|e| format!("{e:?}"))?;
    let (root, hint) = pcs.commit(&shape, rows).map_err(|e| format!("{e:?}"))?;
    if root.0 != hex::<32>(value("root")?)? {
        return Err("commitment mismatch".into());
    }
    let (session, instance) = (value("session")?, value("instance")?);
    let mut prover = build_prover(session, instance);
    BitZProver::new(params, WINDOW)
        .prove(&claim, &pcs, &hint, &mut prover, None)
        .map_err(|e| format!("{e:?}"))?;
    let proof = prover.finish();
    BitZVerifier::new(params, WINDOW)
        .verify(
            &claim,
            &pcs,
            root,
            build_verifier(session, instance, &proof),
            None,
        )
        .map_err(|e| format!("{e:?}"))?;
    println!(
        "commitment matches; local proof verified ({} narg bytes, {} hint bytes)",
        proof.narg_string.len(),
        proof.hints.len()
    );
    Ok(())
}

fn main() -> Result<(), String> {
    let path = std::env::args_os()
        .nth(1)
        .ok_or("usage: wfbitz_reference <dump-dir>")?;
    check(Path::new(&path))
}

//! Per-scope profile of one u64-multiplication proof at a chosen size:
//! `cargo run --release --features span-metrics --example u64_mul_probe -- 21 [reps]`.
//!
//! `BITZ_OPENER=forest` (default) discharges the bitified claim through the
//! crate's chunked exponent-fold forest; `BITZ_OPENER=wfbitz` (needs
//! `--features bitz-parity`) through the parity port of worldfnd/BitZ's
//! scheme (`protocol::wfbitz_opener`), with `BITZ_WFBITZ_LADDER` naming
//! its Ligerito ladder (`fast` = as shipped, default; `udr:<r>:<k>`,
//! `custom:<r>:<k>`). Each opener starts from its own default split (the
//! wfbitz one `MulLayout::wfbitz_split`); `BITZ_U64_SPLIT_SHIFT=k` moves
//! `k` more gate variables from rows to columns. The commit is timed
//! separately (the paper's "online prover" is commit + prove).

use ::bitz::piop::spartan::protocol;
use ::bitz::piop::spartan::protocol::PreparedRelation;
use bitz::piop::spartan::mul::{MulLayout, MulWitness};

use bitz::transcript::Blake3Transcript;

fn median(v: &mut Vec<f64>) -> f64 {
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    v[v.len() / 2]
}

/// `reps` timed prove/verify pairs after one warm-up pair: the wall
/// medians, every traced scope's median, the serialized proof size.
fn measure<P>(
    e: usize,
    reps: usize,
    commit: impl Fn() -> f64,
    prove: impl Fn() -> P,
    verify: impl Fn(&P),
    size: impl Fn(&P) -> usize,
) {
    let p = prove();
    verify(&p);
    std::hint::black_box(p);

    let mut commits = vec![];
    for _ in 0..reps {
        commits.push(commit());
    }

    let mut totals: std::collections::BTreeMap<String, Vec<f64>> = Default::default();
    let mut vtotals: std::collections::BTreeMap<String, Vec<f64>> = Default::default();
    let mut wall = vec![];
    let mut vwall = vec![];
    let mut last = None;
    for _ in 0..reps {
        let profile =
            bitz::observability::Recording::start(Vec::new()).expect("capture prover profile");
        let (p, t) =
            bitz::observability::measure(tracing::info_span!("u64_mul_probe:p"), || prove())
                .expect("measure completed operation");
        wall.push(t.as_secs_f64() * 1e3);
        for (label, secs) in
            bitz::observability::totals(&profile.intervals().expect("prover intervals"))
        {
            totals.entry(label.to_string()).or_default().push(secs * 1e3);
        }
        let profile =
            bitz::observability::Recording::start(Vec::new()).expect("capture verifier profile");
        let (_, t) =
            bitz::observability::measure(tracing::info_span!("u64_mul_probe:t"), || verify(&p))
                .expect("measure completed operation");
        vwall.push(t.as_secs_f64() * 1e3);
        for (label, secs) in
            bitz::observability::totals(&profile.intervals().expect("verifier intervals"))
        {
            vtotals.entry(label.to_string()).or_default().push(secs * 1e3);
        }
        last = Some(p);
    }
    let report =
        |title: &str, wall: &mut Vec<f64>, totals: std::collections::BTreeMap<String, Vec<f64>>| {
            println!("2^{e}: {title} wall median {:.1} ms over {reps} reps", median(wall));
            let mut rows: Vec<_> = totals.into_iter().collect();
            rows.sort_by(|a, b| {
                b.1.iter()
                    .cloned()
                    .fold(0.0, f64::max)
                    .partial_cmp(&a.1.iter().cloned().fold(0.0, f64::max))
                    .unwrap()
            });
            for (label, mut v) in rows {
                println!("  {:<48} {:8.2} ms", label, median(&mut v));
            }
        };
    let commit_ms = median(&mut commits);
    let prove_ms = median(&mut wall.clone());
    println!(
        "2^{e}: commit median {commit_ms:.1} ms | online prover (commit + prove) {:.1} ms",
        commit_ms + prove_ms
    );
    report("prove", &mut wall, totals);
    report("verify", &mut vwall, vtotals);
    println!("proof bytes: {}", size(&last.expect("reps >= 1")));
}

fn main() {
    bitz::observability::install().expect("install Perfetto subscriber");
    let e: usize = std::env::args()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(18);
    let reps: usize = std::env::args()
        .nth(2)
        .and_then(|s| s.parse().ok())
        .unwrap_or(3);

    let witness = MulWitness::<u64>::from_fn(1 << e, |i| {
        let x = (i as u64).wrapping_mul(0x9e37_79b9_7f4a_7c15) | 1;
        let y = (i as u64).wrapping_mul(0xc2b2_ae3d_27d4_eb4f) | 1;
        (x, y)
    })
    .unwrap();
    let opener = std::env::var("BITZ_OPENER").unwrap_or_else(|_| "forest".to_string());
    // The wfbitz opener starts from its own split (`MulLayout::wfbitz_split`);
    // BITZ_U64_SPLIT_SHIFT=k then moves k more gate variables from rows to
    // columns (relative to the opener's default split).
    let shift: i8 = std::env::var("BITZ_U64_SPLIT_SHIFT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);
    let layout = if opener == "wfbitz" {
        witness.layout().wfbitz_split(shift).unwrap()
    } else {
        witness.layout().with_split_shift(shift).unwrap()
    };
    let witness = witness.with_split_shift((layout.col_vars() as i64 - MulLayout::<u64>::new(1 << e).unwrap().col_vars() as i64) as i8).unwrap();
    let params = witness.layout().bitz_params();
    println!(
        "split: shift {shift} -> t={} s={} | opener {opener}",
        params.row_vars, params.col_vars
    );
    let layout = *witness.layout();
    match opener.as_str() {
        "forest" => {
            let prepared = PreparedRelation::<MulLayout<u64>>::new(layout).unwrap();
            let hint = protocol::commit(&prepared, witness.bitz_bit_rows()).unwrap();
            measure(
                e,
                reps,
                || {
                    let rows = witness.bitz_bit_rows();
                    let started = std::time::Instant::now();
                    let h = protocol::commit(&prepared, rows).unwrap();
                    let ms = started.elapsed().as_secs_f64() * 1e3;
                    std::hint::black_box(h);
                    ms
                },
                || protocol::prove(&mut Blake3Transcript::new(), &prepared, &witness, &hint).unwrap(),
                |p| {
                    protocol::verify(&mut Blake3Transcript::new(), &prepared, &hint.commitment, p)
                        .unwrap()
                },
                |p| hint.commitment.root.len() + p.size_bytes(prepared.security()),
            );
        }
        #[cfg(feature = "bitz-parity")]
        "wfbitz" => {
            use bitz::piop::spartan::Lambda100;
            use bitz::piop::spartan::protocol::wfbitz_opener::{self, WfbitzLigerito, WfbitzOpener};
            let ladder = std::env::var("BITZ_WFBITZ_LADDER").unwrap_or_else(|_| "fast".to_string());
            let ladder = WfbitzLigerito::parse(&ladder, 100).expect("BITZ_WFBITZ_LADDER");
            let (prefix, opener) = WfbitzOpener::prepare::<Lambda100, _>(layout, ladder, 100).unwrap();
            let (bits, term) = opener.opening_bits();
            println!(
                "wfbitz ladder {} | opening {bits:.1} bits ({term}) | round 0: {:?}",
                opener.ligerito().name(),
                prefix.security().ood
            );
            let hint = opener.commit(witness.bitz_bit_rows()).unwrap();
            measure(
                e,
                reps,
                || {
                    let rows = witness.bitz_bit_rows();
                    let started = std::time::Instant::now();
                    let h = opener.commit(rows).unwrap();
                    let ms = started.elapsed().as_secs_f64() * 1e3;
                    std::hint::black_box(h);
                    ms
                },
                || {
                    wfbitz_opener::prove(&mut Blake3Transcript::new(), &prefix, &opener, &witness, &hint)
                        .unwrap()
                },
                |p| {
                    wfbitz_opener::verify(
                        &mut Blake3Transcript::new(),
                        &prefix,
                        &opener,
                        &hint.commitment,
                        p,
                    )
                    .unwrap()
                },
                |p| hint.commitment.root.len() + p.size_bytes(prefix.security()),
            );
        }
        other => {
            eprintln!("BITZ_OPENER={other}: use forest or wfbitz (the latter needs --features bitz-parity)");
            std::process::exit(2);
        }
    }
}

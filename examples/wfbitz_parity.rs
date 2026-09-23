//! BitZ transcript parity harness (our side).
//!
//! Reads an instance and proof dumped by f2z-benchmark's `dump_bitz`
//! example (`meta.txt`, `witness.bin`, `claim.bin`, `narg.bin`,
//! `hints.bin`), re-proves it here through `bitz::bitz`, and compares the
//! narg string and hint stream byte for byte. Also verifies their proof
//! with our verifier and ours with ours, and writes our proof next to
//! theirs (`ours.narg.bin`, `ours.hints.bin`) for their `verify_bitz`.
//!
//! Usage:
//!   `bitz_parity <dump-dir>`
//!   `bitz_parity --sweep <their-examples-dir> <scratch-dir> <n-list> <seeds> [--keep]`
//!
//! `BITZ_REPEAT=k` proves `k` times in the single-dir mode (every repeat
//! must produce the same bytes) and reports the minimum and the median.
//!
//! The sweep runs the whole loop for every `n` in the comma list and every
//! seed — `<seeds>` is either a comma list of seeds or a count, in which
//! case the seeds are drawn from a base printed at the start (set
//! `BITZ_SWEEP_BASE` to reproduce a draw): their `dump_bitz n seed dir`,
//! the in-process check above, their `verify_bitz n seed ours.*`. One line
//! per case, a summary, exit status 1 if any case fails. Passing dumps are
//! deleted unless `--keep`.
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

use bitz::wfbitz::{
    BitZParams, BitZProver, BitZVerifier, LinearClaim, Pcs, Proof, Shape, WINDOW, build_prover,
    build_verifier,
};
use field::Gf128 as Gf;
use flock_core::merkle::HashKind;

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

/// What one dump directory yielded.
struct Report {
    t: usize,
    s: usize,
    root_match: bool,
    narg: Option<usize>,
    hints: Option<usize>,
    narg_len: (usize, usize),
    hints_len: (usize, usize),
    ours_on_theirs: bool,
    ours_on_ours: bool,
    prove: Duration,
    verify: Duration,
}

impl Report {
    fn passed(&self) -> bool {
        self.root_match
            && self.narg.is_none()
            && self.hints.is_none()
            && self.ours_on_theirs
            && self.ours_on_ours
    }
}

fn describe(mismatch: Option<usize>) -> String {
    match mismatch {
        None => "IDENTICAL".to_string(),
        Some(i) => format!("first mismatch at byte {i}"),
    }
}

/// Re-proves the dump in `dir`, compares, verifies both ways, writes ours.
fn check(dir: &Path, verbose: bool) -> Result<Report, String> {
    let meta: HashMap<String, String> = std::fs::read_to_string(dir.join("meta.txt"))
        .map_err(|e| format!("meta.txt: {e}"))?
        .lines()
        .filter_map(|l| l.split_once('='))
        .map(|(k, v)| (k.trim().to_string(), v.trim().to_string()))
        .collect();
    let field = |k: &str| meta.get(k).cloned().ok_or_else(|| format!("meta.txt lacks {k}"));
    let t: usize = field("t")?.parse().map_err(|e| format!("t: {e}"))?;
    let s: usize = field("s")?.parse().map_err(|e| format!("s: {e}"))?;
    let q: u128 = field("q")?.parse().map_err(|e| format!("q: {e}"))?;
    let generator = {
        let b = unhex(&field("generator")?);
        Gf::from_polynomial_words([
            u64::from_le_bytes(b[..8].try_into().unwrap()),
            u64::from_le_bytes(b[8..].try_into().unwrap()),
        ])
    };
    let session = field("session")?;
    let instance = field("instance")?;
    let their_root = unhex(&field("root")?);

    // Witness → per-column bit rows (their packed layout is ours).
    let bytes = std::fs::read(dir.join("witness.bin")).map_err(|e| format!("witness.bin: {e}"))?;
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
    if packed.len() != hi_count << s {
        return Err(format!("witness length {} vs shape 2^{}", packed.len(), t + s - 7));
    }
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
    let cb = std::fs::read(dir.join("claim.bin")).map_err(|e| format!("claim.bin: {e}"))?;
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
    if at != cb.len() {
        return Err("claim.bin trailing bytes".to_string());
    }

    let shape = Shape::new(t, s).map_err(|e| format!("shape: {e:?}"))?;
    let params = BitZParams::new(shape, q, generator).map_err(|e| format!("params: {e:?}"))?;
    let claim = LinearClaim::new(&params, row_weights, column_weights, target)
        .map_err(|e| format!("claim: {e:?}"))?;
    let pcs = Pcs::new(&shape, HashKind::Blake3).map_err(|e| format!("pcs: {e:?}"))?;
    let (root, hint) = pcs.commit(&shape, rows).map_err(|e| format!("commit: {e:?}"))?;
    let root_match = root.0[..] == their_root[..];
    if verbose {
        println!(
            "root: ours={} theirs={} {}",
            hex(&root.0),
            hex(&their_root),
            if root_match { "MATCH" } else { "MISMATCH" }
        );
    }

    let theirs = Proof {
        narg_string: std::fs::read(dir.join("narg.bin")).map_err(|e| format!("narg.bin: {e}"))?,
        hints: std::fs::read(dir.join("hints.bin")).map_err(|e| format!("hints.bin: {e}"))?,
    };

    let prover = BitZProver::new(params, WINDOW);
    let repeats: usize = std::env::var("BITZ_REPEAT")
        .ok()
        .and_then(|r| r.parse().ok())
        .filter(|&r| r >= 1 && verbose)
        .unwrap_or(1);
    let mut times = Vec::with_capacity(repeats);
    let mut ours: Option<Proof> = None;
    for _ in 0..repeats {
        let started = Instant::now();
        let mut transcript = build_prover(session.as_str(), instance.as_str());
        prover
            .prove(&claim, &pcs, &hint, &mut transcript)
            .map_err(|e| format!("our prover: {e:?}"))?;
        let proof = transcript.finish();
        times.push(started.elapsed());
        if let Some(previous) = &ours {
            if previous.narg_string != proof.narg_string || previous.hints != proof.hints {
                return Err("our prover is not deterministic across repeats".to_string());
            }
        }
        ours = Some(proof);
    }
    let ours = ours.expect("at least one prove");
    let prove = times[0];
    if verbose {
        if repeats > 1 {
            let mut sorted = times.clone();
            sorted.sort();
            println!(
                "our prove: min {:.1?} median {:.1?} over {} repeats ({} identical proofs)",
                sorted[0],
                sorted[sorted.len() / 2],
                repeats,
                repeats
            );
        } else {
            println!("our prove: {prove:.1?}");
        }
    }

    let narg = first_mismatch(&ours.narg_string, &theirs.narg_string);
    let hints = first_mismatch(&ours.hints, &theirs.hints);
    if verbose {
        println!(
            "narg:  ours={} B theirs={} B {}",
            ours.narg_string.len(),
            theirs.narg_string.len(),
            describe(narg)
        );
        println!(
            "hints: ours={} B theirs={} B {}",
            ours.hints.len(),
            theirs.hints.len(),
            describe(hints)
        );
    }

    let verifier = BitZVerifier::new(params, WINDOW);
    let started = Instant::now();
    let on_theirs = verifier.verify(
        &claim,
        &pcs,
        root,
        build_verifier(session.as_str(), instance.as_str(), &theirs),
    );
    let verify = started.elapsed();
    let on_ours = verifier.verify(
        &claim,
        &pcs,
        root,
        build_verifier(session.as_str(), instance.as_str(), &ours),
    );
    if verbose {
        println!("our verifier on THEIR proof: {on_theirs:?} ({verify:.1?})");
        println!("our verifier on OUR proof:   {on_ours:?}");
    }

    std::fs::write(dir.join("ours.narg.bin"), &ours.narg_string).map_err(|e| e.to_string())?;
    std::fs::write(dir.join("ours.hints.bin"), &ours.hints).map_err(|e| e.to_string())?;
    Ok(Report {
        t,
        s,
        root_match,
        narg,
        hints,
        narg_len: (ours.narg_string.len(), theirs.narg_string.len()),
        hints_len: (ours.hints.len(), theirs.hints.len()),
        ours_on_theirs: on_theirs.is_ok(),
        ours_on_ours: on_ours.is_ok(),
        prove,
        verify,
    })
}

/// splitmix64 — the sweep's seed draw.
fn splitmix(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

fn parse_list(s: &str) -> Vec<u64> {
    s.split(',')
        .filter(|x| !x.is_empty())
        .map(|x| x.trim().parse().expect("number list"))
        .collect()
}

/// Their prover's `prove:` line, if it printed one.
fn their_prove_time(stdout: &str) -> String {
    stdout
        .lines()
        .find_map(|l| l.strip_prefix("prove: "))
        .unwrap_or("?")
        .to_string()
}

fn sweep(their: &Path, scratch: &Path, ns: &[u64], seeds_arg: &str, keep: bool) -> bool {
    let dump = their.join("dump_bitz");
    let verify = their.join("verify_bitz");
    assert!(dump.is_file(), "{} missing", dump.display());
    assert!(verify.is_file(), "{} missing", verify.display());
    let seeds: Vec<u64> = match seeds_arg.parse::<usize>() {
        Ok(count) if !seeds_arg.contains(',') => {
            let base = std::env::var("BITZ_SWEEP_BASE")
                .ok()
                .and_then(|b| b.parse().ok())
                .unwrap_or_else(|| {
                    std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map(|d| d.as_nanos() as u64)
                        .unwrap_or(1)
                });
            println!("seed base: {base} (BITZ_SWEEP_BASE={base} reproduces the draw)");
            let mut state = base;
            (0..count).map(|_| splitmix(&mut state) >> 32).collect()
        }
        _ => parse_list(seeds_arg),
    };
    println!("n: {ns:?}  seeds: {seeds:?}");
    std::fs::create_dir_all(scratch).expect("scratch dir");

    let mut failures = 0usize;
    let mut cases = 0usize;
    for &n in ns {
        for &seed in &seeds {
            cases += 1;
            let dir = scratch.join(format!("sweep_n{n}_seed{seed}"));
            let _ = std::fs::remove_dir_all(&dir);
            let started = Instant::now();
            let out = Command::new(&dump)
                .arg(n.to_string())
                .arg(seed.to_string())
                .arg(&dir)
                .output()
                .expect("run their dump_bitz");
            let their_wall = started.elapsed();
            if !out.status.success() {
                failures += 1;
                let err = String::from_utf8_lossy(&out.stderr);
                println!(
                    "n={n:<2} seed={seed:<11} FAIL: their dump_bitz exited {:?}: {}",
                    out.status.code(),
                    err.lines().last().unwrap_or("")
                );
                continue;
            }
            let their_prove = their_prove_time(&String::from_utf8_lossy(&out.stdout));
            let report = match check(&dir, false) {
                Ok(r) => r,
                Err(e) => {
                    failures += 1;
                    println!("n={n:<2} seed={seed:<11} FAIL: {e}");
                    continue;
                }
            };
            let theirs_on_ours = Command::new(&verify)
                .arg(n.to_string())
                .arg(seed.to_string())
                .arg(dir.join("ours.narg.bin"))
                .arg(dir.join("ours.hints.bin"))
                .output()
                .expect("run their verify_bitz");
            let their_accepts = theirs_on_ours.status.success()
                && String::from_utf8_lossy(&theirs_on_ours.stdout).contains("Ok(())");
            let pass = report.passed() && their_accepts;
            if !pass {
                failures += 1;
            }
            println!(
                "n={n:<2} ({:>2},{:>2}) seed={seed:<11} {} | theirs {their_prove} ({:.1?} wall) | ours {:.1?} verify {:.1?} | root {} | narg {} ({}/{} B) | hints {} ({}/{} B) | our verifier theirs:{} ours:{} | their verifier: {}",
                report.t,
                report.s,
                if pass { "PASS" } else { "FAIL" },
                their_wall,
                report.prove,
                report.verify,
                if report.root_match { "match" } else { "MISMATCH" },
                describe(report.narg),
                report.narg_len.0,
                report.narg_len.1,
                describe(report.hints),
                report.hints_len.0,
                report.hints_len.1,
                if report.ours_on_theirs { "ok" } else { "REJECT" },
                if report.ours_on_ours { "ok" } else { "REJECT" },
                if their_accepts { "ok" } else { "REJECT" },
            );
            if pass && !keep {
                let _ = std::fs::remove_dir_all(&dir);
            }
        }
    }
    println!(
        "sweep: {} cases, {} failed{}",
        cases,
        failures,
        if failures == 0 { " — all IDENTICAL, all accepted" } else { "" }
    );
    failures == 0
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() >= 2 && args[1] == "--sweep" {
        if args.len() < 6 {
            eprintln!("usage: bitz_parity --sweep <their-examples-dir> <scratch-dir> <n-list> <seeds> [--keep]");
            std::process::exit(2);
        }
        let their = PathBuf::from(&args[2]);
        let scratch = PathBuf::from(&args[3]);
        let ns = parse_list(&args[4]);
        let keep = args.iter().skip(6).any(|a| a == "--keep");
        let ok = sweep(&their, &scratch, &ns, &args[5], keep);
        std::process::exit(if ok { 0 } else { 1 });
    }
    let dir = args.get(1).expect("usage: bitz_parity <dump-dir> | --sweep ...");
    match check(Path::new(dir), true) {
        Ok(report) => {
            if !report.passed() {
                std::process::exit(1);
            }
        }
        Err(e) => {
            eprintln!("bitz_parity: {e}");
            std::process::exit(1);
        }
    }
}

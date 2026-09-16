//! End-to-end parity harness (stage B/C): re-proves a `dump_e2e` dump (the
//! oracle's `tooling/cli/examples/dump_e2e.rs`) through `f2z::bitz::e2e`
//! and diffs the root, the out-of-band Spartan bytes, the claim on `h`, the
//! narg string and the hint stream; verifies their proof and ours with our
//! verifier; writes `ours.{spartan,narg,hints}.bin` for their `verify_e2e`.
//!
//! `bitz_e2e_parity <dump-dir>` — `BITZ_REPEAT=k` proves `k` times (every
//! repeat must give the same bytes; min and median reported); `BITZ_TRACE=1`
//! prints the phase timings.
//!
//! `bitz_e2e_parity --sweep <their-examples-dir> <scratch-dir> <circuit>
//! <blocks-list> <seeds> [--keep] [--sampled]` (`--sampled`: their
//! `dump_e2e --sampled` and `verify_e2e --sampled`, our `PreparedSampled`) — for every block count in the comma list
//! and every seed (a comma list, or a count drawn from a printed base;
//! `BITZ_SWEEP_BASE` reproduces a draw): their `dump_e2e`, the in-process
//! check above, their `verify_e2e` on our files. One line per case, a
//! summary, exit status 1 if any case fails.
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

use f2z::bitz::e2e::{Prepared, PreparedSampled, Proof, SampledProof, opening_claim};
use f2z::bitz::fq::{Q, modulus, set_modulus};
use f2z::bitz::spartan::{ScaledMleEvaluationClaim, SpartanPiopProof};
use f2z::bitz::statements::{Sha256Circuit, Sha256Statement, splitmix64};
use f2z::bitz::transcript::Proof as TranscriptProof;
use f2z::bitz::{LinearClaim, Root};
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

fn describe(mismatch: Option<usize>) -> String {
    match mismatch {
        None => "IDENTICAL".to_string(),
        Some(i) => format!("first mismatch at byte {i}"),
    }
}

fn claim_bytes(claim: &LinearClaim) -> Vec<u8> {
    let mut bytes = Vec::new();
    for weights in [claim.row_weights(), claim.column_weights()] {
        bytes.extend_from_slice(&(weights.len() as u64).to_le_bytes());
        for &w in weights {
            bytes.extend_from_slice(&w.to_le_bytes());
        }
    }
    bytes.extend_from_slice(&claim.target().to_le_bytes());
    bytes
}

/// The first `bit_len` bits of LSB-first words, as bytes.
fn bits_to_bytes(words: &[u64], bit_len: usize) -> Vec<u8> {
    let mut out = Vec::with_capacity(bit_len.div_ceil(8));
    for (i, word) in words.iter().enumerate() {
        if 64 * i >= bit_len {
            break;
        }
        let remaining = bit_len - 64 * i;
        out.extend_from_slice(&word.to_le_bytes()[..remaining.min(64).div_ceil(8)]);
    }
    out
}

struct Report {
    blocks: usize,
    root_match: bool,
    spartan: Option<usize>,
    claim_h: Option<usize>,
    narg: Option<usize>,
    hints: Option<usize>,
    narg_len: (usize, usize),
    hints_len: (usize, usize),
    ours_on_theirs: bool,
    ours_on_ours: bool,
    setup: Duration,
    prove: Duration,
    verify: Duration,
}

impl Report {
    fn passed(&self) -> bool {
        self.root_match
            && self.spartan.is_none()
            && self.claim_h.is_none()
            && self.narg.is_none()
            && self.hints.is_none()
            && self.ours_on_theirs
            && self.ours_on_ours
    }
}

fn check(dir: &Path, verbose: bool) -> Result<Report, String> {
    let meta: HashMap<String, String> = std::fs::read_to_string(dir.join("meta.txt"))
        .map_err(|e| format!("meta.txt: {e}"))?
        .lines()
        .filter_map(|l| l.split_once('='))
        .map(|(k, v)| (k.trim().to_string(), v.trim().to_string()))
        .collect();
    if meta.get("kind").map(String::as_str) == Some("e2e-sampled") {
        return check_sampled(dir, &meta, verbose);
    }
    let field = |k: &str| meta.get(k).cloned().ok_or_else(|| format!("meta.txt lacks {k}"));
    let read = |name: &str| std::fs::read(dir.join(name)).map_err(|e| format!("{name}: {e}"));
    let circuit = Sha256Circuit::parse(&field("circuit")?).ok_or("unknown circuit")?;
    let blocks: usize = field("blocks")?.parse().map_err(|e| format!("blocks: {e}"))?;
    let public = read("public.bin")?;
    let statement =
        Sha256Statement::from_public_bytes(circuit, &public).ok_or("public.bin does not parse")?;
    assert_eq!(statement.blocks.len(), blocks);
    let q: u128 = field("q")?.parse().map_err(|e| format!("q: {e}"))?;
    if q != Q {
        return Err(format!("modulus {q} is not this crate's Q"));
    }
    if modulus() != Q {
        set_modulus(Q).map_err(|e| format!("{e:?}"))?;
    }

    let started = Instant::now();
    let prepared = Prepared::new(statement.clone()).map_err(|e| format!("Prepared::new: {e:?}"))?;
    let setup = started.elapsed();
    let mut ok = true;
    let mut check_meta = |what: &str, ours: String, key: &str| {
        let theirs = meta.get(key).cloned().unwrap_or_default();
        let same = ours == theirs;
        ok &= same;
        if verbose || !same {
            println!("{what}: {}", if same { "MATCH".to_string() } else { format!("MISMATCH (ours {ours}, theirs {theirs})") });
        }
    };
    check_meta("constraint digest", hex(prepared.matrices().digest()), "constraint_digest");
    check_meta("map digest", hex(prepared.map_digest()), "map_digest");
    check_meta("claim shape t", prepared.params().shape().log_rows().to_string(), "claim_t");
    check_meta("claim shape s", prepared.params().shape().log_columns().to_string(), "claim_s");
    check_meta("committed shape t", prepared.committed_shape().log_rows().to_string(), "committed_t");
    check_meta("committed shape s", prepared.committed_shape().log_columns().to_string(), "committed_s");
    check_meta(
        "generator",
        hex(&f2z::bitz::codec::gf_to_bytes(prepared.params().generator())),
        "generator",
    );
    if !ok {
        return Err("the derived statement differs from the dump's".to_string());
    }

    let inputs = statement.input();
    let inputs_bin = read("inputs.bin")?;
    let mut ours_inputs = vec![0u8; inputs.len().div_ceil(8)];
    for (i, &b) in inputs.iter().enumerate() {
        if b {
            ours_inputs[i / 8] |= 1 << (i % 8);
        }
    }
    if ours_inputs != inputs_bin {
        return Err("inputs.bin differs from the statement's input bits".to_string());
    }
    let witness = prepared.witness(&inputs).map_err(|e| format!("witness: {e:?}"))?;
    let f_len: usize = field("f_len")?.parse().unwrap();
    let h_len: usize = field("h_len")?.parse().unwrap();
    let f_ours = bits_to_bytes(witness.committed_row(), f_len - 1);
    let h_words: Vec<u64> = witness.virtual_bits().rows().iter().flatten().copied().collect();
    let h_ours = bits_to_bytes(&h_words, h_len);
    if verbose {
        println!("f.bin {}; h.bin {}", describe(first_mismatch(&f_ours, &read("f.bin")?)), describe(first_mismatch(&h_ours, &read("h.bin")?)));
    }
    let (root, hint) = prepared.commit(&witness).map_err(|e| format!("commit: {e:?}"))?;
    let their_root = unhex(&field("root")?);
    let root_match = root.0[..] == their_root[..];
    if verbose {
        println!("root: ours {} theirs {} {}", hex(&root.0), hex(&their_root), if root_match { "MATCH" } else { "MISMATCH" });
    }

    let theirs = TranscriptProof {
        narg_string: read("narg.bin")?,
        hints: read("hints.bin")?,
    };
    let their_spartan = read("spartan.bin")?;
    let their_claim_h = read("claim_h.bin")?;

    let repeats: usize = std::env::var("BITZ_REPEAT")
        .ok()
        .and_then(|r| r.parse().ok())
        .filter(|&r| r >= 1 && verbose)
        .unwrap_or(1);
    let mut times = Vec::with_capacity(repeats);
    let mut ours: Option<Proof> = None;
    for _ in 0..repeats {
        let started = Instant::now();
        let proof = prepared.prove(&witness, &hint).map_err(|e| format!("our prover: {e:?}"))?;
        times.push(started.elapsed());
        if let Some(previous) = &ours {
            if previous != &proof {
                return Err("our prover is not deterministic across repeats".to_string());
            }
        }
        ours = Some(proof);
    }
    let ours = ours.expect("at least one prove");
    times.sort();
    if verbose {
        if repeats > 1 {
            println!("our prove: min {:.1?} median {:.1?} over {repeats} repeats (identical proofs)", times[0], times[times.len() / 2]);
        } else {
            println!("our prove: {:.1?} (setup {:.1?})", times[0], setup);
        }
    }
    let ours_spartan = ours.spartan.to_bytes(&ours.terminal);
    let ours_claim_h = claim_bytes(&opening_claim(prepared.params(), &ours.terminal).map_err(|e| format!("{e:?}"))?);
    let spartan = first_mismatch(&ours_spartan, &their_spartan);
    let claim_h = first_mismatch(&ours_claim_h, &their_claim_h);
    let narg = first_mismatch(&ours.opening.narg_string, &theirs.narg_string);
    let hints = first_mismatch(&ours.opening.hints, &theirs.hints);
    if verbose {
        println!("spartan.bin: ours {} B theirs {} B {}", ours_spartan.len(), their_spartan.len(), describe(spartan));
        println!("claim_h.bin: {}", describe(claim_h));
        println!("narg:  ours {} B theirs {} B {}", ours.opening.narg_string.len(), theirs.narg_string.len(), describe(narg));
        println!("hints: ours {} B theirs {} B {}", ours.opening.hints.len(), theirs.hints.len(), describe(hints));
    }

    // Their proof through our verifier.
    let nr = prepared.matrices().num_row_vars();
    let nc = prepared.matrices().num_column_vars();
    let (their_piop, their_terminal): (SpartanPiopProof, ScaledMleEvaluationClaim) =
        SpartanPiopProof::from_bytes(&their_spartan, nr, nc).map_err(|e| format!("their spartan.bin: {e:?}"))?;
    let their_proof = Proof {
        root: Root(their_root.clone().try_into().map_err(|_| "root length")?),
        spartan: their_piop,
        terminal: their_terminal,
        opening: theirs,
    };
    let started = Instant::now();
    let on_theirs = prepared.verify(&their_proof);
    let verify = started.elapsed();
    let on_ours = prepared.verify(&ours);
    if verbose {
        println!("our verifier on THEIR proof: {on_theirs:?} ({verify:.1?})");
        println!("our verifier on OUR proof:   {on_ours:?}");
    }
    std::fs::write(dir.join("ours.spartan.bin"), &ours_spartan).map_err(|e| e.to_string())?;
    std::fs::write(dir.join("ours.narg.bin"), &ours.opening.narg_string).map_err(|e| e.to_string())?;
    std::fs::write(dir.join("ours.hints.bin"), &ours.opening.hints).map_err(|e| e.to_string())?;
    Ok(Report {
        blocks,
        root_match,
        spartan,
        claim_h,
        narg,
        hints,
        narg_len: (ours.opening.narg_string.len(), theirs_len(&ours_spartan, &their_proof).0),
        hints_len: (ours.opening.hints.len(), their_proof.opening.hints.len()),
        ours_on_theirs: on_theirs.is_ok(),
        ours_on_ours: on_ours.is_ok(),
        setup,
        prove: times[0],
        verify,
    })
}

/// The sampled-prime kind: their `dump_e2e --sampled` through our
/// `PreparedSampled`; the prime is derived, compared with the dump's, and
/// installed before their proof is decoded.
fn check_sampled(dir: &Path, meta: &HashMap<String, String>, verbose: bool) -> Result<Report, String> {
    let field = |k: &str| meta.get(k).cloned().ok_or_else(|| format!("meta.txt lacks {k}"));
    let read = |name: &str| std::fs::read(dir.join(name)).map_err(|e| format!("{name}: {e}"));
    let circuit = Sha256Circuit::parse(&field("circuit")?).ok_or("unknown circuit")?;
    let blocks: usize = field("blocks")?.parse().map_err(|e| format!("blocks: {e}"))?;
    let prime_bits: u32 = field("prime_bits")?.parse().map_err(|e| format!("prime_bits: {e}"))?;
    let their_prime: u128 = field("prime")?.parse().map_err(|e| format!("prime: {e}"))?;
    let public = read("public.bin")?;
    let statement =
        Sha256Statement::from_public_bytes(circuit, &public).ok_or("public.bin does not parse")?;
    assert_eq!(statement.blocks.len(), blocks);

    let started = Instant::now();
    let prepared = PreparedSampled::new(statement.clone(), prime_bits).map_err(|e| format!("PreparedSampled::new: {e:?}"))?;
    let setup = started.elapsed();
    let mut ok = true;
    let mut check_meta = |what: &str, ours: String, key: &str| {
        let theirs = meta.get(key).cloned().unwrap_or_default();
        let same = ours == theirs;
        ok &= same;
        if verbose || !same {
            println!("{what}: {}", if same { "MATCH".to_string() } else { format!("MISMATCH (ours {ours}, theirs {theirs})") });
        }
    };
    check_meta("integer constraint digest", hex(prepared.matrices().digest()), "constraint_digest");
    check_meta("map digest", hex(prepared.map_digest()), "map_digest");
    check_meta("claim shape t", prepared.claim_shape().log_rows().to_string(), "claim_t");
    check_meta("claim shape s", prepared.claim_shape().log_columns().to_string(), "claim_s");
    check_meta("committed shape t", prepared.committed_shape().log_rows().to_string(), "committed_t");
    check_meta("committed shape s", prepared.committed_shape().log_columns().to_string(), "committed_s");
    if !ok {
        return Err("the derived statement differs from the dump's".to_string());
    }
    let inputs = statement.input();
    let witness = prepared.witness(&inputs).map_err(|e| format!("witness: {e:?}"))?;
    let (root, hint) = prepared.commit(&witness).map_err(|e| format!("commit: {e:?}"))?;
    let their_root = unhex(&field("root")?);
    let root_match = root.0[..] == their_root[..];
    let derived_prime = prepared.prime_for(&root);
    if verbose {
        println!("root: ours {} theirs {} {}", hex(&root.0), hex(&their_root), if root_match { "MATCH" } else { "MISMATCH" });
        println!(
            "prime: derived {derived_prime} ({} bits) theirs {their_prime} {}",
            128 - derived_prime.leading_zeros(),
            if derived_prime == their_prime { "MATCH" } else { "MISMATCH" }
        );
    }
    if derived_prime != their_prime {
        return Err(format!("the derived prime {derived_prime} is not the dump's {their_prime}"));
    }

    let theirs = TranscriptProof {
        narg_string: read("narg.bin")?,
        hints: read("hints.bin")?,
    };
    let their_spartan = read("spartan.bin")?;
    let their_claim_h = read("claim_h.bin")?;

    let repeats: usize = std::env::var("BITZ_REPEAT")
        .ok()
        .and_then(|r| r.parse().ok())
        .filter(|&r| r >= 1 && verbose)
        .unwrap_or(1);
    let mut times = Vec::with_capacity(repeats);
    let mut ours: Option<SampledProof> = None;
    for _ in 0..repeats {
        let started = Instant::now();
        let proof = prepared.prove(&witness, &hint).map_err(|e| format!("our prover: {e:?}"))?;
        times.push(started.elapsed());
        if let Some(previous) = &ours {
            if previous != &proof {
                return Err("our prover is not deterministic across repeats".to_string());
            }
        }
        ours = Some(proof);
    }
    let ours = ours.expect("at least one prove");
    times.sort();
    if verbose {
        if repeats > 1 {
            println!("our prove: min {:.1?} median {:.1?} over {repeats} repeats (identical proofs)", times[0], times[times.len() / 2]);
        } else {
            println!("our prove: {:.1?} (setup {:.1?})", times[0], setup);
        }
    }
    if ours.prime != their_prime {
        return Err("our prover's prime is not the dump's".to_string());
    }
    // The modulus is the prime now; the params and the claim are under it.
    let params = prepared.params().map_err(|e| format!("{e:?}"))?;
    let ours_spartan = ours.spartan.to_bytes(&ours.terminal);
    let ours_claim_h = claim_bytes(&opening_claim(&params, &ours.terminal).map_err(|e| format!("{e:?}"))?);
    let spartan = first_mismatch(&ours_spartan, &their_spartan);
    let claim_h = first_mismatch(&ours_claim_h, &their_claim_h);
    let narg = first_mismatch(&ours.opening.narg_string, &theirs.narg_string);
    let hints = first_mismatch(&ours.opening.hints, &theirs.hints);
    if verbose {
        println!("spartan.bin: ours {} B theirs {} B {}", ours_spartan.len(), their_spartan.len(), describe(spartan));
        println!("claim_h.bin: {}", describe(claim_h));
        println!("narg:  ours {} B theirs {} B {}", ours.opening.narg_string.len(), theirs.narg_string.len(), describe(narg));
        println!("hints: ours {} B theirs {} B {}", ours.opening.hints.len(), theirs.hints.len(), describe(hints));
    }

    // Their proof through our verifier: decoded under the derived prime.
    let nr = prepared.matrices().num_row_vars();
    let nc = prepared.matrices().num_column_vars();
    set_modulus(derived_prime).map_err(|e| format!("{e:?}"))?;
    let (their_piop, their_terminal) =
        SpartanPiopProof::from_bytes(&their_spartan, nr, nc).map_err(|e| format!("their spartan.bin: {e:?}"))?;
    let their_proof = SampledProof {
        root: Root(their_root.clone().try_into().map_err(|_| "root length")?),
        prime: their_prime,
        spartan: their_piop,
        terminal: their_terminal,
        opening: theirs,
    };
    let started = Instant::now();
    let on_theirs = prepared.verify(&their_proof);
    let verify = started.elapsed();
    let on_ours = prepared.verify(&ours);
    if verbose {
        println!("our verifier on THEIR proof: {on_theirs:?} ({verify:.1?})");
        println!("our verifier on OUR proof:   {on_ours:?}");
    }
    std::fs::write(dir.join("ours.spartan.bin"), &ours_spartan).map_err(|e| e.to_string())?;
    std::fs::write(dir.join("ours.narg.bin"), &ours.opening.narg_string).map_err(|e| e.to_string())?;
    std::fs::write(dir.join("ours.hints.bin"), &ours.opening.hints).map_err(|e| e.to_string())?;
    let their_lens = (their_proof.opening.narg_string.len(), their_proof.opening.hints.len());
    set_modulus(Q).map_err(|e| format!("{e:?}"))?;
    Ok(Report {
        blocks,
        root_match,
        spartan,
        claim_h,
        narg,
        hints,
        narg_len: (ours.opening.narg_string.len(), their_lens.0),
        hints_len: (ours.opening.hints.len(), their_lens.1),
        ours_on_theirs: on_theirs.is_ok(),
        ours_on_ours: on_ours.is_ok(),
        setup,
        prove: times[0],
        verify,
    })
}

fn theirs_len(_ours: &[u8], theirs: &Proof) -> (usize, usize) {
    (theirs.opening.narg_string.len(), theirs.opening.hints.len())
}

fn parse_list(s: &str) -> Vec<u64> {
    s.split(',').filter(|x| !x.is_empty()).map(|x| x.trim().parse().expect("number list")).collect()
}

fn meta_value(stdout: &str, key: &str) -> String {
    stdout
        .lines()
        .find_map(|l| l.strip_prefix(&format!("{key}=")))
        .unwrap_or("?")
        .to_string()
}

fn sweep(their: &Path, scratch: &Path, circuit: &str, blocks_list: &[u64], seeds_arg: &str, keep: bool, sampled: bool) -> bool {
    let dump = their.join("dump_e2e");
    let verify = their.join("verify_e2e");
    let seeds: Vec<u64> = if seeds_arg.contains(',') || blocks_list.len() * seeds_arg.len() == 0 {
        parse_list(seeds_arg)
    } else if let Ok(count) = seeds_arg.parse::<usize>() {
        let mut base: u64 = std::env::var("BITZ_SWEEP_BASE")
            .ok()
            .and_then(|b| b.parse().ok())
            .unwrap_or_else(|| {
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_nanos() as u64)
                    .unwrap_or(1)
            });
        println!("seed base {base} (BITZ_SWEEP_BASE={base} reproduces)");
        (0..count).map(|_| splitmix64(&mut base) % 1_000_000_007).collect()
    } else {
        parse_list(seeds_arg)
    };
    std::fs::create_dir_all(scratch).expect("scratch dir");
    let mut all_ok = true;
    let mut cases = 0;
    for &blocks in blocks_list {
        for &seed in &seeds {
            cases += 1;
            let dir = scratch.join(format!("e2e{}_{circuit}_{blocks}_seed{seed}", if sampled { "p" } else { "" }));
            let started = Instant::now();
            let mut command = Command::new(&dump);
            command.args([circuit, &blocks.to_string(), &seed.to_string()]).arg(&dir);
            if sampled {
                command.arg("--sampled");
            }
            let output = command.output().expect("run their dump_e2e");
            let their_dump = started.elapsed();
            if !output.status.success() {
                println!("blocks {blocks} seed {seed}: their dump_e2e FAILED: {}", String::from_utf8_lossy(&output.stderr));
                all_ok = false;
                continue;
            }
            let stdout = String::from_utf8_lossy(&output.stdout);
            let their_prove = meta_value(&stdout, "prove_ms");
            let their_verify = meta_value(&stdout, "verify_ms");
            let report = match check(&dir, false) {
                Ok(report) => report,
                Err(e) => {
                    println!("blocks {blocks} seed {seed}: FAILED: {e}");
                    all_ok = false;
                    continue;
                }
            };
            let mut command = Command::new(&verify);
            command.arg(&dir).arg("ours.");
            if sampled {
                command.arg("--sampled");
            }
            let their_verdict = command.output().expect("run their verify_e2e");
            let theirs_on_ours = their_verdict.status.success();
            let passed = report.passed() && theirs_on_ours;
            all_ok &= passed;
            println!(
                "blocks {blocks} seed {seed} shapes derived: {} | root {} spartan {} claim_h {} narg {} ({} B) hints {} ({} B) | ours→theirs {} ours→ours {} theirs→ours {} | their prove {} ms verify {} ms (dump {:.1?}); our setup {:.1?} prove {:.1?} verify {:.1?}",
                if passed { "PASS" } else { "FAIL" },
                if report.root_match { "ok" } else { "MISMATCH" },
                describe(report.spartan),
                describe(report.claim_h),
                describe(report.narg),
                report.narg_len.0,
                describe(report.hints),
                report.hints_len.0,
                if report.ours_on_theirs { "ok" } else { "REJECT" },
                if report.ours_on_ours { "ok" } else { "REJECT" },
                if theirs_on_ours { "ok" } else { "REJECT" },
                their_prove,
                their_verify,
                their_dump,
                report.setup,
                report.prove,
                report.verify
            );
            if passed && !keep {
                let _ = std::fs::remove_dir_all(&dir);
            }
        }
    }
    println!("{}: {cases} cases", if all_ok { "ALL PASS" } else { "FAILURES" });
    all_ok
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().map(String::as_str) == Some("--sweep") {
        let keep = args.iter().any(|a| a == "--keep");
        let sampled = args.iter().any(|a| a == "--sampled");
        let positional: Vec<&String> = args[1..].iter().filter(|a| *a != "--keep" && *a != "--sampled").collect();
        if positional.len() != 5 {
            eprintln!("usage: bitz_e2e_parity --sweep <their-examples-dir> <scratch-dir> <circuit> <blocks-list> <seeds> [--keep] [--sampled]");
            std::process::exit(2);
        }
        let ok = sweep(
            Path::new(positional[0]),
            Path::new(positional[1]),
            positional[2],
            &parse_list(positional[3]),
            positional[4],
            keep,
            sampled,
        );
        std::process::exit(if ok { 0 } else { 1 });
    }
    let dir = PathBuf::from(args.first().expect("usage: bitz_e2e_parity <dump-dir> | --sweep ..."));
    match check(&dir, true) {
        Ok(report) => {
            println!(
                "{} (blocks {}; setup {:.1?}, prove {:.1?}, verify {:.1?})",
                if report.passed() { "PASS" } else { "FAIL" },
                report.blocks,
                report.setup,
                report.prove,
                report.verify
            );
            if !report.passed() {
                std::process::exit(1);
            }
        }
        Err(e) => {
            println!("FAILED: {e}");
            std::process::exit(1);
        }
    }
    let _ = Gf::zero();
}

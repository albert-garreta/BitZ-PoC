#!/usr/bin/env python3
"""Import Zinc+ worker logs into a `mul-bench/v2` campaign directory.

The Zinc+ workers (benchmarks/zinc-plus/*.rs) run inside a zinc-plus
checkout and print one `ZINC_TRIAL {...}` line per timed repetition and one
`ZINC_RESULT {...}` line per case. This turns a directory of such logs into
the same `manifest.json` + `samples.jsonl` pair the Rust harness writes, so
`scripts/mul_report.py` and `scripts/mul_table.py` treat the Zinc+ rows like
every other backend's. A case is imported only if the worker verified its
proof and the digest of the limbs it actually proved equals the corpus
digest the BitZ harness records for the same operands.

Log layout (written by scripts/run_zinc_plus_campaign.py): `<name>.out`
(worker stdout), `<name>.err` (stderr, holds `/usr/bin/time -l` on macOS),
`<name>.meta.json` (workload, log_n, seed, threads, features, exit code,
executable hash). Measurement boundary of the imported rows:
`witness-to-proof-external` — the whole prover call is one number (its
commitment is not timed separately) and the peak resident set covers the
whole worker process (corpus load, setup, warm-up, all repetitions).
"""
from __future__ import annotations

import argparse
import hashlib
import json
import platform
import re
import subprocess
from pathlib import Path

SCHEMA = "mul-bench/v2"
BOUNDARY = "witness-to-proof-external"
RESULT_SCHEMAS = {"zinc-plus/u32-mod32/v1": "u32-mod32", "zinc-plus/wide-mul/v1": None}


def parse_log(out_path: Path):
    trials, result = [], None
    for line in out_path.read_text().splitlines():
        if line.startswith("ZINC_TRIAL "):
            trials.append(json.loads(line[len("ZINC_TRIAL "):]))
        elif line.startswith("ZINC_RESULT "):
            if result is not None:
                raise ValueError(f"{out_path}: two ZINC_RESULT lines")
            result = json.loads(line[len("ZINC_RESULT "):])
    if result is None:
        raise ValueError(f"{out_path}: no ZINC_RESULT line")
    schema = result.get("schema")
    if schema not in RESULT_SCHEMAS:
        raise ValueError(f"{out_path}: unknown worker schema {schema!r}")
    if result.get("proof_verified") is not True:
        raise ValueError(f"{out_path}: proof not verified")
    if result["input_digest"] != result["witness_digest"]:
        raise ValueError(f"{out_path}: the proved limbs do not recombine to the corpus rows")
    if len(trials) != result["reps"] or [t["rep"] for t in trials] != list(range(result["reps"])):
        raise ValueError(f"{out_path}: expected {result['reps']} trial lines")
    if len({t["proof_bytes"] for t in trials}) != 1:
        raise ValueError(f"{out_path}: proof size varies across repetitions")
    return trials, result


def time_l_peak_rss(err_path: Path):
    """`/usr/bin/time -l` maximum resident set size (bytes on macOS), if present."""
    if not err_path.exists():
        return None
    for line in err_path.read_text().splitlines():
        fields = line.split()
        if fields[1:] == ["maximum", "resident", "set", "size"] and fields[0].isdigit():
            return int(fields[0])
    return None


def machine():
    try:
        cpu = subprocess.run(["sysctl", "-n", "machdep.cpu.brand_string"], capture_output=True, text=True, check=True).stdout.strip()
    except Exception:
        cpu = platform.processor() or platform.machine()
    try:
        ram = int(subprocess.run(["sysctl", "-n", "hw.memsize"], capture_output=True, text=True, check=True).stdout.strip())
    except Exception:
        ram = None
    import os
    return {"cpu": cpu, "platform": platform.platform(), "logical_cpus": os.cpu_count(), "physical_memory_bytes": ram}


def import_logs(log_dir: Path, out: Path, provenance: dict):
    out.mkdir(parents=True, exist_ok=False)
    cases, samples = [], []
    metas = sorted(log_dir.glob("*.meta.json"))
    if not metas:
        raise ValueError(f"no *.meta.json case records in {log_dir}")
    for number, meta_path in enumerate(metas):
        meta = json.loads(meta_path.read_text())
        stem = meta_path.name[: -len(".meta.json")]
        identifier = f"case-{number:05d}"
        case = dict(mode="proof", workload=meta["workload"], backend="zinc-plus", log_n=meta["log_n"],
                    seed=meta["seed"], threads=meta["threads"], log_inv_rate=meta["log_inv_rate"])
        job = dict(proof_fingerprints=False, id=identifier, case=case, reps=meta["reps"], warmups=0,
                   memory="rss", skip=None, skip_unsupported=True, tuning_reps=0)
        if meta.get("exit_code") != 0 or not (log_dir / f"{stem}.out").exists():
            cases.append(dict(job=job, status="skipped", effective={},
                              reason=meta.get("reason") or f"worker exited with {meta.get('exit_code')}"))
            continue
        trials, result = parse_log(log_dir / f"{stem}.out")
        expected_workload = RESULT_SCHEMAS[result["schema"]] or result.get("workload")
        if expected_workload != meta["workload"] or result["exponent"] != meta["log_n"] or result["seed"] != meta["seed"]:
            raise ValueError(f"{stem}: worker result disagrees with the case record")
        if result["threads"] != meta["threads"] and not (result["threads"] == 0 and meta["threads"] > 1):
            raise ValueError(f"{stem}: worker thread count {result['threads']} vs case {meta['threads']}")
        if result["reps"] != meta["reps"]:
            raise ValueError(f"{stem}: worker repetitions {result['reps']} vs case {meta['reps']}")
        if meta.get("corpus_digest") and meta["corpus_digest"] != result["input_digest"]:
            raise ValueError(f"{stem}: corpus manifest digest differs from the worker's input digest")
        config = {k: result[k] for k in ["row_len", "num_rows", "columns", "limb_bits", "inverse_rate", "column_openings",
                                          "security_bits", "prime_bits", "grinding_bits", "logup_bits", "checks", "parallel"]}
        # Width of Zinc+'s integer combination ring (64 * LIMB_M); older
        # result lines predate the field.
        config["comb_ring_bits"] = result.get("comb_ring_bits")
        config.update(engine="Zinc+ (Zip+/IPRS over F65537, GKR-LogUp word range checks)",
                      statement={"u32-mod32": "8 int columns of 16-bit limbs; x·y = z + 2^32·w per row",
                                 "u64": "16 int columns of 16-bit limbs; x·y = z per row (z in 8 limbs)",
                                 "u128": "32 int columns of 16-bit limbs; x·y = z per row (z in 16 limbs)"}[meta["workload"]],
                      target_bits=result["security_bits"], prime_source="transcript",
                      proof_size_encoding="transcript bytes (Proof::get_num_bytes)", features=meta.get("features"))
        peaks = [p for p in (result.get("peak_rss_bytes"), time_l_peak_rss(log_dir / f"{stem}.err")) if p]
        if not peaks:
            raise ValueError(f"{stem}: no peak resident set recorded")
        cases.append(dict(job=job, status="measured", reason=None,
                          effective=dict(boundary=BOUNDARY, corpus_digest=result["input_digest"], config=config,
                                         representation="16-bit limb int columns", setup_ms=result["setup_ms"],
                                         warmup="one untimed proof before the timed repetitions (not recorded)")))
        for trial in trials:
            samples.append(dict(case_id=identifier, kind="sample", index=trial["rep"], verified=True,
                                metrics=dict(witness_ms=result["witness_ms"], online_prover_ms=trial["prove_ms"],
                                             witness_to_proof_ms=result["witness_ms"] + trial["prove_ms"],
                                             verify_ms=trial["verify_ms"], proof_bytes=trial["proof_bytes"],
                                             setup_ms=result["setup_ms"])))
        samples.append(dict(case_id=identifier, kind="rss", index=0, verified=True,
                            metrics=dict(peak_rss_bytes=max(peaks))))
    manifest = dict(schema=SCHEMA, target="zinc-plus-external", status="complete", provenance=provenance, cases=cases)
    (out / "manifest.json").write_text(json.dumps(manifest, indent=2) + "\n")
    (out / "samples.jsonl").write_text("".join(json.dumps(s) + "\n" for s in samples))
    return manifest


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("log_dir", type=Path)
    parser.add_argument("--out", type=Path, required=True, help="new campaign directory (must not exist)")
    parser.add_argument("--zinc-revision", required=True, help="zinc-plus commit the workers were built from")
    parser.add_argument("--zinc-repository", default="https://github.com/NethermindEth/zinc-plus")
    parser.add_argument("--zinc-toolchain", default="", help="rustc -V of the zinc-plus build")
    parser.add_argument("--worker-source", type=Path, nargs="*", default=[], help="worker source files to hash")
    parser.add_argument("--note", default="", help="free-text provenance note")
    args = parser.parse_args(argv)
    sources = {str(p): hashlib.sha256(p.read_bytes()).hexdigest() for p in args.worker_source}
    provenance = dict(machine=machine(), importer="scripts/zinc_plus_import.py",
                      zinc_plus=dict(repository=args.zinc_repository, revision=args.zinc_revision, toolchain=args.zinc_toolchain),
                      worker_source_sha256=sources, note=args.note,
                      rss_boundary="whole worker process: corpus load, setup, warm-up, all timed proofs and verifications "
                                   "(max of getrusage maximum RSS and /usr/bin/time -l), not a single-proof child")
    manifest = import_logs(args.log_dir, args.out, provenance)
    measured = sum(c["status"] == "measured" for c in manifest["cases"])
    print(f"imported {measured} measured and {len(manifest['cases']) - measured} skipped Zinc+ cases into {args.out}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

#!/usr/bin/env python3
"""Turn a Zinc+ u32 campaign into a run directory the paper exporter reads.

Zinc+ is measured by its own bench inside a pinned zinc-plus checkout
(`protocol/benches/f2z_u32_mod32.rs`, rate 1/4), not by
`benches/mul_e2e_compare.rs`, because the two crates pin incompatible
`crypto-bigint` releases. The bench proves the same corpus as every other
scheme in the table -- it derives the operands from
`native-mul/mod32/inputs/v1` with the campaign's seed and recomputes the
table's row digest from the limbs it actually proved -- so its rows can be
compared directly once they are written in the campaign schema.

    python3 scripts/zinc_plus_summary.py bench_results/zinc-plus-u32-20260914 \
        --machine-from PerfRuns/suite-u32-f2z-r2-t1 --out PerfRuns/zinc-plus-u32

The machine block is copied from a run of the same campaign (the exporter
refuses to mix measurement machines, and this campaign ran on the machine
that produced the other rows: `--machine-from` names it explicitly rather
than reconstructing it, so a mismatch shows up as a copied-from mistake
instead of a silent "same machine" claim).
"""
from __future__ import annotations

import argparse
import hashlib
import json
import statistics
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from native_mul_results import (POLICY, SAMPLE_SCHEMA, SUMMARY_SCHEMA,  # noqa: E402
                                fingerprint, validate_summary)

BACKEND = "zinc-plus"
WORKLOAD = "u32-mod32"
BENCH_SCHEMA = "zinc-plus/u32-mod32/v1"


def source_digest(tree: Path) -> str:
    """Hash of the measured sources: every .rs/.toml plus the lock file."""
    digest = hashlib.sha256()
    for path in sorted(p for p in tree.rglob("*")
                       if p.is_file() and (p.suffix in (".rs", ".toml") or p.name == "Cargo.lock")
                       and ".git" not in p.parts and "target" not in p.parts):
        digest.update(str(path.relative_to(tree)).encode())
        digest.update(path.read_bytes())
    return digest.hexdigest()


def provenance(clone: Path, machine: dict, revision: str, toolchain: str, threads: int) -> dict:
    lock = clone / "Cargo.lock"
    return {
        "repository": str(clone),
        "git_revision": revision,
        "git_dirty": True,  # the bench itself is not committed upstream
        "source_sha256": source_digest(clone),
        "cargo_lock_sha256": hashlib.sha256(lock.read_bytes()).hexdigest(),
        "build": {
            "rust_toolchain": toolchain,
            "rustflags": "-C target-cpu=native",
            "threads": threads,
            "profile": "bench",
        },
        "machine": machine,
    }


def config_of(result: dict) -> dict:
    """What the row proves and how, as the exporter's identity checks see it."""
    return {
        "engine": "zinc-plus",
        "statement": "8 int columns of 16-bit limbs; x·y = z + 2^32·w per row",
        "lookup": "Word{16} GKR-LogUp range check on every column",
        "pcs": "Zip+/IPRS over F65537",
        "inverse_rate": result["inverse_rate"],
        "column_openings": result["column_openings"],
        "target_bits": result["security_bits"],
        "grinding_bits": result["grinding_bits"],
        "prime_bits": result["prime_bits"],
        "prime_source": "transcript",
        "logup_bits": result["logup_bits"],
        "limb_bits": result["limb_bits"],
        "columns": result["columns"],
        "row_len": result["row_len"],
        "num_rows": result["num_rows"],
        "checks": result["checks"],
        "proof_size_encoding": "transcript bytes (Proof::get_num_bytes)",
    }


def trials(case_out: Path) -> list[dict]:
    return [json.loads(line[len("ZINC_TRIAL "):])
            for line in case_out.read_text().splitlines() if line.startswith("ZINC_TRIAL ")]


def metrics_of(result: dict, prove_ms: float, verify_ms: float, proof_bytes: int) -> dict:
    return {
        "witness_ms": result["witness_ms"],
        "online_prover_ms": prove_ms,
        "witness_to_proof_ms": result["witness_ms"] + prove_ms,
        "verify_ms": verify_ms,
        "proof_bytes": proof_bytes,
    }


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__,
                                 formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("campaign", type=Path, help="the campaign directory (results.jsonl + per-case output)")
    ap.add_argument("--clone", type=Path, required=True, help="the measured zinc-plus checkout")
    ap.add_argument("--revision", required=True, help="its git revision")
    ap.add_argument("--toolchain", required=True, help="the rustc that built the bench")
    ap.add_argument("--machine-from", type=Path, required=True,
                    help="a run directory of the same campaign, for the machine block")
    ap.add_argument("--out", type=Path, required=True, help="run directory to write")
    args = ap.parse_args()

    machine = json.loads((args.machine_from / "summary.json").read_text())[0]["provenance"]["machine"]
    results = [json.loads(line) for line in (args.campaign / "results.jsonl").read_text().splitlines() if line.strip()]

    summaries, samples = [], []
    for result in results:
        if result.get("schema") != BENCH_SCHEMA:
            raise ValueError(f"unexpected bench schema {result.get('schema')!r}")
        if result.get("proof_verified") is not True or result.get("exit") != 0:
            raise ValueError(f"unverified or failed case: {result}")
        if result["input_digest"] != result["witness_digest"]:
            raise ValueError("the proved limbs do not recombine to the corpus rows")
        exponent, threads = result["exponent"], result["threads"]
        case = args.campaign / f"t{threads}-e{exponent}.out"
        rows = trials(case)
        if len(rows) != result["reps"]:
            raise ValueError(f"{case.name}: {len(rows)} trials for {result['reps']} reps")
        proof_bytes = {row["proof_bytes"] for row in rows} | {result["proof_bytes"]}
        if len(proof_bytes) != 1:
            raise ValueError(f"{case.name}: proof size varies across repetitions")
        config = config_of(result)
        source = provenance(args.clone, machine, args.revision, args.toolchain, threads)
        common = dict(backend=BACKEND, workload=WORKLOAD, log_multiplications=exponent,
                      multiplications=1 << exponent, threads=threads, seed=result["seed"],
                      corpus_digest=result["input_digest"], config=config,
                      measurement_policy=POLICY, proof_verified=True, provenance=source)
        medians = {name: statistics.median(values) for name, values in {
            "online_prover_ms": [row["prove_ms"] for row in rows],
            "verify_ms": [row["verify_ms"] for row in rows],
        }.items()}
        summary = dict(schema=SUMMARY_SCHEMA, samples=len(rows), warmups=1,
                       setup_ms=result["setup_ms"],
                       medians=metrics_of(result, medians["online_prover_ms"], medians["verify_ms"],
                                          result["proof_bytes"]),
                       peak_rss_bytes=result["peak_rss_bytes"],
                       memory={"backend": BACKEND, "workload": WORKLOAD,
                               "log_multiplications": exponent,
                               "corpus_digest": result["input_digest"],
                               "peak_rss_bytes": result["peak_rss_bytes"],
                               "proof_bytes": result["proof_bytes"],
                               "proof_verified": True,
                               "boundary": ("fresh process: corpus generation, public setup, witness "
                                            "generation, proving and verification, warm-up included; "
                                            "the whole process maximum, not a per-proof figure"),
                               "config": config},
                       **common)
        summary["protocol_fingerprint"] = fingerprint(summary)
        validate_summary(summary)
        summaries.append(summary)
        for index, row in enumerate(rows):
            samples.append(dict(schema=SAMPLE_SCHEMA, setup_ms=result["setup_ms"],
                                trial={"index": index, "kind": "sample"},
                                metrics=metrics_of(result, row["prove_ms"], row["verify_ms"],
                                                   row["proof_bytes"]),
                                protocol_fingerprint=summary["protocol_fingerprint"], **common))

    args.out.mkdir(parents=True, exist_ok=True)
    (args.out / "summary.json").write_text(json.dumps(summaries, indent=1) + "\n")
    (args.out / "samples.jsonl").write_text("".join(json.dumps(row) + "\n" for row in samples))
    (args.out / "run.txt").write_text(
        f"Zinc+ u32 mod-2^32 rows, converted from {args.campaign} by {Path(__file__).name}.\n"
        f"zinc-plus {args.revision} at {args.clone}, rustc {args.toolchain}.\n"
        f"{len(summaries)} cases, {len(samples)} samples.\n")
    print(f"wrote {args.out} ({len(summaries)} cases, {len(samples)} samples)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

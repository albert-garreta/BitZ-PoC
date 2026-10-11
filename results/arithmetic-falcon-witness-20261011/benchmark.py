#!/usr/bin/env python3
"""Sequential matched arithmetic Falcon benchmarks; no source modifications."""
import argparse
import hashlib
import itertools
import json
import os
from pathlib import Path
import statistics
import subprocess
import sys
import time


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--baseline", type=Path, required=True)
    parser.add_argument("--candidate", type=Path, required=True)
    parser.add_argument("--output", type=Path, default=Path(__file__).parent)
    parser.add_argument("--iterations", type=int, default=3)
    parser.add_argument("--seeds", type=int, nargs="+", default=[42, 43])
    parser.add_argument("--degrees", type=int, nargs="+", default=[512, 1024])
    parser.add_argument("--securities", type=int, nargs="+", default=[100, 128])
    parser.add_argument("--batches", type=int, nargs="+", default=[32, 1024])
    parser.add_argument("--threads", type=int, nargs="+", default=[1, 8])
    parser.add_argument("--reference-candidate-results", type=Path)
    parser.add_argument("--max-prover-regression-percent", type=float,
                        help="Fail after recording all results if any shape's median total prover or proof-only time exceeds this regression percentage.")
    parser.add_argument("--trace", action="store_true")
    args = parser.parse_args()
    args.output.mkdir(parents=True, exist_ok=True)
    binaries = {"baseline": args.baseline.resolve(), "candidate": args.candidate.resolve()}
    metadata = {
        "started_unix": time.time(),
        "platform": subprocess.check_output(["uname", "-a"], text=True).strip(),
        "cpu": subprocess.check_output(["lscpu"], text=True),
        "binaries": {
            label: {"path": str(path), "sha256": hashlib.sha256(path.read_bytes()).hexdigest()}
            for label, path in binaries.items()
        },
        "arguments": {key: str(value) if isinstance(value, Path) else value for key, value in vars(args).items()},
        "ordering": "baseline then candidate for even-indexed seeds, reversed for odd-indexed seeds; all processes sequential",
        "build": "cargo build --locked --offline --release --features falcon-hybrid --example falcon_algebraic; RUSTFLAGS=-C target-cpu=native",
    }
    (args.output / "metadata.json").write_text(json.dumps(metadata, indent=2) + "\n")
    samples = []
    for degree, security, batch, threads in itertools.product(args.degrees, args.securities, args.batches, args.threads):
        if threads not in (1, 8):
            raise ValueError("CPU pinning is defined only for threads=1 or 8")
        cpus = "6" if threads == 1 else "6,7,4,5,2,3,0,1"
        for seed_index, seed in enumerate(args.seeds):
            order = list(binaries) if seed_index % 2 == 0 else list(reversed(binaries))
            pair = {}
            for label in order:
                stem = f"{label}-n{degree}-s{security}-b{batch}-t{threads}-seed{seed}"
                env = os.environ.copy()
                configured = {
                    "RAYON_NUM_THREADS": str(threads),
                    "OMP_NUM_THREADS": str(threads),
                    "BITZ_FALCON_MAIN_CPU": "6",
                    "BITZ_FALCON_WORKER_CPUS": cpus,
                    "BITZ_FALCON_CASE_CACHE": "/tmp/bitz-arithmetic-witness-cases-20261011",
                }
                for key in ["BITZ_FALCON_STAGE_TIMINGS", "BITZ_FALCON_PACKING", "BITZ_FALCON_FIXTURE_CACHE", "BITZ_FALCON_FIXTURE_PATH"]:
                    env.pop(key, None)
                env.update(configured)
                command = ["taskset", "-c", cpus, str(binaries[label]), "--degree", str(degree), "--security", str(security), "--batch", str(batch), "--threads", str(threads), "--seed", str(seed), "--warmup", "1", "--iterations", str(args.iterations)]
                if args.trace:
                    command.append("--trace")
                    env["BITZ_FALCON_STAGE_TIMINGS"] = "1"
                    configured["BITZ_FALCON_STAGE_TIMINGS"] = "1"
                (args.output / f"{stem}.command.json").write_text(json.dumps({"argv": command, "environment": configured}, indent=2) + "\n")
                print(stem, flush=True)
                with (args.output / f"{stem}.jsonl").open("w") as stdout, (args.output / f"{stem}.stderr.jsonl").open("w") as stderr:
                    subprocess.run(command, env=env, stdout=stdout, stderr=stderr, check=True)
                rows = [json.loads(line) for line in (args.output / f"{stem}.jsonl").read_text().splitlines()]
                assert len(rows) == args.iterations + 1
                assert all(row["verified"] for row in rows)
                assert all(row["threads"] == threads and row["auxiliary_pool_threads"] == threads for row in rows)
                if label == "candidate" and args.reference_candidate_results is not None:
                    reference_path = args.reference_candidate_results / f"{stem}.jsonl"
                    reference = [json.loads(line) for line in reference_path.read_text().splitlines()]
                    assert len(reference) == len(rows), "reference trial count mismatch"
                    for old, new in zip(reference, rows):
                        for key in ["input_digest", "trial", "iteration", "source_root", "proof_debug_digest", "proof_payload_bytes"]:
                            assert old[key] == new[key], f"candidate identity mismatch: {stem}: {key}"
                pair[label] = rows[0]["input_digest"]
                for row in rows:
                    if row["trial"] == "sample":
                        samples.append(dict(row, variant=label, committed_source_bits=row.get("committed_source_bits", row["source_bits"])))
                print(json.dumps({"variant": label, "total_prover_ms_median": statistics.median(row["total_prover_ms"] for row in rows if row["trial"] == "sample"), "proof_payload_bytes": rows[0]["proof_payload_bytes"]}), flush=True)
            assert pair["baseline"] == pair["candidate"], "input mismatch"
    (args.output / "samples.json").write_text(json.dumps(samples, indent=2) + "\n")
    metrics = ["witness_commit_ms", "prove_ms", "total_prover_ms", "verify_ms", "proof_payload_bytes", "live_bits_per_signature", "source_bits_per_signature", "committed_source_bits", "process_peak_rss_kib"]
    summary = []
    seed_summary = []
    for degree, security, batch, threads in itertools.product(args.degrees, args.securities, args.batches, args.threads):
        group = [row for row in samples if (row["degree"], row["security_bits"], row["batch"], row["threads"]) == (degree, security, batch, threads)]
        result = {"degree": degree, "security_bits": security, "batch": batch, "threads": threads}
        for label in binaries:
            values = [row for row in group if row["variant"] == label]
            result[label] = {metric: statistics.median(row[metric] for row in values) for metric in metrics}
        result["reduction_percent"] = {metric: 100 * (1 - result["candidate"][metric] / result["baseline"][metric]) for metric in metrics}
        summary.append(result)
        for seed in args.seeds:
            seed_result = {"degree": degree, "security_bits": security, "batch": batch, "threads": threads, "seed": seed}
            for label in binaries:
                values = [row for row in group if row["variant"] == label and row["seed"] == seed]
                seed_result[label] = {metric: statistics.median(row[metric] for row in values) for metric in metrics}
            seed_result["reduction_percent"] = {metric: 100 * (1 - seed_result["candidate"][metric] / seed_result["baseline"][metric]) for metric in metrics}
            seed_summary.append(seed_result)
    (args.output / "summary.json").write_text(json.dumps(summary, indent=2) + "\n")
    (args.output / "seed-summary.json").write_text(json.dumps(seed_summary, indent=2) + "\n")
    metadata["finished_unix"] = time.time()
    metadata["candidate_identity_checked_against_reference"] = args.reference_candidate_results is not None
    (args.output / "metadata.json").write_text(json.dumps(metadata, indent=2) + "\n")
    if args.max_prover_regression_percent is not None:
        regressions = [
            {**{key: row[key] for key in ["degree", "security_bits", "batch", "threads"]},
             "metric": metric,
             "baseline_ms": row["baseline"][metric],
             "candidate_ms": row["candidate"][metric],
             "regression_percent": -row["reduction_percent"][metric]}
            for row in summary
            for metric in ["total_prover_ms", "prove_ms"]
            if -row["reduction_percent"][metric] > args.max_prover_regression_percent
        ]
        gate = {"passed": not regressions,
                "metrics": ["total_prover_ms", "prove_ms"],
                "aggregation": "median over all measured trials and selected seeds, per shape",
                "maximum_regression_percent": args.max_prover_regression_percent,
                "regressions": regressions,
                "interpretation": "Observed comparison only; this gate is not a statistical confidence claim. Inspect seed-summary.json and broaden input coverage for borderline results."}
        (args.output / "regression-gate.json").write_text(json.dumps(gate, indent=2) + "\n")
        print(json.dumps({"regression_gate_passed": gate["passed"], "regressions": regressions}), flush=True)
        if regressions:
            sys.exit(3)


if __name__ == "__main__":
    main()

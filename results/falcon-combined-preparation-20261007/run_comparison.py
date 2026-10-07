#!/usr/bin/env python3
"""Compare the preserved two-step Falcon prover with combined preparation."""

import argparse
import csv
import datetime
import hashlib
import json
import os
from pathlib import Path
import statistics
import subprocess
import time


def utc_now():
    return datetime.datetime.now(datetime.timezone.utc).isoformat()


def sha256(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def preparation_ms(record):
    if record.get("preparation_mode") == "combined":
        return record["witness_commit_ms"]
    return record["witness_ms"] + record["commit_ms"]


def validate_records(records, variant, batch, security, threads):
    assert [r["trial"] for r in records] == ["warmup"] + ["sample"] * 5
    assert [r["iteration"] for r in records] == [None] + list(range(5))
    for record in records:
        assert record["threads"] == record["auxiliary_pool_threads"] == threads
        assert record["batch"] == batch and record["security_bits"] == security
        assert record["seed"] == 42 and record["algebraic_security_bits"] >= security
        assert record["build_rustflags"] == "-C target-cpu=native"
        assert record["live_bits_per_signature"] == 30747
        assert record["source_bits_per_signature"] == 65536
        affinity = record["cpu_affinity"]
        assert affinity["observed_worker_cpus"] == [[i] for i in range(threads)]
        assert affinity["observed_main_cpus"] == [0]
        assert affinity["observed_auxiliary_worker_cpus"] == [[0]] * threads
        if variant == "candidate":
            assert record["preparation_mode"] == "combined"
            assert "witness_ms" not in record and "commit_ms" not in record
        else:
            assert "witness_ms" in record and "commit_ms" in record
        assert preparation_ms(record) >= 0
        assert sum(size for _, size in record["proof_payload_breakdown"]) == record["proof_payload_bytes"]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--candidate", required=True, type=Path)
    parser.add_argument("--baseline", type=Path, default=Path("/tmp/falcon-combined-benchmark/falcon_algebraic_baseline"))
    parser.add_argument("--root", type=Path, default=Path("/home/john-wu/code/BitZ-pcs"))
    parser.add_argument("--out", required=True, type=Path)
    args = parser.parse_args()
    binaries = {"baseline": args.baseline.resolve(), "candidate": args.candidate.resolve()}
    for binary in binaries.values():
        if not binary.is_file():
            parser.error(f"Missing executable: {binary}")
    assert sha256(binaries["baseline"]) == "21107dc244838f2d0aae8059329c8e5dee908b376c137413edd567e411456397"
    assert binaries["baseline"] != binaries["candidate"]
    args.out.mkdir(parents=True, exist_ok=False)
    historical = args.root / "results/falcon-algebraic-1024-threads-20261007"
    tracked_sources = json.loads((historical / "source-sha256.json").read_text())
    source_hashes = {name: sha256(args.root / name) for name in tracked_sources}
    (args.out / "candidate-source-sha256.json").write_text(json.dumps(source_hashes, indent=2) + "\n")
    metadata = {
        "started_utc": utc_now(),
        "binaries": {key: {"path": str(value), "sha256": sha256(value)} for key, value in binaries.items()},
        "batches": [32, 1024], "security_bits": [100, 128],
        "threads": [1, 2, 4, 8, 16], "execution_order": [1, 16, 2, 8, 4],
        "warmup": 1, "measured_trials": 5, "seed": 42,
        "variant_order": "Adjacent baseline/candidate runs; first variant alternates per configuration",
        "preparation_normalization": "Baseline witness_ms + commit_ms per sample; candidate witness_commit_ms",
        "verification": "Each emitted trial follows successful proof verification; every process must exit zero",
        "cpu_policy": "Workers on physical CPUs 0..N-1; main and auxiliary workers on CPU 0; no SMT",
        "build": "release, default parallel feature + falcon-hybrid, -C target-cpu=native, fat LTO, one codegen unit",
        "preflight_loadavg": Path("/proc/loadavg").read_text().strip(),
        "governor": Path("/sys/devices/system/cpu/cpu0/cpufreq/scaling_governor").read_text().strip(),
        "cpu_topology": subprocess.check_output(["lscpu", "-e=CPU,CORE,SOCKET,NODE,ONLINE"], text=True),
        "commands": [],
    }

    def save_metadata():
        (args.out / "metadata.json").write_text(json.dumps(metadata, indent=2) + "\n")

    save_metadata()
    rows = []
    input_digests = {}
    proof_layouts = {}
    group_index = 0
    verified_proofs = 0
    for batch in metadata["batches"]:
        for security in metadata["security_bits"]:
            for threads in metadata["execution_order"]:
                cpus = ",".join(map(str, range(threads)))
                env = os.environ.copy()
                for key in ["PCS_TRACE", "FLOCK_COMMIT_TIMING", "LIGERITO_TRACE", "LIG_PROVE_TRACE", "LIG_VERIFY_TRACE"]:
                    env.pop(key, None)
                env.update({
                    "RAYON_NUM_THREADS": str(threads),
                    "BITZ_FALCON_WORKER_CPUS": cpus,
                    "BITZ_FALCON_MAIN_CPU": "0",
                    "BITZ_FALCON_CASE_CACHE": "/tmp/falcon-algebraic-cases-20261007",
                    "FLOCK_NO_PREFAULT": "1",
                })
                variants = ["baseline", "candidate"] if group_index % 2 == 0 else ["candidate", "baseline"]
                measured = {}
                for variant in variants:
                    prefix = f"batch-{batch}-security-{security}-threads-{threads}-{variant}"
                    cmd = ["taskset", "-c", cpus, str(binaries[variant]), "--batch", str(batch),
                           "--security", str(security), "--threads", str(threads),
                           "--warmup", "1", "--iterations", "5", "--seed", "42"]
                    metadata["commands"].append({
                        "name": prefix, "command": cmd,
                        "environment": {key: value for key, value in sorted(env.items())
                                        if key.startswith(("RAYON_", "BITZ_", "FLOCK_", "LIG_", "LIGERITO_", "PCS_"))},
                    })
                    save_metadata()
                    print(f"Starting {prefix}", flush=True)
                    start = time.monotonic()
                    with (args.out / f"{prefix}.jsonl").open("w") as stdout, (args.out / f"{prefix}.stderr").open("w") as stderr:
                        result = subprocess.run(cmd, cwd=args.root, env=env, stdout=stdout, stderr=stderr, timeout=900)
                    if result.returncode:
                        raise RuntimeError(f"{prefix} failed; see {args.out / f'{prefix}.stderr'}")
                    records = [json.loads(line) for line in (args.out / f"{prefix}.jsonl").read_text().splitlines()]
                    validate_records(records, variant, batch, security, threads)
                    for record in records:
                        digest = input_digests.setdefault(batch, record["input_digest"])
                        assert record["input_digest"] == digest, "Inputs changed across variants, threads, or security targets"
                        layout = (record["proof_payload_bytes"], record["proof_payload_breakdown"])
                        expected = proof_layouts.setdefault((batch, security), layout)
                        assert layout == expected, "Proof payload or breakdown changed"
                    verified_proofs += len(records)
                    measured[variant] = [record for record in records if record["trial"] == "sample"]
                    print(f"Finished {prefix} in {time.monotonic() - start:.1f}s", flush=True)
                row = {"batch": batch, "security_bits": security, "threads": threads, "samples_per_variant": 5}
                for variant in ["baseline", "candidate"]:
                    samples = measured[variant]
                    row[f"{variant}_witness_commit_ms"] = statistics.median(preparation_ms(record) for record in samples)
                    for key in ["prove_ms", "total_prover_ms", "verify_ms"]:
                        row[f"{variant}_{key}"] = statistics.median(record[key] for record in samples)
                    for stat, fn in [("min", min), ("max", max), ("stdev", statistics.stdev)]:
                        row[f"{variant}_total_prover_ms_{stat}"] = fn(record["total_prover_ms"] for record in samples)
                    row[f"{variant}_peak_rss_mib"] = max(record["process_peak_rss_kib"] for record in samples) / 1024
                for key in ["witness_commit_ms", "total_prover_ms"]:
                    row[f"{key}_reduction_percent"] = 100 * (1 - row[f"candidate_{key}"] / row[f"baseline_{key}"])
                row["proof_payload_bytes"] = proof_layouts[(batch, security)][0]
                rows.append(row)
                group_index += 1
                print(json.dumps(row), flush=True)

    rows.sort(key=lambda row: (row["batch"], row["security_bits"], row["threads"]))
    with (args.out / "summary.csv").open("w") as handle:
        writer = csv.DictWriter(handle, fieldnames=rows[0].keys())
        writer.writeheader()
        writer.writerows(rows)
    (args.out / "summary.json").write_text(json.dumps(rows, indent=2) + "\n")
    report = [
        "# Combined Falcon preparation comparison", "",
        "Medians of five measured trials after one warm-up. All 240 proofs verified. "
        "Inputs and proof payload breakdowns match between variants and across thread counts.", "",
        "| Batch | Security | Threads | Preparation before/after (ms) | Preparation reduction | Total before/after (ms) | Total reduction |",
        "|---:|---:|---:|---:|---:|---:|---:|",
    ]
    for row in rows:
        report.append(
            f"| {row['batch']} | {row['security_bits']} | {row['threads']} | "
            f"{row['baseline_witness_commit_ms']:.3f} / {row['candidate_witness_commit_ms']:.3f} | "
            f"{row['witness_commit_ms_reduction_percent']:.1f}% | "
            f"{row['baseline_total_prover_ms']:.3f} / {row['candidate_total_prover_ms']:.3f} | "
            f"{row['total_prover_ms_reduction_percent']:.1f}% |"
        )
    report += ["", "Preparation includes derivation, validation, packing, and the initial commitment. "
               "Baseline phases are added per sample before taking the median. Total prover also includes proof generation. "
               "Setup, input generation, public-target hashing, and input copies are excluded from both variants.", "",
               "Runs are sequential, with adjacent baseline/candidate configurations and alternating first variant. "
               "Workers use physical CPUs 0 through N−1, with main and auxiliary workers on CPU 0. "
               "Sixteen threads spans both CPU chiplets. Verification uses the configured pool. "
               "RSS includes input preparation and caches. Raw trials, commands, source hashes, and binary hashes are retained.", ""]
    (args.out / "REPORT.md").write_text("\n".join(report))
    metadata.update({"finished_utc": utc_now(), "verified_proofs": verified_proofs,
                     "input_digests": input_digests,
                     "proof_payloads": {f"{batch}-{security}": value for (batch, security), value in proof_layouts.items()}})
    save_metadata()


if __name__ == "__main__":
    main()

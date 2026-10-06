#!/usr/bin/env python3
"""Reproduce the local matched V3/V4 campaign using binaries in manifest.json."""
import argparse
import hashlib
import json
import math
import os
from pathlib import Path
import statistics
import subprocess
import sys
import time

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT / "scripts"))
from bench_gate import idle_percent, swap_used_gb
from falcon_v2_campaign import MATCHED, environment, process_order, validate_profile

OUT = Path(__file__).resolve().parent
METRICS = ("total_prover_ms", "proof_verify_ms", "witness_commit_ms", "proof_prove_ms")


def digest(path):
    with open(path, "rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def save(path, data):
    path.write_text(json.dumps(data, indent=2, allow_nan=False) + "\n")


def validate(rows, label, security, batch, threads, seed, warmup, measured, stages):
    assert len(rows) == 2 + warmup + measured
    header, final = rows[0], rows[-1]
    assert header["event"] == "prepared" and final["event"] == "summary"
    assert header["protocol"] == f"bitz/falcon1024-ct/hybrid/shared-prime/non-zk/{label}"
    # V4 retains the V3 field and integer bridge at each target.
    validate_profile(dict(header, protocol="bitz/falcon1024-ct/hybrid/shared-prime/non-zk/v3"), "v3", security)
    assert header["stage_timings"] == stages
    for key, value in zip(("security_target", "batch", "threads", "input_seed", "warmup_trials", "measured_trials"),
                          (security, batch, threads, seed, warmup, measured)):
        assert header[key] == value, key
    terms = header["security_terms"]
    assert len({t["name"] for t in terms}) == len(terms)
    assert all(math.isfinite(t["error_bound"]) and t["error_bound"] >= 0 for t in terms)
    error = sum(t["error_bound"] for t in terms)
    assert 0 < error <= 2.0 ** -security
    assert math.isclose(-math.log2(error), header["algebraic_security_bits"], rel_tol=1e-12)
    trials = rows[1:-1]
    assert all(r["event"] == "trial" and r["verified"] is True for r in trials)
    assert final["verified"] is True and final["samples"] == measured
    assert [r["trial"] for r in trials] == ["warmup"] * warmup + ["sample"] * measured
    assert len({r["proof_debug_digest"] for r in trials}) == 1
    assert len({r["proof_payload_bytes"] for r in trials}) == 1
    for r in trials:
        assert all(r[k] == header[k] for k in ("batch", "capacity", "threads", "security_target", "input_digest"))
        assert all(math.isfinite(r[k]) and r[k] > 0 for k in METRICS)
        assert math.isclose(r["total_prover_ms"], r["witness_commit_ms"] + r["proof_prove_ms"], rel_tol=1e-12)
        assert r["proof_payload_definition"] == "canonical stored payload; excludes Falcon framing and public statement"
        assert r["proof_payload_bytes"] > 0
    samples = trials[warmup:]
    assert [r["sample"] for r in samples] == list(range(1, measured + 1))
    assert all(final["median_" + k] == statistics.median(r[k] for r in samples) for k in METRICS)
    return header


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--mode", choices=("latency", "cold", "stages"), default="latency")
    parser.add_argument("--batches", nargs="+", type=int, default=[1024])
    parser.add_argument("--seeds", nargs="+", type=int, default=list(range(42, 47)))
    parser.add_argument("--targets", nargs="+", type=int, default=[100, 128])
    parser.add_argument("--threads", type=int, default=16)
    args = parser.parse_args()
    warmup, measured = (1, 3) if args.mode == "latency" else (0, 1)
    manifest = json.loads((OUT / "manifest.json").read_text())
    dest = OUT / args.mode
    dest.mkdir(exist_ok=True)
    index = 0
    for batch in args.batches:
        for seed in args.seeds:
            # Alternate target order across seeds too.
            targets = args.targets if seed % 2 == 0 else list(reversed(args.targets))
            for security in targets:
                headers = {}
                for label in process_order(["v3", "v4"], index):
                    item = manifest["binaries"][label]
                    assert digest(item["binary"]) == item["sha256"]
                    stem = f"s{security}-b{batch}-t{args.threads}-seed{seed}-{label}"
                    record_path = dest / (stem + ".run.json")
                    assert not record_path.exists(), stem + " already exists"
                    idle = idle_percent()
                    assert idle >= 88, f"host busy before {stem}: {idle:.1f}% idle"
                    command = [item["binary"], "--protocol", "shared-prime", "--security", str(security),
                               "--batch", str(batch), "--threads", str(args.threads), "--seed", str(seed),
                               "--warmup", str(warmup), "--iterations", str(measured)]
                    env = environment(args.threads)
                    if args.mode == "stages":
                        env["BITZ_FALCON_STAGE_TIMINGS"] = "1"
                    record = dict(command=command, label=label, binary_sha256=item["sha256"],
                                  started=time.time(), idle_before_percent=idle, swap_before_gib=swap_used_gb())
                    save(record_path, dict(record, status="running"))
                    print("START " + args.mode + " " + stem, flush=True)
                    start = time.monotonic()
                    with (dest / (stem + ".jsonl")).open("w") as stdout, (dest / (stem + ".stderr")).open("w") as stderr:
                        process = subprocess.run(command, stdout=stdout, stderr=stderr, env=env, timeout=300)
                    record.update(elapsed_seconds=time.monotonic() - start, returncode=process.returncode,
                                  swap_after_gib=swap_used_gb())
                    save(record_path, dict(record, status="finished"))
                    assert process.returncode == 0, stem
                    assert record["swap_after_gib"] - record["swap_before_gib"] <= 0.1, "swap grew"
                    rows = [json.loads(line) for line in (dest / (stem + ".jsonl")).read_text().splitlines()]
                    headers[label] = validate(rows, label, security, batch, args.threads, seed, warmup, measured, args.mode == "stages")
                    save(record_path, dict(record, status="validated"))
                    print("DONE " + stem + " " + json.dumps({k: rows[-1]["median_" + k] for k in METRICS}), flush=True)
                assert all(headers["v3"][k] == headers["v4"][k] for k in MATCHED), "unmatched workload"
                print("MATCHED " + str((security, batch, args.threads, seed)), flush=True)
                index += 1


if __name__ == "__main__":
    main()

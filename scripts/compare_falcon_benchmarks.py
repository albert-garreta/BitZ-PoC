#!/usr/bin/env python3
"""Validate complete Falcon campaigns and apply a strict no-increase gate.

Usage: compare_falcon_benchmarks.py BASELINE_DIRECTORY CANDIDATE_DIRECTORY
Each directory contains manifest.json (bitz/falcon-campaign/v1) and, per case,
<label>.jsonl plus <label>.memory.jsonl. Case entries may override those paths
with samples_file/memory_file. Initial-baseline manifests are accepted for
reporting, but cannot pass the timing gate.

The required matrix is batches 1,3,32,1024 x threads 1,16 x security 100,128.
Acceptance campaigns set kind="paired", share a nonempty pairing_id, and give
all case entries pair_id and order (AB or BA). Each matrix cell requires six
pairs for each seed 42..46, balanced 3 AB/3 BA, two warmups and three measured
proofs per process. A process median is one observation, never three pairs.
Fresh-process RSS is compared by maximum within each seed. Payload is checked
for every pair. Timing uses the existing deterministic paired 95% bootstrap;
all interval upper bounds must be <= 1.0. No slowdown tolerance is supported.
Exit codes: 0 pass, 1 regression, 2 inconclusive or invalid input.
"""
import argparse
from collections import Counter, defaultdict
import json
import math
from pathlib import Path
import re
import statistics
import sys

from bench_statistics import classify_interval, paired_interval, timing_ratios

SCHEMA = "bitz/falcon-hybrid/v3"
PAYLOAD = "canonical stored payload; excludes Falcon framing and public statement"
SEEDS = tuple(range(42, 47))
TIMINGS = ("total_prover_ms", "proof_verify_ms")
EXPECTED_CELLS = {(security, batch, threads) for security in (100, 128) for batch in (1, 3, 32, 1024) for threads in (1, 16)}
PROVENANCE = ("host", "architecture", "rustc", "lock_sha256", "rustflags", "features", "profile", "cpu_model", "affinity")
MATCHED = ("batch", "capacity", "security_target", "threads", "target_arch", "gf128_kernel", "compiled_target_features", "integer_bridge", "input_source", "input_implementation_version", "input_mode", "input_rng", "input_seed", "input_count", "input_digest", "public_inputs", "source_bits_per_signature", "native_verify_includes_public_key_decode", "warmup_trials", "measured_trials", "build_rustflags", "runtime_rustflags")
REPLACED_NATIVE_TERMS = {"native ideal batching and projection", "native coordinate carry batching"}


def require(condition, message):
    if not condition:
        raise ValueError(message)


def positive(value):
    return type(value) in (int, float) and math.isfinite(value) and value > 0


def child_path(directory, name):
    require(isinstance(name, str) and name, "missing artifact filename")
    path = (directory / name).resolve()
    require(path.is_relative_to(directory.resolve()), "artifact outside campaign directory")
    return path


def read_process(path, warmup, measured):
    rows = [json.loads(line) for line in path.read_text().splitlines() if line.strip()]
    require(all(row.get("schema") == SCHEMA for row in rows), f"invalid schema: {path}")
    prepared = [row for row in rows if row.get("event") == "prepared"]
    summaries = [row for row in rows if row.get("event") == "summary"]
    trials = [row for row in rows if row.get("event") == "trial"]
    require(len(prepared) == len(summaries) == 1 and len(rows) == len(trials) + 2, f"incomplete process: {path}")
    header, summary = prepared[0], summaries[0]
    require(header.get("warmup_trials") == warmup and header.get("measured_trials") == measured, f"trial configuration: {path}")
    require(header.get("stage_timings") is False, f"instrumented latency process: {path}")
    require(header.get("security_target") in (100, 128), f"invalid security target: {path}")
    require(header.get("algebraic_security_bits", 0) >= header["security_target"], f"security below target: {path}")
    terms = header.get("security_terms", [])
    require(terms and len({term["name"] for term in terms}) == len(terms), f"missing/duplicate security terms: {path}")
    require(all(type(term["error_bound"]) in (int, float) and math.isfinite(term["error_bound"]) and term["error_bound"] >= 0 for term in terms), f"invalid security error: {path}")
    error = sum(term["error_bound"] for term in terms)
    require(0 < error <= 2.0 ** -header["security_target"], f"security ledger misses target: {path}")
    require(math.isclose(-math.log2(error), header["algebraic_security_bits"], rel_tol=1e-12), f"security ledger total mismatch: {path}")
    samples = [row for row in trials if row.get("trial") == "sample"]
    warmups = [row for row in trials if row.get("trial") == "warmup"]
    require(len(trials) == warmup + measured and len(samples) == measured and len(warmups) == warmup, f"wrong trial counts: {path}")
    require([row.get("sample") for row in samples] == list(range(1, measured + 1)), f"wrong sample numbering: {path}")
    require(all(row.get("sample") is None for row in warmups), f"numbered warmup: {path}")
    require(summary.get("samples") == measured and summary.get("verified") is True, f"incomplete summary: {path}")
    require(all(summary.get(key) == header.get(key) for key in ("batch", "capacity", "security_target", "threads")), f"summary configuration drift: {path}")
    for row in trials:
        require(row.get("verified") is True, f"unverified proof: {path}")
        require(all(row.get(key) == header.get(key) for key in ("batch", "capacity", "security_target", "threads", "input_digest")), f"trial configuration drift: {path}")
        require(all(positive(row.get(key)) for key in TIMINGS + ("witness_commit_ms", "proof_prove_ms")), f"invalid timing: {path}")
        require(math.isclose(row["total_prover_ms"], row["witness_commit_ms"] + row["proof_prove_ms"], rel_tol=1e-12), f"prover accounting mismatch: {path}")
        require(type(row.get("proof_payload_bytes")) is int and row["proof_payload_bytes"] > 0 and row.get("proof_payload_definition") == PAYLOAD, f"payload accounting mismatch: {path}")
        require(type(row.get("process_peak_rss_kib")) is int and row["process_peak_rss_kib"] > 0, f"missing RSS: {path}")
    for key in TIMINGS:
        require(summary.get("median_" + key) == statistics.median(row[key] for row in samples), f"summary timing mismatch: {path}")
    require(len({row.get("proof_debug_digest") for row in trials}) == 1 and all(re.fullmatch(r"[0-9a-f]{64}", row.get("proof_debug_digest", "")) for row in trials), f"proof changed between repeats: {path}")
    return {"prepared": header, "samples": samples, "summary": summary, "security_error": error}


def read_campaign(directory):
    directory = Path(directory)
    manifest = json.loads((directory / "manifest.json").read_text())
    require(manifest.get("schema") == "bitz/falcon-campaign/v1" and manifest.get("status") == "complete", "campaign incomplete or unknown schema")
    for key in ("revision", "binary_sha256", "lock_sha256"):
        require(re.fullmatch(r"(?:[0-9a-f]{40}|[0-9a-f]{64})", manifest.get(key, "")), f"missing provenance: {key}")
    require(all(key in manifest for key in PROVENANCE), "missing build/host provenance")
    blocks = {}
    for entry in manifest["cases"]:
        label = entry["label"]
        require(entry.get("exit_code") == 0, f"failed case: {label}")
        process = read_process(child_path(directory, entry.get("samples_file", label + ".jsonl")), manifest["warmup"], manifest["iterations"])
        memory = read_process(child_path(directory, entry.get("memory_file", label + ".memory.jsonl")), 0, 1)
        p, mp = process["prepared"], memory["prepared"]
        require(all(p.get(key) == mp.get(key) for key in MATCHED if key not in ("warmup_trials", "measured_trials", "build_rustflags", "runtime_rustflags")), f"memory/input mismatch: {label}")
        require(p.get("protocol") == mp.get("protocol"), f"memory/protocol mismatch: {label}")
        key = (p["security_target"], p["batch"], p["threads"], p["input_seed"], entry.get("pair_id", label))
        require(key not in blocks, f"duplicate process block: {key}")
        process.update(pair_id=entry.get("pair_id"), order=entry.get("order"), rss=memory["samples"][0]["process_peak_rss_kib"])
        if "prepared" in entry:
            require(entry["prepared"] == p, f"manifest/prepared mismatch: {label}")
        if "summary" in entry:
            require(entry["summary"] == process["summary"], f"manifest/summary mismatch: {label}")
        if "fresh_process_peak_rss_kib" in entry:
            require(entry["fresh_process_peak_rss_kib"] == process["rss"], f"manifest/RSS mismatch: {label}")
        blocks[key] = process
    expected = {(s, b, t, seed) for s in manifest["security"] for b in manifest["batches"] for t in manifest["threads"] for seed in manifest["seeds"]}
    require({key[:4] for key in blocks} == expected, "campaign workload matrix incomplete")
    return manifest, blocks


def compare(baseline, candidate):
    bm, bs = baseline
    cm, cs = candidate
    require(all(bm[key] == cm[key] for key in PROVENANCE), "build/host mismatch")
    require(bs.keys() == cs.keys(), "unmatched process blocks/inputs")
    paired = bm.get("kind") == cm.get("kind") == "paired" and bool(bm.get("pairing_id")) and bm.get("pairing_id") == cm.get("pairing_id")
    cells = defaultdict(list)
    for key, before in bs.items():
        after = cs[key]
        a, b = before["prepared"], after["prepared"]
        require(all(a.get(field) == b.get(field) for field in MATCHED), f"case configuration mismatch: {key}")
        require(after["security_error"] <= before["security_error"], f"weaker security ledger: {key}")
        old_terms = {term["name"]: term["error_bound"] for term in a["security_terms"]}
        new_terms = {term["name"]: term["error_bound"] for term in b["security_terms"]}
        require(all(name in new_terms and new_terms[name] <= error for name, error in old_terms.items() if name not in REPLACED_NATIVE_TERMS), f"retained security term weakened or missing: {key}")
        require(before["order"] == after["order"], f"pair order mismatch: {key}")
        cells[key[:3]].append((key[3], before, after))
    results = []
    for (security, batch, threads), blocks in sorted(cells.items()):
        seed_counts = Counter(seed for seed, _, _ in blocks)
        orders = {seed: Counter(before["order"] for s, before, _ in blocks if s == seed) for seed in seed_counts}
        complete = paired and seed_counts == Counter({seed: 6 for seed in SEEDS}) and all(count == Counter({"AB": 3, "BA": 3}) for count in orders.values()) and bm["warmup"] == cm["warmup"] == 2 and bm["iterations"] == cm["iterations"] == 3 and all(before["pair_id"] is not None for _, before, _ in blocks)
        timings = {}
        for field in TIMINGS:
            old = [before["summary"]["median_" + field] for _, before, _ in blocks]
            new = [after["summary"]["median_" + field] for _, _, after in blocks]
            ratios = timing_ratios(old, new)
            interval = paired_interval(ratios) if complete else None
            timings[field] = {"baseline_median": statistics.median(old), "candidate_median": statistics.median(new), "paired_geomean_ratio": math.exp(statistics.mean(map(math.log, ratios))), "interval_95": interval, "status": classify_interval(interval) if interval else "inconclusive"}
        payload_pass = all(max(row["proof_payload_bytes"] for row in after["samples"]) <= max(row["proof_payload_bytes"] for row in before["samples"]) for _, before, after in blocks)
        memory_pass = all(max(after["rss"] for s, _, after in blocks if s == seed) <= max(before["rss"] for s, before, _ in blocks if s == seed) for seed in seed_counts)
        statuses = [item["status"] for item in timings.values()] + ["pass" if payload_pass else "regression", "pass" if memory_pass else "regression"]
        status = "regression" if "regression" in statuses else "inconclusive" if "inconclusive" in statuses else "pass"
        results.append({"security": security, "batch": batch, "threads": threads, "process_pairs": len(blocks), "acceptance_pairing_complete": complete, "timings": timings, "payload": {"status": "pass" if payload_pass else "regression", "baseline_max": max(row["proof_payload_bytes"] for _, before, _ in blocks for row in before["samples"]), "candidate_max": max(row["proof_payload_bytes"] for _, _, after in blocks for row in after["samples"])}, "fresh_process_rss_kib": {"status": "pass" if memory_pass else "regression", "baseline_max": max(before["rss"] for _, before, _ in blocks), "candidate_max": max(after["rss"] for _, _, after in blocks)}, "status": status})
    statuses = [cell["status"] for cell in results]
    matrix_complete = set(cells) == EXPECTED_CELLS
    status = "regression" if "regression" in statuses else "inconclusive" if "inconclusive" in statuses or not matrix_complete else "pass"
    return {"schema": "bitz/falcon-comparison/v1", "matrix_complete": matrix_complete, "baseline_revision": bm["revision"], "candidate_revision": cm["revision"], "threshold": 1.0, "confidence_scope": "95% per timing metric and matrix cell; not simultaneous confidence", "status": status, "cells": results}


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("baseline", type=Path)
    parser.add_argument("candidate", type=Path)
    parser.add_argument("--output", type=Path)
    args = parser.parse_args()
    try:
        report = compare(read_campaign(args.baseline), read_campaign(args.candidate))
    except (ValueError, OSError, KeyError, TypeError) as error:
        print(f"invalid Falcon comparison: {error}", file=sys.stderr)
        return 2
    encoded = json.dumps(report, indent=2, allow_nan=False) + "\n"
    if args.output:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(encoded)
    else:
        print(encoded, end="")
    return {"pass": 0, "regression": 1, "inconclusive": 2}[report["status"]]


if __name__ == "__main__":
    sys.exit(main())

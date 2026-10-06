#!/usr/bin/env python3
"""Aggregate paired process medians, preserving seeds as statistical units."""
import json
import math
from pathlib import Path
import statistics
import sys

OUT = Path(__file__).resolve().parent
sys.path.insert(0, str(OUT.parents[2] / "scripts"))
from bench_statistics import paired_interval, sample_statistics
from falcon_v2_campaign import MATCHED

METRICS = ("total_prover_ms", "proof_verify_ms", "witness_commit_ms", "proof_prove_ms", "proof_payload_bytes")
records = {}
latency_proofs = {}
latency_headers = {}
for path in sorted((OUT / "latency").glob("*.jsonl")):
    run = json.loads(path.with_suffix(".run.json").read_text())
    assert run["status"] == "validated", path
    rows = [json.loads(line) for line in path.read_text().splitlines()]
    h = rows[0]
    label = h["protocol"].rsplit("/", 1)[-1]
    case = (h["security_target"], h["batch"], h["threads"])
    samples = [r for r in rows if r.get("trial") == "sample"]
    latency_proofs[path.name] = samples[0]
    latency_headers[path.name] = h
    values = {k: statistics.median(r[k] for r in samples) for k in METRICS}
    values["nonce_prefixes"] = {r["category"]: int(r["serial_nonce_prefix_sum"])
                                for r in samples[0]["grinding_diagnostics"]["categories"]}
    values["security_bits"] = h["algebraic_security_bits"]
    records.setdefault(case, {}).setdefault(h["input_seed"], {})[label] = values

summary = []
for (security, batch, threads), pairs in sorted(records.items()):
    assert all(set(pair) == {"v3", "v4"} for pair in pairs.values())
    entry = dict(security=security, batch=batch, threads=threads, seeds=sorted(pairs), metrics={})
    for metric in METRICS:
        old = [pairs[seed]["v3"][metric] for seed in sorted(pairs)]
        new = [pairs[seed]["v4"][metric] for seed in sorted(pairs)]
        ratios = [b / a for a, b in zip(old, new)]
        entry["metrics"][metric] = dict(v3=sample_statistics(old), v4=sample_statistics(new),
            paired_ratios=ratios, paired_geomean_ratio=math.exp(statistics.mean(map(math.log, ratios))),
            paired_bootstrap_95_ci=paired_interval(ratios))
    entry["grinding_nonce_prefix_means"] = {
        label: {category: statistics.mean(pairs[seed][label]["nonce_prefixes"][category] for seed in pairs)
                for category in pairs[next(iter(pairs))][label]["nonce_prefixes"]}
        for label in ("v3", "v4")}
    entry["paired_process_medians"] = pairs
    summary.append(entry)

for name, proof in latency_proofs.items():
    if name.endswith("-v3.jsonl"):
        other = name.replace("-v3.jsonl", "-v4.jsonl")
        assert proof["roots"] == latency_proofs[other]["roots"]
        assert all(latency_headers[name][k] == latency_headers[other][k] for k in MATCHED)

for mode in ("cold", "stages"):
    for path in sorted((OUT / mode).glob("*.jsonl")):
        rows = [json.loads(line) for line in path.read_text().splitlines()]
        proof = rows[1]
        assert proof["proof_debug_digest"] == latency_proofs[path.name]["proof_debug_digest"]
        assert proof["proof_payload_bytes"] == latency_proofs[path.name]["proof_payload_bytes"]

cold = []
for path in sorted((OUT / "cold").glob("*.jsonl")):
    run = json.loads(path.with_suffix(".run.json").read_text())
    assert run["status"] == "validated", path
    rows = [json.loads(line) for line in path.read_text().splitlines()]
    h = rows[0]
    cold.append(dict(protocol=h["protocol"], security=h["security_target"], batch=h["batch"],
                     threads=h["threads"], seed=h["input_seed"], process_peak_rss_kib=rows[-1]["process_peak_rss_kib"]))

stages = []
for path in sorted((OUT / "stages").glob("*.stderr")):
    rows = [json.loads(line) for line in path.read_text().splitlines()]
    forest = [r for r in rows if r["name"] == "falcon_arithmetic:compaction_forest"]
    assert len(forest) == 1
    grinding = sum(r["elapsed_ms"] for r in rows if r["name"] == "spartan:grinding"
                   and "falcon_arithmetic:compaction_forest" in r["path"])
    stages.append(dict(run=path.stem, forest_ms=forest[0]["elapsed_ms"], forest_grinding_ms=grinding,
                       forest_non_grinding_ms=forest[0]["elapsed_ms"] - grinding))

result = dict(description="Ratios are V4/V3. Each seed contributes one process median (1 warmup, 3 measured proofs). "
                          "Intervals bootstrap paired seed log ratios; they are exploratory, not simultaneous qualification bounds.",
              cases=summary, cold_process_peak_memory=cold, single_trial_stage_diagnostics=stages,
              audit="Paired commitment roots match. Cold/instrumented proofs match the corresponding latency proof digest and payload size.")
(OUT / "summary.json").write_text(json.dumps(result, indent=2, allow_nan=False) + "\n")
for case in summary:
    print(case["security"], case["batch"], case["threads"])
    for metric, data in case["metrics"].items():
        print(metric, round(data["v3"]["median"], 3), "->", round(data["v4"]["median"], 3),
              "ratio", round(data["paired_geomean_ratio"], 4), "95% CI", [round(v, 4) for v in data["paired_bootstrap_95_ci"]])

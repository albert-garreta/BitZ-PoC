#!/usr/bin/env python3
"""Summarize completed paired qualification without rerunning any timings."""
import argparse
import json
from pathlib import Path

DIMS = ("degree", "security_bits", "batch", "threads")
METRICS = ("prove_ms", "total_prover_ms")


def key(row):
    return tuple(row[k] for k in DIMS)


def bounds(rows, value):
    low = min(rows, key=value)
    high = max(rows, key=value)
    return {
        "minimum": {"value": value(low), "shape": {k: low[k] for k in DIMS}},
        "maximum": {"value": value(high), "shape": {k: high[k] for k in DIMS}},
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("directory", type=Path)
    directory = parser.parse_args().directory
    comparison = directory / "comparisons" / "padding-vs-v7"
    pooled = json.loads((comparison / "summary.json").read_text())
    paired = json.loads((comparison / "paired-seed-analysis.json").read_text())["shapes"]
    by_key = {key(r): r for r in paired}
    assert len(by_key) == len(pooled) == 70
    groups = {"all": pooled, "edges": [r for r in pooled if r["batch"] not in (1, 32, 1024)]}
    groups.update({f"batch_{batch}": [r for r in pooled if r["batch"] == batch] for batch in (1, 32, 1024)})
    summary = {
        "interpretation": "Positive gains mean faster/smaller. Point ranges compare different shapes, not confidence bounds. Bootstrap intervals are descriptive, seed-paired, with no across-shape multiplicity correction.",
        "shape_count": len(pooled),
        "groups": {},
        "confidence": {},
        "auxiliary_metrics": {},
    }
    for name, rows in groups.items():
        summary["groups"][name] = {"shapes": len(rows), "metrics": {}}
        for metric in METRICS:
            summary["groups"][name]["metrics"][metric] = {
                "pooled_median_gain_percent": bounds(rows, lambda r: r["reduction_percent"][metric]),
                "paired_seed_median_gain_percent": bounds(rows, lambda r: -by_key[key(r)]["metrics"][metric]["paired_seed_median_delta_percent"]),
            }
    positive_union = []
    for metric in METRICS:
        summary["confidence"][metric] = {
            "positive_pooled_delta_shapes": sum(r["reduction_percent"][metric] < 0 for r in pooled),
            "positive_paired_median_delta_shapes": sum(r["metrics"][metric]["paired_seed_median_delta_percent"] > 0 for r in paired),
        }
        for estimator in ("median", "geomean"):
            field = f"paired_seed_{estimator}_bootstrap_95_percent_interval"
            intervals = [r["metrics"][metric][field] for r in paired]
            summary["confidence"][metric][estimator] = {
                "positive_lower_endpoints": sum(lo > 0 for lo, hi in intervals),
                "negative_upper_endpoints": sum(hi < 0 for lo, hi in intervals),
                "maximum_upper_endpoint_percent": max(hi for lo, hi in intervals),
            }
            for r in paired:
                if r["metrics"][metric][field][0] > 0:
                    positive_union.append({**{k: r[k] for k in DIMS}, "metric": metric, "estimator": estimator, "interval": r["metrics"][metric][field]})
    changed = [r for r in pooled if r["candidate"]["arithmetic_capacity"] < r["baseline"]["capacity"]]
    unchanged = [r for r in pooled if r not in changed]
    for name, rows in (("all", pooled), ("changed_layout", changed), ("unchanged_layout", unchanged)):
        summary["auxiliary_metrics"][name] = {"shapes": len(rows)}
        for metric in ("proof_payload_bytes", "verify_ms", "process_peak_rss_kib"):
            summary["auxiliary_metrics"][name][metric] = bounds(rows, lambda r: r["reduction_percent"][metric])
        summary["auxiliary_metrics"][name]["proof_payload_bytes_saved"] = bounds(rows, lambda r: r["baseline"]["proof_payload_bytes"] - r["candidate"]["proof_payload_bytes"])
    (directory / "confidence-positive-union.json").write_text(json.dumps(positive_union, indent=2) + "\n")
    (directory / "qualification-summary.json").write_text(json.dumps(summary, indent=2) + "\n")
    print(json.dumps({"shapes": len(pooled), "confidence": summary["confidence"], "confidence_positive_union": positive_union}, indent=2))


if __name__ == "__main__":
    main()

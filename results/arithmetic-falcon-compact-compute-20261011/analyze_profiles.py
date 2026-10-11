#!/usr/bin/env python3
"""Summarize diagnostic spans separately from untraced latency benchmarks."""
import json
from pathlib import Path
import statistics
import sys


def summarize(directory):
    metadata_path = directory / "metadata.json"
    warmup = json.loads(metadata_path.read_text())["plan"]["warmup"] if metadata_path.exists() else 1
    reports = []
    for path in sorted(directory.glob("*.stderr.jsonl")):
        events = []
        for line in path.read_text().splitlines():
            try:
                event = json.loads(line)
            except json.JSONDecodeError:
                continue
            if event.get("event") == "stage":
                events.append(event)
        if not events:
            continue
        results = {}
        for operation in ["commit", "prove", "verify"]:
            roots = [event for event in events if event["name"] == f"falcon_algebraic:{operation}"]
            measurements = {}
            for root in roots[warmup:]:
                children = [event for event in events if root["span_id"] in event["ancestor_ids"] and root is not event]
                groups = {name: sum(event["elapsed_ms"] for event in children if event["name"] == name) for name in set(event["name"] for event in children)}
                groups["total_ms"] = root["elapsed_ms"]
                if operation == "prove":
                    groups["other_including_joint_ms"] = root["elapsed_ms"] - sum(groups.get(f"falcon_algebraic:{name}", 0) for name in ["ring", "arithmetic", "bitz", "pcs"])
                if operation == "commit":
                    groups["packing_and_commit_ms"] = root["elapsed_ms"] - groups.get("falcon_algebraic:witness", 0)
                for name, value in groups.items():
                    measurements.setdefault(name, []).append(value)
            results[operation] = {name: statistics.median(values) for name, values in measurements.items()}
        reports.append({"run": path.name.removesuffix(".stderr.jsonl"), "operations": results})
    (directory / "stage-summary.json").write_text(json.dumps(reports, indent=2, sort_keys=True) + "\n")
    return reports


if __name__ == "__main__":
    directories = [Path(value) for value in sys.argv[1:]] or [Path(__file__).parent / "profiles", Path(__file__).parent / "profiles100"]
    for directory in directories:
        if directory.exists():
            summarize(directory)

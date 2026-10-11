#!/usr/bin/env python3
"""Lightweight progress for a running matrix; invoke on a non-benchmark CPU."""
import itertools
import json
from pathlib import Path
import statistics
import sys

directory = Path(sys.argv[1])
arguments = json.loads((directory / "metadata.json").read_text())["arguments"]
record_path = directory / "reported-shapes.json"
reported = json.loads(record_path.read_text()) if record_path.exists() else []
new = []
for degree, security, batch, threads in itertools.product(*(arguments[key] for key in ("degrees", "securities", "batches", "threads"))):
    shape = [degree, security, batch, threads]
    if shape in reported:
        continue
    data = {variant: [] for variant in ("baseline", "candidate")}
    for variant in data:
        for seed in arguments["seeds"]:
            path = directory / f"{variant}-n{degree}-s{security}-b{batch}-t{threads}-seed{seed}.jsonl"
            try:
                rows = [json.loads(line) for line in path.read_text().splitlines()]
                data[variant].extend(row for row in rows if row.get("trial") == "sample")
            except (FileNotFoundError, json.JSONDecodeError):
                pass
    expected = len(arguments["seeds"]) * arguments["iterations"]
    if all(len(rows) == expected for rows in data.values()):
        deltas = {
            metric: 100 * (statistics.median(row[metric] for row in data["candidate"]) /
                           statistics.median(row[metric] for row in data["baseline"]) - 1)
            for metric in ("total_prover_ms", "prove_ms")
        }
        new.append(shape)
        print(json.dumps({"completed_shape": shape, "deltas_percent": deltas}))
    elif any(data.values()):
        print(json.dumps({"in_progress_shape": shape, "samples": {key: len(rows) for key, rows in data.items()}}))
reported.extend(new)
record_path.write_text(json.dumps(reported, indent=2) + "\n")
print(json.dumps({"completed_shapes": len(reported)}))

"""Medians of the full-Falcon (falcon_hybrid) bench runs in a campaign directory.

    python3 summarize.py <dir> [--breakdown]

Each *.jsonl holds one JSON line per trial (`trial` = warmup or sample); the
matching *.err holds `/usr/bin/time -l` (peak RSS). Only `sample` trials count.
"""
import json
import statistics
import sys
from pathlib import Path

FIELDS = [
    ("total_prover_ms", "prover"),
    ("witness_commit_ms", "commit"),
    ("proof_prove_ms", "prove"),
    ("proof_verify_ms", "verify"),
    ("native_verify_ms", "native"),
    ("proof_payload_bytes", "proof B"),
]


def records(path):
    for line in path.read_text().splitlines():
        if line.startswith("{"):
            record = json.loads(line)
            if record.get("trial") == "sample":
                yield record


def rss_mib(path):
    err = path.with_suffix(".err")
    if not err.exists():
        return float("nan")
    for line in err.read_text().splitlines():
        if "maximum resident set size" in line:
            return int(line.split()[0]) / 2**20
    return float("nan")


def main():
    root = Path(sys.argv[1])
    breakdown = "--breakdown" in sys.argv
    print(f"{'run':14} {'n':>2} " + " ".join(f"{label:>9}" for _, label in FIELDS) + f" {'RSS MiB':>8}")
    for path in sorted(root.glob("*.jsonl")):
        samples = list(records(path))
        if not samples:
            continue
        cells = []
        for field, _ in FIELDS:
            values = [s[field] for s in samples if isinstance(s.get(field), (int, float))]
            cells.append(f"{statistics.median(values):9.1f}" if values else f"{'-':>9}")
        print(f"{path.stem:14} {len(samples):2d} " + " ".join(cells) + f" {rss_mib(path):8.0f}")
        if breakdown:
            parts = samples[0].get("proof_payload_breakdown") or {}
            for name, size in sorted(parts.items(), key=lambda item: -item[1]):
                print(f"    {name:40} {size:9d}")


main()

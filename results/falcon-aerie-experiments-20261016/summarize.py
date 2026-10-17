"""Median prover/verify per run file of the grinding-model A/B (JSON lines)."""
import json
import statistics
import sys
from collections import defaultdict
from pathlib import Path

runs = Path(sys.argv[1])
rows = defaultdict(list)
for path in sorted(runs.glob("*.jsonl")):
    if path.name.startswith(("report-", "trace-")):
        continue
    samples = []
    for line in path.read_text().splitlines():
        if not line.startswith("{"):
            continue
        record = json.loads(line)
        if record.get("trial") != "sample":
            continue
        samples.append(record)
    if not samples:
        continue
    stem = path.stem
    case, _, rep = stem.rpartition("-r") if "-r" in stem.split("-")[-1] else (stem, "", "")
    model = "per-site" if "per-site" in case else "union"
    key = case.replace("-per-site", "").replace("-union", "")
    rows[(key, model)].extend(samples)

def med(samples, field):
    values = [s[field] for s in samples if s.get(field) is not None]
    return statistics.median(values) if values else float("nan")

keys = sorted({k for k, _ in rows})
print(f"{'case':24} {'model':9} {'n':>3} {'total':>8} {'prove':>8} {'commit':>7} {'verify':>7} {'proof B':>8} {'bits':>7}")
for key in keys:
    base = None
    for model in ("union", "per-site"):
        samples = rows.get((key, model))
        if not samples:
            continue
        total = med(samples, "total_prover_ms")
        line = (
            f"{key:24} {model:9} {len(samples):3d} {total:8.2f} {med(samples, 'prove_ms'):8.2f} "
            f"{med(samples, 'witness_commit_ms'):7.2f} {med(samples, 'verify_ms'):7.2f} "
            f"{int(med(samples, 'proof_payload_bytes')):8d} {samples[0].get('algebraic_security_bits', float('nan')):7.2f}"
        )
        if model == "union":
            base = total
        elif base:
            line += f"  ({total / base:.3f}x)"
        print(line)

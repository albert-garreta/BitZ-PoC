"""Medians per run of the algebraic Falcon bench (JSON lines, sample trials only).

    python3 summarize.py <dir> [--breakdown]
"""
import json
import statistics
import sys
from pathlib import Path


def rss_mib(path):
    err = path.with_suffix(".err")
    for line in err.read_text().splitlines() if err.exists() else []:
        if "maximum resident set size" in line:
            return int(line.split()[0]) / 2**20
    return float("nan")


root = Path(sys.argv[1])
print(f"{'run':16} {'n':>2} {'prove':>8} {'commit':>7} {'verify':>7} {'proof B':>9} {'pcs B':>8} {'bridge B':>9} {'RSS MiB':>8}")
for path in sorted(root.glob("*.jsonl")):
    rows = [json.loads(l) for l in path.read_text().splitlines() if l.startswith("{")]
    rows = [r for r in rows if r.get("trial") == "sample"]
    if not rows:
        continue
    med = lambda k: statistics.median(r[k] for r in rows)
    parts = dict(rows[0]["proof_payload_breakdown"])
    print(f"{path.stem:16} {len(rows):2d} {med('total_prover_ms'):8.1f} {med('witness_commit_ms'):7.1f} "
          f"{med('verify_ms'):7.2f} {int(med('proof_payload_bytes')):9d} {parts.get('pcs', 0):8d} "
          f"{parts.get('bitz_bridge', 0):9d} {rss_mib(path):8.0f}")
    if "--breakdown" in sys.argv:
        for name, size in rows[0]["proof_payload_breakdown"]:
            print(f"    {name:28} {size:9d}")
        for name, size in rows[0].get("pcs_breakdown", []):
            print(f"      pcs {name:24} {size:9d}")

"""Medians of the ring/bridge-rows campaign: one line per configuration."""
import collections
import json
import statistics
import sys
from pathlib import Path

runs = Path(sys.argv[1])
rss = {}
for line in (runs / "clock.log").read_text().splitlines():
    parts = line.split()
    if len(parts) > 1 and "rss=" in line:
        rss[parts[1]] = next(p for p in parts if p.startswith("rss="))[4:]

print(f"{'config':32s} {'n':>2s} {'prove':>8s} {'commit':>7s} {'verify':>7s} {'proof':>9s} "
      f"{'bridge':>8s} {'ring':>6s} {'pcs':>7s} {'rss':>9s} spread")
for path in sorted(runs.glob("*.jsonl")):
    name = path.stem
    if name.startswith(("gen-", "trace")):
        continue
    rows = [json.loads(l) for l in path.read_text().splitlines() if l.startswith("{")]
    samples = [r for r in rows if r.get("trial") == "sample"]
    if not samples:
        print(f"{name:32s} no samples")
        continue
    med = lambda key: statistics.median(r[key] for r in samples)
    prove = [r["total_prover_ms"] for r in samples]
    spread = (max(prove) - min(prove)) / statistics.median(prove) * 100
    parts = dict(samples[-1]["proof_payload_breakdown"])
    print(f"{name:32s} {len(samples):2d} {med('total_prover_ms'):8.1f} {med('witness_commit_ms'):7.1f} "
          f"{med('verify_ms'):7.1f} {samples[-1]['proof_payload_bytes']:9d} "
          f"{parts.get('bitz_bridge', 0):8d} {parts.get('ring', 0):6d} {parts.get('pcs', 0):7d} "
          f"{rss.get(name, '?'):>9s} {spread:5.1f}%")

trace = runs / "trace-D.stderr"
if trace.exists():
    events = [json.loads(l) for l in trace.read_text().splitlines()
              if l.startswith("{") and '"event":"stage"' in l]
    for root in ("falcon_algebraic:prove", "falcon_algebraic:verify"):
        tops = [e for e in events if e["name"] == root and len(e["path"]) == 1][1:]
        ids = {e["span_id"] for e in tops}
        sums = collections.defaultdict(float)
        for e in events:
            if e["path"][0] == root and e["ancestor_ids"][0] in ids and len(e["path"]) in (2, 3):
                sums[" > ".join(e["path"][1:])] += e["elapsed_ms"]
        print(f"\n{root} (traced, n={len(tops)}): median "
              f"{statistics.median(e['elapsed_ms'] for e in tops):.1f} ms")
        for key, value in sorted(sums.items(), key=lambda kv: kv[0]):
            if value / len(tops) >= 1.0:
                print(f"   {key:90s} {value / len(tops):8.1f} ms")

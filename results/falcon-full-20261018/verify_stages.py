"""Median per-trial totals of the spans under `falcon_bench:verify` in a stage trace."""
import json
import statistics
import sys
from collections import defaultdict
from pathlib import Path

trials = []
previous_root = None
for line in Path(sys.argv[1]).read_text().splitlines():
    if '"event":"stage"' not in line:
        continue
    event = json.loads(line)
    root = event["path"][0]
    if root == "falcon_bench:commit" and previous_root != "falcon_bench:commit":
        trials.append(defaultdict(float))
    previous_root = root
    if root != "falcon_bench:verify" or not trials:
        continue
    name = " > ".join(event["path"][1:]) or event["name"]
    trials[-1][name] += event["elapsed_ms"]
measured = trials[1:] if len(trials) > 1 else trials
names = sorted({n for t in measured for n in t}, key=lambda n: -statistics.median(t.get(n, 0.0) for t in measured))
for name in names:
    print(f"{statistics.median(t.get(name, 0.0) for t in measured):8.2f} ms  {name}")

"""Per-trial stage totals from a BITZ_FALCON_STAGE_TIMINGS run (stage JSON lines on stderr).

    python3 stages.py <run>.err [prefix ...]

A trial starts at the first span under `falcon_bench:commit` that
follows spans outside it (commit, then prove, then verify, per trial). Spans
are summed by name within a trial, skipping spans nested in a span of the same
name. Prints the median over the measured trials (all but the first, the
warm-up) of every span whose name starts with one of the prefixes.
"""
import json
import statistics
import sys
from collections import defaultdict
from pathlib import Path

DEFAULT = ("falcon_hybrid:", "falcon_shared_ring:", "falcon_arithmetic:", "falcon_bridge:",
           "falcon_integer:", "falcon_keyed_ring:", "op:", "lig:grind_pow", "spartan:grinding")
COMMIT = "falcon_bench:commit"


def events(path):
    for line in path.read_text().splitlines():
        if '"event":"stage"' not in line:
            continue
        try:
            yield json.loads(line)
        except json.JSONDecodeError:
            continue


def main():
    path = Path(sys.argv[1])
    prefixes = tuple(sys.argv[2:]) or DEFAULT
    trials = []
    inside_previous = False
    for event in events(path):
        inside = event["path"][0] == COMMIT
        if inside and not inside_previous:
            trials.append(defaultdict(float))
        inside_previous = inside
        if not trials:
            trials.append(defaultdict(float))
        name = event["name"]
        if name in event["path"][:-1]:
            continue
        trials[-1][name] += event["elapsed_ms"]
    measured = trials[1:] if len(trials) > 1 else trials
    names = sorted({n for t in measured for n in t if n.startswith(prefixes)},
                   key=lambda n: -statistics.median(t.get(n, 0.0) for t in measured))
    print(f"{len(trials)} trials ({len(measured)} measured)")
    for name in names:
        values = [t.get(name, 0.0) for t in measured]
        print(f"{statistics.median(values):9.2f} ms  {name}")


main()

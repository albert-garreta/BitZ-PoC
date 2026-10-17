"""List the frames that build_page.py files under Other for one profile.

usage: python3 -I debug_other.py <flame out dir> <profile name>
"""

import collections
import importlib.util
import json
import sys
from pathlib import Path

here = Path(__file__).parent
spec = importlib.util.spec_from_file_location("build_rules", here / "build_page.py")
source = (here / "build_page.py").read_text()
rules_src = source[: source.index("def build(")]
ns = {"__name__": "rules"}
exec(compile(rules_src.replace("OUT = Path(sys.argv[1])", "OUT = None")
             .replace("TEMPLATE = Path(sys.argv[2])", "TEMPLATE = None")
             .replace("INDEX = Path(sys.argv[3])", "INDEX = None"), "build_page.py", "exec"), ns)
rules = ns["BITZ_RULES"] if sys.argv[2].startswith("bitz") else ns["AERIE_RULES"]
import re
compiled = [(c, re.compile(rx)) for c, rx in rules]
overrides = ns["OVERRIDES"]
OTHER = ns["OTHER"]
summary = json.loads((Path(sys.argv[1]) / "summary.json").read_text())[sys.argv[2]]
scale = summary["ms_per_prove"] / summary["samples"]
counts = collections.Counter()
for line in (Path(sys.argv[1]) / f"{sys.argv[2]}.folded").read_text().splitlines():
    stack, _, count = line.rpartition(" ")
    frames = stack.split(";")
    phase = override = None
    for frame in frames[1:]:
        override = next((c for c, rx in overrides if rx.search(frame)), override)
        if phase is None:
            phase = next((c for c, rx in compiled if rx.search(frame)), None)
    cat = override if override is not None else phase
    if cat is None:
        key = " > ".join(frames[1:5])
        counts[key] += int(count)
for key, count in counts.most_common(15):
    print(f"{count * scale:8.1f} ms  {key[:200]}")

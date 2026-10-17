"""Compare BitZ phase wall times from span traces with the samply CPU attribution.

usage: python3 -I compare_trace.py <trace dir> <samply dir>
"""

import collections
import json
import sys
from pathlib import Path

TRACE = Path(sys.argv[1])
SAMPLY = Path(sys.argv[2])

# trace span (child of falcon_algebraic:prove or :commit) -> frame substring in the folded stacks
PHASES = [
    ("falcon_algebraic:ring", ["ring::coordinates", "ring::collapse", "ring::calculate_carries"]),
    ("falcon_algebraic:arithmetic", ["arithmetic::prove_owned_with_scratch"]),
    ("falcon_algebraic:bitz", ["integer_bridge::prove_weighted"]),
    ("falcon_algebraic:pcs", ["opening::prove_joint_sources_with_security"]),
    ("falcon_algebraic:witness", ["WitnessData::from_s2"]),
]

for name in ["bitz-shipped", "bitz-onelimb"]:
    events = [json.loads(line) for line in (TRACE / f"{name}.stderr").read_text().splitlines()
              if line.startswith("{") and '"event":"stage"' in line]
    roots = [e for e in events if len(e["path"]) == 1 and e["name"] in ("falcon_algebraic:commit", "falcon_algebraic:prove")]
    prove_ids = {e["span_id"] for e in roots}
    proves = sum(1 for e in roots if e["name"] == "falcon_algebraic:prove")
    traced = collections.Counter()
    total = sum(e["elapsed_ms"] for e in roots) / proves
    for e in events:
        if e["ancestor_ids"] and e["ancestor_ids"][0] in prove_ids and len(e["path"]) == 2:
            traced[e["name"]] += e["elapsed_ms"] / proves
    sampled = collections.Counter()
    sampled_total = 0.0
    for line in (SAMPLY / f"{name}.samply.folded").read_text().splitlines():
        stack, _, count = line.rpartition(" ")
        if "profile_commit_region" not in stack and "profile_prove_region" not in stack:
            continue
        sampled_total += int(count)
        for span, frames in PHASES:
            if any(f in stack for f in frames):
                sampled[span] += int(count)
    print(f"{name}: traced prover {total:.0f} ms/prove")
    for span, _ in PHASES:
        t_share = traced[span] / total
        s_share = sampled[span] / sampled_total
        print(f"   {span:30s} traced {100 * t_share:5.1f}%   samply {100 * s_share:5.1f}%")

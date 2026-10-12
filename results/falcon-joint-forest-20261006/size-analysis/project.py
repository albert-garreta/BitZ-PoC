"""Prospective size accounting, not measurements of implemented protocols.

Use current resolver parameters and expected uniform distinct-query Merkle
multiproof sizes. Keep the measured proof's fixed messages, replace its opened
rows, authentication hashes and per-row framing, then model a joint source
root, implicit zero lanes and a 65,536-row bridge with narrow upper sums.
"""
import json
import math
from pathlib import Path

HERE = Path(__file__).resolve().parent
measured = {r["security_target"]: r for r in map(json.loads, (HERE / "seed42-b1024.jsonl").read_text().splitlines())}
profiles = []
for line in (HERE / "falcon_pcs_parameters.txt").read_text().splitlines():
    if line.startswith("target="):
        profile = dict(part.split("=") for part in line.split())
        profile["levels"] = []
        profile["positive_nonquery_boundaries"] = 0
        profiles.append(profile)
    elif line.startswith("level="):
        profile["levels"].append(dict(part.split("=") for part in line.split()))
    elif line.startswith("block="):
        block = dict(part.split("=") for part in line.split())
        if not block["block"].startswith("queries/") and int(block["bits"]) > 0:
            profile["positive_nonquery_boundaries"] += 1


def expected_authentication_bytes(depth, queries):
    size = 1 << depth
    occupied_internal = 0.0
    for height in range(1, depth):
        subtree = 1 << height
        log_empty = sum(math.log1p(-subtree / (size - j)) for j in range(queries))
        occupied_internal += (size // subtree) * -math.expm1(log_empty)
    return 32 * (2 + occupied_internal - queries)


projections = []
baseline_boundaries = {int(p["target"]): p["positive_nonquery_boundaries"]
                       for p in profiles if p["rate"] == "1/2"}
for profile in profiles:
    target = int(profile["target"])
    row = measured[target]
    parts = {c["name"]: c["bytes"] for c in row["components"]}
    levels = profile["levels"]
    assert len(levels) == 6  # Fixed initial arity four for this comparison.
    old_rows = sum(v for k, v in parts.items() if k.endswith("opened_rows"))
    old_auth = sum(v for k, v in parts.items() if "authentication" in k)
    # Current bincode framing: 152 fixed bytes plus eight bytes per opened row.
    old_row_framing = parts["pcs_bincode_framing"] - 152
    new_rows = sum(int(p["queries"]) * (1 << int(p["lane_log"])) * 16 for p in levels)
    authentication = [expected_authentication_bytes(int(p["depth"]), int(p["queries"])) for p in levels]
    new_row_framing = sum(8 * int(p["queries"]) for p in levels)
    initial_queries = int(levels[0]["queries"])
    # Single initial tree; subsequent recursion already uses one tree per level.
    expected_total = (row["payload_bytes"] - old_rows - old_auth - old_row_framing
                      + new_rows + sum(authentication) + new_row_framing
                      - initial_queries * 5 * 16
                      + 8 * (profile["positive_nonquery_boundaries"] - baseline_boundaries[target]))
    old_columns = parts["integer_column_sums_first_limb"] + parts["integer_column_sums_remaining_limbs"]
    new_columns = 38_912 if target == 100 else 40_960
    additional_bridge_messages = 1_760 if target == 100 else 1_680
    total_with_bridge = expected_total - old_columns + new_columns + additional_bridge_messages
    projections.append(dict(security_target=target, rate=profile["rate"], initial_queries=initial_queries,
                            pcs_bound_bits=float(profile["pcs_error_bits"]),
                            joint_source_and_zero_codec_expected_bytes=round(expected_total),
                            including_larger_bridge_rows_expected_bytes=round(total_with_bridge)))

output = dict(
    status="Analytic projections only; not generated proofs or a recertified full changed-geometry security ledger",
    assumptions=["same six-level initial-arity-four PCS geometry", "uniform distinct query positions",
                 "one source commitment before Fiat-Shamir challenges", "five implicit initial zero lanes",
                 "65536 bridge rows, two bounded limbs, 16+3 byte sums at 100 bits and 16+4 at 128",
                 "current fixed message sizes; transcript changes may change exact authentication lengths"],
    profiles=projections)
(HERE / "projections.json").write_text(json.dumps(output, indent=2) + "\n")
print(json.dumps(output, indent=2))

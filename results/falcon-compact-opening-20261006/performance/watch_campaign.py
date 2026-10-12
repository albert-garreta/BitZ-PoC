#!/usr/bin/env python3
"""Report progress and stop at a pair boundary if a completed cell fails."""
import argparse
import json
from pathlib import Path
import time

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("directory", type=Path)
args = parser.parse_args()
seen = set()
last_progress = 0
while True:
    try:
        manifest = json.loads((args.directory / "manifest.json").read_text())
    except (OSError, json.JSONDecodeError):
        time.sleep(1)
        continue
    if time.monotonic() - last_progress >= 45 or manifest["status"] not in ("running", "waiting"):
        print(json.dumps({key: manifest.get(key) for key in
                          ("status", "completed_pairs", "active_case", "active_seed", "qualification_status")}), flush=True)
        last_progress = time.monotonic()
    summary_path = args.directory / "summary.json"
    if summary_path.exists():
        try:
            summary = json.loads(summary_path.read_text())
        except (OSError, json.JSONDecodeError):
            time.sleep(1)
            continue
        for cell in summary["cells"]:
            key = cell["protocol"], cell["security"], cell["batch"]
            if len(cell["seeds"]) != 60 or key in seen:
                continue
            seen.add(key)
            result = dict(case=key, payload_ratio=cell["payload"]["ratio_of_mean_bytes"],
                          payload_pass=cell["payload"]["pass_gate"],
                          timings={metric: {name: cell["timings"][metric][name] for name in
                                           ("ratio", "upper_one_sided_95", "status")}
                                   for metric in ("total_prover_ms", "proof_verify_ms")})
            print("CELL " + json.dumps(result), flush=True)
            if not result["payload_pass"] or any(t["status"] != "pass" for t in result["timings"].values()):
                stop = args.directory / "STOP"
                if not stop.exists() and manifest["status"] == "running":
                    stop.write_text("Stopped after a complete 60-seed cell did not pass the unchanged compact-opening gate: "
                                    + json.dumps(result) + "\n")
                    print("STOP_REQUESTED", flush=True)
    if manifest["status"] not in ("running", "waiting"):
        break
    time.sleep(5)

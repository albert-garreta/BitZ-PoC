#!/usr/bin/env python3
"""Qualify unchanged-code controls before any candidate comparison.

Run under scripts/bench_gate.py after building the binary. The complete finite
ladder and independent confirmation seeds are recorded before the first run.
No missing or noisy case is dropped; an unsuccessful ladder is unqualified.
"""
import argparse
import hashlib
import json
from pathlib import Path
import platform
import subprocess
import sys

from unified_api import FAMILIES, SIZES

LADDER = [(5, 32), (5, 64), (10, 128)]
KEYS = [f"{family}/{size}" for family in FAMILIES for size in SIZES]


def passed(path):
    rows = json.loads(path.read_text())
    keys = [f"{r['family']}/{r['size']}" for r in rows]
    return (len(keys) == len(KEYS) and set(keys) == set(KEYS)
            and all(r.get("status") == "pass" and r.get("complete")
                    and r.get("correctness") and r.get("allocations_ok")
                    and r.get("variant") == "production_repeat"
                    and r.get("baseline") == "production" for r in rows))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("binary", type=Path)
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    binary = args.binary.resolve()
    record = binary.with_name("build.json")
    provenance = json.loads(record.read_text())
    if provenance["binary_sha256"] != hashlib.sha256(binary.read_bytes()).hexdigest():
        parser.error("baseline executable does not match its frozen build.json")
    out = args.output.resolve()
    out.mkdir(parents=True, exist_ok=False)
    policy = dict(status="unqualified", ladder=LADDER, calibration_ms=10.0,
                  keys=KEYS, host=platform.platform(), margin=0.01,
                  median_and_p95_upper95_ci=1.01, each_process_median_limit=1.01,
                  binary_sha256=hashlib.sha256(binary.read_bytes()).hexdigest(),
                  baseline_provenance=str(record),
                  control_seeds=[1501, 106188, 901733],
                  confirmation_seeds=[3001001, 4001001, 5001001],
                  scope="18 prime API microbenchmarks; not a consumer or x86 gate")
    (out / "predeclared.json").write_text(json.dumps(policy, indent=2) + "\n")
    script = Path(__file__).with_name("unified_api.py")
    for stage, (runs, samples) in enumerate(LADDER):
        for phase in ["control", "confirmation"]:
            destination = out / f"stage-{stage + 1}-{phase}"
            seed = policy[f"{phase}_seeds"][stage]
            subprocess.run([sys.executable, str(script), str(binary), str(destination),
                            "--aa", "--runs", str(runs), "--samples", str(samples),
                            "--seed-offset", str(seed), "--calibration-ms", "10"], check=True)
            if not passed(destination / "results.json"):
                print(f"stage {stage + 1} {phase}: not qualified", flush=True)
                break
        else:
            policy.update(status="qualified", runs=runs, samples=samples,
                          confirmation=str(destination),
                          confirmation_sha256=hashlib.sha256(
                              (destination / "results.json").read_bytes()).hexdigest())
            break
    (out / "qualification.json").write_text(json.dumps(policy, indent=2) + "\n")
    print(policy["status"], flush=True)
    return 0 if policy["status"] == "qualified" else 2


if __name__ == "__main__":
    sys.exit(main())

#!/usr/bin/env python3
"""Measure additional thread counts using an already confirmed, identical executable."""
import argparse
import json
import platform
import shutil
import time
from pathlib import Path

from analysis import expanded, write_report
from run import clean_environment, run_round, save, sha


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--from-run", required=True, type=Path)
    parser.add_argument("--out", required=True, type=Path)
    parser.add_argument("--threads", required=True, type=int)
    parser.add_argument("--families", required=True)
    parser.add_argument("--manifest", type=Path, help="Freeze separate preselected cases for this run, keeping the executable unchanged")
    parser.add_argument("--binary", type=Path, help="Extracted archive executable; its hash must match")
    parser.add_argument("--seed-offset", type=int, default=2718281)
    args = parser.parse_args()
    source = args.from_run.resolve()
    original = json.loads((source / "metadata.json").read_text())
    if original["status"] != "complete" or original["mode"] != "optimization":
        parser.error("The source optimization run must be complete")
    if args.threads < 1 or not 0 <= args.seed_offset < 2**63:
        parser.error("Invalid thread count or seed")
    if platform.machine() not in ("aarch64", "arm64"):
        parser.error("This campaign is ARM only")
    spec_path = source / "required_cases.json"
    if sha(spec_path) != original["manifest_sha256"]:
        parser.error("The frozen manifest changed")
    if args.manifest:
        spec_path = args.manifest.resolve()
    spec = json.loads(spec_path.read_text())
    families = set(args.families.split(","))
    if not families <= {g["name"] for g in spec["families"]}:
        parser.error("Unknown benchmark family")
    cases = {r["family"] + "/" + r["size"] for r in expanded(spec) if r["family"] in families}
    binary = args.binary.resolve() if args.binary else Path(original["binary"])
    if sha(binary) != original["binary_sha256"]:
        parser.error("The frozen executable changed")
    out = args.out.resolve()
    out.mkdir(parents=True, exist_ok=False)
    shutil.copy2(spec_path, out / "required_cases.json")
    shutil.copy2(source / "Cargo.lock", out / "Cargo.lock")
    metadata = dict(original, binary=str(binary), threads=args.threads, cases=sorted(cases),
                    runs=spec["confirmation_runs"], samples=spec["confirmation_samples"],
                    seed_offset=args.seed_offset, status="confirming", reused_from=str(source),
                    timestamp=time.strftime("%Y-%m-%dT%H:%M:%S%z"), build_seconds=0)
    metadata["manifest_sha256"] = sha(out / "required_cases.json")
    for key in ("retried_cases", "round_metadata_sha256", "summary_sha256"):
        metadata.pop(key, None)
    save(out / "metadata.json", metadata)
    env = clean_environment(metadata["flags"], source / "unused-target", args.threads)
    rows = run_round(out / "initial", metadata, spec, binary, env, cases)
    retry = {r["family"] + "/" + r["size"] for r in rows if r["selected"] and r["status"] == "inconclusive"}
    if retry:
        retry_meta = dict(metadata, samples=metadata["samples"] * spec["retry_multiplier"],
                          seed_offset=args.seed_offset + 104687, cases=sorted(retry))
        retried = run_round(out / "retry", retry_meta, spec, binary, env, retry)
        replacements = {(r["arch"], r["family"], r["size"], r["variant"]): r for r in retried
                        if r["family"] + "/" + r["size"] in retry}
        rows = [replacements.get((r["arch"], r["family"], r["size"], r["variant"]), r) for r in rows]
    write_report(out, rows, metadata["margin"], metadata["report_title"])
    metadata.update(status="complete", retried_cases=sorted(retry),
                    round_metadata_sha256={name: sha(out / name / "metadata.json")
                                           for name in ("initial", "retry") if (out / name).exists()},
                    summary_sha256=sha(out / "summary.json"))
    save(out / "metadata.json", metadata)
    print(f"Report: {out / 'measurements.md'}", flush=True)


if __name__ == "__main__":
    main()

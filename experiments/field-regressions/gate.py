#!/usr/bin/env python3
"""Require complete manifest coverage; never average away a regressing case."""
import argparse
import hashlib
import json
from pathlib import Path

from analysis import DEFAULT_MARGIN, classify, expanded


def evaluate(report, margin=DEFAULT_MARGIN):
    report = Path(report)
    metadata = json.loads((report / "metadata.json").read_text())
    if metadata.get("schema") != 2 or metadata.get("status") != "complete":
        return ["unmeasured: report is incomplete or uses the legacy measurement policy"]
    manifest = report / "required_cases.json"
    if not manifest.exists() or hashlib.sha256(manifest.read_bytes()).hexdigest() != metadata.get("manifest_sha256"):
        return ["unmeasured: frozen required-case manifest is missing or changed"]
    spec = json.loads(manifest.read_text())
    if spec.get("campaign") == "x86" and (metadata.get("phase") != "confirm" or spec.get("phase") != "confirm"):
        return ["unmeasured: exploration is not independent confirmation"]
    if spec.get("campaign") == "x86":
        for filename,key in [("benchmark.bin","archived_binary_sha256"),("sources.tar.gz","sources_archive_sha256"),
                             ("Cargo.lock","lockfile_sha256"),("rankings.json","rankings_sha256"),("selection.json","selection_sha256")]:
            path=report/filename
            if not path.exists() or hashlib.sha256(path.read_bytes()).hexdigest()!=metadata.get(key):
                return [f"unmeasured: missing or changed {filename}"]
    # Hashes bind the displayed statistics to complete, frozen raw runs. A
    # report copied without its measurements is not usable for promotion.
    rounds = metadata.get("round_metadata_sha256", {})
    if not rounds or "initial" not in rounds:
        return ["unmeasured: missing confirmation-round metadata"]
    for name, expected_hash in rounds.items():
        path = report/name/"metadata.json"
        if not path.exists() or hashlib.sha256(path.read_bytes()).hexdigest()!=expected_hash:
            return [f"unmeasured: missing or changed {name} metadata"]
        details=json.loads(path.read_text())
        if (details.get("runs",0)<spec["confirmation_runs"] or details.get("samples",0)<spec["confirmation_samples"]
                or details.get("binary_sha256")!=metadata.get("binary_sha256")
                or details.get("manifest_sha256")!=metadata["manifest_sha256"]):
            return [f"unmeasured: {name} confirmation configuration does not match"]
        artifacts=details.get("artifacts_sha256",{})
        required={f"run-{i}.{ext}" for i in range(details["runs"]) for ext in ["csv","log"]}
        if set(artifacts)!=required:
            return [f"unmeasured: missing {name} raw measurements"]
        for filename,expected in artifacts.items():
            path=report/name/filename
            if not path.exists() or hashlib.sha256(path.read_bytes()).hexdigest()!=expected:
                return [f"unmeasured: missing or changed {name}/{filename}"]
    if hashlib.sha256((report/"summary.json").read_bytes()).hexdigest()!=metadata.get("summary_sha256"):
        return ["unmeasured: summary changed since analysis"]
    rows = json.loads((report / "summary.json").read_text())
    lookup = {}
    for row in rows:
        key = (row["arch"], row["family"], row["size"], row["variant"])
        if key in lookup:
            return [f"unmeasured: duplicate summary case {key}"]
        lookup[key] = row
    issues = []
    for required in expanded(spec):
        key = tuple(required[k] for k in ("arch", "family", "size", "variant"))
        label = "/".join(key)
        row = lookup.get(key)
        if row is None or not row.get("complete"):
            issues.append(f"unmeasured: {label}")
            continue
        if not required["diagnostic"] and not row.get("correctness"):
            issues.append(f"regression: {label}: correctness failed")
        if required["selected"]:
            # Use frozen policy labels, never editable summary selection flags.
            status = classify(dict(row, **required), margin)
            if status != "pass":
                issues.append(f'{status}: {label}: median upper={row["median_ci_high"]:.4f}, '
                              f'P95 upper={row["p95_ci_high"]:.4f}, worst process={max(row["per_run_medians"]):.4f}, '
                              f'allocations={"OK" if row["allocations_ok"] else "increased"}')
    if spec.get("campaign") == "x86":
        rankings={(r["family"],r["size"]):r for r in json.loads((report/"rankings.json").read_text())}
        for group in spec["families"]:
            if group.get("diagnostic") or not group.get("challenger"):continue
            for size in group["sizes"]:
                row=rankings.get((group["name"],size),{})
                if (row.get("selected")!=group["selected"]["x86_64"] or row.get("challenger")!=group["challenger"]
                        or row.get("head_to_head",{}).get("decision") not in ("faster","within_1_percent")):
                    issues.append(f"inconclusive: {group['name']}/{size}: challenger not cleared")
    return issues


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("report", type=Path)
    parser.add_argument("--max-slowdown", type=float, default=DEFAULT_MARGIN)
    args = parser.parse_args()
    if not 0 <= args.max_slowdown < 1:
        parser.error("max slowdown must be in [0, 1)")
    issues = evaluate(args.report, args.max_slowdown)
    for issue in issues:
        print("BLOCKED " + issue)
    print(f'{"BLOCKED" if issues else "PASS"}: frozen selections, {args.max_slowdown:.1%} margin, all required cases.')
    print("This applies only to the frozen manifest, measured host workloads and build; production behavior is unchanged.")
    raise SystemExit(bool(issues))


if __name__ == "__main__":
    main()

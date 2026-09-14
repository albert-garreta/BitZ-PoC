#!/usr/bin/env python3
"""Refresh line links/hashes from the baseline catalogue's unique source anchors."""
import hashlib
import json
import subprocess
from pathlib import Path


def main():
    here = Path(__file__).resolve().parent
    root = here.parents[1]
    path = here / "baseline_catalog.json"
    catalog = json.loads(path.read_text())
    for entry in catalog["entries"]:
        for location in entry["baseline_sources"] + entry["benchmark_sources"]:
            source = Path(location["path"])
            source = source if source.is_absolute() else root / source
            content = source.read_text()
            matches = [i + 1 for i, line in enumerate(content.splitlines()) if location["anchor"] in line]
            if len(matches) != 1:
                raise ValueError(f"Anchor moved or became ambiguous: {source}: {location['anchor']}")
            location.update(line=matches[0], sha256=hashlib.sha256(source.read_bytes()).hexdigest())
        for evidence in entry["evidence"]:
            summary = json.loads((here / "results" / evidence["suite"] / "summary.json").read_text())
            complete = {r["family"] for r in summary if r["arch"] == "aarch64" and r["complete"]}
            if not set(evidence["families"]) <= complete:
                raise ValueError(f"Missing completed evidence: {entry['id']}: {evidence}")
    catalog["source_head"] = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=root, text=True).strip()
    path.write_text(json.dumps(catalog, indent=2) + "\n")

    def link(location):
        source = Path(location["path"])
        source = source if source.is_absolute() else root / source
        return f"[{location['label']}:{location['line']}]({source}:{location['line']})"

    lines = ["# Baseline comparisons and source locations", "",
             "Scope: isolated ARM and portable experiments. Production integration and dependency removal remain deferred.", "",
             "Line numbers locate the implementation; performance comparisons require matching arithmetic, outputs, preparation and timing contracts. The catalogue records a source anchor and hash for every link. Refresh links with `python3 experiments/field-regressions/refresh_baselines.py`.", "",
             "See [optimization candidates](OPTIMIZATION.md) and the frozen result directories for current measurements. `baseline_catalog.json` is documentation, not an executable benchmark manifest.", "",
             "| Operation | Production baseline / control | Benchmark source | Coverage |",
             "|---|---|---|---|"]
    for entry in catalog["entries"]:
        source = "; ".join(map(link, entry["baseline_sources"]))
        benchmark = "; ".join(map(link, entry["benchmark_sources"])) or "Pending"
        lines.append(f"| {entry['operation']} | {source} | {benchmark} | {entry['status']} — {entry['coverage_note']} |")
    lines += ["", "## Comparison contracts", "",
              "| Operation | Matching contract | Stronger control / limitation |", "|---|---|---|"]
    for entry in catalog["entries"]:
        lines.append(f"| {entry['operation']} | {entry['contract']} | {entry['secondary_comparison']} |")
    lines += ["", "## Evidence", "",
              "| Operation | Completed measurement families |", "|---|---|"]
    for entry in catalog["entries"]:
        if entry["evidence"]:
            evidence = "; ".join(f"[{e['suite']}](results/{e['suite']}/measurements.md): " + ", ".join(e["families"])
                                 for e in entry["evidence"])
            lines.append(f"| {entry['operation']} | {evidence} |")
    lines += ["", "Preparation, reuse and one-shot execution remain separate comparisons. Preserve all output writes and conversion costs. Fold and transform resets are matched. A baseline self-comparison is not a new speedup, and a faster scalar kernel does not establish a faster complete prover. Pointwise confidence intervals and allocation gates apply per workload; inconclusive results stay inconclusive.", ""]
    (here / "BASELINES.md").write_text("\n".join(lines))
    print(f"Refreshed {len(catalog['entries'])} groups and verified their completed evidence")


if __name__ == "__main__":
    main()

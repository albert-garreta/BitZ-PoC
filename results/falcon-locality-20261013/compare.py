#!/usr/bin/env python3
"""Compare frozen locality-runner summaries without pooling phases or traces.

Usage: python3 results/falcon-locality-20261013/compare.py [--phase screen]
Writes comparison.json and comparison.txt next to this file by default. This
reads existing summaries only; it does not launch or validate benchmark runs.
"""

import argparse
import hashlib
import json
import math
from pathlib import Path


DIMENSIONS = ("phase", "degree", "batch", "security", "threads", "mode", "trace")
DISPLAY_METRICS = (
    "total_prover_ms", "prove_ms", "witness_commit_ms", "verify_ms",
    "process_peak_rss_kib", "process_retained_rss_kib", "proof_payload_bytes",
)
NOTES = [
    "All ratios are candidate/reference; a negative percentage means a lower measurement.",
    "Latency comparisons contain trace=false rows only. Traced stage timings are separate diagnostics.",
    "References must match phase, degree, batch, security, workers, mode, and ordered worker/main CPU placement.",
    "Each available baseline and control is compared separately; no reference is borrowed from another phase.",
    "Medians are the runner's pooled post-warmup sample medians; seed ratios use each seed's own medians.",
    "Repeated samples within one process are correlated; ratios do not establish statistical significance.",
    "process_peak_rss_kib in metric tables is the sampled trial median. The separate process_peak_rss block uses each process's maximum, including warmups.",
    "Retained RSS includes inputs, allocator and Prepared; missing metrics are null, never zero.",
    "Profile spans can overlap, and summed worker-time fields are not wall time. Trace overhead prevents treating these as untraced latency.",
    "Proof byte medians alone do not establish proof equality. Consult the runner's validation.json and raw provenance.",
    "This helper trusts the runner summaries; it does not revalidate proofs, binary hashes or raw sample files.",
]


def read_json(path):
    return json.loads(path.read_text())


def fingerprint(path):
    return {"path": str(path.resolve()), "sha256": hashlib.sha256(path.read_bytes()).hexdigest()}


def row_key(row):
    return tuple(row[name] for name in DIMENSIONS)


def identity(row):
    return (*row_key(row), row["variant"])


def topology(row):
    cpus, main = row.get("worker_cpus"), row.get("main_cpu")
    if not isinstance(cpus, list) or not cpus or type(main) is not int:
        return None
    return tuple(cpus), main


def numeric(value):
    if value is None:
        return None
    if type(value) not in (int, float) or not math.isfinite(value) or value < 0:
        raise ValueError(f"invalid metric value: {value!r}")
    return value


def measurement(candidate, reference):
    candidate, reference = numeric(candidate), numeric(reference)
    ratio = candidate / reference if candidate is not None and reference not in (None, 0) else None
    return {"candidate": candidate, "reference": reference, "ratio": ratio,
            "change_percent": (ratio - 1) * 100 if ratio is not None else None,
            "difference": candidate - reference if candidate is not None and reference is not None else None}


def sorted_seeds(seeds):
    return sorted(seeds, key=lambda seed: (not str(seed).isdigit(), int(seed) if str(seed).isdigit() else str(seed)))


def metric_comparisons(candidate, reference, median_key, seed_key):
    left, right = candidate[median_key], reference[median_key]
    left_seeds, right_seeds = candidate[seed_key], reference[seed_key]
    common = sorted_seeds(left_seeds.keys() & right_seeds.keys())
    metrics = {}
    for name in sorted(left.keys() | right.keys()):
        metrics[name] = measurement(left.get(name), right.get(name))
        metrics[name]["seeds"] = {
            seed: measurement(left_seeds[seed].get(name), right_seeds[seed].get(name))
            for seed in common
        }
    return metrics, {
        "common": common,
        "candidate_only": sorted_seeds(left_seeds.keys() - right_seeds.keys()),
        "reference_only": sorted_seeds(right_seeds.keys() - left_seeds.keys()),
    }


def comparisons(rows, summary_index, references, *, profiles=False):
    result, unmatched = [], []
    for candidate in rows:
        # Control-versus-baseline is useful; baseline is always a reference.
        if candidate["variant"] == "baseline":
            continue
        metadata = summary_index.get(identity(candidate), {})
        possible = [row for row in rows if row_key(row) == row_key(candidate)
                    and row["variant"] in references and row["variant"] != candidate["variant"]]
        matches = []
        for reference in possible:
            reference_metadata = summary_index.get(identity(reference), {})
            if topology(metadata) is not None and topology(metadata) == topology(reference_metadata):
                matches.append((reference, reference_metadata))
        if not matches:
            unmatched.append({**{name: candidate[name] for name in DIMENSIONS},
                              "variant": candidate["variant"],
                              "reason": "no same-phase/cell reference with identical known CPU placement",
                              "available_reference_variants": [row["variant"] for row in possible]})
        for reference, reference_metadata in matches:
            median_key, seed_key = ("medians_ms", "seed_medians_ms") if profiles else ("medians", "seed_medians")
            metrics, seeds = metric_comparisons(candidate, reference, median_key, seed_key)
            entry = {**{name: candidate[name] for name in DIMENSIONS},
                     "candidate": candidate["variant"], "reference": reference["variant"],
                     "candidate_binary": metadata.get("binary"), "reference_binary": reference_metadata.get("binary"),
                     "worker_cpus": metadata["worker_cpus"], "main_cpu": metadata["main_cpu"],
                     "samples": {"candidate": candidate["samples"], "reference": reference["samples"]},
                     "seeds": seeds, "metrics": metrics}
            if not profiles:
                entry["processes"] = {"candidate": candidate.get("processes"), "reference": reference.get("processes")}
                entry["metric_samples"] = {"candidate": candidate.get("metric_samples"), "reference": reference.get("metric_samples")}
                entry["process_peak_rss"] = {
                    name: measurement(candidate.get("process_peak_rss_kib", {}).get(name),
                                      reference.get("process_peak_rss_kib", {}).get(name))
                    for name in ("median", "maximum")
                }
            result.append(entry)
    return result, unmatched


def number(value, places=3):
    if value is None:
        return "n/a"
    return f"{value:.{places}f}" if places else f"{value:.0f}"


def format_metric(name, metric):
    integer = name.endswith(("bytes", "kib"))
    precision = 0 if integer else 3
    seeds = " ".join(f"{seed}:{number(values['ratio'], 4)}" for seed, values in metric.get("seeds", {}).items())
    return (f"  {name:56} {number(metric['candidate'], precision):>12} / "
            f"{number(metric['reference'], precision):>12}  ratio {number(metric['ratio'], 4):>7} "
            f"change {number(metric['change_percent'], 2):>7}%  seeds [{seeds}]")


def render(report):
    lines = ["Falcon locality comparison", "", *[f"- {note}" for note in report["notes"]]]
    for section, title in (("latency", "UNTRACED LATENCY AND RESOURCES"), ("profiles", "TRACED STAGE DIAGNOSTICS")):
        lines.extend(["", title])
        if not report[section]:
            lines.append("  No matched comparisons available.")
        for row in report[section]:
            lines.extend(["", f"[{row['phase']}] {row['candidate']} / {row['reference']} "
                          f"N={row['degree']} batch={row['batch']} security={row['security']} "
                          f"workers={row['threads']} mode={row['mode']} samples={row['samples']} "
                          f"CPUs={row['worker_cpus']} main={row['main_cpu']}"])
            names = DISPLAY_METRICS if section == "latency" else sorted(row["metrics"])
            lines.extend(format_metric(name, row["metrics"][name]) for name in names if name in row["metrics"])
            if section == "latency":
                lines.extend(format_metric(f"process_peak_maxima_{name}_kib", value)
                             for name, value in row["process_peak_rss"].items())
            if row["seeds"]["candidate_only"] or row["seeds"]["reference_only"]:
                lines.append(f"  Unpaired seeds: {row['seeds']}")
    if report["unmatched"]:
        lines.extend(["", "UNMATCHED ROWS (no cross-phase or different-placement fallback)"])
        lines.extend("  " + json.dumps(row, sort_keys=True) for row in report["unmatched"])
    return "\n".join(lines) + "\n"


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--directory", type=Path, default=Path(__file__).resolve().parent)
    parser.add_argument("--phase", action="append", help="Only this phase; repeat to select several.")
    parser.add_argument("--references", nargs="+", default=["baseline", "control"])
    parser.add_argument("--output-prefix", type=Path, help="Output path without suffix; default DIRECTORY/comparison.")
    args = parser.parse_args()
    summary_path = args.directory / "summary.json"
    if not summary_path.is_file():
        parser.error(f"summary is not available yet: {summary_path}")
    summary = read_json(summary_path)
    rows = [row for row in summary["rows"] if not args.phase or row["phase"] in args.phase]
    index = {}
    for row in rows:
        if type(row["trace"]) is not bool:
            raise ValueError("trace must be an explicit boolean")
        key = identity(row)
        if key in index:
            raise ValueError(f"duplicate summary row: {key}")
        index[key] = row
    sources = {"summary": fingerprint(summary_path)}
    latency, unmatched = comparisons([row for row in rows if row["trace"] is False], index, args.references)
    profiles = []
    profile_path = args.directory / "profile-summary.json"
    if profile_path.is_file():
        sources["profiles"] = fingerprint(profile_path)
        profile_rows = [row for row in read_json(profile_path)["rows"] if not args.phase or row["phase"] in args.phase]
        if any(row["trace"] is not True for row in profile_rows):
            raise ValueError("profile summary contains an untraced row")
        profiles, missing = comparisons(profile_rows, index, args.references, profiles=True)
        unmatched.extend(missing)
    report = {"schema": "bitz/falcon-locality-comparison/v1", "sources": sources,
              "references": args.references, "selected_phases": sorted({row["phase"] for row in rows}),
              "notes": NOTES, "latency": latency, "profiles": profiles, "unmatched": unmatched}
    validation_path = args.directory / "validation.json"
    if validation_path.is_file():
        report["runner_validation"] = read_json(validation_path)
        sources["validation"] = fingerprint(validation_path)
    prefix = args.output_prefix or args.directory / "comparison"
    prefix.parent.mkdir(parents=True, exist_ok=True)
    json_path, text_path = Path(str(prefix) + ".json"), Path(str(prefix) + ".txt")
    json_path.write_text(json.dumps(report, indent=2, sort_keys=True) + "\n")
    text_path.write_text(render(report))
    print(f"Wrote {json_path} and {text_path}: {len(latency)} latency, "
          f"{len(profiles)} diagnostic comparisons; {len(unmatched)} unmatched rows.")


if __name__ == "__main__":
    main()

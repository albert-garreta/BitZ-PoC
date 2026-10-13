#!/usr/bin/env python3
"""Analyze a completed buffer campaign phase without changing the frozen runner."""

import argparse
from collections import defaultdict
import json
from pathlib import Path
import statistics
import sys

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]
sys.path.insert(0, str(ROOT / "scripts"))
import falcon_buffer_optimization as runner
from bench_support import file_hash, write_json

METRICS = [*runner.METRICS, *runner.EXTRA_METRICS]


def stats(trials):
    output = {}
    for metric in METRICS:
        values = [trial[metric] for trial in trials if trial[metric] is not None]
        output[metric] = dict(samples=len(values), median=statistics.median(values),
                              minimum=min(values), maximum=max(values)) if values else None
    return output


def percent(candidate, reference):
    if candidate is None or reference is None or reference == 0:
        return None
    return 100 * (candidate / reference - 1)


def compare(candidate, reference):
    comparisons = {}
    for metric in METRICS:
        a, b = candidate["statistics"][metric], reference["statistics"][metric]
        if a is None or b is None:
            comparisons[metric] = None
            continue
        seeds = sorted(candidate["seeds"].keys() & reference["seeds"].keys())
        per_seed = {}
        for seed in seeds:
            a_seed = candidate["seeds"][seed][metric]
            b_seed = reference["seeds"][seed][metric]
            if a_seed is not None and b_seed is not None:
                per_seed[seed] = percent(a_seed["median"], b_seed["median"])
        deltas = [value for value in per_seed.values() if value is not None]
        comparisons[metric] = dict(delta=a["median"] - b["median"],
                                   percent=percent(a["median"], b["median"]),
                                   per_seed_percent=per_seed,
                                   seeds_lower=sum(value < 0 for value in deltas),
                                   seeds_higher=sum(value > 0 for value in deltas),
                                   comparable_seeds=len(deltas))
    return comparisons


def analyze(phase):
    manifest_path = HERE / "phases" / f"{phase}.json"
    manifest = json.loads(manifest_path.read_text())
    manifest_hash = file_hash(manifest_path)
    # Check completeness before reading/hashing raw artifacts. An in-progress
    # phase is not a performance result and should be resumed by the root runner.
    missing = [job["id"] for job in manifest["jobs"]
               if not (HERE / "runs" / phase / job["id"] / "result.json").exists()]
    if missing:
        raise ValueError(f"phase {phase} incomplete: {len(missing)}/{len(manifest['jobs'])} jobs missing")
    for binary in manifest["binaries"].values():
        if file_hash(binary["path"]) != binary["sha256"]:
            raise ValueError(f"frozen binary changed: {binary['path']}")
    groups = defaultdict(list)
    proofs, sources, baseline_keys = {}, {}, set()
    for job in manifest["jobs"]:
        path = HERE / "runs" / phase / job["id"] / "result.json"
        result = runner.validate_result(path, job, manifest, manifest_hash)
        key = (job["degree"], manifest["batch"], manifest["security"], job["threads"],
               job["protocol"], job["mode"], job["config"])
        groups[key].append(result)
        for trial in result["trials"]:
            source_key = (job["degree"], manifest["batch"], manifest["security"], job["seed"])
            source = (trial["input_digest"], trial["source_root"])
            if sources.setdefault(source_key, source) != source:
                raise ValueError(f"source identity mismatch: {path}")
            proof_key = (*source_key, trial["protocol_id"])
            proof = (trial["proof_debug_digest"], trial["proof_payload_bytes"])
            if proofs.setdefault(proof_key, proof) != proof:
                raise ValueError(f"complete proof mismatch: {path}")
            if job["config"] == "baseline":
                baseline_keys.add(proof_key)
    rows = {}
    for key, results in sorted(groups.items()):
        degree, batch, security, threads, protocol, mode, variant = key
        samples = [trial for result in results for trial in result["trials"] if trial["trial"] == "sample"]
        seed_samples = defaultdict(list)
        for result in results:
            seed_samples[str(result["job"]["seed"])].extend(
                trial for trial in result["trials"] if trial["trial"] == "sample")
        process_peaks = [max(trial["process_peak_rss_kib"] for trial in result["trials"]) for result in results]
        rows[key] = dict(phase=phase, degree=degree, batch=batch, security=security, threads=threads,
                        protocol=protocol, mode=mode, variant=variant, processes=len(results),
                        samples=len(samples), statistics=stats(samples),
                        seeds={seed: stats(trials) for seed, trials in sorted(seed_samples.items())},
                        peak_rss_per_process_kib=dict(median=statistics.median(process_peaks),
                                                      maximum=max(process_peaks)),
                        warmups_per_process=sorted({r["job"]["warmup"] for r in results}),
                        samples_per_process=sorted({r["job"]["iterations"] for r in results}))
    for key, row in rows.items():
        row["comparisons"] = {reference: compare(row, rows[(*key[:-1], reference)])
                               if (*key[:-1], reference) in rows else None
                               for reference in ["control", "baseline"]}
    production = manifest.get("candidate_production", False)
    return dict(phase=phase, manifest_sha256=manifest_hash,
                candidate_production=production,
                primary_reference="baseline" if production else "control",
                raw_artifacts_and_commands_revalidated=True,
                exact_proof_identity_checked=True,
                missing_frozen_baseline_cases_in_this_phase=len(proofs.keys() - baseline_keys),
                rows=list(rows.values()), notes=[
        "Percent deltas are candidate/reference - 1; negative means lower time or memory.",
        "Cold means first commit+prove in a fresh process/Prepared, after setup/input generation; no cache flush.",
        "Repeated samples share one process/Prepared per seed; within-process samples are correlated.",
        "Per-seed median changes indicate consistency, not a confidence interval or statistical significance.",
        "Compare matching protocol, mode, degree, batch, security and worker count only.",
        ("Production 'control' is the candidate-default binary; the primary comparison is the frozen production baseline."
         if production else "Candidate control uses the same instrumented binary; frozen baseline checks overall change."),
        "Peak RSS is process lifetime high-water; separate per-process maxima avoid treating repeated values as independent peaks.",
        "Post-drop RSS includes inputs, Prepared, JSON and allocator retention. Scratch bytes counts only additional coordinate and word-binding cached vector capacity, excluding existing sumcheck scratch.",
        "Null metrics were not collected (old baseline or feature-disabled production candidate); they are not zero.",
        "A subset phase may lack frozen baselines; the root campaign validator checks these across phases.",
        "No new prover execution or performance measurement is performed by this helper.",
    ])


def format_report(analysis):
    primary = analysis.get("primary_reference", "control")
    lines = [f"Buffer experiment analysis: {analysis['phase']}",
             f"Deltas are percentages; negative is lower. Primary reference: {primary}; comparisons stay within protocol and mode.", ""]
    for row in analysis["rows"]:
        s = row["statistics"]
        def median(name):
            return s[name]["median"] if s[name] is not None else None
        def delta(reference, metric="total_prover_ms"):
            comparison = row["comparisons"][reference]
            value = comparison[metric]["percent"] if comparison and comparison[metric] else None
            return "n/a" if value is None else f"{value:+.2f}%"
        def mib(value, divisor=1024):
            return "n/a" if value is None else f"{value / divisor:.2f}"
        reference = row["comparisons"][primary]
        seed_changes = (reference or {}).get("total_prover_ms")
        seed_text = ", ".join(f"{seed}:{value:+.2f}%" if value is not None else f"{seed}:n/a"
                              for seed, value in (seed_changes or {}).get("per_seed_percent", {}).items())
        lines.extend([
            f"N={row['degree']} batch={row['batch']} security={row['security']} t={row['threads']} "
            f"{row['protocol']} {row['mode']} {row['variant']} samples={row['samples']} processes={row['processes']}",
            f"  total={median('total_prover_ms'):.3f}ms prove={median('prove_ms'):.3f}ms "
            f"verify={median('verify_ms'):.3f}ms vs-control={delta('control')} vs-baseline={delta('baseline')}",
            f"  per-seed total vs {primary}: {seed_text or 'n/a'}",
            f"  faults={median('prover_minor_faults')} retainedRSS={mib(median('process_retained_rss_kib'))}MiB "
            f"scratch={mib(median('prepared_retained_scratch_bytes'), 1024 ** 2)}MiB "
            f"peakRSS/process median={mib(row['peak_rss_per_process_kib']['median'])}MiB "
            f"max={mib(row['peak_rss_per_process_kib']['maximum'])}MiB", "",
        ])
    return "\n".join(lines + ["Notes:", *[f"  {note}" for note in analysis["notes"]], ""])


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--phase", required=True)
    args = parser.parse_args()
    if not runner.safe_name(args.phase):
        parser.error("invalid phase name")
    analysis = analyze(args.phase)
    write_json(HERE / f"analysis-{args.phase}.json", analysis)
    report = format_report(analysis)
    (HERE / f"analysis-{args.phase}.txt").write_text(report)
    print(report)


if __name__ == "__main__":
    main()

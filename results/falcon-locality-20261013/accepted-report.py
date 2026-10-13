#!/usr/bin/env python3
"""Analyze only completed accepted-candidate phases and per-trial lifecycle costs.

Default requires all seven phases. --allow-incomplete writes preview artifacts.
No benchmark, build, prover, or global validator is invoked.
"""

import argparse
import importlib.util
import json
from pathlib import Path
import statistics
import sys

sys.dont_write_bytecode = True
OUT = Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location("locality_compare", OUT / "compare.py")
compare = importlib.util.module_from_spec(spec)
spec.loader.exec_module(compare)
PHASES = ("accepted1", "accepted2", "accepted-regress", "accepted-profile", "accepted-padding", "accepted-padding8", "accepted-security100")
LIFECYCLE = "commit_prove_verify_ms"


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--allow-incomplete", action="store_true")
    args = parser.parse_args()
    rows = [row for row in compare.read_json(OUT / "summary.json")["rows"] if row["phase"] in PHASES]
    present = {row["phase"] for row in rows}
    missing = [phase for phase in PHASES if phase not in present]
    if not present or (missing and not args.allow_incomplete):
        parser.error("completed accepted phases missing: " + ", ".join(missing))
    baseline = compare.read_json(OUT / "baseline.json")
    candidate = compare.read_json(OUT / "candidate-accepted-binary.json")
    index = {compare.identity(row): row for row in rows}
    if len(index) != len(rows):
        raise ValueError("duplicate accepted summary row")
    sources = {name: compare.fingerprint(OUT / name) for name in
               ("summary.json", "baseline.json", "candidate-accepted-binary.json", "candidate-accepted-source.json")}
    counts, lifetimes, result_hashes = {}, {}, {}
    for phase in present:
        path = OUT / "phases" / f"{phase}.json"
        manifest = compare.read_json(path)
        if (manifest["binaries"]["baseline"]["sha256"] != baseline["sha256"]
                or manifest["binaries"]["candidate"]["sha256"] != candidate["sha256"]):
            raise ValueError(f"wrong binary identity in {phase}")
        expected = {(phase, job["degree"], manifest["batch"], manifest["security"],
                     job["threads"], job["mode"], manifest["trace"], job["config"])
                    for job in manifest["jobs"]}
        actual = {compare.identity(row) for row in rows if row["phase"] == phase}
        if expected != actual or {key[-1] for key in actual} != {"baseline", "production"}:
            raise ValueError(f"summary coverage differs from manifest: {phase}")
        sources[f"phases/{phase}.json"] = compare.fingerprint(path)
        counts[phase] = dict(processes=len(manifest["jobs"]),
                             proofs_including_warmups=sum(j["warmup"] + j["iterations"] for j in manifest["jobs"]),
                             measured_proofs=sum(j["iterations"] for j in manifest["jobs"]))
        if manifest["trace"]:
            continue
        for job in manifest["jobs"]:
            path = OUT / "runs" / phase / job["id"] / "result.json"
            result = compare.read_json(path)
            if result.get("verified") is not True or result.get("job") != job:
                raise ValueError(f"unverified/mismatched saved result: {path}")
            trials = [t for t in result["trials"] if t["trial"] == "sample"]
            if len(trials) != job["iterations"]:
                raise ValueError(f"wrong post-warmup sample count: {path}")
            key = (phase, job["degree"], manifest["batch"], manifest["security"],
                   job["threads"], job["mode"], False, job["config"])
            values = lifetimes.setdefault(key, [])
            for trial in trials:
                total, verify = compare.numeric(trial["total_prover_ms"]), compare.numeric(trial["verify_ms"])
                if total is None or verify is None:
                    raise ValueError(f"missing lifecycle timing: {path}")
                values.append((str(job["seed"]), total + verify))
            result_hashes[str(path.relative_to(OUT))] = compare.fingerprint(path)["sha256"]
    augmented = []
    for row in rows:
        if row["trace"]:
            continue
        values = lifetimes[compare.identity(row)]
        if len(values) != row["samples"]:
            raise ValueError("lifecycle sample count differs from summary")
        seeds = {seed: dict(metrics, **{LIFECYCLE: statistics.median(v for s, v in values if s == seed)})
                 for seed, metrics in row["seed_medians"].items()}
        augmented.append(dict(row, medians=dict(row["medians"], **{LIFECYCLE: statistics.median(v for _, v in values)}),
                              seed_medians=seeds,
                              metric_samples=dict(row.get("metric_samples", {}), **{LIFECYCLE: len(values)})))
    latency, unmatched = compare.comparisons(augmented, index, ["baseline"])
    profiles = []
    if "accepted-profile" in present:
        path = OUT / "profile-summary.json"
        sources[path.name] = compare.fingerprint(path)
        p_rows = [row for row in compare.read_json(path)["rows"] if row["phase"] == "accepted-profile"]
        if {compare.identity(row) for row in p_rows} != {compare.identity(row) for row in rows if row["phase"] == "accepted-profile"}:
            raise ValueError("incomplete accepted profile summary")
        profiles, p_missing = compare.comparisons(p_rows, index, ["baseline"], profiles=True)
        unmatched.extend(p_missing)
    if unmatched:
        raise ValueError(f"unmatched same-phase/topology references: {unmatched}")
    for row in latency:
        proof = row["metrics"]["proof_payload_bytes"]
        if proof["candidate"] != proof["reference"] or any(x["candidate"] != x["reference"] for x in proof["seeds"].values()):
            raise ValueError("proof payload medians differ")
    report = dict(schema="bitz/falcon-locality-accepted-report/v1", complete=not missing,
                  missing_phases=missing, baseline=baseline, candidate=candidate,
                  sources=sources, lifecycle_result_hashes=result_hashes, phase_counts=counts,
                  latency=latency, profiles=profiles, notes=compare.NOTES)
    lines = ["Falcon accepted candidate assessment", "",
             "PREVIEW — incomplete campaign" if missing else "Completed campaign",
             f"Baseline: {baseline['revision']} / {baseline['sha256']}",
             f"Accepted candidate SHA256: {candidate['sha256']}", "",
             "Candidate policy: norm prefix4 for Falcon512/8 and Falcon1024/12;",
             "Falcon512/12 and other defaults retain their previous policy.",
             "The parallel changes require K=1, instance_vars=10 and modulus>=2^125.",
             "Earlier v1/v2/qualification measurements are excluded. Compare each phase",
             "with its same-phase frozen baseline and identical ordered CPU placement.",
             "No samples are removed, and modes and confirmation passes stay separate.", "",
             "Lifecycle is total_prover_ms + verify_ms within each individual sample,",
             "then summarized by pooled/per-seed medians. It includes commit+prove+verify",
             "and excludes setup, input generation, sampling overhead and teardown.",
             "Verification follows proving in the same process; no isolated verifier",
             "benchmark was performed. These measured verifier costs remain in the report."]
    if missing:
        lines += ["Pending: " + ", ".join(missing)]
    lines += ["", "POLICY CHECKS (descriptive, no significance claim)"]
    for degree, workers, role in ((512, 8, "prefix4"), (1024, 12, "prefix4"), (512, 12, "reverted prefix1")):
        lines += ["", f"Falcon{degree}/{workers} workers: {role}"]
        for mode in ("repeated", "cold"):
            cells = [row for row in latency if row["phase"] in ("accepted1", "accepted2")
                     and row["degree"] == degree and row["threads"] == workers and row["mode"] == mode]
            for row in cells:
                m = row["metrics"]
                lines.append(f"{row['phase']} {mode}: prover {compare.number(m['total_prover_ms']['change_percent'], 2)}%; "
                             f"lifecycle {compare.number(m[LIFECYCLE]['change_percent'], 2)}%; "
                             f"verify {compare.number(m['verify_ms']['change_percent'], 2)}%; "
                             f"process peak {compare.number(row['process_peak_rss']['median']['change_percent'], 2)}%.")
            if len(cells) == 2:
                lines.append(f"{mode}: both pooled prover medians lower="
                             f"{all(row['metrics']['total_prover_ms']['change_percent'] < 0 for row in cells)}; "
                             f"both pooled lifecycle medians higher="
                             f"{all(row['metrics'][LIFECYCLE]['change_percent'] > 0 for row in cells)}.")
            else:
                lines.append(f"{mode}: confirmation pending.")
    lines += ["", "ALL UNTRACED COMPARISONS (negative percentages mean lower measurements)",
              "Every metric includes seed-specific ratios in the accompanying JSON."]
    for row in latency:
        lines += ["", f"{row['phase']} N{row['degree']} batch{row['batch']} security{row['security']} "
                  f"workers{row['threads']} {row['mode']} samples{row['samples']}"]
        for name in ("total_prover_ms", LIFECYCLE, "prove_ms", "witness_commit_ms", "verify_ms", "process_retained_rss_kib", "proof_payload_bytes"):
            lines.append(compare.format_metric(name, row["metrics"][name]))
        for name, value in row["process_peak_rss"].items():
            lines.append(compare.format_metric(f"process_peak_{name}_kib", value))
    lines += ["", "LIGHTWEIGHT STAGES — TRACED DIAGNOSTICS ONLY",
              "Nested wall spans overlap; do not sum parents with children or treat these",
              "timings as untraced latency. Both binaries use the original LIGHT stage set."]
    for row in profiles:
        lines += ["", f"N{row['degree']} workers{row['threads']}"]
        for name in ("falcon_algebraic:integer_outer", "falcon_algebraic:binding", "falcon_algebraic:binding_construction",
                     "falcon_algebraic:word_first_fold", "falcon_bridge:wfbitz_forest", "falcon_algebraic:ring",
                     "falcon_algebraic_ring:coordinates", "falcon_algebraic_ring:collapse", "falcon_algebraic:pcs"):
            if name in row["metrics"]:
                lines.append(compare.format_metric(name, row["metrics"][name]))
    lines += ["", "LIMITS / PROVENANCE",
              "Pooled median changes and two-pass sign consistency do not establish",
              "statistical significance. Repeated samples within each process correlate.",
              "Cold is a fresh process/Prepared, not flushed CPU caches. Peak RSS includes",
              "warmups; retained RSS includes allocator, inputs and Prepared after verifying.",
              "A peak reduction does not establish an allocation-related timing cause.",
              "Padding/security checks have only one sample per seed; their timings do not",
              "qualify a default. The selector uses padded capacity, not exact live batch.",
              "The normalizer attests proof identity/verification and raw provenance before",
              "writing summaries. This helper checks final candidate hash, cell coverage,",
              "same-placement comparisons, payload medians and per-result verified metadata.",
              "It does not rerun the global validator during active phases.",
              "No builds, tests, provers or benchmarks were launched for this analysis."]
    lines.extend(f"{phase}: {json.dumps(count, sort_keys=True)}" for phase, count in sorted(counts.items()))
    suffix = ".preview" if missing else ""
    json_path, text_path = OUT / f"accepted-comparison{suffix}.json", OUT / f"accepted-report{suffix}.txt"
    json_path.write_text(json.dumps(report, indent=2, sort_keys=True) + "\n")
    text_path.write_text("\n".join(lines) + "\n")
    if not missing:
        for old in (OUT / "accepted-comparison.preview.json", OUT / "accepted-report.preview.txt"):
            old.unlink(missing_ok=True)
    print(f"Wrote {text_path} and {json_path}; {len(present)}/{len(PHASES)} phases complete.")


if __name__ == "__main__":
    main()

#!/usr/bin/env python3
"""Report completed final-candidate phases only; never launch or validate runs.

Default: require every planned final phase and write report.txt/final-comparison.json.
--allow-incomplete: report only completed final phases to report.preview.txt and
final-comparison.preview.json. Safe while a different phase is running.
"""

import argparse
import importlib.util
import json
from pathlib import Path
import re
import statistics
import sys

sys.dont_write_bytecode = True
OUT = Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location("locality_compare", OUT / "compare.py")
compare = importlib.util.module_from_spec(spec)
spec.loader.exec_module(compare)

REPEATED = ("final1", "final2")
COLD = ("final-cold1", "final-cold2")
PROFILE = "final-profile"
PADDING = tuple(f"final-padding-b{batch}-s{security}" for security in (100, 128)
                for batch in ([3, 129, 513, 1024] if security == 100 else [3, 129, 513]))
REQUIRED = (*REPEATED, *COLD, PROFILE, *PADDING)
STAGES = {
    "norm (integer_outer)": "falcon_algebraic:integer_outer",
    "binding": "falcon_algebraic:binding",
    "binding construction": "falcon_algebraic:binding_construction",
    "binding first fold": "falcon_algebraic:word_first_fold",
    "forest": "falcon_bridge:wfbitz_forest",
    "ring": "falcon_algebraic:ring",
    "ring coordinates": "falcon_algebraic_ring:coordinates",
    "ring collapse": "falcon_algebraic_ring:collapse",
    "ring carries": "falcon_algebraic_ring:carries",
    "PCS": "falcon_algebraic:pcs",
}


def selected(row):
    return (row["batch"] == 1024 and row["security"] == 128
            and (row["degree"], row["threads"]) in ((512, 8), (512, 12), (1024, 12)))


def fmt(value, precision=3):
    return compare.number(value, precision)


def seed_changes(metric):
    return " ".join(f"{seed}:{fmt(value['change_percent'], 2)}%"
                    for seed, value in metric["seeds"].items())


def lifecycle_rows(rows, manifests):
    """Sum measured commit+prove and verify within each sample, then summarize."""
    samples, hashes = {}, {}
    for phase, manifest in manifests.items():
        if phase not in (*REPEATED, *COLD):
            continue
        for job in manifest["jobs"]:
            path = OUT / "runs" / phase / job["id"] / "result.json"
            result = compare.read_json(path)
            if result.get("verified") is not True or result.get("job") != job:
                raise ValueError(f"unverified or mismatched saved result: {path}")
            trials = [trial for trial in result["trials"] if trial["trial"] == "sample"]
            if len(trials) != job["iterations"]:
                raise ValueError(f"sample count differs from manifest: {path}")
            key = (phase, job["degree"], manifest["batch"], manifest["security"],
                   job["threads"], job["mode"], manifest["trace"], job["config"])
            values = samples.setdefault(key, [])
            for trial in trials:
                total, verify = compare.numeric(trial["total_prover_ms"]), compare.numeric(trial["verify_ms"])
                if total is None or verify is None:
                    raise ValueError(f"missing lifecycle duration: {path}")
                values.append((str(job["seed"]), total + verify))
            hashes[str(path.relative_to(OUT))] = compare.fingerprint(path)["sha256"]
    output = []
    metric = "commit_prove_verify_ms"
    for row in rows:
        key = compare.identity(row)
        if key not in samples:
            continue
        values = samples[key]
        if len(values) != row["samples"]:
            raise ValueError(f"lifecycle sample count differs from summary: {key}")
        output.append(dict(row, medians={metric: statistics.median(value for _, value in values)},
                           metric_samples={metric: len(values)},
                           seed_medians={seed: {metric: statistics.median(value for s, value in values if s == seed)}
                                         for seed in compare.sorted_seeds({s for s, _ in values})}))
    return output, hashes


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--allow-incomplete", action="store_true")
    args = parser.parse_args()
    summary_path = OUT / "summary.json"
    summary = compare.read_json(summary_path)
    rows = [row for row in summary["rows"] if row["phase"] in REQUIRED]
    available = {row["phase"] for row in rows}
    missing = [phase for phase in REQUIRED if phase not in available]
    if not available or (missing and not args.allow_incomplete):
        parser.error("completed final phases missing: " + ", ".join(missing))
    index = {}
    for row in rows:
        key = compare.identity(row)
        if key in index:
            raise ValueError(f"duplicate final summary row: {key}")
        if type(row["trace"]) is not bool:
            raise ValueError("trace flag is not a boolean")
        index[key] = row
    manifests = {}
    baseline = compare.read_json(OUT / "baseline.json")
    candidate = compare.read_json(OUT / "candidate-final-binary.json")
    for phase in available:
        manifest = compare.read_json(OUT / "phases" / f"{phase}.json")
        if manifest["binaries"]["baseline"]["sha256"] != baseline["sha256"]:
            raise ValueError(f"wrong frozen baseline in {phase}")
        if manifest["binaries"]["candidate"]["sha256"] != candidate["sha256"]:
            raise ValueError(f"wrong final candidate in {phase}")
        expected = {(phase, job["degree"], manifest["batch"], manifest["security"],
                     job["threads"], job["mode"], manifest["trace"], job["config"])
                    for job in manifest["jobs"]}
        actual = {compare.identity(row) for row in rows if row["phase"] == phase}
        if actual != expected:
            raise ValueError(f"summary cell set does not match manifest: {phase}")
        if {row["variant"] for row in rows if row["phase"] == phase} != {"baseline", "production"}:
            raise ValueError(f"unexpected variants in {phase}")
        manifests[phase] = manifest
    latency, unmatched = compare.comparisons([row for row in rows if row["trace"] is False], index, ["baseline"])
    profile_path = OUT / "profile-summary.json"
    profiles = []
    if PROFILE in available:
        profile_rows = [row for row in compare.read_json(profile_path)["rows"] if row["phase"] == PROFILE]
        expected = {compare.identity(row) for row in rows if row["phase"] == PROFILE}
        if {compare.identity(row) for row in profile_rows} != expected or any(row["trace"] is not True for row in profile_rows):
            raise ValueError("incomplete or untraced final profile summary")
        profiles, profile_missing = compare.comparisons(profile_rows, index, ["baseline"], profiles=True)
        unmatched.extend(profile_missing)
    if unmatched:
        raise ValueError(f"unmatched final baseline/production rows: {unmatched}")
    for row in latency:
        proof = row["metrics"]["proof_payload_bytes"]
        if proof["candidate"] != proof["reference"] or any(v["candidate"] != v["reference"] for v in proof["seeds"].values()):
            raise ValueError(f"proof byte mismatch in {row['phase']}")
    sources = {"summary": compare.fingerprint(summary_path), "baseline": compare.fingerprint(OUT / "baseline.json"),
               "candidate": compare.fingerprint(OUT / "candidate-final-binary.json"),
               "source": compare.fingerprint(OUT / "candidate-final-source.json")}
    if profiles:
        sources["profiles"] = compare.fingerprint(profile_path)
    for phase in available:
        sources[phase] = compare.fingerprint(OUT / "phases" / f"{phase}.json")
    report = {"schema": "bitz/falcon-locality-final-report/v1", "complete": not missing,
              "missing_phases": missing, "sources": sources, "baseline": baseline,
              "candidate": candidate, "latency": latency, "profiles": profiles,
              "notes": compare.NOTES}
    lifecycle, result_hashes = lifecycle_rows(rows, manifests)
    lifecycle, lifecycle_missing = compare.comparisons(lifecycle, index, ["baseline"])
    if lifecycle_missing:
        raise ValueError(f"unmatched lifecycle rows: {lifecycle_missing}")
    report["lifecycle"] = lifecycle
    report["lifecycle_result_hashes"] = result_hashes
    lines = ["Falcon locality final production report", "",
             "PREVIEW — incomplete final campaign" if missing else "Completed final campaign",
             f"Baseline revision: {baseline['revision']}", f"Baseline SHA256: {baseline['sha256']}",
             f"Final candidate SHA256: {candidate['sha256']}", "",
             "Selected default: four native square-norm prefix rounds only for K=1,",
             "instance_vars=10, modulus>=2^125, and (row_vars,workers)=(10,8),(10,12),",
             "or(11,12). These correspond to Falcon512 at8/12 workers and Falcon1024",
             "at12 workers for the measured batch1024/security128 cases. Existing serial",
             "and rejection policies remain unchanged. Collapse batch1, dense compaction",
             "off and forest row grain1 remain the production defaults.", "",
             "This report contains only final* phases using the hashes above. Earlier",
             "experimental and production-candidate measurements are excluded. Each phase",
             "is compared with its own baseline at the same worker/main CPU placement.",
             "No phase pooling, significance claim, outlier exclusion or traced/untraced",
             "latency mixing is performed. A positive change is a higher measurement."]
    if missing:
        lines += ["", "Still pending: " + ", ".join(missing)]
    lines += ["", "SELECTED-SCOPE CHECKS AND TRADEOFFS",
              "The rows below retain both passes; seed counts do not imply significance."]
    for degree, workers in ((512, 8), (512, 12), (1024, 12)):
        cell = [row for row in latency if row["phase"] in REPEATED
                and row["degree"] == degree and row["threads"] == workers]
        changes = "; ".join(f"{row['phase']} {fmt(row['metrics']['total_prover_ms']['change_percent'], 2)}%"
                            for row in cell)
        seed_values = [value["change_percent"] for row in cell
                       for value in row["metrics"]["total_prover_ms"]["seeds"].values()]
        both_lower = len(cell) == 2 and all(row["metrics"]["total_prover_ms"]["change_percent"] < 0 for row in cell)
        norm = next((row["metrics"].get("falcon_algebraic:integer_outer") for row in profiles
                     if row["degree"] == degree and row["threads"] == workers), None)
        norm_note = (f"norm stage {fmt(norm['change_percent'], 2)}%" if norm else
                     "norm stage pending" if workers == 12 else "no final8-worker norm profile")
        lines.append(f"N{degree}/{workers}w: {changes or 'pending'}; "
                     f"lower seed medians {sum(value < 0 for value in seed_values)}/{len(seed_values)}; "
                     f"two pooled wins={both_lower if len(cell) == 2 else 'pending'}; {norm_note}.")
        for row in cell:
            lines.append(f"  {row['phase']}: verify {fmt(row['metrics']['verify_ms']['change_percent'], 2)}%; "
                         f"process peak {fmt(row['process_peak_rss']['median']['change_percent'], 2)}%.")
            measured = next(value["metrics"]["commit_prove_verify_ms"] for value in lifecycle
                            if value["phase"] == row["phase"] and value["degree"] == degree
                            and value["threads"] == workers)
            lines.append(f"  {row['phase']} measured commit+prove+verify: "
                         f"{fmt(measured['reference'])}→{fmt(measured['candidate'])}ms "
                         f"({fmt(measured['change_percent'], 2)}%).")
    for row in latency:
        if row["phase"] in COLD:
            lines.append(f"{row['phase']} N{row['degree']}/12w: total "
                         f"{fmt(row['metrics']['total_prover_ms']['change_percent'], 2)}%, "
                         f"verify {fmt(row['metrics']['verify_ms']['change_percent'], 2)}%, "
                         f"process peak {fmt(row['process_peak_rss']['median']['change_percent'], 2)}%.")
    for row in lifecycle:
        if row["phase"] in COLD:
            metric = row["metrics"]["commit_prove_verify_ms"]
            lines.append(f"{row['phase']} N{row['degree']}/12w measured commit+prove+verify: "
                         f"{fmt(metric['reference'])}→{fmt(metric['candidate'])}ms "
                         f"({fmt(metric['change_percent'], 2)}%).")
    for phase_group, title in ((REPEATED, "REPEATED COMMIT+PROVE CONFIRMATION"), (COLD, "COLD COMMIT+PROVE CONFIRMATION")):
        lines += ["", title, "phase             N workers new_norm  baseline_ms candidate_ms change%  seed changes"]
        for row in latency:
            if row["phase"] not in phase_group:
                continue
            metric = row["metrics"]["total_prover_ms"]
            lines.append(f"{row['phase']:16} {row['degree']:4} {row['threads']:7} {str(selected(row)):8} "
                         f"{fmt(metric['reference']):>11} {fmt(metric['candidate']):>12} "
                         f"{fmt(metric['change_percent'], 2):>7}%  {seed_changes(metric)}")
        lines += ["", "Per-phase flags; all measurements retained (including unchanged selector paths):"]
        for row in latency:
            if row["phase"] not in phase_group:
                continue
            metric = row["metrics"]["total_prover_ms"]
            slower = [seed for seed, value in metric["seeds"].items() if value["change_percent"] > 0]
            if metric["change_percent"] > 0 or slower:
                lines.append(f"  {row['phase']} N{row['degree']}/{row['threads']}w: pooled "
                             f"{fmt(metric['change_percent'], 2)}%; higher seed medians {slower}.")
    lines += ["", "COMPONENTS, VERIFIER AND MEMORY (untraced main repeated/cold phases)",
              "Durations are independent stage medians; do not add them to reconstruct total.",
              "Process peak uses each process's lifetime maximum, including warmups. Retained",
              "RSS includes inputs, allocator and Prepared after proof/verification; it is",
              "not cached-scratch capacity. Neither proves a memory-allocation cause.",
              "phase             N workers metric                         baseline   candidate   change%"]
    for row in latency:
        if row["phase"] not in (*REPEATED, *COLD):
            continue
        metrics = {name: row["metrics"][name] for name in ("prove_ms", "witness_commit_ms", "verify_ms", "process_retained_rss_kib", "proof_payload_bytes")}
        metrics["process_peak_median_kib"] = row["process_peak_rss"]["median"]
        metrics["process_peak_maximum_kib"] = row["process_peak_rss"]["maximum"]
        for name, value in metrics.items():
            precision = 0 if name.endswith(("kib", "bytes")) else 3
            lines.append(f"{row['phase']:16} {row['degree']:4} {row['threads']:7} {name:30} "
                         f"{fmt(value['reference'], precision):>10} {fmt(value['candidate'], precision):>11} "
                         f"{fmt(value['change_percent'], 2):>8}%")
    lines += ["", "MEASURED COMMIT+PROVE+VERIFY (sum per trial BEFORE taking medians)",
              "Each sample is total_prover_ms + verify_ms from the same proof. These sums",
              "exclude input generation, preparation, resource sampling and teardown.",
              "They are not sums of separately computed stage medians. Warmups are excluded.",
              "phase             N workers baseline_ms candidate_ms change%  seed changes"]
    for row in lifecycle:
        metric = row["metrics"]["commit_prove_verify_ms"]
        lines.append(f"{row['phase']:16} {row['degree']:4} {row['threads']:7} "
                     f"{fmt(metric['reference']):>11} {fmt(metric['candidate']):>12} "
                     f"{fmt(metric['change_percent'], 2):>7}%  {seed_changes(metric)}")
    lines += ["", "FINAL LIGHTWEIGHT PROFILE — DIAGNOSTIC ONLY",
              "Baseline and final production use the original lightweight stage allowlist.",
              "Traced wall spans overlap and include instrumentation overhead; do not treat",
              "their deltas as untraced latency or add parent and child stages.",
              " N stage                       baseline_ms candidate_ms change%  seed changes"]
    for row in profiles:
        for label, name in STAGES.items():
            if name not in row["metrics"]:
                continue
            value = row["metrics"][name]
            lines.append(f"{row['degree']:4} {label:28} {fmt(value['reference']):>11} {fmt(value['candidate']):>12} "
                         f"{fmt(value['change_percent'], 2):>7}%  {seed_changes(value)}")
    lines += ["", "PADDING / SECURITY CHECKS",
              "These have one cold proof per seed/case, so timing differences are descriptive",
              "and do not qualify a performance default. Same-seed payload medians match.",
              "phase                           N samples baseline_ms candidate_ms verify_change% bytes"]
    for row in latency:
        if row["phase"] not in PADDING:
            continue
        total, verify, proof = (row["metrics"][name] for name in ("total_prover_ms", "verify_ms", "proof_payload_bytes"))
        lines.append(f"{row['phase']:30} {row['degree']:4} {row['samples']['candidate']:7} "
                     f"{fmt(total['reference']):>11} {fmt(total['candidate']):>12} "
                     f"{fmt(verify['change_percent'], 2):>14}% {fmt(proof['candidate'], 0)}")
    counts = {phase: {"processes": len(manifest["jobs"]),
                      "proofs_including_warmups": sum(job["warmup"] + job["iterations"] for job in manifest["jobs"]),
                      "measured_proofs": sum(job["iterations"] for job in manifest["jobs"])}
              for phase, manifest in sorted(manifests.items())}
    report["phase_counts"] = counts
    lines += ["", "PHASE COUNTS AND VALIDATION SCOPE"]
    lines.extend(f"{phase}: {json.dumps(value, sort_keys=True)}" for phase, value in counts.items())
    lines += ["The runner creates summaries only after its proof/source/input, full proof",
              "identity, payload and raw/binary/parser provenance checks. This analysis",
              "trusts those completed summaries and checks final binary hashes, manifest",
              "cell coverage, matching topology and payload medians. It does not rerun",
              "proof verification or the global validator while other phases are active.",
              "", "TEST LOG SUMMARIES (reported individually; do not sum overlapping suites)"]
    test_logs = sorted(OUT.glob("test-*.log"))
    for path in test_logs:
        matches = [line.strip() for line in path.read_text().splitlines()
                   if re.search(r"test result:|^Ran \d+ tests? in|^OK$|^FAILED", line)]
        if matches:
            lines.extend([path.name + ":", *["  " + line for line in matches]])
            sources[path.name] = compare.fingerprint(path)
    lines += ["", "CONFIDENCE LIMITS",
              "Repeated samples share one process and Prepared; cold means first proof in",
              "a fresh process/Prepared, not flushed CPU caches. Each pass has three seed",
              "processes per repeated cell; report both passes without pooling them.",
              "No universal verifier-speed or zero-regression claim follows from medians.",
              "Keep any positive total/seed/verify differences in the assessment. Unchanged",
              "paths can also fluctuate. No cause is inferred solely from faults or RSS.",
              "instance_vars=10 is a padded-capacity condition, not an exact live-batch guard.",
              "The padding checks establish limited exercised cases, not all throughput",
              "behavior. This host's CPU placement/cache topology limits generalization.",
              "The report generator launched no tests, builds, provers or benchmarks."]
    report_path = OUT / ("report.preview.txt" if missing else "report.txt")
    data_path = OUT / ("final-comparison.preview.json" if missing else "final-comparison.json")
    data_path.write_text(json.dumps(report, indent=2, sort_keys=True) + "\n")
    report_path.write_text("\n".join(lines) + "\n")
    if not missing:
        for stale_preview in (OUT / "report.preview.txt", OUT / "final-comparison.preview.json"):
            stale_preview.unlink(missing_ok=True)
    print(f"Wrote {report_path} and {data_path}; {len(available)}/{len(REQUIRED)} phases complete.")


if __name__ == "__main__":
    main()

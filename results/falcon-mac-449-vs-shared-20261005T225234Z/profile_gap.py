#!/usr/bin/env python3
"""Profile the saved seed-42 Mac proof gap, serially, without rebuilding.

Default: baseline then candidate, each B1024/t8/security128, one warmup and
five measured proofs. Run only after the confirmation campaign has finished.
--summarize-only revalidates existing profile artifacts without launching jobs.
All generated files stay in profiles/; comparison matrix records are read-only.
"""
import argparse
from collections import defaultdict
import json
import math
from pathlib import Path
import statistics
import subprocess
import time

import run_comparison as comparison


OUT = Path(__file__).resolve().parent
PROFILES = OUT / "profiles"
LABELS = ("baseline", "candidate")
ROOTS = ("falcon_bench:commit", "falcon_bench:prove", "falcon_bench:verify")
TIMINGS = ("witness_commit_ms", "proof_prove_ms", "total_prover_ms", "proof_verify_ms")
GRINDING = ("pcs_grind_pow_ms", "gkr_spartan_grinding_ms", "other_spartan_grinding_ms")
IDENTITY = ("input_digest", "proof_debug_digest", "roots", "proof_payload_bytes")


def require(condition, message):
    if not condition:
        raise ValueError(message)


def read_json(path):
    return json.loads(path.read_text())


def write_json(path, value):
    path.write_text(json.dumps(value, indent=2, allow_nan=False) + "\n")


def reference(label):
    path = OUT / f"matrix-s128-b1024-t8-seed42-BA-{label}.run.json"
    record = read_json(path)
    require(record.get("exit") == 0 and record.get("label") == label, f"invalid reference: {path}")
    require(record.get("security") == 128 and record.get("batch") == 1024
            and record.get("threads") == 8 and record.get("seed") == 42,
            f"reference shape differs: {path}")
    require(len(record["trials"]) == 6 and all(t["verified"] for t in record["trials"]),
            f"reference proofs incomplete: {path}")
    return record


def command(label):
    cmd = [comparison.META[label]["binary"], "--security", "128", "--batch", "1024",
           "--threads", "8", "--seed", "42", "--warmup", "1", "--iterations", "5"]
    if label == "candidate":
        cmd += ["--protocol", "shared-prime"]
    return cmd


def validate_binary(label):
    item = comparison.META[label]
    require(comparison.digest(item["binary"]) == item["sha256"], f"saved {label} binary hash differs")


def run_profiles():
    # Validate both binaries/references before either process is started.
    for label in LABELS:
        validate_binary(label)
        reference(label)
        for suffix in ("jsonl", "stderr", "run.json"):
            require(not (PROFILES / f"{label}.{suffix}").exists(),
                    f"profile artifact already exists: {label}.{suffix}; use --summarize-only")
    PROFILES.mkdir(exist_ok=True)
    for label in LABELS:
        validate_binary(label)
        env = comparison.environment(label)
        env["BITZ_FALCON_STAGE_TIMINGS"] = "1"
        require([key for key in env if key.startswith(("BITZ_", "FLOCK_"))]
                == ["BITZ_FALCON_STAGE_TIMINGS"], "unexpected benchmark tracing environment")
        item = comparison.META[label]
        cmd = command(label)
        record = dict(label=label, command=cmd, started=time.time(),
                      binary_sha256=item["sha256"], revision=item["revision"],
                      runtime_revision=item.get("runtime_revision", item["revision"]),
                      environment_overrides={"BITZ_FALCON_STAGE_TIMINGS": "1",
                                             "RUSTFLAGS": env["RUSTFLAGS"],
                                             "CARGO_TARGET_DIR": env["CARGO_TARGET_DIR"]})
        print(f"PROFILE {label}: security128 B1024 t8 seed42, 1 warmup + 5 samples", flush=True)
        started = time.monotonic()
        with (PROFILES / f"{label}.jsonl").open("x") as stdout, \
                (PROFILES / f"{label}.stderr").open("x") as stderr:
            process = subprocess.run(cmd, cwd=item["repo"], env=env, stdout=stdout, stderr=stderr)
        record.update(exit=process.returncode, elapsed_seconds=time.monotonic() - started)
        write_json(PROFILES / f"{label}.run.json", record)
        require(process.returncode == 0, f"{label} profile failed; inspect profiles/{label}.stderr")
        # Stop before the second process if the first changed the proof.
        validate_profile(label)
        print(f"PROFILE {label} validated", flush=True)


def validate_profile(label):
    validate_binary(label)
    run = read_json(PROFILES / f"{label}.run.json")
    require(run.get("exit") == 0 and run.get("command") == command(label), f"{label} process differs")
    require(run.get("binary_sha256") == comparison.META[label]["sha256"], f"{label} binary provenance differs")
    require(run.get("environment_overrides", {}).get("BITZ_FALCON_STAGE_TIMINGS") == "1",
            f"{label} stage recording was not requested")
    rows = [json.loads(line) for line in (PROFILES / f"{label}.jsonl").read_text().splitlines() if line.strip()]
    require(len(rows) == 8 and rows[0].get("event") == "prepared"
            and rows[-1].get("event") == "summary", f"{label} stdout event sequence differs")
    header, trials, summary = rows[0], rows[1:-1], rows[-1]
    ref = reference(label)
    excluded = {"input_setup_ms", "prepare_ms", "stage_timings"}
    require({k: v for k, v in header.items() if k not in excluded}
            == {k: v for k, v in ref["prepared"].items() if k not in excluded},
            f"{label} profile header differs from the saved matrix")
    require(header.get("stage_timings") is True, f"{label} stage logging disabled")
    for index, (trial, expected) in enumerate(zip(trials, ref["trials"])):
        require(trial.get("event") == "trial" and trial.get("verified") is True,
                f"{label} trial {index} did not verify")
        require(trial.get("trial") == ("warmup" if index == 0 else "sample")
                and trial.get("sample") == (None if index == 0 else index),
                f"{label} trial ordering differs")
        require(all(trial.get(key) == expected.get(key) for key in IDENTITY),
                f"{label} trial {index} input/proof/roots/payload changed")
        for key in TIMINGS:
            require(isinstance(trial.get(key), (int, float)) and math.isfinite(trial[key])
                    and trial[key] > 0, f"{label} trial {index} invalid {key}")
    require(summary.get("verified") is True and summary.get("samples") == 5,
            f"{label} summary incomplete")
    return run, trials, ref


def closed_trial_spans(stderr_text):
    """Group child-close records when their root closes; span IDs may be reused.

    No process-global span-ID map is used. A root's records are discarded from
    pending immediately on its close, then attached to its explicit trial field.
    Repeated exact paths are retained and summed, never deduplicated by ID.
    """
    pending = defaultdict(list)
    trials = defaultdict(dict)
    ignored = 0
    for line_number, line in enumerate(stderr_text.splitlines(), 1):
        if not line.lstrip().startswith("{"):
            continue
        row = json.loads(line)
        if row.get("event") != "stage":
            continue
        path, ids = row.get("path"), row.get("ancestor_ids")
        require(isinstance(path, list) and path and isinstance(ids, list)
                and len(path) == len(ids) and path[-1] == row.get("name")
                and ids[-1] == row.get("span_id"), f"malformed stage at stderr line {line_number}")
        elapsed = row.get("elapsed_ms")
        require(isinstance(elapsed, (int, float)) and math.isfinite(elapsed) and elapsed >= 0,
                f"invalid stage duration at stderr line {line_number}")
        if path[0] not in ROOTS:
            ignored += 1
            continue
        key = (path[0], ids[0])
        pending[key].append(row)
        if len(path) == 1:
            trial = row.get("fields", {}).get("trial")
            require(type(trial) is int and 0 <= trial <= 5, "missing/invalid root trial field")
            require(path[0] not in trials[trial], f"duplicate closed root {path[0]} for trial {trial}")
            trials[trial][path[0]] = pending.pop(key)
    require(not pending, "unclosed trial roots: stage coverage is incomplete")
    require(set(trials) == set(range(6)), "expected warmup trial0 and measured trials1..5")
    require(all(set(roots) == set(ROOTS) for roots in trials.values()), "missing complete phase boundary")
    return trials, ignored


def trial_metrics(spans, raw):
    proof = spans["falcon_bench:prove"]
    sums = defaultdict(float)
    counts = defaultdict(int)
    for rows in spans.values():
        for row in rows:
            path = tuple(row["path"])
            sums[path] += row["elapsed_ms"]
            counts[path] += 1
    selected = {key: [] for key in GRINDING}
    for row in proof:
        path, name = row["path"], row["name"]
        # Outer PCS proof-of-work spans include their Spartan child. Count
        # that outer duration only, never both parent and child.
        if name == "lig:grind_pow" and path.count(name) == 1:
            selected["pcs_grind_pow_ms"].append(row)
        elif name == "spartan:grinding" and "lig:grind_pow" not in path and path.count(name) == 1:
            key = ("gkr_spartan_grinding_ms" if "falcon_bridge:wfbitz_forest" in path
                   else "other_spartan_grinding_ms")
            selected[key].append(row)
    metrics = {key: raw[key] for key in TIMINGS}
    metrics["proof_root_stage_ms"] = sums[("falcon_bench:prove",)]
    coverage = {}
    for key, rows in selected.items():
        # Even with a closed phase boundary, do not invent zero for a missing
        # instrumentation family. Missing families leave the residual unknown.
        metrics[key] = sum(row["elapsed_ms"] for row in rows) if rows else None
        coverage[key] = dict(status="observed" if rows else "unknown_missing_span", count=len(rows))
    complete = all(metrics[key] is not None for key in GRINDING)
    total = sum(metrics[key] for key in GRINDING) if complete else None
    metrics["recorded_grinding_ms"] = total
    metrics["proof_minus_recorded_grinding_ms"] = None
    if total is not None:
        require(total <= raw["proof_prove_ms"], "grinding sum exceeds proof time; overlap or coverage ambiguity")
        metrics["proof_minus_recorded_grinding_ms"] = raw["proof_prove_ms"] - total
    return dict(metrics=metrics, grinding_coverage=coverage,
                stage_sums_ms={"/".join(path): value for path, value in sorted(sums.items())},
                stage_counts={"/".join(path): count for path, count in sorted(counts.items())})


def nullable_median(values):
    return statistics.median(values) if all(value is not None for value in values) else None


def aggregate(label):
    run, raw_trials, ref = validate_profile(label)
    spans, ignored = closed_trial_spans((PROFILES / f"{label}.stderr").read_text())
    per_trial = [dict(trial=i, warmup=i == 0, **trial_metrics(spans[i], raw_trials[i])) for i in range(6)]
    measured = per_trial[1:]
    names = set().union(*(trial["stage_sums_ms"] for trial in measured))
    return dict(run=run, verified_trials=6, identity_matches_reference=True,
                ignored_stages_outside_trial_roots=ignored, per_trial=per_trial,
                medians_ms={key: nullable_median([trial["metrics"][key] for trial in measured])
                            for key in measured[0]["metrics"]},
                exact_path_medians_ms={name: nullable_median([trial["stage_sums_ms"].get(name) for trial in measured])
                                       for name in sorted(names)},
                uninstrumented_matrix_medians_ms=ref["medians"],
                proof_payload_bytes=raw_trials[0]["proof_payload_bytes"],
                proof_debug_digest=raw_trials[0]["proof_debug_digest"], input_digest=raw_trials[0]["input_digest"])


def compare(a, b):
    return dict(baseline_ms=a, candidate_ms=b,
                delta_ms=None if a is None or b is None else b - a,
                candidate_over_baseline=None if a is None or b is None or a == 0 else b / a)


def summarize():
    runs = {label: aggregate(label) for label in LABELS}
    require(runs["baseline"]["input_digest"] == runs["candidate"]["input_digest"], "profile input corpora differ")
    metrics = {key: compare(runs["baseline"]["medians_ms"][key], runs["candidate"]["medians_ms"][key])
               for key in runs["baseline"]["medians_ms"]}
    raw = {key: compare(runs["baseline"]["uninstrumented_matrix_medians_ms"][key],
                        runs["candidate"]["uninstrumented_matrix_medians_ms"][key]) for key in TIMINGS}
    summary = dict(schema="bitz/falcon-mac-profile-gap/v1", status="complete", diagnostic_only=True,
                   shape=dict(security=128, batch=1024, threads=8, seed=42, warmups=1, samples=5),
                   method="Group close records by root lifetime and explicit trial. Sum repetitions of each exact path per trial. Exclude trial0, then median five trials. Residuals are subtracted per trial before taking their median.",
                   limitations=["Instrumented timings are diagnostic and do not replace uninstrumented comparisons.",
                                "PCS grind_pow includes its nested Spartan grinding; GKR and other Spartan categories exclude PCS descendants.",
                                "Missing grinding families or exact paths are unknown, never silently zero.",
                                "Residual contains all proof work not covered by the selected grinding spans, including logging overhead; it is not a separately measured algorithm stage.",
                                "All trials replay the same proof and therefore do not sample independent grinding nonces."],
                   comparison=metrics, uninstrumented_matrix_comparison=raw, runs=runs)
    write_json(PROFILES / "summary.json", summary)
    def number(value):
        return "unknown" if value is None else f"{value:.3f}"
    lines = ["Mac seed-42 proof gap: bounded instrumented diagnostic", "12 verified proofs; 1 warmup and 5 measured proofs per saved binary.",
             "Each input/proof digest, roots and payload match its own uninstrumented matrix reference.",
             "All numbers below are medians of five per-trial values, milliseconds.", "", "metric | native449 | shared | shared-minus-native"]
    for key in ("proof_prove_ms", "proof_root_stage_ms", *GRINDING, "recorded_grinding_ms", "proof_minus_recorded_grinding_ms"):
        item = metrics[key]
        lines.append(f"{key} | {number(item['baseline_ms'])} | {number(item['candidate_ms'])} | {number(item['delta_ms'])}")
    lines += ["", "Uninstrumented matrix proof-only medians (for context):",
              f"native {number(raw['proof_prove_ms']['baseline_ms'])} ms; shared {number(raw['proof_prove_ms']['candidate_ms'])} ms.", "",
              *summary["limitations"], "", "Exact-path sums, counts, coverage, per-trial residuals and provenance: summary.json."]
    (PROFILES / "REPORT.txt").write_text("\n".join(lines) + "\n")
    print(f"PROFILE SUMMARY {PROFILES / 'summary.json'}", flush=True)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--summarize-only", action="store_true")
    options = parser.parse_args()
    if not options.summarize_only:
        run_profiles()
    summarize()

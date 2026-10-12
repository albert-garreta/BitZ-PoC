#!/usr/bin/env python3
"""Audit frozen experiment artifacts after benchmarks and monitoring stop.

Run: taskset -c 31 python3 final_audit.py
Writes final-audit.json even on assertion failure, and profile-equivalence.json
after accepted-record validation. Does not run a prover or change source/report.
"""
import collections
import functools
import hashlib
import json
import math
import os
import pathlib
import re
import statistics
import time
import traceback

OUT = pathlib.Path(__file__).resolve().parent
WORKERS = (6, 7, 4, 5, 2, 3, 0, 1, 14, 15, 12, 13, 10, 11, 8, 9)
THRESHOLD = 128 + math.log2(16 / 15)
AUDIT = {"status": "running", "started_epoch": time.time(), "flags": []}


def read(path):
    return json.loads(path.read_text())


def save(path, value):
    path.write_text(json.dumps(value, indent=2, allow_nan=False) + "\n")


def require(condition, message):
    if not condition:
        raise AssertionError(message)


@functools.lru_cache(maxsize=None)
def sha256(path):
    digest = hashlib.sha256()
    with pathlib.Path(path).open("rb") as handle:
        for block in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def json_lines(path):
    return [json.loads(line) for line in path.read_text().splitlines()
            if line.startswith("{")]


def near(a, b):
    return math.isfinite(a) and math.isfinite(b) and math.isclose(
        a, b, rel_tol=1e-12, abs_tol=1e-9)


def profile_key(row):
    return row["variant"], row["degree"], row["threads"], row["seed"], row["profile"]


def raw_proof_counts(paths):
    counts = collections.Counter(files=0, trials=0, verified=0,
                                 unverified=0, sample=0, warmup=0)
    failures = []
    for path in sorted(paths):
        counts["files"] += 1
        for row in json_lines(path):
            if row.get("trial") not in ("sample", "warmup"):
                continue
            counts["trials"] += 1
            counts[row["trial"]] += 1
            if row.get("verified") is True:
                counts["verified"] += 1
            else:
                counts["unverified"] += 1
                failures.append(str(path.relative_to(OUT)))
    return dict(counts) | {"unverified_record_files": sorted(set(failures))}


def validate_record(path, source_manifest_hash, build_interval):
    row = read(path)
    stem = path.name.removesuffix(".result.json")
    prefix = path.parent / stem
    command_path = prefix.with_name(stem + ".command.json")
    resources_path = prefix.with_name(stem + ".resources.json")
    command, resource = read(command_path), read(resources_path)
    records = json_lines(prefix.with_name(stem + ".jsonl"))
    trials = [r for r in records if r.get("trial") in ("sample", "warmup")]
    samples = [r for r in trials if r["trial"] == "sample"]
    require(len(trials) == 8 and len(samples) == 7,
            f"{stem}: expected one warmup and seven samples")
    require([r["trial"] for r in trials] == ["warmup"] + ["sample"] * 7,
            f"{stem}: unexpected trial order")
    require(all(r.get("verified") is True for r in trials),
            f"{stem}: proof did not verify")
    require(row["verified_proofs"] == 8, f"{stem}: result proof count")
    require(row["phase"] == path.parent.name, f"{stem}: wrong phase")
    require(resource["exit_code"] == 0, f"{stem}: nonzero process exit")
    require(all(r["degree"] == row["degree"] and r["batch"] == 1024
                and r["threads"] == row["threads"] for r in trials),
            f"{stem}: wrong workload in raw records")
    require({r["input_digest"] for r in trials} == {row["input_digest"]},
            f"{stem}: input digest varies")
    require({r["proof_debug_digest"] for r in trials} == {row["proof_digest"]},
            f"{stem}: proof digest varies")
    root_bytes = 32 if row["variant"] == "full" else 0
    require({r["proof_payload_bytes"] + root_bytes for r in trials}
            == {row["proof_bytes"]}, f"{stem}: proof size disagrees")
    securities = [r["algebraic_security_bits"] for r in records
                  if "algebraic_security_bits" in r]
    require(bool(securities) and all(math.isfinite(b) and b >= THRESHOLD - 1e-10
                                    for b in securities),
            f"{stem}: security below composed threshold")
    require(all(near(b, row["security_bits"]) for b in securities),
            f"{stem}: result security disagrees")
    cpus = WORKERS[:row["threads"]]
    affinities = [r["cpu_affinity"] for r in records if "cpu_affinity" in r]
    require(bool(affinities), f"{stem}: missing affinity evidence")
    require(all(a["observed_worker_cpus"] == [[c] for c in cpus]
                and a["observed_main_cpus"] == [6] for a in affinities),
            f"{stem}: unexpected worker/main affinity")
    require(max(command["idle_busy_pct"].values()) < 5,
            f"{stem}: failed idle criterion")
    sibling_max = max(resource["sibling_busy_pct"].values())
    require(sibling_max < 2 and near(sibling_max, row["max_sibling_busy_pct"]),
            f"{stem}: failed sibling criterion/result mismatch")
    metrics = {"commit": "witness_commit_ms", "total": "total_prover_ms",
               "prove": "prove_ms" if row["variant"] == "arithmetic" else "proof_prove_ms",
               "verify": "verify_ms" if row["variant"] == "arithmetic" else "proof_verify_ms"}
    for metric, raw_key in metrics.items():
        values = [r[raw_key] for r in samples]
        require(all(math.isfinite(x) and x >= 0 for x in values),
                f"{stem}: invalid {metric} times")
        require(values == row[metric + "_samples_ms"],
                f"{stem}: {metric} sample arrays differ")
        require(near(statistics.median(values), row[metric + "_ms"]),
                f"{stem}: {metric} median differs")
    args, settings = command["argv"], command["environment"]
    for flag, expected in (("--degree", row["degree"]), ("--batch", 1024),
                           ("--security", 128), ("--threads", row["threads"]),
                           ("--seed", row["seed"]), ("--warmup", 1),
                           ("--iterations", 7)):
        require(args[args.index(flag) + 1] == str(expected),
                f"{stem}: command {flag} mismatch")
    executable = pathlib.Path(args[3])
    cost = pathlib.Path(settings.get("BITZ_EXPERIMENT_COST_FILE", OUT / "costs/curves.json"))
    fingerprint = hashlib.sha256(json.dumps({
        "argv": args, "settings": settings, "binary": sha256(str(executable)),
        "costs": sha256(str(cost)), "sources": source_manifest_hash,
    }, sort_keys=True).encode()).hexdigest()
    require(fingerprint == command["execution_fingerprint"] == row["execution_fingerprint"],
            f"{stem}: execution fingerprint mismatch")
    # command is saved before spawning; resources are saved after waiting.
    # Their mtimes conservatively bracket the run while original files remain
    # intact. This cannot validate timestamps of copied/recreated artifacts.
    start, end = command_path.stat().st_mtime, resources_path.stat().st_mtime
    require(start <= end, f"{stem}: reversed file timestamp interval")
    require(not (start <= build_interval[1] and end >= build_interval[0]),
            f"{stem}: accepted run overlaps known external build")
    return row, {"case": stem, "phase": row["phase"],
                 "start_epoch_from_command_mtime": start,
                 "end_epoch_from_resources_mtime": end}


def validation_counts():
    totals = {}
    expression = re.compile(r"test result: ok\. (\d+) passed; (\d+) failed;")
    for name in ("validation", "validation-external"):
        records = read(OUT / name / "summary.json")
        passed = 0
        for row in records:
            match = expression.search(row["result"])
            require(match is not None, f"{name}: missing successful test summary")
            require(int(match[2]) == 0 and int(match[1]) > 0,
                    f"{name}: failed or empty test invocation")
            passed += int(match[1])
        log_passed = 0
        log_invocations = 0
        for path in sorted((OUT / name).glob("*.log")):
            summaries = expression.findall(path.read_text())
            require(len(summaries) == 1, f"{path}: ambiguous/failed test log")
            require(int(summaries[0][1]) == 0, f"{path}: failed tests")
            log_passed += int(summaries[0][0])
            log_invocations += 1
        require(log_invocations == len(records) and log_passed == passed,
                f"{name}: logs and summary counts disagree")
        totals[name] = {"invocations": len(records), "passed_test_executions": passed,
                        "failed_test_executions": 0,
                        "note": "Repeated tests under distinct configurations count separately."}
    return totals


def main():
    os.sched_setaffinity(0, {31})
    provenance = read(OUT / "provenance.json")
    selection = read(OUT / "selection.json")
    original = read(OUT.parent / "falcon-grinding-opt-128-d9b13bdb4/provenance.json")
    source = pathlib.Path(provenance["source_checkout"])
    source_hashes = read(OUT / "source-hashes.json")
    require(isinstance(source_hashes, dict) and bool(source_hashes), "source manifest format")
    for relative, expected in source_hashes.items():
        require(isinstance(expected, str) and re.fullmatch(r"[0-9a-f]{64}", expected),
                f"source manifest digest format: {relative}")
        require(sha256(str(source / relative)) == expected, f"changed source: {relative}")
    binaries = {}
    for label, manifest in (("experimental", provenance), ("original", original)):
        for name, entry in manifest["binaries"].items():
            actual = sha256(entry["path"])
            require(actual == entry["sha256"], f"changed binary: {label}/{name}")
            binaries[f"{label}/{name}"] = actual
    require(sha256(str(OUT / "costs/curves.json")) == provenance["cost_file_sha256"],
            "initial cost file changed")
    require(sha256(str(OUT / provenance["external_cost_file"]))
            == provenance["external_cost_file_sha256"], "external cost file changed")
    require(sha256(str(OUT / "screen/summary.json")) == selection["screen_summary_sha256"],
            "screen summary changed after candidate freeze")
    AUDIT["provenance"] = {"source_files_checked": len(source_hashes),
                            "binary_sha256": binaries,
                            "source_manifest_sha256": sha256(str(OUT / "source-hashes.json")),
                            "frozen_screen_summary_sha256": selection["screen_summary_sha256"]}
    build = read(OUT / "external-build.json")
    interval = build["start_epoch"], build["exit_observed_epoch"]
    require(interval[0] < interval[1], "invalid external build interval")
    all_rows, intervals = [], []
    phases = {}
    workloads = [(kind, degree, 8 if kind == "arithmetic" else 16)
                 for kind in ("arithmetic", "full") for degree in (512, 1024)]
    candidates = {(r["variant"], r["degree"], r["threads"]): r["profile"]
                  for r in selection["candidates"]}
    require(set(candidates) == set(workloads), "holdout candidate workload matrix")
    for phase, seeds in (("screen", [42, 43, 44]), ("confirmation", list(range(45, 55)))):
        paths = sorted((OUT / phase).glob("*.result.json"))
        require(len(paths) == 120, f"{phase}: expected 120 accepted records, got {len(paths)}")
        rows = []
        for path in paths:
            row, run_interval = validate_record(path, AUDIT["provenance"]["source_manifest_sha256"], interval)
            rows.append(row)
            intervals.append(run_interval)
        expected = {(kind, degree, threads, seed, profile)
                    for kind, degree, threads in workloads for seed in seeds
                    for profile in (provenance["screen_profiles"] if phase == "screen"
                                    else ["baseline", "original", candidates[kind, degree, threads]])}
        require(len(expected) == 120 and {profile_key(r) for r in rows} == expected,
                f"{phase}: unexpected or missing workload/profile/seed")
        phases[phase] = {"accepted_executions": len(rows),
                         "verified_proofs_including_warmups": sum(r["verified_proofs"] for r in rows),
                         "measured_proofs": len(rows) * 7, "warmup_proofs": len(rows),
                         "minimum_security_bits": min(r["security_bits"] for r in rows),
                         "max_unused_smt_sibling_busy_pct": max(r["max_sibling_busy_pct"] for r in rows)}
        all_rows.extend(rows)
    AUDIT["accepted"] = phases
    AUDIT["known_external_build_overlap"] = {"overlap_count": 0, "build_interval": build,
        "method": "Conservative command/resources file-mtime brackets; assumes original mtimes preserved.",
        "accepted_run_intervals": intervals}
    by_input = collections.defaultdict(list)
    for row in all_rows:
        by_input[row["variant"], row["degree"], row["seed"]].append(row)
    for key, rows in by_input.items():
        require(len({r["input_digest"] for r in rows}) == 1, f"{key}: unmatched profile inputs")
        controls = [r for r in rows if r["profile"] in ("baseline", "original")]
        require(len(controls) == 2 and {r["profile"] for r in controls} == {"baseline", "original"},
                f"{key}: missing control")
        require(len({(r["proof_digest"], r["proof_bytes"]) for r in controls}) == 1,
                f"{key}: original/baseline proof or size differs")
    input_counts = {}
    for kind, degree, threads in workloads:
        rows = [r for r in all_rows if r["variant"] == kind and r["degree"] == degree]
        require({r["seed"] for r in rows} == set(range(42, 55)), f"{kind}/{degree}: seed coverage")
        unique = {r["input_digest"] for r in rows}
        require(len(unique) == 13, f"{kind}/{degree}: expected 13 distinct inputs")
        input_counts[f"{kind}-{degree}"] = len(unique)
    AUDIT["input_and_control_checks"] = {"distinct_inputs_per_workload": input_counts,
        "matched_workload_seed_groups": len(by_input), "control_digest_and_size_pairs_checked": len(by_input)}
    equivalence = []
    for kind, degree, threads in workloads:
        rows = [r for r in all_rows if r["phase"] == "screen"
                and r["variant"] == kind and r["degree"] == degree]
        groups = collections.defaultdict(list)
        for profile in provenance["screen_profiles"]:
            matched = sorted((r for r in rows if r["profile"] == profile), key=lambda r: r["seed"])
            identity = tuple((r["seed"], r["proof_digest"], r["proof_bytes"]) for r in matched)
            groups[identity].append(profile)
        equivalence.append({"variant": kind, "degree": degree, "threads": threads,
            "groups": [{"profiles": profiles,
                        "evidence": [{"seed": seed, "proof_digest": digest, "proof_bytes": size}
                                     for seed, digest, size in identity]}
                       for identity, profiles in groups.items()]})
    save(OUT / "profile-equivalence.json", {
        "definition": "Identical proof Debug digests AND counted payload sizes on all three screening seeds; not a proof of code equivalence.",
        "workloads": equivalence})
    AUDIT["profile_equivalence_artifact"] = "profile-equivalence.json"
    AUDIT["validation"] = validation_counts()
    AUDIT["excluded_proofs"] = {
        "screen_discarded": raw_proof_counts((OUT / "screen/discarded").rglob("*.jsonl")),
        "confirmation_discarded": raw_proof_counts((OUT / "confirmation/discarded").rglob("*.jsonl")),
        "superseded_short_screen_all_attempts": raw_proof_counts((OUT / "screen-short-superseded").rglob("*.jsonl")),
        "note": "Counts raw completed proof records, not unique inputs. Excluded proofs do not contribute to accepted timing statistics."}
    AUDIT["correctness_only_proofs"] = {
        "smoke": raw_proof_counts((OUT / "smoke").glob("*.jsonl")),
        "corpus_preparation": raw_proof_counts((OUT / "corpus-preparation").glob("*.jsonl"))}
    excluded_builds = read(OUT / "external-build-exclusions.json")
    require(len(excluded_builds) == len(set(excluded_builds)) == 19,
            "expected 19 distinct known external-build exclusions")
    AUDIT["external_build_excluded_cases"] = excluded_builds
    for category, counts in AUDIT["excluded_proofs"].items():
        if isinstance(counts, dict) and counts.get("unverified", 0):
            AUDIT["flags"].append(f"{category}: unverified raw trial records require interpretation")
    event_path = OUT / "external-process-events.json"
    if event_path.exists():
        events = read(event_path)
        AUDIT["external_process_events"] = {"artifact_present": True, "contents": events}
        payload = events.get("events", events) if isinstance(events, dict) else events
        if payload:
            AUDIT["flags"].append("External-process events were recorded; root must interpret possible timing impact.")
    else:
        AUDIT["external_process_events"] = {"artifact_present": False}
        AUDIT["flags"].append("External-process event artifact absent; absence does not establish zero observed events.")
    AUDIT["limits"] = [
        "Security checks validate the recorded composed accounting and test evidence, not a new proof of cryptographic soundness.",
        "Affinity checks use benchmark-reported worker pins, not a complete independent /proc affinity history.",
        "External process monitoring polls periodically and cannot establish perfect machine isolation.",
        "Repeated deterministic proofs are counted as executions; 13 seeds per workload are the independent inputs."]
    AUDIT["status"] = "passed_with_flags" if AUDIT["flags"] else "passed"


if __name__ == "__main__":
    try:
        main()
    except Exception as error:
        AUDIT["status"] = "failed"
        AUDIT["failure"] = str(error)
        AUDIT["traceback"] = traceback.format_exc()
        raise
    finally:
        AUDIT["finished_epoch"] = time.time()
        save(OUT / "final-audit.json", AUDIT)
        print(json.dumps({"status": AUDIT["status"], "flags": AUDIT["flags"],
                          "artifact": str(OUT / "final-audit.json")}))

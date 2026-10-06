#!/usr/bin/env python3
"""Fixed-sample qualification for Falcon's one-source commitment change.

The two immutable builds are supplied in a JSON manifest. Qualification uses
exactly seeds 42..101, one warmup and three samples per process, and all sixteen
protocol/security/batch cells at 16 threads. A seed median is one observation.
The estimand is the ratio of arithmetic means, not a geometric mean of ratios.
All timing gates use a paired, one-sided 95% percentile-bootstrap upper bound
of 1.02. Partial campaigns never pass. Diagnostic runs cannot qualify a build.
"""
from __future__ import annotations

import argparse
import itertools
import json
import math
import os
from pathlib import Path
import platform
import random
import re
import signal
import statistics
import subprocess
import sys
import time

import bench_gate
import bench_support as support
from bench_statistics import percentile
from compare_falcon_benchmarks import MATCHED, PAYLOAD, SCHEMA, require
from falcon_v2_campaign import environment, process_order

SEEDS = tuple(range(42, 102))
PROTOCOLS = ("native", "shared-prime")
TARGETS = (100, 128)
BATCHES = (1, 3, 32, 1024)
TIMINGS = ("total_prover_ms", "proof_verify_ms")
METRICS = TIMINGS + ("witness_commit_ms", "proof_prove_ms")
LIMIT = 1.02
HEX64 = re.compile(r"[0-9a-f]{64}")
BUILD_MATCH = ("rustc", "lock_sha256", "rustflags", "features", "profile", "target_arch")


def mean_ratio_interval(baseline, candidate, *, draws=20000):
    """Paired bootstrap of mean(candidate)/mean(baseline), resampling seeds."""
    require(len(baseline) == len(candidate) and len(baseline) >= 2, "unpaired or insufficient observations")
    require(all(math.isfinite(x) and x > 0 for x in baseline + candidate), "invalid timing observation")
    rng = random.Random(0)
    n = len(baseline)
    ratios = []
    for _ in range(draws):
        indices = rng.choices(range(n), k=n)
        ratios.append(sum(candidate[i] for i in indices) / sum(baseline[i] for i in indices))
    return {
        "baseline_mean_ms": statistics.mean(baseline),
        "candidate_mean_ms": statistics.mean(candidate),
        "ratio": sum(candidate) / sum(baseline),
        "lower_one_sided_95": percentile(ratios, .05),
        "upper_one_sided_95": percentile(ratios, .95),
        "bootstrap_draws": draws,
        "pairs": n,
    }


def timing_status(result):
    if result["upper_one_sided_95"] <= LIMIT:
        return "pass"
    if result["lower_one_sided_95"] > LIMIT:
        return "regression"
    return "inconclusive"


def read_rows(path):
    return [json.loads(line) for line in Path(path).read_text().splitlines() if line.strip()]


def validate_rows(rows, build, protocol, security, batch, threads, seed, warmup, measured, stages):
    require(len(rows) == warmup + measured + 2, "incomplete process")
    require(all(row.get("schema") == SCHEMA for row in rows), "unexpected benchmark schema")
    header, summary = rows[0], rows[-1]
    require(header.get("event") == "prepared" and summary.get("event") == "summary", "missing process boundary")
    require(header.get("protocol") == build["protocols"][protocol], "unexpected protocol version")
    expected = dict(security_target=security, batch=batch, capacity=1 << (batch - 1).bit_length(),
                    threads=threads, input_seed=seed, input_count=batch,
                    warmup_trials=warmup, measured_trials=measured, stage_timings=stages)
    require(all(header.get(k) == v for k, v in expected.items()), "incorrect workload metadata")
    require(all(k in header for k in MATCHED), "missing matched metadata")
    require(header["build_rustflags"] == header["runtime_rustflags"] == build["rustflags"], "build flag mismatch")
    require(header["target_arch"] == build["target_arch"], "target architecture mismatch")
    small = protocol == "shared-prime" and security == 100
    require(header["integer_bridge"] == ("wfbitz-unsplit" if small else "wfbitz-joint-limbs"), "wrong bridge")
    prime_min = 1 << (114 if small else 125)
    prime_max = (1 << 115) - (1 << 102) - 1 if small else (1 << 126) - 1
    require(header["arithmetic_prime_min"] == str(prime_min) and header["arithmetic_prime_max"] == str(prime_max), "wrong prime interval")
    require(header["arithmetic_prime_bits"] == (115 if small else 126), "wrong prime width")
    terms = header.get("security_terms", [])
    require(terms and len({term["name"] for term in terms}) == len(terms), "missing/duplicate security terms")
    require(all(math.isfinite(term["error_bound"]) and term["error_bound"] >= 0 for term in terms), "invalid security ledger")
    error = sum(term["error_bound"] for term in terms)
    require(0 < error <= 2.0 ** -security, "security target not met")
    require(math.isclose(-math.log2(error), header["algebraic_security_bits"], rel_tol=1e-12), "security total mismatch")
    require(HEX64.fullmatch(header["input_digest"]) is not None, "invalid input digest")
    trials = rows[1:-1]
    require([r.get("trial") for r in trials] == ["warmup"] * warmup + ["sample"] * measured, "wrong trial order")
    require(all(r.get("event") == "trial" and r.get("verified") is True for r in trials), "unverified trial")
    require(summary.get("verified") is True and summary.get("samples") == measured, "unverified summary")
    require(len({r.get("proof_debug_digest") for r in trials}) == 1, "nondeterministic repeated proof")
    require(len({r.get("proof_payload_bytes") for r in trials}) == 1, "nondeterministic repeated payload")
    require([r.get("sample") for r in trials[warmup:]] == list(range(1, measured + 1)), "sample numbering")
    require(all(r.get("sample") is None for r in trials[:warmup]), "numbered warmup")
    for row in trials:
        require(all(row.get(k) == header[k] for k in ("batch", "capacity", "security_target", "threads", "input_digest")), "trial metadata drift")
        require(HEX64.fullmatch(row.get("proof_debug_digest", "")) is not None, "invalid proof digest")
        require(all(math.isfinite(row[k]) and row[k] > 0 for k in METRICS), "invalid trial timing")
        require(math.isclose(row["total_prover_ms"], row["witness_commit_ms"] + row["proof_prove_ms"], rel_tol=1e-12), "incorrect total prover time")
        require(row.get("proof_payload_definition") == PAYLOAD and row["proof_payload_bytes"] > 0, "invalid payload accounting")
        roots = row.get("roots", [row["source_root"]] if "source_root" in row else [])
        require(len(roots) == build["root_count"] and all(HEX64.fullmatch(x) for x in roots), "wrong root count/encoding")
        require(row.get("process_peak_rss_kib", 0) > 0, "missing peak RSS")
    samples = trials[warmup:]
    for metric in METRICS:
        require(summary.get("median_" + metric) == statistics.median(r[metric] for r in samples), "incorrect benchmark summary")
    return dict(header=header, samples=samples, summary=summary,
                medians={metric: statistics.median(r[metric] for r in samples) for metric in METRICS},
                payload=trials[0]["proof_payload_bytes"],
                peak_rss_kib=max(r["process_peak_rss_kib"] for r in trials))


def validate_pair(baseline, candidate):
    require(all(baseline["header"][k] == candidate["header"][k] for k in MATCHED), "unmatched workload/build settings")
    # Roots and Fiat-Shamir proof digests MUST be allowed to change.
    require(baseline["header"]["arithmetic_prime_min"] == candidate["header"]["arithmetic_prime_min"]
            and baseline["header"]["arithmetic_prime_max"] == candidate["header"]["arithmetic_prime_max"], "prime profile changed")


def verify_build(build):
    require(Path(build["binary"]).is_file(), "missing immutable binary")
    require(support.file_hash(build["binary"]) == build["sha256"], "binary changed")
    manifest = Path(build["source_manifest"])
    require(support.file_hash(manifest) == build["source_manifest_sha256"], "source manifest changed")
    source = json.loads(manifest.read_text())
    root = Path(build["source_root"])
    for name, digest in source["files"].items():
        require(support.file_hash(root / name) == digest, "source changed: " + name)
    require(support.file_hash(root / "Cargo.lock") == build["lock_sha256"], "Cargo.lock changed")


def verify_final_integrity(args, builds, manifest, immutable):
    for build in builds.values():
        verify_build(build)
    manifest["final_build_hashes"] = {
        label: dict(binary_sha256=support.file_hash(build["binary"]),
                    source_manifest_sha256=support.file_hash(build["source_manifest"]))
        for label, build in builds.items()
    }
    if args.cache_reference:
        require(support.file_hash(args.cache_reference) == immutable["cache_reference_sha256"], "cache reference changed")
        for key, reference in args.cache_references.items():
            batch, seed = key.split(":")
            path = args.case_cache / f"fn-dsa-0.3.0-b{batch}-seed{seed}.bin"
            require(support.file_hash(path) == reference["sha256"], "cached fixture changed during campaign")
        manifest["cache_reference_final_sha256"] = support.file_hash(args.cache_reference)
        manifest["final_cache_files_verified"] = len(args.cache_references)


def run_child(command, env, stdout, stderr, timeout, swap_before, max_swap_growth):
    started = time.monotonic()
    process = subprocess.Popen(command, env=env, stdout=stdout, stderr=stderr, start_new_session=True)
    try:
        while process.poll() is None:
            require(time.monotonic() - started < timeout, "benchmark child timed out")
            require(bench_gate.swap_used_gb() - swap_before <= max_swap_growth, "swap increased during benchmark")
            try:
                process.wait(timeout=1)
            except subprocess.TimeoutExpired:
                pass
        require(process.returncode == 0, f"benchmark exited {process.returncode}")
    finally:
        if process.poll() is None:
            bench_gate.stop_process_group(process)


def stem_for(protocol, security, batch, threads, seed, label):
    return f"{protocol}-s{security}-b{batch}-t{threads}-seed{seed}-{label}"


def run_one(args, builds, label, case, seed, index, dest):
    protocol, security, batch = case
    build = builds[label]
    cache_reference = None
    if args.case_cache:
        cache_reference = args.cache_references[f"{batch}:{seed}"]
        cache = args.case_cache / f"fn-dsa-0.3.0-b{batch}-seed{seed}.bin"
        require(support.file_hash(cache) == cache_reference["sha256"], "cached fixture changed")
    warmup, measured = (1, 3) if args.phase in ("qualification", "diagnostic") else (0, 1)
    stages = args.phase == "stages"
    stem = stem_for(*case, args.threads, seed, label)
    record_path = dest / (stem + ".run.json")
    output, errors = dest / (stem + ".jsonl"), dest / (stem + ".stderr")
    if record_path.exists():
        require(args.resume, "existing record without --resume: " + stem)
        record = json.loads(record_path.read_text())
        require(record["status"] == "validated", "incomplete/failed record requires a new campaign: " + stem)
        require(record["binary_sha256"] == build["sha256"], "resumed record from different binary")
        require(record["stdout_sha256"] == support.file_hash(output) and record["stderr_sha256"] == support.file_hash(errors), "resumed evidence changed")
        result = validate_rows(read_rows(output), build, *case, args.threads, seed, warmup, measured, stages)
        if cache_reference:
            require(result["header"]["input_digest"] == cache_reference["input_digest"], "resumed cached input differs from reference")
        return result
    require(not output.exists() and not errors.exists(), "unrecorded artifacts exist: " + stem)
    require(support.file_hash(build["binary"]) == build["sha256"], "binary changed before process")
    bench_gate.wait_idle(stem, args.min_idle, 1, 1, args.max_idle_wait)
    command = [build["binary"], "--protocol", protocol, "--security", str(security), "--batch", str(batch),
               "--threads", str(args.threads), "--seed", str(seed), "--warmup", str(warmup), "--iterations", str(measured)]
    env = environment(args.threads)
    if args.case_cache:
        env["BITZ_FALCON_CASE_CACHE"] = str(args.case_cache.resolve())
    if stages:
        env.update(BITZ_FALCON_STAGE_TIMINGS="1", FLOCK_COMMIT_TIMING="1")
    record = dict(status="running", command=command, label=label, pair_index=index,
                  order=process_order(("baseline", "candidate"), index),
                  binary_sha256=build["sha256"], started_at=time.time(),
                  swap_before_gib=bench_gate.swap_used_gb())
    support.write_json(record_path, record)
    print("START " + args.phase + " " + stem, flush=True)
    started = time.monotonic()
    try:
        with output.open("w") as out, errors.open("w") as err:
            run_child(command, env, out, err, args.process_timeout, record["swap_before_gib"], args.max_swap_growth)
        result = validate_rows(read_rows(output), build, *case, args.threads, seed, warmup, measured, stages)
        if cache_reference:
            require(result["header"]["input_digest"] == cache_reference["input_digest"], "cached input differs from generated reference")
        record.update(status="validated", stdout_sha256=support.file_hash(output), stderr_sha256=support.file_hash(errors))
        print("DONE " + stem + " " + json.dumps(result["medians"]), flush=True)
        return result
    except BaseException as error:
        record.update(status="failed", error=repr(error))
        raise
    finally:
        record.update(elapsed_seconds=time.monotonic() - started, swap_after_gib=bench_gate.swap_used_gb())
        support.write_json(record_path, record)


def summarize(pairs, qualification, complete):
    cells = []
    verified_proofs = 0
    measured_counts = set()
    for case, observations in sorted(pairs.items()):
        protocol, security, batch = case
        seeds = sorted(observations)
        results = {metric: mean_ratio_interval(
            [observations[s]["baseline"]["medians"][metric] for s in seeds],
            [observations[s]["candidate"]["medians"][metric] for s in seeds]) for metric in METRICS} if len(seeds) >= 2 else {}
        payloads = [observations[s]["candidate"]["payload"] / observations[s]["baseline"]["payload"] for s in seeds]
        baseline_mean_payload = statistics.mean(observations[s]["baseline"]["payload"] for s in seeds)
        candidate_mean_payload = statistics.mean(observations[s]["candidate"]["payload"] for s in seeds)
        payload_ratio = candidate_mean_payload / baseline_mean_payload
        payload_limit = .8 if protocol == "shared-prime" and batch == 1024 else 1.0
        payload_pass = payload_ratio <= .8 if payload_limit == .8 else payload_ratio < 1.0
        verified_proofs += sum(result["header"]["warmup_trials"] + len(result["samples"])
                               for observation in observations.values() for result in observation.values())
        measured_counts.update(len(result["samples"])
                               for observation in observations.values() for result in observation.values())
        # A finished cell can report its gate result while the overall matrix
        # remains incomplete. Only the complete all-cells check below can pass
        # the campaign.
        qualified = qualification and tuple(seeds) == SEEDS
        for metric, value in results.items():
            value["status"] = timing_status(value) if qualified and metric in TIMINGS else "diagnostic"
        cells.append(dict(protocol=protocol, security=security, batch=batch, seeds=seeds, timings=results,
                          peak_rss_kib={label: max(observations[s][label]["peak_rss_kib"] for s in seeds)
                                        for label in ("baseline", "candidate")},
                          payload=dict(maximum_ratio=max(payloads), ratio_of_mean_bytes=payload_ratio,
                                       baseline_mean_bytes=baseline_mean_payload,
                                       candidate_mean_bytes=candidate_mean_payload,
                                       limit=payload_limit, strict=payload_limit == 1.0, pass_gate=payload_pass)))
    all_cells = len(cells) == len(PROTOCOLS) * len(TARGETS) * len(BATCHES)
    passes = qualification and complete and all_cells and all(
        tuple(c["seeds"]) == SEEDS and c["payload"]["pass_gate"]
        and all(c["timings"][m]["status"] == "pass" for m in TIMINGS) for c in cells)
    return dict(status="pass" if passes else ("not_passed" if qualification and complete else "diagnostic"),
                complete=complete, qualifying=qualification, timing_limit=LIMIT,
                verified_proofs_in_complete_pairs=verified_proofs,
                measured_trials_per_seed=sorted(measured_counts),
                estimator="ratio of arithmetic means of per-seed process medians",
                confidence="paired percentile bootstrap, one-sided 95%, fixed 60 seeds", cells=cells)


def write_report(path, summary):
    counts = summary["measured_trials_per_seed"]
    observation = ("Each timing observation is the median of three measured proofs for one seed. "
                   if counts == [3] else "These diagnostics use one measured proof per process. ")
    lines = ["# Falcon one-source qualification", "", f"Status: **{summary['status']}**.", "",
             observation +
             "Ratios compare arithmetic means across seeds. The upper confidence bound is a paired "
             "one-sided 95% bootstrap bound. Qualification requires all 60 preselected seeds and "
             "both timing bounds at or below 1.02 in every case.", "",
             "| Protocol | Bits | Batch | Seeds | Prover ratio (upper 95%) | Verifier ratio (upper 95%) | Mean payload ratio |",
             "|---|---:|---:|---:|---:|---:|---:|"]
    for cell in summary["cells"]:
        def timing(metric):
            v = cell["timings"].get(metric)
            return "—" if not v else f"{v['ratio']:.4f} ({v['upper_one_sided_95']:.4f})"
        lines.append(f"| {cell['protocol']} | {cell['security']} | {cell['batch']} | {len(cell['seeds'])} | "
                     f"{timing('total_prover_ms')} | {timing('proof_verify_ms')} | {cell['payload']['ratio_of_mean_bytes']:.4f} |")
    lines += ["", f"These completed pairs contain {summary['verified_proofs_in_complete_pairs']:,} verified proofs including warmups. "
              "A complete qualification requires 1,920 separate processes and 7,680 proofs. "
              "Roots and proof digests are expected to differ between revisions; input digests, public statements, "
              "security targets, geometry, compiler settings, and arithmetic profiles must match. "
              "A diagnostic or incomplete campaign cannot satisfy the gate.", ""]
    lines += ["Peak RSS below is the maximum process-lifetime high-water mark observed in each cell; "
              "it is a diagnostic, not a per-proof allocation measurement or an acceptance gate.", "",
              "| Protocol | Bits | Batch | Baseline peak MiB | Candidate peak MiB |",
              "|---|---:|---:|---:|---:|"]
    for cell in summary["cells"]:
        rss = cell["peak_rss_kib"]
        lines.append(f"| {cell['protocol']} | {cell['security']} | {cell['batch']} | "
                     f"{rss['baseline'] / 1024:.2f} | {rss['candidate'] / 1024:.2f} |")
    lines.append("")
    if summary.get("stop_reason"):
        lines += [summary["stop_reason"], ""]
    path.write_text("\n".join(lines))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--builds", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--phase", choices=("qualification", "diagnostic", "cold", "stages", "smoke"), default="qualification")
    parser.add_argument("--threads", type=int, default=16)
    parser.add_argument("--case-cache", type=Path, help="shared deterministic public input cache, outside timed regions")
    parser.add_argument("--cache-reference", type=Path, help="prewarm manifest with independently generated input digests")
    parser.add_argument("--resume", action="store_true")
    parser.add_argument("--min-idle", type=float, default=88)
    parser.add_argument("--idle-hold", type=float, default=20)
    parser.add_argument("--max-idle-wait", type=float, default=900)
    parser.add_argument("--process-timeout", type=float, default=300)
    parser.add_argument("--max-swap-growth", type=float, default=.1)
    args = parser.parse_args()
    require(args.phase != "qualification" or args.threads == 16, "qualification is fixed at 16 threads")
    require(args.threads > 0, "invalid thread count")
    require(bool(args.case_cache) == bool(args.cache_reference), "cache and reference must be supplied together")
    if args.cache_reference:
        reference = json.loads(args.cache_reference.read_text())
        require(reference.get("schema") == "bitz/falcon-case-cache-reference/v1" and reference.get("complete") is True,
                "incomplete input cache reference")
        args.cache_references = reference["cases"]
    builds_manifest = json.loads(args.builds.read_text())
    builds = builds_manifest["binaries"]
    require(set(builds) == {"baseline", "candidate"}, "expected baseline and candidate")
    for build in builds.values():
        verify_build(build)
    require(all(builds["baseline"][k] == builds["candidate"][k] for k in BUILD_MATCH), "unmatched builds")
    require(builds["baseline"]["revision"] == "51405193878e15e1e4259216464f2bb856c8b592", "wrong baseline revision")
    require(builds["baseline"]["root_count"] == 3 and builds["candidate"]["root_count"] == 1, "incorrect commitment counts")
    seeds = SEEDS if args.phase == "qualification" else ((42, 43) if args.phase == "diagnostic" else (42,))
    args.output.mkdir(parents=True, exist_ok=True)
    dest = args.output / args.phase
    dest.mkdir(exist_ok=True)
    stop_path = dest / "STOP"
    require(not stop_path.exists(), "campaign STOP marker already exists")
    manifest_path = dest / "manifest.json"
    immutable = dict(builds=builds_manifest, phase=args.phase, seeds=seeds, protocols=PROTOCOLS,
                     security=TARGETS, batches=BATCHES, threads=args.threads,
                     host=platform.node(), architecture=platform.machine(), cpu_model=support.cpu_name(),
                     affinity=sorted(os.sched_getaffinity(0)) if hasattr(os, "sched_getaffinity") else None,
                     harness_sha256=support.file_hash(__file__), timing_limit=LIMIT,
                     case_cache=str(args.case_cache.resolve()) if args.case_cache else None,
                     cache_reference_sha256=support.file_hash(args.cache_reference) if args.cache_reference else None,
                     estimator="ratio of arithmetic means of per-seed medians", bootstrap_draws=20000)
    if manifest_path.exists():
        require(args.resume, "campaign already exists")
        saved = json.loads(manifest_path.read_text())
        require(saved["configuration"] == json.loads(json.dumps(immutable)), "resumed campaign configuration changed")
    manifest = dict(configuration=immutable, status="waiting", started_at=time.time())
    support.write_json(manifest_path, manifest)
    pairs = {}
    signal.signal(signal.SIGTERM, bench_gate.terminate)
    acquired = False
    try:
        bench_gate.acquire("falcon-one-source-" + args.phase, 10)
        acquired = True
        bench_gate.wait_idle("falcon-one-source-" + args.phase, args.min_idle, args.idle_hold, 5, args.max_idle_wait)
        manifest["status"] = "running"
        support.write_json(manifest_path, manifest)
        index = 0
        # Each cell has its own balanced AB/BA/BA/AB schedule across seeds.
        for protocol, security, batch in itertools.product(PROTOCOLS, TARGETS, BATCHES):
            case = protocol, security, batch
            pairs[case] = {}
            for seed_index, seed in enumerate(seeds):
                pair = {}
                for label in process_order(("baseline", "candidate"), seed_index):
                    pair[label] = run_one(args, builds, label, case, seed, seed_index, dest)
                validate_pair(pair["baseline"], pair["candidate"])
                pairs[case][seed] = pair
                index += 1
                manifest.update(completed_pairs=index, active_case=list(case), active_seed=seed)
                support.write_json(manifest_path, manifest)
                print("MATCHED " + str((case, seed)), flush=True)
                # Cooperative shutdown works across the exec tool's PID
                # namespaces and never truncates an in-flight matched pair.
                if stop_path.exists():
                    verify_final_integrity(args, builds, manifest, immutable)
                    summary = summarize(pairs, args.phase == "qualification", False)
                    summary.update(status="not_qualified", stop_reason=stop_path.read_text().strip()
                                   or "Stopped by operator at a completed pair boundary.")
                    support.write_json(dest / "summary.json", summary)
                    write_report(dest / "REPORT.md", summary)
                    manifest.update(status="stopped_at_pair_boundary", qualification_status="not_qualified",
                                    complete=False, stop_reason=summary["stop_reason"], finished_at=time.time())
                    print("CAMPAIGN_RESULT not_qualified (stopped)", flush=True)
                    return 2
            summary = summarize(pairs, args.phase == "qualification", False)
            support.write_json(dest / "summary.json", summary)
            write_report(dest / "REPORT.md", summary)
        verify_final_integrity(args, builds, manifest, immutable)
        summary = summarize(pairs, args.phase == "qualification", True)
        support.write_json(dest / "summary.json", summary)
        write_report(dest / "REPORT.md", summary)
        manifest.update(status="completed", qualification_status=summary["status"], finished_at=time.time())
        print("CAMPAIGN_RESULT " + summary["status"], flush=True)
        return 0 if args.phase != "qualification" or summary["status"] == "pass" else 2
    except BaseException as error:
        manifest.update(status="failed", error=repr(error), finished_at=time.time())
        raise
    finally:
        support.write_json(manifest_path, manifest)
        if acquired:
            bench_gate.release()


if __name__ == "__main__":
    sys.exit(main())

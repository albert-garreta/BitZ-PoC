#!/usr/bin/env python3
"""Run balanced paired Falcon comparisons against a retained baseline binary.

Example, inside the clean candidate checkout on the benchmark host:
  python3 scripts/run_falcon_paired.py --revision FULL_CANDIDATE_SHA \
    --baseline-manifest .tmp/falcon-baseline-449-TIMESTAMP/manifest.json \
    --output .tmp/falcon-paired-TIMESTAMP

Default: 16 cells, seeds 42..46, six pairs per seed; every two pairs execute
AB then BA (ABBA). A is the retained native baseline, B the shared-prime build.
Each side has two warmups, three measurements, and a fresh RSS process.
--batches/--threads/--security/--seeds/--pairs-per-seed permit smaller diagnostic
runs. The existing comparator cannot accept an incomplete campaign. No timing
or payload tolerance is introduced. This script does not fetch, switch HEAD,
push, modify the baseline, or run any instrumented proof timings.
"""
from __future__ import annotations

import argparse
import json
import math
import os
from pathlib import Path
import platform
import re
import signal
import sys
import time
import uuid

import bench_gate
import bench_support as support
from compare_falcon_benchmarks import MATCHED, PROVENANCE, compare, read_campaign, read_process, require
from run_falcon_campaign import (BATCHES, PROTOCOLS, RUSTFLAGS, SECURITY, THREADS,
                                 campaign_environment, compiler_version, execute,
                                 require_ignored, verify_revision)

BASELINE_REVISION = "4491309f5d1e45d506410803e3b9ba488047991f"
SEEDS = list(range(42, 47))


def schedule(args):
    """Adjacent AB and BA pairs are one balanced block for the same input."""
    for block in range(args.pairs_per_seed // 2):
        for seed in args.seeds:
            for security in args.security:
                for threads in args.threads:
                    for batch in args.batches:
                        for order in ("AB", "BA"):
                            pair = 2 * block + (order == "BA")
                            label = f"s{security}-b{batch}-t{threads}-seed{seed}-pair{pair}"
                            yield dict(label=label, pair_id=label, order=order, block=block,
                                       security=security, batch=batch, threads=threads, seed=seed)


def command(binary, protocol, case, memory=False):
    result = [str(binary), "--security", str(case["security"]), "--batch", str(case["batch"]),
              "--threads", str(case["threads"]), "--seed", str(case["seed"]),
              "--warmup", "0" if memory else "2", "--iterations", "1" if memory else "3"]
    if protocol != "native":
        result += ["--protocol", protocol]
    return result


def validate(process, protocol, case):
    header = process["prepared"]
    require(header.get("protocol") == PROTOCOLS[protocol], "wrong protocol selected")
    expected = {"security_target": case["security"], "batch": case["batch"],
                "capacity": 1 << (case["batch"] - 1).bit_length(), "threads": case["threads"],
                "input_seed": case["seed"], "input_count": case["batch"],
                "build_rustflags": RUSTFLAGS, "runtime_rustflags": RUSTFLAGS}
    require(all(header.get(key) == value for key, value in expected.items()), "wrong paired workload/build flags")


def retained_baseline(path, current, binary_override=None):
    manifest, _ = read_campaign(path.parent)
    require(path.name == "manifest.json", "baseline manifest must be named manifest.json")
    require(manifest["revision"] == BASELINE_REVISION, "baseline is not the exact 4491309f revision")
    require(all(manifest[key] == current[key] for key in PROVENANCE), "baseline build/host provenance differs")
    binary = binary_override or Path(manifest["binary"])
    require(binary.is_absolute() and binary.is_file(), "retained baseline binary is unavailable")
    require(os.access(binary, os.X_OK), "retained baseline binary is not executable")
    require(support.file_hash(binary) == manifest["binary_sha256"], "retained baseline binary hash changed")
    return manifest, binary.resolve()


def verify_binaries(manifests):
    for manifest in manifests.values():
        require(support.file_hash(manifest["binary"]) == manifest["binary_sha256"], "benchmark binary changed during campaign")


def run(args):
    root = args.repo.resolve()
    verify_revision(root, args.revision)
    output = (root / args.output).resolve()
    require(not output.exists(), f"output directory already exists: {output}")
    target = (root / (args.target_dir or Path("target") / ("falcon-paired-" + args.revision[:12]))).resolve()
    require_ignored(root, output)
    require_ignored(root, target)
    baseline_path = (root / args.baseline_manifest).resolve()
    env = campaign_environment(target)
    current = dict(host=platform.node(), architecture=platform.machine(), platform=platform.platform(),
                   rustc=compiler_version(root), lock_sha256=support.file_hash(root / "Cargo.lock"),
                   cpu_model=support.cpu_name(), affinity=sorted(os.sched_getaffinity(0)) if hasattr(os, "sched_getaffinity") else None,
                   rustflags=RUSTFLAGS, features=["falcon-hybrid"], profile="release")
    require(bool(current["rustc"].strip()), "rustc provenance unavailable")
    baseline_override = (root / args.baseline_binary).resolve() if args.baseline_binary else None
    baseline, baseline_binary = retained_baseline(baseline_path, current, baseline_override)
    # Rebuilding into the retained binary's directory would destroy provenance.
    require(not baseline_binary.is_relative_to(target), "candidate target directory contains the retained baseline binary")
    build = ["cargo", "build", "--locked", "--features", "falcon-hybrid", "--bench", "falcon_hybrid",
             "--profile", "release", "--message-format=json-render-diagnostics"]
    pairing_id = str(uuid.uuid4())
    shared = dict(schema="bitz/falcon-campaign/v1", kind="paired", pairing_id=pairing_id,
                  **current, batches=args.batches, threads=args.threads, security=args.security,
                  seeds=args.seeds, pairs_per_seed=args.pairs_per_seed, warmup=2, iterations=3,
                  status="waiting", started_at=time.time(), execution_order="AB then BA per input block; timing and RSS per side",
                  gate=dict(lock=str(bench_gate.LOCK), min_idle=args.min_idle, hold_seconds=args.hold_seconds,
                            poll_seconds=args.poll_seconds, max_idle_wait_seconds=args.max_idle_wait_seconds,
                            continuous_swap_guard=False),
                  build_timeout_seconds=args.build_timeout, process_timeout_seconds=args.process_timeout)
    manifests = {
        "A": dict(shared, revision=baseline["revision"], protocol_selection="native", cases=[],
                  binary=str(baseline_binary), binary_sha256=baseline["binary_sha256"],
                  original_manifest=str(baseline_path), original_manifest_sha256=support.file_hash(baseline_path),
                  build_command=baseline.get("build_command"), build_environment=baseline.get("build_environment")),
        "B": dict(shared, revision=args.revision, protocol_selection=args.protocol, cases=[],
                  build_command=build, build_environment=support.environment(env)),
    }
    locations = {"A": output / "baseline", "B": output / "candidate"}
    output.mkdir(parents=True, exist_ok=False)
    for path in locations.values():
        path.mkdir()
    support.write_json(output / "baseline-original-manifest.json", baseline)
    support.write_json(output / "schedule.json", list(schedule(args)))
    def save():
        for role, manifest in manifests.items():
            support.write_json(locations[role] / "manifest.json", manifest)
    def status(value):
        for manifest in manifests.values():
            manifest["status"] = value
        save()
    save()
    print("FALCON_PAIRED_DIR=" + str(output), flush=True)
    acquired = False
    try:
        bench_gate.acquire("falcon-paired-" + args.revision[:12], args.poll_seconds)
        acquired = True
        verify_revision(root, args.revision)
        status("building")
        execute(build, root=root, env=env, output=output / "build.log", timeout=args.build_timeout)
        binary = Path(support.cargo_executables((output / "build.log").read_text(), ["falcon_hybrid"])["falcon_hybrid"])
        require(binary.is_absolute() and binary.is_file(), "Cargo benchmark executable is missing")
        require(binary != baseline_binary, "candidate executable aliases the baseline")
        manifests["B"].update(binary=str(binary), binary_sha256=support.file_hash(binary))
        verify_revision(root, args.revision)
        verify_binaries(manifests)
        status("waiting-for-idle")
        bench_gate.wait_idle("falcon-paired", args.min_idle, args.hold_seconds, args.poll_seconds, args.max_idle_wait_seconds)
        status("measuring")
        with (output / "execution.jsonl").open("w", buffering=1) as events:
            for case in schedule(args):
                # Check immutable source/binaries at every pair, outside timed children.
                verify_revision(root, args.revision)
                verify_binaries(manifests)
                processes = {}
                for role in case["order"]:
                    manifest = manifests[role]
                    protocol = manifest["protocol_selection"]
                    directory = locations[role]
                    trial_env = dict(env, RAYON_NUM_THREADS=str(case["threads"]))
                    for item in manifests.values():
                        item["active_case"] = dict(case, role=role)
                    save()
                    print(f"FALCON_PAIR_START={case['label']}:{role}", flush=True)
                    raw = {}
                    elapsed = {}
                    commands = {}
                    for memory in (False, True):
                        phase = "memory" if memory else "timing"
                        suffix = ".memory" if memory else ""
                        filename = case["label"] + suffix + ".jsonl"
                        commands[phase] = command(manifest["binary"], protocol, case, memory)
                        events.write(json.dumps(dict(case, role=role, phase=phase, event="start", at=time.time())) + "\n")
                        elapsed[phase] = execute(commands[phase], root=root, env=trial_env, output=directory / filename,
                                                 errors=directory / (case["label"] + suffix + ".stderr.log"), timeout=args.process_timeout)
                        raw[phase] = read_process(directory / filename, 0 if memory else 2, 1 if memory else 3)
                        validate(raw[phase], protocol, case)
                        events.write(json.dumps(dict(case, role=role, phase=phase, event="finish", at=time.time(), elapsed_seconds=elapsed[phase])) + "\n")
                    timing, memory = raw["timing"], raw["memory"]
                    same = [key for key in MATCHED if key not in ("warmup_trials", "measured_trials")]
                    require(all(timing["prepared"].get(key) == memory["prepared"].get(key) for key in same), "memory/timing workload mismatch")
                    require(timing["samples"][0]["proof_debug_digest"] == memory["samples"][0]["proof_debug_digest"], "proof changed between processes")
                    manifest["cases"].append(dict(label=case["label"], pair_id=case["pair_id"], order=case["order"],
                                                   command=commands["timing"], memory_command=commands["memory"], exit_code=0,
                                                   elapsed_seconds=elapsed["timing"], memory_elapsed_seconds=elapsed["memory"],
                                                   prepared=timing["prepared"], summary=timing["summary"],
                                                   fresh_process_peak_rss_kib=memory["samples"][0]["process_peak_rss_kib"]))
                    processes[role] = timing
                    save()
                require(all(processes["A"]["prepared"].get(key) == processes["B"]["prepared"].get(key) for key in MATCHED), "paired workload mismatch")
                # Do not compare cross-revision Debug digests: the protocol changes.
                for manifest in manifests.values():
                    manifest.pop("active_case", None)
                save()
                print("FALCON_PAIR_FINISHED=" + case["label"], flush=True)
        verify_revision(root, args.revision)
        verify_binaries(manifests)
        for manifest in manifests.values():
            manifest.update(status="complete", finished_at=time.time())
        save()
        report = compare(read_campaign(locations["A"]), read_campaign(locations["B"]))
        support.write_json(output / "comparison.json", report)
    except BaseException as error:
        interrupted = isinstance(error, (KeyboardInterrupt, InterruptedError, SystemExit))
        for manifest in manifests.values():
            manifest.update(status="interrupted" if interrupted else "failed", error=f"{type(error).__name__}: {error}", finished_at=time.time())
        save()
        raise
    finally:
        if acquired:
            bench_gate.release()
    print("FALCON_PAIRED_COMPLETE=" + str(output), flush=True)
    print("FALCON_COMPARISON_STATUS=" + report["status"], flush=True)
    return output, report


def parse_args(argv=None):
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--repo", type=Path, default=Path(__file__).resolve().parents[1])
    parser.add_argument("--revision", required=True)
    parser.add_argument("--baseline-manifest", type=Path, required=True)
    parser.add_argument("--baseline-binary", type=Path, help="optional retained copy; must match the original manifest hash")
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--protocol", choices=PROTOCOLS, default="shared-prime")
    parser.add_argument("--target-dir", type=Path)
    for flag, default in (("batches", BATCHES), ("threads", THREADS), ("security", SECURITY), ("seeds", SEEDS)):
        parser.add_argument("--" + flag, type=int, nargs="+", choices=default, default=default)
    parser.add_argument("--pairs-per-seed", type=int, choices=(2, 4, 6), default=6)
    parser.add_argument("--min-idle", type=float, default=88)
    parser.add_argument("--hold-seconds", type=float, default=30)
    parser.add_argument("--poll-seconds", type=float, default=5)
    parser.add_argument("--max-idle-wait-seconds", type=float, default=300)
    parser.add_argument("--build-timeout", type=float, default=1800)
    parser.add_argument("--process-timeout", type=float, default=600)
    args = parser.parse_args(argv)
    if not re.fullmatch(r"(?:[0-9a-f]{40}|[0-9a-f]{64})", args.revision):
        parser.error("revision must be a full commit SHA")
    if any(len(set(getattr(args, field))) != len(getattr(args, field)) for field in ("batches", "threads", "security", "seeds")):
        parser.error("workload values must not repeat")
    if not 0 <= args.min_idle <= 100 or any(not math.isfinite(getattr(args, field)) or getattr(args, field) <= 0 for field in
            ("hold_seconds", "poll_seconds", "max_idle_wait_seconds", "build_timeout", "process_timeout")):
        parser.error("idle percentage must be 0..100 and time bounds must be positive")
    return args


def main(argv=None):
    def interrupted(signum, _frame):
        raise InterruptedError(f"received signal {signum}")
    signal.signal(signal.SIGTERM, interrupted)
    try:
        _, report = run(parse_args(argv))
    except (Exception, KeyboardInterrupt) as error:
        print(f"Paired Falcon campaign failed: {type(error).__name__}: {error}", file=sys.stderr)
        return 130 if isinstance(error, (KeyboardInterrupt, InterruptedError)) else 1
    # A diagnostic's inconclusive comparison is reported explicitly, not a
    # workload failure. Keep the comparator's exit-code convention.
    return {"pass": 0, "regression": 1, "inconclusive": 2}[report["status"]]


if __name__ == "__main__":
    sys.exit(main())

#!/usr/bin/env python3
"""Run the initial Falcon diagnostic matrix at one exact clean revision.

Example (from the checkout to measure):
  python3 scripts/run_falcon_campaign.py --revision FULL_SHA \
    --protocol shared-prime --output .tmp/falcon-shared-candidate

Builds the release falcon_hybrid benchmark with --locked and native CPU flags.
Runs security 100/128 x threads 1/16 x batches 1/3/32/1024, seed 42, one warmup
and five samples, then one separate fresh-process RSS proof per case. This is
an initial diagnostic campaign, not the 30-pair acceptance campaign.

The existing bench_gate lock is held through build and measurement, with an
idle wait after building. Every child has a timeout and is reaped on failure
or interruption. Failed campaigns and raw logs are retained. Existing output
directories are never overwritten. Run through will-run for remote revision
pinning and checkout protection; this script does not fetch, commit or push.
"""
from __future__ import annotations

import argparse
import json
import math
import os
from pathlib import Path
import platform
import re
import shlex
import signal
import subprocess
import sys
import time

import bench_gate
import bench_support as support
from compare_falcon_benchmarks import MATCHED, read_process, require

BATCHES = [1, 3, 32, 1024]
THREADS = [1, 16]
SECURITY = [100, 128]
RUSTFLAGS = "-C target-cpu=native"
PROTOCOLS = {
    "native": "bitz/falcon1024-ct/hybrid/native-ring/non-zk/v4",
    "shared-prime": "bitz/falcon1024-ct/hybrid/shared-prime/non-zk/v2",
}


def verify_revision(root, revision):
    require(re.fullmatch(r"(?:[0-9a-f]{40}|[0-9a-f]{64})", revision), "revision must be a full commit SHA")
    source = support.root_metadata(root)
    require(source["revision"] == revision, "HEAD differs from the requested revision")
    require(not source["git_dirty"], "checkout is dirty; no changes were discarded")


def require_ignored(root, path):
    if path.is_relative_to(root):
        try:
            support.git(root, "check-ignore", "-q", "--", str(path))
        except subprocess.CalledProcessError as error:
            raise ValueError(f"campaign artifact path must be Git-ignored: {path}") from error


def compiler_version(root):
    return subprocess.check_output(["rustc", "-Vv"], cwd=root, text=True)


def campaign_environment(target_dir):
    env = support.clean_environment(os.environ)
    # Encoded flags override RUSTFLAGS in Cargo. Keep one explicit build policy.
    env.pop("CARGO_ENCODED_RUSTFLAGS", None)
    env.pop("RUST_LOG", None)
    env.update(RUSTFLAGS=RUSTFLAGS, CARGO_TARGET_DIR=str(target_dir))
    return env


def execute(command, *, root, env, output, errors=None, timeout):
    print(f"Running {shlex.join(map(str, command))}; log: {output}", flush=True)
    started = time.monotonic()
    with output.open("w") as stdout:
        if errors is None:
            code, timed_out = support.run_process(command, cwd=root, env=env, stdout=stdout,
                                                 stderr=subprocess.STDOUT, timeout=timeout)
        else:
            with errors.open("w") as stderr:
                code, timed_out = support.run_process(command, cwd=root, env=env, stdout=stdout,
                                                     stderr=stderr, timeout=timeout)
    if timed_out:
        raise TimeoutError(f"command timed out after {timeout}s; inspect {output}")
    if code:
        raise RuntimeError(f"command exited {code}; inspect {output}" + (f" and {errors}" if errors else ""))
    return time.monotonic() - started


def benchmark_command(binary, protocol, security, batch, threads, *, memory=False):
    command = [str(binary), "--security", str(security), "--batch", str(batch),
               "--threads", str(threads), "--seed", "42", "--warmup", "0" if memory else "1",
               "--iterations", "1" if memory else "5"]
    # Native is the established default, including baseline revisions that
    # predate the protocol-selection flag.
    if protocol != "native":
        command += ["--protocol", protocol]
    return command


def validate_case(process, *, protocol, security, batch, threads):
    prepared = process["prepared"]
    require(prepared.get("protocol") == PROTOCOLS[protocol], "benchmark selected the wrong protocol")
    expected = {"security_target": security, "batch": batch, "capacity": 1 << (batch - 1).bit_length(),
                "threads": threads, "input_seed": 42, "input_count": batch}
    require(all(prepared.get(key) == value for key, value in expected.items()), "benchmark selected the wrong workload")
    require(prepared.get("runtime_rustflags") == RUSTFLAGS, "benchmark runtime build flags mismatch")


def run_campaign(args):
    root = args.repo.resolve()
    verify_revision(root, args.revision)
    output = (root / args.output).resolve()
    require(not output.exists(), f"output directory already exists: {output}")
    target = (root / (args.target_dir or Path("target") / ("falcon-campaign-" + args.revision[:12]))).resolve()
    require_ignored(root, output)
    require_ignored(root, target)
    env = campaign_environment(target)
    compiler = compiler_version(root)
    require(bool(compiler.strip()), "rustc provenance unavailable")
    command = ["cargo", "build", "--locked", "--features", "falcon-hybrid", "--bench", "falcon_hybrid",
               "--profile", "release", "--message-format=json-render-diagnostics"]
    manifest = {
        "schema": "bitz/falcon-campaign/v1", "kind": "initial-" + args.kind,
        "revision": args.revision, "protocol_selection": args.protocol,
        "host": platform.node(), "architecture": platform.machine(), "platform": platform.platform(),
        "rustc": compiler, "lock_sha256": support.file_hash(root / "Cargo.lock"),
        "cpu_model": support.cpu_name(),
        "affinity": sorted(os.sched_getaffinity(0)) if hasattr(os, "sched_getaffinity") else None,
        "rustflags": RUSTFLAGS, "features": ["falcon-hybrid"], "profile": "release",
        "build_environment": support.environment(env), "build_command": command,
        "batches": BATCHES, "threads": THREADS, "security": SECURITY, "seeds": [42],
        "warmup": 1, "iterations": 5, "status": "waiting", "started_at": time.time(), "cases": [],
        "gate": {"lock": str(bench_gate.LOCK), "min_idle": args.min_idle,
                 "hold_seconds": args.hold_seconds, "poll_seconds": args.poll_seconds,
                 "max_idle_wait_seconds": args.max_idle_wait_seconds, "continuous_swap_guard": False},
        "build_timeout_seconds": args.build_timeout, "process_timeout_seconds": args.process_timeout,
    }
    output.mkdir(parents=True, exist_ok=False)
    manifest_path = output / "manifest.json"
    save = lambda: support.write_json(manifest_path, manifest)
    save()
    print("FALCON_CAMPAIGN_DIR=" + str(output), flush=True)
    acquired = False
    try:
        bench_gate.acquire("falcon-" + args.protocol + "-" + args.revision[:12], args.poll_seconds)
        acquired = True
        verify_revision(root, args.revision)
        manifest["status"] = "building"
        save()
        execute(command, root=root, env=env, output=output / "build.log", timeout=args.build_timeout)
        binary = Path(support.cargo_executables((output / "build.log").read_text(), ["falcon_hybrid"])["falcon_hybrid"])
        require(binary.is_absolute() and binary.is_file(), "Cargo benchmark executable is missing")
        verify_revision(root, args.revision)
        manifest.update(binary=str(binary), binary_sha256=support.file_hash(binary), status="waiting-for-idle")
        save()
        bench_gate.wait_idle("falcon-" + args.protocol, args.min_idle, args.hold_seconds,
                             args.poll_seconds, args.max_idle_wait_seconds)
        manifest["status"] = "measuring"
        save()
        for security in SECURITY:
            for threads in THREADS:
                for batch in BATCHES:
                    label = f"s{security}-b{batch}-t{threads}-seed42"
                    manifest["active_case"] = label
                    save()
                    print("FALCON_CASE_START=" + label, flush=True)
                    trial_env = dict(env, RAYON_NUM_THREADS=str(threads))
                    cmd = benchmark_command(binary, args.protocol, security, batch, threads)
                    sample_file = output / (label + ".jsonl")
                    elapsed = execute(cmd, root=root, env=trial_env, output=sample_file,
                                      errors=output / (label + ".stderr.log"), timeout=args.process_timeout)
                    latency = read_process(sample_file, 1, 5)
                    validate_case(latency, protocol=args.protocol, security=security, batch=batch, threads=threads)
                    memory_cmd = benchmark_command(binary, args.protocol, security, batch, threads, memory=True)
                    memory_file = output / (label + ".memory.jsonl")
                    execute(memory_cmd, root=root, env=trial_env, output=memory_file,
                            errors=output / (label + ".memory.stderr.log"), timeout=args.process_timeout)
                    memory = read_process(memory_file, 0, 1)
                    validate_case(memory, protocol=args.protocol, security=security, batch=batch, threads=threads)
                    same = [key for key in MATCHED if key not in ("warmup_trials", "measured_trials")]
                    require(all(latency["prepared"].get(key) == memory["prepared"].get(key) for key in same), "memory and timing inputs/configuration differ")
                    require(latency["samples"][0]["proof_debug_digest"] == memory["samples"][0]["proof_debug_digest"], "proof differs between timing and memory processes")
                    verify_revision(root, args.revision)
                    manifest["cases"].append({"label": label, "command": cmd, "memory_command": memory_cmd,
                                              "exit_code": 0, "elapsed_seconds": elapsed,
                                              "prepared": latency["prepared"], "summary": latency["summary"],
                                              "fresh_process_peak_rss_kib": memory["samples"][0]["process_peak_rss_kib"]})
                    manifest.pop("active_case")
                    save()
                    print(json.dumps(latency["summary"], allow_nan=False), flush=True)
                    print("FALCON_CASE_FINISHED=" + label, flush=True)
        verify_revision(root, args.revision)
        manifest.update(status="complete", finished_at=time.time())
        save()
    except BaseException as error:
        interrupted = isinstance(error, (KeyboardInterrupt, InterruptedError, SystemExit))
        manifest.update(status="interrupted" if interrupted else "failed", error=f"{type(error).__name__}: {error}", finished_at=time.time())
        save()
        raise
    finally:
        if acquired:
            bench_gate.release()
    print("FALCON_CAMPAIGN_COMPLETE=" + str(output), flush=True)
    return output


def parse_args(argv=None):
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--repo", type=Path, default=Path(__file__).resolve().parents[1])
    parser.add_argument("--revision", required=True)
    parser.add_argument("--protocol", choices=PROTOCOLS, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--kind", choices=("baseline", "candidate"), default="candidate")
    parser.add_argument("--target-dir", type=Path)
    parser.add_argument("--min-idle", type=float, default=88)
    parser.add_argument("--hold-seconds", type=float, default=30)
    parser.add_argument("--poll-seconds", type=float, default=5)
    parser.add_argument("--max-idle-wait-seconds", type=float, default=300)
    parser.add_argument("--build-timeout", type=float, default=1800)
    parser.add_argument("--process-timeout", type=float, default=600)
    args = parser.parse_args(argv)
    if not 0 <= args.min_idle <= 100 or any(not math.isfinite(getattr(args, name)) or getattr(args, name) <= 0 for name in
            ("hold_seconds", "poll_seconds", "max_idle_wait_seconds", "build_timeout", "process_timeout")):
        parser.error("idle percentage must be 0..100 and time bounds must be positive")
    return args


def main(argv=None):
    def interrupted(signum, _frame):
        raise InterruptedError(f"received signal {signum}")
    signal.signal(signal.SIGTERM, interrupted)
    try:
        run_campaign(parse_args(argv))
    except (Exception, KeyboardInterrupt) as error:
        print(f"Falcon campaign failed: {type(error).__name__}: {error}", file=sys.stderr)
        return 130 if isinstance(error, (KeyboardInterrupt, InterruptedError)) else 1
    return 0


if __name__ == "__main__":
    sys.exit(main())

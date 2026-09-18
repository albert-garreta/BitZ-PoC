#!/usr/bin/env python3
"""Build, gate, and report unified multiplication experiments.

Choose bitz (proof/witness/pcs/piop/outer/bounds) or compare (proof/witness/pcs).
Put launcher options before -- and forward benchmark options after it:
  run_multiplication_benchmarks.py bitz -- proof --workload u64 --w 3
  run_multiplication_benchmarks.py compare -- pcs --workload baby-bear
"""
from __future__ import annotations

import argparse
from datetime import datetime, timezone
import math
import json
import os
from pathlib import Path
import shlex
import signal
import subprocess
import sys
import tempfile

from bench_support import cargo_executables, clean_environment
from mul_report import main as report

ROOT = Path(__file__).resolve().parents[1]
TARGETS = {"bitz": "mul_bitz", "compare": "mul_compare"}


def main(argv=None):
    args = parse_args(argv)
    env = benchmark_environment(os.environ)
    build = build_command(args)
    inspection = any(flag in args.experiment for flag in ("--help", "-h", "--dry-run"))
    if args.dry_run:
        print("Build:", shlex.join(build))
        command = experiment_command(args, f"<Cargo executable: {TARGETS[args.target]}>", inspection)
        print("Run:", shlex.join(command))
        if not inspection:
            print("Report:", shlex.join([sys.executable, str(ROOT / "scripts/mul_report.py"),
                                        str(args.output), "--out", str(args.output / "reports")]))
        return 0
    try:
        # Help and Rust's case-planning mode may build, but never reserve results,
        # require Perfetto, acquire the measurement lock, or generate reports.
        if inspection:
            with tempfile.TemporaryDirectory(prefix="bitz-mul-build-") as directory:
                executable, code = build_executable(args, env, Path(directory) / "build.log")
                if code:
                    return code
                return run_campaign(experiment_command(args, executable, True), env, None, gated=False)
        if args.output.exists():
            raise ValueError(f"results directory already exists: {args.output}")
        args.output.mkdir(parents=True, exist_ok=False)
        print(f"Results: {args.output}", flush=True)
        executable, code = build_executable(args, env, args.output / "build.log")
        if code:
            return code
        command = experiment_command(args, executable, False)
        log = args.output / "run.log"
        print(f"Running {shlex.join(command)}\nLog: {log}", flush=True)
        with log.open("x") as stream:
            code = run_campaign(command, env, stream, gated=not args.no_gate)
        if code:
            print(f"Benchmark exited {code}; partial artifacts retained in {args.output}", file=sys.stderr)
            return code
        # The shared loader checks completion, verified trials, and configuration
        # identities before any tables are emitted.
        return report([str(args.output), "--out", str(args.output / "reports")])
    except KeyboardInterrupt as error:
        print("Interrupted; any partial results and logs are retained.", file=sys.stderr)
        return getattr(error, "exit_code", 130)
    except (OSError, ValueError, RuntimeError) as error:
        print(f"error: {error}", file=sys.stderr)
        return 1


def parse_args(argv=None):
    argv = list(sys.argv[1:] if argv is None else argv)
    separator = argv.index("--") if "--" in argv else len(argv)
    parser = argparse.ArgumentParser(description=__doc__,
                                     formatter_class=argparse.RawDescriptionHelpFormatter,
                                     allow_abbrev=False)
    parser.add_argument("target", choices=TARGETS)
    parser.add_argument("--output", type=Path,
                        help="new results directory (default: PerfRuns/<UTC timestamp>-multiplication)")
    parser.add_argument("--no-gate", action="store_true", help="run without the machine lock or swap guard")
    parser.add_argument("--swap-grow-gb", type=positive_number, default=12.0,
                        help="allowed swap growth while gated (default: 12 GiB)")
    parser.add_argument("--features", action="append", default=[], metavar="FEATURES",
                        help="additional comma-separated Cargo features; repeatable (e.g. bench-peak-memory)")
    parser.add_argument("--dry-run", action="store_true",
                        help="print build/run/report commands without building, running, or writing files")
    args = parser.parse_args(argv[:separator])
    args.experiment = argv[separator + 1:]
    # Output is orchestration state; worker transport is internal to Rust.
    for flag in args.experiment:
        if flag.split("=", 1)[0] in ("--out", "--worker"):
            parser.error("use launcher --output before --; --out and --worker are reserved")
    stamp = datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%S-%fZ")
    args.output = (args.output or ROOT / "PerfRuns" / f"{stamp}-multiplication").resolve()
    return args


def positive_number(value):
    number = float(value)
    if not math.isfinite(number) or number <= 0:
        raise argparse.ArgumentTypeError("must be a positive finite number")
    return number


def benchmark_environment(inherited):
    env = clean_environment(inherited)
    # Preserve a caller's custom lock while dropping pre-rename controls too.
    env = {key: value for key, value in env.items() if not key.startswith("F2Z_")}
    if "BITZ_BENCH_LOCK" in inherited:
        env["BITZ_BENCH_LOCK"] = inherited["BITZ_BENCH_LOCK"]
    if "RUSTFLAGS" not in env and "CARGO_ENCODED_RUSTFLAGS" not in env:
        env["RUSTFLAGS"] = "-C target-cpu=native"
    local = ROOT / ".tools/perfetto/trace_processor_shell"
    if "PERFETTO_TRACE_PROCESSOR" not in env and os.access(local, os.X_OK):
        env["PERFETTO_TRACE_PROCESSOR"] = str(local)
    return env


def build_command(args):
    features = ["span-metrics", "bench-internals"]
    if args.target == "compare":
        features.append("native-mul-compare")
    features.extend(feature for group in args.features for feature in group.split(",") if feature)
    features = list(dict.fromkeys(features))
    return ["cargo", "bench", "--no-run", "--locked", "--bench", TARGETS[args.target],
            "--features", ",".join(features), "--message-format=json"]


def build_executable(args, env, log):
    command = build_command(args)
    print("Building:", shlex.join(command), flush=True)
    with log.open("x") as stream:
        code = run_campaign(command, env, stream, gated=False)
    if code:
        # Inspection logs live in a temporary directory, so always show failures.
        print(log.read_text(), file=sys.stderr)
        return None, code
    target = TARGETS[args.target]
    source = (ROOT / "benches" / f"{target}.rs").resolve()
    artifacts = []
    for line in log.read_text().splitlines():
        if not line.startswith("{"):
            continue
        event = json.loads(line)
        metadata = event.get("target", {})
        if (event.get("reason") == "compiler-artifact"
                and "bench" in metadata.get("kind", [])
                and Path(metadata.get("src_path", "")).resolve() == source):
            artifacts.append(line)
    executable = cargo_executables("\n".join(artifacts), [target])[target]
    return executable, 0


def experiment_command(args, executable, inspection):
    command = [str(executable), *args.experiment]
    if inspection:
        return command
    command += ["--out", str(args.output)]
    if not args.no_gate:
        command = [sys.executable, str(ROOT / "scripts/bench_gate.py"), "run",
                   "--label", args.output.name, "--swap-grow-gb", str(args.swap_grow_gb), "--", *command]
    return command


def run_campaign(command, environment, stream, *, gated):
    with subprocess.Popen(command, cwd=ROOT, env=environment, stdout=stream,
                          stderr=subprocess.STDOUT, start_new_session=True) as process:
        try:
            code = process.wait()
        except BaseException:
            # Finish cleanup even if the user presses Ctrl-C again.
            handlers = {sig: signal.signal(sig, signal.SIG_IGN)
                        for sig in (signal.SIGINT, signal.SIGTERM)}
            try:
                # The gate owns a separate worker group and releases its lock
                # only after those workers have stopped.
                try:
                    if gated:
                        process.terminate()
                    else:
                        os.killpg(process.pid, signal.SIGTERM)
                except ProcessLookupError:
                    pass
                try:
                    process.wait(timeout=15)
                except subprocess.TimeoutExpired:
                    os.killpg(process.pid, signal.SIGKILL)
                    process.wait()
            finally:
                if not gated:
                    kill_remaining_workers(process.pid)
                for sig, handler in handlers.items():
                    signal.signal(sig, handler)
            raise
        finally:
            if not gated and process.poll() is not None:
                kill_remaining_workers(process.pid)
    return code if code >= 0 else 128 - code


def kill_remaining_workers(group):
    # A campaign leader can exit before a worker that ignores SIGTERM.
    try:
        os.killpg(group, signal.SIGKILL)
    except ProcessLookupError:
        pass


def terminated(signum, _frame):
    error = KeyboardInterrupt()
    error.exit_code = 128 + signum
    raise error


if __name__ == "__main__":
    signal.signal(signal.SIGTERM, terminated)
    sys.exit(main())

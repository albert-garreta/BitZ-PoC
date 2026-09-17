#!/usr/bin/env python3
"""Run the u32/u64/u128 multiplication comparisons sequentially.

Defaults reproduce the multiplication matrix of run_suite_2026_09_13.sh:
180 configurations, one warmup and five measured proofs per configuration,
plus separate memory trials. Plonky3-FRI supports u32 only.
"""
from __future__ import annotations

import argparse
import csv
from datetime import datetime, timezone
import json
import os
from pathlib import Path
import shlex
import shutil
import signal
import subprocess
import sys

import run_native_mul_compare as native

ROOT = Path(__file__).resolve().parents[1]
BACKENDS = ("f2z", "binius64", "binius64-ligerito", "plonky3-fri", "limber")
# Preserve the paper suite's per-backend size limits, including paging cells.
MAX_EXPONENT = {
    "u32": {"f2z": 23, "binius64": 23, "binius64-ligerito": 23, "plonky3-fri": 23, "limber": 19},
    "u64": {"f2z": 21, "binius64": 21, "binius64-ligerito": 21, "limber": 19},
    "u128": {"f2z": 21, "binius64": 21, "binius64-ligerito": 19, "limber": 19},
}


def main(argv=None):
    args = parse_args(argv)
    campaigns, skipped = plan(args)
    print_plan(args, campaigns, skipped)
    if args.dry_run:
        return 0
    try:
        environment = benchmark_environment(os.environ)
        environment["PERFETTO_TRACE_PROCESSOR"] = find_processor(environment)
        return execute(args, campaigns, skipped, environment)
    except (OSError, ValueError) as error:
        print(f"error: {error}", file=sys.stderr)
        return 1


def parse_args(argv=None):
    parser = argparse.ArgumentParser(description=__doc__,
                                     formatter_class=argparse.ArgumentDefaultsHelpFormatter)
    parser.add_argument("--workloads", nargs="+", choices=("u32", "u64", "u128"),
                        default=["u32", "u64", "u128"], help="operand widths; u32 is modulo 2^32")
    parser.add_argument("--backends", nargs="+", choices=BACKENDS, default=list(BACKENDS),
                        help="comparison systems; unsupported workload/backend pairs are listed and skipped")
    parser.add_argument("--threads", nargs="+", type=positive, default=[1, 10], help="Rayon worker counts")
    parser.add_argument("--reps", type=positive, default=5, help="measured proofs per configuration; warmup is additional")
    parser.add_argument("--exponents", nargs="+", type=positive,
                        help="override the per-backend default sizes with 2^N multiplications; larger sizes may page")
    parser.add_argument("--f2z-profiles", nargs="+", choices=("custom:1:4", "custom:3:4"),
                        default=["custom:1:4", "custom:3:4"], help="F2Z profiles: rates 1/2 and 1/8")
    parser.add_argument("--binius-rates", nargs="+", type=int, choices=(1, 3), default=[1, 3],
                        help="log inverse rates for both Binius backends: 1=1/2, 3=1/8")
    parser.add_argument("--output", type=Path,
                        help="new results directory; default: PerfRuns/<UTC timestamp>-multiplication")
    parser.add_argument("--no-gate", action="store_true",
                        help="run campaigns directly, without the machine lock or swap guard")
    parser.add_argument("--dry-run", action="store_true", help="print the matrix and exact commands without running or writing files")
    args = parser.parse_args(argv)
    for name in ("workloads", "backends", "threads", "exponents", "f2z_profiles", "binius_rates"):
        values = getattr(args, name)
        if values and len(set(values)) != len(values):
            parser.error(f"--{name.replace('_', '-')} must not contain duplicates")
    if not any(b in MAX_EXPONENT[w] for w in args.workloads for b in args.backends):
        parser.error("no supported workload/backend combinations selected")
    stamp = datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%S-%fZ")
    args.output = (args.output or ROOT / "PerfRuns" / f"{stamp}-multiplication").resolve()
    # Reuse the native runner's domain and shape checks before starting any jobs.
    try:
        plan(args)
    except ValueError as error:
        parser.error(str(error))
    return args


def positive(value):
    number = int(value)
    if number < 1:
        raise argparse.ArgumentTypeError("must be positive")
    return number


def plan(args):
    campaigns, skipped = [], []
    for workload in args.workloads:
        for backend in args.backends:
            if backend not in MAX_EXPONENT[workload]:
                skipped.append(f"{workload}/{backend}: backend supports u32 only")
    for threads in args.threads:
        for workload in args.workloads:
            for backend in args.backends:
                if backend not in MAX_EXPONENT[workload]:
                    continue
                exponents = args.exponents or list(range(15, MAX_EXPONENT[workload][backend] + 1, 2))
                variants = (args.f2z_profiles if backend == "f2z" else
                            args.binius_rates if backend.startswith("binius64") else [None])
                guard = (34 if workload == "u128" and backend == "binius64" else
                         30 if backend.startswith("binius64") else 10)
                for variant in variants:
                    profile = variant if backend == "f2z" else None
                    rate = variant if backend.startswith("binius64") else None
                    label = f"{workload}-{backend}"
                    if profile:
                        label += f"-rate{2 ** int(profile.split(':')[1])}"
                    elif rate:
                        label += f"-rate{2 ** rate}"
                    label += f"-t{threads}"
                    env = dict(RAYON_NUM_THREADS=str(threads), F2Z_BENCH_REPS=str(args.reps),
                               F2Z_BENCH_SHAPES=" ".join(map(str, exponents)),
                               F2Z_MUL_COMPARE_WORKLOADS=workload, F2Z_MUL_COMPARE_BACKENDS=backend,
                               F2Z_MUL_COMPARE_OUTPUT_DIR=str(args.output / label),
                               F2Z_MUL_COMPARE_MEMORY="1", F2Z_LIMBER_BDLAMBDA="100")
                    if profile:
                        env["F2Z_LIG_PROFILE"] = profile
                    if rate:
                        env["F2Z_BINIUS_LOG_INV_RATE"] = str(rate)
                    if backend == "binius64-ligerito":
                        env["F2Z_BINIUS_LIGERITO_ACCOUNTING"] = "rbr"
                    native.configuration(env)
                    command = [sys.executable, str(ROOT / "scripts/run_native_mul_compare.py")]
                    if not args.no_gate:
                        command = [sys.executable, str(ROOT / "scripts/bench_gate.py"), "run",
                                   "--label", label, "--swap-grow-gb", str(guard), "--", *command]
                    campaigns.append(dict(id=label, workload=workload, backend=backend,
                        threads=threads, exponents=exponents, f2z_profile=profile,
                        binius_log_inv_rate=rate, swap_guard_gb=None if args.no_gate else guard, environment=env,
                        command=command, status="pending"))
    return campaigns, skipped


def print_plan(args, campaigns, skipped):
    cells = sum(len(c["exponents"]) for c in campaigns)
    print(f"Multiplication benchmarks: {len(campaigns)} campaigns, {cells} configurations, "
          f"{cells * args.reps} measured proofs")
    print("One warmup and one separate memory trial per configuration; execution is sequential.")
    print(f"Results: {args.output}")
    for reason in skipped:
        print(f"Skipped: {reason}")
    print(f"{'Workload':8} {'Backend':19} {'Rate/profile':12} {'Threads':>7}  Exponents")
    for c in campaigns:
        rate = c["f2z_profile"] or (f"1/{2 ** c['binius_log_inv_rate']}" if c["binius_log_inv_rate"] else
                                    "1/2" if c["backend"] == "plonky3-fri" else "native")
        print(f"{c['workload']:8} {c['backend']:19} {rate:12} {c['threads']:7}  "
              + " ".join(map(str, c["exponents"])))
        if args.dry_run:
            print("  " + shlex.join(["env", "RUSTFLAGS=-C target-cpu=native",
                    *(f"{k}={v}" for k, v in c["environment"].items()), *c["command"]]))
    sys.stdout.flush()


def benchmark_environment(inherited):
    env = {k: v for k, v in inherited.items()
           if not k.startswith(("F2Z_", "F2_FOREST", "RAYON_"))
           and k not in (*native.CLEAR_ENV, "BDLAMBDA")}
    if "F2Z_BENCH_LOCK" in inherited:
        env["F2Z_BENCH_LOCK"] = inherited["F2Z_BENCH_LOCK"]
    env["RUSTFLAGS"] = "-C target-cpu=native"
    return env


def find_processor(env):
    requested = env.get("PERFETTO_TRACE_PROCESSOR")
    local = ROOT / ".tools/perfetto/trace_processor_shell"
    candidate = requested or (str(local) if local.is_file() else "trace_processor_shell")
    resolved = shutil.which(candidate, path=env.get("PATH", os.defpath))
    if not resolved:
        raise ValueError(f"Perfetto processor not executable: {candidate}. "
                         "Run bash scripts/install_trace_processor.sh, or set PERFETTO_TRACE_PROCESSOR.")
    return str(Path(resolved).resolve())


def execute(args, campaigns, skipped, environment):
    args.output.mkdir(parents=True, exist_ok=False)
    manifest = dict(schema="multiplication-benchmarks/v1", status="running",
                    created_utc=datetime.now(timezone.utc).isoformat(), repetitions=args.reps,
                    warmups=1, memory_trials=1, campaigns=campaigns, skipped=skipped,
                    rustflags=environment["RUSTFLAGS"],
                    trace_processor=environment["PERFETTO_TRACE_PROCESSOR"])

    def save():
        temporary = args.output / "suite.json.tmp"
        temporary.write_text(json.dumps(manifest, indent=2) + "\n")
        temporary.replace(args.output / "suite.json")

    save()
    try:
        for index, campaign in enumerate(campaigns, 1):
            campaign["status"] = "running"
            log = args.output / f"{campaign['id']}.log"
            campaign["log"] = str(log)
            save()
            print(f"[{index}/{len(campaigns)}] {campaign['id']} — log: {log}", flush=True)
            with log.open("x") as stream:
                code = run_campaign(campaign["command"], environment | campaign["environment"], stream,
                                    gated=not args.no_gate)
            campaign["returncode"] = code
            if code:
                campaign["status"] = manifest["status"] = "failed"
                save()
                print(f"Failed: {campaign['id']}. See {log}", file=sys.stderr)
                return code if code > 0 else 1
            campaign_dir = Path(campaign["environment"]["F2Z_MUL_COMPARE_OUTPUT_DIR"])
            child = json.loads((campaign_dir / "campaign.json").read_text())
            if child["status"] != "complete":
                raise ValueError(f"{campaign['id']} did not produce a completed campaign")
            campaign["status"] = "complete"
            save()
            combine_metrics(args.output, campaigns)
    except (KeyboardInterrupt, OSError, ValueError, KeyError) as error:
        manifest["status"] = "interrupted" if isinstance(error, KeyboardInterrupt) else "failed"
        for campaign in campaigns:
            if campaign["status"] == "running":
                campaign["status"] = manifest["status"]
        manifest["error"] = str(error)
        save()
        print(f"{manifest['status']}: {error}. Results retained in {args.output}", file=sys.stderr)
        return 130 if isinstance(error, KeyboardInterrupt) else 1
    manifest["status"] = "complete"
    save()
    print(f"Completed. Combined results: {args.output / 'metrics.csv'}")
    return 0


def run_campaign(command, environment, stream, *, gated=True):
    # The gate owns a separate native-worker process group. Let its SIGTERM
    # handler stop that group before releasing the lock; subprocess.run would
    # kill the gate immediately on Ctrl-C and could leave benchmarks running.
    with subprocess.Popen(command, cwd=ROOT, env=environment, stdout=stream,
                          stderr=subprocess.STDOUT, start_new_session=True) as process:
        try:
            return process.wait()
        except KeyboardInterrupt:
            if gated:
                process.terminate()
            else:
                # Without a gate, stop the native runner and its workers together.
                try:
                    os.killpg(process.pid, signal.SIGTERM)
                except ProcessLookupError:
                    pass
            process.wait()
            raise


def combine_metrics(output, campaigns):
    rows = []
    for c in campaigns:
        if c["status"] != "complete":
            continue
        with (output / c["id"] / "metrics.csv").open(newline="") as stream:
            for row in csv.DictReader(stream):
                rows.append(dict(campaign=c["id"], f2z_profile=c["f2z_profile"],
                                 binius_log_inv_rate=c["binius_log_inv_rate"], **row))
    if rows:
        temporary = output / "metrics.csv.tmp"
        with temporary.open("w", newline="") as stream:
            writer = csv.DictWriter(stream, fieldnames=list(rows[0]))
            writer.writeheader()
            writer.writerows(rows)
        temporary.replace(output / "metrics.csv")


if __name__ == "__main__":
    sys.exit(main())

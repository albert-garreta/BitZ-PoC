#!/usr/bin/env python3
"""Run native multiplication backends and the authors' Limber int_mult example."""
from __future__ import annotations

import argparse
import csv
from datetime import datetime, timezone
import hashlib
import json
import os
from pathlib import Path
import re
import shlex
import statistics
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]
TOOLCHAIN = "1.97.1"
LIMBER_ENV = {"RUSTFLAGS": "-C target-cpu=native", "RAYON_NUM_THREADS": "8"}
METRICS = ("setup_ms", "witness_ms", "commit_prove_ms", "verify_ms", "proof_bytes")
BOUNDARY = (
    "authors' int_mult: internal setup, witness generation, commit+prove and verify timers; "
    "Cargo/build/process startup excluded; proof size is eval_arg plus analytical sumcheck bytes"
)


def choices(env, name, default, allowed):
    values = env.get(name, default).replace(",", " ").split()
    if not values or len(values) != len(set(values)) or any(v not in allowed for v in values):
        raise ValueError(f"{name} must select distinct entries from {', '.join(allowed)}")
    return values


def configuration(env):
    workloads = choices(env, "F2Z_MUL_COMPARE_WORKLOADS", "u32 babybear",
                        ("u32", "babybear", "u64", "u128"))
    backends = choices(env, "F2Z_MUL_COMPARE_BACKENDS", "f2z binius64 plonky3-whir limber",
                       ("f2z", "binius64", "plonky3-whir", "limber"))
    exponents = [int(v) for v in env.get("F2Z_BENCH_SHAPES", "15").replace(",", " ").split()]
    external = "u32" in workloads and "limber" in backends
    for workload in workloads:
        if workload in ("u64", "u128"):
            unsupported = [b for b in backends if b not in ("f2z", "binius64")]
            if unsupported:
                raise ValueError(f"{workload} supports only f2z and binius64; "
                                 f"unsupported backends: {', '.join(unsupported)}")
    limits = {"u32": 25, "babybear": 24, "u64": 24, "u128": 23}
    maximum = min(limits[workload] for workload in workloads)
    if external:
        maximum = min(maximum, 24)
    minimum = 15 if "f2z" in backends else 4
    if (not exponents or len(exponents) != len(set(exponents))
            or any(not minimum <= n <= maximum for n in exponents)):
        raise ValueError(f"F2Z_BENCH_SHAPES must contain distinct exponents in {minimum}..={maximum}; "
                         "Limber int_mult supports --log-gates up to 24")
    reps = int(env.get("F2Z_BENCH_REPS", "5"))
    threads = int(env.get("RAYON_NUM_THREADS", "8"))
    if reps < 1 or threads < 1:
        raise ValueError("F2Z_BENCH_REPS and RAYON_NUM_THREADS must be positive")
    if external and threads != 8:
        raise ValueError("the authors' Limber command requires RAYON_NUM_THREADS=8")
    if env.get("F2Z_MUL_COMPARE_MEMORY", "1") not in ("0", "1"):
        raise ValueError("F2Z_MUL_COMPARE_MEMORY must be 0 or 1")
    limber_repo = Path(env.get("LIMBER_REPO", str(ROOT.parent / "limber-impl"))).expanduser().resolve()
    if external and not all((limber_repo / f).is_file()
                            for f in ("Cargo.toml", "examples/int_mult.rs")):
        raise ValueError(f"LIMBER_REPO={limber_repo} must be a Limber checkout containing "
                         "Cargo.toml and the authors' examples/int_mult.rs")
    stamp = datetime.now(timezone.utc).strftime("%Y-%m-%dT%H-%M-%S-%fZ")
    output = Path(env.get("F2Z_MUL_COMPARE_OUTPUT_DIR", str(ROOT / "PerfRuns" / f"{stamp}-native-mul")))
    if not output.is_absolute():
        output = ROOT / output
    return dict(workloads=workloads, backends=backends, exponents=exponents, reps=reps,
                threads=threads, external=external, limber_repo=limber_repo, output=output.resolve())


def jobs(config):
    """Keep the u32 Limber adapter out of every native Cargo invocation."""
    native = [b for b in config["backends"] if b != "limber"] if config["external"] else config["backends"]
    result = []
    if native:
        result.append(dict(kind="native", workloads=config["workloads"], backends=native, directory="."))
    if config["external"] and "babybear" in config["workloads"]:
        result.append(dict(kind="native", workloads=["babybear"], backends=["limber"],
                           directory="babybear-limber"))
    if config["external"]:
        result.append(dict(kind="limber-int-mult", workloads=["u32"], backends=["limber"],
                           directory="limber-int-mult"))
    return result


def limber_command(exponent):
    return ["cargo", f"+{TOOLCHAIN}", "run", "--release", "--example", "int_mult", "--",
            "--bits", "32", "--log-gates", str(exponent)]


def limber_environment(env):
    # Cargo gives the encoded flags priority over RUSTFLAGS. DUMP would add I/O
    # that is absent from the authors' command.
    result = dict(env)
    result.pop("CARGO_ENCODED_RUSTFLAGS", None)
    result.pop("DUMP", None)
    result.update(LIMBER_ENV)
    return result


def one_match(pattern, output, label):
    matches = list(re.finditer(pattern, output, re.MULTILINE))
    if len(matches) != 1:
        raise ValueError(f"expected exactly one int_mult {label} line, found {len(matches)}")
    return matches[0]


def parse_limber(output, exponent):
    header = one_match(
        r"^int_mult: 2\^(\d+)[−-]1 = (\d+) chained gates of (\d+)-bit mult\s+"
        r"\(cons=2\^(\d+), vars=2\^(\d+)\)\s*$", output, "shape")
    shape = tuple(map(int, header.groups()))
    if shape != (exponent, (1 << exponent) - 1, 32, exponent, exponent + 1):
        raise ValueError(f"int_mult returned an unexpected shape: {shape}")
    params = one_match(
        r"^params:\s+log_t_f=(\d+) k=(\d+) -> log_p=(\d+) s=(\d+) numlimb=(\d+)\s*$",
        output, "parameters")
    parameters = dict(zip(("log_t_f", "k", "log_p", "s", "numlimb"), map(int, params.groups())))
    if parameters["log_t_f"] != 32 or parameters["numlimb"] != 1:
        raise ValueError("int_mult did not report the 32-bit single-limb parameters")
    metrics = {}
    for label, name in (("setup", "setup_ms"), ("witness gen", "witness_ms"),
                        ("commit+prove", "commit_prove_ms"), ("verify", "verify_ms")):
        match = one_match(r"^" + re.escape(label) + r":\s+([0-9]+(?:\.[0-9]+)?) ms\s*$",
                          output, label)
        metrics[name] = float(match[1])
    size = one_match(
        r"^proof size:\s+[0-9]+(?:\.[0-9]+)? KB\s+"
        r"\(eval_arg (\d+) B \+ sumcheck (\d+) B\)\s*$", output, "proof size")
    eval_arg_bytes, sumcheck_bytes = map(int, size.groups())
    if eval_arg_bytes <= 0 or sumcheck_bytes != (3 * exponent + 2 * (exponent + 1) + 6) * 16:
        raise ValueError("int_mult proof-size components do not match the requested shape")
    metrics["proof_bytes"] = eval_arg_bytes + sumcheck_bytes
    return dict(gates=shape[1], parameters=parameters, metrics=metrics,
                eval_arg_bytes=eval_arg_bytes, sumcheck_bytes=sumcheck_bytes)


def run_logged(command, cwd, env, log_path):
    print(f"[{cwd}] {shlex.join(command)}", flush=True)
    lines = []
    with log_path.open("x") as log:
        with subprocess.Popen(command, cwd=cwd, env=env, stdout=subprocess.PIPE,
                              stderr=subprocess.STDOUT, text=True, bufsize=1) as process:
            for line in process.stdout:
                print(line, end="", flush=True)
                log.write(line)
                log.flush()
                lines.append(line)
            if process.wait() != 0:
                raise subprocess.CalledProcessError(process.returncode, command)
    return "".join(lines)


def write_json(path, value):
    path.write_text(json.dumps(value, indent=2) + "\n")


def run_limber(config, environment):
    repo = config["limber_repo"]
    directory = config["output"] / "limber-int-mult"
    directory.mkdir()
    env = limber_environment(environment)
    revision = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=repo, text=True).strip()
    dirty = bool(subprocess.check_output(["git", "status", "--porcelain"], cwd=repo, text=True))
    provenance = dict(repository=str(repo), git_revision=revision, git_dirty=dirty,
                      source_sha256=hashlib.sha256((repo / "examples/int_mult.rs").read_bytes()).hexdigest(),
                      rust_toolchain=TOOLCHAIN, environment=LIMBER_ENV, boundary=BOUNDARY,
                      workload="u32-mod32-chain", shared_corpus=False, peak_rss_bytes=None)
    if (repo / "Cargo.lock").is_file():
        provenance["cargo_lock_sha256"] = hashlib.sha256((repo / "Cargo.lock").read_bytes()).hexdigest()
    write_json(directory / "manifest.json", provenance)
    summaries = []
    with (directory / "samples.jsonl").open("x") as samples:
        for exponent in config["exponents"]:
            measured = []
            parameters = None
            command = limber_command(exponent)
            for trial in range(config["reps"] + 1):
                label = "warmup" if trial == 0 else f"sample-{trial - 1}"
                log = directory / f"log-gates-{exponent}-{label}.log"
                output = run_logged(command, repo, env, log)
                parsed = parse_limber(output, exponent)
                if parameters is not None and parameters != parsed["parameters"]:
                    raise ValueError("int_mult parameters changed between repetitions")
                parameters = parsed["parameters"]
                row = dict(schema="limber-int-mult/v1", backend="limber",
                           workload="u32-mod32-chain", log_gates=exponent, bits=32, threads=8,
                           trial=label, command=command, cwd=str(repo), log=log.name,
                           proof_verified=True, **parsed)
                samples.write(json.dumps(row) + "\n")
                samples.flush()
                if trial:
                    measured.append(parsed["metrics"])
            medians = {name: statistics.median(row[name] for row in measured) for name in METRICS}
            summary = dict(backend="limber", workload="u32-mod32-chain", log_gates=exponent,
                           gates=(1 << exponent) - 1, bits=32, threads=8, samples=config["reps"],
                           parameters=parameters, metrics=medians, peak_rss_bytes=None,
                           proof_verified=True, boundary=BOUNDARY)
            summaries.append(summary)
            write_json(directory / "summary.json", summaries)
            print("RESULT schema=limber-int-mult/1 "
                  f"log_gates={exponent} gates={summary['gates']} bits=32 threads=8 "
                  + " ".join(f"{key}={value}" for key, value in medians.items()), flush=True)
    with (directory / "metrics.csv").open("x", newline="") as stream:
        writer = csv.DictWriter(stream, fieldnames=("backend", "workload", "log_gates", "gates",
                                                    "bits", "threads", "samples", *METRICS))
        writer.writeheader()
        for row in summaries:
            writer.writerow({name: row[name] for name in writer.fieldnames if name not in METRICS}
                            | row["metrics"])
    print(f"Authors' Limber results: {directory}", flush=True)


def run_native(config, job, environment):
    directory = config["output"] / job["directory"]
    directory.mkdir(exist_ok=True)
    env = dict(environment)
    env.update(RUSTFLAGS=env.get("RUSTFLAGS", "-C target-cpu=native"),
               RAYON_NUM_THREADS=str(config["threads"]),
               F2Z_BENCH_SHAPES=" ".join(map(str, config["exponents"])),
               F2Z_BENCH_REPS=str(config["reps"]),
               F2Z_MUL_COMPARE_WORKLOADS=" ".join(job["workloads"]),
               F2Z_MUL_COMPARE_BACKENDS=" ".join(job["backends"]),
               F2Z_MUL_COMPARE_OUTPUT_DIR=str(directory))
    command = ["cargo", "bench", "--bench", "mul_e2e_compare",
               "--features", "bench-internals,native-mul-compare"]
    run_logged(command, ROOT, env, directory / "cargo-bench.log")
    profiler = Path(env.get("ZK_TRACE_SCRIPT",
                           str(Path.home() / ".ai-agent-army/skills/zk-proof-profiler/scripts/zk_trace.py")))
    if profiler.is_file():
        subprocess.run([sys.executable, str(profiler), "validate", str(directory / "trace.jsonl")], check=True)
        subprocess.run([sys.executable, str(profiler), "report", str(directory / "trace.jsonl"),
                        "--out-dir", str(directory / "reports"),
                        "--title", "Native u32 and BabyBear multiplication"], check=True)
        print(f"Interactive report: {directory / 'reports/intervals.html'}")
    print(f"Native metrics: {directory / 'metrics.csv'}")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--dry-run", action="store_true", help="print routing and commands without creating results")
    args = parser.parse_args()
    manifest = None
    config = None
    try:
        config = configuration(os.environ)
        planned = jobs(config)
        manifest = dict(schema="native-mul-campaign/v1", status="planned",
                        workloads=config["workloads"], backends=config["backends"],
                        exponents=config["exponents"], repetitions=config["reps"], jobs=planned)
        if config["external"]:
            manifest["limber"] = dict(cwd=str(config["limber_repo"]), environment=LIMBER_ENV,
                                     commands=[limber_command(n) for n in config["exponents"]],
                                     workload="u32-mod32-chain", shared_corpus=False)
        if args.dry_run:
            print(json.dumps(manifest, indent=2))
            return 0
        config["output"].mkdir(parents=True, exist_ok=False)
        manifest["status"] = "running"
        write_json(config["output"] / "campaign.json", manifest)
        for job in planned:
            if job["kind"] == "limber-int-mult":
                run_limber(config, os.environ)
            else:
                run_native(config, job, os.environ)
        manifest["status"] = "complete"
        write_json(config["output"] / "campaign.json", manifest)
        print(f"Campaign: {config['output']}")
        return 0
    except (ValueError, OSError, subprocess.CalledProcessError) as error:
        if manifest is not None and manifest["status"] == "running":
            manifest.update(status="failed", error=str(error))
            write_json(config["output"] / "campaign.json", manifest)
        print(f"error: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())

#!/usr/bin/env python3
"""Verified independent mod-2^32 comparison; Limber runs in its own checkout."""
from __future__ import annotations

import argparse
import csv
from datetime import datetime, timezone
import hashlib
import json
import os
from pathlib import Path
import platform
import shlex
import statistics
import subprocess
import sys

from native_mul_results import (CORE_METRICS, POLICY, SAMPLE_SCHEMA, SUMMARY_SCHEMA,
                                finite_number, fingerprint, validate_summary)

ROOT = Path(__file__).resolve().parents[1]
TOOLCHAIN = "1.98.1"
DEFAULT_SEED = 0x5533325043530064
BUILD = {"rust_toolchain": TOOLCHAIN, "rustflags": "-C target-cpu=native", "threads": 8}
CLEAR_ENV = ("CARGO_ENCODED_RUSTFLAGS", "DUMP", "CHAIN_BITS", "BDLAMBDA", "BDSPEC",
             "BDROWLEN", "BDDIRECT", "BDSPLIT", "F2Z_BINIUS_LOG_INV_RATE", "F2Z_MUL_MEMORY_ONLY")
MEMORY_BOUNDARY = "fresh process: corpus generation, public setup, witness generation, commitment, proving, verification, and proof-size accounting; one verified proof, no warmup"


def choices(env, name, default, allowed):
    values = env.get(name, default).replace(",", " ").split()
    if name == "F2Z_MUL_COMPARE_WORKLOADS":
        values = ["u32-mod32" if value == "u32" else value for value in values]
    if not values or len(values) != len(set(values)) or any(value not in allowed for value in values):
        raise ValueError(f"{name} must select distinct entries from {', '.join(allowed)}")
    return values


def configuration(env):
    workloads = choices(env, "F2Z_MUL_COMPARE_WORKLOADS", "u32-mod32", ("u32-mod32", "u64", "u128"))
    backends = choices(env, "F2Z_MUL_COMPARE_BACKENDS", "f2z binius64 plonky3-fri limber",
                       ("f2z", "binius64", "plonky3-fri", "plonky3-whir", "limber"))
    if any(w != "u32-mod32" for w in workloads) and any(b not in ("f2z", "binius64") for b in backends):
        raise ValueError("u64/u128 support only f2z and binius64; run the mod32 comparison separately")
    exponents = [int(value) for value in env.get("F2Z_BENCH_SHAPES", "15").replace(",", " ").split()]
    # Match the Rust address-space bound, not a particular machine's RAM.
    maximum = sys.maxsize.bit_length() + 1 - 11
    if "plonky3-fri" in backends:
        maximum = min(maximum, 29)  # Goldilocks two-adicity 32, FRI log blowup 3.
    if "limber" in backends:
        maximum = min(maximum, 24)
    minimum = 15 if "f2z" in backends else 4
    if not exponents or len(exponents) != len(set(exponents)) or any(not minimum <= n <= maximum for n in exponents):
        raise ValueError(f"F2Z_BENCH_SHAPES must contain distinct exponents in {minimum}..={maximum}")
    reps = int(env.get("F2Z_BENCH_REPS", "5"))
    threads = int(env.get("RAYON_NUM_THREADS", "8"))
    seed_text = env.get("F2Z_BENCH_SEED", str(DEFAULT_SEED))
    seed = int(seed_text, 16 if seed_text.lower().startswith("0x") else 10)
    if reps < 1 or threads < 1 or not 0 <= seed < 1 << 64:
        raise ValueError("repetitions must be positive, threads must be positive, and seed must fit u64")
    memory = env.get("F2Z_MUL_COMPARE_MEMORY", "1")
    if memory not in ("0", "1"):
        raise ValueError("F2Z_MUL_COMPARE_MEMORY must be 0 or 1")
    repo = Path(env.get("LIMBER_REPO", str(ROOT.parent / "limber-impl"))).expanduser().resolve()
    if "limber" in backends and not all((repo / name).is_file() for name in ("Cargo.toml", "examples/int_mult.rs")):
        raise ValueError(f"LIMBER_REPO={repo} must contain the f2z-benching independent Brakedown int_mult example")
    stamp = datetime.now(timezone.utc).strftime("%Y-%m-%dT%H-%M-%S-%fZ")
    output = Path(env.get("F2Z_MUL_COMPARE_OUTPUT_DIR", str(ROOT / "PerfRuns" / f"{stamp}-native-mul")))
    if not output.is_absolute():
        output = ROOT / output
    return dict(workloads=workloads, backends=backends, exponents=exponents, reps=reps,
                threads=threads, seed=seed, seed_explicit="F2Z_BENCH_SEED" in env, memory=memory == "1", limber_repo=repo, output=output.resolve())


def jobs(config):
    native = [b for b in config["backends"] if b != "limber"]
    result = []
    if native:
        result.append(dict(kind="native", workloads=config["workloads"], backends=native, directory="native"))
    if "limber" in config["backends"]:
        result.append(dict(kind="limber", workloads=["u32-mod32"], backends=["limber"], directory="limber-int-mult"))
    return result


def limber_command(exponent):
    return ["cargo", f"+{TOOLCHAIN}", "run", "--release", "--example", "int_mult", "--",
            "--bits", "32", "--log-gates", str(exponent)]


def campaign_environment(environment, config):
    env = dict(environment)
    for key in CLEAR_ENV:
        if key == "F2Z_BINIUS_LOG_INV_RATE" and "u32-mod32" not in config["workloads"]:
            continue
        env.pop(key, None)
    env.update(RUSTFLAGS=BUILD["rustflags"], RAYON_NUM_THREADS=str(config["threads"]),
               F2Z_BENCH_REPS=str(config["reps"]),
               F2Z_BENCH_SHAPES=" ".join(map(str, config["exponents"])),
               F2Z_MUL_COMPARE_MEMORY=str(int(config["memory"])))
    if config["seed_explicit"]:
        env["F2Z_BENCH_SEED"] = str(config["seed"])
    else:
        env.pop("F2Z_BENCH_SEED", None)
    return env


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
    path.write_text(json.dumps(value, indent=2, allow_nan=False) + "\n")


def machine_info():
    cpu = platform.processor()
    if Path("/proc/cpuinfo").is_file():
        cpu = next((line.split(":", 1)[1].strip() for line in Path("/proc/cpuinfo").read_text().splitlines()
                    if line.startswith("model name")), cpu)
    elif sys.platform == "darwin":
        cpu = subprocess.check_output(["sysctl", "-n", "machdep.cpu.brand_string"], text=True).strip()
    return dict(host=platform.node(), system=platform.system(), release=platform.release(),
                architecture=platform.machine(), cpu=cpu, logical_cpus=os.cpu_count())


def provenance(repo, machine, profile, threads):
    def git(*args):
        return subprocess.check_output(["git", *args], cwd=repo)
    revision = git("rev-parse", "HEAD").decode().strip()
    paths = git("ls-files", "-z", "--cached", "--others", "--exclude-standard").decode().split("\0")
    digest = hashlib.sha256()
    for name in sorted(set(paths)):
        path = Path(name)
        if (not name or path.parts[0] in ("target", "PerfRuns", "bench_results", ".git")
                or path.suffix not in (".rs", ".toml", ".lock", ".py", ".sh")):
            continue
        full = repo / path
        if full.is_file():
            digest.update(name.encode() + b"\0" + full.read_bytes() + b"\0")
    status = git("status", "--porcelain").decode().splitlines()
    dirty = any(not line[3:].startswith("PerfRuns/") for line in status)
    return dict(repository=str(repo), git_revision=revision, git_dirty=dirty,
                source_sha256=digest.hexdigest(),
                cargo_lock_sha256=hashlib.sha256((repo / "Cargo.lock").read_bytes()).hexdigest(),
                build=BUILD | {"profile": profile, "threads": threads}, machine=machine)


def structured_lines(output, prefix):
    return [json.loads(line[len(prefix):]) for line in output.splitlines() if line.startswith(prefix)]


def validate_sample(row, config, exponent, backend, workload):
    if (row.get("schema") != SAMPLE_SCHEMA or row.get("proof_verified") is not True
            or row.get("backend") != backend or row.get("workload") != workload
            or row.get("log_multiplications") != exponent or row.get("multiplications") != 1 << exponent
            or row.get("threads") != config["threads"] or row.get("measurement_policy") != POLICY):
        raise ValueError("sample does not match the requested independent verified workload")
    digest = row.get("corpus_digest", "")
    if len(digest) != 64 or any(c not in "0123456789abcdef" for c in digest):
        raise ValueError("sample lacks a canonical corpus digest")
    if row.get("seed", config["seed"]) != config["seed"] and workload == "u32-mod32":
        raise ValueError("sample used a different corpus seed")
    finite_number(row.get("setup_ms"), "setup_ms")
    for name in CORE_METRICS:
        finite_number(row.get("metrics", {}).get(name), name, positive=name == "proof_bytes")
    settings = row["config"]
    if backend == "f2z":
        from ligerito_results import validate_ligerito
        report = validate_ligerito(settings.get("ligerito"), 100)
        if workload == "u32-mod32":
            cfg = report["configuration"]
            if cfg.get("initial_k") != 4 or cfg["levels"][0].get("log_inv_rate") != 3:
                raise ValueError("mod32 comparison requires matched Ligerito rate 1/8 and initial_k=4")
    if backend == "binius64" and (settings.get("fri_query_target_bits") != 100 or (workload == "u32-mod32" and settings.get("log_inv_rate") != 1)):
        raise ValueError("Binius must use the canonical 100-bit query target at rate 1/2")
    if backend == "binius64" and workload == "u32-mod32":
        n = 1 << exponent
        expected = {"and":4*n,"imul":n,"zero":0,"bmul":0}
        if settings.get("word_constraints") != expected:
            raise ValueError("unexpected compiled Binius mod32 constraint counts")
    if backend == "plonky3-fri":
        expected = dict(pcs="FRI", base_field="Goldilocks", extension_degree=5, target_bits=100,
                        log_inv_rate=3, num_queries=100, max_log_arity=1, log_final_poly_len=0,
                        commit_pow_bits=0, query_pow_bits=0, trace_width=137, num_constraints=139)
        if any(settings.get(key) != value for key, value in expected.items()):
            raise ValueError("Plonky3 must use the agreed Goldilocks AIR and FRI configuration")
        if finite_number(settings.get("proven_bits"), "FRI proven_bits") < 100:
            raise ValueError("Plonky3-FRI security report is below 100 bits")
    if backend == "plonky3-whir":
        expected = dict(pcs="WHIR", base_field="Goldilocks", encoding="Reed-Solomon",
                        opening_claim="prescribed multilinear evaluation")
        security = settings.get("security", {})
        if (any(settings.get(key) != value for key, value in expected.items())
                or settings.get("params", {}).get("extension_degree") not in (2, 5)
                or security.get("model") != "native-air-whir-johnson-union/v1"
                or security.get("assumption") != "JohnsonBound" or security.get("target_bits") != 100
                or security.get("air") != dict(log_height=exponent, width=137, constraints=139, constraint_degree=2)):
            raise ValueError("WHIR must report Johnson accounting for the shared mod32 AIR")
        if finite_number(security.get("achieved_bits"), "WHIR achieved_bits") < 100:
            raise ValueError("Plonky3-WHIR security report is below 100 bits")
    if backend == "limber":
        n = 1 << exponent
        expected = dict(commitment_backend="brakedown", constraints=n, padded_constraints=n,
                        variables=3*n, padded_variables=4*n, quotients=n, log_t_f=32, numlimb=1, k=9, target_bits=114, engine="T256DynPrimeBdEngine")
        if any(settings.get(key) != value for key, value in expected.items()):
            raise ValueError("Limber did not use independent single-limb Brakedown rows")
        if settings.get("brakedown") != dict(target_bits=114, spec=4, row_len_cap=32768, direct_open_max=65536, split=False):
            raise ValueError("Limber used an ambient Brakedown parameter override")
        parts = row.get("proof_size", {})
        for key in ("commitment_bytes", "eval_arg_bytes", "sumcheck_bytes"):
            finite_number(parts.get(key), key, positive=True)
        if (parts["sumcheck_bytes"] != (3*exponent + 2*(exponent+2) + 6)*16
                or sum(parts.values()) != row["metrics"]["proof_bytes"]):
            raise ValueError("Limber proof size must include commitments and the correct sumcheck payload")


def summarize_case(samples, memory, config, backend, workload, exponent, source):
    expected_trials = [{"kind": "warmup", "index": 0}] + [
        {"kind": "sample", "index": i} for i in range(config["reps"])]
    if [row.get("trial") for row in samples] != expected_trials:
        raise ValueError("expected one in-process warmup followed by the requested measured samples")
    first = samples[0]
    for row in samples:
        validate_sample(row, config, exponent, backend, workload)
        if any(row.get(key) != first.get(key) for key in ("config", "corpus_digest", "setup_ms")):
            raise ValueError("case parameters/corpus/setup changed between warm repetitions")
    if config["memory"]:
        if (memory is None or memory.get("backend") != backend or memory.get("workload") != workload
                or memory.get("log_multiplications") != exponent or memory.get("corpus_digest") != first["corpus_digest"]
                or memory.get("proof_verified") is not True or memory.get("boundary") != MEMORY_BOUNDARY):
            raise ValueError("missing or incompatible isolated memory result")
        finite_number(memory.get("peak_rss_bytes"), "peak_rss_bytes", positive=True)
        finite_number(memory.get("proof_bytes", memory.get("metrics", {}).get("proof_bytes")), "memory proof_bytes", positive=True)
        timed_config = {key: value for key, value in first["config"].items() if key != "proof_size_encoding"}
        memory_config = {key: value for key, value in memory.get("config", {}).items() if key != "proof_size_encoding"}
        if memory_config != timed_config:
            raise ValueError("memory proof configuration differs from the timed proof")
    metrics = {name: statistics.median(row["metrics"][name] for row in samples[1:]) for name in CORE_METRICS}
    for name in ("commit_ms", "piop_ms", "opening_ms", "pcs_ms", "post_proof_ms"):
        if all(name in row["metrics"] for row in samples):
            metrics[name] = statistics.median(row["metrics"][name] for row in samples[1:])
    summary = dict(schema=SUMMARY_SCHEMA, backend=backend, workload=workload, log_multiplications=exponent,
                   multiplications=1 << exponent, samples=config["reps"], warmups=1, threads=config["threads"],
                   seed=first.get("seed", config["seed"]), corpus_digest=first["corpus_digest"], config=first["config"],
                   setup_ms=first["setup_ms"], medians=metrics, measurement_policy=POLICY, proof_verified=True,
                   peak_rss_bytes=None if memory is None else memory["peak_rss_bytes"], memory=memory, provenance=source)
    summary["protocol_fingerprint"] = fingerprint(summary)
    validate_summary(summary)
    for row in samples:
        row.update(provenance=source, protocol_fingerprint=summary["protocol_fingerprint"])
    return summary


def run_native(config, job, environment, machine):
    directory = config["output"] / job["directory"]
    directory.mkdir()
    env = campaign_environment(environment, config)
    env.update(F2Z_MUL_COMPARE_WORKLOADS=" ".join(job["workloads"]),
               F2Z_MUL_COMPARE_BACKENDS=" ".join(job["backends"]), F2Z_MUL_COMPARE_OUTPUT_DIR=str(directory))
    command = ["cargo", f"+{TOOLCHAIN}", "bench", "--bench", "mul_e2e_compare",
               "--features", "bench-internals,native-mul-compare"]
    run_logged(command, ROOT, env, directory / "cargo-bench.log")
    source = provenance(ROOT, machine, "bench", config["threads"]) | {"command": command}
    rows = [json.loads(line) for line in (directory / "samples.jsonl").read_text().splitlines()]
    memories = [json.loads(line) for line in (directory / "memory.jsonl").read_text().splitlines()]
    cases = len(job["workloads"]) * len(job["backends"]) * len(config["exponents"])
    if len(rows) != cases * (config["reps"] + 1) or len(memories) != cases * int(config["memory"]):
        raise ValueError("native output contains missing or unexpected cases")
    summaries = []
    for workload in job["workloads"]:
        for backend in job["backends"]:
            for exponent in config["exponents"]:
                match = lambda row: (row["workload"], row["backend"], row["log_multiplications"]) == (workload, backend, exponent)
                memory = [row for row in memories if match(row)]
                if len(memory) > 1:
                    raise ValueError("duplicate memory result")
                summaries.append(summarize_case([row for row in rows if match(row)], memory[0] if memory else None,
                                                config, backend, workload, exponent, source))
    return summaries, rows


def run_limber(config, environment, machine):
    repo = config["limber_repo"]
    directory = config["output"] / "limber-int-mult"
    directory.mkdir()
    env = campaign_environment(environment, config)
    summaries, rows = [], []
    for exponent in config["exponents"]:
        command = limber_command(exponent)
        memory = None
        if config["memory"]:
            output = run_logged(command, repo, env | {"F2Z_MUL_MEMORY_ONLY": "1"}, directory / f"log-gates-{exponent}-memory.log")
            results = structured_lines(output, "LIMBER_MUL_MEMORY ")
            if len(results) != 1:
                raise ValueError("expected one Limber isolated memory result")
            memory = results[0]
        output = run_logged(command, repo, env, directory / f"log-gates-{exponent}-warm.log")
        samples = structured_lines(output, "LIMBER_MUL_RESULT ")
        source = provenance(repo, machine, "release", config["threads"]) | {"command": command}
        summaries.append(summarize_case(samples, memory, config, "limber", "u32-mod32", exponent, source))
        rows.extend(samples)
    write_json(directory / "summary.json", summaries)
    return summaries, rows


def export(config, summaries, samples):
    identities = {}
    for row in summaries:
        key = (row["workload"], row["log_multiplications"])
        identity = (row["corpus_digest"], row["measurement_policy"], row["threads"])
        if key in identities and identities[key] != identity:
            raise ValueError("backends used different corpora or measurement policies")
        identities[key] = identity
    expected = {(w, b, n) for w in config["workloads"] for b in config["backends"] for n in config["exponents"]}
    actual = {(r["workload"], r["backend"], r["log_multiplications"]) for r in summaries}
    if actual != expected or len(actual) != len(summaries):
        raise ValueError("campaign has missing or duplicate backend results")
    write_json(config["output"] / "summary.json", summaries)
    with (config["output"] / "samples.jsonl").open("x") as stream:
        for row in samples:
            stream.write(json.dumps(row, allow_nan=False) + "\n")
    fields = ("workload", "backend", "log_multiplications", "multiplications", "samples", "threads",
              "setup_ms", *CORE_METRICS, "peak_rss_bytes", "corpus_digest", "protocol_fingerprint")
    with (config["output"] / "metrics.csv").open("x", newline="") as stream:
        writer = csv.DictWriter(stream, fieldnames=fields)
        writer.writeheader()
        for row in summaries:
            values = row | row["medians"]
            writer.writerow({key: values.get(key) for key in fields})


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--dry-run", action="store_true")
    args = parser.parse_args()
    manifest = None
    try:
        config = configuration(os.environ)
        planned = jobs(config)
        manifest = dict(schema="native-mul-campaign/v2", status="planned", workloads=config["workloads"],
                        backends=config["backends"], exponents=config["exponents"], repetitions=config["reps"],
                        warmups=1, measurement_policy=POLICY, jobs=planned, build=BUILD | {"threads": config["threads"]},
                        limber_commands=[limber_command(n) for n in config["exponents"]] if "limber" in config["backends"] else [])
        if args.dry_run:
            print(json.dumps(manifest, indent=2))
            return 0
        config["output"].mkdir(parents=True, exist_ok=False)
        manifest.update(status="running", machine=machine_info())
        write_json(config["output"] / "campaign.json", manifest)
        summaries, samples = [], []
        for job in planned:
            if job["kind"] == "limber":
                summary, rows = run_limber(config, os.environ, manifest["machine"])
            else:
                summary, rows = run_native(config, job, os.environ, manifest["machine"])
            summaries.extend(summary)
            samples.extend(rows)
        export(config, summaries, samples)
        manifest["status"] = "complete"
        write_json(config["output"] / "campaign.json", manifest)
        print(f"Verified comparison: {config['output']}")
        return 0
    except (ValueError, KeyError, OSError, subprocess.CalledProcessError) as error:
        if manifest is not None and manifest["status"] == "running":
            manifest.update(status="failed", error=str(error))
            write_json(config["output"] / "campaign.json", manifest)
        print(f"error: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())

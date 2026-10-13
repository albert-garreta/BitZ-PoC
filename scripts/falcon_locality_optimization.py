#!/usr/bin/env python3
"""Pinned canonical Falcon locality/arithmetic experiments with frozen provenance.

--configs JSON maps names to {"binary": "candidate" or "baseline", "env": {...},
optional "degrees": [512, 1024], "threads": [1, 8, 12, 16], ordered
"worker_cpus": [6, 7, ...], and "main_cpu": 6}. Overrides are
explicit; no previous experiment BASE is inherited. Cold trials are first proofs
in fresh processes/Prepared objects, not cache-flushed measurements. Repeated
trials share Prepared across warmups and samples. Trace phases stay separate.
"""

import argparse
from collections import defaultdict
import itertools
import json
import math
import os
from pathlib import Path
import random
import statistics
import sys
import time

from bench_support import ROOT, cpu_name, file_hash, run_process, write_json
from binder_experiments import read_jsonl
from falcon_buffer_optimization import CPUS, EXTRA_METRICS, METRICS, normalize, safe_name

SCHEMA = "bitz/falcon-locality-optimization/v1"
ALLOWED_ENV = {
    "BITZ_ARITHMETIC_NORM_PREFIX": {"auto", "1", "2", "3", "4"},
    "BITZ_FOREST_ROW_GRAIN": {"1", "2", "4", "8", "16"},
    "BITZ_ARITHMETIC_DENSE_COMPACTION": {"off", "2", "4", "8"},
    "BITZ_ARITHMETIC_COLLAPSE_BATCH": {"1", "2", "4", "8"},
    "BITZ_FOREST_JIT_BUCKETS": {"reduced", "wide"},
    "BITZ_FOREST_L4": {"rebuild", "retain"},
    "BITZ_PCS_INITIAL_TILES_PER_TASK": {"16", "4", "2"},
    "BITZ_ARITHMETIC_BINDING_CONSTRUCTION_FUSION": {"on", "off"},
    "BITZ_ARITHMETIC_BINDING_STRATEGY": {"legacy", "optimized"},
}
HASHED_FILES = {"command.json": "command_sha256", "stdout.jsonl": "stdout_sha256",
                "stderr.log": "stderr_sha256", "process.json": "process_sha256"}
RUNTIME_PREFIXES = ("BITZ_", "F2_", "OBLONG_", "FLOCK_", "NTT_", "LIG_", "LIGERITO_",
                    "PCS_", "MERKLE_", "PERM_", "LINCHECK_", "VERIFY_", "OUTER_")


def parser_provenance():
    return dict(runner_sha256=file_hash(Path(__file__)),
                helper_sha256={name: file_hash(Path(__file__).with_name(name)) for name in
                               ["bench_support.py", "binder_experiments.py", "falcon_buffer_optimization.py",
                                "falcon_arithmetic_optimization.py"]})


def subprocess_env(overrides):
    # Dependency kernel and trace flags must not leak from the invoking shell.
    env = {key: value for key, value in os.environ.items() if not key.startswith(RUNTIME_PREFIXES)
           and key not in ["RAYON_NUM_THREADS", "RUSTFLAGS", "CARGO_ENCODED_RUSTFLAGS", "FZF_DEFAULT_OPTS"]}
    env.update(overrides)
    return env


def default_configs():
    control = {"BITZ_FOREST_ROW_GRAIN": "1", "BITZ_ARITHMETIC_DENSE_COMPACTION": "off",
               "BITZ_ARITHMETIC_COLLAPSE_BATCH": "1", "BITZ_ARITHMETIC_NORM_PREFIX": "auto"}
    configs = {"baseline": dict(binary="baseline", env={}),
               "production": dict(binary="candidate", env={}),
               "control": dict(binary="candidate", env=control),
               "legacy": dict(binary="candidate", env={"BITZ_ARITHMETIC_BINDING_STRATEGY": "legacy"}),
               "optimized": dict(binary="candidate", env={"BITZ_ARITHMETIC_BINDING_STRATEGY": "optimized"})}
    for prefix, key, values in [
            ("forest", "BITZ_FOREST_ROW_GRAIN", ["2", "4", "8", "16"]),
            ("compact", "BITZ_ARITHMETIC_DENSE_COMPACTION", ["2", "4", "8"]),
            ("collapse", "BITZ_ARITHMETIC_COLLAPSE_BATCH", ["2", "4", "8"]),
            ("norm", "BITZ_ARITHMETIC_NORM_PREFIX", ["2", "3", "4"])]:
        for value in values:
            configs[prefix + value] = dict(binary="candidate", env=dict(control, **{key: value}))
    return configs


def configurations(args):
    configs = default_configs()
    if args.configs:
        extra = json.loads(args.configs.read_text())
        if not isinstance(extra, dict):
            raise ValueError("configs must map names to configuration objects")
        configs.update(extra)
    selected = args.variants or [name for name in configs if name not in {"production", "legacy", "optimized"}]
    if not selected or len(set(selected)) != len(selected) or set(selected) - configs.keys():
        raise ValueError("unknown, empty or repeated variants")
    output = {}
    for name in selected:
        config = configs[name]
        if (not safe_name(name) or not isinstance(config, dict)
                or config.keys() - {"binary", "env", "degrees", "threads", "worker_cpus", "main_cpu"}
                or config.get("binary") not in ["baseline", "candidate"]
                or not isinstance(config.get("env"), dict)):
            raise ValueError(f"invalid configuration: {name}")
        for key, value in config["env"].items():
            if key not in ALLOWED_ENV or not isinstance(value, str) or value not in ALLOWED_ENV[key]:
                raise ValueError(f"unsupported override in {name}: {key}={value}")
        if config["binary"] == "baseline" and config["env"]:
            raise ValueError("frozen baseline configurations must use their original defaults")
        for field, valid in [("degrees", {512, 1024}), ("threads", set(range(1, len(CPUS) + 1)))]:
            if field in config:
                values = config[field]
                if (not isinstance(values, list) or not values or any(type(v) is not int for v in values)
                        or len(set(values)) != len(values) or set(values) - valid):
                    raise ValueError(f"invalid {field} filter in {name}")
        cpus = config.get("worker_cpus", CPUS)
        if (not isinstance(cpus, list) or not cpus or len(cpus) > len(CPUS)
                or any(type(cpu) is not int or cpu < 0 for cpu in cpus)
                or len(set(cpus)) != len(cpus)):
            raise ValueError(f"invalid worker_cpus in {name}")
        main_cpu = config.get("main_cpu", 6)
        if type(main_cpu) is not int or main_cpu < 0:
            raise ValueError(f"invalid main_cpu in {name}")
        output[name] = config
    return output


def build_manifest(args):
    if not safe_name(args.phase):
        raise ValueError("invalid phase name")
    configs = configurations(args)
    for name in ["degrees", "threads", "seeds", "modes"]:
        values = getattr(args, name)
        if not values or len(set(values)) != len(values):
            raise ValueError(f"empty or repeated {name}")
    if (args.batch <= 0 or args.security <= 0 or args.cold_repetitions <= 0
            or args.iterations <= 0 or args.warmup < 0 or args.timeout <= 0
            or any(seed < 0 for seed in args.seeds)
            or any(t <= 0 or t > len(CPUS) for t in args.threads)):
        raise ValueError("invalid dimensions, seed, workers or trial counts")
    allowed = sorted(os.sched_getaffinity(0)) if hasattr(os, "sched_getaffinity") else []
    binaries = {}
    for role, value in [("baseline", args.baseline), ("candidate", args.binary)]:
        if value is None or not value.is_file() or not os.access(value, os.X_OK):
            raise ValueError(f"{role} must name a frozen executable")
        binaries[role] = dict(path=str(value.resolve()), sha256=file_hash(value))
    if binaries["baseline"]["sha256"] == binaries["candidate"]["sha256"]:
        raise ValueError("baseline and candidate hashes are identical; check build provenance")
    blocks = [(degree, workers, seed, mode, repetition)
              for degree, workers, seed, mode in itertools.product(args.degrees, args.threads, args.seeds, args.modes)
              for repetition in range(args.cold_repetitions if mode == "cold" else 1)]
    rng = random.Random(args.order_seed + sum(map(ord, args.phase)))
    rng.shuffle(blocks)
    jobs = []
    for degree, workers, seed, mode, repetition in blocks:
        names = [name for name, c in configs.items()
                 if degree in c.get("degrees", args.degrees) and workers in c.get("threads", args.threads)]
        rng.shuffle(names)
        for name in names:
            config = configs[name]
            cpus = config.get("worker_cpus", CPUS)[:workers]
            main_cpu = config.get("main_cpu", 6)
            if len(cpus) != workers:
                raise ValueError(f"too few worker CPUs for {workers} threads in {name}")
            if allowed and not set([*cpus, main_cpu]).issubset(allowed):
                raise ValueError(f"worker/main CPUs outside process affinity in {name}")
            job_id = f"{name}-{mode}-n{degree}-b{args.batch}-s{args.security}-t{workers}-seed{seed}-r{repetition}"
            jobs.append(dict(id=job_id, variant="arithmetic", config=name, binary=configs[name]["binary"],
                             protocol="canonical", degree=degree, threads=workers, seed=seed, mode=mode,
                             worker_cpus=cpus, main_cpu=main_cpu,
                             repetition=repetition, warmup=0 if mode == "cold" else args.warmup,
                             iterations=1 if mode == "cold" else args.iterations))
    if not jobs:
        raise ValueError("configuration filters selected no jobs")
    return dict(schema=SCHEMA, phase=args.phase, configs=configs, binaries=binaries, jobs=jobs,
                batch=args.batch, security=args.security, trace=args.trace,
                # The shared normalizer permits unavailable cached-vector metrics
                # from the previous production baseline; selectors still apply below.
                candidate_production=True, worker_cpus=CPUS, main_cpu=6,
                case_cache=str(args.case_cache.resolve()), cwd=str(ROOT), platform=sys.platform,
                cpu=cpu_name(), allowed_cpus=allowed, timeout=args.timeout, order_seed=args.order_seed,
                **parser_provenance())


def command_record(job, manifest):
    binary = manifest["binaries"][job["binary"]]
    env = dict(manifest["configs"][job["config"]]["env"])
    env.update(RAYON_NUM_THREADS=str(job["threads"]),
               BITZ_FALCON_WORKER_CPUS=",".join(map(str, job["worker_cpus"])),
               BITZ_FALCON_MAIN_CPU=str(job["main_cpu"]), BITZ_FALCON_CASE_CACHE=manifest["case_cache"])
    command = [binary["path"], "--degree", str(job["degree"]), "--batch", str(manifest["batch"]),
               "--security", str(manifest["security"]), "--threads", str(job["threads"]),
               "--seed", str(job["seed"]), "--warmup", str(job["warmup"]), "--iterations", str(job["iterations"])]
    if manifest["trace"]:
        command.append("--trace")
        env.update(BITZ_FALCON_LIGHT_PROFILE="1", BITZ_TRACE="1", LIG_PROVE_TRACE="1")
    return dict(command=command, overrides=env, binary_sha256=binary["sha256"], cwd=manifest["cwd"])


def normalized_trials(path, job, manifest):
    # The shared normalizer's legacy metric exception follows binary role, not
    # arbitrary user-selected configuration names.
    normalization_job = dict(job, config=job["binary"])
    topology = dict(manifest, worker_cpus=job["worker_cpus"], main_cpu=job["main_cpu"])
    return normalize(read_jsonl(path), normalization_job, topology)


def validate_result(path, job, manifest, manifest_hash):
    result = json.loads(path.read_text())
    if (result.get("schema") != SCHEMA or result.get("job") != job or result.get("verified") is not True
            or result.get("manifest_sha256") != manifest_hash):
        raise ValueError(f"stale or unverified result: {path}")
    for filename, key in HASHED_FILES.items():
        if file_hash(path.parent / filename) != result.get(key):
            raise ValueError(f"{filename} hash mismatch: {path.parent}")
    if json.loads((path.parent / "command.json").read_text()) != command_record(job, manifest):
        raise ValueError(f"command differs from frozen settings: {path.parent}")
    process = json.loads((path.parent / "process.json").read_text())
    if process.get("exit_code") != 0 or process.get("timed_out") is not False:
        raise ValueError(f"unsuccessful process: {path.parent}")
    if normalized_trials(path.parent / "stdout.jsonl", job, manifest) != result["trials"]:
        raise ValueError(f"saved trials differ from raw output: {path.parent}")
    return result


def validate_all(out):
    results, sources, proofs, baselines, counts, binaries = [], {}, {}, set(), defaultdict(int), {}
    expected = set()
    manifests = sorted((out / "phases").glob("*.json"))
    if not manifests:
        raise ValueError("no phase manifests found")
    provenance = parser_provenance()
    for path in manifests:
        manifest = json.loads(path.read_text())
        if manifest.get("schema") != SCHEMA or manifest["phase"] != path.stem:
            raise ValueError(f"invalid phase manifest: {path}")
        if any(manifest.get(key) != value for key, value in provenance.items()):
            raise ValueError(f"runner/helper parser provenance changed: {path}")
        for binary in manifest["binaries"].values():
            if binary["path"] not in binaries:
                binaries[binary["path"]] = file_hash(binary["path"])
            if binaries[binary["path"]] != binary["sha256"]:
                raise ValueError(f"frozen binary changed: {binary['path']}")
        manifest_hash = file_hash(path)
        for job in manifest["jobs"]:
            result_path = out / "runs" / manifest["phase"] / job["id"] / "result.json"
            if result_path in expected:
                raise ValueError(f"duplicate job: {result_path}")
            expected.add(result_path)
            if not result_path.exists():
                raise ValueError(f"incomplete phase; resume first: {result_path}")
            result = validate_result(result_path, job, manifest, manifest_hash)
            results.append((manifest, result))
            for trial in result["trials"]:
                key = (job["degree"], manifest["batch"], manifest["security"], job["seed"])
                source = trial["input_digest"], trial["source_root"]
                if sources.setdefault(key, source) != source:
                    raise ValueError(f"input/source mismatch: {result_path}")
                proof_key = (*key, trial["protocol_id"])
                proof = trial["proof_debug_digest"], trial["proof_payload_bytes"]
                if proofs.setdefault(proof_key, proof) != proof:
                    raise ValueError(f"complete proof/payload mismatch: {result_path}")
                if job["binary"] == "baseline":
                    baselines.add(proof_key)
                counts[trial["protocol_id"]] += 1
    if set((out / "runs").glob("*/*/result.json")) != expected:
        raise ValueError("unexpected stale results outside frozen job lists")
    if proofs.keys() - baselines:
        raise ValueError(f"missing frozen baseline cases: {sorted(proofs.keys() - baselines)}")
    validation = dict(schema=SCHEMA, verified_proofs=sum(counts.values()), proofs_by_protocol=dict(counts),
                      frozen_baseline_cases=len(baselines), input_source_equality=True,
                      complete_proof_and_payload_equality=True, command_and_raw_hashes_verified=True,
                      frozen_binary_hashes_verified=True, parser_hashes_verified=True, phases_complete=True)
    write_json(out / "validation.json", validation)
    return results, validation


def group_key(manifest, job):
    return (manifest["phase"], job["config"], job["degree"], manifest["batch"],
            manifest["security"], job["threads"], job["mode"], manifest["trace"])


def group_metadata(key):
    return dict(zip(["phase", "variant", "degree", "batch", "security", "threads", "mode", "trace"], key))


def profile_samples(path, job):
    events = []
    for line in path.read_text().splitlines():
        try:
            event = json.loads(line)
        except ValueError:
            continue  # BITZ_TRACE also emits non-JSON diagnostics.
        if isinstance(event, dict) and event.get("event") == "stage":
            events.append(event)
    roots = [event for event in events if event["name"] == "falcon_algebraic:prove"]
    if len(roots) != job["warmup"] + job["iterations"]:
        raise ValueError(f"prover trace-root count mismatch: {path}")
    output = []
    for root in roots[job["warmup"]:]:
        values = defaultdict(float)
        for event in events:
            if event["span_id"] != root["span_id"] and root["span_id"] not in event["ancestor_ids"]:
                continue
            elapsed = event["elapsed_ms"]
            if type(elapsed) not in [int, float] or not math.isfinite(elapsed) or elapsed < 0:
                raise ValueError(f"invalid elapsed time in {path}")
            values[event["name"]] += elapsed
            for name, value in event["fields"].items():
                if name.endswith("_ns"):
                    if type(value) not in [int, float] or not math.isfinite(value) or value < 0:
                        raise ValueError(f"invalid timer field in {path}: {name}")
                    values[f"{event['name']}/{name}"] += value / 1e6
        output.append(dict(values))
    return output


def summarize(out, results):
    groups, profiles = defaultdict(list), defaultdict(list)
    for manifest, result in results:
        job = result["job"]
        key = group_key(manifest, job)
        groups[key].append(result)
        if manifest["trace"]:
            path = out / "runs" / manifest["phase"] / job["id"] / "stderr.log"
            profiles[key].extend((job["seed"], sample) for sample in profile_samples(path, job))
    metrics = [*METRICS, *EXTRA_METRICS]
    def medians(trials):
        return {metric: statistics.median(values) if values else None for metric in metrics
                for values in [[trial[metric] for trial in trials if trial[metric] is not None]]}
    rows = []
    for key, items in sorted(groups.items()):
        samples = [(item["job"]["seed"], t) for item in items for t in item["trials"] if t["trial"] == "sample"]
        trials = [trial for _, trial in samples]
        peaks = [max(t["process_peak_rss_kib"] for t in item["trials"]) for item in items]
        row = dict(group_metadata(key), binary=items[0]["job"]["binary"],
                   worker_cpus=items[0]["job"]["worker_cpus"], main_cpu=items[0]["job"]["main_cpu"],
                   samples=len(trials), processes=len(items),
                   medians=medians(trials), metric_samples={m: sum(t[m] is not None for t in trials) for m in metrics},
                   seed_medians={str(seed): medians([t for s, t in samples if s == seed]) for seed in sorted({s for s, _ in samples})},
                   process_peak_rss_kib=dict(median=statistics.median(peaks), maximum=max(peaks)))
        rows.append(row)
        print(f"{row['phase']:12} {row['variant']:18} N={row['degree']} t={row['threads']} {row['mode']:8} "
              f"n={row['samples']} total={row['medians']['total_prover_ms']:.3f} "
              f"verify={row['medians']['verify_ms']:.3f} traced={row['trace']}", flush=True)
    write_json(out / "summary.json", dict(rows=rows, notes=[
        "Only trace=false rows are latency evidence; traced phases are separate diagnostics.",
        "Cold is first commit+prove after setup in a fresh process/Prepared; caches are not flushed.",
        "Repeated samples share Prepared; warmups are excluded and within-process samples are correlated.",
        "Resource sampling, preparation, input generation and verification are outside prover timing.",
        "Minor faults are counts, not bytes; retained RSS includes inputs, allocator and Prepared.",
        "Cached-buffer bytes cover additional coordinate/word-binding vectors only, not existing sumcheck scratch.",
        "Peak RSS is process-lifetime high-water; per-process maxima are reported separately.",
        "Unavailable metrics are null, never zero. Per-seed medians do not establish statistical significance.",
        "Topology is frozen per configuration/job; different CPU placements are separate variants, not thread-count-only effects.",
    ]))
    rows = []
    for key, samples in sorted(profiles.items()):
        names = sorted(set().union(*(sample.keys() for _, sample in samples)))
        rows.append(dict(group_metadata(key), samples=len(samples),
                         medians_ms={name: statistics.median(sample.get(name, 0) for _, sample in samples) for name in names},
                         seed_medians_ms={str(seed): {name: statistics.median(sample.get(name, 0) for s, sample in samples if s == seed)
                                                     for name in names} for seed in sorted({s for s, _ in samples})}))
    write_json(out / "profile-summary.json", dict(rows=rows, notes=[
        "Samples contain the prover root and descendants only; warmups and verifier spans are excluded.",
        "Nested elapsed wall spans overlap. *_worker_ns values are summed worker time, converted to ms; do not add them to wall time.",
        "Non-JSON BITZ_TRACE diagnostics are preserved in raw stderr and skipped by the stage parser.",
    ]))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--binary", type=Path)
    parser.add_argument("--baseline", type=Path)
    parser.add_argument("--phase", default="screen")
    parser.add_argument("--configs", type=Path)
    parser.add_argument("--variants", nargs="+")
    parser.add_argument("--degrees", type=int, nargs="+", choices=[512, 1024], default=[512, 1024])
    parser.add_argument("--threads", type=int, nargs="+", default=[12])
    parser.add_argument("--seeds", type=int, nargs="+", default=[42, 43, 44])
    parser.add_argument("--batch", type=int, default=1024)
    parser.add_argument("--security", type=int, default=128)
    parser.add_argument("--modes", nargs="+", choices=["cold", "repeated"], default=["cold", "repeated"])
    parser.add_argument("--cold-repetitions", type=int, default=7)
    parser.add_argument("--warmup", type=int, default=2)
    parser.add_argument("--iterations", type=int, default=7)
    parser.add_argument("--case-cache", type=Path, default=ROOT / ".tmp/falcon-poc-performance-cases-20261012")
    parser.add_argument("--order-seed", type=int, default=20261013)
    parser.add_argument("--timeout", type=int, default=600)
    parser.add_argument("--trace", action="store_true")
    parser.add_argument("--summarize-only", action="store_true")
    args = parser.parse_args()
    out = args.output.resolve()
    if not args.summarize_only:
        manifest = build_manifest(args)
        path = out / "phases" / f"{args.phase}.json"
        path.parent.mkdir(parents=True, exist_ok=True)
        if path.exists():
            if json.loads(path.read_text()) != manifest:
                raise ValueError("phase configuration/binary/runner changed; use a new phase")
        else:
            write_json(path, manifest)
        manifest_hash = file_hash(path)
        for job in manifest["jobs"]:
            directory = out / "runs" / args.phase / job["id"]
            if (directory / "result.json").exists():
                validate_result(directory / "result.json", job, manifest, manifest_hash)
                continue
            directory.mkdir(parents=True, exist_ok=True)
            record = command_record(job, manifest)
            write_json(directory / "command.json", record)
            env = subprocess_env(record["overrides"])
            print(f"RUN {args.phase}/{job['id']}", flush=True)
            start = time.time()
            with (directory / "stdout.jsonl").open("w") as stdout, (directory / "stderr.log").open("w") as stderr:
                code, timed_out = run_process(record["command"], env=env, cwd=ROOT, stdout=stdout, stderr=stderr, timeout=args.timeout)
            write_json(directory / "process.json", dict(start_unix=start, elapsed_seconds=time.time() - start,
                                                       exit_code=code, timed_out=timed_out))
            if code != 0 or timed_out:
                raise RuntimeError(f"failed {directory}; inspect stderr.log")
            result = dict(schema=SCHEMA, job=job, verified=True, manifest_sha256=manifest_hash,
                          trials=normalized_trials(directory / "stdout.jsonl", job, manifest))
            result.update({key: file_hash(directory / filename) for filename, key in HASHED_FILES.items()})
            write_json(directory / "result.json", result)
            print(f"OK {job['id']}", flush=True)
    results, validation = validate_all(out)
    print(json.dumps(validation, sort_keys=True), flush=True)
    summarize(out, results)


if __name__ == "__main__":
    main()

#!/usr/bin/env python3
"""Pinned Falcon buffer experiments, separating first proofs from repeated proofs.

Cold means a fresh process, a fresh Prepared object, and no warmup proofs. It
does not mean flushed CPU/page caches. Repeated trials share one Prepared object
per process/seed, including its warmups. All execution is sequential and untraced.
Frozen baseline binaries may omit the newly introduced per-proof memory metrics.
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
from binder_experiments import METRICS, normalize_trials, read_jsonl
from falcon_arithmetic_optimization import BASE as ARITHMETIC_BASE, CPUS

SCHEMA = "bitz/falcon-buffer-optimization/v1"
BASE = dict(ARITHMETIC_BASE, BITZ_ARITHMETIC_BINDING_OWNED="on",
            BITZ_ARITHMETIC_COORDINATE_BUFFERS="baseline",
            BITZ_ARITHMETIC_WORD_DIRECT_INIT="off", BITZ_ARITHMETIC_WORD_SCRATCH="off",
            BITZ_ARITHMETIC_FACTORED_INITIAL_FUSED="off",
            BITZ_ARITHMETIC_FACTORED_FOLD_REDUCTION="off")
VARIANTS = {
    "baseline": {},
    "control": {},
    "coordinate-zeroed": {"BITZ_ARITHMETIC_COORDINATE_BUFFERS": "zeroed"},
    "coordinate-direct": {"BITZ_ARITHMETIC_COORDINATE_BUFFERS": "direct"},
    "coordinate-reuse": {"BITZ_ARITHMETIC_COORDINATE_BUFFERS": "reuse"},
    "word-direct": {"BITZ_ARITHMETIC_WORD_DIRECT_INIT": "on"},
    "word-reuse": {"BITZ_ARITHMETIC_WORD_SCRATCH": "on"},
    "factored-initial": {"BITZ_ARITHMETIC_FACTORED_INITIAL_FUSED": "on"},
    "factored-fold": {"BITZ_ARITHMETIC_FACTORED_FOLD_REDUCTION": "on"},
    "combined": {"BITZ_ARITHMETIC_COORDINATE_BUFFERS": "reuse",
                 "BITZ_ARITHMETIC_WORD_DIRECT_INIT": "on",
                 "BITZ_ARITHMETIC_WORD_SCRATCH": "on"},
}
FACTORED_KEYS = ["BITZ_ARITHMETIC_FACTORED_INITIAL_FUSED",
                 "BITZ_ARITHMETIC_FACTORED_FOLD_REDUCTION"]
MEMORY_METRICS = ["prover_minor_faults", "process_rss_before_prover_kib",
                  "process_rss_after_prover_kib", "process_retained_rss_kib",
                  "prepared_scratch_bytes_before_prover", "prepared_retained_scratch_bytes"]
EXTRA_METRICS = ["prepare_ms", "external_input_ms", *MEMORY_METRICS]


def safe_name(value):
    return bool(value) and all(c in "abcdefghijklmnopqrstuvwxyz0123456789-_" for c in value)


def normalize(events, job, manifest):
    """Reuse strict affinity/proof validation, adding trial order and memory data."""
    settings = dict(degree=job["degree"], batch=manifest["batch"], security=manifest["security"],
                    warmup=job["warmup"], iterations=job["iterations"],
                    worker_cpus=manifest["worker_cpus"], main_cpu=manifest["main_cpu"])
    trials = normalize_trials(events, job, settings)
    raw = [event for event in events if event.get("trial") in ["warmup", "sample"]]
    for index, (event, trial) in enumerate(zip(raw, trials)):
        warmup = index < job["warmup"]
        expected = ("warmup", None) if warmup else ("sample", index - job["warmup"])
        if (event["trial"], event.get("iteration")) != expected:
            raise ValueError(f"trial order/iteration mismatch: {job['id']}")
        version = "v9" if job["protocol"] == "canonical" else "v10-factored-ring"
        if trial["protocol_id"] != f"bitz/falcon{job['degree']}-algebraic/non-zk/{version}":
            raise ValueError(f"selected protocol was not used: {job['id']}")
        security = event.get("algebraic_security_bits")
        if type(security) not in [int, float] or not math.isfinite(security) or security < manifest["security"]:
            raise ValueError(f"algebraic security below requested target: {job['id']}")
        sizes = dict(event["proof_payload_breakdown"])
        if (any(type(size) is not int or size < 0 for size in sizes.values())
                or sum(sizes.values()) != event["proof_payload_bytes"]):
            raise ValueError(f"proof payload breakdown mismatch: {job['id']}")
        trial["iteration"] = expected[1]
        for metric in EXTRA_METRICS:
            value = event.get(metric)
            optional = metric in MEMORY_METRICS and (job["config"] == "baseline" or manifest["platform"] != "linux")
            if value is None and optional:
                trial[metric] = None
                continue
            if type(value) not in [int, float] or not math.isfinite(value) or value < 0:
                raise ValueError(f"missing/invalid {metric}: {job['id']}")
            if metric in MEMORY_METRICS and type(value) is not int:
                raise ValueError(f"noninteger {metric}: {job['id']}")
            if metric.startswith("process_") and "rss" in metric and value == 0:
                raise ValueError(f"zero RSS: {job['id']}")
            trial[metric] = value
    return trials


def command_record(job, manifest):
    binary = manifest["binaries"]["baseline" if job["config"] == "baseline" else "candidate"]
    overrides = dict(BASE)
    overrides.update(manifest["variants"][job["config"]])
    if job["config"] == "combined" and job["protocol"] == "factored":
        overrides.update({key: "on" for key in FACTORED_KEYS})
    overrides.update(BITZ_ARITHMETIC_RING=job["protocol"], RAYON_NUM_THREADS=str(job["threads"]),
                     BITZ_FALCON_WORKER_CPUS=",".join(map(str, manifest["worker_cpus"][:job["threads"]])),
                     BITZ_FALCON_MAIN_CPU=str(manifest["main_cpu"]), BITZ_FALCON_CASE_CACHE=manifest["case_cache"])
    command = [binary["path"], "--degree", str(job["degree"]), "--batch", str(manifest["batch"]),
               "--security", str(manifest["security"]), "--threads", str(job["threads"]),
               "--seed", str(job["seed"]), "--warmup", str(job["warmup"]),
               "--iterations", str(job["iterations"])]
    return dict(command=command, overrides=overrides, binary_sha256=binary["sha256"], cwd=manifest["cwd"])


def validate_result(path, job, manifest, manifest_hash):
    result = json.loads(path.read_text())
    if (result.get("job") != job or result.get("verified") is not True
            or result.get("manifest_sha256") != manifest_hash or result.get("schema") != SCHEMA):
        raise ValueError(f"stale or unverified benchmark result: {path}")
    for filename, key in [("command.json", "command_sha256"), ("stdout.jsonl", "stdout_sha256"),
                          ("stderr.log", "stderr_sha256"), ("process.json", "process_sha256")]:
        if file_hash(path.parent / filename) != result.get(key):
            raise ValueError(f"{filename} hash mismatch: {path.parent}")
    command = json.loads((path.parent / "command.json").read_text())
    if command != command_record(job, manifest):
        raise ValueError(f"command does not match frozen configuration: {path.parent}")
    process = json.loads((path.parent / "process.json").read_text())
    if process.get("exit_code") != 0 or process.get("timed_out") is not False:
        raise ValueError(f"unsuccessful process: {path.parent}")
    trials = normalize(read_jsonl(path.parent / "stdout.jsonl"), job, manifest)
    if trials != result["trials"]:
        raise ValueError(f"saved trials differ from raw output: {path.parent}")
    return result


def validate_all(out):
    results, sources, proofs, baselines, payloads = [], {}, {}, set(), {}
    verified_binaries = {}
    manifests = sorted((out / "phases").glob("*.json"))
    if not manifests:
        raise ValueError("no phase manifests found")
    expected_paths = set()
    for path in manifests:
        manifest = json.loads(path.read_text())
        if manifest.get("schema") != SCHEMA or path.stem != manifest["phase"]:
            raise ValueError(f"invalid phase manifest: {path}")
        for binary in manifest["binaries"].values():
            if binary["path"] not in verified_binaries:
                verified_binaries[binary["path"]] = file_hash(binary["path"])
            if verified_binaries[binary["path"]] != binary["sha256"]:
                raise ValueError(f"frozen binary changed: {binary['path']}")
        manifest_hash = file_hash(path)
        for job in manifest["jobs"]:
            result_path = out / "runs" / manifest["phase"] / job["id"] / "result.json"
            expected_paths.add(result_path)
            if not result_path.exists():
                raise ValueError(f"incomplete phase; resume before summarizing: {result_path}")
            result = validate_result(result_path, job, manifest, manifest_hash)
            results.append((manifest, result))
            for trial in result["trials"]:
                source_key = (job["degree"], manifest["batch"], manifest["security"], job["seed"])
                source = trial["input_digest"], trial["source_root"]
                if sources.setdefault(source_key, source) != source:
                    raise ValueError(f"input/source mismatch: {result_path}")
                key = (*source_key, trial["protocol_id"])
                proof = trial["proof_debug_digest"], trial["proof_payload_bytes"]
                if proofs.setdefault(key, proof) != proof:
                    raise ValueError(f"complete proof mismatch: {result_path}")
                if job["config"] == "baseline":
                    baselines.add(key)
                payloads[(*source_key, job["protocol"])] = trial["proof_payload_bytes"]
    if set((out / "runs").glob("*/*/result.json")) != expected_paths:
        raise ValueError("unexpected result files outside the frozen phase job lists")
    if proofs.keys() - baselines:
        raise ValueError(f"missing frozen baseline proof comparisons: {sorted(proofs.keys() - baselines)}")
    growth = []
    for (*source_key, protocol), size in payloads.items():
        if protocol == "factored" and (*source_key, "canonical") in payloads:
            canonical = payloads[(*source_key, "canonical")]
            if size * 10 > canonical * 11:
                raise ValueError(f"factored proof exceeds 110% payload limit: {source_key}")
            growth.append((size / canonical - 1) * 100)
    counts = defaultdict(int)
    for _, result in results:
        for trial in result["trials"]:
            counts[trial["protocol_id"]] += 1
    validation = dict(schema=SCHEMA, verified_proofs=sum(counts.values()), proofs_by_protocol=dict(counts),
                      frozen_baseline_comparisons=len(proofs), input_source_equality=True,
                      complete_proof_equality_within_protocol=True, command_and_raw_hashes_verified=True,
                      frozen_binary_hashes_verified=True, phases_complete=True,
                      experimental_payload_comparisons=len(growth),
                      max_experimental_payload_growth_percent=max(growth, default=None))
    write_json(out / "validation.json", validation)
    return results, validation


def summarize(out, results):
    groups = defaultdict(list)
    for manifest, result in results:
        job = result["job"]
        key = (manifest["phase"], job["config"], job["protocol"], job["mode"], job["degree"],
               manifest["batch"], manifest["security"], job["threads"])
        groups[key].extend((job["seed"], trial) for trial in result["trials"] if trial["trial"] == "sample")
    metrics = [*METRICS, *EXTRA_METRICS]
    def medians(trials):
        return {metric: statistics.median(values) if values else None for metric in metrics
                for values in [[t[metric] for t in trials if t[metric] is not None]]}
    rows = []
    for key, entries in sorted(groups.items()):
        phase, variant, protocol, mode, degree, batch, security, threads = key
        trials = [trial for _, trial in entries]
        row = dict(phase=phase, variant=variant, protocol=protocol, mode=mode, degree=degree,
                   batch=batch, security=security, threads=threads, samples=len(trials),
                   medians=medians(trials),
                   metric_samples={metric: sum(t[metric] is not None for t in trials) for metric in metrics},
                   seed_medians={str(seed): medians([t for s, t in entries if s == seed])
                                 for seed in sorted({s for s, _ in entries})})
        rows.append(row)
        print(f"{phase:12} {variant:18} {protocol:9} {mode:8} N={degree:4} t={threads:2} "
              f"n={len(trials):3} total={row['medians']['total_prover_ms']:.3f} "
              f"faults={row['medians']['prover_minor_faults']} "
              f"retained_kib={row['medians']['process_retained_rss_kib']}", flush=True)
    write_json(out / "summary.json", dict(rows=rows, notes=[
        "Cold: first proof in a fresh process/Prepared; CPU and filesystem caches are not flushed.",
        "Repeated: warmups and samples share Prepared and process; warmups excluded from summary.",
        "Prover time excludes input generation, Prepared construction, resource sampling, and verification.",
        "Minor-fault deltas cover commit+prove across all process threads; counts are not bytes.",
        "Retained RSS is sampled after proof drop and verification; Prepared, inputs and allocator remain live.",
        "Prepared scratch bytes count cached vector capacity only; exclude in-flight and allocator-retained memory.",
        "Peak RSS is the process lifetime high-water mark, not a per-proof allocation measurement.",
        "Frozen old baselines may lack new memory metrics; their null values are never treated as zero.",
        "Latency trials are untraced. Cold and repeated results must be compared separately.",
    ]))


def build_manifest(args):
    if not safe_name(args.phase):
        raise ValueError("phase must contain lowercase letters, digits, - or _")
    variants = {name: dict(values) for name, values in VARIANTS.items()}
    if args.configs:
        extra = json.loads(args.configs.read_text())
        if not isinstance(extra, dict) or "baseline" in extra:
            raise ValueError("configs must map names to overrides and cannot redefine baseline")
        variants.update(extra)
    if len(set(args.variants)) != len(args.variants) or set(args.variants) - variants.keys():
        raise ValueError("unknown or repeated variant")
    variants = {name: variants[name] for name in args.variants}
    for name, overrides in variants.items():
        if not safe_name(name) or not isinstance(overrides, dict):
            raise ValueError(f"invalid configuration: {name}")
        for key, value in overrides.items():
            allowed = ["baseline", "zeroed", "direct", "reuse"] if key == "BITZ_ARITHMETIC_COORDINATE_BUFFERS" else ["on", "off"]
            if key not in {"BITZ_ARITHMETIC_COORDINATE_BUFFERS", "BITZ_ARITHMETIC_WORD_DIRECT_INIT",
                           "BITZ_ARITHMETIC_WORD_SCRATCH", *FACTORED_KEYS} or value not in allowed:
                raise ValueError(f"unsupported experimental override: {key}={value}")
    for name in ["degrees", "threads", "seeds", "protocols", "modes"]:
        values = getattr(args, name)
        if len(set(values)) != len(values):
            raise ValueError(f"repeated {name}")
    if (args.batch <= 0 or args.security <= 0 or args.cold_repetitions <= 0 or args.iterations <= 0
            or args.warmup < 0 or args.timeout <= 0 or any(s < 0 for s in args.seeds)
            or any(t <= 0 or t > len(CPUS) for t in args.threads)):
        raise ValueError("invalid batch/security/trial count/seed/thread count")
    allowed_cpus = sorted(os.sched_getaffinity(0)) if hasattr(os, "sched_getaffinity") else []
    if allowed_cpus and not set(CPUS[:max(args.threads)]).issubset(allowed_cpus):
        raise ValueError("requested worker CPUs are outside process affinity")
    binaries = {}
    for role in ["baseline", "candidate"]:
        value = args.baseline if role == "baseline" else args.binary
        if value is None or not value.is_file() or not os.access(value, os.X_OK):
            raise ValueError(f"{role} binary must name a frozen executable")
        path = value.resolve()
        binaries[role] = dict(path=str(path), sha256=file_hash(path))
    blocks = [(degree, threads, seed, protocol, mode, repetition)
              for degree, threads, seed, protocol, mode in itertools.product(
                  args.degrees, args.threads, args.seeds, args.protocols, args.modes)
              for repetition in range(args.cold_repetitions if mode == "cold" else 1)]
    rng = random.Random(args.order_seed)
    rng.shuffle(blocks)
    jobs = []
    for degree, threads, seed, protocol, mode, repetition in blocks:
        configs = list(variants)
        rng.shuffle(configs)
        for config in configs:
            if protocol == "canonical" and any(variants[config].get(key) == "on" for key in FACTORED_KEYS):
                continue
            name = f"{config}-{protocol}-{mode}-n{degree}-b{args.batch}-s{args.security}-t{threads}-seed{seed}-r{repetition}"
            jobs.append(dict(id=name, variant="arithmetic", config=config, protocol=protocol, mode=mode,
                             degree=degree, threads=threads, seed=seed, repetition=repetition,
                             warmup=0 if mode == "cold" else args.warmup,
                             iterations=1 if mode == "cold" else args.iterations))
    return dict(schema=SCHEMA, phase=args.phase, binaries=binaries, variants=variants, jobs=jobs,
                batch=args.batch, security=args.security, worker_cpus=CPUS, main_cpu=6,
                case_cache=str(args.case_cache.resolve()), cwd=str(ROOT), platform=sys.platform,
                cpu=cpu_name(), allowed_cpus=allowed_cpus, timeout=args.timeout, order_seed=args.order_seed,
                runner_sha256=file_hash(Path(__file__)),
                helper_sha256={name: file_hash(Path(__file__).with_name(name)) for name in
                               ["bench_support.py", "binder_experiments.py", "falcon_arithmetic_optimization.py"]})


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--binary", type=Path)
    parser.add_argument("--baseline", type=Path)
    parser.add_argument("--phase", default="screen")
    parser.add_argument("--variants", nargs="+", default=list(VARIANTS))
    parser.add_argument("--configs", type=Path, help="Additional named environment override dictionaries")
    parser.add_argument("--protocols", nargs="+", choices=["canonical", "factored"], default=["canonical", "factored"])
    parser.add_argument("--modes", nargs="+", choices=["cold", "repeated"], default=["cold", "repeated"])
    parser.add_argument("--cold-repetitions", type=int, default=7, help="Fresh processes per seed/config")
    parser.add_argument("--warmup", type=int, default=2, help="Repeated-mode warmup proofs")
    parser.add_argument("--iterations", type=int, default=7, help="Repeated-mode measured proofs")
    parser.add_argument("--threads", type=int, nargs="+", default=[16])
    parser.add_argument("--degrees", type=int, nargs="+", choices=[512, 1024], default=[512, 1024])
    parser.add_argument("--seeds", type=int, nargs="+", default=[42, 43, 44])
    parser.add_argument("--batch", type=int, default=1024)
    parser.add_argument("--security", type=int, default=128)
    parser.add_argument("--case-cache", type=Path, default=ROOT / ".tmp/falcon-poc-performance-cases-20261012")
    parser.add_argument("--order-seed", type=int, default=20261012)
    parser.add_argument("--timeout", type=int, default=600)
    parser.add_argument("--summarize-only", action="store_true")
    args = parser.parse_args()
    out = args.output.resolve()
    if not args.summarize_only:
        manifest = build_manifest(args)
        manifest_path = out / "phases" / f"{args.phase}.json"
        manifest_path.parent.mkdir(parents=True, exist_ok=True)
        if manifest_path.exists():
            if json.loads(manifest_path.read_text()) != manifest:
                raise ValueError("phase configuration/binary/runner changed; use a new phase")
        else:
            write_json(manifest_path, manifest)
        manifest_hash = file_hash(manifest_path)
        for job in manifest["jobs"]:
            directory = out / "runs" / args.phase / job["id"]
            result_path = directory / "result.json"
            if result_path.exists():
                validate_result(result_path, job, manifest, manifest_hash)
                continue
            directory.mkdir(parents=True, exist_ok=True)
            command = command_record(job, manifest)
            write_json(directory / "command.json", command)
            env = {key: value for key, value in os.environ.items()
                   if not key.startswith(("BITZ_", "F2_", "OBLONG_"))
                   and key not in ["RAYON_NUM_THREADS", "RUSTFLAGS", "CARGO_ENCODED_RUSTFLAGS", "FZF_DEFAULT_OPTS"]}
            env.update(command["overrides"])
            print(f"RUN {args.phase}/{job['id']}", flush=True)
            start = time.time()
            with (directory / "stdout.jsonl").open("w") as stdout, (directory / "stderr.log").open("w") as stderr:
                code, timed_out = run_process(command["command"], env=env, cwd=ROOT,
                                              stdout=stdout, stderr=stderr, timeout=args.timeout)
            write_json(directory / "process.json", dict(start_unix=start, elapsed_seconds=time.time() - start,
                                                       exit_code=code, timed_out=timed_out))
            if code != 0 or timed_out:
                raise RuntimeError(f"failed {directory}; inspect stderr.log")
            trials = normalize(read_jsonl(directory / "stdout.jsonl"), job, manifest)
            result = dict(schema=SCHEMA, job=job, trials=trials, verified=True, manifest_sha256=manifest_hash)
            for filename, key in [("command.json", "command_sha256"), ("stdout.jsonl", "stdout_sha256"),
                                  ("stderr.log", "stderr_sha256"), ("process.json", "process_sha256")]:
                result[key] = file_hash(directory / filename)
            write_json(result_path, result)
            print(f"OK {job['id']}", flush=True)
    results, validation = validate_all(out)
    print(json.dumps(validation, sort_keys=True), flush=True)
    summarize(out, results)


if __name__ == "__main__":
    main()

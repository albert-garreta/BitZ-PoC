#!/usr/bin/env python3
"""Verified Falcon binder experiments; see binder_experiments.md for examples."""
from __future__ import annotations

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

from bench_statistics import classify_interval, paired_interval, sample_statistics
from bench_support import (
    ROOT, clean_environment, cpu_name, file_hash, run_process, write_csv, write_json,
)

SCHEMA = "bitz/falcon-binder-experiments/v1"
CPUS = [6, 7, 4, 5, 2, 3, 0, 1, 14, 15, 12, 13, 10, 11, 8, 9]
METRICS = ["total_prover_ms", "prove_ms", "witness_commit_ms", "verify_ms",
           "process_peak_rss_kib", "proof_payload_bytes"]


def comma_values(value, convert=str):
    values = [convert(part) for part in value.split(",")]
    if not values or len(set(values)) != len(values):
        raise ValueError(f"expected a nonempty list without duplicates: {value}")
    return values


def default_configurations():
    configs = []
    for variant in ["arithmetic", "full"]:
        for name, binary in [("baseline", "baseline"), ("auto", "candidate")]:
            configs.append(dict(name=name, variant=variant, binary=binary))
    for bits in itertools.product([0, 1], repeat=3):
        configs.append(dict(name="r" + "".join(map(str, bits)), variant="arithmetic",
                            binary="candidate", **dict(zip(
                                ["replay", "construction", "evaluation"],
                                ["on" if bit else "off" for bit in bits]))))
    for state in ["off", "on"]:
        configs.append(dict(name=f"replay-{state}", variant="full", binary="candidate",
                            replay=state))
    configs.append(dict(name="word", variant="arithmetic", binary="candidate", word="on",
                        replay="on", construction="on", evaluation="on"))
    return configs


def configurations(args):
    configs = (json.loads(args.config_file.read_text()) if args.config_file
               else default_configurations())
    selected = set(comma_values(args.configs)) if args.configs else None
    if selected and selected - {c["name"] for c in configs}:
        raise ValueError(f"unknown configurations: {sorted(selected - {c['name'] for c in configs})}")
    output = []
    for raw in configs:
        if raw["variant"] not in args.variants:
            continue
        if selected is not None and raw["name"] not in selected:
            continue
        if selected is None and not args.config_file and raw["name"] == "word":
            continue
        allowed = {"name", "variant", "binary", "replay", "construction", "evaluation",
                   "word", "group_cap", "threads"}
        if raw.keys() - allowed:
            raise ValueError(f"unknown configuration keys: {raw.keys() - allowed}")
        config = dict(replay="auto", construction="auto", evaluation="auto", word="off",
                      group_cap="auto")
        config.update(raw)
        if not config["name"] or any(c not in "abcdefghijklmnopqrstuvwxyz0123456789-_" for c in config["name"]):
            raise ValueError("configuration names must contain lowercase letters, digits, - or _")
        for key in ["replay", "construction", "evaluation", "word"]:
            if config[key] not in ["auto", "on", "off"]:
                raise ValueError(f"invalid {key} in {config['name']}")
        if config["word"] == "on" and config["variant"] != "arithmetic":
            raise ValueError("the word binder applies only to arithmetic")
        if config["binary"] not in ["baseline", "candidate"]:
            raise ValueError("binary must be baseline or candidate")
        if config["binary"] == "baseline" and any(config[k] != "auto" for k in ["replay", "construction", "evaluation"]):
            raise ValueError("baseline configurations must retain their original kernel policy")
        if config["binary"] == "baseline" and config["word"] != "off":
            raise ValueError("baseline configurations cannot select the word binder")
        if config["group_cap"] != "auto" and (type(config["group_cap"]) is not int or config["group_cap"] <= 0):
            raise ValueError("group_cap must be auto or a positive integer")
        if "threads" in config and (not config["threads"] or any(type(t) is not int or t <= 0 for t in config["threads"])):
            raise ValueError("configuration threads must be a nonempty positive integer list")
        output.append(config)
    if not output or len({(c["variant"], c["name"]) for c in output}) != len(output):
        raise ValueError("configurations must be nonempty and unique per variant")
    return output


def build_manifest(args):
    configs = configurations(args)
    binaries = {}
    for config in configs:
        key = f"{config['binary']}_{config['variant']}"
        if key in binaries:
            continue
        path = getattr(args, key)
        if path is None or not path.is_file() or not os.access(path, os.X_OK):
            raise ValueError(f"--{key.replace('_', '-')} must name an executable")
        path = path.resolve()
        binaries[key] = dict(path=str(path), sha256=file_hash(path))
    jobs = []
    rng = random.Random(args.order_seed)
    blocks = list(itertools.product(args.variants, args.threads, args.seeds, range(args.repetitions)))
    rng.shuffle(blocks)
    for variant, threads, seed, repetition in blocks:
        group = [c for c in configs if c["variant"] == variant and threads in c.get("threads", args.threads)]
        rng.shuffle(group)
        for config in group:
            name = f"{variant}-t{threads}-s{seed}-r{repetition}-{config['name']}"
            jobs.append(dict(id=name, variant=variant, threads=threads, seed=seed,
                             repetition=repetition, config=config))
    if not jobs:
        raise ValueError("configuration filters selected no jobs")
    return dict(schema=SCHEMA, binaries=binaries, configs=configs, jobs=jobs,
                degree=args.degree, batch=args.batch, security=args.security,
                iterations=args.iterations, warmup=args.warmup, trace=args.trace,
                worker_cpus=args.worker_cpus, main_cpu=args.main_cpu, order_seed=args.order_seed,
                case_cache=str(args.case_cache.resolve()), cwd=str(ROOT), timeout=args.timeout,
                cpu=cpu_name(), allowed_cpus=sorted(os.sched_getaffinity(0)),
                runner_sha256=file_hash(Path(__file__)),
                helper_sha256={name: file_hash(Path(__file__).with_name(name))
                               for name in ["bench_statistics.py", "bench_support.py"]})


def read_jsonl(path):
    events = []
    for number, line in enumerate(path.read_text().splitlines(), 1):
        if not line.strip():
            continue
        try:
            event = json.loads(line)
        except json.JSONDecodeError as error:
            raise ValueError(f"{path}:{number}: non-JSON benchmark output") from error
        if not isinstance(event, dict):
            raise ValueError(f"{path}:{number}: expected JSON object")
        events.append(event)
    return events


def digest(value, name):
    if isinstance(value, list) and all(type(x) is int and 0 <= x < 256 for x in value):
        value = bytes(value).hex()
    if not isinstance(value, str) or len(value) != 64 or any(c not in "0123456789abcdef" for c in value):
        raise ValueError(f"missing or malformed {name}")
    return value


def normalize_trials(events, job, manifest):
    prepared = next((e for e in events if e.get("event") == "prepared"), {})
    trials = [e for e in events if e.get("trial") in ["sample", "warmup"]]
    samples = []
    counts = defaultdict(int)
    for event in trials:
        if event.get("verified") is not True:
            raise ValueError(f"unverified {event.get('trial')} in {job['id']}")
        for key, expected in [("degree", manifest["degree"]), ("batch", manifest["batch"]),
                              ("threads", job["threads"])]:
            if event.get(key) != expected:
                raise ValueError(f"{key} mismatch in {job['id']}")
        if event.get("security_bits", event.get("security_target")) != manifest["security"]:
            raise ValueError(f"security mismatch in {job['id']}")
        if event.get("seed", prepared.get("input_seed")) != job["seed"]:
            raise ValueError(f"input seed mismatch in {job['id']}")
        if event.get("auxiliary_pool_threads", job["threads"]) != job["threads"]:
            raise ValueError(f"auxiliary worker count mismatch in {job['id']}")
        affinity = event.get("cpu_affinity", prepared.get("cpu_affinity"))
        expected_workers = manifest["worker_cpus"][:job["threads"]]
        if (not affinity or affinity.get("observed_worker_cpus") != [[x] for x in expected_workers]
                or affinity.get("observed_main_cpus") != [manifest["main_cpu"]]
                or affinity.get("observed_auxiliary_worker_cpus") != [[manifest["main_cpu"]]] * job["threads"]):
            raise ValueError(f"CPU affinity readback mismatch in {job['id']}")
        if job["variant"] == "full" and (event.get("flock_verifier_affinity") or {}).get("observed_cpus") != [manifest["main_cpu"]]:
            raise ValueError(f"verifier CPU affinity readback mismatch in {job['id']}")
        protocol = event.get("protocol_id", event.get("protocol", prepared.get("protocol_id", prepared.get("protocol"))))
        if not isinstance(protocol, str) or not protocol:
            raise ValueError(f"missing protocol ID in {job['id']}")
        sample = dict(trial=event["trial"], protocol_id=protocol,
                      input_digest=digest(event.get("input_digest"), "input_digest"),
                      source_root=digest(event.get("source_root"), "source_root"),
                      proof_debug_digest=digest(event.get("proof_debug_digest"), "proof_debug_digest"))
        for key in METRICS:
            alternative = {"prove_ms": "proof_prove_ms", "verify_ms": "proof_verify_ms"}.get(key, key)
            value = event.get(key, event.get(alternative))
            if not isinstance(value, (int, float)) or not math.isfinite(value) or value <= 0:
                raise ValueError(f"missing/invalid {key} in {job['id']}")
            sample[key] = value
        samples.append(sample)
        counts[event["trial"]] += 1
    if counts["sample"] != manifest["iterations"] or counts["warmup"] != manifest["warmup"]:
        raise ValueError(f"trial count mismatch in {job['id']}: {dict(counts)}")
    return samples


def check_equivalence(results):
    sources, proofs = {}, {}
    bit_protocols, word_protocols = defaultdict(set), defaultdict(set)
    for result in results:
        job = result["job"]
        key = (job["variant"], job["seed"])
        for sample in result["trials"]:
            protocol = sample["protocol_id"]
            proof_key = (*key, protocol)
            source = (sample["input_digest"], sample["source_root"])
            proof = (sample["proof_debug_digest"], sample["proof_payload_bytes"])
            if sources.setdefault(key, source) != source:
                raise ValueError(f"input/source commitment mismatch in {job['id']}")
            if proofs.setdefault(proof_key, proof) != proof:
                raise ValueError(f"complete proof contents mismatch in {job['id']}")
            if job["config"]["word"] == "off":
                bit_protocols[key].add(protocol)
            if job["config"]["word"] == "on":
                word_protocols[key].add(protocol)
    for key in sources:
        bits, words = bit_protocols[key], word_protocols[key]
        if len(bits) > 1 or len(words) > 1:
            raise ValueError(f"protocol ID mismatch for explicitly selected binder: {key}")
        if bits and words and (bits == words or proofs[(*key, next(iter(bits)))][0] == proofs[(*key, next(iter(words)))][0]):
            raise ValueError(f"word binder must report its distinct protocol and proof digest: {key}")


def completed_results(output, manifest):
    results = []
    for job in manifest["jobs"]:
        path = output / "runs" / job["id"] / "result.json"
        if path.exists():
            result = json.loads(path.read_text())
            if result.get("job") != job or result.get("verified") is not True:
                raise ValueError(f"invalid completed result: {path}")
            stdout = path.parent / result["attempt"] / "stdout.jsonl"
            if file_hash(stdout) != result["stdout_sha256"]:
                raise ValueError(f"changed raw benchmark output: {stdout}")
            result["trials"] = normalize_trials(read_jsonl(stdout), job, manifest)
            results.append(result)
    check_equivalence(results)
    return results


def execute_job(output, manifest, job):
    directory = output / "runs" / job["id"]
    directory.mkdir(parents=True, exist_ok=True)
    attempt = 1
    while (directory / f"attempt-{attempt:03}").exists():
        attempt += 1
    directory = directory / f"attempt-{attempt:03}"
    directory.mkdir()
    config = job["config"]
    binary = manifest["binaries"][f"{config['binary']}_{job['variant']}"]
    if file_hash(binary["path"]) != binary["sha256"]:
        raise ValueError("benchmark executable changed during experiment")
    command = [binary["path"], "--degree", str(manifest["degree"]), "--batch", str(manifest["batch"]),
               "--security", str(manifest["security"]), "--threads", str(job["threads"]),
               "--seed", str(job["seed"]), "--iterations", str(manifest["iterations"]),
               "--warmup", str(manifest["warmup"])]
    overrides = {
        "RAYON_NUM_THREADS": str(job["threads"]),
        "BITZ_FALCON_WORKER_CPUS": ",".join(map(str, manifest["worker_cpus"][:job["threads"]])),
        "BITZ_FALCON_MAIN_CPU": str(manifest["main_cpu"]),
        "BITZ_FALCON_CASE_CACHE": manifest["case_cache"],
        "BITZ_BENCH_WORD_BINDER": config["word"],
        "BITZ_BENCH_BINDER_GROUP_CAP": str(config["group_cap"]),
    }
    for key in ["replay", "construction", "evaluation"]:
        overrides[f"BITZ_BENCH_BINDER_{key.upper()}"] = config[key]
    if manifest["trace"]:
        if job["variant"] == "arithmetic":
            command.append("--trace")
        else:
            overrides["BITZ_FALCON_STAGE_TIMINGS"] = "1"
    env = clean_environment(os.environ)
    env.update(overrides)
    write_json(directory / "command.json", dict(command=command, overrides=overrides, binary=binary))
    started = time.time()
    print(f"run {job['id']} ({directory.name})", flush=True)
    with (directory / "stdout.jsonl").open("w") as stdout, (directory / "stderr.log").open("w") as stderr:
        code, timeout = run_process(command, env=env, cwd=manifest["cwd"], stdout=stdout,
                                    stderr=stderr, timeout=manifest["timeout"])
    process = dict(exit_code=code, timed_out=timeout, started_unix=started, wall_seconds=time.time() - started)
    write_json(directory / "process.json", process)
    if code != 0 or timeout:
        raise RuntimeError(f"failed {job['id']}; inspect {directory}")
    events = read_jsonl(directory / "stdout.jsonl")
    trials = normalize_trials(events, job, manifest)
    stages = []
    for line in (directory / "stderr.log").read_text().splitlines():
        if line.startswith("{"):
            event = json.loads(line)
            if event.get("event") == "stage":
                stages.append(event)
    with (directory / "stages.jsonl").open("w") as stream:
        for stage in stages:
            stream.write(json.dumps(stage) + "\n")
    return dict(job=job, verified=True, attempt=directory.name, trials=trials, process=process,
                stdout_sha256=file_hash(directory / "stdout.jsonl"), stage_events=len(stages))


def summarize(output, manifest, results):
    groups = defaultdict(list)
    by_block = {}
    for result in results:
        job = result["job"]
        samples = [s for s in result["trials"] if s["trial"] == "sample"]
        summary = {key: statistics.median(s[key] for s in samples) for key in METRICS}
        # RSS observations are cumulative high-water marks. Retain the largest
        # observation, including warmup, rather than the middle sample's peak.
        summary["process_peak_rss_kib"] = max(s["process_peak_rss_kib"] for s in result["trials"])
        group = (job["variant"], job["threads"], job["config"]["name"])
        block = (job["variant"], job["threads"], job["seed"], job["repetition"])
        groups[group].append((block, summary))
        by_block[(*block, job["config"]["name"])] = summary
    rows = []
    for (variant, threads, config), blocks in sorted(groups.items()):
        row = dict(variant=variant, threads=threads, config=config, processes=len(blocks), trace=manifest["trace"])
        for metric in METRICS:
            stats = sample_statistics([values[metric] for _, values in blocks])
            row[f"median_{metric}"] = stats["median"]
            pairs = [(by_block[(*block, "baseline")][metric], values[metric])
                     for block, values in blocks if (*block, "baseline") in by_block]
            row["matched_processes"] = len(pairs)
            if pairs:
                ratios = [candidate / baseline for baseline, candidate in pairs]
                ratio = math.exp(statistics.mean(math.log(x) for x in ratios))
                row[f"ratio_{metric}"] = ratio
                if metric.endswith("_ms"):
                    row[f"speedup_percent_{metric}"] = 100 * (1 - ratio)
                if metric == "total_prover_ms":
                    interval = paired_interval(ratios) if len(ratios) >= 2 else None
                    row["total_ratio_ci95_low"] = interval[0] if interval else None
                    row["total_ratio_ci95_high"] = interval[1] if interval else None
                    row["total_gate"] = classify_interval(interval) if interval else "insufficient-samples"
        rows.append(row)
    write_json(output / "summary.json", dict(schema=SCHEMA, completed=len(results), expected=len(manifest["jobs"]),
               rows=rows, notes=["Ratios are candidate/baseline, paired by variant, threads, seed and repetition.",
               "Each process contributes one timing median; trial iterations are not independent bootstrap units.",
               "RSS uses the maximum observed process high-water mark across warmup and sample trials, including fixture setup and proof digest formatting.",
               "Bootstrap intervals are descriptive; use independent holdout seeds to qualify selected policies.",
               "Trace timings overlap; use separate campaigns for diagnostics and latency acceptance."]))
    write_csv(output / "summary.csv", rows)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    run = commands.add_parser("run", help="Run or resume a fixed manifest; see binder_experiments.md")
    for role, variant in itertools.product(["baseline", "candidate"], ["arithmetic", "full"]):
        run.add_argument(f"--{role}-{variant}", type=Path)
    run.add_argument("--output", type=Path, required=True)
    run.add_argument("--case-cache", type=Path)
    run.add_argument("--variants", default="arithmetic,full")
    run.add_argument("--threads", default="1,4,8,16")
    run.add_argument("--seeds", default="42,43,44")
    run.add_argument("--configs", help="Selected named configurations; default screens all bit kernels")
    run.add_argument("--config-file", type=Path, help="JSON configuration list; replaces built-ins")
    run.add_argument("--degree", type=int, choices=[512, 1024], default=512)
    run.add_argument("--batch", type=int, default=1024)
    run.add_argument("--security", type=int, choices=[100, 128], default=128)
    run.add_argument("--repetitions", type=int, default=1)
    run.add_argument("--iterations", type=int, default=1)
    run.add_argument("--warmup", type=int, default=1)
    run.add_argument("--order-seed", type=int, default=20261010)
    run.add_argument("--worker-cpus", default=",".join(map(str, CPUS)))
    run.add_argument("--main-cpu", type=int, default=6)
    run.add_argument("--timeout", type=float, default=1800)
    run.add_argument("--trace", action="store_true")
    summary = commands.add_parser("summarize", help="Revalidate raw completed output and summarize matched results")
    summary.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    args.output = args.output.resolve()
    if args.command == "summarize":
        manifest = json.loads((args.output / "manifest.json").read_text())
        if (manifest["runner_sha256"] != file_hash(Path(__file__))
                or any(expected != file_hash(Path(__file__).with_name(name))
                       for name, expected in manifest["helper_sha256"].items())):
            raise ValueError("runner or helpers changed; restore the recorded versions before summarizing")
        results = completed_results(args.output, manifest)
        summarize(args.output, manifest, results)
        return
    args.variants = comma_values(args.variants)
    if set(args.variants) - {"arithmetic", "full"}:
        raise ValueError("variants must be arithmetic and/or full")
    args.threads = comma_values(args.threads, int)
    args.seeds = comma_values(args.seeds, int)
    args.worker_cpus = comma_values(args.worker_cpus, int)
    if (min(args.threads) <= 0 or max(args.threads) > len(args.worker_cpus)
            or args.warmup < 0 or min(args.iterations, args.repetitions) < 1
            or not 1 <= args.batch <= 1024 or min(args.seeds) < 0
            or not math.isfinite(args.timeout) or args.timeout <= 0):
        raise ValueError("invalid experiment dimensions or timeout")
    if not set(args.worker_cpus[:max(args.threads)] + [args.main_cpu]) <= os.sched_getaffinity(0):
        raise ValueError("requested CPUs are outside the allowed affinity mask")
    args.output.mkdir(parents=True, exist_ok=True)
    args.case_cache = args.case_cache or args.output / "case-cache"
    args.case_cache.mkdir(parents=True, exist_ok=True)
    manifest = build_manifest(args)
    manifest_path = args.output / "manifest.json"
    if manifest_path.exists():
        if json.loads(manifest_path.read_text()) != manifest:
            raise ValueError("manifest changed; use a new output directory (binary hashes/settings must match)")
    else:
        write_json(manifest_path, manifest)
    results = completed_results(args.output, manifest)
    complete = {r["job"]["id"] for r in results}
    for index, job in enumerate(manifest["jobs"], 1):
        if job["id"] in complete:
            print(f"resume verified {index}/{len(manifest['jobs'])}: {job['id']}", flush=True)
            continue
        result = execute_job(args.output, manifest, job)
        check_equivalence([*results, result])
        write_json(args.output / "runs" / job["id"] / "result.json", result)
        results.append(result)
        print(f"verified {index}/{len(manifest['jobs'])}: {job['id']}", flush=True)
    summarize(args.output, manifest, results)


if __name__ == "__main__":
    try:
        main()
    except (OSError, ValueError, RuntimeError) as error:
        print(f"binder experiment error: {error}", file=sys.stderr)
        sys.exit(1)

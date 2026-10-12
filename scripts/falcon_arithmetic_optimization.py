#!/usr/bin/env python3
"""Run sequential, pinned, proof-checked arithmetic Falcon optimization trials."""

import argparse
import itertools
import json
import math
import os
import random
import statistics
import time
from collections import defaultdict
from pathlib import Path

from bench_support import file_hash, run_process, write_json
from binder_experiments import digest, normalize_trials, read_jsonl

ROOT = Path(__file__).resolve().parents[1]
CPUS = [6, 7, 4, 5, 2, 3, 0, 1, 14, 15, 12, 13, 10, 11, 8, 9]
BASE = {
    "BITZ_ARITHMETIC_BINDING_OWNED": "off",
    "BITZ_ARITHMETIC_WORD_INITIAL": "off",
    "BITZ_ARITHMETIC_WORD_FUSION": "off",
    "BITZ_ARITHMETIC_NORM_FACTORED": "off",
    "BITZ_ARITHMETIC_PUBLIC_EVAL": "legacy",
    "BITZ_ARITHMETIC_RECURRENCE_PAIRS": "4",
    "BITZ_ARITHMETIC_CARRY_FUSION": "off",
    "BITZ_ARITHMETIC_RING": "canonical",
}
VARIANTS = {
    "control": {},
    "owned": {"BITZ_ARITHMETIC_BINDING_OWNED": "on"},
    "initial": {"BITZ_ARITHMETIC_WORD_INITIAL": "on"},
    "fusion": {"BITZ_ARITHMETIC_WORD_FUSION": "on"},
    "norm": {"BITZ_ARITHMETIC_NORM_FACTORED": "on"},
    "public": {"BITZ_ARITHMETIC_PUBLIC_EVAL": "paired"},
    "pairs2": {"BITZ_ARITHMETIC_RECURRENCE_PAIRS": "2"},
    "pairs8": {"BITZ_ARITHMETIC_RECURRENCE_PAIRS": "8"},
    "carries": {"BITZ_ARITHMETIC_CARRY_FUSION": "on"},
    "factored": {"BITZ_ARITHMETIC_RING": "factored"},
}


def validate_all(out):
    proofs, sources, counts = {}, {}, defaultdict(int)
    payloads = {"canonical": {}, "factored": {}}
    for path in sorted(out.glob("runs/*/*/result.json")):
        result = validate_result(path)
        command_record = json.loads((path.parent / "command.json").read_text())
        ring_mode = command_record["overrides"]["BITZ_ARITHMETIC_RING"]
        if ring_mode not in payloads:
            raise ValueError(f"unknown ring mode in {path.parent}")
        job = result["job"]
        for trial in result["trials"]:
            source_key = (result["degree"], result["batch"], result["security"], job["seed"])
            source = (trial["input_digest"], trial["source_root"])
            assert sources.setdefault(source_key, source) == source, (path, "input/source mismatch")
            proof_key = (*source_key, trial["protocol_id"])
            proof = (trial["proof_debug_digest"], trial["proof_payload_bytes"])
            assert proofs.setdefault(proof_key, proof) == proof, (path, "complete proof mismatch")
            size = trial["proof_payload_bytes"]
            if payloads[ring_mode].setdefault(source_key, size) != size:
                raise ValueError(f"inconsistent {ring_mode} proof size for {source_key}")
            counts[trial["protocol_id"]] += 1
    growth = []
    for key, size in payloads["factored"].items():
        if key not in payloads["canonical"]:
            raise ValueError(f"missing matching canonical proof for factored case {key}")
        canonical = payloads["canonical"][key]
        if size * 10 > canonical * 11:
            raise ValueError(f"factored proof exceeds 110% payload limit for {key}: {size} vs {canonical}")
        growth.append((size / canonical - 1) * 100)
    validation = dict(verified_proofs=sum(counts.values()), proofs_by_protocol=dict(counts),
                      input_source_equality=True, complete_proof_equality_within_protocol=True,
                      raw_log_hashes_verified=True, command_provenance_verified=True,
                      experimental_payload_limit_percent=110,
                      experimental_payload_comparisons=len(growth),
                      max_experimental_payload_growth_percent=max(growth, default=None))
    write_json(out / "validation.json", validation)
    return validation


def validate_result(path, expected_command=None):
    """Reconstruct saved trials from their original command and raw output."""
    result = json.loads(path.read_text())
    command_path = path.parent / "command.json"
    command_record = json.loads(command_path.read_text())
    digest(command_record.get("binary_sha256"), "binary_sha256")
    command_hash = file_hash(command_path)
    if expected_command is not None and command_record != expected_command:
        raise ValueError(f"stale benchmark configuration in {path.parent}; use a new phase")
    if "command_sha256" in result and result["command_sha256"] != command_hash:
        raise ValueError(f"command hash mismatch in {path.parent}")
    if result.get("verified") is not True:
        raise ValueError(f"unverified result in {path.parent}")
    for name, key in [("stdout.jsonl", "stdout_sha256"), ("stderr.log", "stderr_sha256")]:
        if file_hash(path.parent / name) != result[key]:
            raise ValueError(f"{name} hash mismatch in {path.parent}")
    process = json.loads((path.parent / "process.json").read_text())
    if process.get("exit_code") != 0 or process.get("timed_out") is not False:
        raise ValueError(f"unsuccessful benchmark process in {path.parent}")
    command = command_record["command"]
    overrides = command_record["overrides"]
    def argument(name):
        if command.count(name) != 1:
            raise ValueError(f"missing or repeated {name} in {path.parent}")
        return int(command[command.index(name) + 1])
    manifest = dict(degree=argument("--degree"), batch=argument("--batch"),
                    security=argument("--security"), warmup=argument("--warmup"),
                    iterations=argument("--iterations"),
                    worker_cpus=[int(cpu) for cpu in overrides["BITZ_FALCON_WORKER_CPUS"].split(",")],
                    main_cpu=int(overrides["BITZ_FALCON_MAIN_CPU"]))
    job = result["job"]
    expected_job = dict(id=path.parent.name, variant="arithmetic", config=job["config"],
                        threads=argument("--threads"), seed=argument("--seed"))
    expected_id = (f"{job['config']}-n{manifest['degree']}-b{manifest['batch']}-s{manifest['security']}"
                   f"-t{expected_job['threads']}-seed{expected_job['seed']}")
    if (job != expected_job or path.parent.name != expected_id
            or result["phase"] != path.parent.parent.name
            or any(result[key] != manifest[key] for key in ["degree", "batch", "security"])
            or overrides["RAYON_NUM_THREADS"] != str(expected_job["threads"])
            or len(manifest["worker_cpus"]) != expected_job["threads"]):
        raise ValueError(f"command/result metadata mismatch in {path.parent}")
    events = read_jsonl(path.parent / "stdout.jsonl")
    normalized = normalize_trials(events, expected_job, manifest)
    if normalized != result["trials"]:
        raise ValueError(f"saved trials differ from raw output in {path.parent}")
    for event in events:
        if event.get("trial") in ["warmup", "sample"]:
            if sum(dict(event["proof_payload_breakdown"]).values()) != event["proof_payload_bytes"]:
                raise ValueError(f"proof payload breakdown mismatch in {path.parent}")
            security = event.get("algebraic_security_bits")
            if (type(security) not in [int, float] or not math.isfinite(security)
                    or security < manifest["security"]):
                raise ValueError(f"algebraic security is below the requested target in {path.parent}")
    if "command_sha256" not in result:
        # Legacy campaigns recorded the command before execution but did not bind
        # its hash into result.json. Migrate only after rebuilding every trial.
        result["command_sha256"] = command_hash
        result["provenance_migration"] = "command-and-raw-output-revalidated-v1"
        write_json(path, result)
        print(f"MIGRATED command provenance {path.parent}", flush=True)
    return result


def summarize(out):
    groups = defaultdict(list)
    seed_groups = defaultdict(list)
    for path in out.glob("runs/*/*/result.json"):
        result = json.loads(path.read_text())
        job = result["job"]
        key = (result["phase"], job["config"], result["degree"], result["batch"], result["security"], job["threads"])
        for trial in result["trials"]:
            if trial["trial"] == "sample":
                groups[key].append(trial)
                seed_groups[(*key, job["seed"])].append(trial)
    metrics = ["witness_commit_ms", "prove_ms", "total_prover_ms", "verify_ms", "process_peak_rss_kib", "proof_payload_bytes"]
    rows = []
    for key, trials in sorted(groups.items()):
        phase, variant, degree, batch, security, threads = key
        row = dict(phase=phase, variant=variant, degree=degree, batch=batch, security=security,
                   threads=threads, samples=len(trials),
                   medians={m: statistics.median(t[m] for t in trials) for m in metrics},
                   seed_medians={str(seed): {m: statistics.median(t[m] for t in values) for m in metrics}
                                 for (*base, seed), values in seed_groups.items() if tuple(base) == key})
        rows.append(row)
    write_json(out / "summary.json", dict(
        rows=rows,
        notes="Smoke and correctness phases validate proofs only; their timings are not performance evidence. Profile phases are traced and kept separate from untraced latency runs.",
    ))
    for row in rows:
        print(f"{row['phase']:14} {row['variant']:12} N={row['degree']:4} t={row['threads']:2} "
              f"total={row['medians']['total_prover_ms']:.3f} prove={row['medians']['prove_ms']:.3f} "
              f"verify={row['medians']['verify_ms']:.3f} rss={row['medians']['process_peak_rss_kib']:.0f}", flush=True)
    summarize_profiles(out)


def summarize_profiles(out):
    """Associate nested spans with each proving call; exclude warmups/verifying."""
    groups = defaultdict(list)
    for path in out.glob("runs/*/*/result.json"):
        result = json.loads(path.read_text())
        command = json.loads((path.parent / "command.json").read_text())["command"]
        if "--trace" not in command:
            continue
        warmup = int(command[command.index("--warmup") + 1])
        events = []
        for line in (path.parent / "stderr.log").read_text().splitlines():
            try:
                event = json.loads(line)
            except ValueError:
                continue
            if event.get("event") == "stage":
                events.append(event)
        roots = [e["span_id"] for e in events if e["name"] == "falcon_algebraic:prove"]
        assert len(roots) == len(result["trials"]), path
        for root in roots[warmup:]:
            values = defaultdict(float)
            for event in events:
                if root not in event["ancestor_ids"]:
                    continue
                values[event["name"]] += event["elapsed_ms"]
                for name, value in event["fields"].items():
                    if name.endswith("_ns"):
                        values[f"{event['name']}/{name}"] += value / 1e6
            job = result["job"]
            groups[(result["phase"], job["config"], result["degree"], result["batch"],
                    result["security"], job["threads"])].append(values)
    rows = []
    for (phase, variant, degree, batch, security, threads), samples in sorted(groups.items()):
        keys = set().union(*(s.keys() for s in samples))
        rows.append(dict(phase=phase, variant=variant, degree=degree, batch=batch,
                         security=security, threads=threads,
                         samples=len(samples), medians_ms={k: statistics.median(s.get(k, 0) for s in samples)
                                                           for k in sorted(keys)}))
    write_json(out / "profile-summary.json", dict(rows=rows,
               note="Nested wall times overlap; *_worker_ns entries are summed worker time, converted to ms."))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--binary", type=Path)
    parser.add_argument("--baseline", type=Path)
    parser.add_argument("--phase", default="screen")
    parser.add_argument("--variants", nargs="+", default=list(VARIANTS))
    parser.add_argument("--configs", type=Path, help="Additional named environment overrides")
    parser.add_argument("--threads", type=int, nargs="+", default=[16])
    parser.add_argument("--degrees", type=int, nargs="+", default=[512, 1024])
    parser.add_argument("--seeds", type=int, nargs="+", default=[42, 43, 44])
    parser.add_argument("--batch", type=int, default=1024)
    parser.add_argument("--security", type=int, default=128)
    parser.add_argument("--warmup", type=int, default=2)
    parser.add_argument("--iterations", type=int, default=7)
    parser.add_argument("--trace", action="store_true")
    parser.add_argument("--summarize-only", action="store_true")
    args = parser.parse_args()
    out = args.output.resolve()
    out.mkdir(parents=True, exist_ok=True)
    if args.summarize_only:
        print(validate_all(out))
        summarize(out)
        return
    if not args.binary:
        parser.error("--binary is required for measurements")
    variants = dict(VARIANTS)
    if args.configs:
        variants.update(json.loads(args.configs.read_text()))
    if args.baseline:
        variants["baseline"] = {}
    if set(args.variants) - variants.keys():
        parser.error("unknown variant")
    jobs = list(itertools.product(args.degrees, args.threads, args.seeds))
    random.Random(20261012 + sum(map(ord, args.phase))).shuffle(jobs)
    manifest = dict(warmup=args.warmup, iterations=args.iterations, batch=args.batch, security=args.security,
                    worker_cpus=CPUS, main_cpu=6)
    for degree, threads, seed in jobs:
        order = list(args.variants)
        random.Random(seed * 10000 + degree + threads + sum(map(ord, args.phase))).shuffle(order)
        for config in order:
            directory = out / "runs" / args.phase / f"{config}-n{degree}-b{args.batch}-s{args.security}-t{threads}-seed{seed}"
            binary = (args.baseline if config == "baseline" else args.binary).resolve()
            command = [str(binary), "--degree", str(degree), "--batch", str(args.batch),
                       "--security", str(args.security), "--threads", str(threads), "--seed", str(seed),
                       "--warmup", str(args.warmup), "--iterations", str(args.iterations)]
            overrides = dict(BASE)
            overrides.update(variants[config])
            overrides.update(RAYON_NUM_THREADS=str(threads), BITZ_FALCON_WORKER_CPUS=",".join(map(str, CPUS[:threads])),
                             BITZ_FALCON_MAIN_CPU="6", BITZ_FALCON_CASE_CACHE=str(ROOT / ".tmp/falcon-poc-performance-cases-20261012"))
            if args.trace:
                command.append("--trace")
                overrides["BITZ_FALCON_LIGHT_PROFILE"] = "1"
            command_record = dict(command=command, overrides=overrides,
                                  binary_sha256=file_hash(binary), cwd=str(ROOT))
            if (directory / "result.json").exists():
                validate_result(directory / "result.json", expected_command=command_record)
                continue
            directory.mkdir(parents=True, exist_ok=True)
            env = {k: v for k, v in os.environ.items() if not k.startswith(("BITZ_", "F2_", "OBLONG_"))
                   and k not in ["RAYON_NUM_THREADS", "RUSTFLAGS", "CARGO_ENCODED_RUSTFLAGS", "FZF_DEFAULT_OPTS"]}
            env.update(overrides)
            job = dict(id=directory.name, variant="arithmetic", config=config, threads=threads, seed=seed)
            write_json(directory / "command.json", command_record)
            print(f"RUN {args.phase}/{directory.name}", flush=True)
            start = time.time()
            with (directory / "stdout.jsonl").open("w") as stdout, (directory / "stderr.log").open("w") as stderr:
                code, timeout = run_process(command, env=env, cwd=ROOT, stdout=stdout, stderr=stderr, timeout=600)
            write_json(directory / "process.json", dict(start_unix=start, elapsed_seconds=time.time()-start, exit_code=code, timed_out=timeout))
            if code != 0 or timeout:
                raise RuntimeError(f"failed {directory}; inspect stderr.log")
            events = read_jsonl(directory / "stdout.jsonl")
            trials = normalize_trials(events, job, dict(manifest, degree=degree))
            for event in events:
                if event.get("trial") in ["warmup", "sample"]:
                    assert sum(dict(event["proof_payload_breakdown"]).values()) == event["proof_payload_bytes"]
            write_json(directory / "result.json", dict(job=job, phase=args.phase, degree=degree, batch=args.batch,
                       security=args.security, trials=trials, verified=True,
                       command_sha256=file_hash(directory / "command.json"),
                       stdout_sha256=file_hash(directory / "stdout.jsonl"), stderr_sha256=file_hash(directory / "stderr.log")))
            print(f"OK {directory.name}", flush=True)
    print(validate_all(out))
    summarize(out)


if __name__ == "__main__":
    main()

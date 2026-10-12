#!/usr/bin/env python3
"""Sequential, paired Falcon arithmetic binding comparisons.

Use --binary for legacy versus optimized in one bench-internals executable.
For production comparisons supply both variant binaries and set both strategies
to default. Run after builds/tests stop; output must be a fresh subdirectory.
"""

import argparse
import importlib.util
import itertools
import json
import os
from pathlib import Path
import statistics
import subprocess
import sys
import time
from types import SimpleNamespace


HERE = Path(__file__).resolve().parent
REPO = HERE.parent.parent
QUALIFY = HERE.parent / "falcon-speed-integration-20261012/qualify.py"
sys.dont_write_bytecode = True
spec = importlib.util.spec_from_file_location("falcon_qualification", QUALIFY)
q = importlib.util.module_from_spec(spec)
spec.loader.exec_module(q)


def parse_args():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("--binary", type=Path)
    for variant, strategy in (("baseline", "legacy"), ("candidate", "optimized")):
        p.add_argument(f"--{variant}-binary", type=Path)
        p.add_argument(f"--{variant}-strategy", choices=["legacy", "optimized", "default"], default=strategy)
    p.add_argument("--degrees", nargs="+", type=int, choices=[512, 1024], default=[512, 1024])
    p.add_argument("--batches", nargs="+", type=int, default=[1, 3, 32, 513, 1024])
    p.add_argument("--securities", nargs="+", type=int, choices=[100, 128], default=[100, 128])
    p.add_argument("--threads", nargs="+", type=int, default=[16])
    p.add_argument("--seeds", nargs="+", type=int, default=[42, 43, 44])
    p.add_argument("--warmup", type=int, default=1)
    p.add_argument("--iterations", type=int, default=3)
    p.add_argument("--output", type=Path, required=True)
    p.add_argument("--cpus", default="6,7,4,5,2,3,0,1,14,15,12,13,10,11,8,9")
    p.add_argument("--cache", type=Path, default=Path("/tmp/bitz-falcon-speed-case-cache"))
    p.add_argument("--build-description", default="release; features=falcon-hybrid,bench-internals; RUSTFLAGS=-C target-cpu=native")
    p.add_argument("--review-percent", type=float, default=3.0)
    p.add_argument("--summarize-only", action="store_true")
    return p.parse_args()


def main():
    args = parse_args()
    output = args.output.resolve()
    if args.summarize_only:
        q.summarize(output, args.review_percent)
        return
    q.require(output.is_relative_to(HERE) and output != HERE, "output must be a fresh subdirectory beside compare.py")
    q.require(args.iterations > 0 and args.warmup >= 0, "invalid trial counts")
    q.require(all(1 <= batch <= 1024 for batch in args.batches), "invalid batch size")
    cpus = [int(cpu) for cpu in args.cpus.split(",")]
    q.require(len(cpus) == len(set(cpus)) and set(cpus) <= os.sched_getaffinity(0), "invalid worker CPUs")
    q.require(all(1 <= n <= len(cpus) for n in args.threads), "insufficient CPUs")
    for key in ("degrees", "batches", "securities", "threads", "seeds"):
        q.require(len(getattr(args, key)) == len(set(getattr(args, key))), f"duplicate {key}")
    if args.binary:
        q.require(not args.baseline_binary and not args.candidate_binary, "use --binary or both variant binaries")
    else:
        q.require(args.baseline_binary and args.candidate_binary, "supply --binary or both variant binaries")
    binaries = {}
    for variant in ("baseline", "candidate"):
        path = (args.binary or getattr(args, f"{variant}_binary")).resolve()
        q.require(path.is_file() and os.access(path, os.X_OK), f"not executable: {path}")
        binaries[variant] = {"path": str(path), "sha256": q.sha256(path),
                             "strategy": getattr(args, f"{variant}_strategy")}
    cells = [dict(mode="arithmetic", degree=n, batch=batch, security_bits=security, threads=workers)
             for n, batch, security, workers in itertools.product(args.degrees, args.batches, args.securities, args.threads)]
    cleared = sorted(key for key in os.environ if key.startswith(("BITZ_", "RAYON_", "OMP_")) or key == "RUSTFLAGS")
    base_env = {key: value for key, value in os.environ.items() if key not in cleared}
    output.mkdir(parents=True, exist_ok=True)
    q.require(not any(output.iterdir()), "output directory must be empty")
    raw = output / "raw"
    raw.mkdir()
    metadata = {
        "configuration": {"cells": cells, "seeds": args.seeds, "warmup": args.warmup, "iterations": args.iterations,
                          "cpus": cpus, "cache": str(args.cache.resolve()), "binaries": binaries,
                          "build_description": args.build_description,
                          "ordering": "Sequential; baseline first when (cell_index + seed_index) is even, candidate first otherwise."},
        "started_unix": time.time(), "runner_sha256": q.sha256(Path(__file__)), "qualification_sha256": q.sha256(QUALIFY),
        "source_revision": subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=REPO, text=True).strip(),
        "source_status": subprocess.check_output(["git", "status", "--short", "--untracked-files=no"], cwd=REPO, text=True),
        "platform": subprocess.check_output(["uname", "-a"], text=True).strip(),
        "cpu": subprocess.check_output(["lscpu"], text=True), "limitations": q.LIMITATION,
        "strategy_note": "Forced strategies require bench-internals; production builds ignore this environment override.",
    }
    q.save(output / "metadata.json", metadata)
    completed = 0

    def run(command, configured, stem, binary, prime=False):
        q.require(q.sha256(Path(binary["path"])) == binary["sha256"], "binary contents changed")
        command_meta = {"argv": command, "environment": configured, "cleared_environment_names": cleared,
                        "binary_sha256": binary["sha256"], "started_unix": time.time(), "excluded_from_measurements": prime}
        q.save(raw / f"{stem}.command.json", command_meta)
        print(stem, flush=True)
        with (raw / f"{stem}.jsonl").open("w") as stdout, (raw / f"{stem}.stderr").open("w") as stderr:
            result = subprocess.run(command, env={**base_env, **configured}, stdout=stdout, stderr=stderr)
        command_meta.update(finished_unix=time.time(), returncode=result.returncode)
        q.save(raw / f"{stem}.command.json", command_meta)
        q.require(result.returncode == 0, f"benchmark failed: {stem}; inspect stderr")
        counts = SimpleNamespace(warmup=0, iterations=1) if prime else args
        return q.read_run(raw / f"{stem}.jsonl", "arithmetic", cell, seed, counts, selected_cpus)

    try:
        for cell_index, cell in enumerate(cells):
            selected_cpus = cpus[:cell["threads"]]
            cpu_list = ",".join(map(str, selected_cpus))
            for seed_index, seed in enumerate(args.seeds):
                order = ["baseline", "candidate"] if (cell_index + seed_index) % 2 == 0 else ["candidate", "baseline"]
                pair = {"cell": cell, "seed": seed, "order": order, "medians": {}, "logs": {}}
                identities = {}
                for variant in order:
                    binary = binaries[variant]
                    stem = f"arithmetic-n{cell['degree']}-b{cell['batch']}-s{cell['security_bits']}-t{cell['threads']}-seed{seed}-{variant}"
                    command = ["taskset", "-c", cpu_list, binary["path"]]
                    for flag, value in (("degree", cell["degree"]), ("batch", cell["batch"]), ("security", cell["security_bits"]),
                                        ("threads", cell["threads"]), ("seed", seed), ("warmup", args.warmup), ("iterations", args.iterations)):
                        command += [f"--{flag}", str(value)]
                    configured = {"RAYON_NUM_THREADS": str(cell["threads"]), "OMP_NUM_THREADS": str(cell["threads"]),
                                  "BITZ_FALCON_MAIN_CPU": str(cpus[0]), "BITZ_FALCON_WORKER_CPUS": cpu_list,
                                  "BITZ_FALCON_CASE_CACHE": str(args.cache.resolve())}
                    if binary["strategy"] != "default":
                        configured["BITZ_ARITHMETIC_BINDING_STRATEGY"] = binary["strategy"]
                    fixture = args.cache / f"fn-dsa-0.3.0-n{cell['degree']}-b{cell['batch']}-seed{seed}.bin"
                    if not fixture.exists():
                        prime_command = command.copy()
                        prime_command[prime_command.index("--warmup") + 1] = "0"
                        prime_command[prime_command.index("--iterations") + 1] = "1"
                        run(prime_command, configured, stem + ".cache-prime", binary, prime=True)
                        q.require(fixture.exists(), "fixture cache priming did not populate cache")
                    rows, identities[variant] = run(command, configured, stem, binary)
                    samples = [row for row in rows if row["trial"] == "sample"]
                    pair["medians"][variant] = {metric: statistics.median(row[metric] for row in samples) for metric in q.METRICS}
                    pair["logs"][variant] = str(Path("raw") / f"{stem}.jsonl")
                q.require(identities["baseline"] == identities["candidate"],
                          f"protocol/input/build identity mismatch: {cell}, seed={seed}")
                pair["identity"] = identities["baseline"]
                pair["ratios"] = {metric: pair["medians"]["candidate"][metric] / pair["medians"]["baseline"][metric] for metric in q.METRICS}
                with (output / "paired-results.jsonl").open("a") as destination:
                    destination.write(json.dumps(pair, sort_keys=True) + "\n")
                completed += 1
                print(json.dumps({"matched": True, "cell": cell, "seed": seed, "ratios": pair["ratios"]}), flush=True)
        for binary in binaries.values():
            q.require(q.sha256(Path(binary["path"])) == binary["sha256"], "binary contents changed")
        metadata.update(status="complete", finished_unix=time.time())
    except BaseException as error:
        metadata.update(status="interrupted", interrupted_unix=time.time(), error=str(error))
        raise
    finally:
        metadata["completed_pairs"] = completed
        q.save(output / "metadata.json", metadata)
        if completed:
            q.summarize(output, args.review_percent)


if __name__ == "__main__":
    main()

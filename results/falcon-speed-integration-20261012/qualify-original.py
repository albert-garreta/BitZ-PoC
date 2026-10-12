#!/usr/bin/env python3
"""Sequential paired qualification of frozen, protocol-identical Falcon binaries.

Run only after builds/tests stop. A fresh output directory is required unless
--resume is supplied. --summarize-only rebuilds reports from an existing run.
No third-party Python packages are required.
"""

import argparse
import hashlib
import itertools
import json
import math
import os
from pathlib import Path
import random
import statistics
import subprocess
import time


METRICS = ("total_prover_ms", "prove_ms", "witness_commit_ms", "verify_ms",
           "process_peak_rss_kib")
IDENTITY = ("input_digest", "source_root", "proof_debug_digest",
            "proof_payload_bytes", "proof_payload_breakdown")
LIMITATION = (
    "Finite-sample comparison on this machine and these inputs only. Each seed "
    "contributes the median of repeated proofs in one process per variant. "
    "Reported 95% bootstrap intervals resample paired seed ratios, not individual "
    "proofs; three seeds give weak uncertainty estimates and do not establish "
    "nonregression. Missing fixture caches are primed in a separate untimed "
    "process before either paired measurement. RSS is process lifetime high-water memory, including fixture "
    "setup and warmups, not per-proof allocated memory. Preparation and fixture "
    "generation are excluded from total_prover_ms."
)


def save(path, value):
    temporary = path.with_suffix(path.suffix + ".tmp")
    temporary.write_text(json.dumps(value, indent=2, sort_keys=True) + "\n")
    temporary.replace(path)


def sha256(path):
    with path.open("rb") as source:
        return hashlib.file_digest(source, "sha256").hexdigest()


def require(condition, message):
    if not condition:
        raise RuntimeError(message)


def parse_args():
    p = argparse.ArgumentParser(description=__doc__)
    for variant in ("baseline", "candidate"):
        for mode in ("arithmetic", "full"):
            p.add_argument(f"--{variant}-{mode}", type=Path)
    p.add_argument("--output", type=Path, required=True)
    p.add_argument("--modes", nargs="+", choices=["arithmetic", "full"],
                   default=["arithmetic", "full"])
    p.add_argument("--degrees", nargs="+", type=int, choices=[512, 1024], default=[512, 1024])
    p.add_argument("--batches", nargs="+", type=int, default=[1, 3, 32, 513, 1024])
    p.add_argument("--securities", nargs="+", type=int, choices=[100, 128], default=[100, 128])
    p.add_argument("--threads", nargs="+", type=int, default=[1, 8])
    p.add_argument("--seeds", nargs="+", type=int, default=[42, 43, 44])
    p.add_argument("--iterations", type=int, default=3)
    p.add_argument("--warmup", type=int, default=1)
    p.add_argument("--cpus", default="6,7,4,5,2,3,0,1",
                   help="Ordered worker CPUs; each run uses the first THREADS entries. Main and auxiliary workers use the first entry.")
    p.add_argument("--cache", type=Path, default=Path("/tmp/bitz-falcon-speed-case-cache"))
    p.add_argument("--build-description", default="release; features=falcon-hybrid; RUSTFLAGS=-C target-cpu=native",
                   help="Record the actual common build configuration; binary output also checks embedded RUSTFLAGS.")
    p.add_argument("--review-percent", type=float, default=3.0,
                   help="Flag observed primary-metric increases above this percentage for confirmation; not a statistical gate.")
    p.add_argument("--resume", action="store_true")
    p.add_argument("--summarize-only", action="store_true")
    return p.parse_args()


def check_affinity(report, cpus):
    require(isinstance(report, dict), "missing benchmark CPU affinity report")
    require(report["requested_worker_cpus"] == cpus, "requested CPU placement differs")
    require(report["observed_worker_cpus"] == [[cpu] for cpu in cpus], "worker pinning differs")
    require(report["observed_main_cpus"] == cpus[:1], "main-thread pinning differs")
    require(report["observed_auxiliary_worker_cpus"] == [cpus[:1]] * len(cpus), "auxiliary pinning differs")


def read_run(path, mode, cell, seed, args, cpus):
    records = [json.loads(line) for line in path.read_text().splitlines() if line.strip()]
    if mode == "full":
        prepared = [row for row in records if row.get("event") == "prepared"]
        require(len(prepared) == 1, f"{path}: expected one prepared record")
        context = prepared[0]
        trials = [row for row in records if row.get("event") == "trial"]
        require(context["input_seed"] == seed, f"{path}: unexpected input seed")
    else:
        trials = records
        require(bool(trials), f"{path}: no trials")
        context = trials[0]
    flags = context.get("build_rustflags")
    require(isinstance(flags, str) and "target-cpu=native" in flags,
            f"{path}: native CPU build flag missing: {flags!r}")
    check_affinity(context.get("cpu_affinity"), cpus)
    require(len(trials) == args.warmup + args.iterations, f"{path}: trial count differs")
    normalized = []
    for index, row in enumerate(trials):
        phase = "warmup" if index < args.warmup else "sample"
        require(row["trial"] == phase and row["verified"] is True, f"{path}: unverified or unexpected trial")
        require(row["degree"] == cell["degree"] and row["batch"] == cell["batch"]
                and row["threads"] == cell["threads"], f"{path}: shape differs")
        security = row["security_target"] if mode == "full" else row["security_bits"]
        require(security == cell["security_bits"], f"{path}: security differs")
        for key in IDENTITY:
            require(key in row and row[key] is not None, f"{path}: missing identity field {key}")
            require(row[key] == trials[0][key], f"{path}: {key} varies across repeated proofs")
        if mode == "arithmetic":
            require(row["seed"] == seed and row["auxiliary_pool_threads"] == len(cpus),
                    f"{path}: seed or auxiliary thread count differs")
            check_affinity(row.get("cpu_affinity"), cpus)
        else:
            verifier = row.get("flock_verifier_affinity")
            require(verifier and verifier["observed_cpus"] == cpus[:1], f"{path}: verifier pinning differs")
        sample = dict(cell, seed=seed, trial=phase, trial_index=index,
                      input_digest=row["input_digest"], source_root=row["source_root"],
                      proof_debug_digest=row["proof_debug_digest"],
                      proof_payload_bytes=row["proof_payload_bytes"])
        for metric in METRICS:
            key = {"prove_ms": "proof_prove_ms", "verify_ms": "proof_verify_ms"}.get(metric, metric) if mode == "full" else metric
            value = row[key]
            require(isinstance(value, (float, int)) and math.isfinite(value) and value > 0,
                    f"{path}: invalid {key}: {value}")
            sample[metric] = value
        normalized.append(sample)
    identity = {key: trials[0][key] for key in IDENTITY}
    identity["build_rustflags"] = flags
    identity["protocol"] = context["protocol"] if mode == "full" else context["protocol_id"]
    return normalized, identity


def geometric_mean(values):
    return math.exp(statistics.mean(math.log(value) for value in values))


def bootstrap_interval(ratios):
    if len(ratios) < 3:
        return None
    rng = random.Random(20261012)
    values = sorted(geometric_mean(rng.choices(ratios, k=len(ratios))) for _ in range(4000))
    return [values[99], values[3899]]


def summarize(output, review_percent):
    path = output / "paired-results.jsonl"
    pairs = [json.loads(line) for line in path.read_text().splitlines() if line.strip()]
    groups = {}
    for pair in pairs:
        cell = pair["cell"]
        key = tuple(cell[k] for k in ("mode", "degree", "batch", "security_bits", "threads"))
        groups.setdefault(key, []).append(pair)
    summaries = []
    seed_summaries = []
    for key, group in groups.items():
        summary = {"cell": group[0]["cell"], "seeds": [pair["seed"] for pair in group], "metrics": {}}
        for pair in group:
            seed_summaries.append({"cell": pair["cell"], "seed": pair["seed"],
                                   "medians": pair["medians"], "candidate_over_baseline": pair["ratios"]})
        for metric in METRICS:
            ratios = [pair["ratios"][metric] for pair in group]
            ratio = geometric_mean(ratios)
            summary["metrics"][metric] = {
                "baseline_median_of_seed_medians": statistics.median(pair["medians"]["baseline"][metric] for pair in group),
                "candidate_median_of_seed_medians": statistics.median(pair["medians"]["candidate"][metric] for pair in group),
                "candidate_over_baseline_geomean": ratio,
                "observed_change_percent": 100 * (ratio - 1),
                "seed_ratios": ratios,
                "paired_seed_bootstrap_95_percent_interval": bootstrap_interval(ratios),
                "review_observed_increase": metric in ("total_prover_ms", "prove_ms") and ratio > 1 + review_percent / 100,
            }
        summaries.append(summary)
    save(output / "summary.json", {"limitations": LIMITATION, "completed_pairs": len(pairs), "cells": summaries})
    save(output / "seed-summary.json", seed_summaries)
    flagged = [{"cell": row["cell"], "metric": metric, **value}
               for row in summaries for metric, value in row["metrics"].items()
               if value["review_observed_increase"]]
    save(output / "confirmation-candidates.json", {
        "definition": f"Observed total/proof geometric-mean ratio exceeds +{review_percent}%; confirm with new seeds and longer samples. This is not proof of a regression.",
        "cells": flagged,
    })
    lines = ["Falcon speed integration qualification", "", LIMITATION, "",
             f"{len(pairs)} completed pairs across {len(summaries)} cells. Ratios are candidate / baseline; lower is faster or uses less peak RSS.", "",
             "| Mode | N | Batch | Security | Workers | Seeds | Total ratio | Proof ratio | Witness ratio | Verify ratio | Peak RSS ratio |",
             "|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|"]
    for row in summaries:
        c = row["cell"]
        ratios = [f"{row['metrics'][metric]['candidate_over_baseline_geomean']:.3f}" for metric in METRICS]
        lines.append("| " + " | ".join(map(str, [c["mode"], c["degree"], c["batch"], c["security_bits"], c["threads"], len(row["seeds"]), *ratios])) + " |")
    lines += ["", f"{len(flagged)} primary metric comparisons exceed the observed +{review_percent}% review threshold. See confirmation-candidates.json; inspect seed ratios before choosing confirmation cells.", ""]
    (output / "report.md").write_text("\n".join(lines))


def main():
    args = parse_args()
    output = args.output.resolve()
    if args.summarize_only:
        summarize(output, args.review_percent)
        return
    require(args.iterations > 0 and args.warmup >= 0, "invalid trial counts")
    require(all(1 <= batch <= 1024 for batch in args.batches), "invalid batch size")
    cpus = [int(cpu) for cpu in args.cpus.split(",")]
    require(len(cpus) == len(set(cpus)), "CPU list contains duplicates")
    require(set(cpus) <= os.sched_getaffinity(0), "requested CPUs outside current affinity mask")
    require(all(1 <= threads <= len(cpus) for threads in args.threads), "insufficient CPUs for thread counts")
    for key in ("modes", "degrees", "batches", "securities", "threads", "seeds"):
        require(len(getattr(args, key)) == len(set(getattr(args, key))), f"duplicate {key}")
    binaries = {}
    stamps = {}
    for mode in args.modes:
        binaries[mode] = {}
        for variant in ("baseline", "candidate"):
            value = getattr(args, f"{variant}_{mode}")
            require(value is not None, f"--{variant}-{mode} is required")
            path = value.resolve()
            require(path.is_file() and os.access(path, os.X_OK), f"not executable: {path}")
            binaries[mode][variant] = {"path": str(path), "sha256": sha256(path)}
            stamps[str(path)] = (path.stat().st_size, path.stat().st_mtime_ns)
    cells = [dict(mode=mode, degree=degree, batch=batch, security_bits=security, threads=threads)
             for mode, degree, batch, security, threads in itertools.product(
                 args.modes, args.degrees, args.batches, args.securities, args.threads)]
    configuration = {
        "cells": cells, "seeds": args.seeds, "iterations": args.iterations, "warmup": args.warmup,
        "cpus": cpus, "cache": str(args.cache.resolve()), "binaries": binaries,
        "build_description": args.build_description,
        "ordering": "All processes sequential; baseline first if (cell_index + seed_index) is even, candidate first otherwise.",
    }
    metadata_path = output / "metadata.json"
    if metadata_path.exists():
        require(args.resume, "output already contains a run; use a fresh directory or --resume")
        metadata = json.loads(metadata_path.read_text())
        require(metadata["configuration"] == configuration, "resume configuration or binary hashes differ")
    else:
        require(not args.resume, "cannot resume without metadata.json")
        output.mkdir(parents=True, exist_ok=True)
        require(not any(output.iterdir()), "output directory must be empty")
        metadata = {
            "configuration": configuration, "started_unix": time.time(),
            "platform": subprocess.check_output(["uname", "-a"], text=True).strip(),
            "cpu": subprocess.check_output(["lscpu"], text=True),
            "runner_sha256": sha256(Path(__file__)), "limitations": LIMITATION,
        }
        save(metadata_path, metadata)
    raw = output / "raw"
    raw.mkdir(exist_ok=True)
    pairs_path = output / "paired-results.jsonl"
    completed = [json.loads(line) for line in pairs_path.read_text().splitlines()] if pairs_path.exists() else []
    pair_keys = {json.dumps([pair["cell"], pair["seed"]], sort_keys=True) for pair in completed}
    sanitized = [key for key in os.environ if key.startswith(("BITZ_", "RAYON_", "OMP_"))]
    base_env = {key: value for key, value in os.environ.items() if key not in sanitized}
    base_env.pop("RUSTFLAGS", None)
    print(f"{len(cells)} cells, {len(cells) * len(args.seeds)} pairs, {len(completed)} completed", flush=True)
    try:
        for cell_index, cell in enumerate(cells):
            selected_cpus = cpus[:cell["threads"]]
            cpu_list = ",".join(map(str, selected_cpus))
            for seed_index, seed in enumerate(args.seeds):
                pair_key = json.dumps([cell, seed], sort_keys=True)
                if pair_key in pair_keys:
                    continue
                order = ["baseline", "candidate"] if (cell_index + seed_index) % 2 == 0 else ["candidate", "baseline"]
                pair = {"cell": cell, "seed": seed, "order": order, "medians": {}, "ratios": {}, "logs": {}}
                identities = {}
                for variant in order:
                    binary = binaries[cell["mode"]][variant]
                    path = Path(binary["path"])
                    require((path.stat().st_size, path.stat().st_mtime_ns) == stamps[str(path)], f"binary changed: {path}")
                    stem = f"{cell['mode']}-n{cell['degree']}-b{cell['batch']}-s{cell['security_bits']}-t{cell['threads']}-seed{seed}-{variant}"
                    command = ["taskset", "-c", cpu_list, str(path)]
                    for flag, value in [("degree", cell["degree"]), ("batch", cell["batch"]),
                                        ("security", cell["security_bits"]), ("threads", cell["threads"]),
                                        ("seed", seed), ("warmup", args.warmup), ("iterations", args.iterations)]:
                        command += [f"--{flag}", str(value)]
                    if cell["mode"] == "full":
                        command += ["--k", "auto"]
                    configured = {"RAYON_NUM_THREADS": str(cell["threads"]), "OMP_NUM_THREADS": str(cell["threads"]),
                                  "BITZ_FALCON_MAIN_CPU": str(cpus[0]), "BITZ_FALCON_WORKER_CPUS": cpu_list,
                                  "BITZ_FALCON_CASE_CACHE": str(args.cache.resolve())}
                    fixture = args.cache / f"fn-dsa-0.3.0-n{cell['degree']}-b{cell['batch']}-seed{seed}.bin"
                    if not fixture.exists():
                        prime_command = command.copy()
                        prime_command[prime_command.index("--warmup") + 1] = "0"
                        prime_command[prime_command.index("--iterations") + 1] = "1"
                        prime_meta = {"argv": prime_command, "environment": configured,
                                      "started_unix": time.time(), "excluded_from_measurements": True}
                        print(f"priming fixture cache: {fixture.name}", flush=True)
                        with (raw / f"{stem}.cache-prime.jsonl").open("w") as stdout, (raw / f"{stem}.cache-prime.stderr").open("w") as stderr:
                            prime = subprocess.run(prime_command, env={**base_env, **configured}, stdout=stdout, stderr=stderr)
                        prime_meta.update(finished_unix=time.time(), returncode=prime.returncode)
                        save(raw / f"{stem}.cache-prime.command.json", prime_meta)
                        require(prime.returncode == 0 and fixture.exists(), f"fixture cache priming failed: {stem}")
                    command_meta = {"argv": command, "environment": configured,
                                    "cleared_environment_names": sorted(sanitized), "binary_sha256": binary["sha256"],
                                    "started_unix": time.time()}
                    save(raw / f"{stem}.command.json", command_meta)
                    print(stem, flush=True)
                    with (raw / f"{stem}.jsonl").open("w") as stdout, (raw / f"{stem}.stderr").open("w") as stderr:
                        result = subprocess.run(command, env={**base_env, **configured}, stdout=stdout, stderr=stderr)
                    command_meta.update(finished_unix=time.time(), returncode=result.returncode)
                    save(raw / f"{stem}.command.json", command_meta)
                    require(result.returncode == 0, f"benchmark failed ({result.returncode}): {stem}; inspect raw stderr")
                    rows, identity = read_run(raw / f"{stem}.jsonl", cell["mode"], cell, seed, args, selected_cpus)
                    identities[variant] = identity
                    samples = [row for row in rows if row["trial"] == "sample"]
                    require(len(samples) == args.iterations, f"{stem}: sample count differs")
                    pair["medians"][variant] = {metric: statistics.median(row[metric] for row in samples) for metric in METRICS}
                    pair["logs"][variant] = str(Path("raw") / f"{stem}.jsonl")
                require(identities["baseline"] == identities["candidate"],
                        f"protocol/input/build identity mismatch for {cell}, seed={seed}: "
                        + ", ".join(key for key in identities["baseline"] if identities["baseline"][key] != identities["candidate"].get(key)))
                pair["identity"] = identities["baseline"]
                pair["ratios"] = {metric: pair["medians"]["candidate"][metric] / pair["medians"]["baseline"][metric] for metric in METRICS}
                with pairs_path.open("a") as destination:
                    destination.write(json.dumps(pair, sort_keys=True) + "\n")
                pair_keys.add(pair_key)
                print(json.dumps({"matched": True, "cell": cell, "seed": seed, "ratios": pair["ratios"]}), flush=True)
        for mode_binaries in binaries.values():
            for binary in mode_binaries.values():
                require(sha256(Path(binary["path"])) == binary["sha256"], f"binary contents changed: {binary['path']}")
        metadata.update(finished_unix=time.time(), completed_pairs=len(pair_keys), status="complete")
        save(metadata_path, metadata)
        summarize(output, args.review_percent)
    except BaseException as error:
        metadata.update(interrupted_unix=time.time(), completed_pairs=len(pair_keys), status="interrupted", error=str(error))
        save(metadata_path, metadata)
        if pair_keys:
            summarize(output, args.review_percent)
        raise


if __name__ == "__main__":
    main()

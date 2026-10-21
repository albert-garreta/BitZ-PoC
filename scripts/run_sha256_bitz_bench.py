#!/usr/bin/env python3
"""Build and measure the BitZ SHA-256 compressions benchmark.

One fresh process of `sha256_compressions` per (thread count, Ligerito
profile, batch size), each with one warm-up and `--reps` measured proofs, every
proof verified. Writes `summary.csv` (the RESULT line of every run),
`samples.csv` (every SAMPLE line) and `metadata.json` to a new directory.
"""

from __future__ import annotations

import argparse
from bench_support import root_metadata
import json
import os
import platform
import re
import statistics
import sys
from datetime import datetime, timezone
from pathlib import Path
from ligerito_results import validate_result_fields
from bench_support import command_text, cpu_name, write_csv, run_logged, cargo_executables, environment


ROOT = Path(__file__).resolve().parents[1]
BENCH = "sha256_compressions"
PROFILE = re.compile(r"johnson|udr|legacy-udr|slim3?|fast|secure|(custom|udrg?):[0-9]+:[0-9]+(:[0-9]+)?")


def fields(line: str) -> dict[str, str]:
    return dict(token.split("=", 1) for token in line.split()[1:] if "=" in token)


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--shapes", type=int, nargs="+", default=[13, 14, 15, 16], metavar="K",
                        help="batch sizes as exponents: 2^K compressions (7..16)")
    parser.add_argument("--threads", type=int, nargs="+", default=[1, 10], metavar="T")
    parser.add_argument("--profiles", nargs="+", default=["custom:1:4"], metavar="PROFILE",
                        help="BITZ_LIG_PROFILE values, e.g. custom:1:4 (rate 1/2) custom:3:4 (rate 1/8)")
    parser.add_argument("--private-io", action="store_true",
                        help="nothing public: input states, blocks and outputs are witness only")
    parser.add_argument("--opener", choices=["block", "virtual"], default="block")
    parser.add_argument("--reps", type=int, default=5)
    parser.add_argument("--lambda", dest="security", choices=["100", "128", "sha128-reference-schedule"], default="100")
    parser.add_argument("--seed", type=lambda value: int(value, 0), default=0x46325A5F53484132)
    parser.add_argument("--rss", action="store_true",
                        help="run each process under /usr/bin/time and record its peak resident set size")
    parser.add_argument("--checked", action="store_true", help="omit the usual unchecked feature")
    parser.add_argument("--offline", action="store_true", help="require Cargo's cached dependencies")
    parser.add_argument("--out-dir", type=Path, help="new directory; an existing path is rejected")
    args = parser.parse_args()
    if args.reps < 1 or any(threads < 1 for threads in args.threads):
        parser.error("--threads and --reps must be positive")
    if any(shape not in range(7, 17) for shape in args.shapes):
        parser.error("--shapes must contain exponents in 7..16")
    for name, values in (("--shapes", args.shapes), ("--threads", args.threads), ("--profiles", args.profiles)):
        if len(set(values)) != len(values):
            parser.error(f"{name} must not contain duplicates")
    if any(not PROFILE.fullmatch(profile) for profile in args.profiles):
        parser.error("--profiles takes BITZ_LIG_PROFILE values such as custom:1:4")
    if not 0 <= args.seed < 2**64:
        parser.error("--seed must fit in an unsigned 64-bit integer")
    if args.rss and sys.platform not in ("darwin", "linux"):
        parser.error("--rss needs /usr/bin/time on macOS or Linux")
    return args


def timed(command: list[str]) -> list[str]:
    """`command` under /usr/bin/time, which reports the peak RSS on stderr."""
    return ["/usr/bin/time", "-l" if sys.platform == "darwin" else "-v", *command]


def peak_rss_bytes(log: str) -> int:
    """The peak RSS /usr/bin/time printed: bytes on macOS, KiB on Linux."""
    darwin = re.search(r"^\s*(\d+)\s+maximum resident set size", log, re.MULTILINE)
    if darwin:
        return int(darwin.group(1))
    linux = re.search(r"Maximum resident set size \(kbytes\):\s*(\d+)", log)
    if linux:
        return 1024 * int(linux.group(1))
    raise RuntimeError("/usr/bin/time printed no peak resident set size")


def main() -> int:
    args = parse_args()
    started = datetime.now(timezone.utc)
    out = (args.out_dir or ROOT / "bench_results" / started.strftime("sha256-bitz-%Y%m%dT%H%M%S.%fZ")).resolve()
    out.mkdir(parents=True, exist_ok=False)
    env = os.environ.copy()
    env.setdefault("RUSTFLAGS", "-C target-cpu=native")
    # Select the standard workload despite aliases or a previous trace run.
    removed = {
        key: env.pop(key) for key in list(env)
        if key.startswith("BITZ_SHA_")
        or key in {"BITZ_BENCH_SHAPES", "BITZ_BENCH_PASS", "BITZ_LIG_PROFILE", "BITZ_OPENER",
                   "OBLONG_PROFILE_INTERVALS"}
    }
    env.update(BITZ_BENCH_REPS=str(args.reps), BITZ_BENCH_SEED=hex(args.seed),
               BITZ_BENCH_LAMBDA=args.security, BITZ_OPENER=args.opener)
    if args.private_io:
        env["BITZ_SHA_PRIVATE_IO"] = "1"
    statement = "private-io" if args.private_io else "public-io"
    features = ["span-metrics"] if args.checked else ["unchecked", "span-metrics"]
    build = ["cargo", "bench", "--locked", "--no-run", "--message-format=json-render-diagnostics",
             "--bench", BENCH, "--features", ",".join(features)]
    if args.offline:
        build.append("--offline")
    source = root_metadata(ROOT)
    metadata = {
        "started_utc": started.isoformat(), "status": "running", "git_revision": source["revision"],
        "git_status": source["git_status"], "rustc": command_text("rustc", "-Vv"), "cargo": command_text("cargo", "-V"),
        "cpu": cpu_name(), "os": platform.platform(), "architecture": platform.machine(),
        "shapes": args.shapes, "threads": args.threads, "profiles": args.profiles,
        "statement": statement, "opener": args.opener, "reps": args.reps, "warmups": 1,
        "lambda": args.security, "seed": hex(args.seed), "peak_rss": args.rss,
        "features": features, "default_features": True, "build_command": build,
        "environment": environment(env),
        "cleared_environment": removed, "runs": [],
    }
    metadata_path = out / "metadata.json"
    summaries: list[dict[str, str]] = []
    samples: list[dict[str, str]] = []
    try:
        metadata_path.write_text(json.dumps(metadata, indent=2) + "\n")
        build_log = run_logged(build, env, out / "build.log")
        executable = cargo_executables(build_log, [BENCH])[BENCH]
        for threads in args.threads:
            for profile in args.profiles:
                for shape in args.shapes:
                    run_id = f"2p{shape}-{profile.replace(':', '_')}-t{threads}"
                    run_env = dict(env, BITZ_BENCH_SHAPES=str(shape), BITZ_LIG_PROFILE=profile,
                                   RAYON_NUM_THREADS=str(threads))
                    command = timed([executable]) if args.rss else [executable]
                    log = run_logged(command, run_env, out / f"{run_id}.log")
                    results = [fields(line.strip()) for line in log.splitlines() if line.strip().startswith("RESULT ")]
                    raw = [fields(line.strip()) for line in log.splitlines() if line.strip().startswith("SAMPLE ")]
                    if len(results) != 1 or results[0].get("verified_samples") != str(args.reps):
                        raise RuntimeError(f"{run_id}: expected one RESULT with {args.reps} verified samples")
                    if len(raw) != args.reps or any(row.get("verified") != "true" for row in raw):
                        raise RuntimeError(f"{run_id}: expected {args.reps} verified SAMPLE rows")
                    validate_result_fields(results[0])
                    labels = dict(run_id=run_id, threads=str(threads), ligerito_profile=profile,
                                  statement=statement, opener=args.opener)
                    row = dict(labels, **results[0])
                    row["witness_to_proof_ms"] = f"{statistics.median(float(sample['witness_ms']) + float(sample['prove_ms']) for sample in raw):.6f}"
                    if args.rss:
                        row["peak_rss_bytes"] = str(peak_rss_bytes(log))
                    summaries.append(row)
                    samples.extend(dict(labels, **sample) for sample in raw)
                    metadata["runs"].append({"run_id": run_id, "executable": executable, "shape": shape,
                                             "threads": threads, "ligerito_profile": profile})
                    write_csv(out / "summary.csv", summaries)
                    write_csv(out / "samples.csv", samples)
                    metadata_path.write_text(json.dumps(metadata, indent=2) + "\n")
        metadata["status"] = "complete"
    except (OSError, ValueError, RuntimeError) as error:
        metadata["status"] = "failed"
        metadata["error"] = str(error)
        print(f"Benchmark failed: {error}\nPartial results: {out}", flush=True)
    finally:
        metadata["finished_utc"] = datetime.now(timezone.utc).isoformat()
        metadata_path.write_text(json.dumps(metadata, indent=2) + "\n")
    print(f"Results: {out}", flush=True)
    return 0 if metadata["status"] == "complete" else 1


if __name__ == "__main__":
    raise SystemExit(main())

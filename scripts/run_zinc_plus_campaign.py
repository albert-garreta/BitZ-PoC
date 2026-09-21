#!/usr/bin/env python3
"""Measure the Zinc+ rows of the multiplication tables and import them.

Zinc+ cannot be linked into this crate (it pins `crypto-bigint = 0.7.0-rc.9`
while `vendor/field` needs 0.7.5), so its rows come from an external worker:
this script pins a zinc-plus checkout, copies the workers from
`benchmarks/zinc-plus/` into `protocol/benches/`, builds a single-threaded
and a `parallel` binary per worker, exports the BitZ corpora with
`examples/mul_corpus_export`, runs one worker process per case under
`/usr/bin/time -l`, and imports the logs with `scripts/zinc_plus_import.py`
into a `mul-bench/v2` campaign directory.

    python3 scripts/run_zinc_plus_campaign.py --output PerfRuns/zinc-plus \\
        --workdir /tmp/zinc-plus-878fbd8 --zinc-source ~/zinc-plus --revision 878fbd8 \\
        --workloads u32-mod32 u64 u128 --exponents 15 17 19 --threads 1 10 --reps 5

Run it under scripts/bench_gate.py like every other campaign. The first
build of each worker also runs once in CHECKED mode (no `unchecked`) at the
smallest exponent, which validates the integer-lane magnitude bounds.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
import shutil
import subprocess
import sys
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
WORKERS = {"u32-mod32": "bitz_u32_mod32", "u64": "bitz_wide_mul", "u128": "bitz_wide_mul"}
DEFAULT_REVISION = "878fbd8292472dcb13b25e2c9c0209406b5fb671"
U32_SEED = 0x5533_3250_4353_0064
RATE_LOG = 2  # the workers' default IPRS rate 1/4


def sh(command, cwd=None, env=None, capture=False):
    print("+", " ".join(map(str, command)), flush=True)
    return subprocess.run(command, cwd=cwd, env=env, check=True, text=True, capture_output=capture)


def prepare_checkout(args):
    workdir = args.workdir
    if not (workdir / ".git").exists():
        source = str(args.zinc_source) if args.zinc_source else args.repository
        sh(["git", "clone", "--quiet", source, str(workdir)])
    sh(["git", "-C", str(workdir), "fetch", "--quiet", "--all"])
    sh(["git", "-C", str(workdir), "checkout", "--quiet", "--force", args.revision])
    revision = sh(["git", "-C", str(workdir), "rev-parse", "HEAD"], capture=True).stdout.strip()
    lock = args.zinc_source / "Cargo.lock" if args.zinc_source else None
    if lock and lock.exists() and not (workdir / "Cargo.lock").exists():
        shutil.copy(lock, workdir / "Cargo.lock")
    benches = workdir / "protocol" / "benches"
    manifest = workdir / "protocol" / "Cargo.toml"
    text = manifest.read_text()
    for name in sorted(set(WORKERS.values())):
        shutil.copy(ROOT / "benchmarks" / "zinc-plus" / f"{name}.rs", benches / f"{name}.rs")
        if f'name = "{name}"' not in text:
            text += f'\n[[bench]]\nname = "{name}"\nharness = false\n'
    if "blake3 = { workspace = true }" not in text:
        text = text.replace('zstd = "0.13"', 'zstd = "0.13"\nblake3 = { workspace = true }', 1)
        if "blake3 = { workspace = true }" not in text:
            raise SystemExit("could not add blake3 to protocol/Cargo.toml dev-dependencies")
    manifest.write_text(text)
    return revision


def build_worker(args, name, features, target_dir):
    env = dict(os.environ, CARGO_TARGET_DIR=str(target_dir), RUSTFLAGS="-C target-cpu=native")
    env.pop("CARGO_ENCODED_RUSTFLAGS", None)
    command = ["cargo", "bench", "--no-run", "--bench", name, "--features", features, "--message-format=json"]
    if args.offline:
        command.append("--offline")
    result = sh(command, cwd=args.workdir / "protocol", env=env, capture=True)
    for line in result.stdout.splitlines():
        try:
            record = json.loads(line)
        except ValueError:
            continue
        if record.get("reason") == "compiler-artifact" and record.get("target", {}).get("name") == name and record.get("executable"):
            return Path(record["executable"])
    raise SystemExit(f"no executable for {name} with features {features}")


def export_corpus(args, workload, exponent, seed):
    directory = args.output / "corpora"
    directory.mkdir(parents=True, exist_ok=True)
    stem = f"corpus-{workload}-2p{exponent}-seed{seed}"
    manifest = directory / f"{stem}.json"
    if not manifest.exists():
        command = [str(args.corpus_exporter), "--workload", workload, "--log-n", str(exponent), "--seed", str(seed), "--out", str(directory)]
        sh(command)
    return json.loads(manifest.read_text())


def run_case(args, binary, workload, exponent, threads, reps, corpus, checked=False):
    log_dir = args.output / ("validation" if checked else "logs")
    log_dir.mkdir(parents=True, exist_ok=True)
    stem = f"{workload}-2p{exponent}-t{threads}" + ("-checked" if checked else "")
    env = dict(os.environ, REPS=str(reps), RAYON_NUM_THREADS=str(threads), EXPONENT=str(exponent), SEED=str(corpus["seed"]))
    if workload != "u32-mod32":
        env["CORPUS"] = str(args.output / "corpora" / f"corpus-{workload}-2p{exponent}-seed{corpus['seed']}.json")
    command = ["/usr/bin/time", "-l", str(binary)] if sys.platform == "darwin" else [str(binary)]
    started = time.monotonic()
    with (log_dir / f"{stem}.out").open("w") as out, (log_dir / f"{stem}.err").open("w") as err:
        code = subprocess.run(command, stdout=out, stderr=err, env=env).returncode
    meta = dict(workload=workload, log_n=exponent, seed=corpus["seed"], threads=threads, reps=reps, log_inv_rate=RATE_LOG,
                features=("simd unchecked" if not checked else "simd") + (" parallel" if threads > 1 else ""),
                executable=str(binary), executable_sha256=hashlib.sha256(binary.read_bytes()).hexdigest(),
                corpus_digest=corpus["corpus_digest"], exit_code=code, elapsed_seconds=round(time.monotonic() - started, 1))
    if code != 0:
        meta["reason"] = f"worker exited with {code}; see {stem}.err"
    (log_dir / f"{stem}.meta.json").write_text(json.dumps(meta, indent=2) + "\n")
    print(f"{stem}: exit {code} ({meta['elapsed_seconds']}s)", flush=True)
    return code == 0


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--output", type=Path, required=True, help="campaign directory (created; must not exist)")
    parser.add_argument("--workdir", type=Path, required=True, help="zinc-plus checkout to create or reuse")
    parser.add_argument("--zinc-source", type=Path, default=None, help="local zinc-plus clone to clone from (its Cargo.lock is reused)")
    parser.add_argument("--repository", default="https://github.com/NethermindEth/zinc-plus")
    parser.add_argument("--revision", default=DEFAULT_REVISION)
    parser.add_argument("--corpus-exporter", type=Path, required=True, help="built examples/mul_corpus_export binary")
    parser.add_argument("--workloads", nargs="+", choices=sorted(WORKERS), default=["u32-mod32", "u64", "u128"])
    parser.add_argument("--exponents", nargs="+", type=int, default=[15, 17, 19])
    parser.add_argument("--threads", nargs="+", type=int, default=[1, 10])
    parser.add_argument("--reps", type=int, default=5)
    parser.add_argument("--skip-validation", action="store_true", help="skip the CHECKED-mode run at the smallest exponent")
    parser.add_argument("--offline", action="store_true")
    args = parser.parse_args(argv)
    if args.output.exists():
        raise SystemExit(f"output exists: {args.output}")
    args.output.mkdir(parents=True)
    revision = prepare_checkout(args)
    toolchain = sh(["rustc", "-V"], cwd=args.workdir, capture=True).stdout.strip()
    binaries = {}
    for name in sorted({WORKERS[w] for w in args.workloads}):
        binaries[(name, 1, False)] = build_worker(args, name, "simd unchecked", args.workdir / "target-st")
        if any(t > 1 for t in args.threads):
            binaries[(name, 10, False)] = build_worker(args, name, "simd unchecked parallel", args.workdir / "target-mt")
        if not args.skip_validation:
            binaries[(name, 1, True)] = build_worker(args, name, "simd", args.workdir / "target-checked")
    complete = True
    for workload in args.workloads:
        for exponent in args.exponents:
            seed = U32_SEED if workload == "u32-mod32" else None
            corpus = export_corpus(args, workload, exponent, seed) if seed is not None else export_corpus_default(args, workload, exponent)
            if not args.skip_validation and exponent == min(args.exponents):
                complete &= run_case(args, binaries[(WORKERS[workload], 1, True)], workload, exponent, 1, 1, corpus, checked=True)
            for threads in args.threads:
                binary = binaries[(WORKERS[workload], 1 if threads == 1 else 10, False)]
                complete &= run_case(args, binary, workload, exponent, threads, args.reps, corpus)
    sources = [ROOT / "benchmarks" / "zinc-plus" / f"{n}.rs" for n in sorted(set(WORKERS.values()))]
    import_command = [sys.executable, str(ROOT / "scripts/zinc_plus_import.py"), str(args.output / "logs"),
                      "--out", str(args.output / "campaign"), "--zinc-revision", revision, "--zinc-repository", args.repository,
                      "--zinc-toolchain", toolchain, "--worker-source", *map(str, sources),
                      "--note", f"features simd,unchecked[,parallel]; rate 1/4; one worker process per case under /usr/bin/time -l"]
    sh(import_command)
    print(f"{'complete' if complete else 'INCOMPLETE'}: {args.output / 'campaign'}")
    return 0 if complete else 1


def export_corpus_default(args, workload, exponent):
    """u64/u128 corpora use the harness's per-workload root seed (the exporter's default)."""
    directory = args.output / "corpora"
    directory.mkdir(parents=True, exist_ok=True)
    existing = sorted(directory.glob(f"corpus-{workload}-2p{exponent}-seed*.json"))
    if not existing:
        sh([str(args.corpus_exporter), "--workload", workload, "--log-n", str(exponent), "--out", str(directory)])
        existing = sorted(directory.glob(f"corpus-{workload}-2p{exponent}-seed*.json"))
    return json.loads(existing[0].read_text())


if __name__ == "__main__":
    raise SystemExit(main())

#!/usr/bin/env python3
"""Split sweep for BitZ logarithmic (`examples/bitz_log_bench`) and its table.

    cargo build --release --example bitz_log_bench
    python3 scripts/bench_gate.py run --label bitz-log-sweep --min-idle 75 --hold-seconds 20 -- \
        python3 scripts/bitz_log_sweep.py run --out PerfRuns/bitz-log-sweep \
            --n 28 30 --dt -2 -1 0 1 --threads 10 --reps 4
    python3 scripts/bitz_log_sweep.py table PerfRuns/bitz-log-sweep

`run` executes one process per cell (threads, n, t[, a]) and keeps its log as
`th<threads>-n<n>-t<t>[-a<a>].log`. The outer split is `t` rows by `s = n - t`
columns: `--t` gives absolute values, `--dt` offsets from the reference split
`ceil(0.6 n) - 1`. `--a` sweeps the recursive instance's split (`t' = 7 + a`
rows, `s' = s - a` columns; default: the opener's own choice). Every other
flag after `--` goes to the bench verbatim (e.g. `-- --ladder custom:2:4`).

`table` prints one Markdown row per cell: prover, verifier, the verifier's
`(*)` term (eq table of `r1` mod `q`, `2^t` exponentiations, one evaluation)
and the rest of the verifier, and the proof bytes of every arm measured
(`std`, `log2` = two Ligerito runs, `log1` = one run).
"""
from __future__ import annotations

import argparse
import datetime
import os
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent


def reference_t(n: int) -> int:
    return (3 * n + 4) // 5 - 1


def run(args: argparse.Namespace) -> int:
    bench = Path(args.bench)
    if not bench.exists():
        print(f"missing {bench}: cargo build --release --example bitz_log_bench", file=sys.stderr)
        return 2
    out = Path(args.out)
    out.mkdir(parents=True, exist_ok=True)
    extra = args.extra[1:] if args.extra[:1] == ["--"] else args.extra
    failures = 0
    for threads in args.threads:
        for n in args.n:
            splits = args.t if args.t else [reference_t(n) + d for d in args.dt]
            for t in splits:
                for a in args.a if args.a else [None]:
                    name = f"th{threads}-n{n}-t{t}" + (f"-a{a}" if a is not None else "")
                    command = [str(bench), str(n), "--t", str(t), "--mode", args.mode, "--reps", str(args.reps)]
                    if a is not None:
                        command += ["--a", str(a)]
                    command += extra
                    with open(out / f"{name}.log", "w") as log:
                        code = subprocess.run(
                            command,
                            stdout=log,
                            stderr=subprocess.STDOUT,
                            env={**os.environ, "RAYON_NUM_THREADS": str(threads)},
                        ).returncode
                    stamp = datetime.datetime.now().strftime("%H:%M:%S")
                    with open(out / "progress", "a") as progress:
                        progress.write(f"{stamp} {name} rc={code}\n")
                    failures += code != 0
    return 1 if failures else 0


def cells(directory: Path) -> dict:
    found: dict = {}
    for path in sorted(directory.glob("th*-n*-t*.log")):
        explicit_a = "-a" in path.stem
        for line in path.read_text().splitlines():
            if not line.startswith("RESULT "):
                continue
            fields = dict(item.split("=", 1) for item in line.split()[1:])
            a = int(fields["a"]) if explicit_a and "a" in fields else None
            key = (int(fields["threads"]), int(fields["n"]), int(fields["t"]), path.stem)
            found.setdefault(key, {"a": a})[fields["mode"]] = fields
    return found


def table(args: argparse.Namespace) -> int:
    found = cells(Path(args.directory))
    if not found:
        print(f"no RESULT lines under {args.directory}", file=sys.stderr)
        return 2

    def rest(result: dict) -> float:
        return float(result["verify_ms"]) - float(result["star_ms"]) - float(result.get("star_rec_ms", 0))

    def column(cell: dict, arms: list, value, digits: int) -> str:
        return " / ".join(f"{value(cell[arm]):.{digits}f}" for arm in arms)

    for threads in sorted({key[0] for key in found}, reverse=True):
        print(f"\n### {threads} thread(s)\n")
        print("| n | t:s | t':s' | arms | prove (ms) | verify (ms) | (*) (ms) | verify - (*) (ms) | proof (KB) |")
        print("|---|---|---|---|---|---|---|---|---|")
        for key in sorted(k for k in found if k[0] == threads):
            cell = found[key]
            arms = [arm for arm in ("std", "log2", "log1") if arm in cell]
            if not arms:
                continue
            first = cell[arms[0]]
            log = next((cell[arm] for arm in ("log1", "log2") if arm in cell), None)
            inner = f"{7 + int(log['a'])}:{int(first['s']) - int(log['a'])}" if log else "-"
            columns = [
                column(cell, arms, lambda r: float(r["prove_ms"]), 1),
                column(cell, arms, lambda r: float(r["verify_ms"]), 2),
                column(cell, arms, lambda r: float(r["star_ms"]), 2),
                column(cell, arms, rest, 2),
                column(cell, arms, lambda r: int(r["proof_bytes"]) / 1000, 1),
            ]
            split = f"{key[2]}:{first['s']}"
            print(f"| {key[1]} | {split} | {inner} | {' / '.join(arms)} | " + " | ".join(columns) + " |")
    return 0


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    commands = parser.add_subparsers(dest="command", required=True)
    runner = commands.add_parser("run", help="run the sweep, one process per cell")
    runner.add_argument("--out", required=True)
    runner.add_argument("--bench", default=str(ROOT / "target/release/examples/bitz_log_bench"))
    runner.add_argument("--n", type=int, nargs="+", required=True)
    runner.add_argument("--t", type=int, nargs="+", help="absolute row variables (overrides --dt)")
    runner.add_argument("--dt", type=int, nargs="+", default=[0], help="offsets from ceil(0.6 n) - 1")
    runner.add_argument("--a", type=int, nargs="+", help="recursive split: t' = 7 + a")
    runner.add_argument("--threads", type=int, nargs="+", default=[10])
    runner.add_argument("--reps", type=int, default=3)
    runner.add_argument("--mode", default="ab3", choices=["std", "log1", "log2", "ab", "ab3"])
    runner.add_argument("extra", nargs=argparse.REMAINDER, help="-- flags passed to the bench")
    runner.set_defaults(handler=run)
    tabulate = commands.add_parser("table", help="Markdown table of a sweep directory")
    tabulate.add_argument("directory")
    tabulate.set_defaults(handler=table)
    args = parser.parse_args()
    return args.handler(args)


if __name__ == "__main__":
    raise SystemExit(main())

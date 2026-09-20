#!/usr/bin/env python3
"""Measure one multiplication cell alone, or record why it cannot be measured.

Large cells of the heavy backends exhaust a 24 GiB machine. The rule for the
tables is: a cell that exhausts memory is excluded and reported as excluded,
with the observed peak resident set and the machine's memory, never measured
while paging. This driver runs ONE cell (workload, backend, size, threads,
rate) as its own launcher campaign under `scripts/bench_gate.py` with a
small swap-growth guard, samples the whole process tree while it runs, and
appends one JSON record per cell to an exclusions log:

    {"workload": "u128", "scheme": "binius64@1", "log_n": 21, "threads": 10,
     "status": "excluded", "reason": "...", "observed_peak_rss_bytes": ...,
     "peak_compressed_bytes": ..., "pageins": ..., "swapouts": ...,
     "free_memory_at_start_bytes": ..., "machine_ram_bytes": ...,
     "campaign": "PerfRuns/..."}

The paging verdict is about the measured process tree itself, read from
`top` (macOS): the bytes of its own pages the kernel compressed out (`CMPRS`)
and the pages it had to page back in (`PAGEINS`). With `--no-gate` the probe
kills the cell once either passes its limit, or once the tree's resident set
passes `--kill-at-rss-gb`. System-wide swap-outs are only a thrash guard with
a large default: on a machine whose other processes hold many cold pages,
the kernel swaps those out long before the measured process pages, and that
is not a reason to exclude the cell.

`status` is `measured` when the launcher completed within the limits (the
campaign directory is then a normal input for scripts/mul_table.py) and
`excluded` when the gate aborted it (exit 86), the worker failed, or the
tree's own pages were compressed out or paged in beyond the limits. Only
records with status `excluded` belong in the table's `--exclusions` file;
`scripts/mul_table.py` refuses a cell that is both.

    python3 scripts/mul_memory_probe.py --output PerfRuns/cs-u128-binius-21-t10 \\
        --workload u128 --backend binius64 --log-n 21 --threads 10 --log-inv-rate 1 \\
        --swap-grow-gb 4 --record PerfRuns/cs-mul-exclusions.jsonl
"""
from __future__ import annotations

import argparse
import json
import os
import re
import subprocess
import sys
import threading
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def machine_ram_bytes():
    try:
        return int(subprocess.run(["sysctl", "-n", "hw.memsize"], capture_output=True, text=True, check=True).stdout.strip())
    except Exception:
        return None


def swapouts():
    """macOS `vm_stat` Swapouts counter (pages), else None."""
    try:
        out = subprocess.run(["vm_stat"], capture_output=True, text=True, check=True).stdout
    except Exception:
        return None
    match = re.search(r"Swapouts:\s+(\d+)", out)
    return int(match.group(1)) if match else None


def free_memory_bytes():
    """macOS `vm_stat` free pages in bytes, else None."""
    try:
        out = subprocess.run(["vm_stat"], capture_output=True, text=True, check=True).stdout
    except Exception:
        return None
    page = re.search(r"page size of (\d+) bytes", out)
    free = re.search(r"Pages free:\s+(\d+)", out)
    return int(free.group(1)) * int(page.group(1)) if page and free else None


SIZE_UNITS = {"B": 1, "K": 1024, "M": 1024**2, "G": 1024**3, "T": 1024**4}


def parse_size(text):
    """`top` sizes: 5569K, 2295M, 13G, 0B (a trailing +/- marks a delta)."""
    text = text.rstrip("+-")
    if text[-1:].isdigit():
        return int(text)
    return int(float(text[:-1]) * SIZE_UNITS[text[-1]])


def process_tree_memory(root_pid):
    """(resident, compressed, pageins) summed over `root_pid` and its descendants, from macOS `top`; None if unavailable."""
    try:
        out = subprocess.run(["top", "-l", "1", "-stats", "pid,ppid,rsize,pageins,cmprs"],
                             capture_output=True, text=True, check=True).stdout
    except Exception:
        return None
    children, rows = {}, {}
    for line in out.splitlines():
        parts = line.split()
        if len(parts) != 5 or not parts[0].isdigit() or not parts[1].isdigit():
            continue
        try:
            rows[int(parts[0])] = (parse_size(parts[2]), int(parts[3].rstrip("+-")), parse_size(parts[4]))
        except (ValueError, KeyError):
            continue
        children.setdefault(int(parts[1]), []).append(int(parts[0]))
    if not rows:
        return None
    rss = compressed = pageins = 0
    stack = [root_pid]
    while stack:
        pid = stack.pop()
        if pid in rows:
            rss += rows[pid][0]
            pageins += rows[pid][1]
            compressed += rows[pid][2]
        stack.extend(children.get(pid, []))
    return rss, compressed, pageins


def process_tree_rss(root_pid):
    """Sum of the resident sets (bytes) of `root_pid` and all its descendants."""
    try:
        out = subprocess.run(["ps", "-axo", "pid=,ppid=,rss="], capture_output=True, text=True, check=True).stdout
    except Exception:
        return 0
    children, rss = {}, {}
    for line in out.splitlines():
        parts = line.split()
        if len(parts) != 3:
            continue
        pid, ppid, kib = int(parts[0]), int(parts[1]), int(parts[2])
        children.setdefault(ppid, []).append(pid)
        rss[pid] = kib * 1024
    total, stack = 0, [root_pid]
    while stack:
        pid = stack.pop()
        total += rss.get(pid, 0)
        stack.extend(children.get(pid, []))
    return total


def kill_tree(root_pid):
    """SIGTERM then SIGKILL the process and every descendant (workers run in their own sessions)."""
    import signal
    for sig in (signal.SIGTERM, signal.SIGKILL):
        try:
            out = subprocess.run(["ps", "-axo", "pid=,ppid="], capture_output=True, text=True, check=True).stdout
        except Exception:
            return
        children = {}
        for line in out.splitlines():
            parts = line.split()
            if len(parts) == 2:
                children.setdefault(int(parts[1]), []).append(int(parts[0]))
        stack, victims = [root_pid], []
        while stack:
            pid = stack.pop()
            victims.append(pid)
            stack.extend(children.get(pid, []))
        for pid in reversed(victims):
            try:
                os.kill(pid, sig)
            except ProcessLookupError:
                pass
        time.sleep(3)


def scheme_of(args):
    if args.backend == "bitz":
        return f"bitz@{args.log_inv_rate}"
    if args.backend == "binius64-ligerito":
        return f"binius64-ligerito-{args.binius_ligerito_accounting}@{args.log_inv_rate}"
    if args.backend in ("binius64", "plonky3-fri"):
        return f"{args.backend}@{args.log_inv_rate}"
    return args.backend


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--output", type=Path, required=True, help="new launcher campaign directory")
    parser.add_argument("--workload", required=True, choices=["u32-mod32", "u64", "u128"])
    parser.add_argument("--backend", required=True)
    parser.add_argument("--log-n", type=int, required=True)
    parser.add_argument("--threads", type=int, required=True)
    parser.add_argument("--log-inv-rate", type=int, default=1)
    parser.add_argument("--bitz-profile", default=None, help="Ligerito profile for BitZ cells, default custom:<rate>:4")
    parser.add_argument("--binius-ligerito-accounting", default="rbr")
    parser.add_argument("--reps", type=int, default=5)
    parser.add_argument("--swap-grow-gb", type=float, default=4.0, help="the launcher's own swap guard (ignored with --no-gate)")
    parser.add_argument("--no-gate", action="store_true",
                        help="run inside an outer bench_gate lock: skip the launcher's gate and enforce the probe's own limits")
    parser.add_argument("--kill-at-rss-gb", type=float, default=None,
                        help="with --no-gate: kill the cell once the process tree's resident set passes this (default 85%% of RAM)")
    parser.add_argument("--kill-at-compressed-mb", type=int, default=1024,
                        help="kill/exclude the cell once this many MiB of its own pages have been compressed out")
    parser.add_argument("--kill-at-pageins", type=int, default=32768,
                        help="kill/exclude the cell once its process tree paged in this many pages after starting")
    parser.add_argument("--kill-at-swapout-pages", type=int, default=262144,
                        help="with --no-gate: thrash guard; kill the cell once the machine swapped out this many pages while it ran")
    parser.add_argument("--record", type=Path, required=True, help="JSONL file to append the cell's record to")
    parser.add_argument("--label", default=None)
    parser.add_argument("--extra", nargs=argparse.REMAINDER, default=[], help="extra Rust-side flags after --")
    args = parser.parse_args(argv)
    if args.output.exists():
        raise SystemExit(f"output exists: {args.output}")
    experiment = ["proof", "--workload", args.workload, "--backends", args.backend, "--log-n", str(args.log_n),
                  "--threads", str(args.threads), "--reps", str(args.reps), "--memory", "rss", "--skip-unsupported"]
    if args.backend == "bitz":
        experiment += ["--bitz-profile", "100", "--ligerito", args.bitz_profile or f"custom:{args.log_inv_rate}:4"]
    if args.backend in ("binius64", "binius64-ligerito", "plonky3-fri"):
        experiment += ["--log-inv-rate", str(args.log_inv_rate)]
    if args.backend == "binius64-ligerito":
        experiment += ["--binius-ligerito-accounting", args.binius_ligerito_accounting]
    experiment += [flag for flag in args.extra if flag != "--"]
    label = args.label or args.output.name
    command = [sys.executable, str(ROOT / "scripts/run_multiplication_benchmarks.py"), "compare",
               "--output", str(args.output)]
    command += ["--no-gate"] if args.no_gate else ["--swap-grow-gb", str(args.swap_grow_gb)]
    command += ["--", *experiment]
    ram = machine_ram_bytes()
    rss_limit = int((args.kill_at_rss_gb or 0) * 2**30) if args.kill_at_rss_gb else (int(ram * 0.85) if ram else None)
    swap_before = swapouts()
    free_before = free_memory_bytes()
    compressed_limit = args.kill_at_compressed_mb * 2**20
    peak = 0
    peak_compressed = 0
    pageins_base = None
    pageins_seen = 0
    killed = None
    process = subprocess.Popen(command, cwd=ROOT)
    stop = threading.Event()

    def sample():
        nonlocal peak, peak_compressed, pageins_base, pageins_seen, killed
        last_top = 0.0
        while not stop.is_set():
            peak = max(peak, process_tree_rss(process.pid))
            if time.monotonic() - last_top >= 2.0:
                last_top = time.monotonic()
                memory = process_tree_memory(process.pid)
                if memory is not None:
                    _, compressed, pageins = memory
                    peak_compressed = max(peak_compressed, compressed)
                    if pageins_base is None:
                        pageins_base = pageins
                    pageins_seen = max(pageins_seen, pageins - pageins_base)
            if args.no_gate and killed is None:
                current_swap = swapouts()
                grown = None if current_swap is None or swap_before is None else current_swap - swap_before
                if rss_limit and peak > rss_limit:
                    killed = f"the probe killed the cell: its process tree reached {peak / 2**30:.1f} GiB, above the {rss_limit / 2**30:.1f} GiB limit"
                elif peak_compressed > compressed_limit:
                    killed = (f"the probe killed the cell: {peak_compressed / 2**30:.1f} GiB of its own pages had been compressed out "
                              f"at {peak / 2**30:.1f} GiB resident (limit {args.kill_at_compressed_mb} MiB)")
                elif pageins_seen > args.kill_at_pageins:
                    killed = (f"the probe killed the cell: it paged {pageins_seen} of its pages back in "
                              f"at {peak / 2**30:.1f} GiB resident (limit {args.kill_at_pageins})")
                elif grown is not None and grown > args.kill_at_swapout_pages:
                    killed = (f"the probe killed the cell: the machine swapped out {grown} pages while it ran "
                              f"(thrash guard, limit {args.kill_at_swapout_pages})")
                if killed:
                    kill_tree(process.pid)
            time.sleep(0.25)

    sampler = threading.Thread(target=sample, daemon=True)
    sampler.start()
    code = process.wait()
    stop.set()
    sampler.join(timeout=2)
    swap_after = swapouts()
    swapped = None if swap_before is None or swap_after is None else swap_after - swap_before
    manifest = args.output / "manifest.json"
    complete = manifest.exists() and json.loads(manifest.read_text()).get("status") == "complete"
    measured_cases = 0
    if complete:
        measured_cases = sum(c.get("status") == "measured" for c in json.loads(manifest.read_text())["cases"])
    if killed:
        status, reason = "excluded", killed + ": its working set does not fit the machine's memory"
    elif code == 86:
        status, reason = "excluded", f"the swap-growth guard ({args.swap_grow_gb:g} GiB) aborted the prover: its working set does not fit the machine's memory"
    elif code != 0 or not complete or measured_cases == 0:
        status, reason = "excluded", f"the launcher exited with {code} (campaign {'complete' if complete else 'incomplete'}, {measured_cases} measured); see {args.output / 'run.log'}"
    elif peak_compressed > compressed_limit or pageins_seen > args.kill_at_pageins:
        status, reason = "excluded", (f"{peak_compressed / 2**30:.1f} GiB of the cell's own pages were compressed out and {pageins_seen} paged back in "
                                      f"while it ran: the measurement is paging-dominated")
    else:
        status, reason = "measured", ""
    record = dict(workload=args.workload, scheme=scheme_of(args), backend=args.backend, log_n=args.log_n, threads=args.threads,
                  status=status, reason=reason, observed_peak_rss_bytes=peak, peak_compressed_bytes=peak_compressed,
                  pageins=pageins_seen, swapouts=swapped, free_memory_at_start_bytes=free_before, machine_ram_bytes=ram,
                  exit_code=code, campaign=str(args.output), label=label)
    with args.record.open("a") as stream:
        stream.write(json.dumps(record) + "\n")
    print(json.dumps(record), flush=True)
    return 0 if status == "measured" else 3


if __name__ == "__main__":
    raise SystemExit(main())

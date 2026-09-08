#!/usr/bin/env python3
"""Run the matched F2Z/Limber MultiSwap performance campaign.

The runner creates a new immutable run directory, executes six cells per
workload ``k``, validates every proof trace before moving on, and finally
renders canonical and combined reports. Use ``--dry-run`` to print the complete
execution plan without creating files or compiling benchmarks.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import platform
import re
import shlex
import subprocess
import sys
from datetime import datetime, timezone
from pathlib import Path
from typing import Any, Sequence

import matched_multiswap_report as report


PROFILER_DEFAULT = Path(
    "/Users/johnwu/.ai-agent-army/skills/zk-proof-profiler/scripts/zk_trace.py"
)
WORKLOAD_DISCLOSURE = (
    "Synthetic wired MultiSwap/RSA cost-model. Modeled hash and Poseidon costs "
    "are protocol accounting, not native hash executions."
)


def _capture(command: list[str], cwd: Path | None = None) -> str | None:
    try:
        result = subprocess.run(
            command,
            cwd=cwd,
            check=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.DEVNULL,
            text=True,
        )
    except (OSError, subprocess.CalledProcessError):
        return None
    value = result.stdout.strip()
    return value or None


def detect_performance_cores() -> tuple[int, str]:
    """Return physical performance cores and the source used to identify them."""
    sysctl = _capture(["sysctl", "-n", "hw.perflevel0.physicalcpu"])
    if sysctl and sysctl.isdecimal() and int(sysctl) > 0:
        return int(sysctl), "sysctl hw.perflevel0.physicalcpu"

    hardware = _capture(["system_profiler", "SPHardwareDataType"])
    if hardware:
        match = re.search(r"\((\d+)\s+performance\s+and\s+\d+\s+efficiency\)", hardware, re.I)
        if not match:
            match = re.search(r"(\d+)\s+Performance\s+Cores?", hardware, re.I)
        if match and int(match.group(1)) > 0:
            return int(match.group(1)), "system_profiler performance-core count"

    cores_per_socket = _capture(["lscpu", "-p=CORE,SOCKET"])
    if cores_per_socket:
        physical = {
            line.strip()
            for line in cores_per_socket.splitlines()
            if line.strip() and not line.startswith("#")
        }
        if physical:
            return len(physical), "lscpu physical core count"
    raise report.CampaignError(
        "could not detect physical performance cores; pass --all-threads explicitly"
    )


def _git_metadata(root: Path) -> dict[str, Any]:
    revision = _capture(["git", "rev-parse", "HEAD"], root)
    status = _capture(["git", "status", "--porcelain", "--untracked-files=all"], root)
    return {
        "root": str(root),
        "git_revision": revision,
        "git_dirty": status is not None,
    }


def _cpu_name() -> str:
    hardware = _capture(["system_profiler", "SPHardwareDataType"])
    if hardware:
        match = re.search(r"^\s*Chip:\s*(.+)$", hardware, re.MULTILINE)
        if match:
            return match.group(1).strip()
    brand = _capture(["sysctl", "-n", "machdep.cpu.brand_string"])
    return brand or platform.processor() or platform.machine()


def build_cells(
    *,
    f2z_root: Path,
    limber_root: Path,
    run_dir: Path,
    campaign_id: str,
    samples: int,
    warmups: int,
    all_threads: int,
    rustflags: str,
    expected_digests: dict[int, str],
    k_values: Sequence[int],
) -> list[dict[str, Any]]:
    thread_modes = (("single", 1), ("performance", all_threads))
    cells: list[dict[str, Any]] = []
    cpu = _cpu_name()
    for workload_k in k_values:
        for thread_mode, threads in thread_modes:
            cell_id = f"k{workload_k}-f2z-{thread_mode}"
            trace = run_dir / "raw" / f"{cell_id}.jsonl"
            environment = {
                "OBLONG_PROFILE_INTERVALS": "1",
                "F2Z_MULTISWAP_TRACE_PATH": str(trace),
                "F2Z_MULTISWAP_CAMPAIGN_ID": campaign_id,
                "F2Z_MULTISWAP_BUILD_PROFILE": "bench",
                "F2Z_MULTISWAP_CPU": cpu,
                "F2Z_BENCH_REPS": str(samples),
                "F2Z_BENCH_SHAPES": str(workload_k),
                "RAYON_NUM_THREADS": str(threads),
                "RUSTFLAGS": rustflags,
            }
            if workload_k in expected_digests:
                environment["F2Z_MULTISWAP_EXPECTED_CONSTRAINT_DIGEST"] = expected_digests[workload_k]
            cells.append(
                {
                    "cell_id": cell_id,
                    "label": f"k={workload_k} · F2Z · {threads} thread{'s' if threads != 1 else ''}",
                    "implementation": "f2z-ligerito",
                    "backend": "virtual-f2z",
                    "workload_k": workload_k,
                    "thread_mode": thread_mode,
                    "rayon_threads": threads,
                    "cwd": str(f2z_root),
                    "trace": str(Path("..") / "raw" / trace.name),
                    "log": str(Path("..") / "logs" / f"{cell_id}.log"),
                    "command": ["cargo", "bench", "--bench", "multiswap", "--features", "unchecked"],
                    "environment": environment,
                    "status": "planned",
                }
            )
        for backend, backend_label in (("hyrax", "Hyrax"), ("brakedown", "Brakedown")):
            for thread_mode, threads in thread_modes:
                cell_id = f"k{workload_k}-limber-{backend}-{thread_mode}"
                trace = run_dir / "raw" / f"{cell_id}.jsonl"
                environment = {
                    "MATCHED_BENCH": "1",
                    "MSCFG": "paper",
                    "MATCHED_BACKEND": backend,
                    "MATCHED_CAMPAIGN_ID": campaign_id,
                    "MATCHED_K": str(workload_k),
                    "MATCHED_TRACE_PATH": str(trace),
                    "MATCHED_SAMPLES": str(samples),
                    "MATCHED_WARMUPS": str(warmups),
                    "RAYON_NUM_THREADS": str(threads),
                    "RUSTFLAGS": rustflags,
                    "CARGO_TARGET_DIR": str(run_dir / "build" / "limber-target"),
                }
                cells.append(
                    {
                        "cell_id": cell_id,
                        "label": f"k={workload_k} · Limber {backend_label} · {threads} thread{'s' if threads != 1 else ''}",
                        "implementation": f"limber-{backend}",
                        "backend": backend,
                        "workload_k": workload_k,
                        "thread_mode": thread_mode,
                        "rayon_threads": threads,
                        "cwd": str(limber_root),
                        "trace": str(Path("..") / "raw" / trace.name),
                        "log": str(Path("..") / "logs" / f"{cell_id}.log"),
                        "command": [
                            "rustup",
                            "run",
                            "nightly-2026-07-01",
                            "cargo",
                            "bench",
                            "--bench",
                            "multiswap_modp",
                        ],
                        "environment": environment,
                        "status": "planned",
                    }
                )
    ordered: list[dict[str, Any]] = []
    implementations = ("f2z-ligerito", "limber-hyrax", "limber-brakedown")
    for k_index, workload_k in enumerate(k_values):
        rotation = k_index % len(implementations)
        implementation_order = implementations[rotation:] + implementations[:rotation]
        thread_order = (
            ("single", "performance")
            if k_index % 2 == 0
            else ("performance", "single")
        )
        by_key = {
            (cell["implementation"], cell["thread_mode"]): cell
            for cell in cells
            if cell["workload_k"] == workload_k
        }
        for implementation in implementation_order:
            for thread_mode in thread_order:
                cell = by_key[(implementation, thread_mode)]
                cell["execution_index"] = len(ordered)
                ordered.append(cell)
    return ordered


def build_manifest(
    *,
    campaign_id: str,
    run_dir: Path,
    f2z_root: Path,
    limber_root: Path,
    samples: int,
    warmups: int,
    all_threads: int,
    core_detection: str,
    cells: list[dict[str, Any]],
    k_values: Sequence[int],
    expected_digests: dict[int, str],
) -> dict[str, Any]:
    return {
        "schema": report.CAMPAIGN_SCHEMA,
        "trace_schema": report.TRACE_SCHEMA,
        "campaign_id": campaign_id,
        "created_at_utc": datetime.now(timezone.utc).isoformat(),
        "status": "planned",
        "workload": {
            "name": "wired MultiSwap/RSA cost-model",
            "configuration": "paper",
            "workload_k_values": list(k_values),
            "source_reference_k": 0,
            "k_semantics": "k=0 is source-backed; k>0 adds modeled per-swap H_delta groups",
            "disclosure": WORKLOAD_DISCLOSURE,
        },
        "sampling": {"warmups": warmups, "samples": samples},
        "digest_expectations": {
            str(workload_k): digest
            for workload_k, digest in sorted(expected_digests.items())
        },
        "thread_modes": [
            {"name": "single", "rayon_threads": 1, "affinity": "unpinned"},
            {
                "name": "performance",
                "rayon_threads": all_threads,
                "affinity": "unpinned; worker count equals detected physical performance cores",
            },
        ],
        "execution_order": {
            "policy": "rotate implementation start by k ordinal; alternate thread-mode order",
            "cell_ids": [cell["cell_id"] for cell in cells],
        },
        "host": {
            "cpu": _cpu_name(),
            "machine": platform.machine(),
            "platform": platform.platform(),
            "detected_physical_performance_core_count": all_threads,
            "performance_core_detection": core_detection,
            "affinity": "unpinned",
        },
        "repositories": {
            "f2z": _git_metadata(f2z_root),
            "limber": _git_metadata(limber_root),
        },
        "paths": {"run_dir": str(run_dir)},
        "cells": cells,
    }


def _write_manifest(path: Path, manifest: dict[str, Any]) -> None:
    temporary = path.with_suffix(".json.tmp")
    temporary.write_text(
        json.dumps(manifest, indent=2, sort_keys=True, ensure_ascii=False) + "\n",
        encoding="utf-8",
    )
    temporary.replace(path)


def _display_command(cell: dict[str, Any]) -> str:
    env = " ".join(
        f"{key}={shlex.quote(value)}" for key, value in sorted(cell["environment"].items())
    )
    command = " ".join(shlex.quote(item) for item in cell["command"])
    return f"cd {shlex.quote(cell['cwd'])} && {env} {command}"


def _run_logged(cell: dict[str, Any], run_dir: Path) -> None:
    log_path = (run_dir / "metadata" / cell["log"]).resolve()
    trace_path = (run_dir / "metadata" / cell["trace"]).resolve()
    if trace_path.exists():
        raise report.CampaignError(f"refusing to overwrite trace {trace_path}")
    process_environment = os.environ.copy()
    process_environment.update(cell["environment"])
    with log_path.open("w", encoding="utf-8") as log:
        log.write(f"$ {_display_command(cell)}\n\n")
        log.flush()
        process = subprocess.Popen(
            cell["command"],
            cwd=cell["cwd"],
            env=process_environment,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            text=True,
            bufsize=1,
        )
        assert process.stdout is not None
        for line in process.stdout:
            sys.stdout.write(line)
            sys.stdout.flush()
            log.write(line)
        return_code = process.wait()
    if return_code:
        raise report.CampaignError(
            f"cell {cell['cell_id']} exited with status {return_code}; see {log_path}"
        )
    if not trace_path.is_file() or trace_path.stat().st_size == 0:
        raise report.CampaignError(f"cell {cell['cell_id']} produced no trace at {trace_path}")


def _canonical_validate(profiler: Path, trace_path: Path) -> None:
    result = subprocess.run(
        [sys.executable, str(profiler), "validate", str(trace_path), "--warnings-as-errors"],
        check=False,
    )
    if result.returncode:
        raise report.CampaignError(f"canonical zkperf validation failed for {trace_path}")


def _resolve_cell_trace(manifest_path: Path, cell: dict[str, Any]) -> Path:
    return (manifest_path.parent / cell["trace"]).resolve()


def _write_combined_trace(
    manifest: dict[str, Any], manifest_path: Path, traces: list[Path]
) -> Path:
    combined = manifest_path.parent.parent / "traces" / "combined.jsonl"
    digest = hashlib.sha256()
    with combined.open("xb") as output:
        for trace in traces:
            with trace.open("rb") as source:
                while chunk := source.read(1024 * 1024):
                    output.write(chunk)
                    digest.update(chunk)
    manifest["derived"] = {
        "combined_trace": "../traces/combined.jsonl",
        "combined_trace_sha256": digest.hexdigest(),
        "source_order": [path.name for path in traces],
    }
    _write_manifest(manifest_path, manifest)
    return combined


def execute_campaign(
    manifest: dict[str, Any],
    manifest_path: Path,
    profiler: Path,
    report_dir: Path,
    canonical_report_dir: Path,
) -> None:
    if not profiler.is_file():
        raise report.CampaignError(f"zk-proof-profiler validator not found: {profiler}")
    manifest["status"] = "running"
    _write_manifest(manifest_path, manifest)
    expected_digests: dict[int, str] = {
        int(workload_k): digest
        for workload_k, digest in manifest.get("digest_expectations", {}).items()
    }
    expected_domains: dict[int, str] = {
        workload_k: report.STATEMENT_DOMAIN for workload_k in expected_digests
    }
    assignment_digests: dict[int, str] = {}
    completed: list[Path] = []
    for cell in manifest["cells"]:
        workload_k = cell["workload_k"]
        cell["status"] = "running"
        _write_manifest(manifest_path, manifest)
        print(f"\n==> {cell['label']}")
        print(_display_command(cell))
        try:
            if cell["implementation"] == "f2z-ligerito" and workload_k in expected_digests:
                cell["environment"]["F2Z_MULTISWAP_EXPECTED_CONSTRAINT_DIGEST"] = expected_digests[workload_k]
            _run_logged(cell, manifest_path.parent.parent)
            trace_path = _resolve_cell_trace(manifest_path, cell)
            _canonical_validate(profiler, trace_path)
            loaded = report.load_cell_trace(
                manifest_path,
                {**cell, "status": "ok"},
                manifest["sampling"]["warmups"],
                manifest["sampling"]["samples"],
            )
            if workload_k not in expected_digests:
                expected_domains[workload_k] = loaded.statement_domain
                expected_digests[workload_k] = loaded.statement_digest
            elif (loaded.statement_domain, loaded.statement_digest) != (
                expected_domains[workload_k],
                expected_digests[workload_k],
            ):
                raise report.CampaignError(
                    f"canonical digest mismatch in {cell['cell_id']}: "
                    f"{loaded.statement_domain}:{loaded.statement_digest}, expected "
                    f"{expected_domains[workload_k]}:{expected_digests[workload_k]}"
                )
            if loaded.assignment_digest:
                if workload_k not in assignment_digests:
                    assignment_digests[workload_k] = loaded.assignment_digest
                elif loaded.assignment_digest != assignment_digests[workload_k]:
                    raise report.CampaignError(
                        f"integer assignment digest mismatch in {cell['cell_id']}"
                    )
            cell["trace_sha256"] = loaded.source_sha256
            cell["status"] = "ok"
            completed.append(trace_path)
            manifest.setdefault("canonical_statements", {})[str(workload_k)] = {
                "domain": expected_domains[workload_k],
                "digest_blake3": expected_digests[workload_k],
            }
            if workload_k in assignment_digests:
                manifest.setdefault("assignment_digests_blake3", {})[
                    str(workload_k)
                ] = assignment_digests[workload_k]
            _write_manifest(manifest_path, manifest)
        except Exception as error:
            cell["status"] = "rejected"
            cell["error"] = str(error)
            manifest["status"] = "failed"
            _write_manifest(manifest_path, manifest)
            if isinstance(error, report.CampaignError):
                raise
            raise report.CampaignError(f"cell {cell['cell_id']} failed: {error}") from error

    try:
        validated_manifest, loaded_cells = report.validate_campaign(manifest_path)
        summary = report.build_summary(validated_manifest, loaded_cells)
        report.write_report(summary, report_dir)
        combined_trace = _write_combined_trace(manifest, manifest_path, completed)
        _canonical_validate(profiler, combined_trace)
        canonical_report_dir.mkdir(parents=True, exist_ok=False)
        canonical = subprocess.run(
            [
                sys.executable,
                str(profiler),
                "report",
                str(combined_trace),
                "--out-dir",
                str(canonical_report_dir),
                "--title",
                "Matched MultiSwap F2Z / Limber campaign",
            ],
            check=False,
        )
        if canonical.returncode:
            raise report.CampaignError("canonical combined report generation failed")
    except Exception as error:
        manifest["status"] = "failed"
        manifest["finalization_error"] = str(error)
        _write_manifest(manifest_path, manifest)
        if isinstance(error, report.CampaignError):
            raise
        raise report.CampaignError(f"campaign finalization failed: {error}") from error
    manifest["status"] = "ok"
    manifest.pop("finalization_error", None)
    _write_manifest(manifest_path, manifest)


def _parse_k_values(raw: str) -> tuple[int, ...]:
    pieces = [piece for piece in re.split(r"[\s,]+", raw.strip()) if piece]
    try:
        values = tuple(int(piece) for piece in pieces)
    except ValueError as error:
        raise report.CampaignError("--k-values must contain non-negative decimal integers") from error
    if not values or any(value < 0 for value in values) or len(values) != len(set(values)):
        raise report.CampaignError("--k-values must be a non-empty list of unique non-negative integers")
    return values


def _parse_expected_digests(raw_values: Sequence[str]) -> dict[int, str]:
    parsed: dict[int, str] = {}
    for raw in raw_values:
        key, separator, digest = raw.partition("=")
        if not separator or not key.isdecimal() or not digest:
            raise report.CampaignError("--expected-digest must use K=DIGEST")
        if not re.fullmatch(r"[0-9a-f]{64}", digest):
            raise report.CampaignError(
                "--expected-digest values must be 64 lowercase hexadecimal characters"
            )
        workload_k = int(key)
        if workload_k in parsed:
            raise report.CampaignError(f"duplicate expected digest for k={workload_k}")
        parsed[workload_k] = digest
    return parsed


def parse_args(argv: Sequence[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--f2z-root", type=Path, default=Path(__file__).resolve().parents[1])
    parser.add_argument("--limber-root", type=Path)
    parser.add_argument("--output-dir", type=Path)
    parser.add_argument("--campaign-id")
    parser.add_argument("--samples", type=int, default=5)
    parser.add_argument("--warmups", type=int, choices=(1,), default=1)
    parser.add_argument("--k-values", default="0,1,2,4,8")
    parser.add_argument(
        "--all-threads",
        type=int,
        help="physical performance-core count; auto-detected when omitted",
    )
    parser.add_argument(
        "--expected-digest",
        action="append",
        default=[],
        metavar="K=DIGEST",
        help="repeatable hard expectation for one workload k",
    )
    parser.add_argument("--rustflags", default="-Ctarget-cpu=native")
    parser.add_argument("--profiler", type=Path, default=PROFILER_DEFAULT)
    parser.add_argument("--dry-run", action="store_true")
    return parser.parse_args(argv)


def main(argv: Sequence[str] | None = None) -> int:
    args = parse_args(argv)
    try:
        if args.samples < 1:
            raise report.CampaignError("--samples must be positive")
        k_values = _parse_k_values(args.k_values)
        expected_digests = _parse_expected_digests(args.expected_digest)
        unexpected = set(expected_digests) - set(k_values)
        if unexpected:
            raise report.CampaignError(
                f"expected digests provided for k values outside the sweep: {sorted(unexpected)}"
            )
        if args.all_threads is not None:
            if args.all_threads < 1:
                raise report.CampaignError("--all-threads must be positive")
            all_threads, core_detection = args.all_threads, "explicit --all-threads"
        else:
            all_threads, core_detection = detect_performance_cores()
        stamp = datetime.now(timezone.utc).strftime("%Y-%m-%dT%H-%M-%SZ")
        campaign_id = args.campaign_id or stamp
        f2z_root = args.f2z_root.resolve()
        limber_root = (args.limber_root or (f2z_root.parent / "limber-impl")).resolve()
        run_dir = (
            args.output_dir.resolve()
            if args.output_dir
            else f2z_root / "PerfRuns" / f"{stamp}-matched-multiswap-k-sweep"
        )
        cells = build_cells(
            f2z_root=f2z_root,
            limber_root=limber_root,
            run_dir=run_dir,
            campaign_id=campaign_id,
            samples=args.samples,
            warmups=args.warmups,
            all_threads=all_threads,
            rustflags=args.rustflags,
            expected_digests=expected_digests,
            k_values=k_values,
        )
        manifest = build_manifest(
            campaign_id=campaign_id,
            run_dir=run_dir,
            f2z_root=f2z_root,
            limber_root=limber_root,
            samples=args.samples,
            warmups=args.warmups,
            all_threads=all_threads,
            core_detection=core_detection,
            cells=cells,
            k_values=k_values,
            expected_digests=expected_digests,
        )
        if args.dry_run:
            plan = {
                "manifest": manifest,
                "commands": [_display_command(cell) for cell in cells],
            }
            print(json.dumps(plan, indent=2, sort_keys=True, ensure_ascii=False))
            return 0
        if run_dir.exists():
            raise report.CampaignError(f"refusing to overwrite run directory {run_dir}")
        if not f2z_root.is_dir() or not limber_root.is_dir():
            raise report.CampaignError("F2Z and Limber repository roots must both exist")
        for directory in (
            run_dir / "raw",
            run_dir / "logs",
            run_dir / "metadata",
            run_dir / "reports",
            run_dir / "traces",
            run_dir / "build" / "limber-target",
        ):
            directory.mkdir(parents=True, exist_ok=False)
        manifest_path = run_dir / "metadata" / "campaign.json"
        _write_manifest(manifest_path, manifest)
        execute_campaign(
            manifest,
            manifest_path,
            args.profiler.resolve(),
            run_dir / "reports" / "combined",
            run_dir / "reports" / "canonical",
        )
        print(f"Campaign manifest: {manifest_path}")
        print(f"Combined report: {run_dir / 'reports' / 'combined' / 'intervals.html'}")
        print(f"Canonical report: {run_dir / 'reports' / 'canonical' / 'intervals.html'}")
        return 0
    except report.CampaignError as error:
        print(f"error: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())

#!/usr/bin/env python3
"""Capture the optional, instrumented F2Z PCS interval series.

Each exponent is written to a hidden same-directory partial, validated, and
then atomically promoted. This command intentionally does not run clean timing,
aggregate traces, export tables, or build the HTML dashboard.
"""

from __future__ import annotations

import argparse
import os
import subprocess
import sys
import tempfile
import time
from datetime import datetime, timezone
from pathlib import Path
from typing import Sequence

from bench_csv import (
    DEFAULT_RUSTFLAGS,
    SERIES_GAP_SECONDS,
    SERIES_REPS,
    SERIES_SHAPE_VALUES,
    SERIES_THREADS,
    BenchError,
    Shape,
    build_benchmark,
    cargo_command,
    nonnegative_float,
    parse_shapes,
    positive_int,
    parse_profile,
)
from export_f2z_intervals import ExportError, validate_part


REPO = Path(__file__).resolve().parent.parent
REPS = SERIES_REPS


class IntervalError(RuntimeError):
    """An instrumented capture failed before validated promotion."""


def parse_args(argv: Sequence[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--profile", required=True, type=parse_profile)
    parser.add_argument("--out-dir", required=True, type=Path)
    parser.add_argument("--shapes", default=" ".join(SERIES_SHAPE_VALUES))
    parser.add_argument("--threads", type=positive_int, default=SERIES_THREADS)
    parser.add_argument(
        "--gap-seconds", type=nonnegative_float, default=SERIES_GAP_SECONDS
    )
    parser.add_argument(
        "--toolchain",
        default=os.environ.get("F2Z_RUST_TOOLCHAIN", "1.97.1"),
    )
    return parser.parse_args(argv)


def command_output(arguments: list[str]) -> str:
    try:
        result = subprocess.run(
            arguments,
            cwd=REPO,
            stdout=subprocess.PIPE,
            stderr=subprocess.DEVNULL,
            text=True,
            encoding="utf-8",
            errors="replace",
            check=False,
        )
    except FileNotFoundError:
        return ""
    return result.stdout.strip() if result.returncode == 0 else ""


def cpu_name() -> str:
    name = command_output(["sysctl", "-n", "machdep.cpu.brand_string"])
    if name:
        return name
    hardware = command_output(["system_profiler", "SPHardwareDataType"])
    for line in hardware.splitlines():
        key, separator, value = line.strip().partition(":")
        if separator and key == "Chip" and value.strip():
            return value.strip()
    return "unknown"


def unique_failed_path(output_dir: Path, exponent: int) -> Path:
    stem = f".n{exponent}.failed-{datetime.now():%Y%m%d-%H%M%S}"
    candidate = output_dir / stem
    suffix = 1
    while candidate.exists():
        candidate = output_dir / f"{stem}-{suffix}"
        suffix += 1
    return candidate


def preserve_failed_partial(partial: Path, output_dir: Path, exponent: int) -> Path:
    failed = unique_failed_path(output_dir, exponent)
    try:
        partial.rename(failed)
    except OSError as error:
        print(
            f"could not rename failed interval capture; preserved at {partial}: {error}",
            file=sys.stderr,
        )
        return partial
    return failed


def capture_shape(
    shape: Shape,
    *,
    output_dir: Path,
    selected_profile: str,
    threads: int,
    toolchain: str,
    rustflags: str,
    cpu: str,
    capture_id: str,
) -> None:
    exponent = shape.exponent
    partial = Path(
        tempfile.mkdtemp(prefix=f".n{exponent}.partial.", dir=output_dir)
    )
    print(f">> intervals i={exponent} profile={selected_profile}", file=sys.stderr)
    environment = os.environ.copy()
    environment.pop("OBLONG_PROFILE", None)
    environment.pop("F2_FOREST_SCHEDULE", None)
    environment.update(
        F2Z_BENCH_INTERVALS="1",
        F2Z_INTERVAL_OUT_DIR=str(partial),
        F2Z_LIG_PROFILE=selected_profile,
        F2Z_BENCH_SHAPES=shape.bench_value,
        F2Z_BENCH_REPS=str(REPS),
        F2Z_BENCH_FILL="1",
        F2Z_COL_ELIDE="1",
        F2Z_INTERVAL_CPU=cpu,
        F2Z_INTERVAL_CAPTURE_ID=capture_id,
        RAYON_NUM_THREADS=str(threads),
        RUSTFLAGS=rustflags,
    )

    try:
        with (partial / "console.log").open("wb") as console:
            result = subprocess.run(
                cargo_command(toolchain, "unchecked,interval-profiling"),
                cwd=REPO,
                env=environment,
                stdout=console,
                stderr=subprocess.STDOUT,
                check=False,
            )
        if result.returncode != 0:
            raise IntervalError(f"Rust interval capture exited {result.returncode}")
        validate_part(
            partial,
            expected_profile=selected_profile,
            expected_exponent=exponent,
            expected_threads=threads,
        )
        partial.rename(output_dir / f"n{exponent}")
    except BaseException:
        failed = preserve_failed_partial(partial, output_dir, exponent)
        print(f"interval capture failed; preserved at {failed}", file=sys.stderr)
        raise


def run(args: argparse.Namespace) -> None:
    shapes = parse_shapes(args.shapes)
    scheduled = [shape for shape in shapes if shape.forest_schedule is not None]
    if scheduled:
        raise IntervalError("interval shapes do not support @l8 schedules")
    exponents = [shape.exponent for shape in shapes]
    if len(set(exponents)) != len(exponents):
        raise IntervalError(f"duplicate interval exponents: {exponents}")

    output_dir = args.out_dir
    output_dir.parent.mkdir(parents=True, exist_ok=True)
    try:
        output_dir.mkdir()
    except FileExistsError as error:
        raise IntervalError(f"{output_dir} already exists") from error

    rustflags = os.environ.get("RUSTFLAGS") or DEFAULT_RUSTFLAGS
    build_environment = os.environ.copy()
    build_environment["RUSTFLAGS"] = rustflags
    build_benchmark(
        args.toolchain,
        "unchecked,interval-profiling",
        build_environment,
    )
    time.sleep(args.gap_seconds)

    capture_id = (
        f"{datetime.now(timezone.utc):%Y%m%dT%H%M%SZ}-{os.getpid()}"
    )
    detected_cpu = cpu_name()
    for index, shape in enumerate(shapes):
        capture_shape(
            shape,
            output_dir=output_dir,
            selected_profile=args.profile,
            threads=args.threads,
            toolchain=args.toolchain,
            rustflags=rustflags,
            cpu=detected_cpu,
            capture_id=capture_id,
        )
        if index + 1 < len(shapes):
            time.sleep(args.gap_seconds)

    print(f"wrote structured interval captures: {output_dir}", file=sys.stderr)


def main(argv: Sequence[str] | None = None) -> int:
    os.chdir(REPO)
    args = parse_args(argv)
    try:
        run(args)
    except (
        argparse.ArgumentTypeError,
        BenchError,
        ExportError,
        IntervalError,
        OSError,
        ValueError,
    ) as error:
        print(f"bench_pcs_intervals.py: {error}", file=sys.stderr)
        return 1
    except KeyboardInterrupt:
        print("bench_pcs_intervals.py: interrupted", file=sys.stderr)
        return 130
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

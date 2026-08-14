#!/usr/bin/env python3
"""Run the F2Z PCS benchmark one profile and shape per process.

The process boundary is part of the measurement protocol. Rust emits a small
machine-oriented record for each run; this collector validates that record and
writes the stable 34-column CSV consumed by the table exporter.
"""

from __future__ import annotations

import argparse
import csv
import os
import re
import subprocess
import sys
import time
from dataclasses import dataclass
from datetime import datetime
from pathlib import Path
from typing import Mapping, Sequence


REPO = Path(__file__).resolve().parent.parent
DEFAULT_RUSTFLAGS = "-C target-cpu=native"
DEFAULT_SHAPES = (
    "10:6:1",
    "12:6:1",
    "13:7:1",
    "14:8:1",
    "15:9:1",
    "16:10:1",
    "17:11:1",
)
BIG_SHAPES = ("18:12:1", "19:12:1@l8", "19:13:1@l8")
SERIES_SHAPE_VALUES = (
    "13:7:1",
    "13:8:1",
    "14:8:1",
    "14:9:1",
    "15:9:1",
    "15:10:1",
    "16:10:1",
    "16:11:1",
    "17:11:1",
    "17:12:1",
    "18:12:1",
)
SERIES_REPS = 21
SERIES_THREADS = 8
SERIES_GAP_SECONDS = 45.0
CSV_FIELDS = (
    "timestamp",
    "profile_arg",
    "lig_geometry",
    "n",
    "t",
    "s",
    "W",
    "chunks",
    "threads",
    "reps",
    "commit_ms",
    "commit_peak_mb",
    "forest_ms",
    "open_ms",
    "untagged_ms",
    "profiled_prove_ms",
    "prove_ms",
    "prove_peak_mb",
    "verify_ms",
    "proof_bytes",
    "forest_side_kib",
    "s_v_kib",
    "lig_kib",
    "serialize_us",
    "deserialize_us",
    "pack_ms",
    "pow2_ms",
    "forest_core_ms",
    "fold_v_ms",
    "presum_tables_ms",
    "presum_run_ms",
    "rings_ms",
    "basis_combine_ms",
    "ligerito_recursive_ms",
)
DETAIL_FIELDS = (
    "forest_ms",
    "open_ms",
    "untagged_ms",
    "profiled_prove_ms",
    "pack_ms",
    "pow2_ms",
    "forest_core_ms",
    "fold_v_ms",
    "presum_tables_ms",
    "presum_run_ms",
    "rings_ms",
    "basis_combine_ms",
    "ligerito_recursive_ms",
)
REQUIRED_FIELDS = tuple(field for field in CSV_FIELDS if field not in DETAIL_FIELDS)

HEADER_RE = re.compile(
    r"^=== n=(?P<n>\d+) \(t=(?P<t>\d+), s=(?P<s>\d+), "
    r"W=(?P<W>\d+), m_p=\d+, chunks=(?P<chunks>\d+), "
    r"lig=(?P<lig>[^,]+),"
)
SPLIT_RE = re.compile(
    r"^\s*split:\s+forest-side\s+(?P<forest>[0-9.]+)\s+KiB\s+\|\s+"
    r"open-side\s+[0-9.]+\s+KiB\s+\(s_v\s+(?P<sv>[0-9.]+)\s+"
    r"\+\s+lig\s+(?P<lig>[0-9.]+)\)"
)
PROFILE_RE = re.compile(r"^[A-Za-z0-9:_-]+$")


class BenchError(RuntimeError):
    """The benchmark command or its machine record was incomplete."""


@dataclass(frozen=True)
class Shape:
    t: int
    s: int
    word_bits: int
    forest_schedule: str | None = None

    @property
    def exponent(self) -> int:
        return self.t + self.s

    @property
    def bench_value(self) -> str:
        return f"{self.t}:{self.s}:{self.word_bits}"

    @classmethod
    def parse(cls, value: str) -> "Shape":
        triple, separator, schedule = value.partition("@")
        if separator and schedule != "l8":
            raise argparse.ArgumentTypeError(
                f"unsupported shape schedule in {value!r}; expected @l8"
            )
        pieces = triple.split(":")
        if len(pieces) != 3:
            raise argparse.ArgumentTypeError(
                f"invalid shape {value!r}; expected t:s:W"
            )
        try:
            t, s, word_bits = (int(piece) for piece in pieces)
        except ValueError as error:
            raise argparse.ArgumentTypeError(
                f"invalid shape {value!r}; expected integer t:s:W"
            ) from error
        if min(t, s, word_bits) <= 0:
            raise argparse.ArgumentTypeError(
                f"invalid shape {value!r}; all components must be positive"
            )
        return cls(t, s, word_bits, schedule or None)


@dataclass(frozen=True)
class SweepConfig:
    profiles: tuple[str | None, ...]
    shapes: tuple[Shape, ...]
    reps: int
    threads: int | None
    gap_seconds: float
    toolchain: str
    phases: bool = False
    skip_build: bool = False


def positive_int(value: str) -> int:
    parsed = int(value)
    if parsed <= 0:
        raise argparse.ArgumentTypeError("value must be positive")
    return parsed


def nonnegative_float(value: str) -> float:
    parsed = float(value)
    if parsed < 0:
        raise argparse.ArgumentTypeError("value must be nonnegative")
    return parsed


def parse_profile(value: str) -> str:
    if not PROFILE_RE.fullmatch(value):
        raise argparse.ArgumentTypeError(f"invalid profile: {value}")
    return value


def parse_shapes(value: str) -> tuple[Shape, ...]:
    shapes = tuple(Shape.parse(item) for item in value.split())
    if not shapes:
        raise argparse.ArgumentTypeError("at least one shape is required")
    return shapes


def parse_profiles(value: str) -> tuple[str | None, ...]:
    if not value:
        return (None,)
    values = tuple(parse_profile(item) for item in value.split(","))
    if not values:
        raise argparse.ArgumentTypeError("at least one profile is required")
    return values


def cargo_command(toolchain: str, features: str, *, no_run: bool = False) -> list[str]:
    command = [
        "rustup",
        "run",
        toolchain,
        "cargo",
        "bench",
        "--bench",
        "pcs",
        "--features",
        features,
    ]
    if no_run:
        command.append("--no-run")
    return command


def build_benchmark(
    toolchain: str,
    features: str,
    environment: Mapping[str, str],
    *,
    hide_stderr: bool = False,
) -> None:
    result = subprocess.run(
        cargo_command(toolchain, features, no_run=True),
        cwd=REPO,
        env=dict(environment),
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL if hide_stderr else None,
        check=False,
    )
    if result.returncode != 0:
        raise BenchError("build failed")


def key_values(line: str) -> dict[str, str]:
    return dict(token.split("=", 1) for token in line.split()[1:] if "=" in token)


def first_match(pattern: str, line: str) -> tuple[str, ...] | None:
    match = re.search(pattern, line)
    return match.groups() if match else None


def parse_bench_output(
    text: str,
    *,
    expected_profile: str,
    threads: str,
    reps: int,
    timestamp: str,
    require_phases: bool,
) -> dict[str, str]:
    """Parse one complete Rust benchmark block; machine details win."""

    row = {field: "" for field in CSV_FIELDS}
    row.update(timestamp=timestamp, threads=threads, reps=str(reps))
    machine_metrics: dict[str, str] = {}
    machine_phases: dict[str, str] = {}
    config: dict[str, str] = {}
    saw_header = False
    saw_split = False

    for line in text.splitlines():
        header = HEADER_RE.match(line)
        if header:
            if saw_header:
                raise BenchError("benchmark emitted more than one shape block")
            saw_header = True
            row.update(
                n=header.group("n"),
                t=header.group("t"),
                s=header.group("s"),
                W=header.group("W"),
                chunks=header.group("chunks"),
                lig_geometry=header.group("lig"),
            )
            continue

        stripped = line.strip()
        if stripped.startswith("config-detail:"):
            config = key_values(stripped)
            continue
        if stripped.startswith("metric-detail:"):
            machine_metrics = key_values(stripped)
            continue
        if stripped.startswith("phase-detail:"):
            machine_phases = key_values(stripped)
            continue

        values = first_match(
            r"^\s*commit:\s+([0-9.]+)\s+ms\s+peak\s+([0-9.]+)", line
        )
        if values:
            row["commit_ms"], row["commit_peak_mb"] = values
            continue
        values = first_match(
            r"^\s*prove:\s+([0-9.]+)\s+ms\s+peak\s+([0-9.]+)", line
        )
        if values:
            row["prove_ms"], row["prove_peak_mb"] = values
            continue
        values = first_match(r"^\s*verify:\s+([0-9.]+)\s+ms", line)
        if values:
            row["verify_ms"] = values[0]
            continue
        values = first_match(
            r"^\s*proof:\s+(\d+)\s+B.*serialize\s+([0-9.]+)\s+\S+\s+/\s+"
            r"deserialize\s+([0-9.]+)",
            line,
        )
        if values:
            row["proof_bytes"], row["serialize_us"], row["deserialize_us"] = values
            continue
        values = first_match(
            r"^\s*phases:\s+forest\+presum\s+([0-9.]+)\s+ms\s+\|\s+"
            r"ligerito open\s+([0-9.]+)\s+ms\s+\|\s+untagged\s+"
            r"([0-9.]+)\s+ms\s+\|\s+total\s+([0-9.]+)",
            line,
        )
        if values:
            (
                row["forest_ms"],
                row["open_ms"],
                row["untagged_ms"],
                row["profiled_prove_ms"],
            ) = values
            continue

        split = SPLIT_RE.match(line)
        if split:
            saw_split = True
            row["forest_side_kib"] = split.group("forest")
            row["s_v_kib"] = split.group("sv")
            row["lig_kib"] = split.group("lig")

    if not saw_header:
        raise BenchError("missing benchmark shape header")
    if not saw_split:
        raise BenchError("missing proof split terminator")
    requested = config.get("requested_ligerito", "")
    if requested != expected_profile:
        reported = requested or "<missing>"
        raise BenchError(
            f"Rust reported requested profile {reported}, expected {expected_profile}"
        )
    resolved = config.get("resolved_ligerito", "")
    if not resolved:
        raise BenchError("missing resolved_ligerito in config-detail")
    row["profile_arg"] = requested
    row["lig_geometry"] = resolved

    metric_mapping = {
        "commit_ms": "commit_ms",
        "commit_peak_mib": "commit_peak_mb",
        "prove_ms": "prove_ms",
        "prove_peak_mib": "prove_peak_mb",
        "verify_ms": "verify_ms",
        "proof_bytes": "proof_bytes",
        "serialize_us": "serialize_us",
        "deserialize_us": "deserialize_us",
    }
    for source, destination in metric_mapping.items():
        if source in machine_metrics:
            row[destination] = machine_metrics[source]

    phase_mapping = {
        "forest_total_ms": "forest_ms",
        "opening_total_ms": "open_ms",
        "untagged_ms": "untagged_ms",
        "profiled_prove_ms": "profiled_prove_ms",
        "pack_ms": "pack_ms",
        "pow2_ms": "pow2_ms",
        "forest_core_ms": "forest_core_ms",
        "fold_v_ms": "fold_v_ms",
        "presum_tables_ms": "presum_tables_ms",
        "presum_run_ms": "presum_run_ms",
        "rings_ms": "rings_ms",
        "basis_combine_ms": "basis_combine_ms",
        "ligerito_recursive_ms": "ligerito_recursive_ms",
    }
    for source, destination in phase_mapping.items():
        if source in machine_phases:
            row[destination] = machine_phases[source]

    required = REQUIRED_FIELDS + (DETAIL_FIELDS if require_phases else ())
    missing = [field for field in required if not row[field]]
    if missing:
        raise BenchError(f"incomplete benchmark record; missing {', '.join(missing)}")
    return row


def sweep_environment(
    config: SweepConfig, base_environment: Mapping[str, str] | None = None
) -> dict[str, str]:
    environment = dict(base_environment or os.environ)
    environment["RUSTFLAGS"] = environment.get("RUSTFLAGS") or DEFAULT_RUSTFLAGS
    if config.phases:
        environment["OBLONG_PROFILE"] = "1"
    else:
        environment.pop("OBLONG_PROFILE", None)
    if config.threads is not None:
        environment["RAYON_NUM_THREADS"] = str(config.threads)
    return environment


def capture_csv(
    output: Path,
    config: SweepConfig,
    *,
    base_environment: Mapping[str, str] | None = None,
) -> int:
    environment = sweep_environment(config, base_environment)
    if not config.skip_build:
        build_benchmark(
            config.toolchain,
            "unchecked",
            environment,
            hide_stderr=True,
        )

    output.parent.mkdir(parents=True, exist_ok=True)
    threads_label = environment.get("RAYON_NUM_THREADS", "all")
    written = 0
    with output.open("w", encoding="utf-8", newline="") as handle:
        writer = csv.DictWriter(handle, fieldnames=CSV_FIELDS, lineterminator="\n")
        writer.writeheader()
        handle.flush()
        for selected_profile in config.profiles:
            expected_profile = selected_profile or "slim"
            for shape in config.shapes:
                run_environment = environment.copy()
                if selected_profile is not None:
                    run_environment["F2Z_LIG_PROFILE"] = selected_profile
                else:
                    run_environment.pop("F2Z_LIG_PROFILE", None)
                if shape.forest_schedule is not None:
                    run_environment["F2_FOREST_SCHEDULE"] = shape.forest_schedule
                else:
                    run_environment.pop("F2_FOREST_SCHEDULE", None)
                run_environment["F2Z_BENCH_SHAPES"] = shape.bench_value
                run_environment["F2Z_BENCH_REPS"] = str(config.reps)
                schedule = (
                    f" sched={shape.forest_schedule}" if shape.forest_schedule else ""
                )
                print(
                    f">> profile={expected_profile} shape={shape.bench_value}{schedule}",
                    file=sys.stderr,
                )
                result = subprocess.run(
                    cargo_command(config.toolchain, "unchecked"),
                    cwd=REPO,
                    env=run_environment,
                    stdout=subprocess.PIPE,
                    stderr=subprocess.STDOUT,
                    text=True,
                    encoding="utf-8",
                    errors="replace",
                    check=False,
                )
                if result.returncode != 0:
                    print(result.stdout, file=sys.stderr, end="")
                    raise BenchError(
                        f"benchmark failed for {expected_profile} {shape.bench_value}"
                    )
                row = parse_bench_output(
                    result.stdout,
                    expected_profile=expected_profile,
                    threads=threads_label,
                    reps=config.reps,
                    timestamp=datetime.now().strftime("%Y-%m-%dT%H:%M:%S"),
                    require_phases=config.phases,
                )
                writer.writerow(row)
                handle.flush()
                written += 1
                time.sleep(config.gap_seconds)

    expected_rows = len(config.profiles) * len(config.shapes)
    if written != expected_rows:
        raise BenchError(
            f"incomplete CSV: expected {expected_rows} data rows, found {written}"
        )
    print(f"wrote {output}", file=sys.stderr)
    print(f"{written} data rows", file=sys.stderr)
    return written


def parse_args(argv: Sequence[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("-o", "--output", type=Path)
    parser.add_argument(
        "-p",
        "--profiles",
        default="",
        help="comma-separated Ligerito profiles; empty uses the Rust default",
    )
    parser.add_argument(
        "-s",
        "--shapes",
        default=" ".join(DEFAULT_SHAPES),
        help="space-separated t:s:W shapes; append @l8 for that forest schedule",
    )
    parser.add_argument("-r", "--reps", type=positive_int, default=3)
    parser.add_argument("-j", "--threads", type=positive_int)
    parser.add_argument("-g", "--gap-seconds", type=nonnegative_float, default=20)
    parser.add_argument(
        "-T",
        "--toolchain",
        default=os.environ.get("F2Z_RUST_TOOLCHAIN", "1.97.1"),
    )
    parser.add_argument("--big", action="store_true")
    parser.add_argument("--phases", action="store_true")
    parser.add_argument("--skip-build", action="store_true")
    return parser.parse_args(argv)


def main(argv: Sequence[str] | None = None) -> int:
    os.chdir(REPO)
    args = parse_args(argv)
    try:
        shapes = parse_shapes(args.shapes)
        if args.big:
            shapes += tuple(Shape.parse(value) for value in BIG_SHAPES)
        output = args.output or Path(
            f"bench_results/f2z-{datetime.now():%Y%m%d-%H%M%S}.csv"
        )
        capture_csv(
            output,
            SweepConfig(
                profiles=parse_profiles(args.profiles),
                shapes=shapes,
                reps=args.reps,
                threads=args.threads,
                gap_seconds=args.gap_seconds,
                toolchain=args.toolchain,
                phases=args.phases,
                skip_build=args.skip_build,
            ),
        )
    except (argparse.ArgumentTypeError, BenchError, OSError, ValueError) as error:
        print(f"bench_csv.py: {error}", file=sys.stderr)
        return 1
    except KeyboardInterrupt:
        print("bench_csv.py: interrupted", file=sys.stderr)
        return 130
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

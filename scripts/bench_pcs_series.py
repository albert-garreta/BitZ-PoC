#!/usr/bin/env python3
"""Capture or rebuild the F2Z PCS timing tables for one Ligerito profile.

This command owns only clean headline timing and the separate one-run phase
diagnostic. Structured intervals and the HTML dashboard are explicit, separate
commands so tracing can never affect the headline measurement.
"""

from __future__ import annotations

import argparse
import hashlib
import os
import re
import subprocess
import sys
import time
from datetime import datetime
from pathlib import Path
from typing import Iterable, Sequence

from bench_csv import (
    DEFAULT_RUSTFLAGS,
    SERIES_GAP_SECONDS,
    SERIES_REPS,
    SERIES_SHAPE_VALUES,
    SERIES_THREADS,
    BenchError,
    Shape,
    SweepConfig,
    build_benchmark,
    capture_csv,
    parse_profile,
)
from export_pcs_tables import InputError, export_reports


REPO = Path(__file__).resolve().parent.parent
SHAPES = tuple(
    Shape.parse(value) for value in SERIES_SHAPE_VALUES
)
REPS = SERIES_REPS
THREADS = SERIES_THREADS
GAP_SECONDS = SERIES_GAP_SECONDS
REPORT_NAMES = (
    "headline.csv",
    "headline.tex",
    "prover-breakdown.csv",
    "prover-breakdown.tex",
)
FLOCK_PATH_RE = re.compile(
    r'flock-core\s*=\s*\{[^}]*path\s*=\s*"([^\"]+)/crates/flock-core"'
)


class SeriesError(RuntimeError):
    """The requested timing bundle could not be captured or rebuilt."""


def parse_args(argv: Sequence[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--profile", required=True, type=parse_profile)
    parser.add_argument("--out-dir", type=Path)
    parser.add_argument(
        "--report-only",
        action="store_true",
        help="rebuild four tables from an existing bundle without Rust",
    )
    args = parser.parse_args(argv)
    if args.report_only and args.out_dir is None:
        parser.error("--out-dir is required with --report-only")
    return args


def default_output_dir(selected_profile: str) -> Path:
    slug = re.sub(r"[^A-Za-z0-9._-]+", "-", selected_profile).strip("-")
    return Path(f"bench_results/pcs-{slug}-{datetime.now():%Y%m%d-%H%M%S}")


def git_text(arguments: list[str], *, cwd: Path) -> str:
    result = subprocess.run(
        ["git", *arguments],
        cwd=cwd,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
        encoding="utf-8",
        errors="replace",
        check=False,
    )
    if result.returncode != 0:
        message = result.stderr.strip() or "git command failed"
        raise SeriesError(message)
    return result.stdout.strip()


def repository_is_dirty(path: Path) -> bool:
    return bool(
        git_text(
            ["status", "--porcelain", "--untracked-files=normal"], cwd=path
        )
    )


def flock_repository() -> Path:
    cargo_toml = (REPO / "Cargo.toml").read_text(encoding="utf-8")
    match = FLOCK_PATH_RE.search(cargo_toml)
    if match is None:
        raise SeriesError("could not resolve flock-core path from Cargo.toml")
    return Path(match.group(1))


def rustc_version(toolchain: str) -> str:
    result = subprocess.run(
        ["rustup", "run", toolchain, "rustc", "--version"],
        cwd=REPO,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
        encoding="utf-8",
        errors="replace",
        check=False,
    )
    if result.returncode != 0:
        raise SeriesError(result.stderr.strip() or "rustc version check failed")
    return result.stdout.strip()


def write_run_spec(
    output_dir: Path,
    *,
    selected_profile: str,
    toolchain: str,
    rustflags: str,
) -> None:
    flock = flock_repository()
    lines = [
        f"profile={selected_profile}",
        f"shapes={' '.join(shape.bench_value for shape in SHAPES)}",
        f"reps={REPS}",
        f"threads={THREADS}",
        f"toolchain={toolchain}",
        f"rustflags={rustflags}",
        f"f2z_git_commit={git_text(['rev-parse', 'HEAD'], cwd=REPO)}",
        f"f2z_git_dirty={str(repository_is_dirty(REPO)).lower()}",
        f"flock_git_commit={git_text(['rev-parse', 'HEAD'], cwd=flock)}",
        f"flock_git_dirty={str(repository_is_dirty(flock)).lower()}",
        f"rustc={rustc_version(toolchain)}",
    ]
    (output_dir / "run-spec.txt").write_text(
        "\n".join(lines) + "\n", encoding="utf-8"
    )


def files_under(path: Path) -> Iterable[Path]:
    if path.is_file():
        yield path
    elif path.is_dir():
        yield from (candidate for candidate in path.rglob("*") if candidate.is_file())
    else:
        raise SeriesError(f"manifest input does not exist: {path}")


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def write_manifest(
    output_dir: Path,
    headline_input: Path,
    breakdown_input: Path,
    report_dir: Path,
) -> None:
    run_spec = output_dir / "run-spec.txt"
    if not run_spec.is_file():
        return
    paths = [*files_under(headline_input), *files_under(breakdown_input)]
    paths.extend(report_dir / name for name in REPORT_NAMES)
    paths = sorted(set(paths), key=lambda path: os.fsencode(str(path)))
    lines = [
        f"generated_at={datetime.now().astimezone():%Y-%m-%dT%H:%M:%S%z}",
        *run_spec.read_text(encoding="utf-8").splitlines(),
        *(f"{sha256(path)}  {path}" for path in paths),
    ]
    (output_dir / "manifest.txt").write_text(
        "\n".join(lines) + "\n", encoding="utf-8"
    )


def table_inputs(raw_dir: Path) -> tuple[Path, Path]:
    headline = raw_dir / "headline.csv"
    breakdown = raw_dir / "breakdown.csv"
    if not headline.is_file():
        headline = raw_dir / "headline"
    if not breakdown.is_file():
        breakdown = raw_dir / "breakdown"
    return headline, breakdown


def export_tables(
    headline_input: Path,
    breakdown_input: Path,
    report_dir: Path,
    selected_profile: str,
) -> None:
    export_reports(
        [headline_input],
        [breakdown_input],
        report_dir,
        expected_profile=selected_profile,
        force=True,
    )


def capture_series(
    output_dir: Path,
    *,
    selected_profile: str,
    toolchain: str,
    rustflags: str,
) -> tuple[Path, Path, Path]:
    raw_dir = output_dir / "raw"
    report_dir = output_dir / "report"
    raw_dir.mkdir()
    report_dir.mkdir()
    headline = raw_dir / "headline.csv"
    breakdown = raw_dir / "breakdown.csv"

    environment = os.environ.copy()
    environment.pop("F2_FOREST_SCHEDULE", None)
    environment.pop("F2Z_BENCH_INTERVALS", None)
    environment.update(
        RUSTFLAGS=rustflags,
        F2Z_BENCH_FILL="1",
        F2Z_COL_ELIDE="1",
    )
    build_benchmark(toolchain, "unchecked", environment)
    time.sleep(GAP_SECONDS)

    common = dict(
        profiles=(selected_profile,),
        shapes=SHAPES,
        threads=THREADS,
        gap_seconds=GAP_SECONDS,
        toolchain=toolchain,
        skip_build=True,
    )
    capture_csv(
        headline,
        SweepConfig(reps=REPS, phases=False, **common),
        base_environment=environment,
    )
    capture_csv(
        breakdown,
        SweepConfig(reps=1, phases=True, **common),
        base_environment=environment,
    )
    export_tables(headline, breakdown, report_dir, selected_profile)
    write_run_spec(
        output_dir,
        selected_profile=selected_profile,
        toolchain=toolchain,
        rustflags=rustflags,
    )
    return headline, breakdown, report_dir


def run(args: argparse.Namespace) -> None:
    output_dir = args.out_dir or default_output_dir(args.profile)
    raw_dir = output_dir / "raw"
    report_dir = output_dir / "report"

    if args.report_only:
        if not raw_dir.is_dir():
            raise SeriesError(f"missing raw benchmark data: {raw_dir}")
        report_dir.mkdir(parents=True, exist_ok=True)
        headline, breakdown = table_inputs(raw_dir)
        export_tables(headline, breakdown, report_dir, args.profile)
    else:
        output_dir.parent.mkdir(parents=True, exist_ok=True)
        try:
            output_dir.mkdir()
        except FileExistsError as error:
            raise SeriesError(
                f"{output_dir} already exists; choose a fresh --out-dir"
            ) from error
        toolchain = os.environ.get("F2Z_RUST_TOOLCHAIN", "1.97.1")
        rustflags = os.environ.get("RUSTFLAGS") or DEFAULT_RUSTFLAGS
        headline, breakdown, report_dir = capture_series(
            output_dir,
            selected_profile=args.profile,
            toolchain=toolchain,
            rustflags=rustflags,
        )

    write_manifest(output_dir, headline, breakdown, report_dir)
    print(f"wrote PCS timing tables: {output_dir}", file=sys.stderr)


def main(argv: Sequence[str] | None = None) -> int:
    os.chdir(REPO)
    args = parse_args(argv)
    try:
        run(args)
    except (BenchError, InputError, OSError, SeriesError, ValueError) as error:
        print(f"bench_pcs_series.py: {error}", file=sys.stderr)
        return 1
    except KeyboardInterrupt:
        print("bench_pcs_series.py: interrupted", file=sys.stderr)
        return 130
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

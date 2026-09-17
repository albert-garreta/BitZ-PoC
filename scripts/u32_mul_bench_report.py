#!/usr/bin/env python3
"""Validate canonical u32_mul benchmark logs and emit raw/summary CSVs."""

from __future__ import annotations

import argparse
import csv
import math
from collections import defaultdict
from itertools import product
from pathlib import Path
from typing import Iterable
from ligerito_results import validate_result_fields


STATIC_FIELDS = (
    "benchmark_algorithm",
    "benchmark_date",
    "run_timestamp_utc",
    "commit",
    "git_dirty",
    "binary_sha256",
    "memory_binary_sha256",
    "hardware",
    "operating_system",
    "cargo_profile",
    "cargo_features",
    "threads",
    "measured_runs",
    "warmup_runs",
    "field_modulus",
    "ligerito_hash",
    "allocator_instrumentation",
    "root_seed",
)

IDENTITY_FIELDS = (
    "protocol",
    "skip_vars",
    "strategy",
    "word_bits",
    "projection_bits",
    "bitz_t",
    "bitz_s",
    "bitz_chunks",
    "exponent",
    "multiplications",
)

RAW_FIELDS = (
    "pass",
    "order",
    *IDENTITY_FIELDS,
    "sample",
    "commit_ms",
    "prove_ms",
    "verify_ms",
    "verified",
)

SUMMARY_REQUIRED_FIELDS = (
    "ligerito_hex",
    "pass",
    "order",
    *IDENTITY_FIELDS,
    "lambda",
    "lambda_achieved",
    "lambda_bind",
    "reps",
    "warmups",
    "seed",
    "witness_ms",
    "setup_ms",
    "prove_ms",
    "s1_commit_ms",
    "s2_project_ms",
    "s3_piop_ms",
    "s4_bitify_ms",
    "s5_0_reduce_ms",
    "s5_open_ms",
    "prove_residual_ms",
    "s3_outer_ms",
    "s3_bind_ms",
    "s3_inner_ms",
    "s5_forest_ms",
    "s5_opener_ms",
    "verify_ms",
    "v2_project_ms",
    "v3_piop_ms",
    "v4_bitify_ms",
    "v5_0_reduce_ms",
    "v5_open_ms",
    "verify_residual_ms",
    "proof_bytes",
    "proof_piop_bytes",
    "proof_open_bytes",
    "shape_seed",
    "verified_samples",
)

SUMMARY_FIELDS = (
    *SUMMARY_REQUIRED_FIELDS,
    "peak_heap_mib",
    "live_before_prove_mib",
    "memory_order",
    "all_samples_passed",
)

Pair = tuple[int, int]


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("log", type=Path)
    parser.add_argument("--raw-csv", required=True, type=Path)
    parser.add_argument("--summary-csv", required=True, type=Path)
    parser.add_argument("--benchmark-date", required=True)
    parser.add_argument("--run-timestamp", required=True)
    parser.add_argument("--commit", required=True)
    parser.add_argument("--git-dirty", choices=("true", "false"), required=True)
    parser.add_argument("--binary-sha256", required=True)
    parser.add_argument("--memory-binary-sha256", default="")
    parser.add_argument("--hardware", required=True)
    parser.add_argument("--operating-system", required=True)
    parser.add_argument("--cargo-features", required=True)
    parser.add_argument("--root-seed", required=True)
    parser.add_argument("--threads", required=True)
    parser.add_argument("--measured-runs", required=True, type=int)
    parser.add_argument("--expected-word-bits", required=True)
    parser.add_argument("--expected-exponents", required=True)
    parser.add_argument("--measure-memory", choices=("0", "1"), required=True)
    return parser.parse_args()


def words(value: str) -> list[str]:
    return value.replace(",", " ").split()


def expected_pairs(args: argparse.Namespace) -> set[Pair]:
    try:
        widths = tuple(int(value) for value in words(args.expected_word_bits))
        exponents = tuple(int(value) for value in words(args.expected_exponents))
    except ValueError as error:
        raise SystemExit(f"invalid benchmark shape: {error}") from error
    if not widths or len(widths) != len(set(widths)) or set(widths) - {1, 8}:
        raise SystemExit("expected word bits must be a unique, nonempty subset of {1, 8}")
    if not exponents or len(exponents) != len(set(exponents)) or min(exponents) < 15:
        raise SystemExit("expected exponents must be unique integers >= 15")
    if args.measured_runs <= 0:
        raise SystemExit("--measured-runs must be positive")
    if args.measure_memory == "1" and not args.memory_binary_sha256:
        raise SystemExit("memory measurement requires --memory-binary-sha256")
    return set(product(widths, exponents))


def key_values(line: str, marker: str) -> dict[str, str] | None:
    try:
        payload = line[line.index(marker) + len(marker) :]
    except ValueError:
        return None
    fields: dict[str, str] = {}
    for item in payload.split():
        if "=" not in item:
            continue
        key, value = item.split("=", 1)
        if key in fields:
            raise SystemExit(f"duplicate field {key!r}: {line}")
        fields[key] = value
    return fields


def pair(record: dict[str, str], kind: str) -> Pair:
    try:
        return int(record["word_bits"]), int(record["exponent"])
    except (KeyError, ValueError) as error:
        raise SystemExit(f"invalid {kind} identity: {record}") from error


def require(record: dict[str, str], fields: Iterable[str], kind: str) -> None:
    missing = [field for field in fields if field not in record]
    if missing:
        raise SystemExit(f"{kind} record is missing {', '.join(missing)}: {record}")


def canonical_shape(record: dict[str, str], kind: str) -> None:
    require(record, IDENTITY_FIELDS, kind)
    width, exponent = pair(record, kind)
    if width not in (1, 8) or exponent < 15:
        raise SystemExit(f"unsupported {kind} shape: W={width}, exponent={exponent}")
    log_width = int(math.log2(width))
    s = exponent // 2
    t = exponent - s + 7 - log_width
    chunk_width = 127 - t - width
    projection_bits = min(113, 126, 128 - t, chunk_width)
    expected = {
        "protocol": "skip-k3",
        "skip_vars": "3",
        "strategy": "delayed-barrett",
        "bitz_t": str(t),
        "bitz_s": str(s),
        "projection_bits": str(projection_bits),
        "bitz_chunks": str((projection_bits + chunk_width - 1) // chunk_width),
        "multiplications": str(1 << exponent),
    }
    for field, value in expected.items():
        if record[field] != value:
            raise SystemExit(
                f"invalid {kind} {field} for W={width}, exponent={exponent}: "
                f"reported={record[field]}, expected={value}"
            )


def number(record: dict[str, str], field: str) -> float:
    try:
        value = float(record[field])
    except (KeyError, ValueError) as error:
        raise SystemExit(f"invalid {field!r}: {record}") from error
    if not math.isfinite(value):
        raise SystemExit(f"non-finite {field!r}: {record}")
    return value


def median(records: list[dict[str, str]], field: str) -> float:
    values = sorted(number(record, field) for record in records)
    return values[len(values) // 2]


def validate_median(
    result: dict[str, str], result_field: str, samples: list[dict[str, str]], sample_field: str
) -> None:
    reported = number(result, result_field)
    computed = median(samples, sample_field)
    if not math.isclose(reported, computed, rel_tol=0.0, abs_tol=5.1e-4):
        raise SystemExit(
            f"median mismatch for {result_field}: reported={reported}, computed={computed}"
        )


def metadata(args: argparse.Namespace) -> dict[str, str]:
    return {
        "benchmark_algorithm": "u32 x u32 -> u64 runtime-prime Spartan PIOP + BitZ",
        "benchmark_date": args.benchmark_date,
        "run_timestamp_utc": args.run_timestamp,
        "commit": args.commit,
        "git_dirty": args.git_dirty,
        "binary_sha256": args.binary_sha256,
        "memory_binary_sha256": args.memory_binary_sha256,
        "hardware": args.hardware,
        "operating_system": args.operating_system,
        "cargo_profile": "bench",
        "cargo_features": args.cargo_features,
        "threads": args.threads,
        "measured_runs": str(args.measured_runs),
        "warmup_runs": "1",
        "field_modulus": "transcript-sampled-prime",
        "ligerito_hash": "BLAKE3",
        "allocator_instrumentation": (
            "absent_in_latency_binary;shared_atomic_peak_allocator_in_memory_binary"
        ),
        "root_seed": args.root_seed,
    }


def write_csv(
    path: Path,
    fields: tuple[str, ...],
    rows: Iterable[dict[str, str]],
    static: dict[str, str],
) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("x", newline="", encoding="utf-8") as output:
        writer = csv.DictWriter(output, fieldnames=(*STATIC_FIELDS, *fields))
        writer.writeheader()
        for row in rows:
            writer.writerow(
                {
                    field: row.get(field, static.get(field, ""))
                    for field in (*STATIC_FIELDS, *fields)
                }
            )


def compare(observed: set[Pair], expected: set[Pair], kind: str) -> None:
    if observed != expected:
        raise SystemExit(
            f"{kind} shapes do not match sweep: "
            f"missing={sorted(expected - observed)}, unexpected={sorted(observed - expected)}"
        )


def main() -> None:
    args = parse_args()
    expected = expected_pairs(args)
    static = metadata(args)
    samples: defaultdict[Pair, list[dict[str, str]]] = defaultdict(list)
    results: defaultdict[Pair, list[dict[str, str]]] = defaultdict(list)
    memories: defaultdict[Pair, list[dict[str, str]]] = defaultdict(list)

    for line in args.log.read_text(encoding="utf-8").splitlines():
        for marker, destination, kind in (
            ("SAMPLE ", samples, "SAMPLE"),
            ("RESULT ", results, "RESULT"),
            ("MEMORY ", memories, "MEMORY"),
        ):
            record = key_values(line, marker)
            if record is None:
                continue
            if kind == "RESULT" and record.get("bench") != "u32_mul":
                break
            canonical_shape(record, kind)
            destination[pair(record, kind)].append(record)
            break

    compare(set(samples), expected, "SAMPLE")
    compare(set(results), expected, "RESULT")
    compare(set(memories), expected if args.measure_memory == "1" else set(), "MEMORY")

    raw_rows: list[dict[str, str]] = []
    summary_rows: list[dict[str, str]] = []
    for shape in sorted(expected):
        shape_samples = samples[shape]
        shape_results = results[shape]
        if len(shape_samples) != args.measured_runs or len(shape_results) != 1:
            raise SystemExit(
                f"shape {shape} needs {args.measured_runs} samples and one result; "
                f"found {len(shape_samples)} and {len(shape_results)}"
            )
        result = shape_results[0]
        require(result, SUMMARY_REQUIRED_FIELDS, "RESULT")
        validate_result_fields(result)
        for sample in shape_samples:
            require(sample, RAW_FIELDS, "SAMPLE")
        if result["pass"] != "latency" or any(
            sample["pass"] != "latency" or sample["verified"] != "true"
            for sample in shape_samples
        ):
            raise SystemExit(f"unverified or non-latency record for shape {shape}")
        indices = sorted(int(sample["sample"]) for sample in shape_samples)
        if indices != list(range(1, args.measured_runs + 1)):
            raise SystemExit(f"sample indices for {shape} are {indices}")
        validate_median(result, "prove_ms", shape_samples, "prove_ms")
        validate_median(result, "verify_ms", shape_samples, "verify_ms")
        validate_median(result, "s1_commit_ms", shape_samples, "commit_ms")
        if int(result["verified_samples"]) != args.measured_runs:
            raise SystemExit(f"verified sample count mismatch for {shape}")

        memory_records = memories.get(shape, [])
        if len(memory_records) > 1:
            raise SystemExit(f"multiple MEMORY records for {shape}")
        memory = memory_records[0] if memory_records else None
        if memory is not None:
            require(
                memory,
                (*IDENTITY_FIELDS, "pass", "order", "peak_heap_mib", "live_before_prove_mib", "verified"),
                "MEMORY",
            )
            if memory["pass"] != "memory" or memory["verified"] != "true":
                raise SystemExit(f"invalid MEMORY record for {shape}")

        result["peak_heap_mib"] = memory["peak_heap_mib"] if memory else ""
        result["live_before_prove_mib"] = memory["live_before_prove_mib"] if memory else ""
        result["memory_order"] = memory["order"] if memory else ""
        result["all_samples_passed"] = "true"
        raw_rows.extend(sorted(shape_samples, key=lambda sample: int(sample["sample"])))
        summary_rows.append(result)

    write_csv(args.raw_csv, RAW_FIELDS, raw_rows, static)
    write_csv(args.summary_csv, SUMMARY_FIELDS, summary_rows, static)


if __name__ == "__main__":
    main()

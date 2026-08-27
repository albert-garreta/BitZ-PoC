#!/usr/bin/env python3
"""Convert stable u32_mul benchmark records into comparison CSV files."""

from __future__ import annotations

import argparse
import csv
import math
from collections import defaultdict
from itertools import product
from pathlib import Path
from typing import Iterable


HISTORICAL_SPARTAN_MS = {
    15: 12.8150,
    16: 24.7753,
    17: 49.2672,
    18: 91.3942,
    19: 179.2600,
    20: 403.9578,
    21: 802.5125,
    22: 1719.1535,
    23: 3547.5121,
    24: 9870.2299,
    25: 36562.7829,
}


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
    "ligerito_inverse_rate",
    "ligerito_initial_k",
    "allocator_instrumentation",
    "root_seed",
)

SAMPLE_FIELDS = (
    "pass",
    "order",
    "strategy",
    "reduction_backend",
    "exponent",
    "multiplications",
    "sample",
    "prove_ms",
    "spartan_prove_ms",
    "spartan_outer_ms",
    "spartan_bind_ms",
    "spartan_inner_ms",
    "bitify_prove_ms",
    "f2z_prove_ms",
    "verify_ms",
    "spartan_verify_ms",
    "bitify_verify_ms",
    "f2z_verify_ms",
    "verified",
    "r1cs_rows",
    "multiplications_per_row",
    "r1cs_columns",
    "r1cs_nnz",
    "integer_witness_values",
    "compact_witness_bits",
    "compact_witness_bytes",
    "spartan_proof_payload_bytes",
    "f2z_proof_bytes",
    "shape_seed",
    "peak_heap_mib",
    "live_before_prove_mib",
    "memory_order",
)

SUMMARY_FIELDS = (
    "pass",
    "order",
    "strategy",
    "reduction_backend",
    "exponent",
    "multiplications",
    "r1cs_rows",
    "multiplications_per_row",
    "r1cs_columns",
    "r1cs_nnz",
    "integer_witness_values",
    "compact_witness_bits",
    "compact_witness_bytes",
    "prove_ms",
    "spartan_prove_ms",
    "historical_spartan_prove_ms",
    "historical_15pct_target_ms",
    "spartan_outer_ms",
    "spartan_bind_ms",
    "spartan_inner_ms",
    "bitify_prove_ms",
    "f2z_prove_ms",
    "verify_ms",
    "spartan_verify_ms",
    "bitify_verify_ms",
    "f2z_verify_ms",
    "spartan_proof_payload_bytes",
    "f2z_proof_bytes",
    "peak_heap_mib",
    "live_before_prove_mib",
    "memory_order",
    "shape_seed",
    "verified_samples",
    "all_samples_passed",
)

PAIRED_FIELDS = (
    "scope",
    "exponent",
    "multiplications",
    "baseline_strategy",
    "baseline_order",
    "optimized_strategy",
    "optimized_order",
    "reduction_backend",
    "historical_spartan_ms",
    "historical_15pct_target_ms",
    "baseline_spartan_ms",
    "optimized_spartan_ms",
    "delta_ms",
    "latency_reduction_pct",
    "speedup",
    "individual_pass",
    "primary_gate",
    "sizes_passing",
    "sizes_total",
    "geomean_latency_reduction_pct",
    "overall_pass",
)

TIMING_FIELDS = (
    "prove_ms",
    "spartan_prove_ms",
    "spartan_outer_ms",
    "spartan_bind_ms",
    "spartan_inner_ms",
    "bitify_prove_ms",
    "f2z_prove_ms",
    "verify_ms",
    "spartan_verify_ms",
    "bitify_verify_ms",
    "f2z_verify_ms",
)

RESULT_DETAIL_FIELDS = (
    "r1cs_rows",
    "r1cs_columns",
    "r1cs_nnz",
    "integer_witness_values",
    "compact_witness_bits",
    "compact_witness_bytes",
    "spartan_proof_payload_bytes",
    "f2z_proof_bytes",
    "shape_seed",
)


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("log", type=Path)
    parser.add_argument("--raw-csv", required=True, type=Path)
    parser.add_argument("--summary-csv", required=True, type=Path)
    parser.add_argument("--paired-csv", required=True, type=Path)
    parser.add_argument("--benchmark-date", required=True)
    parser.add_argument("--run-timestamp", required=True)
    parser.add_argument("--commit", required=True)
    parser.add_argument("--git-dirty", choices=("true", "false"), required=True)
    parser.add_argument("--binary-sha256", required=True)
    parser.add_argument("--memory-binary-sha256", required=True)
    parser.add_argument("--hardware", required=True)
    parser.add_argument("--operating-system", required=True)
    parser.add_argument("--cargo-features", required=True)
    parser.add_argument("--root-seed", required=True)
    parser.add_argument("--threads", required=True)
    parser.add_argument("--measured-runs", required=True, type=int)
    parser.add_argument("--expected-exponents", required=True)
    parser.add_argument("--expected-strategies", "--strategies", dest="expected_strategies", required=True)
    parser.add_argument(
        "--expected-memory-strategies",
        "--memory-strategies",
        dest="expected_memory_strategies",
        required=True,
    )
    parser.add_argument("--measure-memory", choices=("0", "1"), required=True)
    return parser.parse_args()


def words(value: str) -> list[str]:
    return value.replace(",", " ").split()


def unique_cli_values(values: list[str], name: str) -> tuple[str, ...]:
    if not values:
        raise SystemExit(f"{name} must contain at least one value")
    if len(values) != len(set(values)):
        raise SystemExit(f"{name} contains duplicate values: {values}")
    return tuple(values)


def expected_inputs(args: argparse.Namespace) -> tuple[tuple[int, ...], tuple[str, ...], tuple[str, ...]]:
    try:
        exponent_values = [int(value) for value in words(args.expected_exponents)]
    except ValueError as error:
        raise SystemExit(f"invalid --expected-exponents: {error}") from error
    exponent_strings = unique_cli_values([str(value) for value in exponent_values], "expected exponents")
    exponents = tuple(int(value) for value in exponent_strings)
    if any(value < 0 for value in exponents):
        raise SystemExit("expected exponents must be non-negative")
    strategies = unique_cli_values(words(args.expected_strategies), "expected strategies")
    memory_values = words(args.expected_memory_strategies)
    if len(memory_values) != len(set(memory_values)):
        raise SystemExit(f"expected memory strategies contains duplicate values: {memory_values}")
    memory_strategies = tuple(memory_values)
    unknown_memory = set(memory_strategies) - set(strategies)
    if unknown_memory:
        raise SystemExit(
            "memory strategies must be a subset of latency strategies: "
            + ", ".join(sorted(unknown_memory))
        )
    if args.measure_memory == "1" and not memory_strategies:
        raise SystemExit("memory measurement requires at least one memory strategy")
    if args.measured_runs <= 0:
        raise SystemExit("--measured-runs must be positive")
    if args.measure_memory == "1" and not args.memory_binary_sha256:
        raise SystemExit("memory measurement requires --memory-binary-sha256")
    return exponents, strategies, memory_strategies


def key_values(line: str, marker: str) -> dict[str, str] | None:
    try:
        record = line[line.index(marker) + len(marker) :]
    except ValueError:
        return None
    fields: dict[str, str] = {}
    for item in record.split():
        if "=" in item:
            key, value = item.split("=", 1)
            if key in fields:
                raise SystemExit(f"duplicate field {key!r} in log record: {line}")
            fields[key] = value
    return fields


def static_metadata(args: argparse.Namespace) -> dict[str, str]:
    return {
        "benchmark_algorithm": "u32 x u32 -> u64 Spartan PIOP + F2Z",
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
        "field_modulus": "2^100-15",
        "ligerito_hash": "SHA-256",
        "ligerito_inverse_rate": "1/2",
        "ligerito_initial_k": "4",
        "allocator_instrumentation": "absent_in_latency_binary;shared_atomic_peak_allocator_in_memory_binary",
        "root_seed": args.root_seed,
    }


def output_row(metadata: dict[str, str], record: dict[str, str], fields: Iterable[str]) -> dict[str, str]:
    return {field: record.get(field, metadata.get(field, "")) for field in fields}


def exponent(record: dict[str, str]) -> int:
    return int(record["exponent"])


def strategy_rank(name: str) -> int:
    return {
        "immediate": 0,
        "delayed-barrett": 1,
        "delayed-crypto-bigint": 2,
    }.get(name, 99)


def f64(record: dict[str, str], field: str) -> float:
    try:
        value = float(record[field])
    except (KeyError, ValueError) as error:
        raise SystemExit(f"invalid or missing {field!r} in record: {record}") from error
    if not math.isfinite(value):
        raise SystemExit(f"non-finite {field!r} in record: {record}")
    return value


def formatted(value: float) -> str:
    return f"{value:.6f}"


def formatted_timing(value: float) -> str:
    return f"{value:.9f}"


def benchmark_median(values: Iterable[float]) -> float:
    """Match the benchmark executable's deterministic upper-middle median."""
    ordered = sorted(values)
    if not ordered:
        raise SystemExit("cannot compute a median from no samples")
    return ordered[len(ordered) // 2]


def historical(exp: int) -> tuple[str, str]:
    value = HISTORICAL_SPARTAN_MS.get(exp)
    if value is None:
        return "", ""
    return formatted(value), formatted(0.85 * value)


def paired_rows(summaries: list[dict[str, str]]) -> list[dict[str, str]]:
    by_exponent: dict[int, dict[str, dict[str, str]]] = {}
    for row in summaries:
        by_exponent.setdefault(exponent(row), {})[row["strategy"]] = row

    rows: list[dict[str, str]] = []
    ratios: dict[str, list[float]] = {}
    passing: dict[str, int] = {}
    totals: dict[str, int] = {}
    remaining_improves: dict[str, bool] = {}
    for exp in sorted(by_exponent):
        strategies = by_exponent[exp]
        baseline = strategies.get("immediate")
        if baseline is None:
            continue
        baseline_ms = f64(baseline, "spartan_prove_ms")
        for name in ("delayed-barrett", "delayed-crypto-bigint"):
            optimized = strategies.get(name)
            if optimized is None:
                continue
            optimized_ms = f64(optimized, "spartan_prove_ms")
            ratio = optimized_ms / baseline_ms
            reduction = 100.0 * (1.0 - ratio)
            primary = 15 <= exp <= 24
            passed = reduction >= 15.0
            if primary:
                ratios.setdefault(name, []).append(ratio)
                passing[name] = passing.get(name, 0) + int(passed)
                totals[name] = totals.get(name, 0) + 1
                remaining_improves[name] = remaining_improves.get(name, True) and reduction > 0.0
            rows.append(
                {
                    "scope": "size",
                    "exponent": str(exp),
                    "multiplications": baseline["multiplications"],
                    "baseline_strategy": "immediate",
                    "baseline_order": baseline["order"],
                    "optimized_strategy": name,
                    "optimized_order": optimized["order"],
                    "reduction_backend": optimized["reduction_backend"],
                    "historical_spartan_ms": historical(exp)[0],
                    "historical_15pct_target_ms": historical(exp)[1],
                    "baseline_spartan_ms": formatted(baseline_ms),
                    "optimized_spartan_ms": formatted(optimized_ms),
                    "delta_ms": formatted(optimized_ms - baseline_ms),
                    "latency_reduction_pct": formatted(reduction),
                    "speedup": formatted(baseline_ms / optimized_ms),
                    "individual_pass": str(passed).lower(),
                    "primary_gate": str(primary).lower(),
                }
            )

    for name, samples in ratios.items():
        geometric_ratio = math.exp(sum(math.log(value) for value in samples) / len(samples))
        geometric_reduction = 100.0 * (1.0 - geometric_ratio)
        count = passing[name]
        total = totals[name]
        overall = total == 10 and count >= 9 and geometric_reduction >= 15.0 and remaining_improves[name]
        rows.append(
            {
                "scope": "overall",
                "baseline_strategy": "immediate",
                "optimized_strategy": name,
                "reduction_backend": next(
                    row["reduction_backend"] for row in summaries if row["strategy"] == name
                ),
                "sizes_passing": str(count),
                "sizes_total": str(total),
                "geomean_latency_reduction_pct": formatted(geometric_reduction),
                "overall_pass": str(overall).lower(),
            }
        )
    return rows


def write_csv(path: Path, fieldnames: tuple[str, ...], rows: Iterable[dict[str, str]], metadata: dict[str, str]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("x", newline="", encoding="utf-8") as output:
        writer = csv.DictWriter(output, fieldnames=(*STATIC_FIELDS, *fieldnames))
        writer.writeheader()
        for row in rows:
            writer.writerow(output_row(metadata, row, (*STATIC_FIELDS, *fieldnames)))


def record_pair(record: dict[str, str], kind: str) -> tuple[int, str]:
    try:
        return exponent(record), record["strategy"]
    except (KeyError, ValueError) as error:
        raise SystemExit(f"invalid {kind} identity: {record}") from error


def require_fields(record: dict[str, str], fields: Iterable[str], kind: str) -> None:
    missing = [field for field in fields if field not in record]
    if missing:
        raise SystemExit(f"{kind} record is missing {', '.join(missing)}: {record}")


def compare_pair_sets(
    observed: set[tuple[int, str]], expected: set[tuple[int, str]], kind: str
) -> None:
    missing = expected - observed
    unexpected = observed - expected
    if missing or unexpected:
        details: list[str] = []
        if missing:
            details.append(f"missing={sorted(missing)}")
        if unexpected:
            details.append(f"unexpected={sorted(unexpected)}")
        raise SystemExit(f"{kind} pairs do not match the requested sweep: {'; '.join(details)}")


def validate_result_median(
    result: dict[str, str], field: str, computed: float, pair: tuple[int, str]
) -> None:
    reported = f64(result, field)
    tolerance = max(1.0e-6, abs(computed) * 1.0e-9)
    if not math.isclose(reported, computed, rel_tol=0.0, abs_tol=tolerance):
        raise SystemExit(
            f"RESULT median mismatch for {pair} {field}: "
            f"reported={reported:.12g}, recomputed={computed:.12g}"
        )
    # Keep the higher-precision medians for the gate; presentation-only paired
    # columns are rounded separately by `formatted`.
    result[field] = formatted_timing(computed)


def main() -> None:
    args = parse_args()
    exponents, strategies, memory_strategies = expected_inputs(args)
    metadata = static_metadata(args)
    samples_by_pair: defaultdict[tuple[int, str], list[dict[str, str]]] = defaultdict(list)
    results_by_pair: defaultdict[tuple[int, str], list[dict[str, str]]] = defaultdict(list)
    memory_by_pair: defaultdict[tuple[int, str], list[dict[str, str]]] = defaultdict(list)
    for line in args.log.read_text(encoding="utf-8").splitlines():
        sample = key_values(line, "SAMPLE ")
        if sample is not None:
            samples_by_pair[record_pair(sample, "SAMPLE")].append(sample)
            continue
        result = key_values(line, "RESULT ")
        if result is not None:
            results_by_pair[record_pair(result, "RESULT")].append(result)
            continue
        peak = key_values(line, "MEMORY ")
        if peak is not None:
            memory_by_pair[record_pair(peak, "MEMORY")].append(peak)

    expected_latency_pairs = set(product(exponents, strategies))
    compare_pair_sets(set(samples_by_pair), expected_latency_pairs, "SAMPLE")
    compare_pair_sets(set(results_by_pair), expected_latency_pairs, "RESULT")

    expected_memory_pairs = (
        set(product(exponents, memory_strategies)) if args.measure_memory == "1" else set()
    )
    compare_pair_sets(set(memory_by_pair), expected_memory_pairs, "MEMORY")

    summaries: list[dict[str, str]] = []
    samples: list[dict[str, str]] = []
    for pair in sorted(expected_latency_pairs, key=lambda value: (value[0], strategy_rank(value[1]))):
        result_records = results_by_pair[pair]
        if len(result_records) != 1:
            raise SystemExit(f"expected exactly one RESULT record for {pair}, found {len(result_records)}")
        result = result_records[0]
        require_fields(
            result,
            (
                "pass",
                "order",
                "strategy",
                "reduction_backend",
                "exponent",
                "multiplications",
                "verified_samples",
                *TIMING_FIELDS,
                *RESULT_DETAIL_FIELDS,
            ),
            "RESULT",
        )
        if result["pass"] != "latency":
            raise SystemExit(f"RESULT record has non-latency pass for {pair}: {result['pass']}")
        try:
            verified_samples = int(result["verified_samples"])
        except ValueError as error:
            raise SystemExit(
                f"invalid RESULT verified_samples for {pair}: {result['verified_samples']}"
            ) from error
        if verified_samples != args.measured_runs:
            raise SystemExit(
                f"RESULT verified_samples for {pair} is {result['verified_samples']}, "
                f"expected {args.measured_runs}"
            )

        pair_samples = samples_by_pair[pair]
        if len(pair_samples) != args.measured_runs:
            raise SystemExit(
                f"expected {args.measured_runs} SAMPLE records for {pair}, found {len(pair_samples)}"
            )
        sample_indices: list[int] = []
        for sample in pair_samples:
            require_fields(
                sample,
                (
                    "pass",
                    "order",
                    "strategy",
                    "reduction_backend",
                    "exponent",
                    "multiplications",
                    "sample",
                    "verified",
                    *TIMING_FIELDS,
                ),
                "SAMPLE",
            )
            if sample["pass"] != "latency":
                raise SystemExit(f"SAMPLE record has non-latency pass for {pair}: {sample['pass']}")
            if sample["verified"].lower() != "true":
                raise SystemExit(f"unverified SAMPLE record for {pair}: {sample}")
            try:
                sample_indices.append(int(sample["sample"]))
            except ValueError as error:
                raise SystemExit(f"invalid SAMPLE index for {pair}: {sample['sample']}") from error
            for identity in ("order", "reduction_backend", "multiplications"):
                if sample[identity] != result[identity]:
                    raise SystemExit(
                        f"SAMPLE/RESULT {identity} mismatch for {pair}: "
                        f"{sample[identity]} != {result[identity]}"
                    )
        if sorted(sample_indices) != list(range(args.measured_runs)):
            raise SystemExit(
                f"SAMPLE indices for {pair} must be unique 0..{args.measured_runs - 1}: "
                f"{sorted(sample_indices)}"
            )

        for field in TIMING_FIELDS:
            computed = benchmark_median(f64(sample, field) for sample in pair_samples)
            validate_result_median(result, field, computed, pair)

        memory_records = memory_by_pair.get(pair, [])
        if len(memory_records) > 1:
            raise SystemExit(f"expected at most one MEMORY record for {pair}, found {len(memory_records)}")
        memory = memory_records[0] if memory_records else None
        if memory is not None:
            require_fields(
                memory,
                (
                    "pass",
                    "order",
                    "strategy",
                    "reduction_backend",
                    "exponent",
                    "multiplications",
                    "peak_heap_mib",
                    "live_before_prove_mib",
                    "verified",
                ),
                "MEMORY",
            )
            if memory["pass"] != "memory":
                raise SystemExit(f"MEMORY record has non-memory pass for {pair}: {memory['pass']}")
            if memory["verified"].lower() != "true":
                raise SystemExit(f"unverified MEMORY record for {pair}: {memory}")
            for identity in ("reduction_backend", "multiplications"):
                if memory[identity] != result[identity]:
                    raise SystemExit(
                        f"MEMORY/RESULT {identity} mismatch for {pair}: "
                        f"{memory[identity]} != {result[identity]}"
                    )
            f64(memory, "peak_heap_mib")
            f64(memory, "live_before_prove_mib")

        result["multiplications_per_row"] = "1"
        result["peak_heap_mib"] = memory["peak_heap_mib"] if memory else ""
        result["live_before_prove_mib"] = memory["live_before_prove_mib"] if memory else ""
        result["memory_order"] = memory["order"] if memory else ""
        historical_ms, historical_target = historical(pair[0])
        result["historical_spartan_prove_ms"] = historical_ms
        result["historical_15pct_target_ms"] = historical_target
        result["all_samples_passed"] = "true"

        for sample in pair_samples:
            sample["multiplications_per_row"] = "1"
            for field in RESULT_DETAIL_FIELDS:
                sample[field] = result[field]
            sample["peak_heap_mib"] = result["peak_heap_mib"]
            sample["live_before_prove_mib"] = result["live_before_prove_mib"]
            sample["memory_order"] = result["memory_order"]
            samples.append(sample)
        summaries.append(result)

    ordered_samples = sorted(
        samples,
        key=lambda row: (exponent(row), int(row["order"]), int(row["sample"])),
    )
    ordered_summaries = sorted(
        summaries,
        key=lambda row: (exponent(row), strategy_rank(row["strategy"])),
    )

    write_csv(args.raw_csv, SAMPLE_FIELDS, ordered_samples, metadata)
    write_csv(args.summary_csv, SUMMARY_FIELDS, ordered_summaries, metadata)
    write_csv(args.paired_csv, PAIRED_FIELDS, paired_rows(ordered_summaries), metadata)


if __name__ == "__main__":
    main()

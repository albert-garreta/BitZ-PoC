#!/usr/bin/env python3
#
# COMPATIBILITY NOTE (2026-08-30): this script parses the PRE-SCHEMA output
# of benches/baby_bear_mul.rs (its original SAMPLE/RESULT/MEMORY records and
# the bench-peak-memory pass). The bench now emits the unified
# `RESULT schema=bitz/1` format (docs/bench-schema.md) with two rows per
# shape (Lambda100 + Lambda128) and no memory pass. To reproduce the output
# this script expects, run the bench from commit b7713d8. Porting this
# audit harness to the unified schema is an open task.
#
"""Convert stable baby_bear_mul benchmark records into comparison CSV files."""

from __future__ import annotations

import argparse
import csv
import math
from collections import defaultdict
from itertools import product
from pathlib import Path
from typing import Iterable


BABY_BEAR_MODULUS = 2_013_265_921
ALLOWED_STRATEGIES = (
    "immediate",
    "delayed-barrett",
    "delayed-crypto-bigint",
)
EXPECTED_BACKENDS = {
    "immediate": "immediate",
    "delayed-barrett": "barrett",
    "delayed-crypto-bigint": "crypto-bigint",
}
EXPECTED_PROJECTION_BACKENDS = {
    "immediate": "field",
    "delayed-barrett": "native-u64",
    "delayed-crypto-bigint": "native-u64",
}
OPERAND_SAMPLING = "rejection-31-bit"
EXPONENT_25_MINIMUM_MEMORY_GIB = 128


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
    "cargo_features_requested",
    "latency_cargo_features_resolved",
    "memory_cargo_features_resolved",
    "physical_memory_bytes",
    "physical_memory_gib",
    "exponent_25_minimum_memory_gib",
    "exponent_25_memory_requirement_met",
    "threads",
    "measured_runs",
    "warmup_runs",
    "field_modulus",
    "allocator_instrumentation",
    "root_seed",
)

SAMPLE_FIELDS = (
    "pass",
    "order",
    "strategy",
    "reduction_backend",
    "projection_backend",
    "exponent",
    "multiplications",
    "sample",
    "prove_ms",
    "spartan_prove_ms",
    "spartan_outer_ms",
    "spartan_bind_ms",
    "spartan_inner_ms",
    "inner_native_coefficients_ms",
    "inner_native_witness_fold_ms",
    "inner_field_rounds_ms",
    "bitify_prove_ms",
    "bitz_prove_ms",
    "bitz_prepare_prove_ms",
    "prove_residual_ms",
    "verify_ms",
    "spartan_verify_ms",
    "bitify_verify_ms",
    "bitz_verify_ms",
    "bitz_prepare_verify_ms",
    "verify_residual_ms",
    "verified",
    "r1cs_rows",
    "multiplications_per_row",
    "baby_bear_modulus",
    "quotient_bits",
    "canonicality",
    "operand_sampling",
    "r1cs_logical_columns",
    "r1cs_padded_columns",
    "r1cs_nnz_a",
    "r1cs_nnz_b",
    "r1cs_nnz_c",
    "r1cs_nnz",
    "logical_witness_values",
    "padded_assignment_values",
    "semantic_witness_bits",
    "committed_witness_bits",
    "committed_witness_bytes",
    "ligerito_hash",
    "ligerito_log_inv_rate",
    "ligerito_inverse_rate",
    "ligerito_initial_k",
    "witness_setup_ms",
    "relation_setup_ms",
    "projection_setup_ms",
    "bit_pack_setup_ms",
    "commitment_setup_ms",
    "spartan_proof_payload_bytes",
    "bitz_proof_bytes",
    "shape_seed",
    "peak_heap_mib",
    "live_before_prove_mib",
    "peak_heap_delta_mib",
    "memory_order",
    "memory_verified",
)

SUMMARY_FIELDS = (
    "pass",
    "order",
    "strategy",
    "reduction_backend",
    "projection_backend",
    "exponent",
    "multiplications",
    "r1cs_rows",
    "multiplications_per_row",
    "baby_bear_modulus",
    "quotient_bits",
    "canonicality",
    "operand_sampling",
    "r1cs_logical_columns",
    "r1cs_padded_columns",
    "r1cs_nnz_a",
    "r1cs_nnz_b",
    "r1cs_nnz_c",
    "r1cs_nnz",
    "logical_witness_values",
    "padded_assignment_values",
    "semantic_witness_bits",
    "committed_witness_bits",
    "committed_witness_bytes",
    "ligerito_hash",
    "ligerito_log_inv_rate",
    "ligerito_inverse_rate",
    "ligerito_initial_k",
    "witness_setup_ms",
    "relation_setup_ms",
    "projection_setup_ms",
    "bit_pack_setup_ms",
    "commitment_setup_ms",
    "prove_ms",
    "spartan_prove_ms",
    "spartan_outer_ms",
    "spartan_bind_ms",
    "spartan_inner_ms",
    "inner_native_coefficients_ms",
    "inner_native_witness_fold_ms",
    "inner_field_rounds_ms",
    "bitify_prove_ms",
    "bitz_prove_ms",
    "bitz_prepare_prove_ms",
    "prove_residual_ms",
    "verify_ms",
    "spartan_verify_ms",
    "bitify_verify_ms",
    "bitz_verify_ms",
    "bitz_prepare_verify_ms",
    "verify_residual_ms",
    "spartan_proof_payload_bytes",
    "bitz_proof_bytes",
    "peak_heap_mib",
    "live_before_prove_mib",
    "peak_heap_delta_mib",
    "memory_order",
    "memory_verified",
    "shape_seed",
    "verified_samples",
    "all_samples_verified",
)

PAIRED_FIELDS = (
    "scope",
    "exponent",
    "multiplications",
    "baseline_strategy",
    "baseline_order",
    "baseline_projection_backend",
    "optimized_strategy",
    "optimized_order",
    "optimized_projection_backend",
    "reduction_backend",
    "ligerito_hash",
    "ligerito_log_inv_rate",
    "ligerito_inverse_rate",
    "ligerito_initial_k",
    "baseline_prove_ms",
    "optimized_prove_ms",
    "prove_delta_ms",
    "prove_latency_reduction_pct",
    "prove_speedup",
    "baseline_spartan_ms",
    "optimized_spartan_ms",
    "spartan_delta_ms",
    "spartan_latency_reduction_pct",
    "spartan_speedup",
    "sizes_total",
    "geomean_prove_latency_reduction_pct",
    "geomean_spartan_latency_reduction_pct",
)

TIMING_FIELDS = (
    "prove_ms",
    "spartan_prove_ms",
    "spartan_outer_ms",
    "spartan_bind_ms",
    "spartan_inner_ms",
    "bitify_prove_ms",
    "bitz_prove_ms",
    "bitz_prepare_prove_ms",
    "prove_residual_ms",
    "verify_ms",
    "spartan_verify_ms",
    "bitify_verify_ms",
    "bitz_verify_ms",
    "bitz_prepare_verify_ms",
    "verify_residual_ms",
)

OPTIONAL_TIMING_FIELDS = (
    "inner_native_coefficients_ms",
    "inner_native_witness_fold_ms",
    "inner_field_rounds_ms",
)

RESULT_DETAIL_FIELDS = (
    "projection_backend",
    "r1cs_rows",
    "baby_bear_modulus",
    "quotient_bits",
    "canonicality",
    "operand_sampling",
    "r1cs_logical_columns",
    "r1cs_padded_columns",
    "r1cs_nnz_a",
    "r1cs_nnz_b",
    "r1cs_nnz_c",
    "r1cs_nnz",
    "logical_witness_values",
    "padded_assignment_values",
    "semantic_witness_bits",
    "committed_witness_bits",
    "committed_witness_bytes",
    "ligerito_hash",
    "ligerito_log_inv_rate",
    "ligerito_inverse_rate",
    "ligerito_initial_k",
    "witness_setup_ms",
    "relation_setup_ms",
    "projection_setup_ms",
    "bit_pack_setup_ms",
    "commitment_setup_ms",
    "spartan_proof_payload_bytes",
    "bitz_proof_bytes",
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
    parser.add_argument(
        "--cargo-features-requested",
        "--cargo-features",
        dest="cargo_features_requested",
        required=True,
    )
    parser.add_argument("--latency-cargo-features-resolved", required=True)
    parser.add_argument("--memory-cargo-features-resolved", required=True)
    parser.add_argument("--physical-memory-bytes", required=True)
    parser.add_argument("--physical-memory-gib", required=True)
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


def canonical_feature_values(value: str, name: str, *, allow_empty: bool = False) -> tuple[str, ...]:
    values = words(value)
    if not values and not allow_empty:
        raise SystemExit(f"{name} must contain at least one feature")
    if len(values) != len(set(values)):
        raise SystemExit(f"{name} contains duplicate features: {values}")
    return tuple(sorted(values))


def expected_inputs(args: argparse.Namespace) -> tuple[tuple[int, ...], tuple[str, ...], tuple[str, ...]]:
    try:
        exponent_values = [int(value) for value in words(args.expected_exponents)]
    except ValueError as error:
        raise SystemExit(f"invalid --expected-exponents: {error}") from error
    exponent_strings = unique_cli_values([str(value) for value in exponent_values], "expected exponents")
    exponents = tuple(int(value) for value in exponent_strings)
    if any(value < 15 for value in exponents):
        raise SystemExit("expected exponents must be at least 15")
    strategies = unique_cli_values(words(args.expected_strategies), "expected strategies")
    unknown_strategies = set(strategies) - set(ALLOWED_STRATEGIES)
    if unknown_strategies:
        raise SystemExit(
            "unknown strategies: " + ", ".join(sorted(unknown_strategies))
        )
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


def static_metadata(args: argparse.Namespace, exponents: tuple[int, ...]) -> dict[str, str]:
    requested_features = canonical_feature_values(
        args.cargo_features_requested,
        "requested Cargo features",
    )
    if any(
        feature == "bench-peak-memory" or feature.endswith("/bench-peak-memory")
        for feature in requested_features
    ):
        raise SystemExit(
            "requested Cargo features must not contain reserved bench-peak-memory"
        )

    latency_features = canonical_feature_values(
        args.latency_cargo_features_resolved,
        "resolved latency Cargo features",
    )
    latency_feature_set = set(latency_features)
    if not {"default", "parallel"}.issubset(latency_feature_set):
        raise SystemExit(
            "resolved latency Cargo features must include default and parallel"
        )
    if "bench-peak-memory" in latency_feature_set:
        raise SystemExit(
            "resolved latency Cargo features must not include bench-peak-memory"
        )
    if not set(requested_features).issubset(latency_feature_set):
        raise SystemExit(
            "resolved latency Cargo features do not include every requested feature"
        )

    memory_features = canonical_feature_values(
        args.memory_cargo_features_resolved,
        "resolved memory Cargo features",
        allow_empty=args.measure_memory == "0",
    )
    memory_feature_set = set(memory_features)
    if args.measure_memory == "1":
        if "bench-peak-memory" not in memory_feature_set:
            raise SystemExit(
                "resolved memory Cargo features must include bench-peak-memory"
            )
        if not latency_feature_set.issubset(memory_feature_set):
            raise SystemExit(
                "resolved memory Cargo features must include the latency feature closure"
            )
    elif memory_features:
        raise SystemExit(
            "resolved memory Cargo features must be empty when no memory binary is built"
        )

    try:
        physical_memory_bytes = int(args.physical_memory_bytes)
    except ValueError as error:
        raise SystemExit("--physical-memory-bytes must be a decimal integer") from error
    if not 0 < physical_memory_bytes <= (1 << 64) - 1:
        raise SystemExit("--physical-memory-bytes must be a positive u64")
    try:
        physical_memory_gib = float(args.physical_memory_gib)
    except ValueError as error:
        raise SystemExit("--physical-memory-gib must be finite and positive") from error
    expected_memory_gib = physical_memory_bytes / float(1 << 30)
    if (
        not math.isfinite(physical_memory_gib)
        or physical_memory_gib <= 0.0
        or not math.isclose(
            physical_memory_gib,
            expected_memory_gib,
            rel_tol=0.0,
            abs_tol=1.0e-6,
        )
    ):
        raise SystemExit(
            "--physical-memory-gib does not match --physical-memory-bytes: "
            f"{physical_memory_gib!r} vs {expected_memory_gib:.6f}"
        )

    memory_requirement_met = "not-applicable"
    if 25 in exponents:
        if physical_memory_bytes < EXPONENT_25_MINIMUM_MEMORY_GIB * (1 << 30):
            raise SystemExit(
                "exponent 25 requires at least 128 GiB physical memory, but metadata "
                f"records {physical_memory_bytes} bytes ({expected_memory_gib:.6f} GiB)"
            )
        memory_requirement_met = "true"

    return {
        "benchmark_algorithm": "BabyBear integer multiplication Spartan PIOP + BitZ",
        "benchmark_date": args.benchmark_date,
        "run_timestamp_utc": args.run_timestamp,
        "commit": args.commit,
        "git_dirty": args.git_dirty,
        "binary_sha256": args.binary_sha256,
        "memory_binary_sha256": args.memory_binary_sha256,
        "hardware": args.hardware,
        "operating_system": args.operating_system,
        "cargo_profile": "bench",
        "cargo_features_requested": ",".join(requested_features),
        "latency_cargo_features_resolved": ",".join(latency_features),
        "memory_cargo_features_resolved": ",".join(memory_features),
        "physical_memory_bytes": str(physical_memory_bytes),
        "physical_memory_gib": f"{expected_memory_gib:.6f}",
        "exponent_25_minimum_memory_gib": str(EXPONENT_25_MINIMUM_MEMORY_GIB),
        "exponent_25_memory_requirement_met": memory_requirement_met,
        "threads": args.threads,
        "measured_runs": str(args.measured_runs),
        "warmup_runs": "1",
        "field_modulus": "2^100-15",
        "allocator_instrumentation": (
            "absent_in_latency_binary;shared_atomic_peak_allocator_in_memory_binary"
            if args.measure_memory == "1"
            else "absent_in_latency_binary;memory_binary_not_built"
        ),
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


def optional_f64(record: dict[str, str], field: str) -> float | None:
    try:
        value = record[field]
    except KeyError as error:
        raise SystemExit(f"missing optional timing {field!r} in record: {record}") from error
    if value == "NA":
        return None
    return f64(record, field)


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


def positive_ratio(numerator: float, denominator: float, label: str) -> float:
    if numerator <= 0.0 or denominator <= 0.0:
        raise SystemExit(
            f"{label} requires positive timings, got numerator={numerator}, "
            f"denominator={denominator}"
        )
    return numerator / denominator


def common_value(rows: Iterable[dict[str, str]], field: str) -> str:
    values = {row[field] for row in rows}
    return next(iter(values)) if len(values) == 1 else "varies"


def paired_rows(summaries: list[dict[str, str]]) -> list[dict[str, str]]:
    by_exponent: dict[int, dict[str, dict[str, str]]] = {}
    for row in summaries:
        by_exponent.setdefault(exponent(row), {})[row["strategy"]] = row

    rows: list[dict[str, str]] = []
    prove_ratios: dict[str, list[float]] = {}
    spartan_ratios: dict[str, list[float]] = {}
    ligerito_fields = (
        "ligerito_hash",
        "ligerito_log_inv_rate",
        "ligerito_inverse_rate",
        "ligerito_initial_k",
    )
    for exp in sorted(by_exponent):
        strategies = by_exponent[exp]
        baseline = strategies.get("immediate")
        if baseline is None:
            continue
        baseline_prove_ms = f64(baseline, "prove_ms")
        baseline_spartan_ms = f64(baseline, "spartan_prove_ms")
        for name in ("delayed-barrett", "delayed-crypto-bigint"):
            optimized = strategies.get(name)
            if optimized is None:
                continue
            for field in ligerito_fields:
                if baseline[field] != optimized[field]:
                    raise SystemExit(
                        f"Ligerito metadata mismatch at exponent {exp} for {name}: "
                        f"{field}={baseline[field]!r} vs {optimized[field]!r}"
                    )
            optimized_prove_ms = f64(optimized, "prove_ms")
            optimized_spartan_ms = f64(optimized, "spartan_prove_ms")
            prove_ratio = positive_ratio(
                optimized_prove_ms,
                baseline_prove_ms,
                f"end-to-end comparison at exponent {exp} for {name}",
            )
            spartan_ratio = positive_ratio(
                optimized_spartan_ms,
                baseline_spartan_ms,
                f"Spartan comparison at exponent {exp} for {name}",
            )
            prove_ratios.setdefault(name, []).append(prove_ratio)
            spartan_ratios.setdefault(name, []).append(spartan_ratio)
            rows.append(
                {
                    "scope": "size",
                    "exponent": str(exp),
                    "multiplications": baseline["multiplications"],
                    "baseline_strategy": "immediate",
                    "baseline_order": baseline["order"],
                    "baseline_projection_backend": baseline["projection_backend"],
                    "optimized_strategy": name,
                    "optimized_order": optimized["order"],
                    "optimized_projection_backend": optimized["projection_backend"],
                    "reduction_backend": optimized["reduction_backend"],
                    **{field: baseline[field] for field in ligerito_fields},
                    "baseline_prove_ms": formatted(baseline_prove_ms),
                    "optimized_prove_ms": formatted(optimized_prove_ms),
                    "prove_delta_ms": formatted(optimized_prove_ms - baseline_prove_ms),
                    "prove_latency_reduction_pct": formatted(100.0 * (1.0 - prove_ratio)),
                    "prove_speedup": formatted(1.0 / prove_ratio),
                    "baseline_spartan_ms": formatted(baseline_spartan_ms),
                    "optimized_spartan_ms": formatted(optimized_spartan_ms),
                    "spartan_delta_ms": formatted(optimized_spartan_ms - baseline_spartan_ms),
                    "spartan_latency_reduction_pct": formatted(
                        100.0 * (1.0 - spartan_ratio)
                    ),
                    "spartan_speedup": formatted(1.0 / spartan_ratio),
                }
            )

    for name, samples in prove_ratios.items():
        prove_geometric_ratio = math.exp(
            sum(math.log(value) for value in samples) / len(samples)
        )
        spartan_samples = spartan_ratios[name]
        spartan_geometric_ratio = math.exp(
            sum(math.log(value) for value in spartan_samples) / len(spartan_samples)
        )
        strategy_rows = [row for row in summaries if row["strategy"] == name]
        rows.append(
            {
                "scope": "overall",
                "baseline_strategy": "immediate",
                "baseline_projection_backend": EXPECTED_PROJECTION_BACKENDS["immediate"],
                "optimized_strategy": name,
                "optimized_projection_backend": common_value(
                    strategy_rows,
                    "projection_backend",
                ),
                "reduction_backend": strategy_rows[0]["reduction_backend"],
                **{field: common_value(strategy_rows, field) for field in ligerito_fields},
                "sizes_total": str(len(samples)),
                "geomean_prove_latency_reduction_pct": formatted(
                    100.0 * (1.0 - prove_geometric_ratio)
                ),
                "geomean_spartan_latency_reduction_pct": formatted(
                    100.0 * (1.0 - spartan_geometric_ratio)
                ),
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


def integer(record: dict[str, str], field: str) -> int:
    try:
        return int(record[field])
    except (KeyError, ValueError) as error:
        raise SystemExit(f"invalid or missing integer {field!r} in record: {record}") from error


def validate_relation_shape(result: dict[str, str], pair: tuple[int, str]) -> None:
    exp = exponent(result)
    expected_multiplications = 1 << exp
    expected = {
        "multiplications": expected_multiplications,
        "baby_bear_modulus": BABY_BEAR_MODULUS,
        "quotient_bits": 31,
        "r1cs_rows": expected_multiplications,
        "r1cs_logical_columns": 5 * expected_multiplications,
        "r1cs_padded_columns": 8 * expected_multiplications,
        "r1cs_nnz_a": expected_multiplications,
        "r1cs_nnz_b": expected_multiplications,
        "r1cs_nnz_c": 2 * expected_multiplications,
        "r1cs_nnz": 4 * expected_multiplications,
        "logical_witness_values": 5 * expected_multiplications,
        "padded_assignment_values": 8 * expected_multiplications,
        "semantic_witness_bits": 124 * expected_multiplications,
        "committed_witness_bits": 128 * expected_multiplications,
        "committed_witness_bytes": 16 * expected_multiplications,
    }
    mismatches = [
        f"{field}={integer(result, field)} (expected {value})"
        for field, value in expected.items()
        if integer(result, field) != value
    ]
    if result.get("canonicality") != "host-precondition":
        mismatches.append(
            f"canonicality={result.get('canonicality')!r} (expected 'host-precondition')"
        )
    if result.get("operand_sampling") != OPERAND_SAMPLING:
        mismatches.append(
            f"operand_sampling={result.get('operand_sampling')!r} "
            f"(expected {OPERAND_SAMPLING!r})"
        )
    if mismatches:
        raise SystemExit(f"invalid BabyBear relation shape for {pair}: " + "; ".join(mismatches))


def validate_ligerito_metadata(result: dict[str, str], pair: tuple[int, str]) -> None:
    if result["ligerito_hash"] not in {"sha256", "blake3"}:
        raise SystemExit(
            f"unsupported Ligerito hash for {pair}: {result['ligerito_hash']!r}"
        )
    log_inv_rate = integer(result, "ligerito_log_inv_rate")
    if not 1 <= log_inv_rate < 64:
        raise SystemExit(f"invalid Ligerito inverse-rate log for {pair}: {log_inv_rate}")
    expected_inverse_rate = f"1/{1 << log_inv_rate}"
    if result["ligerito_inverse_rate"] != expected_inverse_rate:
        raise SystemExit(
            f"invalid Ligerito inverse rate for {pair}: "
            f"{result['ligerito_inverse_rate']!r}, expected {expected_inverse_rate!r}"
        )
    initial_k = integer(result, "ligerito_initial_k")
    if initial_k <= 0:
        raise SystemExit(f"invalid Ligerito initial_k for {pair}: {initial_k}")


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
    # Keep higher-precision medians for comparisons; presentation-only paired
    # columns are rounded separately by `formatted`.
    result[field] = formatted_timing(computed)


def validate_optional_result_median(
    result: dict[str, str],
    samples: list[dict[str, str]],
    field: str,
    pair: tuple[int, str],
) -> None:
    sample_values = [optional_f64(sample, field) for sample in samples]
    present = [value for value in sample_values if value is not None]
    if present and len(present) != len(sample_values):
        raise SystemExit(
            f"optional phase {field!r} is present for only {len(present)} of "
            f"{len(sample_values)} samples for {pair}"
        )
    reported = optional_f64(result, field)
    if not present:
        if reported is not None:
            raise SystemExit(
                f"RESULT reports optional phase {field!r} for {pair}, but samples do not"
            )
        result[field] = "NA"
        return
    computed = benchmark_median(present)
    if reported is None:
        raise SystemExit(f"RESULT omits optional phase {field!r} for {pair}")
    tolerance = max(1.0e-6, abs(computed) * 1.0e-9)
    if not math.isclose(reported, computed, rel_tol=0.0, abs_tol=tolerance):
        raise SystemExit(
            f"RESULT median mismatch for {pair} {field}: "
            f"reported={reported:.12g}, recomputed={computed:.12g}"
        )
    result[field] = formatted_timing(computed)


def validate_latency_order(
    exponents: tuple[int, ...],
    strategies: tuple[str, ...],
    summaries: list[dict[str, str]],
) -> None:
    by_exponent: dict[int, dict[str, dict[str, str]]] = defaultdict(dict)
    for row in summaries:
        by_exponent[exponent(row)][row["strategy"]] = row
    for exponent_index, exp in enumerate(exponents):
        rows = by_exponent[exp]
        orders = {name: integer(row, "order") for name, row in rows.items()}
        for field in (
            "ligerito_hash",
            "ligerito_log_inv_rate",
            "ligerito_inverse_rate",
            "ligerito_initial_k",
            "shape_seed",
        ):
            values = {row[field] for row in rows.values()}
            if len(values) != 1:
                raise SystemExit(
                    f"benchmark metadata {field!r} differs across strategies at "
                    f"exponent {exp}: {sorted(values)}"
                )
        if sorted(orders.values()) != list(range(1, len(strategies) + 1)):
            raise SystemExit(f"latency orders at exponent {exp} are not 1..{len(strategies)}: {orders}")
        if "delayed-crypto-bigint" in orders and orders["delayed-crypto-bigint"] != len(strategies):
            raise SystemExit(f"delayed-crypto-bigint was not scheduled last at exponent {exp}: {orders}")
        if "immediate" in orders and "delayed-barrett" in orders:
            expected_first = "immediate" if exponent_index % 2 == 0 else "delayed-barrett"
            expected_second = (
                "delayed-barrett" if expected_first == "immediate" else "immediate"
            )
            if not orders[expected_first] < orders[expected_second]:
                raise SystemExit(
                    f"Immediate/Barrett order is not counterbalanced at exponent {exp}: {orders}"
                )


def validate_memory_order(
    exponents: tuple[int, ...],
    strategies: tuple[str, ...],
    summaries: list[dict[str, str]],
) -> None:
    if not strategies:
        return
    by_exponent: dict[int, dict[str, dict[str, str]]] = defaultdict(dict)
    for row in summaries:
        if row["strategy"] in strategies:
            by_exponent[exponent(row)][row["strategy"]] = row
    for exponent_index, exp in enumerate(exponents):
        rows = by_exponent[exp]
        orders = {name: integer(row, "memory_order") for name, row in rows.items()}
        if sorted(orders.values()) != list(range(1, len(strategies) + 1)):
            raise SystemExit(
                f"memory orders at exponent {exp} are not 1..{len(strategies)}: {orders}"
            )
        if (
            "delayed-crypto-bigint" in orders
            and orders["delayed-crypto-bigint"] != len(strategies)
        ):
            raise SystemExit(
                f"delayed-crypto-bigint memory sample was not scheduled last "
                f"at exponent {exp}: {orders}"
            )
        if "immediate" in orders and "delayed-barrett" in orders:
            expected_first = "immediate" if exponent_index % 2 == 0 else "delayed-barrett"
            expected_second = (
                "delayed-barrett" if expected_first == "immediate" else "immediate"
            )
            if not orders[expected_first] < orders[expected_second]:
                raise SystemExit(
                    f"Immediate/Barrett memory order is not counterbalanced "
                    f"at exponent {exp}: {orders}"
                )


def main() -> None:
    args = parse_args()
    output_paths = (args.raw_csv, args.summary_csv, args.paired_csv)
    if len(set(output_paths)) != len(output_paths):
        raise SystemExit("raw, summary, and paired CSV paths must be distinct")
    existing_outputs = [path for path in output_paths if path.exists()]
    if existing_outputs:
        raise SystemExit(
            "refusing to overwrite existing report artifact(s): "
            + ", ".join(str(path) for path in existing_outputs)
    )
    exponents, strategies, memory_strategies = expected_inputs(args)
    metadata = static_metadata(args, exponents)
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
                *OPTIONAL_TIMING_FIELDS,
                *RESULT_DETAIL_FIELDS,
            ),
            "RESULT",
        )
        if result["pass"] != "latency":
            raise SystemExit(f"RESULT record has non-latency pass for {pair}: {result['pass']}")
        expected_backend = EXPECTED_BACKENDS[result["strategy"]]
        if result["reduction_backend"] != expected_backend:
            raise SystemExit(
                f"wrong reduction backend for {pair}: {result['reduction_backend']!r}, "
                f"expected {expected_backend!r}"
            )
        expected_projection_backend = EXPECTED_PROJECTION_BACKENDS[result["strategy"]]
        if result["projection_backend"] != expected_projection_backend:
            raise SystemExit(
                f"wrong projection backend for {pair}: {result['projection_backend']!r}, "
                f"expected {expected_projection_backend!r}"
            )
        validate_relation_shape(result, pair)
        validate_ligerito_metadata(result, pair)
        for field in (
            "witness_setup_ms",
            "relation_setup_ms",
            "projection_setup_ms",
            "bit_pack_setup_ms",
            "commitment_setup_ms",
        ):
            if f64(result, field) < 0.0:
                raise SystemExit(f"negative setup timing {field!r} for {pair}: {result[field]}")
        for field in ("spartan_proof_payload_bytes", "bitz_proof_bytes"):
            if integer(result, field) <= 0:
                raise SystemExit(f"non-positive proof size {field!r} for {pair}: {result[field]}")
        try:
            int(result["shape_seed"], 0)
        except ValueError as error:
            raise SystemExit(f"invalid shape_seed for {pair}: {result['shape_seed']!r}") from error
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
                    "projection_backend",
                    "exponent",
                    "multiplications",
                    "sample",
                    "verified",
                    *TIMING_FIELDS,
                    *OPTIONAL_TIMING_FIELDS,
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
            for identity in (
                "order",
                "reduction_backend",
                "projection_backend",
                "multiplications",
            ):
                if sample[identity] != result[identity]:
                    raise SystemExit(
                        f"SAMPLE/RESULT {identity} mismatch for {pair}: "
                        f"{sample[identity]} != {result[identity]}"
                    )
            for field in TIMING_FIELDS:
                if f64(sample, field) < 0.0:
                    raise SystemExit(f"negative timing {field!r} for {pair}: {sample[field]}")
            for field in OPTIONAL_TIMING_FIELDS:
                value = optional_f64(sample, field)
                if value is not None and value < 0.0:
                    raise SystemExit(
                        f"negative optional timing {field!r} for {pair}: {sample[field]}"
                    )
        if sorted(sample_indices) != list(range(args.measured_runs)):
            raise SystemExit(
                f"SAMPLE indices for {pair} must be unique 0..{args.measured_runs - 1}: "
                f"{sorted(sample_indices)}"
            )

        for field in TIMING_FIELDS:
            computed = benchmark_median(f64(sample, field) for sample in pair_samples)
            validate_result_median(result, field, computed, pair)
        for field in OPTIONAL_TIMING_FIELDS:
            validate_optional_result_median(result, pair_samples, field, pair)

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
                    "projection_backend",
                    "exponent",
                    "multiplications",
                    "peak_heap_mib",
                    "live_before_prove_mib",
                    "peak_heap_delta_mib",
                    "verified",
                ),
                "MEMORY",
            )
            if memory["pass"] != "memory":
                raise SystemExit(f"MEMORY record has non-memory pass for {pair}: {memory['pass']}")
            if memory["verified"].lower() != "true":
                raise SystemExit(f"unverified MEMORY record for {pair}: {memory}")
            for identity in (
                "reduction_backend",
                "projection_backend",
                "multiplications",
            ):
                if memory[identity] != result[identity]:
                    raise SystemExit(
                        f"MEMORY/RESULT {identity} mismatch for {pair}: "
                        f"{memory[identity]} != {result[identity]}"
                    )
            peak_heap_mib = f64(memory, "peak_heap_mib")
            live_before_prove_mib = f64(memory, "live_before_prove_mib")
            peak_heap_delta_mib = f64(memory, "peak_heap_delta_mib")
            if min(peak_heap_mib, live_before_prove_mib, peak_heap_delta_mib) < 0.0:
                raise SystemExit(f"negative heap metric for {pair}: {memory}")
            expected_delta = max(peak_heap_mib - live_before_prove_mib, 0.0)
            if not math.isclose(
                peak_heap_delta_mib,
                expected_delta,
                rel_tol=0.0,
                abs_tol=2.0e-6,
            ):
                raise SystemExit(
                    f"MEMORY peak delta mismatch for {pair}: "
                    f"reported={peak_heap_delta_mib}, expected={expected_delta}"
                )

        result["multiplications_per_row"] = "1"
        result["peak_heap_mib"] = memory["peak_heap_mib"] if memory else ""
        result["live_before_prove_mib"] = memory["live_before_prove_mib"] if memory else ""
        result["peak_heap_delta_mib"] = memory["peak_heap_delta_mib"] if memory else ""
        result["memory_order"] = memory["order"] if memory else ""
        result["memory_verified"] = memory["verified"].lower() if memory else ""
        result["all_samples_verified"] = "true"

        for sample in pair_samples:
            sample["multiplications_per_row"] = "1"
            for field in RESULT_DETAIL_FIELDS:
                sample[field] = result[field]
            sample["peak_heap_mib"] = result["peak_heap_mib"]
            sample["live_before_prove_mib"] = result["live_before_prove_mib"]
            sample["peak_heap_delta_mib"] = result["peak_heap_delta_mib"]
            sample["memory_order"] = result["memory_order"]
            sample["memory_verified"] = result["memory_verified"]
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
    validate_latency_order(exponents, strategies, ordered_summaries)
    if args.measure_memory == "1":
        validate_memory_order(exponents, memory_strategies, ordered_summaries)

    write_csv(args.raw_csv, SAMPLE_FIELDS, ordered_samples, metadata)
    write_csv(args.summary_csv, SUMMARY_FIELDS, ordered_summaries, metadata)
    write_csv(args.paired_csv, PAIRED_FIELDS, paired_rows(ordered_summaries), metadata)


if __name__ == "__main__":
    main()

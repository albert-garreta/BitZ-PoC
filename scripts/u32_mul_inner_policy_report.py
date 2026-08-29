#!/usr/bin/env python3
"""Parse controlled u32 inner-policy benchmark output into CSV artifacts."""

from __future__ import annotations

import argparse
import csv
import math
import re
from collections import defaultdict
from pathlib import Path


SAMPLE_PREFIX = "SAMPLE "
TEXT_FIELDS = {
    "policy",
    "native_fold",
    "field_accumulation",
    "verified",
}
INTEGER_FIELDS = {
    "order",
    "exponent",
    "multiplications",
    "integer_witness_values",
    "sample",
    "spartan_proof_payload_bytes",
}
TIMING_FIELDS = [
    "spartan_prove_ms",
    "spartan_outer_ms",
    "spartan_bind_ms",
    "spartan_inner_ms",
    "inner_native_coefficients_ms",
    "inner_native_witness_fold_ms",
    "inner_field_rounds_ms",
    "spartan_verify_ms",
]
BASELINE_POLICY = "fold-delayed__field-delayed"


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("log", type=Path)
    parser.add_argument("--raw-csv", required=True, type=Path)
    parser.add_argument("--summary-csv", required=True, type=Path)
    parser.add_argument("--paired-csv", required=True, type=Path)
    parser.add_argument("--overall-csv", required=True, type=Path)
    parser.add_argument("--metadata", action="append", default=[])
    return parser.parse_args()


def parse_metadata(items: list[str]) -> dict[str, str]:
    metadata: dict[str, str] = {}
    for item in items:
        key, separator, value = item.partition("=")
        if not separator:
            raise ValueError(f"metadata must be key=value, got {item!r}")
        metadata[key] = value
    return metadata


def parse_samples(path: Path) -> list[dict[str, object]]:
    samples: list[dict[str, object]] = []
    for line in path.read_text().splitlines():
        if not line.startswith(SAMPLE_PREFIX):
            continue
        fields = dict(re.findall(r"([a-z_]+)=([^ ]+)", line[len(SAMPLE_PREFIX) :]))
        row: dict[str, object] = {}
        for key, value in fields.items():
            if key in TEXT_FIELDS:
                row[key] = value
            elif key in INTEGER_FIELDS:
                row[key] = int(value)
            else:
                row[key] = float(value)
        exponent = int(row["exponent"])
        multiplications = int(row["multiplications"])
        row.setdefault("integer_witness_values", 4 * multiplications)
        row.setdefault("spartan_proof_payload_bytes", 16 * (7 * exponent + 9))
        samples.append(row)
    if not samples:
        raise ValueError(f"no SAMPLE rows found in {path}")
    return samples


def median(values: list[float]) -> float:
    ordered = sorted(values)
    return ordered[len(ordered) // 2]


def geometric_mean(values: list[float]) -> float:
    return math.exp(sum(math.log(value) for value in values) / len(values))


def write_csv(path: Path, rows: list[dict[str, object]], columns: list[str]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("w", newline="") as output:
        writer = csv.DictWriter(output, fieldnames=columns, lineterminator="\n")
        writer.writeheader()
        writer.writerows(rows)


def main() -> None:
    args = parse_args()
    metadata = parse_metadata(args.metadata)
    samples = parse_samples(args.log)
    metadata_columns = list(metadata)

    raw_rows = [{**metadata, **sample} for sample in samples]
    raw_columns = metadata_columns + [
        "order",
        "policy",
        "native_fold",
        "field_accumulation",
        "exponent",
        "multiplications",
        "integer_witness_values",
        "sample",
        *TIMING_FIELDS,
        "spartan_proof_payload_bytes",
        "verified",
    ]
    write_csv(args.raw_csv, raw_rows, raw_columns)

    grouped: dict[tuple[int, str], list[dict[str, object]]] = defaultdict(list)
    for sample in samples:
        grouped[(int(sample["exponent"]), str(sample["policy"]))].append(sample)

    summary_rows: list[dict[str, object]] = []
    for (exponent, policy), group in sorted(grouped.items()):
        first = group[0]
        row: dict[str, object] = {
            **metadata,
            "order": first["order"],
            "policy": policy,
            "native_fold": first["native_fold"],
            "field_accumulation": first["field_accumulation"],
            "exponent": exponent,
            "multiplications": first["multiplications"],
            "integer_witness_values": first["integer_witness_values"],
            "spartan_proof_payload_bytes": first["spartan_proof_payload_bytes"],
            "measured_samples": len(group),
            "verified_samples": sum(sample["verified"] == "true" for sample in group),
        }
        for timing in TIMING_FIELDS:
            row[timing] = median([float(sample[timing]) for sample in group])
        summary_rows.append(row)

    summary_columns = metadata_columns + [
        "order",
        "policy",
        "native_fold",
        "field_accumulation",
        "exponent",
        "multiplications",
        "integer_witness_values",
        "measured_samples",
        "verified_samples",
        *TIMING_FIELDS,
        "spartan_proof_payload_bytes",
    ]
    write_csv(args.summary_csv, summary_rows, summary_columns)

    by_exponent: dict[int, dict[str, dict[str, object]]] = defaultdict(dict)
    for row in summary_rows:
        by_exponent[int(row["exponent"])][str(row["policy"])] = row

    paired_rows: list[dict[str, object]] = []
    policy_wins: dict[str, int] = defaultdict(int)
    for exponent, policies in sorted(by_exponent.items()):
        if BASELINE_POLICY not in policies:
            raise ValueError(f"missing baseline policy at exponent {exponent}")
        baseline = policies[BASELINE_POLICY]
        best_policy = min(
            policies,
            key=lambda policy: float(policies[policy]["spartan_inner_ms"]),
        )
        policy_wins[best_policy] += 1
        for policy, candidate in sorted(policies.items()):
            baseline_inner = float(baseline["spartan_inner_ms"])
            candidate_inner = float(candidate["spartan_inner_ms"])
            baseline_total = float(baseline["spartan_prove_ms"])
            candidate_total = float(candidate["spartan_prove_ms"])
            paired_rows.append(
                {
                    **metadata,
                    "exponent": exponent,
                    "multiplications": candidate["multiplications"],
                    "baseline_policy": BASELINE_POLICY,
                    "candidate_policy": policy,
                    "baseline_inner_ms": baseline_inner,
                    "candidate_inner_ms": candidate_inner,
                    "inner_latency_reduction_pct":
                        100.0 * (baseline_inner - candidate_inner) / baseline_inner,
                    "baseline_spartan_ms": baseline_total,
                    "candidate_spartan_ms": candidate_total,
                    "spartan_latency_reduction_pct":
                        100.0 * (baseline_total - candidate_total) / baseline_total,
                    "best_inner_at_exponent": policy == best_policy,
                }
            )

    paired_columns = metadata_columns + [
        "exponent",
        "multiplications",
        "baseline_policy",
        "candidate_policy",
        "baseline_inner_ms",
        "candidate_inner_ms",
        "inner_latency_reduction_pct",
        "baseline_spartan_ms",
        "candidate_spartan_ms",
        "spartan_latency_reduction_pct",
        "best_inner_at_exponent",
    ]
    write_csv(args.paired_csv, paired_rows, paired_columns)

    by_policy: dict[str, list[dict[str, object]]] = defaultdict(list)
    for row in summary_rows:
        by_policy[str(row["policy"])].append(row)
    baseline_rows = sorted(by_policy[BASELINE_POLICY], key=lambda row: int(row["exponent"]))
    baseline_inner_gm = geometric_mean(
        [float(row["spartan_inner_ms"]) for row in baseline_rows]
    )
    baseline_total_gm = geometric_mean(
        [float(row["spartan_prove_ms"]) for row in baseline_rows]
    )
    overall_rows: list[dict[str, object]] = []
    for policy, rows in sorted(by_policy.items()):
        ordered = sorted(rows, key=lambda row: int(row["exponent"]))
        inner_gm = geometric_mean([float(row["spartan_inner_ms"]) for row in ordered])
        total_gm = geometric_mean([float(row["spartan_prove_ms"]) for row in ordered])
        overall_rows.append(
            {
                **metadata,
                "policy": policy,
                "native_fold": ordered[0]["native_fold"],
                "field_accumulation": ordered[0]["field_accumulation"],
                "exponents": len(ordered),
                "inner_geometric_mean_ms": inner_gm,
                "inner_latency_reduction_pct":
                    100.0 * (baseline_inner_gm - inner_gm) / baseline_inner_gm,
                "spartan_geometric_mean_ms": total_gm,
                "spartan_latency_reduction_pct":
                    100.0 * (baseline_total_gm - total_gm) / baseline_total_gm,
                "best_inner_exponent_count": policy_wins[policy],
            }
        )
    overall_rows.sort(key=lambda row: float(row["inner_geometric_mean_ms"]))
    for rank, row in enumerate(overall_rows, 1):
        row["inner_rank"] = rank
    overall_columns = metadata_columns + [
        "inner_rank",
        "policy",
        "native_fold",
        "field_accumulation",
        "exponents",
        "inner_geometric_mean_ms",
        "inner_latency_reduction_pct",
        "spartan_geometric_mean_ms",
        "spartan_latency_reduction_pct",
        "best_inner_exponent_count",
    ]
    write_csv(args.overall_csv, overall_rows, overall_columns)


if __name__ == "__main__":
    main()

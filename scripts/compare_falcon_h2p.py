#!/usr/bin/env python3
"""Compare full Falcon measurements with the archived, fixed-seed baseline.

Timing confidence intervals resample the two archives independently: these are
not paired multi-seed measurements. RSS is the process cumulative high-water
mark, including setup and warmup, as recorded by the existing benchmark.
"""

import argparse
import itertools
import json
import random
import statistics
from pathlib import Path


def read_case(path):
    records = [json.loads(line) for line in path.read_text().splitlines()]
    header = next(row for row in records if row.get("event") == "prepared")
    trials = [row for row in records if row.get("event") == "trial"]
    samples = [row for row in trials if row["trial"] == "sample"]
    if len(samples) != 5 or len(trials) != 6 or trials[0]["trial"] != "warmup":
        raise ValueError(f"unexpected trial schedule: {path}")
    if not all(row["verified"] for row in trials):
        raise ValueError(f"unverified proof: {path}")
    if header["algebraic_security_bits"] < header["security_target"]:
        raise ValueError(f"security target not met: {path}")
    return header, samples


def timing_comparison(baseline, candidate):
    rng = random.Random(0)
    ratios = sorted(
        statistics.mean(rng.choices(candidate, k=len(candidate)))
        / statistics.mean(rng.choices(baseline, k=len(baseline)))
        for _ in range(20_000)
    )
    upper = ratios[int(0.95 * (len(ratios) - 1))]
    median_ratio = statistics.median(candidate) / statistics.median(baseline)
    return {
        "baseline_median_ms": statistics.median(baseline),
        "candidate_median_ms": statistics.median(candidate),
        "median_ratio": median_ratio,
        "mean_ratio": statistics.mean(candidate) / statistics.mean(baseline),
        "independent_bootstrap_upper_95": upper,
        "pass": upper <= 1.0 and median_ratio <= 1.0,
    }


def compare(baseline_dir, candidate_dir):
    cases, missing = [], []
    matched = (
        "batch", "capacity", "degree", "security_target", "ring_extension",
        "threads", "input_digest", "input_seed", "input_source", "input_mode",
        "input_rng", "input_implementation_version", "build_rustflags",
        "compiled_target_features", "target_arch", "gf128_kernel", "cpu_affinity",
        "warmup_trials", "measured_trials",
    )
    for degree, security, threads in itertools.product(
        (512, 1024), (100, 128), (1, 2, 4, 8, 16)
    ):
        name = f"full-n{degree}-b1024-s{security}-t{threads}.jsonl"
        candidate_path = candidate_dir / name
        if not candidate_path.is_file():
            missing.append(name)
            continue
        old_header, old = read_case(baseline_dir / name)
        new_header, new = read_case(candidate_path)
        for key in matched:
            if old_header[key] != new_header[key]:
                raise ValueError(f"{name}: unmatched {key}")
        if old_header["protocol"] == new_header["protocol"]:
            raise ValueError(f"{name}: HashToPoint protocol was not versioned")
        timings = {
            key: timing_comparison([r[key] for r in old], [r[key] for r in new])
            for key in ("total_prover_ms", "proof_verify_ms")
        }
        old_rss = max(row["process_peak_rss_kib"] for row in old)
        new_rss = max(row["process_peak_rss_kib"] for row in new)
        case = {
            "degree": degree, "security": security, "threads": threads,
            "timings": timings,
            "baseline_peak_rss_kib": old_rss,
            "candidate_peak_rss_kib": new_rss,
            "peak_rss_ratio": new_rss / old_rss,
            "baseline_payload_bytes": statistics.median(r["proof_payload_bytes"] for r in old),
            "candidate_payload_bytes": statistics.median(r["proof_payload_bytes"] for r in new),
            "candidate_selection_mask_bytes": statistics.median(
                r.get("proof_payload_breakdown", {}).get("hash_to_point_selection_masks", 0)
                for r in new
            ),
            "witness": {
                "baseline_live_arithmetic_bits_per_signature": old_header["arithmetic_live_bits_per_signature"],
                "candidate_live_arithmetic_bits_per_signature": new_header["arithmetic_live_bits_per_signature"],
                "baseline_padded_arithmetic_bits_per_signature": old_header["source_bits_per_signature"][0],
                "candidate_padded_arithmetic_bits_per_signature": new_header["source_bits_per_signature"][0],
                "baseline_total_packed_bytes_per_signature": sum(old_header["source_bits_per_signature"]) // 8,
                "candidate_total_packed_bytes_per_signature": sum(new_header["source_bits_per_signature"]) // 8,
                "baseline_total_packed_bytes": old_header["capacity"] * sum(old_header["source_bits_per_signature"]) // 8,
                "candidate_total_packed_bytes": new_header["capacity"] * sum(new_header["source_bits_per_signature"]) // 8,
            },
            "pass": all(value["pass"] for value in timings.values()),
        }
        cases.append(case)
    return {
        "baseline": str(baseline_dir), "candidate": str(candidate_dir),
        "scope": "full Falcon, batch 1024, archived input seed 42",
        "confidence_scope": "independent resampling of five timing samples; not multi-seed qualification",
        "gate": "prover and verifier timings; RSS and payload are reported, not pass/fail criteria",
        "missing": missing, "cases": cases,
        "pass": not missing and len(cases) == 20 and all(case["pass"] for case in cases),
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("candidate", type=Path)
    parser.add_argument("--baseline", type=Path, default=Path(__file__).resolve().parents[1]
                        / "results/falcon-power-basis-20261007/results-candidate")
    args = parser.parse_args()
    report = compare(args.baseline, args.candidate)
    print(json.dumps(report, indent=2))
    return 0 if report["pass"] else 1


if __name__ == "__main__":
    raise SystemExit(main())

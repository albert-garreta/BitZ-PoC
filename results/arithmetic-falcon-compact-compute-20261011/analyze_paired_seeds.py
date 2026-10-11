#!/usr/bin/env python3
"""Supplement observed timing gates with input-paired, seed-level dispersion.

Run only after the timing matrix has finished. Each seed, not each repeated
timing measurement, is one independent unit for the bootstrap.
"""
import argparse
import json
import math
from pathlib import Path
import random
import statistics


def quantile(values, probability):
    ordered = sorted(values)
    location = (len(ordered) - 1) * probability
    lower = math.floor(location)
    upper = math.ceil(location)
    return ordered[lower] + (location - lower) * (ordered[upper] - ordered[lower])


def delta(ratio):
    return 100 * (ratio - 1)


def analyze(directory, replicates):
    primary = json.loads((directory / "summary.json").read_text())
    seeds = json.loads((directory / "seed-summary.json").read_text())
    dimensions = ("degree", "security_bits", "batch", "threads")
    results = []
    for row in primary:
        shape = {key: row[key] for key in dimensions}
        pairs = [seed for seed in seeds if all(seed[key] == row[key] for key in dimensions)]
        result = dict(shape, independent_seeds=len(pairs), metrics={})
        for metric in ("total_prover_ms", "prove_ms"):
            ratios = [pair["candidate"][metric] / pair["baseline"][metric] for pair in pairs]
            logs = [math.log(ratio) for ratio in ratios]
            # Fixed reproducible bootstrap seed; independent of benchmark inputs.
            rng = random.Random(20261011)
            bootstrap_median = []
            bootstrap_geomean = []
            for _ in range(replicates):
                selected = [rng.randrange(len(pairs)) for _ in pairs]
                bootstrap_median.append(statistics.median(ratios[index] for index in selected))
                bootstrap_geomean.append(math.exp(statistics.mean(logs[index] for index in selected)))
            result["metrics"][metric] = {
                "primary_pooled_sample_median_delta_percent": -row["reduction_percent"][metric],
                "paired_seed_median_delta_percent": delta(statistics.median(ratios)),
                "paired_seed_geomean_delta_percent": delta(math.exp(statistics.mean(logs))),
                "paired_seed_median_bootstrap_95_percent_interval": [delta(quantile(bootstrap_median, p)) for p in (0.025, 0.975)],
                "paired_seed_geomean_bootstrap_95_percent_interval": [delta(quantile(bootstrap_geomean, p)) for p in (0.025, 0.975)],
                "paired_seed_delta_percentiles": {str(p): delta(quantile(ratios, p / 100)) for p in (0, 10, 25, 50, 75, 90, 100)},
                "seeds_candidate_slower": sum(ratio > 1 for ratio in ratios),
                "seeds_candidate_faster": sum(ratio < 1 for ratio in ratios),
                "paired_seed_deltas_percent": [{"seed": pair["seed"], "delta_percent": delta(ratio)} for pair, ratio in zip(pairs, ratios)],
            }
        results.append(result)
    report = {
        "source_directory": str(directory),
        "primary_gate_unchanged": True,
        "bootstrap_replicates": replicates,
        "bootstrap_prng_seed": 20261011,
        "unit": "One matched input seed, summarized by the median of its measured timing repetitions for each variant.",
        "interpretation": "Positive deltas mean candidate slower. Supplemental paired-seed summaries do not replace the pooled-sample median gate. Percentile bootstrap intervals describe observed seed variability with no across-shape multiplicity correction; they cannot establish universal non-regression. Repeated timings on one seed repeat transcript-dependent prime/grinding work and are not additional independent inputs. Timing noise and input-dependent work are not separately identified by this analysis.",
        "shapes": results,
    }
    (directory / "paired-seed-analysis.json").write_text(json.dumps(report, indent=2) + "\n")
    return report


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("directories", type=Path, nargs="+")
    parser.add_argument("--replicates", type=int, default=10000)
    args = parser.parse_args()
    if args.replicates <= 0:
        raise ValueError("replicates must be positive")
    for directory in args.directories:
        report = analyze(directory, args.replicates)
        print(json.dumps({"directory": str(directory), "shapes": len(report["shapes"]), "bootstrap_replicates": args.replicates}))


if __name__ == "__main__":
    main()

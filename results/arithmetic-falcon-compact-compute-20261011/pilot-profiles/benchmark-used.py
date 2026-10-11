#!/usr/bin/env python3
"""Run frozen arithmetic Falcon variants against one matched input plan.

All processes are sequential. Build binaries before invoking this script and
coordinate with other workers so no builds, tests, profiles, or analyses run
concurrently. --describe-plan only inspects JSON; it never launches a binary.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import statistics
import subprocess
import sys
import time


HERE = Path(__file__).resolve().parent
DIMENSIONS = ("degree", "security_bits", "batch", "threads")
METRICS = (
    "prepare_ms", "witness_commit_ms", "prove_ms", "total_prover_ms",
    "verify_ms", "proof_payload_bytes", "live_bits_per_signature",
    "source_bits_per_signature", "committed_source_bits",
    "virtual_arithmetic_bits", "capacity", "arithmetic_capacity", "logical_binding_bits",
    "arithmetic_variables", "arithmetic_witness_elements", "arithmetic_bits",
    "source_packed_bytes", "process_peak_rss_kib",
)
REQUIRED_METRICS = (
    "witness_commit_ms", "prove_ms", "total_prover_ms", "verify_ms",
    "proof_payload_bytes", "committed_source_bits",
)
PROVING_METRICS = ("total_prover_ms", "prove_ms")


def write_json(path, value):
    path.write_text(json.dumps(value, indent=2) + "\n")


def numeric(value):
    return isinstance(value, (int, float)) and not isinstance(value, bool)


def medians(rows):
    return {
        metric: statistics.median(row[metric] for row in rows)
        for metric in METRICS
        if all(numeric(row.get(metric)) for row in rows)
    }


def paired_summary(shape, variants, control, candidate):
    baseline = variants[control]
    selected = variants[candidate]
    return {
        **{key: shape[key] for key in DIMENSIONS},
        "baseline": baseline,
        "candidate": selected,
        "reduction_percent": {
            metric: 100 * (1 - selected[metric] / value)
            for metric, value in baseline.items()
            if metric in selected and value != 0
        },
    }


def run(args):
    plan = json.loads(args.plan.read_text())
    manifest = json.loads(args.variants.read_text())
    variants = manifest["variants"]
    labels = [variant["label"] for variant in variants]
    if len(labels) != len(set(labels)) or len(labels) < 2:
        raise ValueError("variant labels must be unique, with at least two variants")
    if any(not re.fullmatch(r"[a-z0-9_-]+", label) for label in labels):
        raise ValueError("variant labels may contain only lowercase letters, digits, underscores, and hyphens")
    control = manifest["primary_control"]
    if control not in labels:
        raise ValueError("primary_control must name a variant")
    identity_rules = manifest.get("identity_rules", [])
    for rule in identity_rules:
        if rule["source_layout"] not in ("single", "split") or len(rule["variants"]) != 2:
            raise ValueError("identity rules require a Single/Split layout and exactly two variants")
        if any(label not in labels for label in rule["variants"]):
            raise ValueError("identity-rule variant is missing from the manifest")
    shapes = [shape for shape in plan["shapes"] if not args.groups or shape["group"] in args.groups]
    if not shapes:
        raise ValueError("no shapes selected")
    if args.groups and set(args.groups) - {shape["group"] for shape in plan["shapes"]}:
        raise ValueError("unknown plan group")
    identifiers = [tuple(shape[key] for key in DIMENSIONS) for shape in shapes]
    if len(identifiers) != len(set(identifiers)):
        raise ValueError("duplicate shapes in plan")
    expected = sum(len(shape["seeds"]) * (plan["warmup"] + plan["iterations"]) for shape in shapes)
    description = {
        "shapes": len(shapes), "variants": labels, "primary_control": control,
        "input_cases_per_variant": sum(len(shape["seeds"]) for shape in shapes),
        "proofs_per_variant_including_warmup": expected,
        "proofs_all_variants_including_warmup": expected * len(labels),
        "groups": sorted({shape["group"] for shape in shapes}),
    }
    print(json.dumps(description), flush=True)
    if args.describe_plan:
        return 0
    if manifest.get("binary_path_pending_root_confirmation"):
        raise ValueError("variant manifest is pending binary confirmation; freeze the ready manifest before running")
    if args.output is None:
        raise ValueError("--output is required to run a benchmark")
    if args.output.exists() and any(args.output.iterdir()):
        raise ValueError("output directory must be empty; prior evidence is never overwritten")
    args.output.mkdir(parents=True, exist_ok=True)
    prepared_variants = []
    for variant in variants:
        binary = Path(variant["binary"]).resolve()
        if not binary.is_file() or not os.access(binary, os.X_OK):
            raise ValueError(f"missing executable for {variant['label']}: {binary}")
        extra_args = variant.get("args", [])
        forbidden = {"--degree", "--security", "--batch", "--threads", "--seed", "--warmup", "--iterations", "--trace"}
        if any(argument.split("=", 1)[0] in forbidden for argument in extra_args):
            raise ValueError("variant arguments cannot replace fixed input/timing arguments")
        environment = variant.get("environment", {})
        reserved = {"RAYON_NUM_THREADS", "OMP_NUM_THREADS", "BITZ_FALCON_MAIN_CPU", "BITZ_FALCON_WORKER_CPUS", "BITZ_FALCON_CASE_CACHE", "BITZ_FALCON_STAGE_TIMINGS", "BITZ_FALCON_FIXTURE_CACHE", "BITZ_FALCON_FIXTURE_PATH"}
        if set(environment) & reserved:
            raise ValueError("variant environment cannot replace affinity, input, or tracing settings")
        prepared_variants.append({**variant, "binary": str(binary), "sha256": hashlib.sha256(binary.read_bytes()).hexdigest()})
    metadata = {
        "started_unix": time.time(), "finished_unix": None,
        "platform": subprocess.check_output(["uname", "-a"], text=True).strip(),
        "cpu": subprocess.check_output(["lscpu"], text=True),
        "plan": plan, "selected_shapes": shapes,
        "plan_sha256": hashlib.sha256(args.plan.read_bytes()).hexdigest(),
        "variant_manifest_sha256": hashlib.sha256(args.variants.read_bytes()).hexdigest(),
        "variants": prepared_variants, "primary_control": control,
        "same_protocol_identity_rules": identity_rules,
        "trace": args.trace, "description": description,
        "ordering": plan.get("variant_order", "cyclic"),
        "build_requirement": "Frozen native release binaries; --locked --offline --features falcon-hybrid --example falcon_algebraic; RUSTFLAGS=-C target-cpu=native. Record any benchmark-only feature or phase-control differences in the variant manifest.",
        "acceptance": "Report both pooled timing medians and seed-paired uncertainty. An observed positive delta is preserved; repeatability requires separate evidence. Cross-protocol proof identity is not assumed.",
    }
    write_json(args.output / "metadata.json", metadata)
    summaries = []
    seed_summaries = []
    all_samples = []
    verified_proofs = 0
    identity_checks = []
    for shape in shapes:
        degree, security, batch, threads = (shape[key] for key in DIMENSIONS)
        if threads not in (1, 8):
            raise ValueError("CPU pinning is defined only for one or eight threads")
        if len(shape["seeds"]) != len(set(shape["seeds"])):
            raise ValueError("duplicate seeds in a shape")
        cpus = "6" if threads == 1 else "6,7,4,5,2,3,0,1"
        grouped = {label: [] for label in labels}
        for seed_index, seed in enumerate(shape["seeds"]):
            if plan.get("variant_order", "cyclic") == "forward-reverse":
                order = prepared_variants if seed_index % 2 == 0 else list(reversed(prepared_variants))
            else:
                offset = seed_index % len(prepared_variants)
                order = prepared_variants[offset:] + prepared_variants[:offset]
            digests = {}
            seed_grouped = {}
            trial_rows = {}
            for variant in order:
                label = variant["label"]
                stem = f"{label}-n{degree}-s{security}-b{batch}-t{threads}-seed{seed}"
                configured = {
                    "RAYON_NUM_THREADS": str(threads), "OMP_NUM_THREADS": str(threads),
                    "BITZ_FALCON_MAIN_CPU": "6", "BITZ_FALCON_WORKER_CPUS": cpus,
                    "BITZ_FALCON_CASE_CACHE": "/tmp/bitz-arithmetic-witness-cases-20261011",
                }
                environment = os.environ.copy()
                # Do not inherit a phase-control knob from an interactive shell.
                for key in list(environment):
                    if key.startswith(("BITZ_FALCON_", "BITZ_ARITHMETIC_")):
                        environment.pop(key)
                environment.update(configured)
                environment.update(variant.get("environment", {}))
                configured.update(variant.get("environment", {}))
                command = ["taskset", "-c", cpus, variant["binary"], "--degree", str(degree), "--security", str(security), "--batch", str(batch), "--threads", str(threads), "--seed", str(seed), "--warmup", str(plan["warmup"]), "--iterations", str(plan["iterations"])]
                command.extend(variant.get("args", []))
                if args.trace:
                    command.append("--trace")
                    environment["BITZ_FALCON_STAGE_TIMINGS"] = "1"
                    configured["BITZ_FALCON_STAGE_TIMINGS"] = "1"
                write_json(args.output / f"{stem}.command.json", {"argv": command, "environment": configured, "variant": label, "group": shape["group"]})
                print(stem, flush=True)
                with (args.output / f"{stem}.jsonl").open("w") as stdout, (args.output / f"{stem}.stderr.jsonl").open("w") as stderr:
                    subprocess.run(command, env=environment, stdout=stdout, stderr=stderr, check=True)
                rows = [json.loads(line) for line in (args.output / f"{stem}.jsonl").read_text().splitlines()]
                if len(rows) != plan["iterations"] + plan["warmup"]:
                    raise ValueError(f"wrong trial count for {stem}")
                for index, row in enumerate(rows):
                    if not row.get("verified") or any(row.get(key) != shape[key] for key in DIMENSIONS):
                        raise ValueError(f"unverified proof or wrong input shape for {stem}")
                    if row.get("seed") != seed or row.get("auxiliary_pool_threads") != threads:
                        raise ValueError(f"wrong seed or auxiliary worker count for {stem}")
                    if any(row.get(key) != value for key, value in variant.get("expected_fields", {}).items()):
                        raise ValueError(f"variant diagnostics do not match the requested mode for {stem}")
                    if row["input_digest"] != rows[0]["input_digest"]:
                        raise ValueError(f"input changes between trials for {stem}")
                    measured = index >= plan["warmup"]
                    if row["trial"] != ("sample" if measured else "warmup"):
                        raise ValueError(f"wrong trial identity for {stem}")
                    if measured and row["iteration"] != index - plan["warmup"]:
                        raise ValueError(f"wrong measured iteration for {stem}")
                    row["committed_source_bits"] = row.get("committed_source_bits", row.get("source_bits"))
                    if any(not numeric(row.get(metric)) for metric in REQUIRED_METRICS):
                        raise ValueError(f"missing required metric for {stem}")
                verified_proofs += len(rows)
                digests[label] = rows[0]["input_digest"]
                trial_rows[label] = rows
                samples = [dict(row, variant=label, group=shape["group"]) for row in rows if row["trial"] == "sample"]
                grouped[label].extend(samples)
                all_samples.extend(samples)
                seed_grouped[label] = medians(samples)
            if len(set(digests.values())) != 1:
                raise ValueError(f"input mismatch at {shape}, seed {seed}: {digests}")
            matched_identity_rules = 0
            for rule in identity_rules:
                first, second = rule["variants"]
                left, right = trial_rows[first], trial_rows[second]
                if left[0]["committed_source_bits"] < 1 << 19:
                    raise ValueError("identity layout classifier requires the unchanged 2^19-bit PCS floor")
                layout = "single" if left[0]["committed_source_bits"] == 1 << 19 else "split"
                if layout != rule["source_layout"]:
                    continue
                matched_identity_rules += 1
                for before, after in zip(left, right):
                    for key in rule["keys"]:
                        if before[key] != after[key]:
                            raise ValueError(f"same-v8 no-op identity mismatch: {first} vs {second}, {shape}, seed {seed}, key {key}")
                identity_checks.append({**{key: shape[key] for key in DIMENSIONS}, "seed": seed,
                                        "source_layout": layout, "variants": rule["variants"], "proofs": len(left)})
            if identity_rules and matched_identity_rules != 1:
                raise ValueError("each case must match exactly one same-v8 identity rule")
            seed_summaries.append({**{key: shape[key] for key in DIMENSIONS}, "group": shape["group"], "seed": seed, "variants": seed_grouped})
        result = {**{key: shape[key] for key in DIMENSIONS}, "group": shape["group"], "variants": {label: medians(rows) for label, rows in grouped.items()}}
        summaries.append(result)
        comparison_deltas = {label: {metric: 100 * (result["variants"][label][metric] / result["variants"][control][metric] - 1) for metric in PROVING_METRICS} for label in labels if label != control}
        print(json.dumps({"completed_shape": {key: shape[key] for key in DIMENSIONS}, "deltas_percent_vs_control": comparison_deltas}), flush=True)
        # Completed shapes remain readable if a subsequent process fails.
        write_json(args.output / "summary.json", summaries)
        write_json(args.output / "seed-summary.json", seed_summaries)
        write_json(args.output / "same-v8-proof-identity.json", {
            "passed": True, "rules": identity_rules,
            "complete": bool(identity_rules) and sum(check["proofs"] for check in identity_checks) == expected,
            "expected_proofs": expected if identity_rules else 0,
            "checked_proofs": sum(check["proofs"] for check in identity_checks), "checks": identity_checks,
        })
    write_json(args.output / "samples.json", all_samples)
    any_gate_failure = False
    for label in labels:
        if label == control:
            continue
        comparison = args.output / "comparisons" / f"{label}-vs-{control}"
        comparison.mkdir(parents=True)
        paired = [paired_summary(row, row["variants"], control, label) for row in summaries]
        paired_seeds = [{**paired_summary(row, row["variants"], control, label), "seed": row["seed"]} for row in seed_summaries]
        write_json(comparison / "summary.json", paired)
        write_json(comparison / "seed-summary.json", paired_seeds)
        threshold = args.max_prover_regression_percent if args.max_prover_regression_percent is not None else 0
        regressions = [{**{key: row[key] for key in DIMENSIONS}, "metric": metric, "baseline_ms": row["baseline"][metric], "candidate_ms": row["candidate"][metric], "regression_percent": -row["reduction_percent"][metric]} for row in paired for metric in PROVING_METRICS if -row["reduction_percent"][metric] > threshold]
        gate = {"passed": not regressions, "metrics": list(PROVING_METRICS), "maximum_regression_percent": threshold, "regressions": regressions,
                "interpretation": "Observed pooled-sample median gate only. A tiny positive delta does not establish a repeatable regression; inspect seed-paired dispersion and an independently authorized confirmation run. This gate is not a statistical confidence claim."}
        write_json(comparison / "regression-gate.json", gate)
        any_gate_failure |= not gate["passed"]
        print(json.dumps({"comparison": f"{label}-vs-{control}", "observed_gate_passed": gate["passed"], "positive_entries": len(regressions)}), flush=True)
    metadata["finished_unix"] = time.time()
    metadata["verified_proofs"] = verified_proofs
    metadata["verified_measured_proofs"] = len(all_samples)
    metadata["all_input_digests_matched"] = True
    metadata["cross_protocol_proof_identity_required"] = False
    metadata["same_protocol_identity_checked_proofs"] = sum(check["proofs"] for check in identity_checks)
    write_json(args.output / "metadata.json", metadata)
    return 3 if args.max_prover_regression_percent is not None and any_gate_failure else 0


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--plan", type=Path, default=HERE / "fixed-plan.json")
    parser.add_argument("--variants", type=Path, required=True)
    parser.add_argument("--output", type=Path)
    parser.add_argument("--groups", nargs="+", choices=("core-small", "core-large", "edges"))
    parser.add_argument("--describe-plan", action="store_true")
    parser.add_argument("--trace", action="store_true", help="Diagnostic profiles only; do not mix traced timings with acceptance measurements.")
    parser.add_argument("--max-prover-regression-percent", type=float, help="Optionally return exit code 3 if either observed median exceeds this percentage; preserve all results first.")
    sys.exit(run(parser.parse_args()))


if __name__ == "__main__":
    main()

#!/usr/bin/env python3
"""Summarize complete, explicitly selected traced phases; never run a prover.

Example after the final profile phase completes:
  python3 results/falcon-next-20261013/forest-profile.py --phase final-profile \
    --output results/falcon-next-20261013/final-forest-profile.json

The default configurations are baseline and production. This reuses the saved
forest parser's proof-boundary join and the current campaign runner's strict
raw-result validation. It does not inspect or summarize unselected phases.
Supported geometry: canonical Falcon512/1024, batch1024, security128 only.
"""

import argparse
from collections import defaultdict
import importlib.util
import json
from pathlib import Path
import statistics
import sys


HERE = Path(__file__).resolve().parent
ROOT = HERE.parent.parent
RUNNER = ROOT / "scripts/falcon_next_optimization.py"
PARSER = HERE.parent / "falcon-forest-pcs-binding-20261013/analyze-forest.py"


def load_module(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def require(condition, message):
    if not condition:
        raise ValueError(message)


def expected_labels(job, manifest, overrides):
    # Deliberately support only the final campaign's known geometry. Layout has
    # row_vars=13; the 128-bit two-limb bridge creates t=14, depth=13. Both full-
    # batch degrees cross NIBBLE_FROM=23 and construct nibble rows in the forest.
    # The ordinary constructor enables one-pass rounds at L1/L0. Derive nothing
    # from labels present in the log, since a missing label must not imply zero.
    require(job.get("degree") in (512, 1024) and job.get("protocol") == "canonical"
            and manifest.get("batch") == 1024 and manifest.get("security") == 128,
            "unsupported forest geometry: require canonical Falcon512/1024, batch1024, security128")
    retained = overrides.get("BITZ_FOREST_L4", "rebuild")
    require(retained in ("rebuild", "retain"), "unsupported L4 policy")
    labels = {"nibble rows", "L3 value tables", "levels 5..13 build", "levels ≥4",
              "L4 retained" if retained == "retain" else "L4 rebuild",
              "L2 bit round 1", "L1 one-pass bit rounds", "L0 one-pass bit rounds"}
    labels.update(f"level {level}" for level in range(13))
    labels.update(f"L{level} {stage}" for level in range(4)
                  for stage in ("tables", "jit round", "jit fold", "dense tail"))
    return labels


def parse_complete_trace(text, job, trials, manifest, overrides, parser):
    expected = expected_labels(job, manifest, overrides)
    # Reuse the checked block/root join, duplicate-label rejection and warmup
    # selection. Then validate ALL blocks, including excluded warmups, before
    # allowing any zero-filled derived categories out of the reused parser.
    samples = parser.parse_trace(text, job, trials)
    labels = set()
    block_index = 0
    for line in text.splitlines():
        match = parser.TRACE.match(line)
        if match:
            labels.add(match.group(1))
            continue
        try:
            event = json.loads(line)
        except ValueError:
            continue
        if isinstance(event, dict) and event.get("event") == "stage" and event.get("name") == parser.FOREST:
            missing, unexpected = expected - labels, labels - expected
            require(not missing and not unexpected,
                    f"forest block {block_index} label mismatch: missing={sorted(missing)}, "
                    f"unexpected={sorted(unexpected)}")
            block_index += 1
            labels = set()
    require(block_index == job["warmup"] + job["iterations"] and not labels,
            "forest label validation did not cover every complete proof")
    return samples


def summarize_samples(samples):
    names = sorted(samples[0]["components_ms"])
    return dict(
        forest_wall_ms=statistics.median(sample["forest_wall_ms"] for sample in samples),
        components_ms={name: statistics.median(sample["components_ms"][name] for sample in samples)
                       for name in names},
    )


def analyze(campaign, phases, configs):
    # Imports read source only. Neither module executes its guarded main().
    sys.path.insert(0, str(ROOT / "scripts"))
    runner = load_module("falcon_next_forest_runner", RUNNER)
    parser = load_module("saved_forest_profile_parser", PARSER)
    provenance = runner.parser_provenance()
    groups, binary_hashes, identities, baseline_cases = defaultdict(list), {}, {}, set()
    input_files, expected_results = {}, set()
    for phase in phases:
        require(runner.safe_name(phase), f"invalid phase: {phase}")
        manifest_path = campaign / "phases" / f"{phase}.json"
        manifest = json.loads(manifest_path.read_text())
        require(manifest.get("schema") == runner.SCHEMA and manifest.get("phase") == phase,
                f"wrong phase schema/name: {manifest_path}")
        require(manifest.get("trace") is True, f"not a traced phase: {phase}")
        require(all(manifest.get(key) == value for key, value in provenance.items()),
                f"runner/helper provenance changed: {phase}")
        require(set(configs) <= {job["config"] for job in manifest["jobs"]},
                f"requested configuration missing from phase: {phase}")
        manifest_hash = runner.file_hash(manifest_path)
        input_files[str(manifest_path)] = manifest_hash
        for binary in manifest["binaries"].values():
            if binary["path"] not in binary_hashes:
                binary_hashes[binary["path"]] = runner.file_hash(Path(binary["path"]))
            require(binary_hashes[binary["path"]] == binary["sha256"],
                    f"frozen binary changed: {binary['path']}")
        for job in manifest["jobs"]:
            require(runner.safe_name(job["id"]), f"invalid job id: {job['id']}")
            path = campaign / "runs" / phase / job["id"] / "result.json"
            require(path not in expected_results, f"duplicate job: {path}")
            expected_results.add(path)
            require(path.is_file(), f"selected phase incomplete: {path}")
            result = runner.validate_result(path, job, manifest, manifest_hash)
            if job["config"] not in configs:
                continue
            input_files[str(path)] = runner.file_hash(path)
            for trial in result["trials"]:
                key = (job["degree"], manifest["batch"], manifest["security"], job["seed"],
                       trial["protocol_id"])
                identity = tuple(trial[name] for name in
                                 ["input_digest", "source_root", "proof_debug_digest", "proof_payload_bytes"])
                require(identities.setdefault(key, identity) == identity,
                        f"source/proof identity mismatch: {path}")
                if job["binary"] == "baseline":
                    baseline_cases.add(key)
            command = runner.command_record(job, manifest)
            require(command["overrides"].get("BITZ_TRACE") == "1" and "--trace" in command["command"],
                    f"missing plaintext/JSON diagnostic flags: {path}")
            samples = parse_complete_trace((path.parent / "stderr.log").read_text(), job,
                                           result["trials"], manifest, command["overrides"], parser)
            metadata = dict(phase=phase, config=job["config"], degree=job["degree"],
                            threads=job["threads"], batch=manifest["batch"], security=manifest["security"],
                            mode=job["mode"], binary=job["binary"], binary_sha256=command["binary_sha256"],
                            worker_cpus=job["worker_cpus"], main_cpu=job["main_cpu"],
                            overrides=command["overrides"])
            for sample in samples:
                labels = sample["trace_ms"]
                walls = [value for name, value in sample["stage_ms"].items()
                         if name.split("/")[-1] == parser.FOREST]
                require(len(walls) == 1, f"expected one forest wall span: {path}")
                # These categories are disjoint. Parent 'levels ≥4' and all
                # level0..4 parents are not added to their internal timings.
                components = dict(sample["derived_ms"], nibble_rows_ms=labels["nibble rows"])
                groups[json.dumps(metadata, sort_keys=True)].append(dict(
                    seed=job["seed"], job_id=job["id"], iteration=sample["iteration"],
                    forest_wall_ms=walls[0], components_ms=components,
                    raw_labels_ms=labels, result=str(path), excluded_job_warmups=job["warmup"],
                ))
        actual = set((campaign / "runs" / phase).glob("*/result.json"))
        require(actual == {path for path in expected_results if path.parent.parent.name == phase},
                f"unexpected result outside frozen phase jobs: {phase}")
    require(groups, "no selected diagnostic samples")
    require(identities.keys() <= baseline_cases, "selected samples have cases without a frozen baseline")
    rows = []
    for key, samples in sorted(groups.items()):
        jobs = {sample["job_id"]: sample["excluded_job_warmups"] for sample in samples}
        rows.append(dict(json.loads(key), samples=len(samples), processes=len(jobs),
                         excluded_warmups=sum(jobs.values()), medians=summarize_samples(samples),
                         seed_medians={str(seed): summarize_samples([s for s in samples if s["seed"] == seed])
                                       for seed in sorted({s["seed"] for s in samples})}, trials=samples))
    return dict(schema="bitz/falcon-next-forest-profile/v2", rows=rows, input_hashes=input_files,
                binary_hashes=binary_hashes, parser_hashes={str(path): runner.file_hash(path)
                                                          for path in [Path(__file__), PARSER, RUNNER]},
                notes=[
                    "Diagnostic traces only; these are not untraced latency measurements.",
                    "Complete level0 blocks join the following forest span and its prover root; warmups and verifier roots are excluded.",
                    "Every block, including warmups, must contain exactly the required labels for canonical Falcon512/1024, batch1024, security128; other geometries fail.",
                    "Component families exclude overlapping parents; forest_wall_ms is separate and is never added to components.",
                    "l4_remaining_ms subtracts rebuild/retained from the level4 parent; printed rounding may make it slightly negative.",
                    "Trace durations are rounded, and categories omit untimed overhead. No residual or aggregate total is inferred.",
                    "Fault-MB text is ignored; its fixed page-size conversion is not used as memory evidence.",
                    "Selected phases must be complete; unselected phases are not read or certified. Normalized raw results and frozen hashes are checked.",
                    "Parser assumes one noninterleaved forest block per Falcon proof, as emitted by the sequential benchmark; ambiguous or incomplete blocks fail.",
                ])


def main():
    cli = argparse.ArgumentParser(description=__doc__)
    cli.add_argument("--input", type=Path, default=HERE)
    cli.add_argument("--phase", action="append", required=True)
    cli.add_argument("--config", action="append", help="repeatable; defaults to baseline and production")
    cli.add_argument("--output", type=Path, required=True)
    args = cli.parse_args()
    require(len(set(args.phase)) == len(args.phase), "duplicate selected phase")
    configs = args.config or ["baseline", "production"]
    require(len(set(configs)) == len(configs), "duplicate selected configuration")
    result = analyze(args.input.resolve(), args.phase, configs)
    args.output.parent.mkdir(parents=True, exist_ok=True)
    with args.output.open("x") as output:
        json.dump(result, output, indent=2, sort_keys=True)
        output.write("\n")
    print(f"Wrote {len(result['rows'])} diagnostic groups to {args.output}")


if __name__ == "__main__":
    main()

#!/usr/bin/env python3
"""Summarize traced forest diagnostics; accepts a campaign or baseline-profiles root."""

import argparse
from collections import defaultdict
import hashlib
import json
import math
from pathlib import Path
import re
import statistics


TRACE = re.compile(r"^bitz:\s*(.*?)\s+(\d+(?:\.\d+)?)(ns|µs|μs|us|ms|s)\s+")
UNITS = {"ns": 1e-6, "µs": 1e-3, "μs": 1e-3, "us": 1e-3, "ms": 1, "s": 1000}
PROVE = "falcon_algebraic:prove"
FOREST = "falcon_bridge:wfbitz_forest"


def require(condition, message):
    if not condition:
        raise ValueError(message)


def elapsed(value):
    require(type(value) in (int, float) and math.isfinite(value) and value >= 0,
            f"invalid duration: {value!r}")
    return value


def parse_trace(text, job, trials):
    """Join each complete text block to the following forest span and its proof root."""
    warmup, iterations = job["warmup"], job["iterations"]
    require(type(warmup) is int and warmup >= 0 and type(iterations) is int and iterations > 0,
            "invalid warmup/iteration counts")
    expected = warmup + iterations
    require([trial["trial"] for trial in trials] == ["warmup"] * warmup + ["sample"] * iterations,
            "trial labels/counts disagree with job warmup/iterations")
    events, roots, forests = [], [], {}
    block, complete = {}, False
    for line_number, line in enumerate(text.splitlines(), 1):
        match = TRACE.match(line)
        if match:
            require(not complete, f"forest span missing before trace line {line_number}")
            label, number, unit = match.groups()
            require(label not in block, f"duplicate forest label at line {line_number}: {label}")
            block[label] = elapsed(float(number) * UNITS[unit])
            complete = label == "level 0"
            continue
        require(not line.startswith("bitz:"), f"unrecognized forest trace at line {line_number}")
        try:
            event = json.loads(line)
        except ValueError:
            continue
        if not isinstance(event, dict) or event.get("event") != "stage":
            continue
        elapsed(event["elapsed_ms"])
        events.append(event)
        if event["name"] == FOREST:
            require(complete and block, f"incomplete forest before span at line {line_number}")
            require(event["path"][0] == PROVE and event["ancestor_ids"], "forest outside prover root")
            root = event["ancestor_ids"][0]
            require(root not in forests, "multiple forest blocks for one proof")
            forests[root] = block
            block, complete = {}, False
        elif event["name"] == PROVE:
            require(event["span_id"] in forests, "prover root has no preceding forest block")
            require(event["span_id"] not in roots, "duplicate prover root")
            roots.append(event["span_id"])
    require(not block and not complete, "unfinished forest block at end of log")
    require(len(roots) == expected and set(roots) == set(forests),
            f"proof/forest count mismatch: expected {expected}, roots {len(roots)}, forests {len(forests)}")
    samples = []
    for index, root in enumerate(roots[warmup:]):
        stages, fields = defaultdict(float), defaultdict(float)
        for event in events:
            if event["span_id"] != root and root not in event["ancestor_ids"]:
                continue
            path = "/".join(event["path"])
            stages[path] += event["elapsed_ms"]
            for name, value in event.get("fields", {}).items():
                if name.endswith("_ns"):
                    fields[f"{path}/{name}"] += elapsed(value) / 1e6
        trace = forests[root]
        def matching(pattern):
            return sum(value for label, value in trace.items() if re.fullmatch(pattern, label))
        derived = {
            "jit_rounds_ms": matching(r"L\d+ jit round"),
            "jit_folds_ms": matching(r"L\d+ jit fold"),
            "jit_dense_tails_ms": matching(r"L\d+ dense tail"),
            "bit_rounds_ms": sum(value for label, value in trace.items() if "bit round" in label),
            "value_tables_ms": matching(r"L\d+ (?:value )?tables"),
            "upper_build_ms": matching(r"levels \d+\.\.\d+ build"),
            "upper_levels_ms": sum(value for label, value in trace.items()
                                   if re.fullmatch(r"level \d+", label) and int(label.split()[1]) >= 5),
            "l4_rebuild_ms": trace.get("L4 rebuild", 0),
            "l4_retained_ms": trace.get("L4 retained", 0),
            "l4_remaining_ms": trace.get("level 4", 0) - trace.get("L4 rebuild", 0) - trace.get("L4 retained", 0),
        }
        samples.append(dict(seed=job["seed"], job_id=job["id"], iteration=index,
                            root_span_id=root, derived_ms=derived, trace_ms=trace,
                            stage_ms=dict(stages), fields_ms=dict(fields)))
    return samples


def read_json(path):
    return json.loads(path.read_text())


def sha256(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def flag(command, name):
    require(name in command, f"command missing {name}")
    return int(command[command.index(name) + 1])


def load_job(path, manifest=None):
    result = read_json(path)
    job = result["job"]
    directory = path.parent
    command = read_json(directory / "command.json")
    phase = manifest["phase"] if manifest else "baseline-profiles"
    trace = manifest["trace"] if manifest else "--trace" in command["command"]
    if not trace:
        return None, dict(phase=phase, job_id=job["id"], reason="untraced latency job")
    require(result.get("verified") is True, f"unverified result: {path}")
    require("--trace" in command["command"] and command["overrides"].get("BITZ_TRACE") == "1",
            f"job does not enable both JSON stages and BITZ_TRACE: {path}")
    if manifest:
        require(job in manifest["jobs"], f"job missing from phase manifest: {path}")
    for name, digest_key in [("stderr.log", "stderr_sha256"), ("command.json", "command_sha256")]:
        digest = result.get(digest_key, result.get("hashes", {}).get(name))
        if digest:
            require(sha256(directory / name) == digest, f"{name} hash mismatch: {path}")
    for field in ["degree", "threads", "seed", "warmup", "iterations"]:
        require(job[field] == flag(command["command"], f"--{field}"), f"command/job {field} mismatch: {path}")
    metadata = dict(phase=phase, config=job["config"], variant=job["variant"], degree=job["degree"],
                    threads=job["threads"], batch=flag(command["command"], "--batch"),
                    security=flag(command["command"], "--security"), mode=job["mode"], trace=True,
                    binary=job.get("binary", "baseline"), binary_sha256=command["binary_sha256"],
                    overrides=command["overrides"])
    samples = parse_trace((directory / "stderr.log").read_text(), job, result["trials"])
    return dict(metadata=metadata, samples=samples, warmup=job["warmup"], result=str(path)), None


def medians(samples):
    output = {}
    for category in ["derived_ms", "trace_ms", "stage_ms", "fields_ms"]:
        keys = sorted(set().union(*(sample[category] for sample in samples)))
        output[category] = {key: statistics.median(sample[category].get(key, 0) for sample in samples)
                            for key in keys}
    return output


def analyze(root, phases=()):
    if (root / "runs").is_dir():
        paths = sorted((root / "runs").glob("*/*/result.json"))
        jobs = [(path, read_json(root / "phases" / f"{path.parent.parent.name}.json"))
                for path in paths if not phases or path.parent.parent.name in phases]
    else:
        require(not phases or "baseline-profiles" in phases, "phase filter excludes baseline-profiles")
        paths = [root / "result.json"] if (root / "result.json").is_file() else sorted(root.glob("*/result.json"))
        jobs = [(path, None) for path in paths]
    require(jobs, f"no selected result.json files beneath {root}")
    groups, skipped = defaultdict(list), []
    for path, manifest in jobs:
        try:
            item, skip = load_job(path, manifest)
        except (KeyError, ValueError) as error:
            raise ValueError(f"{path}: {error}") from error
        if skip:
            skipped.append(skip)
        else:
            groups[json.dumps(item["metadata"], sort_keys=True)].append(item)
    rows = []
    for key, items in sorted(groups.items()):
        samples = [sample for item in items for sample in item["samples"]]
        rows.append(dict(json.loads(key), processes=len(items), samples=len(samples),
                         excluded_warmups=sum(item["warmup"] for item in items),
                         medians=medians(samples), seed_medians={
                             str(seed): medians([sample for sample in samples if sample["seed"] == seed])
                             for seed in sorted({sample["seed"] for sample in samples})},
                         results=[item["result"] for item in items], trials=samples))
    return dict(schema="bitz/forest-trace-analysis/v1", rows=rows, skipped=skipped, notes=[
        "Trace diagnostics only: never compare these timings with untraced latency results.",
        "Forest blocks end at level 0, join the following forest span, and align to the enclosing prover root.",
        "Warmups are excluded using job counts and checked against result trial labels and command flags.",
        "Medians pool measured proofs; within-process samples share Prepared and are correlated.",
        "Nested wall spans overlap. fields_ms ending in worker_ns are summed worker time, not wall time.",
        "BITZ_TRACE durations are rounded; >=1ms prints with 0.1ms precision. Fault-MB text is ignored.",
        "Derived categories absent from a valid complete forest have zero cost; missing raw labels use zero when pooling.",
        "L4 remaining subtracts rebuild/retained from level 4; independent rounding may make it slightly negative.",
        "Each phase/configuration/geometry/mode/binary hash/environment is grouped separately; no significance claim.",
    ])


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--input", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--phase", action="append", default=[], help="optional repeatable campaign phase filter")
    args = parser.parse_args()
    result = analyze(args.input.resolve(), args.phase)
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(result, indent=2, sort_keys=True) + "\n")
    print(f"{len(result['rows'])} diagnostic groups; "
          f"{sum(row['samples'] for row in result['rows'])} measured proofs; "
          f"{len(result['skipped'])} untraced jobs skipped; wrote {args.output}")


if __name__ == "__main__":
    main()

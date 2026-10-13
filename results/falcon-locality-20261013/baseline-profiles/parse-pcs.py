#!/usr/bin/env python3
"""Pair textual Ligerito timers with closing prover spans, then drop warmups."""
from collections import defaultdict
import json
import math
from pathlib import Path
import re
import statistics
import sys

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT / "scripts"))
from bench_support import file_hash, write_json
from binder_experiments import read_jsonl
from falcon_buffer_optimization import normalize

OUT = Path(__file__).resolve().parent
LABELS = {
    "initial sumcheck (initial_k folds + SC build)": "initial_sumcheck_ms",
    "recursive commits (NTT + merkle)": "recursive_commits_ms",
    "opens (rows + multi-proof)": "opens_ms",
    "induce_sumcheck_poly": "induce_ms",
    "sumcheck recursive folds": "recursive_folds_ms",
    "introduce_new + glue": "introduce_glue_ms",
}
TOTAL = re.compile(r"^\[lig-prove\] total = ([0-9]+(?:\.[0-9]+)?) ms$")
DURATION = re.compile(r"^  (.+?):\s+([0-9]+(?:\.[0-9]+)?) ms$")


def parse(path, trials):
    stages, blocks, pending = [], [], None
    for line_no, line in enumerate(path.read_text().splitlines(), 1):
        total = TOTAL.fullmatch(line)
        if total:
            if pending is not None:
                raise ValueError("unclosed Ligerito timer block")
            pending = dict(text_line=line_no, timings=dict(total_ms=float(total[1])))
            continue
        if pending is not None and (match := DURATION.fullmatch(line)):
            label, duration = match.groups()
            if re.fullmatch(r"OOD samples \([0-9]+\): MLE evals \+ glue", label):
                key = "ood_ms"
            elif label in LABELS:
                key = LABELS[label]
            else:
                raise ValueError(f"unknown Ligerito timer label: {label}")
            if key in pending["timings"]:
                raise ValueError(f"duplicate timer: {key}")
            pending["timings"][key] = float(duration)
            continue
        try:
            event = json.loads(line)
        except ValueError:
            continue
        if not isinstance(event, dict) or event.get("event") != "stage":
            continue
        event = dict(event, text_line=line_no)
        stages.append(event)
        if event["name"] == "op:ligerito" and "falcon_algebraic:prove" in event["path"]:
            if pending is None or not set(LABELS.values()).issubset(pending["timings"]):
                raise ValueError("missing or incomplete Ligerito timer block")
            root = event["ancestor_ids"][event["path"].index("falcon_algebraic:prove")]
            if pending["timings"]["total_ms"] > event["elapsed_ms"] + 0.005:
                raise ValueError("textual total exceeds enclosing Ligerito span")
            pending.update(root_span_id=root, ligerito_span_id=event["span_id"], close_line=line_no)
            blocks.append(pending)
            pending = None
    if pending is not None:
        raise ValueError("unfinished timer block at EOF")
    roots = [e for e in stages if e["name"] == "falcon_algebraic:prove"]
    if len(blocks) != len(trials) or len(roots) != len(trials):
        raise ValueError("timer/prover/trial count mismatch")
    output = []
    for index, (root, block, trial) in enumerate(zip(roots, blocks, trials)):
        if block["root_span_id"] != root["span_id"] or block["close_line"] >= root["text_line"]:
            raise ValueError("timer/prover order mismatch")
        child = [e for e in stages if root["span_id"] in e["ancestor_ids"]]
        wall = defaultdict(float)
        for e in child:
            elapsed = e["elapsed_ms"]
            if not isinstance(elapsed, (int, float)) or not math.isfinite(elapsed) or elapsed < 0:
                raise ValueError("invalid wall timer")
            wall[e["name"]] += elapsed
        timings = block["timings"]
        timings["unattributed_ms"] = timings["total_ms"] - sum(
            v for k, v in timings.items() if k != "total_ms")
        output.append(dict(proof_index=index, trial=trial["trial"], iteration=trial["iteration"],
            root_span_id=root["span_id"], ligerito_span_id=block["ligerito_span_id"],
            text_lines=[block["text_line"], block["close_line"]], timings_ms=timings,
            wall_ms=dict(wall), proof_debug_digest=trial["proof_debug_digest"],
            source_root=trial["source_root"], proof_payload_bytes=trial["proof_payload_bytes"]))
    return output


def main():
    settings = json.loads((OUT / "diagnostic-settings.json").read_text())["settings"]
    grouped = defaultdict(list)
    for result_path in sorted(OUT.glob("baseline-n*/result.json")):
        result = json.loads(result_path.read_text())
        directory = result_path.parent
        for name, expected in result["hashes"].items():
            if file_hash(directory / name) != expected:
                raise ValueError(f"changed raw artifact: {directory / name}")
        trials = normalize(read_jsonl(directory / "stdout.jsonl"), result["job"], settings)
        if trials != result["trials"]:
            raise ValueError("normalization differs from saved trials")
        rows = parse(directory / "stderr.log", trials)
        write_json(directory / "pcs-timings.json", dict(parser_sha256=file_hash(Path(__file__)),
            raw_stderr_sha256=file_hash(directory / "stderr.log"), proofs=rows))
        for row in rows:
            if row["trial"] == "sample":
                grouped[result["job"]["degree"]].append(dict(seed=result["job"]["seed"], **row))
    summaries = []
    for degree, rows in sorted(grouped.items()):
        if len(rows) != 9 or set(row["seed"] for row in rows) != {42, 43, 44}:
            raise ValueError("incomplete measured profile set")
        keys = set.intersection(*(set(row["timings_ms"]) for row in rows))
        wall_keys = set.intersection(*(set(row["wall_ms"]) for row in rows))
        medians = {key: statistics.median(row["timings_ms"][key] for row in rows) for key in sorted(keys)}
        per_seed = {str(seed): {key: statistics.median(row["timings_ms"][key] for row in rows
                    if row["seed"] == seed) for key in sorted(keys)} for seed in [42, 43, 44]}
        summaries.append(dict(degree=degree, workers=12, measured_proofs=9,
            medians_ms=medians, seed_medians_ms=per_seed,
            wall_medians_ms={key: statistics.median(row["wall_ms"][key] for row in rows)
                             for key in sorted(wall_keys)}))
    if len(summaries) != 2:
        raise ValueError("both Falcon degrees are required")
    write_json(OUT / "pcs-summary.json", dict(rows=summaries, parser_sha256=file_hash(Path(__file__)),
        notes=["Warmups excluded by matching each text block to its closing op:ligerito and prover span ID, then normalized trial order.",
               "Text durations have 0.01 ms precision. Unattributed time is a per-proof residual of those rounded values.",
               "Initial/recursive sumcheck timers include their challenge grinding; deferred_first_fold is nested within initial sumcheck.",
               "Tracing is diagnostic only. No latency qualification or before/after speedup is inferred."]))
    print(json.dumps(summaries, indent=2))


if __name__ == "__main__":
    main()

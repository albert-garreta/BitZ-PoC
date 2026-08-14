#!/usr/bin/env python3
"""Validate F2Z ``zkperf.trace/v1`` JSONL and export dashboard interval data.

The input is one canonical trace file per benchmark trial.  A strict export
expects ``n20`` through ``n30``, one warmup and 21 measured trials per size.
The output deliberately contains real interval geometry from the measured
trial nearest the Type-7 median; medians are used only for labels and
distributions, never to manufacture a synthetic timeline.
"""

from __future__ import annotations

import argparse
import json
import math
import os
import re
import sys
import tempfile
from collections import defaultdict
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Iterable


TRACE_SCHEMA = "zkperf.trace/v1"
EXPECTED_EXPONENTS = list(range(20, 31))
EXPECTED_SAMPLE_INDICES = list(range(21))
EXPECTED_ARTIFACTS = (
    "folds",
    "merged_gkr",
    "presum",
    "ring",
    "ligerito",
    "framing",
)
ROW_ORDER = (
    ("proving", "Proving"),
    ("verification", "Verifier time"),
    ("pcs", "PCS"),
    ("commit", "Commit"),
    ("opening-proof", "Opening proof"),
    ("constraint-proof", "Constraint proof"),
    ("sumcheck", "Sumcheck"),
    ("fri", "FRI / proximity"),
    ("serialization", "Serialization"),
    ("preparation", "Preparation"),
)


class ExportError(RuntimeError):
    pass


@dataclass
class TraceRun:
    path: Path
    run: dict[str, Any]
    spans: dict[str, dict[str, Any]]
    root_id: str
    kind: str
    index: int
    exponent_hint: int | None
    stable: dict[str, str]
    depth: dict[str, int]
    occurrence: dict[str, int | None]


# Human-readable fallbacks for the in-crate scopes.  Rich upstream
# ``perf_span!`` metadata takes precedence whenever it is present.
SCOPE_CATALOG: dict[str, dict[str, Any]] = {
    "bench:trial": dict(name="Complete PCS trial", short="Trial", tags=["end-to-end"], group="end-to-end", kind="scope"),
    "bench:commit": dict(name="Commit to the packed witness", short="Commit", tags=["commit", "pcs"], group="commit", kind="phase"),
    "bench:prove": dict(name="Prove the PCS opening", short="Prove", tags=["proving", "pcs", "opening-proof"], group="proving", kind="phase"),
    "bench:serialize": dict(name="Serialize the proof", short="Serialize", tags=["serialization"], group="serialization", kind="phase"),
    "bench:artifacts": dict(name="Account for proof artifacts", short="Artifacts", tags=["serialization"], group="serialization", kind="procedure"),
    "bench:verify": dict(name="Verify the PCS opening", short="Verify", tags=["verification", "pcs", "opening-proof"], group="verification", kind="phase"),
    "mc:pack": dict(name="Pack committed values", short="Pack", tags=["preparation", "proving"], group="preparation", kind="phase"),
    "mc:pow2": dict(name="Build powers-of-two claims", short="Powers of two", tags=["preparation", "proving", "constraint-proof"], group="preparation", kind="phase"),
    "mc:forest": dict(name="Prove the merged bit forest", short="Forest", tags=["proving", "constraint-proof", "sumcheck"], group="constraint-proof", kind="phase"),
    "mc:fold_v": dict(name="Fold the verifier vector", short="Fold v", tags=["preparation", "proving", "constraint-proof"], group="preparation", kind="phase"),
    "mc:presum_tbls": dict(name="Build pre-sumcheck tables", short="Pre-SC tables", tags=["preparation", "proving", "sumcheck"], group="preparation", kind="phase"),
    "mc:presum_run": dict(name="Run the pre-sumcheck", short="Pre-sumcheck", tags=["proving", "constraint-proof", "sumcheck"], group="sumcheck", kind="phase"),
    "mq:rings": dict(name="Ring-switching frontend", short="Ring switch", tags=["preparation", "proving", "pcs", "opening-proof"], group="opening-proof", kind="phase"),
    "mq:bcomb": dict(name="Combine opening claims", short="Batch claims", tags=["preparation", "proving", "pcs", "opening-proof"], group="opening-proof", kind="phase"),
    "mq:lig": dict(name="Recursive Ligerito opening", short="Ligerito", tags=["proving", "pcs", "opening-proof", "fri"], group="fri", kind="phase"),
    "mf:build_levels": dict(name="Build merged-forest levels", short="Build levels", tags=["preparation", "proving", "constraint-proof"], group="preparation", kind="procedure"),
    "mf:phaseA": dict(name="Merged-forest phase A", short="Forest A", tags=["proving", "constraint-proof", "sumcheck"], group="constraint-proof", kind="phase"),
    "mf:bitgen": dict(name="Generate bit-decomposition tables", short="Bit tables", tags=["preparation", "proving", "constraint-proof"], group="preparation", kind="procedure"),
    "mf:phaseB": dict(name="Merged-forest phase B", short="Forest B", tags=["proving", "constraint-proof", "sumcheck"], group="constraint-proof", kind="phase"),
    "mds:round1": dict(name="Pre-sumcheck first round", short="Round 1", tags=["proving", "constraint-proof", "sumcheck"], group="sumcheck", kind="procedure"),
    "mds:rounds": dict(name="Pre-sumcheck remaining rounds", short="Rounds 2+", tags=["proving", "constraint-proof", "sumcheck"], group="sumcheck", kind="procedure"),
    "eqf:suffix": dict(name="Build equality suffix", short="Eq suffix", tags=["preparation", "proving", "constraint-proof", "sumcheck"], group="preparation", kind="procedure"),
    "eqf:rounds": dict(name="Factored-equality sumcheck rounds", short="Eq rounds", tags=["proving", "constraint-proof", "sumcheck"], group="sumcheck", kind="phase"),
    "v:total": dict(name="Verify the proof", short="Verify", tags=["verification"], group="verification", kind="phase"),
    "v:setup": dict(name="Verifier setup", short="Setup", tags=["preparation", "verification"], group="verification", kind="procedure"),
    "v:forest_roots": dict(name="Verify forest roots", short="Forest roots", tags=["constraint-proof", "verification"], group="verification", kind="procedure"),
    "v:merged_gkr": dict(name="Verify merged GKR proof", short="Merged GKR", tags=["constraint-proof", "sumcheck", "verification"], group="verification", kind="procedure"),
    "v:presum": dict(name="Verify pre-sumcheck", short="Pre-sumcheck", tags=["constraint-proof", "sumcheck", "verification"], group="verification", kind="procedure"),
    "v:ring_switch": dict(name="Verify ring switching", short="Ring switch", tags=["pcs", "opening-proof", "verification"], group="verification", kind="procedure"),
    "v:ligerito": dict(name="Verify recursive Ligerito opening", short="Ligerito", tags=["pcs", "opening-proof", "fri", "verification"], group="verification", kind="procedure"),
    "v:read_off": dict(name="Read verifier outputs", short="Read outputs", tags=["verification"], group="verification", kind="procedure"),
}

# Canonical producers normalize scope identifiers to lower-case dotted wire
# operations. Index the source-label catalog through that same identity so
# mixed-case legacy labels such as `mf:phaseA` retain their semantics.
NORMALIZED_SCOPE_CATALOG = {
    key.lower().replace(":", "."): value for key, value in SCOPE_CATALOG.items()
}


def type7(values: list[float], probability: float) -> float:
    """Hyndman-Fan Type 7 (R/NumPy default linear sample quantile)."""
    if not values:
        raise ExportError("cannot summarize an empty sample")
    ordered = sorted(values)
    position = (len(ordered) - 1) * probability
    lo = math.floor(position)
    hi = math.ceil(position)
    fraction = position - lo
    return ordered[lo] + fraction * (ordered[hi] - ordered[lo])


def ms(ns: float) -> float:
    return round(ns / 1_000_000.0, 6)


def ms_stat(values_ns: list[float]) -> dict[str, Any]:
    dec = [ms(type7(values_ns, p / 100.0)) for p in range(10, 100, 10)]
    return {"med": dec[4], "dec": dec, "n": len(values_ns)}


def byte_stat(values: list[int]) -> dict[str, Any]:
    quantiles = [type7([float(v) for v in values], p / 100.0) for p in range(10, 100, 10)]
    clean = [int(q) if q.is_integer() else round(q, 3) for q in quantiles]
    return {"med": clean[4], "dec": clean, "n": len(values)}


def as_nonnegative_int(value: Any, context: str) -> int:
    if isinstance(value, bool):
        raise ExportError(f"{context}: boolean is not an integer")
    try:
        parsed = int(value)
    except (TypeError, ValueError) as error:
        raise ExportError(f"{context}: invalid integer {value!r}") from error
    if parsed < 0:
        raise ExportError(f"{context}: negative integer {parsed}")
    return parsed


def read_jsonl(path: Path) -> tuple[dict[str, Any], dict[str, dict[str, Any]]]:
    run: dict[str, Any] | None = None
    spans: dict[str, dict[str, Any]] = {}
    try:
        lines = path.read_text(encoding="utf-8").splitlines()
    except OSError as error:
        raise ExportError(f"{path}: {error}") from error
    if not lines:
        raise ExportError(f"{path}: empty trace")
    for line_no, line in enumerate(lines, 1):
        if not line.strip():
            continue
        try:
            record = json.loads(line)
        except json.JSONDecodeError as error:
            raise ExportError(f"{path}:{line_no}: malformed JSON: {error}") from error
        if not isinstance(record, dict) or record.get("schema") != TRACE_SCHEMA:
            raise ExportError(f"{path}:{line_no}: expected schema {TRACE_SCHEMA!r}")
        kind = record.get("record")
        if kind == "run":
            if run is not None:
                raise ExportError(f"{path}:{line_no}: duplicate run record")
            run = record
        elif kind == "span":
            span_id = record.get("span_id")
            if not isinstance(span_id, str) or not span_id:
                raise ExportError(f"{path}:{line_no}: span_id must be a nonempty string")
            if span_id in spans:
                raise ExportError(f"{path}:{line_no}: duplicate span_id {span_id!r}")
            spans[span_id] = record
        else:
            raise ExportError(f"{path}:{line_no}: unsupported record {kind!r}")
    if run is None:
        raise ExportError(f"{path}: missing run record")
    if run.get("status") != "ok" or run.get("trace_complete") is not True:
        raise ExportError(f"{path}: run is not a complete successful trace")
    run_id = run.get("run_id")
    if not isinstance(run_id, str) or not run_id:
        raise ExportError(f"{path}: invalid run_id")
    for span in spans.values():
        if span.get("run_id") != run_id:
            raise ExportError(f"{path}: span/run run_id mismatch")
    return run, spans


def trial_identity(run: dict[str, Any], path: Path) -> tuple[str, int]:
    trial = run.get("trial")
    if not isinstance(trial, dict) or trial.get("kind") not in {"warmup", "sample"}:
        raise ExportError(f"{path}: trial must declare kind warmup or sample")
    kind = trial["kind"]
    key = "warmup_index" if kind == "warmup" else "sample_index"
    if key not in trial:
        raise ExportError(f"{path}: trial missing {key}")
    return kind, as_nonnegative_int(trial[key], f"{path}: {key}")


def exponent_from_path(path: Path) -> int | None:
    for part in reversed(path.parts):
        match = re.fullmatch(r"n(\d+)", part)
        if match:
            return int(match.group(1))
    match = re.search(r"(?:^|[-_])n(\d+)(?:[-_.]|$)", path.name)
    return int(match.group(1)) if match else None


def nested_values(value: Any, key: str) -> list[Any]:
    found: list[Any] = []
    if isinstance(value, dict):
        for current_key, current_value in value.items():
            if current_key == key:
                found.append(current_value)
            found.extend(nested_values(current_value, key))
    elif isinstance(value, list):
        for current in value:
            found.extend(nested_values(current, key))
    return found


def unique_nested(run: dict[str, Any], keys: Iterable[str], context: str, required: bool = False) -> Any:
    for key in keys:
        values = nested_values(run, key)
        unique: list[Any] = []
        for value in values:
            if value not in unique:
                unique.append(value)
        if len(unique) == 1:
            return unique[0]
        if len(unique) > 1:
            raise ExportError(f"{context}: conflicting {key} values: {unique!r}")
    if required:
        raise ExportError(f"{context}: missing one of {list(keys)!r}")
    return None


def validate_spans(path: Path, run: dict[str, Any], spans: dict[str, dict[str, Any]]) -> str:
    root_id = run.get("root_span_id")
    if not isinstance(root_id, str) or root_id not in spans:
        raise ExportError(f"{path}: declared root_span_id is missing")
    for span_id, span in spans.items():
        parent = span.get("parent_span_id")
        if span_id == root_id:
            if parent is not None:
                raise ExportError(f"{path}: root span must have no parent")
        elif not isinstance(parent, str) or parent not in spans:
            raise ExportError(f"{path}: span {span_id!r} has missing parent {parent!r}")
        start = as_nonnegative_int(span.get("start_ns"), f"{path}: {span_id} start_ns")
        end = as_nonnegative_int(span.get("end_ns"), f"{path}: {span_id} end_ns")
        duration = as_nonnegative_int(span.get("duration_ns"), f"{path}: {span_id} duration_ns")
        if end < start or end - start != duration:
            raise ExportError(f"{path}: inconsistent half-open interval for {span_id!r}")
        if not isinstance(span.get("operation"), str) or not span["operation"]:
            raise ExportError(f"{path}: span {span_id!r} has no operation")
        if not isinstance(span.get("name"), str) or not span["name"]:
            raise ExportError(f"{path}: span {span_id!r} has no name")

    root_start = as_nonnegative_int(spans[root_id]["start_ns"], f"{path}: root start")
    root_end = as_nonnegative_int(spans[root_id]["end_ns"], f"{path}: root end")
    for span_id, span in spans.items():
        seen: set[str] = set()
        current = span_id
        while current != root_id:
            if current in seen:
                raise ExportError(f"{path}: parent cycle containing {span_id!r}")
            seen.add(current)
            current = spans[current]["parent_span_id"]
        start = int(span["start_ns"])
        end = int(span["end_ns"])
        if start < root_start or end > root_end:
            raise ExportError(f"{path}: span {span_id!r} escapes the declared root")
        parent_id = span.get("parent_span_id")
        if isinstance(parent_id, str):
            parent = spans[parent_id]
            if start < int(parent["start_ns"]) or end > int(parent["end_ns"]):
                raise ExportError(
                    f"{path}: span {span_id!r} escapes parent {parent_id!r}"
                )
    return root_id


def coordinate_atom(span: dict[str, Any]) -> str:
    coordinate = span.get("coordinate") if isinstance(span.get("coordinate"), dict) else {}
    parts = []
    for key, prefix in (
        ("occurrence_index", "o"),
        ("round_index", "r"),
        ("recursion_depth", "d"),
        ("recursion_instance_index", "i"),
    ):
        if key in coordinate:
            parts.append(f"{prefix}{as_nonnegative_int(coordinate[key], key)}")
    return ",".join(parts)


def assign_stable_keys(
    path: Path, spans: dict[str, dict[str, Any]], root_id: str
) -> tuple[dict[str, str], dict[str, int], dict[str, int | None]]:
    children: dict[str, list[str]] = defaultdict(list)
    for span_id, span in spans.items():
        if span_id != root_id:
            children[span["parent_span_id"]].append(span_id)
    for ids in children.values():
        ids.sort(key=lambda sid: (int(spans[sid]["start_ns"]), int(spans[sid]["end_ns"]), sid))

    stable = {root_id: "root"}
    depths = {root_id: 0}
    occurrence: dict[str, int | None] = {root_id: None}

    def visit(parent_id: str) -> None:
        groups: dict[tuple[str, str], list[str]] = defaultdict(list)
        for child_id in children.get(parent_id, []):
            child = spans[child_id]
            groups[(child["operation"], coordinate_atom(child))].append(child_id)
        for (operation, coordinate), ids in groups.items():
            for sibling_index, child_id in enumerate(ids):
                span = spans[child_id]
                declared = None
                coord = span.get("coordinate")
                if isinstance(coord, dict) and "occurrence_index" in coord:
                    declared = as_nonnegative_int(coord["occurrence_index"], "occurrence_index")
                occ = declared if declared is not None else (sibling_index if len(ids) > 1 else None)
                atom = operation
                if coordinate:
                    atom += f"[{coordinate}]"
                if len(ids) > 1 and declared is None:
                    atom += f"[#{sibling_index}]"
                stable[child_id] = f"{stable[parent_id]}/{atom}"
                depths[child_id] = depths[parent_id] + 1
                occurrence[child_id] = occ
                visit(child_id)

    visit(root_id)
    if len(stable) != len(spans):
        raise ExportError(f"{path}: not every span is reachable from the root")
    if len(set(stable.values())) != len(stable):
        raise ExportError(f"{path}: stable-key collision")
    return stable, depths, occurrence


def load_trace(path: Path) -> TraceRun:
    run, spans = read_jsonl(path)
    kind, index = trial_identity(run, path)
    root_id = validate_spans(path, run, spans)
    stable, depth, occurrence = assign_stable_keys(path, spans, root_id)
    return TraceRun(path, run, spans, root_id, kind, index, exponent_from_path(path), stable, depth, occurrence)


def operation_matches(operation: str, metric: str) -> bool:
    normalized = operation.lower().replace("_", ":").replace(".", ":")
    if metric == "trial":
        return normalized in {"bench:trial", "proof:end:to:end", "proof:end_to_end"}
    aliases = {
        "commit": {"bench:commit", "pcs:commit"},
        "prove": {"bench:prove", "pcs:prove", "pcs:open"},
        "serialize": {"bench:serialize", "proof:serialize", "serialize"},
        "verify": {"bench:verify", "pcs:verify", "verify", "v:total"},
    }
    return normalized in aliases[metric]


def interval_union_ns(intervals: list[tuple[int, int]]) -> int:
    if not intervals:
        return 0
    ordered = sorted(intervals)
    total = 0
    start, end = ordered[0]
    for next_start, next_end in ordered[1:]:
        if next_start <= end:
            end = max(end, next_end)
        else:
            total += end - start
            start, end = next_start, next_end
    return total + end - start


def metric_duration_ns(trace: TraceRun, metric: str) -> int:
    if metric == "trial":
        return int(trace.spans[trace.root_id]["duration_ns"])
    exact = [span for span in trace.spans.values() if operation_matches(span["operation"], metric)]
    # The benchmark boundary is preferred to similarly named nested upstream spans.
    bench = [span for span in exact if span["operation"].lower().startswith("bench:")]
    candidates = bench or exact
    if candidates:
        return max(int(span["duration_ns"]) for span in candidates)
    tag = {"commit": "commit", "prove": "proving", "verify": "verification", "serialize": "serialization"}[metric]
    tagged = []
    for span in trace.spans.values():
        tags = span.get("phase_tags")
        if isinstance(tags, list) and tag in tags:
            tagged.append((int(span["start_ns"]), int(span["end_ns"])))
    duration = interval_union_ns(tagged)
    if duration == 0:
        # A canonical producer may put boundary values in the run metadata.
        value = unique_nested(trace.run, (f"{metric}_ns",), str(trace.path), required=False)
        if value is None:
            raise ExportError(f"{trace.path}: cannot resolve {metric} timing")
        duration = as_nonnegative_int(value, f"{trace.path}: {metric}_ns")
    return duration


def span_metadata(span: dict[str, Any]) -> dict[str, Any]:
    operation = span["operation"]
    # The canonical producer normalizes the legacy profiler's first `:` to
    # `.` (for example `mc:forest` becomes `mc.forest`). Keep the catalog in
    # the source labels used by the Rust code, but recognize that wire form.
    legacy_operation = operation.replace(".", ":", 1)
    fallback = (
        SCOPE_CATALOG.get(operation)
        or SCOPE_CATALOG.get(legacy_operation)
        or NORMALIZED_SCOPE_CATALOG.get(operation.lower(), {})
    )
    attrs = span.get("attributes") if isinstance(span.get("attributes"), dict) else {}
    raw_tags = span.get("phase_tags")
    declared_tags = [str(tag) for tag in raw_tags] if isinstance(raw_tags, list) else []
    # The canonical adapter can only attach a generic ``proving`` tag to old
    # in-crate scopes.  Enrich those known labels here while retaining any
    # extra tags emitted by a newer producer.
    tags = list(dict.fromkeys([*fallback.get("tags", []), *declared_tags]))
    if not tags:
        tags = ["proving"]
    raw_name = span.get("name")
    adapter_names = {operation, legacy_operation, "F2Z prof scope"}
    name = fallback.get("name") if raw_name in adapter_names and fallback else raw_name
    name = name or fallback.get("name") or operation
    short = fallback.get("short") or attrs.get("short_name") or name
    group = fallback.get("group") or span.get("primary_phase") or tags[0]
    kind = fallback.get("kind") or attrs.get("scope_kind") or "procedure"
    math_value = attrs.get("math_latex", fallback.get("math", []))
    if isinstance(math_value, str):
        math_value = [math_value]
    math_out = [str(item) for item in math_value] if isinstance(math_value, list) else []
    return {
        "label": operation,
        "op": operation,
        "name": str(name),
        "short": str(short),
        "tags": tags,
        "group": str(group),
        "kind": str(kind),
        "math": math_out,
        "primary_sequence": bool(attrs.get("primary_sequence", False)),
    }


def extract_proof(trace: TraceRun) -> tuple[int, dict[str, int], str | None]:
    total_value = unique_nested(
        trace.run,
        ("proof_bytes", "serialized_proof_bytes"),
        str(trace.path),
        required=True,
    )
    total = as_nonnegative_int(total_value, f"{trace.path}: proof_bytes")
    artifacts: dict[str, int] = {}
    for name in EXPECTED_ARTIFACTS:
        value = unique_nested(
            trace.run,
            (f"artifact_{name}_bytes", f"{name}_bytes"),
            str(trace.path),
            required=True,
        )
        artifacts[name] = as_nonnegative_int(value, f"{trace.path}: artifact_{name}_bytes")
    if sum(artifacts.values()) != total:
        raise ExportError(
            f"{trace.path}: proof artifacts sum to {sum(artifacts.values())}, expected {total}"
        )
    fnv = unique_nested(trace.run, ("proof_fnv",), str(trace.path), required=False)
    if fnv is not None and not re.fullmatch(r"[0-9a-fA-F]{16}", str(fnv)):
        raise ExportError(f"{trace.path}: proof_fnv must contain 16 hexadecimal digits")
    return total, artifacts, str(fnv).lower() if fnv is not None else None


def trace_exponent(trace: TraceRun) -> int:
    value = unique_nested(
        trace.run,
        ("committed_bits_exponent", "witness_bits_exponent", "bits_exponent", "i", "n"),
        str(trace.path),
        required=False,
    )
    exponent = as_nonnegative_int(value, f"{trace.path}: exponent") if value is not None else trace.exponent_hint
    if exponent is None:
        raise ExportError(f"{trace.path}: cannot determine witness exponent")
    if trace.exponent_hint is not None and exponent != trace.exponent_hint:
        raise ExportError(f"{trace.path}: path exponent n{trace.exponent_hint} != metadata {exponent}")
    return exponent


def scalar_metadata(trace: TraceRun, keys: tuple[str, ...], required: bool = True) -> Any:
    return unique_nested(trace.run, keys, str(trace.path), required=required)


def assert_same(values: list[Any], context: str) -> Any:
    first = values[0]
    if any(value != first for value in values[1:]):
        raise ExportError(f"{context} changed across trials: {values!r}")
    return first


def aggregate_intervals(samples: list[TraceRun], representative: TraceRun) -> tuple[dict[str, Any], list[dict[str, Any]], dict[str, list[dict[str, Any]]], int]:
    by_run = [{key: trace.spans[span_id] for span_id, key in trace.stable.items()} for trace in samples]
    expected_keys = set(by_run[0])
    for trace, spans in zip(samples[1:], by_run[1:]):
        if set(spans) != expected_keys:
            missing = sorted(expected_keys - set(spans))
            extra = sorted(set(spans) - expected_keys)
            raise ExportError(f"{trace.path}: unstable event set; missing={missing}, extra={extra}")

    rep_by_key = by_run[samples.index(representative)]
    rep_root = representative.spans[representative.root_id]
    root_start = int(rep_root["start_ns"])
    aggregated: dict[str, dict[str, Any]] = {}
    root: dict[str, Any] | None = None
    for key in sorted(expected_keys):
        spans = [run_spans[key] for run_spans in by_run]
        rep_span = rep_by_key[key]
        metadata = [span_metadata(span) for span in spans]
        invariant_fields = ("label", "op", "name", "short", "tags", "group", "kind", "math")
        for field in invariant_fields:
            assert_same([item[field] for item in metadata], f"event {key} metadata {field}")
        durations = [int(span["duration_ns"]) for span in spans]
        stat = ms_stat([float(value) for value in durations])
        rep_id = next(span_id for span_id, stable_key in representative.stable.items() if stable_key == key)
        parent_id = rep_span.get("parent_span_id")
        item = {
            "stable_key": key,
            **{field: metadata[0][field] for field in invariant_fields},
            "start_ms": ms(int(rep_span["start_ns"]) - root_start),
            "end_ms": ms(int(rep_span["end_ns"]) - root_start),
            "rep_ms": ms(int(rep_span["duration_ns"])),
            "med_ms": stat["med"],
            "dec_ms": stat["dec"],
            "n": stat["n"],
            "depth": representative.depth[rep_id],
            "parent_key": representative.stable[parent_id] if isinstance(parent_id, str) else None,
            "occurrence": representative.occurrence[rep_id],
        }
        coordinate = rep_span.get("coordinate")
        if isinstance(coordinate, dict) and coordinate:
            item["coordinate"] = coordinate
        if not item["math"]:
            del item["math"]
        if key == "root":
            root = item
        else:
            aggregated[key] = item
    if root is None:
        raise ExportError("aggregated trace lost its root")

    explicitly_primary = [
        key for key, span in rep_by_key.items()
        if key != "root" and span_metadata(span)["primary_sequence"]
    ]
    primary_keys = explicitly_primary or [
        key for key, item in aggregated.items() if item["parent_key"] == "root"
    ]
    primary = sorted((aggregated[key] for key in primary_keys), key=lambda item: (item["start_ms"], item["end_ms"], item["stable_key"]))

    rows: dict[str, list[dict[str, Any]]] = {}
    for tag, _label in ROW_ORDER:
        selected = [item for item in aggregated.values() if tag in item["tags"]]
        if selected:
            rows[tag] = sorted(selected, key=lambda item: (item["start_ms"], item["end_ms"], item["stable_key"]))
    return root, primary, rows, len(expected_keys) - 1


def build_size(exponent: int, traces: list[TraceRun], allow_partial: bool) -> dict[str, Any]:
    series_ids = {trace.run.get("series_id") for trace in traces}
    if len(series_ids) != 1 or None in series_ids or "" in series_ids:
        raise ExportError(
            f"n{exponent}: trials must share one nonempty series_id, got {sorted(map(str, series_ids))}"
        )
    warmups = sorted((trace for trace in traces if trace.kind == "warmup"), key=lambda trace: trace.index)
    samples = sorted((trace for trace in traces if trace.kind == "sample"), key=lambda trace: trace.index)
    warmup_indices = [trace.index for trace in warmups]
    sample_indices = [trace.index for trace in samples]
    if allow_partial:
        if not samples or sample_indices != list(range(len(samples))):
            raise ExportError(f"n{exponent}: partial sample indices must be contiguous from zero")
    else:
        if warmup_indices != [0]:
            raise ExportError(f"n{exponent}: expected warmup index [0], got {warmup_indices}")
        if sample_indices != EXPECTED_SAMPLE_INDICES:
            raise ExportError(f"n{exponent}: expected sample indices 0..20, got {sample_indices}")

    metrics_ns = {
        metric: [metric_duration_ns(trace, metric) for trace in samples]
        for metric in ("trial", "commit", "prove", "serialize", "verify")
    }
    metrics = {metric: ms_stat([float(value) for value in values]) for metric, values in metrics_ns.items()}
    median_trial = type7([float(value) for value in metrics_ns["trial"]], 0.5)
    representative = min(
        samples,
        key=lambda trace: (abs(metric_duration_ns(trace, "trial") - median_trial), trace.index),
    )
    root, primary, rows, event_count = aggregate_intervals(samples, representative)

    proof_values: list[int] = []
    artifact_values: dict[str, list[int]] = {name: [] for name in EXPECTED_ARTIFACTS}
    proof_fnvs: list[str] = []
    for trace in samples:
        total, artifacts, fnv = extract_proof(trace)
        proof_values.append(total)
        for name, value in artifacts.items():
            artifact_values[name].append(value)
        if fnv is not None:
            proof_fnvs.append(fnv)

    t = as_nonnegative_int(assert_same([scalar_metadata(trace, ("t",)) for trace in traces], f"n{exponent} t"), "t")
    s = as_nonnegative_int(assert_same([scalar_metadata(trace, ("s",)) for trace in traces], f"n{exponent} s"), "s")
    word_bits = as_nonnegative_int(assert_same([scalar_metadata(trace, ("word_bits", "W")) for trace in traces], f"n{exponent} word_bits"), "word_bits")
    requested = str(assert_same([scalar_metadata(trace, ("requested_config", "profile_arg", "ligerito_requested", "lig_profile")) for trace in traces], f"n{exponent} requested config"))
    resolved = str(assert_same([scalar_metadata(trace, ("resolved_config", "ligerito_resolved", "ligerito_profile", "ligerito_config", "lig")) for trace in traces], f"n{exponent} resolved config"))
    threads = as_nonnegative_int(assert_same([scalar_metadata(trace, ("threads",)) for trace in traces], f"n{exponent} threads"), "threads")
    if t + s != exponent:
        raise ExportError(f"n{exponent}: t+s={t+s}, expected {exponent}")
    if not requested.strip() or "," in requested:
        raise ExportError(f"n{exponent}: requested config must be one nonempty profile")
    if re.fullmatch(r"[^@]+@r1/[0-9]+k[0-9]+", resolved) is None:
        raise ExportError(f"n{exponent}: malformed resolved config {resolved!r}")

    proof: dict[str, Any] = {
        "total": byte_stat(proof_values),
        "artifacts": {name: byte_stat(values) for name, values in artifact_values.items()},
    }
    if proof_fnvs:
        if len(proof_fnvs) != len(samples):
            raise ExportError(f"n{exponent}: proof_fnv present for only some samples")
        if len(set(proof_fnvs)) != 1:
            raise ExportError(f"n{exponent}: deterministic proof_fnv changed across samples")
        proof["fnv"] = proof_fnvs

    return {
        "i": exponent,
        "t": t,
        "s": s,
        "word_bits": word_bits,
        "requested_config": requested,
        "resolved_config": resolved,
        "threads": threads,
        "warmup_n": len(warmups),
        "measured_n": len(samples),
        "representative_trial": representative.index,
        "representative_run_id": representative.run["run_id"],
        "root_rep_ms": root["rep_ms"],
        "root_median_ms": metrics["trial"]["med"],
        "geometry_scale": 1.0,
        "metrics": metrics,
        "root": root,
        "primary": primary,
        "rows": rows,
        "proof": proof,
        "validation": {
            "sample_indices": sample_indices,
            "warmup_indices": warmup_indices,
            "stable_event_count": event_count,
            "real_representative_geometry": True,
            "all_events_present_every_trial": True,
            "proof_artifacts_reconcile_every_trial": True,
        },
    }


def discover(args: argparse.Namespace) -> list[Path]:
    paths: list[Path] = list(args.logs)
    paths.extend(args.log)
    if args.logs_dir is not None:
        if not args.logs_dir.is_dir():
            raise ExportError(f"logs directory does not exist: {args.logs_dir}")
        # The resumable runner preserves failed/replaced/partial captures in
        # hidden sibling directories. They are audit evidence, not members of
        # the active series, so never merge them into the strict export.
        paths.extend(
            path
            for path in args.logs_dir.rglob("*.jsonl")
            if not any(
                part.startswith(".")
                for part in path.relative_to(args.logs_dir).parts
            )
        )
    unique = sorted({path.resolve() for path in paths})
    if not unique:
        raise ExportError("no JSONL traces found")
    return unique


def build_collection(paths: list[Path], allow_partial: bool) -> dict[str, Any]:
    traces = [load_trace(path) for path in paths]
    run_ids = [trace.run["run_id"] for trace in traces]
    if len(set(run_ids)) != len(run_ids):
        raise ExportError("run_id values must be globally unique across the input collection")
    grouped: dict[int, list[TraceRun]] = defaultdict(list)
    for trace in traces:
        grouped[trace_exponent(trace)].append(trace)
    exponents = sorted(grouped)
    if not allow_partial and exponents != EXPECTED_EXPONENTS:
        raise ExportError(f"expected exponents 20..30, got {exponents}")
    sizes = [build_size(exponent, grouped[exponent], allow_partial) for exponent in exponents]
    profiles = {size["requested_config"] for size in sizes}
    if len(profiles) != 1:
        raise ExportError(f"inconsistent requested profiles: {sorted(profiles)}")
    return {
        "schema_version": 1,
        "schema": "f2z.interval-data/v1",
        "quantile_method": "Hyndman-Fan Type 7 sample deciles",
        "geometry": "unscaled intervals from the measured trial nearest the Type-7 median trial wall time",
        "sizes": sizes,
        "row_order": [{"tag": tag, "label": label} for tag, label in ROW_ORDER if any(tag in size["rows"] for size in sizes)],
        "validation": {
            "exponents": exponents,
            "strict_complete": not allow_partial,
            "trace_schema": TRACE_SCHEMA,
            "all_sizes_validated": True,
        },
    }


def validate_part(
    logs_dir: Path,
    *,
    expected_profile: str,
    expected_exponent: int,
    expected_threads: int,
) -> None:
    paths = sorted(logs_dir.glob("*.jsonl"))
    payload = build_collection(paths, allow_partial=True)
    if len(payload["sizes"]) != 1:
        raise ExportError(f"expected one interval size in {logs_dir}")
    size = payload["sizes"][0]
    if size["i"] != expected_exponent:
        raise ExportError(
            f"captured exponent {size['i']} != expected {expected_exponent}"
        )
    if size["requested_config"] != expected_profile:
        raise ExportError(
            f"captured profile {size['requested_config']!r} != expected "
            f"{expected_profile!r}"
        )
    if size["threads"] != expected_threads:
        raise ExportError(
            f"captured threads {size['threads']} != expected {expected_threads}"
        )
    if size["warmup_n"] != 1 or size["measured_n"] != 21:
        raise ExportError(
            f"expected 1 warmup + 21 samples, got "
            f"{size['warmup_n']} + {size['measured_n']}"
        )
    validation = size["validation"]
    warmup_indices = validation["warmup_indices"]
    sample_indices = validation["sample_indices"]
    if warmup_indices != [0]:
        raise ExportError(f"expected warmup index [0], got {warmup_indices}")
    if sample_indices != list(range(21)):
        raise ExportError(f"expected sample indices 0..20, got {sample_indices}")


def atomic_write(path: Path, text: str, force: bool) -> None:
    if path.exists() and not force:
        raise ExportError(f"refusing to overwrite {path}; pass --force")
    path.parent.mkdir(parents=True, exist_ok=True)
    descriptor, temporary = tempfile.mkstemp(prefix=f".{path.name}.", dir=path.parent)
    try:
        with os.fdopen(descriptor, "w", encoding="utf-8") as output:
            output.write(text)
            output.flush()
            os.fsync(output.fileno())
        os.replace(temporary, path)
    finally:
        try:
            os.unlink(temporary)
        except FileNotFoundError:
            pass


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("logs", nargs="*", type=Path, help="canonical JSONL trace files")
    parser.add_argument("--log", action="append", default=[], type=Path, help="add one trace file")
    parser.add_argument("--logs-dir", type=Path, help="recursively discover *.jsonl traces")
    parser.add_argument("--output", type=Path, help="write interval-data.json here")
    parser.add_argument(
        "--expected-profile",
        help="optional caller-supplied guard for the requested Ligerito profile",
    )
    parser.add_argument("--force", action="store_true", help="replace an existing output")
    parser.add_argument("--allow-partial", action="store_true", help="allow a contiguous subset for smoke tests")
    parser.add_argument("--validate-part", type=Path, help="validate one completed nXX trace directory and write nothing")
    parser.add_argument("--expected-exponent", type=int)
    parser.add_argument("--expected-threads", type=int)
    parser.add_argument("--pretty", action="store_true", help="indent JSON instead of the compact default")
    args = parser.parse_args()
    try:
        if args.validate_part is not None:
            if args.expected_profile is None or args.expected_exponent is None or args.expected_threads is None:
                raise ExportError(
                    "--validate-part requires --expected-profile, "
                    "--expected-exponent, and --expected-threads"
                )
            validate_part(
                args.validate_part,
                expected_profile=args.expected_profile,
                expected_exponent=args.expected_exponent,
                expected_threads=args.expected_threads,
            )
            print(f"validated interval part: {args.validate_part}", file=sys.stderr)
            return 0
        if args.output is None:
            raise ExportError("--output is required unless --validate-part is used")
        payload = build_collection(discover(args), args.allow_partial)
        profiles = {size["requested_config"] for size in payload["sizes"]}
        captured_profile = next(iter(profiles))
        if args.expected_profile is not None and captured_profile != args.expected_profile:
            raise ExportError(
                f"captured profile {captured_profile!r} != expected {args.expected_profile!r}"
            )
        text = json.dumps(
            payload,
            ensure_ascii=False,
            indent=2 if args.pretty else None,
            separators=None if args.pretty else (",", ":"),
        ) + "\n"
        atomic_write(args.output, text, args.force)
    except (ExportError, OSError, ValueError) as error:
        print(f"export_f2z_intervals.py: {error}", file=sys.stderr)
        return 2
    print(
        f"validated {len(payload['sizes'])} size(s); wrote {args.output}",
        file=sys.stderr,
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

#!/usr/bin/env python3
"""Aggregate canonical BabyBear BitZ/WHIR PCS comparison traces.

The input is canonical ``zkperf.trace/v1`` JSONL.  This reporter deliberately
does not render interval timelines; use the zk-proof-profiler ``zk_trace.py``
tool for that view.
"""

from __future__ import annotations

import argparse
import csv
import html
import json
import math
import sys
from collections import defaultdict
from datetime import datetime, timezone
from pathlib import Path
from typing import Any, Iterable, Sequence


TRACE_SCHEMA = "zkperf.trace/v1"
REPORT_SCHEMA = "baby-bear-pcs-compare-report/v2"
CAMPAIGN_SCHEMA = "baby-bear-pcs-compare-campaign/v2"
BACKENDS = ("bitz", "plonky3-whir")
BACKEND_LABELS = {"bitz": "BitZ", "plonky3-whir": "Plonky3 WHIR"}
EXPONENTS = tuple(range(15, 25))
CAMPAIGN_STATUSES = ("measured", "unavailable", "not_requested")

TIMING_OPERATIONS = (
    ("materialize", "pcs_compare.materialize", "Materialize"),
    ("commit", "pcs_compare.commit", "Commit"),
    ("claim_setup", "pcs_compare.claim_setup", "Claim setup"),
    ("opening", "pcs_compare.opening", "Opening"),
    ("verify", "pcs_compare.verify", "Verify"),
)
PCS_TOTAL_KEY = "pcs_total_prover"
PCS_TOTAL_LABEL = "PCS total prover"
PCS_TOTAL_OPERATIONS = {
    "pcs_compare.materialize",
    "pcs_compare.commit",
    "pcs_compare.opening",
}

ARTIFACT_METRICS = (
    ("commitment_bytes", "Commitment"),
    ("public_claim_bytes", "Public claim"),
    ("opening_proof_bytes", "Opening proof"),
    ("total_wire_bytes", "Total wire"),
)

CONTROLLED_TAGS = {
    "end-to-end",
    "witness-generation",
    "preparation",
    "proving",
    "commit",
    "pcs",
    "opening-proof",
    "constraint-proof",
    "sumcheck",
    "fri",
    "verification",
}

OUTPUT_NAMES = ("summary.json", "metrics.csv", "comparison.md", "comparison.html")


class ReportError(Exception):
    """A user-facing trace or output error."""


def parse_args(argv: Sequence[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description=(
            "Aggregate BitZ versus Plonky3 WHIR BabyBear PCS traces into "
            "JSON, CSV, Markdown, and HTML comparison reports."
        )
    )
    parser.add_argument("trace", type=Path, metavar="TRACE", help="canonical trace JSONL")
    parser.add_argument("--out-dir", type=Path, required=True, metavar="DIR")
    parser.add_argument(
        "--campaign",
        type=Path,
        metavar="JSON",
        help=(
            "optional baby-bear-pcs-compare-campaign/v2 preflight manifest; "
            "distinguishes unavailable and unrequested cells from missing trace data"
        ),
    )
    parser.add_argument(
        "--force",
        action="store_true",
        help="overwrite existing report files in DIR",
    )
    return parser.parse_args(argv)


def require_object(value: Any, context: str) -> dict[str, Any]:
    if not isinstance(value, dict):
        raise ReportError(f"{context} must be a JSON object")
    return value


def require_string(value: Any, context: str) -> str:
    if not isinstance(value, str) or not value:
        raise ReportError(f"{context} must be a non-empty string")
    return value


def require_nonnegative_int(value: Any, context: str) -> int:
    if isinstance(value, bool) or not isinstance(value, int) or value < 0:
        raise ReportError(f"{context} must be a non-negative integer")
    return value


def parse_ns(value: Any, context: str) -> int:
    if not isinstance(value, str) or not value or not value.isascii() or not value.isdecimal():
        raise ReportError(f"{context} must be an unsigned decimal string")
    return int(value)


def whir_config_from_run(
    run: dict[str, Any],
) -> tuple[int | None, int | None, int | None]:
    if run["benchmark"]["implementation"] != "plonky3-whir":
        return None, None, None
    run_id = run["run_id"]
    parameters = require_object(run.get("parameters"), f"run {run_id}.parameters")
    field = require_object(parameters.get("field"), f"run {run_id}.parameters.field")
    security = require_object(
        parameters.get("security"), f"run {run_id}.parameters.security"
    )
    degree = require_nonnegative_int(
        field.get("challenge_extension_degree"),
        f"run {run_id}.parameters.field.challenge_extension_degree",
    )
    security_degree = require_nonnegative_int(
        security.get("challenge_extension_degree"),
        f"run {run_id}.parameters.security.challenge_extension_degree",
    )
    if degree == 0 or security_degree == 0:
        raise ReportError(f"run {run_id} WHIR extension degree must be positive")
    if degree != security_degree:
        raise ReportError(
            f"run {run_id} reports different WHIR extension degrees in field and security"
        )
    configured_pow = require_nonnegative_int(
        security.get("configured_max_pow_bits"),
        f"run {run_id}.parameters.security.configured_max_pow_bits",
    )
    derived_pow = require_nonnegative_int(
        security.get("derived_max_pow_bits"),
        f"run {run_id}.parameters.security.derived_max_pow_bits",
    )
    if derived_pow > configured_pow:
        raise ReportError(
            f"run {run_id} derived WHIR PoW ({derived_pow}) exceeds its configured cap "
            f"({configured_pow})"
        )
    return degree, configured_pow, derived_pow


def read_campaign(
    path: Path,
) -> tuple[str, dict[tuple[str, int], dict[str, Any]]]:
    if not path.is_file():
        raise ReportError(f"campaign manifest is not a file: {path}")
    try:
        manifest = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        raise ReportError(f"cannot read campaign manifest {path}: {error}") from error
    manifest = require_object(manifest, "campaign manifest")
    if manifest.get("schema") != CAMPAIGN_SCHEMA:
        raise ReportError(
            f"campaign manifest schema must be {CAMPAIGN_SCHEMA!r}; "
            f"got {manifest.get('schema')!r}"
        )
    campaign_id = require_string(
        manifest.get("campaign_id"), "campaign manifest.campaign_id"
    )
    trace_schema = manifest.get("trace_schema")
    if trace_schema is not None and trace_schema != TRACE_SCHEMA:
        raise ReportError(
            f"campaign manifest.trace_schema must be {TRACE_SCHEMA!r}; got {trace_schema!r}"
        )
    whir_configuration = require_object(
        manifest.get("whir_fixed_configuration"),
        "campaign manifest.whir_fixed_configuration",
    )
    whir_extension_degree = require_nonnegative_int(
        whir_configuration.get("challenge_extension_degree"),
        "campaign manifest.whir_fixed_configuration.challenge_extension_degree",
    )
    if whir_extension_degree == 0:
        raise ReportError("WHIR challenge extension degree must be positive")
    whir_pow_cap = require_nonnegative_int(
        whir_configuration.get("max_pow_bits"),
        "campaign manifest.whir_fixed_configuration.max_pow_bits",
    )
    cells = manifest.get("cells")
    if not isinstance(cells, list):
        raise ReportError("campaign manifest.cells must be an array")

    by_key: dict[tuple[str, int], dict[str, Any]] = {}
    for index, raw_cell in enumerate(cells):
        context = f"campaign manifest.cells[{index}]"
        cell = require_object(raw_cell, context)
        implementation = require_string(cell.get("implementation"), f"{context}.implementation")
        if implementation not in BACKENDS:
            raise ReportError(
                f"{context}.implementation must be one of {BACKENDS}; got {implementation!r}"
            )
        exponent = require_nonnegative_int(
            cell.get("log_multiplications"), f"{context}.log_multiplications"
        )
        if exponent not in EXPONENTS:
            raise ReportError(
                f"{context}.log_multiplications must be in 15 through 24; got {exponent}"
            )
        status = require_string(cell.get("status"), f"{context}.status")
        if status not in CAMPAIGN_STATUSES:
            raise ReportError(
                f"{context}.status must be one of {CAMPAIGN_STATUSES}; got {status!r}"
            )
        reason = cell.get("reason")
        if reason is not None and (not isinstance(reason, str) or not reason):
            raise ReportError(f"{context}.reason must be a non-empty string when present")
        for key in (
            "required_pow_bits",
            "budget",
            "challenge_extension_degree",
            "configured_max_pow_bits",
            "derived_max_pow_bits",
        ):
            value = cell.get(key)
            if value is not None:
                require_nonnegative_int(value, f"{context}.{key}")
        if implementation == "plonky3-whir":
            if cell.get("challenge_extension_degree") != whir_extension_degree:
                raise ReportError(
                    f"{context}.challenge_extension_degree must match the fixed WHIR "
                    f"configuration ({whir_extension_degree})"
                )
            if cell.get("configured_max_pow_bits") != whir_pow_cap:
                raise ReportError(
                    f"{context}.configured_max_pow_bits must match the fixed WHIR "
                    f"configuration ({whir_pow_cap})"
                )
            if status == "measured" and cell.get("derived_max_pow_bits") is None:
                raise ReportError(
                    f"{context}.derived_max_pow_bits is required for measured WHIR cells"
                )

        key = (implementation, exponent)
        if key in by_key:
            raise ReportError(
                f"campaign manifest contains duplicate cell {implementation} 2^{exponent}"
            )
        by_key[key] = {
            "implementation": implementation,
            "log_multiplications": exponent,
            "status": status,
            "reason": reason,
            "required_pow_bits": cell.get("required_pow_bits"),
            "budget": cell.get("budget"),
            "challenge_extension_degree": cell.get("challenge_extension_degree"),
            "configured_max_pow_bits": cell.get("configured_max_pow_bits"),
            "derived_max_pow_bits": cell.get("derived_max_pow_bits"),
        }

    expected = {(backend, exponent) for backend in BACKENDS for exponent in EXPONENTS}
    actual = set(by_key)
    missing = sorted(expected - actual)
    extra = sorted(actual - expected)
    if missing or extra:
        details = []
        if missing:
            details.append(
                "missing " + ", ".join(f"{backend} 2^{exponent}" for backend, exponent in missing)
            )
        if extra:
            details.append(
                "unexpected "
                + ", ".join(f"{backend} 2^{exponent}" for backend, exponent in extra)
            )
        raise ReportError(
            "campaign manifest must cover the complete backend/size matrix: "
            + "; ".join(details)
        )

    requested_raw = manifest.get("requested_log_multiplications")
    selected_raw = manifest.get("selected_implementations")
    if (requested_raw is None) != (selected_raw is None):
        raise ReportError(
            "campaign manifest must provide requested_log_multiplications and "
            "selected_implementations together"
        )
    if requested_raw is not None and selected_raw is not None:
        if not isinstance(requested_raw, list) or not requested_raw:
            raise ReportError(
                "campaign manifest.requested_log_multiplications must be a non-empty array"
            )
        requested: list[int] = []
        for index, value in enumerate(requested_raw):
            exponent = require_nonnegative_int(
                value, f"campaign manifest.requested_log_multiplications[{index}]"
            )
            if exponent not in EXPONENTS:
                raise ReportError(
                    "campaign manifest.requested_log_multiplications entries must be "
                    f"in 15 through 24; got {exponent}"
                )
            requested.append(exponent)
        if len(requested) != len(set(requested)):
            raise ReportError(
                "campaign manifest.requested_log_multiplications contains duplicates"
            )

        if not isinstance(selected_raw, list) or not selected_raw:
            raise ReportError(
                "campaign manifest.selected_implementations must be a non-empty array"
            )
        selected: list[str] = []
        for index, value in enumerate(selected_raw):
            backend = require_string(
                value, f"campaign manifest.selected_implementations[{index}]"
            )
            if backend not in BACKENDS:
                raise ReportError(
                    "campaign manifest.selected_implementations entries must be one of "
                    f"{BACKENDS}; got {backend!r}"
                )
            selected.append(backend)
        if len(selected) != len(set(selected)):
            raise ReportError("campaign manifest.selected_implementations contains duplicates")

        requested_matrix = {
            (backend, exponent) for backend in selected for exponent in requested
        }
        for key, cell in by_key.items():
            should_run_or_preflight = key in requested_matrix
            is_not_requested = cell["status"] == "not_requested"
            if should_run_or_preflight == is_not_requested:
                backend, exponent = key
                expected_status = (
                    "measured or unavailable"
                    if should_run_or_preflight
                    else "not_requested"
                )
                raise ReportError(
                    f"campaign cell {backend} 2^{exponent} must be {expected_status} "
                    "under requested_log_multiplications and selected_implementations"
                )
    return campaign_id, by_key


def validate_trace_provenance(
    runs: Sequence[dict[str, Any]], campaign_id: str | None
) -> None:
    identity_by_exponent: dict[int, tuple[str, str, int, int]] = {}
    observed_campaign_ids: set[str] = set()
    for run in runs:
        run_id = run["run_id"]
        tags = require_object(run.get("tags"), f"run {run_id}.tags")
        raw_campaign_id = tags.get("campaign_id")
        if raw_campaign_id is not None:
            observed_campaign_ids.add(
                require_string(raw_campaign_id, f"run {run_id}.tags.campaign_id")
            )
        if campaign_id is not None and raw_campaign_id != campaign_id:
            raise ReportError(
                f"run {run_id} campaign_id {raw_campaign_id!r} does not match "
                f"manifest campaign_id {campaign_id!r}"
            )

        parameters = require_object(run.get("parameters"), f"run {run_id}.parameters")
        inputs = require_object(parameters.get("input"), f"run {run_id}.parameters.input")
        exponent = require_nonnegative_int(
            inputs.get("log_multiplications"),
            f"run {run_id}.parameters.input.log_multiplications",
        )
        identity = (
            require_string(parameters.get("seed"), f"run {run_id}.parameters.seed"),
            require_string(
                inputs.get("witness_digest_blake3"),
                f"run {run_id}.parameters.input.witness_digest_blake3",
            ),
            require_nonnegative_int(
                inputs.get("capacity"), f"run {run_id}.parameters.input.capacity"
            ),
            require_nonnegative_int(
                inputs.get("gate_variables"),
                f"run {run_id}.parameters.input.gate_variables",
            ),
        )
        prior = identity_by_exponent.setdefault(exponent, identity)
        if identity != prior:
            raise ReportError(
                f"trace does not use one shared logical witness for every 2^{exponent} run"
            )

    if campaign_id is None and len(observed_campaign_ids) > 1:
        raise ReportError(
            "trace contains more than one campaign_id: "
            + ", ".join(sorted(observed_campaign_ids))
        )


def validate_run_record(run: dict[str, Any], line_number: int) -> None:
    context = f"line {line_number} run record"
    run_id = require_string(run.get("run_id"), f"{context}.run_id")
    require_string(run.get("series_id"), f"run {run_id}.series_id")
    require_string(run.get("root_span_id"), f"run {run_id}.root_span_id")

    benchmark = require_object(run.get("benchmark"), f"run {run_id}.benchmark")
    implementation = require_string(
        benchmark.get("implementation"), f"run {run_id}.benchmark.implementation"
    )
    if implementation not in BACKENDS:
        raise ReportError(
            f"run {run_id}.benchmark.implementation must be one of {BACKENDS}; "
            f"got {implementation!r}"
        )

    parameters = require_object(run.get("parameters"), f"run {run_id}.parameters")
    input_parameters = require_object(
        parameters.get("input"), f"run {run_id}.parameters.input"
    )
    exponent = require_nonnegative_int(
        input_parameters.get("log_multiplications"),
        f"run {run_id}.parameters.input.log_multiplications",
    )
    if exponent not in EXPONENTS:
        raise ReportError(
            f"run {run_id} has log_multiplications={exponent}; expected 15 through 24"
        )
    whir_config_from_run(run)

    trial = require_object(run.get("trial"), f"run {run_id}.trial")
    kind = trial.get("kind")
    if kind == "warmup":
        require_nonnegative_int(trial.get("warmup_index"), f"run {run_id}.trial.warmup_index")
    elif kind == "sample":
        require_nonnegative_int(trial.get("sample_index"), f"run {run_id}.trial.sample_index")
    else:
        raise ReportError(f"run {run_id}.trial.kind must be 'warmup' or 'sample'")

    status = require_string(run.get("status"), f"run {run_id}.status")
    trace_complete = run.get("trace_complete")
    if not isinstance(trace_complete, bool):
        raise ReportError(f"run {run_id}.trace_complete must be a boolean")

    clock = require_object(run.get("clock"), f"run {run_id}.clock")
    require_string(clock.get("id"), f"run {run_id}.clock.id")
    if clock.get("kind") != "monotonic":
        raise ReportError(f"run {run_id}.clock.kind must be 'monotonic'")
    if clock.get("unit") != "ns":
        raise ReportError(f"run {run_id}.clock.unit must be 'ns'")

    # Failed/incomplete records remain useful exclusion evidence and may stop
    # before artifacts are available. Complete successful records must carry
    # the complete wire-size accounting contract.
    if status == "ok" and trace_complete:
        artifacts = require_object(run.get("artifacts"), f"run {run_id}.artifacts")
        for key, _ in ARTIFACT_METRICS:
            require_nonnegative_int(artifacts.get(key), f"run {run_id}.artifacts.{key}")


def validate_span_record(span: dict[str, Any], line_number: int) -> None:
    context = f"line {line_number} span record"
    required_fields = {
        "schema",
        "record",
        "run_id",
        "span_id",
        "parent_span_id",
        "operation",
        "name",
        "primary_phase",
        "phase_tags",
        "start_ns",
        "end_ns",
        "duration_ns",
    }
    missing_fields = sorted(required_fields - span.keys())
    if missing_fields:
        raise ReportError(f"{context} is missing fields: {', '.join(missing_fields)}")
    run_id = require_string(span.get("run_id"), f"{context}.run_id")
    require_string(span.get("span_id"), f"{context}.span_id")
    parent = span.get("parent_span_id")
    if parent is not None and (not isinstance(parent, str) or not parent):
        raise ReportError(f"{context}.parent_span_id must be null or a non-empty string")
    require_string(span.get("operation"), f"{context}.operation")
    require_string(span.get("name"), f"{context}.name")
    primary_phase = require_string(span.get("primary_phase"), f"{context}.primary_phase")
    if primary_phase not in CONTROLLED_TAGS:
        raise ReportError(f"{context}.primary_phase is not a controlled tag")
    phase_tags = span.get("phase_tags")
    if not isinstance(phase_tags, list) or any(not isinstance(tag, str) for tag in phase_tags):
        raise ReportError(f"{context}.phase_tags must be a list of strings")
    if len(set(phase_tags)) != len(phase_tags):
        raise ReportError(f"{context}.phase_tags contains duplicates")
    unknown_tags = set(phase_tags) - CONTROLLED_TAGS
    if unknown_tags:
        raise ReportError(f"{context}.phase_tags has unknown tags: {sorted(unknown_tags)}")
    if primary_phase not in phase_tags:
        raise ReportError(f"{context}.phase_tags must include primary_phase")

    start = parse_ns(span.get("start_ns"), f"{context}.start_ns")
    end = parse_ns(span.get("end_ns"), f"{context}.end_ns")
    duration = parse_ns(span.get("duration_ns"), f"{context}.duration_ns")
    if end < start:
        raise ReportError(f"{context} in run {run_id} ends before it starts")
    if duration != end - start:
        raise ReportError(
            f"{context} in run {run_id} has duration_ns={duration}, expected {end - start}"
        )


def read_trace(
    path: Path, *, allow_empty: bool = False
) -> tuple[list[dict[str, Any]], dict[str, list[dict[str, Any]]]]:
    if not path.is_file():
        raise ReportError(f"trace is not a file: {path}")

    runs: dict[str, dict[str, Any]] = {}
    spans_by_run: dict[str, list[dict[str, Any]]] = defaultdict(list)
    saw_record = False

    try:
        handle = path.open("r", encoding="utf-8")
    except OSError as error:
        raise ReportError(f"cannot open trace {path}: {error}") from error

    with handle:
        for line_number, raw_line in enumerate(handle, 1):
            line = raw_line.strip()
            if not line:
                continue
            saw_record = True
            try:
                record = json.loads(line)
            except json.JSONDecodeError as error:
                raise ReportError(f"invalid JSON on line {line_number}: {error.msg}") from error
            if not isinstance(record, dict):
                raise ReportError(f"line {line_number} must contain a JSON object")
            if record.get("schema") != TRACE_SCHEMA:
                raise ReportError(
                    f"line {line_number} schema must be {TRACE_SCHEMA!r}; "
                    f"got {record.get('schema')!r}"
                )
            record_kind = record.get("record")
            if record_kind == "run":
                validate_run_record(record, line_number)
                run_id = record["run_id"]
                if run_id in runs:
                    raise ReportError(f"duplicate run_id {run_id!r} on line {line_number}")
                record["_source_line"] = line_number
                runs[run_id] = record
            elif record_kind == "span":
                validate_span_record(record, line_number)
                record["_source_line"] = line_number
                spans_by_run[record["run_id"]].append(record)
            else:
                raise ReportError(
                    f"line {line_number}.record must be 'run' or 'span'; got {record_kind!r}"
                )

    if not saw_record:
        if allow_empty:
            return [], {}
        raise ReportError(f"trace is empty: {path}")
    orphan_run_ids = sorted(set(spans_by_run) - set(runs))
    if orphan_run_ids:
        raise ReportError(f"span records reference missing runs: {', '.join(orphan_run_ids)}")
    if not runs:
        raise ReportError("trace contains no run records")
    return list(runs.values()), spans_by_run


def span_interval(span: dict[str, Any]) -> tuple[int, int]:
    return int(span["start_ns"]), int(span["end_ns"])


def validate_complete_run_topology(
    run: dict[str, Any], spans: Sequence[dict[str, Any]]
) -> None:
    run_id = run["run_id"]
    if not spans:
        raise ReportError(f"complete successful run {run_id} has no spans")

    spans_by_id: dict[str, dict[str, Any]] = {}
    for span in spans:
        span_id = span["span_id"]
        if span_id in spans_by_id:
            raise ReportError(f"run {run_id} has duplicate span_id {span_id!r}")
        spans_by_id[span_id] = span

    root_id = run["root_span_id"]
    root = spans_by_id.get(root_id)
    if root is None:
        raise ReportError(f"complete successful run {run_id} is missing root span {root_id!r}")
    if root.get("parent_span_id") is not None:
        raise ReportError(f"run {run_id} root span must have parent_span_id=null")
    root_start, root_end = span_interval(root)

    for span_id, span in spans_by_id.items():
        start, end = span_interval(span)
        if start < root_start or end > root_end:
            raise ReportError(f"run {run_id} span {span_id!r} lies outside its root")
        if span_id == root_id:
            continue
        parent_id = span.get("parent_span_id")
        if parent_id not in spans_by_id:
            raise ReportError(
                f"run {run_id} span {span_id!r} references missing parent {parent_id!r}"
            )
        parent_start, parent_end = span_interval(spans_by_id[parent_id])
        if start < parent_start or end > parent_end:
            raise ReportError(
                f"run {run_id} span {span_id!r} is not contained by parent {parent_id!r}"
            )

        seen: set[str] = set()
        cursor: str | None = span_id
        while cursor is not None:
            if cursor in seen:
                raise ReportError(f"run {run_id} has a parent cycle through span {cursor!r}")
            seen.add(cursor)
            cursor_span = spans_by_id.get(cursor)
            if cursor_span is None:
                break
            cursor = cursor_span.get("parent_span_id")


def run_group_key(run: dict[str, Any]) -> tuple[str, int]:
    return (
        run["benchmark"]["implementation"],
        run["parameters"]["input"]["log_multiplications"],
    )


def eligible_sample(run: dict[str, Any]) -> bool:
    return (
        run["trial"]["kind"] == "sample"
        and run["status"] == "ok"
        and run["trace_complete"] is True
    )


def successful_complete(run: dict[str, Any]) -> bool:
    return run["status"] == "ok" and run["trace_complete"] is True


def validate_campaign(
    runs: Sequence[dict[str, Any]], spans_by_run: dict[str, list[dict[str, Any]]]
) -> dict[tuple[str, int], list[dict[str, Any]]]:
    groups: dict[tuple[str, int], list[dict[str, Any]]] = defaultdict(list)
    for run in runs:
        groups[run_group_key(run)].append(run)
        span_ids = [span["span_id"] for span in spans_by_run.get(run["run_id"], [])]
        if len(span_ids) != len(set(span_ids)):
            raise ReportError(f"run {run['run_id']} has duplicate span_id values")
        if successful_complete(run):
            validate_complete_run_topology(run, spans_by_run.get(run["run_id"], []))

    required_operations = {operation for _, operation, _ in TIMING_OPERATIONS}
    for (backend, exponent), group_runs in sorted(groups.items()):
        warmups = [run for run in group_runs if run["trial"]["kind"] == "warmup"]
        samples = [run for run in group_runs if run["trial"]["kind"] == "sample"]
        if len(warmups) != 1:
            raise ReportError(
                f"{backend} 2^{exponent} must have exactly one warmup; found {len(warmups)}"
            )
        warmup = warmups[0]
        if warmup["trial"].get("warmup_index") != 0:
            raise ReportError(f"{backend} 2^{exponent} warmup_index must be 0")
        if not successful_complete(warmup):
            raise ReportError(f"{backend} 2^{exponent} warmup must be successful and complete")
        if not samples:
            raise ReportError(f"{backend} 2^{exponent} has no measured sample records")

        sample_indices = [run["trial"]["sample_index"] for run in samples]
        if len(sample_indices) != len(set(sample_indices)):
            raise ReportError(f"{backend} 2^{exponent} has duplicate sample_index values")
        series_ids = {run["series_id"] for run in group_runs}
        if len(series_ids) != 1:
            raise ReportError(
                f"{backend} 2^{exponent} spans multiple series_id values: {sorted(series_ids)}"
            )

        for run in [warmup, *(sample for sample in samples if eligible_sample(sample))]:
            operations = {
                span["operation"] for span in spans_by_run.get(run["run_id"], [])
            }
            missing = sorted(required_operations - operations)
            if missing:
                raise ReportError(
                    f"complete run {run['run_id']} is missing required operations: "
                    f"{', '.join(missing)}"
                )
    return groups


def validate_manifest_trace_consistency(
    cells: dict[tuple[str, int], dict[str, Any]],
    groups: dict[tuple[str, int], list[dict[str, Any]]],
) -> None:
    traced = set(groups)
    declared_measured = {
        key for key, cell in cells.items() if cell["status"] == "measured"
    }
    unexpected_trace = sorted(traced - declared_measured)
    missing_trace = sorted(declared_measured - traced)
    if unexpected_trace:
        formatted = ", ".join(
            f"{backend} 2^{exponent}" for backend, exponent in unexpected_trace
        )
        raise ReportError(
            "trace contains cells not declared measured by the campaign manifest: "
            + formatted
        )
    if missing_trace:
        formatted = ", ".join(
            f"{backend} 2^{exponent}" for backend, exponent in missing_trace
        )
        raise ReportError(
            "campaign manifest declares measured cells with no trace records: " + formatted
        )
    for key in sorted(declared_measured):
        backend, exponent = key
        if backend != "plonky3-whir":
            continue
        configs = {whir_config_from_run(run) for run in groups[key]}
        expected = (
            cells[key]["challenge_extension_degree"],
            cells[key]["configured_max_pow_bits"],
            cells[key]["derived_max_pow_bits"],
        )
        if configs != {expected}:
            raise ReportError(
                f"campaign and trace WHIR configuration differ for 2^{exponent}: "
                f"manifest={expected}, trace={sorted(configs)}"
            )


def union_duration(intervals: Iterable[tuple[int, int]]) -> int:
    ordered = sorted(intervals)
    if not ordered:
        return 0
    total = 0
    current_start, current_end = ordered[0]
    for start, end in ordered[1:]:
        if start <= current_end:
            current_end = max(current_end, end)
        else:
            total += current_end - current_start
            current_start, current_end = start, end
    return total + current_end - current_start


def operation_duration(spans: Sequence[dict[str, Any]], operation: str) -> int:
    intervals = (span_interval(span) for span in spans if span["operation"] == operation)
    return union_duration(intervals)


def operations_duration(spans: Sequence[dict[str, Any]], operations: set[str]) -> int:
    intervals = (span_interval(span) for span in spans if span["operation"] in operations)
    return union_duration(intervals)


def type7(values: Sequence[int], numerator: int, denominator: int = 10) -> float:
    """Hyndman--Fan Type 7 quantile for an exact rational probability."""
    if not values:
        raise ValueError("Type-7 quantile needs at least one value")
    ordered = sorted(values)
    if len(ordered) == 1:
        return float(ordered[0])
    position_numerator = (len(ordered) - 1) * numerator
    lower = position_numerator // denominator
    remainder = position_numerator % denominator
    upper = min(lower + 1, len(ordered) - 1)
    return (
        ordered[lower] * (denominator - remainder) + ordered[upper] * remainder
    ) / denominator


def statistics(values: Sequence[int], scale: int) -> dict[str, Any] | None:
    if not values:
        return None
    return {
        "p10": type7(values, 1) / scale,
        "median": type7(values, 5) / scale,
        "p90": type7(values, 9) / scale,
        "min": min(values) / scale,
        "max": max(values) / scale,
        "n": len(values),
    }


def empty_group(
    backend: str,
    exponent: int,
    *,
    cell_status: str = "missing",
    reason: str | None = None,
    required_pow_bits: int | None = None,
    budget: int | None = None,
    challenge_extension_degree: int | None = None,
    configured_max_pow_bits: int | None = None,
    derived_max_pow_bits: int | None = None,
) -> dict[str, Any]:
    return {
        "implementation": backend,
        "implementation_label": BACKEND_LABELS[backend],
        "log_multiplications": exponent,
        "cell_status": cell_status,
        "reason": reason,
        "required_pow_bits": required_pow_bits,
        "budget": budget,
        "challenge_extension_degree": challenge_extension_degree,
        "configured_max_pow_bits": configured_max_pow_bits,
        "derived_max_pow_bits": derived_max_pow_bits,
        "present": False,
        "series_id": None,
        "warmup_count": 0,
        "measured_count": 0,
        "eligible_count": 0,
        "excluded_samples": [],
        "timings_ms": {
            **{key: None for key, _, _ in TIMING_OPERATIONS},
            PCS_TOTAL_KEY: None,
        },
        "artifacts_bytes": {key: None for key, _ in ARTIFACT_METRICS},
    }


def aggregate_group(
    backend: str,
    exponent: int,
    group_runs: Sequence[dict[str, Any]],
    spans_by_run: dict[str, list[dict[str, Any]]],
) -> dict[str, Any]:
    configurations = {whir_config_from_run(run) for run in group_runs}
    if len(configurations) != 1:
        raise ReportError(
            f"{backend} 2^{exponent} mixes WHIR field or PoW configurations: "
            f"{sorted(configurations)}"
        )
    extension_degree, configured_pow, derived_pow = configurations.pop()
    samples = sorted(
        (run for run in group_runs if run["trial"]["kind"] == "sample"),
        key=lambda run: run["trial"]["sample_index"],
    )
    eligible = [run for run in samples if eligible_sample(run)]
    excluded = [
        {
            "run_id": run["run_id"],
            "sample_index": run["trial"]["sample_index"],
            "status": run["status"],
            "trace_complete": run["trace_complete"],
        }
        for run in samples
        if not eligible_sample(run)
    ]

    timing_values: dict[str, list[int]] = {
        key: [] for key, _, _ in TIMING_OPERATIONS
    }
    timing_values[PCS_TOTAL_KEY] = []
    artifact_values: dict[str, list[int]] = {key: [] for key, _ in ARTIFACT_METRICS}
    eligible_run_ids: list[str] = []

    for run in eligible:
        run_id = run["run_id"]
        eligible_run_ids.append(run_id)
        spans = spans_by_run[run_id]
        for key, operation, _ in TIMING_OPERATIONS:
            timing_values[key].append(operation_duration(spans, operation))
        # Compute this overlap-safe union for every individual trial before
        # taking any cross-trial quantile.
        timing_values[PCS_TOTAL_KEY].append(operations_duration(spans, PCS_TOTAL_OPERATIONS))
        artifacts = run["artifacts"]
        for key, _ in ARTIFACT_METRICS:
            artifact_values[key].append(artifacts[key])

    return {
        "implementation": backend,
        "implementation_label": BACKEND_LABELS[backend],
        "log_multiplications": exponent,
        "cell_status": "measured",
        "reason": None,
        "required_pow_bits": None,
        "budget": None,
        "challenge_extension_degree": extension_degree,
        "configured_max_pow_bits": configured_pow,
        "derived_max_pow_bits": derived_pow,
        "present": True,
        "series_id": group_runs[0]["series_id"],
        "warmup_count": 1,
        "measured_count": len(samples),
        "eligible_count": len(eligible),
        "eligible_run_ids": eligible_run_ids,
        "excluded_samples": excluded,
        "timings_ms": {
            key: statistics(values, 1_000_000) for key, values in timing_values.items()
        },
        "artifacts_bytes": {
            key: statistics(values, 1) for key, values in artifact_values.items()
        },
    }


def comparison_entry(
    bitz_stats: dict[str, Any] | None,
    whir_stats: dict[str, Any] | None,
    bitz_status: str,
    whir_status: str,
) -> dict[str, Any]:
    bitz_median = None if bitz_stats is None else bitz_stats["median"]
    whir_median = None if whir_stats is None else whir_stats["median"]
    lower: str | None = None
    if bitz_median is not None and whir_median is not None:
        if bitz_median < whir_median:
            lower = "bitz"
        elif whir_median < bitz_median:
            lower = "plonky3-whir"
        else:
            lower = "tie"
    return {
        "bitz_median": bitz_median,
        "plonky3_whir_median": whir_median,
        "bitz_status": bitz_status,
        "plonky3_whir_status": whir_status,
        "lower": lower,
    }


def build_summary(
    trace_path: Path,
    groups: dict[tuple[str, int], list[dict[str, Any]]],
    spans_by_run: dict[str, list[dict[str, Any]]],
    campaign_path: Path | None = None,
    campaign_id: str | None = None,
    campaign_cells: dict[tuple[str, int], dict[str, Any]] | None = None,
) -> dict[str, Any]:
    aggregated: list[dict[str, Any]] = []
    by_key: dict[tuple[str, int], dict[str, Any]] = {}
    for backend in BACKENDS:
        for exponent in EXPONENTS:
            group_runs = groups.get((backend, exponent))
            if group_runs:
                group = aggregate_group(backend, exponent, group_runs, spans_by_run)
            elif campaign_cells is not None:
                cell = campaign_cells[(backend, exponent)]
                group = empty_group(
                    backend,
                    exponent,
                    cell_status=cell["status"],
                    reason=cell["reason"],
                    required_pow_bits=cell["required_pow_bits"],
                    budget=cell["budget"],
                    challenge_extension_degree=cell["challenge_extension_degree"],
                    configured_max_pow_bits=cell["configured_max_pow_bits"],
                    derived_max_pow_bits=cell["derived_max_pow_bits"],
                )
            else:
                group = empty_group(backend, exponent)
            aggregated.append(group)
            by_key[(backend, exponent)] = group

    comparison: list[dict[str, Any]] = []
    timing_keys = [key for key, _, _ in TIMING_OPERATIONS] + [PCS_TOTAL_KEY]
    artifact_keys = [key for key, _ in ARTIFACT_METRICS]
    for exponent in EXPONENTS:
        bitz = by_key[("bitz", exponent)]
        whir = by_key[("plonky3-whir", exponent)]
        comparison.append(
            {
                "log_multiplications": exponent,
                "timings_ms": {
                    key: comparison_entry(
                        bitz["timings_ms"][key],
                        whir["timings_ms"][key],
                        bitz["cell_status"],
                        whir["cell_status"],
                    )
                    for key in timing_keys
                },
                "artifacts_bytes": {
                    key: comparison_entry(
                        bitz["artifacts_bytes"][key],
                        whir["artifacts_bytes"][key],
                        bitz["cell_status"],
                        whir["cell_status"],
                    )
                    for key in artifact_keys
                },
            }
        )

    excluded_count = sum(len(group["excluded_samples"]) for group in aggregated)
    eligible_count = sum(group["eligible_count"] for group in aggregated)
    return {
        "schema": REPORT_SCHEMA,
        "source_schema": TRACE_SCHEMA,
        "source_trace": str(trace_path.resolve()),
        "campaign": (
            None
            if campaign_path is None
            else {
                "schema": CAMPAIGN_SCHEMA,
                "id": campaign_id,
                "source": str(campaign_path.resolve()),
            }
        ),
        "generated_at": datetime.now(timezone.utc).isoformat(),
        "methodology": {
            "expected_log_multiplications": list(EXPONENTS),
            "warmups_per_present_backend_size": 1,
            "eligible_trial": "trial.kind == sample && status == ok && trace_complete == true",
            "quantiles": "Hyndman-Fan Type 7 sample quantiles",
            "pcs_total_prover": (
                "per-trial interval union of pcs_compare.materialize, "
                "pcs_compare.commit, and pcs_compare.opening"
            ),
            "timeline_report": "not generated; use zk_trace.py",
        },
        "eligible_sample_count": eligible_count,
        "excluded_sample_count": excluded_count,
        "groups": aggregated,
        "comparison": comparison,
    }


def number_text(value: float, decimals: int) -> str:
    if not math.isfinite(value):
        raise ReportError("non-finite aggregate encountered")
    return f"{value:,.{decimals}f}"


def timing_compact(stats: dict[str, Any] | None) -> str:
    if stats is None:
        return "missing"
    return (
        f"{number_text(stats['median'], 3)} "
        f"[{number_text(stats['p10'], 3)}–{number_text(stats['p90'], 3)}]"
    )


def byte_number(value: float) -> str:
    if float(value).is_integer():
        return f"{int(value):,}"
    return f"{value:,.1f}"


def bytes_compact(stats: dict[str, Any] | None) -> str:
    if stats is None:
        return "missing"
    return (
        f"{byte_number(stats['median'])} "
        f"[{byte_number(stats['p10'])}–{byte_number(stats['p90'])}]"
    )


def median_text(stats: dict[str, Any] | None, unit: str) -> str:
    if stats is None:
        return "missing"
    if unit == "ms":
        return f"{number_text(stats['median'], 3)} ms"
    return f"{byte_number(stats['median'])} B"


def cell_status_text(group: dict[str, Any]) -> str:
    status = group["cell_status"]
    if status == "unavailable":
        return "N/A (unavailable)"
    if status == "not_requested":
        return "not requested"
    return "missing"


def cell_status_detail(group: dict[str, Any]) -> str:
    parts = [cell_status_text(group)]
    if group.get("reason"):
        parts.append(group["reason"])
    required = group.get("required_pow_bits")
    budget = group.get("budget")
    if required is not None:
        parts.append(f"required PoW: {required} bits")
    if budget is not None:
        parts.append(f"PoW budget: {budget} bits")
    return "; ".join(parts)


def sample_count_text(group: dict[str, Any]) -> str:
    if group["cell_status"] != "measured":
        return "—"
    return str(group["eligible_count"])


def config_value_text(value: int | None) -> str:
    return "—" if value is None else str(value)


def whir_config_cells(group: dict[str, Any]) -> list[str]:
    return [
        config_value_text(group["challenge_extension_degree"]),
        config_value_text(group["configured_max_pow_bits"]),
        config_value_text(group["derived_max_pow_bits"]),
    ]


def markdown_backend_stat(
    stats: dict[str, Any] | None, group: dict[str, Any], unit: str
) -> str:
    if stats is None:
        return cell_status_text(group)
    return timing_compact(stats) if unit == "ms" else bytes_compact(stats)


def markdown_compare_cell(
    bitz_stats: dict[str, Any] | None,
    whir_stats: dict[str, Any] | None,
    bitz_group: dict[str, Any],
    whir_group: dict[str, Any],
    unit: str,
) -> str:
    left = (
        cell_status_text(bitz_group)
        if bitz_stats is None
        else median_text(bitz_stats, unit)
    )
    right = (
        cell_status_text(whir_group)
        if whir_stats is None
        else median_text(whir_stats, unit)
    )
    if bitz_stats is not None and whir_stats is not None:
        bitz_median = bitz_stats["median"]
        whir_median = whir_stats["median"]
        if bitz_median < whir_median:
            left = f"**{left}**"
        elif whir_median < bitz_median:
            right = f"**{right}**"
    return f"{left} / {right}"


def group_index(summary: dict[str, Any]) -> dict[tuple[str, int], dict[str, Any]]:
    return {
        (group["implementation"], group["log_multiplications"]): group
        for group in summary["groups"]
    }


def markdown_table(headers: Sequence[str], rows: Sequence[Sequence[str]]) -> str:
    lines = [
        "| " + " | ".join(headers) + " |",
        "| " + " | ".join("---" for _ in headers) + " |",
    ]
    lines.extend("| " + " | ".join(row) + " |" for row in rows)
    return "\n".join(lines)


def render_markdown(summary: dict[str, Any]) -> str:
    index = group_index(summary)
    timing_columns = [*TIMING_OPERATIONS, (PCS_TOTAL_KEY, "", PCS_TOTAL_LABEL)]
    lines = [
        "# BabyBear PCS comparison",
        "",
        f"Source: `{summary['source_trace']}`",
        "",
        (
            "Successful complete measured samples only. Values in backend tables are "
            "median [P10–P90]; sample quantiles use Hyndman–Fan Type 7. "
            "Min, max, and exact n are retained in `metrics.csv` and `summary.json`."
        ),
        "",
        (
            "`PCS total prover` is computed within each trial as the overlap-safe union of "
            "materialize, commit, and opening, before aggregation. Claim setup is reported "
            "separately and is not added again."
        ),
        "",
        (
            "`N/A (unavailable)` means the frozen backend configuration failed preflight; "
            "`not requested` reflects the campaign selection; `missing` means a cell was "
            "expected but has no trace data."
        ),
        "",
    ]

    for backend in BACKENDS:
        label = BACKEND_LABELS[backend]
        config_headers = (
            ["Challenge ext. degree", "PoW cap", "Derived PoW"]
            if backend == "plonky3-whir"
            else []
        )
        lines.extend([f"## {label}", "", "### Timings (ms)", ""])
        timing_rows: list[list[str]] = []
        for exponent in EXPONENTS:
            group = index[(backend, exponent)]
            timing_rows.append(
                [
                    f"2^{exponent}",
                    *(whir_config_cells(group) if backend == "plonky3-whir" else []),
                    *(
                        markdown_backend_stat(group["timings_ms"][key], group, "ms")
                        for key, _, _ in timing_columns
                    ),
                    sample_count_text(group),
                ]
            )
        lines.extend(
            [
                markdown_table(
                    [
                        "Multiplications",
                        *config_headers,
                        *(label for _, _, label in timing_columns),
                        "n",
                    ],
                    timing_rows,
                ),
                "",
                "### Wire sizes (bytes)",
                "",
            ]
        )
        size_rows: list[list[str]] = []
        for exponent in EXPONENTS:
            group = index[(backend, exponent)]
            size_rows.append(
                [
                    f"2^{exponent}",
                    *(whir_config_cells(group) if backend == "plonky3-whir" else []),
                    *(
                        markdown_backend_stat(group["artifacts_bytes"][key], group, "bytes")
                        for key, _ in ARTIFACT_METRICS
                    ),
                    sample_count_text(group),
                ]
            )
        lines.extend(
            [
                markdown_table(
                    [
                        "Multiplications",
                        *config_headers,
                        *(label for _, label in ARTIFACT_METRICS),
                        "n",
                    ],
                    size_rows,
                ),
                "",
            ]
        )

    lines.extend(
        [
            "## Combined comparison",
            "",
            "Every comparison cell is **BitZ / Plonky3 WHIR**; the lower median is bold.",
            "",
            "### Timing medians",
            "",
        ]
    )
    combined_timing_rows: list[list[str]] = []
    for exponent in EXPONENTS:
        bitz = index[("bitz", exponent)]
        whir = index[("plonky3-whir", exponent)]
        combined_timing_rows.append(
            [
                f"2^{exponent}",
                *whir_config_cells(whir),
                *(
                    markdown_compare_cell(
                        bitz["timings_ms"][key],
                        whir["timings_ms"][key],
                        bitz,
                        whir,
                        "ms",
                    )
                    for key, _, _ in timing_columns
                ),
                f"{sample_count_text(bitz)} / {sample_count_text(whir)}",
            ]
        )
    lines.extend(
        [
            markdown_table(
                [
                    "Multiplications",
                    "WHIR ext. degree",
                    "WHIR PoW cap",
                    "WHIR derived PoW",
                    *(label for _, _, label in timing_columns),
                    "n",
                ],
                combined_timing_rows,
            ),
            "",
            "### Wire-size medians",
            "",
        ]
    )
    combined_size_rows: list[list[str]] = []
    for exponent in EXPONENTS:
        bitz = index[("bitz", exponent)]
        whir = index[("plonky3-whir", exponent)]
        combined_size_rows.append(
            [
                f"2^{exponent}",
                *whir_config_cells(whir),
                *(
                    markdown_compare_cell(
                        bitz["artifacts_bytes"][key],
                        whir["artifacts_bytes"][key],
                        bitz,
                        whir,
                        "bytes",
                    )
                    for key, _ in ARTIFACT_METRICS
                ),
                f"{sample_count_text(bitz)} / {sample_count_text(whir)}",
            ]
        )
    lines.extend(
        [
            markdown_table(
                [
                    "Multiplications",
                    "WHIR ext. degree",
                    "WHIR PoW cap",
                    "WHIR derived PoW",
                    *(label for _, label in ARTIFACT_METRICS),
                    "n",
                ],
                combined_size_rows,
            ),
            "",
        ]
    )

    excluded = [
        (group["implementation_label"], group["log_multiplications"], sample)
        for group in summary["groups"]
        for sample in group["excluded_samples"]
    ]
    if excluded:
        lines.extend(["## Excluded measured runs", ""])
        for label, exponent, sample in excluded:
            lines.append(
                f"- `{sample['run_id']}` — {label} 2^{exponent}, "
                f"status={sample['status']}, trace_complete={str(sample['trace_complete']).lower()}"
            )
        lines.append("")

    unavailable = [group for group in summary["groups"] if group["cell_status"] == "unavailable"]
    if unavailable:
        lines.extend(["## Configuration-unavailable cells", ""])
        for group in unavailable:
            lines.append(
                f"- {group['implementation_label']} 2^{group['log_multiplications']}: "
                f"{cell_status_detail(group)}"
            )
        lines.append("")

    lines.extend(
        [
            "Timeline intervals are intentionally not rendered here; use the canonical "
            "`zk_trace.py` reporter for interval HTML.",
            "",
        ]
    )
    return "\n".join(lines)


def stats_title(stats: dict[str, Any]) -> str:
    return (
        f"P10 {stats['p10']}; median {stats['median']}; P90 {stats['p90']}; "
        f"min {stats['min']}; max {stats['max']}; n={stats['n']}"
    )


def html_backend_stat(
    stats: dict[str, Any] | None, group: dict[str, Any], unit: str
) -> str:
    if stats is None:
        label = html.escape(cell_status_text(group))
        detail = html.escape(cell_status_detail(group))
        css_class = "unavailable" if group["cell_status"] == "unavailable" else "missing"
        return f'<span class="{css_class}" title="{detail}">{label}</span>'
    compact = timing_compact(stats) if unit == "ms" else bytes_compact(stats)
    suffix = " ms" if unit == "ms" else " B"
    return (
        f'<span class="stat" title="{html.escape(stats_title(stats))}">'
        f"{html.escape(compact)}{suffix}</span>"
    )


def html_compare_cell(
    bitz_stats: dict[str, Any] | None,
    whir_stats: dict[str, Any] | None,
    bitz_group: dict[str, Any],
    whir_group: dict[str, Any],
    unit: str,
) -> str:
    def one(
        stats: dict[str, Any] | None, group: dict[str, Any]
    ) -> tuple[str, str]:
        if stats is None:
            label = html.escape(cell_status_text(group))
            css_class = "unavailable" if group["cell_status"] == "unavailable" else "missing"
            return f'<span class="{css_class}">{label}</span>', html.escape(
                cell_status_detail(group)
            )
        text = html.escape(median_text(stats, unit))
        return text, html.escape(stats_title(stats))

    left, left_title = one(bitz_stats, bitz_group)
    right, right_title = one(whir_stats, whir_group)
    if bitz_stats is not None and whir_stats is not None:
        if bitz_stats["median"] < whir_stats["median"]:
            left = f"<strong>{left}</strong>"
        elif whir_stats["median"] < bitz_stats["median"]:
            right = f"<strong>{right}</strong>"
    left_attr = f' title="{left_title}"' if left_title else ""
    right_attr = f' title="{right_title}"' if right_title else ""
    return (
        f'<span class="pair-left"{left_attr}>{left}</span>'
        '<span class="slash"> / </span>'
        f'<span class="pair-right"{right_attr}>{right}</span>'
    )


def html_table(headers: Sequence[str], rows: Sequence[Sequence[str]]) -> str:
    head = "".join(f"<th>{html.escape(header)}</th>" for header in headers)
    body = "".join(
        "<tr>" + "".join(f"<td>{cell}</td>" for cell in row) + "</tr>" for row in rows
    )
    return f"<div class=\"table-wrap\"><table><thead><tr>{head}</tr></thead><tbody>{body}</tbody></table></div>"


def render_html(summary: dict[str, Any]) -> str:
    index = group_index(summary)
    timing_columns = [*TIMING_OPERATIONS, (PCS_TOTAL_KEY, "", PCS_TOTAL_LABEL)]
    sections: list[str] = []

    for backend in BACKENDS:
        config_headers = (
            ["Challenge ext. degree", "PoW cap", "Derived PoW"]
            if backend == "plonky3-whir"
            else []
        )
        timing_rows: list[list[str]] = []
        size_rows: list[list[str]] = []
        for exponent in EXPONENTS:
            group = index[(backend, exponent)]
            timing_rows.append(
                [
                    f"2<sup>{exponent}</sup>",
                    *(whir_config_cells(group) if backend == "plonky3-whir" else []),
                    *(
                        html_backend_stat(group["timings_ms"][key], group, "ms")
                        for key, _, _ in timing_columns
                    ),
                    html.escape(sample_count_text(group)),
                ]
            )
            size_rows.append(
                [
                    f"2<sup>{exponent}</sup>",
                    *(whir_config_cells(group) if backend == "plonky3-whir" else []),
                    *(
                        html_backend_stat(group["artifacts_bytes"][key], group, "bytes")
                        for key, _ in ARTIFACT_METRICS
                    ),
                    html.escape(sample_count_text(group)),
                ]
            )
        sections.append(
            f"<section><h2>{html.escape(BACKEND_LABELS[backend])}</h2>"
            "<h3>Timings <span class=\"unit\">median [P10–P90], ms</span></h3>"
            + html_table(
                [
                    "Multiplications",
                    *config_headers,
                    *(label for _, _, label in timing_columns),
                    "n",
                ],
                timing_rows,
            )
            + "<h3>Wire sizes <span class=\"unit\">median [P10–P90], bytes</span></h3>"
            + html_table(
                [
                    "Multiplications",
                    *config_headers,
                    *(label for _, label in ARTIFACT_METRICS),
                    "n",
                ],
                size_rows,
            )
            + "</section>"
        )

    combined_timing_rows: list[list[str]] = []
    combined_size_rows: list[list[str]] = []
    for exponent in EXPONENTS:
        bitz = index[("bitz", exponent)]
        whir = index[("plonky3-whir", exponent)]
        combined_timing_rows.append(
            [
                f"2<sup>{exponent}</sup>",
                *whir_config_cells(whir),
                *(
                    html_compare_cell(
                        bitz["timings_ms"][key],
                        whir["timings_ms"][key],
                        bitz,
                        whir,
                        "ms",
                    )
                    for key, _, _ in timing_columns
                ),
                f"{html.escape(sample_count_text(bitz))} / {html.escape(sample_count_text(whir))}",
            ]
        )
        combined_size_rows.append(
            [
                f"2<sup>{exponent}</sup>",
                *whir_config_cells(whir),
                *(
                    html_compare_cell(
                        bitz["artifacts_bytes"][key],
                        whir["artifacts_bytes"][key],
                        bitz,
                        whir,
                        "bytes",
                    )
                    for key, _ in ARTIFACT_METRICS
                ),
                f"{html.escape(sample_count_text(bitz))} / {html.escape(sample_count_text(whir))}",
            ]
        )

    excluded = [
        (group["implementation_label"], group["log_multiplications"], sample)
        for group in summary["groups"]
        for sample in group["excluded_samples"]
    ]
    excluded_html = ""
    if excluded:
        items = "".join(
            "<li><code>"
            + html.escape(sample["run_id"])
            + "</code> — "
            + html.escape(label)
            + f" 2<sup>{exponent}</sup>, status="
            + html.escape(sample["status"])
            + ", trace_complete="
            + str(sample["trace_complete"]).lower()
            + "</li>"
            for label, exponent, sample in excluded
        )
        excluded_html = f"<section><h2>Excluded measured runs</h2><ul>{items}</ul></section>"

    unavailable = [group for group in summary["groups"] if group["cell_status"] == "unavailable"]
    unavailable_html = ""
    if unavailable:
        items = "".join(
            "<li>"
            + html.escape(group["implementation_label"])
            + f" 2<sup>{group['log_multiplications']}</sup>: "
            + html.escape(cell_status_detail(group))
            + "</li>"
            for group in unavailable
        )
        unavailable_html = (
            "<section><h2>Configuration-unavailable cells</h2>"
            f"<ul>{items}</ul></section>"
        )

    combined = (
        "<section><h2>Combined comparison</h2>"
        "<p>Every cell is <strong>BitZ / Plonky3 WHIR</strong>. The lower median is bold.</p>"
        "<h3>Timing medians</h3>"
        + html_table(
            [
                "Multiplications",
                "WHIR ext. degree",
                "WHIR PoW cap",
                "WHIR derived PoW",
                *(label for _, _, label in timing_columns),
                "n",
            ],
            combined_timing_rows,
        )
        + "<h3>Wire-size medians</h3>"
        + html_table(
            [
                "Multiplications",
                "WHIR ext. degree",
                "WHIR PoW cap",
                "WHIR derived PoW",
                *(label for _, label in ARTIFACT_METRICS),
                "n",
            ],
            combined_size_rows,
        )
        + "</section>"
    )

    return f"""<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>BabyBear PCS comparison</title>
<style>
:root {{ color-scheme: light; --ink:#172033; --muted:#667085; --line:#d9deea; --panel:#fff; --bg:#f5f7fb; --accent:#315ee7; }}
* {{ box-sizing:border-box; }}
body {{ margin:0; background:var(--bg); color:var(--ink); font:14px/1.45 system-ui,-apple-system,BlinkMacSystemFont,"Segoe UI",sans-serif; }}
main {{ max-width:1600px; margin:0 auto; padding:32px 24px 64px; }}
h1 {{ margin:0 0 8px; font-size:30px; letter-spacing:-.02em; }}
h2 {{ margin:0 0 16px; font-size:22px; }}
h3 {{ margin:24px 0 10px; font-size:15px; }}
p {{ color:var(--muted); max-width:1000px; }}
code {{ font:12px ui-monospace,SFMono-Regular,Menlo,monospace; overflow-wrap:anywhere; }}
.meta {{ margin-bottom:24px; }}
.cards {{ display:flex; gap:12px; flex-wrap:wrap; margin:20px 0 4px; }}
.card {{ min-width:180px; padding:14px 16px; background:var(--panel); border:1px solid var(--line); border-radius:12px; }}
.card b {{ display:block; font-size:24px; }}
.card span,.unit {{ color:var(--muted); font-weight:400; }}
section {{ margin-top:24px; padding:24px; background:var(--panel); border:1px solid var(--line); border-radius:16px; box-shadow:0 3px 14px rgba(28,39,64,.04); }}
.table-wrap {{ overflow:auto; border:1px solid var(--line); border-radius:10px; }}
table {{ width:100%; border-collapse:collapse; white-space:nowrap; font-variant-numeric:tabular-nums; }}
th,td {{ padding:10px 12px; text-align:right; border-bottom:1px solid var(--line); }}
th {{ position:sticky; top:0; background:#f8f9fc; color:#475467; font-size:12px; letter-spacing:.02em; }}
th:first-child,td:first-child {{ text-align:left; }}
tbody tr:last-child td {{ border-bottom:0; }}
tbody tr:hover {{ background:#f7f9ff; }}
strong {{ color:var(--accent); }}
.missing {{ color:#98a2b3; font-style:italic; }}
.unavailable {{ color:#b54708; font-style:italic; }}
.slash {{ color:#98a2b3; }}
ul {{ margin-bottom:0; }}
</style>
</head>
<body><main>
<header class="meta">
<h1>BabyBear PCS comparison</h1>
<p>BitZ and Plonky3 WHIR over 2<sup>15</sup> through 2<sup>24</sup> multiplication witnesses. Successful, complete measured runs only; P10/P50/P90 use Hyndman–Fan Type 7.</p>
<p>PCS total prover is an overlap-safe per-trial union of materialize, commit, and opening. Claim setup is shown separately. Timeline rendering is intentionally delegated to <code>zk_trace.py</code>.</p>
<p><strong>N/A (unavailable)</strong> means the frozen backend configuration failed preflight; <strong>not requested</strong> reflects campaign selection; <strong>missing</strong> means an expected cell has no trace data.</p>
<p>Source: <code>{html.escape(summary['source_trace'])}</code></p>
<div class="cards"><div class="card"><b>{summary['eligible_sample_count']}</b><span>eligible samples</span></div><div class="card"><b>{summary['excluded_sample_count']}</b><span>excluded samples</span></div></div>
</header>
{''.join(sections)}
{combined}
{excluded_html}
{unavailable_html}
</main></body></html>
"""


def csv_number(value: Any) -> str:
    if value is None:
        return ""
    if isinstance(value, int):
        return str(value)
    return format(float(value), ".12g")


def metric_status(group: dict[str, Any], stats: dict[str, Any] | None) -> str:
    if stats is not None:
        return "ok"
    if group["cell_status"] in {"unavailable", "not_requested"}:
        return group["cell_status"]
    return "missing"


def write_metrics_csv(path: Path, summary: dict[str, Any]) -> None:
    fieldnames = [
        "implementation",
        "log_multiplications",
        "metric",
        "unit",
        "status",
        "reason",
        "challenge_extension_degree",
        "configured_max_pow_bits",
        "derived_max_pow_bits",
        "required_pow_bits",
        "budget",
        "p10",
        "median",
        "p90",
        "min",
        "max",
        "n",
    ]
    with path.open("w", encoding="utf-8", newline="") as handle:
        writer = csv.DictWriter(handle, fieldnames=fieldnames)
        writer.writeheader()
        for group in summary["groups"]:
            timing_keys = [key for key, _, _ in TIMING_OPERATIONS] + [PCS_TOTAL_KEY]
            for key in timing_keys:
                stats = group["timings_ms"][key]
                status = metric_status(group, stats)
                writer.writerow(
                    {
                        "implementation": group["implementation"],
                        "log_multiplications": group["log_multiplications"],
                        "metric": key,
                        "unit": "ms",
                        "status": status,
                        "reason": group["reason"] or "",
                        "challenge_extension_degree": csv_number(
                            group["challenge_extension_degree"]
                        ),
                        "configured_max_pow_bits": csv_number(
                            group["configured_max_pow_bits"]
                        ),
                        "derived_max_pow_bits": csv_number(
                            group["derived_max_pow_bits"]
                        ),
                        "required_pow_bits": csv_number(group["required_pow_bits"]),
                        "budget": csv_number(group["budget"]),
                        "p10": csv_number(None if stats is None else stats["p10"]),
                        "median": csv_number(None if stats is None else stats["median"]),
                        "p90": csv_number(None if stats is None else stats["p90"]),
                        "min": csv_number(None if stats is None else stats["min"]),
                        "max": csv_number(None if stats is None else stats["max"]),
                        "n": "" if stats is None else stats["n"],
                    }
                )
            for key, _ in ARTIFACT_METRICS:
                stats = group["artifacts_bytes"][key]
                status = metric_status(group, stats)
                writer.writerow(
                    {
                        "implementation": group["implementation"],
                        "log_multiplications": group["log_multiplications"],
                        "metric": key,
                        "unit": "bytes",
                        "status": status,
                        "reason": group["reason"] or "",
                        "challenge_extension_degree": csv_number(
                            group["challenge_extension_degree"]
                        ),
                        "configured_max_pow_bits": csv_number(
                            group["configured_max_pow_bits"]
                        ),
                        "derived_max_pow_bits": csv_number(
                            group["derived_max_pow_bits"]
                        ),
                        "required_pow_bits": csv_number(group["required_pow_bits"]),
                        "budget": csv_number(group["budget"]),
                        "p10": csv_number(None if stats is None else stats["p10"]),
                        "median": csv_number(None if stats is None else stats["median"]),
                        "p90": csv_number(None if stats is None else stats["p90"]),
                        "min": csv_number(None if stats is None else stats["min"]),
                        "max": csv_number(None if stats is None else stats["max"]),
                        "n": "" if stats is None else stats["n"],
                    }
                )


def write_outputs(out_dir: Path, summary: dict[str, Any], force: bool) -> None:
    if out_dir.exists() and not out_dir.is_dir():
        raise ReportError(f"output path exists and is not a directory: {out_dir}")
    targets = {name: out_dir / name for name in OUTPUT_NAMES}
    source_trace = Path(summary["source_trace"])
    for target in targets.values():
        if target.resolve() == source_trace:
            raise ReportError(f"report target would overwrite the source trace: {target}")
    existing = [path for path in targets.values() if path.exists()]
    if existing and not force:
        joined = ", ".join(str(path) for path in existing)
        raise ReportError(f"refusing to overwrite existing report files: {joined}; pass --force")
    out_dir.mkdir(parents=True, exist_ok=True)

    targets["summary.json"].write_text(
        json.dumps(summary, indent=2, sort_keys=False) + "\n", encoding="utf-8"
    )
    write_metrics_csv(targets["metrics.csv"], summary)
    targets["comparison.md"].write_text(render_markdown(summary), encoding="utf-8")
    targets["comparison.html"].write_text(render_html(summary), encoding="utf-8")


def main(argv: Sequence[str] | None = None) -> int:
    args = parse_args(argv)
    try:
        campaign_cells = None
        campaign_id = None
        if args.campaign is not None:
            campaign_id, campaign_cells = read_campaign(args.campaign)
        allow_empty = campaign_cells is not None and not any(
            cell["status"] == "measured" for cell in campaign_cells.values()
        )
        runs, spans_by_run = read_trace(args.trace, allow_empty=allow_empty)
        validate_trace_provenance(runs, campaign_id)
        groups = validate_campaign(runs, spans_by_run)
        if campaign_cells is not None:
            validate_manifest_trace_consistency(campaign_cells, groups)
        summary = build_summary(
            args.trace,
            groups,
            spans_by_run,
            campaign_path=args.campaign,
            campaign_id=campaign_id,
            campaign_cells=campaign_cells,
        )
        write_outputs(args.out_dir, summary, args.force)
    except (OSError, ReportError) as error:
        print(f"error: {error}", file=sys.stderr)
        return 2

    print(f"wrote {args.out_dir / 'summary.json'}")
    print(f"wrote {args.out_dir / 'metrics.csv'}")
    print(f"wrote {args.out_dir / 'comparison.md'}")
    print(f"wrote {args.out_dir / 'comparison.html'}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

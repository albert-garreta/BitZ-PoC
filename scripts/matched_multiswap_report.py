#!/usr/bin/env python3
"""Validate and report a matched F2Z/Limber MultiSwap campaign.

The input manifest points at immutable ``zkperf.trace/v1`` JSONL files.  This
module is deliberately dependency-free so a campaign can be checked and
rendered on a clean machine with only Python 3.
"""

from __future__ import annotations

import argparse
import csv
import hashlib
import html
import json
import re
import sys
from collections import defaultdict
from dataclasses import dataclass
from decimal import Decimal
from pathlib import Path
from typing import Any, Iterable, Sequence


TRACE_SCHEMA = "zkperf.trace/v1"
CAMPAIGN_SCHEMA = "matched-multiswap-campaign/v1"
REPORT_SCHEMA = "matched-multiswap-report/v1"
OUTPUT_NAMES = ("summary.json", "metrics.csv", "intervals.html")
WORKLOAD_ID = "multiswap-rsa-wired-cost-model-v1"
STATEMENT_DOMAIN = "f2z/multiswap/circuit-digest/v1"
ASSIGNMENT_DOMAIN = "f2z/multiswap/integer-assignment/v1"

CONTROLLED_PHASES = {
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

HEADLINES = (
    ("witness", "Witness generation"),
    ("projection", "Projection / field reduction"),
    ("piop", "PIOP / relation reduction"),
    ("pcs_opening", "PCS opening proof"),
    ("commit", "Commit"),
    ("pcs_total", "PCS total"),
    ("prover", "Total prover"),
    ("application_total", "Application total"),
    ("verify", "Verify"),
    ("end_to_end", "Verified trial"),
)

LIMBER_PIOP_OPERATIONS = {
    "limber.piop.outer_sumcheck",
    "limber.piop.inner_setup",
    "limber.piop.inner_sumcheck",
    "limber.piop.eval_recover",
}

LIMBER_PROJECTION_OPERATIONS = {
    "limber.projection.sample_prime",
    "limber.projection.reduce",
    "limber.projection.spmv",
}

FALLBACK_MATH: tuple[tuple[re.Pattern[str], tuple[str, ...]], ...] = (
    (
        re.compile(r"witness", re.I),
        (r"\mathbf z=(\mathbf W,1,\mathbf x)",),
    ),
    (
        re.compile(r"outer.*sumcheck|spartan", re.I),
        (
            r"\sum_{u\in\{0,1\}^n}\widetilde{eq}(\tau,u)"
            r"(\widetilde{Az}\widetilde{Bz}-\widetilde{Cz}-"
            r"\widetilde m\widetilde Q)=0",
        ),
    ),
    (
        re.compile(r"project|sample[_ -]?prime|\.reduce$", re.I),
        (
            r"p\leftarrow\operatorname{FS}(C_W,C_Q,\mathbf x)",
            r"A\mathbf z\circ B\mathbf z=C\mathbf z+\mathbf m\circ\mathbf Q"
            r"\quad\text{over }\mathbb F_p",
        ),
    ),
    (
        re.compile(r"inner.*sumcheck|spmv", re.I),
        (r"\widetilde{Mz}(r)=\sum_j \widetilde M(r,j)\widetilde z(j)",),
    ),
    (
        re.compile(r"logup|range", re.I),
        (
            r"\sum_b\sum_i(r+w_{b,i})^{-1}="
            r"\sum_{j=0}^{2^{16}-1}m_j(r+j)^{-1}",
        ),
    ),
    (
        re.compile(r"opening|pcs", re.I),
        (r"\sum_i\lambda^i C(z_i)=\sum_i\lambda^i v_i",),
    ),
    (
        re.compile(r"commit", re.I),
        (r"C_W\leftarrow\operatorname{Commit}(\mathbf W)",),
    ),
    (
        re.compile(r"trial|prover|proof", re.I),
        (
            r"A\mathbf z\circ B\mathbf z="
            r"C\mathbf z+\mathbf m\circ\mathbf Q",
        ),
    ),
)


class CampaignError(Exception):
    """A stable, user-facing campaign validation error."""


@dataclass(frozen=True)
class CellTrace:
    manifest_cell: dict[str, Any]
    path: Path
    runs: list[dict[str, Any]]
    spans_by_run: dict[str, list[dict[str, Any]]]
    source_sha256: str
    statement_domain: str
    statement_digest: str
    assignment_digest: str | None
    assignment_domain: str
    workload_id: str
    workload_k: int
    relation_shape: tuple[int, int, int, int, int, int, int]


def _pairs_no_duplicates(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    result: dict[str, Any] = {}
    for key, value in pairs:
        if key in result:
            raise CampaignError(f"duplicate JSON key {key!r}")
        result[key] = value
    return result


def _object(value: Any, context: str) -> dict[str, Any]:
    if not isinstance(value, dict):
        raise CampaignError(f"{context} must be a JSON object")
    return value


def _string(value: Any, context: str) -> str:
    if not isinstance(value, str) or not value:
        raise CampaignError(f"{context} must be a non-empty string")
    return value


def _positive_int(value: Any, context: str, *, allow_zero: bool = False) -> int:
    lower = 0 if allow_zero else 1
    if isinstance(value, bool) or not isinstance(value, int) or value < lower:
        adjective = "non-negative" if allow_zero else "positive"
        raise CampaignError(f"{context} must be a {adjective} integer")
    return value


def _ns(value: Any, context: str) -> int:
    if not isinstance(value, str) or not value.isascii() or not value.isdecimal():
        raise CampaignError(f"{context} must be an exact unsigned decimal string")
    return int(value)


def _blake3(value: Any, context: str) -> str:
    digest = _string(value, context)
    if not re.fullmatch(r"[0-9a-f]{64}", digest):
        raise CampaignError(f"{context} must be 64 lowercase hexadecimal characters")
    return digest


def _read_json(path: Path, context: str) -> dict[str, Any]:
    try:
        return _object(
            json.loads(path.read_text(encoding="utf-8"), object_pairs_hook=_pairs_no_duplicates),
            context,
        )
    except (OSError, UnicodeDecodeError, json.JSONDecodeError) as error:
        raise CampaignError(f"cannot read {context} {path}: {error}") from error


def read_manifest(path: Path) -> dict[str, Any]:
    manifest = _read_json(path, "campaign manifest")
    if manifest.get("schema") != CAMPAIGN_SCHEMA:
        raise CampaignError(
            f"campaign schema must be {CAMPAIGN_SCHEMA!r}; got {manifest.get('schema')!r}"
        )
    if manifest.get("trace_schema") != TRACE_SCHEMA:
        raise CampaignError(
            f"campaign trace_schema must be {TRACE_SCHEMA!r}; "
            f"got {manifest.get('trace_schema')!r}"
        )
    _string(manifest.get("campaign_id"), "campaign_id")
    sampling = _object(manifest.get("sampling"), "sampling")
    _positive_int(sampling.get("warmups"), "sampling.warmups", allow_zero=True)
    _positive_int(sampling.get("samples"), "sampling.samples")
    cells = manifest.get("cells")
    if not isinstance(cells, list) or not cells:
        raise CampaignError("campaign cells must be a non-empty array")
    workload = _object(manifest.get("workload"), "workload")
    k_values = workload.get("workload_k_values")
    if (
        not isinstance(k_values, list)
        or not k_values
        or any(isinstance(value, bool) or not isinstance(value, int) or value < 0 for value in k_values)
        or len(k_values) != len(set(k_values))
    ):
        raise CampaignError("workload.workload_k_values must be unique non-negative integers")
    seen: set[str] = set()
    execution_indices: set[int] = set()
    for index, raw_cell in enumerate(cells):
        cell = _object(raw_cell, f"cells[{index}]")
        cell_id = _string(cell.get("cell_id"), f"cells[{index}].cell_id")
        if cell_id in seen:
            raise CampaignError(f"duplicate campaign cell_id {cell_id!r}")
        seen.add(cell_id)
        execution_index = _positive_int(
            cell.get("execution_index"),
            f"cells[{index}].execution_index",
            allow_zero=True,
        )
        if execution_index != index or execution_index in execution_indices:
            raise CampaignError(
                "cells must be stored once each in ascending execution_index order"
            )
        execution_indices.add(execution_index)
        if cell.get("status") != "ok":
            raise CampaignError(
                f"cell {cell_id!r} has status {cell.get('status')!r}, not 'ok'"
            )
        _positive_int(cell.get("rayon_threads"), f"cell {cell_id}.rayon_threads")
        workload_k = _positive_int(
            cell.get("workload_k"), f"cell {cell_id}.workload_k", allow_zero=True
        )
        if workload_k not in k_values:
            raise CampaignError(f"cell {cell_id} workload_k is outside the configured sweep")
        _string(cell.get("trace"), f"cell {cell_id}.trace")
        _string(cell.get("trace_sha256"), f"cell {cell_id}.trace_sha256")
    expected_cells = {
        f"k{workload_k}-{suffix}"
        for workload_k in k_values
        for suffix in (
            "f2z-single",
            "f2z-performance",
            "limber-hyrax-single",
            "limber-hyrax-performance",
            "limber-brakedown-single",
            "limber-brakedown-performance",
        )
    }
    if seen != expected_cells:
        raise CampaignError(
            f"campaign must contain the six fixed cells; missing={sorted(expected_cells - seen)}, "
            f"unexpected={sorted(seen - expected_cells)}"
        )
    execution_order = _object(manifest.get("execution_order"), "execution_order")
    if execution_order.get("cell_ids") != [cell["cell_id"] for cell in cells]:
        raise CampaignError("execution_order.cell_ids must match manifest cell order")
    return manifest


def _extract_statement(run: dict[str, Any], run_id: str) -> tuple[str, str]:
    parameters = _object(run.get("parameters"), f"run {run_id}.parameters")
    input_parameters = parameters.get("input")
    input_object = input_parameters if isinstance(input_parameters, dict) else parameters
    statement = input_object.get("statement")
    if not isinstance(statement, dict):
        statement = parameters.get("statement")
    statement = statement if isinstance(statement, dict) else {}
    domain = (
        input_object.get("constraint_digest_domain")
        or parameters.get("constraint_digest_domain")
        or statement.get("domain")
        or statement.get("constraint_digest_domain")
    )
    digest = (
        input_object.get("constraint_digest_blake3")
        or parameters.get("constraint_digest_blake3")
        or statement.get("digest_blake3")
        or statement.get("constraint_digest_blake3")
    )
    return (
        _string(domain, f"run {run_id} canonical statement domain"),
        _blake3(digest, f"run {run_id} canonical statement digest"),
    )


def _extract_assignment_digest(run: dict[str, Any], run_id: str) -> str | None:
    parameters = run.get("parameters", {})
    candidates: list[Any] = []
    if isinstance(parameters, dict):
        for container in (parameters, parameters.get("input"), parameters.get("statement")):
            if isinstance(container, dict):
                candidates.extend(
                    [
                        container.get("assignment_digest_blake3"),
                        container.get("witness_digest_blake3"),
                    ]
                )
                witness_stats = container.get("witness_stats")
                if isinstance(witness_stats, dict):
                    candidates.append(witness_stats.get("assignment_digest_blake3"))
        witness_stats = parameters.get("witness_stats")
        if isinstance(witness_stats, dict):
            candidates.append(witness_stats.get("assignment_digest_blake3"))
    validation = run.get("validation")
    if isinstance(validation, dict):
        candidates.append(validation.get("assignment_digest_blake3"))
    for candidate in candidates:
        if isinstance(candidate, str) and candidate:
            return _blake3(candidate, f"run {run_id} assignment digest")
    return None


def _extract_assignment_domain(run: dict[str, Any], run_id: str) -> str:
    parameters = _object(run.get("parameters"), f"run {run_id}.parameters")
    candidates: list[Any] = []
    for container in (parameters, parameters.get("input"), parameters.get("statement")):
        if isinstance(container, dict):
            candidates.append(container.get("assignment_digest_domain"))
            witness_stats = container.get("witness_stats")
            if isinstance(witness_stats, dict):
                candidates.append(witness_stats.get("assignment_digest_domain"))
    for candidate in candidates:
        if isinstance(candidate, str) and candidate:
            return candidate
    raise CampaignError(f"run {run_id} omits assignment digest domain")


def _extract_relation_shape(
    run: dict[str, Any], run_id: str
) -> tuple[int, int, int, int, int, int, int]:
    parameters = _object(run.get("parameters"), f"run {run_id}.parameters")
    values = _object(parameters.get("input"), f"run {run_id}.parameters.input")

    def dimension(canonical: str, *aliases: str) -> int:
        value: Any = None
        for key in (canonical, *aliases):
            if key in values:
                value = values[key]
                break
        return _positive_int(value, f"run {run_id}.parameters.input.{canonical}")

    return (
        dimension("live_rows"),
        dimension("live_columns", "live_cols"),
        dimension("padded_rows", "num_cons"),
        dimension("padded_columns", "num_vars"),
        dimension("nnz_a", "a_nnz"),
        dimension("nnz_b", "b_nnz"),
        dimension("nnz_c", "c_nnz"),
    )


def _extract_workload_id(run: dict[str, Any], run_id: str) -> str:
    parameters = _object(run.get("parameters"), f"run {run_id}.parameters")
    input_parameters = parameters.get("input")
    value = input_parameters.get("workload_id") if isinstance(input_parameters, dict) else None
    if value is None:
        value = parameters.get("workload_id")
    workload_id = _string(value, f"run {run_id}.parameters.input.workload_id")
    if workload_id != WORKLOAD_ID:
        raise CampaignError(
            f"run {run_id} workload_id must be {WORKLOAD_ID!r}; got {workload_id!r}"
        )
    return workload_id


def _extract_workload_k(run: dict[str, Any], run_id: str) -> int:
    parameters = _object(run.get("parameters"), f"run {run_id}.parameters")
    input_parameters = _object(
        parameters.get("input"), f"run {run_id}.parameters.input"
    )
    value = input_parameters.get("workload_k", input_parameters.get("limber_k"))
    if isinstance(value, str) and value.isdecimal():
        value = int(value)
    return _positive_int(
        value, f"run {run_id}.parameters.input.workload_k", allow_zero=True
    )


def _reported_threads(run: dict[str, Any], run_id: str) -> int:
    environment = _object(run.get("environment"), f"run {run_id}.environment")
    value = environment.get("threads")
    if isinstance(value, str) and value.isdecimal():
        value = int(value)
    return _positive_int(value, f"run {run_id}.environment.threads")


def _validate_proof(run: dict[str, Any], run_id: str) -> None:
    if run.get("status") != "ok":
        raise CampaignError(f"run {run_id} status is {run.get('status')!r}, not 'ok'")
    if run.get("trace_complete") is not True:
        raise CampaignError(f"run {run_id} is not trace_complete")
    validation = _object(run.get("validation"), f"run {run_id}.validation")
    proof_verified = validation.get("proof_verified")
    if proof_verified is not True:
        raise CampaignError(f"run {run_id} does not report proof_verified=true")
    for key, value in validation.items():
        assertion_key = any(
            marker in key
            for marker in (
                "_satisfied",
                "_matches_",
                "_verified",
                "_valid",
                "_preflight",
                "verification_passed",
            )
        )
        if assertion_key and value is not True:
            raise CampaignError(f"run {run_id} validation.{key} is not true")


def load_cell_trace(
    manifest_path: Path,
    cell: dict[str, Any],
    warmups: int,
    samples: int,
) -> CellTrace:
    cell_id = _string(cell.get("cell_id"), "cell_id")
    trace_path = (manifest_path.parent / _string(cell.get("trace"), f"cell {cell_id}.trace")).resolve()
    try:
        raw = trace_path.read_bytes()
        text = raw.decode("utf-8")
    except (OSError, UnicodeDecodeError) as error:
        raise CampaignError(f"cannot read trace for cell {cell_id} at {trace_path}: {error}") from error

    runs: list[dict[str, Any]] = []
    spans_by_run: dict[str, list[dict[str, Any]]] = defaultdict(list)
    run_ids: set[str] = set()
    span_ids: dict[str, set[str]] = defaultdict(set)
    for line_number, line in enumerate(text.splitlines(), 1):
        where = f"{trace_path}:{line_number}"
        if not line.strip():
            raise CampaignError(f"{where}: blank JSONL lines are forbidden")
        try:
            record = _object(
                json.loads(line, object_pairs_hook=_pairs_no_duplicates), where
            )
        except json.JSONDecodeError as error:
            raise CampaignError(f"{where}: invalid JSON: {error}") from error
        if record.get("schema") != TRACE_SCHEMA:
            raise CampaignError(f"{where}: schema must be {TRACE_SCHEMA!r}")
        kind = record.get("record")
        run_id = _string(record.get("run_id"), f"{where}.run_id")
        if kind == "run":
            if run_id in run_ids:
                raise CampaignError(f"{where}: duplicate run_id {run_id!r}")
            run_ids.add(run_id)
            runs.append(record)
        elif kind == "span":
            span_id = _string(record.get("span_id"), f"{where}.span_id")
            if span_id in span_ids[run_id]:
                raise CampaignError(f"{where}: duplicate span_id {span_id!r}")
            span_ids[run_id].add(span_id)
            spans_by_run[run_id].append(record)
        else:
            raise CampaignError(f"{where}: record must be 'run' or 'span'")
    if not runs:
        raise CampaignError(f"cell {cell_id} trace contains no runs")
    orphan_runs = set(spans_by_run) - run_ids
    if orphan_runs:
        raise CampaignError(f"cell {cell_id} has spans for missing runs: {sorted(orphan_runs)}")

    trial_keys: set[tuple[str, str, int]] = set()
    series: set[str] = set()
    domains: set[str] = set()
    digests: set[str] = set()
    assignments: set[str] = set()
    assignment_domains: set[str] = set()
    workload_ids: set[str] = set()
    workload_ks: set[int] = set()
    relation_shapes: set[tuple[int, int, int, int, int, int, int]] = set()
    count_by_kind = defaultdict(int)
    for run in runs:
        run_id = _string(run.get("run_id"), f"cell {cell_id} run_id")
        series_id = _string(run.get("series_id"), f"run {run_id}.series_id")
        series.add(series_id)
        trial = _object(run.get("trial"), f"run {run_id}.trial")
        kind = trial.get("kind")
        index_key = "warmup_index" if kind == "warmup" else "sample_index" if kind == "sample" else None
        if index_key is None:
            raise CampaignError(f"run {run_id}.trial.kind must be warmup or sample")
        index = _positive_int(trial.get(index_key), f"run {run_id}.trial.{index_key}", allow_zero=True)
        key = (series_id, kind, index)
        if key in trial_keys:
            raise CampaignError(f"cell {cell_id} has duplicate trial {key}")
        trial_keys.add(key)
        count_by_kind[kind] += 1
        clock = _object(run.get("clock"), f"run {run_id}.clock")
        if clock.get("kind") != "monotonic" or clock.get("unit") != "ns":
            raise CampaignError(f"run {run_id} clock must be monotonic nanoseconds")
        _validate_proof(run, run_id)
        benchmark = _object(run.get("benchmark"), f"run {run_id}.benchmark")
        _string(benchmark.get("name"), f"run {run_id}.benchmark.name")
        implementation = _string(
            benchmark.get("implementation"), f"run {run_id}.benchmark.implementation"
        )
        if implementation != cell.get("implementation"):
            raise CampaignError(
                f"run {run_id} implementation {implementation!r} does not match cell "
                f"{cell.get('implementation')!r}"
            )
        threads = _reported_threads(run, run_id)
        if threads != cell.get("rayon_threads"):
            raise CampaignError(
                f"run {run_id} reports {threads} threads; cell expects "
                f"{cell.get('rayon_threads')}"
            )
        workload_ids.add(_extract_workload_id(run, run_id))
        workload_ks.add(_extract_workload_k(run, run_id))
        relation_shapes.add(_extract_relation_shape(run, run_id))
        domain, digest = _extract_statement(run, run_id)
        if domain != STATEMENT_DOMAIN:
            raise CampaignError(
                f"run {run_id} statement domain must be {STATEMENT_DOMAIN!r}; got {domain!r}"
            )
        domains.add(domain)
        digests.add(digest)
        assignment_domain = _extract_assignment_domain(run, run_id)
        if assignment_domain != ASSIGNMENT_DOMAIN:
            raise CampaignError(
                f"run {run_id} assignment domain must be {ASSIGNMENT_DOMAIN!r}; "
                f"got {assignment_domain!r}"
            )
        assignment_domains.add(assignment_domain)
        assignment = _extract_assignment_digest(run, run_id)
        if assignment is not None:
            assignments.add(assignment)

        spans = spans_by_run.get(run_id, [])
        if not spans:
            raise CampaignError(f"run {run_id} contains no spans")
        by_id: dict[str, dict[str, Any]] = {}
        for span in spans:
            span_id = _string(span.get("span_id"), f"run {run_id} span_id")
            by_id[span_id] = span
            operation = _string(span.get("operation"), f"span {span_id}.operation")
            if not re.fullmatch(r"[a-z0-9]+(?:[._-][a-z0-9]+)*", operation):
                raise CampaignError(f"span {span_id}.operation is not a stable operation ID")
            _string(span.get("name"), f"span {span_id}.name")
            tags = span.get("phase_tags")
            if not isinstance(tags, list) or not tags or any(not isinstance(tag, str) for tag in tags):
                raise CampaignError(f"span {span_id}.phase_tags must be a non-empty string array")
            unknown = set(tags) - CONTROLLED_PHASES
            if unknown:
                raise CampaignError(f"span {span_id} has unknown phase tags {sorted(unknown)}")
            primary = _string(span.get("primary_phase"), f"span {span_id}.primary_phase")
            if primary not in tags:
                raise CampaignError(f"span {span_id}.primary_phase must occur in phase_tags")
            start = _ns(span.get("start_ns"), f"span {span_id}.start_ns")
            end = _ns(span.get("end_ns"), f"span {span_id}.end_ns")
            duration = _ns(span.get("duration_ns"), f"span {span_id}.duration_ns")
            if end < start or duration != end - start:
                raise CampaignError(f"span {span_id} has inconsistent timestamps")
            span["_start"] = start
            span["_end"] = end
            span["_duration"] = duration
        root_id = _string(run.get("root_span_id"), f"run {run_id}.root_span_id")
        root = by_id.get(root_id)
        if root is None or root.get("parent_span_id") is not None:
            raise CampaignError(f"run {run_id} has no parentless root span {root_id!r}")
        for span in spans:
            if span is root:
                continue
            parent = by_id.get(span.get("parent_span_id"))
            if parent is None:
                raise CampaignError(f"span {span['span_id']} has a missing parent")
            if not (parent["_start"] <= span["_start"] <= span["_end"] <= parent["_end"]):
                raise CampaignError(f"span {span['span_id']} is not contained by its parent")
        for span in spans:
            visited: set[str] = set()
            cursor = span
            while cursor.get("parent_span_id") is not None:
                cursor_id = cursor["span_id"]
                if cursor_id in visited:
                    raise CampaignError(f"run {run_id} contains a parent cycle")
                visited.add(cursor_id)
                cursor = by_id[cursor["parent_span_id"]]
            if cursor is not root:
                raise CampaignError(f"span {span['span_id']} does not descend from the root")

    if len(series) != 1:
        raise CampaignError(f"cell {cell_id} must contain exactly one series; got {sorted(series)}")
    if count_by_kind["warmup"] != warmups or count_by_kind["sample"] != samples:
        raise CampaignError(
            f"cell {cell_id} has {count_by_kind['warmup']} warmups and "
            f"{count_by_kind['sample']} samples; expected {warmups} and {samples}"
        )
    if len(domains) != 1 or len(digests) != 1:
        raise CampaignError(f"cell {cell_id} has canonical statement metadata drift")
    if len(assignments) > 1:
        raise CampaignError(f"cell {cell_id} has assignment digest drift")
    if len(assignment_domains) != 1:
        raise CampaignError(f"cell {cell_id} has assignment domain drift")
    if len(workload_ids) != 1:
        raise CampaignError(f"cell {cell_id} has workload_id drift")
    if workload_ks != {cell.get("workload_k")}:
        raise CampaignError(
            f"cell {cell_id} trace workload k {sorted(workload_ks)} does not match "
            f"manifest k={cell.get('workload_k')}"
        )
    if len(relation_shapes) != 1:
        raise CampaignError(f"cell {cell_id} has relation dimensions/nnz drift")
    source_sha256 = hashlib.sha256(raw).hexdigest()
    manifest_sha256 = cell.get("trace_sha256")
    if manifest_sha256 is not None and manifest_sha256 != source_sha256:
        raise CampaignError(
            f"cell {cell_id} trace SHA-256 does not match campaign manifest"
        )
    return CellTrace(
        manifest_cell=cell,
        path=trace_path,
        runs=runs,
        spans_by_run=dict(spans_by_run),
        source_sha256=source_sha256,
        statement_domain=next(iter(domains)),
        statement_digest=next(iter(digests)),
        assignment_digest=next(iter(assignments)) if assignments else None,
        assignment_domain=next(iter(assignment_domains)),
        workload_id=next(iter(workload_ids)),
        workload_k=next(iter(workload_ks)),
        relation_shape=next(iter(relation_shapes)),
    )


def validate_campaign(manifest_path: Path) -> tuple[dict[str, Any], list[CellTrace]]:
    manifest = read_manifest(manifest_path)
    sampling = manifest["sampling"]
    cells = [
        load_cell_trace(
            manifest_path,
            _object(cell, "campaign cell"),
            sampling["warmups"],
            sampling["samples"],
        )
        for cell in manifest["cells"]
    ]
    expected_by_k = manifest.get("canonical_statements", {})
    if not isinstance(expected_by_k, dict):
        raise CampaignError("canonical_statements must be an object keyed by workload k")
    for workload_k in manifest["workload"]["workload_k_values"]:
        group = [cell for cell in cells if cell.workload_k == workload_k]
        domains = {cell.statement_domain for cell in group}
        digests = {cell.statement_digest for cell in group}
        if len(domains) != 1 or len(digests) != 1:
            details = ", ".join(
                f"{cell.manifest_cell['cell_id']}={cell.statement_domain}:{cell.statement_digest}"
                for cell in group
            )
            raise CampaignError(
                f"canonical statement digest mismatch within k={workload_k}: {details}"
            )
        expected = expected_by_k.get(str(workload_k))
        if expected is not None:
            expected = _object(expected, f"canonical_statements[{workload_k}]")
            if (
                expected.get("domain") != next(iter(domains))
                or expected.get("digest_blake3") != next(iter(digests))
            ):
                raise CampaignError(
                    f"manifest canonical statement does not match k={workload_k} traces"
                )
        if any(cell.assignment_digest is None for cell in group):
            raise CampaignError(f"one or more k={workload_k} cells omit assignment digest")
        assignment_values = {cell.assignment_digest for cell in group}
        if len(assignment_values) != 1:
            details = ", ".join(
                f"{cell.manifest_cell['cell_id']}={cell.assignment_digest or 'missing'}"
                for cell in group
            )
            raise CampaignError(
                f"integer assignment digest mismatch within k={workload_k}: {details}"
            )
        workload_ids = {cell.workload_id for cell in group}
        if len(workload_ids) != 1:
            details = ", ".join(
                f"{cell.manifest_cell['cell_id']}={cell.workload_id}" for cell in group
            )
            raise CampaignError(f"workload_id mismatch within k={workload_k}: {details}")
        relation_shapes = {cell.relation_shape for cell in group}
        if len(relation_shapes) != 1:
            labels = (
                "live_rows",
                "live_columns",
                "padded_rows",
                "padded_columns",
                "nnz_a",
                "nnz_b",
                "nnz_c",
            )
            details = ", ".join(
                f"{cell.manifest_cell['cell_id']}="
                + str(dict(zip(labels, cell.relation_shape, strict=True)))
                for cell in group
            )
            raise CampaignError(
                f"relation dimensions/nnz mismatch within k={workload_k}: {details}"
            )
    return manifest, cells


def _union_duration(intervals: Iterable[tuple[int, int]]) -> int:
    ordered = sorted(intervals)
    if not ordered:
        return 0
    total = 0
    start, end = ordered[0]
    for next_start, next_end in ordered[1:]:
        if next_start <= end:
            end = max(end, next_end)
        else:
            total += end - start
            start, end = next_start, next_end
    return total + end - start


def _merged_intervals(intervals: Iterable[tuple[int, int]]) -> list[tuple[int, int]]:
    ordered = sorted(intervals)
    if not ordered:
        return []
    merged: list[tuple[int, int]] = []
    start, end = ordered[0]
    for next_start, next_end in ordered[1:]:
        if next_start <= end:
            end = max(end, next_end)
        else:
            merged.append((start, end))
            start, end = next_start, next_end
    merged.append((start, end))
    return merged


def _type7(values: Iterable[int], percentile: int) -> Decimal:
    ordered = sorted(values)
    if not ordered:
        raise CampaignError("cannot aggregate an empty timing distribution")
    position = Decimal(len(ordered) - 1) * Decimal(percentile) / Decimal(100)
    lower = int(position)
    fraction = position - lower
    upper = min(lower + 1, len(ordered) - 1)
    return Decimal(ordered[lower]) + fraction * Decimal(ordered[upper] - ordered[lower])


def _decimal(value: Decimal) -> str:
    rendered = format(value, "f")
    return rendered.rstrip("0").rstrip(".") if "." in rendered else rendered


def _distribution(values: list[int]) -> dict[str, Any]:
    return {
        "n": len(values),
        "median_ns": _decimal(_type7(values, 50)),
        "p10_ns": _decimal(_type7(values, 10)),
        "p90_ns": _decimal(_type7(values, 90)),
        "samples_ns": [str(value) for value in values],
    }


def _metric_selected(metric: str, span: dict[str, Any], implementation: str) -> bool:
    operation = span["operation"]
    tags = set(span["phase_tags"])
    exact = {
        "f2z-ligerito": {
            "witness": "multiswap-trace.witness_generation",
            "projection": "step2.project_prove",
            "commit": "multiswap-trace.commit",
            "prover": "multiswap-trace.end_to_end_prove",
            "verify": "multiswap-trace.verification",
            "end_to_end": "multiswap-trace.verified_trial",
            "pcs_opening": "step5.open_prove",
            "piop": "step3.piop_prove",
        },
        "limber-hyrax": {
            "witness": "multiswap.witness_generation",
            "projection": "limber.projection",
            "commit": "multiswap.commit",
            "prover": "multiswap.prover",
            "verify": "multiswap.verify",
            "end_to_end": "multiswap.trial",
            "pcs_opening": "limber.pcs.opening",
            "piop": "limber.piop",
        },
        "limber-brakedown": {
            "witness": "multiswap.witness_generation",
            "projection": "limber.projection",
            "commit": "multiswap.commit",
            "prover": "multiswap.prover",
            "verify": "multiswap.verify",
            "end_to_end": "multiswap.trial",
            "pcs_opening": "limber.pcs.opening",
            "piop": "limber.piop",
        },
    }
    expected = exact.get(implementation, {}).get(metric)
    if expected is not None:
        return operation == expected
    if metric == "pcs_total":
        return _metric_selected("commit", span, implementation) or _metric_selected(
            "pcs_opening", span, implementation
        )
    if metric == "application_total":
        return _metric_selected("witness", span, implementation) or _metric_selected(
            "prover", span, implementation
        )
    if metric == "witness":
        return operation == "multiswap.witness_generation" or "witness-generation" in tags
    if metric == "commit":
        return operation == "multiswap.commit" or "commit" in tags
    if metric == "prover":
        return operation == "multiswap.prover" or (
            "proving" in tags and span.get("attributes", {}).get("scope_kind") == "phase"
        )
    if metric == "verify":
        return operation == "multiswap.verify" or "verification" in tags
    if metric == "end_to_end":
        return operation == "multiswap.trial" or "end-to-end" in tags
    if metric == "pcs_opening":
        if implementation.startswith("limber"):
            return operation == "limber.pcs.opening"
        return "opening-proof" in tags
    if metric == "piop":
        if implementation.startswith("limber"):
            return operation in LIMBER_PIOP_OPERATIONS
        return "constraint-proof" in tags and "opening-proof" not in tags
    if metric == "projection":
        if implementation.startswith("limber"):
            return operation in LIMBER_PROJECTION_OPERATIONS
        return operation == "step2.project_prove"
    raise AssertionError(metric)


def _math_for(operation: str, name: str, attributes: dict[str, Any]) -> list[str]:
    raw = attributes.get("math_latex")
    if isinstance(raw, list):
        math = [item for item in raw if isinstance(item, str) and item.strip()]
        if math:
            return math[:2]
    haystack = f"{operation} {name}"
    for pattern, expressions in FALLBACK_MATH:
        if pattern.search(haystack):
            return list(expressions)
    return [r"t_{\mathrm{op}}=t_{\mathrm{end}}-t_{\mathrm{start}}"]


def _sample_runs(cell: CellTrace) -> list[dict[str, Any]]:
    return sorted(
        (run for run in cell.runs if run["trial"]["kind"] == "sample"),
        key=lambda run: run["trial"]["sample_index"],
    )


def build_summary(manifest: dict[str, Any], cells: list[CellTrace]) -> dict[str, Any]:
    summaries: list[dict[str, Any]] = []
    for cell in cells:
        cell_meta = cell.manifest_cell
        implementation = _string(cell_meta.get("implementation"), "cell implementation")
        runs = _sample_runs(cell)
        headline_values: dict[str, list[int]] = {metric: [] for metric, _ in HEADLINES}
        operation_values: dict[str, list[int]] = defaultdict(list)
        operation_meta: dict[str, dict[str, Any]] = {}
        per_run_prover: dict[str, int] = {}
        for run in runs:
            run_id = run["run_id"]
            spans = cell.spans_by_run[run_id]
            for metric, label in HEADLINES:
                value = _union_duration(
                    (span["_start"], span["_end"])
                    for span in spans
                    if _metric_selected(metric, span, implementation)
                )
                if value <= 0:
                    raise CampaignError(
                        f"cell {cell_meta['cell_id']} run {run_id} has no {label} intervals"
                    )
                headline_values[metric].append(value)
                if metric == "prover":
                    per_run_prover[run_id] = value
            by_operation: dict[str, list[tuple[int, int]]] = defaultdict(list)
            for span in spans:
                operation = span["operation"]
                by_operation[operation].append((span["_start"], span["_end"]))
                attributes = span.get("attributes") if isinstance(span.get("attributes"), dict) else {}
                operation_meta.setdefault(
                    operation,
                    {
                        "operation": operation,
                        "name": attributes.get("short_name") or span["name"],
                        "primary_phase": span["primary_phase"],
                        "math_latex": _math_for(operation, span["name"], attributes),
                    },
                )
            for operation, intervals in by_operation.items():
                operation_values[operation].append(_union_duration(intervals))

        prover_dist = _distribution(headline_values["prover"])
        prover_median = Decimal(prover_dist["median_ns"])
        representative = min(
            runs,
            key=lambda run: (
                abs(Decimal(per_run_prover[run["run_id"]]) - prover_median),
                run["trial"]["sample_index"],
            ),
        )
        representative_spans = cell.spans_by_run[representative["run_id"]]
        root = next(
            span for span in representative_spans if span["span_id"] == representative["root_span_id"]
        )
        representative_by_operation: dict[str, list[tuple[int, int]]] = defaultdict(list)
        for span in representative_spans:
            representative_by_operation[span["operation"]].append((span["_start"], span["_end"]))
        operations = []
        for operation, values in operation_values.items():
            metadata = operation_meta[operation]
            segments = [
                {
                    "start_ns": str(start - root["_start"]),
                    "end_ns": str(end - root["_start"]),
                }
                for start, end in _merged_intervals(representative_by_operation[operation])
            ]
            operations.append(
                {
                    **metadata,
                    "distribution": _distribution(values),
                    "representative_segments": segments,
                    "representative_first_start_ns": segments[0]["start_ns"] if segments else "0",
                }
            )
        operations.sort(key=lambda item: (int(item["representative_first_start_ns"]), item["operation"]))
        summaries.append(
            {
                "cell_id": cell_meta["cell_id"],
                "execution_index": cell_meta["execution_index"],
                "label": cell_meta.get("label", cell_meta["cell_id"]),
                "implementation": implementation,
                "backend": cell_meta.get("backend"),
                "workload_k": cell.workload_k,
                "thread_mode": cell_meta.get("thread_mode"),
                "rayon_threads": cell_meta["rayon_threads"],
                "trace": str(cell.path),
                "trace_sha256": cell.source_sha256,
                "sample_count": len(runs),
                "headline": {
                    metric: {"label": label, **_distribution(headline_values[metric])}
                    for metric, label in HEADLINES
                },
                "representative": {
                    "run_id": representative["run_id"],
                    "sample_index": representative["trial"]["sample_index"],
                    "root_duration_ns": str(root["_duration"]),
                },
                "operations": operations,
            }
        )
    return {
        "schema": REPORT_SCHEMA,
        "campaign_id": manifest["campaign_id"],
        "workload": manifest.get("workload", {}),
        "sampling": manifest["sampling"],
        "canonical_statements": {
            str(workload_k): {
                "domain": next(cell.statement_domain for cell in cells if cell.workload_k == workload_k),
                "digest_blake3": next(cell.statement_digest for cell in cells if cell.workload_k == workload_k),
            }
            for workload_k in manifest["workload"]["workload_k_values"]
        },
        "workload_ids": {
            str(workload_k): next(cell.workload_id for cell in cells if cell.workload_k == workload_k)
            for workload_k in manifest["workload"]["workload_k_values"]
        },
        "assignment_digests_blake3": {
            str(workload_k): next(cell.assignment_digest for cell in cells if cell.workload_k == workload_k)
            for workload_k in manifest["workload"]["workload_k_values"]
        },
        "relation_shapes": {
            str(workload_k): dict(
                zip(
                    (
                        "live_rows",
                        "live_columns",
                        "padded_rows",
                        "padded_columns",
                        "nnz_a",
                        "nnz_b",
                        "nnz_c",
                    ),
                    next(
                        cell.relation_shape
                        for cell in cells
                        if cell.workload_k == workload_k
                    ),
                    strict=True,
                )
            )
            for workload_k in manifest["workload"]["workload_k_values"]
        },
        "host": manifest.get("host", {}),
        "repositories": manifest.get("repositories", {}),
        "cells": summaries,
    }


def _format_ms(ns: str | int | Decimal) -> str:
    value = Decimal(ns) / Decimal(1_000_000)
    if value >= 1000:
        return f"{value / 1000:.3f} s"
    if value >= 10:
        return f"{value:.2f} ms"
    return f"{value:.3f} ms"


def write_csv(summary: dict[str, Any], path: Path) -> None:
    with path.open("w", encoding="utf-8", newline="") as output:
        writer = csv.writer(output)
        writer.writerow(
            [
                "campaign_id",
                "workload_k",
                "k_semantics",
                "cell_id",
                "execution_index",
                "implementation",
                "backend",
                "thread_mode",
                "rayon_threads",
                "metric",
                "n",
                "median_ns",
                "p10_ns",
                "p90_ns",
            ]
        )
        for cell in summary["cells"]:
            for metric, distribution in cell["headline"].items():
                writer.writerow(
                    [
                        summary["campaign_id"],
                        cell["workload_k"],
                        "source reference" if cell["workload_k"] == 0 else "modeled per-swap H_delta scaling",
                        cell["cell_id"],
                        cell["execution_index"],
                        cell["implementation"],
                        cell["backend"],
                        cell["thread_mode"],
                        cell["rayon_threads"],
                        metric,
                        distribution["n"],
                        distribution["median_ns"],
                        distribution["p10_ns"],
                        distribution["p90_ns"],
                    ]
                )


def render_html(summary: dict[str, Any]) -> str:
    cells = summary["cells"]
    options = "".join(
        f'<option value="{html.escape(cell["cell_id"])}">{html.escape(cell["label"])}</option>'
        for cell in cells
    )
    headline_groups = []
    for workload_k in summary["workload"]["workload_k_values"]:
        implementation_rank = {
            "f2z-ligerito": 0,
            "limber-hyrax": 1,
            "limber-brakedown": 2,
        }
        thread_rank = {"single": 0, "performance": 1}
        group = sorted(
            (cell for cell in cells if cell["workload_k"] == workload_k),
            key=lambda cell: (
                implementation_rank[cell["implementation"]],
                thread_rank[cell["thread_mode"]],
            ),
        )
        table_headers = "".join(
            f"<th>{html.escape(cell['label'].split(' · ', 1)[-1])}</th>" for cell in group
        )
        headline_rows = []
        for metric, label in HEADLINES:
            values = "".join(
                "<td><strong>{}</strong><small>P10–P90 {}–{}</small></td>".format(
                    _format_ms(cell["headline"][metric]["median_ns"]),
                    _format_ms(cell["headline"][metric]["p10_ns"]),
                    _format_ms(cell["headline"][metric]["p90_ns"]),
                )
                for cell in group
            )
            headline_rows.append(f"<tr><th>{html.escape(label)}</th>{values}</tr>")
        semantics = (
            "quotable source-backed reference"
            if workload_k == 0
            else "modeled per-swap H_delta scaling"
        )
        digest = summary["canonical_statements"][str(workload_k)]
        headline_groups.append(
            f'<section class="k-group"><h3>k={workload_k} <small>· {semantics}</small></h3>'
            f'<p class="digest">{html.escape(digest["domain"])} / '
            f'{html.escape(digest["digest_blake3"])}</p>'
            f'<div class="summary-wrap"><table><thead><tr><th>Metric</th>{table_headers}'
            f'</tr></thead><tbody>{"".join(headline_rows)}</tbody></table></div></section>'
        )

    panels = []
    for cell_index, cell in enumerate(cells):
        cards = "".join(
            '<article class="card"><span>{}</span><strong>{}</strong><small>P10–P90 {}–{} · n={}</small></article>'.format(
                html.escape(distribution["label"]),
                _format_ms(distribution["median_ns"]),
                _format_ms(distribution["p10_ns"]),
                _format_ms(distribution["p90_ns"]),
                distribution["n"],
            )
            for distribution in cell["headline"].values()
        )
        root_duration = max(1, int(cell["representative"]["root_duration_ns"]))
        rows = []
        for operation in cell["operations"]:
            segments = []
            for segment in operation["representative_segments"]:
                left = max(0.0, min(100.0, 100 * int(segment["start_ns"]) / root_duration))
                width = max(0.18, 100 * (int(segment["end_ns"]) - int(segment["start_ns"])) / root_duration)
                width = min(width, 100 - left)
                segments.append(
                    f'<i class="segment" style="left:{left:.5f}%;width:{width:.5f}%"></i>'
                )
            math_lines = "".join(
                f'<div class="math" data-latex="{html.escape(expression, quote=True)}"><code>{html.escape(expression)}</code></div>'
                for expression in operation["math_latex"]
            )
            dist = operation["distribution"]
            tooltip = (
                f'<aside class="tip"><b>{html.escape(operation["name"])}</b>'
                f'<span>{html.escape(operation["operation"])}</span>{math_lines}'
                f'<span>Median {_format_ms(dist["median_ns"])} · '
                f'P10–P90 {_format_ms(dist["p10_ns"])}–{_format_ms(dist["p90_ns"])} · n={dist["n"]}</span></aside>'
            )
            rows.append(
                '<div class="interval-row" tabindex="0">'
                f'<div class="row-name"><b>{html.escape(operation["name"])}</b><code>{html.escape(operation["operation"])}</code></div>'
                f'<div class="row-time">{_format_ms(dist["median_ns"])}</div>'
                f'<div class="track">{"".join(segments)}</div>{tooltip}</div>'
            )
        hidden = "" if cell_index == 0 else " hidden"
        panels.append(
            f'<section class="panel{hidden}" data-cell="{html.escape(cell["cell_id"])}">'
            f'<h2>{html.escape(cell["label"])}</h2><p class="muted">Representative sample '
            f'{cell["representative"]["sample_index"]} · execution #{cell["execution_index"]} · '
            f'{cell["rayon_threads"]} unpinned Rayon thread(s) · '
            f'{html.escape(cell["trace_sha256"][:12])}…</p><div class="cards">{cards}</div>'
            f'<h3>Measured operation intervals</h3><p class="muted">Bars preserve chronology from the '
            f'representative run. Hover or focus any row for its equation and P10–P90 distribution.</p>'
            f'<div class="timeline">{"".join(rows)}</div></section>'
        )

    workload = summary.get("workload", {})
    workload_name = workload.get("name", "Wired MultiSwap/RSA cost-model")
    disclosure = workload.get(
        "disclosure",
        "Synthetic wired cost-model; modeled hashes/Poseidon are not native hash executions.",
    )
    return f"""<!doctype html>
<html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1">
<title>Matched MultiSwap prover campaign</title>
<link rel="stylesheet" href="https://cdn.jsdelivr.net/npm/katex@0.16.11/dist/katex.min.css">
<script defer src="https://cdn.jsdelivr.net/npm/katex@0.16.11/dist/katex.min.js"></script>
<style>
:root{{--ink:#18191d;--muted:#686d78;--line:#dfe3eb;--paper:#fbfbfd;--blue:#356ae6;--pink:#ef4b9a;--violet:#8757e8;--shadow:0 12px 30px #1d24300d}}
*{{box-sizing:border-box}} body{{margin:0;background:var(--paper);color:var(--ink);font:15px/1.45 Inter,ui-sans-serif,system-ui,-apple-system,sans-serif}}
main{{max-width:1500px;margin:auto;padding:38px 32px 80px}} h1{{font-size:32px;margin:0 0 6px}} h2{{margin:30px 0 4px}} h3{{margin:28px 0 4px}} .muted{{color:var(--muted)}}
.notice{{border:1px solid #f1c76e;background:#fff9e9;border-radius:12px;padding:12px 16px;margin:20px 0}} .digest{{font-family:ui-monospace,monospace;font-size:12px;overflow-wrap:anywhere}}
.summary-wrap{{overflow:auto;border:1px solid var(--line);border-radius:15px;background:white;box-shadow:var(--shadow)}} .k-group>h3 small{{color:var(--muted);font-weight:500}} table{{border-collapse:collapse;min-width:100%;white-space:nowrap}} th,td{{padding:13px 15px;border-bottom:1px solid var(--line);text-align:right}} th:first-child{{text-align:left;position:sticky;left:0;background:white}} thead th{{background:#f4f6fa}} td small,.card small{{display:block;color:var(--muted);font-size:11px}}
.picker{{display:flex;align-items:center;gap:12px;margin-top:30px}} select{{font:inherit;padding:8px 12px;border:1px solid var(--line);border-radius:9px;background:white}}
.cards{{display:grid;grid-template-columns:repeat(auto-fit,minmax(170px,1fr));gap:10px;margin:18px 0}} .card{{border:1px solid var(--line);border-radius:13px;background:white;padding:14px;box-shadow:var(--shadow)}} .card span{{display:block;color:var(--muted);font-size:12px}} .card strong{{font-size:22px}}
.hidden{{display:none}} .timeline{{border-top:1px solid var(--line)}} .interval-row{{position:relative;display:grid;grid-template-columns:minmax(240px,1fr) 110px 3fr;gap:14px;align-items:center;min-height:50px;padding:7px 10px;border-bottom:1px solid var(--line);outline:none}} .interval-row:hover,.interval-row:focus{{background:#f2f6ff}}
.row-name{{min-width:0}} .row-name b,.row-name code{{display:block;overflow:hidden;text-overflow:ellipsis;white-space:nowrap}} .row-name code{{font-size:10px;color:var(--muted)}} .row-time{{text-align:right;font-variant-numeric:tabular-nums}} .track{{height:24px;position:relative;background:repeating-linear-gradient(90deg,#eef1f6 0,#eef1f6 1px,transparent 1px,transparent 25%);border-radius:4px}} .segment{{position:absolute;top:2px;height:20px;border-radius:4px;background:linear-gradient(90deg,var(--blue),var(--violet));min-width:2px}}
.tip{{display:none;position:absolute;z-index:10;left:min(32%,430px);top:44px;width:min(630px,65vw);padding:14px 16px;border:1px solid #cbd3e2;border-radius:10px;background:#111827;color:#f9fafb;box-shadow:0 18px 50px #0005}} .interval-row:hover .tip,.interval-row:focus .tip{{display:grid;gap:6px}} .tip>span{{color:#cfd6e4;font-size:12px}} .math{{overflow:auto;background:#ffffff12;border-radius:6px;padding:7px}} .math code{{color:#f7d6ec;white-space:pre-wrap}}
@media(max-width:850px){{main{{padding:24px 14px}}.interval-row{{grid-template-columns:1fr 90px}}.track{{grid-column:1/-1}}.tip{{left:4%;width:92%}}}}
</style></head><body><main>
<h1>Matched F2Z / Limber MultiSwap campaign</h1>
<p class="muted">{html.escape(workload_name)} · one warmup excluded · median of {summary['sampling']['samples']} measured trials · Hyndman–Fan Type 7 P10–P90</p>
<div class="notice"><strong>Interpretation boundary.</strong> {html.escape(disclosure)}</div>
<section><h2>Headline comparison by workload k</h2>{''.join(headline_groups)}</section>
<div class="picker"><label for="cell-picker"><strong>Detailed trace</strong></label><select id="cell-picker">{options}</select></div>
{''.join(panels)}
</main><script>
const picker=document.querySelector('#cell-picker'); picker.addEventListener('change',()=>{{document.querySelectorAll('[data-cell]').forEach(p=>p.classList.toggle('hidden',p.dataset.cell!==picker.value));}});
function renderMath(){{if(!globalThis.katex)return;document.querySelectorAll('[data-latex]').forEach(node=>{{try{{katex.render(node.dataset.latex,node,{{throwOnError:false,displayMode:false}})}}catch(_){{}}}})}}
window.addEventListener('load',renderMath);
</script></body></html>"""


def write_report(summary: dict[str, Any], out_dir: Path, *, force: bool = False) -> None:
    out_dir.mkdir(parents=True, exist_ok=True)
    outputs = [out_dir / name for name in OUTPUT_NAMES]
    existing = [path for path in outputs if path.exists()]
    if existing and not force:
        raise CampaignError(f"refusing to overwrite report files: {', '.join(map(str, existing))}")
    (out_dir / "summary.json").write_text(
        json.dumps(summary, indent=2, sort_keys=True, ensure_ascii=False) + "\n",
        encoding="utf-8",
    )
    write_csv(summary, out_dir / "metrics.csv")
    (out_dir / "intervals.html").write_text(render_html(summary), encoding="utf-8")


def parse_args(argv: Sequence[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    subparsers = parser.add_subparsers(dest="command", required=True)
    validate = subparsers.add_parser("validate", help="strictly validate a completed campaign")
    validate.add_argument("manifest", type=Path)
    report = subparsers.add_parser("report", help="validate, aggregate, and render a campaign")
    report.add_argument("manifest", type=Path)
    report.add_argument("--out-dir", type=Path, required=True)
    report.add_argument("--force", action="store_true")
    return parser.parse_args(argv)


def main(argv: Sequence[str] | None = None) -> int:
    args = parse_args(argv)
    try:
        manifest, cells = validate_campaign(args.manifest.resolve())
        if args.command == "validate":
            print(
                f"validated {len(cells)} cells / "
                f"{sum(len(_sample_runs(cell)) for cell in cells)} measured trials"
            )
            return 0
        summary = build_summary(manifest, cells)
        write_report(summary, args.out_dir.resolve(), force=args.force)
        for name in OUTPUT_NAMES:
            print(args.out_dir.resolve() / name)
        return 0
    except CampaignError as error:
        print(f"error: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())

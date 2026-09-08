#!/usr/bin/env python3
"""Fixture-only tests for the matched MultiSwap campaign tooling."""

from __future__ import annotations

import json
import hashlib
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

import matched_multiswap_report as report
import run_matched_multiswap_campaign as runner


DOMAIN = "f2z/multiswap/circuit-digest/v1"
DIGEST = "ab" * 32
ASSIGNMENT = "cd" * 32


def _span(
    run_id: str,
    span_id: str,
    operation: str,
    name: str,
    start: int,
    end: int,
    phase: str,
    parent: str | None,
    math: list[str] | None = None,
) -> dict[str, object]:
    attributes: dict[str, object] = {"scope_kind": "operation"}
    if math:
        attributes["math_latex"] = math
    return {
        "schema": report.TRACE_SCHEMA,
        "record": "span",
        "run_id": run_id,
        "span_id": span_id,
        "parent_span_id": parent,
        "operation": operation,
        "name": name,
        "primary_phase": phase,
        "phase_tags": [phase],
        "start_ns": str(start),
        "end_ns": str(end),
        "duration_ns": str(end - start),
        "attributes": attributes,
    }


def _records(
    cell_id: str,
    implementation: str,
    digest: str = DIGEST,
    threads: int = 1,
    workload_k: int = 0,
) -> list[dict[str, object]]:
    records: list[dict[str, object]] = []
    trials = [("warmup", 0), *(('sample', index) for index in range(5))]
    for kind, index in trials:
        run_id = f"{cell_id}-{kind}-{index}"
        root_id = f"{run_id}-root"
        scale = 1 if kind == "warmup" else index + 1
        statement = {"domain": DOMAIN, "digest_blake3": digest}
        parameters: dict[str, object]
        if implementation == "f2z-ligerito":
            parameters = {
                "input": {
                    "workload_id": "multiswap-rsa-wired-cost-model-v1",
                    "limber_k": workload_k,
                    "constraint_digest_domain": DOMAIN,
                    "constraint_digest_blake3": digest,
                    "live_rows": 100 + workload_k,
                    "live_columns": 96 + workload_k,
                    "padded_rows": 128,
                    "padded_columns": 128,
                    "nnz_a": 200 + workload_k,
                    "nnz_b": 210 + workload_k,
                    "nnz_c": 220 + workload_k,
                    "witness_stats": {
                        "assignment_digest_domain": report.ASSIGNMENT_DOMAIN,
                        "assignment_digest_blake3": ASSIGNMENT,
                    },
                }
            }
            operations = {
                "root": "multiswap-trace.verified_trial",
                "witness": "multiswap-trace.witness_generation",
                "commit": "multiswap-trace.commit",
                "prover": "multiswap-trace.end_to_end_prove",
                "projection": "step2.project_prove",
                "piop": "step3.piop_prove",
                "open": "step5.open_prove",
                "verify": "multiswap-trace.verification",
            }
        else:
            parameters = {
                "input": {
                    "workload_id": "multiswap-rsa-wired-cost-model-v1",
                    "limber_k": workload_k,
                    "live_rows": 100 + workload_k,
                    "live_cols": 96 + workload_k,
                    "num_cons": 128,
                    "num_vars": 128,
                    "a_nnz": 200 + workload_k,
                    "b_nnz": 210 + workload_k,
                    "c_nnz": 220 + workload_k,
                },
                "statement": {
                    **statement,
                    "assignment_digest_domain": report.ASSIGNMENT_DOMAIN,
                    "assignment_digest_blake3": ASSIGNMENT,
                }
            }
            operations = {
                "root": "multiswap.trial",
                "witness": "multiswap.witness_generation",
                "commit": "multiswap.commit",
                "prover": "multiswap.prover",
                "projection": "limber.projection",
                "piop": "limber.piop",
                "open": "limber.pcs.opening",
                "verify": "multiswap.verify",
            }
        trial = {"kind": kind, f"{kind}_index": index}
        records.append(
            {
                "schema": report.TRACE_SCHEMA,
                "record": "run",
                "run_id": run_id,
                "series_id": cell_id,
                "root_span_id": root_id,
                "benchmark": {"name": "multiswap-rsa-matched", "implementation": implementation},
                "trial": trial,
                "clock": {"kind": "monotonic", "unit": "ns"},
                "status": "ok",
                "trace_complete": True,
                "environment": {"threads": threads},
                "parameters": parameters,
                "validation": {
                    "proof_verified": True,
                    "relation_valid": True,
                    "expected_digest_supplied": False,
                },
            }
        )
        end = 12_000_000 * scale
        records.extend(
            [
                _span(run_id, root_id, operations["root"], "Verified trial", 0, end, "end-to-end", None),
                _span(run_id, f"{run_id}-w", operations["witness"], "Witness generation", 0, 1_000_000 * scale, "witness-generation", root_id, [r"\mathbf z=(\mathbf W,1,\mathbf x)"]),
                _span(run_id, f"{run_id}-c", operations["commit"], "Commit", 1_000_000 * scale, 2_000_000 * scale, "commit", root_id),
                _span(run_id, f"{run_id}-p", operations["prover"], "Prover", 2_000_000 * scale, 9_000_000 * scale, "proving", root_id),
                _span(run_id, f"{run_id}-r", operations["projection"], "Projection", 2_000_000 * scale, 3_000_000 * scale, "preparation", f"{run_id}-p"),
                _span(run_id, f"{run_id}-s", operations["piop"], "PIOP", 3_000_000 * scale, 5_000_000 * scale, "constraint-proof", f"{run_id}-p"),
                _span(run_id, f"{run_id}-o", operations["open"], "PCS opening", 5_000_000 * scale, 8_000_000 * scale, "opening-proof", f"{run_id}-p", [r"\sum_i\lambda^i C(z_i)=\sum_i\lambda^i v_i"]),
                _span(run_id, f"{run_id}-v", operations["verify"], "Verify", 9_000_000 * scale, 10_000_000 * scale, "verification", root_id),
            ]
        )
    return records


def _write_fixture(
    root: Path,
    *,
    mismatch_cell: str | None = None,
    k_values: tuple[int, ...] = (0,),
) -> Path:
    raw_dir = root / "raw"
    metadata_dir = root / "metadata"
    raw_dir.mkdir()
    metadata_dir.mkdir()
    cells = []
    for workload_k in k_values:
        specs = (
            (f"k{workload_k}-f2z-single", "f2z-ligerito", "virtual-f2z", 1),
            (f"k{workload_k}-f2z-performance", "f2z-ligerito", "virtual-f2z", 8),
            (f"k{workload_k}-limber-hyrax-single", "limber-hyrax", "hyrax", 1),
            (f"k{workload_k}-limber-hyrax-performance", "limber-hyrax", "hyrax", 8),
            (f"k{workload_k}-limber-brakedown-single", "limber-brakedown", "brakedown", 1),
            (f"k{workload_k}-limber-brakedown-performance", "limber-brakedown", "brakedown", 8),
        )
        group_digest = DIGEST if workload_k == 0 else f"{workload_k:02x}" * 32
        for cell_id, implementation, backend, threads in specs:
            digest = "ef" * 32 if cell_id == mismatch_cell else group_digest
            path = raw_dir / f"{cell_id}.jsonl"
            path.write_text(
                "".join(
                    json.dumps(record, separators=(",", ":")) + "\n"
                    for record in _records(
                        cell_id, implementation, digest, threads, workload_k
                    )
                ),
                encoding="utf-8",
            )
            trace_sha256 = hashlib.sha256(path.read_bytes()).hexdigest()
            cells.append(
                {
                    "cell_id": cell_id,
                    "label": cell_id,
                    "implementation": implementation,
                    "backend": backend,
                    "workload_k": workload_k,
                    "thread_mode": "single" if threads == 1 else "performance",
                    "rayon_threads": threads,
                    "trace": f"../raw/{path.name}",
                    "trace_sha256": trace_sha256,
                    "status": "ok",
                }
            )
    for execution_index, cell in enumerate(cells):
        cell["execution_index"] = execution_index
    manifest = {
        "schema": report.CAMPAIGN_SCHEMA,
        "trace_schema": report.TRACE_SCHEMA,
        "campaign_id": "fixture",
        "sampling": {"warmups": 1, "samples": 5},
        "workload": {
            "name": "wired MultiSwap/RSA cost-model",
            "workload_k_values": list(k_values),
        },
        "execution_order": {"cell_ids": [cell["cell_id"] for cell in cells]},
        "cells": cells,
    }
    path = metadata_dir / "campaign.json"
    path.write_text(json.dumps(manifest), encoding="utf-8")
    return path


class CampaignFixtureTests(unittest.TestCase):
    def test_plan_has_six_isolated_trace_files_per_k(self) -> None:
        root = Path("/tmp/f2z")
        cells = runner.build_cells(
            f2z_root=root,
            limber_root=Path("/tmp/limber"),
            run_dir=Path("/tmp/campaign"),
            campaign_id="fixture",
            samples=5,
            warmups=1,
            all_threads=8,
            rustflags="-Ctarget-cpu=native",
            expected_digests={},
            k_values=(0, 1, 2, 4, 8),
        )
        self.assertEqual(len(cells), 30)
        self.assertEqual({cell["rayon_threads"] for cell in cells}, {1, 8})
        self.assertEqual(len({cell["environment"].get("F2Z_MULTISWAP_TRACE_PATH") or cell["environment"].get("MATCHED_TRACE_PATH") for cell in cells}), 30)
        self.assertEqual(sum(cell["backend"] == "hyrax" for cell in cells), 10)
        self.assertEqual(sum(cell["backend"] == "brakedown" for cell in cells), 10)
        self.assertEqual({cell["workload_k"] for cell in cells}, {0, 1, 2, 4, 8})
        limber_commands = [cell["command"] for cell in cells if cell["implementation"].startswith("limber")]
        self.assertTrue(
            all(command[:4] == ["rustup", "run", "nightly-2026-07-01", "cargo"] for command in limber_commands)
        )
        limber_cells = [cell for cell in cells if cell["implementation"].startswith("limber")]
        self.assertTrue(
            all(cell["environment"]["MATCHED_K"] == str(cell["workload_k"]) for cell in limber_cells)
        )
        self.assertEqual(
            {cell["environment"]["CARGO_TARGET_DIR"] for cell in limber_cells},
            {"/tmp/campaign/build/limber-target"},
        )
        self.assertEqual([cell["execution_index"] for cell in cells], list(range(30)))
        first_by_k = [cells[index] for index in range(0, 30, 6)]
        self.assertEqual(
            [cell["implementation"] for cell in first_by_k],
            [
                "f2z-ligerito",
                "limber-hyrax",
                "limber-brakedown",
                "f2z-ligerito",
                "limber-hyrax",
            ],
        )
        self.assertEqual(
            [cell["thread_mode"] for cell in first_by_k],
            ["single", "performance", "single", "performance", "single"],
        )

    def test_valid_fixture_aggregates_type7_and_renders_math(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            manifest_path = _write_fixture(root)
            manifest, cells = report.validate_campaign(manifest_path)
            self.assertEqual({cell.assignment_digest for cell in cells}, {ASSIGNMENT})
            summary = report.build_summary(manifest, cells)
            first = summary["cells"][0]
            self.assertEqual(first["headline"]["witness"]["median_ns"], "3000000")
            self.assertEqual(first["headline"]["witness"]["p10_ns"], "1400000")
            self.assertEqual(first["headline"]["witness"]["p90_ns"], "4600000")
            self.assertEqual(first["headline"]["pcs_total"]["median_ns"], "12000000")
            self.assertEqual(
                first["headline"]["application_total"]["median_ns"], "24000000"
            )
            out_dir = root / "report"
            report.write_report(summary, out_dir)
            page = (out_dir / "intervals.html").read_text(encoding="utf-8")
            self.assertIn("Matched F2Z / Limber", page)
            self.assertIn(r"\mathbf z", page)
            self.assertTrue((out_dir / "metrics.csv").is_file())

    def test_digest_mismatch_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            manifest_path = _write_fixture(Path(temporary), mismatch_cell="k0-limber-hyrax-single")
            with self.assertRaisesRegex(report.CampaignError, "digest mismatch"):
                report.validate_campaign(manifest_path)

    def test_distinct_k_groups_may_have_distinct_digests(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            manifest_path = _write_fixture(Path(temporary), k_values=(0, 2))
            manifest, cells = report.validate_campaign(manifest_path)
            summary = report.build_summary(manifest, cells)
            self.assertEqual(len(cells), 12)
            self.assertEqual(set(summary["canonical_statements"]), {"0", "2"})
            self.assertNotEqual(
                summary["canonical_statements"]["0"]["digest_blake3"],
                summary["canonical_statements"]["2"]["digest_blake3"],
            )

    def test_invalid_proof_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            manifest_path = _write_fixture(Path(temporary))
            manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
            trace_path = (manifest_path.parent / manifest["cells"][0]["trace"]).resolve()
            records = [json.loads(line) for line in trace_path.read_text(encoding="utf-8").splitlines()]
            next(record for record in records if record["record"] == "run")["validation"]["proof_verified"] = False
            trace_path.write_text("".join(json.dumps(record) + "\n" for record in records), encoding="utf-8")
            with self.assertRaisesRegex(report.CampaignError, "proof_verified"):
                report.validate_campaign(manifest_path)

    def test_relation_shape_mismatch_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            manifest_path = _write_fixture(Path(temporary))
            manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
            cell = next(
                cell for cell in manifest["cells"] if cell["cell_id"] == "k0-limber-hyrax-single"
            )
            trace_path = (manifest_path.parent / cell["trace"]).resolve()
            records = [json.loads(line) for line in trace_path.read_text(encoding="utf-8").splitlines()]
            for record in records:
                if record["record"] == "run":
                    record["parameters"]["input"]["a_nnz"] += 1
            trace_path.write_text(
                "".join(json.dumps(record) + "\n" for record in records), encoding="utf-8"
            )
            cell["trace_sha256"] = hashlib.sha256(trace_path.read_bytes()).hexdigest()
            manifest_path.write_text(json.dumps(manifest), encoding="utf-8")
            with self.assertRaisesRegex(report.CampaignError, "dimensions/nnz mismatch"):
                report.validate_campaign(manifest_path)

    def test_manifest_trace_hash_mismatch_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            manifest_path = _write_fixture(Path(temporary))
            manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
            cell = manifest["cells"][0]
            trace_path = (manifest_path.parent / cell["trace"]).resolve()
            records = [json.loads(line) for line in trace_path.read_text(encoding="utf-8").splitlines()]
            trace_path.write_text(
                "".join(json.dumps(record, sort_keys=True) + "\n" for record in records),
                encoding="utf-8",
            )
            with self.assertRaisesRegex(report.CampaignError, "trace SHA-256"):
                report.validate_campaign(manifest_path)

    def test_noncanonical_statement_domain_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            manifest_path = _write_fixture(Path(temporary))
            manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
            cell = manifest["cells"][0]
            trace_path = (manifest_path.parent / cell["trace"]).resolve()
            records = [json.loads(line) for line in trace_path.read_text(encoding="utf-8").splitlines()]
            first_run = next(record for record in records if record["record"] == "run")
            first_run["parameters"]["input"]["constraint_digest_domain"] = "wrong/domain"
            trace_path.write_text(
                "".join(json.dumps(record) + "\n" for record in records), encoding="utf-8"
            )
            cell["trace_sha256"] = hashlib.sha256(trace_path.read_bytes()).hexdigest()
            manifest_path.write_text(json.dumps(manifest), encoding="utf-8")
            with self.assertRaisesRegex(report.CampaignError, "statement domain"):
                report.validate_campaign(manifest_path)


if __name__ == "__main__":
    unittest.main()

#!/usr/bin/env python3
"""Focused regression tests for export_f2z_intervals.py."""

from __future__ import annotations

import importlib.util
import json
import argparse
import sys
import tempfile
import unittest
from pathlib import Path


SCRIPT = Path(__file__).with_name("export_f2z_intervals.py")
SPEC = importlib.util.spec_from_file_location("export_f2z_intervals", SCRIPT)
assert SPEC is not None and SPEC.loader is not None
EXPORTER = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = EXPORTER
SPEC.loader.exec_module(EXPORTER)


class ExportIntervalsTest(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary.name) / "n20"
        self.root.mkdir()

    def tearDown(self) -> None:
        self.temporary.cleanup()

    def write_trace(
        self,
        kind: str,
        index: int,
        trial_ns: int,
        *,
        omit_sumcheck: bool = False,
    ) -> Path:
        run_id = f"f2z-n20-{kind}{index}"
        index_key = "warmup_index" if kind == "warmup" else "sample_index"
        run = {
            "schema": EXPORTER.TRACE_SCHEMA,
            "record": "run",
            "run_id": run_id,
            "series_id": "f2z-n20-custom34-8t",
            "root_span_id": "root",
            "status": "ok",
            "trace_complete": True,
            "trial": {"kind": kind, index_key: index},
            "environment": {"threads": 8},
            "parameters": {
                "input": {
                    "committed_bits_exponent": 20,
                    "t": 13,
                    "s": 7,
                    "word_bits": 1,
                    "requested_config": "custom:3:4",
                    "resolved_config": "custom-k4@r1/8k4",
                },
                "output": {
                    "proof_bytes": 60,
                    **{
                        f"artifact_{name}_bytes": 10
                        for name in EXPORTER.EXPECTED_ARTIFACTS
                    },
                    "proof_fnv": "0123456789abcdef",
                },
            },
        }

        def span(
            span_id: str,
            parent: str | None,
            operation: str,
            start_ns: int,
            end_ns: int,
            tags: list[str],
            primary: bool = False,
        ) -> dict[str, object]:
            return {
                "schema": EXPORTER.TRACE_SCHEMA,
                "record": "span",
                "run_id": run_id,
                "span_id": span_id,
                "parent_span_id": parent,
                "operation": operation,
                "name": operation,
                "primary_phase": tags[-1],
                "phase_tags": tags,
                "start_ns": str(start_ns),
                "end_ns": str(end_ns),
                "duration_ns": str(end_ns - start_ns),
                "attributes": {
                    "scope_kind": "phase",
                    "primary_sequence": primary,
                },
            }

        records = [
            run,
            span("root", None, "bench:trial", 100, 100 + trial_ns, ["end-to-end"]),
            span("commit", "root", "bench:commit", 200, 100_200, ["commit", "pcs"], True),
            span("prove", "root", "bench:prove", 100_200, 600_200, ["proving", "pcs"], True),
            span("serialize", "root", "bench:serialize", 600_200, 650_200, ["preparation"], True),
            span("verify", "root", "bench:verify", 650_200, 850_200, ["verification"], True),
        ]
        if not omit_sumcheck:
            records.append(
                span(
                    "sumcheck",
                    "prove",
                    "eqf:rounds",
                    200_000,
                    500_000,
                    ["proving", "constraint-proof", "sumcheck"],
                )
            )
        path = self.root / f"{run_id}.jsonl"
        path.write_text(
            "".join(json.dumps(record, separators=(",", ":")) + "\n" for record in records),
            encoding="utf-8",
        )
        return path

    def make_fixture(self) -> list[Path]:
        return [
            self.write_trace("warmup", 0, 1_000_000),
            self.write_trace("sample", 0, 1_000_000),
            self.write_trace("sample", 1, 1_200_000),
            self.write_trace("sample", 2, 900_000),
        ]

    def test_type7_statistics_and_real_representative_geometry(self) -> None:
        payload = EXPORTER.build_collection(self.make_fixture(), allow_partial=True)
        size = payload["sizes"][0]
        self.assertEqual(size["representative_trial"], 0)
        self.assertEqual(size["metrics"]["trial"]["med"], 1.0)
        self.assertEqual(
            size["metrics"]["trial"]["dec"],
            [0.92, 0.94, 0.96, 0.98, 1.0, 1.04, 1.08, 1.12, 1.16],
        )
        self.assertEqual(size["root"]["start_ms"], 0.0)
        self.assertEqual(size["root"]["end_ms"], 1.0)
        self.assertEqual(size["proof"]["total"]["med"], 60)
        self.assertEqual(len(size["primary"]), 4)
        self.assertIn("sumcheck", size["rows"])
        self.assertEqual(
            next(item for item in size["rows"]["sumcheck"] if item["op"] == "eqf:rounds")["name"],
            "Factored-equality sumcheck rounds",
        )

    def test_rejects_an_event_missing_from_one_trial(self) -> None:
        paths = self.make_fixture()
        paths[-1].unlink()
        paths[-1] = self.write_trace("sample", 2, 900_000, omit_sumcheck=True)
        with self.assertRaisesRegex(EXPORTER.ExportError, "unstable event set"):
            EXPORTER.build_collection(paths, allow_partial=True)

    def test_normalized_f2z_scope_uses_catalog_metadata(self) -> None:
        metadata = EXPORTER.span_metadata(
            {
                "operation": "mc.forest",
                "name": "F2Z prof scope",
                "primary_phase": "proving",
                "phase_tags": ["proving"],
                "attributes": {
                    "scope_kind": "procedure",
                    "short_name": "mc:forest",
                },
            }
        )
        self.assertEqual(metadata["name"], "Prove the merged bit forest")
        self.assertEqual(metadata["short"], "Forest")
        self.assertEqual(metadata["group"], "constraint-proof")
        self.assertIn("constraint-proof", metadata["tags"])
        self.assertIn("sumcheck", metadata["tags"])

    def test_discovery_ignores_preserved_hidden_captures(self) -> None:
        active = self.write_trace("sample", 0, 1_000_000)
        hidden_dir = self.root.parent / ".n20.failed-example"
        hidden_dir.mkdir()
        hidden = hidden_dir / "duplicate.jsonl"
        hidden.write_text(active.read_text(encoding="utf-8"), encoding="utf-8")
        args = argparse.Namespace(
            logs=[],
            log=[],
            logs_dir=self.root.parent,
        )
        discovered = EXPORTER.discover(args)
        self.assertIn(active.resolve(), discovered)
        self.assertNotIn(hidden.resolve(), discovered)

    def test_mixed_case_legacy_scope_keeps_semantic_metadata(self) -> None:
        metadata = EXPORTER.span_metadata(
            {
                "operation": "mf.phasea",
                "name": "F2Z prof scope",
                "primary_phase": "proving",
                "phase_tags": ["proving"],
                "attributes": {"scope_kind": "procedure"},
            }
        )
        self.assertEqual(metadata["name"], "Merged-forest phase A")
        self.assertEqual(metadata["short"], "Forest A")
        self.assertIn("constraint-proof", metadata["tags"])
        self.assertIn("sumcheck", metadata["tags"])

    def test_rejects_mixed_series_ids(self) -> None:
        paths = self.make_fixture()
        records = [json.loads(line) for line in paths[-1].read_text(encoding="utf-8").splitlines()]
        records[0]["series_id"] = "different-capture-series"
        paths[-1].write_text(
            "".join(json.dumps(record, separators=(",", ":")) + "\n" for record in records),
            encoding="utf-8",
        )
        with self.assertRaisesRegex(EXPORTER.ExportError, "one nonempty series_id"):
            EXPORTER.build_collection(paths, allow_partial=True)

    def test_rejects_duplicate_run_ids_globally(self) -> None:
        paths = self.make_fixture()
        duplicate = self.root / "duplicate-run.jsonl"
        duplicate.write_text(paths[1].read_text(encoding="utf-8"), encoding="utf-8")
        with self.assertRaisesRegex(EXPORTER.ExportError, "globally unique"):
            EXPORTER.build_collection([*paths, duplicate], allow_partial=True)

    def test_validates_one_complete_resumable_part(self) -> None:
        self.write_trace("warmup", 0, 1_000_000)
        for index in range(21):
            self.write_trace("sample", index, 1_000_000 + index)
        EXPORTER.validate_part(
            self.root,
            expected_profile="custom:3:4",
            expected_exponent=20,
            expected_threads=8,
        )

    def test_part_validation_rejects_noncanonical_warmup_index(self) -> None:
        self.write_trace("warmup", 7, 1_000_000)
        for index in range(21):
            self.write_trace("sample", index, 1_000_000 + index)
        with self.assertRaisesRegex(EXPORTER.ExportError, "warmup index"):
            EXPORTER.validate_part(
                self.root,
                expected_profile="custom:3:4",
                expected_exponent=20,
                expected_threads=8,
            )


if __name__ == "__main__":
    unittest.main()

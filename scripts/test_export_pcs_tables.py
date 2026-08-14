#!/usr/bin/env python3
"""Regression tests for the profile-parameterized table exporter."""

from __future__ import annotations

import importlib.util
import csv
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path


SCRIPT = Path(__file__).with_name("export_pcs_tables.py")
SPEC = importlib.util.spec_from_file_location("export_pcs_tables", SCRIPT)
assert SPEC is not None and SPEC.loader is not None
EXPORTER = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = EXPORTER
SPEC.loader.exec_module(EXPORTER)


def rows(profile: str, *, breakdown: bool) -> dict[int, dict[str, str]]:
    result: dict[int, dict[str, str]] = {}
    for exponent, (t_value, s_value, word_bits) in EXPORTER.EXPECTED_SHAPES.items():
        row = {
            "n": str(exponent),
            "t": str(t_value),
            "s": str(s_value),
            "W": str(word_bits),
            "profile_arg": profile,
            "lig_geometry": "fast@r1/2k6",
            "threads": "8",
            "reps": "1" if breakdown else "21",
            "commit_ms": "1",
            "commit_peak_mb": "2",
            "prove_ms": "3",
            "prove_peak_mb": "4",
            "verify_ms": "5",
            "proof_bytes": "1024",
            "serialize_us": "6",
            "deserialize_us": "7",
            "forest_ms": "",
            "open_ms": "",
            "untagged_ms": "",
            "profiled_prove_ms": "",
            "timestamp": "2026-08-13T00:00:00",
            "_source": "fixture.csv",
        }
        for key in EXPORTER.DETAIL_KEYS:
            row[key] = ""
        if breakdown:
            detail = {
                "pack_ms": 0,
                "pow2_ms": 1,
                "forest_core_ms": 1,
                "fold_v_ms": 1,
                "presum_tables_ms": 1,
                "presum_run_ms": 1,
                "rings_ms": 1,
                "basis_combine_ms": 1,
                "ligerito_recursive_ms": 1,
            }
            row.update({key: str(value) for key, value in detail.items()})
            row.update(
                forest_ms="5",
                open_ms="3",
                untagged_ms="1",
                profiled_prove_ms="9",
            )
        result[exponent] = row
    return result


class ExportTablesTest(unittest.TestCase):
    def test_named_profile_flows_into_all_outputs(self) -> None:
        headline = EXPORTER.normalized_headline(rows("fast", breakdown=False))
        breakdown = EXPORTER.normalized_breakdown(
            rows("fast", breakdown=True), headline
        )
        payload = EXPORTER.dashboard_payload(headline, breakdown)
        self.assertEqual(payload["series"]["requested_config"], "fast")
        self.assertEqual(headline[0]["resolved_ligerito"], "fast@r1/2k6")
        self.assertEqual(
            headline[0]["resolved_ligerito_label"], "fast (rate 1/2, k=6)"
        )
        self.assertIn(r"\texttt{fast}", EXPORTER.headline_tex(headline, "fast"))
        self.assertIn(
            r"\label{tab:f2z-pcs-fast-headline}",
            EXPORTER.headline_tex(headline, "fast"),
        )

    def test_rejects_mixed_requested_profiles(self) -> None:
        values = rows("fast", breakdown=False)
        values[30]["profile_arg"] = "secure"
        with self.assertRaisesRegex(EXPORTER.InputError, "inconsistent"):
            EXPORTER.normalized_headline(values)

    def test_requested_profile_may_resolve_differently_by_size(self) -> None:
        values = rows("secure", breakdown=False)
        values[20]["lig_geometry"] = "adhoc@r1/4k2"
        values[21]["lig_geometry"] = "adhoc@r1/4k2"
        for exponent in range(22, 31):
            values[exponent]["lig_geometry"] = "secure@r1/2k6"
        headline = EXPORTER.normalized_headline(values)
        self.assertEqual(headline[0]["requested_ligerito"], "secure")
        self.assertEqual(headline[0]["resolved_ligerito"], "adhoc@r1/4k2")
        self.assertEqual(headline[2]["resolved_ligerito"], "secure@r1/2k6")

    def test_dashboard_data_is_opt_in(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            headline_path = root / "headline.csv"
            breakdown_path = root / "breakdown.csv"

            def write_fixture(path: Path, values: dict[int, dict[str, str]]) -> None:
                first = next(iter(values.values()))
                fields = [key for key in first if key != "_source"]
                with path.open("w", newline="", encoding="utf-8") as handle:
                    writer = csv.DictWriter(handle, fieldnames=fields)
                    writer.writeheader()
                    for row in values.values():
                        writer.writerow({key: row[key] for key in fields})

            write_fixture(headline_path, rows("fast", breakdown=False))
            write_fixture(breakdown_path, rows("fast", breakdown=True))

            expected_tables = {
                "headline.csv",
                "headline.tex",
                "prover-breakdown.csv",
                "prover-breakdown.tex",
            }
            for with_dashboard in (False, True):
                out_dir = root / ("dashboard" if with_dashboard else "tables")
                command = [
                    sys.executable,
                    str(SCRIPT),
                    "--headline",
                    str(headline_path),
                    "--breakdown",
                    str(breakdown_path),
                    "--out-dir",
                    str(out_dir),
                    "--expected-profile",
                    "fast",
                ]
                if with_dashboard:
                    command.append("--with-dashboard-data")
                subprocess.run(command, check=True, capture_output=True, text=True)
                actual = {path.name for path in out_dir.iterdir()}
                expected = set(expected_tables)
                if with_dashboard:
                    expected.add("dashboard-data.json")
                self.assertEqual(actual, expected)

if __name__ == "__main__":
    unittest.main()

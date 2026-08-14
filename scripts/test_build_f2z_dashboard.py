#!/usr/bin/env python3
"""Regression tests for build_f2z_dashboard.py."""

from __future__ import annotations

import copy
import importlib.util
import sys
import tempfile
import unittest
from pathlib import Path


SCRIPT = Path(__file__).with_name("build_f2z_dashboard.py")
SPEC = importlib.util.spec_from_file_location("build_f2z_dashboard", SCRIPT)
assert SPEC is not None and SPEC.loader is not None
DASHBOARD = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = DASHBOARD
SPEC.loader.exec_module(DASHBOARD)


def stat(value: float | int, n: int = 21) -> dict[str, object]:
    return {"med": value, "dec": [value] * 9, "n": n}


def event(
    stable_key: str,
    op: str,
    name: str,
    start: float,
    end: float,
    tags: list[str],
) -> dict[str, object]:
    duration = end - start
    return {
        "stable_key": stable_key,
        "label": op,
        "op": op,
        "name": name,
        "short": name,
        "tags": tags,
        "group": tags[0],
        "kind": "phase",
        "start_ms": start,
        "end_ms": end,
        "rep_ms": duration,
        "med_ms": duration,
        "dec_ms": [duration] * 9,
        "n": 21,
        "depth": 1,
        "parent_key": "root",
        "occurrence": None,
    }


def headline_payload() -> dict[str, object]:
    points = []
    for exponent in range(20, 31):
        t_value = exponent // 2
        proof_bytes = 60_000 + exponent
        resolved = "custom-k4@r1/8k4"
        resolved_label = "custom-k4 (rate 1/8, k=4)"
        points.append(
            {
                "i": exponent,
                "witness_bits": 1 << exponent,
                "t": t_value,
                "s": exponent - t_value,
                "word_bits": 1,
                "requested_ligerito": "custom:3:4",
                "resolved_ligerito": resolved,
                "resolved_ligerito_label": resolved_label,
                "threads": "8",
                "reps": 21,
                "prover_ms": float(exponent),
                "verifier_ms": float(exponent) / 10,
                "proof_bytes": proof_bytes,
                "proof_kib": proof_bytes / 1024,
                "prover_peak_heap_mib": float(exponent * 2),
                "commit_ms": 2.0,
                "commit_peak_heap_mib": 1.0,
                "breakdown": {
                    "i": exponent,
                    "requested_ligerito": "custom:3:4",
                    "resolved_ligerito": resolved,
                    "resolved_ligerito_label": resolved_label,
                    "threads": "8",
                    "profiled_runs": 1,
                    "pack_ms": 0.0,
                    "pow2_ms": 1.0,
                    "forest_core_ms": 1.0,
                    "fold_v_ms": 1.0,
                    "presum_tables_ms": 1.0,
                    "presum_run_ms": 1.0,
                    "rings_ms": 1.0,
                    "basis_combine_ms": 1.0,
                    "ligerito_recursive_ms": 1.0,
                    "untagged_ms": 1.0,
                    "forest_presum_ms": 5.0,
                    "ligerito_open_ms": 3.0,
                    "profiled_prove_ms": 9.0,
                },
            }
        )
    return {
        "schema_version": 3,
        "algorithm": "F2Z PCS — custom:3:4 Ligerito",
        "series": {
            "requested_config": "custom:3:4",
            "exponents": [20, 30],
            "word_bits": 1,
            "measurement_boundary": "commit external; prover is PCS opening/proving",
            "peak_definition": "absolute tracked Rust heap high-water",
            "small_instance_derivation": "mechanically derived",
            "breakdown_boundary": "one separately profiled prove",
        },
        "points": points,
    }


def interval_payload() -> dict[str, object]:
    row_order = [
        {"tag": "commit", "label": "Commit"},
        {"tag": "proving", "label": "Proving"},
        {"tag": "serialization", "label": "Serialization"},
        {"tag": "verification", "label": "Verifier time"},
    ]
    sizes = []
    for exponent in range(20, 31):
        t_value = exponent // 2
        proof_bytes = 60_000 + exponent
        primary = [
            event("root/bench:commit", "bench:commit", "Commit", 0, 2, ["commit", "pcs"]),
            event("root/bench:prove", "bench:prove", "Prove", 2, 7, ["proving", "pcs", "opening-proof"]),
            event("root/bench:serialize", "bench:serialize", "Serialize", 7, 8, ["serialization"]),
            event("root/bench:verify", "bench:verify", "Verify", 8, 10, ["verification"]),
        ]
        root_event = {
            "stable_key": "root",
            "label": "bench:trial",
            "op": "bench:trial",
            "name": "Trial",
            "short": "Trial",
            "tags": ["end-to-end"],
            "group": "end-to-end",
            "kind": "scope",
            "start_ms": 0.0,
            "end_ms": 10.0,
            "rep_ms": 10.0,
            "med_ms": 10.0,
            "dec_ms": [10.0] * 9,
            "n": 21,
            "depth": 0,
            "parent_key": None,
            "occurrence": None,
        }
        sizes.append(
            {
                "i": exponent,
                "t": t_value,
                "s": exponent - t_value,
                "word_bits": 1,
                "requested_config": "custom:3:4",
                "resolved_config": "custom-k4@r1/8k4",
                "threads": 8,
                "warmup_n": 1,
                "measured_n": 21,
                "representative_trial": 10,
                "representative_run_id": f"f2z-n{exponent}-sample10",
                "root_rep_ms": 10.0,
                "root_median_ms": 10.0,
                "geometry_scale": 1.0,
                "metrics": {
                    "trial": stat(10.0),
                    "commit": stat(2.0),
                    "prove": stat(5.0),
                    "serialize": stat(1.0),
                    "verify": stat(2.0),
                },
                "root": root_event,
                "primary": primary,
                "rows": {
                    "commit": [primary[0]],
                    "proving": [primary[1]],
                    "serialization": [primary[2]],
                    "verification": [primary[3]],
                },
                "proof": {
                    "total": stat(proof_bytes),
                    "artifacts": {"framing": stat(proof_bytes)},
                },
                "validation": {
                    "sample_indices": list(range(21)),
                    "warmup_indices": [0],
                    "stable_event_count": 4,
                    "real_representative_geometry": True,
                    "all_events_present_every_trial": True,
                    "proof_artifacts_reconcile_every_trial": True,
                },
            }
        )
    return {
        "schema_version": 1,
        "schema": "f2z.interval-data/v1",
        "quantile_method": "Hyndman-Fan Type 7 sample deciles",
        "geometry": "unscaled intervals from a representative measured trial",
        "sizes": sizes,
        "row_order": row_order,
        "validation": {
            "exponents": list(range(20, 31)),
            "strict_complete": True,
            "trace_schema": "zkperf.trace/v1",
            "all_sizes_validated": True,
        },
    }


class BuildDashboardTest(unittest.TestCase):
    def test_valid_inputs_render_interval_and_separate_headline_graph(self) -> None:
        headline = DASHBOARD.validate_payload(headline_payload())
        intervals = DASHBOARD.validate_interval_payload(interval_payload())
        DASHBOARD.validate_cross_inputs(headline, intervals)
        html = DASHBOARD.render_html(headline, intervals)
        self.assertIn(DASHBOARD.GENERATED_MARKER, html[:256])
        self.assertIn("Interval timeline", html)
        self.assertIn("Pinned interval", html)
        self.assertIn("Clean headline scaling", html)
        self.assertIn("Peak tracked heap is the maximum", html)
        self.assertLess(html.index("Clean headline scaling"), html.index("Phase breakdown"))
        self.assertIn('window.addEventListener("scroll", () => hideIntervalTip(true)', html)
        self.assertNotIn("__HEADLINE_DATA__", html)

    def test_rejects_scaled_or_synthetic_geometry(self) -> None:
        intervals = interval_payload()
        intervals["sizes"][0]["geometry_scale"] = 1.2
        with self.assertRaisesRegex(DASHBOARD.DashboardError, "geometry_scale"):
            DASHBOARD.validate_interval_payload(intervals)

    def test_inspector_treats_optional_math_as_an_empty_list(self) -> None:
        headline = DASHBOARD.validate_payload(headline_payload())
        intervals = DASHBOARD.validate_interval_payload(interval_payload())
        self.assertNotIn("math", intervals["sizes"][0]["primary"][0])
        html = DASHBOARD.render_html(headline, intervals)
        self.assertIn(
            "const mathExpressions = Array.isArray(selected.math) ? selected.math : [];",
            html,
        )
        self.assertNotIn("selected.math.length", html)

    def test_rejects_cross_series_proof_mismatch(self) -> None:
        headline = DASHBOARD.validate_payload(headline_payload())
        intervals_value = interval_payload()
        intervals_value["sizes"][0]["proof"]["total"] = stat(99999)
        intervals = DASHBOARD.validate_interval_payload(intervals_value)
        with self.assertRaisesRegex(DASHBOARD.DashboardError, "proof bytes differ"):
            DASHBOARD.validate_cross_inputs(headline, intervals)

    def test_rejects_cross_series_repetition_mismatch(self) -> None:
        headline_value = headline_payload()
        for point in headline_value["points"]:
            point["reps"] = 3
        headline = DASHBOARD.validate_payload(headline_value)
        intervals = DASHBOARD.validate_interval_payload(interval_payload())
        with self.assertRaisesRegex(DASHBOARD.DashboardError, "measured trials"):
            DASHBOARD.validate_cross_inputs(headline, intervals)

    def test_profile_is_data_not_a_dashboard_constant(self) -> None:
        headline_value = headline_payload()
        headline_value["algorithm"] = "F2Z PCS — fast Ligerito"
        headline_value["series"]["requested_config"] = "fast"
        intervals_value = interval_payload()
        for point, size in zip(
            headline_value["points"], intervals_value["sizes"], strict=True
        ):
            point["requested_ligerito"] = "fast"
            point["resolved_ligerito"] = "fast@r1/2k6"
            point["resolved_ligerito_label"] = "fast (rate 1/2, k=6)"
            point["breakdown"]["requested_ligerito"] = "fast"
            point["breakdown"]["resolved_ligerito"] = "fast@r1/2k6"
            point["breakdown"]["resolved_ligerito_label"] = (
                "fast (rate 1/2, k=6)"
            )
            size["requested_config"] = "fast"
            size["resolved_config"] = "fast@r1/2k6"
        headline = DASHBOARD.validate_payload(headline_value)
        intervals = DASHBOARD.validate_interval_payload(intervals_value)
        DASHBOARD.validate_cross_inputs(headline, intervals)
        html = DASHBOARD.render_html(headline, intervals)
        self.assertIn("F2Z PCS · requested <code>fast</code> intervals", html)
        self.assertNotIn("__PROFILE_HTML__", html)

    def test_marker_protects_existing_output(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "dashboard.html"
            path.write_text("hand-authored", encoding="utf-8")
            with self.assertRaisesRegex(DASHBOARD.DashboardError, "marker is absent"):
                DASHBOARD.write_generated(path, "replacement")
            DASHBOARD.write_generated(path, "replacement", adopt_existing=True)
            self.assertEqual(path.read_text(encoding="utf-8"), "replacement")


if __name__ == "__main__":
    unittest.main()

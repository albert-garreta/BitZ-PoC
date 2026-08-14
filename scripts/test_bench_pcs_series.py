#!/usr/bin/env python3
"""Control-flow and ownership tests for the Python PCS benchmark runners."""

from __future__ import annotations

import csv
import os
import shutil
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from unittest import mock


REPO = Path(__file__).resolve().parent.parent
SCRIPTS = REPO / "scripts"
sys.path.insert(0, str(SCRIPTS))

import bench_csv  # noqa: E402
import bench_pcs_intervals  # noqa: E402


CSV_RUNNER = SCRIPTS / "bench_csv.py"
SERIES_RUNNER = SCRIPTS / "bench_pcs_series.py"
INTERVAL_RUNNER = SCRIPTS / "bench_pcs_intervals.py"
CANONICAL_RAW = REPO / "bench_results" / "albert-custom34-20260813" / "raw"


def python_executable(path: Path, source: str) -> None:
    path.write_text(f"#!{sys.executable}\n{source}", encoding="utf-8")
    path.chmod(0o755)


def copy_table_inputs(bundle: Path) -> None:
    raw = bundle / "raw"
    raw.mkdir(parents=True)
    shutil.copytree(CANONICAL_RAW / "headline", raw / "headline")
    shutil.copytree(CANONICAL_RAW / "breakdown", raw / "breakdown")


class BenchPcsSeriesTest(unittest.TestCase):
    def test_csv_parses_all_machine_fields_and_profile(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            fake_bin = root / "bin"
            fake_bin.mkdir()
            python_executable(
                fake_bin / "rustup",
                '''
import os

profile = os.environ.get("F2Z_LIG_PROFILE", "slim")
print(f"""=== n=20 (t=13, s=7, W=1, m_p=13, chunks=1, lig=rounded@r1/4k2, data=128 KiB, live=128/128 elide=1) ===
  config-detail: requested_ligerito={profile} resolved_ligerito=custom-k4@r1/8k4
  commit: 1.0 ms peak 2.0 MB live-after 1.0 MB
  prove: 3.0 ms peak 4.0 MB
  phases: forest+presum 9.0 ms | ligerito open 10.0 ms | untagged 1.0 ms | total 20.0 ms
  phase-detail: forest_total_ms=11.1 opening_total_ms=12.2 untagged_ms=1.3 profiled_prove_ms=24.6 pack_ms=0.1 pow2_ms=1.1 forest_core_ms=2.2 fold_v_ms=3.3 presum_tables_ms=4.4 presum_run_ms=0.0 rings_ms=5.5 basis_combine_ms=6.6 ligerito_recursive_ms=0.1
  verify: 5.0 ms
  proof: 1024 B (1.0 KiB) serialize 6 us / deserialize 7 us
  metric-detail: commit_ms=1.125 commit_peak_mib=2.125 prove_ms=3.125 prove_peak_mib=4.125 verify_ms=5.125 proof_bytes=1025 serialize_us=6.125 deserialize_us=7.125
  split: forest-side 0.4 KiB | open-side 0.6 KiB (s_v 0.1 + lig 0.5)
""")
''',
            )
            output = root / "result.csv"
            environment = os.environ.copy()
            environment["PATH"] = f"{fake_bin}:/usr/bin:/bin"
            result = subprocess.run(
                [
                    sys.executable,
                    str(CSV_RUNNER),
                    "--output",
                    str(output),
                    "--profiles",
                    "custom:3:4",
                    "--shapes",
                    "13:7:1",
                    "--reps",
                    "1",
                    "--threads",
                    "8",
                    "--gap-seconds",
                    "0",
                    "--toolchain",
                    "1.97.1",
                    "--skip-build",
                    "--phases",
                ],
                cwd=REPO,
                env=environment,
                text=True,
                capture_output=True,
                check=False,
            )
            self.assertEqual(result.returncode, 0, result.stderr)
            with output.open(newline="", encoding="utf-8") as handle:
                row = next(csv.DictReader(handle))
            self.assertEqual(tuple(row), bench_csv.CSV_FIELDS)
            self.assertTrue(row.pop("timestamp"))
            self.assertEqual(
                row,
                {
                    "profile_arg": "custom:3:4",
                    "lig_geometry": "custom-k4@r1/8k4",
                    "n": "20",
                    "t": "13",
                    "s": "7",
                    "W": "1",
                    "chunks": "1",
                    "threads": "8",
                    "reps": "1",
                    "commit_ms": "1.125",
                    "commit_peak_mb": "2.125",
                    "forest_ms": "11.1",
                    "open_ms": "12.2",
                    "untagged_ms": "1.3",
                    "profiled_prove_ms": "24.6",
                    "prove_ms": "3.125",
                    "prove_peak_mb": "4.125",
                    "verify_ms": "5.125",
                    "proof_bytes": "1025",
                    "forest_side_kib": "0.4",
                    "s_v_kib": "0.1",
                    "lig_kib": "0.5",
                    "serialize_us": "6.125",
                    "deserialize_us": "7.125",
                    "pack_ms": "0.1",
                    "pow2_ms": "1.1",
                    "forest_core_ms": "2.2",
                    "fold_v_ms": "3.3",
                    "presum_tables_ms": "4.4",
                    "presum_run_ms": "0.0",
                    "rings_ms": "5.5",
                    "basis_combine_ms": "6.6",
                    "ligerito_recursive_ms": "0.1",
                },
            )

    def test_csv_rejects_profile_mismatch_reported_by_rust(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            fake_bin = root / "bin"
            fake_bin.mkdir()
            python_executable(
                fake_bin / "rustup",
                '''
print("""=== n=20 (t=13, s=7, W=1, m_p=13, chunks=1, lig=slim@r1/4k6, data=128 KiB, live=128/128 elide=1) ===
  config-detail: requested_ligerito=slim resolved_ligerito=slim@r1/4k6
  split: forest-side 0.4 KiB | open-side 0.6 KiB (s_v 0.1 + lig 0.5)
""")
''',
            )
            environment = os.environ.copy()
            environment["PATH"] = f"{fake_bin}:/usr/bin:/bin"
            result = subprocess.run(
                [
                    sys.executable,
                    str(CSV_RUNNER),
                    "-o",
                    str(root / "result.csv"),
                    "-p",
                    "custom:3:4",
                    "-s",
                    "13:7:1",
                    "-g",
                    "0",
                    "--skip-build",
                ],
                cwd=REPO,
                env=environment,
                text=True,
                capture_output=True,
                check=False,
            )
            self.assertNotEqual(result.returncode, 0)
            self.assertIn("expected custom:3:4", result.stderr)

    def test_csv_rejects_a_record_without_the_split_terminator(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            fake_bin = root / "bin"
            fake_bin.mkdir()
            python_executable(
                fake_bin / "rustup",
                '''
import os
profile = os.environ["F2Z_LIG_PROFILE"]
print(f"""=== n=20 (t=13, s=7, W=1, m_p=13, chunks=1, lig=x@r1/8k4, data=128 KiB, live=128/128 elide=1) ===
  config-detail: requested_ligerito={profile} resolved_ligerito=x@r1/8k4
  metric-detail: commit_ms=1 commit_peak_mib=2 prove_ms=3 prove_peak_mib=4 verify_ms=5 proof_bytes=1024 serialize_us=6 deserialize_us=7
""")
''',
            )
            output = root / "result.csv"
            environment = os.environ.copy()
            environment["PATH"] = f"{fake_bin}:/usr/bin:/bin"
            result = subprocess.run(
                [
                    sys.executable,
                    str(CSV_RUNNER),
                    "-o",
                    str(output),
                    "-p",
                    "custom:3:4",
                    "-s",
                    "13:7:1",
                    "-g",
                    "0",
                    "--skip-build",
                ],
                cwd=REPO,
                env=environment,
                text=True,
                capture_output=True,
                check=False,
            )
            self.assertNotEqual(result.returncode, 0)
            self.assertIn("missing proof split terminator", result.stderr)
            self.assertEqual(len(output.read_text(encoding="utf-8").splitlines()), 1)

    def test_default_sweep_clears_ambient_profile_and_schedule(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            fake_bin = root / "bin"
            fake_bin.mkdir()
            python_executable(
                fake_bin / "rustup",
                '''
import os
if "F2Z_LIG_PROFILE" in os.environ or "F2_FOREST_SCHEDULE" in os.environ:
    raise SystemExit(23)
print("""=== n=20 (t=13, s=7, W=1, m_p=13, chunks=1, lig=slim@r1/4k6, data=128 KiB, live=128/128 elide=1) ===
  config-detail: requested_ligerito=slim resolved_ligerito=slim@r1/4k6
  metric-detail: commit_ms=1 commit_peak_mib=2 prove_ms=3 prove_peak_mib=4 verify_ms=5 proof_bytes=1024 serialize_us=6 deserialize_us=7
  split: forest-side 0.4 KiB | open-side 0.6 KiB (s_v 0.1 + lig 0.5)
""")
''',
            )
            environment = os.environ.copy()
            environment["PATH"] = f"{fake_bin}:/usr/bin:/bin"
            environment["F2Z_LIG_PROFILE"] = "custom:3:4"
            environment["F2_FOREST_SCHEDULE"] = "l8"
            result = subprocess.run(
                [
                    sys.executable,
                    str(CSV_RUNNER),
                    "--output",
                    str(root / "result.csv"),
                    "--shapes",
                    "13:7:1",
                    "--gap-seconds",
                    "0",
                    "--skip-build",
                ],
                cwd=REPO,
                env=environment,
                text=True,
                capture_output=True,
                check=False,
            )
            self.assertEqual(result.returncode, 0, result.stderr)

    def test_report_only_does_not_invoke_rust(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            bundle = root / "bundle"
            copy_table_inputs(bundle)
            fake_bin = root / "bin"
            fake_bin.mkdir()
            calls = root / "rust-calls.txt"
            for name in ("rustup", "cargo"):
                python_executable(
                    fake_bin / name,
                    f'''from pathlib import Path
Path({str(calls)!r}).write_text("called", encoding="utf-8")
raise SystemExit(99)
''',
                )
            environment = os.environ.copy()
            environment["PATH"] = f"{fake_bin}:/usr/bin:/bin"
            result = subprocess.run(
                [
                    sys.executable,
                    str(SERIES_RUNNER),
                    "--profile",
                    "custom:3:4",
                    "--out-dir",
                    str(bundle),
                    "--report-only",
                ],
                cwd=REPO,
                env=environment,
                text=True,
                capture_output=True,
                check=False,
            )
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertFalse(calls.exists())
            self.assertEqual(
                {path.name for path in (bundle / "report").iterdir()},
                {
                    "headline.csv",
                    "headline.tex",
                    "prover-breakdown.csv",
                    "prover-breakdown.tex",
                },
            )
            self.assertFalse((bundle / "raw" / "intervals").exists())

    def test_series_has_no_interval_or_html_dependencies(self) -> None:
        source = SERIES_RUNNER.read_text(encoding="utf-8")
        for forbidden in (
            "bench_pcs_intervals",
            "export_f2z_intervals",
            "build_f2z_dashboard",
            "INTERVALS_DIR",
            "HTML_OUT",
            "--html",
        ):
            with self.subTest(forbidden=forbidden):
                self.assertNotIn(forbidden, source)

    def test_series_rejects_html_argument(self) -> None:
        result = subprocess.run(
            [
                sys.executable,
                str(SERIES_RUNNER),
                "--profile",
                "custom:3:4",
                "--out-dir",
                "/tmp/unused-f2z-bundle",
                "--html",
                "/tmp/unused-f2z-dashboard.html",
            ],
            cwd=REPO,
            text=True,
            capture_output=True,
            check=False,
        )
        self.assertEqual(result.returncode, 2)
        self.assertIn("unrecognized arguments: --html", result.stderr)

    def test_manifest_does_not_read_or_hash_interval_artifacts(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            bundle = root / "bundle"
            copy_table_inputs(bundle)
            interval_dir = bundle / "raw" / "intervals"
            interval_dir.mkdir()
            interval_sentinel = interval_dir / "sentinel.jsonl"
            interval_sentinel.write_bytes(b"interval sentinel")
            report_dir = bundle / "report"
            report_dir.mkdir()
            interval_json = report_dir / "interval-data.json"
            interval_json.write_bytes(b"aggregate sentinel")
            (bundle / "run-spec.txt").write_text(
                "profile=custom:3:4\n", encoding="utf-8"
            )

            result = subprocess.run(
                [
                    sys.executable,
                    str(SERIES_RUNNER),
                    "--profile",
                    "custom:3:4",
                    "--out-dir",
                    str(bundle),
                    "--report-only",
                ],
                cwd=REPO,
                text=True,
                capture_output=True,
                check=False,
            )
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertEqual(interval_sentinel.read_bytes(), b"interval sentinel")
            self.assertEqual(interval_json.read_bytes(), b"aggregate sentinel")
            manifest = (bundle / "manifest.txt").read_text(encoding="utf-8")
            self.assertNotIn("intervals", manifest)
            self.assertNotIn("interval-data.json", manifest)

    def test_report_only_requires_an_explicit_bundle(self) -> None:
        result = subprocess.run(
            [
                sys.executable,
                str(SERIES_RUNNER),
                "--profile",
                "custom:3:4",
                "--report-only",
            ],
            cwd=REPO,
            text=True,
            capture_output=True,
            check=False,
        )
        self.assertEqual(result.returncode, 2)
        self.assertIn("--out-dir is required", result.stderr)

    def test_interval_capture_promotes_only_validated_parts(self) -> None:
        shape = bench_csv.Shape.parse("13:7:1")
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory)
            completed = subprocess.CompletedProcess([], 0)
            with mock.patch.object(
                bench_pcs_intervals.subprocess, "run", return_value=completed
            ), mock.patch.object(bench_pcs_intervals, "validate_part") as validate:
                bench_pcs_intervals.capture_shape(
                    shape,
                    output_dir=output,
                    selected_profile="custom:3:4",
                    threads=8,
                    toolchain="1.97.1",
                    rustflags="-C target-cpu=native",
                    cpu="test cpu",
                    capture_id="test-capture",
                )
            self.assertTrue((output / "n20" / "console.log").is_file())
            self.assertFalse(any(output.glob(".n20.partial.*")))
            validate.assert_called_once()

    def test_interval_capture_preserves_failed_parts(self) -> None:
        shape = bench_csv.Shape.parse("13:7:1")
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory)
            failed = subprocess.CompletedProcess([], 9)
            with mock.patch.object(
                bench_pcs_intervals.subprocess, "run", return_value=failed
            ):
                with self.assertRaises(bench_pcs_intervals.IntervalError):
                    bench_pcs_intervals.capture_shape(
                        shape,
                        output_dir=output,
                        selected_profile="custom:3:4",
                        threads=8,
                        toolchain="1.97.1",
                        rustflags="-C target-cpu=native",
                        cpu="test cpu",
                        capture_id="test-capture",
                    )
            preserved = list(output.glob(".n20.failed-*"))
            self.assertEqual(len(preserved), 1)
            self.assertTrue((preserved[0] / "console.log").is_file())
            self.assertFalse((output / "n20").exists())

    def test_interval_capture_preserves_a_part_when_promotion_fails(self) -> None:
        shape = bench_csv.Shape.parse("13:7:1")
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory)
            completed_dir = output / "n20"
            completed_dir.mkdir()
            (completed_dir / "blocker").write_text("keep", encoding="utf-8")
            completed = subprocess.CompletedProcess([], 0)
            with mock.patch.object(
                bench_pcs_intervals.subprocess, "run", return_value=completed
            ), mock.patch.object(bench_pcs_intervals, "validate_part"):
                with self.assertRaises(OSError):
                    bench_pcs_intervals.capture_shape(
                        shape,
                        output_dir=output,
                        selected_profile="custom:3:4",
                        threads=8,
                        toolchain="1.97.1",
                        rustflags="-C target-cpu=native",
                        cpu="test cpu",
                        capture_id="test-capture",
                    )
            preserved = list(output.glob(".n20.failed-*"))
            self.assertEqual(len(preserved), 1)
            self.assertTrue((preserved[0] / "console.log").is_file())
            self.assertEqual(
                (completed_dir / "blocker").read_text(encoding="utf-8"), "keep"
            )

    def test_benchmark_pipeline_has_no_shell_scripts(self) -> None:
        self.assertEqual(sorted(SCRIPTS.glob("*.sh")), [])

    def test_live_files_do_not_reference_removed_shell_entrypoints(self) -> None:
        old_names = tuple(
            stem + "." + "sh"
            for stem in ("bench_csv", "bench_pcs_series", "bench_pcs_intervals")
        )
        live_files = [
            REPO / "README.md",
            REPO / "benches" / "pcs.rs",
            REPO / "docs" / "pcs-benchmark-series.md",
            *sorted(SCRIPTS.glob("*.py")),
        ]
        for path in live_files:
            source = path.read_text(encoding="utf-8")
            for old_name in old_names:
                with self.subTest(path=path, old_name=old_name):
                    self.assertNotIn(old_name, source)

    def test_python_runners_never_request_a_shell(self) -> None:
        for path in (CSV_RUNNER, SERIES_RUNNER, INTERVAL_RUNNER):
            source = path.read_text(encoding="utf-8")
            with self.subTest(path=path):
                self.assertNotIn("shell=True", source)
                self.assertNotIn("/bin/bash", source)
                self.assertNotIn("/bin/sh", source)


if __name__ == "__main__":
    unittest.main()

#!/usr/bin/env python3
"""Offline parser checks using saved baseline logs; launches no Rust code."""

import copy
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest


HERE = Path(__file__).resolve().parent
SPEC = importlib.util.spec_from_file_location("forest_analysis", HERE / "analyze-forest.py")
ANALYSIS = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(ANALYSIS)
BASELINE = HERE / "baseline-profiles"
FIXTURE = BASELINE / "baseline-n512-t8-seed42"


class ForestAnalysisTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.result = ANALYSIS.read_json(FIXTURE / "result.json")
        cls.command = ANALYSIS.read_json(FIXTURE / "command.json")
        cls.log = (FIXTURE / "stderr.log").read_text()

    def test_matches_previous_baseline_analysis(self):
        current = ANALYSIS.analyze(BASELINE)
        previous = ANALYSIS.read_json(BASELINE / "forest-analysis.json")
        self.assertEqual(len(current["rows"]), 2)
        for row in current["rows"]:
            self.assertEqual((row["samples"], row["processes"], row["excluded_warmups"]), (9, 3, 3))
            old = previous["summary"][str(row["threads"])]
            for category in ["derived_ms", "trace_ms", "stage_ms"]:
                for name, value in old[category].items():
                    self.assertAlmostEqual(row["medians"][category][name], value, places=10)

    def test_warmup_count_is_not_hardcoded(self):
        original = ANALYSIS.parse_trace(self.log, self.result["job"], self.result["trials"])
        job = dict(self.result["job"], warmup=2, iterations=2)
        trials = copy.deepcopy(self.result["trials"])
        trials[1]["trial"] = "warmup"
        parsed = ANALYSIS.parse_trace(self.log, job, trials)
        self.assertEqual([sample["derived_ms"] for sample in parsed],
                         [sample["derived_ms"] for sample in original[1:]])

    def test_retained_label_and_time_units(self):
        changed = self.log.replace("L4 rebuild", "L4 retained")
        parsed = ANALYSIS.parse_trace(changed, self.result["job"], self.result["trials"])
        original = ANALYSIS.parse_trace(self.log, self.result["job"], self.result["trials"])
        for before, after in zip(original, parsed):
            self.assertEqual(after["derived_ms"]["l4_rebuild_ms"], 0)
            self.assertEqual(after["derived_ms"]["l4_retained_ms"], before["derived_ms"]["l4_rebuild_ms"])
            self.assertEqual(after["derived_ms"]["l4_remaining_ms"], before["derived_ms"]["l4_remaining_ms"])
        for suffix, duration in [("1000000ns", 1), ("1000µs", 1), ("1000us", 1), ("1ms", 1), ("1s", 1000)]:
            label, number, unit = ANALYSIS.TRACE.match(f"bitz: level 0 {suffix} 0 MB faulted").groups()
            self.assertEqual(label, "level 0")
            self.assertEqual(float(number) * ANALYSIS.UNITS[unit], duration)

    def test_rejects_incomplete_misaligned_and_wrong_warmup(self):
        with self.assertRaisesRegex(ValueError, "incomplete forest"):
            ANALYSIS.parse_trace(self.log.replace("level 0 ", "level 99 ", 1),
                                 self.result["job"], self.result["trials"])
        lines = self.log.splitlines()
        index = next(i for i, line in enumerate(lines) if '"name":"falcon_bridge:wfbitz_forest"' in line)
        del lines[index]
        with self.assertRaisesRegex(ValueError, "no preceding forest"):
            ANALYSIS.parse_trace("\n".join(lines), self.result["job"], self.result["trials"])
        with self.assertRaisesRegex(ValueError, "trial labels/counts"):
            ANALYSIS.parse_trace(self.log, dict(self.result["job"], warmup=0, iterations=4), self.result["trials"])

    def test_synthetic_campaign_grouping_skip_and_hash_validation(self):
        with tempfile.TemporaryDirectory(prefix="forest-parser-") as temporary:
            root = Path(temporary)
            (root / "phases").mkdir()
            for phase, configs, traced in [("diagnostic", ["baseline", "retain"], True), ("latency", ["baseline"], False)]:
                jobs = []
                for config in configs:
                    job = dict(self.result["job"], id=f"{config}-synthetic", config=config, binary="candidate")
                    jobs.append(job)
                    directory = root / "runs" / phase / job["id"]
                    directory.mkdir(parents=True)
                    command = copy.deepcopy(self.command)
                    command["overrides"]["BITZ_FOREST_L4"] = "retain" if config == "retain" else "rebuild"
                    if not traced:
                        command["command"].remove("--trace")
                        command["overrides"].pop("BITZ_TRACE")
                    (directory / "command.json").write_text(json.dumps(command))
                    log = self.log.replace("L4 rebuild", "L4 retained") if config == "retain" else self.log
                    (directory / "stderr.log").write_text(log if traced else "")
                    result = dict(job=job, verified=True, trials=self.result["trials"],
                                  command_sha256=ANALYSIS.sha256(directory / "command.json"),
                                  stderr_sha256=ANALYSIS.sha256(directory / "stderr.log"))
                    (directory / "result.json").write_text(json.dumps(result))
                (root / "phases" / f"{phase}.json").write_text(json.dumps(dict(phase=phase, trace=traced, jobs=jobs)))
            parsed = ANALYSIS.analyze(root)
            self.assertEqual(len(parsed["rows"]), 2)
            self.assertEqual(len(parsed["skipped"]), 1)
            self.assertEqual({row["config"] for row in parsed["rows"]}, {"baseline", "retain"})
            self.assertTrue(all(row["trace"] and row["samples"] == 3 for row in parsed["rows"]))
            self.assertEqual(len(ANALYSIS.analyze(root, ["diagnostic"])["skipped"]), 0)
            (root / "runs" / "diagnostic" / "retain-synthetic" / "stderr.log").write_text("tampered")
            with self.assertRaisesRegex(ValueError, "stderr.log hash mismatch"):
                ANALYSIS.analyze(root)


if __name__ == "__main__":
    unittest.main()

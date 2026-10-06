import copy
import json
from pathlib import Path
import tempfile
import unittest
from unittest import mock

import qualify_falcon_one_source as qualification


class StatisticsTests(unittest.TestCase):
    def test_ratio_is_arithmetic_means_not_mean_or_geometric_ratio(self):
        result = qualification.mean_ratio_interval([1., 100.], [2., 90.], draws=1000)
        self.assertAlmostEqual(result["ratio"], 92 / 101)
        self.assertNotAlmostEqual(result["ratio"], (2 + .9) / 2)
        self.assertNotAlmostEqual(result["ratio"], (2 * .9) ** .5)

    def test_pairs_are_resampled_together(self):
        result = qualification.mean_ratio_interval([1., 10., 100.], [1.01, 10.1, 101.], draws=1000)
        self.assertAlmostEqual(result["lower_one_sided_95"], 1.01)
        self.assertAlmostEqual(result["upper_one_sided_95"], 1.01)
        self.assertEqual(qualification.timing_status(result), "pass")

    def test_invalid_and_unpaired_values_fail(self):
        for baseline, candidate in [([], []), ([1.], [1.]), ([1., 2.], [1.]),
                                    ([0., 2.], [1., 2.]), ([1., 2.], [float("nan"), 2.])]:
            with self.assertRaises(ValueError):
                qualification.mean_ratio_interval(baseline, candidate)

    def test_exact_threshold_and_uncertainty(self):
        self.assertEqual(qualification.timing_status(dict(upper_one_sided_95=1.02, lower_one_sided_95=1.01)), "pass")
        self.assertEqual(qualification.timing_status(dict(upper_one_sided_95=1.03, lower_one_sided_95=1.01)), "inconclusive")
        self.assertEqual(qualification.timing_status(dict(upper_one_sided_95=1.04, lower_one_sided_95=1.03)), "regression")


class ValidationTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        root = Path(__file__).resolve().parents[1]
        path = root / "results/falcon-joint-forest-20261006/performance/latency/s128-b1024-t16-seed42-v4.jsonl"
        cls.fixture = [json.loads(line) for line in path.read_text().splitlines()]
        cls.build = dict(protocols={"shared-prime": "bitz/falcon1024-ct/hybrid/shared-prime/non-zk/v4"},
                         rustflags="-C target-cpu=native", target_arch="x86_64", root_count=3)

    def validate(self, rows):
        return qualification.validate_rows(rows, self.build, "shared-prime", 128, 1024, 16, 42, 1, 3, False)

    def test_valid_historical_record(self):
        result = self.validate(copy.deepcopy(self.fixture))
        self.assertEqual(result["payload"], 1294874)

    def test_unverified_warmup_fails(self):
        rows = copy.deepcopy(self.fixture)
        rows[1]["verified"] = False
        with self.assertRaisesRegex(ValueError, "unverified trial"):
            self.validate(rows)

    def test_insufficient_security_fails(self):
        rows = copy.deepcopy(self.fixture)
        rows[0]["security_terms"][0]["error_bound"] = .1
        with self.assertRaisesRegex(ValueError, "security target"):
            self.validate(rows)

    def test_missing_root_and_bad_profile_fail(self):
        rows = copy.deepcopy(self.fixture)
        rows[1]["roots"] = []
        with self.assertRaisesRegex(ValueError, "root count"):
            self.validate(rows)
        rows = copy.deepcopy(self.fixture)
        rows[0]["integer_bridge"] = "wfbitz-unsplit"
        with self.assertRaisesRegex(ValueError, "wrong bridge"):
            self.validate(rows)

    def test_roots_may_change_across_versions_but_inputs_may_not(self):
        baseline = self.validate(copy.deepcopy(self.fixture))
        candidate = copy.deepcopy(baseline)
        candidate["samples"][0]["roots"] = ["a" * 64]
        qualification.validate_pair(baseline, candidate)
        candidate["header"]["input_digest"] = "b" * 64
        with self.assertRaisesRegex(ValueError, "unmatched workload"):
            qualification.validate_pair(baseline, candidate)

    def test_partial_qualification_cannot_pass(self):
        result = self.validate(copy.deepcopy(self.fixture))
        candidate = copy.deepcopy(result)
        candidate["payload"] = 700000
        pairs = {("shared-prime", 128, 1024): {42: dict(baseline=result, candidate=candidate)}}
        self.assertNotEqual(qualification.summarize(pairs, True, True)["status"], "pass")
        self.assertNotEqual(qualification.summarize(pairs, False, True)["status"], "pass")

    def test_payload_gate_uses_mean_bytes_not_worst_seed(self):
        result = self.validate(copy.deepcopy(self.fixture))
        result["payload"] = 1000
        candidates = [copy.deepcopy(result), copy.deepcopy(result)]
        candidates[0]["payload"], candidates[1]["payload"] = 810, 790
        pairs = {("shared-prime", 128, 1024): {
            seed: dict(baseline=result, candidate=candidate)
            for seed, candidate in zip((42, 43), candidates)}}
        cell = qualification.summarize(pairs, False, True)["cells"][0]
        self.assertEqual(cell["payload"]["ratio_of_mean_bytes"], .8)
        self.assertEqual(cell["payload"]["maximum_ratio"], .81)
        self.assertTrue(cell["payload"]["pass_gate"])

    def test_stop_marker_finishes_pair_and_cannot_qualify(self):
        result = self.validate(copy.deepcopy(self.fixture))
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            builds_path = root / "builds.json"
            baseline = {key: "matched" for key in qualification.BUILD_MATCH}
            baseline.update(revision="51405193878e15e1e4259216464f2bb856c8b592", root_count=3)
            candidate = dict(baseline, root_count=1)
            builds_path.write_text(json.dumps(dict(binaries=dict(baseline=baseline, candidate=candidate))))
            output = root / "results"
            calls = []

            def run_one(args, builds, label, case, seed, index, dest):
                calls.append(label)
                if label == "baseline":
                    (dest / "STOP").write_text("Operator requested a new candidate build.")
                return copy.deepcopy(result)

            argv = ["qualify", "--builds", str(builds_path), "--output", str(output)]
            with mock.patch("sys.argv", argv), \
                    mock.patch.object(qualification, "verify_build"), \
                    mock.patch.object(qualification, "verify_final_integrity"), \
                    mock.patch.object(qualification, "run_one", side_effect=run_one), \
                    mock.patch.object(qualification.bench_gate, "acquire"), \
                    mock.patch.object(qualification.bench_gate, "release"), \
                    mock.patch.object(qualification.bench_gate, "wait_idle"), \
                    mock.patch("builtins.print"):
                self.assertEqual(qualification.main(), 2)
            self.assertEqual(calls, ["baseline", "candidate"])
            summary = json.loads((output / "qualification/summary.json").read_text())
            manifest = json.loads((output / "qualification/manifest.json").read_text())
            self.assertEqual(summary["status"], "not_qualified")
            self.assertFalse(summary["complete"])
            self.assertEqual(summary["verified_proofs_in_complete_pairs"], 8)
            self.assertEqual(manifest["completed_pairs"], 1)
            self.assertEqual(manifest["status"], "stopped_at_pair_boundary")


if __name__ == "__main__":
    unittest.main()

import copy
import json
from pathlib import Path
import tempfile
import unittest

import run_sha256_ecdsa_compare as campaign


class CampaignTests(unittest.TestCase):
    def setUp(self):
        self.case = dict(method="f2z-split", log_compressions=3, r=None, c=None,
                         security_target=100, threads=1, seed=0)
        self.rows = []
        for sample in range(2):
            self.rows.append(dict(self.case, schema=campaign.SCHEMA, verified=True, sample=sample,
                                  trial="sample" if sample else "warmup", compressions=8, message_bytes=448,
                                  signatures=1, statement_bytes=129, fixture_id="a"*64,
                                  spartan_revision="b"*40,
                                  security={"model": "round-by-round-economic"},
                                  **dict.fromkeys(campaign.METRICS, 0)))

    def test_f2z_is_not_duplicated_per_chunking(self):
        cases = list(campaign.cases([(0, 3), (1, 2), (3, 0)], campaign.METHODS, [100, 128], [1], [0]))
        self.assertEqual(len(cases), 7)
        self.assertEqual(sum(c["method"] == "spartan-mc" for c in cases), 3)
        self.assertTrue(all(c["security_target"] is None for c in cases if c["method"] == "spartan-mc"))

    def test_validation_requires_complete_verified_matched_samples(self):
        self.assertTrue(campaign.validate_rows(self.rows, self.case, 1))
        for key, value in [("verified", False), ("message_bytes", 512), ("sample", 9),
                           ("fixture_id", "b"*64), ("method", "spartan-mc"),
                           ("r", 3), ("prove_ms", 100), ("security", None),
                           ("spartan_revision", None), ("verify_ms", float("nan")),
                           ("opening_ms", 1), ("opening_ms", None)]:
            rows = copy.deepcopy(self.rows)
            rows[1][key] = value
            self.assertFalse(campaign.validate_rows(rows, self.case, 1), key)
        self.assertFalse(campaign.validate_rows(self.rows[:1], self.case, 1))

    def test_summary_detects_cross_method_fixture_mismatch_and_retains_failure(self):
        with tempfile.TemporaryDirectory() as path:
            directory = Path(path)
            for method, fixture_id in [("f2z-split", "a"*64), ("f2z-all", "b"*64)]:
                rows = copy.deepcopy(self.rows)
                for row in rows:
                    row.update(method=method, fixture_id=fixture_id)
                result = dict(case=dict(self.case, method=method), rows=rows, status="complete", peak_rss_bytes=123)
                (directory / f"{method}.result.json").write_text(json.dumps(result))
            failed = dict(case=dict(self.case, method="spartan-mc"), rows=[], status="timeout", peak_rss_bytes=None)
            (directory / "failure.result.json").write_text(json.dumps(failed))
            malformed = dict(case=self.case, rows=[dict(self.rows[1], security=None)],
                             status="failed", peak_rss_bytes=None)
            (directory / "malformed.result.json").write_text(json.dumps(malformed))
            self.assertFalse(campaign.summarize(directory))
            summary = (directory / "summary.csv").read_text()
            self.assertIn("timeout", summary)
            self.assertFalse(json.loads((directory / "comparison.json").read_text())["matched_fixtures"])

    def test_summary_uses_per_sample_totals(self):
        with tempfile.TemporaryDirectory() as path:
            directory = Path(path)
            rows = []
            for witness, prove in [(1, 100), (100, 1), (100, 100)]:
                row = copy.deepcopy(self.rows[1])
                row.update(witness_ms=witness, prove_ms=prove, witness_to_proof_ms=witness+prove)
                rows.append(row)
            result = dict(case=self.case, rows=rows, status="complete", peak_rss_bytes=123)
            (directory / "case.result.json").write_text(json.dumps(result))
            self.assertTrue(campaign.summarize(directory))
            import csv
            with (directory / "summary.csv").open() as stream:
                summary = next(csv.DictReader(stream))
            self.assertEqual(float(summary["witness_to_proof_ms"]), 101)

    def test_protocol_breakdown_uses_per_sample_differences_and_exports_every_trial(self):
        import csv
        with tempfile.TemporaryDirectory() as path:
            directory = Path(path)
            rows = [copy.deepcopy(self.rows[0])]
            for sample, (protocol, opening) in enumerate([(20, 10), (100, 90), (100, 10)], 1):
                row = copy.deepcopy(self.rows[1])
                row.update(sample=sample, protocol_ms=protocol, opening_ms=opening)
                rows.append(row)
            result = dict(case=self.case, rows=rows, status="complete", peak_rss_bytes=123)
            raw = json.dumps(result)
            raw_path = directory / "case.result.json"
            raw_path.write_text(raw)
            self.assertTrue(campaign.summarize(directory))
            self.assertEqual(raw_path.read_text(), raw)
            with (directory / "summary.csv").open() as stream:
                summary = next(csv.DictReader(stream))
            self.assertEqual(float(summary["piop_ms"]), 10)
            self.assertEqual(float(summary["iop_ms"]), 10)
            with (directory / "samples.csv").open() as stream:
                samples = list(csv.DictReader(stream))
            self.assertEqual(len(samples), 4)
            self.assertEqual(samples[0]["trial"], "warmup")
            self.assertEqual([float(s["piop_ms"]) for s in samples[1:]], [10, 10, 90])
            self.assertTrue(all(s["peak_rss_bytes"] == "123" for s in samples))


if __name__ == "__main__":
    unittest.main()

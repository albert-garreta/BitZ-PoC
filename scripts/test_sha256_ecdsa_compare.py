import copy
import json
from pathlib import Path
import tempfile
import unittest

import run_sha256_ecdsa_compare as campaign
from test_ligerito_results import report as ligerito_report


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
                                  security={"model": "round-by-round-economic", "ligerito": ligerito_report()},
                                  **dict.fromkeys(campaign.METRICS, 0)))

    def test_f2z_is_not_duplicated_per_chunking(self):
        cases = list(campaign.cases([(0, 3), (1, 2), (3, 0)], campaign.METHODS, [100, 128], [1], [0]))
        self.assertEqual(len(cases), 8)
        self.assertEqual(sum(c["method"] == "spartan-mc" for c in cases), 3)
        honk = [c for c in cases if c["method"] == "zkpassport-honk"]
        self.assertEqual(len(honk), 1)
        self.assertIsNone(honk[0]["security_target"])
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

    def test_mixed_ligerito_regimes_are_rejected(self):
        rows = copy.deepcopy(self.rows)
        rows[1]["security"]["ligerito"] = ligerito_report(False)
        self.assertFalse(campaign.validate_rows(rows, self.case, 1))

    def test_resume_rejects_changed_ligerito_profile(self):
        old = dict(binary_sha256="native", ligerito_profile="custom:3:4")
        self.assertTrue(campaign.compatible_manifest(old, copy.deepcopy(old)))
        self.assertFalse(campaign.compatible_manifest(old, dict(old, ligerito_profile="udrg:3:4")))

    def test_honk_requires_non_zk_and_keeps_unavailable_phases_null(self):
        case = dict(self.case, method="zkpassport-honk", security_target=None)
        rows = copy.deepcopy(self.rows)
        for row in rows:
            row.update(case, zk=False, barretenberg_version="5.0.0", zkpassport_revision="c"*40,
                       spartan_revision=None, witness_ms=2, prove_ms=3, witness_to_proof_ms=5)
            for key in ["commit_ms", "protocol_ms", "outer_ms", "inner_ms", "opening_ms", "folding_ms"]:
                row[key] = None
        self.assertTrue(campaign.validate_rows(rows, case, 1))
        self.assertIsNone(campaign.sample_metrics(rows[0])["piop_ms"])
        self.assertIsNone(campaign.sample_metrics(rows[0])["iop_ms"])
        for key, value in [("zk", True), ("barretenberg_version", "4.0.0"),
                           ("zkpassport_revision", None), ("witness_to_proof_ms", 3), ("prove_ms", None)]:
            bad = copy.deepcopy(rows)
            bad[1][key] = value
            self.assertFalse(campaign.validate_rows(bad, case, 1), key)

    def test_resume_allows_failed_compilation_to_succeed_but_rejects_changed_inputs(self):
        old = dict(source_hash="source", binary_sha256="binary",
                   artifacts={"3": "abc", "4": "preparation_failed"})
        new = copy.deepcopy(old)
        new["artifacts"]["4"] = "def"
        self.assertTrue(campaign.compatible_zkpassport(old, new))
        for key, value in [("source_hash", "changed"), ("binary_sha256", "changed"),
                           ("artifacts", {"3": "changed", "4": "def"})]:
            bad = dict(new, **{key: value})
            self.assertFalse(campaign.compatible_zkpassport(old, bad))
        self.assertFalse(campaign.compatible_zkpassport(None, new))

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

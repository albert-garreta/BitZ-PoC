import copy
import json
from pathlib import Path
import tempfile
import resource
import unittest
from unittest.mock import patch

import run_sha256_ecdsa_compare as campaign
from test_ligerito_results import report as ligerito_report


class CampaignTests(unittest.TestCase):
    def test_address_space_limit_only_applies_on_linux(self):
        for platform in ["darwin", "win32"]:
            with patch.object(campaign.sys, "platform", platform):
                self.assertIsNone(campaign.address_space_limit(4))
        with patch.object(campaign.sys, "platform", "linux"):
            limit = campaign.address_space_limit(4)
            with patch.object(resource, "setrlimit") as setrlimit:
                limit()
            size = 4 * 1024**3
            setrlimit.assert_called_once_with(resource.RLIMIT_AS, (size, size))

    def test_peak_memory_units_on_linux_and_macos(self):
        with tempfile.TemporaryDirectory() as path:
            linux = Path(path) / "rss-kib"
            stderr = Path(path) / "stderr"
            linux.write_text("1048576\n")
            stderr.write_text("worker diagnostic\n  1073741824  maximum resident set size\n")
            for platform in ["linux", "darwin"]:
                with patch.object(campaign.sys, "platform", platform):
                    self.assertEqual(campaign.peak_rss_bytes(linux, stderr), 1073741824)

    def setUp(self):
        self.case = dict(method="bitz-split", curve="p256", log_compressions=3, r=None, c=None,
                         security_target=100, threads=1, seed=0)
        self.rows = []
        for sample in range(2):
            self.rows.append(dict(self.case, schema=campaign.SCHEMA, verified=True, sample=sample,
                                  trial="sample" if sample else "warmup", compressions=8, message_bytes=448,
                                  signatures=1, statement_bytes=129, fixture_id="a"*64,
                                  spartan_revision="b"*40, zk=False, fixture_profile=campaign.FIXTURE_SCHEMA,
                                  circuit_profile=campaign.circuit_profile("p256", "bitz-split"),
                                  security={"model": "round-by-round-economic", "ligerito": ligerito_report()},
                                  **dict.fromkeys(campaign.METRICS, 0)))

    def test_bitz_is_not_duplicated_per_chunking(self):
        p256_methods = ["bitz-split", "bitz-all", "spartan-mc"]
        cases = list(campaign.cases("p256", [(0, 3), (1, 2), (3, 0)], p256_methods, [100, 128], [1], [0]))
        # bitz-split/bitz-all: 2 targets x 2 profiles each; spartan: 3 splits.
        self.assertEqual(len(cases), 4 + 4 + 3)
        self.assertEqual(sum(c["method"] == "spartan-mc" for c in cases), 3)
        self.assertTrue(all(c["curve"] == "p256" for c in cases))
        bitz = [c for c in cases if c["method"] == "bitz-split"]
        self.assertEqual({c["ligerito_profile"] for c in bitz}, {"custom:1:4", "custom:3:4"})
        self.assertTrue(all(c["security_target"] is None for c in cases if c["method"] == "spartan-mc"))
        secp = list(campaign.cases("secp256k1", [(3, 0)], campaign.DEFAULT_METHODS, [100, 128], [1], [0]))
        # bitz-split: 2 targets x 2 profiles; binius64: 2 targets x 2 rates;
        # binius64-ligerito: fixed 100-bit gate x 2 rates.
        self.assertEqual(len(secp), 4 + 4 + 2)
        binius = [c for c in secp if c["method"] == "binius64"]
        self.assertEqual(len(binius), 4)
        self.assertEqual({c["security_target"] for c in binius}, {100, 128})
        self.assertEqual({c["log_inv_rate"] for c in binius}, {1, 3})
        opener = [c for c in secp if c["method"] == "binius64-ligerito"]
        self.assertEqual({c["security_target"] for c in opener}, {100})
        self.assertEqual({c["log_inv_rate"] for c in opener}, {1, 3})
        self.assertTrue(all(c["curve"] == "secp256k1" for c in secp))
        self.assertNotIn("zkpassport-honk", campaign.METHODS)
        self.assertEqual(campaign.DEFAULT_METHODS, ["bitz-split", "binius64", "binius64-ligerito"])

    def test_binius_is_refused_on_p256_and_spartan_on_secp256k1(self):
        # The pinned fork's P-256 gadget is not a Binius64 verifier: no row.
        for method in ["binius64", "binius64-ligerito"]:
            reasons = campaign.unsupported_methods("p256", ["bitz-split", method])
            self.assertEqual(len(reasons), 1)
            self.assertIn("no P-256 verifier of its own", reasons[0])
            with self.assertRaises(ValueError):
                list(campaign.cases("p256", [(3, 0)], ["bitz-split", method], [100], [1], [0]))
        self.assertEqual(campaign.unsupported_methods("secp256k1", ["spartan-mc"]), ["spartan-mc: no secp256k1 circuit"])
        self.assertEqual(campaign.unsupported_methods("secp256k1", campaign.DEFAULT_METHODS), [])
        self.assertEqual(campaign.unsupported_methods("p256", ["bitz-split", "bitz-all", "spartan-mc"]), [])
        # Every recordable pairing names exactly one circuit, distinct per curve.
        self.assertNotEqual(campaign.circuit_profile("p256", "bitz-split"), campaign.circuit_profile("secp256k1", "bitz-split"))
        self.assertEqual(campaign.circuit_profile("secp256k1", "binius64"), campaign.circuit_profile("secp256k1", "binius64-ligerito"))
        self.assertIsNone(campaign.circuit_profile("p256", "binius64"))

    def test_rows_must_name_the_curve_fixture_and_matched_circuit(self):
        self.assertTrue(campaign.validate_rows(self.rows, self.case, 1))
        for key, value in [("curve", "secp256k1"), ("fixture_profile", campaign.FIXTURE_SCHEMAS["secp256k1"]),
                           ("circuit_profile", campaign.circuit_profile("secp256k1", "bitz-split")),
                           ("circuit_profile", None)]:
            rows = copy.deepcopy(self.rows)
            rows[1][key] = value
            self.assertFalse(campaign.validate_rows(rows, self.case, 1), key)
        secp_case = dict(self.case, curve="secp256k1")
        secp_rows = copy.deepcopy(self.rows)
        for row in secp_rows:
            row.update(curve="secp256k1", fixture_profile=campaign.FIXTURE_SCHEMAS["secp256k1"],
                       circuit_profile=campaign.circuit_profile("secp256k1", "bitz-split"))
        self.assertTrue(campaign.validate_rows(secp_rows, secp_case, 1))
        # A P-256 paper-circuit row cannot be recorded into a secp256k1 case.
        wrong = copy.deepcopy(secp_rows)
        for row in wrong:
            row["circuit_profile"] = campaign.circuit_profile("p256", "bitz-split")
        self.assertFalse(campaign.validate_rows(wrong, secp_case, 1))

    def test_validation_requires_complete_verified_matched_samples(self):
        self.assertTrue(campaign.validate_rows(self.rows, self.case, 1))
        for key, value in [("verified", False), ("message_bytes", 512), ("sample", 9),
                           ("fixture_id", "b"*64), ("method", "spartan-mc"),
                           ("r", 3), ("prove_ms", 100), ("security", None),
                           ("spartan_revision", None), ("verify_ms", float("nan")),
                           ("opening_ms", 1), ("opening_ms", None), ("zk", True), ("fixture_profile", "low-s/v1"), ("e2e_prover_ms", None)]:
            rows = copy.deepcopy(self.rows)
            rows[1][key] = value
            self.assertFalse(campaign.validate_rows(rows, self.case, 1), key)
        self.assertFalse(campaign.validate_rows(self.rows[:1], self.case, 1))

    def test_mixed_ligerito_regimes_are_rejected(self):
        rows = copy.deepcopy(self.rows)
        rows[1]["security"]["ligerito"] = ligerito_report(False)
        self.assertFalse(campaign.validate_rows(rows, self.case, 1))

    def test_wall_clock_requires_explicit_backend_and_unavailable_phases(self):
        case = dict(self.case, timing="wall-clock")
        rows = copy.deepcopy(self.rows)
        for row in rows:
            row.update(timing="wall-clock", outer_ms=None, inner_ms=None,
                       opening_ms=None, folding_ms=None,
                       phases_seconds=[], verify_phases_seconds=[])
        self.assertTrue(campaign.validate_rows(rows, case, 1))
        self.assertFalse(campaign.validate_rows(rows, self.case, 1))
        for key, value in [("timing", "perfetto"), ("timing", "unknown"),
                           ("verify_ms", None), ("e2e_prover_ms", None),
                           ("verified", False), ("opening_ms", 0), ("inner_ms", 0)]:
            bad = copy.deepcopy(rows)
            bad[1][key] = value
            self.assertFalse(campaign.validate_rows(bad, case, 1), key)
        with tempfile.TemporaryDirectory() as path:
            directory = Path(path)
            (directory / "case.result.json").write_text(json.dumps(dict(
                case=case, rows=rows, status="complete", peak_rss_bytes=123)))
            self.assertTrue(campaign.summarize(directory))
            import csv
            with (directory / "summary.csv").open() as stream:
                summary = next(csv.DictReader(stream))
            self.assertEqual(summary["timing"], "wall-clock")
            self.assertEqual(summary["piop_ms"], "")
            self.assertEqual(summary["iop_ms"], "")

    def test_resume_rejects_changed_timing_backend(self):
        old = dict(binary_sha256="native", runner_sha256="runner", timing="perfetto")
        self.assertFalse(campaign.compatible_manifest(old, dict(old, timing="wall-clock")))

    def test_resume_rejects_changed_ligerito_profile(self):
        old = dict(binary_sha256="native", ligerito_profile="custom:3:4")
        self.assertTrue(campaign.compatible_manifest(old, copy.deepcopy(old)))
        self.assertFalse(campaign.compatible_manifest(old, dict(old, ligerito_profile="udrg:3:4")))

    def test_binius_requires_matching_security_rate_and_its_own_revision(self):
        case = dict(self.case, method="binius64", curve="secp256k1", log_inv_rate=3)
        rows = copy.deepcopy(self.rows)
        for row in rows:
            row.update(case, binius_revision="c"*40, spartan_revision=None,
                       fixture_profile=campaign.FIXTURE_SCHEMAS["secp256k1"],
                       circuit_profile=campaign.circuit_profile("secp256k1", "binius64"),
                       security={"model":"query target", "pcs":"BaseFold", "fri_query_target_bits":100,
                                 "log_inv_rate":3})
        self.assertTrue(campaign.validate_rows(rows, case, 1))
        self.assertFalse(campaign.validate_rows(rows, case, 1, binius_log_inv_rate=2))
        for rate in (1, 2, 3):
            rated_case = dict(case, log_inv_rate=rate)
            rated_rows = copy.deepcopy(rows)
            for row in rated_rows:
                row["log_inv_rate"] = rate
                row["security"]["log_inv_rate"] = rate
            self.assertTrue(campaign.validate_rows(rated_rows, rated_case, 1))
            self.assertTrue(campaign.validate_rows(rated_rows, rated_case, 1, binius_log_inv_rate=rate))
        for key, value in [("binius_revision", None), ("zk", True), ("circuit_profile", "sha256-chain-p256/standard/v1"),
                           ("security", {"model":"query target", "pcs":"BaseFold", "fri_query_target_bits":96, "log_inv_rate":3}),
                           ("security", {"model":"query target", "pcs":"BaseFold", "fri_query_target_bits":100, "log_inv_rate":1}),
                           ("log_inv_rate", 1)]:
            bad = copy.deepcopy(rows)
            bad[1][key] = value
            self.assertFalse(campaign.validate_rows(bad, case, 1), key)

    def test_opener_rows_require_the_round_by_round_gate(self):
        case = dict(self.case, method="binius64-ligerito", curve="secp256k1", log_inv_rate=1)
        rows = copy.deepcopy(self.rows)
        good = {"model":"Binius64 PIOP with the BitZ opener", "pcs":"BitZ-Ligerito",
                "accounting":"round-by-round", "target_bits":100, "round_by_round_bits":100.4,
                "union_bound_bits":97.2, "log_inv_rate":1}
        for row in rows:
            row.update(case, binius_revision="c"*40, spartan_revision=None,
                       fixture_profile=campaign.FIXTURE_SCHEMAS["secp256k1"],
                       circuit_profile=campaign.circuit_profile("secp256k1", "binius64-ligerito"), security=dict(good))
        self.assertTrue(campaign.validate_rows(rows, case, 1))
        for override in [dict(accounting="union-bound"), dict(round_by_round_bits=99.9),
                         dict(pcs="BaseFold"), dict(target_bits=96), dict(log_inv_rate=3)]:
            bad = copy.deepcopy(rows)
            bad[1]["security"] = dict(good, **override)
            self.assertFalse(campaign.validate_rows(bad, case, 1), str(override))
        wrong_target = copy.deepcopy(rows)
        case_128 = dict(case, security_target=128)
        for row in wrong_target:
            row["security_target"] = 128
        self.assertFalse(campaign.validate_rows(wrong_target, case_128, 1))

    def test_bitz_cases_pin_profile_and_level_rate(self):
        case = dict(self.case, ligerito_profile="custom:3:4")
        rows = copy.deepcopy(self.rows)
        for row in rows:
            row["ligerito_profile"] = "custom:3:4"
            row["security"]["ligerito"]["configuration"]["levels"][0]["log_inv_rate"] = 3
        self.assertTrue(campaign.validate_rows(rows, case, 1))
        wrong_request = copy.deepcopy(rows)
        for row in wrong_request:
            row["security"]["ligerito"]["requested_profile"] = "custom:1:4"
            row["security"]["ligerito"]["resolved_profile"] = "custom:1:4"
        self.assertFalse(campaign.validate_rows(wrong_request, case, 1))
        wrong_rate = copy.deepcopy(rows)
        for row in wrong_rate:
            row["security"]["ligerito"]["configuration"]["levels"][0]["log_inv_rate"] = 1
        self.assertFalse(campaign.validate_rows(wrong_rate, case, 1))
        unlabeled = copy.deepcopy(rows)
        del unlabeled[1]["ligerito_profile"]
        self.assertFalse(campaign.validate_rows(unlabeled, case, 1))

    def test_resume_rejects_changed_binaries_fixtures_and_fork_revision(self):
        old = dict(binary_sha256="native", runner_sha256="runner", fixtures={"profile":campaign.FIXTURE_SCHEMA, "files":{"fixture":"hash"}},
                   binius64={"binary_sha256":"binius", "binius_revision":"c"*40}, curve="p256")
        self.assertTrue(campaign.compatible_manifest(old, copy.deepcopy(old)))
        self.assertFalse(campaign.compatible_manifest(old, dict(old, curve="secp256k1")))
        self.assertFalse(campaign.compatible_manifest(dict(old, binius_log_inv_rate=1),
                                                      dict(old, binius_log_inv_rate=2)))
        for key, value in [("binary_sha256", "changed"), ("runner_sha256", "changed"), ("fixtures", {}), ("binius64", None)]:
            self.assertFalse(campaign.compatible_manifest(old, dict(old, **{key:value})))

    def test_summary_detects_cross_method_fixture_mismatch_and_retains_failure(self):
        with tempfile.TemporaryDirectory() as path:
            directory = Path(path)
            for method, fixture_id in [("bitz-split", "a"*64), ("bitz-all", "b"*64)]:
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
            comparison = json.loads((directory / "comparison.json").read_text())
            self.assertFalse(comparison["matched_fixtures"])
            self.assertEqual(comparison["curve"], "p256")
            self.assertTrue(comparison["matched_circuits"])
            self.assertEqual(comparison["circuit_profiles"]["bitz-split"], [campaign.circuit_profile("p256", "bitz-split")])

    def test_summary_flags_mixed_curves_or_circuits(self):
        with tempfile.TemporaryDirectory() as path:
            directory = Path(path)
            for curve in ["p256", "secp256k1"]:
                rows = copy.deepcopy(self.rows)
                for row in rows:
                    row.update(curve=curve, fixture_profile=campaign.FIXTURE_SCHEMAS[curve],
                               circuit_profile=campaign.circuit_profile(curve, "bitz-split"))
                result = dict(case=dict(self.case, curve=curve), rows=rows, status="complete", peak_rss_bytes=1)
                (directory / f"{curve}.result.json").write_text(json.dumps(result))
            self.assertFalse(campaign.summarize(directory))
            comparison = json.loads((directory / "comparison.json").read_text())
            self.assertEqual(comparison["curve"], ["p256", "secp256k1"])
            self.assertFalse(comparison["matched_circuits"])

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

import json
from pathlib import Path
import tempfile
import unittest

import sha256_ecdsa_table as table
from test_ligerito_results import report as ligerito_report

SECP = ("secp256k1-matched", "sha256-chain-secp256k1/binius-matched/v1")
SECP_BINIUS = ("secp256k1", "sha256-chain-secp256k1/standard/v1")
P256 = ("p256-paper", "sha256-chain-p256/paper/v1")


def ligerito(rate):
    """The shared Ligerito fixture, reported at the rate the case asked for."""
    report = ligerito_report()
    report["configuration"]["levels"][0].update(log_inv_rate=rate, queries=183 if rate == 1 else 60,
                                                grinding_bits=16, fold_grinding_bits=8)
    return report


def rows(method, circuit, threads, rate, metrics):
    """One warm-up and two verified samples of one case, as the worker records them."""
    curve, profile = circuit
    security = ({"model": "round-by-round-economic", "economic_bits": 100.0, "ligerito": ligerito(rate)}
                if method.startswith("bitz") else
                {"log_inv_rate": rate, "fri_queries": 241, "fri_query_target_bits": 100, "merkle_hash": "SHA-256"}
                if method == "binius64" else
                {"log_inv_rate": rate, "component_bits": 100, "round_by_round_bits": 100.0})
    return [dict(schema=table.SCHEMA, verified=True, sample=sample, trial="sample" if sample else "warmup",
                 method=method, curve=curve, circuit_profile=profile, threads=threads,
                 compressions=16, message_bytes=960, fixture_id="a" * 64,
                 circuit={"gates": 1}, security=security,
                 **{k: v for k, v in metrics.items()})
            for sample in range(3)]


def case(method, circuit, threads=1, rate=1, **metrics):
    measured = dict(witness_ms=1.0, prove_ms=10.0, e2e_prover_ms=11.0, verify_ms=2.0, setup_ms=3.0,
                    opening_ms=4.0, proof_material_bytes=100000.0)
    measured.update(metrics)
    record = dict(method=method, log_compressions=4, r=None, c=None, security_target=100, threads=threads, seed=0)
    if method.startswith("binius64"):
        record["log_inv_rate"] = rate
    else:
        record["ligerito_profile"] = "custom:1:4" if rate == 1 else "custom:3:4"
    return dict(status="complete", peak_rss_bytes=1 << 30, case=record,
                rows=rows(method, circuit, threads, rate, measured))


def campaign(directory, curve, cases, manifest=True):
    directory.mkdir(parents=True, exist_ok=True)
    if manifest:
        (directory / "manifest.json").write_text(json.dumps({"curve": curve, "cpu": "Apple M5"}))
    for index, record in enumerate(cases):
        (directory / f"case{index}.result.json").write_text(json.dumps(record))
    return directory


class TableTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.root = Path(self.tmp.name)
        self.out = self.root / "out"
        self.out.mkdir()

    def generate(self, *run_dirs, extra=()):
        table.main([str(d) for d in run_dirs] + ["--out-dir", str(self.out)] + list(extra))
        return {path.name: path.read_text() for path in self.out.glob("*.tex")}

    def test_each_curve_gets_its_own_table_from_one_invocation(self):
        secp = campaign(self.root / "secp", "secp256k1",
                        [case("bitz-split", SECP), case("binius64", SECP_BINIUS)])
        p256 = campaign(self.root / "p256", "p256", [case("bitz-split", P256)])
        written = self.generate(secp, p256)
        self.assertEqual(sorted(written), ["sha256-ecdsa-p256-table.tex", "sha256-ecdsa-secp256k1-table.tex"])
        head, solo = written["sha256-ecdsa-secp256k1-table.tex"], written["sha256-ecdsa-p256-table.tex"]
        self.assertIn("\\label{tab:sha256-ecdsa-secp256k1}", head)
        self.assertIn("secp256k1 ECDSA signature verification", head)
        # Neither table may carry the other curve's label, statement or rows.
        self.assertNotIn("P-256", head)
        self.assertNotIn("\\label{tab:sha256-ecdsa-p256}", head)
        self.assertNotIn("secp256k1 ECDSA signature verification", solo)
        self.assertNotIn(SECP[1], solo)
        self.assertNotIn("Binius (UDR)", solo)

    def test_the_matched_and_paper_circuits_are_named_in_the_caption(self):
        secp = campaign(self.root / "secp", "secp256k1",
                        [case("bitz-split", SECP), case("binius64", SECP_BINIUS)])
        p256 = campaign(self.root / "p256", "p256", [case("bitz-split", P256)])
        written = self.generate(secp, p256)
        self.assertIn("on Binius64's scalar-multiplication schedule", written["sha256-ecdsa-secp256k1-table.tex"])
        self.assertIn("its own secp256k1 ECDSA verifier", written["sha256-ecdsa-secp256k1-table.tex"])
        self.assertIn("complete additions throughout", written["sha256-ecdsa-p256-table.tex"])

    def test_a_p256_table_without_binius_says_why(self):
        p256 = campaign(self.root / "p256", "p256", [case("bitz-split", P256)])
        text = self.generate(p256)["sha256-ecdsa-p256-table.tex"]
        self.assertIn("Binius64 implements ECDSA over secp256k1 only and has no row here", text)
        # A solo table has one security policy, so the multi-scheme disclaimer does not belong in it.
        self.assertNotIn("Native security targets are reported separately", text)

    def test_a_p256_binius_row_is_rendered_but_marked_as_ported(self):
        """Campaigns that predate the split still render; their Binius row says what it measured."""
        p256 = campaign(self.root / "p256", "p256",
                        [case("bitz-split", P256), case("binius64", ("p256", "sha256-chain-p256/standard/v1"))])
        text = self.generate(p256)["sha256-ecdsa-p256-table.tex"]
        self.assertIn("ported into the pinned Binius64 tree for this comparison", text)

    def test_rows_of_two_curves_in_one_directory_are_refused(self):
        mixed = campaign(self.root / "mixed", None,
                         [case("bitz-split", SECP), case("binius64", ("p256", "sha256-chain-p256/standard/v1"))],
                         manifest=False)
        with self.assertRaises(SystemExit) as refusal:
            self.generate(mixed)
        self.assertIn("several curves", str(refusal.exception))

    def test_a_manifest_disagreeing_with_its_rows_is_refused(self):
        lying = campaign(self.root / "lying", "secp256k1", [case("bitz-split", P256)])
        with self.assertRaises(SystemExit) as refusal:
            self.generate(lying)
        self.assertIn("manifest names curve", str(refusal.exception))

    def test_one_family_measured_on_two_circuits_cannot_share_a_table(self):
        """Merging directories can pair rows the runner would never record together."""
        old = campaign(self.root / "old", "secp256k1", [case("bitz-split", SECP)])
        new = campaign(self.root / "new", "secp256k1",
                       [case("bitz-split", ("secp256k1", "sha256-chain-secp256k1/optimized/v1"), threads=10)])
        with self.assertRaises(SystemExit) as refusal:
            self.generate(old, new)
        self.assertIn("different circuits", str(refusal.exception))

    def test_one_case_whose_samples_disagree_is_refused(self):
        record = case("bitz-split", SECP)
        record["rows"][1]["circuit_profile"] = "sha256-chain-secp256k1/optimized/v1"
        directory = campaign(self.root / "drift", "secp256k1", [record])
        with self.assertRaises(SystemExit) as refusal:
            self.generate(directory)
        self.assertIn("ran different circuits", str(refusal.exception))

    def test_naming_one_output_across_curves_is_refused(self):
        secp = campaign(self.root / "secp", "secp256k1", [case("bitz-split", SECP)])
        p256 = campaign(self.root / "p256", "p256", [case("bitz-split", P256)])
        for flag, value in [("--out", str(self.out / "one.tex")), ("--label", "tab:one")]:
            with self.assertRaises(SystemExit) as refusal:
                self.generate(secp, p256, extra=[flag, value])
            self.assertIn("span curves", str(refusal.exception))

    def test_ratios_are_recorded_against_binius_at_equal_rate(self):
        secp = campaign(self.root / "secp", "secp256k1", [
            case("bitz-split", SECP, prove_ms=10.0, verify_ms=2.0, proof_material_bytes=100000.0),
            case("binius64", SECP_BINIUS, prove_ms=25.0, verify_ms=3.0, proof_material_bytes=300000.0)])
        text = self.generate(secp)["sha256-ecdsa-secp256k1-table.tex"]
        self.assertIn("2^4 rate 1/2: prover 2.50x, verifier 1.50x; proof 3.00x", text)
        # Peak memory shares the legend: the two rows here have equal RSS, so it must not read as a win.
        self.assertIn("peak mem. 1.00x", text)
        self.assertIn(">1 favours \\ftwoz-SNARK, <1 favours Binius", text)

    def test_the_regenerate_line_reproduces_the_invocation(self):
        p256 = campaign(self.root / "p256", "p256", [case("bitz-split", P256)])
        text = self.generate(p256)["sha256-ecdsa-p256-table.tex"]
        self.assertIn(f"%   python3 scripts/sha256_ecdsa_table.py {p256} --out-dir {self.out}", text)

    def test_rows_from_before_the_curve_field_are_read_as_p256(self):
        legacy = case("bitz-split", (None, None))
        directory = campaign(self.root / "legacy", None, [legacy], manifest=False)
        written = self.generate(directory)
        self.assertEqual(sorted(written), ["sha256-ecdsa-p256-table.tex"])

    def test_an_unverified_or_foreign_sample_is_refused(self):
        for field, value in [("verified", False), ("schema", "f2z/sha256-ecdsa-compare/v1")]:
            record = case("bitz-split", SECP)
            record["rows"][1][field] = value
            directory = campaign(self.root / f"bad-{field}", "secp256k1", [record])
            with self.assertRaises(SystemExit) as refusal:
                self.generate(directory)
            self.assertIn("complete case without verified samples", str(refusal.exception))


if __name__ == "__main__":
    unittest.main()

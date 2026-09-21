import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

import sha256_ecdsa_table as table
from test_ligerito_results import report as ligerito_report

ROOT = Path(__file__).resolve().parents[1]


def sample(method, curve, rate, **overrides):
    security = {"model": "round-by-round-economic", "economic_bits": 100.4, "ligerito": ligerito_report()}
    security["ligerito"]["configuration"]["levels"][0]["log_inv_rate"] = rate
    if method.startswith("binius64"):
        security = {"model": "query target", "pcs": "BaseFold", "fri_query_target_bits": 100, "fri_queries": 200,
                    "log_inv_rate": rate, "merkle_hash": "SHA-256"}
        if method == "binius64-ligerito":
            security = {"model": "opener", "pcs": "BitZ-Ligerito", "accounting": "round-by-round", "target_bits": 100,
                        "round_by_round_bits": 100.2, "log_inv_rate": rate, "component_bits": 100}
    row = dict(schema=table.SCHEMA, trial="sample", verified=True, curve=curve,
               circuit_profile=table.EXPECTED_PROFILES.get((curve, table.family(method))),
               fixture_profile=f"bitz/sha256-ecdsa-fixture/standard-{curve}/v1", fixture_id="f" * 64,
               compressions=16, message_bytes=960, security=security, circuit={"rows": 1},
               witness_ms=1.0, prove_ms=10.0, e2e_prover_ms=12.0, verify_ms=3.0, proof_material_bytes=70000, setup_ms=5.0,
               opening_ms=4.0)
    row.update(overrides)
    return row


def write_case(directory, method, curve, rate, threads=1, **overrides):
    case = dict(method=method, curve=curve, log_compressions=4, r=None, c=None, security_target=100, threads=threads, seed=0)
    if method.startswith("bitz"):
        case["ligerito_profile"] = f"custom:{rate}:4"
    else:
        case["log_inv_rate"] = rate
    rows = [dict(sample(method, curve, rate, **overrides), trial="warmup", sample=0)] + \
           [dict(sample(method, curve, rate, **overrides), sample=i) for i in (1, 2, 3)]
    record = dict(case=case, status="complete", returncode=0, rows=rows, peak_rss_bytes=400 << 20)
    (directory / f"{method}-{curve}-rate{rate}-t{threads}.result.json").write_text(json.dumps(record))


class TableTests(unittest.TestCase):
    def run_generator(self, dirs, extra=()):
        with tempfile.TemporaryDirectory() as out:
            out_path = Path(out) / "table.tex"
            proc = subprocess.run([sys.executable, str(ROOT / "scripts/sha256_ecdsa_table.py"), *map(str, dirs),
                                   "--out", str(out_path), *extra], capture_output=True, text=True, cwd=ROOT)
            return proc, out_path.read_text() if out_path.exists() else ""

    def test_secp256k1_head_to_head_records_both_circuits(self):
        with tempfile.TemporaryDirectory() as path:
            directory = Path(path)
            for method in ["bitz-split", "binius64", "binius64-ligerito"]:
                for rate in (1, 3):
                    for threads in (1, 10):
                        write_case(directory, method, "secp256k1", rate, threads)
            proc, tex = self.run_generator([directory], ["--label", "tab:test"])
            self.assertEqual(proc.returncode, 0, proc.stderr)
            self.assertIn("% Curve: secp256k1", tex)
            self.assertIn(table.EXPECTED_PROFILES[("secp256k1", "bitz")], tex)
            self.assertIn(table.EXPECTED_PROFILES[("secp256k1", "binius64")], tex)
            self.assertIn("stock upstream secp256k1 verifier", tex)
            self.assertIn("\\label{tab:test}", tex)
            self.assertIn("Binius (UDR), rate $1/2$", tex)
            self.assertEqual(tex.count("\\ftwoz-SNARK, rate $1/2$ &"), 1)

    def test_p256_is_bitz_alone_and_names_the_paper_circuit(self):
        with tempfile.TemporaryDirectory() as path:
            directory = Path(path)
            write_case(directory, "bitz-split", "p256", 1)
            write_case(directory, "bitz-split", "p256", 3)
            proc, tex = self.run_generator([directory])
            self.assertEqual(proc.returncode, 0, proc.stderr)
            self.assertIn("% Curve: p256", tex)
            self.assertIn("no P-256 verifier of its own", tex)
            self.assertIn("paper's P-256 verifier", tex)
            self.assertNotIn("Binius (UDR)", tex)

    def test_mixed_curves_and_wrong_circuits_are_refused(self):
        with tempfile.TemporaryDirectory() as path:
            p256 = Path(path) / "p256"
            secp = Path(path) / "secp"
            p256.mkdir()
            secp.mkdir()
            write_case(p256, "bitz-split", "p256", 1)
            write_case(secp, "bitz-split", "secp256k1", 1)
            proc, _ = self.run_generator([p256, secp])
            self.assertNotEqual(proc.returncode, 0)
            self.assertIn("mix curves", proc.stderr)
            wrong = Path(path) / "wrong"
            wrong.mkdir()
            write_case(wrong, "bitz-split", "secp256k1", 1, circuit_profile=table.EXPECTED_PROFILES[("p256", "bitz")])
            proc, _ = self.run_generator([wrong])
            self.assertNotEqual(proc.returncode, 0)
            self.assertIn("expected", proc.stderr)
            binius_p256 = Path(path) / "binius-p256"
            binius_p256.mkdir()
            write_case(binius_p256, "binius64", "p256", 1, circuit_profile="sha256-chain-p256/standard/v1")
            proc, _ = self.run_generator([binius_p256])
            self.assertNotEqual(proc.returncode, 0)


if __name__ == "__main__":
    unittest.main()

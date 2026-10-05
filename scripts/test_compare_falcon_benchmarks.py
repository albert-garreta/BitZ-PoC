#!/usr/bin/env python3
"""Regression tests for fail-closed Falcon benchmark acceptance."""
import json
from pathlib import Path
import tempfile
import unittest

import compare_falcon_benchmarks as gate


def header(warmup=2, measured=3):
    return {"schema": gate.SCHEMA, "event": "prepared", "batch": 1, "capacity": 1,
            "security_target": 128, "threads": 1, "input_seed": 42, "input_digest": "a" * 64,
            "protocol": "bitz/falcon1024-ct/hybrid/native-ring/non-zk/v4",
            "integer_bridge": "wfbitz-joint-limbs", "target_arch": "x86_64",
            "gf128_kernel": "test", "compiled_target_features": {"avx2": True},
            "input_source": "pornin/rust-fn-dsa", "input_implementation_version": "0.3.0",
            "input_mode": "original-falcon-1024", "input_rng": "fixed", "input_count": 1,
            "public_inputs": ["public_key", "message", "signature_nonce", "signature_s2"],
            "source_bits_per_signature": [131072, 1048576, 524288],
            "native_verify_includes_public_key_decode": True, "stage_timings": False,
            "warmup_trials": warmup, "measured_trials": measured,
            "build_rustflags": "-C target-cpu=native", "runtime_rustflags": "-C target-cpu=native",
            "algebraic_security_bits": 140.0,
            "security_terms": [{"name": "retained", "error_bound": 2.0 ** -140, "bits": 140.0}]}


def rows(warmup=2, measured=3):
    h = header(warmup, measured)
    result = [h]
    for i in range(warmup + measured):
        result.append(dict(schema=gate.SCHEMA, event="trial", batch=1, capacity=1,
                           security_target=128, threads=1, input_digest=h["input_digest"],
                           trial="warmup" if i < warmup else "sample",
                           sample=None if i < warmup else i - warmup + 1,
                           total_prover_ms=10.0, witness_commit_ms=4.0, proof_prove_ms=6.0,
                           proof_verify_ms=2.0, proof_payload_bytes=1000,
                           proof_payload_definition=gate.PAYLOAD, process_peak_rss_kib=5000,
                           proof_debug_digest="b" * 64, verified=True))
    result.append(dict(schema=gate.SCHEMA, event="summary", batch=1, capacity=1,
                       security_target=128, threads=1, samples=measured, verified=True,
                       median_total_prover_ms=10.0, median_proof_verify_ms=2.0))
    return result


def manifest():
    return dict(schema="bitz/falcon-campaign/v1", status="complete", kind="paired",
                pairing_id="campaign-one", revision="1" * 40, binary_sha256="2" * 64,
                lock_sha256="3" * 64, host="will", architecture="x86_64", rustc="rustc test",
                rustflags="-C target-cpu=native", features=["falcon-hybrid"], profile="release",
                cpu_model="test CPU", affinity=[0, 1], security=[128], batches=[1], threads=[1],
                seeds=list(gate.SEEDS), warmup=2, iterations=3)


def campaign(ratio=1.0):
    m = manifest()
    blocks = {}
    for seed in gate.SEEDS:
        for block in range(6):
            record = rows()
            p = record[0]
            p["input_seed"] = seed
            summary = record[-1]
            summary["median_total_prover_ms"] *= ratio
            summary["median_proof_verify_ms"] *= ratio
            samples = [row for row in record if row.get("trial") == "sample"]
            blocks[(128, 1, 1, seed, str(block))] = dict(
                prepared=p, summary=summary, samples=samples,
                security_error=2.0 ** -140, pair_id=str(block),
                order="AB" if block < 3 else "BA", rss=5000)
    return m, blocks


class ComparisonTests(unittest.TestCase):
    def test_paired_improvement_passes(self):
        report = gate.compare(campaign(), campaign(.9))
        self.assertEqual(report["cells"][0]["status"], "pass")
        self.assertEqual(report["status"], "inconclusive")
        self.assertFalse(report["matrix_complete"])
        self.assertEqual(report["cells"][0]["process_pairs"], 30)

    def test_full_matrix_with_all_pairs_can_pass(self):
        before, after = (manifest(), {}), (manifest(), {})
        for security, batch, threads in gate.EXPECTED_CELLS:
            for destination, ratio in ((before, 1.0), (after, .9)):
                for key, block in campaign(ratio)[1].items():
                    block["prepared"].update(security_target=security, batch=batch, threads=threads)
                    destination[1][(security, batch, threads, key[3], key[4])] = block
        report = gate.compare(before, after)
        self.assertTrue(report["matrix_complete"])
        self.assertEqual(report["status"], "pass")

    def test_equal_constant_measurements_pass_at_exact_boundary(self):
        self.assertEqual(gate.compare(campaign(), campaign())["cells"][0]["status"], "pass")

    def test_even_small_slowdown_fails(self):
        self.assertEqual(gate.compare(campaign(), campaign(1.000001))["status"], "regression")

    def test_uncertain_interval_is_not_a_pass(self):
        after = campaign()
        for i, block in enumerate(after[1].values()):
            ratio = .9 if i % 2 else 1.1
            block["summary"]["median_total_prover_ms"] *= ratio
        report = gate.compare(campaign(), after)
        self.assertEqual(report["status"], "inconclusive")
        self.assertGreater(report["cells"][0]["timings"]["total_prover_ms"]["interval_95"][1], 1)

    def test_initial_campaign_cannot_pass_timing(self):
        before, after = campaign(), campaign(.5)
        before[0]["kind"] = after[0]["kind"] = "initial-baseline"
        self.assertEqual(gate.compare(before, after)["status"], "inconclusive")

    def test_incomplete_or_unbalanced_pairing_cannot_pass(self):
        for kind in ("missing", "order"):
            before, after = campaign(), campaign(.5)
            if kind == "missing":
                before[1].pop((128, 1, 1, 42, "0"))
                after[1].pop((128, 1, 1, 42, "0"))
            else:
                for data in (before, after):
                    data[1][(128, 1, 1, 42, "0")]["order"] = "BA"
            self.assertEqual(gate.compare(before, after)["status"], "inconclusive")

    def test_one_extra_payload_byte_fails(self):
        after = campaign(.5)
        next(iter(after[1].values()))["samples"][0]["proof_payload_bytes"] += 1
        self.assertEqual(gate.compare(campaign(), after)["status"], "regression")

    def test_one_extra_rss_kib_fails(self):
        after = campaign(.5)
        next(iter(after[1].values()))["rss"] += 1
        self.assertEqual(gate.compare(campaign(), after)["status"], "regression")

    def test_host_or_build_mismatch_rejected(self):
        after = campaign()
        after[0]["rustflags"] = "different"
        with self.assertRaisesRegex(ValueError, "build/host"):
            gate.compare(campaign(), after)

    def test_weaker_security_rejected(self):
        after = campaign()
        next(iter(after[1].values()))["security_error"] *= 2
        with self.assertRaisesRegex(ValueError, "weaker security"):
            gate.compare(campaign(), after)

    def test_cross_revision_debug_representation_is_not_wire_format(self):
        after = campaign(.9)
        after[0]["revision"] = "4" * 40
        for block in after[1].values():
            for sample in block["samples"]:
                sample["proof_debug_digest"] = "c" * 64
        self.assertEqual(gate.compare(campaign(), after)["cells"][0]["status"], "pass")


class ArtifactValidationTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.root = Path(self.temp.name)

    def tearDown(self):
        self.temp.cleanup()

    def write_rows(self, name, records):
        path = self.root / name
        path.write_text("".join(json.dumps(row) + "\n" for row in records))
        return path

    def test_valid_process(self):
        parsed = gate.read_process(self.write_rows("case.jsonl", rows()), 2, 3)
        self.assertEqual(len(parsed["samples"]), 3)

    def test_invalid_proof_or_metric_rejected(self):
        for field, value in [("verified", False), ("total_prover_ms", float("nan")),
                             ("total_prover_ms", 12.0), ("proof_payload_bytes", 1.2),
                             ("process_peak_rss_kib", None), ("sample", 19)]:
            with self.subTest(field=field, value=value):
                data = rows()
                data[3][field] = value
                with self.assertRaises(ValueError):
                    gate.read_process(self.write_rows("case.jsonl", data), 2, 3)

    def test_summary_and_security_totals_must_match(self):
        for mutate in (lambda data: data[-1].update(median_total_prover_ms=1),
                       lambda data: data[0].update(algebraic_security_bits=139),
                       lambda data: data[0].update(stage_timings=True)):
            data = rows()
            mutate(data)
            with self.assertRaises(ValueError):
                gate.read_process(self.write_rows("case.jsonl", data), 2, 3)

    def test_missing_or_mismatched_memory_rejected(self):
        m = manifest()
        m.update(seeds=[42], cases=[dict(label="case", exit_code=0)])
        (self.root / "manifest.json").write_text(json.dumps(m))
        self.write_rows("case.jsonl", rows())
        with self.assertRaises(OSError):
            gate.read_campaign(self.root)
        memory = rows(0, 1)
        memory[0]["input_seed"] = 43
        self.write_rows("case.memory.jsonl", memory)
        with self.assertRaisesRegex(ValueError, "memory/input"):
            gate.read_campaign(self.root)

    def test_initial_campaign_with_memory_is_valid_but_inconclusive(self):
        m = manifest()
        m.update(kind="initial-baseline", seeds=[42], cases=[dict(label="case", exit_code=0)])
        (self.root / "manifest.json").write_text(json.dumps(m))
        self.write_rows("case.jsonl", rows())
        self.write_rows("case.memory.jsonl", rows(0, 1))
        parsed = gate.read_campaign(self.root)
        self.assertEqual(gate.compare(parsed, parsed)["status"], "inconclusive")


if __name__ == "__main__":
    unittest.main()

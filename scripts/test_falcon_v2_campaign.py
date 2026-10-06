#!/usr/bin/env python3
"""Independent fail-closed checks; no benchmark, subprocess, or Rust build runs.

While staged, run with PYTHONPATH=<worktree>/scripts python3 this_file.py.
"""
import copy
from collections import Counter
import math
import unittest
import signal
import tempfile
from pathlib import Path
from unittest import mock

import falcon_v2_campaign as runner


SCHEMA = "bitz/falcon-hybrid/v3"
PAYLOAD = "canonical stored payload; excludes Falcon framing and public statement"
PROFILES = {
    "native": ("bitz/falcon1024-ct/hybrid/native-ring/non-zk/v4", "wfbitz-joint-limbs"),
    "v1": ("bitz/falcon1024-ct/hybrid/shared-prime/non-zk/v1", "wfbitz-joint-limbs"),
    "direct": ("bitz/falcon1024-ct/hybrid/shared-prime/non-zk/v2", "wfbitz-joint-limbs"),
    "v2": ("bitz/falcon1024-ct/hybrid/shared-prime/non-zk/v2", "wfbitz-unsplit"),
    "v3": ("bitz/falcon1024-ct/hybrid/shared-prime/non-zk/v3", "wfbitz-joint-limbs"),
}
COMMON_TERMS = (
    "prime sampling", "per-signature norm identity", "norm sumchecks",
    "HashToPoint initial row point", "HashToPoint product sumcheck",
    "ordered compaction fingerprints", "ordered compaction forest sumchecks",
    "ordered compaction forest claim reductions", "compaction leaf instance batching",
    "compaction leaf sumcheck", "linear constraints and terminal batching",
    "prime source sumcheck", "batched integer-to-binary forest", "binary Keccak PIOP",
    "SHAKE wiring and binary claim batching", "joint binary sumcheck",
    "ring switch and support padding", "shared Ligerito",
)


def rows(label="v2", seed=42, security=128):
    protocol, bridge = PROFILES[label]
    if label == "v3" and security == 100:
        bridge = "wfbitz-unsplit"
    ring = {
        "native": ("native ideal batching and projection", "native coordinate carry batching"),
        "v1": ("shared ring outer, endpoint batch and inner", "integer polynomial projection"),
        "direct": ("shared ring outer and endpoint batch", "integer polynomial projection"),
        "v2": ("shared ring outer and endpoint batch", "integer polynomial projection"),
        "v3": ("shared ring outer and endpoint batch", "integer polynomial projection"),
    }[label]
    terms = [dict(name=name, error_bound=2.0**-145, bits=145.0)
             for name in COMMON_TERMS + ring]
    h = dict(schema=SCHEMA, event="prepared", batch=1, capacity=1,
             security_target=security, threads=1, input_seed=seed, input_digest="a" * 64,
             protocol=protocol, integer_bridge=bridge, target_arch="x86_64",
             gf128_kernel="test", compiled_target_features={"avx512f": True},
             input_source="pornin/rust-fn-dsa", input_implementation_version="0.3.0",
             input_mode="original-falcon-1024", input_rng="fixed", input_count=1,
             public_inputs=["public_key", "message", "signature_nonce", "signature_s2"],
             source_bits_per_signature=[131072, 1048576, 524288],
             native_verify_includes_public_key_decode=True, stage_timings=False,
             warmup_trials=1, measured_trials=3,
             build_rustflags="-C target-cpu=native", runtime_rustflags="-C target-cpu=native",
             security_terms=terms,
             algebraic_security_bits=-math.log2(sum(t["error_bound"] for t in terms)))
    if label == "v3":
        small = security == 100
        h.update(arithmetic_prime_bits=115 if small else 126,
                 arithmetic_prime_min=str(1 << (114 if small else 125)),
                 arithmetic_prime_max=str((1 << 115) - (1 << 102) - 1 if small else (1 << 126) - 1))
    result = [h]
    for index in range(4):
        result.append(dict(schema=SCHEMA, event="trial", batch=1, capacity=1,
                           security_target=security, threads=1, input_digest=h["input_digest"],
                           trial="warmup" if index == 0 else "sample",
                           sample=None if index == 0 else index,
                           total_prover_ms=10.0, witness_commit_ms=4.0, proof_prove_ms=6.0,
                           proof_verify_ms=2.0, proof_payload_bytes=1000,
                           proof_payload_definition=PAYLOAD, proof_debug_digest="b" * 64,
                           verified=True))
    result.append(dict(schema=SCHEMA, event="summary", batch=1, capacity=1,
                       security_target=security, threads=1, samples=3, verified=True,
                       median_total_prover_ms=10.0, median_witness_commit_ms=4.0,
                       median_proof_prove_ms=6.0, median_proof_verify_ms=2.0))
    return result


def validate(data, label="v2", seed=42, security=128):
    return runner.validate(data, label, (security, 1, 1, seed), 1, 3)


def records(ratios=(0.9, 0.9, 0.9), seeds=(47, 48, 49), labels=("native", "v1", "v2")):
    result = {}
    for label in labels:
        result[label] = {}
        for seed, ratio in zip(seeds, ratios):
            item = validate(rows(label, seed), label, seed)
            item.update(rss_bytes=5000, status="complete")
            if label in ("v2", "v3"):
                item["medians"] = {k: v * ratio for k, v in item["medians"].items()}
                # Debug representations may differ across protocols/revisions.
                item["proof_digest"] = "c" * 64
            result[label][seed] = item
    return result


class ValidationTests(unittest.TestCase):
    def test_all_profiles_are_accepted_with_exact_bridge(self):
        for label in PROFILES:
            with self.subTest(label=label):
                item = validate(rows(label), label)
                self.assertEqual(item["verified_proofs"], 4)
                self.assertEqual(item["payload_bytes"], 1000)

    def test_v3_selects_exact_prime_and_bridge_at_each_security_target(self):
        for security in (100, 128):
            with self.subTest(security=security):
                item = validate(rows("v3", security=security), "v3", security=security)
                self.assertEqual(item["verified_proofs"], 4)
                for key in ("protocol", "integer_bridge", "arithmetic_prime_bits",
                            "arithmetic_prime_min", "arithmetic_prime_max"):
                    wrong = rows("v3", security=security)
                    opposite = rows("v3", security=228-security)[0]
                    wrong[0][key] = (PROFILES["v2"][0] if key == "protocol" else opposite[key])
                    with self.subTest(key=key), self.assertRaises(ValueError):
                        validate(wrong, "v3", security=security)

    def test_v3_requires_exact_string_bounds_and_all_prime_metadata(self):
        for key in ("arithmetic_prime_bits", "arithmetic_prime_min", "arithmetic_prime_max"):
            data = rows("v3")
            data[0].pop(key)
            with self.subTest(missing=key), self.assertRaises(ValueError):
                validate(data, "v3")
        for key in ("arithmetic_prime_min", "arithmetic_prime_max"):
            for numeric in (False, True):
                data = rows("v3")
                value = int(data[0][key])
                data[0][key] = value if numeric else str(value + 1)
                with self.subTest(key=key, numeric=numeric), self.assertRaises(ValueError):
                    validate(data, "v3")

    def test_historical_profiles_remain_valid_without_prime_metadata(self):
        for label in ("native", "v1", "v2", "direct"):
            for security in (100, 128):
                data = rows(label, security=security)
                self.assertNotIn("arithmetic_prime_min", data[0])
                with self.subTest(label=label, security=security):
                    validate(data, label, security=security)

    def test_wrong_protocol_and_bridge_are_rejected_separately(self):
        for key in ("protocol", "integer_bridge"):
            data = rows()
            data[0][key] = PROFILES["v1"][0 if key == "protocol" else 1]
            with self.subTest(key=key), self.assertRaises(ValueError):
                validate(data)

    def test_incomplete_unverified_or_instrumented_process_is_rejected(self):
        mutations = (
            lambda data: data.pop(),
            lambda data: data[2].update(verified=False),
            lambda data: data[-1].update(verified=False),
            lambda data: data[0].update(stage_timings=True),
            lambda data: data[3].update(sample=7),
            lambda data: data[-1].update(median_total_prover_ms=9.0),
        )
        for index, mutate in enumerate(mutations):
            data = rows()
            mutate(data)
            with self.subTest(index=index), self.assertRaises(ValueError):
                validate(data)

    def test_wrong_warmup_label_and_summary_workload_are_rejected(self):
        for row, key, value in ((1, "trial", "garbage"), (-1, "batch", 9), (-1, "samples", 99)):
            data = rows()
            data[row][key] = value
            with self.subTest(row=row, key=key), self.assertRaises(ValueError):
                validate(data)

    def test_missing_duplicate_and_wrong_security_terms_are_rejected(self):
        for kind in ("missing", "duplicate", "wrong"):
            data = rows()
            terms = data[0]["security_terms"]
            if kind == "missing":
                terms.pop()
            elif kind == "duplicate":
                terms[-1] = copy.deepcopy(terms[0])
            else:
                terms[-1]["name"] = "different relation"
            with self.subTest(kind=kind), self.assertRaises(ValueError):
                validate(data)

    def test_nonfinite_negative_weak_or_misreported_security_is_rejected(self):
        for value in (float("nan"), float("inf"), -1.0, 2.0**-100):
            data = rows()
            data[0]["security_terms"][0]["error_bound"] = value
            with self.subTest(value=value), self.assertRaises(ValueError):
                validate(data)
        data = rows()
        data[0]["algebraic_security_bits"] += 1
        with self.assertRaises(ValueError):
            validate(data)

    def test_changed_proof_or_input_digest_is_rejected(self):
        for key in ("proof_debug_digest", "input_digest"):
            data = rows()
            data[2][key] = "d" * 64
            with self.subTest(key=key), self.assertRaises(ValueError):
                validate(data)

    def test_empty_or_nonhex_digests_are_rejected_even_when_repeated(self):
        for key in ("proof_debug_digest", "input_digest"):
            for value in (None, "", "g" * 64, "f" * 63):
                data = rows()
                if key == "input_digest":
                    data[0][key] = value
                for row in data[1:-1]:
                    row[key] = value
                with self.subTest(key=key, value=value), self.assertRaises(ValueError):
                    validate(data)

    def test_missing_matched_provenance_is_rejected(self):
        for key in ("input_digest", "target_arch", "build_rustflags", "input_source"):
            data = rows()
            data[0].pop(key)
            if key == "input_digest":
                for row in data[1:-1]:
                    row.pop(key)
            with self.subTest(key=key), self.assertRaises(ValueError):
                validate(data)


class PairedComparisonTests(unittest.TestCase):
    def compare(self, data, expected=(47, 48, 49), confirmation=True):
        return runner.compare(data, "v2", expected, confirmation)

    def test_complete_confirmation_passes_against_both_baselines(self):
        result = self.compare(records())
        self.assertEqual({r["baseline"] for r in result}, {"native", "v1"})
        self.assertTrue(all(r["status"] == "pass" and r["complete"] for r in result))

    def test_v3_candidate_compares_to_both_retained_baselines(self):
        data = records(labels=("native", "v1", "v3"))
        confirmed = runner.compare(data, "v3", (47, 48, 49), True)
        self.assertEqual({r["baseline"] for r in confirmed}, {"native", "v1"})
        self.assertTrue(all(r["status"] == "pass" for r in confirmed))
        discovery = runner.compare(data, "v3", (47, 48, 49), False)
        self.assertTrue(all(r["status"] == "inconclusive" for r in discovery))

    def test_discovery_and_incomplete_seed_sets_cannot_pass(self):
        self.assertTrue(all(r["status"] == "inconclusive"
                            for r in self.compare(records(), confirmation=False)))
        result = self.compare(records(), expected=(47, 48, 49, 50))
        self.assertTrue(all(r["status"] == "inconclusive" and not r["complete"] for r in result))

    def test_unpaired_seed_and_changed_input_are_rejected(self):
        data = records()
        data["native"].pop(48)
        with self.assertRaises(ValueError):
            self.compare(data)
        for key in ("input_seed", "input_digest", "security_target"):
            data = records()
            data["v2"][48]["prepared"][key] = "changed"
            with self.subTest(key=key), self.assertRaises(ValueError):
                self.compare(data)

    def test_even_small_slowdown_is_regression(self):
        self.assertTrue(all(r["status"] == "regression"
                            for r in self.compare(records((1.000001,) * 3))))

    def test_uncertain_seed_ratios_are_inconclusive(self):
        self.assertTrue(all(r["status"] == "inconclusive"
                            for r in self.compare(records((0.5, 1.0, 2.0)))))

    def test_one_extra_payload_or_rss_byte_is_regression(self):
        for key in ("payload_bytes", "rss_bytes"):
            data = records()
            data["v2"][48][key] += 1
            with self.subTest(key=key):
                self.assertTrue(all(r["status"] == "regression" for r in self.compare(data)))

    def test_single_seed_is_inconclusive(self):
        result = self.compare(records((0.5,), (47,)), expected=(47,))
        self.assertTrue(all(r["status"] == "inconclusive" for r in result))

    def test_seed_key_must_match_its_prepared_workload(self):
        data = records()
        for label in data:
            data[label][48]["prepared"]["input_seed"] = 47
        with self.assertRaises(ValueError):
            self.compare(data)




class ProcessOrderTests(unittest.TestCase):
    def test_six_seeds_balance_positions_and_pair_directions(self):
        labels = ["native", "v1", "v3"]
        orders = [runner.process_order(labels, i) for i in range(6)]
        self.assertEqual(len({tuple(order) for order in orders}), 6)
        for label in labels:
            self.assertEqual(Counter(order.index(label) for order in orders), {0: 2, 1: 2, 2: 2})
        for a, b in (("native", "v1"), ("native", "v3"), ("v1", "v3")):
            self.assertEqual(sum(order.index(a) < order.index(b) for order in orders), 3)
        self.assertEqual(runner.process_order(labels, 6), orders[0])
        self.assertEqual(labels, ["native", "v1", "v3"])

    def test_five_seeds_are_nearly_balanced_in_each_position(self):
        labels = ["native", "v1", "v3"]
        orders = [runner.process_order(labels, i) for i in range(5)]
        for label in labels:
            counts = [sum(order[index] == label for order in orders) for index in range(3)]
            self.assertEqual(sorted(counts), [1, 2, 2])
        with self.assertRaises(ValueError):
            runner.process_order(["v3", "v3"], 0)


class ProcessCleanupTests(unittest.TestCase):
    def test_gate_termination_is_catchable(self):
        with self.assertRaises(SystemExit) as caught:
            runner.terminate(signal.SIGTERM, None)
        self.assertEqual(caught.exception.code, 128 + signal.SIGTERM)

    def test_interrupted_run_reaps_child_group_and_records_failure(self):
        with tempfile.TemporaryDirectory() as temporary:
            out = Path(temporary)
            binary = out / "binary"
            binary.write_bytes(b"test binary")
            item = dict(binary=str(binary), sha256=runner.digest(binary))
            process = mock.Mock()
            process.wait.side_effect = SystemExit(143)
            with mock.patch.object(runner.subprocess, "Popen", return_value=process), \
                    mock.patch.object(runner, "stop_process_group") as cleanup:
                with self.assertRaises(SystemExit):
                    runner.run_one(out, item, "v2", (100, 1, 1, 42), 1, 3, 30)
                cleanup.assert_called_once_with(process)
            record = runner.json.loads(next(out.glob("*.run.json")).read_text())
            self.assertEqual(record["status"], "interrupted")
            self.assertEqual(record["cleanup_status"], "succeeded")
            self.assertEqual(record["interruption_error"], "SystemExit: 143")
            self.assertNotIn("cleanup_error", record)

    def test_cleanup_permission_error_preserves_interruption_and_propagates(self):
        for interruption in (SystemExit(143), runner.subprocess.TimeoutExpired("binary", 30)):
            with self.subTest(interruption=type(interruption).__name__), tempfile.TemporaryDirectory() as temporary:
                out = Path(temporary)
                binary = out / "binary"
                binary.write_bytes(b"test binary")
                item = dict(binary=str(binary), sha256=runner.digest(binary))
                process = mock.Mock()
                process.wait.side_effect = interruption
                cleanup_error = PermissionError(1, "Operation not permitted")

                def fail_cleanup(child):
                    self.assertIs(child, process)
                    record = runner.json.loads(next(out.glob("*.run.json")).read_text())
                    self.assertEqual(record["status"], "interrupted")
                    self.assertEqual(record["cleanup_status"], "pending")
                    self.assertEqual(record["interruption_error"],
                                     f"{type(interruption).__name__}: {interruption}")
                    raise cleanup_error

                with mock.patch.object(runner.subprocess, "Popen", return_value=process), \
                        mock.patch.object(runner, "stop_process_group", side_effect=fail_cleanup) as cleanup:
                    with self.assertRaises(PermissionError) as caught:
                        runner.run_one(out, item, "v2", (100, 1, 1, 42), 1, 3, 30)
                    cleanup.assert_called_once_with(process)
                self.assertIs(caught.exception, cleanup_error)
                self.assertIs(caught.exception.__cause__, interruption)
                record = runner.json.loads(next(out.glob("*.run.json")).read_text())
                self.assertEqual(record["status"], "interrupted")
                self.assertEqual(record["cleanup_status"], "failed")
                self.assertEqual(record["cleanup_error"],
                                 "PermissionError: [Errno 1] Operation not permitted")
                self.assertNotIn("verified_proofs", record)

if __name__ == "__main__":
    unittest.main()

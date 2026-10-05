#!/usr/bin/env python3
"""Paired campaign lifecycle and ordering tests; no build or proof is run."""
from collections import Counter, defaultdict
from contextlib import ExitStack, redirect_stdout
import io
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

import run_falcon_paired as runner
from compare_falcon_benchmarks import read_campaign
from test_compare_falcon_benchmarks import rows

REVISION = "1" * 40


class PairedTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.root = Path(self.temp.name).resolve()
        (self.root / "Cargo.lock").write_text("fixture lock\n")
        self.baseline = self.root / "retained-benchmark"
        self.baseline.write_text("baseline\n")
        self.baseline.chmod(0o700)
        self.binary = self.root / "candidate-benchmark"
        self.binary.write_text("candidate\n")
        self.original = self.root / "old" / "manifest.json"
        self.original.parent.mkdir()
        self.original.write_text("{}\n")
        self.args = runner.parse_args(["--repo", str(self.root), "--revision", REVISION,
                                      "--baseline-manifest", str(self.original), "--output", ".tmp/paired",
                                      "--security", "100", "--threads", "1", "--batches", "1",
                                      "--seeds", "42", "43", "--pairs-per-seed", "2"])
        self.commands = []
        self.stack = ExitStack()
        self.stack.enter_context(redirect_stdout(io.StringIO()))
        self.revision = self.stack.enter_context(patch.object(runner, "verify_revision"))
        self.stack.enter_context(patch.object(runner, "require_ignored"))
        self.stack.enter_context(patch.object(runner, "compiler_version", return_value="rustc fixture\n"))
        self.acquire = self.stack.enter_context(patch.object(runner.bench_gate, "acquire"))
        self.release = self.stack.enter_context(patch.object(runner.bench_gate, "release"))
        self.idle = self.stack.enter_context(patch.object(runner.bench_gate, "wait_idle"))
        self.retained = self.stack.enter_context(patch.object(runner, "retained_baseline", side_effect=self.retained_fixture))
        self.execute = self.stack.enter_context(patch.object(runner, "execute", side_effect=self.simulate))

    def tearDown(self):
        self.stack.close()
        self.temp.cleanup()

    def retained_fixture(self, path, current, binary_override=None):
        return dict(current, revision=runner.BASELINE_REVISION, binary=str(self.baseline),
                    binary_sha256=runner.support.file_hash(self.baseline)), self.baseline

    def simulate(self, command, *, root, env, output, errors=None, timeout):
        self.commands.append(command)
        if command[0] == "cargo":
            output.write_text(json.dumps({"reason": "compiler-artifact", "target": {"name": "falcon_hybrid"}, "executable": str(self.binary)}) + "\n")
            return 1.0
        flags = dict(zip(command[1::2], command[2::2]))
        batch, seed = int(flags["--batch"]), int(flags["--seed"])
        data = rows(int(flags["--warmup"]), int(flags["--iterations"]))
        for row in data:
            row.update(batch=batch, capacity=1 << (batch - 1).bit_length(),
                       security_target=int(flags["--security"]), threads=int(flags["--threads"]),
                       input_digest=f"input-{seed}")
        data[0].update(input_count=batch, input_seed=seed,
                       protocol=runner.PROTOCOLS[flags.get("--protocol", "native")])
        output.write_text("".join(json.dumps(row) + "\n" for row in data))
        if errors:
            errors.write_text("")
        return 1.0

    def manifest(self, role="candidate"):
        return json.loads((self.root / self.args.output / role / "manifest.json").read_text())

    def test_diagnostic_is_balanced_readable_and_inconclusive(self):
        output, report = runner.run(self.args)
        before, blocks = read_campaign(output / "baseline")
        after, _ = read_campaign(output / "candidate")
        self.assertEqual(len(blocks), 4)
        self.assertEqual(before["pairing_id"], after["pairing_id"])
        self.assertEqual(report["status"], "inconclusive")
        self.assertFalse(report["matrix_complete"])
        roles = ["A" if command[0] == str(self.baseline) else "B" for command in self.commands[1:]]
        # Each role's timing child is immediately followed by its fresh RSS child.
        self.assertEqual(roles, list("AABBBBAAAABBBBAA"))
        self.assertEqual(len(self.commands), 17)
        for command in self.commands[1:]:
            if command[0] == str(self.baseline):
                self.assertNotIn("--protocol", command)
        self.assertEqual(sum(len(block["samples"]) + block["prepared"]["warmup_trials"] + 1 for block in blocks.values()) * 2, 48)
        self.assertTrue((output / "execution.jsonl").exists())
        self.assertTrue((output / "comparison.json").exists())
        self.acquire.assert_called_once()
        self.idle.assert_called_once()
        self.release.assert_called_once()

    def test_full_schedule_has_thirty_pairs_per_cell_with_balanced_abba(self):
        args = runner.parse_args(["--revision", REVISION, "--baseline-manifest", "old/manifest.json", "--output", ".tmp/full"])
        cases = list(runner.schedule(args))
        counts, orders = Counter(), defaultdict(Counter)
        for case in cases:
            cell = (case["security"], case["batch"], case["threads"])
            counts[cell] += 1
            orders[cell + (case["seed"],)][case["order"]] += 1
        self.assertEqual(len(cases), 480)
        self.assertEqual(set(counts.values()), {30})
        self.assertTrue(all(value == Counter(AB=3, BA=3) for value in orders.values()))
        for left, right in zip(cases[::2], cases[1::2]):
            self.assertEqual(left["order"] + right["order"], "ABBA")
            self.assertEqual((left["seed"], left["batch"], left["threads"], left["security"]),
                             (right["seed"], right["batch"], right["threads"], right["security"]))

    def test_full_campaign_satisfies_unchanged_pairing_gate(self):
        self.args.security = runner.SECURITY
        self.args.threads = runner.THREADS
        self.args.batches = runner.BATCHES
        self.args.seeds = runner.SEEDS
        self.args.pairs_per_seed = 6
        _, report = runner.run(self.args)
        self.assertEqual(report["status"], "pass")
        self.assertTrue(report["matrix_complete"])
        self.assertEqual(len(report["cells"]), 16)
        self.assertTrue(all(cell["acceptance_pairing_complete"] for cell in report["cells"]))
        self.assertEqual(len(self.commands), 1921)

    def test_existing_output_and_baseline_target_are_never_overwritten(self):
        output = self.root / self.args.output
        output.mkdir(parents=True)
        sentinel = output / "preserve"
        sentinel.write_text("unchanged")
        with self.assertRaisesRegex(ValueError, "already exists"):
            runner.run(self.args)
        self.assertEqual(sentinel.read_text(), "unchanged")
        self.args.output = Path(".tmp/other")
        self.args.target_dir = self.root
        with self.assertRaisesRegex(ValueError, "retained baseline binary"):
            runner.run(self.args)
        self.acquire.assert_not_called()

    def test_failure_preserves_both_incomplete_manifests(self):
        def fail(command, **kwargs):
            if command[0] == str(self.binary):
                raise TimeoutError("candidate timed out")
            return self.simulate(command, **kwargs)
        self.execute.side_effect = fail
        with self.assertRaisesRegex(TimeoutError, "timed out"):
            runner.run(self.args)
        self.assertEqual(self.manifest()["status"], "failed")
        self.assertEqual(self.manifest("baseline")["status"], "failed")
        self.assertEqual(self.manifest()["active_case"]["role"], "B")
        self.assertEqual(len(self.manifest("baseline")["cases"]), 1)
        self.assertEqual(self.manifest()["cases"], [])
        self.release.assert_called_once()

    def test_interruption_before_lock_never_releases_someone_elses_lock(self):
        self.acquire.side_effect = KeyboardInterrupt
        with self.assertRaises(KeyboardInterrupt):
            runner.run(self.args)
        self.assertEqual(self.manifest()["status"], "interrupted")
        self.release.assert_not_called()

    def test_binary_changed_after_build_is_rejected_before_timing(self):
        def overwrite(command, **kwargs):
            result = self.simulate(command, **kwargs)
            if command[0] == "cargo":
                self.baseline.write_text("overwritten")
            return result
        self.execute.side_effect = overwrite
        with self.assertRaisesRegex(ValueError, "binary changed"):
            runner.run(self.args)
        self.assertEqual(len(self.commands), 1)
        self.idle.assert_not_called()
        self.release.assert_called_once()

    def test_wrong_seed_and_unverified_proofs_fail(self):
        for field, value, message in (("input_seed", 99, "workload"), ("verified", False, "unverified")):
            with self.subTest(field=field):
                self.args.output = Path(".tmp/" + field)
                def wrong(command, **kwargs):
                    result = self.simulate(command, **kwargs)
                    if command[0] != "cargo":
                        path = kwargs["output"]
                        data = [json.loads(line) for line in path.read_text().splitlines()]
                        data[0 if field == "input_seed" else 1][field] = value
                        path.write_text("".join(json.dumps(row) + "\n" for row in data))
                    return result
                self.execute.side_effect = wrong
                with self.assertRaisesRegex(ValueError, message):
                    runner.run(self.args)
                self.assertEqual(self.manifest()["status"], "failed")



class RetainedTests(unittest.TestCase):
    def test_revision_host_hash_and_executable_are_verified(self):
        with tempfile.TemporaryDirectory() as temporary:
            binary = Path(temporary) / "benchmark"
            binary.write_text("retained")
            binary.chmod(0o700)
            current = {field: "fixture" for field in runner.PROVENANCE}
            manifest = dict(current, revision=runner.BASELINE_REVISION, binary=str(binary), binary_sha256=runner.support.file_hash(binary))
            with patch.object(runner, "read_campaign", return_value=(manifest, {})):
                self.assertEqual(runner.retained_baseline(Path(temporary) / "manifest.json", current)[1], binary.resolve())
                for field, value, message in (("revision", REVISION, "revision"), ("host", "different", "provenance"), ("binary_sha256", "0" * 64, "hash changed")):
                    with patch.dict(manifest, {field: value}):
                        with self.assertRaisesRegex(ValueError, message):
                            runner.retained_baseline(Path(temporary) / "manifest.json", current)
                copied = Path(temporary) / "copied-benchmark"
                copied.write_bytes(binary.read_bytes())
                copied.chmod(0o700)
                self.assertEqual(runner.retained_baseline(Path(temporary) / "manifest.json", current, copied)[1], copied.resolve())
                binary.chmod(0o600)
                with self.assertRaisesRegex(ValueError, "not executable"):
                    runner.retained_baseline(Path(temporary) / "manifest.json", current)

    def test_invalid_schedule_arguments_are_rejected(self):
        base = ["--revision", REVISION, "--baseline-manifest", "old/manifest.json", "--output", "out"]
        for extra in (["--pairs-per-seed", "3"], ["--seeds", "42", "42"], ["--min-idle", "nan"]):
            with self.subTest(extra=extra), self.assertRaises(SystemExit), patch("sys.stderr", io.StringIO()):
                runner.parse_args(base + extra)


if __name__ == "__main__":
    unittest.main()

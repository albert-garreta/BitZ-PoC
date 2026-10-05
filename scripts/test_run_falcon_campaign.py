#!/usr/bin/env python3
"""The campaign lifecycle is exercised without Cargo or a remote workload."""
from contextlib import ExitStack, redirect_stdout
import io
import json
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch

import run_falcon_campaign as runner
from compare_falcon_benchmarks import read_campaign
from test_compare_falcon_benchmarks import rows

REVISION = "1" * 40


class CampaignTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.root = Path(self.temp.name)
        (self.root / "Cargo.lock").write_text("lockfile fixture\n")
        self.binary = self.root / "benchmark"
        self.binary.write_text("binary fixture\n")
        self.args = runner.parse_args(["--repo", str(self.root), "--revision", REVISION,
                                       "--protocol", "shared-prime", "--output", ".tmp/campaign"])
        self.commands = []
        self.stack = ExitStack()
        self.stack.enter_context(redirect_stdout(io.StringIO()))
        self.revision = self.stack.enter_context(patch.object(runner, "verify_revision"))
        self.stack.enter_context(patch.object(runner, "require_ignored"))
        self.stack.enter_context(patch.object(runner, "compiler_version", return_value="rustc fixture\n"))
        self.acquire = self.stack.enter_context(patch.object(runner.bench_gate, "acquire"))
        self.release = self.stack.enter_context(patch.object(runner.bench_gate, "release"))
        self.idle = self.stack.enter_context(patch.object(runner.bench_gate, "wait_idle"))
        self.execute = self.stack.enter_context(patch.object(runner, "execute", side_effect=self.simulate))

    def tearDown(self):
        self.stack.close()
        self.temp.cleanup()

    def simulate(self, command, *, root, env, output, errors=None, timeout):
        self.commands.append(command)
        if command[0] == "cargo":
            output.write_text(json.dumps({"reason": "compiler-artifact", "target": {"name": "falcon_hybrid"}, "executable": str(self.binary)}) + "\n")
            return 1.0
        flags = dict(zip(command[1::2], command[2::2]))
        batch = int(flags["--batch"])
        warmup, measured = int(flags["--warmup"]), int(flags["--iterations"])
        data = rows(warmup, measured)
        for row in data:
            row.update(batch=batch, capacity=1 << (batch - 1).bit_length(),
                       security_target=int(flags["--security"]), threads=int(flags["--threads"]))
        data[0].update(input_count=batch, protocol=runner.PROTOCOLS[flags.get("--protocol", "native")])
        output.write_text("".join(json.dumps(row) + "\n" for row in data))
        if errors:
            errors.write_text("")
        return 1.0

    def manifest(self):
        return json.loads((self.root / self.args.output / "manifest.json").read_text())

    def test_complete_matrix_has_112_verified_proofs_and_matches_reader(self):
        output = runner.run_campaign(self.args)
        manifest, blocks = read_campaign(output)
        self.assertEqual(manifest["status"], "complete")
        self.assertEqual(manifest["rustc"], "rustc fixture\n")
        self.assertEqual(len(blocks), 16)
        self.assertEqual(sum(len(block["samples"]) + block["prepared"]["warmup_trials"] + 1 for block in blocks.values()), 112)
        self.assertEqual(len(self.commands), 33)
        self.assertEqual(self.commands[0][0:3], ["cargo", "build", "--locked"])
        self.assertTrue(all(command[-2:] == ["--protocol", "shared-prime"] for command in self.commands[1:]))
        self.acquire.assert_called_once()
        self.idle.assert_called_once()
        self.release.assert_called_once()
        self.assertEqual(self.revision.call_count, 20)

    def test_existing_output_is_not_overwritten(self):
        output = self.root / self.args.output
        output.mkdir(parents=True)
        sentinel = output / "preserve"
        sentinel.write_text("unchanged")
        with self.assertRaisesRegex(ValueError, "already exists"):
            runner.run_campaign(self.args)
        self.assertEqual(sentinel.read_text(), "unchanged")
        self.acquire.assert_not_called()
        self.release.assert_not_called()

    def test_build_failure_is_retained_and_releases_owned_lock(self):
        self.execute.side_effect = RuntimeError("compiler failed")
        with self.assertRaisesRegex(RuntimeError, "compiler failed"):
            runner.run_campaign(self.args)
        self.assertEqual(self.manifest()["status"], "failed")
        self.assertEqual(self.manifest()["cases"], [])
        self.release.assert_called_once()

    def test_unacquired_lock_is_never_released(self):
        self.acquire.side_effect = KeyboardInterrupt
        with self.assertRaises(KeyboardInterrupt):
            runner.run_campaign(self.args)
        self.assertEqual(self.manifest()["status"], "interrupted")
        self.release.assert_not_called()

    def test_interrupted_child_keeps_active_case(self):
        def interrupted(command, **kwargs):
            if command[0] != "cargo":
                raise KeyboardInterrupt
            return self.simulate(command, **kwargs)
        self.execute.side_effect = interrupted
        with self.assertRaises(KeyboardInterrupt):
            runner.run_campaign(self.args)
        self.assertEqual(self.manifest()["status"], "interrupted")
        self.assertEqual(self.manifest()["active_case"], "s100-b1-t1-seed42")
        self.release.assert_called_once()

    def test_verification_failure_is_not_a_completed_case(self):
        def unverified(command, **kwargs):
            elapsed = self.simulate(command, **kwargs)
            if command[0] != "cargo":
                path = kwargs["output"]
                data = [json.loads(line) for line in path.read_text().splitlines()]
                data[1]["verified"] = False
                path.write_text("".join(json.dumps(row) + "\n" for row in data))
            return elapsed
        self.execute.side_effect = unverified
        with self.assertRaisesRegex(ValueError, "unverified"):
            runner.run_campaign(self.args)
        self.assertEqual(self.manifest()["status"], "failed")
        self.assertEqual(self.manifest()["cases"], [])
        self.release.assert_called_once()

    def test_revision_change_after_build_is_rejected(self):
        self.revision.side_effect = [None, None, ValueError("HEAD changed")]
        with self.assertRaisesRegex(ValueError, "HEAD changed"):
            runner.run_campaign(self.args)
        self.assertEqual(len(self.commands), 1)
        self.assertEqual(self.manifest()["status"], "failed")
        self.idle.assert_not_called()
        self.release.assert_called_once()


class HelperTests(unittest.TestCase):
    def test_compiler_provenance_preserves_exact_output(self):
        with patch.object(runner.subprocess, "check_output", return_value="rustc fixture\n"):
            self.assertEqual(runner.compiler_version(Path("/repo")), "rustc fixture\n")

    def test_revision_and_dirt_are_checked(self):
        good = dict(revision=REVISION, git_dirty=False)
        with patch.object(runner.support, "root_metadata", return_value=good):
            runner.verify_revision(Path("/repo"), REVISION)
            with self.assertRaisesRegex(ValueError, "full commit"):
                runner.verify_revision(Path("/repo"), "HEAD")
            with self.assertRaisesRegex(ValueError, "HEAD differs"):
                runner.verify_revision(Path("/repo"), "2" * 40)
        with patch.object(runner.support, "root_metadata", return_value=dict(good, git_dirty=True)):
            with self.assertRaisesRegex(ValueError, "dirty"):
                runner.verify_revision(Path("/repo"), REVISION)

    def test_native_works_without_protocol_flag_and_memory_is_fresh(self):
        command = runner.benchmark_command("benchmark", "native", 128, 1024, 16, memory=True)
        self.assertNotIn("--protocol", command)
        self.assertEqual(command[-4:], ["--warmup", "0", "--iterations", "1"])

    def test_environment_cannot_override_selected_flags_or_inputs(self):
        with patch.dict(runner.os.environ, {"RUSTFLAGS": "wrong", "CARGO_ENCODED_RUSTFLAGS": "wrong",
                                          "BITZ_FALCON_STAGE_TIMINGS": "1", "BITZ_FALCON_SEED": "9",
                                          "RAYON_NUM_THREADS": "99", "PATH": "/bin"}, clear=True):
            env = runner.campaign_environment(Path("/target"))
        self.assertEqual(env["RUSTFLAGS"], runner.RUSTFLAGS)
        self.assertEqual(env["CARGO_TARGET_DIR"], "/target")
        self.assertEqual(env["PATH"], "/bin")
        self.assertNotIn("CARGO_ENCODED_RUSTFLAGS", env)
        self.assertNotIn("BITZ_FALCON_STAGE_TIMINGS", env)
        self.assertNotIn("RAYON_NUM_THREADS", env)

    def test_nonignored_output_is_rejected(self):
        with patch.object(runner.support, "git", side_effect=subprocess.CalledProcessError(1, "git")):
            with self.assertRaisesRegex(ValueError, "Git-ignored"):
                runner.require_ignored(Path("/repo"), Path("/repo/results"))

    def test_child_timeout_or_failure_preserves_logs(self):
        with tempfile.TemporaryDirectory() as temporary:
            log = Path(temporary) / "process.log"
            for result, error in [((None, True), TimeoutError), ((2, False), RuntimeError)]:
                with patch.object(runner.support, "run_process", return_value=result), redirect_stdout(io.StringIO()):
                    with self.assertRaises(error):
                        runner.execute(["fixture"], root=Path(temporary), env={}, output=log, timeout=1)
                self.assertTrue(log.exists())


if __name__ == "__main__":
    unittest.main()

"""Regression tests for the authors' Limber command and campaign routing."""
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

import run_native_mul_compare as runner

OUTPUT = """int_mult: 2^15−1 = 32767 chained gates of 32-bit mult  (cons=2^15, vars=2^16)
params:   log_t_f=32 k=9 -> log_p=113 s=5 numlimb=1
setup:              100.0 ms
witness gen:          10.0 ms
commit+prove:        200.0 ms
verify:               20.0 ms
proof size:            2.3 KB  (eval_arg 1000 B + sumcheck 1328 B)
"""


class LimberRunnerTests(unittest.TestCase):
    def test_reports_actual_chain_length_and_exact_component_size(self):
        result = runner.parse_limber(OUTPUT, 15)
        self.assertEqual(result["gates"], 32767)
        self.assertEqual(result["metrics"]["proof_bytes"], 2328)
        self.assertEqual(result["metrics"]["commit_prove_ms"], 200)

    def test_rejects_unverified_incomplete_or_wrong_shape_output(self):
        for output in (OUTPUT.replace("verify:", "unfinished:"),
                       OUTPUT.replace("32767", "32768"),
                       OUTPUT.replace("32-bit mult", "64-bit mult"),
                       OUTPUT.replace("1328 B", "1327 B"),
                       OUTPUT.replace("200.0 ms", "nan ms"),
                       OUTPUT + "verify: 20.0 ms\n"):
            with self.subTest(output=output):
                with self.assertRaises(ValueError):
                    runner.parse_limber(output, 15)

    def test_mixed_workloads_never_route_u32_to_the_adapter(self):
        config = dict(external=True, workloads=["u32", "babybear"],
                      backends=["f2z", "limber"])
        planned = runner.jobs(config)
        self.assertEqual(planned[0]["backends"], ["f2z"])
        self.assertEqual(planned[1]["workloads"], ["babybear"])
        self.assertEqual(planned[1]["backends"], ["limber"])
        self.assertEqual(planned[2]["kind"], "limber-int-mult")
        only = runner.jobs(dict(external=True, workloads=["u32"], backends=["limber"]))
        self.assertEqual([job["kind"] for job in only], ["limber-int-mult"])

    def test_wide_workloads_keep_master_limits_and_native_routing(self):
        for workload, maximum in (("u64", 24), ("u128", 23)):
            with self.subTest(workload=workload):
                env = dict(F2Z_MUL_COMPARE_WORKLOADS=workload,
                           F2Z_MUL_COMPARE_BACKENDS="f2z binius64",
                           F2Z_BENCH_SHAPES=f"15 {maximum}",
                           LIMBER_REPO="/missing/limber-repo")
                config = runner.configuration(env)
                self.assertFalse(config["external"])
                self.assertEqual(config["exponents"], [15, maximum])
                self.assertEqual(runner.jobs(config), [dict(
                    kind="native", workloads=[workload],
                    backends=["f2z", "binius64"], directory=".")])
                with self.assertRaises(ValueError):
                    runner.configuration(env | {"F2Z_BENCH_SHAPES": str(maximum + 1)})
                for backend in ("limber", "plonky3-whir"):
                    with self.assertRaisesRegex(ValueError, "supports only f2z and binius64"):
                        runner.configuration(env | {"F2Z_MUL_COMPARE_BACKENDS": backend})
        env = dict(F2Z_MUL_COMPARE_WORKLOADS="u32 u64 u128",
                   F2Z_MUL_COMPARE_BACKENDS="f2z binius64", F2Z_BENCH_SHAPES="15 23")
        config = runner.configuration(env)
        self.assertEqual(runner.jobs(config)[0]["workloads"], ["u32", "u64", "u128"])
        with self.assertRaises(ValueError):
            runner.configuration(env | {"F2Z_BENCH_SHAPES": "24"})

    def test_author_flags_override_encoded_flags(self):
        env = runner.limber_environment(dict(RUSTFLAGS="other", CARGO_ENCODED_RUSTFLAGS="override",
                                            RAYON_NUM_THREADS="1", DUMP="/unwanted/file"))
        self.assertEqual(env["RUSTFLAGS"], "-C target-cpu=native")
        self.assertEqual(env["RAYON_NUM_THREADS"], "8")
        self.assertNotIn("CARGO_ENCODED_RUSTFLAGS", env)
        self.assertNotIn("DUMP", env)

    def test_limits_and_missing_checkout_fail_before_running(self):
        for extra in ({"LIMBER_REPO": "/missing/limber-repo"},
                      {"RAYON_NUM_THREADS": "1"},
                      {"F2Z_BENCH_SHAPES": "25"},
                      {"F2Z_BENCH_REPS": "0"}):
            with self.subTest(extra=extra):
                with self.assertRaises(ValueError):
                    runner.configuration(dict(F2Z_MUL_COMPARE_WORKLOADS="u32",
                                              F2Z_MUL_COMPARE_BACKENDS="limber", **extra))

    def test_shell_runs_exact_author_command_and_retains_failure(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            repo = root / "limber checkout"
            (repo / "examples").mkdir(parents=True)
            (repo / "Cargo.toml").write_text('[package]\nname="limber"\n')
            (repo / "examples/int_mult.rs").write_text("// test fixture\n")
            commands = root / "commands.jsonl"
            bindir = root / "bin"
            bindir.mkdir()
            # Each fresh cargo invocation records its complete argv, cwd and
            # effective environment. It must not call cargo bench at all.
            cargo = bindir / "cargo"
            cargo.write_text("#!" + sys.executable + "\n"
                             "import json, os, sys\n"
                             "with open(os.environ['COMMANDS'], 'a') as f:\n"
                             " f.write(json.dumps(dict(args=sys.argv[1:], cwd=os.getcwd(), "
                             "rustflags=os.environ.get('RUSTFLAGS'), threads=os.environ.get('RAYON_NUM_THREADS'), "
                             "encoded=os.environ.get('CARGO_ENCODED_RUSTFLAGS'))) + '\\n')\n"
                             "if os.environ.get('FAIL_CARGO'): sys.exit(7)\n"
                             "print(" + repr(OUTPUT) + ")\n")
            cargo.chmod(0o755)
            git = bindir / "git"
            git.write_text("#!/bin/sh\nif [ \"$1\" = rev-parse ]; then printf '%s\\n' test-revision; fi\n")
            git.chmod(0o755)
            output = root / "results"
            env = {k: v for k, v in os.environ.items()
                   if not k.startswith(("F2Z_", "CARGO_")) and k != "DUMP"}
            env.update(PATH=str(bindir) + os.pathsep + env["PATH"], COMMANDS=str(commands),
                       LIMBER_REPO=str(repo), RAYON_NUM_THREADS="8", F2Z_BENCH_REPS="2",
                       F2Z_BENCH_SHAPES="15", F2Z_MUL_COMPARE_WORKLOADS="u32",
                       F2Z_MUL_COMPARE_BACKENDS="limber", F2Z_MUL_COMPARE_OUTPUT_DIR=str(output),
                       CARGO_ENCODED_RUSTFLAGS="must-be-removed", PYTHONDONTWRITEBYTECODE="1")
            script = runner.ROOT / "scripts/run_native_mul_compare.sh"
            run = subprocess.run(["bash", str(script)], env=env, capture_output=True, text=True)
            self.assertEqual(run.returncode, 0, run.stdout + run.stderr)
            calls = [json.loads(line) for line in commands.read_text().splitlines()]
            self.assertEqual(len(calls), 3)  # one warmup, two measured trials
            for call in calls:
                self.assertEqual(call["args"], ["+1.97.1", "run", "--release", "--example",
                                                "int_mult", "--", "--bits", "32", "--log-gates", "15"])
                self.assertEqual(call["cwd"], str(repo))
                self.assertEqual(call["rustflags"], "-C target-cpu=native")
                self.assertEqual(call["threads"], "8")
                self.assertIsNone(call["encoded"])
            summary = json.loads((output / "limber-int-mult/summary.json").read_text())[0]
            self.assertEqual(summary["samples"], 2)
            self.assertEqual(summary["gates"], 32767)
            self.assertEqual(summary["metrics"]["proof_bytes"], 2328)
            self.assertEqual(json.loads((output / "campaign.json").read_text())["status"], "complete")
            failed = root / "failed"
            env.update(FAIL_CARGO="1", F2Z_MUL_COMPARE_OUTPUT_DIR=str(failed))
            run = subprocess.run(["bash", str(script)], env=env, capture_output=True, text=True)
            self.assertNotEqual(run.returncode, 0)
            self.assertFalse((failed / "limber-int-mult/summary.json").exists())
            self.assertEqual(json.loads((failed / "campaign.json").read_text())["status"], "failed")


if __name__ == "__main__":
    unittest.main()

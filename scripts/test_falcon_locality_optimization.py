"""Synthetic evidence tests; no executable, prover or compiler is launched."""

import argparse
import copy
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

import falcon_locality_optimization as runner


class EvidenceTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.binaries = {}
        for role in ["baseline", "candidate"]:
            path = self.root / role
            path.write_text(f"synthetic {role}; never executed\n")
            path.chmod(0o700)
            self.binaries[role] = path

    def args(self, **changes):
        values = dict(phase="test", configs=None, variants=["baseline", "production"],
                      degrees=[512], threads=[1], seeds=[42], modes=["cold"],
                      batch=1024, security=128, cold_repetitions=1, iterations=7, warmup=2,
                      timeout=600, baseline=self.binaries["baseline"], binary=self.binaries["candidate"],
                      case_cache=self.root / "cache", order_seed=20261013, trace=False)
        values.update(changes)
        return argparse.Namespace(**values)

    def manifest(self, **changes):
        with patch.object(runner.os, "sched_getaffinity", return_value=set(runner.CPUS)):
            return runner.build_manifest(self.args(**changes))

    def raw_trial(self, job, manifest):
        return dict(trial="sample", iteration=0, verified=True, degree=job["degree"],
                    batch=manifest["batch"], security_bits=manifest["security"],
                    algebraic_security_bits=130, seed=job["seed"], threads=job["threads"],
                    auxiliary_pool_threads=job["threads"],
                    protocol_id=f"bitz/falcon{job['degree']}-algebraic/non-zk/v9",
                    cpu_affinity=dict(observed_worker_cpus=[[cpu] for cpu in job["worker_cpus"]],
                                      observed_main_cpus=[job["main_cpu"]],
                                      observed_auxiliary_worker_cpus=[[job["main_cpu"]]] * job["threads"]),
                    input_digest="a" * 64, source_root="b" * 64, proof_debug_digest="c" * 64,
                    proof_payload_breakdown=[["proof", 10]],
                    **{metric: 10 for metric in [*runner.METRICS, *runner.EXTRA_METRICS]})

    def save_phase(self, manifest):
        path = self.root / "phases" / f"{manifest['phase']}.json"
        path.parent.mkdir(exist_ok=True)
        runner.write_json(path, manifest)
        for job in manifest["jobs"]:
            self.save_result(manifest, job)

    def save_result(self, manifest, job, event=None):
        directory = self.root / "runs" / manifest["phase"] / job["id"]
        directory.mkdir(parents=True, exist_ok=True)
        runner.write_json(directory / "command.json", runner.command_record(job, manifest))
        runner.write_json(directory / "process.json", dict(exit_code=0, timed_out=False))
        (directory / "stderr.log").write_text("")
        (directory / "stdout.jsonl").write_text(json.dumps(event or self.raw_trial(job, manifest)) + "\n")
        result = dict(schema=runner.SCHEMA, verified=True, job=job,
                      manifest_sha256=runner.file_hash(self.root / "phases" / f"{manifest['phase']}.json"),
                      trials=runner.normalized_trials(directory / "stdout.jsonl", job, manifest))
        result.update({key: runner.file_hash(directory / filename) for filename, key in runner.HASHED_FILES.items()})
        runner.write_json(directory / "result.json", result)
        return directory

    def test_plan_filters_and_cold_repetitions(self):
        configs = self.root / "configs.json"
        runner.write_json(configs, {"optimized": dict(binary="candidate", threads=[8], degrees=[512],
                                                 env={"BITZ_ARITHMETIC_BINDING_STRATEGY": "optimized"})})
        manifest = self.manifest(configs=configs, variants=["baseline", "production", "optimized"],
                                 threads=[1, 8], seeds=[42, 43, 44], cold_repetitions=7,
                                 modes=["cold", "repeated"])
        self.assertEqual(len(manifest["jobs"]), 120)
        filtered = [job for job in manifest["jobs"] if job["config"] == "optimized"]
        self.assertEqual({job["threads"] for job in filtered}, {8})
        self.assertEqual(sum(job["iterations"] for job in filtered if job["mode"] == "cold"), 21)
        self.assertEqual(sum(job["iterations"] for job in filtered if job["mode"] == "repeated"), 21)
        self.assertEqual(sum(job["warmup"] for job in filtered), 6)
        self.assertEqual(manifest["jobs"], self.manifest(configs=configs, variants=["baseline", "production", "optimized"],
                         threads=[1, 8], seeds=[42, 43, 44], cold_repetitions=7, modes=["cold", "repeated"])["jobs"])

    def test_commands_do_not_inherit_old_selectors_and_trace_is_explicit(self):
        manifest = self.manifest()
        for job in manifest["jobs"]:
            command = runner.command_record(job, manifest)
            self.assertNotIn("BITZ_ARITHMETIC_RING", command["overrides"])
            self.assertNotIn("BITZ_ARITHMETIC_COORDINATE_BUFFERS", command["overrides"])
            self.assertNotIn("BITZ_TRACE", command["overrides"])
            self.assertNotIn("--trace", command["command"])
            self.assertNotIn("BITZ_ARITHMETIC_BINDING_STRATEGY", command["overrides"])
        selected = self.manifest(variants=["legacy", "optimized"])
        for job in selected["jobs"]:
            self.assertEqual(runner.command_record(job, selected)["overrides"]["BITZ_ARITHMETIC_BINDING_STRATEGY"], job["config"])
        manifest["trace"] = True
        command = runner.command_record(manifest["jobs"][0], manifest)
        self.assertIn("--trace", command["command"])
        self.assertEqual(command["overrides"]["BITZ_TRACE"], "1")
        self.assertEqual(command["overrides"]["BITZ_FALCON_LIGHT_PROFILE"], "1")

    def test_locality_selectors_and_diagnostic_isolation(self):
        names = ["baseline", "control", "forest2", "forest4", "forest8", "forest16",
                 "compact2", "compact4", "compact8", "collapse2", "collapse4", "collapse8",
                 "norm2", "norm3", "norm4"]
        manifest = self.manifest(variants=names)
        for job in manifest["jobs"]:
            command = runner.command_record(job, manifest)
            self.assertNotIn("LIG_PROVE_TRACE", command["overrides"])
            if job["config"] not in {"baseline", "control"}:
                different = {k: v for k, v in manifest["configs"][job["config"]]["env"].items()
                             if v != manifest["configs"]["control"]["env"][k]}
                self.assertEqual(len(different), 1)
        manifest["trace"] = True
        for job in manifest["jobs"]:
            self.assertEqual(runner.command_record(job, manifest)["overrides"]["LIG_PROVE_TRACE"], "1")

    def test_invalid_selectors_and_identical_binary_are_rejected(self):
        with self.assertRaisesRegex(ValueError, "identical"):
            self.manifest(binary=self.binaries["baseline"])
        config_path = self.root / "invalid.json"
        for env in [{"BITZ_FOREST_L4": "wrong"}, {"BITZ_ARITHMETIC_BINDING_STRATEGY": "on"}, {"BITZ_TRACE": "1"},
                    {"BITZ_ARITHMETIC_BINDING_CONSTRUCTION_FUSION": True}]:
            runner.write_json(config_path, {"x": dict(binary="candidate", env=env)})
            with self.assertRaisesRegex(ValueError, "unsupported override"):
                self.manifest(configs=config_path, variants=["x"])

    def test_subprocess_environment_strips_inherited_kernel_and_trace_flags(self):
        inherited = {"PATH": "/synthetic/path", "FLOCK_COMMIT_TIMING": "1", "FLOCK_NO_PREFAULT": "1",
                     "NTT_DEEP_NOFUSE": "1", "LIG_LOOKAHEAD_DISABLE": "1", "LIGERITO_TRACE": "1",
                     "PCS_TRACE": "1", "MERKLE_PCORES_ONLY": "1", "LINCHECK_TRACE": "1",
                     "VERIFY_TRACE": "1", "PERM_TRACE": "1", "OUTER_PHASES": "1",
                     "BITZ_ARITHMETIC_RING": "factored", "RAYON_NUM_THREADS": "256"}
        manifest = self.manifest()
        for job in manifest["jobs"]:
            overrides = runner.command_record(job, manifest)["overrides"]
            with patch.dict(runner.os.environ, inherited, clear=True):
                child = runner.subprocess_env(overrides)
            self.assertEqual(child, dict(PATH="/synthetic/path", **overrides))

    def test_changed_parser_or_helper_cannot_reinterpret_frozen_results(self):
        self.save_phase(self.manifest())
        actual_hash = runner.file_hash
        for changed in [Path(runner.__file__), Path(runner.__file__).with_name("falcon_buffer_optimization.py")]:
            with patch.object(runner, "file_hash", side_effect=lambda path: "changed" if Path(path) == changed else actual_hash(path)):
                with self.assertRaisesRegex(ValueError, "parser provenance changed"):
                    runner.validate_all(self.root)

    def test_custom_topology_is_frozen_and_verified_per_job(self):
        cpus = [6, 7, 4, 5, 2, 3, 0, 1, 22, 23, 20, 21]
        config = self.root / "topology.json"
        runner.write_json(config, {"smt12": dict(binary="candidate", env={
            "BITZ_ARITHMETIC_BINDING_STRATEGY": "optimized", "BITZ_PCS_INITIAL_TILES_PER_TASK": "4"},
            worker_cpus=cpus, main_cpu=7, threads=[12])})
        args = self.args(configs=config, variants=["baseline", "smt12"], threads=[12])
        with patch.object(runner.os, "sched_getaffinity", return_value=set([*runner.CPUS, *cpus])):
            manifest = runner.build_manifest(args)
        job = next(job for job in manifest["jobs"] if job["config"] == "smt12")
        self.assertEqual(job["worker_cpus"], cpus)
        self.assertEqual(job["main_cpu"], 7)
        command = runner.command_record(job, manifest)
        self.assertEqual(command["overrides"]["BITZ_FALCON_WORKER_CPUS"], ",".join(map(str, cpus)))
        self.assertEqual(command["overrides"]["BITZ_FALCON_MAIN_CPU"], "7")
        self.save_phase(manifest)
        runner.validate_all(self.root)
        for field, incorrect in [("observed_worker_cpus", [[cpu] for cpu in runner.CPUS[:12]]),
                                 ("observed_main_cpus", [6]),
                                 ("observed_auxiliary_worker_cpus", [[6]] * 12)]:
            event = self.raw_trial(job, manifest)
            event["cpu_affinity"][field] = incorrect
            path = self.root / "wrong-affinity.jsonl"
            path.write_text(json.dumps(event) + "\n")
            with self.assertRaisesRegex(ValueError, "affinity"):
                runner.normalized_trials(path, job, manifest)

    def test_invalid_or_unavailable_topology_is_rejected(self):
        path = self.root / "invalid-topology.json"
        for fields in [dict(worker_cpus=[]), dict(worker_cpus=[6, 6]), dict(worker_cpus=[True]),
                       dict(worker_cpus=[-1]), dict(worker_cpus=[99]), dict(worker_cpus=[6]),
                       dict(main_cpu=True), dict(main_cpu=-1), dict(main_cpu=99)]:
            runner.write_json(path, {"bad": dict(binary="candidate", env={}, **fields)})
            with self.assertRaises(ValueError):
                self.manifest(configs=path, variants=["bad"], threads=[12])

    def test_metadata_and_optional_metrics(self):
        manifest = self.manifest()
        job = next(job for job in manifest["jobs"] if job["binary"] == "candidate")
        sample = self.raw_trial(job, manifest)
        sample["prepared_retained_scratch_bytes"] = None
        sample["prepared_scratch_bytes_before_prover"] = None
        path = self.root / "raw.jsonl"
        path.write_text(json.dumps(sample) + "\n")
        self.assertIsNone(runner.normalized_trials(path, job, manifest)[0]["prepared_retained_scratch_bytes"])
        for changes in [dict(prover_minor_faults=None), dict(iteration=1), dict(algebraic_security_bits=100),
                        dict(protocol_id="bitz/falcon512-algebraic/non-zk/v10-factored-ring")]:
            path.write_text(json.dumps(dict(sample, **changes)) + "\n")
            with self.assertRaises(ValueError):
                runner.normalized_trials(path, job, manifest)

    def test_cross_phase_identity_and_strict_provenance(self):
        first = self.manifest()
        second = self.manifest(phase="next", variants=["optimized"], threads=[8])
        self.save_phase(first)
        self.save_phase(second)
        _, validation = runner.validate_all(self.root)
        self.assertEqual(validation["verified_proofs"], 3)
        job = second["jobs"][0]
        changed = self.raw_trial(job, second)
        changed["proof_debug_digest"] = "d" * 64
        self.save_result(second, job, changed)
        with self.assertRaisesRegex(ValueError, "complete proof/payload mismatch"):
            runner.validate_all(self.root)
        directory = self.save_result(second, job)
        command = json.loads((directory / "command.json").read_text())
        command["overrides"]["BITZ_PCS_INITIAL_TILES_PER_TASK"] = "4"
        runner.write_json(directory / "command.json", command)
        with self.assertRaisesRegex(ValueError, "command.json hash mismatch"):
            runner.validate_all(self.root)
        self.save_result(second, job)
        self.binaries["candidate"].write_text("changed frozen executable\n")
        with self.assertRaisesRegex(ValueError, "frozen binary changed"):
            runner.validate_all(self.root)

    def test_missing_baseline_and_stale_results_rejected(self):
        candidate = self.manifest(variants=["optimized"])
        self.save_phase(candidate)
        with self.assertRaisesRegex(ValueError, "missing frozen baseline"):
            runner.validate_all(self.root)
        baseline = self.manifest(phase="reference", variants=["baseline"])
        self.save_phase(baseline)
        runner.validate_all(self.root)
        stale = self.root / "runs/stale/unused/result.json"
        stale.parent.mkdir(parents=True)
        stale.write_text("{}")
        with self.assertRaisesRegex(ValueError, "unexpected stale"):
            runner.validate_all(self.root)

    def test_trace_parser_excludes_warmup_verifier_and_plaintext(self):
        def stage(name, span_id, ancestors, elapsed, **fields):
            return dict(event="stage", name=name, span_id=span_id, ancestor_ids=ancestors,
                        elapsed_ms=elapsed, fields=fields)
        events = [stage("falcon_algebraic:prove", 1, [], 100),
                  stage("op:initial_messages", 2, [1], 80),
                  stage("falcon_algebraic:prove", 3, [], 20),
                  stage("op:initial_messages", 4, [3], 5, recurrence_worker_ns=2000000),
                  stage("falcon_algebraic:verify", 5, [], 200),
                  stage("op:initial_messages", 6, [5], 99)]
        path = self.root / "trace.log"
        path.write_text("forest plaintext diagnostics\n" + "\n".join(map(json.dumps, events)))
        samples = runner.profile_samples(path, dict(warmup=1, iterations=1))
        self.assertEqual(samples, [{"falcon_algebraic:prove": 20, "op:initial_messages": 5,
                                   "op:initial_messages/recurrence_worker_ns": 2}])


if __name__ == "__main__":
    unittest.main()

"""Lightweight validation of benchmark evidence handling; no Falcon proofs run."""
import copy
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

import binder_experiments as runner


class EvidenceTests(unittest.TestCase):
    def fixture(self, variant="arithmetic"):
        job = dict(id="test", variant=variant, seed=42, threads=2,
                   config=dict(word="off"))
        manifest = dict(degree=512, batch=1024, security=128, iterations=1,
                        warmup=0, worker_cpus=[6, 7], main_cpu=6)
        affinity = dict(observed_worker_cpus=[[6], [7]], observed_main_cpus=[6],
                        observed_auxiliary_worker_cpus=[[6], [6]])
        sample = dict(trial="sample", verified=True, degree=512, batch=1024,
                      threads=2, security_bits=128, seed=42, protocol_id="bit/v5",
                      cpu_affinity=affinity, input_digest="a" * 64,
                      source_root=[1] * 32, proof_debug_digest="b" * 64,
                      **{key: 10 for key in runner.METRICS})
        return job, manifest, sample

    def test_both_benchmark_output_formats(self):
        job, manifest, sample = self.fixture()
        trials = runner.normalize_trials([sample], job, manifest)
        self.assertEqual(trials[0]["source_root"], "01" * 32)
        prepared = dict(event="prepared", protocol_id=sample.pop("protocol_id"),
                        input_seed=sample.pop("seed"), cpu_affinity=sample.pop("cpu_affinity"))
        sample["security_target"] = sample.pop("security_bits")
        sample["proof_prove_ms"] = sample.pop("prove_ms")
        sample["proof_verify_ms"] = sample.pop("verify_ms")
        sample["source_root"] = "01" * 32
        job["variant"] = "full"
        sample["flock_verifier_affinity"] = dict(observed_cpus=[6])
        self.assertEqual(runner.normalize_trials([prepared, sample], job, manifest), trials)
        sample["flock_verifier_affinity"]["observed_cpus"] = [7]
        with self.assertRaisesRegex(ValueError, "verifier CPU affinity"):
            runner.normalize_trials([prepared, sample], job, manifest)

    def test_missing_verification_affinity_and_samples_are_rejected(self):
        job, manifest, sample = self.fixture()
        for mutate in [lambda s: s.update(verified=False),
                       lambda s: s.pop("proof_debug_digest"),
                       lambda s: s.update(threads=1),
                       lambda s: s.update(total_prover_ms=float("nan")),
                       lambda s: s["cpu_affinity"].update(observed_main_cpus=[7])]:
            invalid = copy.deepcopy(sample)
            mutate(invalid)
            with self.assertRaises(ValueError):
                runner.normalize_trials([invalid], job, manifest)
        with self.assertRaises(ValueError):
            runner.normalize_trials([], job, manifest)

    def test_proof_groups_use_actual_protocol_across_threads(self):
        job, manifest, sample = self.fixture()
        baseline = dict(job=job, trials=runner.normalize_trials([sample], job, manifest))
        candidate = copy.deepcopy(baseline)
        candidate["job"]["threads"] = 16
        candidate["job"]["config"]["word"] = "auto"
        runner.check_equivalence([baseline, candidate])
        candidate["trials"][0]["proof_debug_digest"] = "c" * 64
        with self.assertRaisesRegex(ValueError, "proof contents mismatch"):
            runner.check_equivalence([baseline, candidate])
        candidate["trials"][0]["protocol_id"] = "word/v6"
        runner.check_equivalence([baseline, candidate])
        candidate["job"]["config"]["word"] = "on"
        runner.check_equivalence([baseline, candidate])
        candidate["trials"][0]["proof_debug_digest"] = "b" * 64
        with self.assertRaisesRegex(ValueError, "distinct protocol and proof digest"):
            runner.check_equivalence([baseline, candidate])

    def test_changed_input_and_bit_protocol_rejected(self):
        job, manifest, sample = self.fixture()
        baseline = dict(job=job, trials=runner.normalize_trials([sample], job, manifest))
        for key, value in [("input_digest", "c" * 64), ("source_root", "c" * 64),
                           ("protocol_id", "unexplained/v9")]:
            changed = copy.deepcopy(baseline)
            changed["trials"][0][key] = value
            with self.assertRaises(ValueError):
                runner.check_equivalence([baseline, changed])

    def test_process_runner_resume_and_raw_output_tampering(self):
        # A synthetic executable exercises orchestration, not cryptographic performance.
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            binary = root / "fake_bench"
            binary.write_text("""#!/usr/bin/env python3
import json, os, sys
options = dict(zip(sys.argv[1::2], sys.argv[2::2]))
workers = [int(x) for x in os.environ['BITZ_FALCON_WORKER_CPUS'].split(',')]
main = int(os.environ['BITZ_FALCON_MAIN_CPU'])
for i in range(int(options['--warmup']) + int(options['--iterations'])):
    sample = dict(trial='warmup' if i < int(options['--warmup']) else 'sample',
                  verified=True, degree=int(options['--degree']), batch=int(options['--batch']),
                  security_bits=int(options['--security']), seed=int(options['--seed']),
                  threads=int(options['--threads']), protocol_id='fake/bit/v5',
                  cpu_affinity=dict(observed_worker_cpus=[[x] for x in workers],
                                    observed_main_cpus=[main],
                                    observed_auxiliary_worker_cpus=[[main]] * len(workers)),
                  input_digest='a'*64, source_root='b'*64, proof_debug_digest='c'*64)
    sample.update({key: 10 for key in ['total_prover_ms', 'prove_ms', 'witness_commit_ms',
                                      'verify_ms', 'process_peak_rss_kib', 'proof_payload_bytes']})
    print(json.dumps(sample))
""")
            binary.chmod(0o700)
            cpu = min(os.sched_getaffinity(0))
            output = root / "results"
            command = [sys.executable, str(Path(runner.__file__)), "run", "--output", str(output),
                       "--variants", "arithmetic", "--configs", "baseline,auto", "--threads", "1",
                       "--seeds", "42", "--worker-cpus", str(cpu), "--main-cpu", str(cpu),
                       "--baseline-arithmetic", str(binary), "--candidate-arithmetic", str(binary)]
            first = subprocess.run(command, text=True, capture_output=True)
            self.assertEqual(first.returncode, 0, first.stderr)
            self.assertEqual(json.loads((output / "summary.json").read_text())["completed"], 2)
            second = subprocess.run(command, text=True, capture_output=True)
            self.assertEqual(second.returncode, 0, second.stderr)
            self.assertIn("resume verified", second.stdout)
            self.assertEqual(len(list(output.glob("runs/*/attempt-*"))), 2)
            raw = next(output.glob("runs/*/attempt-*/stdout.jsonl"))
            raw.write_text(raw.read_text() + "\n")
            third = subprocess.run(command, text=True, capture_output=True)
            self.assertNotEqual(third.returncode, 0)
            self.assertIn("changed raw benchmark output", third.stderr)

    def test_summary_retains_peak_rss_including_warmup(self):
        job, _, sample = self.fixture()
        job.update(variant="arithmetic", repetition=0)
        job["config"]["name"] = "baseline"
        trials = [dict(sample, trial="warmup", process_peak_rss_kib=50)]
        trials += [dict(sample, total_prover_ms=x, process_peak_rss_kib=x)
                   for x in [10, 20, 30]]
        with tempfile.TemporaryDirectory() as temporary:
            output = Path(temporary)
            runner.summarize(output, dict(trace=False, jobs=[job]),
                             [dict(job=job, trials=trials)])
            row = json.loads((output / "summary.json").read_text())["rows"][0]
            self.assertEqual(row["median_total_prover_ms"], 20)
            self.assertEqual(row["median_process_peak_rss_kib"], 50)


if __name__ == "__main__":
    unittest.main()

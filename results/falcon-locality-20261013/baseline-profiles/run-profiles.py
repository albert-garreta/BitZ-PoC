#!/usr/bin/env python3
"""Standalone frozen-baseline diagnostics; not a latency qualification phase."""
import json
import os
from pathlib import Path
import sys
import time

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT / "scripts"))
from bench_support import file_hash, run_process, write_json
from binder_experiments import read_jsonl
from falcon_buffer_optimization import normalize
from falcon_worker_optimization import parser_provenance, subprocess_env

OUT = Path(__file__).resolve().parent
PRIOR = ROOT / "results/falcon-twelve-workers-20261013"
BINARY = ROOT / ".tmp/falcon-twelve-workers-20261013/candidate-production"
EXPECTED_HASH = "bf303cb59a2e6830f0beb97670bd693aaef0d22fd5ad54bb7304696e0c850511"
CPUS = [6, 7, 4, 5, 2, 3, 0, 1, 14, 15, 12, 13]
SETTINGS = dict(batch=1024, security=128, platform=sys.platform,
                worker_cpus=CPUS, main_cpu=6, candidate_production=True)


def main():
    if file_hash(BINARY) != EXPECTED_HASH:
        raise ValueError("frozen baseline hash changed")
    provenance = parser_provenance()
    artifacts = ["candidate-production-source.json", "candidate-production.patch",
                 "candidate-production-binary.json", "production-build-audit.json"]
    write_json(OUT / "diagnostic-settings.json", dict(
        schema="bitz/falcon-standalone-pcs-diagnostics/v1", settings=SETTINGS,
        binary=dict(path=str(BINARY), sha256=EXPECTED_HASH),
        source_artifacts={name: dict(path=str(PRIOR / name), sha256=file_hash(PRIOR / name))
                          for name in artifacts},
        parser_provenance=provenance, runner_sha256=file_hash(Path(__file__)),
        notes=["Six sequential fresh processes; one warmup and three samples per process.",
               "Diagnostic tracing only; not latency qualification.",
               "Environment uses the existing strict runtime-selector scrubber and recorded overrides."]
    ))
    for degree in [512, 1024]:
        for seed in [42, 43, 44]:
            prior_id = f"production-repeated-n{degree}-b1024-s128-t12-seed{seed}-r0"
            prior = PRIOR / "runs/profiles" / prior_id
            job = json.loads((prior / "result.json").read_text())["job"]
            job = dict(job, id=f"baseline-n{degree}-b1024-s128-t12-seed{seed}", config="baseline")
            directory = OUT / job["id"]
            directory.mkdir()
            record = json.loads((prior / "command.json").read_text())
            record["overrides"]["LIG_PROVE_TRACE"] = "1"
            if record["binary_sha256"] != EXPECTED_HASH or record["command"][0] != str(BINARY):
                raise ValueError("unexpected prior profile command")
            if record["overrides"]["BITZ_FALCON_WORKER_CPUS"] != ",".join(map(str, CPUS)):
                raise ValueError("unexpected prior affinity")
            write_json(directory / "command.json", record)
            env = subprocess_env(record["overrides"])
            print(f"RUN {job['id']}", flush=True)
            start = time.time()
            with (directory / "stdout.jsonl").open("w") as stdout, (directory / "stderr.log").open("w") as stderr:
                code, timeout = run_process(record["command"], env=env, cwd=ROOT,
                                            stdout=stdout, stderr=stderr, timeout=120)
            write_json(directory / "process.json", dict(start_unix=start,
                elapsed_seconds=time.time() - start, exit_code=code, timed_out=timeout))
            if code != 0 or timeout:
                raise RuntimeError(f"diagnostic failed: {directory}")
            trials = normalize(read_jsonl(directory / "stdout.jsonl"), job, SETTINGS)
            reference = json.loads((prior / "result.json").read_text())["trials"]
            for actual, expected in zip(trials, reference):
                for key in ["protocol_id", "input_digest", "source_root", "proof_debug_digest", "proof_payload_bytes"]:
                    if actual[key] != expected[key]:
                        raise ValueError(f"baseline identity mismatch for {key}: {job['id']}")
            write_json(directory / "result.json", dict(
                schema="bitz/falcon-standalone-pcs-diagnostics/v1", job=job,
                verified=True, matches_previous_baseline_proofs=True, trials=trials,
                binary_sha256=EXPECTED_HASH,
                hashes={name: file_hash(directory / name) for name in
                        ["command.json", "stdout.jsonl", "stderr.log", "process.json"]}))
            print(f"OK {job['id']}", flush=True)
    if file_hash(BINARY) != EXPECTED_HASH or parser_provenance() != provenance:
        raise ValueError("baseline binary or parser changed during diagnostics")
    write_json(OUT / "execution-complete.json", dict(processes=6, verified_proofs=24,
        measured_proofs=18, warmups=6, binary_sha256=EXPECTED_HASH,
        parser_provenance=provenance, completed_unix=time.time()))
    print("COMPLETE: six processes, 24 verified proofs (six warmups excluded from profiles)", flush=True)


if __name__ == "__main__":
    main()

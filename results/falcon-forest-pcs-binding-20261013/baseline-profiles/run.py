#!/usr/bin/env python3
"""Capture the unchanged production forest timers before selecting experiments."""
import json
import os
from pathlib import Path
import sys
import time

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT / 'scripts'))
from bench_support import file_hash, run_process, write_json
from binder_experiments import read_jsonl
from falcon_buffer_optimization import normalize
from falcon_arithmetic_optimization import CPUS

OUT = Path(__file__).resolve().parent
BINARY = ROOT / '.tmp/falcon-buffer-optimization-20261012/candidate-production'
assert file_hash(BINARY) == '321f791c5d7b5c09de5cb3b92ff69327cd0e23272117cc5a69da0d76bb9d235d'
manifest = dict(batch=1024, security=128, worker_cpus=CPUS, main_cpu=6,
                platform=sys.platform, candidate_production=True)
reference_dir = ROOT / 'results/falcon-buffer-optimization-20261012/runs/production'
references = {}
for path in reference_dir.glob('baseline-*/result.json'):
    result = json.loads(path.read_text())
    for trial in result['trials']:
        key = result['job']['degree'], result['job']['seed']
        references[key] = tuple(trial[name] for name in
                                ['input_digest', 'source_root', 'proof_debug_digest', 'proof_payload_bytes'])
for threads in [8, 12]:
    for seed in [42, 43, 44]:
        job = dict(id=f'baseline-n512-t{threads}-seed{seed}', degree=512, threads=threads,
                   seed=seed, warmup=1, iterations=3, variant='arithmetic',
                   config='baseline', protocol='canonical', mode='repeated')
        directory = OUT / job['id']
        directory.mkdir(exist_ok=True)
        overrides = dict(RAYON_NUM_THREADS=str(threads),
                         BITZ_FALCON_WORKER_CPUS=','.join(map(str, CPUS[:threads])),
                         BITZ_FALCON_MAIN_CPU='6', BITZ_TRACE='1',
                         BITZ_FALCON_LIGHT_PROFILE='1',
                         BITZ_FALCON_CASE_CACHE=str(ROOT / '.tmp/falcon-poc-performance-cases-20261012'))
        command = [str(BINARY), '--degree', '512', '--batch', '1024', '--security', '128',
                   '--threads', str(threads), '--seed', str(seed), '--warmup', '1',
                   '--iterations', '3', '--trace']
        env = {key: value for key, value in os.environ.items()
               if not key.startswith(('BITZ_', 'F2_', 'OBLONG_'))
               and key not in ['RAYON_NUM_THREADS', 'RUSTFLAGS', 'CARGO_ENCODED_RUSTFLAGS']}
        env.update(overrides)
        write_json(directory / 'command.json', dict(command=command, overrides=overrides,
                   binary_sha256=file_hash(BINARY), source_commit='a4585dffa', cwd=str(ROOT)))
        started = time.time()
        with (directory / 'stdout.jsonl').open('w') as stdout, (directory / 'stderr.log').open('w') as stderr:
            code, timed_out = run_process(command, env=env, cwd=ROOT, stdout=stdout,
                                          stderr=stderr, timeout=600)
        assert code == 0 and not timed_out, directory
        trials = normalize(read_jsonl(directory / 'stdout.jsonl'), job, manifest)
        for trial in trials:
            actual = tuple(trial[name] for name in
                           ['input_digest', 'source_root', 'proof_debug_digest', 'proof_payload_bytes'])
            assert actual == references[512, seed], directory
        write_json(directory / 'result.json', dict(job=job, trials=trials, verified=True,
                   elapsed_seconds=time.time()-started,
                   hashes={name:file_hash(directory/name) for name in
                           ['command.json', 'stdout.jsonl', 'stderr.log']}))
        print(f'OK {job["id"]}', flush=True)

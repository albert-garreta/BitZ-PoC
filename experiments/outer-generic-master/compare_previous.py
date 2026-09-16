#!/usr/bin/env python3
"""Supplementary comparison against a frozen generic implementation.

This does not replace either acceptance requirement in qualify.py.
"""
import argparse
import csv
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import statistics
import subprocess

ROOT = Path(__file__).resolve().parents[2]
p = argparse.ArgumentParser(description=__doc__)
p.add_argument('--previous', type=Path, required=True)
p.add_argument('--current', type=Path, required=True)
p.add_argument('--out', type=Path, required=True)
p.add_argument('--reps', type=int, default=21)
p.add_argument('--protocols', nargs='+', choices=['ordinary', 'skip-3'], default=['skip-3'])
p.add_argument('--cases', type=Path, help='JSON list of [bits, exponent, threads, protocol] for confirmation')
p.add_argument('--affinity-1', help='Optional fixed CPU list for one-thread confirmation')
p.add_argument('--affinity-10', help='Optional fixed CPU list for ten-thread confirmation')
p.add_argument('--previous-sha256', default='94a0f8043b57c3e3b93b3ca5a45277b85db743f2090cffe8798f90c6ce9d68e0',
               help='Expected previous executable hash; defaults to frozen f86193f1')
a = p.parse_args()
assert a.reps >= 21
affinity = {t: {int(cpu) for cpu in value.split(',')} for t, value in
            [(1, a.affinity_1), (10, a.affinity_10)] if value}
assert all(len(cpus) >= threads and cpus <= os.sched_getaffinity(0)
           for threads, cpus in affinity.items())
a.out.mkdir(parents=True, exist_ok=False)
executables = {'previous': a.previous.resolve(), 'current': a.current.resolve()}
hashes = {k: hashlib.file_digest(v.open('rb'), 'sha256').hexdigest() for k, v in executables.items()}
pins = json.loads((Path(__file__).parent / 'preoptimization-pins.json').read_text())
assert hashes['previous'] == a.previous_sha256
(a.out / 'metadata.json').write_text(json.dumps(dict(
    executables={k: str(v) for k, v in executables.items()}, sha256=hashes,
    previous_expected_sha256=a.previous_sha256, protocols=a.protocols, samples=a.reps,
    affinity={threads: sorted(cpus) for threads, cpus in affinity.items()},
    note='Supplementary comparison; acceptance remains master ordinary and new ordinary.'
), indent=2) + '\n')
spec = importlib.util.spec_from_file_location('gate', ROOT / 'scripts/bench_gate.py')
gate = importlib.util.module_from_spec(spec)
spec.loader.exec_module(gate)
env = {k: v for k, v in os.environ.items() if not k.startswith(('F2Z_', 'OUTER_', 'RAYON_'))}
cases = [(b, n, t, protocol) for t in [1, 10] for n in [12, 15, 17, 19]
         for b in [32, 64, 128] for protocol in a.protocols]
if a.cases:
    selected = [tuple(case) for case in json.loads(a.cases.read_text())]
    assert selected and len(set(selected)) == len(selected) and set(selected) <= set(cases)
    cases = selected
summary = []
gate.acquire('previous-generic-skip', 10)
try:
    for session in range(2):
        gate.wait_idle(f'previous-skip-{session}', 95, 30, 5, 600)
        indexed_cases = list(enumerate(cases))
        for i, (bits, n, threads, protocol) in indexed_cases if session == 0 else reversed(indexed_cases):
            medians = {}
            order = ['previous', 'current'] if (i+session) % 2 == 0 else ['current', 'previous']
            for name in order:
                path = a.out / f's{session}-u{bits}-e{n}-t{threads}-{protocol}-{name}.jsonl'
                run_env = dict(env, OUTER_REPS=str(a.reps), OUTER_SHAPES=str(n),
                               OUTER_WIDTHS=str(bits), OUTER_PROTOCOLS=protocol,
                               OUTER_VARIANTS='generic', RAYON_NUM_THREADS=str(threads))
                with path.open('w') as f:
                    bind = (lambda: os.sched_setaffinity(0, affinity[threads])) if threads in affinity else None
                    subprocess.run([str(executables[name])], env=run_env, stdout=f,
                                   stderr=subprocess.STDOUT, check=True, preexec_fn=bind)
                rows = [json.loads(line[13:]) for line in path.read_text().splitlines()
                        if line.startswith('OUTER_SAMPLE ')]
                assert len(rows) == a.reps+1 and sum(r['warmup'] for r in rows) == 1
                assert all(r['verified'] and r['threads'] == threads and r['bits'] == bits
                           and r['rows'] == 1 << n and r['protocol'] == protocol
                           and r['proof_digest'] == pins['pins'][f'{bits}/{1 << n}/{protocol}']
                           for r in rows)
                medians[name] = statistics.median(int(r['ns']) for r in rows if not r['warmup'])
            row = dict(session=session, bits=bits, exponent=n, threads=threads, protocol=protocol,
                       previous_ns=medians['previous'], current_ns=medians['current'],
                       ratio=medians['current']/medians['previous'])
            summary.append(row)
            print(json.dumps(row), flush=True)
    with (a.out / 'comparison.csv').open('w') as f:
        writer = csv.DictWriter(f, fieldnames=list(summary[0]))
        writer.writeheader()
        writer.writerows(summary)
    assert hashes == {k: hashlib.file_digest(v.open('rb'), 'sha256').hexdigest()
                      for k, v in executables.items()}
finally:
    gate.release()

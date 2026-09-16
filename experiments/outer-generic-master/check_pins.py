#!/usr/bin/env python3
"""Check ordinary and every K against frozen f86193f1 proof/transcript digests."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess

p = argparse.ArgumentParser(description=__doc__)
p.add_argument('executable', type=Path)
p.add_argument('output', type=Path)
a = p.parse_args()
a.output.mkdir(parents=True, exist_ok=False)
exe = a.executable.resolve()
pins = json.loads((Path(__file__).parent/'preoptimization-pins.json').read_text())['pins']
env = {k: v for k, v in os.environ.items() if not k.startswith(('F2Z_', 'OUTER_', 'RAYON_'))}
for threads in [1, 10]:
    run_env = dict(env, OUTER_REPS='1', OUTER_SHAPES='12 15 17 19',
                   OUTER_WIDTHS='32 64 128', OUTER_PROTOCOLS='ordinary skip-1 skip-2 skip-3 skip-4',
                   OUTER_VARIANTS='production generic', RAYON_NUM_THREADS=str(threads))
    path = a.output/f'threads-{threads}.jsonl'
    with path.open('w') as f:
        subprocess.run([str(exe)], env=run_env, stdout=f, stderr=subprocess.STDOUT, check=True)
    rows = [json.loads(line[13:]) for line in path.read_text().splitlines()
            if line.startswith('OUTER_SAMPLE ')]
    expected = {(bits, 1 << n, protocol, variant, sample)
                for bits in [32, 64, 128] for n in [12, 15, 17, 19]
                for protocol in ['ordinary', 'skip-1', 'skip-2', 'skip-3', 'skip-4']
                for variant in ['production', 'generic'] for sample in [0, 1]}
    assert len(rows) == len(expected)
    assert {(r['bits'], r['rows'], r['protocol'], r['variant'], r['sample']) for r in rows} == expected
    for r in rows:
        key = f"{r['bits']}/{r['rows']}/{r['protocol']}"
        assert r['verified'] and r['threads'] == threads and r['proof_digest'] == pins[key], r
    print(f'{threads} threads: all 60 proof/transcript pins match through both entrypoints', flush=True)
(a.output/'status.json').write_text(json.dumps(dict(
    passed=True, sha256=hashlib.file_digest(exe.open('rb'), 'sha256').hexdigest(),
    reference='f86193f1dcbb43a4bba48ea20670914df334f750',
    cases=240, warmups_excluded_from_case_count=True))+'\n')

#!/usr/bin/env python3
"""Run final production qualification serially after build and tests finish."""
import json
from pathlib import Path
import subprocess
import sys
import time

ROOT = Path(__file__).resolve().parents[2]
OUT = Path(__file__).resolve().parent
while not (OUT / 'candidate-final-binary.json').exists() or not (OUT / 'test-final-exit.json').exists():
    time.sleep(1)
assert json.loads((OUT / 'test-final-exit.json').read_text())['exit_code'] == 0
base = [sys.executable, str(ROOT / 'scripts/falcon_locality_optimization.py'),
        '--output', str(OUT), '--binary', str(ROOT / '.tmp/falcon-locality-20261013/candidate-final'),
        '--baseline', str(ROOT / '.tmp/falcon-twelve-workers-20261013/candidate-production'),
        '--variants', 'baseline', 'production']
phases = [
    ('final1', ['--threads', '1', '8', '12', '16', '--modes', 'repeated']),
    ('final2', ['--threads', '1', '8', '12', '16', '--modes', 'repeated']),
    ('final-cold1', ['--threads', '12', '--modes', 'cold']),
    ('final-cold2', ['--threads', '12', '--modes', 'cold']),
    ('final-profile', ['--threads', '12', '--modes', 'repeated', '--warmup', '1', '--iterations', '3', '--trace']),
]
for security in [100, 128]:
    for batch in ([3, 129, 513, 1024] if security == 100 else [3, 129, 513]):
        phases.append((f'final-padding-b{batch}-s{security}',
                       ['--threads', '12', '--modes', 'cold', '--cold-repetitions', '1',
                        '--batch', str(batch), '--security', str(security)]))
for phase, extra in phases:
    cmd = base + ['--phase', phase] + extra
    (OUT / f'{phase}-command.json').write_text(json.dumps(cmd, indent=2) + '\n')
    print('START', phase, flush=True)
    with (OUT / f'{phase}-console.log').open('w') as log:
        result = subprocess.call(cmd, cwd=ROOT, stdout=log, stderr=subprocess.STDOUT)
    print('EXIT', phase, result, flush=True)
    if result:
        print((OUT / f'{phase}-console.log').read_text()[-6000:], flush=True)
        sys.exit(result)
print('ALL FINAL PHASES COMPLETE', flush=True)

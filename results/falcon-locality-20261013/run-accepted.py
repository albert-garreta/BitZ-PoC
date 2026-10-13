#!/usr/bin/env python3
"""Run the final narrowed production policy only after its build and tests pass."""
import json
from pathlib import Path
import subprocess
import sys
import time

ROOT = Path(__file__).resolve().parents[2]
OUT = Path(__file__).resolve().parent
while not (OUT / 'candidate-accepted-binary.json').exists() or not (OUT / 'test-accepted-exit.json').exists():
    time.sleep(1)
assert json.loads((OUT / 'test-accepted-exit.json').read_text())['exit_code'] == 0
base = [sys.executable, str(ROOT / 'scripts/falcon_locality_optimization.py'),
        '--output', str(OUT),
        '--binary', str(ROOT / '.tmp/falcon-locality-20261013/candidate-accepted'),
        '--baseline', str(ROOT / '.tmp/falcon-twelve-workers-20261013/candidate-production'),
        '--variants', 'baseline', 'production']
phases = [
    ('accepted1', ['--threads', '8', '12', '--modes', 'cold', 'repeated']),
    ('accepted2', ['--threads', '8', '12', '--modes', 'cold', 'repeated']),
    ('accepted-regress', ['--threads', '1', '16', '--modes', 'repeated']),
    ('accepted-profile', ['--threads', '12', '--modes', 'repeated', '--warmup', '1', '--iterations', '3', '--trace']),
    ('accepted-padding', ['--threads', '12', '--modes', 'cold', '--cold-repetitions', '1', '--batch', '513', '--security', '128']),
    ('accepted-security100', ['--threads', '12', '--modes', 'cold', '--cold-repetitions', '1', '--batch', '1024', '--security', '100']),
]
for phase, extra in phases:
    cmd = base + ['--phase', phase] + extra
    (OUT / f'{phase}-command.json').write_text(json.dumps(cmd, indent=2) + '\n')
    print('START', phase, flush=True)
    with (OUT / f'{phase}-console.log').open('w') as log:
        rc = subprocess.call(cmd, cwd=ROOT, stdout=log, stderr=subprocess.STDOUT)
    print('EXIT', phase, rc, flush=True)
    if rc:
        print((OUT / f'{phase}-console.log').read_text()[-6000:], flush=True)
        sys.exit(rc)
print('ALL ACCEPTED PHASES COMPLETE', flush=True)

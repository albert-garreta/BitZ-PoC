import json
from pathlib import Path
import subprocess
import sys

root = Path.cwd()
results = root / 'results/falcon-commit-experiments-20261008'
labels = sys.argv[1:] or ['merkle', 'keccak', 'arithmetic', 'encoding']
for label in labels:
    builds = {v: json.loads((results / (name + '-build.json')).read_text())
              for v, name in [('baseline', 'baseline'), ('candidate', label)]}
    for campaign, blocks in [('e2e', 5), ('stages', 2)]:
        command = [sys.executable, '-B', 'scripts/qualify_falcon_witness.py']
        for variant in ['baseline', 'candidate']:
            name = 'baseline' if variant == 'baseline' else label
            command += ['--' + variant, builds[variant]['binaries']['full']['frozen_path'],
                        '--' + variant + '-provenance', str(results / (name + '-build.json'))]
        command += ['--out', str(results / label / campaign),
                    '--cache', '/tmp/falcon-algebraic-cases-20261007',
                    '--mode', 'full', '--degrees', '512,1024', '--threads', '1,8',
                    '--batch', '1024', '--security', '128', '--seed', '42',
                    '--blocks', str(blocks), '--warmup', '2', '--samples', '3']
        if campaign == 'stages':
            command += ['--diagnostic']
        print('START', label, campaign, flush=True)
        with (results / (label + '-' + campaign + '.log')).open('x') as log:
            subprocess.run(command, check=True, stdout=log, stderr=subprocess.STDOUT)
        print('DONE', label, campaign, flush=True)
        subprocess.run([sys.executable, '-B', str(results / 'summarize.py')], check=True)

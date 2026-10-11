"""Run the fixed RSS extension or the final eight-cell qualification."""
import argparse
import json
from pathlib import Path
import subprocess
import sys

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('campaign', choices=['arithmetic-extension', 'final', 'arithmetic-final-extension'])
args = parser.parse_args()
results = Path(__file__).resolve().parent
root = results.parents[1]


def run(label, phase, mode, blocks, degrees='512,1024', threads='1,8'):
    command = [sys.executable, '-B', str(root / 'scripts/qualify_falcon_witness.py')]
    for variant, build in [('baseline', 'baseline'), ('candidate', label)]:
        provenance = results / (build + '-build.json')
        manifest = json.loads(provenance.read_text())
        command += ['--' + variant, manifest['binaries'][mode]['frozen_path'],
                    '--' + variant + '-provenance', str(provenance)]
    command += ['--out', str(results / label / phase),
                '--cache', '/tmp/falcon-algebraic-cases-20261007',
                '--mode', mode, '--degrees', degrees, '--threads', threads,
                '--batch', '1024', '--security', '128', '--seed', '42',
                '--blocks', str(blocks), '--warmup', '2', '--samples', '3']
    if phase.endswith('stages'):
        command.append('--diagnostic')
    print('START', label, phase, flush=True)
    with (results / (label + '-' + phase + '.log')).open('x') as log:
        subprocess.run(command, cwd=root, check=True, stdout=log, stderr=subprocess.STDOUT)
    print('DONE', label, phase, flush=True)
    subprocess.run([sys.executable, '-B', str(results / 'summarize.py')], cwd=root, check=True)


if args.campaign == 'arithmetic-extension':
    run('arithmetic', 'e2e-extension', 'full', 5, degrees='512', threads='8')
elif args.campaign == 'arithmetic-final-extension':
    run('final', 'arithmetic-e2e-extension-n512', 'arithmetic', 5, degrees='512')
    run('final', 'arithmetic-e2e-extension-n1024t8', 'arithmetic', 5, degrees='1024', threads='8')
else:
    retained = json.loads((results / 'arithmetic-build.json').read_text())
    final = json.loads((results / 'final-build.json').read_text())
    same_source = retained['source']['source_sha256'] == final['source']['source_sha256']
    same_binaries = all(retained['binaries'][mode]['sha256'] == final['binaries'][mode]['sha256']
                        for mode in ['full', 'arithmetic'])
    modes = ['full', 'arithmetic']
    if same_source and same_binaries:
        reused = ['arithmetic/e2e', 'arithmetic/e2e-extension', 'arithmetic/stages']
        for name in reused:
            assert json.loads((results / name / 'manifest.json').read_text())['complete']
        (results / 'final').mkdir(exist_ok=True)
        (results / 'final/reused-full.json').write_text(json.dumps({
            'reason': 'Identical final and retained candidate source files and executable SHA-256 hashes.',
            'campaigns': reused,
            'retained_build': 'arithmetic-build.json',
            'final_build': 'final-build.json',
            'binary_sha256': final['binaries']['full']['sha256'],
        }, indent=2) + '\n')
        print('REUSE byte-identical full-Falcon measurements:', ', '.join(reused), flush=True)
        modes = ['arithmetic']
    for mode in modes:
        for phase, blocks in [('e2e', 5), ('stages', 2)]:
            run('final', mode + '-' + phase, mode, blocks)

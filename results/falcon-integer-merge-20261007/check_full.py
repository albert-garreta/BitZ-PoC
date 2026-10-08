import json, os, statistics, subprocess
from pathlib import Path

ROOT = Path.cwd()
OUT = ROOT / 'target/falcon-integer-merge/full-validation'
OUT.mkdir(exist_ok=True)
meta = json.loads((ROOT / 'target/falcon-integer-merge/measurements/metadata.json').read_text())
(OUT / 'metadata.json').write_text(json.dumps({**meta, 'family': 'full', 'security': 128, 'samples': 21}, indent=2) + '\n')
env = os.environ.copy()
for key in list(env):
    if key.startswith(('BITZ_', 'FLOCK_')):
        del env[key]
env.update(RAYON_NUM_THREADS='8', NO_COLOR='1')
records = []
for degree in [512, 1024]:
    order = ['baseline', 'candidate'] if degree == 512 else ['candidate', 'baseline']
    for revision in order:
        name = f'full-{degree}-128-{revision}'
        cmd = [meta['executables'][revision]['full']['path'], '--degree', str(degree),
               '--batch', '1024', '--security', '128', '--threads', '8', '--seed', '42',
               '--warmup', '1', '--iterations', '21']
        print('START', name, flush=True)
        with (OUT / f'{name}.stdout.log').open('w') as out, (OUT / f'{name}.stderr.log').open('w') as err:
            subprocess.run(cmd, cwd=ROOT, env=env, stdout=out, stderr=err, check=True)
        trials = [json.loads(line) for line in (OUT / f'{name}.stdout.log').read_text().splitlines() if line.startswith('{')]
        trials = [t for t in trials if t.get('trial') in ('sample', 'warmup')]
        samples = [t for t in trials if t['trial'] == 'sample']
        assert len(trials) == 22 and len(samples) == 21
        assert all(t['verified'] and t['threads'] == 8 and t['batch'] == 1024 and t['security_target'] == 128 for t in trials)
        medians = {k: statistics.median(s[k] for s in samples)
                   for k in ['proof_prove_ms', 'witness_commit_ms', 'total_prover_ms', 'proof_verify_ms', 'proof_payload_bytes']}
        records.append({'name': name, 'degree': degree, 'revision': revision, 'command': cmd, 'samples': samples, 'medians': medians})
        (OUT / 'results.json').write_text(json.dumps(records, indent=2) + '\n')
        print('DONE', name, json.dumps(medians), flush=True)
    pair = [r for r in records if r['degree'] == degree]
    assert len({s['input_digest'] for r in pair for s in r['samples']}) == 1
print('COMPLETE', flush=True)

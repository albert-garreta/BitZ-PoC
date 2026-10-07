#!/usr/bin/env python3
"""Separate traced runs; never mix these samples into throughput results."""
import argparse
import json
import os
from pathlib import Path
import re
import statistics
import subprocess

p = argparse.ArgumentParser(description=__doc__)
p.add_argument('--root', type=Path, default=Path('/home/john-wu/code/BitZ-pcs'))
p.add_argument('--baseline', type=Path, default=Path('/tmp/falcon-layout-baseline'))
p.add_argument('--candidate', type=Path, required=True)
p.add_argument('--out', type=Path, required=True)
a = p.parse_args()
a.out.mkdir(parents=True, exist_ok=False)
rows = []
ansi = re.compile(r'\x1b\[[0-9;]*m')
for kind, executable in [('algebraic', 'falcon_algebraic'), ('full', 'falcon_hybrid')]:
    for batch in [32, 1024]:
        for threads in [1, 16]:
            row = dict(kind=kind, batch=batch, security_bits=128, threads=threads)
            for variant, directory in [('baseline', a.baseline), ('candidate', a.candidate)]:
                name = f'{kind}-b{batch}-t{threads}-{variant}'
                cpus = ','.join(map(str, range(threads)))
                env = os.environ.copy()
                for key in ['PCS_TRACE', 'FLOCK_COMMIT_TIMING', 'LIGERITO_TRACE', 'LIG_PROVE_TRACE', 'LIG_VERIFY_TRACE', 'BITZ_FALCON_STAGE_TIMINGS']:
                    env.pop(key, None)
                env.update(RAYON_NUM_THREADS=str(threads), BITZ_FALCON_WORKER_CPUS=cpus,
                           BITZ_FALCON_MAIN_CPU='0', FLOCK_NO_PREFAULT='1',
                           BITZ_FALCON_CASE_CACHE='/tmp/falcon-algebraic-cases-20261007')
                cmd = ['taskset', '-c', cpus, str(directory / executable), '--batch', str(batch),
                       '--security', '128', '--threads', str(threads), '--warmup', '1',
                       '--iterations', '5', '--seed', '42']
                if kind == 'algebraic':
                    cmd.append('--trace')
                else:
                    cmd += ['--degree', '1024', '--k', '11']
                    env['BITZ_FALCON_STAGE_TIMINGS'] = '1'
                print(name, flush=True)
                with (a.out / f'{name}.jsonl').open('w') as stdout, (a.out / f'{name}.stderr').open('w') as stderr:
                    subprocess.run(cmd, cwd=a.root, env=env, stdout=stdout, stderr=stderr, check=True, timeout=1800)
                lines = (a.out / f'{name}.stderr').read_text().splitlines()
                if kind == 'full':
                    events = [json.loads(line) for line in lines if line.startswith('{')]
                    samples = [e['elapsed_ms'] for e in events if e.get('name') == 'falcon_arithmetic:binding_inner']
                else:
                    samples = []
                    for line in lines:
                        line = ansi.sub('', line)
                        if not re.search(r'falcon_algebraic:binding:\s+\S+:\s+close\s+time\.', line):
                            continue
                        durations = re.findall(r'time\.(busy|idle)=([0-9.]+)(ns|µs|us|ms|s)', line)
                        if len(durations) == 2:
                            samples.append(sum(float(value) * {'ns': 1e-6, 'µs': 1e-3, 'us': 1e-3, 'ms': 1, 's': 1000}[unit] for _, value, unit in durations))
                assert len(samples) == 6, (name, samples)
                row[f'{variant}_binding_ms'] = statistics.median(samples[1:])
                row[f'{variant}_samples_ms'] = samples[1:]
            rows.append(row)
            (a.out / 'summary.json').write_text(json.dumps(rows, indent=2) + '\n')
            print(json.dumps(row), flush=True)

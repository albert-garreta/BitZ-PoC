"""Separate traced runs; do not use these as end-to-end benchmark samples."""
from pathlib import Path
import argparse, json, os, re, statistics, subprocess

p = argparse.ArgumentParser()
p.add_argument('--repo', type=Path, required=True)
p.add_argument('--root', type=Path, required=True)
p.add_argument('--out', type=Path, required=True)
a = p.parse_args()
a.out.mkdir(parents=True, exist_ok=False)
env = os.environ.copy()
env.update(RAYON_NUM_THREADS='1', BITZ_FALCON_WORKER_CPUS='0', BITZ_FALCON_MAIN_CPU='0',
           FLOCK_NO_PREFAULT='1', BITZ_FALCON_CASE_CACHE='/tmp/falcon-algebraic-cases-20261007')
rows, commands = [], []
for n in [512, 1024]:
    for mode in ['baseline', 'candidate']:
        cmd = ['taskset', '-c', '0', str(a.root / mode / 'falcon_algebraic'), '--degree', str(n),
               '--batch', '1024', '--security', '128', '--threads', '1', '--warmup', '1',
               '--iterations', '5', '--seed', '42', '--trace']
        name = f'{mode}-n{n}'
        with (a.out / (name+'.jsonl')).open('w') as stdout, (a.out / (name+'.stderr')).open('w') as stderr:
            subprocess.run(cmd, cwd=a.repo, env=env, stdout=stdout, stderr=stderr, check=True, timeout=1800)
        commands.append(cmd)
        for stage in ['prove', 'verify']:
            times = []
            for line in (a.out / (name+'.stderr')).read_text().splitlines():
                line = re.sub(r'\x1b\[[0-9;]*m', '', line)
                if f'falcon_algebraic_ring:{stage}:' in line and 'falcon_algebraic_ring:coordinates:' in line and 'time.busy=' in line:
                    parts = re.findall(r'time\.(?:busy|idle)=([\d.]+)(ns|µs|us|ms|s)', line)
                    times.append(sum(float(v)*{'ns':1e-6, 'µs':1e-3, 'us':1e-3, 'ms':1, 's':1000}[u] for v,u in parts))
            assert len(times) == 6, (name, stage, len(times))
            rows.append(dict(mode=mode, degree=n, stage=stage, coordinates_ms=statistics.median(times[1:]), trials_ms=times))
        print(name, 'complete', flush=True)
(a.out / 'summary.json').write_text(json.dumps(rows, indent=2)+'\n')
(a.out / 'commands.json').write_text(json.dumps(commands, indent=2)+'\n')

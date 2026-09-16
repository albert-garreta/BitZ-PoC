#!/usr/bin/env python3
"""Capture opt-in generic phase measurements using the normal verified driver."""
import argparse,json,os,statistics,subprocess
from pathlib import Path
p=argparse.ArgumentParser();p.add_argument('executable',type=Path);p.add_argument('output',type=Path);p.add_argument('--protocol',choices=['ordinary','skip-3'],default='ordinary');p.add_argument('--widths',default='32 64 128');a=p.parse_args()
a.output.mkdir(parents=True,exist_ok=False)
for threads in [1,10]:
    env={k:v for k,v in os.environ.items() if not k.startswith(('OUTER_','RAYON_','F2Z_'))}
    env.update(OUTER_PHASES='1',OUTER_SHAPES='12 15 17 19',OUTER_WIDTHS=a.widths,OUTER_REPS='101',OUTER_PROTOCOLS=a.protocol,OUTER_VARIANTS='generic',RAYON_NUM_THREADS=str(threads))
    path=a.output/f'threads-{threads}.jsonl'
    with path.open('w') as f:subprocess.run([str(a.executable.resolve())],env=env,stdout=f,stderr=subprocess.STDOUT,check=True)
    rows=[json.loads(l.removeprefix('OUTER_SAMPLE ')) for l in path.read_text().splitlines() if l.startswith('OUTER_SAMPLE ')]
    for n in [12,15,17,19]:
        for b in map(int,a.widths.split()):
            cell=[r for r in rows if r['rows']==1<<n and r['bits']==b and not r['warmup']]
            assert len(cell)==101 and all(r['verified'] for r in cell)
            values=[statistics.median(r['phase_ns'][i] for r in cell)/1e6 for i in range(4)]
            print(json.dumps(dict(protocol=a.protocol,bits=b,exponent=n,threads=threads,setup_ms=values[0],coefficients_ms=values[1],fold_ms=values[2],continuation_ms=values[3],total_ms=statistics.median(int(r['ns']) for r in cell)/1e6)),flush=True)

#!/usr/bin/env python3
"""Development probes only: preserve raw pairs; never label these confirmation passes."""
import argparse
import csv
import hashlib
import json
import os
from pathlib import Path
import statistics
import subprocess

HERE = Path(__file__).resolve().parent


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=Path, required=True)
    parser.add_argument('--out', type=Path, required=True)
    parser.add_argument('--threads', type=int, default=1)
    parser.add_argument('--families', required=True)
    parser.add_argument('--manifest',type=Path,default=HERE/'optimization_cases.json')
    parser.add_argument('--samples', type=int, default=8)
    parser.add_argument('--runs', type=int, default=2)
    args = parser.parse_args()
    spec = json.loads(args.manifest.read_text())
    families = set(args.families.split(','))
    cases = {g['name'] + '/' + s for g in spec['families'] if g['name'] in families for s in g['sizes']}
    assert cases
    baselines = {g['name']: g['baseline'] for g in spec['families']}
    binary = args.binary.resolve()
    digest = hashlib.sha256(binary.read_bytes()).hexdigest()
    args.out.mkdir(parents=True, exist_ok=False)
    env = os.environ.copy()
    env.update(RAYON_NUM_THREADS=str(args.threads), FIELD_REGRESSION_CASES=','.join(sorted(cases)),
               FIELD_REGRESSION_BASELINES=json.dumps(baselines))
    metadata = dict(kind='development; not confirmation', binary=str(binary), binary_sha256=digest,
                    threads=args.threads, families=sorted(families), samples=args.samples, runs=args.runs)
    (args.out / 'metadata.json').write_text(json.dumps(metadata, indent=2) + '\n')
    timings, allocation = {}, {}
    for run in range(args.runs):
        assert hashlib.sha256(binary.read_bytes()).hexdigest() == digest
        print(f'probe process {run+1}/{args.runs}', flush=True)
        with (args.out / f'run-{run}.csv').open('w') as csvout, (args.out / f'run-{run}.log').open('w') as log:
            subprocess.run([str(binary), 'optimization', str(args.samples), str(971911 + run)],
                           env=env, stdout=csvout, stderr=log, check=True)
        assert 'CORRECTNESS_COMPLETE' in (args.out / f'run-{run}.log').read_text()
        with (args.out / f'run-{run}.csv').open() as f:
            for r in csv.DictReader(f):
                key = (r['family'], r['size'], r['variant'])
                timings.setdefault(key, {}).setdefault(run, []).append(float(r['ns']))
                allocation[key] = max(allocation.get(key, (0, 0)), (int(r['allocations']), int(r['allocated_bytes'])))
    results = []
    for key, runs in timings.items():
        family, size, variant = key
        base = timings[(family, size, baselines[family])]
        ratios = [statistics.median(runs[r]) / statistics.median(base[r]) for r in runs]
        row = dict(family=family, size=size, variant=variant, ratio=statistics.median(ratios),
                   run_ratios=ratios, allocations=allocation[key])
        results.append(row)
        if variant != baselines[family]:
            print(f'{family}/{size} {variant}: {row["ratio"]:.3f}, alloc={allocation[key]}', flush=True)
    (args.out / 'ratios.json').write_text(json.dumps(results, indent=2) + '\n')


if __name__ == '__main__':
    main()

#!/usr/bin/env python3
"""Compare allocation-instrumented gkr_capture binaries in fresh processes.

Timings from these runs are intentionally not reported. RSS includes setup;
phase counters measure requested live Rust heap, not resident memory.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--baseline', type=Path, required=True)
    parser.add_argument('--candidate', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--exponents', type=int, nargs='+', default=[7, 10])
    parser.add_argument('--threads', type=int, nargs='+', default=[1, 10])
    args = parser.parse_args()
    args.output.mkdir(parents=True, exist_ok=False)
    binaries = {v: getattr(args, v).resolve(strict=True) for v in ['baseline', 'candidate']}
    (args.output/'manifest.json').write_text(json.dumps({v: dict(path=str(p), sha256=hashlib.sha256(p.read_bytes()).hexdigest()) for v,p in binaries.items()}, indent=2))
    clean = {k:v for k,v in os.environ.items() if not k.startswith(('F2Z_', 'F2_FOREST_', 'RAYON_')) and k != 'HARDWARE_CONCURRENCY'}
    results = []
    for exponent in args.exponents:
        digest = None
        for threads in args.threads:
            for variant, binary in binaries.items():
                name = f'i{exponent}-t{threads}-{variant}'
                directory = (args.output/name).resolve()
                env = dict(clean, F2_FOREST_SCHEDULE='l4', RAYON_NUM_THREADS=str(threads), HARDWARE_CONCURRENCY=str(threads))
                cpus = ','.join(map(str, sorted(os.sched_getaffinity(0))[:threads]))
                log = args.output/(name+'.log')
                with log.open('w') as out:
                    subprocess.run(['/usr/bin/time', '-v', 'taskset', '-c', cpus, str(binary), str(exponent), str(threads), '1', str(directory)], env=env, stdout=out, stderr=out, timeout=300, check=True)
                sample, = json.loads((directory/'samples.json').read_text())
                assert sample['verified']
                digest = digest or sample['proof_digest']
                assert sample['proof_digest'] == digest, 'proof changed'
                allocation = json.loads((directory/'allocations.json').read_text())
                rss = int(next(line.rsplit(':',1)[1] for line in log.read_text().splitlines() if 'Maximum resident set size' in line))
                row = dict(exponent=exponent, threads=threads, variant=variant, proof_digest=digest, max_rss_kib=rss, allocations=allocation)
                results.append(row)
                print(name, 'RSS MiB', round(rss/1024, 2), 'heap peak MiB', round(allocation['peak_rust_live_bytes']/2**20, 2), flush=True)
                (args.output/'summary.json').write_text(json.dumps(results, indent=2))


if __name__ == '__main__':
    main()

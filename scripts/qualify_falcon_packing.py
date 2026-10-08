#!/usr/bin/env python3
"""Paired, tracing-disabled Falcon packing qualification; never rewrites a run.

Use separate output directories for screening, confirmation, and final auto
dispatch. Each process median is one observation (not five independent pairs).
Archived executables may be supplied with --binary label=/absolute/path.
"""
from __future__ import annotations

import argparse
import hashlib
import itertools
import json
import math
import os
from pathlib import Path
import platform
import re
import statistics
import subprocess
import sys
import time

from bench_statistics import paired_interval, sample_statistics

# The current qualification is intentionally restricted to the requested size.
BATCHES = [1024]
BOUNDARIES = [15, 17, 31, 33, 63, 65, 127, 129, 255, 257, 511, 513, 1023]


def integers(value):
    return [int(part) for part in value.split(',')]


def sha(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def cpu_order():
    if sys.platform != 'linux':
        return None
    allowed = os.sched_getaffinity(0)
    rows = subprocess.check_output(['lscpu', '-p=CPU,CORE,SOCKET'], text=True)
    first, siblings, seen = [], [], set()
    for row in rows.splitlines():
        if row.startswith('#'):
            continue
        cpu, core, socket = map(int, row.split(','))
        if cpu not in allowed:
            continue
        key = (socket, core)
        (siblings if key in seen else first).append(cpu)
        seen.add(key)
    return first + siblings


def environment(threads, cache, variant, cpus):
    env = {k: v for k, v in os.environ.items()
           if not k.startswith(('BITZ_', 'FLOCK_', 'CARGO_PROFILE_'))
           and k not in ('RUST_LOG', 'PERFETTO_TRACE', 'CARGO_ENCODED_RUSTFLAGS')}
    env.update(RAYON_NUM_THREADS=str(threads), RUSTFLAGS='-C target-cpu=native',
               BITZ_FALCON_CASE_CACHE=str(cache), NO_COLOR='1')
    if variant in ('original', 'serial-word', 'parallel-word', 'auto'):
        env['BITZ_FALCON_PACKING'] = variant
    if cpus is not None:
        if threads > len(cpus):
            raise ValueError('worker count exceeds available logical CPUs')
        env.update(BITZ_FALCON_WORKER_CPUS=','.join(map(str, cpus[:threads])),
                   BITZ_FALCON_MAIN_CPU=str(cpus[0]))
    return env


def extract(stdout, stderr, mode, samples):
    rows = [json.loads(line) for line in stdout.splitlines() if line.startswith('{')]
    trials = [r for r in rows if r.get('trial') in ('warmup', 'sample')]
    measured = [r for r in trials if r['trial'] == 'sample']
    if len(measured) != samples or len(trials) != samples + 1:
        raise ValueError('wrong number of trials')
    if mode != 'packing' and not all(r.get('verified') for r in trials):
        raise ValueError('proof verification failed')
    keys = ('packing_ms',) if mode == 'packing' else (
        'total_prover_ms', 'witness_commit_ms',
        'proof_prove_ms' if mode == 'full' else 'prove_ms',
        'proof_verify_ms' if mode == 'full' else 'verify_ms')
    medians = {k: statistics.median(r[k] for r in measured) for k in keys}
    if any(not math.isfinite(r[k]) or r[k] <= 0 for r in trials for k in keys):
        raise ValueError('missing or invalid timing')
    if sys.platform == 'darwin':
        match = re.search(r'(\d+)\s+maximum resident set size', stderr)
        rss = int(match[1]) if match else None
    else:
        match = re.search(r'Maximum resident set size \(kbytes\):\s*(\d+)', stderr)
        rss = int(match[1]) * 1024 if match else None
    if rss is None:
        raise ValueError('missing process peak RSS')
    medians['peak_rss_bytes'] = rss
    invariants = ('input_digest', 'packed_digest') if mode == 'packing' else (
        'input_digest', 'proof_payload_bytes', 'proof_payload_breakdown',
        'source_root', 'proof_debug_digest', 'capacity', 'source_bits_per_signature')
    identity = {k: trials[0][k] for k in invariants if k in trials[0]}
    for row in trials:
        if any(row.get(k) != v for k, v in identity.items()):
            raise ValueError('nondeterministic contents within one process')
    return {'medians': medians, 'identity': identity, 'samples': measured,
            'warmup': trials[0], 'verified_proofs': len(trials) if mode != 'packing' else 0}


def hardware():
    if sys.platform == 'darwin':
        keys = ['machdep.cpu.brand_string', 'hw.physicalcpu', 'hw.logicalcpu', 'hw.memsize']
        return {k: subprocess.check_output(['sysctl', '-n', k], text=True).strip() for k in keys}
    return json.loads(subprocess.check_output(['lscpu', '-J'], text=True))


def compare(left, right):
    a, b = left['identity'], right['identity']
    if 'input_digest' not in a or 'input_digest' not in b:
        raise ValueError('missing input identity')
    for key in a.keys() & b.keys():
        if a[key] != b[key]:
            raise ValueError(f'comparison changed {key}')


def summarize(records, variants, blocks, mode):
    cells = {}
    for r in records:
        cells.setdefault(tuple(r['cell']), {}).setdefault(r['variant'], []).append(r)
    summary = []
    for cell, paths in cells.items():
        if any(len(paths.get(v, [])) != blocks for v in variants):
            continue
        left, right = ([r for r in sorted(paths[v], key=lambda x: x['block'])] for v in variants)
        metrics = {}
        for key in left[0]['medians']:
            a = [r['medians'][key] for r in left]
            b = [r['medians'][key] for r in right]
            ratios = [y / x for x, y in zip(a, b)]
            interval = paired_interval(ratios)
            metrics[key] = {'baseline': sample_statistics(a), 'candidate': sample_statistics(b),
                            'ratio': statistics.geometric_mean(ratios), 'interval95': interval,
                            'status': 'improvement' if interval[1] < 1 else
                                      'regression' if interval[0] > 1 else 'inconclusive'}
        primary = 'packing_ms' if mode == 'packing' else 'total_prover_ms'
        summary.append({'cell': cell, 'metrics': metrics,
                        'primary_improves': metrics[primary]['status'] == 'improvement',
                        'apparent_regressions': [k for k, v in metrics.items()
                                                if k != primary and v['status'] == 'regression']})
    return summary


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--executable', type=Path, required=True)
    parser.add_argument('--binary', action='append', default=[])
    parser.add_argument('--out', type=Path, required=True)
    parser.add_argument('--cache', type=Path, required=True)
    parser.add_argument('--mode', choices=['packing', 'e2e', 'full'], required=True)
    parser.add_argument('--variants', default='original,serial-word')
    parser.add_argument('--threads', type=integers, default=[1, 2, 4, 8])
    parser.add_argument('--batches', type=integers, default=BATCHES)
    parser.add_argument('--degrees', type=integers, default=[512, 1024])
    parser.add_argument('--security', type=integers, default=[100, 128])
    parser.add_argument('--boundaries', action='store_true')
    parser.add_argument('--blocks', type=int, default=12)
    parser.add_argument('--samples', type=int, default=5)
    parser.add_argument('--seed', type=int, default=42)
    parser.add_argument('--cells', type=Path, help='JSON list of [degree,batch,workers,security]')
    args = parser.parse_args()
    variants = args.variants.split(',')
    if len(variants) != 2 or len(set(variants)) != 2 or args.blocks < 2:
        parser.error('supply two distinct variants and at least two blocks')
    args.out = args.out.resolve()
    args.out.mkdir(parents=True, exist_ok=False)
    args.cache = args.cache.resolve()
    binaries = {v: args.executable.resolve() for v in variants}
    for item in args.binary:
        label, path = item.split('=', 1)
        if label not in binaries:
            parser.error('binary label must be a variant')
        binaries[label] = Path(path).resolve()
    cpus = cpu_order()
    cells = json.loads(args.cells.read_text()) if args.cells else list(itertools.product(
        args.degrees, sorted(set(args.batches + (BOUNDARIES if args.boundaries else []))),
        args.threads, args.security))
    config = vars(args) | {'cells': cells, 'platform': platform.platform(), 'cpu_order': cpus,
                         'hardware': hardware(),
                         'runner_sha256': sha(__file__),
                         'statistics_sha256': sha(Path(__file__).with_name('bench_statistics.py')),
                         'binaries': {v: {'path': str(p), 'sha256': sha(p)} for v, p in binaries.items()},
                         'started': time.time(), 'complete': False}
    (args.out / 'manifest.json').write_text(json.dumps(config, indent=2, default=str) + '\n')
    records = []
    for cell in cells:
        degree, batch, threads, security = cell
        for block in range(args.blocks):
            paired = []
            for variant in variants[::1 if block % 2 == 0 else -1]:
                name = f'n{degree}-b{batch}-t{threads}-s{security}-r{block:02}-{variant}'
                env = environment(threads, args.cache, variant, cpus)
                command = [str(binaries[variant]), '--degree', str(degree), '--batch', str(batch),
                           '--security', str(security), '--threads', str(threads),
                           '--warmup', '1', '--iterations', str(args.samples), '--seed', str(args.seed)]
                if args.mode == 'packing':
                    reps = max(1, (512 if variant == 'original' else 32768) // batch)
                    command += ['--packing-only', '--packing-repetitions', str(reps)]
                if args.mode == 'full':
                    command = [str(binaries[variant]), '--degree', str(degree), '--batch', str(batch),
                               '--security', str(security), '--threads', str(threads),
                               '--warmup', '1', '--iterations', str(args.samples), '--seed', str(args.seed)]
                timed = ['/usr/bin/time', '-l' if sys.platform == 'darwin' else '-v', *command]
                with (args.out / f'{name}.stdout').open('x') as stdout, (args.out / f'{name}.stderr').open('x') as stderr:
                    completed = subprocess.run(timed, env=env, stdout=stdout, stderr=stderr)
                if completed.returncode:
                    raise RuntimeError(f'{name} exited {completed.returncode}')
                record = extract((args.out / f'{name}.stdout').read_text(),
                                 (args.out / f'{name}.stderr').read_text(), args.mode, args.samples)
                for row in [record['warmup'], *record['samples']]:
                    if (row['degree'], row['batch'], row['threads'], row.get('security_bits', row.get('security_target'))) != tuple(cell):
                        raise ValueError('requested and observed configuration differ')
                    if row.get('auxiliary_pool_threads', threads) != threads:
                        raise ValueError('auxiliary pool size differs')
                record.update(cell=cell, variant=variant, block=block, command=command,
                              environment={k: v for k, v in env.items() if k.startswith(('BITZ_', 'FLOCK_', 'RAYON_', 'RUSTFLAGS'))})
                paired.append(record)
                records.append(record)
                with (args.out / 'runs.jsonl').open('a') as stream:
                    stream.write(json.dumps(record) + '\n')
            compare(*paired)
        print(json.dumps({'cell': cell, 'completed_processes': len(records)}), flush=True)
    for v, p in binaries.items():
        if sha(p) != config['binaries'][v]['sha256']:
            raise ValueError('executable changed during campaign')
    summary = summarize(records, variants, args.blocks, args.mode)
    if len(summary) != len(cells):
        raise ValueError('incomplete campaign')
    (args.out / 'summary.json').write_text(json.dumps(summary, indent=2) + '\n')
    config.update(complete=True, finished=time.time(), verified_proofs=sum(r['verified_proofs'] for r in records))
    (args.out / 'manifest.json').write_text(json.dumps(config, indent=2, default=str) + '\n')


if __name__ == '__main__':
    main()

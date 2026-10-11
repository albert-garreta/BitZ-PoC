#!/usr/bin/env python3
"""Paired Falcon witness-preparation measurements against frozen executables.

Run e2e and diagnostic campaigns into separate new directories. Only e2e
latencies enter the total-prover acceptance gate; diagnostic spans explain
witness cost and are never substituted for uninstrumented process timings.
Each fresh process contributes one median, independently of its sample count.
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
import signal
import statistics
import subprocess
import sys
import time

import bench_gate
from bench_statistics import paired_interval, sample_statistics
from qualify_falcon_packing import cpu_order, hardware, integers, sha


IDENTITY = ('input_digest', 'source_root', 'proof_debug_digest',
            'proof_payload_bytes', 'proof_payload_breakdown', 'capacity')


def extract(stdout, stderr, mode, warmup, samples, witness_span=None):
    rows = [json.loads(line) for line in stdout.splitlines() if line.startswith('{')]
    trials = [row for row in rows if row.get('trial') in ('warmup', 'sample')]
    if [row['trial'] for row in trials] != ['warmup'] * warmup + ['sample'] * samples:
        raise ValueError('wrong trial count or order')
    if not all(row.get('verified') is True for row in trials):
        raise ValueError('proof verification failed')
    identity = {key: trials[0][key] for key in IDENTITY}
    for row in trials:
        if any(row.get(key) != value for key, value in identity.items()):
            raise ValueError('proof or input identity changed within process')
    names = {'witness_commit_ms': 'witness_commit_ms', 'total_prover_ms': 'total_prover_ms',
             'prove_ms': 'proof_prove_ms' if mode == 'full' else 'prove_ms',
             'verify_ms': 'proof_verify_ms' if mode == 'full' else 'verify_ms'}
    medians = {}
    for name, source in names.items():
        values = [row[source] for row in trials]
        if any(not math.isfinite(value) or value <= 0 for value in values):
            raise ValueError(f'invalid {source}')
        medians[name] = statistics.median(values[warmup:])
    match = re.search(r'Maximum resident set size \(kbytes\):\s*(\d+)', stderr)
    if match:
        rss = int(match[1]) * 1024
    else:
        match = re.search(r'(\d+)\s+maximum resident set size', stderr)
        rss = int(match[1]) if match else None
    if rss is None or rss <= 0:
        raise ValueError('missing process peak RSS')
    medians['peak_rss_bytes'] = rss
    if witness_span:
        stages = [json.loads(line) for line in stderr.splitlines() if line.startswith('{')]
        values = [row['elapsed_ms'] for row in stages
                  if row.get('event') == 'stage' and row.get('name') == witness_span]
        if len(values) != warmup + samples:
            raise ValueError(f'missing or ambiguous diagnostic span {witness_span}')
        if any(not math.isfinite(value) or value <= 0 for value in values):
            raise ValueError('invalid witness span timing')
        medians['diagnostic_witness_ms'] = statistics.median(values[warmup:])
    prepared = [row for row in rows if row.get('event') == 'prepared']
    if len(prepared) > 1:
        raise ValueError('multiple prepared configurations')
    return dict(medians=medians, identity=identity, trials=trials,
                prepared=prepared[0] if prepared else {},
                verified_proofs=len(trials))


def validate_configuration(record, cell, seed):
    _, _, threads, _ = cell
    for row in record['trials']:
        observed = (row['degree'], row['batch'], row['threads'],
                    row.get('security_bits', row.get('security_target')))
        if observed != tuple(cell) or row.get('auxiliary_pool_threads', threads) != threads:
            raise ValueError('requested and observed configuration differ')
        if row.get('seed', record['prepared'].get('input_seed')) != seed:
            raise ValueError('requested and observed input seed differ')
        flags = row.get('build_rustflags', record['prepared'].get('build_rustflags'))
        if flags != '-C target-cpu=native':
            raise ValueError('benchmark was not built with the native flags')


def compare(left, right):
    for key in IDENTITY:
        if key not in left['identity'] or key not in right['identity']:
            raise ValueError(f'missing identity {key}')
        if left['identity'][key] != right['identity'][key]:
            raise ValueError(f'comparison changed {key}')


def summarize(records, cells, blocks, diagnostic):
    summary = []
    for cell in cells:
        pair = []
        for variant in ('baseline', 'candidate'):
            group = sorted((row for row in records if row['cell'] == list(cell)
                            and row['variant'] == variant), key=lambda row: row['block'])
            if [row['block'] for row in group] != list(range(blocks)):
                raise ValueError('incomplete or duplicate paired blocks')
            pair.append(group)
        metrics = {}
        for key in pair[0][0]['medians']:
            left, right = ([row['medians'][key] for row in group] for group in pair)
            ratios = [b / a for a, b in zip(left, right)]
            interval = paired_interval(ratios)
            metrics[key] = dict(baseline=sample_statistics(left), candidate=sample_statistics(right),
                                ratio=statistics.geometric_mean(ratios), interval95=interval,
                                status=('improvement' if interval[1] < 1 else
                                        'regression' if interval[0] > 1 else 'inconclusive'))
        summary.append(dict(cell=list(cell), metrics=metrics, diagnostic_only=diagnostic,
                            witness_significantly_improves=(
                                metrics['diagnostic_witness_ms']['interval95'][1] < 1
                                if 'diagnostic_witness_ms' in metrics else None),
                            commit_significantly_improves=(
                                metrics['witness_commit_ms']['interval95'][1] < 1
                                if not diagnostic else None),
                            secondary_regressions=[key for key in ('prove_ms', 'verify_ms', 'peak_rss_bytes')
                                                   if key in metrics and metrics[key]['status'] == 'regression'],
                            total_within_two_percent=(
                                metrics['total_prover_ms']['interval95'][1] <= 1.02
                                if not diagnostic else None)))
    return summary


def source_identity(path):
    path = path.resolve()
    revision = subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=path, text=True).strip()
    files = subprocess.check_output(['git', 'ls-files', '-z', 'Cargo.toml', 'Cargo.lock',
                                    'build.rs', '.cargo', 'src', 'crates', 'vendor', 'field',
                                    'examples', 'benches'], cwd=path).decode().split('\0')
    hashes = {name: sha(path / name) for name in files if name and
              (name.endswith('.rs') or Path(name).name in ('Cargo.toml', 'Cargo.lock', 'config.toml'))}
    diff = subprocess.check_output(['git', 'diff', 'HEAD', '--binary'], cwd=path)
    status = subprocess.check_output(['git', 'status', '--short', '--untracked-files=no'],
                                     cwd=path, text=True)
    return dict(path=str(path), revision=revision, tracked_status=status,
                tracked_diff_sha256=hashlib.sha256(diff).hexdigest(), source_sha256=hashes)


def provenance(path, binary_sha256):
    content = json.loads(path.read_text())
    def contains_hash(value):
        if isinstance(value, dict):
            return any(contains_hash(item) for item in value.values())
        if isinstance(value, list):
            return any(contains_hash(item) for item in value)
        return value == binary_sha256
    if not contains_hash(content):
        raise ValueError(f'build provenance does not identify the executable SHA-256: {path}')
    return dict(path=str(path.resolve()), sha256=sha(path), content=content)


def overrides(items):
    result = {}
    for item in items:
        key, value = item.split('=', 1)
        if not key.startswith(('BITZ_', 'FLOCK_')):
            raise ValueError('variant overrides must start BITZ_ or FLOCK_')
        result[key] = value
    return result


def environment(threads, cache, cpus, extra, diagnostic, mode):
    env = {key: value for key, value in os.environ.items()
           if not key.startswith(('BITZ_', 'FLOCK_', 'RAYON_', 'CARGO_PROFILE_', 'LIG_', 'PCS_'))
           and key not in ('RUST_LOG', 'RUSTFLAGS', 'CARGO_ENCODED_RUSTFLAGS', 'PERFETTO_TRACE')}
    env.update(RAYON_NUM_THREADS=str(threads), BITZ_FALCON_CASE_CACHE=str(cache),
               RUSTFLAGS='-C target-cpu=native', NO_COLOR='1')
    if cpus is not None:
        if threads > len(cpus):
            raise ValueError('requested workers exceed available CPUs')
        env.update(BITZ_FALCON_WORKER_CPUS=','.join(map(str, cpus[:threads])),
                   BITZ_FALCON_MAIN_CPU=str(cpus[0]))
    env.update(extra)
    if diagnostic and mode == 'full':
        env['BITZ_FALCON_STAGE_TIMINGS'] = '1'
    if not diagnostic and any(key in env for key in ('BITZ_FALCON_STAGE_TIMINGS',
                                                    'BITZ_FALCON_TRACE_JSONL', 'FLOCK_COMMIT_TIMING')):
        raise ValueError('diagnostic logging cannot be enabled in an e2e campaign')
    return env


def save(path, value):
    path.write_text(json.dumps(value, indent=2, default=str) + '\n')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for variant in ('baseline', 'candidate'):
        parser.add_argument(f'--{variant}', type=Path, required=True)
        parser.add_argument(f'--{variant}-provenance', type=Path, required=True,
                            help='preserved build manifest identifying executable and source')
        parser.add_argument(f'--{variant}-env', action='append', default=[])
    parser.add_argument('--source', type=Path, default=Path(__file__).resolve().parents[1])
    parser.add_argument('--out', type=Path, required=True)
    parser.add_argument('--cache', type=Path, required=True)
    parser.add_argument('--mode', choices=['arithmetic', 'full'], required=True)
    parser.add_argument('--diagnostic', action='store_true')
    parser.add_argument('--witness-span', help='exact shared span name; arithmetic default is falcon_algebraic:witness')
    parser.add_argument('--threads', type=integers, default=[16])
    parser.add_argument('--degrees', type=integers, default=[512, 1024])
    parser.add_argument('--batch', type=int, default=1024)
    parser.add_argument('--security', type=int, choices=[100, 128], default=100)
    parser.add_argument('--blocks', type=int, default=12)
    parser.add_argument('--warmup', type=int, default=2)
    parser.add_argument('--samples', type=int, default=7)
    parser.add_argument('--seed', type=int, default=42)
    parser.add_argument('--timeout', type=float, default=1800)
    args = parser.parse_args()
    if (args.blocks < 2 or args.warmup < 0 or args.samples < 1 or
            not 1 <= args.batch <= 1024 or any(t < 1 for t in args.threads) or
            any(n not in (512, 1024) for n in args.degrees)):
        parser.error('invalid cell or trial configuration')
    if args.witness_span and not args.diagnostic:
        parser.error('--witness-span requires --diagnostic')
    if args.diagnostic and args.mode == 'arithmetic' and not args.witness_span:
        args.witness_span = 'falcon_algebraic:witness'
    binaries = {v: getattr(args, v).resolve() for v in ('baseline', 'candidate')}
    extra = {v: overrides(getattr(args, v + '_env')) for v in binaries}
    source = source_identity(args.source)
    cells = [(n, args.batch, t, args.security) for n, t in itertools.product(args.degrees, args.threads)]
    cpus = cpu_order()
    manifest = dict(configuration=vars(args), cells=cells, source=source,
                    platform=platform.platform(), hardware=hardware(), cpu_order=cpus,
                    runner_sha256=sha(__file__), statistics_sha256=sha(Path(__file__).with_name('bench_statistics.py')),
                    binaries={v: dict(path=str(p), sha256=sha(p)) for v, p in binaries.items()},
                    provenance={v: provenance(getattr(args, v + '_provenance'), sha(binaries[v]))
                                for v in binaries},
                    started=time.time(), complete=False,
                    acceptance='Significant witness improvement requires separate comparable witness evidence; uninstrumented total ratio upper95 <= 1.02.')
    args.out.mkdir(parents=True, exist_ok=False)
    save(args.out / 'manifest.json', manifest)
    records, references = [], {}
    process = None
    signal.signal(signal.SIGTERM, bench_gate.terminate)
    bench_gate.acquire('falcon-witness-' + args.mode, 5)
    try:
        for cell in cells:
            degree, batch, threads, security = cell
            for block in range(args.blocks):
                bench_gate.wait_idle('falcon-witness', 88, 5, 5, 300)
                paired = []
                for variant in ('baseline', 'candidate')[::1 if block % 2 == 0 else -1]:
                    name = f'n{degree}-b{batch}-t{threads}-s{security}-r{block:02}-{variant}'
                    env = environment(threads, args.cache.resolve(), cpus, extra[variant], args.diagnostic, args.mode)
                    command = [str(binaries[variant]), '--degree', str(degree), '--batch', str(batch),
                               '--security', str(security), '--threads', str(threads), '--warmup', str(args.warmup),
                               '--iterations', str(args.samples), '--seed', str(args.seed)]
                    if args.diagnostic and args.mode == 'arithmetic':
                        command.append('--trace')
                    timed = ['/usr/bin/time', '-l' if sys.platform == 'darwin' else '-v', *command]
                    start, initial_swap = time.monotonic(), bench_gate.swap_used_gb()
                    with (args.out / (name + '.stdout')).open('x') as stdout, (args.out / (name + '.stderr')).open('x') as stderr:
                        process = subprocess.Popen(timed, env=env, stdout=stdout, stderr=stderr, start_new_session=True)
                        while process.poll() is None:
                            if time.monotonic() - start > args.timeout or bench_gate.swap_used_gb() - initial_swap > 2:
                                raise RuntimeError('process timeout or swap growth exceeded limit')
                            time.sleep(1)
                        if process.returncode:
                            raise RuntimeError(f'{name} exited {process.returncode}')
                        process = None
                    record = extract((args.out / (name + '.stdout')).read_text(),
                                     (args.out / (name + '.stderr')).read_text(), args.mode,
                                     args.warmup, args.samples, args.witness_span)
                    validate_configuration(record, cell, args.seed)
                    key = tuple(cell)
                    if key in references:
                        compare(references[key], record)
                    else:
                        references[key] = record
                    record.update(cell=list(cell), variant=variant, block=block, command=timed,
                                  environment={k: v for k, v in env.items() if k.startswith(('BITZ_', 'FLOCK_', 'RAYON_'))})
                    records.append(record)
                    paired.append(record)
                    with (args.out / 'runs.jsonl').open('a') as stream:
                        stream.write(json.dumps(record) + '\n')
                    print(json.dumps(dict(completed=name, medians=record['medians'])), flush=True)
                compare(*paired)
        for variant, binary in binaries.items():
            if sha(binary) != manifest['binaries'][variant]['sha256']:
                raise ValueError('binary changed during campaign')
        if source_identity(args.source) != source:
            raise ValueError('source changed during campaign')
        save(args.out / 'summary.json', summarize(records, cells, args.blocks, args.diagnostic))
        manifest.update(complete=True, verified_proofs=sum(row['verified_proofs'] for row in records))
    except BaseException as error:
        manifest['error'] = str(error)
        raise
    finally:
        if process is not None:
            bench_gate.stop_process_group(process)
        bench_gate.release()
        manifest.update(finished=time.time(), completed_processes=len(records))
        save(args.out / 'manifest.json', manifest)


if __name__ == '__main__':
    main()

#!/usr/bin/env python3
"""Sequential before/after qualification of both aligned Falcon layouts."""
import argparse
import csv
import datetime
import hashlib
import json
import os
from pathlib import Path
import statistics
import subprocess
import time


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def now():
    return datetime.datetime.now(datetime.timezone.utc).isoformat()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--root', type=Path, default=Path('/home/john-wu/code/BitZ-pcs'))
    parser.add_argument('--baseline', type=Path, default=Path('/tmp/falcon-layout-baseline'))
    parser.add_argument('--candidate-algebraic', type=Path, required=True)
    parser.add_argument('--candidate-full', type=Path, required=True)
    parser.add_argument('--out', type=Path, required=True)
    parser.add_argument('--kinds', nargs='+', default=['algebraic', 'full'])
    args = parser.parse_args()
    args.out.mkdir(parents=True, exist_ok=False)
    binaries = {
        'algebraic': {'baseline': args.baseline / 'falcon_algebraic', 'candidate': args.candidate_algebraic},
        'full': {'baseline': args.baseline / 'falcon_hybrid', 'candidate': args.candidate_full},
    }
    metadata = {
        'started_utc': now(), 'baseline_commit': json.loads((args.baseline / 'metadata.json').read_text())['commit'],
        'binaries': {kind: {variant: {'path': str(path), 'sha256': digest(path)} for variant, path in variants.items()} for kind, variants in binaries.items()},
        'batches': [32, 1024], 'targets': [100, 128], 'threads': [1, 2, 4, 8, 16],
        'execution_order': [1, 16, 2, 8, 4], 'warmup': 1, 'measured': 5, 'seed': 42,
        'policy': 'Sequential adjacent baseline/candidate runs; first variant alternates. Native release, physical CPUs, no SMT; main and auxiliary pool CPU0.',
        'timing': 'Existing preparation/prove/verify boundaries retained; stage tracing disabled. Full preparation includes statement clone.',
        'size': 'Canonical stored payload; full root is separate and adds 32 bytes. Source memory excludes codeword and proof buffers.',
        'build': {'profile': 'release', 'features': ['falcon-hybrid'], 'rustflags': '-C target-cpu=native', 'jobs': 4, 'rustc': subprocess.check_output(['rustc', '-Vv'], text=True), 'cargo': subprocess.check_output(['cargo', '-V'], text=True)},
        'preflight_loadavg': Path('/proc/loadavg').read_text().strip(),
        'cpu_model': next(line.split(':', 1)[1].strip() for line in Path('/proc/cpuinfo').read_text().splitlines() if line.startswith('model name')),
        'cpu_topology': subprocess.check_output(['lscpu', '-e=CPU,CORE,SOCKET,ONLINE'], text=True), 'commands': [],
    }
    source_hashes = {str(p.relative_to(args.root)): digest(p) for p in (args.root / 'src').rglob('*.rs')}
    source_hashes.update({str(p.relative_to(args.root)): digest(p) for directory in ['benches', 'examples'] for p in (args.root / directory).rglob('*.rs')})
    source_hashes.update({name: digest(args.root / name) for name in ['Cargo.toml', 'Cargo.lock', 'rust-toolchain.toml'] if (args.root / name).is_file()})
    (args.out / 'candidate-source-sha256.json').write_text(json.dumps(source_hashes, indent=2) + '\n')
    def save():
        (args.out / 'metadata.json').write_text(json.dumps(metadata, indent=2) + '\n')
    save()
    rows = []
    inputs = {}
    verified = 0
    index = 0
    for kind in args.kinds:
        for batch in metadata['batches']:
            for security in metadata['targets']:
                for threads in metadata['execution_order']:
                    cpus = ','.join(map(str, range(threads)))
                    env = os.environ.copy()
                    for key in ['PCS_TRACE', 'FLOCK_COMMIT_TIMING', 'LIGERITO_TRACE', 'LIG_PROVE_TRACE', 'LIG_VERIFY_TRACE', 'BITZ_FALCON_STAGE_TIMINGS']:
                        env.pop(key, None)
                    env.update(RAYON_NUM_THREADS=str(threads), BITZ_FALCON_WORKER_CPUS=cpus, BITZ_FALCON_MAIN_CPU='0', FLOCK_NO_PREFAULT='1', BITZ_FALCON_CASE_CACHE='/tmp/falcon-algebraic-cases-20261007')
                    samples = {}
                    headers = {}
                    variants = ['baseline', 'candidate'] if index % 2 == 0 else ['candidate', 'baseline']
                    for variant in variants:
                        name = f'{kind}-b{batch}-s{security}-t{threads}-{variant}'
                        cmd = ['taskset', '-c', cpus, str(binaries[kind][variant]), '--batch', str(batch), '--security', str(security), '--threads', str(threads), '--warmup', '1', '--iterations', '5', '--seed', '42']
                        if kind == 'full':
                            cmd += ['--degree', '1024', '--k', str(9 if security == 100 else 11)]
                        metadata['commands'].append({'name': name, 'command': cmd, 'environment': {k: v for k, v in env.items() if k.startswith(('BITZ_', 'FLOCK_', 'RAYON_'))}})
                        save()
                        print(f'Starting {name}', flush=True)
                        start = time.monotonic()
                        with (args.out / f'{name}.jsonl').open('w') as stdout, (args.out / f'{name}.stderr').open('w') as stderr:
                            result = subprocess.run(cmd, cwd=args.root, env=env, stdout=stdout, stderr=stderr, timeout=1800)
                        if result.returncode:
                            raise RuntimeError(f'{name} failed; inspect stderr')
                        records = [json.loads(line) for line in (args.out / f'{name}.jsonl').read_text().splitlines()]
                        trials = [r for r in records if kind == 'algebraic' or r.get('event') == 'trial']
                        header = trials[0] if kind == 'algebraic' else next(r for r in records if r.get('event') == 'prepared')
                        assert len(trials) == 6 and [r['trial'] for r in trials] == ['warmup'] + ['sample'] * 5
                        assert header['algebraic_security_bits'] >= security
                        assert header['build_rustflags'] == '-C target-cpu=native'
                        affinity = header['cpu_affinity']
                        assert affinity['observed_worker_cpus'] == [[i] for i in range(threads)]
                        assert affinity['observed_main_cpus'] == [0]
                        assert affinity['observed_auxiliary_worker_cpus'] == [[0]] * threads
                        if variant == 'candidate':
                            assert header['source_layout'] == 'aligned16-v2'
                        for trial in trials:
                            assert trial['threads'] == threads and trial['batch'] == batch
                            assert trial['security_bits' if kind == 'algebraic' else 'security_target'] == security
                            assert trial['input_digest'] == inputs.setdefault(batch, trial['input_digest'])
                            if kind == 'full':
                                assert trial['verified'] is True
                                assert sum(trial['proof_payload_breakdown'].values()) == trial['proof_payload_bytes']
                            else:
                                assert sum(v for _, v in trial['proof_payload_breakdown']) == trial['proof_payload_bytes']
                        if kind == 'algebraic':
                            assert header['source_bits_per_signature'] == (32768 if variant == 'candidate' else 65536)
                        else:
                            assert header['arithmetic_live_bits_per_signature'] == 114914
                            assert header['source_bits_per_signature'] == [131072, 1048576, 262144]
                        verified += len(trials)
                        samples[variant] = trials[1:]
                        headers[variant] = header
                        print(f'Finished {name} in {time.monotonic()-start:.1f}s', flush=True)
                    row = {'kind': kind, 'batch': batch, 'security_bits': security, 'threads': threads}
                    for variant in ['baseline', 'candidate']:
                        measured = samples[variant]
                        mapping = {'preparation_ms': 'witness_commit_ms', 'prove_ms': 'prove_ms' if kind == 'algebraic' else 'proof_prove_ms', 'verify_ms': 'verify_ms' if kind == 'algebraic' else 'proof_verify_ms', 'total_prover_ms': 'total_prover_ms', 'proof_payload_bytes': 'proof_payload_bytes'}
                        for dst, src in mapping.items():
                            row[f'{variant}_{dst}'] = statistics.median(r[src] for r in measured)
                        row[f'{variant}_total_stdev_ms'] = statistics.stdev(r['total_prover_ms'] for r in measured)
                        row[f'{variant}_peak_rss_mib'] = max(r['process_peak_rss_kib'] for r in measured) / 1024
                        source = headers[variant]['source_bits_per_signature']
                        row[f'{variant}_source_bytes'] = (sum(source) if isinstance(source, list) else source) * batch // 8
                    row['total_reduction_percent'] = 100 * (1 - row['candidate_total_prover_ms'] / row['baseline_total_prover_ms'])
                    rows.append(row)
                    metadata['verified_proofs'] = verified
                    save()
                    (args.out / 'summary.json').write_text(json.dumps(rows, indent=2) + '\n')
                    print(json.dumps(row), flush=True)
                    index += 1
    with (args.out / 'summary.csv').open('w') as f:
        writer = csv.DictWriter(f, fieldnames=rows[0].keys())
        writer.writeheader()
        writer.writerows(rows)
    report = ['# Aligned Falcon layouts', '', f'{verified} verified proofs. Medians of five measured trials after one warm-up; matched inputs, both security targets.', '', '| Prover | Batch | Bits | Threads | Total before / after (ms) | Reduction | Verify before / after (ms) | Payload before / after (KiB) |', '|---|---:|---:|---:|---:|---:|---:|---:|']
    for r in sorted(rows, key=lambda x: (x['kind'], x['batch'], x['security_bits'], x['threads'])):
        report.append(f"| {r['kind']} | {r['batch']} | {r['security_bits']} | {r['threads']} | {r['baseline_total_prover_ms']:.2f} / {r['candidate_total_prover_ms']:.2f} | {r['total_reduction_percent']:.1f}% | {r['baseline_verify_ms']:.2f} / {r['candidate_verify_ms']:.2f} | {r['baseline_proof_payload_bytes']/1024:.2f} / {r['candidate_proof_payload_bytes']/1024:.2f} |")
    report += ['', 'The algebraic source slot falls from 65,536 to 32,768 bits. Full arithmetic retains 131,072 bits plus its existing Keccak sources. Protocol layouts and challenges change; proof bytes need not match the baseline. Timings include grinding, whose nonce work can vary across protocol versions. Full payload excludes the 32-byte source root; algebraic payload includes it. The full-prover change includes authenticated padding enforcement and its increased link-grinding budget as well as aligned coefficient binding.', '', 'Detailed phase timings, source bytes, process RSS, raw trials, source hashes, and commands are included alongside this report. Process RSS includes input setup and persistent caches.', '']
    (args.out / 'REPORT.md').write_text('\n'.join(report))
    metadata.update(finished_utc=now(), input_digests=inputs)
    save()


if __name__ == '__main__':
    main()

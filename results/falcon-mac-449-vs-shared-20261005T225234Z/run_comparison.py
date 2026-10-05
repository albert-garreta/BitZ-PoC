#!/usr/bin/env python3
"""Local, serial Falcon comparison; retain exact binaries and every raw result."""
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import statistics
import subprocess
import sys
import time

OUT = Path(__file__).resolve().parent
META = json.loads((OUT / 'manifest.json').read_text())


def save_meta():
    (OUT / 'manifest.json').write_text(json.dumps(META, indent=2) + '\n')


def environment(label):
    env = dict(os.environ)
    for key in list(env):
        if key.startswith(('BITZ_', 'FLOCK_', 'CARGO_PROFILE_')) or key in (
            'RUSTFLAGS', 'CARGO_ENCODED_RUSTFLAGS', 'CARGO_BUILD_TARGET',
            'CARGO_TARGET_DIR', 'RAYON_NUM_THREADS', 'PERFETTO_TRACE',
        ):
            env.pop(key, None)
    env['RUSTFLAGS'] = META['rustflags']
    env['CARGO_TARGET_DIR'] = META[label].get('target_dir', META['target_dir'])
    return env


def digest(path):
    with open(path, 'rb') as f:
        return hashlib.file_digest(f, 'sha256').hexdigest()


def build(labels=('baseline', 'candidate')):
    for label in labels:
        item = META[label]
        actual = subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=item['repo'], text=True).strip()
        assert actual == item['revision'], (label, actual)
        cmd = ['cargo', 'bench', '--offline', '--locked', '--profile', 'release',
               '--features', META['features'], '--bench', 'falcon_hybrid', '--no-run',
               '--message-format=json-render-diagnostics']
        item['build_command'] = cmd
        start = time.monotonic()
        print(f'BUILD {label} {actual}', flush=True)
        with (OUT / f'{label}.build.jsonl').open('w') as stdout, (OUT / f'{label}.build.stderr').open('w') as stderr:
            p = subprocess.run(cmd, cwd=item['repo'], env=environment(label), stdout=stdout, stderr=stderr)
        item['build_seconds'] = time.monotonic() - start
        item['build_exit'] = p.returncode
        save_meta()
        if p.returncode:
            print((OUT / f'{label}.build.stderr').read_text()[-6000:], flush=True)
            raise SystemExit(p.returncode)
        executables = []
        for line in (OUT / f'{label}.build.jsonl').read_text().splitlines():
            row = json.loads(line)
            if row.get('reason') == 'compiler-artifact' and row.get('target', {}).get('name') == 'falcon_hybrid' and row.get('executable'):
                executables.append(row['executable'])
        assert len(executables) == 1, executables
        binary = OUT / f'falcon_hybrid_{label}_{actual[:12]}'
        shutil.copy2(executables[0], binary)
        item['binary'] = str(binary)
        item['sha256'] = digest(binary)
        help_text = subprocess.check_output([str(binary), '--help'], text=True)
        assert ('--protocol' in help_text) == (label == 'candidate'), help_text
        item['help'] = help_text
        if label == 'candidate':
            assert item['sha256'] != META['baseline']['sha256']
        save_meta()
        print(f'BUILT {label} seconds={item["build_seconds"]:.1f} sha256={item["sha256"]}', flush=True)


def run(label, security, batch, threads, seed, series, order):
    item = META[label]
    assert digest(item['binary']) == item['sha256']
    name = f'{series}-s{security}-b{batch}-t{threads}-seed{seed}-{order}-{label}'
    assert not (OUT / f'{name}.jsonl').exists(), name
    cmd = ['/usr/bin/time', '-l', item['binary'], '--security', str(security),
           '--batch', str(batch), '--threads', str(threads), '--seed', str(seed),
           '--warmup', str(META['warmups']), '--iterations', str(META['iterations'])]
    if label == 'candidate':
        cmd += ['--protocol', 'shared-prime']
    record = dict(name=name, label=label, security=security, batch=batch, threads=threads,
                  seed=seed, series=series, order=order, command=cmd, started=time.time())
    print(f'RUN {name}', flush=True)
    start = time.monotonic()
    with (OUT / f'{name}.jsonl').open('w') as stdout, (OUT / f'{name}.stderr').open('w') as stderr:
        p = subprocess.run(cmd, cwd=item['repo'], env=environment(label), stdout=stdout, stderr=stderr)
    record.update(exit=p.returncode, elapsed_seconds=time.monotonic()-start)
    stderr_text = (OUT / f'{name}.stderr').read_text()
    match = re.search(r'^\s*(\d+)\s+maximum resident set size\s*$', stderr_text, re.M)
    record['peak_rss_bytes'] = int(match.group(1)) if match else None
    (OUT / f'{name}.run.json').write_text(json.dumps(record, indent=2)+'\n')
    if p.returncode:
        print(stderr_text[-4000:], flush=True)
        raise SystemExit(p.returncode)
    rows = [json.loads(line) for line in (OUT / f'{name}.jsonl').read_text().splitlines() if line.startswith('{')]
    prepared = [r for r in rows if r.get('event') == 'prepared']
    trials = [r for r in rows if r.get('event') == 'trial']
    samples = [r for r in trials if r['trial'] == 'sample']
    assert len(prepared) == 1 and len(samples) == META['iterations']
    assert len(trials) == META['iterations'] + META['warmups']
    assert all(r['verified'] for r in trials)
    assert len({r['input_digest'] for r in trials}) == 1
    assert len({r['proof_debug_digest'] for r in trials}) == 1
    assert len({r['proof_payload_bytes'] for r in trials}) == 1
    assert record['peak_rss_bytes'] is not None
    record['prepared'] = prepared[0]
    record['trials'] = trials
    record['medians'] = {key: statistics.median(r[key] for r in samples) for key in (
        'witness_commit_ms', 'proof_prove_ms', 'total_prover_ms', 'proof_verify_ms', 'native_verify_ms', 'end_to_end_ms')}
    record['payload_bytes'] = samples[0]['proof_payload_bytes']
    (OUT / f'{name}.run.json').write_text(json.dumps(record, indent=2)+'\n')
    print(f'DONE {name} total={record["medians"]["total_prover_ms"]:.3f}ms verify={record["medians"]["proof_verify_ms"]:.3f}ms payload={record["payload_bytes"]} rss={record["peak_rss_bytes"]/2**20:.1f}MiB', flush=True)
    return record


def matrix():
    index = 0
    for security in META['securities']:
        for threads in META['threads']:
            for batch in META['batches']:
                labels = ('baseline', 'candidate') if index % 2 == 0 else ('candidate', 'baseline')
                pair = [run(label, security, batch, threads, 42, 'matrix', 'AB' if index % 2 == 0 else 'BA') for label in labels]
                assert pair[0]['prepared']['input_digest'] == pair[1]['prepared']['input_digest']
                assert pair[0]['prepared']['security_target'] == pair[1]['prepared']['security_target']
                index += 1
    print('MATRIX COMPLETE: 192 verified proofs including warmups', flush=True)


def confirmation():
    # Reverse the matrix's B1024 ordering for seed42; include two new input seeds.
    for security in META['securities']:
        for seed in (42, 43, 44):
            labels = ('baseline', 'candidate') if seed % 2 == 0 else ('candidate', 'baseline')
            pair = [run(label, security, 1024, 8, seed, 'confirm', 'AB' if seed % 2 == 0 else 'BA') for label in labels]
            assert pair[0]['prepared']['input_digest'] == pair[1]['prepared']['input_digest']
    print('CONFIRMATION COMPLETE: 72 verified proofs including warmups', flush=True)


if __name__ == '__main__':
    {'build': build, 'build-candidate': lambda: build(('candidate',)), 'matrix': matrix, 'confirmation': confirmation}[sys.argv[1]]()

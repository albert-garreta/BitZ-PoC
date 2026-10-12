#!/usr/bin/env python3
"""Compare verified Falcon proofs on one idle core; retain raw timing and RSS."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import statistics
import subprocess
import time

ROOT = Path(__file__).resolve().parents[2]
OUT = Path(__file__).resolve().parent
BASE = ROOT / 'results/falcon-single-core-128-20261008'
CPU, SIBLING = 6, 22


def save(path, value):
    path.write_text(json.dumps(value, indent=2) + '\n')


def cpu_stats():
    return {int(p[0][3:]): list(map(int, p[1:9]))
            for line in Path('/proc/stat').read_text().splitlines()
            if (p := line.split())[0].startswith('cpu') and p[0] != 'cpu'}


def busy(a, b, cpu):
    delta = [y - x for x, y in zip(a[cpu], b[cpu])]
    return 100 * (sum(delta) - delta[3] - delta[4]) / max(1, sum(delta))


def idle_check():
    for _ in range(30):
        a = cpu_stats()
        time.sleep(5)
        b = cpu_stats()
        result = {'cpu': CPU, 'sibling': SIBLING,
                  'cpu_busy_pct': busy(a, b, CPU), 'sibling_busy_pct': busy(a, b, SIBLING)}
        if max(result['cpu_busy_pct'], result['sibling_busy_pct']) < 2:
            return result
        print('Waiting for idle core: ' + json.dumps(result), flush=True)
    raise RuntimeError('Measured core did not become idle')


def rows(path):
    return [json.loads(line) for line in path.read_text().splitlines() if line.startswith('{')]


def source_hashes():
    paths = subprocess.check_output(
        ['git', 'ls-files', '--cached', '--others', '--exclude-standard', '-z',
         'src', 'vendor', 'crates', 'benches', 'examples', 'Cargo.toml', 'Cargo.lock'], cwd=ROOT).split(b'\0')
    return {os.fsdecode(p): hashlib.sha256((ROOT / os.fsdecode(p)).read_bytes()).hexdigest()
            for p in paths if p and (ROOT / os.fsdecode(p)).is_file()}


def run(label, kind, degree, executable, overrides, samples, trace):
    stem = f'{label}-{kind}-{degree}'
    folder = OUT / ARGS.suite
    folder.mkdir(exist_ok=True)
    idle = idle_check()
    settings = {'RAYON_NUM_THREADS': '1', 'OMP_NUM_THREADS': '1',
                'BITZ_FALCON_MAIN_CPU': str(CPU), 'BITZ_FALCON_WORKER_CPUS': str(CPU),
                'BITZ_FALCON_CASE_CACHE': '/tmp/falcon-algebraic-cases-20261007', **overrides}
    if trace and kind == 'full':
        settings['BITZ_FALCON_STAGE_TIMINGS'] = '1'
    env = {k: v for k, v in os.environ.items()
           if not k.startswith(('BITZ_', 'RAYON_', 'RUSTFLAGS', 'CARGO_ENCODED_RUSTFLAGS'))}
    env.update(settings)
    command = ['taskset', '-c', str(CPU), str(executable), '--degree', str(degree),
               '--batch', '1024', '--security', '128', '--threads', '1',
               '--seed', '42', '--warmup', '1', '--iterations', str(samples)]
    if trace and kind == 'arithmetic':
        command.append('--trace')
    save(folder / (stem + '.command.json'), {'argv': command, 'environment': settings, 'idle': idle})
    print('Starting ' + stem, flush=True)
    before, start = cpu_stats(), time.monotonic()
    observations, violations = [], []
    with (folder / (stem + '.jsonl')).open('w') as output, (folder / (stem + '.stderr')).open('w') as errors:
        process = subprocess.Popen(command, cwd=ROOT, env=env, stdout=output, stderr=errors)
        while True:
            pid, status, usage = os.wait4(process.pid, os.WNOHANG)
            if pid:
                process.returncode = os.waitstatus_to_exitcode(status)
                break
            time.sleep(0.5)
            tasks = []
            for path in Path(f'/proc/{process.pid}/task').glob('*/status'):
                try:
                    fields = dict(line.split(':', 1) for line in path.read_text().splitlines() if ':' in line)
                    task = {'tid': int(path.parent.name), 'cpus': fields['Cpus_allowed_list'].strip()}
                    tasks.append(task)
                    if task['cpus'] != str(CPU):
                        violations.append(task)
                except (FileNotFoundError, ProcessLookupError):
                    pass
            observations.append({'elapsed_s': time.monotonic() - start, 'tasks': tasks})
    after, elapsed = cpu_stats(), time.monotonic() - start
    resources = {'exit_code': process.returncode, 'elapsed_s': elapsed,
                 'peak_rss_kib': usage.ru_maxrss,
                 'user_s': usage.ru_utime, 'system_s': usage.ru_stime,
                 'sibling_busy_pct': busy(before, after, SIBLING),
                 'affinity_violations': violations}
    save(folder / (stem + '.resources.json'), resources)
    save(folder / (stem + '.threads.json'), observations)
    assert process.returncode == 0, (stem, (folder / (stem + '.stderr')).read_text()[-8000:])
    assert not violations, violations
    data = rows(folder / (stem + '.jsonl'))
    trials = [r for r in data if r.get('trial') in ('sample', 'warmup')]
    measured = [r for r in trials if r['trial'] == 'sample']
    assert len(measured) == samples and len(trials) == samples + 1
    assert all(r['verified'] and r['threads'] == 1 for r in trials)
    security = next(r['algebraic_security_bits'] for r in data if 'algebraic_security_bits' in r)
    assert security >= 128
    original = next(r for r in rows(BASE / f'{kind}-{degree}.jsonl') if r.get('trial') == 'sample')
    for r in trials:
        assert r.get('security_target', r.get('security_bits')) == 128
        for key in ('input_digest', 'proof_payload_bytes', 'proof_debug_digest'):
            assert r[key] == original[key], (stem, key, r[key], original[key])
    result = {'label': label, 'variant': kind, 'degree': degree, 'samples': samples,
              'trace': trace, 'security_bits': security, 'settings': overrides,
              'peak_rss_mib': usage.ru_maxrss / 1024, 'sibling_busy_pct': resources['sibling_busy_pct'],
              'proof_bytes': measured[0]['proof_payload_bytes'] + (32 if kind == 'full' else 0),
              'input_digest': original['input_digest'], 'proof_digest': original['proof_debug_digest']}
    keys = {'commit': 'witness_commit_ms', 'prover': 'proof_prove_ms' if kind == 'full' else 'prove_ms',
            'commit_plus_prover': 'total_prover_ms', 'verifier': 'proof_verify_ms' if kind == 'full' else 'verify_ms'}
    for name, key in keys.items():
        values = [r[key] for r in measured]
        result[name + '_ms'] = statistics.median(values)
        result[name + '_range_ms'] = [min(values), max(values)]
    if trace:
        totals, trial_stages = {}, []
        root = 'falcon_algebraic:prove' if kind == 'arithmetic' else 'falcon_bench:prove'
        for row in rows(folder / (stem + '.stderr')):
            if row.get('event') != 'stage' or row['path'][0] != root:
                continue
            path = '/'.join(row['path'])
            totals[path] = totals.get(path, 0) + row['elapsed_ms']
            if row['name'] == root:
                trial_stages.append(totals)
                totals = {}
        assert len(trial_stages) == samples + 1
        result['stage_medians_ms'] = {
            path: statistics.median(r.get(path, 0) for r in trial_stages[1:])
            for path in set().union(*(r.keys() for r in trial_stages[1:]))}
    print('Finished ' + stem + ': ' + json.dumps({k: v for k, v in result.items() if k != 'stage_medians_ms'}), flush=True)
    return result


def main():
    os.sched_setaffinity(0, {8})
    folder = OUT / ARGS.suite
    folder.mkdir(exist_ok=True)
    sources_before = source_hashes()
    save(folder / 'sources.json', sources_before)
    patch = subprocess.check_output(['git', 'diff'], cwd=ROOT, text=True)
    new_files = subprocess.check_output(
        ['git', 'ls-files', '--others', '--exclude-standard', '-z', 'src', 'vendor', 'crates'], cwd=ROOT).split(b'\0')
    for path in new_files:
        if path:
            diff = subprocess.run(['git', 'diff', '--no-index', '--', '/dev/null', os.fsdecode(path)],
                                  cwd=ROOT, text=True, capture_output=True)
            assert diff.returncode in (0, 1), diff.stderr
            patch += diff.stdout
    (folder / 'changes.patch').write_text(patch)
    binaries = {}
    artifacts = rows(Path(ARGS.artifacts))
    binary_dir = Path('/tmp/falcon-sumcheck-opt-binaries') / ARGS.suite
    binary_dir.mkdir(parents=True, exist_ok=True)
    for kind, name in [('arithmetic', 'falcon_algebraic'), ('full', 'falcon_hybrid')]:
        source = next(Path(r['executable']) for r in reversed(artifacts)
                      if r.get('reason') == 'compiler-artifact' and r['target']['name'] == name and r.get('executable'))
        target = binary_dir / name
        shutil.copy2(source, target)
        binaries[kind] = target
    save(folder / 'binaries.json', {k: {'path': str(p), 'sha256': hashlib.sha256(p.read_bytes()).hexdigest()}
                                   for k, p in binaries.items()})
    results = json.loads((folder / 'summary.json').read_text()) if ARGS.resume else []
    for degree in ARGS.degrees:
        for kind in ARGS.variants:
            if ARGS.suite == 'binding':
                configurations = [(f'p{p}-{mode}', {'BITZ_FALCON_BINDING_PREFIX': str(p),
                                    'BITZ_FALCON_BINDING_KERNELS': mode, 'BITZ_FALCON_INTEGER_PREFIX': '1'})
                                   for p in [3, 4] for mode in ['reference', 'optimized']] if kind == 'arithmetic' else [
                    ('reference', {'BITZ_FALCON_BINDING_KERNELS': 'reference', 'BITZ_FALCON_INTEGER_PREFIX': '1'})] + [
                    (f'group{g}', {'BITZ_FALCON_BINDING_KERNELS': 'optimized', 'BITZ_FALCON_BINDING_GROUP': str(g),
                                  'BITZ_FALCON_INTEGER_PREFIX': '1'}) for g in [1, 8, 16]]
            elif ARGS.suite == 'integer':
                configurations = [(f'prefix{p}', {'BITZ_FALCON_INTEGER_PREFIX': str(p)}) for p in [1, 2, 3, 4]]
            elif ARGS.suite == 'layout':
                configurations = [(f'{construction}-{evaluation}', {
                    'BITZ_FALCON_BINDING_KERNELS': 'optimized', 'BITZ_FALCON_BINDING_PREFIX': '4',
                    'BITZ_FALCON_INTEGER_PREFIX': '1',
                    'BITZ_FALCON_BINDING_CONSTRUCTION': construction,
                    'BITZ_FALCON_BINDING_EVALUATION': evaluation})
                    for construction, evaluation in [('factored', 'legacy'), ('factored', 'batched')]]
            else:
                configurations = [('baseline', {}), ('candidate', {})]
            for label, settings in configurations:
                if any(r['label'] == label and r['variant'] == kind and r['degree'] == degree for r in results):
                    continue
                executable = (Path('/tmp/falcon-single-core-128-20261008-binaries') / binaries[kind].name
                              if label == 'baseline' else binaries[kind])
                results.append(run(label, kind, degree, executable, settings, ARGS.samples, ARGS.trace))
                save(folder / 'summary.json', results)
    unchanged = source_hashes() == sources_before
    save(folder / 'validation.json', {'sources_unchanged_during_run': unchanged,
         'all_proofs_verified_and_matched_baseline_digest': True, 'all_security_ledgers_at_least_128': True,
         'all_observed_threads_on_one_logical_cpu': True})
    if ARGS.suite in ('final', 'profile'):
        assert unchanged, 'Sources changed during final measurement'


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--suite', choices=['binding', 'layout', 'integer', 'final', 'profile'], required=True)
    parser.add_argument('--artifacts', required=True)
    parser.add_argument('--samples', type=int, default=3)
    parser.add_argument('--trace', action='store_true')
    parser.add_argument('--resume', action='store_true')
    parser.add_argument('--degrees', type=int, nargs='+', default=[512, 1024])
    parser.add_argument('--variants', nargs='+', default=['arithmetic', 'full'])
    ARGS = parser.parse_args()
    main()

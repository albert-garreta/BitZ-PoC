#!/usr/bin/env python3
"""Run four verified Falcon benchmarks sequentially on one idle logical CPU."""
import csv
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
BIN = OUT / 'binaries'
CPU, SIBLING = 6, 22


def save(name, value):
    (OUT / name).write_text(json.dumps(value, indent=2) + '\n')


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def cpu_stats():
    return {int(p[0][3:]): list(map(int, p[1:9]))
            for line in Path('/proc/stat').read_text().splitlines()
            if (p := line.split())[0].startswith('cpu') and p[0] != 'cpu'}


def busy(a, b, cpu):
    delta = [y - x for x, y in zip(a[cpu], b[cpu])]
    return 100 * (sum(delta) - delta[3] - delta[4]) / sum(delta)


def idle_check():
    for _ in range(12):
        a = cpu_stats()
        time.sleep(5)
        b = cpu_stats()
        result = {'cpu': CPU, 'sibling': SIBLING,
                  'cpu_busy_pct': busy(a, b, CPU),
                  'sibling_busy_pct': busy(a, b, SIBLING), 'seconds': 5}
        if max(result['cpu_busy_pct'], result['sibling_busy_pct']) < 2:
            return result
        print('Waiting for idle core: ' + json.dumps(result), flush=True)
    raise RuntimeError('Selected physical core did not become idle')


def source_hashes():
    paths = subprocess.check_output(['git', 'ls-files', '-z'], cwd=ROOT).split(b'\0')
    selected = [os.fsdecode(p) for p in paths if p and
                (os.fsdecode(p).startswith(('src/', 'benches/', 'examples/', 'crates/', 'vendor/'))
                 or os.fsdecode(p) in ('Cargo.toml', 'Cargo.lock', 'rust-toolchain.toml'))]
    return {p: sha(ROOT / p) for p in selected if (ROOT / p).is_file()}


def run(kind, degree, executable):
    stem = f'{kind}-{degree}'
    idle = idle_check()
    env = {k: v for k, v in os.environ.items()
           if not k.startswith(('BITZ_', 'RAYON_', 'RUSTFLAGS', 'CARGO_ENCODED_RUSTFLAGS'))}
    settings = {'RAYON_NUM_THREADS': '1', 'OMP_NUM_THREADS': '1',
                'BITZ_FALCON_MAIN_CPU': str(CPU), 'BITZ_FALCON_WORKER_CPUS': str(CPU),
                'BITZ_FALCON_CASE_CACHE': '/tmp/falcon-algebraic-cases-20261007'}
    env.update(settings)
    command = ['taskset', '-c', str(CPU), str(executable), '--degree', str(degree),
               '--batch', '1024', '--security', '128', '--threads', '1',
               '--seed', '42', '--warmup', '1', '--iterations', '5']
    save(stem + '.command.json', {'argv': command, 'environment': settings, 'idle_check': idle})
    print('Starting ' + stem + ': ' + json.dumps(idle), flush=True)
    before, start = cpu_stats(), time.monotonic()
    observations, violations, last_trial = [], [], -1
    with (OUT / (stem + '.jsonl')).open('w') as output, (OUT / (stem + '.stderr')).open('w') as errors:
        process = subprocess.Popen(command, cwd=ROOT, env=env, stdout=output, stderr=errors)
        while True:
            pid, status, usage = os.wait4(process.pid, os.WNOHANG)
            if pid:
                process.returncode = os.waitstatus_to_exitcode(status)
                break
            time.sleep(1)
            tasks = []
            for path in Path(f'/proc/{process.pid}/task').glob('*/status'):
                try:
                    fields = dict(line.split(':', 1) for line in path.read_text().splitlines() if ':' in line)
                    mask = fields['Cpus_allowed_list'].strip()
                    tasks.append({'tid': int(path.parent.name), 'name': fields['Name'].strip(), 'cpus': mask})
                    if mask != str(CPU):
                        violations.append(tasks[-1])
                except (FileNotFoundError, ProcessLookupError):
                    pass
            observations.append({'elapsed_s': time.monotonic() - start, 'tasks': tasks})
            rows = [json.loads(line) for line in (OUT / (stem + '.jsonl')).read_text().splitlines() if line.startswith('{')]
            trials = [r for r in rows if r.get('trial') in ('sample', 'warmup')]
            if len(trials) != last_trial:
                last_trial = len(trials)
                print(f'{stem}: {last_trial}/6 verified trials emitted', flush=True)
    elapsed, after = time.monotonic() - start, cpu_stats()
    metrics = {'exit_code': process.returncode, 'elapsed_s': elapsed,
               'user_cpu_s': usage.ru_utime, 'system_cpu_s': usage.ru_stime,
               'cpu_utilization_pct': 100 * (usage.ru_utime + usage.ru_stime) / elapsed,
               'process_peak_rss_kib': usage.ru_maxrss,
               'major_page_faults': usage.ru_majflt,
               'minor_page_faults': usage.ru_minflt,
               'voluntary_context_switches': usage.ru_nvcsw,
               'involuntary_context_switches': usage.ru_nivcsw,
               'sibling_busy_pct': busy(before, after, SIBLING),
               'selected_cpu_busy_pct': busy(before, after, CPU),
               'affinity_violations': violations, 'observation_count': len(observations)}
    save(stem + '.resources.json', metrics)
    save(stem + '.thread-observations.json', observations)
    assert process.returncode == 0, (stem, (OUT / (stem + '.stderr')).read_text())
    assert not violations, violations
    assert metrics['sibling_busy_pct'] < 2, 'SMT sibling was not sufficiently idle'
    rows = [json.loads(line) for line in (OUT / (stem + '.jsonl')).read_text().splitlines() if line.startswith('{')]
    samples = [r for r in rows if r.get('trial') == 'sample']
    assert len(samples) == 5 and all(r['verified'] and r['threads'] == 1 for r in samples)
    security = next(r['algebraic_security_bits'] for r in rows if 'algebraic_security_bits' in r)
    assert security >= 128
    assert all(r.get('security_target', r.get('security_bits')) == 128 for r in samples)
    assert len({r['proof_payload_bytes'] for r in samples}) == 1
    assert len({r['proof_debug_digest'] for r in samples}) == 1
    result = {'variant': kind, 'degree': degree, 'batch': 1024, 'threads': 1,
              'security_target_bits': 128, 'algebraic_security_bits': security,
              'samples': 5, 'input_digest': samples[0]['input_digest'],
              'payload_bytes': samples[0]['proof_payload_bytes'],
              'proof_including_root_bytes': samples[0]['proof_payload_bytes'] + (32 if kind == 'full' else 0),
              'peak_rss_mib': metrics['process_peak_rss_kib'] / 1024,
              'sibling_busy_pct': metrics['sibling_busy_pct']}
    keys = {'commit': 'witness_commit_ms', 'prover': 'proof_prove_ms' if kind == 'full' else 'prove_ms',
            'commit_plus_prover': 'total_prover_ms', 'verifier': 'proof_verify_ms' if kind == 'full' else 'verify_ms'}
    for label, key in keys.items():
        values = [r[key] for r in samples]
        for stat, function in [('median', statistics.median), ('min', min), ('max', max)]:
            result[f'{label}_{stat}_ms'] = function(values)
    result['peak_prover_signatures_per_second'] = 1024_000 / result['prover_min_ms']
    result['peak_commit_plus_prover_signatures_per_second'] = 1024_000 / result['commit_plus_prover_min_ms']
    print('Finished ' + stem + ': ' + json.dumps(result), flush=True)
    return result


def main():
    os.sched_setaffinity(0, {8})  # Keep monitoring off the measured physical core.
    BIN.mkdir(exist_ok=True)
    artifacts = [json.loads(line) for line in (OUT / 'build.jsonl').read_text().splitlines() if line.startswith('{')]
    executables = {}
    for name in ('falcon_algebraic', 'falcon_hybrid'):
        path = next(Path(r['executable']) for r in reversed(artifacts)
                    if r.get('reason') == 'compiler-artifact' and r['target']['name'] == name and r.get('executable'))
        shutil.copy2(path, BIN / name)
        executables[name] = BIN / name
    source_before = source_hashes()
    assert source_before == json.loads((OUT / 'sources-before-build.json').read_text()), 'Source changed during build'
    save('sources.json', source_before)
    save('provenance.json', {'git_head': subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=ROOT, text=True).strip(),
                             'binaries': {name: {'path': str(path), 'sha256': sha(path)} for name, path in executables.items()},
                             'rustflags': '-C target-cpu=native', 'cpu': CPU, 'sibling': SIBLING,
                             'governor': Path(f'/sys/devices/system/cpu/cpu{CPU}/cpufreq/scaling_governor').read_text().strip(),
                             'timestamp_utc': time.strftime('%Y-%m-%dT%H:%M:%SZ', time.gmtime())})
    (OUT / 'lscpu.txt').write_text(subprocess.check_output(['lscpu'], text=True))
    results = []
    for degree in (512, 1024):
        for kind, name in [('arithmetic', 'falcon_algebraic'), ('full', 'falcon_hybrid')]:
            results.append(run(kind, degree, executables[name]))
            save('summary.json', results)
        assert results[-1]['input_digest'] == results[-2]['input_digest']
    assert source_hashes() == source_before, 'Source changed during measurement'
    with (OUT / 'summary.csv').open('w') as file:
        writer = csv.DictWriter(file, fieldnames=list(results[0]))
        writer.writeheader()
        writer.writerows(results)
    save('validation.json', {'sources_unchanged': True, 'all_24_proofs_verified': True,
                             'matched_inputs_between_variants': True, 'all_threads_pinned': True,
                             'all_security_ledgers_at_least_128_bits': True})


if __name__ == '__main__':
    main()

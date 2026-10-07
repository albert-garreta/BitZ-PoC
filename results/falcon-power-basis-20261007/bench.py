import argparse, hashlib, json, os, statistics, subprocess
from pathlib import Path

p = argparse.ArgumentParser()
p.add_argument('--repo', type=Path, default=Path.cwd())
p.add_argument('--binary-dir', type=Path, required=True)
p.add_argument('--out', type=Path, required=True)
p.add_argument('--kinds', nargs='+', default=['algebraic', 'full'])
p.add_argument('--degrees', nargs='+', type=int, default=[512, 1024])
p.add_argument('--legacy-algebraic', action='store_true')
p.add_argument('--batch', type=int, default=1024)
a = p.parse_args()
root = a.repo.resolve()
a.out.mkdir(parents=True, exist_ok=False)
metadata = {'commit': subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=root, text=True).strip(),
            'batch': a.batch, 'warmup': 1, 'samples': 5, 'seed': 42, 'commands': [], 'binaries': {}}
for kind in a.kinds:
    path = a.binary_dir / ('falcon_algebraic' if kind == 'algebraic' else 'falcon_hybrid')
    metadata['binaries'][kind] = {'path': str(path), 'sha256': hashlib.sha256(path.read_bytes()).hexdigest()}
metadata['cpu'] = next(line.split(':', 1)[1].strip() for line in Path('/proc/cpuinfo').read_text().splitlines() if line.startswith('model name'))
rows, digests = [], {}
for kind in a.kinds:
    for degree in a.degrees:
        if kind == 'algebraic' and a.legacy_algebraic and degree != 1024:
            continue
        for target in [100, 128]:
            for threads in [1, 16, 2, 8, 4]:
                name = f'{kind}-n{degree}-b{a.batch}-s{target}-t{threads}'
                cpus = ','.join(map(str, range(threads)))
                env = os.environ.copy()
                for key in ['PCS_TRACE', 'FLOCK_COMMIT_TIMING', 'LIGERITO_TRACE', 'LIG_PROVE_TRACE', 'LIG_VERIFY_TRACE', 'BITZ_FALCON_STAGE_TIMINGS']:
                    env.pop(key, None)
                env.update(RAYON_NUM_THREADS=str(threads), BITZ_FALCON_WORKER_CPUS=cpus,
                           BITZ_FALCON_MAIN_CPU='0', FLOCK_NO_PREFAULT='1',
                           BITZ_FALCON_CASE_CACHE='/tmp/falcon-algebraic-cases-20261007')
                cmd = ['taskset', '-c', cpus, metadata['binaries'][kind]['path'], '--batch', str(a.batch), '--security', str(target), '--threads', str(threads), '--warmup', '1', '--iterations', '5', '--seed', '42']
                if kind == 'full':
                    cmd += ['--degree', str(degree), '--k', str(9 if target == 100 else 11)]
                elif not a.legacy_algebraic:
                    cmd += ['--degree', str(degree)]
                metadata['commands'].append({'name': name, 'command': cmd, 'env': {k: v for k,v in env.items() if k.startswith(('BITZ_', 'RAYON_', 'FLOCK_'))}})
                (a.out / 'metadata.json').write_text(json.dumps(metadata, indent=2)+'\n')
                print('Starting', name, flush=True)
                with (a.out / (name+'.jsonl')).open('w') as out, (a.out / (name+'.stderr')).open('w') as err:
                    subprocess.run(cmd, cwd=root, env=env, stdout=out, stderr=err, check=True, timeout=1800)
                records = [json.loads(line) for line in (a.out / (name+'.jsonl')).read_text().splitlines()]
                trials = records if kind == 'algebraic' else [r for r in records if r.get('event') == 'trial']
                header = trials[0] if kind == 'algebraic' else next(r for r in records if r.get('event') == 'prepared')
                assert len(trials) == 6 and [r['trial'] for r in trials] == ['warmup'] + ['sample']*5
                assert header['algebraic_security_bits'] >= target
                assert header['build_rustflags'] == '-C target-cpu=native'
                assert header['cpu_affinity']['observed_worker_cpus'] == [[i] for i in range(threads)]
                assert header['cpu_affinity']['observed_main_cpus'] == [0]
                assert header['cpu_affinity']['observed_auxiliary_worker_cpus'] == [[0]]*threads
                for trial in trials:
                    assert trial['batch'] == a.batch and trial['threads'] == threads
                    assert trial['input_digest'] == digests.setdefault(degree, trial['input_digest'])
                    if kind == 'full' or 'verified' in trial: assert trial['verified'] is True
                    if 'degree' in trial: assert trial['degree'] == degree
                measured = trials[1:]
                row = dict(kind=kind, degree=degree, batch=a.batch, target=target, threads=threads, input_digest=trials[0]['input_digest'])
                mapping = {'preparation_ms': 'witness_commit_ms', 'prove_ms': 'prove_ms' if kind == 'algebraic' else 'proof_prove_ms', 'verify_ms': 'verify_ms' if kind == 'algebraic' else 'proof_verify_ms', 'total_prover_ms': 'total_prover_ms', 'proof_payload_bytes': 'proof_payload_bytes'}
                row.update({k: statistics.median(r[v] for r in measured) for k,v in mapping.items()})
                row['peak_rss_mib'] = max(r['process_peak_rss_kib'] for r in measured)/1024
                row['stdev_total_ms'] = statistics.stdev(r['total_prover_ms'] for r in measured)
                rows.append(row)
                (a.out / 'summary.json').write_text(json.dumps(rows, indent=2)+'\n')
                print(json.dumps(row), flush=True)
metadata['verified_proofs'] = 6*len(rows)
(a.out / 'metadata.json').write_text(json.dumps(metadata, indent=2)+'\n')

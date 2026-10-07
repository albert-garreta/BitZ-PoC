"""Interleaved, verified algebraic Falcon baseline/candidate measurements."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import statistics
import subprocess

p = argparse.ArgumentParser()
p.add_argument('--repo', type=Path, required=True)
p.add_argument('--root', type=Path, required=True)
p.add_argument('--out', type=Path, required=True)
a = p.parse_args()
a.out.mkdir(parents=True, exist_ok=False)
metadata = {'batch': 1024, 'warmup_per_process': 1, 'samples_per_process': 5,
            'blocks': 2, 'seed': 42, 'commands': [], 'binaries': {}}
metadata['cpu'] = next(line.split(':', 1)[1].strip() for line in Path('/proc/cpuinfo').read_text().splitlines() if line.startswith('model name'))
for mode in ['baseline', 'candidate']:
    binary = a.root / mode / 'falcon_algebraic'
    metadata['binaries'][mode] = {'path': str(binary), 'sha256': hashlib.sha256(binary.read_bytes()).hexdigest()}
rows = []
invariants = {}
fixed_keys = ['degree', 'batch', 'security_bits', 'input_digest', 'protocol_id',
              'proof_payload_bytes', 'proof_payload_breakdown', 'live_bits_per_signature',
              'source_bits_per_signature', 'capacity', 'source_bits', 'algebraic_security_bits']
for degree in [512, 1024]:
    for security in [100, 128]:
        for threads in [1, 16, 2, 8, 4]:
            samples = {'baseline': [], 'candidate': []}
            cpus = ','.join(map(str, range(threads)))
            env = os.environ.copy()
            for key in ['PCS_TRACE', 'FLOCK_COMMIT_TIMING', 'LIGERITO_TRACE', 'LIG_PROVE_TRACE', 'LIG_VERIFY_TRACE', 'BITZ_FALCON_STAGE_TIMINGS']:
                env.pop(key, None)
            env.update(RAYON_NUM_THREADS=str(threads), BITZ_FALCON_WORKER_CPUS=cpus,
                       BITZ_FALCON_MAIN_CPU='0', FLOCK_NO_PREFAULT='1',
                       BITZ_FALCON_CASE_CACHE='/tmp/falcon-algebraic-cases-20261007')
            for block in range(2):
                for mode in (['baseline', 'candidate'] if block == 0 else ['candidate', 'baseline']):
                    name = f'n{degree}-s{security}-t{threads}-block{block}-{mode}'
                    command = ['taskset', '-c', cpus, metadata['binaries'][mode]['path'],
                               '--degree', str(degree), '--batch', '1024', '--security', str(security),
                               '--threads', str(threads), '--warmup', '1', '--iterations', '5', '--seed', '42']
                    metadata['commands'].append({'name': name, 'command': command,
                        'env': {k: v for k, v in env.items() if k.startswith(('BITZ_', 'RAYON_', 'FLOCK_'))}})
                    (a.out / 'metadata.json').write_text(json.dumps(metadata, indent=2)+'\n')
                    print('Starting', name, flush=True)
                    with (a.out / (name+'.jsonl')).open('w') as out, (a.out / (name+'.stderr')).open('w') as err:
                        subprocess.run(command, cwd=a.repo, env=env, stdout=out, stderr=err, check=True, timeout=1800)
                    trials = [json.loads(line) for line in (a.out / (name+'.jsonl')).read_text().splitlines()]
                    assert len(trials) == 6 and [r['trial'] for r in trials] == ['warmup'] + ['sample']*5
                    for r in trials:
                        assert r['verified'] is True
                        assert r['threads'] == threads
                        assert r['algebraic_security_bits'] >= security
                        assert r['build_rustflags'] == '-C target-cpu=native'
                        assert r['cpu_affinity']['observed_worker_cpus'] == [[i] for i in range(threads)]
                        assert r['cpu_affinity']['observed_main_cpus'] == [0]
                        assert r['cpu_affinity']['observed_auxiliary_worker_cpus'] == [[0]]*threads
                        invariant = {k: r[k] for k in fixed_keys}
                        assert invariant == invariants.setdefault((degree, security), invariant), (name, invariant)
                    samples[mode] += trials[1:]
            for mode, trials in samples.items():
                row = {'degree': degree, 'security': security, 'threads': threads, 'mode': mode, 'samples': len(trials)}
                for key in ['total_prover_ms', 'witness_commit_ms', 'prove_ms', 'verify_ms', 'proof_payload_bytes']:
                    row[key] = statistics.median(r[key] for r in trials)
                row['total_prover_stdev_ms'] = statistics.stdev(r['total_prover_ms'] for r in trials)
                rows.append(row)
                print(json.dumps(row), flush=True)
            (a.out / 'summary.json').write_text(json.dumps(rows, indent=2)+'\n')
metadata['verified_proofs'] = len(metadata['commands'])*6
(a.out / 'metadata.json').write_text(json.dumps(metadata, indent=2)+'\n')

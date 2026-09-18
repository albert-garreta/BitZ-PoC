#!/usr/bin/env python3
"""Interleave original-master and merged SHA+ECDSA workers; preserve every sample."""
import hashlib
import json
import os
from pathlib import Path
import statistics
import subprocess
import time

ROOT = Path('/home/john-wu/code/BitZ-pcs')
OUT = ROOT / 'bench_results/sha256-ecdsa-verifier-merge-20260916/prover-followup'
OUT.mkdir(exist_ok=True)
BUILD = Path('/tmp/bitz-vopt-integration')
BINS = {}
for variant in ['base', 'merged']:
    matches = [p for p in (BUILD / f'{variant}-target/release/deps').glob('sha256_ecdsa_compare-*') if p.is_file() and os.access(p, os.X_OK)]
    assert len(matches) == 1, matches
    BINS[variant] = matches[0]
configs = [('f2z-all', 3, 100, 10, 1)]
metadata = {
    'base_revision': '07680319a4ab73b6d45c3644420755155866cd34',
    'merged_parent': 'b725f2b3c51b12ba7f11d4090a3b8f9ecc75493e',
    'started_utc': time.strftime('%Y-%m-%dT%H:%M:%SZ', time.gmtime()),
    'rustc': subprocess.check_output(['rustc', '-Vv'], text=True),
    'cpu': subprocess.check_output(['lscpu'], text=True),
    'rustflags': '-C target-cpu=native',
    'profile': 'release; fat LTO; codegen-units=1',
    'samples_per_process': 9,
    'passes': 2,
    'harness': 'identical merged benchmark source in both builds; baseline library sources unchanged',
    'binary_sha256': {v: hashlib.sha256(p.read_bytes()).hexdigest() for v,p in BINS.items()},
    'source_sha256': {},
}
for variant, source in [('base', BUILD/'base'), ('merged', ROOT)]:
    tracked = subprocess.check_output(['git', 'ls-files', 'src', 'crates', 'vendor', 'benches', 'Cargo.toml', 'Cargo.lock'], cwd=ROOT, text=True).splitlines()
    metadata['source_sha256'][variant] = {name: hashlib.sha256((source/name).read_bytes()).hexdigest() for name in tracked if (source/name).is_file()}
(OUT/'metadata.json').write_text(json.dumps(metadata, indent=2)+'\n')
all_rows=[]
for pass_no in range(2):
    cases = configs if pass_no == 0 else configs[::-1]
    for mode, exponent, target, threads, rate in cases:
        key=f'{mode}-i{exponent}-l{target}-t{threads}-r{rate}'
        for variant in (['base','merged'] if pass_no == 0 else ['merged','base']):
            env={k:v for k,v in os.environ.items() if not k.startswith(('F2Z_', 'RAYON_', 'PERFETTO_')) and k != 'HARDWARE_CONCURRENCY'}
            env.update(RAYON_NUM_THREADS=str(threads), HARDWARE_CONCURRENCY=str(threads), F2Z_LIG_PROFILE=(f'custom:{rate}:4' if target == 100 else f'udrg:{rate}:4:128'), PERFETTO_TRACE_PROCESSOR='/tmp/bitz-prover-validation-03/trace_processor_shell')
            cmd=[str(BINS[variant]), '--method',mode,'--r',str(exponent),'--c','0','--target',str(target),'--threads',str(threads),'--reps','9','--seed','0']
            log=OUT/f'{key}-p{pass_no}-{variant}.log'
            print(f'{key} pass={pass_no} {variant}', flush=True)
            with log.open('w') as f:
                subprocess.run(cmd, cwd=ROOT, env=env, stdout=f, stderr=subprocess.STDOUT, check=True)
            rows=[]
            for line in log.read_text().splitlines():
                if line.startswith('{'):
                    row=json.loads(line)
                    if row.get('trial') == 'sample':
                        rows.append(dict(row, variant=variant, pass_no=pass_no, config=key))
            assert len(rows)==9, (log, len(rows))
            all_rows.extend(rows)
            (OUT/'samples.json').write_text(json.dumps(all_rows, indent=2)+'\n')
            print('  median verify {:.3f} ms; prove {:.3f} ms'.format(statistics.median(r['verify_ms'] for r in rows), statistics.median(r['prove_ms'] for r in rows)), flush=True)
summary=[]
for mode, exponent, target, threads, rate in configs:
    key=f'{mode}-i{exponent}-l{target}-t{threads}-r{rate}'
    rows=[r for r in all_rows if r['config']==key]
    assert len({r['proof_digest'] for r in rows})==1, f'proof mismatch: {key}'
    assert len({r['proof_material_bytes'] for r in rows})==1, f'proof size mismatch: {key}'
    entry={'config':key,'mode':mode,'compressions':2**exponent,'security':target,'threads':threads,'log_inv_rate':rate,'profile':(f'custom:{rate}:4' if target == 100 else f'udrg:{rate}:4:128'),'proof_digest':rows[0]['proof_digest'],'proof_bytes':rows[0]['proof_material_bytes']}
    for variant in ['base','merged']:
        samples=[r for r in rows if r['variant']==variant]
        entry[variant]={field:statistics.median(r[field] for r in samples) for field in ['setup_ms','witness_ms','commit_ms','protocol_ms','prove_ms','e2e_prover_ms','verify_ms']}
        entry[variant]['verify_samples_ms']=[r['verify_ms'] for r in samples]
        entry[variant]['verify_phases_ms']={phase:statistics.median(dict(r['verify_phases_seconds']).get(phase,0)*1000 for r in samples) for phase in dict(samples[0]['verify_phases_seconds'])}
    entry['verify_speedup']=entry['base']['verify_ms']/entry['merged']['verify_ms']
    entry['prover_speedup']=entry['base']['prove_ms']/entry['merged']['prove_ms']
    summary.append(entry)
(OUT/'summary.json').write_text(json.dumps(summary, indent=2)+'\n')
print(json.dumps([{k:e[k] for k in ['config','verify_speedup','prover_speedup']} for e in summary],indent=2),flush=True)
assert all(e['verify_speedup']>1.05 for e in summary), 'Investigate configuration without a clear verifier improvement'

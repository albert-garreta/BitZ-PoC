#!/usr/bin/env python3
"""Per-cell interleaved qualification; never averages across configurations."""
import argparse, csv, hashlib, importlib.util, json, os, statistics, subprocess, threading, time
from pathlib import Path
ROOT=Path(__file__).resolve().parents[2]
p=argparse.ArgumentParser()
p.add_argument('--master',type=Path,required=True)
p.add_argument('--current',type=Path,required=True)
p.add_argument('--out',type=Path,required=True)
p.add_argument('--ordinary-only',action='store_true')
p.add_argument('--reps',type=int,default=21)
p.add_argument('--confirm-reps',type=int,default=101)
a=p.parse_args(); out=a.out.resolve();out.mkdir(parents=True,exist_ok=False)
a.master=a.master.resolve();a.current=a.current.resolve()
assert a.reps>=21 and a.confirm_reps>=101
spec=importlib.util.spec_from_file_location('gate',ROOT/'scripts/bench_gate.py')
gate=importlib.util.module_from_spec(spec);spec.loader.exec_module(gate)
env={k:v for k,v in os.environ.items() if not k.startswith(('F2Z_','OUTER_','RAYON_'))}
hashes={name:hashlib.file_digest(path.open('rb'),'sha256').hexdigest() for name,path in [('master',a.master),('current',a.current)]}
for name,path in [('master',a.master),('current',a.current)]:
    build=json.loads((path.parent/'metadata.json').read_text())
    assert build['sha256']==hashes[name] and build['revision']==name, ('build provenance mismatch',name)
    expected_revision='ac0aa44c5785db30f889fce8c5cdc98264a0e686' if name=='master' else 'f86193f1dcbb43a4bba48ea20670914df334f750'
    assert build['source_revision']==expected_revision, ('unexpected source revision',name)
(out/'metadata.json').write_text(json.dumps(dict(master=str(a.master),current=str(a.current),sha256=hashes,master_revision='ac0aa44c5785db30f889fce8c5cdc98264a0e686',preoptimization='f86193f1dcbb43a4bba48ea20670914df334f750',reps=a.reps,confirmation_reps=a.confirm_reps),indent=2)+'\n')
stop=threading.Event()
def monitor():
    with (out/'host.jsonl').open('w') as f:
        while not stop.is_set():
            f.write(json.dumps(dict(time=time.time(),cpu=Path('/proc/stat').read_text().splitlines()[0],load=Path('/proc/loadavg').read_text().strip(),memory=Path('/proc/meminfo').read_text()))+'\n');f.flush();stop.wait(5)
samples=[]
fixtures={}
reference=json.loads((ROOT/"experiments/outer-generic-master/preoptimization-pins.json").read_text())
def run(stage,session,case,variant,reps):
    bits,n,threads=case
    exe=a.master if variant=='master' else a.current
    protocol='skip-3' if variant=='skip' else 'ordinary'
    path=out/f'{stage}-s{session}-u{bits}-e{n}-t{threads}-{variant}.jsonl'
    run_env=dict(env,RAYON_NUM_THREADS=str(threads),OUTER_REPS=str(reps),OUTER_SHAPES=str(n),OUTER_WIDTHS=str(bits),OUTER_PROTOCOLS=protocol,OUTER_VARIANTS='generic')
    with path.open('w') as f:
        subprocess.run([str(exe)],env=run_env,stdout=f,stderr=subprocess.STDOUT,check=True)
    rows=[json.loads(line.removeprefix('OUTER_SAMPLE ')) for line in path.read_text().splitlines() if line.startswith('OUTER_SAMPLE ')]
    assert len(rows)==reps+1 and sum(r['warmup'] for r in rows)==1
    assert all(r['verified'] and r['threads']==threads and r['rows']==1<<n and r['bits']==bits and r['protocol']==protocol for r in rows)
    expected_revision='ac0aa44c5785db30f889fce8c5cdc98264a0e686' if variant=='master' else 'current'
    expected_variant='master' if variant=='master' else 'generic'
    assert all(r['revision']==expected_revision and r['variant']==expected_variant for r in rows)
    assert len({r['proof_digest'] for r in rows})==1
    if variant != 'master':
        key=f"{bits}/{1<<n}/{protocol}"
        assert rows[0]['proof_digest']==reference['pins'][key], ('preoptimization transcript mismatch',key)
    digest=rows[0]['fixture_digest']
    assert all(r['fixture_digest']==digest for r in rows)
    if case in fixtures: assert fixtures[case]==digest, ('fixture mismatch',case,variant)
    else: fixtures[case]=digest
    samples.extend(dict(r,stage=stage,session=session,implementation=variant) for r in rows)
    print(stage,session,case,variant,round(statistics.median(int(r['ns']) for r in rows if not r['warmup'])/1e6,4),'ms',flush=True)
def sessions(stage,cases,reps):
    variants=['master','ordinary']+([] if a.ordinary_only else ['skip'])
    for session in range(2):
        gate.wait_idle(f'{stage}-{session}',95,30,5,600)
        indexed_cases=list(enumerate(cases))
        for i,case in indexed_cases if session==0 else reversed(indexed_cases):
            order=variants if (i+session)%2==0 else list(reversed(variants))
            for v in order: run(stage,session,case,v,reps)
def table(stage,cases):
    table=[]
    for bits,n,threads in cases:
        med={}
        for s in range(2):
            for v in ['master','ordinary']+([] if a.ordinary_only else ['skip']):
                data=[int(r['ns']) for r in samples if r['stage']==stage and r['session']==s and r['implementation']==v and r['bits']==bits and r['rows']==1<<n and r['threads']==threads and not r['warmup']]
                med[s,v]=statistics.median(data)
        for req,num,den in [('ordinary_vs_master','ordinary','master')]+([] if a.ordinary_only else [('skip_vs_ordinary','skip','ordinary')]):
            ratios=[med[s,num]/med[s,den] for s in range(2)]
            table.append(dict(stage=stage,requirement=req,bits=bits,exponent=n,threads=threads,session0_ratio=ratios[0],session1_ratio=ratios[1],passes=all(r<=.95 for r in ratios),borderline=any(r>.93 for r in ratios),session0_numerator_ns=med[0,num],session0_denominator_ns=med[0,den],session1_numerator_ns=med[1,num],session1_denominator_ns=med[1,den]))
    return table
cases=[(b,n,t) for t in [1,10] for n in [12,15,17,19] for b in [32,64,128]]
gate.acquire('generic-outer-master',10)
watcher=threading.Thread(target=monitor);watcher.start()
try:
    sessions('initial',cases,a.reps)
    results=table('initial',cases)
    confirm=sorted({(r['bits'],r['exponent'],r['threads']) for r in results if r['borderline']})
    if confirm:
        sessions('confirmation',confirm,a.confirm_reps)
        results+=table('confirmation',confirm)
    with (out/'acceptance.csv').open('w') as f:
        writer=csv.DictWriter(f,fieldnames=list(results[0]));writer.writeheader();writer.writerows(results)
    (out/'samples.json').write_text(json.dumps(samples)+'\n')
    (out/'acceptance.json').write_text(json.dumps(results,indent=2)+'\n')
    subprocess.run(['python3',str(ROOT/'experiments/outer-generic-master/report.py'),str(out)],check=True)
    assert hashes=={name:hashlib.file_digest(path.open('rb'),'sha256').hexdigest() for name,path in [('master',a.master),('current',a.current)]}
    (out/'status.json').write_text(json.dumps(dict(status='complete',all_initial_pass=all(r['passes'] for r in results if r['stage']=='initial'),all_confirmation_pass=all(r['passes'] for r in results if r['stage']=='confirmation')))+'\n')
finally:
    stop.set();watcher.join();gate.release()

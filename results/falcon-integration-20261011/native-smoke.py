"""Run sequential Falcon integration proof checks and a small timing screen.

Candidate/baseline protocol IDs may differ. Each binary must produce identical
proofs across worker counts; paired inputs and successful verification are mandatory.
"""
import hashlib,json,os,random,statistics,subprocess,sys,time
from pathlib import Path
OUT=Path(sys.argv[1]);OUT.mkdir(parents=True,exist_ok=True)
BINDER=Path('/home/john-wu/code/BitZ-pcs/.tmp/falcon-binder-experiments')
BINS=json.loads(Path(sys.argv[2]).read_text())
CPUS=[6,7,4,5,2,3,0,1,14,15,12,13,10,11,8,9]
cache=BINDER/'results/binder-experiments-20261010/case-cache'
records=[];identities={};inputs={}

def run(version,variant,n,batch,security,workers,seed,iterations,warmup):
    key=f'{version}-{variant}-n{n}-b{batch}-s{security}-t{workers}-seed{seed}'
    binary=BINS[version][variant]
    argv=[binary,'--degree',str(n),'--batch',str(batch),'--security',str(security),'--threads',str(workers),'--seed',str(seed),'--iterations',str(iterations),'--warmup',str(warmup)]
    env={k:v for k,v in os.environ.items() if not k.startswith(('BITZ_','FLOCK_','RAYON_'))}
    env.update(RAYON_NUM_THREADS=str(workers),BITZ_FALCON_WORKER_CPUS=','.join(map(str,CPUS[:workers])),BITZ_FALCON_MAIN_CPU='6',BITZ_FALCON_CASE_CACHE=str(cache))
    start=time.time()
    p=subprocess.run(argv,env=env,text=True,capture_output=True)
    (OUT/(key+'.stdout.jsonl')).write_text(p.stdout)
    (OUT/(key+'.stderr')).write_text(p.stderr)
    if p.returncode: raise RuntimeError((key,p.returncode,p.stderr[-3000:]))
    data=[json.loads(line) for line in p.stdout.splitlines() if line.startswith('{')]
    rows=[r for r in data if r.get('trial') in ('sample','warmup')]
    assert len(rows)==iterations+warmup,(key,len(rows))
    for row in rows:
        assert row['verified'] and row['threads']==workers
        shape=(version,variant,n,batch,security,seed)
        identity=(row['proof_debug_digest'],row['proof_payload_bytes'],row['source_root'])
        assert identities.setdefault(shape,identity)==identity,(key,'thread proof mismatch')
        inp=(variant,n,batch,security,seed)
        assert inputs.setdefault(inp,row['input_digest'])==row['input_digest'],(key,'paired input mismatch')
    measured=[r for r in rows if r['trial']=='sample']
    def med(k):return statistics.median(r[k] for r in measured)
    rec=dict(version=version,variant=variant,degree=n,batch=batch,security=security,threads=workers,seed=seed,argv=argv,start=start,end=time.time(),verified_trials=len(rows),input_digest=rows[0]['input_digest'],proof_debug_digest=rows[0]['proof_debug_digest'],total_ms=med('total_prover_ms'),prove_ms=med('prove_ms' if variant=='arithmetic' else 'proof_prove_ms'),verify_ms=med('verify_ms' if variant=='arithmetic' else 'proof_verify_ms'),peak_rss_kib=max(r['process_peak_rss_kib'] for r in rows),protocol=next((r.get('protocol_id',r.get('protocol')) for r in data if r.get('protocol_id',r.get('protocol'))),None))
    records.append(rec)
    (OUT/'records.json').write_text(json.dumps(records,indent=2)+'\n')
    print(key,'verified',len(rows),'total_ms',round(rec['total_ms'],3),flush=True)

scaling=len(sys.argv)>3 and sys.argv[3]=='scaling'
variants=['arithmetic'] if scaling else ['arithmetic','full']
workers_set=[8,16] if scaling else [1,4,8,16]
seeds=list(range(180,186)) if scaling else [100,101]
jobs=[(v,w,s) for v in variants for w in workers_set for s in seeds]
random.Random(20261011).shuffle(jobs)
for variant,workers,seed in jobs:
    versions=['baseline','candidate'] if seed%2==0 else ['candidate','baseline']
    for version in versions:run(version,variant,512,1024,128,workers,seed,7 if scaling else 3,2 if scaling else 1)
# Degree-1024 checks exercise the new triple-Keccak packing. Small batches at
# both targets also cover both profile selectors and trimmed arithmetic domains.
for variant in ([] if scaling else ['arithmetic','full']):
    for workers in [1,4,8,16]:run('candidate',variant,1024,32,128,workers,42,1,1)
    for n in [512,1024]:run('candidate',variant,n,3,100,4,42,1,0)
summary=[]
for variant in variants:
    for w in workers_set:
        selected=[r for r in records if r['variant']==variant and r['degree']==512 and r['threads']==w and r['batch']==1024]
        entry=dict(variant=variant,threads=w)
        for v in ['baseline','candidate']:
            rr=[r for r in selected if r['version']==v]
            entry[v]={k:statistics.median(r[k] for r in rr) for k in ['total_ms','prove_ms','verify_ms','peak_rss_kib']}
        for k in ['total_ms','prove_ms']:
            entry[k+'_change_percent']=100*(entry['candidate'][k]/entry['baseline'][k]-1)
        summary.append(entry)
report=dict(scope=('Arithmetic scaling confirmation: six seeds, seven measured samples after two warmups.' if scaling else 'Descriptive integration timing screen; two seeds, three measured samples after one warmup. Not a statistical non-regression qualification.'),baseline_commit='c3bf4e4e3d52a350e3dcd6f9475654b6a307f52d',verified_trials=sum(r['verified_trials'] for r in records),processes=len(records),proof_identity='Identical within each binary and input across requested worker counts; branch-to-branch identity is not expected after protocol/layout changes.',binary_sha256={v:{k:hashlib.sha256(Path(p).read_bytes()).hexdigest() for k,p in bs.items()} for v,bs in BINS.items()},rows=summary)
(OUT/'summary.json').write_text(json.dumps(report,indent=2)+'\n')
print(json.dumps(report,indent=2),flush=True)

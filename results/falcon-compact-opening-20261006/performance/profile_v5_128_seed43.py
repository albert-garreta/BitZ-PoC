#!/usr/bin/env python3
"""Repeated diagnostic traces of frozen V5 candidate and V4 baseline; never qualification."""
import json, os, subprocess, sys, time
from pathlib import Path
ROOT=Path(__file__).resolve().parents[3]
sys.path.insert(0,str(ROOT/'scripts'))
import qualify_falcon_one_source as q
import bench_gate
import bench_support as support
from falcon_v2_campaign import environment
P=Path(__file__).resolve().parent
OUT=P/'v5/repeated-profile128-seed43'
OUT.mkdir(exist_ok=True)
assert not (OUT/'manifest.json').exists(), 'Refuse to overwrite an existing profile'
B=json.loads((P/'builds-v5.json').read_text())['binaries']
REF=json.loads((P/'cache-reference.json').read_text())['cases']['1:43']
CACHE=Path('/tmp/falcon-one-source-case-cache/fn-dsa-0.3.0-b1-seed43.bin')
CPUS=list(range(16))
env=environment(16)
env.update(BITZ_FALCON_WORKER_CPUS=','.join(map(str,CPUS)),BITZ_FALCON_MAIN_CPU='0',BITZ_FALCON_CASE_CACHE=str(CACHE.parent),BITZ_FALCON_STAGE_TIMINGS='1',FLOCK_COMMIT_TIMING='1',LIG_VERIFY_TRACE='1',VERIFY_TRACE='1')
flags={k:env[k] for k in ['BITZ_FALCON_WORKER_CPUS','BITZ_FALCON_MAIN_CPU','BITZ_FALCON_CASE_CACHE','BITZ_FALCON_STAGE_TIMINGS','FLOCK_COMMIT_TIMING','LIG_VERIFY_TRACE','VERIFY_TRACE','RUSTFLAGS','RAYON_NUM_THREADS']}
m=dict(status='preparing',qualification=False,purpose='Attribution only: repeated warm stages and verifier trace',builds=B,order=['baseline','candidate','candidate','baseline'],warmups=3,measured=30,seed=43,protocol='native',security=128,batch=1,environment=flags,runs=[],started_at=time.time())
def save():support.write_json(OUT/'manifest.json',m)
save()
perf=subprocess.run(['perf','stat','-e','cycles:u,instructions:u','--','/bin/true'],capture_output=True,text=True)
(OUT/'perf-access.stdout').write_text(perf.stdout);(OUT/'perf-access.stderr').write_text(perf.stderr)
m['perf_access']={'command':['perf','stat','-e','cycles:u,instructions:u','--','/bin/true'],'returncode':perf.returncode,'perf_event_paranoid':Path('/proc/sys/kernel/perf_event_paranoid').read_text().strip()};save()
bench_gate.acquire('falcon-v5-native128-repeated-profile',5)
results=[]
try:
 for build in B.values():q.verify_build(build)
 assert support.file_hash(CACHE)==REF['sha256']
 bench_gate.wait_idle('falcon-v5-native128-repeated-profile',88,20,20,300)
 m['status']='running';save()
 for i,label in enumerate(m['order']):
  build=B[label];stem=f'{i+1}-{label}'
  command=[build['binary'],'--protocol','native','--security','128','--batch','1','--threads','16','--seed','43','--warmup','3','--iterations','30']
  record=dict(label=label,command=command,started_at=time.time())
  m['runs'].append(record);save()
  with (OUT/f'{stem}.jsonl').open('w') as out,(OUT/f'{stem}.stderr').open('w') as err:
   q.run_child(command,env,out,err,120,bench_gate.swap_used_gb(),.1)
  result=q.validate_rows(q.read_rows(OUT/f'{stem}.jsonl'),build,'native',128,1,16,43,3,30,True,CPUS)
  assert result['header']['input_digest']==REF['input_digest']
  results.append(result)
  record.update(status='validated',finished_at=time.time(),medians=result['medians'],stdout_sha256=support.file_hash(OUT/f'{stem}.jsonl'),stderr_sha256=support.file_hash(OUT/f'{stem}.stderr'));save()
  print('DONE',stem,result['medians'],flush=True)
 q.validate_pair(results[0],results[1],'compact-opening');q.validate_pair(results[3],results[2],'compact-opening')
 for build in B.values():q.verify_build(build)
 assert support.file_hash(CACHE)==REF['sha256']
 m.update(status='completed',verified_proofs=132,measured_proofs=120,final_integrity='pass',finished_at=time.time());save()
finally:bench_gate.release()

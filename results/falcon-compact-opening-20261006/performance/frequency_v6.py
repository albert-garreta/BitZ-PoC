#!/usr/bin/env python3
"""Bounded read-only cpufreq observation; diagnostic only, no policy changes."""
import json,os,subprocess,sys,threading,time
from pathlib import Path
P=Path(__file__).resolve().parent;ROOT=P.parents[2];sys.path.insert(0,str(ROOT/'scripts'))
import bench_gate
import bench_support as support
import qualify_falcon_one_source as q
from falcon_v2_campaign import environment
OUT=P/'v6/frequency-abba';OUT.mkdir(exist_ok=False)
B=json.loads((P/'builds-v6.json').read_text())['binaries'];CPUS=list(range(16));REF=json.loads((P/'cache-reference.json').read_text())['cases']['1:42'];CACHE=Path('/tmp/falcon-one-source-case-cache/fn-dsa-0.3.0-b1-seed42.bin')
def policy():
 keys=['scaling_driver','scaling_governor','energy_performance_preference','scaling_min_freq','scaling_max_freq','cpuinfo_min_freq','cpuinfo_max_freq']
 return {'cpus':{str(i):{k:(Path(f'/sys/devices/system/cpu/cpu{i}/cpufreq')/k).read_text().strip() for k in keys} for i in CPUS},'boost':Path('/sys/devices/system/cpu/cpufreq/boost').read_text().strip(),'amd_pstate_status':Path('/sys/devices/system/cpu/amd_pstate/status').read_text().strip()}
env=environment(16)
for k in ['LIG_VERIFY_TRACE','VERIFY_TRACE']:env.pop(k,None)
env.update(BITZ_FALCON_WORKER_CPUS=','.join(map(str,CPUS)),BITZ_FALCON_MAIN_CPU='0',BITZ_FALCON_CASE_CACHE=str(CACHE.parent))
m={'status':'prepared','qualification':False,'purpose':'Observe reported CPU frequencies and stdout arrival times during fresh processes; no policy changes','order':['baseline','candidate','candidate','baseline'],'protocol':'native','security':128,'batch':1,'seed':42,'warmup':1,'measured':3,'frequency_interval_seconds':.05,'frequency_path':'/sys/devices/system/cpu/cpu{0..15}/cpufreq/scaling_cur_freq','frequency_units':'kHz, driver-reported sample; not hardware-counter effective frequency','time_axis':'Python time.monotonic_ns; each stdout line timestamped on receipt','trial_boundary_limit':'A trial interval runs from receipt of prepared/previous-trial line to receipt of trial line; includes untimed bookkeeping and verifier work. Exact substage times unavailable without instrumentation.','builds':B,'policy_before':policy(),'environment':{k:env[k] for k in ['BITZ_FALCON_WORKER_CPUS','BITZ_FALCON_MAIN_CPU','BITZ_FALCON_CASE_CACHE','RUSTFLAGS','RAYON_NUM_THREADS']},'runs':[],'started_at':time.time()}
def save():support.write_json(OUT/'manifest.json',m)
save();bench_gate.acquire('falcon-v6-frequency-diagnostic',5);results=[]
try:
 for b in B.values():q.verify_build(b)
 assert support.file_hash(CACHE)==REF['sha256']
 bench_gate.wait_idle('falcon-v6-frequency-diagnostic',88,20,5,300)
 m['status']='running';save()
 for index,label in enumerate(m['order'],1):
  b=B[label];stem=f'{index}-{label}';frequencies=[];events=[];stop=threading.Event();errors=[]
  def sample():
   next_at=time.monotonic()
   while not stop.is_set():
    begin=time.monotonic_ns()
    try:values=[int(Path(f'/sys/devices/system/cpu/cpu{i}/cpufreq/scaling_cur_freq').read_text()) for i in CPUS]
    except BaseException as e:errors.append(repr(e));return
    frequencies.append({'start_ns':begin,'end_ns':time.monotonic_ns(),'cpu_khz':values})
    next_at+=.05;stop.wait(max(0,next_at-time.monotonic()))
  thread=threading.Thread(target=sample,daemon=True);thread.start()
  bench_gate.wait_idle(stem,88,1,1,300)
  command=[b['binary'],'--protocol','native','--security','128','--batch','1','--threads','16','--seed','42','--warmup','1','--iterations','3']
  record={'label':label,'command':command,'started_at':time.time(),'start_monotonic_ns':time.monotonic_ns()}
  with (OUT/f'{stem}.stderr').open('w') as err,(OUT/f'{stem}.jsonl').open('w') as out:
   proc=subprocess.Popen(command,env=env,stdout=subprocess.PIPE,stderr=err,text=True,bufsize=1)
   for line in proc.stdout:
    received=time.monotonic_ns();out.write(line);row=json.loads(line);events.append({'received_ns':received,'event':row['event'],'trial':row.get('trial'),'sample':row.get('sample')})
   assert proc.wait(timeout=120)==0
  record['end_monotonic_ns']=time.monotonic_ns();stop.set();thread.join();assert not errors,errors
  for name,rows in [('frequency',frequencies),('events',events)]:
   (OUT/f'{stem}.{name}.jsonl').write_text(''.join(json.dumps(x)+'\n' for x in rows))
  result=q.validate_rows(q.read_rows(OUT/f'{stem}.jsonl'),b,'native',128,1,16,42,1,3,False,CPUS);assert result['header']['input_digest']==REF['input_digest'];results.append(result)
  record.update(status='validated',frequency_samples=len(frequencies),files={f'{stem}{ext}':support.file_hash(OUT/f'{stem}{ext}') for ext in ['.jsonl','.stderr','.frequency.jsonl','.events.jsonl']});m['runs'].append(record);save();print('DONE',stem,result['medians'],flush=True)
 q.validate_pair(results[0],results[1],'compact-opening');q.validate_pair(results[3],results[2],'compact-opening')
 for b in B.values():q.verify_build(b)
 assert support.file_hash(CACHE)==REF['sha256'];m['policy_after']=policy();assert m['policy_before']==m['policy_after']
 m.update(status='completed',verified_proofs=16,final_integrity='pass',policy_unchanged=True,finished_at=time.time());save()
finally:bench_gate.release()

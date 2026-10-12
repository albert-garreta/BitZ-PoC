#!/usr/bin/env python3
"""Interleaved matched-input experiments; each proof is verified by the binary."""
import argparse,csv,hashlib,json,math,os,pathlib,random,statistics,subprocess,time,shutil
OUT=pathlib.Path(__file__).resolve().parent
SOURCE=pathlib.Path('/tmp/falcon-grinding-experiment-20261009')
WORKERS=(6,7,4,5,2,3,0,1,14,15,12,13,10,11,8,9)
COST=str(OUT/'costs/curves.json')
PROFILES={
 'baseline':{},
 'original':{},
 'global':{'BITZ_EXPERIMENT_GLOBAL':'1'},
 'weighted':{'BITZ_EXPERIMENT_GLOBAL':'1','BITZ_EXPERIMENT_CORRECTED_SITES':'1','BITZ_EXPERIMENT_COST_FILE':COST},
 **{f'queries{q}':{'BITZ_EXPERIMENT_GLOBAL':'1','BITZ_EXPERIMENT_QUERY_BITS':str(q)} for q in (124,120,116)},
 'weighted_queries120':{'BITZ_EXPERIMENT_GLOBAL':'1','BITZ_EXPERIMENT_CORRECTED_SITES':'1','BITZ_EXPERIMENT_COST_FILE':COST,'BITZ_EXPERIMENT_QUERY_BITS':'120'},
 'weighted_external':{'BITZ_EXPERIMENT_GLOBAL':'1','BITZ_EXPERIMENT_CORRECTED_SITES':'1','BITZ_EXPERIMENT_COST_FILE':str(OUT/'costs-external/curves.json')},
 'weighted_external_queries120':{'BITZ_EXPERIMENT_GLOBAL':'1','BITZ_EXPERIMENT_CORRECTED_SITES':'1','BITZ_EXPERIMENT_COST_FILE':str(OUT/'costs-external/curves.json'),'BITZ_EXPERIMENT_QUERY_BITS':'120'},
}
def save(path,data):path.write_text(json.dumps(data,indent=2)+'\n')
def stats():
 return {int(v[0][3:]):list(map(int,v[1:9])) for l in pathlib.Path('/proc/stat').read_text().splitlines() if (v:=l.split())[0].startswith('cpu') and v[0]!='cpu'}
def busy(a,b,c):
 d=[y-x for x,y in zip(a[c],b[c])];return 100*(sum(d)-d[3]-d[4])/max(1,sum(d))
def run(phase,kind,degree,threads,seed,profile,reps=3,attempt=0):
 dest=OUT/phase;dest.mkdir(exist_ok=True)
 stem=f'{kind}-{degree}-t{threads}-s{seed}-{profile}'
 resultpath=dest/(stem+'.result.json')
 cpus=WORKERS[:threads];siblings=[i+16 for i in cpus]
 env={k:v for k,v in os.environ.items() if not k.startswith(('BITZ_','RAYON_','RUSTFLAGS','CARGO_ENCODED_RUSTFLAGS'))}
 settings={'RAYON_NUM_THREADS':str(threads),'OMP_NUM_THREADS':str(threads),'BITZ_FALCON_MAIN_CPU':'6','BITZ_FALCON_WORKER_CPUS':','.join(map(str,cpus)),'BITZ_FALCON_CASE_CACHE':'/tmp/falcon-algebraic-cases-20261007'}|PROFILES[profile]
 if 'BITZ_EXPERIMENT_COST_FILE' in settings:settings['BITZ_EXPERIMENT_COST_THREADS']=str(threads)
 env.update(settings)
 binary_dir=OUT.parent/'falcon-grinding-opt-128-d9b13bdb4'/'binaries' if profile=='original' else OUT/'binaries'
 exe=binary_dir/('falcon_algebraic' if kind=='arithmetic' else 'falcon_hybrid')
 args=['taskset','-c',','.join(map(str,cpus)),str(exe),'--degree',str(degree),'--batch','1024','--security','128','--threads',str(threads),'--seed',str(seed),'--warmup','1','--iterations',str(reps)]
 fingerprint=hashlib.sha256(json.dumps({'argv':args,'settings':settings,'binary':hashlib.sha256(exe.read_bytes()).hexdigest(),'costs':hashlib.sha256(pathlib.Path(settings.get('BITZ_EXPERIMENT_COST_FILE',COST)).read_bytes()).hexdigest(),'sources':hashlib.sha256((OUT/'source-hashes.json').read_bytes()).hexdigest()},sort_keys=True).encode()).hexdigest()
 if resultpath.exists():
  old=json.loads(resultpath.read_text());assert old['execution_fingerprint']==fingerprint,'cached experiment provenance differs';return old
 for idle_attempt in range(300):
  idle_a=stats();time.sleep(1);idle_b=stats()
  loads={str(c):busy(idle_a,idle_b,c) for c in (*cpus,*siblings)}
  if max(loads.values())<5:break
  if idle_attempt%30==29:print('Waiting for idle benchmark cores: '+stem,flush=True)
 else:raise RuntimeError('benchmark cores are busy: '+json.dumps(loads))
 save(dest/(stem+'.command.json'),{'argv':args,'environment':settings,'execution_fingerprint':fingerprint,'idle_busy_pct':loads})
 a=stats();start=time.monotonic()
 with (dest/(stem+'.jsonl')).open('w') as stdout,(dest/(stem+'.stderr')).open('w') as stderr:
  process=subprocess.Popen(args,cwd=SOURCE,env=env,stdout=stdout,stderr=stderr)
  _,status,usage=os.wait4(process.pid,0);process.returncode=os.waitstatus_to_exitcode(status)
 b=stats();elapsed=time.monotonic()-start
 resource={'exit_code':process.returncode,'elapsed_s':elapsed,'max_rss_kib':usage.ru_maxrss,'sibling_busy_pct':{str(c):busy(a,b,c) for c in siblings},'involuntary_context_switches':usage.ru_nivcsw}
 resource['cpu_state_tick_deltas']={str(c):dict(zip(('user','nice','system','idle','iowait','irq','softirq','steal'),[y-x for x,y in zip(a[c],b[c])])) for c in (*cpus,*siblings)}
 save(dest/(stem+'.resources.json'),resource)
 assert process.returncode==0,(stem,(dest/(stem+'.stderr')).read_text()[-5000:])
 if max(resource['sibling_busy_pct'].values())>=2:
  rejected=dest/'discarded'/f'{stem}-{time.time_ns()}';rejected.mkdir(parents=True)
  for file in dest.glob(stem+'.*'):shutil.move(file,rejected/file.name)
  save(rejected/'reason.json',{'reason':'predeclared sibling utilization >=2%','attempt':attempt})
  assert attempt<29,'thirty noisy attempts'
  print('Discarding noisy timing attempt: '+stem,flush=True)
  return run(phase,kind,degree,threads,seed,profile,reps,attempt+1)
 records=[json.loads(l) for l in (dest/(stem+'.jsonl')).read_text().splitlines() if l.startswith('{')]
 trials=[r for r in records if r.get('trial') in ('sample','warmup')];samples=[r for r in trials if r['trial']=='sample']
 assert len(samples)==reps and len(trials)==reps+1 and all(r['verified'] for r in trials),stem
 security=next(r['algebraic_security_bits'] for r in records if 'algebraic_security_bits' in r)
 assert security>=128+math.log2(16/15)-1e-10,(stem,security)
 for r in records:
  if 'cpu_affinity' in r:assert r['cpu_affinity']['observed_worker_cpus']==[[c] for c in cpus],stem
 keys={'commit':'witness_commit_ms','prove':'prove_ms' if kind=='arithmetic' else 'proof_prove_ms','total':'total_prover_ms','verify':'verify_ms' if kind=='arithmetic' else 'proof_verify_ms'}
 result={'variant':kind,'degree':degree,'threads':threads,'seed':seed,'profile':profile,'security_bits':security,'verified_proofs':len(trials),'input_digest':samples[0]['input_digest'],'proof_digest':samples[0]['proof_debug_digest'],'proof_bytes':samples[0]['proof_payload_bytes']+(32 if kind=='full' else 0),'max_rss_mib':usage.ru_maxrss/1024,'max_sibling_busy_pct':max(resource['sibling_busy_pct'].values()),'phase':phase,'execution_fingerprint':fingerprint}
 assert len({r['proof_debug_digest'] for r in trials})==1,stem
 for k,key in keys.items():result[k+'_ms']=statistics.median(r[key] for r in samples);result[k+'_samples_ms']=[r[key] for r in samples]
 save(resultpath,result)
 print(f'{stem}: total={result["total_ms"]:.2f} prove={result["prove_ms"]:.2f} verify={result["verify_ms"]:.2f} bits={security:.6f} sibling={result["max_sibling_busy_pct"]:.2f}%',flush=True)
 return result

def main():
 parser=argparse.ArgumentParser();parser.add_argument('--phase',default='screen');parser.add_argument('--seeds',default='42,43,44');parser.add_argument('--threads',default='best');parser.add_argument('--profiles',default=','.join(PROFILES));parser.add_argument('--reps',type=int,default=7);args=parser.parse_args()
 os.sched_setaffinity(0,{31});rng=random.Random(20261009)
 for seed in map(int,args.seeds.split(',')):
  configs=[(kind,degree) for kind in ('arithmetic','full') for degree in (512,1024)];rng.shuffle(configs)
  for kind,degree in configs:
   threads=[8 if kind=='arithmetic' else 16] if args.threads=='best' else list(map(int,args.threads.split(',')))
   for t in threads:
    profiles=args.profiles.split(',');rng.shuffle(profiles)
    results=[run(args.phase,kind,degree,t,seed,profile,args.reps) for profile in profiles]
    assert len({r['input_digest'] for r in results})==1
if __name__=='__main__':main()

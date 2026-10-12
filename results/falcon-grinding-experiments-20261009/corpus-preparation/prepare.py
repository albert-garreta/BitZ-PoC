"""Correctness-only corpus generation/reload; timings must not enter benchmarks."""
import concurrent.futures,hashlib,json,os,pathlib,queue,subprocess,time
OUT=pathlib.Path(__file__).resolve().parent
ROOT=OUT.parent
EXE=ROOT/'binaries/falcon_algebraic'
CACHE=pathlib.Path('/tmp/falcon-algebraic-cases-20261007')
CORES=queue.Queue()
for c in (0,2,4,6): CORES.put(c)
def save(p,j): p.write_text(json.dumps(j,indent=2)+'\n')
def one(degree,seed):
 if (OUT/'STOP').exists(): return {'degree':degree,'seed':seed,'skipped':'STOP requested'}
 cpu=CORES.get()
 try:
  stem=f'n{degree}-b1024-seed{seed}'
  cached=CACHE/f'fn-dsa-0.3.0-{stem}.bin'
  existed=cached.exists()
  env={k:v for k,v in os.environ.items() if not k.startswith(('BITZ_','RAYON_','RUSTFLAGS','CARGO_ENCODED_RUSTFLAGS'))}
  settings={'RAYON_NUM_THREADS':'1','OMP_NUM_THREADS':'1','BITZ_FALCON_MAIN_CPU':str(cpu),'BITZ_FALCON_WORKER_CPUS':str(cpu),'BITZ_FALCON_CASE_CACHE':str(CACHE)}
  env.update(settings)
  argv=['taskset','-c',str(cpu),str(EXE),'--threads','1','--warmup','0','--iterations','1','--security','128','--degree',str(degree),'--batch','1024','--seed',str(seed)]
  save(OUT/(stem+'.command.json'),{'argv':argv,'environment':settings,'purpose':'correctness-only corpus preparation; all timings excluded from performance analysis'})
  before=hashlib.sha256(EXE.read_bytes()).hexdigest()
  result={'degree':degree,'seed':seed,'batch':1024,'cpu':cpu,'existed_before':existed,'binary_sha256':before,'timing_purpose':'correctness-only, excluded','verified_proofs':0,'runs':[]}
  for stage in (['reload'] if existed else ['generate','reload']):
   start=time.monotonic()
   with (OUT/f'{stem}.{stage}.jsonl').open('w') as log,(OUT/f'{stem}.{stage}.stderr').open('w') as err:
    process=subprocess.run(argv,env=env,cwd='/tmp/falcon-grinding-experiment-20261009',stdout=log,stderr=err)
   assert process.returncode==0,(stem,stage,process.returncode)
   rows=[json.loads(l) for l in (OUT/f'{stem}.{stage}.jsonl').read_text().splitlines() if l.startswith('{')]
   trials=[r for r in rows if r.get('trial')=='sample']
   assert len(trials)==1 and trials[0]['verified'],(stem,stage)
   affinity=[r['cpu_affinity'] for r in rows if 'cpu_affinity' in r]
   assert affinity and all(r['observed_worker_cpus']==[[cpu]] for r in affinity)
   result['runs'].append({'stage':stage,'wall_seconds':time.monotonic()-start,'input_digest':trials[0]['input_digest'],'proof_digest':trials[0]['proof_debug_digest'],'external_input_ms':trials[0]['external_input_ms']})
   result['verified_proofs']+=1
   print(stem,stage,round(result['runs'][-1]['wall_seconds'],2),'s',flush=True)
  assert len({r['input_digest'] for r in result['runs']})==1
  assert len({r['proof_digest'] for r in result['runs']})==1
  assert hashlib.sha256(EXE.read_bytes()).hexdigest()==before
  result['cache_file']=str(cached)
  result['cache_bytes']=cached.stat().st_size
  result['cache_sha256']=hashlib.sha256(cached.read_bytes()).hexdigest()
  result['cache_checksum_validated_by_binary']=True
  save(OUT/(stem+'.result.json'),result)
  return result
 finally: CORES.put(cpu)
def main():
 os.sched_setaffinity(0,{31})
 started=time.time()
 jobs=[(degree,seed) for seed in range(43,55) for degree in (512,1024)]
 with concurrent.futures.ThreadPoolExecutor(max_workers=4) as pool:
  results=list(pool.map(lambda job:one(*job),jobs))
 save(OUT/'summary.json',{'purpose':'correctness-only corpus generation and cached checksum/upstream/native/proof verification; timings excluded','elapsed_seconds':time.time()-started,'results':results,'verified_proofs':sum(r.get('verified_proofs',0) for r in results)})
 print('DONE',len(results),'corpora',sum(r.get('verified_proofs',0) for r in results),'verified proofs',round(time.time()-started,2),'seconds',flush=True)
if __name__=='__main__': main()

from pathlib import Path
import os,json,subprocess,re,statistics
p=Path('/tmp/falcon-power-basis');out=p/'traces';out.mkdir(exist_ok=False)
env=os.environ.copy();env.update(RAYON_NUM_THREADS='1',BITZ_FALCON_WORKER_CPUS='0',BITZ_FALCON_MAIN_CPU='0',FLOCK_NO_PREFAULT='1',BITZ_FALCON_CASE_CACHE='/tmp/falcon-algebraic-cases-20261007')
rows=[]; commands=[]
for n in [512,1024]:
 for mode in ['baseline','candidate']:
  directory='candidate' if mode=='candidate' else ('baseline-512' if n==512 else 'baseline-head')
  cmd=['taskset','-c','0',str(p/directory/'falcon_algebraic'),'--batch','1024','--security','128','--threads','1','--warmup','1','--iterations','3','--seed','42','--trace']
  if mode=='candidate' or n==512:cmd+=['--degree',str(n)]
  name=f'{mode}-n{n}'
  with (out/(name+'.jsonl')).open('w') as stdout,(out/(name+'.stderr')).open('w') as stderr:
   subprocess.run(cmd,cwd='/home/john-wu/code/BitZ-pcs',env=env,stdout=stdout,stderr=stderr,check=True,timeout=1800)
  commands.append(cmd)
  for stage in ['prove','verify']:
   times=[]
   for line in (out/(name+'.stderr')).read_text().splitlines():
    line=re.sub(r'\x1b\[[0-9;]*m','',line)
    if f'falcon_algebraic_ring:{stage}:' in line and 'falcon_algebraic_ring:coordinates:' in line and 'time.busy=' in line:
     parts=re.findall(r'time\.(?:busy|idle)=([\d.]+)(ns|µs|us|ms|s)',line)
     times.append(sum(float(v)*{'ns':1e-6,'µs':1e-3,'us':1e-3,'ms':1,'s':1000}[u] for v,u in parts))
   assert len(times)==4,(name,stage,len(times))
   rows.append(dict(mode=mode,degree=n,stage=stage,coordinates_ms=statistics.median(times[1:]),trials_ms=times))
(out/'summary.json').write_text(json.dumps(rows,indent=2)+'\n')
(out/'commands.json').write_text(json.dumps(commands,indent=2)+'\n')
print(json.dumps(rows,indent=2))

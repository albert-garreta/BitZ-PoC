import hashlib,json,os,pathlib,subprocess
import campaign
OUT=campaign.OUT

def exe_from(log,name):
 rows=[json.loads(l) for l in (OUT/log).read_text().splitlines() if l.startswith('{')]
 assert rows[-1].get('success') is True,(log,rows[-1])
 return pathlib.Path(next(r['executable'] for r in reversed(rows) if r.get('reason')=='compiler-artifact' and r['target']['name']==name and r.get('executable')))
def main():
 binary=exe_from('test-build.jsonl','bitz');dest=OUT/'validation';dest.mkdir(exist_ok=True)
 results=[]
 cases=[('base-unit',{},['hybrid::grinding_']),('query-unit',{},['experimental_queries']),('baseline',{},['experimental_128','experiment_prime','hybrid_falcon_roundtrip_and_tampering'])]
 for name in ('global','weighted','queries124','queries120','queries116','weighted_queries120'):
  settings=campaign.PROFILES[name].copy()
  if 'BITZ_EXPERIMENT_COST_FILE' in settings:settings['BITZ_EXPERIMENT_COST_THREADS']='8'
  cases.append((name,settings,['experimental_128','experiment_prime']))
 for threads in (1,2,4,16):
  for name in ('weighted','weighted_queries120'):
   cases.append((f'{name}-t{threads}',campaign.PROFILES[name]|{'BITZ_EXPERIMENT_COST_THREADS':str(threads)},['experimental_128','experiment_prime']))
 for label,settings,filters in cases:
  env={k:v for k,v in os.environ.items() if not k.startswith(('BITZ_','RAYON_'))};env|=settings|{'RAYON_NUM_THREADS':'2'}
  for selector in filters:
   stem=label+'-'+selector.replace('::','_');args=['taskset','-c','14,15',str(binary),selector,'--test-threads=1','--nocapture']
   campaign.save(dest/(stem+'.command.json'),{'argv':args,'environment':settings|{'RAYON_NUM_THREADS':'2'}})
   with (dest/(stem+'.log')).open('w') as log:r=subprocess.run(args,env=env,cwd=campaign.SOURCE,stdout=log,stderr=subprocess.STDOUT)
   text=(dest/(stem+'.log')).read_text()
   assert r.returncode==0 and '0 passed; 0 failed' not in text,(stem,text[-6000:])
   line=next(l for l in reversed(text.splitlines()) if l.startswith('test result:'))
   results.append({'case':label,'filter':selector,'result':line});print(label,selector,line,flush=True)
   campaign.save(dest/'summary.json',results)
 print('ALL EXPERIMENT VALIDATION PASSED',flush=True)
if __name__=='__main__':main()

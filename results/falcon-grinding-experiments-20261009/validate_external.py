import os,subprocess
import campaign,validate

def main():
 binary=validate.exe_from('test-build.jsonl','bitz')
 dest=campaign.OUT/'validation-external';dest.mkdir(exist_ok=True);results=[]
 for threads in (1,2,4,8,16):
  for name in ('weighted_external','weighted_external_queries120'):
   settings=campaign.PROFILES[name]|{'BITZ_EXPERIMENT_COST_THREADS':str(threads),'RAYON_NUM_THREADS':'2'}
   env={k:v for k,v in os.environ.items() if not k.startswith(('BITZ_','RAYON_'))}|settings
   for selector in ('experimental_128','experiment_prime'):
    stem=f'{name}-t{threads}-{selector}';args=['taskset','-c','14,15',str(binary),selector,'--test-threads=1','--nocapture']
    campaign.save(dest/(stem+'.command.json'),{'argv':args,'environment':settings})
    with (dest/(stem+'.log')).open('w') as log:r=subprocess.run(args,cwd=campaign.SOURCE,env=env,stdout=log,stderr=subprocess.STDOUT)
    text=(dest/(stem+'.log')).read_text()
    assert r.returncode==0 and '0 passed; 0 failed' not in text,(stem,text[-6000:])
    line=next(l for l in reversed(text.splitlines()) if l.startswith('test result:'))
    results.append({'case':stem,'result':line});campaign.save(dest/'summary.json',results);print(stem,line,flush=True)
 print('ALL EXTERNAL-COST VALIDATION PASSED',flush=True)
if __name__=='__main__':main()

import json,os,subprocess,statistics,time
from pathlib import Path
ROOT=Path.cwd(); root=ROOT/'target/falcon-integer-merge'; out=root/'arithmetic-512-confirmation';out.mkdir(exist_ok=True)
meta=json.loads((root/'measurements/metadata.json').read_text())
executables={'pre_native':str(root/'rebalanced-candidate-binaries/arithmetic'), **{r:meta['executables'][r]['arithmetic']['path'] for r in ['candidate','baseline']}}
env={k:v for k,v in os.environ.items() if not k.startswith(('BITZ_','FLOCK_'))};env.update(RAYON_NUM_THREADS='8',NO_COLOR='1')
records=[]
for revision,exe in executables.items():
    cmd=[exe,'--degree','512','--batch','1024','--security','100','--threads','8','--seed','42','--warmup','1','--iterations','21']
    print('ARITHMETIC CONFIRM',revision,flush=True);start=time.monotonic()
    with (out/(revision+'.stdout.log')).open('w') as stdout, (out/(revision+'.stderr.log')).open('w') as stderr:
        subprocess.run(cmd,cwd=ROOT,env=env,stdout=stdout,stderr=stderr,check=True)
    trials=[json.loads(x) for x in (out/(revision+'.stdout.log')).read_text().splitlines() if x.startswith('{')]
    samples=[t for t in trials if t.get('trial')=='sample']
    assert len(samples)==21 and all(t['verified'] for t in trials)
    records.append({'name':'arithmetic-512-'+revision,'revision':revision,'family':'arithmetic','degree':512,'command':cmd,'elapsed_seconds':time.monotonic()-start,'samples':samples})
    print(revision,{k:statistics.median(s[k] for s in samples) for k in ['prove_ms','total_prover_ms','verify_ms']},flush=True)
(out/'results.json').write_text(json.dumps(records,indent=2)+'\n')
assert len({s['input_digest'] for r in records for s in r['samples']})==1
all_records=json.loads((root/'measurements/results.json').read_text())
all_records=[r for r in all_records if not (r['family']=='arithmetic' and r['degree']==512)]+[r for r in records if r['revision']!='pre_native']
(root/'measurements/results.json').write_text(json.dumps(all_records,indent=2)+'\n')
meta['samples_by_case']={f'{family}-{degree}':21 if family=='full' or degree==512 else 5 for family in ['full','arithmetic'] for degree in [512,1024]}
meta.pop('samples_by_family',None)
(root/'measurements/metadata.json').write_text(json.dumps(meta,indent=2)+'\n')
summary=[]
for family in ['full','arithmetic']:
 for degree in [512,1024]:
  row={'family':family,'degree':degree}
  for r in all_records:
   if r['family']==family and r['degree']==degree:
    samples=r['samples'];row[r['revision']]={k:statistics.median(x[k] for x in samples) for k in samples[0] if isinstance(samples[0][k],(float,int)) and (k.endswith('_ms') or k.endswith('_bytes'))}
  summary.append(row)
(root/'measurements/summary.json').write_text(json.dumps(summary,indent=2)+'\n')

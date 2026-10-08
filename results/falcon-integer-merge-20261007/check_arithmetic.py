import hashlib,json,os,subprocess,statistics,time
from pathlib import Path
ROOT=Path.cwd();root=ROOT/'target/falcon-integer-merge';out=root/'arithmetic-validation';out.mkdir(exist_ok=True)
meta_path=root/'measurements/metadata.json';meta=json.loads(meta_path.read_text());(out/'prior-metadata.json').write_text(json.dumps(meta,indent=2)+'\n')
env={k:v for k,v in os.environ.items() if not k.startswith(('BITZ_','FLOCK_'))};env.update(RAYON_NUM_THREADS='8',NO_COLOR='1')
records=[]
for degree in [512,1024]:
 for revision in (['baseline','candidate'] if degree==512 else ['candidate','baseline']):
    exe=meta['executables'][revision]['arithmetic']['path'];name=f'arithmetic-{degree}-{revision}'
    cmd=[exe,'--degree',str(degree),'--batch','1024','--security','100','--threads','8','--seed','42','--warmup','1','--iterations','21']
    print('START',name,flush=True);start=time.monotonic()
    with (out/(name+'.stdout.log')).open('w') as stdout,(out/(name+'.stderr.log')).open('w') as stderr:
     subprocess.run(cmd,cwd=ROOT,env=env,stdout=stdout,stderr=stderr,check=True)
    trials=[json.loads(x) for x in (out/(name+'.stdout.log')).read_text().splitlines() if x.startswith('{')]
    samples=[s for s in trials if s.get('trial')=='sample']
    assert len(samples)==21 and len(trials)==22 and all(s['verified'] for s in trials)
    records.append({'name':name,'family':'arithmetic','degree':degree,'revision':revision,'command':cmd,'elapsed_seconds':time.monotonic()-start,'samples':samples})
    (out/'results.json').write_text(json.dumps(records,indent=2)+'\n')
    print('DONE',name,{k:statistics.median(s[k] for s in samples) for k in ['prove_ms','total_prover_ms','verify_ms']},flush=True)
 for r in records:
  if r['degree']==degree:assert all(s['input_digest']==samples[0]['input_digest'] for s in r['samples'])
old=json.loads((root/'measurements/results.json').read_text());all_records=[r for r in old if r['family']=='full']+records
(root/'measurements/results-before-validation.json').write_text(json.dumps(old,indent=2)+'\n')
(root/'measurements/results.json').write_text(json.dumps(all_records,indent=2)+'\n')
meta['candidate_patch_sha256_by_family']={'full':meta.pop('candidate_patch_sha256'),'arithmetic':hashlib.sha256(subprocess.check_output(['git','diff','HEAD','--','src'])).hexdigest()}
meta['component_build_note']='Only the algebraic arithmetic module changed after the full-Falcon benchmark. The qualified full-Falcon executable is unchanged.'
meta['samples_by_case']={f'{f}-{n}':21 for f in ['full','arithmetic'] for n in [512,1024]}
meta['executables']['candidate']['arithmetic']['sha256']=hashlib.sha256(Path(meta['executables']['candidate']['arithmetic']['path']).read_bytes()).hexdigest()
meta_path.write_text(json.dumps(meta,indent=2)+'\n');(out/'metadata.json').write_text(json.dumps(meta,indent=2)+'\n')
summary=[]
for family in ['full','arithmetic']:
 for degree in [512,1024]:
  row={'family':family,'degree':degree}
  for r in all_records:
   if r['family']==family and r['degree']==degree:
    samples=r['samples'];row[r['revision']]={k:statistics.median(x[k] for x in samples) for k in samples[0] if isinstance(samples[0][k],(float,int)) and (k.endswith('_ms') or k.endswith('_bytes'))}
  summary.append(row)
(root/'measurements/summary.json').write_text(json.dumps(summary,indent=2)+'\n')

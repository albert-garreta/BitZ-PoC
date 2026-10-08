import hashlib, json, os, platform, statistics, subprocess, time
from pathlib import Path
ROOT=Path.cwd()
OUT=ROOT/'target/falcon-integer-merge/measurements'
OUT.mkdir(exist_ok=True)
HEAD='95b79124f1958e3efb9ca9078e330542665cb1c3'
executables={}
for revision in ['baseline','candidate']:
    release=ROOT/f'target/falcon-integer-{revision}/release'
    full=[p for p in (release/'deps').glob('falcon_hybrid-*') if p.is_file() and os.access(p,os.X_OK) and not p.suffix]
    assert len(full)==1, full
    executables[revision]={'full':str(full[0]), 'arithmetic':str(release/'examples/falcon_algebraic')}
metadata={'baseline_commit':HEAD,'host':platform.platform(),'machine':platform.machine(),
          'threads':8,'security':100,'batch':1024,'seed':42,'warmups':1,'samples_by_family':{'full':21,'arithmetic':5},
          'rustflags':'-C target-cpu=native','features':'falcon-hybrid',
          'candidate_patch_sha256':hashlib.sha256(subprocess.check_output(['git','diff','HEAD','--','src'])).hexdigest(),
          'integer_kernel_sha256':hashlib.sha256((ROOT/'src/piop/spartan/falcon_integer.rs').read_bytes()).hexdigest(),
          'grinding_policy_sha256':hashlib.sha256((ROOT/'src/piop/spartan/falcon/hybrid_grinding.rs').read_bytes()).hexdigest(),
          'executables':{r:{f:{'path':p,'sha256':hashlib.sha256(Path(p).read_bytes()).hexdigest()} for f,p in fs.items()} for r,fs in executables.items()}}
(OUT/'metadata.json').write_text(json.dumps(metadata,indent=2)+'\n')
env=os.environ.copy()
for k in list(env):
    if k.startswith(('BITZ_','FLOCK_')): del env[k]
env.update(RAYON_NUM_THREADS='8',NO_COLOR='1')
records=[]
for degree in [512,1024]:
    for family in ['arithmetic','full']:
        order=['baseline','candidate'] if degree==512 else ['candidate','baseline']
        for revision in order:
            name=f'{family}-{degree}-{revision}'
            count=21 if family=='full' else 5
            cmd=[executables[revision][family],'--degree',str(degree),'--batch','1024','--security','100','--threads','8','--seed','42','--warmup','1','--iterations',str(count)]
            print('START',name,flush=True)
            start=time.monotonic()
            with (OUT/f'{name}.stdout.log').open('w') as out, (OUT/f'{name}.stderr.log').open('w') as err:
                result=subprocess.run(cmd,cwd=ROOT,env=env,stdout=out,stderr=err)
            assert result.returncode==0,(name,result.returncode)
            items=[json.loads(x) for x in (OUT/f'{name}.stdout.log').read_text().splitlines() if x.startswith('{')]
            samples=[x for x in items if x.get('trial')=='sample']
            warmup=[x for x in items if x.get('trial')=='warmup']
            assert len(samples)==count and len(warmup)==1
            assert all(x['verified'] and x['threads']==8 and x['batch']==1024 for x in samples+warmup)
            record={'name':name,'revision':revision,'family':family,'degree':degree,'command':cmd,'elapsed_seconds':time.monotonic()-start,'samples':samples}
            records.append(record)
            (OUT/'results.json').write_text(json.dumps(records,indent=2)+'\n')
            print('DONE',name,round(record['elapsed_seconds'],1),'seconds',flush=True)
        pair=[x for x in records if x['family']==family and x['degree']==degree]
        assert len({s['input_digest'] for r in pair for s in r['samples']})==1
summary=[]
for family in ['full','arithmetic']:
    for degree in [512,1024]:
        row={'family':family,'degree':degree}
        for r in [x for x in records if x['family']==family and x['degree']==degree]:
            samples=r['samples']
            row[r['revision']]={k:statistics.median(x[k] for x in samples) for k in samples[0] if isinstance(samples[0][k],(float,int)) and (k.endswith('_ms') or k.endswith('_bytes'))}
        summary.append(row)
(OUT/'summary.json').write_text(json.dumps(summary,indent=2)+'\n')
print('COMPLETE',json.dumps(summary),flush=True)

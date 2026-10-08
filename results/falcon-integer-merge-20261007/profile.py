import json, os, re, subprocess, time
from pathlib import Path
ROOT=Path.cwd()
OUT=ROOT/'target/falcon-integer-merge/measurements'
meta=json.loads((OUT/'metadata.json').read_text())
env=os.environ.copy()
for k in list(env):
    if k.startswith(('BITZ_','FLOCK_')): del env[k]
env.update(RAYON_NUM_THREADS='8',NO_COLOR='1')
results=[]
for family in ['full']:
    for revision in ['baseline','candidate']:
        name=f'diagnostic-{family}-1024-{revision}'
        case=env.copy()
        cmd=[meta['executables'][revision][family]['path'],'--degree','1024','--batch','1024','--security','100','--threads','8','--seed','42','--warmup','1','--iterations','5']
        if family=='full': case['BITZ_FALCON_STAGE_TIMINGS']='1'
        else: cmd.append('--trace')
        print('START',name,flush=True)
        with (OUT/f'{name}.stdout.log').open('w') as out, (OUT/f'{name}.stderr.log').open('w') as err:
            result=subprocess.run(cmd,cwd=ROOT,env=case,stdout=out,stderr=err)
        assert result.returncode==0, name
        trials=[json.loads(s) for s in (OUT/f'{name}.stdout.log').read_text().splitlines() if s.startswith('{')]
        trials=[t for t in trials if t.get('trial') in ('warmup','sample')]
        assert len(trials)==6 and all(t['verified'] for t in trials)
        stages={}
        for line in (OUT/f'{name}.stderr.log').read_text().splitlines():
            if family=='full' and line.startswith('{'):
                x=json.loads(line)
                if x.get('event')=='stage': stages.setdefault(x['name'],[]).append(x['elapsed_ms'])
            elif family=='arithmetic':
                m=re.search(r'([\w]+:[\w]+)(?:\{[^}]*\})?: bitz::.*close time.busy=([0-9.]+)(µs|ns|ms|s)',line)
                if m:
                    val=float(m[2])*{'ns':1e-6,'µs':1e-3,'ms':1,'s':1000}[m[3]]
                    stages.setdefault(m[1],[]).append(val)
        # Duration-only diagnostic spans, not a timeline. Do not add parents to children.
        results.append({'name':name,'family':family,'revision':revision,'degree':1024,'command':cmd,'verified':True,'trials':trials,'stages_ms':stages})
        (OUT/'diagnostics.json').write_text(json.dumps(results,indent=2)+'\n')
        wanted={k:v for k,v in stages.items() if k in ['falcon_arithmetic:norm','falcon_arithmetic:hash_to_point_rejection','falcon_arithmetic:integer_outer','falcon_algebraic:norm','falcon_algebraic:integer_outer','falcon_algebraic:binding','falcon_arithmetic:binding','falcon_hybrid:arithmetic_prefix']}
        print('DONE',name,json.dumps(wanted),flush=True)
print('COMPLETE',flush=True)

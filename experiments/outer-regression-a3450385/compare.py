#!/usr/bin/env python3
"""Sequential AB/BA regression runs; match proof+transcript digests before timing analysis."""
import argparse, collections, csv, gzip, json, math, os, statistics, subprocess
from pathlib import Path
p=argparse.ArgumentParser()
p.add_argument('--baseline',required=True);p.add_argument('--current',required=True)
p.add_argument('--out',type=Path,required=True)
p.add_argument('--threads',type=int,default=10);p.add_argument('--reps',type=int,default=21)
p.add_argument('--shapes',default='12 15 17 19');p.add_argument('--widths',default='32 64 128')
p.add_argument('--protocols',default='ordinary skip-1 skip-2 skip-3 skip-4')
p.add_argument('--paired',action='store_true',help='Alternate revisions for each individual input/size/protocol case')
p.add_argument('--gate',choices=['report','no-regression','faster'],default='report')
p.add_argument('--generic',action='store_true');p.add_argument('--analyze-only',action='store_true')
a=p.parse_args();a.out.mkdir(parents=True,exist_ok=True)
names=['baseline','production']+(['generic'] if a.generic else [])
env=dict(os.environ,RAYON_NUM_THREADS=str(a.threads),OUTER_REPS=str(a.reps),OUTER_SHAPES=a.shapes,OUTER_WIDTHS=a.widths,OUTER_PROTOCOLS=a.protocols)
cases=[(w,n,protocol) for w in a.widths.split() for n in a.shapes.split() for protocol in a.protocols.split() if protocol=='ordinary' or int(n)>=int(protocol.split('-')[1])]
if not a.analyze_only:
 if a.paired:
  for session in (1,2):
   files={name:(a.out/f'{session}-{name}.log').open('w') for name in names}
   try:
    for i,(width,shape,protocol) in enumerate(cases):
     for name in names if session==1 else reversed(names):
      subprocess.run([a.baseline if name=='baseline' else a.current],env=dict(env,OUTER_VARIANTS='generic' if name=='generic' else 'production',OUTER_WIDTHS=width,OUTER_SHAPES=shape,OUTER_PROTOCOLS=protocol),stdout=files[name],stderr=subprocess.STDOUT,check=True)
     if (i+1)%10==0 or i+1==len(cases):print(f'Finished paired session {session}: {i+1}/{len(cases)} cases',flush=True)
   finally:
    for f in files.values():f.close()
 else:
  for session in (1,2):
   for name in names if session==1 else reversed(names):
    print(f'Starting session {session}: {name}',flush=True)
    with (a.out/f'{session}-{name}.log').open('w') as f:subprocess.run([a.baseline if name=='baseline' else a.current],env=dict(env,OUTER_VARIANTS='generic' if name=='generic' else 'production'),stdout=f,stderr=subprocess.STDOUT,check=True)
    print(f'Finished session {session}: {name}',flush=True)
values=collections.defaultdict(list);digests={}
for session in (1,2):
 for name in names:
  path=a.out/f'{session}-{name}.log'
  lines=path.read_text().splitlines() if path.exists() else gzip.open(str(path)+'.gz','rt').read().splitlines()
  for line in lines:
   if not line.startswith('OUTER_SAMPLE '):continue
   r=json.loads(line[13:]);key=(r['bits'],r['protocol'],r['rows'],r['threads'])
   assert r['verified']
   if key in digests:assert r['proof_digest']==digests[key],f'proof/transcript mismatch: {key}'
   digests[key]=r['proof_digest']
   if not r['warmup']:values[(session,name,*key)].append(int(r['ns']))
def quantile(v,q):
 v=sorted(v);i=(len(v)-1)*q;j=int(i);return v[j]+(v[min(j+1,len(v)-1)]-v[j])*(i-j)
expected={(session,name,int(w),protocol,1<<int(n),a.threads) for session in (1,2) for name in names for w,n,protocol in cases}
assert set(values)==expected, 'Incomplete or unexpected benchmark cases'
rows=[]
for key,v in sorted(values.items()):
 session,name,bits,protocol,n,threads=key
 if name=='baseline':continue
 b=values[(session,'baseline',bits,protocol,n,threads)]
 assert len(v)==len(b)==a.reps,(key,len(v),len(b))
 base=statistics.median(b);now=statistics.median(v)
 rows.append(dict(session=session,variant=name,bits=bits,protocol=protocol,rows=n,threads=threads,samples=len(v),baseline_ms=base/1e6,current_ms=now/1e6,baseline_p10_ms=quantile(b,.1)/1e6,baseline_p90_ms=quantile(b,.9)/1e6,current_p10_ms=quantile(v,.1)/1e6,current_p90_ms=quantile(v,.9)/1e6,saved_percent=100*(1-now/base),proof_digest=digests[(bits,protocol,n,threads)]))
with (a.out/'comparison.csv').open('w') as f:
 w=csv.DictWriter(f,fieldnames=list(rows[0]));w.writeheader();w.writerows(rows)
for name in names[1:]:
 for bits in sorted({r['bits'] for r in rows}):
  for group in ('ordinary','skip'):
   saved=[]
   for session in (1,2):
    rs=[r for r in rows if r['session']==session and r['variant']==name and r['bits']==bits and r['protocol'].startswith(group)]
    if rs:saved.append(round(100*(1-math.exp(statistics.mean(math.log(r['current_ms']/r['baseline_ms']) for r in rs))),2))
   if saved:print(name,bits,group,'latency saved %',saved)
first={(r['variant'],r['bits'],r['protocol'],r['rows'],r['threads']):r for r in rows if r['session']==1}
regressions=[];not_faster=[]
for r in rows:
 if r['session']!=2:continue
 prev=first[(r['variant'],r['bits'],r['protocol'],r['rows'],r['threads'])]
 if r['saved_percent'] < -3 and prev['saved_percent'] < -3:regressions.append(r)
 if min(r['saved_percent'],prev['saved_percent'])<5:not_faster.append(r)
(a.out/'gate.json').write_text(json.dumps({'matched_proof_cases':len(digests),'repeat_over_3_percent_regressions':regressions,'not_5_percent_faster_in_both_runs':not_faster},indent=2)+'\n')
print('Repeated >3% regressions:',len(regressions))
for r in regressions:print(r['variant'],r['bits'],r['protocol'],r['rows'],round(r['saved_percent'],2))
print('Not consistently >=5% faster:',len(not_faster),'of',len(rows)//2)

if a.gate=='no-regression' and regressions:raise SystemExit(1)
if a.gate=='faster' and not_faster:raise SystemExit(1)

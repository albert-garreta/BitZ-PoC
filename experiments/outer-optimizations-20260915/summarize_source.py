#!/usr/bin/env python3
import json,csv,statistics,collections,math,gzip,os
from pathlib import Path
root=Path(__file__).parent
rows=[]
for session in (1,2):
 values=collections.defaultdict(list)
 for name in ('original','final'):
  path=root/f'{os.environ.get("SOURCE_PREFIX", "source-fixed")}-{session}-{name}.log'
  lines=path.read_text().splitlines() if path.exists() else gzip.open(str(path)+'.gz','rt').read().splitlines()
  for line in lines:
   if line.startswith('OUTER_SAMPLE '):
    r=json.loads(line[13:])
    if not r['warmup']:values[(name,r['input'],r['protocol'],r['rows'],r['threads'])].append(int(r['ns']))
 for key,v in sorted(values.items()):
  name,kind,protocol,n,threads=key
  if name!='final':continue
  b=values[('original',*key[1:])]
  def q(v,p):
   v=sorted(v);i=(len(v)-1)*p;j=int(i);return v[j]+(v[min(j+1,len(v)-1)]-v[j])*(i-j)
  base=statistics.median(b);final=statistics.median(v)
  rows.append(dict(session=session,input=kind,protocol=protocol,rows=n,threads=threads,samples=len(v),original_ms=base/1e6,final_ms=final/1e6,original_p10_ms=q(b,.1)/1e6,original_p90_ms=q(b,.9)/1e6,final_p10_ms=q(v,.1)/1e6,final_p90_ms=q(v,.9)/1e6,saved_percent=100*(1-final/base)))
with (root/os.environ.get('SOURCE_OUTPUT', 'final-source.csv')).open('w') as f:
 w=csv.DictWriter(f,fieldnames=list(rows[0]));w.writeheader();w.writerows(rows)
for session in (1,2):
 for group in ('ordinary','zero','skip'):
  rs=[r for r in rows if r['session']==session and r['protocol'].startswith(group)]
  if not rs:continue
  print(session,group,'geomean saved %',round(100*(1-math.exp(statistics.mean(math.log(r['final_ms']/r['original_ms']) for r in rs))),2))
first={(r['input'],r['protocol'],r['rows']):r for r in rows if r['session']==1}
print('Repeated >3% regressions:')
for r in rows:
 if r['session']==2 and r['saved_percent'] < -3 and first[(r['input'],r['protocol'],r['rows'])]['saved_percent'] < -3:print(r)

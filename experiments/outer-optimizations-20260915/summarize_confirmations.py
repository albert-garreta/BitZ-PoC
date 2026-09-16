#!/usr/bin/env python3
import json,math,statistics,collections,csv,gzip
from pathlib import Path
root=Path(__file__).parent
allrows=[]
for session in (1,2):
 values=collections.defaultdict(list)
 for threads in (1,4,10):
  path=root/f'confirm-{session}-t{threads}.log'
  lines=path.read_text().splitlines() if path.exists() else gzip.open(str(path)+'.gz','rt').read().splitlines()
  for line in lines:
   if not line.startswith('OUTER_SAMPLE '):continue
   r=json.loads(line[13:])
   if not r['warmup']:values[(r['input'],r['protocol'],r['rows'],r['threads'],r['candidate'])].append(int(r['ns']))
 for (kind,protocol,rows,threads,candidate),samples in sorted(values.items()):
  if candidate=='baseline':continue
  baseline=values[(kind,protocol,rows,threads,'baseline')]
  def quantile(v,q):
   v=sorted(v);i=(len(v)-1)*q;j=int(i);return v[j]+(v[min(j+1,len(v)-1)]-v[j])*(i-j)
  median=statistics.median(samples);base=statistics.median(baseline)
  allrows.append(dict(session=session,input=kind,protocol=protocol,rows=rows,threads=threads,samples=len(samples),baseline_ms=base/1e6,candidate_ms=median/1e6,p10_ms=quantile(samples,.1)/1e6,p90_ms=quantile(samples,.9)/1e6,saved_percent=100*(1-median/base)))
with (root/'confirmation.csv').open('w') as f:
 w=csv.DictWriter(f,fieldnames=list(allrows[0]));w.writeheader();w.writerows(allrows)
for t in (1,4,10):
 for group in ('ordinary','zero','skip'):
  for session in (1,2):
   rs=[r for r in allrows if r['threads']==t and r['session']==session and r['protocol'].startswith(group)]
   saved=100*(1-math.exp(statistics.mean(math.log(r['candidate_ms']/r['baseline_ms']) for r in rs)))
   print(t,group,session,round(saved,2))
print('Repeated >3% regressions:')
first={(r['input'],r['protocol'],r['rows'],r['threads']):r for r in allrows if r['session']==1}
for r in allrows:
 if r['session']==2 and r['saved_percent'] < -3 and first[(r['input'],r['protocol'],r['rows'],r['threads'])]['saved_percent'] < -3:print(r)

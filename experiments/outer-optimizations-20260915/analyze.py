#!/usr/bin/env python3
import argparse,json,math,statistics,collections,gzip
from pathlib import Path
p=argparse.ArgumentParser();p.add_argument('logs',nargs='+');p.add_argument('--json',action='store_true');args=p.parse_args()
values=collections.defaultdict(list)
for filename in args.logs:
 lines = gzip.open(filename,"rt").read().splitlines() if filename.endswith(".gz") else Path(filename).read_text().splitlines()
 for line in lines:
  if not line.startswith('OUTER_SAMPLE '):continue
  r=json.loads(line[len('OUTER_SAMPLE '):])
  if not r['warmup']:values[(r['input'],r['protocol'],r['rows'],r['threads'],r['candidate'])].append(int(r['ns']))
ratios=collections.defaultdict(list)
rows=[]
for (kind,protocol,n,t,c),samples in sorted(values.items()):
 key=(kind,protocol,n,t,'baseline')
 if key not in values:continue
 median=statistics.median(samples); base=statistics.median(values[key]); ratio=median/base
 ratios[c].append(ratio)
 row=dict(input=kind,protocol=protocol,rows=n,threads=t,candidate=c,samples=len(samples),median_ms=median/1e6,baseline_ms=base/1e6,speedup=1/ratio,percent_saved=(1-ratio)*100)
 rows.append(row)
 if not args.json:print(f'{kind:5} {protocol:8} 2^{int(math.log2(n)):2} t={t:2} {c:30} {median/1e6:9.4f} ms {100*(1-ratio):+7.2f}%')
if args.json:print(json.dumps(rows,indent=2))
else:
 print('\nGROUP RATIOS:')
 for c,rs in ratios.items():print(f'{c:30} saved={100*(1-math.exp(sum(map(math.log,rs))/len(rs))):+.2f}% worst={100*(1-max(rs)):+.2f}% cases={len(rs)}')

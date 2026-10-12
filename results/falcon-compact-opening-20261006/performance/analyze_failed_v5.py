#!/usr/bin/env python3
import collections,json,math,statistics
from pathlib import Path
P=Path(__file__).resolve().parent/'v5/qualification'
METRICS=['total_prover_ms','proof_prove_ms','witness_commit_ms','proof_verify_ms']
records=[]
for seed in range(42,102):
 d={'seed':seed,'block':(seed-42)//15,'schedule_index':(seed-42)%4}
 for label in ['baseline','candidate']:
  stem=f'native-s128-b1-t16-seed{seed}-{label}'
  rows=[json.loads(x) for x in (P/f'{stem}.jsonl').read_text().splitlines()];run=json.loads((P/f'{stem}.run.json').read_text());samples=[r for r in rows if r.get('trial')=='sample']
  d[label]={metric:statistics.median(r[metric] for r in samples) for metric in METRICS}
  d[label]['process_elapsed_seconds']=run['elapsed_seconds'];d[label]['started_at']=run['started_at'];d[label]['trials']=[{metric:r[metric] for metric in METRICS} for r in samples]
  d['order']=run['order']
 d['candidate_first']=d['order'][0]=='candidate';d['delta_ms']=d['candidate']['total_prover_ms']-d['baseline']['total_prover_ms'];records.append(d)
def summarize(rs):
 out={}
 for metric in METRICS+['process_elapsed_seconds']:
  a=statistics.mean(r['baseline'][metric] for r in rs);b=statistics.mean(r['candidate'][metric] for r in rs)
  out[metric]={'baseline_mean':a,'candidate_mean':b,'delta':b-a,'ratio':b/a}
 out.update(pairs=len(rs),positive_prover_deltas=sum(r['delta_ms']>0 for r in rs),median_prover_delta_ms=statistics.median(r['delta_ms'] for r in rs))
 return out
summary={'all':summarize(records),'by_candidate_first':{str(k):summarize([r for r in records if r['candidate_first']==k]) for k in [False,True]},'chronological_blocks_15_seeds':{str(k):summarize([r for r in records if r['block']==k]) for k in range(4)},'by_schedule_index':{str(k):summarize([r for r in records if r['schedule_index']==k]) for k in range(4)}}
x=[r['delta_ms'] for r in records];y=[r['candidate']['witness_commit_ms']-r['baseline']['witness_commit_ms'] for r in records];xm=statistics.mean(x);ym=statistics.mean(y)
summary['pearson_prover_vs_commit_absolute_delta']=sum((a-xm)*(b-ym) for a,b in zip(x,y))/math.sqrt(sum((a-xm)**2 for a in x)*sum((b-ym)**2 for b in y))
result={'qualification':False,'scope':'Read-only diagnostics of all 60 retained Native128B1 seed pairs; no exclusions or replacement measurements. Only coarse commitment/proof stages exist in qualified logs; fine stage instrumentation is disabled.','summary':summary,'records':records}
(P.parent/'failed-cell-analysis.json').write_text(json.dumps(result,indent=2)+'\n')
print(json.dumps(summary,indent=2))

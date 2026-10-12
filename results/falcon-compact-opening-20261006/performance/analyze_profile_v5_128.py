#!/usr/bin/env python3
import collections,json,re,statistics
from pathlib import Path
P=Path(__file__).resolve().parent/'v5/repeated-profile128-seed42'
AGG={label:collections.defaultdict(list) for label in ['baseline','candidate']}
PER=[]
for index,label in enumerate(['baseline','candidate','candidate','baseline'],1):
 stem=f'{index}-{label}';rows=[json.loads(x) for x in (P/f'{stem}.jsonl').read_text().splitlines()]
 stages=collections.defaultdict(float);trials=[]
 for line in (P/f'{stem}.stderr').read_text().splitlines():
  if line.startswith('{'):
   row=json.loads(line)
   if row.get('event')=='stage':
    key='stage:'+row['path'][0]+'/'+row['name']
    stages[key]+=row['elapsed_ms']
    if row['name']=='falcon_bench:verify':
     assert row['fields']['trial']==len(trials)
     trials.append(dict(stages));stages=collections.defaultdict(float)
  else:
   match=re.search(r'\[(vco|lcv)\] (.*?):\s*([\d.]+) (µs|ms)',line)
   if match:stages[f'trace:{match[1]}:{match[2]}']+=float(match[3])*(.001 if match[4]=='µs' else 1)
   match=re.search(r'\[lig-verify\] total = ([\d.]+) ms',line)
   if match:stages['trace:lig:total']+=float(match[1])
   match=re.match(r'\s+(merkle multi-proofs|sample_distinct_queries|enforced_sum \(eq\+dot\)|residual basis eval|eval_b \(yr_len positions\)):\s*([\d.]+) ms',line)
   if match:stages['trace:lig:'+match[1]]+=float(match[2])
 assert len(trials)==33
 samples=rows[4:-1];assert len(samples)==30
 for t,row in zip(trials[3:],samples):
  for key in ['total_prover_ms','proof_verify_ms','witness_commit_ms','proof_prove_ms']:t[key]=row[key]
  for key,value in t.items():AGG[label][key].append(value)
 PER.append({'process':stem,'means':{key:statistics.mean(t[key] for t in trials[3:]) for key in trials[3]}})
keys=sorted(set(AGG['baseline'])|set(AGG['candidate']))
summary={}
for k in keys:
 a=AGG['baseline'][k];b=AGG['candidate'][k];assert len(a)==len(b)==60
 av=statistics.mean(a);bv=statistics.mean(b)
 summary[k]={'baseline_mean_ms':av,'candidate_mean_ms':bv,'delta_ms':bv-av,'ratio':bv/av if av else None,'observations_per_revision':60}
result={'qualification':False,'estimator':'pooled arithmetic means of 60 same-seed measured repeats per revision, 2 processes per revision, ABBA','warmups_per_process':3,'scope':'instrumented Native128B1 seed42; no confidence gate or multi-seed generalization','timings':summary,'processes':PER}
(P/'analysis.json').write_text(json.dumps(result,indent=2)+'\n')
for key in ['total_prover_ms','proof_verify_ms','witness_commit_ms','proof_prove_ms']+[k for k in keys if k.startswith('stage:falcon_bench:prove/falcon_hybrid:')]+[k for k in keys if k.startswith('trace:')]:
 print(key,summary[key])

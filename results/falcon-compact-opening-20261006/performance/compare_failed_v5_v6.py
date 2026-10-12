#!/usr/bin/env python3
"""Read-only descriptive comparison; preserves the accepted qualification gate."""
import collections,json,statistics
from pathlib import Path
P=Path(__file__).resolve().parent
METRICS=['total_prover_ms','proof_prove_ms','witness_commit_ms','proof_verify_ms']
D={}
for version in ['v5','v6']:
 rows=[]
 for seed in range(42,102):
  d={'seed':seed,'schedule_index':(seed-42)%4}
  for side in ['baseline','candidate']:
   stem=f'native-s128-b1-t16-seed{seed}-{side}';raw=[json.loads(x) for x in (P/version/'qualification'/f'{stem}.jsonl').read_text().splitlines()];run=json.loads((P/version/'qualification'/f'{stem}.run.json').read_text());trials=[x for x in raw if x.get('event')=='trial'];samples=[x for x in trials if x['trial']=='sample'];assert len(trials)==4 and len(samples)==3
   d[side]={'medians':{k:statistics.median(x[k] for x in samples) for k in METRICS},'trials':[{k:x[k] for k in METRICS} for x in trials],'proof_debug_digest':trials[0]['proof_debug_digest'],'source_root':trials[0]['source_root'],'input_digest':trials[0]['input_digest'],'grinding':trials[0]['grinding_diagnostics'],'started_at':run['started_at'],'elapsed_seconds':run['elapsed_seconds']}
   d['order']=run['order']
  rows.append(d)
 D[version]=rows
for a,b in zip(D['v5'],D['v6']):
 assert a['seed']==b['seed'] and a['order']==b['order']
 for side in ['baseline','candidate']:
  for k in ['proof_debug_digest','source_root','input_digest','grinding']:assert a[side][k]==b[side][k],(a['seed'],side,k)
def vec(v,k,side):return [x[side]['medians'][k] for x in D[v]]
def delta(v,k):return [x['candidate']['medians'][k]-x['baseline']['medians'][k] for x in D[v]]
def corr(x,y):return statistics.correlation(x,y)
def centered_by_schedule(xs):
 means={k:statistics.mean(xs[i] for i in range(60) if i%4==k) for k in range(4)}
 return [x-means[i%4] for i,x in enumerate(xs)]
def describe(xs):return {'mean':statistics.mean(xs),'median':statistics.median(xs),'min':min(xs),'max':max(xs)}
summary={'cross_campaign_correlations':{},'campaigns':{},'cross_campaign_exact_identity':{'seed_pairs':60,'variants':2,'fields':['proof_debug_digest','source_root','input_digest','grinding'],'status':'pass'}}
for k in METRICS:
 x=delta('v5',k);y=delta('v6',k)
 summary['cross_campaign_correlations'][k]={'baseline':corr(vec('v5',k,'baseline'),vec('v6',k,'baseline')),'candidate':corr(vec('v5',k,'candidate'),vec('v6',k,'candidate')),'paired_delta':corr(x,y),'paired_delta_after_centering_each_schedule_position':corr(centered_by_schedule(x),centered_by_schedule(y)),'positive_in_both':sum(a>0 and b>0 for a,b in zip(x,y)),'negative_in_both':sum(a<0 and b<0 for a,b in zip(x,y)),'different_sign':sum((a>0)!=(b>0) for a,b in zip(x,y))}
for version,rows in D.items():
 out={'sides':{},'estimators':{},'schedule_positions':{}}
 for side in ['baseline','candidate']:
  trials=[r[side]['trials'] for r in rows]
  med_indices=collections.Counter();large_samples=[];decreasing=0;commit_large=0
  for r in rows:
   t=r[side]['trials'][1:];vals=[x['total_prover_ms'] for x in t];med=statistics.median(vals);med_indices[vals.index(med)+1]+=1;decreasing+=vals[0]>vals[1]>vals[2]
   for idx,x in enumerate(t,1):
    if x['total_prover_ms']>1.1*med:large_samples.append({'seed':r['seed'],'sample':idx,'prover_ms':x['total_prover_ms'],'prover_over_process_median':x['total_prover_ms']/med,'commit_ms':x['witness_commit_ms'],'commit_over_process_median':x['witness_commit_ms']/r[side]['medians']['witness_commit_ms']})
  out['sides'][side]={'trial_means':{label:{k:statistics.mean(t[idx][k] for t in trials) for k in METRICS} for idx,label in enumerate(['warmup','sample1','sample2','sample3'])},'median_sample_index_counts':dict(med_indices),'strictly_decreasing_prover_samples_processes':decreasing,'samples_more_than_10pct_above_own_process_median':large_samples,'spike_threshold_is_descriptive_only':True,'paired_with_commit_above_10pct':sum(x['commit_over_process_median']>1.1 for x in large_samples),'prover_commit_correlation_all_measured_trials':corr([x['total_prover_ms'] for t in trials for x in t[1:]],[x['witness_commit_ms'] for t in trials for x in t[1:]])}
 for k in METRICS:
  base_med=statistics.mean(vec(version,k,'baseline'));cand_med=statistics.mean(vec(version,k,'candidate'));base_all=statistics.mean(x[k] for r in rows for x in r['baseline']['trials'][1:]);cand_all=statistics.mean(x[k] for r in rows for x in r['candidate']['trials'][1:]);out['estimators'][k]={'accepted_mean_of_seed_medians':{'baseline':base_med,'candidate':cand_med,'ratio':cand_med/base_med},'descriptive_mean_of_all_measured_trials_not_a_gate':{'baseline':base_all,'candidate':cand_all,'ratio':cand_all/base_all}}
 for pos in range(4):
  rs=[r for r in rows if r['schedule_index']==pos];out['schedule_positions'][str(pos)]={'order':rs[0]['order'],'pairs':len(rs),'prover_ratio':statistics.mean(r['candidate']['medians']['total_prover_ms'] for r in rs)/statistics.mean(r['baseline']['medians']['total_prover_ms'] for r in rs),'mean_prover_delta_ms':statistics.mean(r['candidate']['medians']['total_prover_ms']-r['baseline']['medians']['total_prover_ms'] for r in rs)}
 summary['campaigns'][version]=out
xs=delta('v5','total_prover_ms');ys=delta('v6','total_prover_ms');top5=sorted(range(60),key=lambda i:xs[i],reverse=True)[:10];top6=sorted(range(60),key=lambda i:ys[i],reverse=True)[:10];summary['top10_positive_delta_overlap']={'v5_seeds':[i+42 for i in top5],'v6_seeds':[i+42 for i in top6],'overlap':sorted(i+42 for i in set(top5)&set(top6))}
(P/'v6/v5-comparison.json').write_text(json.dumps({'qualification':False,'description':'Descriptive analysis only. No samples excluded and accepted median-based gate unchanged. All 60 seed pairs are included for both campaigns.','summary':summary,'data':D},indent=2)+'\n')
print(json.dumps({'correlations':summary['cross_campaign_correlations'],'overlap':summary['top10_positive_delta_overlap']},indent=2))
for v,c in summary['campaigns'].items():
 print(v,'estimators',c['estimators']['total_prover_ms'])
 for side,d in c['sides'].items():print(v,side,'medianindex',d['median_sample_index_counts'],'decreasing',d['strictly_decreasing_prover_samples_processes'],'spikes',len(d['samples_more_than_10pct_above_own_process_median']),'commitspikes',d['paired_with_commit_above_10pct'],'corr',d['prover_commit_correlation_all_measured_trials'])
 print('schedule',c['schedule_positions'])

#!/usr/bin/env python3
"""Summarize raw samples and inclusive span times; never add nested stages."""
from collections import defaultdict, Counter
import argparse, json, math, statistics
from pathlib import Path
OUT=Path(__file__).resolve().parent
median=statistics.median

def stats(values):
    return dict(median=median(values), min=min(values), max=max(values), count=len(values))

def main():
    parser=argparse.ArgumentParser()
    parser.add_argument('--profiles',default='profile-light',choices=['profile','profile-light'])
    args=parser.parse_args()
    grouped=defaultdict(list); profiles=defaultdict(list); seenroots=set()
    for p in sorted((OUT/'latency').glob('*/result.json')):
        d=json.loads(p.read_text());j=d['job'];key=(j['variant'],d['degree'],j['threads'])
        for t in d['trials']:
            if t['trial']=='sample': grouped[key].append(dict(t,seed=j['seed']))
    for p in sorted((OUT/args.profiles).glob('*/result.json')):
        d=json.loads(p.read_text());j=d['job'];key=(j['variant'],d['degree'],j['threads'])
        spans=[]
        for line in (p.parent/'stderr.log').read_text().splitlines():
            if line.startswith('{'):
                e=json.loads(line)
                if e.get('event')=='stage':spans.append(e)
        roots={}; indices=Counter(); names=(
            {'falcon_algebraic:commit':'commit','falcon_algebraic:prove':'prove','falcon_algebraic:verify':'verify'}
            if j['variant']=='arithmetic' else
            {'falcon_bench:commit':'commit','falcon_bench:prove':'prove','falcon_bench:verify':'verify'})
        for s in spans:
            if s['name'] in names and len(s['path'])==1:
                phase=names[s['name']]
                trial=s['fields'].get('trial',indices[phase])
                indices[phase]+=1
                assert s['span_id'] not in roots, 'reused root ID'
                roots[s['span_id']]=(phase,trial)
        assert indices=={'commit':4,'prove':4,'verify':4}, (p,indices)
        pertrial=defaultdict(lambda:defaultdict(float)); counts=defaultdict(Counter)
        for s in spans:
            matching=[roots[i] for i in s['ancestor_ids'] if i in roots]
            if not matching:continue
            assert len(matching)==1
            phase,trial=matching[0]
            pertrial[trial][phase+'|'+s['name']]+=s['elapsed_ms']
            counts[trial][phase+'|'+s['name']]+=1
        samples=[t for t in d['trials'] if t['trial']=='sample']
        for i,t in enumerate(samples,1):
            assert counts[i]['prove|falcon_bridge:wfbitz_forest']==1,(p,i,counts[i])
            profile=dict(seed=j['seed'],iteration=i,stages=dict(pertrial[i]),counts=dict(counts[i]),timings=t)
            profile['gkr_fraction_of_proof']=pertrial[i]['prove|falcon_bridge:wfbitz_forest']/t['prove_ms']
            profile['bridge_fraction_of_proof']=pertrial[i][
                'prove|'+('falcon_algebraic:bitz' if j['variant']=='arithmetic' else 'falcon_hybrid:binary_bridge')]/t['prove_ms']
            profiles[key].append(profile)
    rows=[]
    for key,trials in sorted(grouped.items()):
        variant,degree,threads=key
        row=dict(variant=variant,degree=degree,threads=threads,samples=len(trials))
        metrics=['witness_commit_ms','prove_ms','total_prover_ms','verify_ms','proof_payload_bytes','process_peak_rss_kib']
        row['latency']={m:stats([t[m] for t in trials]) for m in metrics}
        row['seed_medians']={str(seed):{m:median([t[m] for t in trials if t['seed']==seed]) for m in metrics} for seed in sorted({t['seed'] for t in trials})}
        if key in profiles:
            ps=profiles[key]
            names=set.intersection(*(set(p['stages']) for p in ps))
            row['stages']={name:stats([p['stages'][name] for p in ps]) for name in sorted(names)}
            row['profile_samples']=len(ps)
            row['profile_latency']={m:stats([p['timings'][m] for p in ps]) for m in ['witness_commit_ms','prove_ms','total_prover_ms','verify_ms']}
            row['gkr_fraction_of_proof']=stats([p['gkr_fraction_of_proof'] for p in ps])
            row['bridge_fraction_of_proof']=stats([p['bridge_fraction_of_proof'] for p in ps])
            row['profile_prove_overhead_ratio']=row['profile_latency']['prove_ms']['median']/row['latency']['prove_ms']['median']
        rows.append(row)
    summary=dict(profile_directory=args.profiles, definition='Median across 21 untraced trials per cell; stages are inclusive diagnostic medians across 9 traced trials. Warmups excluded.',rows=rows)
    (OUT/'summary.json').write_text(json.dumps(summary,indent=2)+'\n')
    (OUT/'profile-trials.json').write_text(json.dumps({str(k):v for k,v in profiles.items()},indent=2)+'\n')
    for r in rows:
        v=r['latency']; g=r.get('stages',{}).get('prove|falcon_bridge:wfbitz_forest',{}).get('median',float('nan'))
        print(f"{r['variant']:10} N={r['degree']:4} t={r['threads']:2} commit={v['witness_commit_ms']['median']:8.2f} proof={v['prove_ms']['median']:8.2f} total={v['total_prover_ms']['median']:8.2f} verify={v['verify_ms']['median']:7.2f} GKR={g:7.2f} profile/proof={r.get('profile_prove_overhead_ratio',float('nan')):.3f}")
if __name__=='__main__': main()

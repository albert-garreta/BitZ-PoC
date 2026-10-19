#!/usr/bin/env python3
"""summarize.py <tag> [<tag>...]: per-prove span ms from t1-<tag>.err and prove stats from t1/t10-<tag>.out"""
import json, sys, collections, statistics
KEY = ['falcon_algebraic:prove','falcon_algebraic:ring','falcon_keyed_ring:weights','falcon_keyed_ring:quotient',
       'falcon_algebraic:arithmetic','falcon_algebraic:integer_outer','falcon_algebraic:binding',
       'falcon_algebraic:binding_construction','falcon_algebraic:word_initial_sums','falcon_algebraic:word_first_fold',
       'falcon_algebraic:word_dense_fold','falcon_algebraic:word_round_grinding','falcon_integer:round_boundary',
       'falcon_algebraic:merged_norm','falcon_algebraic:merged_sumcheck',
       'falcon_algebraic:bitz','falcon_bridge:wfbitz_forest','falcon_algebraic:pcs','falcon_algebraic:commit','falcon_algebraic:verify']
def spans(path):
    tot=collections.Counter(); cnt=collections.Counter()
    for line in open(path):
        line=line.strip()
        if not line.startswith('{'): continue
        try: d=json.loads(line)
        except Exception: continue
        n=d.get('name'); ms=d.get('elapsed_ms')
        if n and ms is not None: tot[n]+=float(ms); cnt[n]+=1
    return tot,cnt
def results(path):
    out=[]
    for line in open(path):
        line=line.strip()
        if line.startswith('{'):
            try: out.append(json.loads(line))
            except Exception: pass
    return out
tags=sys.argv[1:]
rows={}
for tag in tags:
    tot,cnt=spans(f't1-{tag}.err'); n=cnt.get('falcon_algebraic:prove',1)
    rows[tag]={k:tot.get(k,0)/n for k in KEY}
    grind=sum(v for k,v in tot.items() if 'grind' in k and 'verify' not in k)/n
    rows[tag]['grinding(all prover sites)']=grind
    r1=[x for x in results(f't1-{tag}.out') if x.get('trial')=='sample']
    r10=[x for x in results(f't10-{tag}.out') if x.get('trial')=='sample']
    rows[tag]['t1 prove_ms (median)']=statistics.median(x['prove_ms'] for x in r1) if r1 else 0
    rows[tag]['t1 total_prover_ms']=statistics.median(x['total_prover_ms'] for x in r1) if r1 else 0
    rows[tag]['t10 prove_ms (median)']=statistics.median(x['prove_ms'] for x in r10) if r10 else 0
    rows[tag]['t10 total_prover_ms']=statistics.median(x['total_prover_ms'] for x in r10) if r10 else 0
    rows[tag]['t10 verify_ms']=statistics.median(x['verify_ms'] for x in r10) if r10 else 0
    rows[tag]['proof bytes']=r10[0]['proof_payload_bytes'] if r10 else 0
    rows[tag]['digest']=r10[0]['proof_debug_digest'][:12] if r10 else ''
keys=list(rows[tags[0]].keys())
print(f"{'':44s}"+''.join(f'{t:>14s}' for t in tags))
for k in keys:
    vals=[rows[t][k] for t in tags]
    if isinstance(vals[0],str): print(f'{k:44s}'+''.join(f'{v:>14s}' for v in vals)); continue
    if all(v==0 for v in vals): continue
    print(f'{k:44s}'+''.join(f'{v:14.1f}' for v in vals))

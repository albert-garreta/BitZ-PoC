#!/usr/bin/env python3
"""Render separate acceptance tables, preserving initial and confirmation runs."""
import argparse,csv,json
from pathlib import Path
p=argparse.ArgumentParser();p.add_argument('campaign',type=Path);a=p.parse_args()
rows=json.loads((a.campaign/'acceptance.json').read_text())
meta=json.loads((a.campaign/'metadata.json').read_text())
lines=['# Outer sumcheck acceptance','',f"Master: `{meta['master_revision']}`.",f"Pre-optimization reference: `{meta['preoptimization']}`.",'',f"Master executable SHA-256: `{meta['sha256']['master']}`.",f"Current executable SHA-256: `{meta['sha256']['current']}`.",'','A cell passes only when both session latency ratios are at most 0.95.','All latencies below are medians. Initial failures remain visible.','']
for requirement,title in [('ordinary_vs_master','Generic ordinary / master ordinary'),('skip_vs_ordinary','Generic K=3 / generic ordinary')]:
    selected=[r for r in rows if r['requirement']==requirement]
    lines += ['## '+title,'']
    if not selected:
        lines += ['Not run.',''];continue
    with (a.campaign/(requirement+'.csv')).open('w') as f:
        w=csv.DictWriter(f,fieldnames=list(selected[0]));w.writeheader();w.writerows(selected)
    for stage in ['initial','confirmation']:
        group=[r for r in selected if r['stage']==stage]
        if not group:continue
        lines += [f'### {stage.title()}','',f"{sum(r['passes'] for r in group)}/{len(group)} cells pass both sessions.",'','| Width | Rows | Threads | Session 0 numerator / denominator (ms) | Ratio | Session 1 numerator / denominator (ms) | Ratio | Pass |','|---|---:|---:|---:|---:|---:|---:|:---:|']
        for r in sorted(group,key=lambda x:(x['threads'],x['bits'],x['exponent'])):
            lines += [f"| u{r['bits']} | 2^{r['exponent']} | {r['threads']} | {r['session0_numerator_ns']/1e6:.4f} / {r['session0_denominator_ns']/1e6:.4f} | {r['session0_ratio']:.3f} | {r['session1_numerator_ns']/1e6:.4f} / {r['session1_denominator_ns']/1e6:.4f} | {r['session1_ratio']:.3f} | {'yes' if r['passes'] else '**no**'} |"]
        lines += ['']
(a.campaign/'REPORT.md').write_text('\n'.join(lines)+'\n')

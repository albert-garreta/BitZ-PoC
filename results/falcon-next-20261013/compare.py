#!/usr/bin/env python3
"""Report completed phases; no pooling across phases/modes/configurations."""
import json, statistics, sys
from pathlib import Path
OUT=Path(__file__).resolve().parent
rows=json.loads((OUT/'summary.json').read_text())['rows']
phase=sys.argv[1]
control=sys.argv[2] if len(sys.argv)>2 else 'control'
selected=[r for r in rows if r['phase']==phase]
print('phase',phase,'comparison',control)
for r in selected:
    if r['variant']==control:continue
    ref=next((b for b in selected if b['variant']==control and all(b[k]==r[k] for k in ['degree','threads','mode','trace'])),None)
    if ref is None:continue
    key=lambda m:r['medians'][m]
    ratios=lambda m:100*(key(m)/ref['medians'][m]-1)
    seeds=[100*(v['total_prover_ms']/ref['seed_medians'][s]['total_prover_ms']-1) for s,v in r['seed_medians'].items()]
    peaks=[v['process_peak_rss_kib']['median']/1024 for v in [ref,r]]
    def lifecycle(row):
        samples=[]
        for f in (OUT/'runs'/phase).glob('*/result.json'):
            x=json.loads(f.read_text());j=x['job']
            if (j['config'],j['degree'],j['threads'],j['mode'])==(row['variant'],row['degree'],row['threads'],row['mode']):
                samples.extend(t['total_prover_ms']+t['verify_ms'] for t in x['trials'] if t['trial']=='sample')
        return statistics.median(samples)
    before,after=lifecycle(ref),lifecycle(r)
    print(f"N={r['degree']} t={r['threads']} {r['mode']:8} {r['variant']:15} total {ref['medians']['total_prover_ms']:.3f}->{key('total_prover_ms'):.3f} ({ratios('total_prover_ms'):+.2f}%) seeds {','.join(f'{s:+.2f}' for s in seeds)} verify {ref['medians']['verify_ms']:.3f}->{key('verify_ms'):.3f} ({ratios('verify_ms'):+.2f}%) peakMiB {peaks[0]:.2f}->{peaks[1]:.2f} lifecycle {before:.3f}->{after:.3f} ({100*(after/before-1):+.2f}%)")

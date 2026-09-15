#!/usr/bin/env python3
"""Check that profiler-rendered medians match the separately analyzed raw samples."""
import argparse
import json
import math
from pathlib import Path

parser=argparse.ArgumentParser()
parser.add_argument('family',choices=('binius-focused','all-provers'))
parser.add_argument('report_directory',type=Path)
args=parser.parse_args()
root=Path(__file__).resolve().parent
analysis=json.loads((root/'analysis.json').read_text())
summary=json.loads((args.report_directory/'summary.json').read_text())
lookup={(r['backend'],r['log_inv_rate'],r['log_multiplications']):r
        for r in analysis['native'] if r['family']==args.family}
pairs={'proving':'witness_to_proof_ms','witness-generation':'witness_ms','commit':'commit_ms',
       'constraint-proof':'piop_ms','opening-proof':'opening_ms','pcs':'pcs_ms','verification':'verify_ms'}
checks=0
seen=set()
for series in summary['series']:
    cfg=series['parameters']['security']
    rate=cfg.get('log_inv_rate')
    if rate is None and 'ligerito' in cfg:
        rate=cfg['ligerito']['configuration']['levels'][0]['log_inv_rate']
    key=(series['benchmark']['implementation'],rate,series['parameters']['input']['log_multiplications'])
    assert key not in seen, ('duplicate plotted case',key)
    seen.add(key)
    row=lookup[key]
    assert series['measuredN']==5 and series['warmupN']==1
    for phase,metric in pairs.items():
        observed=float(series['metrics'][phase]['medianMsExact'])
        assert math.isclose(observed,row[metric],rel_tol=1e-10,abs_tol=1e-6),(key,phase,observed,row[metric])
        checks+=1
assert seen==set(lookup), 'plotted/analyzed case coverage differs'
result={'status':'pass','family':args.family,'series':len(seen),'metric_medians_checked':checks,
        'comparison':'Canonical trace union medians agree with original sample-metric medians; warmups excluded.'}
(args.report_directory/'metric-validation.json').write_text(json.dumps(result,indent=2)+'\n')
print(json.dumps(result))

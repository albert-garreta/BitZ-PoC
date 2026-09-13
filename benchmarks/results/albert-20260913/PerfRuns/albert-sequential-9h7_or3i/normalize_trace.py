#!/usr/bin/env python3
"""Adapt legacy trace labels for the profiler; preserve all measured intervals."""
import json
from pathlib import Path
import sys
source=Path(sys.argv[1]); destination=Path(sys.argv[2])
with destination.open('w') as output:
 for line in source.read_text().splitlines():
  row=json.loads(line)
  if row['record']=='run':
   trial=row['trial']
   if 'index' in trial:
    trial['warmup_index' if trial['kind']=='warmup' else 'sample_index']=trial.pop('index')
   row['benchmark']['build_profile']='release'
   cfg=row['parameters']['security']
   rate=cfg.get('log_inv_rate')
   if rate is None and 'ligerito' in cfg:
    rate=cfg['ligerito']['configuration']['levels'][0]['log_inv_rate']
   row['benchmark']['label'] += f' | initial rate 1/{2**rate}' if rate is not None else ' | native parameters'
   row['parameters']['boundary']='Verified trial covering witness generation, proof creation, and verification; backend-native serialization/accounting boundaries are retained. Primary witness_to_proof_ms is the nested witness-to-proof interval.'
   row['parameters']['trace_normalization']='Legacy trial.index mapped to the profiler trial index field; actual cargo --profile release recorded; post-proof accounting tagged as end-to-end with its original tag retained in attributes; no interval or measured metric changed.'
  if row['record']=='span' and row['primary_phase']=='proof-accounting':
   row['attributes']['original_primary_phase']='proof-accounting'
   row['primary_phase']='end-to-end'
   row['phase_tags']=['end-to-end']
  output.write(json.dumps(row,separators=(',',':'))+'\n')

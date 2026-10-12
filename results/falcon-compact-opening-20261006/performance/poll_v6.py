#!/usr/bin/env python3
"""Bounded file polling after UI stream loss; never starts a benchmark."""
import argparse,json,time
from pathlib import Path
p=argparse.ArgumentParser();p.add_argument('--known-cells',type=int,required=True);p.add_argument('--seconds',type=float,default=50);a=p.parse_args()
d=Path(__file__).resolve().parent/'v6/qualification';end=time.monotonic()+a.seconds
while True:
 try:m=json.loads((d/'manifest.json').read_text());s=json.loads((d/'summary.json').read_text())
 except json.JSONDecodeError:time.sleep(1);continue
 cells=[c for c in s['cells'] if len(c['seeds'])==60]
 bad=[c for c in cells if not c['payload']['pass_gate'] or any(c['timings'][k]['status']!='pass' for k in ['total_prover_ms','proof_verify_ms'])]
 if bad and m['status']=='running' and not (d/'STOP').exists():
  (d/'STOP').write_text('Resumed file monitor detected a completed cell failing the unchanged gate: '+str([(c['protocol'],c['security'],c['batch']) for c in bad])+'\n')
 if m['status']!='running' or len(cells)>a.known_cells or time.monotonic()>=end:
  print(json.dumps({k:m.get(k) for k in ['status','completed_pairs','active_case','active_seed','qualification_status']}))
  for c in cells[a.known_cells:]:
   print('CELL',json.dumps({'case':[c[k] for k in ['protocol','security','batch']],'timings':{k:{n:c['timings'][k][n] for n in ['ratio','upper_one_sided_95','status']} for k in ['total_prover_ms','proof_verify_ms']},'payload':c['payload']['ratio_of_mean_bytes']}))
  break
 time.sleep(5)

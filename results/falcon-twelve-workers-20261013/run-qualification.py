#!/usr/bin/env python3
"""Run after the production build and tests finish; phases run serially."""
from pathlib import Path
import subprocess
import sys
ROOT=Path(__file__).resolve().parents[2]
OUT=Path(__file__).resolve().parent
base=[sys.executable,str(ROOT/'scripts/falcon_worker_optimization.py'),'--output',str(OUT),'--baseline',str(ROOT/'.tmp/falcon-forest-pcs-binding-20261013/candidate-production'),'--binary',str(ROOT/'.tmp/falcon-twelve-workers-20261013/candidate-production'),'--configs',str(OUT/'production-configs.json')]
phases=[('production',['--variants','baseline','production','production-ccd0-smt','--threads','12']),
        ('regression',['--variants','baseline','production','--threads','1','8','16','--modes','repeated']),
        ('profiles',['--variants','baseline','production','production-ccd0-smt','--threads','8','12','16','--modes','repeated','--trace','--warmup','1','--iterations','3'])]
for batch in [3,129]:
 for security in [100,128]:
  phases.append((f'correctness-b{batch}-s{security}',['--variants','baseline','production','--threads','12','--batch',str(batch),'--security',str(security),'--modes','cold','--seeds','42','--cold-repetitions','1']))
for phase,args in phases:
 command=base+['--phase',phase]+args
 print('RUN',phase,flush=True)
 with (OUT/f'{phase}-run.log').open('w') as stream:
  code=subprocess.call(command,cwd=ROOT,stdout=stream,stderr=subprocess.STDOUT)
 print('DONE',phase,code,flush=True)
 if code:
  print((OUT/f'{phase}-run.log').read_text()[-5000:]);sys.exit(code)

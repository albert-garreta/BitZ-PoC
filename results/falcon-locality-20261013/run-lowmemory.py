#!/usr/bin/env python3
import json
from pathlib import Path
import subprocess
import sys
import time
ROOT=Path(__file__).resolve().parents[2]
OUT=Path(__file__).resolve().parent
while not (OUT/'candidate-lowmemory-binary.json').exists() or not (OUT/'test-lowmemory-exit.json').exists():
    time.sleep(1)
assert json.loads((OUT/'test-lowmemory-exit.json').read_text())['exit_code']==0
base=[sys.executable,str(ROOT/'scripts/falcon_locality_optimization.py'),
      '--output',str(OUT),'--binary',str(ROOT/'.tmp/falcon-locality-20261013/candidate-lowmemory'),
      '--baseline',str(ROOT/'.tmp/falcon-twelve-workers-20261013/candidate-production'),
      '--variants','baseline','production']
phases=[
    ('lowmem1',['--threads','12','--modes','cold','repeated']),
    ('lowmem2',['--threads','12','--modes','cold','repeated']),
    ('lowmem-regress',['--threads','1','8','16','--modes','repeated']),
    ('lowmem-profile',['--threads','12','--modes','repeated','--warmup','1','--iterations','3','--trace']),
    ('lowmem-padding',['--threads','12','--modes','cold','--cold-repetitions','1','--batch','513','--security','128']),
    ('lowmem-security100',['--threads','12','--modes','cold','--cold-repetitions','1','--batch','1024','--security','100']),
]
for phase,extra in phases:
    cmd=base+['--phase',phase]+extra
    (OUT/f'{phase}-command.json').write_text(json.dumps(cmd,indent=2)+'\n')
    print('START',phase,flush=True)
    with (OUT/f'{phase}-console.log').open('w') as log:
        rc=subprocess.call(cmd,cwd=ROOT,stdout=log,stderr=subprocess.STDOUT)
    print('EXIT',phase,rc,flush=True)
    if rc:
        print((OUT/f'{phase}-console.log').read_text()[-6000:],flush=True)
        sys.exit(rc)
print('ALL LOWMEMORY PHASES COMPLETE',flush=True)

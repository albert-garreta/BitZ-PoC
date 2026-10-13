#!/usr/bin/env python3
import json
from pathlib import Path
import subprocess
import sys
import time
ROOT=Path(__file__).resolve().parents[2]
OUT=Path(__file__).resolve().parent
# The v1 serial campaign writes its summary only after the last phase's prover
# processes and complete proof/provenance validation have finished.
while True:
    try:
        rows=json.loads((OUT/'summary.json').read_text())['rows']
        if len([r for r in rows if r['phase']=='padding-b513-s128']) == 4:
            break
    except (FileNotFoundError, json.JSONDecodeError):
        pass
    time.sleep(1)
cmd=[sys.executable,str(OUT/'build-candidate.py'),'candidate-final','--production']
print('START final build after v1 timings',flush=True)
raise SystemExit(subprocess.call(cmd,cwd=ROOT))

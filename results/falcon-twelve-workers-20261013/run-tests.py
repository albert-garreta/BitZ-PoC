#!/usr/bin/env python3
import json, os, subprocess, sys, time
from pathlib import Path
ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0,str(ROOT/'scripts'))
from bench_support import clean_environment, write_json
OUT = Path(__file__).resolve().parent
PREFIX=['cargo','test','--locked','--features','falcon-hybrid,bench-internals','--lib']
commands=[('falcon',PREFIX+['falcon','--','--test-threads=1'])]
env=clean_environment(os.environ);env.pop("FZF_DEFAULT_OPTS",None);env.pop("CARGO_ENCODED_RUSTFLAGS",None)
overrides=dict(CARGO_TARGET_DIR=str(ROOT/'.tmp/falcon-poc-push-target'),CARGO_PROFILE_TEST_OPT_LEVEL='2',RUSTFLAGS='-C target-cpu=native',RAYON_NUM_THREADS='8',CARGO_BUILD_JOBS='16',TMPDIR=str(ROOT/'.tmp/falcon-twelve-workers-20261013/build-tmp'))
env.update(overrides)
write_json(OUT/'tests-command.json',dict(commands=commands,overrides=overrides,experiment_environment_cleared=True,source_snapshot='candidate-production-source.json'))
results=[]
for name,command in commands:
 start=time.time()
 with (OUT/f'test-{name}.log').open('w') as stream:
  code=subprocess.call(command,cwd=ROOT,env=env,stdout=stream,stderr=subprocess.STDOUT)
 result=dict(name=name,exit_code=code,elapsed_seconds=time.time()-start);results.append(result)
 write_json(OUT/'tests-results.json',results)
 print(result,flush=True)
 if code:
  print((OUT/f'test-{name}.log').read_text()[-12000:]);sys.exit(code)

#!/usr/bin/env python3
"""Sequential checks; never invoke alongside timed campaigns."""
import os, subprocess, sys
from pathlib import Path
ROOT=Path(__file__).resolve().parents[2]
sys.path.insert(0,str(ROOT/'scripts'))
from falcon_next_optimization import subprocess_env
from bench_support import write_json
OUT=Path(__file__).resolve().parent
mode=sys.argv[1]
env=subprocess_env({})
overrides=dict(CARGO_TARGET_DIR=str(ROOT/'.tmp/falcon-poc-push-target'),CARGO_PROFILE_TEST_OPT_LEVEL='2',RUSTFLAGS='-C target-cpu=native',RAYON_NUM_THREADS='8',CARGO_BUILD_JOBS='16',TMPDIR=str(ROOT/'.tmp/falcon-next-20261013/build-tmp'))
if mode=='native':
    commands=[['cargo','test','--locked','--features','falcon-hybrid,bench-internals','--lib',name,'--','--test-threads=1'] for name in ['falcon','bitz::']]
elif mode=='portable':
    overrides.update(CARGO_TARGET_DIR=str(ROOT/'.tmp/falcon-ring-portable-target'),RUSTFLAGS='-C target-cpu=x86-64')
    commands=[['cargo','test','--locked','--features','falcon-hybrid,bench-internals','--lib','factored_','--','--test-threads=1'],['cargo','check','--locked','--no-default-features','--features','hybrid','--lib']]
elif mode=='production':
    commands=[['cargo','test','--locked','--features','falcon-hybrid','--lib',name,'--','--test-threads=1'] for name in ['falcon','bitz::']]
else:raise ValueError(mode)
env.update(overrides)
write_json(OUT/f'check-{mode}-commands.json',dict(commands=commands,overrides=overrides))
for i,cmd in enumerate(commands):
    path=OUT/f'check-{mode}-{i}.log'
    print('RUN',cmd,flush=True)
    with path.open('w') as log:code=subprocess.call(cmd,cwd=ROOT,env=env,stdout=log,stderr=subprocess.STDOUT)
    print('EXIT',code,path,flush=True)
    if code: print(path.read_text()[-12000:]);sys.exit(code)
    print('\n'.join(path.read_text().splitlines()[-5:]),flush=True)

#!/usr/bin/env python3
import os, subprocess, json, sys, time
from pathlib import Path
ROOT=Path(__file__).resolve().parents[2]
sys.path.insert(0,str(ROOT/'scripts'))
from bench_support import clean_environment
out=Path(__file__).resolve().parent
while not (out/'candidate-accepted-source.json').exists(): time.sleep(1)
env=clean_environment(os.environ)
for k in ['FZF_DEFAULT_OPTS','CARGO_ENCODED_RUSTFLAGS']: env.pop(k,None)
overrides={'CARGO_TARGET_DIR':str(ROOT/'.tmp/falcon-poc-push-target'),'CARGO_PROFILE_TEST_OPT_LEVEL':'2','RUSTFLAGS':'-C target-cpu=native','RAYON_NUM_THREADS':'8','CARGO_BUILD_JOBS':'16','TMPDIR':str(ROOT/'.tmp/falcon-locality-20261013/build-tmp')}
env.update(overrides)
cmd=['cargo','test','--locked','--features','falcon-hybrid','--lib','falcon_integer','--','--test-threads=1']
(out/'test-accepted-command.json').write_text(json.dumps({'command':cmd,'overrides':overrides},indent=2)+'\n')
with (out/'test-accepted.log').open('w') as log: rc=subprocess.call(cmd,cwd=ROOT,env=env,stdout=log,stderr=subprocess.STDOUT)
print('accepted norm tests exit',rc,flush=True)
(out/'test-accepted-exit.json').write_text(json.dumps({'exit_code':rc})+'\n')
if rc: print((out/'test-accepted.log').read_text()[-6000:])
raise SystemExit(rc)

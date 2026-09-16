#!/usr/bin/env python3
"""Build serially and freeze exact Cargo-reported benchmark artifacts."""
import argparse, hashlib, json, os, shutil, subprocess
from pathlib import Path
ROOT=Path(__file__).resolve().parents[2]
p=argparse.ArgumentParser()
p.add_argument('--output',type=Path,required=True)
p.add_argument('--revision',choices=['master','current'],required=True)
a=p.parse_args(); out=a.output.resolve(); out.mkdir(parents=True,exist_ok=False)
if a.revision=='master':
    cwd=ROOT/'bench_results/outer-generic-master-20260916/master-source'
    target=ROOT/'bench_results/outer-generic-master-20260916/target-master'
    bench='outer_master'
else:
    cwd=ROOT; target=ROOT/'target'; bench='outer_regression'
env={k:v for k,v in os.environ.items() if not k.startswith(('RAYON_','OUTER_','F2Z_','CARGO_PROFILE_')) and k!='CARGO_ENCODED_RUSTFLAGS'}
env.update(RUSTFLAGS='-C target-cpu=native',CARGO_TARGET_DIR=str(target),CARGO_BUILD_JOBS='4')
cmd=['cargo','+1.98.1','bench','--offline','--locked','--bench',bench,'--features','bench-internals','--no-run','--message-format=json-render-diagnostics']
# Capture sources before compiling; no inference from artifact mtimes.
(out/'working.diff').write_bytes(subprocess.check_output(['git','diff','HEAD'],cwd=ROOT))
source_files=[path for path in subprocess.check_output(['git','ls-files','--others','--exclude-standard'],cwd=ROOT,text=True).splitlines()
              if path.endswith(('.rs','.py','.md','.json'))]
for path in source_files:
    dest=out/'new-sources'/path; dest.parent.mkdir(parents=True,exist_ok=True); shutil.copyfile(ROOT/path,dest)
with (out/'cargo.jsonl').open('w') as stdout,(out/'cargo.log').open('w') as stderr:
    subprocess.run(cmd,cwd=cwd,env=env,stdout=stdout,stderr=stderr,check=True)
artifacts=[v for line in (out/'cargo.jsonl').read_text().splitlines() if (v:=json.loads(line)).get('reason')=='compiler-artifact' and v.get('target',{}).get('name')==bench and v.get('executable')]
assert len(artifacts)==1,artifacts
exe=out/'outer'; shutil.copy2(artifacts[0]['executable'],exe)
meta=dict(command=cmd,cwd=str(cwd),target=str(target),rustflags=env['RUSTFLAGS'],revision=a.revision,source_revision=('ac0aa44c5785db30f889fce8c5cdc98264a0e686' if a.revision=='master' else 'f86193f1dcbb43a4bba48ea20670914df334f750'),harness_base_head=subprocess.check_output(['git','rev-parse','HEAD'],cwd=ROOT,text=True).strip(),sha256=hashlib.sha256(exe.read_bytes()).hexdigest(),artifact=artifacts[0])
(out/'metadata.json').write_text(json.dumps(meta,indent=2)+'\n')
print(exe,flush=True)

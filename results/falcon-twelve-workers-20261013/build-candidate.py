#!/usr/bin/env python3
import argparse, hashlib, os, shutil, subprocess, sys
from pathlib import Path
ROOT=Path(__file__).resolve().parents[2]
sys.path.insert(0,str(ROOT/'scripts'))
from bench_support import clean_environment,write_json,file_hash,cargo_executables
p=argparse.ArgumentParser();p.add_argument('name');p.add_argument('--production',action='store_true');args=p.parse_args()
OUT=Path(__file__).resolve().parent
patch=subprocess.check_output(['git','diff','--binary','HEAD'],cwd=ROOT);(OUT/f'{args.name}.patch').write_bytes(patch)
files=subprocess.check_output(['git','diff','--name-only','HEAD'],cwd=ROOT).decode().splitlines()
files += ['scripts/falcon_worker_optimization.py','scripts/test_falcon_worker_optimization.py']
write_json(OUT/f'{args.name}-source.json',dict(base=subprocess.check_output(['git','rev-parse','HEAD'],cwd=ROOT).decode().strip(),patch_sha256=hashlib.sha256(patch).hexdigest(),files={f:file_hash(ROOT/f) for f in set(files)}))
env=clean_environment(os.environ);env.pop("FZF_DEFAULT_OPTS",None);env.pop("CARGO_ENCODED_RUSTFLAGS",None);overrides=dict(RUSTFLAGS='-C target-cpu=native',CARGO_BUILD_JOBS='16',TMPDIR=str(ROOT/'.tmp/falcon-twelve-workers-20261013/build-tmp'),CARGO_TARGET_DIR=str(ROOT/'.tmp/falcon-buffer-production-target'));env.update(overrides)
cmd=['cargo','bench','--locked','--features','falcon-hybrid' if args.production else 'falcon-hybrid,bench-internals','--bench','falcon_algebraic','--no-run','--message-format=json']
write_json(OUT/f'{args.name}-build-command.json',dict(command=cmd,overrides=overrides,experiment_environment_cleared=True))
with (OUT/f'{args.name}-build.jsonl').open('w') as so,(OUT/f'{args.name}-build.stderr').open('w') as se:code=subprocess.call(cmd,cwd=ROOT,env=env,stdout=so,stderr=se)
print('build exit',code,flush=True)
if code:print((OUT/f'{args.name}-build.stderr').read_text()[-10000:]);sys.exit(code)
artifact=Path(cargo_executables((OUT/f'{args.name}-build.jsonl').read_text(),['falcon_algebraic'])['falcon_algebraic']);dest=ROOT/'.tmp/falcon-twelve-workers-20261013'/args.name
assert not dest.exists(), f'Frozen binary already exists: {dest}'
snapshot=__import__('json').loads((OUT/f'{args.name}-source.json').read_text());assert all(file_hash(ROOT/f)==h for f,h in snapshot['files'].items() if f.endswith('.rs')), 'Rust source changed during build'
assert subprocess.check_output(['git','diff','--binary','HEAD'],cwd=ROOT)==patch, 'Source patch changed during build'
shutil.copy2(artifact,dest)
write_json(OUT/f'{args.name}-binary.json',dict(path=str(dest),sha256=file_hash(dest),cargo_artifact=str(artifact)))
print('frozen',dest,flush=True)

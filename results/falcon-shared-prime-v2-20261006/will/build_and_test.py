from pathlib import Path
import hashlib,json,os,platform,shutil,subprocess
root=Path.cwd()
out=root/'.tmp/falcon-v2'
out.mkdir(parents=True,exist_ok=True)
env=dict(os.environ,RUSTUP_TOOLCHAIN='1.98.1',RUSTFLAGS='-C target-cpu=native',CARGO_TARGET_DIR=str(root/'target/falcon-v2'),RAYON_NUM_THREADS='16')
# Baselines are immutable copies, not the mutable paths in their original manifests.
baselines={
 'native':('.tmp/falcon-baseline-449-20261005T180359Z/falcon_hybrid_4491309f','3a4f62756ba098780a9f51b7f435199d932ed14ef54b1b9a4bf56e12270da7c3','4491309f5d1e45d506410803e3b9ba488047991f','.tmp/falcon-baseline-449-20261005T180359Z/manifest.json'),
 'v1':('.tmp/falcon-shared-candidate-e370e3b35da692d86e814c687e597c3a1d0a6bea/falcon_hybrid_e370e3b35da6','b673d6a72f759be291deee7f696120a298897e71a79e0de2ededfc788b517227','e370e3b35da692d86e814c687e597c3a1d0a6bea','.tmp/falcon-shared-candidate-e370e3b35da692d86e814c687e597c3a1d0a6bea/manifest.json')}
def digest(p):
 with Path(p).open('rb') as f: return hashlib.file_digest(f,'sha256').hexdigest()
for _,(path,sha,_,_) in baselines.items():
 assert digest(root/path)==sha
with (out/'tests.log').open('w') as f:
 test=subprocess.run(['cargo','test','--offline','--locked','--release','--features','falcon-hybrid','--lib','falcon1024_ct','--','--test-threads=1'],env=env,stdout=f,stderr=subprocess.STDOUT)
assert test.returncode==0, 'remote Falcon tests failed; see tests.log'
with (out/'build.jsonl').open('w') as stdout,(out/'build.stderr').open('w') as stderr:
 build=subprocess.run(['cargo','bench','--offline','--locked','--profile','release','--features','falcon-hybrid','--bench','falcon_hybrid','--no-run','--message-format=json-render-diagnostics'],env=env,stdout=stdout,stderr=stderr)
assert build.returncode==0, 'remote build failed; see build.stderr'
artifacts=[r['executable'] for line in (out/'build.jsonl').read_text().splitlines() if (r:=json.loads(line)).get('reason')=='compiler-artifact' and r['target']['name']=='falcon_hybrid' and r.get('executable')]
assert len(artifacts)==1
binary=out/'falcon_v2'
assert not binary.exists(), 'refusing to overwrite pinned V2 binary'
shutil.copy2(artifacts[0],binary)
revision=subprocess.check_output(['git','rev-parse','HEAD'],text=True).strip()
common=dict(host=platform.node(),architecture=platform.machine(),rustc=subprocess.check_output(['rustc','-Vv'],env=env,text=True),rustflags=env['RUSTFLAGS'],features=['falcon-hybrid'],profile='release',lock_sha256=digest(root/'Cargo.lock'))
manifest=dict(common,candidate='v2',binaries={})
for label,(path,sha,rev,original) in baselines.items():
 previous=json.loads((root/original).read_text())
 assert all(previous[k]==v for k,v in common.items()), 'retained baseline build provenance mismatch: '+label
 assert previous['revision']==rev
 manifest['binaries'][label]=dict(common,binary=str(root/path),sha256=sha,revision=rev,original_manifest=str(root/original))
manifest['binaries']['v2']=dict(common,binary=str(binary),sha256=digest(binary),revision=revision)
(out/'manifest.json').write_text(json.dumps(manifest,indent=2)+'\n')
print(json.dumps(dict(revision=revision,binary=str(binary),sha256=digest(binary),tests='passed',manifest=str(out/'manifest.json'))))

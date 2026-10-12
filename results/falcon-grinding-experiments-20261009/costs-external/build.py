import hashlib,json,pathlib,subprocess
root=pathlib.Path('/home/john-wu/code/BitZ-pcs')
out=root/'results/falcon-grinding-experiments-20261009/costs-external'
wanted={'bitz','rayon','blake3','libc','flock_core'}
libs={}
for line in (root/'results/falcon-grinding-experiments-20261009/build-final.jsonl').read_text().splitlines():
    j=json.loads(line)
    if j.get('reason')=='compiler-artifact' and j['target']['name'] in wanted:
        for path in j['filenames']:
            if path.endswith('.rlib'): libs[j['target']['name']]=path
assert set(libs)==wanted
command=['taskset','-c','14','rustc','--edition=2024','-C','opt-level=3','-C','target-cpu=native',
         '-L','dependency='+str(root/'target/falcon-native/release/deps'),str(out/'probe.rs'),'-o',str(out/'probe')]
for name,path in sorted(libs.items()): command += ['--extern',name+'='+path]
provenance={'command':command,'libraries':{k:{'path':v,'sha256':hashlib.sha256(pathlib.Path(v).read_bytes()).hexdigest()} for k,v in libs.items()},
            'source_sha256':hashlib.sha256((out/'probe.rs').read_bytes()).hexdigest(),
            'rustc':subprocess.check_output(['rustc','-Vv'],text=True),
            'baseline_commit':subprocess.check_output(['git','rev-parse','HEAD'],cwd=root,text=True).strip(),
            'experiment_source_hashes':str(root/'results/falcon-grinding-experiments-20261009/source-hashes.json'),
            'entry_context':'external non-Rayon mainCPU6; globalpool worker0 alsoCPU6'}
(out/'build.json').write_text(json.dumps(provenance,indent=2)+'\n')
with (out/'build.log').open('w') as f: subprocess.run(command,cwd=root,stdout=f,stderr=subprocess.STDOUT,check=True)
provenance['binary_sha256']=hashlib.sha256((out/'probe').read_bytes()).hexdigest()
(out/'build.json').write_text(json.dumps(provenance,indent=2)+'\n')

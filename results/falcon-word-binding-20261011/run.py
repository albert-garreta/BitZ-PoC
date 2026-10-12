import hashlib, json, os, pathlib, statistics, subprocess, sys, time
out = pathlib.Path(sys.argv[1])
out.mkdir(parents=True, exist_ok=True)
binaries = {'r7': '/tmp/bitz-arithmetic-v8-r7-qualification-binary', 'word': '/home/john-wu/code/BitZ-pcs/target/release/examples/falcon_algebraic'}
shapes = [(512, 1024, 100, 1), (1024, 1024, 100, 1), (512, 1024, 128, 8), (1024, 1024, 128, 8)]
trace = '--trace' in sys.argv
if '--boundaries' in sys.argv:
    shapes = [(n,b,s,t) for n in [512,1024] for b in [3,513] for s in [100,128] for t in [1,8]]
if '--small-128' in sys.argv:
    shapes = [(n,3,128,8) for n in [512,1024]]
if trace:
    seeds=[42]
else:
    seeds=[42,43,44]
manifest = {'binaries': {k: {'path':v,'sha256':hashlib.sha256(pathlib.Path(v).read_bytes()).hexdigest()} for k,v in binaries.items()}, 'shapes': shapes, 'seeds':seeds, 'warmup':1,'iterations':2,'trace':trace,'created_unix':time.time()}
(out/'manifest.json').write_text(json.dumps(manifest,indent=2)+'\n')
all_rows=[]
for shape in shapes:
    n,b,s,t=shape
    for seed in manifest['seeds']:
        for variant in (['r7','word'] if seed%2==0 else ['word','r7']):
            stem=f'n{n}-b{b}-s{s}-t{t}-seed{seed}-{variant}'
            cmd=[binaries[variant],'--degree',str(n),'--batch',str(b),'--security',str(s),'--threads',str(t),'--seed',str(seed),'--warmup','1','--iterations','2']
            if trace:
                cmd.append('--trace')
            env=os.environ.copy()
            env.update(RAYON_NUM_THREADS=str(t),BITZ_FALCON_WORKER_CPUS=','.join(map(str,range(t))),BITZ_FALCON_MAIN_CPU='0')
            with (out/(stem+'.jsonl')).open('w') as stdout, (out/(stem+'.stderr')).open('w') as stderr:
                subprocess.run(cmd,env=env,stdout=stdout,stderr=stderr,check=True)
            rows=[json.loads(line) for line in (out/(stem+'.jsonl')).read_text().splitlines()]
            assert len(rows)==3 and all(row['verified'] for row in rows)
            reference = pathlib.Path(__file__).parent/'initial'/'large'/(stem+'.jsonl')
            # The final 128-bit schedule funds the terminal merge from the
            # removed lane rounds, changing its transcript. The unground
            # 100-bit transcript remains identical to the initial v9 kernel.
            if variant=='word' and s==100 and reference.exists():
                prior=[json.loads(line) for line in reference.read_text().splitlines()]
                assert {r['proof_debug_digest'] for r in rows}=={r['proof_debug_digest'] for r in prior}
            all_rows.extend(dict(row,variant=variant) for row in rows if row['trial']=='sample')
            print(stem, 'prove_ms',round(statistics.median(row['prove_ms'] for row in rows[1:]),3),flush=True)
for shape in shapes:
    for seed in manifest['seeds']:
        paired=[r for r in all_rows if (r['degree'],r['batch'],r['security_bits'],r['threads'])==shape and r['seed']==seed]
        assert len({r['input_digest'] for r in paired})==1
        assert len({tuple(r['source_root']) for r in paired})==1
summary=[]
for n,b,s,t in shapes:
    group=[r for r in all_rows if (r['degree'],r['batch'],r['security_bits'],r['threads'])==(n,b,s,t)]
    result={'degree':n,'batch':b,'security':s,'threads':t}
    for key in ['prove_ms','total_prover_ms','verify_ms','process_peak_rss_kib','proof_payload_bytes']:
        med={v:statistics.median(r[key] for r in group if r['variant']==v) for v in binaries}
        result[key]=dict(med,change_percent=100*(med['word']/med['r7']-1))
    summary.append(result)
(out/'summary.json').write_text(json.dumps(summary,indent=2)+'\n')
print(json.dumps(summary,indent=2),flush=True)

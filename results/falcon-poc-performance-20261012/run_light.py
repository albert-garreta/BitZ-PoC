import itertools,json,os,random,subprocess,time
from pathlib import Path
from run import OUT,SOURCE,REV,CPUS,SEEDS,snapshot,normalize_trials,read_jsonl,write_json,file_hash,run_process,source_metadata

def main():
    assert source_metadata(SOURCE)['revision']==REV and not source_metadata(SOURCE)['git_dirty']
    manifest=json.loads((OUT/'light-build.json').read_text())
    reference={}
    for p in (OUT/'latency').glob('*/result.json'):
        d=json.loads(p.read_text()); j=d['job'];t=d['trials'][0]
        reference[(j['variant'],d['degree'],j['seed'])]=[t[k] for k in ['protocol_id','input_digest','source_root','proof_debug_digest','proof_payload_bytes']]
    jobs=list(itertools.product(['arithmetic','full'],[512,1024],[1,8,16],SEEDS))
    random.Random(20261014).shuffle(jobs)
    count=0
    for variant,degree,threads,seed in jobs:
        name=f'{variant}-n{degree}-t{threads}-s{seed}'; directory=OUT/'profile-light'/name
        directory.mkdir(parents=True,exist_ok=False)
        binary=manifest['binaries'][variant]; assert file_hash(binary['path'])==binary['sha256']
        command=[binary['path'],'--degree',str(degree),'--batch','1024','--security','128','--threads',str(threads),
            '--seed',str(seed),'--warmup','1','--iterations','3']
        overrides=dict(RAYON_NUM_THREADS=str(threads),BITZ_FALCON_WORKER_CPUS=','.join(map(str,CPUS[:threads])),BITZ_FALCON_MAIN_CPU='6',
            BITZ_FALCON_CASE_CACHE=str(Path('/home/john-wu/code/BitZ-pcs/.tmp/falcon-poc-performance-cases-20261012')))
        if variant=='arithmetic':command.append('--trace')
        else:overrides['BITZ_FALCON_STAGE_TIMINGS']='1'
        env={k:v for k,v in os.environ.items() if not k.startswith(('BITZ_','F2_','OBLONG_')) and k not in ['RAYON_NUM_THREADS','RUSTFLAGS','CARGO_ENCODED_RUSTFLAGS']}
        env.update(overrides)
        write_json(directory/'command.json',dict(command=command,overrides=overrides,cwd=str(SOURCE),binary=binary))
        start=snapshot();print('RUN '+name,flush=True)
        with (directory/'stdout.jsonl').open('w') as stdout,(directory/'stderr.log').open('w') as stderr:
            code,timeout=run_process(command,env=env,cwd=SOURCE,stdout=stdout,stderr=stderr,timeout=600)
        write_json(directory/'process.json',dict(start=start,end=snapshot(),exit_code=code,timed_out=timeout))
        assert code==0 and not timeout,directory
        job=dict(id=name,variant=variant,threads=threads,seed=seed)
        contract=dict(degree=degree,batch=1024,security=128,worker_cpus=CPUS,main_cpu=6,warmup=1,iterations=3)
        trials=normalize_trials(read_jsonl(directory/'stdout.jsonl'),job,contract)
        for t in trials:
            assert reference[(variant,degree,seed)]==[t[k] for k in ['protocol_id','input_digest','source_root','proof_debug_digest','proof_payload_bytes']]
            count+=1
        write_json(directory/'result.json',dict(job=job,phase='profile-light',degree=degree,trials=trials,
            stdout_sha256=file_hash(directory/'stdout.jsonl'),stderr_sha256=file_hash(directory/'stderr.log'),verified=True))
        print(f'OK {name}; verified={count}',flush=True)
    assert count==144
    write_json(OUT/'light-validation.json',dict(verified_proofs=count,complete_proof_identities_equal_to_untraced_production=True,source_clean=not source_metadata(SOURCE)['git_dirty']))
    print('ALL COMPLETE',flush=True)

if __name__=='__main__':main()

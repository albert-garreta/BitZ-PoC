#!/usr/bin/env python3
"""Sequential measurements of the exact pushed Falcon revision; no source edits."""
import hashlib, itertools, json, os, random, shutil, subprocess, sys, time
from pathlib import Path

OUT = Path(__file__).resolve().parent
SOURCE = Path('/tmp/bitz-falcon-poc-push-20261012')
REV = 'ee79186745962e3e285fc5090dae8a95545dda43'
BIN = Path('/home/john-wu/code/BitZ-pcs/.tmp/falcon-poc-performance-binaries-20261012')
CACHE = Path('/home/john-wu/code/BitZ-pcs/.tmp/falcon-poc-performance-cases-20261012')
CPUS = [6,7,4,5,2,3,0,1,14,15,12,13,10,11,8,9]
SEEDS = [42,43,44]
sys.path.insert(0, str(SOURCE/'scripts'))
from bench_support import file_hash, write_json, run_process, cargo_executables, source_metadata
from binder_experiments import normalize_trials, read_jsonl


def snapshot():
    return dict(time_unix=time.time(), loadavg=Path('/proc/loadavg').read_text().strip())


def main():
    metadata=source_metadata(SOURCE)
    assert metadata['revision']==REV and not metadata['git_dirty'], metadata
    artifacts=cargo_executables((OUT/'build.jsonl').read_text(), ['falcon_algebraic','falcon_hybrid'])
    BIN.mkdir(parents=True,exist_ok=True)
    binaries={}
    for variant,name in [('arithmetic','falcon_algebraic'),('full','falcon_hybrid')]:
        target=BIN/name
        shutil.copy2(artifacts[name], target)
        binaries[variant]=dict(path=str(target),sha256=file_hash(target))
    manifest=dict(source=metadata,binaries=binaries,worker_cpus=CPUS,main_cpu=6,batch=1024,
        security=128,seeds=SEEDS,threads=[1,8,16],degrees=[512,1024],
        features=['falcon-hybrid'],profile='release; LTO; codegen-units=1',rustflags='-C target-cpu=native',
        runner_sha256=file_hash(__file__),
        source_helper_hashes={name:file_hash(SOURCE/'scripts'/name) for name in ['bench_support.py','binder_experiments.py']},
        latency=dict(warmup=2,iterations=7),profile_timings=dict(warmup=1,iterations=3),
        case_cache=str(CACHE), allowed_cpus=sorted(os.sched_getaffinity(0)),
        notes=['Benchmarks sequential, after all builds. Tracing disabled for headline latency.',
               'Main and auxiliary Flock threads pinned to CPU 6; global workers use distinct physical cores.',
               'Host is not exclusively reserved. Setup and deterministic fixture generation excluded from prover timers.'])
    write_json(OUT/'manifest.json',manifest)
    subprocess.run(['lscpu','-J'],stdout=(OUT/'lscpu.json').open('w'),check=True)
    proofs={}; inputs={}; count=0
    for phase,warmup,iterations in [('prewarm',0,1),('latency',2,7),('profile',1,3)]:
        jobs=list(itertools.product(['arithmetic','full'],[512,1024],[1,8,16],SEEDS))
        if phase=='prewarm':
            jobs=[('arithmetic',degree,1,seed) for degree,seed in itertools.product([512,1024],SEEDS)]
        else:
            random.Random(20261012+(phase=='profile')).shuffle(jobs)
        for variant,degree,threads,seed in jobs:
            name=f'{variant}-n{degree}-t{threads}-s{seed}'
            directory=OUT/phase/name; directory.mkdir(parents=True,exist_ok=False)
            executable=binaries[variant]
            assert file_hash(executable['path'])==executable['sha256']
            command=[executable['path'],'--degree',str(degree),'--batch','1024','--security','128',
                '--threads',str(threads),'--seed',str(seed),'--warmup',str(warmup),'--iterations',str(iterations)]
            overrides=dict(RAYON_NUM_THREADS=str(threads),BITZ_FALCON_WORKER_CPUS=','.join(map(str,CPUS[:threads])),
                BITZ_FALCON_MAIN_CPU='6',BITZ_FALCON_CASE_CACHE=str(CACHE))
            if phase=='profile':
                if variant=='arithmetic':command.append('--trace')
                else:overrides['BITZ_FALCON_STAGE_TIMINGS']='1'
            env={k:v for k,v in os.environ.items() if not k.startswith(('BITZ_','F2_','OBLONG_'))
                 and k not in ['RAYON_NUM_THREADS','RUSTFLAGS','CARGO_ENCODED_RUSTFLAGS']}
            env.update(overrides)
            write_json(directory/'command.json',dict(command=command,overrides=overrides,cwd=str(SOURCE),binary=executable))
            start=snapshot(); print(f'RUN {phase}/{name}',flush=True)
            with (directory/'stdout.jsonl').open('w') as stdout,(directory/'stderr.log').open('w') as stderr:
                code,timeout=run_process(command,env=env,cwd=SOURCE,stdout=stdout,stderr=stderr,timeout=600)
            process=dict(start=start,end=snapshot(),exit_code=code,timed_out=timeout)
            write_json(directory/'process.json',process)
            if code!=0 or timeout:raise RuntimeError(f'failed {directory}')
            events=read_jsonl(directory/'stdout.jsonl')
            job=dict(id=name,variant=variant,threads=threads,seed=seed)
            contract=dict(degree=degree,batch=1024,security=128,worker_cpus=CPUS,main_cpu=6,warmup=warmup,iterations=iterations)
            trials=normalize_trials(events,job,contract)
            rawtrials=[e for e in events if e.get('trial') in ['sample','warmup']]
            for trial,raw in zip(trials,rawtrials,strict=True):
                assert inputs.setdefault((degree,seed),trial['input_digest'])==trial['input_digest']
                key=(variant,degree,seed)
                identity=[trial[k] for k in ['protocol_id','input_digest','source_root','proof_debug_digest','proof_payload_bytes']]
                assert proofs.setdefault(key,identity)==identity,(key,threads,phase,'identity mismatch')
                assert sum(dict(raw['proof_payload_breakdown']).values())==trial['proof_payload_bytes']
                count+=1
            write_json(directory/'result.json',dict(job=job,phase=phase,degree=degree,trials=trials,
                stdout_sha256=file_hash(directory/'stdout.jsonl'),stderr_sha256=file_hash(directory/'stderr.log'),verified=True))
            print(f'OK  {phase}/{name}; verified={count}',flush=True)
    assert source_metadata(SOURCE)['git_dirty'] is False
    write_json(OUT/'validation.json',dict(verified_proofs=count,expected_proofs=474,
        complete_proof_identities_equal_across_workers_and_trace_modes=True,
        input_digests_equal_across_arithmetic_and_full=True,source_revision=REV,source_clean=True))
    assert count==474
    print('ALL COMPLETE',flush=True)

if __name__=='__main__': main()

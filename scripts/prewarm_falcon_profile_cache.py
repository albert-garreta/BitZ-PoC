#!/usr/bin/env python3
"""Freshly generate and validate all480 public Falcon fixtures outside timings.

Each child independently generates an uncached reference, atomically publishes
or compares its degree-aware cache, and revalidates its upstream signatures,
CT encoding and native verification. This tool never runs the Falcon prover.
"""
from __future__ import annotations
import argparse
from concurrent.futures import ThreadPoolExecutor, wait, FIRST_COMPLETED
import datetime
import hashlib
import itertools
import json
import os
from pathlib import Path
import subprocess
import time

from qualify_falcon_simplification import DEGREES, BATCHES, SEEDS, HEX64, environment, require


def sha256(path):
    digest=hashlib.sha256()
    with Path(path).open('rb') as stream:
        for chunk in iter(lambda:stream.read(1<<20),b''):digest.update(chunk)
    return digest.hexdigest()


def save(path,value):
    temporary=path.with_suffix(path.suffix+'.tmp')
    temporary.write_text(json.dumps(value,indent=2,allow_nan=False)+'\n')
    temporary.replace(path)


def key_for(case):return ':'.join(map(str,case))


def filename_for(case):
    degree,batch,seed=case
    return f'fn-dsa-0.3.0-n{degree}-b{batch}-seed{seed}.bin'


def validate_reference(row,case,cache):
    degree,batch,seed=case
    require(row.get('schema')=='bitz/falcon-degree-fixture-reference/v1','wrong fixture-reference schema')
    require(all(row.get(k)==v for k,v in dict(degree=degree,batch=batch,seed=seed,cache_filename=filename_for(case)).items()),'fixture shape/filename mismatch')
    require(all(row.get(k) is True for k in ('fresh_uncached_generation','cache_roundtrip_validated','upstream_verified','ct_conversion_verified','native_preflight_verified')),'missing independent validation')
    require(HEX64.fullmatch(row.get('input_digest','')) and HEX64.fullmatch(row.get('cache_payload_blake3','')),'invalid input/checksum digest')
    path=cache/filename_for(case)
    require(path.stat().st_size==row['cache_bytes'] and row['cache_bytes']>32,'cache length mismatch')
    with path.open('rb') as stream:stream.seek(-32,2);checksum=stream.read().hex()
    require(checksum==row['cache_payload_blake3'],'stored cache checksum differs from generated reference')
    return {**row,'sha256':sha256(path)}


def run_one(args,method,case):
    stem=filename_for(case).removesuffix('.bin')
    record_path=args.output/(stem+'.run.json')
    out=args.output/(stem+'.jsonl');err=args.output/(stem+'.stderr')
    if record_path.exists():
        require(args.resume,'existing fixture reference without --resume')
        record=json.loads(record_path.read_text())
        require(record['status']=='validated','incomplete fixture record requires investigation')
        require(record['generator_sha256']==method['sha256'],'resumed generator differs')
        require(record['stdout_sha256']==sha256(out) and record['stderr_sha256']==sha256(err),'resumed evidence changed')
        row=json.loads(out.read_text());reference=validate_reference(row,case,args.cache)
        require(reference==record['reference'],'resumed cache/reference changed')
        return reference
    require(not out.exists() and not err.exists(),'unrecorded fixture artifacts exist')
    command=[method['binary'],*map(str,case),str(args.cache)]
    record={'status':'running','argv':command,'generator_sha256':method['sha256'],'started_at':time.time()}
    save(record_path,record)
    started=time.monotonic()
    try:
        with out.open('w') as stdout,err.open('w') as stderr:
            result=subprocess.run(command,env=environment(1),stdout=stdout,stderr=stderr,timeout=args.timeout)
        require(result.returncode==0,f'fixture generator failed for {case}: {err.read_text().strip()}')
        row=json.loads(out.read_text());reference=validate_reference(row,case,args.cache)
        record.update(status='validated',reference=reference,stdout_sha256=sha256(out),stderr_sha256=sha256(err))
        return reference
    except BaseException as error:
        record.update(status='failed',error=repr(error));raise
    finally:
        record.update(elapsed_seconds=time.monotonic()-started,finished_at=time.time());save(record_path,record)


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--generator-method',type=Path,required=True)
    parser.add_argument('--cache',type=Path,required=True)
    parser.add_argument('--output',type=Path,required=True)
    parser.add_argument('--workers',type=int,default=4)
    parser.add_argument('--timeout',type=float,default=300)
    parser.add_argument('--resume',action='store_true')
    args=parser.parse_args()
    require(1<=args.workers<=4,'fixture workers must be between1 and4')
    args.cache=args.cache.resolve();args.output=args.output.resolve()
    args.cache.mkdir(parents=True,exist_ok=True);args.output.mkdir(parents=True,exist_ok=True)
    method=json.loads(args.generator_method.read_text())
    require(method.get('status')=='built' and sha256(method['binary'])==method['sha256'],'generator binary/provenance mismatch')
    cases=list(itertools.product(DEGREES,BATCHES,SEEDS));assert len(cases)==480
    config={'generator_method':str(args.generator_method.resolve()),'generator_method_sha256':sha256(args.generator_method),'generator_sha256':method['sha256'],'cache':str(args.cache),'degrees':DEGREES,'batches':BATCHES,'seeds':SEEDS,'workers':args.workers,'harness_sha256':sha256(__file__)}
    path=args.output/'manifest.json'
    if path.exists():
        old=json.loads(path.read_text());require(args.resume,'existing prewarm campaign')
        require(old['configuration']==json.loads(json.dumps(config)),'resumed prewarm configuration changed')
    manifest={'configuration':config,'status':'running','started_utc':datetime.datetime.now(datetime.timezone.utc).isoformat(),'completed_fixtures':0,'cases':{}}
    save(path,manifest)
    references={};pending=iter(cases);active={};failure=None;stop=False
    with ThreadPoolExecutor(max_workers=args.workers) as pool:
        for _ in range(args.workers):
            case=next(pending,None)
            if case is not None:active[pool.submit(run_one,args,method,case)]=case
        while active:
            done,_=wait(active,return_when=FIRST_COMPLETED)
            for future in done:
                case=active.pop(future)
                try:
                    references[key_for(case)]=future.result()
                    print(f'VALIDATED {len(references)}/480 {key_for(case)}',flush=True)
                except BaseException as error:
                    failure=error;print(f'FAILED {case}: {error}',flush=True)
                stop=stop or (args.output/'STOP').exists()
                if failure is None and not stop:
                    next_case=next(pending,None)
                    if next_case is not None:active[pool.submit(run_one,args,method,next_case)]=next_case
            manifest.update(completed_fixtures=len(references),cases=references);save(path,manifest)
    require(sha256(method['binary'])==method['sha256'],'generator changed during prewarm')
    for key,reference in references.items():
        require(sha256(args.cache/reference['cache_filename'])==reference['sha256'],'cache changed during prewarm: '+key)
    complete=failure is None and len(references)==480
    manifest.update(status='complete' if complete else 'failed' if failure else 'stopped',finished_utc=datetime.datetime.now(datetime.timezone.utc).isoformat())
    if failure:manifest['error']=repr(failure)
    save(path,manifest)
    reference={'schema':'bitz/falcon-degree-case-cache-reference/v1','complete':complete,'generation':'fresh uncached per degree,batch,seed; independent of existing cache bytes','configuration':config,'cases':references}
    save(args.output/'cache-reference.json',reference)
    print('PREWARM_RESULT '+manifest['status'],flush=True)
    return 0 if complete else 2

if __name__=='__main__':raise SystemExit(main())

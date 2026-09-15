#!/usr/bin/env python3
"""Execute the requested commands sequentially and preserve every exit status."""
import datetime
import json
import os
from pathlib import Path
import signal
import subprocess
import sys
import time

OUT = Path(__file__).resolve().parent
REPO = OUT.parents[1]
BASE = dict(os.environ)
for key in ('CARGO_ENCODED_RUSTFLAGS','F2Z_BENCH_SEED','F2Z_BENCH_SHAPES','F2Z_BENCH_REPS',
            'F2Z_MUL_COMPARE_BACKENDS','F2Z_MUL_COMPARE_WORKLOADS','F2Z_MUL_COMPARE_MEMORY',
            'F2Z_MUL_COMPARE_OUTPUT_DIR','F2Z_LIG_PROFILE','F2Z_BINIUS_LOG_INV_RATE',
            'F2Z_BINIUS_LIGERITO_LOG_INV_RATE','F2Z_PLONKY3_LOG_INV_RATE',
            'F2Z_MUL_MEMORY_ONLY','BDLAMBDA','BDSPEC','BDROWLEN','BDDIRECT','BDSPLIT','CHAIN_BITS','DUMP'):
    BASE.pop(key, None)
BASE.update(RUSTFLAGS='-C target-cpu=native', RAYON_NUM_THREADS='8',
            PERFETTO_TRACE_PROCESSOR='/private/tmp/f2z-perfetto.F7C3rn/trace_processor_shell')
NATIVE = ['cargo','+1.98.1','bench','--profile','release','--bench','mul_e2e_compare',
          '--features','bench-internals,native-mul-compare,unchecked']
COMMON = dict(F2Z_BENCH_SHAPES='15 16 17 18 19 20 21 22', F2Z_BENCH_REPS='5',
              F2Z_MUL_COMPARE_MEMORY='1', F2Z_MUL_COMPARE_WORKLOADS='u32-mod32')
jobs = []
for r in (1,2,3):
    folder = OUT/'binius-pcs'/f'rate{r}'
    env = dict(COMMON, F2Z_MUL_COMPARE_BACKENDS='binius64 binius64-ligerito',
               F2Z_BINIUS_LOG_INV_RATE=str(r), F2Z_BINIUS_LIGERITO_LOG_INV_RATE=str(r),
               F2Z_MUL_COMPARE_OUTPUT_DIR=str(folder))
    jobs.append((f'binius-pcs-rate{r}',NATIVE,env))
for r in (1,2,3):
    cmd = ['cargo','+1.98.1','run','--release','--features','unchecked','--',
           '--mul-sweep','15-22','--threads','8','--reps','5','--lambda','100','--word-bits','1',
           '--profile',f'custom:{r}:4','--cooldown','20',
           '--latex',str(OUT/'all-provers'/f'bitz-u32-wide-rate{r}.tex')]
    jobs.append((f'bitz-u32-wide-rate{r}',cmd,{}))
for r in (1,2,3):
    env = dict(COMMON, F2Z_MUL_COMPARE_BACKENDS='f2z binius64 binius64-ligerito plonky3-fri',
               F2Z_LIG_PROFILE=f'custom:{r}:4',F2Z_BINIUS_LOG_INV_RATE=str(r),
               F2Z_BINIUS_LIGERITO_LOG_INV_RATE=str(r), F2Z_PLONKY3_LOG_INV_RATE=str(r),
               F2Z_MUL_COMPARE_OUTPUT_DIR=str(OUT/'all-provers'/f'u32-mod32-rate{r}'))
    jobs.append((f'u32-mod32-rate{r}',NATIVE,env))
jobs.append(('u32-mod32-limber',NATIVE,dict(COMMON,F2Z_MUL_COMPARE_BACKENDS='limber',
             F2Z_MUL_COMPARE_OUTPUT_DIR=str(OUT/'all-provers'/'u32-mod32-limber'))))
for r in (1,2,3):
    cmd = ['python3','scripts/run_sha256_ecdsa_compare.py','--methods','f2z-split','binius64',
           '--exponents','7','--targets','100','--threads','8','--seeds','0','--reps','5',
           '--binius-log-inv-rate',str(r),'--output',str(OUT/'all-provers'/f'sha256-ecdsa-rate{r}')]
    jobs.append((f'sha256-ecdsa-rate{r}',cmd,{'F2Z_LIG_PROFILE':f'custom:{r}:4'}))
plan = [dict(name=n,command=c,environment=BASE|e) for n,c,e in jobs]
# Only record the explicitly controlled environment, never unrelated shell values.
for item, (_,_,env) in zip(plan,jobs):
    item['environment'] = {k:(BASE|env)[k] for k in ('RUSTFLAGS','RAYON_NUM_THREADS','PERFETTO_TRACE_PROCESSOR')}
    item['environment'].update(env)
(OUT/'commands.json').write_text(json.dumps(plan,indent=2)+'\n')
MAX_GROUP_RSS = 48 * 1024**3
(OUT/'execution-policy.json').write_text(json.dumps({'sequential':True,'max_owned_process_group_rss_bytes':MAX_GROUP_RSS,
    'reason':'Preserve headroom on this 64 GiB machine; retain resource-limit failures without relabelling them.'},indent=2)+'\n')
records=[]
for name,cmd,env in jobs:
    logpath=OUT/'logs'/f'{name}.log'
    record=dict(name=name,command=cmd,status='running',started_utc=datetime.datetime.now(datetime.timezone.utc).isoformat())
    records.append(record)
    (OUT/'status.json').write_text(json.dumps(records,indent=2)+'\n')
    print(f'START {name}',flush=True)
    started=time.monotonic()
    peak=0
    with logpath.open('x') as log:
        child=subprocess.Popen(cmd,cwd=REPO,env=BASE|env,stdout=log,stderr=subprocess.STDOUT,start_new_session=True)
        try:
            while child.poll() is None:
                time.sleep(2)
                rows=subprocess.check_output(['ps','-axo','pgid=,rss='],text=True).splitlines()
                rss=sum(int(parts[1])*1024 for row in rows if len(parts:=row.split())==2 and int(parts[0])==child.pid)
                peak=max(peak,rss)
                if rss > MAX_GROUP_RSS:
                    record['status']='resource_limit'
                    os.killpg(child.pid,signal.SIGTERM)
                    try:
                        child.wait(timeout=5)
                    except subprocess.TimeoutExpired:
                        os.killpg(child.pid,signal.SIGKILL)
                    break
        except BaseException:
            os.killpg(child.pid,signal.SIGTERM)
            child.wait()
            raise
        code=child.wait()
    record.update(exit_code=code,elapsed_seconds=time.monotonic()-started,observed_process_group_peak_rss_bytes=peak,
                  finished_utc=datetime.datetime.now(datetime.timezone.utc).isoformat())
    if record['status']=='running':
        record['status']='complete' if code==0 else 'failed'
    (OUT/'status.json').write_text(json.dumps(records,indent=2)+'\n')
    print(f"END {name}: {record['status']} ({record['elapsed_seconds']:.1f}s)",flush=True)
print('REQUESTED COMMANDS FINISHED',flush=True)
sys.exit(0 if all(r['status']=='complete' for r in records) else 1)

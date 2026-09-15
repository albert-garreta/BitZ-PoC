#!/usr/bin/env python3
"""Run the recorded matrix serially; retain exit statuses and system observations."""
import datetime
import fcntl
import hashlib
import json
import os
from pathlib import Path
import re
import signal
import subprocess
import sys
import time

ROOT = Path(__file__).resolve().parent
REPO = ROOT.parents[1]
JOBS = json.loads((ROOT/'commands.json').read_text())
POLICY = json.loads((ROOT/'request.json').read_text())
BASE = {k:v for k,v in os.environ.items() if not k.startswith('F2Z_')}
for key in ('CARGO_ENCODED_RUSTFLAGS','BDLAMBDA','BDSPEC','BDROWLEN','BDDIRECT','BDSPLIT','CHAIN_BITS','DUMP'):
    BASE.pop(key,None)

def utc():
    return datetime.datetime.now(datetime.timezone.utc).isoformat()

def save_status(records):
    temporary=ROOT/'status.json.tmp'
    temporary.write_text(json.dumps(records,indent=2)+'\n')
    temporary.replace(ROOT/'status.json')

def processes():
    result=[]
    output=subprocess.check_output(['ps','-axo','pid=,ppid=,pgid=,%cpu=,rss=,comm='],text=True)
    for line in output.splitlines():
        p=line.strip().split(None,5)
        if len(p)==6:
            result.append(dict(pid=int(p[0]),ppid=int(p[1]),pgid=int(p[2]),cpu_pct=float(p[3]),rss_bytes=int(p[4])*1024,process=p[5]))
    return result

def snapshot(job,stage,group=None,rows=None):
    rows=processes() if rows is None else rows
    own=[p for p in rows if p['pgid']==group] if group else []
    external=[p for p in rows if p['pgid']!=group and p['pid']!=os.getpid()]
    vm=subprocess.check_output(['vm_stat'],text=True)
    counters={}
    for line in vm.splitlines()[1:]:
        match=re.match(r'(.+?):\s+(\d+)\.',line)
        if match:
            counters[match[1].strip().strip('"')]=int(match[2])
    record=dict(utc=utc(),monotonic_seconds=time.monotonic(),job=job,stage=stage,
                owned_processes=own,top_external_cpu=sorted(external,key=lambda p:p['cpu_pct'],reverse=True)[:10],
                top_external_rss=sorted(external,key=lambda p:p['rss_bytes'],reverse=True)[:5],
                vm_stat=vm,vm_counters=counters,
                swap=subprocess.check_output(['sysctl','vm.swapusage'],text=True).strip())
    with (ROOT/'system-observations.jsonl').open('a') as stream:
        stream.write(json.dumps(record)+'\n')
    return record

def stop_owned(child):
    try:
        os.killpg(child.pid,signal.SIGTERM)
    except ProcessLookupError:
        return
    try:
        child.wait(timeout=5)
    except subprocess.TimeoutExpired:
        os.killpg(child.pid,signal.SIGKILL)
        child.wait()

def main():
    # A global campaign lock also prevents starting a second copy from another directory.
    with (REPO/'PerfRuns/.albert-sequential.lock').open('a') as lock:
        fcntl.flock(lock,fcntl.LOCK_EX|fcntl.LOCK_NB)
        if (ROOT/'status.json').exists():
            raise RuntimeError('This campaign already has execution records; do not restart it or overwrite logs.')
        expected=json.loads((ROOT/'source-sha256.json').read_text())
        assert all(hashlib.sha256((REPO/name).read_bytes()).hexdigest()==digest for name,digest in expected.items()), 'Source changed since preparation'
        rows=processes()
        other=[p for p in rows if Path(p['process']).name=='f2z' or Path(p['process']).name.startswith(('mul_e2e_compare-','sha256_ecdsa_compare-'))]
        if other:
            (ROOT/'preflight-conflicts.json').write_text(json.dumps(other,indent=2)+'\n')
            raise RuntimeError('Another benchmark process is active; see preflight-conflicts.json')
        snapshot('campaign','preflight',rows=rows)
        records=[]
        (ROOT/'controller.json').write_text(json.dumps({'pid':os.getpid(),'started_utc':utc(),'sequential':True})+'\n')
        for index,job in enumerate(JOBS):
            if index:
                print('COOLDOWN 20 seconds',flush=True)
                time.sleep(20)
            record=dict(name=job['name'],kind=job['kind'],status='running',started_utc=utc(),command=job['command'])
            records.append(record)
            save_status(records)
            snapshot(job['name'],'before')
            print(f'START {index+1}/{len(JOBS)} {job["name"]}',flush=True)
            started=time.monotonic()
            peak=0
            with (ROOT/'logs'/f'{job["name"]}.log').open('x') as output:
                child=subprocess.Popen(['/usr/bin/time','-l',*job['command']],cwd=REPO,env=BASE|job['environment'],stdout=output,stderr=subprocess.STDOUT,start_new_session=True)
                record['process_group']=child.pid
                save_status(records)
                last_snapshot=0
                try:
                    while child.poll() is None:
                        rows=processes()
                        rss=sum(p['rss_bytes'] for p in rows if p['pgid']==child.pid)
                        peak=max(peak,rss)
                        now=time.monotonic()
                        if now-last_snapshot>=POLICY['monitor_interval_seconds']:
                            snapshot(job['name'],'during',child.pid,rows)
                            last_snapshot=now
                        if rss>POLICY['max_owned_process_group_rss_bytes']:
                            record['status']='resource_limit'
                            stop_owned(child)
                            break
                        time.sleep(2)
                except BaseException:
                    stop_owned(child)
                    record.update(status='interrupted',finished_utc=utc(),exit_code=child.returncode)
                    save_status(records)
                    raise
                code=child.wait()
            record.update(exit_code=code,finished_utc=utc(),elapsed_seconds=time.monotonic()-started,observed_process_group_peak_rss_bytes=peak)
            if record['status']=='running':
                record['status']='complete' if code==0 else 'failed'
            save_status(records)
            snapshot(job['name'],'after')
            print(f'END {job["name"]}: {record["status"]} ({record["elapsed_seconds"]:.1f}s)',flush=True)
            if record['status']!='complete':
                print('Stopped before the next command; failure and logs preserved.',flush=True)
                return 1
        (ROOT/'execution-complete.json').write_text(json.dumps({'status':'complete','jobs':len(records),'finished_utc':utc(),'sequential':True},indent=2)+'\n')
        print('SEQUENTIAL CAMPAIGN FINISHED',flush=True)
        return 0

if __name__=='__main__':
    sys.exit(main())

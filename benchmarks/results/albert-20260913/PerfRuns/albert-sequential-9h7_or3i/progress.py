#!/usr/bin/env python3
"""Read compact progress; never run a benchmark or alter measurements."""
import datetime
import json
from pathlib import Path
import re

root=Path(__file__).resolve().parent
records=json.loads((root/'status.json').read_text())
active=next((r for r in records if r['status']=='running'),None)
result={'completed_jobs':sum(r['status']=='complete' for r in records),'total_jobs':26}
if active:
    now=datetime.datetime.now(datetime.timezone.utc)
    result.update(active_job=active['name'],elapsed_minutes=round((now-datetime.datetime.fromisoformat(active['started_utc'])).total_seconds()/60,1))
    path=root/'logs'/f"{active['name']}.log"
    if path.exists():
        lines=[l for l in path.read_text(errors='replace').splitlines() if l.startswith(('u32-mod32 ','f2z --mul:','RESULT schema=','test result:','error:','Finished ','   Compiling'))]
        result['latest_log_events']=[' '.join(s for s in line.split() if not s.startswith('ligerito_hex=')) for line in lines[-2:]]
snapshots=[]
with (root/'system-observations.jsonl').open() as stream:
    for line in stream:
        try: snapshots.append(json.loads(line))
        except json.JSONDecodeError: pass
if snapshots:
    s=snapshots[-1]
    result['latest_snapshot_utc']=s['utc']
    result['owned_cpu_pct']=round(sum(p['cpu_pct'] for p in s['owned_processes']),1)
    result['owned_rss_gib']=round(sum(p['rss_bytes'] for p in s['owned_processes'])/2**30,2)
    result['top_external_cpu']=[{'process':Path(p['process']).name,'cpu_pct':p['cpu_pct']} for p in s['top_external_cpu'][:3]]
    if len(snapshots)>1:
        before=snapshots[-2]
        seconds=s['monotonic_seconds']-before['monotonic_seconds']
        page_size=int(re.search(r'page size of (\d+) bytes',s['vm_stat']).group(1))
        result['system_swap_mib_per_second']={k:round((s['vm_counters'][k]-before['vm_counters'][k])*page_size/2**20/seconds,2) for k in ('Swapins','Swapouts')}
print(json.dumps(result,indent=2))

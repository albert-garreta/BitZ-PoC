#!/usr/bin/env python3
"""Short read-only host load observation; run outside the PID sandbox."""
import argparse,json,os,time
from pathlib import Path
p=argparse.ArgumentParser();p.add_argument('--output',type=Path,required=True);a=p.parse_args();assert not a.output.exists();clk=os.sysconf('SC_CLK_TCK')
BUILD={'cargo','rustc','ld','ld.lld','cc','gcc','clang','ninja','make'}
def snapshot():
 out={}
 for f in Path('/proc').iterdir():
  if not f.name.isdigit():continue
  try:
   text=(f/'stat').read_text();left=text.index('(');right=text.rindex(')');rest=text[right+2:].split();out[int(f.name)]={'comm':text[left+1:right],'ticks':int(rest[11])+int(rest[12]),'ppid':int(rest[1])}
  except (OSError,ValueError,IndexError):continue
 return out
def cpu():
 v=list(map(int,Path('/proc/stat').read_text().splitlines()[0].split()[1:9]));return sum(v),v[3]+v[4]
started=time.time();before=snapshot();c0=cpu();t=time.monotonic();time.sleep(2);after=snapshot();c1=cpu();elapsed=time.monotonic()-t;rows=[]
for pid,d in after.items():
 if pid not in before:continue
 usage=(d['ticks']-before[pid]['ticks'])/clk/elapsed*100
 if usage>0:rows.append({'pid':pid,'ppid':d['ppid'],'comm':d['comm'],'cpu_percent_one_core':usage})
rows.sort(key=lambda x:x['cpu_percent_one_core'],reverse=True)
builds={k:[{'pid':pid,**d} for pid,d in v.items() if d['comm'] in BUILD] for k,v in [('start',before),('end',after)]}
r={'captured_at':started,'elapsed_seconds':elapsed,'method':'Host /proc process CPU-time deltas across two seconds; names only, no arguments or environment','host_cpu_idle_percent':100*(c1[1]-c0[1])/max(1,c1[0]-c0[0]),'active_processes':rows,'compiler_build_processes':builds,'processes_seen_start':len(before),'processes_seen_end':len(after),'limitation':'Only this interval is observed. Short exited jobs between snapshots and prior overlap cannot be ruled out. No jobs terminated or settings changed.'};a.output.write_text(json.dumps(r,indent=2)+'\n');print(json.dumps({'top_active':rows[:8],'compiler_build_processes':builds,'host_cpu_idle_percent':r['host_cpu_idle_percent']},indent=2))

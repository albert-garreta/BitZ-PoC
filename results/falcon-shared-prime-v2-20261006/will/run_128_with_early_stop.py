from pathlib import Path
import json,signal,subprocess,time
out=Path('.tmp/falcon-v2/will-discovery128')
command=['python3','scripts/bench_gate.py','run','--label','falcon-v2-will-discovery128','--min-idle','88','--hold-seconds','10','--poll-seconds','2','--max-idle-wait-seconds','300','--','python3','scripts/falcon_v2_campaign.py','.tmp/falcon-v2/manifest.json','--output',str(out),'--phase','discovery','--security','128','--batches','1024','32','3','1','--threads','16','1','--seeds','42','43','44','45','46','--warmup','1','--iterations','3','--stop-on-regression']
with Path('.tmp/falcon-v2/will-discovery128.log').open('w') as log:
 process=subprocess.Popen(command,stdout=log,stderr=subprocess.STDOUT)
 early=False
 while process.poll() is None:
  files={label:out/f's128-b1024-t16-seed42-{label}.jsonl' for label in ['native','v1','v2']}
  warmups={}
  for label,path in files.items():
   if not path.exists(): continue
   for line in path.read_text().splitlines():
    try: r=json.loads(line)
    except json.JSONDecodeError: continue
    if r.get('event')=='trial' and r.get('trial')=='warmup' and r.get('verified'):
     warmups[label]=r;break
  if len(warmups)==3 and all(warmups['v2']['total_prover_ms']>10*warmups[x]['total_prover_ms'] for x in ['native','v1']):
   record=dict(reason='Early stop under requested regression policy: verified V2 warmup exceeds both matched baseline warmups by more than 10x. No completed V2 measured process or paired confidence interval; diagnostic only.',candidate=warmups['v2'],baseline_warmups={x:warmups[x] for x in ['native','v1']},stopped_at=time.time(),owned_gate_pid=process.pid)
   (out/'early-stop.json').write_text(json.dumps(record,indent=2)+'\n')
   process.send_signal(signal.SIGTERM)
   early=True;break
  time.sleep(2)
 code=process.wait()
 print(json.dumps(dict(gate_exit_code=code,early_stop=early,output=str(out))))
 if not early and code: raise SystemExit(code)

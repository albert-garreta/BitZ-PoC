import json,os,pathlib,subprocess,time
base=pathlib.Path('results/falcon-grinding-opt-128-d9b13bdb4')
out=pathlib.Path(__file__).resolve().parent
for kind in ('arithmetic','full'):
 for degree in (512,1024):
  for threads in (1,8,16):
   stem=f'{kind}-{degree}-t{threads}'
   cmd=json.loads((base/f'{stem}.command.json').read_text())
   argv=cmd['argv'][:]; argv[argv.index('--iterations')+1]='3'
   if kind=='arithmetic': argv+=['--trace']
   env=os.environ|cmd['environment']|{'BITZ_FALCON_STAGE_TIMINGS':'1'}
   (out/f'{stem}.command.json').write_text(json.dumps({'argv':argv,'environment':cmd['environment']|{'BITZ_FALCON_STAGE_TIMINGS':'1'},'source_commit':'d9b13bdb4','purpose':'diagnostic stage timings; not replacement latency benchmarks'},indent=2))
   start=time.monotonic()
   with (out/f'{stem}.jsonl').open('w') as stdout, (out/f'{stem}.stages.jsonl').open('w') as stderr:
    p=subprocess.run(argv,env=env,stdout=stdout,stderr=stderr)
   print(stem,p.returncode,round(time.monotonic()-start,2),flush=True)
   if p.returncode: raise SystemExit(p.returncode)

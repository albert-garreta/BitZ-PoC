#!/usr/bin/env python3
"""Supplemental generic-vs-historical view at 2^17 rows, ordinary and skip K=3."""
import gzip,json
from pathlib import Path
root=Path(__file__).parent
with (root/'generic-trace.jsonl').open('w') as out:
 for session in (1,2):
  for name in ('baseline','generic'):
   path=root/'confirmation'/f'{session}-{name}.log'
   lines=path.read_text().splitlines() if path.exists() else gzip.open(str(path)+'.gz','rt').read().splitlines()
   for line in lines:
    if not line.startswith('OUTER_SAMPLE '):continue
    x=json.loads(line[13:])
    if x['rows']!=131072 or x['protocol'] not in ('ordinary','skip-3'):continue
    series=f"u{x['bits']}-{x['protocol']}-{name}-session{session}"
    rid=f"{series}-{x['sample']}"
    run={'schema':'zkperf.trace/v1','record':'run','run_id':rid,'series_id':series,'root_span_id':'outer','benchmark':{'suite':'f2z','name':'outer-regression-a3450385','label':series,'algorithm':'F2Z multiplication outer sumcheck','implementation':name,'git_rev':x['revision'],'build_profile':'bench'},'trial':{'kind':'warmup','warmup_index':0} if x['warmup'] else {'kind':'sample','sample_index':x['sample']-1},'clock':{'id':rid,'kind':'monotonic','unit':'ns','source':'std::time::Instant'},'status':'ok','trace_complete':True,'environment':{'os':'macOS','arch':'aarch64','cpu':'Apple M1 Max','threads':x['threads']},'parameters':{'input':{'rows':x['rows'],'operand_bits':x['bits'],'protocol':x['protocol']},'repetition':{'count':1}}}
    span={'schema':'zkperf.trace/v1','record':'span','run_id':rid,'span_id':'outer','parent_span_id':None,'operation':'outer.complete-call','name':'Outer call including equality setup and required projection','primary_phase':'proving','phase_tags':['end-to-end','proving','sumcheck'],'start_ns':'0','end_ns':x['ns'],'duration_ns':x['ns'],'lane':{'process':'prover','thread':'driver','task':'outer'},'coordinate':{'occurrence_index':0,'occurrence_count':1},'attributes':{'scope_kind':'scope','primary_sequence':True,'math_latex':[r'\sum_x \operatorname{eq}(\tau,x)(A(x)B(x)-C(x))=0']}}
    out.write(json.dumps(run)+'\n'+json.dumps(span)+'\n')

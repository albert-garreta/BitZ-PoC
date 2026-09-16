#!/usr/bin/env python3
import json,sys,gzip
from pathlib import Path
src,dest=map(Path,sys.argv[1:])
metadata=json.loads((src.parent/'metadata.json').read_text())
with dest.open('w') as out:
 lines=gzip.open(src,"rt").read().splitlines() if src.suffix==".gz" else src.read_text().splitlines()
 for line in lines:
  if not line.startswith('OUTER_SAMPLE '):continue
  r=json.loads(line[len('OUTER_SAMPLE '):]);series=f"{r['input']}-{r['protocol']}-n{r['rows']}-t{r['threads']}-{r['candidate']}";rid=f"{series}-{r['sample']}"
  run={'schema':'zkperf.trace/v1','record':'run','run_id':rid,'series_id':series,'root_span_id':'outer','benchmark':{'suite':'f2z','name':'outer-sumcheck','label':series,'algorithm':'F2Z outer sumcheck','implementation':r['candidate'],'git_rev':metadata['revision']+'+experimental-patch','build_profile':'bench'},'trial':{'kind':'warmup','warmup_index':0} if r['warmup'] else {'kind':'sample','sample_index':r['sample']-1},'clock':{'id':'rust-Instant/'+rid,'kind':'monotonic','unit':'ns','source':'std::time::Instant'},'status':'ok','trace_complete':True,'environment':{'os':'macOS','arch':'aarch64','cpu':metadata['cpu'],'threads':r['threads']},'parameters':{'input':{'num_rows':r['rows'],'representation':r['input'],'protocol':r['protocol']},'repetition':{'count':1}}}
  out.write(json.dumps(run)+'\n')
  spans=[('outer',0,int(r['ns']))]+r.get('spans',[])
  for i,(name,start,end) in enumerate(spans):
   span={'schema':'zkperf.trace/v1','record':'span','run_id':rid,'span_id':'outer' if i==0 else str(i),'parent_span_id':None if i==0 else 'outer','operation':'outer.'+name,'name':name.replace('_',' '),'primary_phase':'proving','phase_tags':['proving','sumcheck']+(['end-to-end'] if i==0 else []),'start_ns':str(start),'end_ns':str(end),'duration_ns':str(end-start),'lane':{'process':'prover','thread':'driver','task':'outer'},'coordinate':{'occurrence_index':0,'occurrence_count':1},'attributes':{'scope_kind':'scope' if i==0 else 'phase','primary_sequence':i>0,'math_latex':[r'\sum_x \mathrm{eq}(\tau,x)(A(x)B(x)-C(x))']}}
   out.write(json.dumps(span)+'\n')

import json,pathlib,statistics,collections
p=pathlib.Path(__file__).resolve().parent
all_results=[]
for f in sorted(p.glob('*.stages.jsonl')):
 name=f.name.removesuffix('.stages.jsonl');kind,degree,thread=name.split('-')
 events=[json.loads(l) for l in f.read_text().splitlines() if l.startswith('{')]
 events=[r for r in events if r.get('event')=='stage']
 root_name='falcon_algebraic:prove' if kind=='arithmetic' else 'falcon_bench:prove'
 roots=[r for r in events if r['name']==root_name]
 assert len(roots)==4,(name,len(roots))
 runs=[]; paths=collections.defaultdict(list);bits=collections.defaultdict(list)
 for root in roots[1:]:
  selected=[r for r in events if r['ancestor_ids'][0]==root['span_id']]
  grinding_names={'spartan:grinding','lig:grind_pow'}
  grinds=[r for r in selected if r['name'] in grinding_names and not grinding_names.intersection(r['path'][:-1])]
  g=sum(r['elapsed_ms'] for r in grinds)
  run={'prove_ms':root['elapsed_ms'],'grinding_ms':g,'grinding_pct':g/root['elapsed_ms']*100}
  grouped=collections.defaultdict(float)
  for r in selected:grouped['/'.join(r['path'])]+=r['elapsed_ms']
  for k,v in grouped.items():paths[k].append(v)
  runs.append(run)
 for r in events:
  if r['name']=='spartan:grinding':bits['/'.join(r['path'])].append(r['fields']['bits'])
 result={'variant':kind,'degree':int(degree),'threads':int(thread[1:]),'medians':{k:statistics.median(r[k] for r in runs) for k in runs[0]},'samples':runs,'path_medians':{k:statistics.median(v) for k,v in paths.items()},'grinding_bits_by_path':{k:dict(collections.Counter(v)) for k,v in bits.items()}}
 all_results.append(result)
 print(name,json.dumps(result['medians']))
 for path,value in sorted(result['path_medians'].items(),key=lambda x:-x[1]):
  if path.endswith(('spartan:grinding','lig:grind_pow')):print(' ',round(value,2),path)
(p/'profile-analysis.json').write_text(json.dumps(all_results,indent=2)+'\n')

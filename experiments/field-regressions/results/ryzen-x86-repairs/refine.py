import copy,hashlib,json,subprocess,sys
from pathlib import Path

HERE=Path(__file__).resolve().parents[2]
sys.path.insert(0,str(HERE))
from analysis import classify
from run import planned_seeds

primary=Path(sys.argv[1])
exploration=Path(sys.argv[2])
out=Path(sys.argv[3])
assert json.loads((primary/'campaign.json').read_text())['status']=='complete'
out.mkdir(exist_ok=False)
jobs=[]
for index,label in enumerate(['l3-96m','l3-32m','l3-96m-8t','l3-32m-8t','physical-16t','smt-32t']):
 d=primary/label
 metadata=json.loads((d/'metadata.json').read_text())
 spec=json.loads((d/'required_cases.json').read_text())
 rows={(r['family'],r['size'],r['variant']):r for r in json.loads((d/'summary.json').read_text())}
 rankings={(r['family'],r['size']):r for r in json.loads((d/'rankings.json').read_text())}
 initial_rankings={(r['family'],r['size']):r for r in json.loads((exploration/label/'rankings.json').read_text())}
 selection=json.loads((d/'selection.json').read_text())
 refinements=[]
 for group in spec['families']:
  if group['name']=='integer_full_mac':
   continue  # Keep one preconfirmed public-shape choice across both fixtures.
  for size in group['sizes']:
   key=(group['name'],size);rank=rankings[key]
   proposed=group['selected']['x86_64'];candidate=group.get('challenger')
   if classify(rows[*key,proposed],.01)=='pass' and rank.get('head_to_head',{}).get('decision','single_implementation') in ('faster','within_1_percent','single_implementation'):
    continue
   if not candidate or candidate==group['baseline']:
    continue
   row=rows[*key,candidate]
   if classify(row,.01)!='pass' or row['median_ci_high']>=1:
    continue
   # This data is development evidence for a different, frozen proposal.
   # It is never reused as that proposal's independent confirmation.
   challenger=next((v['variant'] for v in initial_rankings[key]['ranking'] if v['variant']!=candidate),None)
   case='/'.join(key)
   selection['choices'][case]=dict(variant=candidate,challenger=challenger)
   refinements.append(dict(case=case,previous=proposed,proposed=candidate,challenger=challenger))
 # Refine integer choices as a public-shape group, never one fixture alone.
 integer_shapes={}
 for group in spec['families']:
  if group['name']=='integer_full_mac':
   for size in group['sizes']:
    integer_shapes.setdefault(size.split('_',1)[1],[]).append((group,size))
 for shape,fixtures in integer_shapes.items():
  assert len(fixtures)==2
  old={g['selected']['x86_64'] for g,s in fixtures}
  assert len(old)==1
  old=next(iter(old))
  if all(classify(rows[g['name'],s,old],.01)=='pass' and rankings[g['name'],s].get('head_to_head',{}).get('decision','single_implementation') in ('faster','within_1_percent','single_implementation') for g,s in fixtures):
   continue
  common=set.intersection(*(set(g['variants']) for g,s in fixtures))-{old,'circuit_z'}
  eligible=[v for v in common if all(classify(rows[g['name'],s,v],.01)=='pass' and rows[g['name'],s,v]['median_ci_high']<1 for g,s in fixtures)]
  if not eligible:
   continue
  candidate=min(eligible,key=lambda v:(max(rows[g['name'],s,v]['median_ratio'] for g,s in fixtures),v))
  for group,size in fixtures:
   key=(group['name'],size)
   challenger=next((v['variant'] for v in initial_rankings[key]['ranking'] if v['variant']!=candidate),None)
   case='/'.join(key)
   selection['choices'][case]=dict(variant=candidate,challenger=challenger)
   refinements.append(dict(case=case,previous=old,proposed=candidate,challenger=challenger,public_shape_group=shape))
 if not refinements:
  continue
 used=set(selection['used_seeds'])
 for name in metadata['round_metadata_sha256']:
  rm=json.loads((d/name/'metadata.json').read_text())
  used.update(rm['seed_offset']+7919*i for i in range(rm['runs']))
 selection['used_seeds']=sorted(used)
 seed=77182817+index*1000003
 assert not planned_seeds(seed,5,True)&used
 selection_path=out/(label+'-selection.json')
 selection_path.write_text(json.dumps(selection,indent=2)+'\n')
 command=[sys.executable,str(HERE/'run.py'),'--suite','x86','--arch','x86_64',
  '--scope',metadata['scope'],'--cpu-set',metadata['cpu_set'],'--threads',str(metadata['threads']),
  '--phase','confirm','--selection',str(selection_path),'--seed-offset',str(seed),
  '--cases',','.join(r['case'] for r in refinements),'--out',str(out/label),
  '--target-dir','/tmp/bitz-ryzen-target']
 if metadata.get('stream_n'):
  command.append('--streaming')
 jobs.append(dict(label=label,refinements=refinements,command=command,
  development=str(d),development_summary_sha256=hashlib.sha256((d/'summary.json').read_bytes()).hexdigest()))
plan=dict(policy='New choices use the primary data only for development; five fresh processes and 32 paired samples (one predeclared 64-sample retry) independently confirm them. No source changes or threshold changes.',jobs=jobs,status='planned')
(out/'plan.json').write_text(json.dumps(plan,indent=2)+'\n')
print(json.dumps([{k:j[k] for k in ['label','refinements']} for j in jobs],indent=2),flush=True)
for job in jobs:
 print('Confirming refined choices:',job['label'],flush=True)
 subprocess.run(job['command'],cwd=HERE.parents[1],check=True)
plan['status']='complete'
(out/'plan.json').write_text(json.dumps(plan,indent=2)+'\n')

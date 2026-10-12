"""Run the frozen per-workload candidate against both baseline controls."""
import json,os,random
import campaign

def main():
 selection=json.loads((campaign.OUT/'selection.json').read_text())
 os.sched_setaffinity(0,{31});rng=random.Random(20261010)
 for seed in selection['holdout_seeds']:
  configs=selection['candidates'].copy();rng.shuffle(configs)
  for config in configs:
   profiles=['original','baseline',config['profile']];rng.shuffle(profiles)
   rows=[campaign.run('confirmation',config['variant'],config['degree'],config['threads'],seed,p,7) for p in profiles]
   assert len({r['input_digest'] for r in rows})==1
   controls=[r for r in rows if r['profile'] in ('baseline','original')]
   assert len({r['proof_digest'] for r in controls})==1
 print('FRESH-INPUT CONFIRMATION COMPLETE',flush=True)
if __name__=='__main__':main()

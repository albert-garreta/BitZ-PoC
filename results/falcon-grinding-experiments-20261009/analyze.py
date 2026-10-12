#!/usr/bin/env python3
import argparse,collections,csv,json,math,pathlib,random,statistics
OUT=pathlib.Path(__file__).resolve().parent

def summarize(phase,reference='baseline'):
 rows=[json.loads(f.read_text()) for f in (OUT/phase).glob('*.result.json')]
 groups=collections.defaultdict(list)
 baseline={(r['variant'],r['degree'],r['threads'],r['seed']):r for r in rows if r['profile']==reference}
 for row in rows:groups[(row['variant'],row['degree'],row['threads'],row['profile'])].append(row)
 summary=[];rng=random.Random(20261009)
 for (kind,degree,threads,profile),rs in sorted(groups.items()):
  pairs=[(r,baseline[(kind,degree,threads,r['seed'])]) for r in rs if (kind,degree,threads,r['seed']) in baseline]
  if len(pairs)!=len(rs):continue
  n=len(pairs)
  result={'variant':kind,'degree':degree,'threads':threads,'profile':profile,'reference':reference,'seeds':n,'verified_proofs':sum(r['verified_proofs'] for r in rs),'security_bits_min':min(r['security_bits'] for r in rs),'proof_bytes_mean':statistics.mean(r['proof_bytes'] for r in rs)}
  for metric in ('commit','prove','total','verify'):
   key=metric+'_ms';ratios=[r[key]/b[key] for r,b in pairs];logs=[math.log(x) for x in ratios]
   result[metric+'_mean_ms']=statistics.mean(r[key] for r in rs)
   result[metric+'_median_ms']=statistics.median(r[key] for r in rs)
   result[metric+'_paired_ratio']=math.exp(statistics.mean(logs))
   boots=sorted(math.exp(statistics.mean(rng.choices(logs,k=n))) for _ in range(5000))
   result[metric+'_ratio_ci95_low']=boots[125];result[metric+'_ratio_ci95_high']=boots[4874]
   result[metric+'_mean_ratio']=sum(r[key] for r,b in pairs)/sum(b[key] for r,b in pairs)
  summary.append(result)
 (OUT/phase/'summary.json').write_text(json.dumps(summary,indent=2)+'\n')
 if summary:
  with (OUT/phase/'summary.csv').open('w') as f:
   w=csv.DictWriter(f,fieldnames=list(summary[0]));w.writeheader();w.writerows(summary)
 for r in summary:
  print(f'{r["variant"]} {r["degree"]} t{r["threads"]} {r["profile"]}: n={r["seeds"]} total mean={r["total_mean_ms"]:.2f}ms paired={r["total_paired_ratio"]:.4f} CI=[{r["total_ratio_ci95_low"]:.4f},{r["total_ratio_ci95_high"]:.4f}] prove={r["prove_mean_ms"]:.2f}ms verify={r["verify_mean_ms"]:.2f}ms bytes={r["proof_bytes_mean"]:.0f}')
 return summary
if __name__=='__main__':
 p=argparse.ArgumentParser();p.add_argument('phase');p.add_argument('--reference',default='baseline');args=p.parse_args();summarize(args.phase,args.reference)

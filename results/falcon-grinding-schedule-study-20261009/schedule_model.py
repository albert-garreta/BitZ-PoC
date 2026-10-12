"""Normalized-error cost model; analytical candidates, not proof benchmarks."""
import json,math,pathlib,itertools,copy
p=pathlib.Path(__file__).resolve().parent
survey=[json.loads(l) for l in (p/'pcs-survey.jsonl').read_text().splitlines()]
def group(name,raw,sites,minimum=0):return dict(name=name,raw=raw,sites=sites,minimum=minimum)
def error(gs,b):return sum(g['raw']*2**-v for g,v in zip(gs,b))
def work(gs,b):return sum(g['sites']*2**v for g,v in zip(gs,b))
def allocate(gs,budget):
 b=[g['minimum'] for g in gs]
 while error(gs,b)>budget:
  i=max((i for i in range(len(gs)) if b[i]<32 and gs[i]['raw']),key=lambda i:gs[i]['raw']/gs[i]['sites']*2**(-2*b[i]));b[i]+=1
 for i in sorted(range(len(gs)),key=lambda i:-gs[i]['sites']*2**b[i]):
  while b[i]>gs[i]['minimum']:
   b[i]-=1
   if error(gs,b)>budget:b[i]+=1;break
 return b
def swaps(gs,b,budget):
 b=b[:]
 while True:
  oldw=work(gs,b);olde=error(gs,b);best=None
  for i,j in itertools.permutations(range(len(gs)),2):
   for down in range(1,min(3,b[i]-gs[i]['minimum'])+1):
    for up in range(1,min(3,32-b[j])+1):
     e=olde+gs[i]['raw']*2**-b[i]*(2**down-1)+gs[j]['raw']*2**-b[j]*(2**-up-1)
     w=oldw+gs[i]['sites']*2**b[i]*(2**-down-1)+gs[j]['sites']*2**b[j]*(2**up-1)
     if e<=budget and w<oldw and (best is None or w<best[0]):best=(w,i,j,down,up)
  if best is None:return b
  _,i,j,d,u=best;b[i]-=d;b[j]+=u
out=[]
for kind,degree in itertools.product(('arithmetic','full'),(512,1024)):
 nlog=int(math.log2(degree));packed_log=nlog+(9 if kind=='arithmetic' else 14);bitlog=packed_log+7
 col=nlog+(2 if kind=='arithmetic' else 4);s=col+1
 binary=[group('bridge',s+3*(78+13*s)+13,78+13*(col+1)+13)]
 if kind=='full':binary.append(group('links',256,1))
 binary +=[group('joint',2*bitlog,bitlog),group('opening',256,packed_log+8)]
 if kind=='full':
  for i,m in enumerate([29,27,26] if degree==512 else [30,28]):binary.append(group(f'keccak/{i}',4*m+256,m+8))
 primes=[];primebits=[]
 if kind=='full':
  base=pathlib.Path('results/falcon-grinding-opt-128-d9b13bdb4');terms=json.loads((base/f'full-{degree}-t1.jsonl').read_text().splitlines()[0])['security_terms']
  fixed=sum(t['error_bound']*2**128 for t in terms[:9]);unalloc=sum(terms[i]['error_bound']*2**128 for i in [0,5])
  sites=[1,1,1,nlog+11,1,1,nlog+17];bs=[12,8,13,14,14,19,14]
  for t,site,b in zip([terms[i] for i in [1,2,3,4,6,7,8]],sites,bs):primes.append(group(t['name'],t['error_bound']*2**(128+b),site));primebits.append(b)
 else:
  unalloc=2**-16+(2*degree-2+10)*2**-21
  for name,num,sites in [('projection',10,1),('norm',10,1),('outer',3*(nlog+1+10),nlog+11),('merge',3,1),('binding',2*(nlog+5+10),nlog+15)]:
   primes.append(group(name,8*num,sites));primebits.append(7+(num-1).bit_length())
  fixed=unalloc+error(primes,primebits)
 budget=15/16-fixed
 shapes=[]
 for row in survey:
  if row['packed_log']!=packed_log:continue
  gs=binary+[group(b['label'],b['raw_normalized'],1,b['min_bits']) for b in row['blocks']]
  bits=allocate(gs,budget)
  shapes.append(dict(rate=row['rate'],initial_k=row['initial_k'],modeled_hashes=work(gs,bits),security_bits=128-math.log2(fixed+error(gs,bits))))
  if row['rate']==1 and row['initial_k']==4:baseline_gs=gs;baseline_bits=bits
 gs=baseline_gs;b=baseline_bits;basework=work(gs,b)+work(primes,primebits)
 baseline_security=128-math.log2(fixed+error(gs,b))
 expected=json.loads(pathlib.Path(f'results/falcon-grinding-opt-128-d9b13bdb4/{kind}-{degree}-t1.jsonl').read_text().splitlines()[0])['algebraic_security_bits']
 assert abs(expected-baseline_security)<1e-10,(kind,degree,expected,baseline_security)
 cases=[]
 candidates=[('pairwise swaps',gs,swaps(gs,b,budget),fixed,work(primes,primebits)),('prime groups included',gs+primes,allocate(gs+primes,15/16-unalloc),unalloc,0)]
 for title,gg,bb,ff,pw in candidates:
  cases.append(dict(name=title,modeled_hashes=work(gg,bb)+pw,reduction_pct=100*(1-(work(gg,bb)+pw)/basework),security_bits=128-math.log2(ff+error(gg,bb)),schedule=[dict(**g,bits=v) for g,v in zip(gg,bb)]))
 result=dict(variant=kind,degree=degree,baseline_modeled_hashes=basework,baseline_security_bits=baseline_security,baseline_schedule=[dict(**g,bits=v) for g,v in zip(gs+primes,b+primebits)],candidates=cases,geometry_candidates=shapes)
 out.append(result)
 print(kind,degree,'baseline hashes',int(basework),'security',baseline_security)
 for c in cases:print(' ',c['name'],'hash reduction',round(c['reduction_pct'],2),'security',c['security_bits'])
 print(' geometry (PCS+binary modeled hashes only):',[(x['rate'],x['initial_k'],round(x['modeled_hashes']/work(gs,b),3)) for x in shapes])
(p/'schedule-model.json').write_text(json.dumps(out,indent=2)+'\n')

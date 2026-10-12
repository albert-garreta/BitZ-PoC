import collections,csv,json,math,pathlib,statistics

out=pathlib.Path(__file__).resolve().parent
groups=collections.defaultdict(list)
for raw in csv.DictReader((out/'raw.csv').open()):
    r={k:int(v) if k not in ('kernel','valid') else v for k,v in raw.items()}
    assert r['valid']=='true'
    if r['sample']>=0: groups[r['threads'],r['kernel'],r['bits']].append(r)

def fit(rows):
    xs=[r['attempts'] for r in rows]
    ys=[r['elapsed_ns'] for r in rows]
    scale=max(1,statistics.mean(xs))
    xs=[x/scale for x in xs]
    weights=[1.0]*len(xs)
    for _ in range(20):
        s=sum(weights)
        xm=sum(w*x for w,x in zip(weights,xs))/s
        ym=sum(w*y for w,y in zip(weights,ys))/s
        den=sum(w*(x-xm)**2 for w,x in zip(weights,xs))
        slope=max(0,sum(w*(x-xm)*(y-ym) for w,x,y in zip(weights,xs,ys))/den) if den else 0
        intercept=ym-slope*xm
        if intercept<0:
            intercept=0
            slope=sum(w*x*y for w,x,y in zip(weights,xs,ys))/sum(w*x*x for w,x in zip(weights,xs))
        residuals=[y-intercept-slope*x for x,y in zip(xs,ys)]
        center=statistics.median(residuals)
        mad=max(20,1.4826*statistics.median(abs(v-center) for v in residuals))
        weights=[min(1,1.345*mad/max(1,abs(r))) for r in residuals]
    return intercept,slope/scale,sum(1 for w in weights if w<0.5)

records=[]
curves={}
raw_curves={}
highfits={}
for threads in (1,2,4,8,16):
    curves[str(threads)]={}
    raw_curves[str(threads)]={}
    highfits[str(threads)]={}
    for kernel in ('spartan','pcs'):
        values=[0.0]*33
        for bits in range(25):
            if (threads,kernel,bits) not in groups: continue
            rows=groups[threads,kernel,bits]
            intercept,slope,downweighted=fit(rows)
            expected=intercept+slope*2**bits
            times=[r['elapsed_ns'] for r in rows]
            attempts=[r['attempts'] for r in rows]
            records.append({'threads':threads,'kernel':kernel,'bits':bits,'samples':len(rows),
                'mean_ns':statistics.mean(times),'median_ns':statistics.median(times),
                'min_ns':min(times),'max_ns':max(times),'mean_attempts':statistics.mean(attempts),
                'median_ns_per_logical_attempt':statistics.median(t/a for t,a in zip(times,attempts)),
                'aggregate_ns_per_logical_attempt':sum(times)/sum(attempts),
                'fitted_overhead_ns':intercept,'fitted_ns_per_logical_attempt':slope,
                'expected_ns_at_2pow_bits':expected,'strongly_downweighted_samples':downweighted,
                'worker_indices':sorted({r['worker'] for r in rows}),'cpus':sorted({r['cpu'] for r in rows})})
            if bits: values[bits]=expected
        # Extrapolation preserves measured bit24 and doubles fitted long-search work.
        highrows=[r for b in range(21,25) for r in groups[threads,kernel,b]]
        intercept,slope,_=fit(highrows)
        highfits[str(threads)][kernel]={'intercept_ns':intercept,'ns_per_logical_attempt':slope,'fit_bits':[21,22,23,24]}
        for bits in range(25,33): values[bits]=values[24]+slope*(2**bits-2**24)
        raw_curves[str(threads)][kernel]=values
        # An upper monotone envelope retains actual short-search costs. The
        # bit12 dispatch makes true curves nonmonotone; never present this
        # envelope as the observed timing of individual searches.
        curves[str(threads)][kernel]=list(__import__('itertools').accumulate(values,max))

# The deterministic smallest nonce must be identical across worker counts.
nonces=collections.defaultdict(set)
for (threads,kernel,bits),rows in groups.items():
    for r in rows: nonces[kernel,bits,r['sample']].add(r['nonce'])
assert all(len(v)==1 for v in nonces.values())

result={'units':'nanoseconds','curves':curves,'curves_unconstrained':raw_curves,'high_bit_fits':highfits,
    'fit':'Per (kernel,thread,bits) Huber robust affine fit of elapsed_ns versus nonce+1, nonnegative overhead and slope; evaluated at 2^bits. curves applies a cumulative-maximum monotone upper envelope; curves_unconstrained preserves threshold drops. bits25..32 extrapolate from robust bits21..24 throughput, anchored at measured bit24.',
    'zero_bits':'C(0)=0 means grinding disabled. PCS grind_pow(0) wrapper overhead is separately measured in summary; it samples/absorbs but searches no nonce.',
    'caveats':['nonce+1 counts the required ordered prefix, not redundant parallel in-flight hashes; measured intercept includes dispatch and overscan.',
               'Parallel begins at bit12. On this AVX512 host chunks are256 for bits12..16 and1024 thereafter.',
               'Spartan uses a32-byte prefix; PCS ZincChallenger uses16bytes and includes transcript seed extraction/nonce absorption.',
               'No tracing subscriber or transcript construction in timed regions. Every grinder is invoked by external non-Rayon mainCPU6; parallel kernel includes blocking dispatch and return.',
               '250ms high-bit preconditioning before every process. Threads1/2/4 use128samples<=18 and64samples>=19; threads8/16 use1024samples<=18 and512samples>=19. Three excluded warmups percell.',
               'Microbenchmarks characterize isolated repeated grinding; full proof data can have different worker sleep/cache/clock state.'],
    'validation':{'all_nonces_valid':True,'all_nonces_identical_across_worker_counts':True,'measured_samples':sum(map(len,groups.values())),'sample_cells':len(groups)}}
(out/'curves.json').write_text(json.dumps(result,indent=2)+'\n')
(out/'summary.json').write_text(json.dumps(records,indent=2)+'\n')
with (out/'summary.csv').open('w') as f:
    w=csv.DictWriter(f,fieldnames=list(records[0]));w.writeheader();w.writerows(records)
print(json.dumps(result['validation']))
for t in ('1','2','4','8','16'):
    for k in ('spartan','pcs'):
        print(t,k,{b:round(raw_curves[t][k][b]/1000,3) for b in (8,11,12,13,14,16,18,20,22,24)})

#!/usr/bin/env python3
"""Render the measured tables from the campaign's validated summaries."""
import json
from pathlib import Path
OUT=Path(__file__).resolve().parent
rows=json.loads((OUT/'summary.json').read_text())['rows']
profiles=json.loads((OUT/'profile-summary.json').read_text())['rows']
validation=json.loads((OUT/'validation.json').read_text())
def row(phase,variant,n,t,mode='repeated'):
 return next(r for r in rows if (r['phase'],r['variant'],r['degree'],r['threads'],r['mode'])==(phase,variant,n,t,mode))
def table(headers,lines):
 return '| '+' | '.join(headers)+' |\n|'+'|'.join(['---']*len(headers))+'|\n'+'\n'.join('| '+' | '.join(map(str,line))+' |' for line in lines)
def ms(r,k='total_prover_ms'):return r['medians'][k]
production=[];memory=[]
for n in [512,1024]:
 for mode in ['cold','repeated']:
  a=row('production','baseline',n,12,mode);b=row('production','production',n,12,mode)
  production.append([n,mode,f'{ms(a):.3f} → {ms(b):.3f}',f'{100*(1-ms(b)/ms(a)):.2f}%',f'{ms(a,"verify_ms"):.3f} → {ms(b,"verify_ms"):.3f}'])
  memory.append([n,mode,f'{a["process_peak_rss_kib"]["median"]/1024:.2f} → {b["process_peak_rss_kib"]["median"]/1024:.2f}',f'{ms(a,"process_retained_rss_kib")/1024:.2f} → {ms(b,"process_retained_rss_kib")/1024:.2f}'])
scaling=[]
for t in [1,8,12,16]:
 scaling.append([t,*[f'{ms(row("production" if t==12 else "regression","production",n,t)):.3f}' for n in [512,1024]]])
regression=[]
for phase in ['regression','regression-confirmation']:
 for n in [512,1024]:
  for t in ([1,8,16] if phase=='regression' else [8,16]):
   a=row(phase,'baseline',n,t);b=row(phase,'production',n,t)
   regression.append([phase,n,t,f'{ms(a):.3f} → {ms(b):.3f}',f'{100*(ms(b)/ms(a)-1):+.2f}%'])
topology=[]
for n in [512,1024]:
 for mode in ['cold','repeated']:
  a=row('production','production',n,12,mode);b=row('production','production-ccd0-smt',n,12,mode)
  topology.append([n,mode,f'{ms(a):.3f}',f'{ms(b):.3f}',f'{ms(a,"verify_ms"):.3f} → {ms(b,"verify_ms"):.3f}'])
def prof(v,n,t):return next(r['medians_ms'] for r in profiles if r['variant']==v and r['degree']==n and r['threads']==t)
stages=[]
for n in [512,1024]:
 a=prof('baseline',n,12);b=prof('production',n,12)
 for label,key in [('Binding construction','falcon_algebraic:binding_construction'),('Whole binding','falcon_algebraic:binding'),('Forest','falcon_bridge:wfbitz_forest')]:
  stages.append([n,label,f'{a[key]:.3f} → {b[key]:.3f}'])
forest=[]
for n in [512,1024]:
 forest.append([n,*[f'{prof("production",n,t)["falcon_bridge:wfbitz_forest"]:.3f}' for t in [8,12,16]],f'{prof("production-ccd0-smt",n,12)["falcon_bridge:wfbitz_forest"]:.3f}'])
text=f'''# Qualify optimized arithmetic Falcon binding at 12 workers

The 12-worker production path now uses the existing optimized binding constructor and evaluator above the small-batch cutoff. The previous selector enabled these paths at 1, 8 and 16 workers but skipped 12. This caused a large, avoidable proving and verification cliff. The change is one runtime selector line in `src/piop/spartan/falcon1024_algebraic/arithmetic.rs`; added tests exercise the non-power-of-two worker pool and both sides of its cutoff.

The cutoff remains batch >16 for Falcon-512 and >8 for Falcon-1024. Other worker policies, including Falcon-512's batch-1024 special case at 2/4 workers, are unchanged. Field arithmetic, coefficient validation, transcript challenges/domains, security/grinding parameters and proof format are unchanged. This campaign uses canonical algebraic Falcon v9 at **batch 1024, 128-bit security** unless explicitly labelled correctness coverage.

## Production result

Times are median total commit + prove milliseconds, excluding verification. Each row has 21 cold and 21 repeated samples per configuration across seeds 42/43/44. Both binaries use `falcon-hybrid` without `bench-internals`, no experiment overrides, and the same pinned 12 physical cores. The baseline is commit `bfd6eaf98901bec2bac4504f649d48028cc87f2c`.

{table(['Degree','Mode','Prover: baseline → new, ms','Prover reduction','Verification: baseline → new, ms'],production)}

All three per-seed medians improve for both degrees and modes. The earlier same-binary experiment independently isolated the binding selector: Falcon-512 repeated time fell 42.388 → 31.751 ms; Falcon-1024 fell 77.892 → 56.408 ms. That experiment used the previously frozen `candidate-v2` with **explicit PCS four-tile selection for both strategies**, because it predates the PCS default promotion. Its control differed only in `BITZ_ARITHMETIC_BINDING_STRATEGY=legacy|optimized`. `causal-review.txt` records all cold/repeated and per-seed comparisons. The production table above is the independent confirmation with the new default.

## Worker scaling and regressions

New-production repeated medians, using physical cores in the established order. The 12-worker row is from the production phase; 1/8/16 rows are from the subsequent regression phase with the same frozen binaries and methodology:

{table(['Workers','Falcon-512, ms','Falcon-1024, ms'],scaling)}

Twelve physical cores still trail 8/16 in these workloads after the binding fix. This is not a monotonic slowdown above 12. The remaining forest costs are discussed below. The 22 ms Falcon-512 target remains unmet.

The unchanged 1/8/16 policies received 21 repeated samples per binary/configuration. Small positive changes in the first comparison motivated an independent 8/16 confirmation, also with 21 repeated samples per binary/configuration. Negative changes mean faster:

{table(['Run','Degree','Workers','Baseline → new, ms','Change'],regression)}

Pooled regression-median changes stay between -0.52% and +0.69%, but seed-level variation is larger. In the confirmation, Falcon-512 at 16 workers changes by +5.22% for seed 42 and -0.92%/-0.31% for seeds 43/44; Falcon-1024 at 16 workers has a +1.63% seed. These mixed results must not be described as proof of zero performance regression on every workload. The code change selects a different path only at 12 workers; binary layout and run-to-run timing can still affect other measurements. No correctness regression was observed.

## Why 12 physical cores still have a penalty

Separate traced diagnostics use three seeds, one warmup and three measured proofs per seed (nine samples per row). They are not included in untraced latency qualification. Construction is included within whole binding; nested wall spans must not be added.

{table(['Degree','Stage at 12 physical workers','Baseline → new, ms'],stages)}

The host is a Ryzen 9 9950X3D. Physical cores 0–7 share 96 MiB L3; 8–15 share 32 MiB. The established 8-worker placement fits the larger cache, while 12 physical workers span both. Forest stable row ownership is also enabled at 16 workers but intentionally excludes 12: power-of-two row counts cannot remain divisible by 12 across high-bit folds. Those restrictions remain intact.

{table(['Degree','Forest, 8 physical','Forest, 12 physical','Forest, 16 physical','Forest, 12 logical in first cache'],forest)}

This supports a cache-locality/scheduling explanation for the remaining forest penalty. It is not a hardware-counter measurement of cache traffic. `scheduling-analysis.txt` explains the evidence and a possible future row-grain experiment. No forest scheduler change is promoted here.

## Separate placement experiment

Keeping all 12 logical workers on the larger-cache group uses **8 physical cores plus 4 SMT siblings**. It is not a 12-physical-core result or an additional source-code optimization. Production-binary measurements confirm:

{table(['Degree','Mode','12 physical prover, ms','12 logical cache-local prover, ms','Verification: physical → cache-local, ms'],topology)}

The cache-local placement improves every prover per-seed median, but Falcon-1024 verification gets slightly slower. It remains an explicitly selected benchmark configuration; no process-wide affinity policy is installed. Worker CPUs are `[6,7,4,5,2,3,0,1,22,23,20,21]`, compared with physical-12 `[6,7,4,5,2,3,0,1,14,15,12,13]`. Main and auxiliary-pool CPUs remain 6.

## Memory and proofs

Process peak and retained RSS are distinct metrics. The peak table reports the median of each process's maximum observed high-water mark, not a pooled median of per-trial high-water readings. Retained RSS is sampled **after verification and dropping the proof**, while Prepared and the input corpus remain live. It includes allocator-retained pages; it is not a persistent proof/scratch allocation measure.

{table(['Degree','Mode','Peak RSS: baseline → new, MiB','Retained RSS: baseline → new, MiB'],memory)}

Peak RSS falls, but Falcon-512 cold retained RSS rises by about 29.9 MiB. Repeated retained RSS is effectively unchanged. Do not infer that every memory metric improves. Cached-vector counters are unavailable in production builds and recorded as null rather than zero.

For every matching input, complete proof Debug digests, source roots and payload sizes remain identical. Debug-digest comparison checks complete in-build proof contents, not a canonical wire codec. Across seeds 42/43/44, Falcon-512 payloads are 256,368 / 256,112 / 256,848 bytes; Falcon-1024 payloads are 321,272 / 321,304 / 321,432 bytes. No proof-size or security change is introduced.

## Validation and method

- Native Falcon suite: **361 passed, zero failed, three ignored**. The expanded ordinary tests cover complete proofs at batches 17/1024, security 100/128, Bits/Words binding and worker pools 1/2/3/4/8/12/16; cross-pool verification; owned/borrowed arithmetic proofs and transcripts at/below/above each cutoff; canonical-field rejection; constructor/evaluator equivalence for 61-/127-bit fields and Boolean/non-Boolean challenges. Ignored tests were not rerun in this narrow selector campaign.
- Eleven synthetic runner tests passed. The new runner preserves the prior frozen runner/helper files and adds strict per-configuration affinity validation.
- The completed benchmark audit verifies **{validation['verified_proofs']:,} proofs**, including warmups, across all recorded phases. Correctness-only fresh-process cases include batches 3/129 at security 100/128 for both degrees at 12 workers, against the previous production binary. Every proof verifies and matches its baseline input, source, Debug digest and payload across threads, placements and phases.
- Benchmark variants run serially in randomized matched blocks. No builds/tests overlap timed benchmarks. Cold means the first proof in a fresh process/Prepared, seven processes per seed; machine caches are not flushed. Repeated means two warmups and seven samples per seed sharing Prepared. Within-process samples are correlated. Preparation, input generation, resource sampling and verification are outside prover timing.
- Worker order for physical-core runs is `[6,7,4,5,2,3,0,1,14,15,12,13,10,11,8,9]`. Affinity is read back for every trial. `host.json` records cache topology, governor, SMT and page size. This host uses 4 KiB pages; ignore the legacy forest trace's hard-coded “MB faulted” display. Trace worker-nanosecond totals are summed CPU work, not elapsed wall time.

The production executable SHA-256 is `bf303cb59a2e6830f0beb97670bd693aaef0d22fd5ad54bb7304696e0c850511`; the frozen previous-production SHA-256 is `b4eb01b1fa8f40b0c0b0c61d82372fe94070106b2d5023a2b47f7e1488150415`. `candidate-production-source.json`, its patch, Cargo build records and `production-build-audit.json` tie the executable to the final source and verify that `bench-internals` is absent. `source-review.txt`, `runner-review.txt`, `topology-runner-review.txt`, `causal-review.txt` and `production-review.txt` provide independent inspections.

`summary.json` and `profile-summary.json` contain all pooled and per-seed metrics. `forest-summary.json` contains parsed internal forest timers. Phase manifests freeze binaries, runner/helpers, commands, flags, inputs and affinity; each raw result carries checked command/stdout/stderr/process hashes. Revalidate while the frozen binaries remain available:

```sh
python3 scripts/falcon_worker_optimization.py --output results/falcon-twelve-workers-20261013 --summarize-only
```

`run-qualification.py`, `build-candidate.py`, `run-tests.py` and the phase command records document reproduction. The final regression-confirmation phase is independently frozen in its manifest. The previous campaign's runner and evidence remain unchanged.
'''
(OUT/'README.md').write_text(text)

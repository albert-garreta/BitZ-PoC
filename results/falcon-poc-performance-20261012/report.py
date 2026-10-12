import csv,hashlib,json,shutil,statistics
from collections import defaultdict
from pathlib import Path
OUT=Path(__file__).resolve().parent
s=json.loads((OUT/'summary.json').read_text()); rows=s['rows']; m=json.loads((OUT/'manifest.json').read_text())
assert len(rows)==12
assert all(r['samples']==21 and r['profile_samples']==9 for r in rows)
proofs={};inputs={}; count=0
for phase in ['prewarm','latency','profile','profile-light']:
 for p in sorted((OUT/phase).glob('*/result.json')):
  d=json.loads(p.read_text());j=d['job']
  for file,hashkey in [('stdout.jsonl','stdout_sha256'),('stderr.log','stderr_sha256')]:
   assert hashlib.sha256((p.parent/file).read_bytes()).hexdigest()==d[hashkey],p
  process=json.loads((p.parent/'process.json').read_text())
  assert process['exit_code']==0 and not process['timed_out'],p
  raw=[json.loads(line) for line in (p.parent/'stdout.jsonl').read_text().splitlines() if line]
  raw=[t for t in raw if t.get('trial') in ['sample','warmup']]
  assert len(raw)==len(d['trials']) and all(t['verified'] is True for t in raw)
  for t in d['trials']:
   key=(j['variant'],d['degree'],j['seed'])
   identity=tuple(t[k] for k in ['protocol_id','input_digest','source_root','proof_debug_digest','proof_payload_bytes'])
   assert proofs.setdefault(key,identity)==identity
   assert inputs.setdefault((d['degree'],j['seed']),t['input_digest'])==t['input_digest']
   count+=1
assert count==618
v=dict(verified_proofs=count,raw_log_hashes_verified=True,complete_proof_identities_equal_across_workers_and_instrumentation=True,
      equal_input_digests_across_arithmetic_and_full=True,
      latency_samples_per_cell=21,light_profile_samples_per_cell=9,
      light_profile_proof_latency_ratio_range=[min(r['profile_prove_overhead_ratio'] for r in rows),max(r['profile_prove_overhead_ratio'] for r in rows)])
(OUT/'final-validation.json').write_text(json.dumps(v,indent=2)+'\n')
metrics=['witness_commit_ms','prove_ms','total_prover_ms','verify_ms']
with (OUT/'performance.csv').open('w') as f:
 w=csv.writer(f,lineterminator='\n');w.writerow(['variant','degree','threads','batch','security_bits',*metrics,'gkr_ms','gkr_fraction_of_proof','proof_bytes'])
 for r in rows:
  w.writerow([r['variant'],r['degree'],r['threads'],1024,128,*[r['latency'][key]['median'] for key in metrics],
   r['stages']['prove|falcon_bridge:wfbitz_forest']['median'],r['gkr_fraction_of_proof']['median'],r['latency']['proof_payload_bytes']['median']])
with (OUT/'stages.csv').open('w') as f:
 w=csv.writer(f,lineterminator='\n');w.writerow(['variant','degree','threads','phase','stage','inclusive_median_ms','min_ms','max_ms','samples'])
 for r in rows:
  for key,stat in r['stages'].items():
   phase,name=key.split('|',1);w.writerow([r['variant'],r['degree'],r['threads'],phase,name,stat['median'],stat['min'],stat['max'],stat['count']])
bykey={(r['variant'],r['degree'],r['threads']):r for r in rows}
lines=['Falcon performance: pushed falcon-poc ee79186745962e3e285fc5090dae8a95545dda43',
'','Batch: 1,024 distinct original-Falcon signatures. Security target: 128 bits. All times are milliseconds per batch.',
'CPU: AMD Ryzen 9 9950X3D. Rust 1.98.1. Release, LTO, codegen-units=1, target-cpu=native. Features: falcon-hybrid (default parallel); no bench-internals.',
'Workers: 1, 8, 16 physical cores, individually pinned. Worker order '+str(m['worker_cpus'])+'. Main and serial auxiliary Flock work on CPU 6.',
'1/8 workers use the 96 MiB L3 cluster; 16 workers span that cluster and the 32 MiB L3 cluster. SMT siblings unused. Governor: powersave / amd-pstate-epp. Host not exclusively reserved.',
'','Headline latency: median of 21 samples (7 per seed 42/43/44), after 2 warmups per process. Configurations shuffled; one benchmark process at a time; builds finished before timing.',
'Total prover = witness construction + commitment + proof; verification listed separately. Public input setup/decoding, fixture generation, native reference verification and proof diagnostic hashing excluded from these timers.',
'Arithmetic proves the algebraic relation with public h and t, using the default v9 word binder. Full Falcon also proves SHAKE/Keccak and hash-to-point constraints.',
'','GKR = BitZ weighted forest proving inside the integer bridge, including its round grinding. It excludes integer column folds, column packing, power tables and the final PCS opening.',
'GKR and other stage times: medians of 9 lightweight-instrumentation samples (3 per seed), after 1 warmup. Only the benchmark tracing subscriber filter differs; all prover/verifier/kernel sources are unchanged. The exact observer patch and binary hashes are saved.',
'Full diagnostic tracing inflated short arithmetic timings, so its output is preserved separately in summary-detailed.json and is NOT used for the GKR headline column.',
'Lightly traced proof-time medians / untraced medians ranged %.3f to %.3f. This calibration includes run-to-run variation, not just instrumentation cost.'%tuple(v['light_profile_proof_latency_ratio_range']),
'All stage durations are inclusive; nested stages must not be added to their parents.',
'',f'Validation: {count} proofs verified, with identical complete proof diagnostic digests, source roots and payload sizes across worker counts and instrumentation for each input/variant. Inputs matched across arithmetic/full variants.',
'','Variant       N    T    Witness+commit      Proof      Total     Verify        GKR']
for r in rows:
 a=r['latency'];g=r['stages']['prove|falcon_bridge:wfbitz_forest']['median']
 lines.append(f"{r['variant']:10} {r['degree']:4} {r['threads']:4} {a['witness_commit_ms']['median']:17.3f} {a['prove_ms']['median']:10.3f} {a['total_prover_ms']['median']:10.3f} {a['verify_ms']['median']:10.3f} {g:10.3f}")
lines+=['','8 -> 16 workers (negative percentage = less time):']
for variant in ['arithmetic','full']:
 for degree in [512,1024]:
  a=bykey[(variant,degree,8)];b=bykey[(variant,degree,16)]
  total=(b['latency']['total_prover_ms']['median']/a['latency']['total_prover_ms']['median']-1)*100
  gkr=(b['stages']['prove|falcon_bridge:wfbitz_forest']['median']/a['stages']['prove|falcon_bridge:wfbitz_forest']['median']-1)*100
  lines.append(f'{variant} N={degree}: total {total:+.2f}%; GKR {gkr:+.2f}%.')
lines+=['','Major proving stages by cell (non-overlapping top-level stages; GKR included within bridge):']
keys={'arithmetic':['falcon_algebraic:bitz','falcon_algebraic:arithmetic','falcon_algebraic:ring','falcon_algebraic:pcs'],
'full':['falcon_hybrid:arithmetic_prefix','falcon_hybrid:binary_bridge','falcon_hybrid:keccak_prefix','falcon_hybrid:joint_sumcheck','falcon_hybrid:shared_opening','falcon_hybrid:link_coefficients']}
for r in rows:
 lines.append(f"{r['variant']} N={r['degree']} T={r['threads']}:")
 for name in sorted(keys[r['variant']],key=lambda n:-r['stages']['prove|'+n]['median']):
  lines.append(f"  {name}: {r['stages']['prove|'+name]['median']:.3f}")
lines+=['','At 16 workers, finer bottleneck detail:']
for r in rows:
 if r['threads']!=16:continue
 lines.append(f"{r['variant']} N={r['degree']}:")
 details=['prove|falcon_bridge:wfbitz_forest','prove|falcon_algebraic:binding','prove|falcon_algebraic:integer_outer',
  'prove|falcon_algebraic_ring:coordinates','prove|falcon_algebraic_ring:collapse','prove|falcon_arithmetic:binding_inner',
  'prove|falcon_arithmetic:integer_outer','prove|falcon_shared_ring:prove','commit|shared_source:encode','commit|falcon_hybrid:shake_witness']
 for key in details:
  if key in r['stages']:lines.append(f"  {key}: {r['stages'][key]['median']:.3f}")
lines+=['','Reading the results:',
'Arithmetic: GKR is the largest measured proving stage at 8/16 workers and stops scaling past 8 on this CPU. The arithmetic norm/word binder comes next, followed by ring reduction. 16 workers help N1024 overall by accelerating other stages; N512 is slightly faster at 8.',
'Full: arithmetic-prefix binding, the BitZ bridge/GKR, and Keccak dominate proof time. Shared PCS opening remains material. Witness/commit is also significant; source encoding is the largest measured part of that phase.',
'Optimization implications are hypotheses: improve GKR scheduling/cache behavior beyond 8 cores; focus full arithmetic work on the binding inner pass; profile Keccak before selecting a kernel change. These measurements do not establish an instruction-level cause.',
'','Artifacts: manifest.json, light-build.json, final-validation.json, performance.csv, stages.csv, summary.json, profile-trials.json, raw logs/commands under latency/ and profile-light/. Detailed intrusive profiles are under profile/.',
'Reproduce headline binaries from the recorded commit using the recorded build flags, then run the commands saved per cell. Reproduce minimal profiles by applying light-profile-subscriber.patch only for the diagnostic build.',
'No production source changes were retained. Benchmarks were run locally against the revision recorded in manifest.json.']
(OUT/'REPORT.txt').write_text('\n'.join(lines)+'\n')
print(json.dumps(v,indent=2))
print('\n'.join(lines[lines.index('8 -> 16 workers (negative percentage = less time):'):lines.index('8 -> 16 workers (negative percentage = less time):')+5]))

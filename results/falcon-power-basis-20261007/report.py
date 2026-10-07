from pathlib import Path
import json,statistics,shutil
base=Path('/tmp/falcon-power-basis')
before=json.loads((base/'results-baseline-head/summary.json').read_text())+json.loads((base/'results-baseline-512/summary.json').read_text())
after=json.loads((base/'results-candidate/summary.json').read_text())
key=lambda r:(r['kind'],r['degree'],r['target'],r['threads'])
b={key(r):r for r in before};a={key(r):r for r in after}
assert len(b)==len(a)==40 and b.keys()==a.keys()
for k in a:
 assert b[k]['input_digest']==a[k]['input_digest']

out=base/'report';out.mkdir(exist_ok=True)
s=['# Falcon power-basis benchmarks — 2026-10-07','',
'1,024 distinct original-Falcon signatures per proof. Ryzen 9 9950X3D (`will`); native release build (`-C target-cpu=native`, fat LTO, one codegen unit). Medians of five measured runs after one warm-up, seed 42. Prover times include checked witness preparation, packing, commitment, and proving. Input generation, external hash-to-point preparation for the algebraic statement, and reusable parameter setup are excluded. Full Falcon proves SHAKE-256 and HashToPoint; algebraic Falcon proves the public-h/public-t ring equation and integer norm.', '',
'## Baseline identity','',
'Existing paths use commit `5c051cf767ab3b69f23ebaa036b6e6e3c6d1f726`. That revision has no algebraic Falcon-512 frontend. Its baseline is the new degree-512 port **before** the power-basis optimization, using the same ordinary extension multiplications as the old degree-1024 path. `port512.patch` records that port; a hard-coded padding-count test expectation was subsequently generalized. Binary checksums and source hashes are recorded for each build.', '',
'The optimized algebraic prover retains canonical coordinates in the challenge power basis through carries and projection. Both full and algebraic Falcon share the field backend in `vendor/field`. Full Falcon retains its fixed-basis tensor lift; these measurements do not implement a power-basis lift for full Falcon. The NTT proposal is also separate.', '',
'## Results','',
'All times below are milliseconds. Each row uses the same degree, target, inputs, thread count, and affinity before/after. CPUs are pinned to logical CPU IDs 0 through threads-1; auxiliary workers and the main thread are pinned to CPU 0. There were no simultaneous benchmark, build, or test jobs during measured runs. Both algebraic targets use extension degree 11; full Falcon uses degree 9 at 100 bits and degree 11 at 128 bits.', '']
for target in [100,128]:
 for kind in ['algebraic','full']:
  for degree in [512,1024]:
   s += [f'### {kind.title()} Falcon-{degree}, {target}-bit target','', '| Threads | Baseline prover | Updated prover | Ratio | Baseline verify | Updated verify | Payload KiB before → after |','|---:|---:|---:|---:|---:|---:|---:|']
   for t in [1,2,4,8,16]:
    x,y=b[kind,degree,target,t],a[kind,degree,target,t]
    s += [f"| {t} | {x['total_prover_ms']:.2f} | {y['total_prover_ms']:.2f} | {x['total_prover_ms']/y['total_prover_ms']:.2f}× | {x['verify_ms']:.2f} | {y['verify_ms']:.2f} | {x['proof_payload_bytes']/1024:.2f} → {y['proof_payload_bytes']/1024:.2f} |"]
   s += ['']
s += ['## Interpretation','',
'Ratios are observed whole-prover measurements for this fixed input seed. The algebraic protocol has a new transcript domain and different carry values, which also change grinding challenges. Repeating the same input primarily measures timing noise, not the distribution of grinding work across seeds. Do not attribute every whole-prover difference to weight generation. Full Falcon has no algorithmic lift change and should be interpreted as a backend-move regression check.', '',
'Proof message schemas, source sizes, and commitment counts are unchanged by the basis optimization. Actual serialized payload sizes can vary with transcript-dependent Merkle multiproof overlap. The algebraic payload changes in these runs are confined to PCS openings; ring and arithmetic messages have identical sizes. Payload sizes exclude outer transport framing. Algebraic version-2 proofs are incompatible with the new version-3 transcript despite having the same message layout.', '',
'## Isolated public field kernel','',
'`vendor/field/examples/q12289_power_basis.rs` generates two weight sequences per signature for 1,024 signatures on one thread. Both paths retain all output vectors. The updated timing includes matrix setup and both start conversions per signature; correctness checks run outside the timer. Trial 0 is warm-up. An initial measurement immediately after compilation showed clock-transition drift for degree 512 and is retained as `kernel-benchmark-initial.jsonl`. The reported run follows an additional untimed complete pass. This isolates field arithmetic and has no commitments, sumchecks, projection, or grinding.', '',
'| Degree | Ordinary multiplication ms | Power basis ms | Ratio | Basis setup µs |', '|---:|---:|---:|---:|---:|']
rows=[json.loads(l) for l in (base/'kernel-benchmark.jsonl').read_text().splitlines()]
for n in [512,1024]:
 rowsn=[r for r in rows if r['degree']==n and r['trial']>0];x=statistics.median(r['fixed_ms'] for r in rowsn);y=statistics.median(r['power_ms'] for r in rowsn)
 s += [f"| {n} | {x:.3f} | {y:.3f} | {x/y:.2f}× | {statistics.median(r['setup_us'] for r in rowsn):.3f} |"]
s += ['', '## Ring-coordinate stage trace','',
'Instrumented single-thread runs at 128 bits, one warm-up plus three measured proofs, using the existing tracing spans. These are separate from the uninstrumented matrix. Each entry times public evaluation, target formation, and weight-coordinate construction for 1,024 signatures. The optimized span also includes cached-power generation and basis setup.', '',
'| Degree | Stage | Baseline ms | Updated ms | Ratio |', '|---:|---|---:|---:|---:|']
trace=json.loads((base/'traces/summary.json').read_text());trace={(r['degree'],r['stage'],r['mode']):r['coordinates_ms'] for r in trace}
for n in [512,1024]:
 for stage in ['prove','verify']:
  x=trace[n,stage,'baseline'];y=trace[n,stage,'candidate'];s += [f'| {n} | {stage} | {x:.3f} | {y:.3f} | {x/y:.2f}× |']
s += ['', '## Validation and reproduction','',
'All 480 baseline/updated benchmark proofs verified (40 configurations × 6 proofs × 2 versions), as did 16 additional stage-trace proofs. Raw logs include every trial and proof payload. Input digests match before/after in all configurations; observed payload sizes are reported separately for both versions. Field tests cover registered extension arithmetic, irreducibility, canonical coordinates, recurrence equality, and singular bases. All 175 selected Falcon tests and 3 standalone field tests passed. Four large qualification tests were ignored by the unit-test run; the benchmark matrix separately verifies real 1,024-signature batches. Test results are in `validation/`.', '',
'Build: `CARGO_TARGET_DIR=/tmp/falcon-simplify-candidate-target CARGO_BUILD_JOBS=4 RUSTFLAGS="-C target-cpu=native" cargo build --release --locked --offline --features falcon-hybrid --example falcon_algebraic --bench falcon_hybrid`.', '',
'Run from the repository root: `python3 results/falcon-power-basis-20261007/bench.py --binary-dir /path/to/preserved/binaries --out /path/to/new/results`. It runs all degrees, targets, and thread counts; use `--kinds algebraic --degrees 512` for the degree-512 algebraic port baseline. Exact executed commands and environment values are in each results directory’s `metadata.json`.', '',
'The implementation patch omits unrelated pre-existing workspace changes. `candidate.patch` plus new files listed in its diff reproduce the implementation relative to the baseline revision. Source/binary hash manifests distinguish the baseline revision, unoptimized 512 port, and updated build.']
(out/'RESULTS.md').write_text('\n'.join(s)+'\n')
(out/'comparison.json').write_text(json.dumps([{'before':b[k],'after':a[k]} for k in sorted(a)],indent=2)+'\n')

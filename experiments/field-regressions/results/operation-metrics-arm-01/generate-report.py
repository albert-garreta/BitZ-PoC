import collections,csv,json,re
from pathlib import Path
out=Path(__file__).resolve().parent
root=out.parents[1]
rows=json.loads((out/'summary.json').read_text())
meta=json.loads((out/'metadata.json').read_text())
lookup={(r['family'],r['size'],r['variant']):r for r in rows}
assert all(r['complete'] and r['correctness'] for r in rows)
def get(f,s,v): return lookup[f,s,v]
def us(r): return f"{r['median_ns']/1000:.3f}"
def ci(r): return f"{r['median_ratio']:.3f} [{r['median_ci_low']:.3f}, {r['median_ci_high']:.3f}]"
def table(head,body): return '\n'.join(['| '+' | '.join(head)+' |','|'+'|'.join(['---']*len(head))+'|']+['| '+' | '.join(map(str,r))+' |' for r in body])
def count(r):
 m=re.search(r'(?:^|_)n(\d+)$',r['size'])
 return int(m[1]) if m else int(r['size']) if r['size'].isdigit() else 1
selected=[r for r in rows if r['selected']]
counts=collections.Counter(r['status'] for r in selected)
with (out/'operation-metrics.csv').open('w') as f:
 fields=['family','size','variant','baseline','coverage','median_us_per_pass','p95_us_per_pass','ns_per_element_or_pair','median_time_ratio','median_ci_low','median_ci_high','allocation_calls','allocation_bytes','status','round']
 w=csv.DictWriter(f,fieldnames=fields);w.writeheader()
 for r in rows:
  baseline_only=not r['family'].startswith('metrics_gf_')
  w.writerow(dict(family=r['family'],size=r['size'],variant=r['variant'],baseline=r['baseline'],coverage='baseline_only' if baseline_only else 'implementation_comparison',median_us_per_pass=r['median_ns']/1000,p95_us_per_pass=r['p95_ns']/1000,ns_per_element_or_pair=r['median_ns']/count(r),median_time_ratio='' if baseline_only else r['median_ratio'],median_ci_low='' if baseline_only else r['median_ci_low'],median_ci_high='' if baseline_only else r['median_ci_high'],allocation_calls=r['allocation_calls'],allocation_bytes=r['allocation_bytes'],status='replacement_unmeasured' if baseline_only else r['status'],round=r['round']))
parts=['# Unified arithmetic operation metrics',
 f"Measured **{len(rows)} variant cases across {len({r['family'] for r in rows})} operation families** on the Apple M1 Max. All correctness checks completed, and every new timed hot path allocated zero bytes. The eight preselected GF128 comparisons against F2Z finished with **{counts['pass']} pass, {counts['regression']} regression and {counts['inconclusive']} inconclusive** results under the existing 1% gate.",
 'These are operation-level measurements of current implementations and isolated prototypes. The complete unified library is not implemented or integrated. Baseline-only costs below establish comparison targets; they do not establish speedups for future implementations. No full-prover, hybrid or x86 speedup is established.',
 '## Fresh GF128 comparisons',
 'Times are microseconds per batch of 1,024 independent operations. Native input layouts and output buffers are prepared before timing. Ratios compare the selected implementation with **F2Z**, using paired process medians; they may differ slightly from quotients of the pooled times. A pass establishes the specified regression bound, not a strict speedup.',
]
body=[]
for op,var in [('add','shared'),('mul','scalar_lanes'),('square','shared'),('inverse','shared')]:
 f='metrics_gf_'+op;r=get(f,'1024',var)
 body.append([op,us(get(f,'1024','f2z')),us(get(f,'1024','flock')),us(get(f,'1024','shared')),us(r) if var=='scalar_lanes' else '—',ci(r),r['status']])
parts.append(table(['Operation','F2Z µs','Flock µs','Shared µs','ARM scalar-lane µs','Selected/F2Z [95% CI]','Gate'],body))
parts.append('F2Z already has specialized GF128 squaring and inversion. Earlier large gains were measured against Flock, so they cannot be attributed to replacing the F2Z implementations. Multiplication uses the adapted ARM scalar-lane candidate where shown. GF addition/subtraction is XOR; subtraction equivalence is checked outside timing. Inverse timings use the common nonzero-input domain.')
parts.append('All selected small/large GF128 cases:')
parts.append(table(['Operation','Elements','Selected','Time/F2Z [95% CI]','P95 ratio [95% CI]','Gate'],[[r['family'].removeprefix('metrics_gf_'),r['size'],r['variant'],ci(r),f"{r['p95_ratio']:.3f} [{r['p95_ci_low']:.3f}, {r['p95_ci_high']:.3f}]",r['status']] for r in selected]))
parts.extend(['## Newly measured baseline costs','These rows measure current code only. Each batch contains 1,024 elements, except the round contains 1,024 weighted pairs. The per-element number is batch throughput, not isolated-call latency. No replacement speedup has been measured for these rows.'])
body=[]
for family,label,variant in [('gf8_add','GF8 add','flock'),('gf8_mul','GF8 multiply','flock'),('gf8_inverse','GF8 inverse','flock'),('b127_add','B127 add','f2z'),('b127_mul','B127 multiply','f2z'),('b127_square','B127 square','f2z'),('b127_inverse','B127 inverse','f2z'),('gf128_round','GF128 fused weighted round','f2z_single_pair')]:
 r=get(family,'1024',variant);body.append([label,variant,us(r),f"{r['median_ns']/1024:.3f}"])
parts.append(table(['Operation','Existing implementation','Batch µs','ns/element or pair'],body))
parts.append('The fused round calls `WideMulAcc::eqf_single_pair_round` and consumes 2,048 left values, 2,048 right values and 1,024 weights. GF8 is the existing Flock implementation; B127 is the existing F2Z field, using its own modulus. These are not new owned-library implementations.')
parts.append('### Integer baselines')
parts.append('Microseconds per 1,024 operations. Widths are 64-bit limbs: 2 = 128 bits, 4 = 256 bits, 9 = 576 bits. The same mixed fixture includes successful operations, overflows, signed extrema and full-width random inputs. Checked timings retain the existing `Option` results and branches; they are not constant-time replacement measurements.')
body=[]
for family,label in [('metrics_integer_add','Wrapping add'),('metrics_integer_sub','Wrapping subtract'),('metrics_uint_checked_add','Checked unsigned add'),('metrics_uint_checked_sub','Checked unsigned subtract'),('metrics_uint_checked_mul','Checked unsigned multiply'),('metrics_int_checked_add','Checked signed add'),('metrics_int_checked_sub','Checked signed subtract'),('metrics_int_checked_mul','Checked signed multiply'),('metrics_uint_compare','Unsigned compare'),('metrics_uint_wide_mul','Exact widened unsigned multiply')]:
 variant='circuit_z' if family.startswith('metrics_integer_') else 'existing'
 body.append([label,*[us(get(family,f'l{n}_n1024',variant)) for n in [2,4,9]]])
for shape,label in [('d64','Unsigned div/rem, 64-bit divisor'),('dfull','Unsigned div/rem, full-width divisor')]:
 body.append([label,*[us(get('metrics_uint_divrem',f'l{n}_{shape}_n1024','existing_prevalidated_divisor')) for n in [2,4,9]]])
parts.append(table(['Operation','2 limbs µs','4 limbs µs','9 limbs µs'],body))
parts.append('Wrapping add/subtract call `circuit::witgen::Z`; checked operations use the existing crypto-primitives wrappers. Widened multiplication produces all `2L` limbs, unlike a wrapping MAC. Division uses crypto-bigint with a public divisor whose nonzero status is validated before timing; it does not benchmark a future reciprocal-preparation API.')
parts.append('### Runtime prime baselines')
parts.append('Known prime `q = 2^127−1`; no primality testing is included. Inversion is the current variable-time operation applied independently, not a Montgomery batch inversion.')
body=[]
for family,size,label in [('prime_inverse_each','q127_n1024','1,024 independent inversions'),('prime_pow_each','q127_e17_sparse_n1024','1,024 powers, exponent 65537 (17-bit bound)'),('prime_pow_each','q127_e127_alternating_n1024','1,024 powers, alternating exponent (127-bit bound)'),('prime_canonical_encode_into','q127_n1024','Encode 1,024 canonical field elements'),('prime_canonical_decode_into','q127_n1024','Decode 1,024 canonical field elements'),('prime_context_setup','q127','One Montgomery + raw context setup')]:
 r=get(family,size,'existing');body.append([label,us(r),f"{r['median_ns']/count(r):.3f}"])
parts.append(table(['Operation','µs/pass','ns/element or setup'],body))
parts.append('Codec timings measure existing element-conversion components into preallocated buffers, excluding allocating wrappers, framing and full proof serialization. Decode uses public canonical fixtures. Prime selection remains low priority; the user-provided PIOP breakdown records 0.18 ms of a 53.52 ms PIOP, and is not a new measurement from this suite.')
parts.extend(['## Retained comparisons from earlier frozen campaigns','These are the latest applicable saved comparisons, not reruns in the new executable. Every row compares candidates with their baseline **inside its own campaign**. Do not divide absolute times across campaigns. Time ratios below 1 mean faster. The updated integer campaign supersedes the older two-limb and bounded-product prototypes.'])
oldrows=[]
def append_old(campaign,family,size,variant,label):
 source=json.loads((root/'results'/campaign/'summary.json').read_text())
 r=next(r for r in source if r['family']==family and r['size']==size and r['variant']==variant and r['arch']=='aarch64')
 b=next(b for b in source if b['family']==family and b['size']==size and b['variant']==r['baseline'] and b['arch']=='aarch64')
 oldrows.append(dict(campaign=campaign,operation=label,**r,baseline_us=b['median_ns']/1000,candidate_us=r['median_ns']/1000))
 return [label,f"{b['median_ns']/1000:.3f}",us(r),f"{r['median_ratio']:.3f}",r['status']]
body=[]
for args in [
 ('arithmetic-arm-01','gf_dot','1024','shared_wide1','GF128 delayed dot, 1,024 terms'),
 ('integer-focus-arm-03','two_limb_mac','signed16_n1024','selected','2-limb MAC, signed16 fixture'),
 ('integer-focus-arm-03','two_limb_mac','full128_n1024','selected','2-limb MAC, full128 fixture'),
 ('arithmetic-arm-01','integer_mac','l1_n1024','fused','1-limb MAC, signed16 fixture'),
 ('arithmetic-arm-01','integer_mac','l4_n1024','fused','4-limb MAC, signed16 fixture'),
 ('arithmetic-arm-01','integer_mac','l9_n1024','fused','9-limb MAC, signed16 fixture'),
 ('integer-focus-arm-03','bounded_product','active4_n1024','prepared_selected','4-active-limb products, prepared'),
 ('integer-focus-arm-03','bounded_product','active4_n1024','validate_execute_selected','4-active-limb products, including validation'),
 ('integer-focus-arm-03','bounded_product','active9_n1024','prepared_selected','9-active-limb products, prepared'),
 ('integer-focus-arm-03','bounded_product','active9_n1024','validate_execute_selected','9-active-limb products, including validation'),
 ('arithmetic-arm-01','prime_mul','q128_n1024','branded','Prime scalar multiply, q128, branded'),
 ('arithmetic-arm-01','prime_dot','q128_n1024','acc4','Prime product MAC (R²), q128'),
 ('arithmetic-arm-01','prime_linear','q128_n1024','acc4','Prime native MAC (R), q128'),
 ('arithmetic-arm-01','fixed','zero_n1024','specialized','GF fixed scalar, zero'),
 ('arithmetic-arm-01','fixed','half_n1024','specialized','GF fixed scalar, half width'),
 ('arithmetic-arm-01','fixed','full_n1024','specialized','GF fixed scalar, full width'),
 ('arithmetic-arm-01','butterfly','zero_n1024','specialized','GF butterfly, zero scalar'),
 ('arithmetic-arm-01','butterfly','half_n1024','specialized','GF butterfly, half-width scalar'),
 ('arithmetic-arm-01','butterfly','full_n1024','specialized','GF butterfly, full-width scalar')]: body.append(append_old(*args))
parts.append(table(['Operation, 1,024 terms/products','Baseline µs','Candidate µs','Time ratio','Gate'],body))
parts.append('Baselines: GF dot uses actual F2Z `WideMulAcc`; integer MAC uses actual circuit `Z<L>`; bounded products use existing P-256 multiplication. Fixed-scalar/butterfly use the existing experiment’s prepared formula adaptation, not a call to the private production API. Prime scalar uses raw Montgomery context arithmetic; the two prime MACs use their respective existing delayed accumulators. Prime product MAC still regresses about 4–5% at 16 terms. A scoped prime API and mixed signed-coefficient MAC have not been benchmarked as completed replacements.')
parts.append('### Reduction and integer-to-prime projection controls')
parts.append('Reduction times are for **one reduction** of a previously built 1,024-term accumulator. The existing optimized reducer is already faster than the crypto-bigint reference; replacing it with that reference would lose performance.')
body=[]
for family,label in [('prime_reduce_product','Product accumulator'),('prime_reduce_linear','Native accumulator')]:
 for bits in ['100','128']: body.append(append_old('arithmetic-arm-01',family,bits,'crypto_bigint',f'{label}, {bits}-bit modulus'))
parts.append(table(['Reduction','Existing optimized µs','Reference µs','Reference/existing','Gate'],body))
parts.append('Projection compares the existing `RuntimeModulus` path against an external fixed-storage, variable-time crypto-bigint control. This identifies headroom; it is not an implemented owned constant-time replacement. Times below are for 1,024 signed coefficients.')
body=[]
for bits in [100,128]:
 for width in [2,4,9]: body.append(append_old('arithmetic-arm-01','projection',f'q{bits}_l{width}_n1024','fixed_crypto_bigint_vartime',f'q{bits}, {width} limbs'))
parts.append(table(['Projection','Existing µs','Reference µs','Reference/existing','Gate'],body))
parts.append('### NTT coverage')
body=[]
for size in ['log8_lanes1','log8_lanes32','log12_lanes8','log15_lanes32','log16_lanes32','log17_lanes32','log18_lanes32']:
 body.append(append_old('arm-gated-01','ntt',size,'preserved_schedule',size))
parts.append(table(['Transform shape','Flock µs','Preserved-schedule candidate µs','Time ratio','Gate'],body))
parts.append('The log15/log16 candidates add one hot allocation of 1,520 bytes; their near-parity medians do not pass the allocation gate. Log8/one-lane has a measured slowdown; other listed inconclusive cases do not establish the P95 bound. This one-thread campaign does not establish multicore NTT performance. There is no 1,024-point NTT row in this campaign.')
with (out/'retained-comparisons.csv').open('w') as f:
 fields=['campaign','operation','family','size','baseline','variant','baseline_us','candidate_us','median_ratio','median_ci_low','median_ci_high','p95_ratio','p95_ci_low','p95_ci_high','status','allocation_calls','allocation_bytes']
 w=csv.DictWriter(f,fieldnames=fields,extrasaction='ignore');w.writeheader();w.writerows(oldrows)
parts.extend(['## Method, limitations and evidence',
 f"The new campaign used five fresh processes × 32 shuffled paired samples, seed offset {meta['seed_offset']}, one caller thread, `-C target-cpu=native`, optimization level 3, fat LTO and one codegen unit. The runner froze sources, dependencies, the manifest and executable before measurement. Retried cases: {', '.join(meta.get('retried_cases',[])) or 'none'}. Each retry follows the predeclared five-process × 64-sample policy and replaces the initial result; no retry-until-pass selection was used.",
 'The 1% gate requires the median and P95 upper confidence bounds and every process median to be at most 1.01, correct outputs and no added allocations. Intervals are pointwise 95% hierarchical paired bootstrap intervals; P95 describes timed-batch averages, not individual-call tails. Core placement, temperature and unrelated host load were uncontrolled. The Apple M1 Max/64 GiB host identity comes from the earlier host observation; the runner’s sandboxed hardware query may be unavailable in metadata.',
 'Validation: independent GF polynomial/BigUint/BigInt oracles, exhaustive GF8 products, carries and signed boundaries, quotient/remainder identities, prime powers/inverses, canonical codec checks, empty and ragged inputs. All 125 smoke cases matched the manifest. Every confirmation process completed correctness checks. The existing Python policy suite passed 17 tests. Independent reviews checked integer oracle semantics, output observability, divisor setup and report attribution; no remaining issues were found.',
 'Remaining measurements require implementations: full owned-library/context API, exact signed-wide and mixed coefficient accumulators, fixed-schedule secret-input arithmetic, batch inversion, mutable fused folds and integrated proof paths. New baseline timings include current variable-time APIs and do not certify constant-time behavior. ARM results do not establish x86 performance. Production code was unchanged by this work.',
 '- [Every new measurement, including 16-element batches and P95](measurements.md)\n- [New operation CSV](operation-metrics.csv) and [retained comparison CSV](retained-comparisons.csv)\n- [New statistics](summary.json), [frozen manifest](required_cases.json), [metadata and hashes](metadata.json), [gate output](gate.txt)\n- Raw CSV/logs under `initial/` and optional `retry/`\n- [Frozen source/executable archive](frozen-inputs.tar.gz) and [attachment hashes](attachments-sha256.json)\n- [Reproduction and API scope](../../OPERATION_METRICS.md)\n- Earlier reports: [arithmetic](../arithmetic-arm-01/REPORT.md), [updated integer kernels](../integer-focus-arm-03/REPORT.md), [GF/NTT statistics](../arm-gated-01/measurements.md)'
])
(out/'REPORT.md').write_text('\n\n'.join(parts)+'\n')
print('Wrote REPORT.md, operation-metrics.csv, retained-comparisons.csv')
print('Selected statuses:',dict(counts))

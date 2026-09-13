#!/usr/bin/env python3
"""Build the human-readable report from validated measurements, without running proofs."""
import datetime
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parent
data = json.loads((ROOT / 'analysis.json').read_text())
native, wide, sha = (data[k] for k in ('native', 'wide', 'sha'))
names = {'binius64':'Binius64 / BaseFold', 'binius64-ligerito':'Binius64 / Ligerito',
         'f2z':'BitZ', 'plonky3-fri':'Plonky3 / FRI', 'limber':'Limber', 'f2z-split':'BitZ Split'}

def link(name):
    return f'[{name}]({ROOT / name})'

def table(headers, rows):
    return '\n'.join(['| '+' | '.join(headers)+' |', '| '+' | '.join(['---']*len(headers))+' |',
                      *['| '+' | '.join(str(x) for x in r)+' |' for r in rows]])

def num(n):
    return f'{n:,.2f}'

missing = sum(len(rows) for rows in data['missing'].values())
completion_path = ROOT/'completion.json'
completion = json.loads(completion_path.read_text()) if completion_path.exists() else None
complete = missing == 0 and not data['incomplete'] and completion is not None and completion['status']=='complete'
parts = ['# Albert benchmark verification — requested campaign',
         f"Generated {datetime.datetime.now(datetime.timezone.utc).isoformat()}. "
         f"**{'Complete' if complete else 'In progress'}:** {len(native)}/152 native comparison cases, "
         f"{len(wide)}/24 BitZ wide cases, {len(sha)}/6 SHA+ECDSA cases. Each completed case has five measured repetitions; "
         'native and SHA warmups are retained separately. The native matrix includes the independently repeated Binius sweeps requested in both command groups.',
         '## What this establishes',
         'The original four intended Binius configurations are BaseFold and the Ligerito adapter, each at initial rates **1/2 and 1/8**. '
         'The historical prose/labels disagree; no historical rate-1/4 result was verified. This campaign adds rate 1/4 explicitly, '
         'producing **six current configurations**. A new rate-1/4 measurement does not authenticate an old rate-1/4 label.',
         'The adapter replaces the Binius PCS opening path with ring switching plus Johnson-regime Ligerito, while retaining the supported Binius constraint system and PIOP. '
         'The compared implementations also differ in hash/transcript and folding parameters: standard Binius uses SHA-256, the adapter uses BLAKE3. '
         'Treat these as configured implementation comparisons, not an isolated experiment changing only the PCS algorithm.',
         'A blanket claim of negligible prover or memory impact is not established. Use the measured changes by rate and size below. '
         'Proof size is stable for a given case; the timing data have appreciable variation and known machine contention.',
         '## Execution and validation',
         'Apple M1 Max, 64 GiB RAM, Rust 1.98.1, `-C target-cpu=native`, eight Rayon threads, release profile. '
         'Commands run sequentially. The requested `unchecked` feature applies to the performance sweeps; the selectable-rate unit test uses its requested `binius64-bench` feature. '
         'The unit test passed for all three rates, both committed oracles, and cross-rate proof rejection. '
         'All native timing and memory records included here report successful proof verification and matching corpus hashes. All SHA cases share the same fixture.',
         'The first native attempts stopped before measurement because two new environment knobs were missing from the harness allowlist. '
         'The wide attempts stopped before measurement because `cargo run` needed both `--bin f2z` and the required `span-metrics` feature. Corrected retries preserve all original failures and logs. '
         'No failed group is silently relabeled as a successful run. The SHA runner tests passed (10 tests). '
         'Earlier circuit/statement validation passed 22 tests; see the linked earlier audit for the negative cases.',
         'Only this campaign’s process group is monitored, with a 48 GiB RSS stop threshold on this 64 GiB host. '
         'No unrelated application was stopped. macOS StorageManagementService was observed using 152% CPU at 14:10:24 UTC. '
         'This is a timing limitation: five samples and these sequential duplicate sweeps do not establish precise small differences or statistical equivalence. '
         'During the final Plonky3 case, the host also had 36.6 GiB of swap in use. A warmup-time vmmap snapshot reported a 43.9G main-process footprint, while the separate memory-only child reported 29.2 GiB peak RSS. '
         'These are different measurement boundaries, and RSS excludes part of the compressed/swapped footprint. The largest-case timings and RSS must be interpreted under this memory pressure. See '+link('memory-pressure.json')+'.',
         'Commands/provenance: '+', '.join(link(n) for n in ('request.json','retry-commands.json','wide-retry-commands.json','status.json','retry-status.json','wide-retry-status.json','initial-source.patch','retry-source.patch','retry-source-sha256.json','timing-contention.json') if (ROOT/n).exists())+'.',
         'Earlier statement/negative-test audit: [albert-review-2026-09-13.md](/Users/johnwu/code/zk/f2z-pcs/docs/albert-review-2026-09-13.md).',
         'Source stability: all 481 snapshotted source/build files match the recorded baseline plus the documented retry corrections. See '+link('source-stability.json')+'.',
         '## Measurement boundaries',
         '- Native **witness-to-proof** is the recorded interval from witness generation through proof completion, excluding reusable setup and verification. '
         'Adapter serialization is inside its proving interval and decoding is inside verification; standard Binius generates/consumes native transcript bytes. '
         'These are the existing benchmark boundaries. Phase intervals can overlap (for example witness packing inside the online prover); do not sum all columns.\n'
         '- Native peak RSS is a separate fresh-process pass including corpus, setup, witness, one verified proof, and proof accounting. It is not prover-only memory.\n'
         '- Wide CLI proving reuses a witness and reports witness construction separately. Its peak is tracked live heap in **MiB**, although its original output says MB. '
         'It is not comparable directly with RSS. The CLI retains aggregate medians, not all individual timing samples.\n'
         '- SHA witness-to-proof excludes reusable setup, fixture/signature generation, verification, and transport codec work. Its RSS is the worker process peak across setup and all trials.\n'
         '- Native/SHA proof figures count complete proof material including commitment. Wide CLI raw bytes omit its 32-byte BLAKE3 commitment root; an explicitly adjusted figure is also provided. '
         'Expected public inputs are separate. All KB below are decimal; GiB is binary.',
         '## Focused Binius comparison at 2^22 multiplications']

focused = sorted((r for r in native if r['family']=='binius-focused' and r['log_multiplications']==22),
                 key=lambda r:(r['log_inv_rate'],r['backend']))
parts.append(table(['Rate','Backend','Witness→proof ms','P10–P90 ms','Verify ms','Proof KB','Peak RSS GiB'],
                   [[r['rate'],names[r['backend']],num(r['witness_to_proof_ms']),
                     num(r['witness_to_proof_ms_p10'])+'–'+num(r['witness_to_proof_ms_p90']),
                     num(r['verify_ms']),num(r['proof_bytes']/1000),num(r['peak_rss_bytes']/2**30)] for r in focused]))
changes = [r for r in data['binius_changes'] if r['family']=='binius-focused' and r['log_multiplications']==22]
parts.append('Change from BaseFold to Ligerito, using the five-sample medians (negative means smaller/faster):')
parts.append(table(['Rate','Witness→proof','Commit','PIOP','Opening','Verifier','Proof bytes','Peak RSS'],
                   [[r['rate'],*[f"{r[k+'_change_pct']:+.1f}%" for k in ('witness_to_proof_ms','commit_ms','piop_ms','opening_ms','verify_ms','proof_bytes','peak_rss_bytes')]] for r in changes]))
parts.append('Stage changes are observations for the configured backends, including their different hashing, serialization, and security models. '
             'The full exponent sweep is retained below and in '+link('binius-changes.csv')+'.')
repeat_changes = [r for r in data['binius_changes'] if r['family']=='all-provers' and r['log_multiplications']==22]
parts.append('Independent repeat within the broader comparison at 2^22; percentage change from BaseFold to Ligerito:')
parts.append(table(['Rate','Witness→proof','Verifier','Proof bytes','Peak RSS'],
                   [[r['rate'],*[f"{r[k+'_change_pct']:+.1f}%" for k in ('witness_to_proof_ms','verify_ms','proof_bytes','peak_rss_bytes')]] for r in repeat_changes]))
quarter_repeat=next((r for r in native if r['family']=='all-provers' and r['backend']=='binius64' and r['log_inv_rate']==2 and r['log_multiplications']==22),None)
quarter_focused=next((r for r in focused if r['backend']=='binius64' and r['log_inv_rate']==2),None)
if quarter_repeat and quarter_focused:
    parts.append(f"**Verifier timing is not stable across the repeated runs.** Rate-1/4 BaseFold at 2^22 measured {quarter_focused['verify_ms']:.2f} ms in the focused pass and {quarter_repeat['verify_ms']:.2f} ms in the broader pass with identical protocol configuration and proof size. "
                 'The large apparent verifier reduction in the focused table is not sufficient evidence for a precise speedup claim. Keep both observations and confirm verifier performance on an idle machine.')
parts.append('Interactive phase intervals with sample distributions: '+link('focused-intervals/intervals.html')+'. '
             'The focused traces passed structural validation: 288 warmup/measured runs and 3,456 spans. '
             'Derived traces normalize legacy trial-index labels, record the actual release build profile and clarify measurement boundaries; '
             'all original measured intervals and raw traces are preserved. Primary totals use interval unions, not sums of nested spans.')
parts.append('The broader comparison is available in '+link('all-provers-intervals/intervals.html')+'. '
             'Its traces passed structural validation for 624 warmup/measured runs and 6,816 spans. '
             'Across both visualizations, all 1,064 phase-median checks match the original measurements. '
             'The browser URL policy blocked visual preview of the local HTML; the structural and numerical checks passed.')

parts.extend(['## Actual Binius parameters',
              table(['Initial rate','BaseFold queries','BaseFold grinding','BaseFold security scope'],
                    [['1/2',241,0,'FRI query phase only'],['1/4',148,0,'FRI query phase only'],['1/8',121,0,'FRI query phase only']]),
              'BaseFold query counts use the implementation’s target-100 formula; the label does not include PIOP or FRI folding soundness. '
              'The reported regime is unique decoding. Fold arities, message lengths and final challenges are recorded per shape.',
              'Ligerito uses Johnson OOD proximity with eta=0.02, 32 initial lanes, per-level query/fold grinding, and Round-0 OOD grinding. '
              'Its reported `whole_protocol_bits` is the implementation’s modeled composition bound including its grinding model, gated at at least 100. '
              'It is not an independently established unconditional Fiat–Shamir security theorem. Rates in the following table describe the initial commitment rate; deeper levels can use different rates.'])
ligs = [r for r in focused if r['backend']=='binius64-ligerito']
parts.append(table(['Rate','Component target','Modeled composition bits','Initial queries','Initial fold grind','Initial query grind','Round-0 grind (oracle 0)'],
                   [[r['rate'],r['config']['ligerito_component_bits'],f"{r['config']['whole_protocol_bits']:.6f}",
                     r['config']['level0_queries'],r['config']['level0_fold_grinding_bits'],r['config']['level0_query_grinding_bits'],r['config']['ood_grinding_bits']] for r in ligs]))
parts.append('Every oracle’s full level schedule, error terms, and Round-0 grinding are preserved in `config` in '+link('analysis.json')+' and the original `samples.jsonl` records. '
             'A flat table of all 189 oracle-level parameter rows from the focused runs is available in '+link('binius-ligerito-parameters.csv')+'. '
             'BitZ’s native comparison reports round-by-round economic accounting. Plonky3 reports its native proven-security calculation, including AIR/FRI, with queries increased as required. '
             'Limber uses its native Brakedown parameter set and is a single baseline, not a rate-1/2, 1/4, or 1/8 experiment.')
p3_examples=sorted([r for r in native if r['backend']=='plonky3-fri' and r['log_multiplications']==15],key=lambda r:r['log_inv_rate'])
parts.append(table(['Plonky3 initial rate','Queries','Reported proven bits','Unique-decoding bits','List-decoding bits'],
                   [[r['rate'],*[r['config'][k] for k in ('num_queries','proven_bits','unique_decoding_bits','list_decoding_bits')]] for r in p3_examples]))
parts.append('The Plonky3 table shows the recorded 2^15 configuration; every case retains its full report. '
             'The existing minimum of 100 queries is preserved. It produces a reported 127-bit bound at rate 1/8, so these rate sweeps are not uniformly tuned to exactly 100 bits even within Plonky3.')

parts.extend(['## SHA-256 followed by P-256 ECDSA',
              'The measured workload is **one chain of 128 total compressions including padding**, over **8,128 message bytes**, followed by one P-256 ECDSA verification. '
              'It is not 128 independent chains. A 4,032-byte message requires 64 total compressions; exactly 4,096 bytes requires 65. '
              'The current power-of-two interface does not select 65 compressions.',
              'Both circuits constrain the SHA output to the ECDSA digest input, match P-256/public-key/signature/padding semantics, and reject the tested message/signature/public-input mutations. '
              'The public statement contains the exponent, key and signature; message and digest are witnesses. These benchmarks do not promise zero knowledge. '
              'The Binius–Ligerito adapter does not support this composed circuit because it contains BMUL constraints; the SHA comparison here uses standard Binius64/BaseFold.'])
parts.append(table(['Rate','Backend','Setup ms (once)','Witness→proof ms','P10–P90 ms','Verify ms','Proof KB','Peak RSS GiB'],
                   [[r['rate'],names[r['method']],num(r['setup_ms']),num(r['witness_to_proof_ms']),
                     num(r['witness_to_proof_ms_p10'])+'–'+num(r['witness_to_proof_ms_p90']),num(r['verify_ms']),
                     num(r['proof_material_bytes']/1000),num(r['peak_rss_bytes']/2**30)] for r in sorted(sha,key=lambda r:(r['log_inv_rate'],r['method']))]))
parts.append('These “target 100” settings have different scopes. BitZ reports an economic bound including grinding and separately a statistical bound without grinding; '
             'standard Binius reports the FRI-query-only target. Do not describe this as a comparison at a demonstrated identical complete-protocol statistical security level. '
             'Full security reports, phase timings and samples: '+link('sha-summary.csv')+' and '+link('analysis.json')+'.')
parts.append('The witness-to-proof comparison amortizes reusable setup. BitZ’s measured setup is larger here and is shown separately; '
             'the faster proof-generation interval must not be presented as the total latency of a cold invocation including setup.')

parts.extend(['## Full-width versus wrapping u32 multiplication',
              'The CLI `--mul-sweep` constructs `u32 × u32 → u64` products. The shared comparison workload `u32-mod32` exposes the low 32-bit result. '
              'In BitZ, both use the same full-product relation `x*y = z_low + 2^32*z_high` with four committed 32-bit limbs (128 committed bits per multiplication). '
              'Thus the code supports equal BitZ relation/constraint cost. These requested commands use different input generators and measurement drivers, '
              'so their timings alone are not a paired experiment proving identical runtime. The shared benchmark has no full-width-u32 cross-backend mode; its `u64` mode would use 64-bit operands.',
              table(['Rate','Exponent','Witness ms','Prove after witness ms','Verify ms','Raw proof B','Proof B incl. root','Tracked heap MiB'],
                    [[r['rate'],r['log_multiplications'],num(r['witness_ms']),num(r['prove_ms']),num(r['verify_ms']),
                      r['proof_bytes'],r['proof_material_bytes_with_root'],num(r['tracked_heap_peak_mib'])] for r in wide])])
if data.get('wide_vs_wrapping'):
    parts.append(f"For all {len(data['wide_vs_wrapping'])} completed wide/wrapping counterpart pairs, the full recorded Ligerito configuration and matrix geometry match exactly. "
                 'The proof-byte comparison includes the CLI’s omitted commitment root. See '+link('wide-vs-wrapping.csv')+'. '
                 'This verifies configuration equivalence, not equality of runtime under the different drivers and input generators.')

parts.append('## All native comparison measurements')
parts.append('Times are five-sample medians in milliseconds. Setup is a single untimed-for-proving setup measurement. '
             'Original samples and P10/P90 values are in '+link('analysis.json')+'.')
for family in ('binius-focused','all-provers'):
    rows = sorted([r for r in native if r['family']==family],key=lambda r:(r['log_inv_rate'] or 99,r['log_multiplications'],r['backend']))
    parts.append('### '+family)
    parts.append(table(['Rate','e','Backend','Setup','Witness','Commit','PIOP','Opening','Witness→proof','Verify','Proof KB','RSS GiB'],
                       [[r['rate'],r['log_multiplications'],names[r['backend']],
                         *[num(r[k]) for k in ('setup_ms','witness_ms','commit_ms','piop_ms','opening_ms','witness_to_proof_ms','verify_ms')],
                         num(r['proof_bytes']/1000),num(r['peak_rss_bytes']/2**30)] for r in rows]))

parts.extend(['## Coverage and evidence',
              'Missing/incomplete cases (empty lists mean all requested cases were measured):\n\n```json\n'+json.dumps({k:data[k] for k in ('missing','incomplete')},indent=2)+'\n```',
              'Machine-readable outputs: '+', '.join(link(n) for n in ('native-summary.csv','binius-changes.csv','wide-summary.csv','sha-summary.csv','analysis.json'))+'.',
              'Local draft response: '+link('ALBERT-REPLY.md')+'. It has not been sent to Slack.',
              'Rebuild this report after the campaign finishes by running `analyze_results.py`, then `build_report.py` in this directory. '
              'Both only read recorded results and write summaries; they do not execute benchmarks.'])
if completion:
    parts.append('Execution status: '+link('completion.json')+f" records {completion['completed_requested_groups']}/{completion['requested_groups']} requested command groups complete and {completion['measured_proof_repetitions']} measured proof repetitions. "
                 'It preserves the original failed launches and the successful corrected attempts separately.')
    test = completion.get('additional_plonky3_validation')
    if test and test['status']=='complete':
        parts.append('The four additional Plonky3 tests passed, covering arithmetic boundary/carry constraints, rejection of a field alias, proven-security accounting across supported shapes and rates, and proof roundtrip/tamper rejection at all three rates. '
                     'Log: '+link('logs/plonky3-rate-validation.retry2.log')+'.')
(ROOT/'REPORT.md').write_text('\n\n'.join(parts)+'\n')
print(f"Wrote {ROOT/'REPORT.md'} ({'complete' if complete else 'in progress'})")

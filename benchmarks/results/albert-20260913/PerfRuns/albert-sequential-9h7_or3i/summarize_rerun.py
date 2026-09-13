#!/usr/bin/env python3
"""Compare recorded rerun data with the previous campaign and verify serial execution."""
import csv
import datetime
import hashlib
import json
from pathlib import Path
import re
import statistics

ROOT=Path(__file__).resolve().parent
REPO=ROOT.parents[1]
request=json.loads((ROOT/'request.json').read_text())
OLD=Path(request['previous_campaign'])
data=json.loads((ROOT/'analysis.json').read_text())
prior=json.loads((OLD/'analysis.json').read_text())
jobs=json.loads((ROOT/'commands.json').read_text())
statuses=json.loads((ROOT/'status.json').read_text())

def key(r):
    return (r['family'],r['backend'],r['log_inv_rate'],r['log_multiplications'])

def utc(s):
    return datetime.datetime.fromisoformat(s)

old_native={key(r):r for r in prior['native']}
comparison=[]
for row in data['native']:
    old=old_native[key(row)]
    assert row['config']==old['config'], ('configuration changed',key(row))
    assert row['corpus_digest']==old['corpus_digest'], ('inputs changed',key(row))
    result={k:row[k] for k in ('family','rate','backend','log_multiplications')}
    for metric in ('setup_ms','witness_to_proof_ms','verify_ms','proof_bytes','peak_rss_bytes'):
        result['old_'+metric]=old[metric]
        result['new_'+metric]=row[metric]
        result[metric+'_change_pct']=100*(row[metric]/old[metric]-1)
    comparison.append(result)

if comparison:
    with (ROOT/'previous-vs-rerun.csv').open('w',newline='') as stream:
        writer=csv.DictWriter(stream,fieldnames=list(comparison[0]))
        writer.writeheader(); writer.writerows(comparison)

assert [s['name'] for s in statuses]==[j['name'] for j in jobs[:len(statuses)]], 'execution order mismatch'
gaps=[]
for before,after in zip(statuses,statuses[1:]):
    assert before['status']=='complete' and 'finished_utc' in before, 'next job started before successful completion'
    gap=(utc(after['started_utc'])-utc(before['finished_utc'])).total_seconds()
    assert gap>=20, ('missing cooldown or overlapping commands',gap)
    gaps.append(gap)

observations=[]
for line in (ROOT/'system-observations.jsonl').read_text().splitlines():
    try: observations.append(json.loads(line))
    except json.JSONDecodeError: pass
telemetry=[]
for job in jobs:
    samples=[s for s in observations if s['job']==job['name']]
    if not samples: continue
    during=[s for s in samples if s['stage']=='during']
    worker_paths=set()
    if job['kind']=='sha':
        output=Path(job['command'][job['command'].index('--output')+1])
        manifest=output/'manifest.json'
        if manifest.exists():
            metadata=json.loads(manifest.read_text())
            worker_paths={metadata['binary'],metadata['binius64']['binary']}
    # The SHA runner starts each worker in a separate process group. The raw
    # monitor's outside-PG list can therefore contain an owned worker; exclude
    # the exact executables recorded by that job's manifest from this summary.
    external_totals=[sum(p['cpu_pct'] for p in s['top_external_cpu'] if p['process'] not in worker_paths) for s in during]
    paging=[]
    for a,b in zip(samples,samples[1:]):
        seconds=b['monotonic_seconds']-a['monotonic_seconds']
        size=int(re.search(r'page size of (\d+) bytes',b['vm_stat']).group(1))
        if seconds>0:
            paging.append({k:(b['vm_counters'][k]-a['vm_counters'][k])*size/2**20/seconds for k in ('Swapins','Swapouts','Compressions','Decompressions')})
    telemetry.append(dict(job=job['name'],snapshots=len(samples),
       peak_observed_recorded_external_cpu_pct=max(external_totals,default=None),
       median_observed_recorded_external_cpu_pct=statistics.median(external_totals) if external_totals else None,
       excluded_owned_worker_paths=sorted(worker_paths),
       maximum_system_paging_mib_per_second={k:max((r[k] for r in paging),default=0) for k in ('Swapins','Swapouts','Compressions','Decompressions')}))
(ROOT/'system-observations-summary.json').write_text(json.dumps(telemetry,indent=2)+'\n')

counts={k:len(data[k]) for k in ('native','wide','sha')}
source_hashes=json.loads((ROOT/'source-sha256.json').read_text())
changed=[name for name,wanted in source_hashes.items() if hashlib.sha256((REPO/name).read_bytes()).hexdigest()!=wanted]
rate_test='test result: ok. 1 passed; 0 failed;' in (ROOT/'logs/rate-validation.log').read_text()
complete=(len(statuses)==len(jobs) and all(s['status']=='complete' for s in statuses)
          and counts==request['expected_cases'] and not data['incomplete'] and not any(data['missing'].values())
          and not changed and rate_test)
completion={'status':'complete' if complete else 'in_progress','counts':counts,'measured_repetitions':5*sum(counts.values()),
            'completed_jobs':sum(s['status']=='complete' for s in statuses),'expected_jobs':len(jobs),
            'observed_job_intervals_nonoverlapping':True,'checked_intercommand_gaps':len(gaps),
            'minimum_intercommand_gap_seconds':min(gaps) if gaps else None,
            'source_files_checked':len(source_hashes),'source_files_changed':changed,'rate_test_passed':rate_test,
            'same_native_configuration_and_corpus_pairs':len(comparison),
            'generated_utc':datetime.datetime.now(datetime.timezone.utc).isoformat()}
(ROOT/'completion.json').write_text(json.dumps(completion,indent=2)+'\n')

def link(name): return f'[{name}]({ROOT/name})'
def table(headers,rows):
    return '\n'.join(['| '+' | '.join(headers)+' |','| '+' | '.join(['---']*len(headers))+' |',*['| '+' | '.join(str(v) for v in row)+' |' for row in rows]])
def n(x): return f'{x:,.2f}'

parts=['# Sequential benchmark rerun',
       f"**{'Complete' if complete else 'In progress'}:** {counts['native']}/152 native cases, {counts['wide']}/24 wide cases, {counts['sha']}/6 SHA+ECDSA cases. Five measured repetitions and one excluded warmup per case. All included cases passed verification.",
       'The previous campaign also ran commands sequentially. This rerun selects one native backend per fresh process, waits 20 seconds between commands, and records system activity approximately every 10 seconds. Each backend process still sweeps sizes 2^15 through 2^22. The BitZ wide CLI also waits 20 seconds between sizes. All provers use eight threads internally.',
       'A global controller lock prevents duplicate campaign controllers. Each command must exit successfully before the next starts. The completed command intervals and cooldowns were checked: '+link('completion.json')+'. Exact commands: '+link('COMMANDS.md')+'.',
       'Benchmark source, inputs, initial rates, queries, security accounting, and repetition counts are held fixed. All completed native cases have the same recorded configuration and input corpus as their counterparts in the earlier campaign. The process-isolation policy and observation overhead changed, so differences cannot be attributed exclusively to background activity.',
       '## Machine state and interpretation',
       'Sequential execution does not isolate the machine from other applications or OS work. Storage scanning, Spotlight and other application activity were observed. The rerun is not certified as an idle-machine measurement. System swap occupancy alone is not an active paging rate; the telemetry summary uses differences in cumulative VM counters between snapshots.',
       'Telemetry is attached to whole commands, not individual proof intervals. It cannot identify the precise cause of a particular verification spike. `/usr/bin/time -l` resource summaries in the logs include command startup, possible compilation, setup and all trials; they are not per-proof CPU times. Peak RSS from native records comes from a separate verified memory pass; the controller RSS observations have a different boundary.',
       'The raw monitor labels processes outside the command process group as external. SHA workers start in separate process groups; the telemetry summary excludes their exact manifest-recorded executable paths from external CPU totals. The controller RSS guard covers its selected process group, while the SHA runner records each worker’s peak RSS separately.',
       'Observations: '+link('system-observations.jsonl')+'; per-command summaries: '+link('system-observations-summary.json')+'.',
       '## Binius at 2^22: previous run versus rerun',
       'Times below are five-sample medians in milliseconds. Keep the focused and broader campaigns separate. Complete distributions and per-stage timings remain in '+link('analysis.json')+'.']
rows=[r for r in comparison if r['log_multiplications']==22]
parts.append(table(['Sweep','Rate','Backend','Old witness→proof','New witness→proof','Old verify','New verify','Proof-byte change'],
                   [[r['family'],r['rate'],r['backend'],*[n(r[k]) for k in ('old_witness_to_proof_ms','new_witness_to_proof_ms','old_verify_ms','new_verify_ms')],f"{r['proof_bytes_change_pct']:+.2f}%"] for r in rows if r['backend'] in ('binius64','binius64-ligerito')]))
parts += ['## Binius PCS comparison within the rerun',
          table(['Sweep','Rate','Witness→proof change','Verifier change','Proof-byte change','RSS change'],
          [[r['family'],r['rate'],*[f"{r[k+'_change_pct']:+.1f}%" for k in ('witness_to_proof_ms','verify_ms','proof_bytes','peak_rss_bytes')]] for r in data['binius_changes'] if r['log_multiplications']==22]),
          'Changes are Ligerito relative to BaseFold at 2^22. They are observations of the configured implementations, which differ in hashing, folding and security accounting as well as PCS. They do not establish an isolated PCS effect or statistical equivalence.',
          '## SHA-256 followed by P-256 ECDSA',
          'One chain of 128 total compressions including padding, 8,128 message bytes, one signature. Standard Binius64 and BitZ Split; the Binius–Ligerito adapter cannot handle the composed circuit. Security-target scopes differ as documented in the earlier audit.',
          table(['Rate','Backend','Setup ms','Witness→proof ms','Verify ms','Proof bytes','RSS GiB'],[[r['rate'],r['method'],n(r['setup_ms']),n(r['witness_to_proof_ms']),n(r['verify_ms']),int(r['proof_material_bytes']),n(r['peak_rss_bytes']/2**30)] for r in data['sha']]),
          '## Full-width BitZ multiplication',
          'The CLI proves u32 × u32 → u64. Its prove interval excludes witness construction; its memory figure is tracked heap MiB, not native RSS. Root-inclusive proof bytes add the 32-byte commitment root omitted by the CLI output. The wrapping BitZ backend retains the same full-product relation. Different drivers and inputs prevent paired runtime equivalence claims.',
          table(['Rate','Exponent','Witness ms','Prove ms','Verify ms','Root-inclusive proof bytes','Tracked heap MiB'],[[r['rate'],r['log_multiplications'],n(r['witness_ms']),n(r['prove_ms']),n(r['verify_ms']),r['proof_material_bytes_with_root'],n(r['tracked_heap_peak_mib'])] for r in data['wide']]),
          '## All native measurements',
          'Times are milliseconds. Setup is separate; phase intervals overlap and must not be summed. Proof sizes include commitment material. Native RSS is a separate full-process memory pass.',
          table(['Sweep','Rate','e','Backend','Setup','Witness','Commit','PIOP','Opening','Witness→proof','Verify','Proof bytes','RSS GiB'],[[r['family'],r['rate'],r['log_multiplications'],r['backend'],*[n(r[k]) for k in ('setup_ms','witness_ms','commit_ms','piop_ms','opening_ms','witness_to_proof_ms','verify_ms')],int(r['proof_bytes']),n(r['peak_rss_bytes']/2**30)] for r in data['native']]),
          '## Evidence',
          ', '.join(link(name) for name in ('native-summary.csv','previous-vs-rerun.csv','binius-changes.csv','wide-summary.csv','sha-summary.csv','analysis.json','source-sha256.json','source.patch') if (ROOT/name).exists()),
          'Earlier campaign and statement/security audit: '+f'[previous report]({OLD/"REPORT.md"}).',
          'Missing or incomplete cases:\n\n```json\n'+json.dumps({k:data[k] for k in ('missing','incomplete')},indent=2)+'\n```']
for name in ('focused-intervals','all-provers-intervals'):
    if (ROOT/name/'intervals.html').exists():
        parts.append('Interactive phase report: '+link(name+'/intervals.html')+'. Numerical/structural validation is recorded alongside the report.')
(ROOT/'REPORT.md').write_text('\n\n'.join(parts)+'\n')
print(json.dumps(completion,indent=2))

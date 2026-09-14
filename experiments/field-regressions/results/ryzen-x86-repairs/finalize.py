import collections
import gzip
import hashlib
import json
import shutil
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT))
from analysis import classify, expanded
from gate import evaluate

campaign = Path(sys.argv[1])
out = ROOT / 'results/ryzen-x86-repairs'
labels = ['l3-96m', 'l3-32m', 'l3-96m-8t', 'l3-32m-8t', 'physical-16t', 'smt-32t']
assert json.loads((campaign/'campaign.json').read_text())['status'] == 'complete'
records = []
original_gates = {}
source_hashes = None
for label in labels:
    directory = campaign/label
    issues = evaluate(directory)
    assert not any(issue.startswith('unmeasured:') for issue in issues), issues
    original_gates[label] = issues
    metadata = json.loads((directory/'metadata.json').read_text())
    assert metadata['phase'] == 'confirm' and metadata['runs'] >= 5 and metadata['samples'] >= 32
    if source_hashes is None:
        source_hashes = metadata['source_sha256']
    assert source_hashes == metadata['source_sha256']
    rows = json.loads((directory/'summary.json').read_text())
    assert all(r['complete'] and (r['diagnostic'] or r['correctness']) for r in rows)
    rows = {(r['family'], r['size'], r['variant']): r for r in rows}
    rankings = {(r['family'],r['size']):r for r in json.loads((directory/'rankings.json').read_text())}
    spec = json.loads((directory/'required_cases.json').read_text())
    assert spec['max_slowdown'] == .01
    for group in spec['families']:
        if group.get('diagnostic'):
            continue
        proposed = group['selected']['x86_64']
        baseline = group['baseline']
        for size in group['sizes']:
            key = (group['name'],size)
            row = rows[*key,proposed]
            rank = rankings[key]
            assert rank['selected'] == proposed and rank['challenger'] == group.get('challenger')
            baseline_status = classify(row, .01)
            challenger_status = rank.get('head_to_head',{}).get('decision','single_implementation')
            improved = proposed == baseline or row['median_ci_high'] < 1
            passed = improved and baseline_status == 'pass' and challenger_status in ('faster','within_1_percent','single_implementation')
            chosen = proposed if passed else baseline
            if proposed == baseline:
                decision = 'retain_production'
            elif not improved:
                decision = 'retain_production_no_demonstrated_gain'
            elif passed:
                decision = 'accept_candidate'
            else:
                decision = 'fallback_to_production'
            actual = rows[*key,chosen]
            assert classify(actual,.01) == 'pass'
            if chosen != baseline:
                assert passed and actual['allocations_ok']
            records.append(dict(placement=label,family=key[0],size=size,
                proposed=proposed,chosen=chosen,baseline=baseline,decision=decision,
                proposal_production_status=baseline_status,proposal_improvement_verified=improved,
                proposal_challenger_status=challenger_status,
                proposal_round=row['round'],proposal_median_ratio=row['median_ratio'],
                chosen_median_ns=actual['median_ns'],baseline_median_ns=rows[*key,baseline]['median_ns'],
                chosen_median_ratio=actual['median_ratio'],
                chosen_median_ci_high=actual['median_ci_high'],
                chosen_p95_ci_high=actual['p95_ci_high'],
                chosen_worst_process=max(actual['per_run_medians']),
                baseline_allocation_calls=rows[*key,baseline]['allocation_calls'],baseline_allocation_bytes=rows[*key,baseline]['allocation_bytes'],
                chosen_allocation_calls=actual['allocation_calls'],
                chosen_allocation_bytes=actual['allocation_bytes'],
                evidence=str(directory)))
    dest = out/'evidence'/label
    dest.mkdir(parents=True,exist_ok=True)
    for filename in ['metadata.json','required_cases.json','summary.json','rankings.json','selection.json']:
        shutil.copy2(directory/filename,dest/filename)
    for round_name in metadata['round_metadata_sha256']:
        rd = dest/round_name
        rd.mkdir(exist_ok=True)
        shutil.copy2(directory/round_name/'metadata.json',rd/'metadata.json')
        for path in (directory/round_name).glob('run-*.*'):
            with path.open('rb') as source, gzip.open(rd/(path.name+'.gz'),'wb') as target:
                shutil.copyfileobj(source,target)
        for path in (directory/round_name).glob('host-*.json'):
            shutil.copy2(path,rd/path.name)

def retain_consistent_integer_shapes(records):
    # A fixture is test data, not a permissible runtime dispatch condition.
    integer_groups = collections.defaultdict(list)
    for record in records:
        if record['family'] == 'integer_full_mac':
            integer_groups[record['placement'],record['size'].split('_',1)[1]].append(record)
    for key, fixtures in integer_groups.items():
        assert len(fixtures) == 2
        if len({r['chosen'] for r in fixtures}) > 1:
            for r in fixtures:
                if r['chosen'] != r['baseline']:
                    r.update(chosen=r['baseline'],decision='fallback_to_production_for_fixture_group',
                        chosen_median_ns=r['baseline_median_ns'],chosen_median_ratio=1.,
                        chosen_median_ci_high=1.,chosen_p95_ci_high=1.,chosen_worst_process=1.,
                        chosen_allocation_calls=r['baseline_allocation_calls'],chosen_allocation_bytes=r['baseline_allocation_bytes'])
                r['fixture_group_reason']='A single public-shape kernel must pass both random and carry-heavy inputs.'
        assert len({r['chosen'] for r in fixtures}) == 1

assert len(records) == 302, len(records)
retain_consistent_integer_shapes(records)
refinement_issues = {}
if len(sys.argv) > 2:
    refinement_root = Path(sys.argv[2])
    plan = json.loads((refinement_root/'plan.json').read_text())
    assert plan['status'] == 'complete'
    for job in plan['jobs']:
        label = job['label']
        directory = refinement_root/label
        metadata = json.loads((directory/'metadata.json').read_text())
        assert metadata['phase'] == 'confirm' and metadata['runs'] >= 5 and metadata['samples'] >= 32
        assert metadata['source_sha256'] == source_hashes
        primary_metadata = json.loads((campaign/label/'metadata.json').read_text())
        for key in ['cpu_set','threads','flags','arch','scope','stream_n']:
            assert metadata.get(key) == primary_metadata.get(key)
        assert metadata['host']['model'] == primary_metadata['host']['model']
        assert job['development_summary_sha256'] == hashlib.sha256((campaign/label/'summary.json').read_bytes()).hexdigest()
        used = set(json.loads((campaign/label/'selection.json').read_text())['used_seeds'])
        for round_name in primary_metadata['round_metadata_sha256']:
            rm = json.loads((campaign/label/round_name/'metadata.json').read_text())
            used.update(rm['seed_offset']+7919*i for i in range(rm['runs']))
        fresh = set()
        for round_name in metadata['round_metadata_sha256']:
            rm = json.loads((directory/round_name/'metadata.json').read_text())
            fresh.update(rm['seed_offset']+7919*i for i in range(rm['runs']))
        assert not fresh & used
        wanted = {r['case'] for r in job['refinements']}
        assert set(metadata['cases']) == wanted
        spec = json.loads((directory/'required_cases.json').read_text())
        assert spec['max_slowdown'] == .01
        allowed_missing = set()
        for item in expanded(spec):
            if item['family']+'/'+item['size'] not in wanted:
                allowed_missing.add('unmeasured: '+'/'.join(item[k] for k in ('arch','family','size','variant')))
        for group in spec['families']:
            for size in group['sizes']:
                if group['name']+'/'+size not in wanted and group.get('challenger'):
                    allowed_missing.add(f"inconclusive: {group['name']}/{size}: challenger not cleared")
        issues = [i for i in evaluate(directory) if i not in allowed_missing]
        assert not any(i.startswith('unmeasured:') for i in issues), issues
        refinement_issues[label] = issues
        rows = json.loads((directory/'summary.json').read_text())
        assert all(r['complete'] and (r['diagnostic'] or r['correctness']) for r in rows if r['family']+'/'+r['size'] in wanted)
        rows = {(r['family'],r['size'],r['variant']):r for r in rows}
        rankings = {(r['family'],r['size']):r for r in json.loads((directory/'rankings.json').read_text())}
        for refinement in job['refinements']:
            family,size = refinement['case'].split('/')
            record = next(r for r in records if (r['placement'],r['family'],r['size']) == (label,family,size))
            assert record['chosen'] == record['baseline']
            proposed = refinement['proposed']
            row = rows[family,size,proposed]
            rank = rankings[family,size]
            assert rank['selected'] == proposed and rank['challenger'] == refinement['challenger']
            status = classify(row,.01)
            h2h = rank.get('head_to_head',{}).get('decision','single_implementation')
            record['refinement'] = dict(proposed=proposed,production_status=status,challenger_status=h2h,evidence=str(directory))
            if status == 'pass' and row['median_ci_high'] < 1 and h2h in ('faster','within_1_percent','single_implementation'):
                record.update(chosen=proposed,decision='accept_refined_candidate',
                    chosen_median_ns=row['median_ns'],baseline_median_ns=rows[family,size,record['baseline']]['median_ns'],
                    chosen_median_ratio=row['median_ratio'],chosen_median_ci_high=row['median_ci_high'],
                    chosen_p95_ci_high=row['p95_ci_high'],chosen_worst_process=max(row['per_run_medians']),
                    chosen_allocation_calls=row['allocation_calls'],chosen_allocation_bytes=row['allocation_bytes'],
                    evidence=str(directory))
        dest = out/'evidence'/'refinements'/label
        dest.mkdir(parents=True,exist_ok=True)
        for filename in ['metadata.json','required_cases.json','summary.json','rankings.json','selection.json']:
            shutil.copy2(directory/filename,dest/filename)
        for round_name in metadata['round_metadata_sha256']:
            rd = dest/round_name
            rd.mkdir(exist_ok=True)
            shutil.copy2(directory/round_name/'metadata.json',rd/'metadata.json')
            for path in (directory/round_name).glob('run-*.*'):
                with path.open('rb') as source, gzip.open(rd/(path.name+'.gz'),'wb') as target:
                    shutil.copyfileobj(source,target)
            for path in (directory/round_name).glob('host-*.json'):
                shutil.copy2(path,rd/path.name)
    shutil.copy2(refinement_root/'plan.json',out/'refinement-plan.json')
retain_consistent_integer_shapes(records)
assert all(r['chosen_median_ci_high'] <= 1.01 and r['chosen_p95_ci_high'] <= 1.01 and r['chosen_worst_process'] <= 1.01 for r in records)
# Make the accepted policy directly replayable with run.py and fresh seeds.
for label in labels:
    directory = campaign/label
    selection = json.loads((directory/'selection.json').read_text())
    used = set(selection['used_seeds'])
    evidence_dirs = {directory}
    evidence_dirs.update(Path(r['evidence']) for r in records if r['placement']==label)
    evidence_dirs.update(Path(r['refinement']['evidence']) for r in records if r['placement']==label and 'refinement' in r)
    for evidence in evidence_dirs:
        em = json.loads((evidence/'metadata.json').read_text())
        for round_name in em['round_metadata_sha256']:
            rm = json.loads((evidence/round_name/'metadata.json').read_text())
            used.update(rm['seed_offset']+7919*i for i in range(rm['runs']))
    selection['used_seeds'] = sorted(used)
    for r in records:
        if r['placement'] != label:
            continue
        key = r['family']+'/'+r['size']
        challenger = None
        if r['chosen'] != r['baseline']:
            ranks = json.loads((Path(r['evidence'])/'rankings.json').read_text())
            rank = next(x for x in ranks if (x['family'],x['size']) == (r['family'],r['size']))
            assert rank['selected'] == r['chosen']
            challenger = rank.get('challenger')
        selection['choices'][key] = dict(variant=r['chosen'],challenger=challenger)
    selection['acceptance_policy'] = 'A demonstrably faster median plus independent production and strongest-challenger checks, or explicit retention of the actual production kernel. Integer choices are consistent across both input fixtures.'
    destination = out/'confirmed-selections'/label
    destination.mkdir(parents=True,exist_ok=True)
    (destination/'selection.json').write_text(json.dumps(selection,indent=2)+'\n')
result = dict(schema=1,confirmation=str(campaign),scope='six reported arithmetic families; 302 operation/size/placement cases',
    policy='Accept only complete independently confirmed candidates demonstrating a faster median and passing the unchanged 1% production and strongest-challenger checks; otherwise choose the actual production kernel.',
    production_dispatch_changed=False,upstream_revision='2d67dc6',allocation_audit_calls=64,
    counts=dict(collections.Counter(r['decision'] for r in records)),
    original_frozen_proposal_gate_issues=original_gates,refinement_case_gate_issues=refinement_issues,choices=records)
(out/'verified-choices.json').write_text(json.dumps(result,indent=2)+'\n')
lines=['# Conservative confirmed choices','',
    'These are benchmark choices, not an installed production dispatch table. Rejected and inconclusive proposals retain production. The original proposal gates are preserved in `verified-choices.json`.','',
    '| Placement | Operation | Size | Initial proposal | Final choice | Decision | Final / production |',
    '|---|---|---|---|---|---|---|']
for r in records:
    lines.append(f"| {r['placement']} | {r['family']} | {r['size']} | {r['proposed']} | {r['chosen']} | {r['decision']} | {r['chosen_median_ratio']:.4f} |")
(out/'verified-choices.md').write_text('\n'.join(lines)+'\n')
hashes = {str(p.relative_to(out)):hashlib.sha256(p.read_bytes()).hexdigest() for p in (out/'evidence').rglob('*') if p.is_file()}
(out/'evidence-sha256.json').write_text(json.dumps(hashes,indent=2)+'\n')
print(json.dumps(result['counts'],indent=2))
print('Original proposal gate issues:', {k:len(v) for k,v in original_gates.items()})
print('Refinement case gate issues:', {k:len(v) for k,v in refinement_issues.items()})

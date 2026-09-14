"""Exploratory selections and independent challenger confirmation from paired data."""
import csv
import json
import statistics
from collections import defaultdict
from pathlib import Path
from analysis import paired_stats


def samples(directory, round_name, runs):
    result=defaultdict(lambda: [[] for _ in range(runs)])
    for run in range(runs):
        with (directory/round_name/f'run-{run}.csv').open() as stream:
            rows=sorted(csv.DictReader(stream),key=lambda r:int(r['rep']))
            for row in rows:
                result[(row['family'],row['size'],row['variant'])][run].append(float(row['ns']))
    return result


def comparison(candidate, challenger):
    result=paired_stats(candidate,challenger)
    high=result['median_ci_high']
    stable=result['p95_ci_high']<=1.01 and max(result['per_run_medians'])<=1.01
    result['decision']='faster' if high<1 and stable else 'within_1_percent' if high<=1.01 and stable else 'unresolved'
    if result['median_ci_low']>1.01 or result['p95_ci_low']>1.01:
        result['decision']='slower'
    return result


def unresolved_challengers(directory, rows, metadata, spec):
    """The one predeclared retry also applies to challenger uncertainty."""
    if metadata.get('phase') != 'confirm':
        return set()
    complete={(r['family'],r['size'],r['variant']) for r in rows if r.get('complete')}
    raw=None;retry=set()
    for group in spec['families']:
        challenger=group.get('challenger')
        selected=group.get('selected',{}).get('x86_64')
        if not challenger or not selected:
            continue
        for size in group['sizes']:
            key=(group['name'],size)
            if (*key,selected) not in complete or (*key,challenger) not in complete:
                continue
            if raw is None:
                raw=samples(directory,'initial',metadata['runs'])
            if comparison(raw[*key,selected],raw[*key,challenger])['decision']=='unresolved':
                retry.add('/'.join(key))
    return retry


def report(directory, rows, metadata, spec):
    directory=Path(directory);groups=defaultdict(list);cache={};rankings=[];choices={}
    policies={(g['name'],s):g for g in spec['families'] for s in g['sizes']}
    for row in rows:
        if row.get('complete') and row.get('correctness') and not row.get('diagnostic'):
            groups[row['family'],row['size']].append(row)
    for (family,size),values in sorted(groups.items()):
        policy=policies[family,size]
        aliases=spec.get('implementation_aliases',{}).get(f'{family}/{size}',{})
        eligible=[v for v in values if v.get('allocations_ok') and v['variant'] not in aliases]
        baseline=next(v for v in values if v['variant']==policy['baseline'])
        ranked=sorted(eligible,key=lambda v:v['median_ns'])
        best=ranked[0]
        # Do not switch from an incumbent on an uncertain development result.
        proposed=best if best['median_ci_high']<1 and best['p95_ci_high']<=1.01 and max(best['per_run_medians'])<=1.01 else baseline
        selected=policy.get('selected',{}).get('x86_64') if metadata['phase']=='confirm' else proposed['variant']
        other=next((v['variant'] for v in ranked if v['variant']!=selected),None)
        challenger=policy.get('challenger',other) if metadata['phase']=='confirm' else other
        entry=dict(family=family,size=size,selected=selected,challenger=challenger,implementation_aliases=aliases,
                   ranking=[dict(variant=v['variant'],ns_per_term=v.get('ns_per_term'),median_ns=v['median_ns'],production_ratio=v['median_ratio']) for v in ranked])
        if challenger:
            selected_row=next(v for v in values if v['variant']==selected)
            round_name=selected_row['round']
            if round_name not in cache:
                cache[round_name]=samples(directory,round_name,metadata['runs'])
            raw=cache[round_name]
            entry['head_to_head']=comparison(raw[family,size,selected],raw[family,size,challenger])
        rankings.append(entry)
        choices[f'{family}/{size}']=dict(variant=selected,challenger=challenger)
    (directory/'rankings.json').write_text(json.dumps(rankings,indent=2)+'\n')
    lines=['# Candidate rankings', '', 'Exploration rankings are selection evidence, not independent confirmation.', '',
           '| Operation | Size | Selected | Challenger | Head-to-head |', '|---|---|---|---|---|']
    for r in rankings:
        stats=r.get('head_to_head')
        result=f"{stats['median_ratio']:.4f} [{stats['median_ci_low']:.4f}, {stats['median_ci_high']:.4f}] {stats['decision']}" if stats else 'single implementation'
        lines.append(f"| {r['family']} | {r['size']} | {r['selected']} | {r['challenger'] or '—'} | {result} |")
    (directory/'rankings.md').write_text('\n'.join(lines)+'\n')
    if metadata['phase']=='explore':
        used=set()
        for round_name in ['initial','retry']:
            path=directory/round_name/'metadata.json'
            if path.exists():
                round_meta=json.loads(path.read_text())
                used.update(round_meta['seed_offset']+7919*r for r in range(round_meta['runs']))
        selection=dict(schema=1,source_sha256=metadata['source_sha256'],host_model=metadata['host']['model'],
                       cpu_set=metadata['cpu_set'],threads=metadata['threads'],flags=metadata['flags'],used_seeds=sorted(used),choices=choices)
        (directory/'selection.json').write_text(json.dumps(selection,indent=2)+'\n')

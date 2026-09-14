"""Freeze one integer kernel per public shape, across every input fixture."""
import copy
import hashlib
import json
import sys
from pathlib import Path

source, out = map(Path, sys.argv[1:3])
assert json.loads((source / 'campaign.json').read_text())['status'] == 'complete'
out.mkdir(exist_ok=False)
changes = []
for directory in sorted(source.iterdir()):
    if not (directory / 'selection.json').exists():
        continue
    selection = json.loads((directory / 'selection.json').read_text())
    rows = json.loads((directory / 'summary.json').read_text())
    ranks = {(r['family'], r['size']): r for r in json.loads((directory / 'rankings.json').read_text())}
    cases = {}
    for row in rows:
        if row['family'] == 'integer_full_mac':
            shape = row['size'].split('_', 1)[1]
            cases.setdefault(shape, {}).setdefault(row['size'], {})[row['variant']] = row
    for shape, fixtures in sorted(cases.items()):
        assert len(fixtures) == 2 and {s.split('_', 1)[0] for s in fixtures} == {'full', 'carry'}
        def eligible(row):
            return (row['complete'] and row['correctness'] and row['allocations_ok']
                    and row['median_ci_high'] < 1 and row['p95_ci_high'] <= 1.01
                    and max(row['per_run_medians']) <= 1.01)
        names = set.intersection(*(set(v) for v in fixtures.values()))
        names &= set.intersection(*({r['variant'] for r in ranks['integer_full_mac', size]['ranking']} for size in fixtures))
        candidates = [name for name in names if all(eligible(v[name]) for v in fixtures.values())]
        # Minimize the worst fixture ratio; stable lexical order breaks exact ties.
        chosen = min(candidates, key=lambda name: (max(v[name]['median_ratio'] for v in fixtures.values()), name)) if candidates else 'circuit_z'
        for size in fixtures:
            key = 'integer_full_mac/' + size
            previous = copy.deepcopy(selection['choices'][key])
            challenger = next((r['variant'] for r in ranks['integer_full_mac', size]['ranking'] if r['variant'] != chosen), None)
            selection['choices'][key] = dict(variant=chosen, challenger=challenger)
            if previous != selection['choices'][key]:
                changes.append(dict(placement=directory.name, case=key, previous=previous, frozen=selection['choices'][key]))
    selection['public_shape_policy'] = 'One integer kernel per limb width and length; it must pass development checks on both full-width random and carry-heavy fixtures. Minimize their worst median ratio, or retain production.'
    selection['development_summary_sha256'] = hashlib.sha256((directory / 'summary.json').read_bytes()).hexdigest()
    dest = out / directory.name
    dest.mkdir()
    (dest / 'selection.json').write_text(json.dumps(selection, indent=2) + '\n')
(out / 'plan.json').write_text(json.dumps(dict(exploration=str(source), changes=changes), indent=2) + '\n')
print(json.dumps(changes, indent=2))

#!/usr/bin/env python3
"""Aggregate nested spans within each trial, excluding the warm-up."""
from collections import defaultdict
import csv
import json
from pathlib import Path
from statistics import median

ROOT = Path(__file__).resolve().parent
summaries = []
flat = []
for kind in ('arithmetic', 'full'):
    for degree in (512, 1024):
        stem = f'{kind}-{degree}'
        events = [json.loads(line) for line in (ROOT / (stem + '.stages.jsonl')).read_text().splitlines()
                  if line.startswith('{')]
        events = [r for r in events if r.get('event') == 'stage']
        root_name = 'falcon_algebraic:prove' if kind == 'arithmetic' else 'falcon_bench:prove'
        roots = [r for r in events if r['name'] == root_name]
        assert len(roots) == 4
        samples = []
        path_samples = defaultdict(list)
        for root in roots[1:]:
            selected = [r for r in events if r['ancestor_ids'][0] == root['span_id']]
            grouped = defaultdict(float)
            for r in selected:
                grouped['/'.join(r['path'])] += r['elapsed_ms']
            for path, duration in grouped.items():
                path_samples[path].append(duration)
            binding_name = 'falcon_algebraic:binding' if kind == 'arithmetic' else 'falcon_arithmetic:binding_inner'
            binding = sum(r['elapsed_ms'] for r in selected if r['name'] == binding_name)
            grinding = sum(r['elapsed_ms'] for r in selected if r['name'] == 'spartan:grinding')
            grinding_names = {'spartan:grinding', 'lig:grind_pow'}
            all_grinding = sum(r['elapsed_ms'] for r in selected
                               if r['name'] in grinding_names
                               and not grinding_names.intersection(r['path'][:-1]))
            binding_grinding = sum(r['elapsed_ms'] for r in selected
                                   if r['name'] == 'spartan:grinding' and binding_name in r['path'])
            samples.append({'prover_ms': root['elapsed_ms'], 'binding_ms': binding,
                            'binding_pct_of_prover': 100 * binding / root['elapsed_ms'],
                            'spartan_grinding_ms': grinding,
                            'all_recorded_grinding_ms': all_grinding,
                            'binding_recorded_grinding_ms': binding_grinding})
        result = {'variant': kind, 'degree': degree, 'samples': samples}
        result['medians'] = {key: median(s[key] for s in samples) for key in samples[0]}
        result['path_medians_ms'] = {path: median(values) for path, values in path_samples.items()}
        summaries.append(result)
        for path, values in path_samples.items():
            flat.append({'variant': kind, 'degree': degree, 'path': path,
                         'median_ms': median(values), 'min_ms': min(values), 'max_ms': max(values),
                         'measured_trials_containing_path': len(values)})
        print(stem, json.dumps(result['medians']))
        for path, duration in sorted(result['path_medians_ms'].items(), key=lambda pair: -pair[1])[:22]:
            print(f'{duration:9.3f} ms  {path}')
(ROOT / 'analysis.json').write_text(json.dumps(summaries, indent=2) + '\n')
with (ROOT / 'stage-medians.csv').open('w') as file:
    writer = csv.DictWriter(file, fieldnames=list(flat[0]))
    writer.writeheader()
    writer.writerows(flat)

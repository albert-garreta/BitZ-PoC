#!/usr/bin/env python3
"""Check batch-1024 paired improvements and independently repeated regressions."""
import argparse
import json
from pathlib import Path


def read_campaign(path):
    manifest = json.loads((path / 'manifest.json').read_text())
    if not manifest['complete']:
        raise ValueError(f'incomplete campaign: {path}')
    return {tuple(row['cell']): row for row in json.loads((path / 'summary.json').read_text())}


def qualified(row, repeat=None):
    if not row['primary_improves']:
        return False
    if not row['apparent_regressions']:
        return True
    if repeat is None:
        return False
    # An apparent secondary regression must fail independently to be confirmed.
    # A primary regression in the repeat also blocks the range.
    if repeat['metrics']['total_prover_ms']['status'] == 'regression':
        return False
    return not set(row['apparent_regressions']) & set(repeat['apparent_regressions'])


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('campaign', type=Path)
    parser.add_argument('--repeat', type=Path)
    parser.add_argument('--out', type=Path, required=True)
    args = parser.parse_args()
    rows = read_campaign(args.campaign)
    repeat = read_campaign(args.repeat) if args.repeat else {}
    coverage = {}
    for degree, batch, workers, security in rows:
        if batch != 1024 or workers not in (1, 2, 4, 8):
            raise ValueError('qualification is restricted to batch 1024 and 1/2/4/8 workers')
        coverage.setdefault((degree, batch, workers), set()).add(security)
    if any(targets != {100, 128} for targets in coverage.values()):
        raise ValueError('qualification requires both security targets in every cell')
    passed = []
    unresolved, regressions = [], []
    for cell, row in rows.items():
        if qualified(row, repeat.get(cell)):
            passed.append(cell)
        if row['apparent_regressions'] and cell not in repeat:
            unresolved.append(cell)
        if cell in repeat and set(row['apparent_regressions']) & set(repeat[cell]['apparent_regressions']):
            regressions.append(cell)
    output = {'campaign': str(args.campaign), 'qualified_cells': passed,
        'all_qualified': len(passed) == len(rows),
        'needs_independent_repeat': unresolved, 'confirmed_regressions': regressions}
    with args.out.open('x') as stream:
        json.dump(output, stream, indent=2)
        stream.write('\n')
    print(json.dumps(output, indent=2))


if __name__ == '__main__':
    main()

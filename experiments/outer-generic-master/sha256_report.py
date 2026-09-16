#!/usr/bin/env python3
"""Compare verified official SHA-256/P-256 campaigns without averaging cases."""
import argparse
import csv
import json
from pathlib import Path

KEYS = ['method', 'log_compressions', 'security_target', 'threads', 'seed',
        'ligerito_profile', 'log_inv_rate']
METRICS = ['witness_ms', 'commit_ms', 'piop_ms', 'iop_ms', 'outer_ms', 'inner_ms',
           'protocol_ms', 'prove_ms', 'witness_to_proof_ms', 'e2e_prover_ms',
           'verify_ms', 'peak_rss_bytes', 'proof_object_bytes', 'proof_material_bytes']
p = argparse.ArgumentParser(description=__doc__)
p.add_argument('master', type=Path)
p.add_argument('current', type=Path)
p.add_argument('output', type=Path)
a = p.parse_args()
a.output.mkdir(parents=True, exist_ok=False)


def load(directory):
    rows = list(csv.DictReader((directory/'summary.csv').open()))
    assert len(rows) == 48, (directory, len(rows))
    assert all(r['status'] == 'complete' and int(r['samples']) == 5 for r in rows)
    result = {tuple(r[k] for k in KEYS): r for r in rows}
    assert len(result) == len(rows)
    return result


master, current = load(a.master), load(a.current)
assert master.keys() == current.keys()
records = []
security_changes = []
for key, before in master.items():
    after = current[key]
    assert before['fixture_id'] == after['fixture_id'], key
    for metric in ['security_model', 'economic_bits', 'statistical_bits_lower_bound']:
        if before[metric] != after[metric]:
            security_changes.append(dict(zip(KEYS, key), metric=metric,
                                         master=before[metric], current=after[metric]))
    for metric in METRICS:
        if before[metric] == '' or after[metric] == '':
            assert before[metric] == after[metric], (key, metric)
            continue
        b, c = float(before[metric]), float(after[metric])
        delta = 100*(c/b-1) if b else None
        records.append(dict(zip(KEYS, key), metric=metric, master=b, current=c,
                            change_percent=delta, above_five_percent=delta is not None and delta > 5))
with (a.output/'comparison.csv').open('w') as stream:
    writer = csv.DictWriter(stream, fieldnames=list(records[0]))
    writer.writeheader()
    writer.writerows(records)
(a.output/'comparison.json').write_text(json.dumps(records, indent=2)+'\n')
(a.output/'security-comparison.json').write_text(json.dumps(security_changes, indent=2)+'\n')


def pair(before, after, metric, divisor=1):
    if before[metric] == '':
        return '—'
    b, c = float(before[metric]), float(after[metric])
    delta = f'{100*(c/b-1):+.1f}%' if b else 'n/a'
    return f'{b/divisor:.3f} → {c/divisor:.3f} ({delta})'


lines = ['# SHA-256 chain + P-256 verification: master versus generic outer', '',
         'Each cell shows **master → current (change)**. Times are milliseconds,',
         'memory is MiB, and proof material is KiB. Both campaigns use one excluded',
         'warmup and five measured repetitions; reported times are medians.', '',
         'PIOP = protocol minus opening, derived per sample before aggregation.',
         'It includes non-opening preparation, matrix work, folding, sumchecks and',
         'transcript work. PCS/IOP is the complete timed opening stage.',
         'Prove = commitment + protocol; fresh-input prover also includes witness',
         'generation. Peak memory is the whole worker maximum, including setup',
         'and verification, not an independently measured peak for each proof.', '',
         'The 5% flag describes observed medians, not statistical significance.',
         'No configurations are averaged together. Missing backend phases are',
         'shown as dashes. All measured proofs were decoded and verified.', '']
lines += [('Security accounting labels match in every paired configuration.' if not security_changes
           else 'Security accounting differs in some pairs; see `security-comparison.json`.'), '']
for method in ['f2z-split', 'binius64', 'binius64-ligerito']:
    lines += [f'## {method}', '',
              '| SHA exponent | Rate/profile | Threads | Prove ms | Fresh-input prover ms | Verify ms | Peak MiB | Proof KiB |',
              '|---:|---|---:|---|---|---|---|---|']
    keys = sorted((k for k in master if k[0] == method), key=lambda k: (int(k[3]), k[5], k[6], int(k[1])))
    for key in keys:
        b, c = master[key], current[key]
        columns = [key[1], key[5] or f'1/{1 << int(key[6])}', key[3],
                   pair(b,c,'prove_ms'), pair(b,c,'e2e_prover_ms'), pair(b,c,'verify_ms'),
                   pair(b,c,'peak_rss_bytes', 1024**2), pair(b,c,'proof_material_bytes', 1024)]
        lines.append('| '+' | '.join(columns)+' |')
    lines += ['', '### Prover breakdown', '',
              '| SHA exponent | Rate/profile | Threads | Witness ms | Commit ms | PIOP ms | PCS/IOP ms | Outer ms | Inner ms |',
              '|---:|---|---:|---|---|---|---|---|---|']
    for key in keys:
        b, c = master[key], current[key]
        columns = [key[1], key[5] or f'1/{1 << int(key[6])}', key[3]]
        columns += [pair(b,c,m) for m in ['witness_ms','commit_ms','piop_ms','iop_ms','outer_ms','inner_ms']]
        lines.append('| '+' | '.join(columns)+' |')
    lines.append('')
lines += ['## Observed increases above 5%', '',
          '| Method | Exponent | Profile/rate | Threads | Metric | Change |',
          '|---|---:|---|---:|---|---:|']
for r in records:
    if r['above_five_percent']:
        lines.append(f"| {r['method']} | {r['log_compressions']} | {r['ligerito_profile'] or r['log_inv_rate']} | {r['threads']} | {r['metric']} | {r['change_percent']:+.1f}% |")
(a.output/'REPORT.md').write_text('\n'.join(lines)+'\n')
print(a.output/'REPORT.md')

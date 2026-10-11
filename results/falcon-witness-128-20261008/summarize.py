#!/usr/bin/env python3
"""Validate and summarize the requested eight-cell Falcon-128 comparison.

Run only after all four campaigns have finished. Input files are read from the
campaign directories (or their byte-preserving raw-runs.tar.gz archives).
"""
from __future__ import annotations

import argparse
import hashlib
import json
import math
from pathlib import Path
import statistics
import sys
import tarfile


IDENTITY = ('input_digest', 'source_root', 'proof_debug_digest',
            'proof_payload_bytes', 'proof_payload_breakdown', 'capacity')
STAGES = {'arithmetic': ('falcon_algebraic:witness',),
          'full': ('falcon_hybrid:shake_witness', 'falcon_hybrid:arithmetic_witness')}


def require(condition, reason):
    if not condition:
        raise ValueError(reason)


def read_raw(directory, name):
    path = directory / name
    if path.is_file():
        return path.read_text()
    archive_path = directory / 'raw-runs.tar.gz'
    with tarfile.open(archive_path, 'r:gz') as archive:
        matches = [member for member in archive.getmembers()
                   if member.isfile() and Path(member.name).name == name]
        require(len(matches) == 1, f'{archive_path}: missing or ambiguous {name}')
        return archive.extractfile(matches[0]).read().decode()


def json_lines(text):
    return [json.loads(line) for line in text.splitlines() if line.startswith('{')]


def witness_samples(stderr, mode, warmup, samples):
    names = STAGES[mode]
    stages = [row for row in json_lines(stderr)
              if row.get('event') == 'stage' and row.get('name') in names]
    trials = warmup + samples
    require([row['name'] for row in stages] == list(names) * trials,
            f'{mode}: missing, duplicated or reordered witness stage')
    values = []
    components = {name: [] for name in names}
    for trial in range(trials):
        group = stages[trial * len(names):(trial + 1) * len(names)]
        if mode == 'full':
            # The two scopes are consecutive children of the same per-trial
            # witness_commit span; this prevents pairing spans across trials.
            parents = [tuple(row['ancestor_ids'][:-1]) for row in group]
            require(parents[0] and parents[0] == parents[1],
                    'SHAKE and arithmetic stages belong to different trials')
        elapsed = [row['elapsed_ms'] for row in group]
        require(all(math.isfinite(value) and value > 0 for value in elapsed),
                'invalid witness stage duration')
        for name, value in zip(names, elapsed):
            components[name].append(value)
        # Sum the disjoint stages for each trial BEFORE computing any median.
        values.append(sum(elapsed))
    return values[warmup:], {name: values[warmup:] for name, values in components.items()}


def paired_metric(records, name, paired_interval, sample_statistics):
    groups = {variant: sorted((row for row in records if row['variant'] == variant),
                              key=lambda row: row['block'])
              for variant in ('baseline', 'candidate')}
    blocks = [row['block'] for row in groups['baseline']]
    require(blocks == list(range(len(blocks))) and len(blocks) >= 2,
            'missing or duplicated baseline block')
    require([row['block'] for row in groups['candidate']] == blocks,
            'missing or duplicated candidate block')
    left = [row['metrics'][name] for row in groups['baseline']]
    right = [row['metrics'][name] for row in groups['candidate']]
    require(all(math.isfinite(value) and value > 0 for value in left + right),
            f'invalid process metric {name}')
    ratios = [b / a for a, b in zip(left, right)]
    interval = paired_interval(ratios)
    return dict(baseline=sample_statistics(left), candidate=sample_statistics(right),
                ratio=statistics.geometric_mean(ratios), interval95=interval,
                status=('improvement' if interval[1] < 1 else
                        'regression' if interval[0] > 1 else 'inconclusive'),
                paired_process_ratios=ratios)


def validate_campaign(root, mode, phase, identities):
    directory = root / f'{mode}-{phase}'
    manifest = json.loads((directory / 'manifest.json').read_text())
    configuration = manifest['configuration']
    diagnostic = phase == 'witness'
    blocks, warmup, samples = (2 if diagnostic else 3), 2, 3
    require(manifest.get('complete') is True, f'{directory}: campaign unfinished')
    require(configuration['mode'] == mode and configuration['diagnostic'] is diagnostic,
            f'{directory}: wrong mode or diagnostic policy')
    require((configuration['blocks'], configuration['warmup'], configuration['samples'])
            == (blocks, warmup, samples), f'{directory}: unexpected sampling schedule')
    require(configuration['batch'] == 1024 and configuration['security'] == 128
            and configuration['seed'] == 42, f'{directory}: unexpected workload')
    require(sorted(configuration['degrees']) == [512, 1024]
            and sorted(configuration['threads']) == [1, 8], f'{directory}: unexpected matrix')
    records = json_lines(read_raw(directory, 'runs.jsonl'))
    expected_cells = {(degree, 1024, threads, 128) for degree in (512, 1024) for threads in (1, 8)}
    expected = {(cell, variant, block) for cell in expected_cells
                for variant in ('baseline', 'candidate') for block in range(blocks)}
    observed = [(tuple(row['cell']), row['variant'], row['block']) for row in records]
    require(len(observed) == len(expected) and set(observed) == expected,
            f'{directory}: missing or duplicated process record')
    require(manifest.get('completed_processes') == len(records),
            f'{directory}: manifest process count differs')
    validated = []
    for record in records:
        degree, batch, threads, security = record['cell']
        variant, block = record['variant'], record['block']
        stem = f'n{degree}-b{batch}-t{threads}-s{security}-r{block:02}-{variant}'
        stdout = json_lines(read_raw(directory, stem + '.stdout'))
        trials = [row for row in stdout if row.get('trial') in ('warmup', 'sample')]
        prepared = [row for row in stdout if row.get('event') == 'prepared']
        require(len(prepared) <= 1, f'{stem}: duplicate prepared row')
        header = prepared[0] if prepared else {}
        require([row['trial'] for row in trials] == ['warmup'] * warmup + ['sample'] * samples,
                f'{stem}: wrong warmup/sample count or order')
        require(record['trials'] == trials, f'{stem}: raw stdout differs from stored trials')
        require(record['verified_proofs'] == len(trials), f'{stem}: wrong verified count')
        key = (mode, degree, batch, security, threads)
        identity = record['identity']
        require(all(name in identity for name in IDENTITY), f'{stem}: missing identity field')
        if key in identities:
            require(identities[key] == identity, f'{stem}: cross-campaign proof/input identity changed')
        else:
            identities[key] = identity
        for row in trials:
            require(row.get('verified') is True, f'{stem}: unverified proof')
            require((row['degree'], row['batch'], row['threads'],
                     row.get('security_bits', row.get('security_target')))
                    == (degree, batch, threads, security), f'{stem}: wrong observed configuration')
            require(row.get('seed', header.get('input_seed')) == 42, f'{stem}: wrong observed seed')
            require(row.get('build_rustflags', header.get('build_rustflags')) == '-C target-cpu=native',
                    f'{stem}: unexpected native build flags')
            require(all(row.get(name) == identity[name] for name in IDENTITY),
                    f'{stem}: identity changed within process')
        measured = trials[warmup:]
        metrics = {}
        for name, source in [('witness_commit_ms', 'witness_commit_ms'),
                             ('total_prover_ms', 'total_prover_ms'),
                             ('prove_ms', 'proof_prove_ms' if mode == 'full' else 'prove_ms'),
                             ('verify_ms', 'proof_verify_ms' if mode == 'full' else 'verify_ms')]:
            values = [row[source] for row in measured]
            require(all(math.isfinite(value) and value > 0 for value in values),
                    f'{stem}: invalid {name}')
            metrics[name] = statistics.median(values)
            require(metrics[name] == record['medians'][name], f'{stem}: stored median differs')
        metrics['peak_rss_bytes'] = record['medians']['peak_rss_bytes']
        witness, components = None, None
        if diagnostic:
            witness, components = witness_samples(read_raw(directory, stem + '.stderr'), mode, warmup, samples)
            metrics['witness_ms'] = statistics.median(witness)
            for name, values in components.items():
                metrics[name] = statistics.median(values)
        validated.append(dict(mode=mode, cell=record['cell'], variant=variant, block=block,
                              metrics=metrics, witness_samples_ms=witness,
                              witness_component_samples_ms=components))
    proofs = len(records) * (warmup + samples)
    require(manifest.get('verified_proofs') == proofs, f'{directory}: verified proof count differs')
    return validated, dict(campaign=directory.name, processes=len(records), verified_proofs=proofs,
                           blocks=blocks, warmup=warmup, measured_samples=samples,
                           manifest_sha256=hashlib.sha256((directory / 'manifest.json').read_bytes()).hexdigest())


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--repo', type=Path,
                        default=Path(__file__).resolve().parents[2])
    parser.add_argument('--root', type=Path)
    args = parser.parse_args()
    root = args.root or args.repo / 'results/falcon-witness-128-20261008'
    sys.path.insert(0, str(args.repo / 'scripts'))
    from bench_statistics import paired_interval, sample_statistics

    identities, campaigns, records = {}, [], {}
    for mode in ('arithmetic', 'full'):
        for phase in ('e2e', 'witness'):
            records[mode, phase], manifest = validate_campaign(root, mode, phase, identities)
            campaigns.append(manifest)
    rows = []
    for mode in ('arithmetic', 'full'):
        for degree in (512, 1024):
            for threads in (1, 8):
                cell = [degree, 1024, threads, 128]
                e2e = [row for row in records[mode, 'e2e'] if row['cell'] == cell]
                diagnostic = [row for row in records[mode, 'witness'] if row['cell'] == cell]
                metrics = {name: paired_metric(e2e, name, paired_interval, sample_statistics)
                           for name in ('witness_commit_ms', 'total_prover_ms', 'prove_ms',
                                        'verify_ms', 'peak_rss_bytes')}
                metrics['witness_ms'] = paired_metric(diagnostic, 'witness_ms', paired_interval, sample_statistics)
                components = {name: paired_metric(diagnostic, name, paired_interval, sample_statistics)
                              for name in STAGES[mode]}
                rows.append(dict(mode=mode, degree=degree, threads=threads, batch=1024,
                                 security=128, metrics=metrics, witness_components=components))
    result = dict(schema='bitz/falcon-witness-128-comparison/v1',
                  all_campaigns_complete=True, cross_campaign_identities_match=True,
                  expected_cells=8, verified_proofs=sum(row['verified_proofs'] for row in campaigns),
                  processes=sum(row['processes'] for row in campaigns), campaigns=campaigns, rows=rows,
                  definitions=dict(witness='Instrumented generation; full sums SHAKE plus arithmetic per trial before medians.',
                                   absolute='Median of process medians.',
                                   ratio='Geometric mean of paired candidate/baseline process-median ratios.',
                                   interval='Descriptive paired bootstrap 95% interval; only 2 diagnostic or 3 e2e pairs.'),
                  derived_diagnostic_processes=[row for mode in ('arithmetic', 'full')
                                                for row in records[mode, 'witness']],
                  identities=[dict(mode=key[0], degree=key[1], batch=key[2], security=key[3], threads=key[4],
                                   identity=value) for key, value in sorted(identities.items())])
    require(result['verified_proofs'] == 400 and result['processes'] == 80,
            'unexpected complete campaign total')
    lines = [
        '# Falcon witness generation at security 128', '',
        'Exact ac2b42fea baseline versus the simplified NTT implementation at 5f2149a8f. '
        'Batch 1024, security target 128, seed 42, both Falcon degrees, one and eight threads, '
        'native CPU builds on Linux with an AMD Ryzen 9 9950X3D. These are not MacBook measurements.', '',
        'Generation is measured in separate instrumented runs. Arithmetic generation includes '
        'checked polynomial reconstruction, centered S1, integer norms, slack and quotient values. '
        'Full generation sums the disjoint SHAKE and arithmetic scopes **for each trial before taking medians**. '
        'SHAKE generation includes its fused packed-witness construction and auxiliary values; arithmetic generation '
        'includes HashToPoint and the native ring/norm trace. Both exclude the later arithmetic-source packing '
        'and PCS commitment. The diagnostic baseline adds only matching SHAKE/arithmetic timing spans.', '',
        'Witness + commit and total prover times below come from separate uninstrumented runs. '
        'Absolute values are medians of process medians. Percentage changes are geometric means of paired '
        'candidate/baseline ratios and cannot be calculated directly from those displayed medians.', '',
        '| Workload | Threads | Generation, ms (before → after) | Generation change | Witness + commit, ms (before → after) | Total prover, ms (before → after) |',
        '|---|---:|---:|---:|---:|---:|',
    ]
    def before_after(metric):
        return f"{metric['baseline']['median']:.3f} → {metric['candidate']['median']:.3f}"
    for row in rows:
        metrics = row['metrics']
        lines.append(f"| {row['mode'].capitalize()} {row['degree']} | {row['threads']} | "
                     f"{before_after(metrics['witness_ms'])} | {(metrics['witness_ms']['ratio'] - 1) * 100:+.2f}% | "
                     f"{before_after(metrics['witness_commit_ms'])} | {before_after(metrics['total_prover_ms'])} |")
    lines += ['', 'Each uninstrumented cell has three alternating paired process blocks; each diagnostic '
              'cell has two. Every process has two warm-ups followed by three measured trials. '
              'These short runs are a focused comparison, not a broad performance qualification. '
              'Detailed paired ratios and descriptive bootstrap intervals, including prove, verify, '
              'RSS and the separate full-witness components, are retained in `summary.json`.', '',
              '**Validation:** all four campaigns completed, 80 processes and 400 verified proofs. '
              'Input digest, source root, complete-proof debug digest, payload size/breakdown and capacity '
              'match across variants and campaigns for every workload/thread configuration. '
              'The debug digest is an exact-build comparison aid, not a stable proof wire encoding.', '']
    (root / 'summary.json').write_text(json.dumps(result, indent=2) + '\n')
    (root / 'README.md').write_text('\n'.join(lines))
    print(json.dumps(dict(summary=str(root / 'summary.json'), report=str(root / 'README.md'),
                          processes=result['processes'], verified_proofs=result['verified_proofs'])))


if __name__ == '__main__':
    main()

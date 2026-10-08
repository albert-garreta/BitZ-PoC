#!/usr/bin/env python3
"""Validate completed Falcon commitment experiments and report paired evidence.

Run from anywhere; defaults to this results directory and its repository. Each
experiment owns e2e/stages subdirectories, with optional additional campaigns
such as e2e-extension. Completed campaigns under the same experiment are pooled
only when their binaries and variant environments match. Incomplete campaigns
are listed and excluded. Raw files may instead reside in raw-runs.tar.gz.
"""
from __future__ import annotations

import argparse
from collections import defaultdict
import hashlib
import json
import math
from pathlib import Path
import re
import statistics
import sys
import tarfile


IDENTITY = ('input_digest', 'source_root', 'proof_debug_digest',
            'proof_payload_bytes', 'proof_payload_breakdown', 'capacity')
HEADER_IDENTITY = ('protocol', 'source_layout', 'ring_extension', 'ring_extension_selection',
                   'max_batch', 'source_bits_per_signature', 'pcs_query_shape',
                   'integer_bridge', 'target_arch', 'gf128_kernel',
                   'compiled_target_features', 'build_rustflags', 'runtime_rustflags', 'cpu_affinity', 'input_source',
                   'input_implementation_version', 'input_rng')
ARITHMETIC_IDENTITY = ('protocol_id', 'relation', 'extension_degree', 'layout_version',
                       'source_layout', 'source_bits_per_signature', 'coefficient_stride',
                       'source_bits', 'source_packed_bytes', 'live_bits_per_signature',
                       'build_rustflags', 'cpu_affinity', 'input_corpus',
                       'auxiliary_pool_threads', 'verification_thread_policy')
STAGES = {
    'shake_ms': 'falcon_hybrid:shake_witness',
    'arithmetic_ms': 'falcon_hybrid:arithmetic_witness',
    'source_construction_ms': 'falcon_hybrid:source_construction',
    'source_packing_ms': 'falcon_hybrid:source_packing',
    'encode_ms': 'shared_source:encode',
    'merkle_ms': 'shared_source:merkle',
}
TIMINGS = ('witness_commit_ms', 'total_prover_ms', 'prove_ms', 'verify_ms')


def require(condition, message):
    if not condition:
        raise ValueError(message)


def rows(text):
    return [json.loads(line) for line in text.splitlines() if line.startswith('{')]


def digest(data):
    return hashlib.sha256(data).hexdigest()


class RawFiles:
    def __init__(self, directory):
        self.directory = directory
        self.archived = {}
        archive = directory / 'raw-runs.tar.gz'
        if archive.exists():
            with tarfile.open(archive, 'r:gz') as stream:
                for member in stream:
                    if member.isfile():
                        name = Path(member.name).name
                        require(name not in self.archived, f'{archive}: duplicate member {name}')
                        self.archived[name] = stream.extractfile(member).read()

    def read(self, name):
        path = self.directory / name
        data = path.read_bytes() if path.exists() else self.archived.get(name)
        require(data is not None, f'{self.directory}: missing {name}')
        if path.exists() and name in self.archived:
            require(data == self.archived[name], f'{path}: archive differs from loose file')
        return data.decode()


def stage_samples(stderr, warmup, samples):
    stages = [row for row in rows(stderr) if row.get('event') == 'stage']
    commits = [row for row in stages if row.get('name') == 'falcon_bench:commit']
    require([row['fields'].get('trial') for row in commits] == list(range(warmup + samples)),
            'missing, duplicated or reordered commit trial spans')
    selected = {**STAGES, 'commit_body_ms': 'falcon_hybrid:witness_commit'}
    for name in selected.values():
        require(sum(row.get('name') == name for row in stages) == len(commits),
                f'missing or duplicated stage {name}')
    values = defaultdict(list)
    for commit in commits:
        commit_id = commit['span_id']
        require(commit['ancestor_ids'] == [commit_id], 'commit span is not a root')
        group = {}
        for key, name in selected.items():
            matches = [row for row in stages if row.get('name') == name
                       and row.get('ancestor_ids', [None])[0] == commit_id]
            require(len(matches) == 1, f'trial {commit["fields"]}: ambiguous {name}')
            group[key] = matches[0]
        body_id = group['commit_body_ms']['span_id']
        require(group['commit_body_ms']['ancestor_ids'] == [commit_id, body_id],
                'witness commit span has unexpected parent')
        for key in STAGES:
            row = group[key]
            require(row['ancestor_ids'] == [commit_id, body_id, row['span_id']],
                    f'{row["name"]}: expected disjoint direct child of witness commit')
        elapsed = {key: row['elapsed_ms'] for key, row in group.items()}
        elapsed['commit_wall_ms'] = commit['elapsed_ms']
        require(all(math.isfinite(value) and value > 0 for value in elapsed.values()),
                'nonpositive or nonfinite stage time')
        elapsed['generation_ms'] = elapsed['shake_ms'] + elapsed['arithmetic_ms']
        elapsed['source_preparation_ms'] = elapsed['source_construction_ms'] + elapsed['source_packing_ms']
        elapsed['pcs_ms'] = elapsed['encode_ms'] + elapsed['merkle_ms']
        elapsed['accounted_ms'] = sum(elapsed[key] for key in STAGES)
        elapsed['commit_other_ms'] = elapsed['commit_body_ms'] - elapsed['accounted_ms']
        elapsed['outside_commit_body_ms'] = elapsed['commit_wall_ms'] - elapsed['commit_body_ms']
        require(elapsed['commit_other_ms'] >= 0 and elapsed['outside_commit_body_ms'] >= 0,
                'child durations exceed parent; scopes overlap or timing records are invalid')
        for key, value in elapsed.items():
            values[key].append(value)
    return {key: value[warmup:] for key, value in values.items()}


def arithmetic_stage_samples(stderr, warmup, samples):
    stages = [row for row in rows(stderr) if row.get('event') == 'stage']
    commits = [row for row in stages if row.get('name') == 'falcon_algebraic:commit']
    witnesses = [row for row in stages if row.get('name') == 'falcon_algebraic:witness']
    require(len(commits) == len(witnesses) == warmup + samples,
            'missing or duplicated arithmetic commit/witness stages')
    values = []
    for commit, witness in zip(commits, witnesses):
        commit_id = commit['span_id']
        require(commit['ancestor_ids'] == [commit_id] and
                witness['ancestor_ids'] == [commit_id, witness['span_id']],
                'arithmetic witness does not match its ordered commit trial')
        value = witness['elapsed_ms']
        require(math.isfinite(value) and 0 < value <= commit['elapsed_ms'],
                'invalid arithmetic witness duration')
        values.append(value)
    return {'generation_ms': values[warmup:]}


def validate_campaign(directory, manifest, identities, headers, seen_processes):
    config = manifest['configuration']
    mode = config['mode']
    require(mode in ('full', 'arithmetic') and config['batch'] == 1024 and config['security'] == 128
            and config['seed'] == 42, f'{directory}: unexpected workload')
    require(config['warmup'] == 2 and config['samples'] == 3 and config['blocks'] >= 2,
            f'{directory}: unexpected sampling schedule')
    diagnostic = config['diagnostic']
    require(type(diagnostic) is bool, f'{directory}: missing diagnostic policy')
    cells = [(degree, 1024, threads, 128) for degree in config['degrees'] for threads in config['threads']]
    require(len(cells) == len(set(cells)) and cells
            and all(degree in (512, 1024) and threads in (1, 8) for degree, _, threads, _ in cells),
            f'{directory}: invalid cell matrix')
    require(sorted(map(tuple, manifest['cells'])) == sorted(cells), f'{directory}: manifest cell list differs')
    raw = RawFiles(directory)
    records = rows(raw.read('runs.jsonl'))
    expected = {(cell, variant, block) for cell in cells for variant in ('baseline', 'candidate')
                for block in range(config['blocks'])}
    observed = [(tuple(row['cell']), row['variant'], row['block']) for row in records]
    require(len(observed) == len(expected) and set(observed) == expected,
            f'{directory}: incomplete or duplicate process records')
    require(manifest['completed_processes'] == len(records), f'{directory}: wrong process count')
    validated = []
    for record in records:
        degree, batch, threads, security = cell = tuple(record['cell'])
        variant, block = record['variant'], record['block']
        stem = f'n{degree}-b{batch}-t{threads}-s{security}-r{block:02}-{variant}'
        stdout, stderr = raw.read(stem + '.stdout'), raw.read(stem + '.stderr')
        stdout_hash = digest(stdout.encode())
        require(stdout_hash not in seen_processes, f'{directory}/{stem}: duplicate raw observation')
        seen_processes.add(stdout_hash)
        output = rows(stdout)
        prepared = [row for row in output if row.get('event') == 'prepared']
        require(len(prepared) == (1 if mode == 'full' else 0), f'{stem}: unexpected prepared header count')
        header = prepared[0] if prepared else {}
        require(record.get('prepared') == header, f'{stem}: stored header differs')
        trials = [row for row in output if row.get('trial') in ('warmup', 'sample')]
        require([row['trial'] for row in trials] == ['warmup'] * 2 + ['sample'] * 3,
                f'{stem}: incorrect warmup/sample order')
        require(record['trials'] == trials and record['verified_proofs'] == 5,
                f'{stem}: stored trials/count differs')
        if mode == 'full':
            require(header['input_seed'] == 42 and header['stage_timings'] == diagnostic
                    and header['warmup_trials'] == 2 and header['measured_trials'] == 3
                    and header['build_rustflags'] == header['runtime_rustflags'] == '-C target-cpu=native',
                    f'{stem}: wrong header configuration')
        identity = {name: record['identity'][name] for name in IDENTITY}
        identity_key = (mode, *cell)
        if identity_key in identities:
            require(identities[identity_key] == identity, f'{directory}/{stem}: cross-campaign identity differs')
        identities[identity_key] = identity
        header_identity = ({name: header[name] for name in HEADER_IDENTITY} if mode == 'full'
                           else {name: trials[0][name] for name in ARITHMETIC_IDENTITY})
        if identity_key in headers:
            require(headers[identity_key] == header_identity, f'{directory}/{stem}: protocol/build shape differs')
        headers[identity_key] = header_identity
        timing_sources = {name: 'proof_' + name if mode == 'full' and name in ('prove_ms', 'verify_ms') else name
                          for name in TIMINGS}
        for trial in trials:
            require(trial.get('verified') is True and
                    (trial['degree'], trial['batch'], trial['threads'],
                     trial.get('security_target', trial.get('security_bits'))) == cell,
                    f'{stem}: unverified proof or wrong configuration')
            if mode == 'arithmetic':
                require(trial['seed'] == 42 and trial['build_rustflags'] == '-C target-cpu=native'
                        and trial['auxiliary_pool_threads'] == threads
                        and all(trial[name] == value for name, value in header_identity.items()),
                        f'{stem}: arithmetic configuration differs')
            require(all(trial.get(key) == value for key, value in identity.items()),
                    f'{stem}: repeated identity differs')
            phase_sum = trial['witness_commit_ms'] + trial[timing_sources['prove_ms']]
            if mode == 'full':
                require(math.isclose(trial['total_prover_ms'], phase_sum, rel_tol=1e-12),
                        f'{stem}: total does not equal commit plus prove')
            else:
                # Arithmetic uses an enclosing wall timer, including timer overhead.
                require(trial['total_prover_ms'] + 1e-9 >= phase_sum,
                        f'{stem}: arithmetic total is shorter than its timed phases')
        metrics = {}
        for key, source in timing_sources.items():
            samples = [trial[source] for trial in trials[2:]]
            require(all(math.isfinite(value) and value > 0 for value in samples), f'{stem}: invalid {source}')
            metrics[key] = statistics.median(samples)
            require(metrics[key] == record['medians'][key], f'{stem}: stored median differs')
        match = re.search(r'Maximum resident set size \(kbytes\):\s*(\d+)', stderr)
        require(match is not None, f'{stem}: missing Linux process RSS')
        metrics['peak_rss_bytes'] = int(match[1]) * 1024
        require(metrics['peak_rss_bytes'] > 0 and metrics['peak_rss_bytes'] == record['medians']['peak_rss_bytes'],
                f'{stem}: wrong RSS median')
        stage_parser = stage_samples if mode == 'full' else arithmetic_stage_samples
        stage_values = stage_parser(stderr, 2, 3) if diagnostic else {}
        metrics.update({key: statistics.median(values) for key, values in stage_values.items()})
        validated.append(dict(mode=mode, cell=list(cell), variant=variant, block=block, metrics=metrics,
                              stage_samples_ms=stage_values, stdout_sha256=stdout_hash))
    require(manifest['verified_proofs'] == len(records) * 5, f'{directory}: wrong verified proof count')
    return validated


def paired_metric(records, name, paired_interval, sample_statistics):
    groups = [sorted((row for row in records if row['variant'] == variant), key=lambda row: row['pair'])
              for variant in ('baseline', 'candidate')]
    require([row['pair'] for row in groups[0]] == [row['pair'] for row in groups[1]], 'unpaired records')
    left, right = ([row['metrics'][name] for row in group] for group in groups)
    result = dict(baseline=sample_statistics(left), candidate=sample_statistics(right), pairs=len(left))
    if any(value == 0 for value in left + right):
        return dict(result, ratio=None, interval95=None, status='zero_diagnostic_duration')
    ratios = [b / a for a, b in zip(left, right)]
    interval = paired_interval(ratios)
    return dict(result, ratio=statistics.geometric_mean(ratios), interval95=interval,
                paired_process_ratios=ratios, pair_ids=[row['pair'] for row in groups[0]],
                status='improvement' if interval[1] < 1 else 'regression' if interval[0] > 1 else 'inconclusive')


def selection(metrics):
    commit, total = metrics['witness_commit_ms'], metrics['total_prover_ms']
    gates = {name: ('pass' if metrics[name]['interval95'][1] <= 1.01 else
                    'regression' if metrics[name]['interval95'][0] > 1.01 else 'inconclusive')
             for name in TIMINGS}
    return dict(commit_improves_95=commit['interval95'][1] < 1,
                total_nonregression_demonstrated_95=total['interval95'][1] <= 1,
                total_within_one_percent_95=total['interval95'][1] <= 1.01,
                latency_one_percent_gates=gates,
                all_latencies_within_one_percent_95=all(value == 'pass' for value in gates.values()),
                total_significant_regression=total['interval95'][0] > 1,
                secondary_regressions=[key for key in ('prove_ms', 'verify_ms', 'peak_rss_bytes')
                                       if metrics[key]['status'] == 'regression'],
                note='A 1% latency gate passes when the upper95 ratio is <= 1.01; this is not proof of zero regression.')


def report(result):
    def duration(metric):
        return f"{metric['baseline']['median']:.2f} → {metric['candidate']['median']:.2f}"
    def change(metric, interval=False):
        if metric['ratio'] is None:
            return 'n/a'
        value = f"{100 * (metric['ratio'] - 1):+.2f}%"
        if interval:
            lo, hi = metric['interval95']
            value += f" [{100 * (lo - 1):+.2f}, {100 * (hi - 1):+.2f}]"
        return value
    lines = ['# Falcon commitment experiment measurements', '',
             'Batch 1024, security 128, seed 42; two warm-ups and three measured trials per fresh process. '
             'Times are milliseconds and absolute values are medians of process medians. Changes use the '
             'geometric mean of paired candidate/baseline ratios; brackets show a paired bootstrap 95% interval. '
             'Intervals are exploratory and unadjusted for multiple experiments. These measurements do not establish MacBook performance.', '',
             '| Experiment | Workload | Threads | Pairs | Commit, ms | Commit change [95%] | Total, ms | Total change [95%] | Prove change | Verify change | RSS change [95%] | 1% latency gates |',
             '|---|---|---:|---:|---|---|---|---|---:|---:|---|---|']
    for row in result['comparisons']:
        if row['diagnostic']:
            continue
        m = row['metrics']
        gates = '/'.join(row['selection']['latency_one_percent_gates'][name] for name in TIMINGS)
        lines.append(f"| {row['experiment']} | {row['mode']} {row['cell'][0]} | {row['cell'][2]} | {m['total_prover_ms']['pairs']} | "
                     f"{duration(m['witness_commit_ms'])} | {change(m['witness_commit_ms'], True)} | "
                     f"{duration(m['total_prover_ms'])} | {change(m['total_prover_ms'], True)} | "
                     f"{change(m['prove_ms'])} | {change(m['verify_ms'])} | {change(m['peak_rss_bytes'], True)} | {gates} |")
    lines += ['', 'The 1% latency gates are listed in commit/total/prove/verify order: pass means upper95 ratio ≤ 1.01; '
              'regression means lower95 ratio > 1.01; otherwise inconclusive. These tolerance gates do not prove zero regression. '
              'Significant slowdowns below the tolerance and all metric intervals remain explicit in summary.json.', '',
              'Separate instrumented runs explain stage costs; they do not enter performance selection. '
              'SHAKE includes fused witness packing and auxiliary construction. Source construction/packing are '
              'later arithmetic preparation. PCS is encoding plus Merkle; all component sums are formed per trial '
              'before medians. Other time is commit-body wall time minus the six disjoint scopes and includes logging overhead.', '',
              '| Experiment | N | Threads | Pairs | SHAKE | Arithmetic | Source construction | Source packing | Encode | Merkle | Other |',
              '|---|---:|---:|---:|---|---|---|---|---|---|---|']
    for row in result['comparisons']:
        if row['diagnostic'] and row['mode'] == 'full':
            m = row['metrics']
            parts = [duration(m[key]) for key in (*STAGES, 'commit_other_ms')]
            lines.append(f"| {row['experiment']} | {row['cell'][0]} | {row['cell'][2]} | {m['encode_ms']['pairs']} | "
                         + ' | '.join(parts) + ' |')
    arithmetic = [row for row in result['comparisons'] if row['diagnostic'] and row['mode'] == 'arithmetic']
    if arithmetic:
        lines += ['', 'Arithmetic diagnostics measure only `falcon_algebraic:witness`, excluding packing and PCS.', '',
                  '| Experiment | N | Threads | Pairs | Arithmetic generation, ms | Change [95%], descriptive |',
                  '|---|---:|---:|---:|---|---|']
        for row in arithmetic:
            metric = row['metrics']['generation_ms']
            lines.append(f"| {row['experiment']} | {row['cell'][0]} | {row['cell'][2]} | {metric['pairs']} | "
                         f"{duration(metric)} | {change(metric, True)} |")
    lines += ['', f"Validated {result['processes']} processes and {result['verified_proofs']} proofs across "
              f"{len(result['campaigns'])} completed campaigns. Inputs, source roots, proof debug digests, payloads, "
              'capacity and public protocol/source shape match within each mode and configuration across all included campaigns. The debug digest is '
              'an exact-build comparison aid, not a stable proof encoding. RSS is whole-process peak memory.', '',
              'All completed extension campaigns within an experiment are retained and pooled when their builds '
              'and variant environments match. Detailed stage samples, paired intervals, source manifests and '
              'selection evidence flags are listed in summary.json; an inconclusive interval does not establish nonregression.', '']
    if result['incomplete_campaigns']:
        lines += ['Incomplete campaigns excluded: ' + ', '.join(result['incomplete_campaigns']) + '.', '']
    return '\n'.join(lines)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--root', type=Path, default=Path(__file__).resolve().parent)
    parser.add_argument('--repo', type=Path, default=Path(__file__).resolve().parents[2])
    args = parser.parse_args()
    root = args.root.resolve()
    sys.path.insert(0, str(args.repo.resolve() / 'scripts'))
    from bench_statistics import paired_interval, sample_statistics

    identities, headers, seen = {}, {}, set()
    groups, group_builds = defaultdict(list), {}
    campaigns, incomplete = [], []
    for path in sorted(root.rglob('manifest.json')):
        manifest = json.loads(path.read_text())
        config = manifest.get('configuration', {})
        if 'mode' not in config or 'diagnostic' not in config:
            continue
        relative = path.parent.relative_to(root)
        if not manifest.get('complete'):
            incomplete.append(str(relative))
            continue
        records = validate_campaign(path.parent, manifest, identities, headers, seen)
        experiment = str(relative.parent) if len(relative.parts) > 1 else relative.name
        mode = config['mode']
        diagnostic = config['diagnostic']
        builds = {variant: dict(sha256=manifest['binaries'][variant]['sha256'],
                                environment=config.get(variant + '_env', []))
                  for variant in ('baseline', 'candidate')}
        key = (experiment, mode, diagnostic)
        require(key not in group_builds or group_builds[key] == builds,
                f'{experiment}: changed builds/environment across extension campaigns')
        group_builds[key] = builds
        for record in records:
            record.update(campaign=str(relative), pair=f'{relative}:{record["block"]:04d}')
            groups[experiment, mode, diagnostic, tuple(record['cell'])].append(record)
        campaigns.append(dict(path=str(relative), experiment=experiment, mode=mode, diagnostic=diagnostic,
                              blocks=config['blocks'], cells=manifest['cells'], processes=len(records),
                              verified_proofs=manifest['verified_proofs'], builds=builds,
                              source=manifest.get('source'), provenance=manifest.get('provenance'),
                              manifest_sha256=digest(path.read_bytes())))
    require(campaigns, 'no completed benchmark campaigns found')
    comparisons = []
    for (experiment, mode, diagnostic, cell), records in sorted(groups.items()):
        names = records[0]['metrics']
        require(all(set(row['metrics']) == set(names) for row in records), 'inconsistent metric sets')
        metrics = {name: paired_metric(records, name, paired_interval, sample_statistics) for name in names}
        comparisons.append(dict(experiment=experiment, mode=mode, diagnostic=diagnostic, cell=list(cell),
                                metrics=metrics, selection=None if diagnostic else selection(metrics),
                                diagnostic_processes=records if diagnostic else None))
    result = dict(schema='bitz/falcon-commit-experiments/v1', campaigns=campaigns,
                  incomplete_campaigns=incomplete, cross_campaign_identities_match=True,
                  processes=sum(row['processes'] for row in campaigns),
                  verified_proofs=sum(row['verified_proofs'] for row in campaigns),
                  comparisons=comparisons,
                  identities=[dict(mode=key[0], cell=list(key[1:]), identity=value, header=headers[key])
                              for key, value in sorted(identities.items())],
                  definitions=dict(absolute='Median of fresh-process medians.',
                                   ratio='Geometric mean of paired candidate/baseline process-median ratios.',
                                   interval='Two-sided 95% paired percentile bootstrap; 10000 resamples; no multiple-comparison correction.',
                                   latency_gates='Each commit/total/prove/verify upper95 ratio <= 1.01; distinct from zero-regression evidence.',
                                   diagnostics='Descriptive only; component sums and residuals computed within trials before medians.',
                                   pooling='All completed matching-build extension campaigns under each experiment; no discarded initial samples.'))
    (root / 'summary.json').write_text(json.dumps(result, indent=2) + '\n')
    (root / 'report.md').write_text(report(result))
    print(json.dumps(dict(summary=str(root / 'summary.json'), report=str(root / 'report.md'),
                          campaigns=len(campaigns), processes=result['processes'],
                          verified_proofs=result['verified_proofs'], incomplete=incomplete)))


if __name__ == '__main__':
    main()

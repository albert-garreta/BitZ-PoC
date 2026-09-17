#!/usr/bin/env python3
"""Paired regression runs for the existing multiplication, SHA and MultiSwap benches.

Each manifest maps bench names to prebuilt executables. Run under bench_gate.py;
use matching features/compiler flags and no allocation instrumentation.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import resource
import shlex
import statistics
import subprocess
from compare_prover_snapshots import paired_interval, classify_interval


def jsonl(path):
    return [json.loads(line) for line in path.read_text().splitlines() if line.startswith('{')]


def read_samples(kind, directory, reps, trial_kind="sample"):
    count = reps if trial_kind == "sample" else 1
    lines = [line.strip() for line in (directory / 'stdout').read_text().splitlines()]
    if kind == 'mul':
        rows = [r for r in jsonl(directory/'native/samples.jsonl') if r['trial']['kind'] == trial_kind]
        assert len(rows) == count and all(r['proof_verified'] for r in rows)
        runs = [r for r in jsonl(directory/'native/trace.jsonl') if r['record'] == 'run' and r['trial']['kind'] == trial_kind]
        spans = [r for r in jsonl(directory/'native/trace.jsonl') if r['record'] == 'span' and r['name'] == 'gkr']
        result = []
        for row, run in zip(rows, runs):
            metric = row['metrics']
            result.append(dict(e2e_ms=metric['witness_to_proof_ms'], prove_ms=metric['online_prover_ms'],
                               verify_ms=metric['verify_ms'], proof_bytes=metric['proof_bytes'],
                               gkr_ms=sum(int(s['duration_ns']) for s in spans if s['run_id'] == run['run_id'])/1e6))
        return result
    if kind == 'multiswap':
        runs = [r for r in jsonl(directory/'multiswap.jsonl') if r['record'] == 'run' and r['trial']['kind'] == trial_kind]
        assert len(runs) == count and all(r['validation']['proof_verified'] for r in runs)
        return [dict(e2e_ms=int(r['measurements_ns']['application_total'])/1e6,
                     prove_ms=int(r['measurements_ns']['online_prover'])/1e6,
                     verify_ms=int(r['measurements_ns']['verification'])/1e6,
                     gkr_ms=int(r['measurements_ns']['merged_forest_gkr'])/1e6,
                     proof_bytes=r['artifacts']['proof_bytes']) for r in runs]
    trials = [json.loads(line.split(' ', 1)[1]) for line in lines if line.startswith('PROVER_TRIAL ')]
    if trials:
        selected = [r for r in trials if r['trial'] == trial_kind]
        assert len(selected) == count and all(r['verified'] for r in selected)
        return selected
    if trial_kind != 'sample':
        return []  # Legacy binaries do not expose their first proof.
    report = next(line for line in lines if line.startswith('RESULT '))
    report = dict(piece.split('=', 1) for piece in shlex.split(report)[1:])
    assert int(report['verified_samples']) == reps
    phases = [json.loads(line.split(' ', 1)[1]) for line in lines if line.startswith('PHASE_SAMPLE ')]
    proves = [p for p in phases if p['kind'] == 'prove']
    verifies = [p for p in phases if p['kind'] == 'verify']
    assert len(proves) == len(verifies) == reps
    # These legacy benches report witness generation separately as a median
    # (u32 prepares it once); do not label this sum an enclosing wall-clock span.
    return [dict(e2e_ms=p['total_ms'] + float(report['witness_ms']), prove_ms=p['total_ms'],
                 verify_ms=v['total_ms'], proof_bytes=int(report['proof_bytes']),
                 gkr_ms=1000*dict(p['phases_seconds'])['mc:forest']) for p, v in zip(proves, verifies)]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--baseline', type=Path, required=True)
    parser.add_argument('--candidate', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--kinds', nargs='+', default=['mul', 'sha', 'u32', 'multiswap'])
    parser.add_argument('--workloads', nargs='+', choices=['u32-mod32', 'u64', 'u128'], default=['u32-mod32', 'u64', 'u128'])
    parser.add_argument('--exponents', type=int, nargs='+', default=[15, 19])
    parser.add_argument('--threads', type=int, nargs='+', default=[1, 10])
    parser.add_argument('--batches', type=int, nargs='+', default=[1, 2, 4, 8])
    parser.add_argument('--sha-exponents', type=int, nargs='+', default=[7, 10, 12])
    parser.add_argument('--blocks', type=int, default=6)
    parser.add_argument('--reps', type=int, default=5)
    parser.add_argument('--seed', type=int, default=0)
    parser.add_argument('--baseline-env', action='append', default=[])
    parser.add_argument('--candidate-env', action='append', default=[])
    parser.add_argument('--require-cold-and-fingerprints', action='store_true')
    parser.add_argument('--schedule', choices=['l2', 'l4', 'l8'], default='l4')
    args = parser.parse_args()
    args.output.mkdir(parents=True, exist_ok=False)
    manifests = {v: json.loads(getattr(args, v).read_text()) for v in ['baseline', 'candidate']}
    binaries = {v: {name: dict(path=str(Path(path).resolve()), sha256=hashlib.sha256(Path(path).read_bytes()).hexdigest())
                    for name, path in manifest.items()} for v, manifest in manifests.items()}
    (args.output/'manifest.json').write_text(json.dumps(dict(binaries=binaries, args={k:str(v) for k,v in vars(args).items()}, affinity=sorted(os.sched_getaffinity(0))), indent=2))
    cases = []
    if 'mul' in args.kinds:
        cases += [('mul', 'mul_e2e_compare', f'{w}-n{n}', dict(F2Z_MUL_COMPARE_WORKLOADS=w, F2Z_BENCH_SHAPES=str(n)))
                  for w in args.workloads for n in args.exponents]
    if 'u32' in args.kinds:
        cases += [('u32', 'u32_mul', f'full-u32-w{w}-n{n}', dict(F2Z_MUL_WORD_BITS=str(w), F2Z_BENCH_SHAPES=str(n)))
                  for w in [1, 8] for n in args.exponents]
    if 'sha' in args.kinds:
        cases += [('sha', 'sha256_chain', f'sha-n{n}', dict(F2Z_BENCH_SHAPES=str(n))) for n in args.sha_exponents]
    if 'multiswap' in args.kinds:
        cases += [('multiswap', 'multiswap', f'multiswap-b{b}', dict(F2Z_BENCH_SHAPES='0', F2Z_MULTISWAP_BATCH_COUNT=str(b), F2Z_BENCH_LAMBDA='114')) for b in args.batches]
    clean = {k:v for k,v in os.environ.items() if not k.startswith(('F2Z_', 'F2_FOREST_', 'RAYON_')) and k != 'HARDWARE_CONCURRENCY'}
    results = []
    for kind, bench, case, case_env in cases:
        for threads in args.threads:
            blocks = {v: [] for v in manifests}
            sample_sizes = None
            first_size = None
            expected_fingerprints = None
            for block in range(args.blocks):
                for variant in (list(manifests) if block % 2 == 0 else list(manifests)[::-1]):
                    directory = (args.output/f'{case}-t{threads}-b{block}-{variant}').resolve()
                    directory.mkdir()
                    env = dict(clean, F2Z_LIG_PROFILE='custom:1:4', F2_FOREST_SCHEDULE=args.schedule,
                               RAYON_NUM_THREADS=str(threads), HARDWARE_CONCURRENCY=str(threads), F2Z_BENCH_SEED=str(args.seed),
                               F2Z_BENCH_REPS=str(args.reps), F2Z_BENCH_LAMBDA='100', F2Z_BENCH_PASS='latency',
                               F2Z_BENCH_PHASE_SAMPLES='1', F2Z_BENCH_PROOF_FINGERPRINT='1', F2Z_MUL_COMPARE_BACKENDS='f2z', F2Z_MUL_COMPARE_MEMORY='0',
                               F2Z_MUL_COMPARE_OUTPUT_DIR=str(directory/'native'), F2Z_MULTISWAP_TRACE_PATH=str(directory/'multiswap.jsonl'))
                    env.update(case_env)
                    env.update(item.split('=', 1) for item in getattr(args, variant + '_env'))
                    cpus = ','.join(map(str, sorted(os.sched_getaffinity(0))[:threads]))
                    command = ['taskset', '-c', cpus, binaries[variant][bench]['path']]
                    def limit():
                        resource.setrlimit(resource.RLIMIT_AS, (48*1024**3, 48*1024**3))
                    with (directory/'stdout').open('w') as out, (directory/'stderr').open('w') as err:
                        subprocess.run(command, env=env, stdout=out, stderr=err, timeout=1800, check=True, preexec_fn=limit)
                    fingerprints = [json.loads(line.split(' ', 1)[1]) for line in
                                    (directory/'stdout').read_text().splitlines()
                                    if line.startswith('PROOF_FINGERPRINT ')]
                    if args.require_cold_and_fingerprints:
                        assert fingerprints, f'missing proof fingerprint: {directory}'
                    if fingerprints:
                        assert len(fingerprints) == args.reps + 1
                        # SHA chains deliberately use a different seed for
                        # each trial. Compare corresponding trials across
                        # variants and blocks, not different inputs in a run.
                        if expected_fingerprints is None:
                            expected_fingerprints = fingerprints
                        assert fingerprints == expected_fingerprints, f'proof/transcript changed: {case}'
                    samples = read_samples(kind, directory, args.reps)
                    sizes = [sample['proof_bytes'] for sample in samples]
                    if sample_sizes is None:
                        sample_sizes = sizes
                    assert sizes == sample_sizes, f'proof size changed: {case}'
                    for sample in samples:
                        assert sample['gkr_ms'] > 0, 'missing GKR measurement'
                    (directory/'samples.json').write_text(json.dumps(samples, indent=2))
                    medians = {m:statistics.median(r[m] for r in samples) for m in ['e2e_ms', 'prove_ms', 'gkr_ms', 'verify_ms']}
                    first = read_samples(kind, directory, args.reps, 'warmup')
                    if args.require_cold_and_fingerprints:
                        assert first, f'missing first-proof metrics: {directory}'
                    if first:
                        if first_size is None:
                            first_size = first[0]['proof_bytes']
                        assert first[0]['proof_bytes'] == first_size, f'first proof size changed: {case}'
                        medians.update({'cold_' + m: first[0][m] for m in ['e2e_ms', 'prove_ms', 'gkr_ms', 'verify_ms']})
                    blocks[variant].append(medians)
                    print(directory.name, {m:round(v,3) for m,v in medians.items()}, flush=True)
            result = dict(case=case, threads=threads, sample_proof_bytes=sample_sizes,
                          first_proof_bytes=first_size, fingerprints=expected_fingerprints, metrics={})
            common_metrics = set.intersection(*(set(row) for rows in blocks.values() for row in rows))
            for metric in sorted(common_metrics):
                ratios = [b[metric]/a[metric] for a,b in zip(blocks['baseline'],blocks['candidate'])]
                result['metrics'][metric] = dict(baseline_ms=statistics.median(b[metric] for b in blocks['baseline']),
                    candidate_ms=statistics.median(b[metric] for b in blocks['candidate']), paired_ratios=ratios, ratio_ci95=paired_interval(ratios),
                    nonregression=classify_interval(paired_interval(ratios)))
            results.append(result)
            (args.output/'summary.json').write_text(json.dumps(results, indent=2))


if __name__ == '__main__':
    main()

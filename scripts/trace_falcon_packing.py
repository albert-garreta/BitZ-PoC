#!/usr/bin/env python3
"""Capture separate diagnostic traces after tracing-disabled qualification.

Reuse the preserved campaign's validated interval converter without modifying
that campaign. Only batch 1024, eight workers, and security 100 are profiled.
"""
import argparse
import importlib.util
import json
from pathlib import Path
import platform
import subprocess

from qualify_falcon_packing import compare, environment, sha


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--baseline', type=Path, required=True)
    parser.add_argument('--candidate', type=Path, required=True)
    parser.add_argument('--baseline-manifest', type=Path, required=True)
    parser.add_argument('--candidate-manifest', type=Path, required=True)
    parser.add_argument('--cache', type=Path, required=True)
    parser.add_argument('--out', type=Path, required=True)
    args = parser.parse_args()
    args.out.mkdir(parents=True, exist_ok=False)
    repo = Path(__file__).resolve().parents[1]
    converter_path = repo / 'results/falcon-bottlenecks-20261008/analyze_exact.py'
    spec = importlib.util.spec_from_file_location('preserved_falcon_converter', converter_path)
    converter = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(converter)
    revision = subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=repo, text=True).strip()
    metadata = {'platform': platform.platform(), 'machine': 'Apple M1 Max'}
    converted, profiles, records = [], [], []
    for degree in (512, 1024):
        identities = []
        # Alternate the two diagnostic process pairs too.
        variants = ['original', 'production'][::1 if degree == 512 else -1]
        for variant in variants:
            binary = (args.baseline if variant == 'original' else args.candidate).resolve()
            source = args.baseline_manifest if variant == 'original' else args.candidate_manifest
            directory = args.out / variant
            directory.mkdir(exist_ok=True)
            stem = f'exact-arithmetic-falcon{degree}'
            env = environment(8, args.cache.resolve(), variant, None)
            env['BITZ_FALCON_TRACE_JSONL'] = str((directory / (stem + '.trace.jsonl')).resolve())
            command = [str(binary), '--degree', str(degree), '--batch', '1024',
                       '--security', '100', '--threads', '8', '--warmup', '1',
                       '--iterations', '5', '--seed', '42']
            with (directory / (stem + '.stdout.log')).open('x') as stdout, (directory / (stem + '.stderr.log')).open('x') as stderr:
                subprocess.run(command, env=env, stdout=stdout, stderr=stderr, check=True)
            trials = [r for r in converter.read_jsonl(directory / (stem + '.stdout.log'))
                      if r.get('trial') in ('warmup', 'sample')]
            identity = {k: trials[0][k] for k in ('input_digest', 'proof_payload_bytes',
                                                'proof_payload_breakdown', 'source_root', 'proof_debug_digest')}
            assert all(all(r[k] == v for k, v in identity.items()) for r in trials)
            identities.append({'identity': identity})
            converter.OUT = directory
            canonical, profile = converter.convert_case('arithmetic', degree, metadata, revision, sha(source))
            profile['name'] = f'{variant}-{profile["name"]}'
            profile['variant'] = variant
            for record in canonical:
                record['run_id'] = f'{variant}-{record["run_id"]}'
                if record['record'] == 'run':
                    record['series_id'] = f'{variant}-{record["series_id"]}'
                    record['benchmark'].update(
                        implementation=f'BitZ arithmetic Falcon ({variant})',
                        source_changes='Direct arithmetic packing; see frozen source manifest',
                        executable_sha256=sha(binary))
                elif record['name'] == 'falcon_algebraic:witness_pack':
                    record['attributes']['math_latex'] = [r'w=\sum_{j=0}^{3}(c_j\mathbin{\&}32767)2^{16j}']
            converted.extend(canonical)
            profiles.append(profile)
            records.append({'variant': variant, 'degree': degree, 'command': command,
                            'executable_sha256': sha(binary), 'source_manifest_sha256': sha(source)})
        compare(*identities)
    with (args.out / 'traces.jsonl').open('x') as stream:
        for record in converted:
            stream.write(json.dumps(record) + '\n')
    (args.out / 'analysis.json').write_text(json.dumps({'profiles': profiles, 'runs': records,
        'converter_sha256': sha(converter_path), 'performance_acceptance': False,
        'boundary': 'Trial root includes verification; reported prover total is commit plus prove only.'}, indent=2) + '\n')


if __name__ == '__main__':
    main()

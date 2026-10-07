import csv
import datetime
import hashlib
import json
import os
from pathlib import Path
import statistics
import subprocess
import time

OUT = Path(__file__).resolve().parent
ROOT = Path('/home/john-wu/code/BitZ-pcs')
metadata = json.loads((OUT / 'metadata.json').read_text())
BINARY = Path(metadata['binary_path'])
metadata['binary_sha256'] = hashlib.sha256(BINARY.read_bytes()).hexdigest()
metadata['started_utc'] = datetime.datetime.now(datetime.timezone.utc).isoformat()
samples = []
for threads in metadata['execution_order']:
    cpus = ','.join(map(str, range(threads)))
    env = os.environ.copy()
    for key in ['PCS_TRACE', 'FLOCK_COMMIT_TIMING', 'LIGERITO_TRACE',
                'LIG_PROVE_TRACE', 'LIG_VERIFY_TRACE', 'BITZ_FALCON_STAGE_TIMINGS']:
        env.pop(key, None)
    env.update({
        'RAYON_NUM_THREADS': str(threads),
        'BITZ_FALCON_WORKER_CPUS': cpus,
        'BITZ_FALCON_MAIN_CPU': '0',
        'BITZ_FALCON_CASE_CACHE': '/tmp/falcon-algebraic-cases-20261007',
        'FLOCK_NO_PREFAULT': '1',
    })
    cmd = ['taskset', '-c', cpus, str(BINARY), '--degree', str(metadata['degree']),
           '--k', str(metadata['ring_extension']), '--batch', str(metadata['batch']),
           '--security', str(metadata['security_bits']), '--threads', str(threads),
           '--warmup', str(metadata['warmup']), '--iterations', str(metadata['measured_trials']),
           '--seed', str(metadata['seed'])]
    metadata.setdefault('commands', {})[str(threads)] = cmd
    metadata.setdefault('runtime_environment', {})[str(threads)] = {
        key: value for key, value in sorted(env.items())
        if key.startswith(('RAYON_', 'BITZ_', 'FLOCK_', 'LIG_', 'LIGERITO_', 'PCS_'))
    }
    (OUT / 'metadata.json').write_text(json.dumps(metadata, indent=2) + '\n')
    print(f'Starting full verification at {threads} thread(s), physical CPUs {cpus}', flush=True)
    start = time.monotonic()
    with (OUT / f'threads-{threads}.jsonl').open('w') as stdout, \
         (OUT / f'threads-{threads}.stderr').open('w') as stderr:
        result = subprocess.run(cmd, cwd=ROOT, env=env, stdout=stdout, stderr=stderr, timeout=3600)
    if result.returncode:
        raise SystemExit(f'{threads} threads failed: {(OUT / f"threads-{threads}.stderr").read_text()}')
    records = [json.loads(line) for line in (OUT / f'threads-{threads}.jsonl').read_text().splitlines()]
    prepared = [r for r in records if r.get('event') == 'prepared']
    trials = [r for r in records if r.get('event') == 'trial']
    measured = [r for r in trials if r['trial'] == 'sample']
    assert len(prepared) == 1
    assert len(trials) == metadata['warmup'] + metadata['measured_trials']
    assert len(measured) == metadata['measured_trials']
    p = prepared[0]
    assert p['algebraic_security_bits'] >= metadata['security_bits']
    assert not p['stage_timings']
    affinity = p['cpu_affinity']
    assert affinity['observed_worker_cpus'] == [[i] for i in range(threads)]
    assert affinity['observed_main_cpus'] == [0]
    assert affinity['observed_auxiliary_worker_cpus'] == [[0] for _ in range(threads)]
    for record in [p] + trials:
        assert record['threads'] == threads
        assert record['batch'] == metadata['batch']
        assert record['degree'] == metadata['degree']
        assert record['ring_extension'] == metadata['ring_extension']
        assert record['security_target'] == metadata['security_bits']
        assert record['input_digest'] == metadata['expected_input_digest']
    for record in trials:
        assert record['verified'] is True
        assert record['flock_verifier_affinity']['observed_cpus'] == [0]
        assert sum(record['proof_payload_breakdown'].values()) == record['proof_payload_bytes']
    samples.extend(measured)
    median = statistics.median(r['total_prover_ms'] for r in measured)
    print(f'Finished {threads} thread(s): median total prover {median:.3f} ms; '
          f'wall time {time.monotonic() - start:.1f} s', flush=True)

rows = []
baseline = statistics.median(r['total_prover_ms'] for r in samples if r['threads'] == 1)
for threads in metadata['threads']:
    group = [r for r in samples if r['threads'] == threads]
    def median(key):
        return statistics.median(r[key] for r in group)
    rows.append({
        'threads': threads, 'batch': metadata['batch'], 'security_bits': metadata['security_bits'],
        'samples': len(group), 'witness_commit_ms_median': median('witness_commit_ms'),
        'prove_ms_median': median('proof_prove_ms'),
        'total_prover_ms_median': median('total_prover_ms'),
        'total_prover_ms_min': min(r['total_prover_ms'] for r in group),
        'total_prover_ms_max': max(r['total_prover_ms'] for r in group),
        'total_prover_ms_stdev': statistics.stdev(r['total_prover_ms'] for r in group),
        'verify_ms_median': median('proof_verify_ms'),
        'speedup': baseline / median('total_prover_ms'),
        'signatures_per_second': 1000 * metadata['batch'] / median('total_prover_ms'),
        'proof_payload_bytes_median': median('proof_payload_bytes'),
        'proof_payload_bytes_min': min(r['proof_payload_bytes'] for r in group),
        'proof_payload_bytes_max': max(r['proof_payload_bytes'] for r in group),
        'peak_rss_mib': max(r['process_peak_rss_kib'] for r in group) / 1024,
    })
with (OUT / 'summary.csv').open('w') as f:
    writer = csv.DictWriter(f, fieldnames=rows[0].keys())
    writer.writeheader()
    writer.writerows(rows)
(OUT / 'summary.json').write_text(json.dumps(rows, indent=2) + '\n')
report = [
    '# Full Falcon-1024 verification proof thread scaling', '',
    f"{metadata['batch']:,} signatures, {metadata['security_bits']}-bit target, seed {metadata['seed']}. "
    'One warm-up and five measured trials per configuration. Every proof verified. '
    'Includes SHAKE-256 and HashToPoint. Medians unless otherwise marked.', '',
    '| Threads | Total prover (ms) | Speedup | Signatures/s | Verify (ms) | Payload (KiB) | Peak RSS (MiB) |',
    '|---:|---:|---:|---:|---:|---:|---:|',
]
for r in rows:
    report.append(f"| {r['threads']} | {r['total_prover_ms_median']:.3f} | {r['speedup']:.2f}x | "
                  f"{r['signatures_per_second']:.1f} | {r['verify_ms_median']:.3f} | "
                  f"{r['proof_payload_bytes_median']/1024:.2f} | {r['peak_rss_mib']:.1f} |")
report += ['', '## Timing boundaries and reproducibility', '',
    '- Total prover includes witness generation (SHAKE, HashToPoint, ring and norm data), packing, commitment, statement clone, and proof generation.',
    '- Configuration, key/signature generation, public parsing, upstream native verification, proof debug digest, and payload accounting are outside prover timing.',
    '- Verification combines the configured global pool with a serial Flock Keccak verifier pinned to CPU 0.',
    '- Peak RSS is process-wide, including input preparation and persistent caches.',
    '- Proof size is canonical stored proof payload; excludes the public statement, its 32-byte source root, and outer transport framing.',
    '- Native release build: -C target-cpu=native, fat LTO, one codegen unit. Stage timings disabled.',
    '- AMD Ryzen 9 9950X3D. Workers on physical CPUs 0..N-1; main and auxiliary workers on CPU 0. No SMT.',
    '- CPUs 0–7 share 96 MiB L3; CPUs 8–15 share 32 MiB L3. The 16-thread run crosses chiplets. Governor: powersave.',
    '- Sequential execution order: 1, 16, 2, 8, 4. No competing benchmark or compilation was started by this agent.',
    '- Same original-Falcon corpus as the algebraic 1024 sweep.',
    f"- Input digest: `{metadata['expected_input_digest']}`.", '',
    'Raw records, metadata, source hashes, and CSV phase statistics are saved alongside this report.', '']
(OUT / 'REPORT.md').write_text('\n'.join(report))
metadata['finished_utc'] = datetime.datetime.now(datetime.timezone.utc).isoformat()
metadata['input_digest'] = metadata['expected_input_digest']
metadata['verified_proofs'] = len(metadata['threads']) * (metadata['warmup'] + metadata['measured_trials'])
(OUT / 'metadata.json').write_text(json.dumps(metadata, indent=2) + '\n')
print(json.dumps(rows, indent=2), flush=True)

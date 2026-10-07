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
BINARY = Path('/tmp/falcon-simplify-candidate-target/release/examples/falcon_algebraic')
metadata = json.loads((OUT / 'metadata.json').read_text())
metadata['binary_sha256'] = hashlib.sha256(BINARY.read_bytes()).hexdigest()
metadata['started_utc'] = datetime.datetime.now(datetime.timezone.utc).isoformat()
(OUT / 'metadata.json').write_text(json.dumps(metadata, indent=2) + '\n')
samples = []
for threads in metadata['execution_order']:
    cpus = ','.join(map(str, range(threads)))
    env = os.environ.copy()
    for key in ['PCS_TRACE', 'FLOCK_COMMIT_TIMING', 'LIGERITO_TRACE', 'LIG_PROVE_TRACE', 'LIG_VERIFY_TRACE']:
        env.pop(key, None)
    env.update({
        'RAYON_NUM_THREADS': str(threads),
        'BITZ_FALCON_WORKER_CPUS': cpus,
        'BITZ_FALCON_MAIN_CPU': '0',
        'BITZ_FALCON_CASE_CACHE': '/tmp/falcon-algebraic-cases-20261007',
        'FLOCK_NO_PREFAULT': '1',
    })
    cmd = ['taskset', '-c', cpus, str(BINARY), '--batch', str(metadata['batch']),
           '--security', str(metadata['security_bits']), '--threads', str(threads),
           '--warmup', str(metadata['warmup']), '--iterations', str(metadata['measured_trials']),
           '--seed', str(metadata['seed'])]
    metadata.setdefault('commands', {})[str(threads)] = cmd
    metadata.setdefault('runtime_environment', {})[str(threads)] = {
        key: value for key, value in sorted(env.items())
        if key.startswith(('RAYON_', 'BITZ_', 'FLOCK_', 'LIG_', 'LIGERITO_', 'PCS_'))
    }
    (OUT / 'metadata.json').write_text(json.dumps(metadata, indent=2) + '\n')
    print(f'Starting {threads} thread(s), physical CPUs {cpus}', flush=True)
    start = time.monotonic()
    with (OUT / f'threads-{threads}.jsonl').open('w') as stdout, \
         (OUT / f'threads-{threads}.stderr').open('w') as stderr:
        result = subprocess.run(cmd, cwd=ROOT, env=env, stdout=stdout, stderr=stderr, timeout=900)
    if result.returncode:
        raise SystemExit(f'{threads} threads failed: {(OUT / f"threads-{threads}.stderr").read_text()}')
    records = [json.loads(line) for line in (OUT / f'threads-{threads}.jsonl').read_text().splitlines()]
    measured = [r for r in records if r['trial'] == 'sample']
    if len(measured) != metadata['measured_trials'] or len(records) != metadata['warmup'] + metadata['measured_trials']:
        raise RuntimeError('Incorrect trial count')
    for record in records:
        assert record['threads'] == threads == record['auxiliary_pool_threads']
        assert record['batch'] == metadata['batch'] and record['security_bits'] == metadata['security_bits']
        affinity = record['cpu_affinity']
        assert affinity['observed_worker_cpus'] == [[i] for i in range(threads)]
        assert affinity['observed_main_cpus'] == [0]
        assert affinity['observed_auxiliary_worker_cpus'] == [[0] for _ in range(threads)]
    assert all(record['algebraic_security_bits'] >= metadata['security_bits'] for record in records)
    assert len({record['input_digest'] for record in records}) == 1
    samples.extend(measured)
    median = statistics.median(r['total_prover_ms'] for r in measured)
    print(f'Finished {threads} thread(s): median total prover {median:.3f} ms; '
          f'wall time {time.monotonic() - start:.1f} s', flush=True)

assert len({r['input_digest'] for r in samples}) == 1, 'Inputs differ across configurations'
rows = []
baseline = statistics.median(r['total_prover_ms'] for r in samples if r['threads'] == 1)
for threads in metadata['threads']:
    group = [r for r in samples if r['threads'] == threads]
    def median(key):
        return statistics.median(r[key] for r in group)
    rows.append({
        'threads': threads, 'batch': metadata['batch'], 'security_bits': metadata['security_bits'],
        'samples': len(group), 'witness_ms_median': median('witness_ms'),
        'commit_ms_median': median('commit_ms'), 'prove_ms_median': median('prove_ms'),
        'total_prover_ms_median': median('total_prover_ms'),
        'total_prover_ms_min': min(r['total_prover_ms'] for r in group),
        'total_prover_ms_max': max(r['total_prover_ms'] for r in group),
        'total_prover_ms_stdev': statistics.stdev(r['total_prover_ms'] for r in group),
        'verify_ms_median': median('verify_ms'),
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
    '# Falcon-1024 algebraic proof thread scaling', '',
    f"{metadata['batch']:,} signatures, {metadata['security_bits']}-bit target, seed {metadata['seed']}. One warm-up and five measured trials per configuration. "
    'Every proof was verified. Results are medians unless otherwise marked.', '',
    '| Threads | Total prover (ms) | Speedup | Signatures/s | Verify (ms) | Payload (KiB) | Peak RSS (MiB) |',
    '|---:|---:|---:|---:|---:|---:|---:|',
]
for r in rows:
    report.append(f"| {r['threads']} | {r['total_prover_ms_median']:.3f} | {r['speedup']:.2f}x | "
                  f"{r['signatures_per_second']:.1f} | {r['verify_ms_median']:.3f} | "
                  f"{r['proof_payload_bytes_median']/1024:.2f} | {r['peak_rss_mib']:.1f} |")
report += ['', '## Timing boundaries and reproducibility', '',
    '- Total prover includes centered witness reconstruction, native validation, slack/quotient preparation, commitment, and proof generation.',
    '- Configuration setup, key/signature generation, public-target hashing, and input copies are outside total prover timing.',
    '- Verification uses the configured global worker pool. It is not a single-thread verifier comparison.',
    '- Peak RSS is process-wide, including input preparation and persistent caches; it is not an isolated prover allocation peak.',
    '- Stored proof payload includes its commitment root and excludes public inputs and outer transport framing.',
    '- Native release build: `-C target-cpu=native`, fat LTO, one codegen unit. Tracing disabled.',
    '- AMD Ryzen 9 9950X3D. Workers pinned to physical CPUs 0..N-1; main and auxiliary workers pinned to CPU 0. No SMT.',
    '- CPUs 0–7 have 96 MiB shared L3; CPUs 8–15 have 32 MiB shared L3. Sixteen threads crosses chiplets. Governor: powersave.',
    '- Sequential execution order: 1, 16, 2, 8, 4. No simultaneous benchmark configurations.',
    '- Arithmetic grinding searches the smallest valid nonce in both serial and parallel modes.',
    '- Raw per-trial JSON, command metadata, source hashes and machine details are saved alongside this report.',
    f"- Shared input digest: `{samples[0]['input_digest']}`.", '',
    'See `summary.csv` for phase medians, ranges and standard deviations.', '']
(OUT / 'REPORT.md').write_text('\n'.join(report))
metadata['finished_utc'] = datetime.datetime.now(datetime.timezone.utc).isoformat()
metadata['input_digest'] = samples[0]['input_digest']
(OUT / 'metadata.json').write_text(json.dumps(metadata, indent=2) + '\n')
print(json.dumps(rows, indent=2), flush=True)

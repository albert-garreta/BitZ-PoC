#!/usr/bin/env python3
"""Diagnostic stage timings using the exact previously measured binaries."""
import importlib.util
import json
import os
from pathlib import Path
import subprocess
import time

OUT = Path(__file__).resolve().parent
BASE = OUT.parent
spec = importlib.util.spec_from_file_location('baseline_runner', BASE / 'run.py')
baseline = importlib.util.module_from_spec(spec)
spec.loader.exec_module(baseline)


def save(name, value):
    (OUT / name).write_text(json.dumps(value, indent=2) + '\n')


def main():
    os.sched_setaffinity(0, {8})
    expected_sources = json.loads((BASE / 'sources.json').read_text())
    assert baseline.source_hashes() == expected_sources
    provenance = json.loads((BASE / 'provenance.json').read_text())
    for b in provenance['binaries'].values():
        assert baseline.sha(Path(b['path'])) == b['sha256']
    for degree in (512, 1024):
        for kind in ('arithmetic', 'full'):
            stem = f'{kind}-{degree}'
            config = json.loads((BASE / (stem + '.command.json')).read_text())
            argv = config['argv']
            argv[argv.index('--iterations') + 1] = '3'
            env = {k: v for k, v in os.environ.items()
                   if not k.startswith(('BITZ_', 'RAYON_', 'RUSTFLAGS', 'CARGO_ENCODED_RUSTFLAGS'))}
            settings = config['environment']
            if kind == 'arithmetic':
                argv.append('--trace')
            else:
                settings['BITZ_FALCON_STAGE_TIMINGS'] = '1'
            env.update(settings)
            idle = baseline.idle_check()
            save(stem + '.command.json', {'argv': argv, 'environment': settings, 'idle_check': idle})
            before, start = baseline.cpu_stats(), time.monotonic()
            masks = set()
            print('Starting ' + stem, flush=True)
            with (OUT / (stem + '.jsonl')).open('w') as stdout, (OUT / (stem + '.stages.jsonl')).open('w') as stderr:
                proc = subprocess.Popen(argv, cwd=baseline.ROOT, env=env, stdout=stdout, stderr=stderr)
                while True:
                    pid, status, usage = os.wait4(proc.pid, os.WNOHANG)
                    if pid:
                        proc.returncode = os.waitstatus_to_exitcode(status)
                        break
                    time.sleep(0.5)
                    for path in Path(f'/proc/{proc.pid}/task').glob('*/status'):
                        try:
                            for line in path.read_text().splitlines():
                                if line.startswith('Cpus_allowed_list:'):
                                    masks.add(line.split(':')[1].strip())
                        except (FileNotFoundError, ProcessLookupError):
                            pass
            after = baseline.cpu_stats()
            save(stem + '.resources.json', {'exit_code': proc.returncode,
                 'elapsed_s': time.monotonic() - start,
                 'sibling_busy_pct': baseline.busy(before, after, baseline.SIBLING),
                 'observed_affinity_masks': sorted(masks), 'peak_rss_kib': usage.ru_maxrss})
            assert proc.returncode == 0 and masks == {'6'}
            rows = [json.loads(line) for line in (OUT / (stem + '.jsonl')).read_text().splitlines()]
            trials = [r for r in rows if r.get('trial') in ('warmup', 'sample')]
            assert len(trials) == 4 and all(r['verified'] and r['threads'] == 1 for r in trials)
            print('Finished ' + stem, flush=True)
    assert baseline.source_hashes() == expected_sources
    save('validation.json', {'exact_baseline_binaries': True, 'sources_unchanged': True,
         'all_16_proofs_verified': True, 'all_observed_threads_pinned_to_cpu_6': True})


if __name__ == '__main__':
    main()

#!/usr/bin/env python3
"""Check that a scoped power-profile hold applies and restores host policy."""
import json
from pathlib import Path
import subprocess
import sys


def state():
    policies = {}
    for directory in sorted(Path('/sys/devices/system/cpu/cpufreq').glob('policy*')):
        policies[directory.name] = {
            name: (directory / name).read_text().strip()
            for name in ('scaling_governor', 'energy_performance_preference')
        }
    return {
        'profile': subprocess.check_output(['powerprofilesctl', 'get'], text=True).strip(),
        'policies': policies,
    }


if len(sys.argv) > 1 and sys.argv[1] == '--inside':
    observed = state()
    print(json.dumps(observed))
    assert observed['profile'] == 'performance'
    assert observed['policies']
    assert all(p['energy_performance_preference'] == 'performance'
               for p in observed['policies'].values())
else:
    before = state()
    command = [
        'powerprofilesctl', 'launch', '--profile', 'performance',
        '--reason', 'Stable CPU policy for the Falcon baseline/candidate comparison',
        '--appid', 'org.bitz.falcon-benchmark',
        sys.executable, str(Path(__file__).resolve()), '--inside',
    ]
    result = subprocess.run(command, capture_output=True, text=True)
    after = state()
    report = dict(before=before, after=after, command=command,
                  exit_code=result.returncode, stdout=result.stdout,
                  stderr=result.stderr, restored=(after == before))
    Path(__file__).with_suffix('.json').write_text(json.dumps(report, indent=2) + '\n')
    print(json.dumps({'exit_code': result.returncode,
                      'before': before['profile'], 'after': after['profile'],
                      'restored': report['restored'], 'stderr': result.stderr}))
    assert result.returncode == 0
    assert report['restored']

#!/usr/bin/env python3
"""Compact read-only campaign progress snapshot."""
import datetime
import json
from pathlib import Path

root = Path(__file__).resolve().parent
now = datetime.datetime.now(datetime.timezone.utc)
result = {'utc':now.isoformat()}
for filename, suffix in [('retry-status.json','retry1'),('wide-retry-status.json','retry2')]:
    path = root/filename
    if not path.exists():
        continue
    records = json.loads(path.read_text())
    active = next((r for r in records if r['status']=='running'),None)
    if active:
        result['active_group'] = active['name']
        result['elapsed_minutes'] = round((now-datetime.datetime.fromisoformat(active['started_utc'])).total_seconds()/60,1)
        log = root/'logs'/f"{active['name']}.{suffix}.log"
        if log.exists():
            relevant = [line for line in log.read_text(errors='replace').splitlines()
                        if line.startswith(('u32-mod32 ', 'f2z --mul:', '(cooldown ', 'RESULT schema=', 'error:', 'test result:'))]
            result['latest_events'] = [' '.join(token for token in line.split()
                                              if not token.startswith('ligerito_hex='))
                                       for line in relevant[-2:]]
analysis_path = root/'analysis.json'
if analysis_path.exists():
    analysis=json.loads(analysis_path.read_text())
    result['last_analyzed_counts']={key:len(analysis[key]) for key in ('native','wide','sha')}
print(json.dumps(result,indent=2))

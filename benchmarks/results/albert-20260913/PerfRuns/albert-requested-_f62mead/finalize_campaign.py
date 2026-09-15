#!/usr/bin/env python3
"""Consolidate execution attempts and measured coverage; never relabel failures."""
import datetime
import json
from pathlib import Path
import sys

root=Path(__file__).resolve().parent
analysis=json.loads((root/'analysis.json').read_text())
requested={job['name'] for job in json.loads((root/'commands.json').read_text())}
attempts=[]
effective={}
for filename in ('status.json','retry-status.json','wide-retry-status.json'):
    path=root/filename
    if not path.exists():
        continue
    for row in json.loads(path.read_text()):
        item=dict(row,status_source=filename)
        attempts.append(item)
        effective[row['name']]=item
pending=[name for name in requested if name not in effective or effective[name]['status']=='running']
unsuccessful=[name for name in requested if name in effective and effective[name]['status'] not in ('complete','running')]
missing=sum(len(rows) for rows in analysis['missing'].values())
counts={key:len(analysis[key]) for key in ('native','wide','sha')}
case_complete=counts=={'native':152,'wide':24,'sha':6} and missing==0 and not analysis['incomplete']
test=effective.get('plonky3-rate-validation')
rate_log=(root/'logs/rate-validation.log').read_text()
rate_test_passed='test result: ok. 1 passed; 0 failed;' in rate_log
post_log=root/'logs/plonky3-rate-validation.retry2.log'
post_test_passed=post_log.exists() and 'test result: ok. 4 passed; 0 failed;' in post_log.read_text()
complete=not pending and not unsuccessful and case_complete and rate_test_passed and post_test_passed and test is not None and test['status']=='complete'
status='complete' if complete else ('in_progress' if pending or test is None or test['status']=='running' else 'finished_with_failures')
result={'status':status,'generated_utc':datetime.datetime.now(datetime.timezone.utc).isoformat(),
        'requested_groups':len(requested),'completed_requested_groups':sum(effective.get(name,{}).get('status')=='complete' for name in requested),
        'complete_cases':counts,'measured_proof_repetitions':5*sum(counts.values()),
        'pending_groups':sorted(pending),'unsuccessful_groups':sorted(unsuccessful),
        'missing':analysis['missing'],'incomplete':analysis['incomplete'],
        'latest_group_results':effective,'all_attempts':attempts,
        'rate_validation_log':'logs/rate-validation.log',
        'requested_rate_test_passed':rate_test_passed,
        'additional_plonky3_tests_passed':post_test_passed,
        'additional_plonky3_validation':test,
        'interpretation_limits':['Native backend security-accounting scopes differ.',
          'Background CPU activity and memory compression/swap affect timing and RSS interpretation.',
          'Wide and wrapping BitZ drivers use different inputs and timing/memory policies.']}
(root/'completion.json').write_text(json.dumps(result,indent=2)+'\n')
print(json.dumps({k:result[k] for k in ('status','completed_requested_groups','complete_cases','measured_proof_repetitions','pending_groups','unsuccessful_groups')},indent=2))
sys.exit(0 if complete else 2)

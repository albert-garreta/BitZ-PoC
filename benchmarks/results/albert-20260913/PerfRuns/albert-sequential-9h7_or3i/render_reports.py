#!/usr/bin/env python3
"""Build and validate derived phase reports after the native matrix completes."""
import json
from pathlib import Path
import subprocess
import sys

root=Path(__file__).resolve().parent
data=json.loads((root/'analysis.json').read_text())
selected=sys.argv[1:] or ['binius-focused','all-provers']
expected={'binius-focused':48,'all-provers':104}
assert all(f in expected for f in selected)
for family in selected:
    assert sum(r['family']==family for r in data['native'])==expected[family], f'{family} must complete first'
jobs=json.loads((root/'commands.json').read_text())
derived=root/'derived-traces'
derived.mkdir(exist_ok=True)
families={f:[] for f in selected}
for job in jobs:
    if job['kind']!='native': continue
    family='binius-focused' if job['name'].startswith('binius-pcs-') else 'all-provers'
    if family not in families: continue
    source=Path(job['environment']['F2Z_MUL_COMPARE_OUTPUT_DIR'])/'trace.jsonl'
    destination=derived/(job['name']+'.trace.jsonl')
    subprocess.run([sys.executable,str(root/'normalize_trace.py'),str(source),str(destination)],check=True)
    families[family].append(str(destination))
tool='/Users/johnwu/.ai-agent-army/skills/zk-proof-profiler/scripts/zk_trace.py'
for family,paths in families.items():
    directory=root/('focused-intervals' if family=='binius-focused' else 'all-provers-intervals')
    with (root/'logs'/f'{family}-trace-validation.log').open('w') as log:
        subprocess.run([sys.executable,tool,'validate',*paths],stdout=log,stderr=subprocess.STDOUT,check=True)
    subprocess.run([sys.executable,tool,'report',*paths,'--out-dir',str(directory),'--title',f'u32 multiplication modulo 2^32 — sequential rerun ({family})'],check=True)
    subprocess.run([sys.executable,str(root/'validate_report_metrics.py'),family,str(directory)],check=True)
print('Selected native phase reports passed structural and numerical validation.')

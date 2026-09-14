#!/usr/bin/env python3
"""Report frozen repair results, including failures and explicit retentions."""
import argparse
import hashlib
import json
import re
from collections import Counter
from pathlib import Path

from gate import evaluate

HERE = Path(__file__).resolve().parent


def read(path):
    meta = json.loads((path / 'metadata.json').read_text())
    assert meta['status'] == 'complete'
    for file, key in [('summary.json','summary_sha256'),('required_cases.json','manifest_sha256')]:
        assert hashlib.sha256((path/file).read_bytes()).hexdigest() == meta[key]
    return meta, json.loads((path/'required_cases.json').read_text()), json.loads((path/'summary.json').read_text())


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--one',required=True,type=Path)
    parser.add_argument('--ten',required=True,type=Path)
    args=parser.parse_args()
    sources={1:args.one.resolve(),10:args.ten.resolve()}
    inputs={t:read(p) for t,p in sources.items()}
    assert inputs[1][0]['binary_sha256'] == inputs[10][0]['binary_sha256']
    decisions=[]
    lookup={}
    for threads,(_,spec,rows) in inputs.items():
        for r in rows: lookup[(threads,r['family'],r['size'],r['variant'])]=r
        for g in spec['families']:
            selected=g['selected'].get('aarch64',[])
            if isinstance(selected,str):selected=[selected]
            for size in g['sizes']:
                for name in selected:
                    r=lookup[(threads,g['name'],size,name)]
                    decisions.append(dict(threads=threads,family=g['name'],size=size,
                        baseline=g['baseline'],selected=name,status=r['status'],
                        retained=name==g['baseline'],median_ratio=r.get('median_ratio'),
                        p95_ratio=r.get('p95_ratio'),allocations_ok=r.get('allocations_ok')))
    (HERE/'repair_decisions.json').write_text(json.dumps(dict(binary_sha256=inputs[1][0]['binary_sha256'],decisions=decisions),indent=2)+'\n')
    lines=['# Regression repair results','',
        'ARM experiments on the M1 Max. Both thread counts use the same frozen executable. Production code and x86 are unchanged.','']
    for t in [1,10]:
        ds=[d for d in decisions if d['threads']==t]
        counts=Counter(d['status'] for d in ds)
        retained=sum(d['retained'] for d in ds)
        statuses=', '.join(f'{n} {status}' for status,n in sorted(counts.items()))
        lines.append(f'- **{t} thread(s):** {statuses} selected checks; {retained} retain the actual baseline. Retention is not a speedup.')
        issues=evaluate(sources[t])
        (sources[t]/'gate.txt').write_text('\n'.join(issues+['BLOCKED' if issues else 'PASS'])+'\n')
        lines.append(f'  Strict frozen-manifest gate: **{"BLOCKED" if issues else "PASS"}**.')
    lines+=['','## Measured improvements','',
        'Ratios use paired process medians. Primitive rows below have 1,024 terms/items; composite shapes are stated explicitly.','',
        '| Operation | Threads | Baseline / candidate (µs) | Speedup | Source |',
        '|---|---:|---:|---:|---|']
    highlights=[
        ('Two-limb checked unsigned addition',1,'opt_uint_checked_add','l2_n1024','limb_option','words.rs','limb_checked_batch'),
        ('Four-limb exact unsigned MAC',1,'opt_exact_unsigned_mac','l4_n1024','column_exact','words.rs','exact_columns'),
        ('Public sparse 9×9 polynomial dot',1,'opt_f2_poly_dot','a9_b9_sparse_n1024','row_density','binary.rs','public_rows'),
        ('Packing, logs18/18',1,'opt_pack','logs18_18','tiled_write','composite.rs','pack_tiled'),
        ('Packing + OOD, logs18/18',1,'opt_packed_ood','logs18_18','tiled_indexed_ood','composite.rs','packing_cases'),
        ('Packing + OOD, logs18/18',10,'opt_packed_ood','logs18_18','tiled_indexed_ood','composite.rs','packing_cases'),
        ('NTT, log16/32 lanes',1,'opt_ntt','log16_lanes32','half_depth','composite.rs','half_depth'),
        ('NTT, log15/8 lanes',10,'opt_ntt','log15_lanes8','half_depth','composite.rs','half_depth'),
        ('NTT, log18/32 lanes',1,'opt_ntt','log18_lanes32','half_depth','composite.rs','half_depth'),
        ('NTT, log18/32 lanes',10,'opt_ntt','log18_lanes32','half_depth','composite.rs','half_depth'),
    ]
    for label,t,f,s,v,file,fn in highlights:
        r=lookup[(t,f,s,v)];b=lookup[(t,f,s,r['baseline'])]
        path=HERE/'src/campaign/candidates'/file;source=path.read_text()
        match=re.search(r'\bfn '+fn+r'\b',source);assert match
        line=source[:match.start()].count('\n')+1
        ratio=f'{1/r["median_ratio"]:.2f}×' if r['status']=='pass' else r['status']
        lines.append(f'| {label} | {t} | {b["median_ns"]/1000:.3f} / {r["median_ns"]/1000:.3f} | {ratio} | [{file}:{line}]({path}:{line}) |')
    lines+=['','## Previously failing cases','',
        '| Threads | Workload | Previous time ratio | New choice | New time ratio | Result |',
        '|---:|---|---:|---|---:|---|']
    old=json.loads((HERE/'optimization_decisions.json').read_text())['decisions']
    for prior in old:
        for s in prior['selected']:
            if s['status']!='regression':continue
            ds=[d for d in decisions if (d['threads'],d['family'],d['size'])==(prior['threads'],prior['family'],prior['size'])]
            for d in ds:
                choice='retain '+d['selected'] if d['retained'] else d['selected']
                lines.append(f'| {d["threads"]} | {d["family"]}/{d["size"]} | {s["median_ratio"]:.3f}× | {choice} | {d["median_ratio"]:.3f}× | {d["status"]} |')
    lines+=['','## Evidence and limits','',
        '- [Repair changes and allocation-audit correction](REPAIR_NOTES.md). The audit still measures maximum allocations for one call; it now samples 64 calls to cover periodic Rayon queue allocations.',
        '- [Every selected decision](repair_decisions.json). Baseline retentions and unresolved selections are explicit.',
        '- [Frozen source and executable archive](results/repair-arm-02/archive.json) and [ARM instruction review](results/repair-arm-02/arm-audit/REVIEW.md). Replay commands are in the repair notes.',
        '- Public polynomial variants retain a variable-time public-input contract. The fixed-schedule private-input kernel remains separate; the faster public path does not establish a private-input speedup.',
        '- All-zero, alternating, dense-prefix and sparse-prefix polynomial workloads are included, alongside the original dense/sparse fixtures.',
        '- Correctness runs before timing. These are operation-level results, not an end-to-end prover or x86 performance claim.']
    for t,p in sources.items():
        lines.append(f'- [{t}-thread measurements]({p}/measurements.md), including rejected diagnostic variants, with raw logs and hashes in the same directory.')
    (HERE/'REPAIR_RESULTS.md').write_text('\n'.join(lines)+'\n')
    print(HERE/'REPAIR_RESULTS.md')


if __name__=='__main__':main()

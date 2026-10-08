import importlib.util,json,statistics
from pathlib import Path
root=Path('target/falcon-integer-merge')
import subprocess,sys
if not (root/'arithmetic-512-confirmation/results.json').exists():
    subprocess.run([sys.executable,str(root/'arith_confirm.py')],check=True)
spec=importlib.util.spec_from_file_location('comparison', 'scripts/compare_falcon_h2p.py')
comparison=importlib.util.module_from_spec(spec)
spec.loader.exec_module(comparison)
rows=[]
stable=[]
for folder,security in [('measurements',100),('full-validation',128)]:
    records=json.loads((root/folder/'results.json').read_text())
    reference=json.loads((root/(folder+'-rebalanced')/'results.json').read_text())
    for family in ['full','arithmetic']:
        for degree in [512,1024]:
            pair={r['revision']:r for r in records if r.get('family','full')==family and r['degree']==degree}
            if not pair: continue
            assert set(pair)=={'baseline','candidate'}
            old,new=pair['baseline']['samples'],pair['candidate']['samples']
            assert all(s['verified'] for s in old+new)
            assert len({s['input_digest'] for s in old+new})==1
            keys=['total_prover_ms','proof_prove_ms','proof_verify_ms'] if family=='full' else ['total_prover_ms','prove_ms','verify_ms']
            timings={k:comparison.timing_comparison([s[k] for s in old],[s[k] for s in new]) for k in keys}
            prior=next(r for r in reference if r['revision']=='candidate' and r.get('family','full')==family and r['degree']==degree)['samples']
            identity_keys=[k for k in new[0] if 'digest' in k or 'root' in k]
            for key in identity_keys:
                assert len({json.dumps(s[key],sort_keys=True) for s in prior+new})==1,(family,degree,security,key)
            stable.append({'family':family,'degree':degree,'security':security,'unchanged_keys':identity_keys,'proof_digest_available':'proof_debug_digest' in identity_keys})
            rows.append({'family':family,'degree':degree,'security':security,'samples':len(new),'timings':timings,
                         'baseline_payload_bytes':statistics.median(s['proof_payload_bytes'] for s in old),
                         'candidate_payload_bytes':statistics.median(s['proof_payload_bytes'] for s in new)})
report={'scope':'1024 signatures, 8 threads, seed 42, Mac M1 Max; one warmup per case',
        'baseline_commit':'95b79124f1958e3efb9ca9078e330542665cb1c3',
        'timing_definition':'total_prover_ms includes witness commitment and proving; excludes input setup, preparation and verification',
        'confidence_scope':'independent resampling of timing samples on a fixed input; not a multi-seed grinding comparison',
        'full_gate_passed':all(r['timings'][key]['pass'] for r in rows if r['family']=='full' for key in ['total_prover_ms','proof_verify_ms']),
        'arithmetic_gate_passed':all(r['timings'][key]['pass'] for r in rows if r['family']=='arithmetic' for key in ['total_prover_ms','verify_ms']),
        'native_optimization_proof_identity':stable,'cases':rows}
(root/'comparison.json').write_text(json.dumps(report,indent=2)+'\n')
for row in rows:
    t=row['timings']['total_prover_ms']
    print(row['family'],row['degree'],row['security'],'total',round(t['baseline_median_ms'],2),round(t['candidate_median_ms'],2),'change %',round(100*(t['median_ratio']-1),2),'upper',round(t['independent_bootstrap_upper_95'],4))
print('FULL GATE',report['full_gate_passed'])
print('ARITHMETIC GATE',report['arithmetic_gate_passed'])

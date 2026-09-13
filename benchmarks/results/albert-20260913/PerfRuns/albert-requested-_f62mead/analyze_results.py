#!/usr/bin/env python3
"""Read original records, reject partial cases, and summarize paired measurements."""
import csv
import json
import math
from pathlib import Path
import statistics
from collections import defaultdict

OUT=Path(__file__).resolve().parent
METRICS=('witness_ms','commit_ms','piop_ms','opening_ms','pcs_ms','online_prover_ms',
         'witness_to_proof_ms','verify_ms','proof_bytes')

def read_lines(path):
    return [json.loads(line) for line in path.read_text().splitlines() if line.strip()]

def write_csv(path,rows,fields):
    with path.open('w',newline='') as stream:
        writer=csv.DictWriter(stream,fieldnames=fields,extrasaction='ignore')
        writer.writeheader()
        writer.writerows(rows)

def percentile(values,q):
    values=sorted(values)
    at=(len(values)-1)*q
    lo=int(at)
    return values[lo]+(values[min(lo+1,len(values)-1)]-values[lo])*(at-lo)

native=[]
incomplete=[]
for folder in [*sorted((OUT/'binius-pcs').glob('rate*')), *sorted((OUT/'all-provers').glob('u32-mod32-*'))]:
    if not folder.is_dir() or not (folder/'samples.jsonl').exists():
        continue
    rate=int(folder.name[-1]) if folder.name[-1].isdigit() else None
    family='binius-focused' if folder.parent.name=='binius-pcs' else 'all-provers'
    grouped=defaultdict(list)
    for row in read_lines(folder/'samples.jsonl'):
        grouped[(row['backend'],row['log_multiplications'])].append(row)
    memories={(r['backend'],r['log_multiplications']):r for r in read_lines(folder/'memory.jsonl')}
    for (backend,exponent),rows in grouped.items():
        expected=[{'kind':'warmup','index':0}]+[{'kind':'sample','index':i} for i in range(5)]
        if [r['trial'] for r in rows]!=expected:
            incomplete.append(dict(source=str(folder),backend=backend,exponent=exponent,records=len(rows)))
            continue
        first=rows[0]
        assert all(r['proof_verified'] is True and r['threads']==8 and r['multiplications']==1<<exponent for r in rows)
        assert all(r['config']==first['config'] and r['corpus_digest']==first['corpus_digest'] for r in rows)
        memory=memories[(backend,exponent)]
        assert memory['proof_verified'] is True and memory['corpus_digest']==first['corpus_digest']
        # Only latency records add the explanatory encoding label after setup.
        protocol_config=lambda cfg:{k:v for k,v in cfg.items() if k!='proof_size_encoding'}
        assert protocol_config(memory['config'])==protocol_config(first['config']), 'memory/latency configuration mismatch'
        assert all(memory['proof_bytes']==r['metrics']['proof_bytes'] for r in rows), 'memory/latency proof size mismatch'
        cfg=first['config']
        actual=(cfg['ligerito']['configuration']['levels'][0]['log_inv_rate'] if backend=='f2z'
                else cfg.get('log_inv_rate'))
        if backend!='limber':
            assert actual==rate,(folder,backend,actual,rate)
        if backend=='binius64':
            expected_queries=math.ceil(100/-math.log2((1+2**-rate)/2))
            assert cfg['fri_query_target_bits']==100 and cfg['fri_queries']==expected_queries
        elif backend=='binius64-ligerito':
            assert cfg['whole_protocol_bits']>=100
            assert all(o['configuration']['levels'][0]['log_inv_rate']==rate for o in cfg['oracles'])
        elif backend=='plonky3-fri':
            assert cfg['proven_bits']>=100
        elif backend=='f2z':
            assert cfg['target_bits']==100 and cfg['word_bits']==1
            assert cfg['security_scope']=='round-by-round-economic' and cfg['modeled_min_bits']>=100
        result=dict(family=family,rate=f'1/{2**rate}' if rate else 'native',log_inv_rate=rate,backend=backend,
                    log_multiplications=exponent,multiplications=1<<exponent,samples=5,warmups=1,
                    source=str(folder.relative_to(OUT)),corpus_digest=first['corpus_digest'],setup_ms=first['setup_ms'],
                    peak_rss_bytes=memory['peak_rss_bytes'],config=cfg)
        for metric in METRICS:
            values=[r['metrics'][metric] for r in rows[1:]]
            assert all(isinstance(v,(int,float)) and math.isfinite(v) and v>=0 for v in values)
            result[metric]=statistics.median(values)
            result[metric+'_p10']=percentile(values,.1)
            result[metric+'_p90']=percentile(values,.9)
        native.append(result)
corpora=defaultdict(set)
for row in native:
    corpora[row['log_multiplications']].add(row['corpus_digest'])
assert all(len(v)==1 for v in corpora.values()), 'cross-backend corpus mismatch'
paired=[]
for family in ('binius-focused','all-provers'):
    for rate in (1,2,3):
        for exponent in range(15,23):
            rows={r['backend']:r for r in native if r['family']==family and r['log_inv_rate']==rate and r['log_multiplications']==exponent}
            if {'binius64','binius64-ligerito'}<=rows.keys():
                a,b=rows['binius64'],rows['binius64-ligerito']
                assert a['config']['word_constraints']==b['config']['word_constraints']
                result=dict(family=family,rate=f'1/{2**rate}',log_multiplications=exponent)
                for metric in (*METRICS,'setup_ms','peak_rss_bytes'):
                    result[metric+'_change_pct']=100*(b[metric]/a[metric]-1) if a[metric] else None
                paired.append(result)
wide=[]
for path in sorted((OUT/'logs').glob('bitz-u32-wide-rate*.log')):
    for line in path.read_text().splitlines():
        if line.startswith('RESULT schema=f2z-cli-mul/2 '):
            row=dict(s.split('=',1) for s in line[7:].split())
            result=dict(rate=f"1/{2**int(row['lig_log_inv_rate'])}",log_inv_rate=int(row['lig_log_inv_rate']),
                        log_multiplications=int(row['e']),source=str(path.relative_to(OUT)))
            result.update(row)
            result['ligerito']=json.loads(bytes.fromhex(row['ligerito_hex']))
            for key in ('e','multiplications','threads','reps','lig_log_inv_rate','proof_bytes'):
                result[key]=int(row[key])
            for key in ('witness_ms','setup_ms','commit_ms','prove_ms','verify_ms','prove_peak_mb','s2_project_ms','s3_piop_ms','s4_bitify_ms','s5_open_ms'):
                result[key]=float(row[key])
            result['witness_to_proof_ms_approx']=result['witness_ms']+result['prove_ms']
            # The CLI reports payload/opening bytes; the shared runner also counts
            # the 32-byte BLAKE3 commitment root. Preserve both conventions.
            result['proof_material_bytes_with_root']=result['proof_bytes']+32
            result['tracked_heap_peak_mib']=result['prove_peak_mb']
            assert result['threads']==8 and result['reps']==5
            assert int(row['lambda'])==100 and int(row['W'])==1
            assert result['ligerito']['configuration']['levels'][0]['log_inv_rate']==result['log_inv_rate']
            wide.append(result)
sha=[]
for folder in sorted((OUT/'all-provers').glob('sha256-ecdsa-rate*')):
    for path in sorted(folder.glob('*.result.json')):
        data=json.loads(path.read_text())
        if data['status']!='complete':
            incomplete.append(dict(source=str(path.relative_to(OUT)),status=data['status']))
            continue
        rows=[r for r in data['rows'] if r['trial']=='sample']
        assert len(rows)==5 and all(r['verified'] and r['compressions']==128 and r['message_bytes']==8128 for r in rows)
        assert len({r['fixture_id'] for r in rows})==1
        r=rows[0]
        security=r['security']
        rate=int(folder.name[-1])
        actual=security['log_inv_rate'] if r['method']=='binius64' else security['ligerito']['configuration']['levels'][0]['log_inv_rate']
        assert actual==rate
        if r['method']=='binius64':
            assert security['fri_query_target_bits']==100
            assert security['fri_queries']==math.ceil(100/-math.log2((1+2**-rate)/2))
        else:
            assert security['economic_bits']>=100
        result=dict(method=r['method'],rate=f'1/{2**rate}',log_inv_rate=rate,samples=5,warmups=1,
                    source=str(path.relative_to(OUT)),fixture_id=r['fixture_id'],security=security,
                    peak_rss_bytes=data['peak_rss_bytes'])
        for metric in ('setup_ms','witness_ms','commit_ms','protocol_ms','prove_ms','witness_to_proof_ms','e2e_prover_ms','verify_ms','opening_ms','proof_material_bytes'):
            values=[s[metric] for s in rows]
            result[metric]=statistics.median(values)
            result[metric+'_p10']=percentile(values,.1)
            result[metric+'_p90']=percentile(values,.9)
        sha.append(result)
assert len({r['fixture_id'] for r in sha})<=1,'SHA fixture mismatch'
wide_vs_wrapping=[]
for row in wide:
    wrapping=next((r for r in native if r['family']=='all-provers' and r['backend']=='f2z'
                   and r['log_inv_rate']==row['log_inv_rate'] and r['log_multiplications']==row['log_multiplications']),None)
    if wrapping is None:
        continue
    assert row['ligerito']==wrapping['config']['ligerito'], 'wide/wrapping Ligerito configuration mismatch'
    assert wrapping['config']['geometry']=={'t':int(row['t']),'s':int(row['s']),'word_bits':int(row['W'])}
    wide_vs_wrapping.append(dict(rate=row['rate'],log_multiplications=row['log_multiplications'],
        same_ligerito_configuration=True,same_geometry=True,
        cli_proof_bytes_including_root=row['proof_material_bytes_with_root'],
        wrapping_proof_bytes=wrapping['proof_bytes'],
        proof_byte_difference=row['proof_material_bytes_with_root']-wrapping['proof_bytes'],
        cli_prove_ms=row['prove_ms'],wrapping_online_prover_ms=wrapping['online_prover_ms'],
        timing_comparison='Different input generators, witness handling, process/cooldown and allocator policies; not paired timing evidence.'))
expected_native={(family,rate,b,e) for family,backends in [('binius-focused',['binius64','binius64-ligerito']),('all-provers',['f2z','binius64','binius64-ligerito','plonky3-fri'])] for rate in (1,2,3) for b in backends for e in range(15,23)}
expected_native|={('all-provers',None,'limber',e) for e in range(15,23)}
present_native={(r['family'],r['log_inv_rate'],r['backend'],r['log_multiplications']) for r in native}
assert len(present_native)==len(native), 'duplicate native cases'
assert len({(r['log_inv_rate'],r['log_multiplications']) for r in wide})==len(wide), 'duplicate wide cases'
assert len({(r['method'],r['log_inv_rate']) for r in sha})==len(sha), 'duplicate SHA cases'
missing=dict(native=sorted(expected_native-present_native,key=str),wide=sorted({(r,e) for r in (1,2,3) for e in range(15,23)}-{(r['log_inv_rate'],r['log_multiplications']) for r in wide}),sha=sorted({(m,r) for m in ('f2z-split','binius64') for r in (1,2,3)}-{(r['method'],r['log_inv_rate']) for r in sha}))
(OUT/'analysis.json').write_text(json.dumps(dict(native=native,binius_changes=paired,wide=wide,sha=sha,wide_vs_wrapping=wide_vs_wrapping,incomplete=incomplete,missing=missing),indent=2)+'\n')
write_csv(OUT/'native-summary.csv',native,['family','rate','backend','log_multiplications','samples','setup_ms',*METRICS,'peak_rss_bytes','source'])
write_csv(OUT/'binius-changes.csv',paired,['family','rate','log_multiplications',*[k+'_change_pct' for k in (*METRICS,'setup_ms','peak_rss_bytes')]])
write_csv(OUT/'wide-summary.csv',wide,['rate','log_multiplications','witness_ms','commit_ms','prove_ms','verify_ms','proof_bytes','proof_material_bytes_with_root','tracked_heap_peak_mib','source'])
write_csv(OUT/'sha-summary.csv',sha,['method','rate','samples','setup_ms','witness_ms','commit_ms','prove_ms','witness_to_proof_ms','verify_ms','proof_material_bytes','peak_rss_bytes','source'])
write_csv(OUT/'wide-vs-wrapping.csv',wide_vs_wrapping,['rate','log_multiplications','same_ligerito_configuration','same_geometry','cli_proof_bytes_including_root','wrapping_proof_bytes','proof_byte_difference','cli_prove_ms','wrapping_online_prover_ms','timing_comparison'])
print(json.dumps({'complete_native_cases':len(native),'paired_binius_cases':len(paired),'complete_wide_cases':len(wide),'complete_sha_cases':len(sha),'incomplete_cases':incomplete},indent=2))

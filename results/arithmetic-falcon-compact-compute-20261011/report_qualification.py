#!/usr/bin/env python3
"""Audit and report the fixed production qualification after timings/analysis."""
import argparse
import json
from pathlib import Path

DIMS = ('degree', 'security_bits', 'batch', 'threads')

def read(path):
    return json.loads(path.read_text())

def shape_key(row):
    return tuple(row[k] for k in DIMS)

def percent(before, after):
    return 100 * (after / before - 1)

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('directory', type=Path)
    args = parser.parse_args()
    out = args.directory
    metadata = read(out / 'metadata.json')
    assert metadata['finished_unix'] is not None
    selected = metadata['selected_shapes']
    plan = metadata['plan']
    comparison = out / 'comparisons' / 'padding-vs-v7'
    summary = read(comparison / 'summary.json')
    analysis = read(comparison / 'paired-seed-analysis.json')
    identity = read(out / 'v7-reference-proof-identity.json')
    assert identity['passed'] and identity['complete'] and identity['expected_coverage_passed']
    candidate_count = changed_count = unchanged_count = 0
    layout_records = []
    for s in selected:
        degree, security, batch, threads = shape_key(s)
        capacity = 1 << (batch - 1).bit_length()
        floor = 32 if degree == 512 else 16
        changed = capacity < floor
        for seed in s['seeds']:
            stem = f'n{degree}-s{security}-b{batch}-t{threads}-seed{seed}'
            old = [json.loads(line) for line in (out / f'v7-{stem}.jsonl').read_text().splitlines()]
            new = [json.loads(line) for line in (out / f'padding-{stem}.jsonl').read_text().splitlines()]
            assert len(old) == len(new) == plan['warmup'] + plan['iterations']
            for a, b in zip(old, new):
                assert b['compute_variant'] == 'padding'
                assert b['arithmetic_capacity'] == capacity
                assert b['capacity'] == max(capacity, floor)
                assert b['logical_binding_bits'] == 32 * degree * capacity
                assert b['virtual_arithmetic_bits'] == 32 * degree * capacity
                assert b['bridge_layouts'] == [[13, (32 * degree * capacity).bit_length() - 1 - 13]]
                assert b['committed_source_bits'] == a['committed_source_bits']
                assert b['source_root'] == a['source_root']
                assert b['source_packed_bytes'] == a['source_packed_bytes']
                assert b['protocol_id'].endswith('/v8' if changed else '/v7')
                assert b['algebraic_security_bits'] >= security
                assert b['input_digest'] == a['input_digest']
                assert a['verified'] and b['verified']
                candidate_count += 1
                changed_count += changed
                unchanged_count += not changed
        layout_records.append({**{k:s[k] for k in DIMS},'arithmetic_capacity':capacity,'allocation_capacity':max(capacity,floor),'logical_binding_bits':32*degree*capacity,'new_protocol':changed})
    assert candidate_count * 2 == metadata['verified_proofs']
    assert unchanged_count == identity['checked_proofs']
    assert changed_count == identity['new_protocol_proofs_not_compared']
    audit = {'passed':True,'candidate_proofs_checked':candidate_count,'unchanged_exact_v7_proofs':unchanged_count,'changed_layout_proofs':changed_count,'all_physical_sources_and_roots_unchanged':True,'all_layouts_security_and_modes_checked':True,'layouts':layout_records}
    (out / 'production-layout-audit.json').write_text(json.dumps(audit,indent=2)+'\n')
    paired = {shape_key(r):r for r in analysis['shapes']}
    positives = []
    lines = ['# Selected production qualification', '', f"All **{metadata['verified_proofs']:,} proofs verified** across **{len(selected)} shapes**. Input digests matched for every pair. All **{unchanged_count:,} unchanged proofs** exactly matched the preserved v7 control; **{changed_count:,} proofs** used the reduced arithmetic layout and v8 transcript. Physical source bits, packed bytes and commitment roots remained unchanged for all inputs.", '', 'The shipping candidate enables arithmetic-padding reduction and uses the existing binding/bridge computation. Both variants use native release binaries with only `falcon-hybrid`. Failed experimental compact-computation pilots remain in the parent directory.', '', 'Each input had one warmup and three measured proofs, with 32 independent seeds for core-small shapes and eight for core-large and edge shapes. All timing processes ran sequentially with alternating variant order, pinned one/eight worker settings, and no concurrent builds, tests or analysis.', '', 'Positive percentages mean slower or larger. The table uses pooled-sample medians. `Proof paired 95%` is the seed-paired median bootstrap interval (10,000 resamples); intervals are descriptive and have no across-shape multiplicity adjustment. RSS is process peak memory, including preparation/external inputs.', '', '| N | Security | Batch | Threads | Total Δ | Proof Δ | Proof paired 95% | Payload Δ | Verify Δ | RSS Δ |', '|---:|---:|---:|---:|---:|---:|---|---:|---:|---:|']
    for r in summary:
        p = paired[shape_key(r)]
        values = {m:percent(r['baseline'][m],r['candidate'][m]) for m in ('total_prover_ms','prove_ms','proof_payload_bytes','verify_ms','process_peak_rss_kib')}
        interval = p['metrics']['prove_ms']['paired_seed_median_bootstrap_95_percent_interval']
        lines.append('| '+' | '.join([*(str(r[k]) for k in DIMS),f"{values['total_prover_ms']:+.2f}%",f"{values['prove_ms']:+.2f}%",f'[{interval[0]:+.2f}%, {interval[1]:+.2f}%]',f"{values['proof_payload_bytes']:+.2f}%",f"{values['verify_ms']:+.2f}%",f"{values['process_peak_rss_kib']:+.2f}%"])+ ' |')
        for m in ('total_prover_ms','prove_ms'):
            stats = p['metrics'][m]
            if stats['paired_seed_median_bootstrap_95_percent_interval'][0] > 0:
                positives.append({**{k:r[k] for k in DIMS},'metric':m,'paired_seed_median_delta_percent':stats['paired_seed_median_delta_percent'],'interval':stats['paired_seed_median_bootstrap_95_percent_interval']})
    gate = read(comparison / 'regression-gate.json')
    lines += ['',f"The zero-threshold observed gate has **{len(gate['regressions'])} positive metric entries**. **{len(positives)} entries** also have a positive lower endpoint in the descriptive paired-median interval. Positive observations remain in the evidence; they cannot be erased by gains in other shapes. Acceptance or confirmation decisions should consider both proving metrics and the complete paired analysis.",'','See [paired analysis](comparisons/padding-vs-v7/paired-seed-analysis.json), [observed gate](comparisons/padding-vs-v7/regression-gate.json), [layout audit](production-layout-audit.json), [exact v7 identity audit](v7-reference-proof-identity.json), and [run metadata](metadata.json). Raw per-input outputs, commands, sample timings and input hashes are preserved beside this report.']
    (out / 'README.md').write_text('\n'.join(lines)+'\n')
    (out / 'positive-paired-intervals.json').write_text(json.dumps(positives,indent=2)+'\n')
    print(json.dumps({'verified_proofs':metadata['verified_proofs'],'unchanged_exact_proofs':unchanged_count,'new_layout_proofs':changed_count,'positive_observed_metrics':len(gate['regressions']),'positive_paired_intervals':positives},indent=2))

if __name__ == '__main__':
    main()

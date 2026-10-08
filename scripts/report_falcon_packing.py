#!/usr/bin/env python3
"""Render immutable campaign summaries without merging historical baselines."""
import argparse
import html
import json
from pathlib import Path


def table(headers, rows):
    return '<table><thead><tr>' + ''.join(f'<th>{html.escape(str(h))}</th>' for h in headers) + '</tr></thead><tbody>' + ''.join(
        '<tr>' + ''.join(f'<td>{html.escape(str(c))}</td>' for c in row) + '</tr>' for row in rows) + '</tbody></table>'


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('root', type=Path)
    args = parser.parse_args()
    root = args.root.resolve()
    historical = json.loads((root.parent / 'falcon-bitz-witness-8t-20261008/summary.json').read_text())
    controls = json.loads((root.parent / 'falcon-bottlenecks-20261008/exact-runs.json').read_text())
    components = json.loads((root.parent / 'falcon-bottlenecks-20261008/exact-analysis.json').read_text())
    campaigns = []
    for path in sorted(root.glob('*/manifest.json')):
        metadata = json.loads(path.read_text())
        summary = path.parent / 'summary.json'
        if metadata.get('complete') and summary.exists():
            campaigns.append({'name': path.parent.name, 'metadata': metadata,
                              'summary': json.loads(summary.read_text())})
    policy = json.loads((root / 'dispatch.json').read_text()) if (root / 'dispatch.json').exists() else None
    analysis = {'status': policy.get('status', 'in progress') if policy else 'in progress', 'dispatch': policy,
                'historical_throughput': historical, 'historical_untraced_controls': [r for r in controls if not r['traced']],
                'historical_components': components,
                'campaigns': campaigns, 'verified_benchmark_proofs': sum(c['metadata'].get('verified_proofs', 0) for c in campaigns)}
    (root / 'analysis.json').write_text(json.dumps(analysis, indent=2) + '\n')
    parts = ['<h1>Arithmetic Falcon packing</h1>',
             '<p>Apple M1 Max · 10 physical / 10 logical CPUs · local only · 1,024 signatures · 1, 2, 4, and 8 workers · security targets 100 and 128. macOS scheduler placement; no CPU affinity pinning.</p>',
             '<p>No CPU-idle wait. Baseline and candidate processes run sequentially in alternating order. Earlier broad screening is retained separately; qualification was narrowed to batch 1,024 at the user’s request.</p>',
             '<p>Direct signed15 word stores preserve the aligned16 source. Parallel signature packing uses disjoint column slices. Full Falcon packing and witness binding are unchanged.</p>',
             '<h2>Production selection</h2>',
             '<pre>' + html.escape(json.dumps(policy, indent=2) if policy else 'Qualification in progress; automatic packing remains on the original path.') + '</pre>',
             '<h2>Historical throughput campaign</h2>']
    fresh = [c for c in campaigns if c['metadata']['mode'] == 'e2e']
    if fresh:
        final = [c for c in fresh if c['name'] == 'final-production']
        latest = final[0] if final else max(fresh, key=lambda c: c['metadata']['finished'])
        highlights = []
        for row in latest['summary']:
            if row['cell'][1:3] != [1024, 8]:
                continue
            value = row['metrics']['total_prover_ms']
            highlights.append([row['cell'][0], row['cell'][3], f"{value['baseline']['median']:.3f}",
                               f"{value['candidate']['median']:.3f}", f"{100*(1-value['ratio']):.1f}%",
                               value['status']])
        if highlights:
            parts.insert(5, '<h2>Latest fresh comparison: 1,024 signatures, eight workers</h2><p>'
                         + html.escape(latest['name']) + '</p>'
                         + table(['Degree', 'Security', 'Baseline total ms', 'Candidate total ms', 'Reduction', 'Evidence'], highlights))
    parts.append(table(['Workload', 'Total ms', 'Proving ms', 'Verify ms', 'Proof bytes', 'Peak RSS MiB'], [
        [r['name'], f"{r['total_prover_ms']:.3f}", f"{r['prove_ms']:.3f}", f"{r['verify_ms']:.3f}",
         r['proof_bytes'], f"{r['process_peak_rss_bytes']/2**20:.2f}"] for r in historical if r['family'] != 'bitz']))
    parts.append('<h2>Historical untraced profiling controls</h2>')
    parts.append(table(['Workload', 'Commit ms', 'Proving ms', 'Total ms', 'Verify ms'], [
        [r['name'], *[f"{r['medians_ms'][k]:.3f}" for k in ['commit', 'prove', 'total_prover', 'verify']]]
        for r in controls if not r['traced']]))
    parts.append('<h2>Historical component profiles</h2><p>Diagnostic medians include tracing overhead and are not performance acceptance measurements. Each PCS total is an interval union calculated within each trial before aggregation.</p>')
    parts.append(table(['Workload', 'Packing ms', 'Binding ms', 'PCS union ms', 'PCS share'], [
        [p['name'], f"{p['aggregate']['commit_buckets']['witness_packing']['median_ms']:.3f}",
         f"{p['aggregate']['prove_buckets']['witness_binding']['median_ms']:.3f}",
         f"{p['aggregate']['pcs']['including_commitment_union']['median_ms']:.3f}",
         f"{p['aggregate']['pcs']['including_commitment_union']['median_percent_of_total']:.1f}%"]
        for p in components['profiles']]))
    parts.append('<h2>Fresh paired comparisons</h2><p>Times below are medians of process medians. Ratio confidence intervals use paired process observations. Screening and confirmation remain separate. An apparent secondary regression requires an independent repeat before it is treated as confirmed. Peak-memory acceptance uses end-to-end processes; constructor microbenchmarks repeat the faster paths more often to obtain stable timings.</p>')
    diagnostics = root / 'diagnostics/analysis.json'
    if diagnostics.exists():
        profiles = json.loads(diagnostics.read_text())['profiles']
        parts.append('<h2>Fresh component diagnostics</h2><p>One warmup and five measured samples per trace, batch 1,024, eight workers, security 100. These runs include tracing overhead and remain separate from acceptance. <a href="diagnostics/timeline/intervals.html">Interactive operation timeline</a>.</p>')
        parts.append(table(['Workload', 'Packing ms', 'Binding ms', 'PCS union ms', 'Commit + prove ms'], [
            [p['name'], *[f"{p['aggregate'][group][key]['median_ms']:.3f}" for group, key in (
                ('commit_buckets', 'witness_packing'), ('prove_buckets', 'witness_binding'),
                ('pcs', 'including_commitment_union'), ('boundaries', 'commit_plus_prove_union'))]]
            for p in profiles]))
    parts.append('<label>Filter rows <input id="filter" placeholder="e.g. 1024 / 1024 / 8 / 100" oninput="filterRows()"></label>')
    for campaign in campaigns:
        meta = campaign['metadata']
        parts.append(f"<details><summary>{html.escape(campaign['name'])}</summary><p>{html.escape(meta['variants'])}; {meta['blocks']} paired blocks, one warmup and {meta['samples']} measured trials per process.</p>")
        rows = []
        for row in campaign['summary']:
            for metric, values in row['metrics'].items():
                scale = 2**20 if metric == 'peak_rss_bytes' else 1
                interval = values['interval95']
                rows.append([' / '.join(map(str, row['cell'])), metric,
                             f"{values['baseline']['median']/scale:.4f}", f"{values['candidate']['median']/scale:.4f}",
                             f"{values['baseline']['p05']/scale:.4f}–{values['baseline']['p95']/scale:.4f}",
                             f"{values['candidate']['p05']/scale:.4f}–{values['candidate']['p95']/scale:.4f}",
                             f"{values['ratio']:.4f} [{interval[0]:.4f}, {interval[1]:.4f}]", values['status']])
        parts.append(table(['Degree / batch / threads / security', 'Metric (ms or MiB)', 'Baseline', 'Candidate', 'Baseline p05–p95', 'Candidate p05–p95', 'Candidate/baseline [95% CI]', 'Evidence'], rows))
        parts.append('</details>')
    parts.append('<h2>Unchanged raw BitZ reference</h2><p>Raw BitZ bridges every input bit; full Falcon bridges only its arithmetic witness. These totals are not lower bounds for Falcon. Falcon sizes are stored payload; BitZ sizes use its encoded-byte convention.</p>')
    parts.append(table(['Workload', 'Commit ms', 'Proving ms', 'Total ms', 'Proof bytes', 'Peak RSS MiB'], [
        [r['name'], f"{r['commit_ms']:.3f}", f"{r['prove_ms']:.3f}", f"{r['total_prover_ms']:.3f}",
         r['proof_bytes'], f"{r['process_peak_rss_bytes']/2**20:.2f}"] for r in historical if r['family'] == 'bitz']))
    parts.append(f"<p>Verified benchmark proofs in completed fresh campaigns: {analysis['verified_benchmark_proofs']:,}. Constructor-only campaigns do not produce proofs. Source hashes, executable hashes, inputs, commands, and raw logs accompany each campaign. Process RSS includes preparation, inputs, warmups, proving, and verification.</p>")
    document = '''<!doctype html><html><meta charset="utf-8"><title>Falcon packing qualification</title><style>
body{font:15px system-ui;max-width:1450px;margin:32px auto;padding:0 24px;color:#182338;background:#f7f9fc}h1,h2,h3{color:#103966}h2{margin-top:36px}table{border-collapse:collapse;width:100%;background:white;margin:16px 0;font-variant-numeric:tabular-nums}th,td{padding:8px 10px;text-align:left;border-bottom:1px solid #dbe2ec}th{background:#e6eef8;position:sticky;top:0}td:not(:first-child){white-space:nowrap}p{max-width:1000px;line-height:1.55}pre{background:#e6eef8;padding:18px;overflow:auto}input{padding:8px;width:320px}thead{font-size:13px}</style><body>'''
    document += '\n'.join(parts)
    document += '''<script>function filterRows(){let q=document.querySelector('#filter').value.toLowerCase();document.querySelectorAll('tbody tr').forEach(r=>r.hidden=!r.innerText.toLowerCase().includes(q))}</script></body></html>'''
    (root / 'report.html').write_text(document)


if __name__ == '__main__':
    main()

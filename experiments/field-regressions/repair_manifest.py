#!/usr/bin/env python3
"""Write the pre-confirmation choices for this repair revision; retain controls."""
import copy
import json
from pathlib import Path

HERE = Path(__file__).resolve().parent
COMPOSITES = {'opt_ood', 'opt_ood_reuse', 'opt_ntt', 'opt_pack', 'opt_packed_ood'}


def create(threads):
    spec = json.loads((HERE / 'optimization_cases.json').read_text())
    old = json.loads((HERE / 'results/optimization-arm-01/summary.json').read_text())
    previous = {(r['family'], r['size'], r['variant']): r for r in old}
    groups = []
    for group in spec['families']:
        family = group['name']
        if threads == 10 and family not in COMPOSITES:
            continue
        sizes = list(group['sizes'])
        if family == 'opt_f2_poly_dot':
            sizes += [f'a{a}_b{b}_{kind}_n{n}' for a,b in [(1,1),(3,7),(9,9)]
                      for kind in ['zero','dense_prefix','sparse_prefix','alternating'] for n in [16,1024]]
        for size in sizes:
            g = dict(copy.deepcopy(group), sizes=[size])
            extra = []
            if family == 'opt_integer_mac' and size.startswith('l1_'):
                extra = ['native_word']
            elif family in ('opt_uint_add', 'opt_uint_sub'):
                extra = ['native_batch']
            elif family in ('opt_uint_checked_add', 'opt_uint_checked_sub'):
                extra = ['native_option', 'limb_option']
            elif family in ('opt_int_checked_add', 'opt_int_checked_sub'):
                extra = ['native_option']
            elif family == 'opt_exact_unsigned_mac':
                extra = ['column_exact']
            elif family == 'opt_f2_poly_dot':
                extra = ['public_fused', 'public_adaptive', 'row_density']
            elif family == 'opt_ood':
                extra = ['indexed_products', 'collect_products', 'vector_products', 'prepared_products']
            elif family == 'opt_ood_reuse':
                extra = ['indexed_scratch']
            elif family == 'opt_ntt':
                extra = ['tiled_half', 'half_depth']
            elif family == 'opt_pack':
                extra = ['tiled_write']
            elif family == 'opt_packed_ood':
                extra = ['tiled_indexed_ood']
            g['variants'] += extra
            chosen = g['selected'].get('aarch64', [])
            if isinstance(chosen, str):
                chosen = [chosen]
            # For unchanged experiments only keep established improvements
            # with headroom. A baseline retention is not a new speedup.
            chosen = [v for v in chosen if (r := previous.get((family, size, v)))
                      and r['status'] == 'pass' and r['median_ratio'] < .97
                      and max(r['per_run_medians']) < .99]
            reason = 'previous confirmed improvement with headroom' if chosen else 'retain existing implementation; no dependable replacement advantage'
            if family == 'opt_integer_mac' and size.startswith('l1_'):
                chosen = []
            if family in ('opt_uint_add', 'opt_uint_sub'):
                chosen = [] if size.startswith('l1_') or (family == 'opt_uint_sub' and not size.startswith('l2_')) else ['native_batch']
            if family == 'opt_uint_checked_add':
                chosen = [] if size == 'l1_n1024' else ['limb_option']
            if family == 'opt_uint_checked_sub':
                chosen = [] if size == 'l1_n1024' else ['native_option']
            if family == 'opt_exact_unsigned_mac':
                chosen = ['column_exact']
            if family == 'opt_f2_poly_dot':
                retain = size.startswith('a3_b7_') or (size.startswith('a1_b1_') and size.endswith('_n16'))
                chosen = [] if retain else ['row_density']
                reason = 'explicit public-input contract; fixed_schedule remains the separate private-input kernel'
            if family == 'opt_ood':
                chosen = [] if threads == 10 and size != 'log10' else ['reuse_products']
            if family == 'opt_ood_reuse':
                chosen = ['scratch'] if size == 'log10' else []
            if family == 'opt_ntt':
                retain = size == 'log8_lanes1' or (threads == 10 and size in ['log15_lanes32','log16_lanes32'])
                chosen = [] if retain else ['half_depth']
            if family == 'opt_pack':
                chosen = ['tiled_write']
            if family == 'opt_packed_ood':
                chosen = ['tiled_indexed_ood']
            if chosen and any(v in extra for v in chosen):
                reason = 'repair chosen from development probes before fresh confirmation' if family != 'opt_f2_poly_dot' else reason
            if not chosen:
                reason = 'retain actual baseline; no new replacement or claimed speedup'
                chosen = [g['baseline']]
            g['selected'] = {'aarch64': chosen}
            g['selection_reason'] = reason
            groups.append(g)
    spec.update(families=groups, revision='regression-repair-2' if threads == 1 else 'regression-repair-3', threads=threads,
                allocation_audit_passes=64,
                selection_note='Frozen before confirmation. Baseline retentions are explicit, not candidate speedups. No timing/privacy contract downgrade.')
    path = HERE / f'repair_cases_arm_{threads}.json'
    path.write_text(json.dumps(spec, indent=2) + '\n')
    print(f'{path}: {len(groups)} workloads')


if __name__ == '__main__':
    create(1)
    create(10)

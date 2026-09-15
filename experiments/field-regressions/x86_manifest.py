"""Explicit Ryzen arithmetic coverage; old ARM manifests remain unchanged."""
import copy
import json
from pathlib import Path

HERE = Path(__file__).resolve().parent
ARRAY = [16, 1024, 65536, 1048576]
SHORT = [1, 3, 7, 17]
NTT = ['log8_lanes1','log8_lanes32','log12_lanes8','log15_lanes32',
       'log16_lanes32','log17_lanes32','log18_lanes32']
REPAIRS = {'x86_products','gf_round_single','gf_round_two','integer_full_mac','prime_dot','ntt'}


def build(features, stream_n=None, scope='all', threads=1):
    if scope in {'candidate-repairs', 'candidate-repairs-consumers'}:
        spec = build(features, scope='comprehensive', threads=threads)
        definitions = [
            ('opt_ood', [f'log{n}' for n in [9,10,11,15,16,17,18]],
             'production_blocked', ['production_blocked','tiled_grouped','tiled_256','prepared_products']),
            ('opt_ntt', [f'log{log}_lanes{lanes}' for log,lanes in
                         [(8,1),(8,32),(10,32),(11,32),(11,8),(12,8),(13,8),(15,32),(18,32)]],
             'production', ['production','incremental']),
        ]
        if scope == 'candidate-repairs':
            definitions.append(('opt_f2_poly_dot',
                [f'a3_b7_{kind}_n{n}' for kind in
                 ['dense','sparse','zero','dense_prefix','sparse_prefix','alternating']
                 for n in [15,16,17,1023,1024,1025]],
                'production_zero_skip', ['production_zero_skip','public_prefix']))
        spec['families'] = [dict(name=name, sizes=sizes, baseline=baseline,
            variants=variants, arm_variants=[], selected={}, diagnostic=False,
            consumer=name!='opt_f2_poly_dot') for name,sizes,baseline,variants in definitions]
        spec['implementation_aliases'] = {}
        candidate_aliases(spec, threads)
        spec['coverage_scope'] = scope
        spec['unavailable_variants'] = []
        return spec
    comprehensive = scope in {'comprehensive', 'comprehensive-consumers'}
    if comprehensive:
        # Keep the established x86 matrix, including repaired cutoff neighbors.
        spec = build(features, stream_n, 'all' if scope == 'comprehensive' else 'consumers')
        by_case = {(g['name'], s): copy.deepcopy(dict(g, sizes=[s]))
                   for g in spec['families'] for s in g['sizes']}
        repair = build(features, stream_n, 'regressions' if scope == 'comprehensive' else 'regressions-consumers')
        for g in repair['families']:
            for s in g['sizes']:
                by_case.setdefault((g['name'], s), copy.deepcopy(dict(g, sizes=[s])))
        consumers = {'opt_ood', 'opt_ood_reuse', 'opt_ntt', 'opt_pack', 'opt_packed_ood',
                     'opt_gf_round', 'opt_gf_two_pair', 'opt_gf_fold_round', 'opt_gf_grid'}
        unavailable=[]
        # Import workloads and alternatives, never ARM promotion decisions.
        for filename in ['operation_metrics_cases.json', 'optimization_cases.json',
                         'candidate_fix_cases_v3_arm_1.json']:
            for g in json.loads((HERE / filename).read_text())['families']:
                consumer = g['name'] in consumers
                if scope == 'comprehensive-consumers' and not consumer:
                    continue
                variants=list(g['variants'])
                if g['name']=='opt_b127_mul':
                    # These experiment entry points are compiled only on ARM.
                    omitted=[v for v in variants if v in {'karatsuba','pfold'}]
                    unavailable.extend(dict(family=g['name'],size=s,variant=v,
                        reason='candidate entry point is compiled only on aarch64')
                        for s in g['sizes'] for v in omitted)
                    variants=[v for v in variants if v not in omitted]
                for s in g['sizes']:
                    key = (g['name'], s)
                    if key in by_case:
                        assert by_case[key]['baseline'] == g['baseline'], key
                        by_case[key]['variants'] = list(dict.fromkeys(by_case[key]['variants'] + variants))
                    else:
                        by_case[key] = dict(name=g['name'], sizes=[s], baseline=g['baseline'],
                            variants=variants, arm_variants=[], selected={},
                            diagnostic=g.get('diagnostic', False) or len(variants) == 1,
                            consumer=consumer)
        spec['families'] = list(by_case.values())
        spec['implementation_aliases'].update(repair['implementation_aliases'])
        spec['coverage_scope'] = scope
        spec['unavailable_variants'] = unavailable
        candidate_aliases(spec, threads)
        return spec
    features = set(features)
    if not {'pclmulqdq', 'sse4.1'} <= features:
        raise ValueError('x86 suite requires compiled PCLMUL and SSE4.1')
    spec = dict(schema=2, suite='arithmetic', campaign='x86', phase='explore',
                max_slowdown=.01, confirmation_runs=5, confirmation_samples=32,
                retry_multiplier=2, architectures=['x86_64'],
                required_features={'x86_64': ['pclmulqdq','sse4.1']},
                compiled_features=sorted(features), families=[],
                modulus_fixtures={'q100':str(2**100+277),'q128':str(2**128-159)},
                note='q100 is the historical fixture ID; its actual bit length is 101.')
    def add(name, sizes, baseline, variants, *, diagnostic=False, consumer=False):
        if scope == 'micro' and consumer or scope == 'consumers' and not consumer:
            return
        if scope.startswith('regressions') and (name not in REPAIRS or scope.endswith('consumers') and not consumer):
            return
        if scope.startswith('regressions'):
            if name in {'x86_products','gf_round_single','gf_round_two'}:
                sizes=sorted(set(map(int,sizes))|{4,5,8,9,15,1023,1025})
            if name=='integer_full_mac':
                sizes=[f'{p}_l2_n{n}' for p in ['full','carry'] for n in [1,3,7,16,17,31,32,33,1024,65536,1048576]]
        spec['families'].append(dict(name=name, sizes=list(map(str,sizes)), baseline=baseline,
            variants=variants, arm_variants=[], selected={}, diagnostic=diagnostic,
            consumer=consumer))
    packed = ['vpclmul4'] if {'avx512f','vpclmulqdq'} <= features else []
    if packed:
        spec['required_features']['x86_64'] += ['avx512f','vpclmulqdq']
    scalar = ['flock','schoolbook','karatsuba','barrett','binius','f2z']
    add('x86_products',SHORT+ARRAY,'flock',scalar+['barrett_u2','barrett_u4','barrett_u8']+packed)
    add('x86_chain',ARRAY,'flock',scalar)
    add('x86_dot',SHORT+ARRAY,'f2z_wide',['f2z_wide','wide1','wide2','wide4','wide8']+packed)
    for name,base,variants in [('xor','flock',['flock','f2z']),('wide','f2z_wide',['f2z_wide','flock_wide']),
                               ('reduce','f2z_reduce',['f2z_reduce','flock_reduce']),('square','flock',['flock','f2z'])]:
        add('x86_'+name,ARRAY,base,variants)
    add('x86_inverse',[1,16,1024],'flock',['flock','f2z'])
    add('x86_square_chain',[f'k{k}_n{n}' for k in [1,3,6,12,24,48] for n in [16,1024]],'flock',['flock','f2z'])
    for name in ['fixed','butterfly']:
        add('x86_'+name,[f'{t}_n{n}' for t in ['zero','half','full'] for n in SHORT+ARRAY],
            'f2z_fixed',['f2z_fixed','prepared','specialized'])
    add('x86_fixed_prepare',['zero','half','full'],'prepared',['prepared'],diagnostic=True)
    if stream_n:
        # Each case has at least 32*n actively traversed bytes, independently of
        # the other variants' resident allocations.
        for group in spec['families']:
            if group['name'] in ['x86_products','x86_dot','x86_xor','x86_wide','x86_reduce']:
                group['sizes'].append(f'stream_n{stream_n}')
    old=json.loads((HERE/'arithmetic_cases.json').read_text())
    for group in old['families']:
        name=group['name']
        if name.startswith('prime_') or name in ['integer_mac','projection']:
            group=copy.deepcopy(group)
            sizes=group['sizes']
            if name in ['prime_mul','prime_dot','prime_linear']:
                sizes=[f'q{q}_n{n}' for q in [100,128] for n in ARRAY]
            if name=='projection':
                sizes=[f'q{q}_l{l}_n{n}' for q in [100,128] for l in [2,4,9] for n in [16,1024,65536]]
            if name=='prime_dot':group['variants'].append('borrowed_delayed')
            add(name,sizes,group['baseline'],group['variants'],diagnostic=group.get('diagnostic',False))
    sizes=[f'q{q}_n{n}' for q in [100,128] for n in ARRAY]
    for name in ['prime_add','prime_sub','prime_chain']:
        add(name,sizes,'raw_ctx',['raw_ctx','configured_field'])
    add('prime_bits',sizes,'existing_bits',['existing_bits','acc2','acc4','acc8'])
    for name in ['prime_reduce_product_inputs','prime_reduce_linear_inputs']:
        add(name,[f'q{q}_{p}_n{n}' for q in [100,128] for p in ['uniform','carry'] for n in [1,1024,1048576]],
            'existing_optimized',['existing_optimized','crypto_bigint'])
    integer_sizes=[f'{p}_l{l}_n{n}' for p in ['full','carry'] for l in [1,2,4,9] for n in [16,1024,65536]]
    add('integer_full_mac',integer_sizes,'circuit_z',['circuit_z','fused1','fused2','fused4','fused8'])
    for name in ['integer_add','integer_sub','integer_mul']:
        add(name,integer_sizes,'circuit_z',['circuit_z'],diagnostic=True)
    focus=json.loads((HERE/'integer_focus_cases.json').read_text())
    for group in focus['families']:
        add(group['name'],group['sizes'],group['baseline'],group['variants'])
    for name,variants in [('gf_fold',['production','prepared']),('gf_round_single',['production','barrett','binius','native']),
                          ('gf_round_two',['production','barrett','binius','native']),('gf_fold_round',['production','two_pass','prepared'])]:
        add(name,[1,3,7,16,17,1024,65536],'production',variants,consumer=True)
    add('gf_fold_cascade',['log8','log16','log20'],'production',['production','prepared','reset_only'],consumer=True)
    add('prime_fold',[f'q{q}_n{n}' for q in [100,128] for n in [16,1024,65536]],'raw_ctx',['raw_ctx','unroll4'],consumer=True)
    add('ntt',NTT,'flock',['flock','preserved_schedule','tiled_half','reset_only'],consumer=True)
    # These wrappers deliberately call the named incumbent below a public
    # cutoff. Preserve their measurements, but report the actual kernel as
    # the selection rather than promoting duplicate names on timing noise.
    aliases={}
    for group in spec['families']:
        for size in group['sizes']:
            key=f"{group['name']}/{size}"
            if group['name']=='integer_full_mac' and '_l2_' in size and int(size.rsplit('n',1)[1])<32:
                aliases[key]={v:'circuit_z' for v in ['fused1','fused2','fused4','fused8']}
            if group['name'] in {'gf_round_single','gf_round_two'} and int(size)<4:
                aliases[key]={'native':'production' if group['name']=='gf_round_two' else 'barrett'}
    spec['implementation_aliases']=aliases
    return spec


def candidate_aliases(spec, threads):
    """Record public shape/thread dispatch, never count fallback wrappers as wins."""
    for g in spec['families']:
        for size in g['sizes']:
            alias = {}
            if g['name'] == 'opt_ntt' and 'incremental' in g['variants']:
                log, lanes = map(int, size.replace('log','').split('_lanes'))
                if (1 << log) * lanes > 1 << 15 or (log >= 12 and threads > 1):
                    alias['incremental'] = 'production'
            if g['name'] == 'opt_ood' and 'tiled_grouped' in g['variants']:
                log = int(size.removeprefix('log'))
                if log <= 10:
                    alias['tiled_grouped'] = 'tiled_256'
                elif log <= 16 or threads == 1:
                    alias['tiled_grouped'] = 'prepared_products'
            if alias:
                spec['implementation_aliases'].setdefault(f"{g['name']}/{size}", {}).update(alias)


def selections(spec, selection):
    """Freeze individual operation/size decisions, never a universal ARM winner."""
    groups=[];seen=set()
    for group in spec['families']:
        for size in group['sizes']:
            g=copy.deepcopy(group);g['sizes']=[size]
            key=f"{g['name']}/{size}"
            if not g.get('diagnostic'):
                if key not in selection: raise ValueError(f'missing selection: {key}')
                choice=selection[key];name=choice['variant'];challenger=choice.get('challenger')
                aliases=spec.get('implementation_aliases',{}).get(key,{})
                if name in aliases or challenger in aliases:
                    raise ValueError(f'select the underlying kernel, not its fallback wrapper: {key}')
                if name not in g['variants'] or name=='reset_only':raise ValueError(f'invalid selection: {key}/{name}')
                if challenger and (challenger not in g['variants'] or challenger==name or challenger=='reset_only'):
                    raise ValueError(f'invalid challenger: {key}/{challenger}')
                g['selected']={'x86_64':name};g['challenger']=challenger;seen.add(key)
                # Fresh confirmation measures the frozen choice against its
                # production baseline and predeclared strongest challenger.
                # Full alternative rankings remain in the exploration archive.
                required={g['baseline'],name,challenger,'reset_only'}
                g['variants']=[v for v in g['variants'] if v in required]
            groups.append(g)
    if set(selection)!=seen:raise ValueError('selection contains unknown or diagnostic cases')
    return dict(spec,families=groups,phase='confirm')

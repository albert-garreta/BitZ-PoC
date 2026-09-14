import copy
import json
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch
import host
import x86_manifest
import rank
from analysis import expanded

FEATURES=['pclmulqdq','sse4.1','avx2','avx512f','vpclmulqdq']

class X86Tests(unittest.TestCase):
    def test_cpu_sets(self):
        self.assertEqual(host.cpu_set('0-3,8,16-17'),{0,1,2,3,8,16,17})
        for value in ['', '-1', '4-2','1-2-3']:
            with self.assertRaises(ValueError):host.cpu_set(value)

    def test_exclusive_measurement_lease(self):
        with tempfile.TemporaryDirectory() as directory:
            path=Path(directory)/'lease'
            with host.exclusive(path):
                with self.assertRaises(RuntimeError):host.exclusive(path)
            with host.exclusive(path):pass

    def test_streaming_exceeds_l3_for_smallest_payload(self):
        for kib in [32768,98304]:
            n=host.streaming_terms({'topology':[{'l3_size':f'{kib}K'}]})
            self.assertGreater(n*32,2*kib*1024)
            self.assertEqual(n&(n-1),0)

    def test_feature_applicability(self):
        with self.assertRaises(ValueError):x86_manifest.build(['pclmulqdq'])
        scalar=x86_manifest.build(FEATURES[:2])
        packed=x86_manifest.build(FEATURES)
        self.assertFalse(any(r['variant']=='vpclmul4' for r in expanded(scalar)))
        self.assertTrue(any(r['variant']=='vpclmul4' for r in expanded(packed)))
        self.assertIn('vpclmulqdq',packed['required_features']['x86_64'])

    def test_no_duplicate_cases_and_no_arm_selections(self):
        rows=list(expanded(x86_manifest.build(FEATURES)))
        keys=[(r['family'],r['size'],r['variant']) for r in rows]
        self.assertEqual(len(keys),len(set(keys)))
        self.assertFalse(any(r['selected'] for r in rows))
        self.assertTrue(all(r['arch']=='x86_64' for r in rows))

    def test_all_operations_have_coverage_and_real_baselines(self):
        spec=x86_manifest.build(FEATURES,8388608)
        groups={g['name']:g for g in spec['families']}
        self.assertEqual(groups['x86_fixed']['baseline'],'f2z_fixed')
        self.assertEqual(groups['x86_dot']['baseline'],'f2z_wide')
        self.assertEqual(len(groups['ntt']['sizes']),7)
        for name in ['x86_reduce','x86_wide','x86_square_chain','prime_bits',
                     'prime_reduce_product_inputs','prime_reduce_linear_inputs','integer_full_mac',
                     'bounded_product','projection','gf_fold','gf_round_single','gf_round_two','gf_fold_round']:
            self.assertIn(name,groups)
        for g in groups.values():self.assertIn(g['baseline'],g['variants'])
        self.assertIn('stream_n8388608',groups['x86_dot']['sizes'])
        self.assertIn('q100_l9_n65536',groups['projection']['sizes'])

    def test_selection_is_complete_and_checked(self):
        spec=x86_manifest.build(FEATURES)
        choices={f"{g['name']}/{size}":{'variant':g['baseline'],'challenger':None}
                 for g in spec['families'] if not g['diagnostic'] for size in g['sizes']}
        frozen=x86_manifest.selections(spec,choices)
        self.assertEqual(frozen['phase'],'confirm')
        self.assertTrue(any(r['selected'] for r in expanded(frozen)))
        missing=dict(choices);missing.pop(next(iter(missing)))
        with self.assertRaises(ValueError):x86_manifest.selections(spec,missing)
        bad=copy.deepcopy(choices);bad[next(iter(bad))]['variant']='software_fake'
        with self.assertRaises(ValueError):x86_manifest.selections(spec,bad)
        bad=copy.deepcopy(choices);bad[next(iter(bad))]['challenger']='unsupported'
        with self.assertRaises(ValueError):x86_manifest.selections(spec,bad)

    def test_scopes_partition_coverage(self):
        all_rows={(r['family'],r['size'],r['variant']) for r in expanded(x86_manifest.build(FEATURES))}
        parts=[{(r['family'],r['size'],r['variant']) for r in expanded(x86_manifest.build(FEATURES,scope=s))} for s in ['micro','consumers']]
        self.assertFalse(parts[0]&parts[1]);self.assertEqual(all_rows,parts[0]|parts[1])

    def test_regression_scope_keeps_boundaries_and_controls(self):
        spec=x86_manifest.build(FEATURES,8388608,scope='regressions')
        groups={g['name']:g for g in spec['families']}
        self.assertEqual(set(groups),x86_manifest.REPAIRS)
        for size in ['3','4','5']:
            self.assertIn(size,groups['gf_round_two']['sizes'])
        for size in ['full_l2_n31','full_l2_n32','full_l2_n33']:
            self.assertIn(size,groups['integer_full_mac']['sizes'])
        self.assertIn('borrowed_delayed',groups['prime_dot']['variants'])
        self.assertIn('convert_then_delayed',groups['prime_dot']['variants'])
        self.assertIn('native',groups['gf_round_single']['variants'])
        self.assertIn('tiled_half',groups['ntt']['variants'])
        self.assertEqual(len(groups['ntt']['sizes']),7)

    def test_confirmation_retains_baseline_selected_and_challenger(self):
        spec=x86_manifest.build(FEATURES,scope='regressions-consumers')
        choices={f"{g['name']}/{size}":dict(variant=g['variants'][1],challenger=g['variants'][2] if g['variants'][2]!='reset_only' else None)
                 for g in spec['families'] for size in g['sizes']}
        confirmed=x86_manifest.selections(spec,choices)
        for group in confirmed['families']:
            self.assertIn(group['baseline'],group['variants'])
            self.assertIn(group['selected']['x86_64'],group['variants'])
            if group['challenger']:self.assertIn(group['challenger'],group['variants'])

    def test_fallbacks_are_reported_as_the_actual_kernel(self):
        spec=x86_manifest.build(FEATURES,scope='regressions')
        aliases=spec['implementation_aliases']
        self.assertEqual(aliases['gf_round_two/1']['native'],'production')
        self.assertEqual(aliases['integer_full_mac/full_l2_n31']['fused2'],'circuit_z')
        self.assertNotIn('integer_full_mac/full_l2_n32',aliases)
        choices={f"{g['name']}/{s}":dict(variant=g['baseline'],challenger=None)
                 for g in spec['families'] for s in g['sizes']}
        choices['gf_round_two/1']['variant']='native'
        with self.assertRaisesRegex(ValueError,'underlying kernel'):
            x86_manifest.selections(spec,choices)

    def test_challenger_decisions(self):
        baseline=[[100.+i for i in range(32)] for _ in range(5)]
        fast=[[x*.8 for x in row] for row in baseline]
        slow=[[x*1.2 for x in row] for row in baseline]
        self.assertEqual(rank.comparison(fast,baseline)['decision'],'faster')
        self.assertEqual(rank.comparison(baseline,baseline)['decision'],'within_1_percent')
        self.assertEqual(rank.comparison(slow,baseline)['decision'],'slower')

    def test_fresh_seeds_include_potential_retry(self):
        from run import planned_seeds
        self.assertEqual(planned_seeds(42,2,False),{42,42+7919})
        self.assertIn(42+104687,planned_seeds(42,2,True))

    def test_challenger_uncertainty_retries_only_affected_case(self):
        group=dict(name='products',sizes=['16'],selected={'x86_64':'candidate'},challenger='flock')
        rows=[dict(family='products',size='16',variant=v,complete=True) for v in ['candidate','flock']]
        raw={('products','16',v):[[1.0]] for v in ['candidate','flock']}
        with patch.object(rank,'samples',return_value=raw), patch.object(rank,'comparison',return_value={'decision':'unresolved'}):
            self.assertEqual(rank.unresolved_challengers('.',rows,{'phase':'confirm','runs':1},{'families':[group]}),{'products/16'})
            self.assertEqual(rank.unresolved_challengers('.',rows,{'phase':'explore','runs':1},{'families':[group]}),set())

class ConsumerDriverTests(unittest.TestCase):
    def test_missing_processor_fails_before_build(self):
        import end_to_end
        with patch.object(end_to_end.shutil,'which',return_value=None):
            with self.assertRaisesRegex(ValueError,'PERFETTO_TRACE_PROCESSOR'):
                end_to_end.trace_processor()

    def test_driver_matrix_and_record_parsing(self):
        import end_to_end
        import ryzen
        self.assertEqual(len(ryzen.jobs(True)),6)
        self.assertEqual(ryzen.jobs(False)[0][1],'0')
        self.assertEqual(ryzen.jobs(False)[1][1],'8')
        cases=end_to_end.cases()
        self.assertEqual(len(cases),9)
        self.assertEqual(len({c['id'] for c in cases}),9)
        for case in cases:
            if case['bench']=='mul_e2e_compare':self.assertEqual(case['env']['F2Z_MUL_COMPARE_BACKENDS'],'f2z')
        parsed=end_to_end.records('noise\n  RESULT schema=f2z/2 prove_ms=12.3 proof_bytes=1024\n{"schema":"x", "verified":true}\n')
        self.assertEqual(parsed[0]['prove_ms'],'12.3')
        self.assertTrue(parsed[1]['verified'])

class RuntimeEvidenceTests(unittest.TestCase):
    def test_wrong_affinity_and_backend_block_coverage(self):
        from test_gate import GateTests
        import analysis
        fixture=GateTests();fixture.setUp()
        self.addCleanup(fixture.temp.cleanup)
        fixture.spec.update(campaign='x86',suite='arithmetic',architectures=['x86_64'],required_features={'x86_64':['pclmulqdq','sse4.1']})
        fixture.meta.update(arch='x86_64',cpu_set='0',phase='confirm')
        fixture.runtime.update(arch='x86_64',affinity='0',pclmulqdq=True,arithmetic_campaign=True,
                               flock_kernel='x86-karatsuba-barrett',candidate_mul_kernel='x86-karatsuba-barrett',**{'sse4.1':True})
        fixture.make_data()
        self.assertTrue(all(r['complete'] for r in fixture.rows()))
        for key,bad in [('affinity','8'),('flock_kernel','portable'),('candidate_mul_kernel','portable'),('sse4.1',False)]:
            old=fixture.runtime[key];fixture.runtime[key]=bad;fixture.make_data()
            self.assertTrue(all(not r['complete'] for r in fixture.rows()))
            fixture.runtime[key]=old


class X86GateTests(unittest.TestCase):
    def setUp(self):
        from test_gate import GateTests
        from run import save,sha
        self.fixture=fixture=GateTests();fixture.setUp()
        self.addCleanup(fixture.temp.cleanup)
        fixture.spec.update(campaign='x86',phase='confirm',suite='arithmetic',architectures=['x86_64'],
                            required_features={'x86_64':['pclmulqdq','sse4.1']})
        fixture.spec['families'][0]['selected']={'x86_64':'candidate'}
        fixture.meta.update(phase='confirm',arch='x86_64',cpu_set='0')
        fixture.runtime.update(arch='x86_64',affinity='0',pclmulqdq=True,arithmetic_campaign=True,
                               flock_kernel='x86-karatsuba-barrett',candidate_mul_kernel='x86-karatsuba-barrett',**{'sse4.1':True})
        fixture.make_data()
        # Supply otherwise valid frozen evidence to exercise x86 gate policy.
        fixture.publish(fixture.rows())
        metadata=json.loads((fixture.root/'metadata.json').read_text())
        for filename,key in [('benchmark.bin','archived_binary_sha256'),('sources.tar.gz','sources_archive_sha256'),
                             ('Cargo.lock','lockfile_sha256'),('rankings.json','rankings_sha256'),('selection.json','selection_sha256')]:
            path=fixture.root/filename
            path.write_text('[]' if filename=='rankings.json' else '{}')
            metadata[key]=sha(path)
        save(fixture.root/'metadata.json',metadata)

    def test_changed_source_archive_is_rejected(self):
        import gate
        self.assertEqual(gate.evaluate(self.fixture.root),[])
        (self.fixture.root/'sources.tar.gz').write_text('changed')
        self.assertIn('sources.tar.gz',gate.evaluate(self.fixture.root)[0])

    def test_exploration_cannot_pass_confirmation(self):
        import gate
        from run import save
        path=self.fixture.root/'metadata.json'
        metadata=json.loads(path.read_text());metadata['phase']='explore';save(path,metadata)
        self.assertIn('exploration',gate.evaluate(self.fixture.root)[0])


if __name__=='__main__':unittest.main()

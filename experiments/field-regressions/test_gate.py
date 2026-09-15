"""Policy tests use constructed timings so noise cannot turn a failure into a pass."""
import csv
import hashlib
import json
from pathlib import Path
import tempfile
import unittest

import analysis
import gate
import run


class GateTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.spec = dict(schema=2, max_slowdown=.01, confirmation_runs=5, confirmation_samples=32,
                         architectures=['aarch64'], required_features={'aarch64':['aes']},
                         families=[dict(name='products', sizes=['16','1024'], baseline='flock',
                                        selected={'aarch64':'candidate'}, variants=['flock','candidate','slow'], arm_variants=[])])
        self.meta = dict(schema=2, status='complete', arch='aarch64', runs=5, samples=32, threads=1, margin=.01)
        self.runtime = dict(arch='aarch64', aes=True, candidate_snapshot=True, caller_threads=1,
                            baseline_all_core_threads=1, candidate_all_core_threads=1)
        self.make_data()

    def make_data(self):
        for process in range(5):
            (self.root/f'run-{process}.log').write_text('RUNTIME '+json.dumps(self.runtime)+'\nCORRECTNESS_COMPLETE\n')
            with (self.root/f'run-{process}.csv').open('w') as stream:
                writer=csv.writer(stream)
                writer.writerow(['family','size','variant','rep','iterations','ns','bytes','allocations','allocated_bytes','correctness'])
                for size in ['16','1024']:
                    for rep in range(32):
                        for variant,factor in [('flock',1.),('candidate',.8),('slow',1.3)]:
                            writer.writerow(['products',size,variant,rep,1,(100+rep)*factor,32,0,0,1])

    def rows(self):
        return analysis.summarize_round(self.root,self.meta,self.spec,draws=100)[0]

    def publish(self,rows):
        manifest=self.root/'required_cases.json'
        manifest.write_text(json.dumps(self.spec))
        metadata=dict(self.meta,manifest_sha256=hashlib.sha256(manifest.read_bytes()).hexdigest())
        (self.root/'summary.json').write_text(json.dumps(rows))
        import shutil
        initial=self.root/'initial';initial.mkdir(exist_ok=True)
        for p in self.root.glob('run-*.*'):shutil.copy2(p,initial/p.name)
        artifacts={p.name:hashlib.sha256(p.read_bytes()).hexdigest() for p in initial.glob('run-*.*')}
        detail=dict(metadata,artifacts_sha256=artifacts)
        (initial/'metadata.json').write_text(json.dumps(detail))
        metadata.update(round_metadata_sha256={'initial':hashlib.sha256((initial/'metadata.json').read_bytes()).hexdigest()},
                        summary_sha256=hashlib.sha256((self.root/'summary.json').read_bytes()).hexdigest())
        (self.root/'metadata.json').write_text(json.dumps(metadata))

    def test_multiple_predeclared_candidates_are_all_gated(self):
        self.spec['families'][0]['selected']={'aarch64':['candidate','slow']}
        rows=self.rows()
        self.assertTrue(all(r['selected'] for r in rows if r['variant']in['candidate','slow']))
        self.publish(rows)
        self.assertTrue(any('slow' in issue for issue in gate.evaluate(self.root)))

    def test_defaults_share_one_percent(self):
        self.assertEqual(analysis.DEFAULT_MARGIN,.01)
        self.assertEqual(run.DEFAULT_MARGIN,gate.DEFAULT_MARGIN)

    def test_good_candidate_and_negative_control(self):
        rows=self.rows()
        self.assertTrue(all(r['status']=='pass' for r in rows if r['selected']))
        self.assertTrue(all(r['status']=='regression' for r in rows if r['variant']=='slow'))
        self.publish(rows)
        self.assertEqual(gate.evaluate(self.root),[])

    def test_arithmetic_does_not_require_an_ntt_candidate_pool(self):
        self.spec['suite']='arithmetic'
        self.runtime.update(arithmetic_campaign=True,candidate_snapshot=False,candidate_all_core_threads=0)
        self.make_data()
        self.assertTrue(all(r['complete'] for r in self.rows()))

    def test_arithmetic_still_requires_its_binary_and_cpu_features(self):
        self.spec['suite']='arithmetic'
        self.runtime.update(arithmetic_campaign=False)
        self.make_data()
        self.assertTrue(all(not r['complete'] for r in self.rows()))
        self.runtime.update(arithmetic_campaign=True,aes=False)
        self.make_data()
        self.assertTrue(all(not r['complete'] for r in self.rows()))

    def test_setup_is_diagnostic_even_when_complete(self):
        self.spec['families'][0]['diagnostic']=True
        rows=self.rows()
        self.assertTrue(all(r['status']=='unmeasured' and r['complete'] for r in rows))

    def test_required_size_cannot_disappear(self):
        rows=[r for r in self.rows() if r['size']!='1024']
        self.publish(rows)
        self.assertTrue(any('unmeasured' in x for x in gate.evaluate(self.root)))

    def test_required_architecture_cannot_disappear(self):
        rows=self.rows()
        self.spec['architectures'].append('x86_64')
        self.publish(rows)
        self.assertTrue(any('x86_64' in x for x in gate.evaluate(self.root)))

    def test_duplicate_summary_rejected(self):
        rows=self.rows();rows.append(rows[0]);self.publish(rows)
        self.assertIn('duplicate',gate.evaluate(self.root)[0])

    def test_duplicate_sample_rejected(self):
        p=self.root/'run-0.csv'
        with p.open('a') as stream:stream.write(p.read_text().splitlines()[1]+'\n')
        with self.assertRaises(ValueError):self.rows()

    def test_missing_process_sample_or_runtime_blocks_coverage(self):
        for what in ['process','sample','backend','pool']:
            with self.subTest(what=what):
                self.make_data()
                if what=='process':(self.root/'run-4.log').unlink()
                if what=='sample':
                    p=self.root/'run-4.csv';p.write_text('\n'.join(p.read_text().splitlines()[:-1])+'\n')
                if what in ['backend','pool']:
                    runtime=dict(self.runtime)
                    runtime['aes' if what=='backend' else 'candidate_all_core_threads']=False if what=='backend' else 2
                    (self.root/'run-4.log').write_text('RUNTIME '+json.dumps(runtime)+'\nCORRECTNESS_COMPLETE\n')
                self.assertTrue(any(not r['complete'] for r in self.rows()))

    def test_smoke_cannot_pass(self):
        self.meta['runs']=1;self.meta['samples']=2
        self.assertTrue(all(r['status']=='unmeasured' for r in self.rows()))

    def test_p95_process_allocation_and_correctness_gates(self):
        row=next(r for r in self.rows() if r['selected'])
        changes=[dict(p95_ci_low=1.02,p95_ci_high=1.04),dict(per_run_medians=[.8,.8,.8,.8,1.02]),
                 dict(allocations_ok=False),dict(correctness=False),dict(median_ci_high=1.011)]
        for change in changes:
            with self.subTest(change=change):
                self.assertNotEqual(analysis.classify(dict(row,**change)),'pass')

    def test_p95_is_ratio_of_quantiles(self):
        # A ratio's quantile is not generally the ratio of latency quantiles.
        c=[[1.,100.,100.,100.]]*5;b=[[100.,1.,100.,100.]]*5
        self.assertEqual(analysis.paired_stats(c,b,draws=100)['p95_ratio'],1.)

    def test_selection_flags_cannot_hide_regression(self):
        rows=self.rows()
        for r in rows:
            if r['selected']:
                r.update(selected=False,p95_ci_low=1.1,p95_ci_high=1.2)
        self.publish(rows)
        self.assertTrue(any('regression' in x for x in gate.evaluate(self.root)))

    def test_missing_raw_data_blocks_gate(self):
        self.publish(self.rows())
        (self.root/'initial/run-0.csv').unlink()
        self.assertIn('missing',gate.evaluate(self.root)[0])

    def test_manifest_changes_block_gate(self):
        self.publish(self.rows())
        with (self.root/'required_cases.json').open('a') as stream:stream.write(' ')
        self.assertIn('changed',gate.evaluate(self.root)[0])


if __name__=='__main__':
    unittest.main()

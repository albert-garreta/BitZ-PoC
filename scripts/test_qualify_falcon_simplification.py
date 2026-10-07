import copy
import unittest
import qualify_falcon_simplification as q


def process(payload=1000, fixed=400, root='a'*64):
    header={k:'matched' for k in q.MATCHED}
    header.update(cpu_affinity=None,arithmetic_prime_min='1',arithmetic_prime_max='2',warmup_trials=1)
    return dict(header=header,samples=[dict(source_root=root)]*3,
                medians={m:1.0 for m in q.METRICS},payload=payload,
                fixed_payload={'messages':fixed},nonce_boundaries={'prime':1},peak_rss_kib=1024)


class QualificationTests(unittest.TestCase):
    def test_fixed_mean_estimator_not_mean_of_ratios(self):
        r=q.mean_ratio_interval([1.,100.],[2.,100.],draws=20)
        self.assertAlmostEqual(r['ratio'],102/101)
        self.assertNotAlmostEqual(r['ratio'],1.5)

    def test_new_transcript_allows_changed_payload_paths(self):
        old,new=process(),process(payload=1200)
        q.validate_pair(old,new)
        report=q.summarize({(512,100,1):{42:{'baseline':old,'candidate':new},43:{'baseline':old,'candidate':new}}},False,False)
        payload=report['cells'][0]['payload']
        self.assertTrue(payload['pass_gate'])
        self.assertEqual(payload['ratio_of_mean_bytes'],1.2)
        self.assertFalse(payload['actual_total_is_gate'])
        self.assertEqual(payload['actual_total_ranges']['candidate'],{'min':1200,'max':1200})

    def test_fixed_bucket_increase_rejected_even_with_smaller_total(self):
        with self.assertRaisesRegex(ValueError,'fixed payload bucket increased'):
            q.validate_pair(process(),process(payload=900,fixed=401))

    def test_retired_zero_bucket_is_zero(self):
        old,new=process(),process()
        old['fixed_payload']['opening_ood']=0
        q.validate_pair(old,new)
        new['fixed_payload']['new_message']=1
        with self.assertRaisesRegex(ValueError,'fixed payload bucket increased'):
            q.validate_pair(old,new)

    def test_source_root_change_rejected(self):
        with self.assertRaisesRegex(ValueError,'source commitment changed'):
            q.validate_pair(process(),process(root='b'*64))

    def test_degree_and_extension_must_match(self):
        for field in ('degree','ring_extension','max_batch','pcs_query_shape'):
            new=process();new['header'][field]='changed'
            with self.assertRaisesRegex(ValueError,'unmatched workload'):
                q.validate_pair(process(),new)

    def test_nonce_inventory_must_match(self):
        new=process();new['nonce_boundaries']['prime']=2
        with self.assertRaisesRegex(ValueError,'nonce boundary inventory'):
            q.validate_pair(process(),new)

    def test_only_path_hashes_are_variable(self):
        row=dict(proof_payload_bytes=500,proof_payload_breakdown={'source_authentication_joint':64,'pcs_recursive_authentication':32,'messages':404})
        self.assertEqual(q.structural_payload(row),{'messages':404})
        row['proof_payload_breakdown']['messages']+=1
        with self.assertRaisesRegex(ValueError,'does not sum'):
            q.structural_payload(row)

    def test_partial_matrix_never_qualifies(self):
        pair={'baseline':process(),'candidate':process()}
        report=q.summarize({(512,100,1):{42:pair,43:pair}},True,False)
        self.assertNotEqual(report['status'],'pass')

    def test_row_bytes_and_framing_match_distinct_query_counts(self):
        shape={'recursive_steps':2,'queries':[3,2,1],'recursive_opened_row_widths':[8],'initial_opened_row_width':11}
        row={'proof_payload_breakdown':{'pcs_initial_opened_rows':3*11*16,'pcs_recursive_opened_rows':2*8*16,'pcs_bincode_framing':72+16+8*5}}
        q.validate_query_buckets(row,shape)
        row['proof_payload_breakdown']['pcs_bincode_framing']+=8
        with self.assertRaisesRegex(ValueError,'framing differs'):
            q.validate_query_buckets(row,shape)

    def test_nonce_prefix_cannot_be_below_boundaries(self):
        row={'grinding_diagnostics':{'definition':'prefix','categories':[{'category':'prime','stored_nonce_boundaries':2,'serial_nonce_prefix_sum':'1'}]}}
        with self.assertRaisesRegex(ValueError,'serial grinding prefix'):
            q.nonce_boundaries(row)

    def test_balanced_order_unchanged(self):
        self.assertEqual([q.process_order(('baseline','candidate'),i) for i in range(4)],
                         [['baseline','candidate'],['candidate','baseline'],['candidate','baseline'],['baseline','candidate']])

    def test_resolved_auto_matrix_is_16_cases(self):
        self.assertEqual(len(q.DEGREES)*len(q.TARGETS)*len(q.BATCHES),16)
        self.assertEqual(q.SEEDS,tuple(range(42,102)))
        self.assertEqual(q.LIMIT,1.02)

if __name__=='__main__':unittest.main()

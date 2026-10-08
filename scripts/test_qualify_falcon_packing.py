import unittest

from qualify_falcon_packing import compare, summarize
from analyze_falcon_packing import qualified


class QualificationTests(unittest.TestCase):
    def test_regression_requires_independent_confirmation(self):
        row = dict(primary_improves=True, apparent_regressions=['prove_ms'],
                   metrics={'total_prover_ms': {'status': 'improvement'}})
        self.assertFalse(qualified(row))
        self.assertFalse(qualified(row, row))
        repeat = row | {'apparent_regressions': []}
        self.assertTrue(qualified(row, repeat))

    def test_different_proofs_fail_despite_identical_sizes(self):
        a = {'identity': {'input_digest': 'a', 'proof_payload_bytes': 10, 'proof_debug_digest': 'b'}}
        b = {'identity': {'input_digest': 'a', 'proof_payload_bytes': 10, 'proof_debug_digest': 'c'}}
        with self.assertRaisesRegex(ValueError, 'proof_debug_digest'):
            compare(a, b)

    def test_missing_input_identity_fails(self):
        with self.assertRaisesRegex(ValueError, 'input identity'):
            compare({'identity': {}}, {'identity': {}})

    def test_incomplete_cells_cannot_qualify(self):
        record = dict(cell=[512, 3, 8, 100], variant='original', block=0,
                      medians={'packing_ms': 10, 'peak_rss_bytes': 100})
        self.assertEqual(summarize([record], ['original', 'serial-word'], 12, 'packing'), [])

    def test_primary_gain_does_not_hide_secondary_regression(self):
        records = []
        for block in range(12):
            for variant, total, prove in [('original', 10, 5), ('serial-word', 8, 6)]:
                records.append(dict(cell=[512, 1024, 8, 100], block=block, variant=variant,
                                    medians={'total_prover_ms': total, 'prove_ms': prove,
                                             'peak_rss_bytes': 100}))
        result = summarize(records, ['original', 'serial-word'], 12, 'e2e')[0]
        self.assertTrue(result['primary_improves'])
        self.assertEqual(result['apparent_regressions'], ['prove_ms'])
        self.assertEqual(result['metrics']['peak_rss_bytes']['status'], 'inconclusive')


if __name__ == '__main__':
    unittest.main()

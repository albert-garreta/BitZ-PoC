"""Guard against accepting uncertain or invalid performance measurements."""
import unittest

from compare_prover_snapshots import classify_interval, paired_interval


class PerformanceGateTests(unittest.TestCase):
    def test_uncertainty_is_not_a_pass(self):
        self.assertEqual(classify_interval([0.99, 1.01]), 'inconclusive')
        self.assertEqual(classify_interval([1.001, 1.01]), 'regression')
        self.assertEqual(classify_interval([0.98, 1.0]), 'pass')

    def test_two_percent_is_a_separate_gate(self):
        self.assertEqual(classify_interval([0.975, 0.985], 0.98), 'inconclusive')
        self.assertEqual(classify_interval([0.97, 0.98], 0.98), 'pass')

    def test_invalid_or_unpaired_data_is_rejected(self):
        for ratios in [[], [1], [1, 0], [1, -1], [1, float('nan')], [1, float('inf')]]:
            with self.assertRaises(ValueError):
                paired_interval(ratios)

    def test_exact_repeated_ratio(self):
        low, high = paired_interval([0.97] * 6)
        self.assertAlmostEqual(low, 0.97)
        self.assertAlmostEqual(high, 0.97)


if __name__ == '__main__':
    unittest.main()

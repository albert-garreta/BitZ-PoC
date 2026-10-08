import json
import unittest

from qualify_falcon_witness import compare, environment, extract, summarize, validate_configuration


def trial(phase='sample', **changes):
    return dict(trial=phase, verified=True, input_digest='input', source_root='root',
                proof_debug_digest='proof', proof_payload_bytes=1024,
                proof_payload_breakdown={'pcs': 1024}, capacity=1024,
                witness_commit_ms=10, total_prover_ms=30, prove_ms=20, verify_ms=5,
                **changes)


def output(rows):
    return '\n'.join(json.dumps(row) for row in rows)


RSS = 'Maximum resident set size (kbytes): 1234\n'


class WitnessQualificationTests(unittest.TestCase):
    def test_exact_warmup_count_and_diagnostic_boundary(self):
        rows = [trial('warmup'), trial('warmup'), trial(), trial()]
        rows[0]['witness_commit_ms'] = 1000
        stages = [dict(event='stage', name='falcon_algebraic:witness', elapsed_ms=value)
                  for value in [100, 100, 2, 4]]
        result = extract(output(rows), RSS + output(stages), 'arithmetic', 2, 2,
                         'falcon_algebraic:witness')
        self.assertEqual(result['medians']['witness_commit_ms'], 10)
        self.assertEqual(result['medians']['diagnostic_witness_ms'], 3)
        self.assertEqual(result['medians']['peak_rss_bytes'], 1234 * 1024)

    def test_rejects_missing_or_duplicate_witness_spans(self):
        rows = [trial('warmup'), trial()]
        for count in [0, 1, 3]:
            stages = [dict(event='stage', name='witness', elapsed_ms=1)] * count
            with self.assertRaisesRegex(ValueError, 'span'):
                extract(output(rows), RSS + output(stages), 'arithmetic', 1, 1, 'witness')

    def test_rejects_invalid_proofs_or_trial_order(self):
        rows = [trial('warmup'), trial()]
        rows[1]['verified'] = False
        with self.assertRaisesRegex(ValueError, 'verification'):
            extract(output(rows), RSS, 'arithmetic', 1, 1)
        with self.assertRaisesRegex(ValueError, 'order'):
            extract(output([trial(), trial('warmup')]), RSS, 'arithmetic', 1, 1)

    def test_rejects_missing_proof_identity_and_same_size_different_proof(self):
        row = trial()
        del row['proof_debug_digest']
        with self.assertRaises(KeyError):
            extract(output([row]), RSS, 'arithmetic', 0, 1)
        left = extract(output([trial()]), RSS, 'arithmetic', 0, 1)
        right = extract(output([trial()]), RSS, 'arithmetic', 0, 1)
        right['identity']['proof_debug_digest'] = 'other-proof'
        with self.assertRaisesRegex(ValueError, 'proof_debug_digest'):
            compare(left, right)

    def test_full_names_and_macos_rss(self):
        row = trial()
        row['proof_prove_ms'] = row.pop('prove_ms')
        row['proof_verify_ms'] = row.pop('verify_ms')
        result = extract(output([row]), '123456 maximum resident set size', 'full', 0, 1)
        self.assertEqual(result['medians']['prove_ms'], 20)
        self.assertEqual(result['medians']['peak_rss_bytes'], 123456)

    def test_diagnostic_total_never_passes_uninstrumented_gate(self):
        cell = [512, 1024, 16, 100]
        records = [dict(cell=cell, block=block, variant=variant,
                        medians=dict(witness_commit_ms=value, total_prover_ms=value,
                                     diagnostic_witness_ms=value, peak_rss_bytes=100))
                   for block in range(3) for variant, value in [('baseline', 10), ('candidate', 8)]]
        row = summarize(records, [cell], 3, True)[0]
        self.assertTrue(row['witness_significantly_improves'])
        self.assertIsNone(row['total_within_two_percent'])
        self.assertIsNone(row['commit_significantly_improves'])
        with self.assertRaisesRegex(ValueError, 'blocks'):
            summarize(records[:-1], [cell], 3, True)

    def test_total_slowdown_cannot_hide_behind_commit_improvement(self):
        cell = [512, 1024, 16, 100]
        records = [dict(cell=cell, block=block, variant=variant,
                        medians=dict(witness_commit_ms=commit, total_prover_ms=total))
                   for block in range(3) for variant, commit, total in
                   [('baseline', 10, 100), ('candidate', 5, 103)]]
        row = summarize(records, [cell], 3, False)[0]
        self.assertTrue(row['commit_significantly_improves'])
        self.assertFalse(row['total_within_two_percent'])
        self.assertIsNone(row['witness_significantly_improves'])

    def test_configuration_requires_matching_seed_and_native_build(self):
        row = trial(degree=512, batch=1024, threads=16, security_bits=100,
                    auxiliary_pool_threads=16, seed=42, build_rustflags='-C target-cpu=native')
        record = extract(output([row]), RSS, 'arithmetic', 0, 1)
        validate_configuration(record, [512, 1024, 16, 100], 42)
        with self.assertRaisesRegex(ValueError, 'seed'):
            validate_configuration(record, [512, 1024, 16, 100], 43)
        record['trials'][0]['build_rustflags'] = None
        with self.assertRaisesRegex(ValueError, 'native'):
            validate_configuration(record, [512, 1024, 16, 100], 42)
        record['trials'][0].pop('seed')
        record['trials'][0].pop('build_rustflags')
        record['prepared'] = dict(input_seed=42, build_rustflags='-C target-cpu=native')
        validate_configuration(record, [512, 1024, 16, 100], 42)

    def test_secondary_regressions_are_explicit(self):
        cell = [512, 1024, 16, 100]
        records = [dict(cell=cell, block=block, variant=variant,
                        medians=dict(witness_commit_ms=commit, total_prover_ms=total,
                                     prove_ms=prove, verify_ms=verify, peak_rss_bytes=rss))
                   for block in range(3) for variant, commit, total, prove, verify, rss in
                   [('baseline', 10, 100, 90, 10, 100), ('candidate', 5, 99, 94, 11, 110)]]
        row = summarize(records, [cell], 3, False)[0]
        self.assertTrue(row['total_within_two_percent'])
        self.assertEqual(row['secondary_regressions'], ['prove_ms', 'verify_ms', 'peak_rss_bytes'])

    def test_debug_overrides_cannot_contaminate_e2e(self):
        with self.assertRaisesRegex(ValueError, 'logging'):
            environment(16, '/tmp/cache', None, {'FLOCK_COMMIT_TIMING': '1'}, False, 'full')


if __name__ == '__main__':
    unittest.main()

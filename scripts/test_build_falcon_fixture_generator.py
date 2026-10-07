from pathlib import Path
import unittest
import build_falcon_fixture_generator as b

class BuilderTests(unittest.TestCase):
    def artifacts(self):
        return [{'reason':'compiler-artifact','target':{'name':name},'filenames':[f'/tmp/deps/lib{name}-abc.rlib',f'/tmp/deps/lib{name}-abc.rmeta']} for name in b.DEPENDENCIES]

    def test_selects_rlibs_from_cargo_json(self):
        result=b.resolve_rlibs(self.artifacts())
        self.assertEqual(set(result),set(b.DEPENDENCIES))
        self.assertEqual(result['bitz'],Path('/tmp/deps/libbitz-abc.rlib'))

    def test_rejects_missing_or_conflicting_artifacts(self):
        with self.assertRaisesRegex(ValueError,'missing'):
            b.resolve_rlibs(self.artifacts()[:-1])
        rows=self.artifacts()+[{'reason':'compiler-artifact','target':{'name':'bitz'},'filenames':['/tmp/deps/libbitz-other.rlib']}]
        with self.assertRaisesRegex(ValueError,'ambiguous'):
            b.resolve_rlibs(rows)

    def test_main_wraps_canonical_input_helper(self):
        self.assertIn('mod inputs;',b.MAIN)
        self.assertIn('generate_cases_uncached(degree, count, seed)',b.WRAPPER)
        self.assertNotIn('keygen(',b.WRAPPER)

if __name__=='__main__':unittest.main()

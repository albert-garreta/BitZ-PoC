import copy
import json
from pathlib import Path
import tempfile
import unittest
import prewarm_falcon_profile_cache as p


class PrewarmTests(unittest.TestCase):
    def fixture(self,directory):
        case=(512,1,42)
        checksum='01'*32
        path=directory/p.filename_for(case)
        path.write_bytes(b'reference-payload'+bytes.fromhex(checksum))
        row={'schema':'bitz/falcon-degree-fixture-reference/v1','degree':512,'batch':1,'seed':42,'cache_filename':path.name,'cache_bytes':path.stat().st_size,'input_digest':'02'*32,'cache_payload_blake3':checksum,'fresh_uncached_generation':True,'cache_roundtrip_validated':True,'upstream_verified':True,'ct_conversion_verified':True,'native_preflight_verified':True}
        return case,path,row

    def test_new_degree_name_and_key(self):
        self.assertEqual(p.key_for((512,3,42)),'512:3:42')
        self.assertEqual(p.filename_for((1024,3,42)),'fn-dsa-0.3.0-n1024-b3-seed42.bin')

    def test_verified_reference_records_sha256(self):
        with tempfile.TemporaryDirectory() as folder:
            case,path,row=self.fixture(Path(folder))
            self.assertEqual(p.validate_reference(row,case,Path(folder))['sha256'],p.sha256(path))

    def test_degree_and_seed_are_not_interchangeable(self):
        with tempfile.TemporaryDirectory() as folder:
            case,path,row=self.fixture(Path(folder))
            for wrong in [(1024,1,42),(512,1,43)]:
                with self.assertRaisesRegex(ValueError,'shape/filename'):
                    p.validate_reference(row,wrong,Path(folder))

    def test_reference_requires_fresh_generation_and_all_checks(self):
        with tempfile.TemporaryDirectory() as folder:
            case,path,row=self.fixture(Path(folder))
            for flag in ['fresh_uncached_generation','cache_roundtrip_validated','upstream_verified','ct_conversion_verified','native_preflight_verified']:
                altered={**row,flag:False}
                with self.assertRaisesRegex(ValueError,'independent validation'):
                    p.validate_reference(altered,case,Path(folder))

    def test_changed_trailing_checksum_rejected(self):
        with tempfile.TemporaryDirectory() as folder:
            case,path,row=self.fixture(Path(folder))
            payload=bytearray(path.read_bytes());payload[-1]^=1;path.write_bytes(payload)
            with self.assertRaisesRegex(ValueError,'stored cache checksum'):
                p.validate_reference(row,case,Path(folder))

    def test_matrix_is_exactly480(self):
        self.assertEqual(len(p.DEGREES)*len(p.BATCHES)*len(p.SEEDS),480)

if __name__=='__main__':unittest.main()

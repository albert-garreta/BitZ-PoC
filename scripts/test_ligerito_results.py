import copy
import unittest
from ligerito_results import VERSION, validate_ligerito, validate_result_fields
from native_mul_table import f2z_caption


def report(johnson=True):
    """Small metadata fixture; cryptographic validation lives in Rust tests."""
    return dict(protocol_version=VERSION, requested_profile="custom:3:4" if johnson else "udrg:3:4",
                resolved_profile="custom:3:4" if johnson else "udrg:3:4", configuration_fingerprint="a"*64,
                regime="johnson" if johnson else "udr", target_bits=100, outer_ood=johnson,
                outer_ood_grinding_bits=0 if johnson else None, outer_ood_raw_bits=105 if johnson else None,
                recursive_ood=[0, 1 if johnson else 0],
                configuration=dict(hash="blake3", target_security_bits=100,
                    levels=[dict(regime="johnson_ood" if johnson else "udr", ood_samples=n) for n in [0, 1 if johnson else 0]]))


class LigeritoResultsTests(unittest.TestCase):
    def test_both_regimes_and_actual_caption(self):
        for johnson in [True, False]:
            r=report(johnson)
            validate_ligerito(r,100)
            caption=f2z_caption([dict(backend="f2z",config=dict(ligerito=r))])
            self.assertIn("Johnson" if johnson else "unique decoding radius",caption)
            self.assertIn(r["resolved_profile"],caption)
            if not johnson: self.assertNotIn("Johnson",caption)

    def test_missing_historical_and_conflicting_metadata_rejected(self):
        for key,value in [("protocol_version","old"),("configuration_fingerprint",None),("outer_ood",False),
                          ("target_bits",106),("recursive_ood",[0,0]),("requested_profile","")]:
            r=report();r[key]=value
            with self.assertRaises(ValueError):validate_ligerito(r,100)
        with self.assertRaises(ValueError):validate_ligerito(None)
        with self.assertRaises(ValueError):
            f2z_caption([dict(backend="f2z",config=dict(ligerito=report(j))) for j in [True,False]])

    def test_versioned_result_encoding_and_historical_rejection(self):
        import json
        identity=json.dumps(report()).encode().hex()
        self.assertEqual(validate_result_fields(dict(schema="f2z/2",ligerito_hex=identity)),report())
        for fields in [dict(schema="f2z/1",ligerito_hex=identity),dict(schema="f2z/2"),dict(schema="f2z/2",ligerito_hex="xyz")]:
            with self.assertRaises(ValueError):validate_result_fields(fields)

    def test_other_backends_do_not_need_ligerito_metadata(self):
        self.assertNotIn("Johnson",f2z_caption([dict(backend="binius64",config=dict(pcs="BaseFold"))]))

if __name__ == "__main__": unittest.main()

import json
from pathlib import Path
import tempfile
import unittest

from calibrate_unified import KEYS, passed


class CalibrationCoverage(unittest.TestCase):
    def check_rows(self, rows):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "results.json"
            path.write_text(json.dumps(rows))
            return passed(path)

    def rows(self):
        return [dict(family=key.split("/")[0], size=key.split("/")[1],
                     status="pass", complete=True, correctness=True,
                     allocations_ok=True, variant="production_repeat", baseline="production")
                for key in KEYS]

    def test_complete_independent_control_can_qualify(self):
        self.assertTrue(self.check_rows(self.rows()))

    def test_missing_duplicate_and_automatic_self_comparison_cannot_qualify(self):
        rows = self.rows()
        self.assertFalse(self.check_rows(rows[:-1]))
        self.assertFalse(self.check_rows(rows[:-1] + [rows[0]]))
        rows[0]["variant"] = "production"
        self.assertFalse(self.check_rows(rows))

    def test_failure_or_inconclusive_case_cannot_be_dropped(self):
        for field, value in [("status", "inconclusive"), ("status", "regression"),
                             ("correctness", False), ("allocations_ok", False),
                             ("complete", False)]:
            rows = self.rows()
            rows[0][field] = value
            self.assertFalse(self.check_rows(rows))


if __name__ == "__main__":
    unittest.main()

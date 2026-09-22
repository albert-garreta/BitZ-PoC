import json
import tempfile
import unittest
from pathlib import Path

import mul_table


class ExclusionLoading(unittest.TestCase):
    def records(self):
        return [
            dict(workload="u64", scheme="binius64@1", log_n=21, threads=10, status="excluded",
                 reason="killed", observed_peak_rss_bytes=20 * 2**30, machine_ram_bytes=24 * 2**30),
            dict(workload="u64", scheme="bitz@1", log_n=23, threads=10, status="measured", reason=""),
            dict(workload="u128", scheme="binius64@1", log_n=21, threads=10, status="excluded", reason="swap"),
        ]

    def test_jsonl_keeps_only_excluded_records_of_the_workload(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "exclusions.jsonl"
            path.write_text("".join(json.dumps(r) + "\n" for r in self.records()))
            kept = mul_table.load_exclusions(path, "u64")
            self.assertEqual([(e["scheme"], e["log_n"]) for e in kept], [("binius64@1", 21)])
            self.assertEqual(mul_table.load_exclusions(path, "u32-mod32"), [])
            path.write_text("")
            self.assertEqual(mul_table.load_exclusions(path, "u64"), [])

    def test_json_list_without_status_is_taken_as_excluded(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "exclusions.json"
            entries = [{k: v for k, v in r.items() if k != "status"} for r in self.records()]
            path.write_text(json.dumps(entries))
            kept = mul_table.load_exclusions(path, "u64")
            self.assertEqual({e["log_n"] for e in kept}, {21, 23})
            del entries[0]["reason"]
            path.write_text(json.dumps(entries))
            with self.assertRaises(ValueError):
                mul_table.load_exclusions(path, "u64")


if __name__ == "__main__":
    unittest.main()

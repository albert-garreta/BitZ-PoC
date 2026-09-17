import copy
import json
from pathlib import Path
import tempfile
import unittest
from mul_results import load, aggregate, percentile
from mul_report import main


def fixture(directory, configurations=(1,)):
    entries, records = [], []
    for i, width in enumerate(configurations):
        identifier = f"case-{i}"
        case = dict(mode="proof", workload="u64", backend="f2z", log_n=15, seed=7, threads=1,
                    f2z=dict(w=width, split=0, profile=100, bound="johnson", ligerito="custom:1:4"))
        entries.append(dict(job=dict(id=identifier, case=case, reps=2, warmups=1, memory="rss", skip=None),
                            status="measured", effective=dict(boundary="standalone-proving", corpus_digest="abc")))
        for kind, index, value in [("warmup", 0, 10000), ("sample", 0, 10), ("sample", 1, 20)]:
            records.append(dict(case_id=identifier, kind=kind, index=index, verified=True,
                                metrics=dict(online_prover_ms=value, commit_ms=1, verify_ms=2, proof_bytes=1024)))
        records.append(dict(case_id=identifier, kind="rss", index=0, verified=True, metrics=dict(peak_rss_bytes=4096)))
    manifest = dict(schema="mul-bench/v1", status="complete", provenance=dict(executable_blake3="abc"), cases=entries)
    (directory / "manifest.json").write_text(json.dumps(manifest))
    (directory / "samples.jsonl").write_text("".join(json.dumps(r) + "\n" for r in records))
    return manifest, records


class Reports(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.path = Path(self.temp.name)

    def rewrite(self, manifest, records):
        (self.path / "manifest.json").write_text(json.dumps(manifest))
        (self.path / "samples.jsonl").write_text("".join(json.dumps(r) + "\n" for r in records))

    def test_even_median_type7_and_warmups(self):
        fixture(self.path)
        rows = aggregate(load(self.path))
        metric = rows[0]["metrics"]["online_prover_ms"]
        self.assertEqual(metric["median"], 15)
        self.assertEqual(metric["p05"], 10.5)
        self.assertEqual(metric["p95"], 19.5)
        self.assertEqual(metric["count"], 2)
        self.assertEqual(rows[0]["memory"]["peak_rss_bytes"], 4096)
        self.assertEqual(percentile([10, 20, 30, 40], .25), 17.5)

    def test_complete_identity_separates_packing_profile_split_threads(self):
        manifest, records = fixture(self.path, (1, 3, 8, 5))
        manifest["cases"][1]["job"]["case"]["threads"] = 8
        manifest["cases"][2]["job"]["case"]["f2z"]["profile"] = 128
        manifest["cases"][3]["job"]["case"]["f2z"]["split"] = 1
        self.rewrite(manifest, records)
        cases = load(self.path)
        self.assertEqual(len({c.identity for c in cases}), 4)
        self.assertEqual(len(aggregate(cases)), 4)

    def test_duplicate_missing_unverified_unknown_and_nonfinite_rejected(self):
        manifest, records = fixture(self.path)
        changes = [records + [records[1]], records[:2] + records[3:]]
        for field, value in [("case_id", "unknown"), ("verified", False), ("index", True), ("kind", "unknown")]:
            bad = copy.deepcopy(records)
            bad[1][field] = value
            changes.append(bad)
        bad = copy.deepcopy(records)
        bad[1]["metrics"]["online_prover_ms"] = float("nan")
        changes.append(bad)
        for bad in changes:
            self.rewrite(manifest, bad)
            with self.assertRaises(ValueError):
                load(self.path)

    def test_required_metrics_and_configuration(self):
        manifest, records = fixture(self.path)
        for metric in ("proof_bytes", "online_prover_ms", "verify_ms"):
            bad = copy.deepcopy(records)
            for record in bad:
                record["metrics"].pop(metric, None)
            self.rewrite(manifest, bad)
            with self.assertRaisesRegex(ValueError, "required metrics"):
                load(self.path)
        del manifest["cases"][0]["job"]["case"]["f2z"]["ligerito"]
        self.rewrite(manifest, records)
        with self.assertRaisesRegex(ValueError, "configuration"):
            load(self.path)

    def test_skip_requires_reason_and_has_no_samples(self):
        manifest, records = fixture(self.path)
        entry = manifest["cases"][0]
        entry["status"] = "skipped"
        entry["reason"] = "unsupported geometry"
        self.rewrite(manifest, [])
        self.assertEqual(aggregate(load(self.path))[0]["status"], "skipped")
        self.rewrite(manifest, records)
        with self.assertRaises(ValueError):
            load(self.path)

    def test_renderers_share_validated_statistics(self):
        fixture(self.path, (1, 3))
        out = self.path / "report"
        self.assertEqual(main([str(self.path), "--out", str(out)]), 0)
        self.assertTrue((out / "online_prover_ms.svg").exists())
        self.assertIn("15", (out / "summary.csv").read_text())
        self.assertEqual(len(json.loads((out / "summary.json").read_text())), 2)
        self.assertIn("w=3", (out / "table.md").read_text())
        self.assertTrue((out / "table.tex").exists())

    def test_incomplete_campaign_and_duplicate_case_rejected(self):
        manifest, records = fixture(self.path)
        manifest["status"] = "running"
        self.rewrite(manifest, records)
        with self.assertRaises(ValueError):
            load(self.path)
        manifest["status"] = "complete"
        manifest["cases"].append(copy.deepcopy(manifest["cases"][0]))
        self.rewrite(manifest, records)
        with self.assertRaises(ValueError):
            load(self.path)


if __name__ == "__main__":
    unittest.main()

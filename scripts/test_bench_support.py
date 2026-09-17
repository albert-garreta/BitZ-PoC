import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch
import bench_support as support


class Support(unittest.TestCase):
    def test_artifact_discovery_is_structured_and_complete(self):
        event = dict(reason="compiler-artifact", target=dict(name="mul"), executable="/tmp/bin with spaces")
        text = "compiler output\n" + json.dumps(event)
        self.assertEqual(support.cargo_executables(text, ["mul"])["mul"], "/tmp/bin with spaces")
        with self.assertRaises(RuntimeError):
            support.cargo_executables(text, ["other"])
        with self.assertRaises(ValueError):
            support.cargo_executables(text + "\n" + json.dumps({**event, "executable": "/tmp/other"}))

    def test_build_environment_values_are_preserved(self):
        env = dict(RUSTFLAGS="-C target-cpu=native --cfg custom", CARGO_ENCODED_RUSTFLAGS="-C\x1fopt-level=3",
                   CARGO_TARGET_DIR="/tmp/build", F2Z_BENCH_SEED="7", SECRET_TOKEN="hidden")
        filtered = support.environment(env)
        self.assertEqual(filtered, {k: v for k, v in env.items() if k != "SECRET_TOKEN"})

    def test_failed_and_timed_out_processes_keep_logs_and_are_reaped(self):
        with tempfile.TemporaryDirectory() as temp:
            log = Path(temp) / "worker.log"
            with self.assertRaises(RuntimeError):
                support.run_logged([sys.executable, "-c", "print('before failure'); raise SystemExit(3)"], dict(os.environ), log)
            self.assertIn("before failure", log.read_text())
        process = unittest.mock.Mock(pid=123)
        process.wait.side_effect = [subprocess.TimeoutExpired("worker", 1), -9]
        with patch.object(support.subprocess, "Popen", return_value=process), patch.object(support.os, "killpg") as kill:
            self.assertEqual(support.run_process(["worker"], timeout=1), (None, True))
            kill.assert_called_once()
            self.assertEqual(process.wait.call_count, 2)

    def test_atomic_json_and_file_hash(self):
        with tempfile.TemporaryDirectory() as temp:
            path = Path(temp) / "result.json"
            support.write_json(path, {"a": 1})
            first = support.file_hash(path)
            support.write_json(path, {"a": 2})
            self.assertNotEqual(support.file_hash(path), first)
            self.assertEqual(json.loads(path.read_text()), {"a": 2})
            self.assertFalse(path.with_suffix(".json.tmp").exists())


if __name__ == "__main__":
    unittest.main()

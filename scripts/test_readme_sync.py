import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

import sync_readme_campaigns as sync

ROOT = Path(__file__).resolve().parents[1]


class ReadmeSyncTests(unittest.TestCase):
    def test_readme_carries_the_instructions_verbatim(self):
        proc = subprocess.run([sys.executable, str(ROOT / "scripts/sync_readme_campaigns.py"), "--check"],
                              capture_output=True, text=True)
        self.assertEqual(proc.returncode, 0, proc.stderr)
        readme = (ROOT / "README.md").read_text()
        block = readme[readme.index(sync.BEGIN) + len(sync.BEGIN):readme.index(sync.END)]
        self.assertIn("### 1. SHA-256 + ECDSA over secp256k1", block)
        self.assertIn("### 2. SHA-256 + ECDSA over P-256: BitZ alone", block)
        self.assertIn("### 4. Multiplication comparisons", block)
        self.assertIn("### Tables and figures", block)

    def test_splice_is_idempotent_and_check_detects_drift(self):
        source = "# Title\n\nintro\n\n## Setup\n\n```bash\necho hi\n```\n\n## 1. First\n\ntext\n"
        readme = f"# Repo\n\nbefore\n\n{sync.BEGIN}\nstale\n{sync.END}\n\nafter\n"
        once = sync.spliced(readme, sync.body(source))
        self.assertEqual(once, sync.spliced(once, sync.body(source)))
        self.assertIn("### Setup", once)
        self.assertIn("### 1. First", once)
        self.assertNotIn("stale", once)
        self.assertTrue(once.startswith("# Repo\n\nbefore\n") and once.endswith("\n\nafter\n"))
        with tempfile.TemporaryDirectory() as path:
            readme_path, source_path = Path(path) / "README.md", Path(path) / "BENCH_INSTRUCTIONS.md"
            readme_path.write_text(readme)
            source_path.write_text(source)
            self.assertEqual(sync.main(["--check", "--readme", str(readme_path), "--source", str(source_path)]), 1)
            self.assertEqual(sync.main(["--readme", str(readme_path), "--source", str(source_path)]), 0)
            self.assertEqual(sync.main(["--check", "--readme", str(readme_path), "--source", str(source_path)]), 0)


if __name__ == "__main__":
    unittest.main()

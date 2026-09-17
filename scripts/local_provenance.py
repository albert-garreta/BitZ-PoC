"""Record local vendor snapshots and pinned upstream Git dependencies."""
from pathlib import Path
import subprocess
import tomllib


def vendor_snapshots(root: Path) -> dict:
    snapshots = tomllib.loads((root / "provenance.toml").read_text())
    for record in snapshots.values():
        if "git" in record:
            continue
        repository = root / record["path"]
        if not (repository / ".git").is_dir():
            raise ValueError(f"Missing vendor repository: {record['path']}; restore it including its .git directory")
        def git(*args):
            return subprocess.check_output(["git", "--no-optional-locks", "-C", str(repository), *args], text=True).strip()
        record["expected_snapshot_commit"] = record["snapshot_commit"]
        record["snapshot_commit"] = git("rev-parse", "HEAD")
        record["dirty"] = bool(git("status", "--porcelain", "--untracked-files=no"))
    return snapshots

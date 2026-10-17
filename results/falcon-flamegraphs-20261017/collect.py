"""Copy the committed subset of this campaign into results/falcon-flamegraphs-20261017."""

import shutil
from pathlib import Path

SRC = Path(__file__).parent
DST = SRC.parents[1] / "results" / "falcon-flamegraphs-20261017"

TOP = [
    "flame.py", "build_page.py", "samply2folded.py", "validate.py", "compare_trace.py",
    "show_tree.py", "debug_other.py", "test_short.py", "collect.py",
    "profile-t1.zsh", "profile-rerun.zsh", "profile-samply.zsh", "profile-samply-onelimb.zsh",
    "profile-t16.zsh", "profile-t16-rerun.zsh", "build-and-profile-t16.zsh", "trace-t1.zsh",
    "t10.zsh", "sizes-run.zsh", "build-aerie.zsh",
    "notes.json", "size_notes.json", "threads_notes.json",
    "profiling-additions.patch", "aerie-profiling.patch", "aerie-commit.txt",
]
PAGE = ["index.html", "template.html", "snap.swift", "wrap_preview.py"]
SAMPLY_SUFFIXES = (".out", ".err", ".clock.out", ".clock.err", ".samply.folded", ".binary.txt")


def copy(rel: str):
    src, dst = SRC / rel, DST / rel
    dst.parent.mkdir(parents=True, exist_ok=True)
    shutil.copy2(src, dst)


for name in TOP:
    copy(name)
for name in PAGE:
    copy(f"page/{name}")
for path in sorted((SRC / "out-samply").iterdir()):
    copy(f"out-samply/{path.name}")
for path in sorted((SRC / "samply").iterdir()):
    if ".disturbed." in path.name:
        continue
    if path.name == "clock.log" or path.name.endswith(SAMPLY_SUFFIXES):
        copy(f"samply/{path.name}")
for name in ["aerie.clock.out", "aerie.clock.err", "bitz-shipped.clock.out", "bitz-shipped.clock.err", "clock.log"]:
    copy(f"samples/{name}")
for folder in ["t10", "sizes", "trace"]:
    for path in sorted((SRC / folder).iterdir()):
        copy(f"{folder}/{path.name}")

total = sum(p.stat().st_size for p in DST.rglob("*") if p.is_file())
count = sum(1 for p in DST.rglob("*") if p.is_file())
print(f"{DST}: {count} files, {total / 1e6:.1f} MB")

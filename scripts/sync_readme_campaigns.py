#!/usr/bin/env python3
"""Keep README.md's benchmark section identical to BENCH_INSTRUCTIONS.md.

BENCH_INSTRUCTIONS.md is the single source of the campaign instructions. This
splices its body (everything after its first heading) into README.md between
the markers below, so the development README and the artifact README carry
the same text; `--check` exits 1 when they differ, which the unit tests run.

    python3 scripts/sync_readme_campaigns.py            # rewrite README.md
    python3 scripts/sync_readme_campaigns.py --check    # verify only
"""
from __future__ import annotations

import argparse
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
BEGIN = "<!-- bench-campaigns:begin (generated from BENCH_INSTRUCTIONS.md by scripts/sync_readme_campaigns.py; edit that file) -->"
END = "<!-- bench-campaigns:end -->"


def body(source: str) -> str:
    """The instructions without their top-level title, headings demoted one level."""
    lines = source.splitlines()
    while lines and not lines[0].startswith("# "):
        lines.pop(0)
    if lines:
        lines.pop(0)
    out = []
    for line in lines:
        if line.startswith("## "):
            out.append("#" + line)
        else:
            out.append(line)
    return "\n".join(out).strip("\n") + "\n"


def spliced(readme: str, block: str) -> str:
    start = readme.index(BEGIN)
    end = readme.index(END, start)
    return readme[: start + len(BEGIN)] + "\n" + block + END + readme[end + len(END):]


def main(argv=None) -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--check", action="store_true")
    parser.add_argument("--readme", type=Path, default=ROOT / "README.md")
    parser.add_argument("--source", type=Path, default=ROOT / "BENCH_INSTRUCTIONS.md")
    args = parser.parse_args(argv)
    readme = args.readme.read_text()
    if BEGIN not in readme or END not in readme:
        print(f"{args.readme} lacks the bench-campaigns markers", file=sys.stderr)
        return 2
    updated = spliced(readme, body(args.source.read_text()))
    if updated == readme:
        return 0
    if args.check:
        print(f"{args.readme} is out of sync with {args.source}; run scripts/sync_readme_campaigns.py", file=sys.stderr)
        return 1
    args.readme.write_text(updated)
    print(f"updated {args.readme}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

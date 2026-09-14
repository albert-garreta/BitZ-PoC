#!/usr/bin/env python3
"""Capture selected ARM kernel assembly for manual dataflow review, not a CT proof."""
import argparse
import hashlib
import json
import re
import subprocess
from pathlib import Path


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("binary", type=Path)
    parser.add_argument("--out", required=True, type=Path)
    args = parser.parse_args()
    binary = args.binary.resolve()
    tool = subprocess.check_output(["xcrun", "--find", "llvm-objdump"], text=True).strip()
    symbols = subprocess.check_output([tool, "--syms", str(binary)], text=True)
    header = "\n".join(symbols.splitlines()[:5])
    if "arm64" not in header and "aarch64" not in header:
        parser.error("This audit captures only ARM binaries")
    selected = set()
    for line in symbols.splitlines():
        if "candidates" not in line or "__text" not in line:
            continue
        symbol = line.split()[-1]
        if ("7native4" in symbol or ("10Projection" in symbol and "5batch" in symbol)
                or "5prime5batchKb1_" in symbol
                or ("10polynomial5fixed" in symbol)
                or "13exact_columns" in symbol or "exact_signed_columns" in symbol
                or ("4grid" in symbol and ("fold_tile" in symbol or "accumulate" in symbol))
                or "4dot3" in symbol):
            selected.add(symbol)
    if not selected:
        parser.error("No candidate symbols found; inspect the compiler's symbol naming")
    args.out.mkdir(parents=True, exist_ok=False)
    items = []
    for index, symbol in enumerate(sorted(selected)):
        assembly = subprocess.check_output(
            [tool, "--disassemble-symbols=" + symbol, str(binary)], text=True)
        if f"<{symbol}>:" not in assembly:
            raise ValueError(f"Disassembler did not return the requested function: {symbol}")
        path = args.out / f"kernel-{index:02}.asm"
        path.write_text(assembly)
        instructions = [line for line in assembly.splitlines()
                        if re.search(r"\b(?:[usf]div|b\.\w+|cbnz|cbz|tbnz|tbz|bl)\b", line)]
        items.append(dict(symbol=symbol, assembly=path.name,
                          assembly_sha256=hashlib.sha256(path.read_bytes()).hexdigest(),
                          instructions_requiring_review=instructions))
    report = dict(binary=str(binary), binary_sha256=hashlib.sha256(binary.read_bytes()).hexdigest(),
                  tool=tool, format=header, kernels=items,
                  scope="Manual ARM instruction/dataflow review; no automatic constant-time verdict")
    (args.out / "index.json").write_text(json.dumps(report, indent=2) + "\n")
    print(f"Captured {len(items)} kernels in {args.out}")


if __name__ == "__main__":
    main()

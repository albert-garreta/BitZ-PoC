# Baseline V4 versus candidate V6 grinding disassembly

The disassembly does not attribute the measured prover slowdown to changed scanner instructions. All six inspected functions have exactly equal normalized instruction sequences, register choices, immediates, stack offsets, branch destinations relative to functions, and function sizes. Only relocated addresses and RIP-relative displacements differ. No production source was changed.

Binaries: `/tmp/falcon-compact-opening-binaries/baseline-v4` and `/tmp/falcon-compact-opening-binaries/candidate-v6`. Exact SHA256 values and symbol metadata are in `metadata.json`. Host CPU: AMD Ryzen 9 9950X3D.

|Function|Instructions, both|Bytes, both|Stack references, both|Entry mod 64 baseline→candidate|
|---|---:|---:|---:|---|
|dispatch|230|967|46|32→16|
|parallel|820|3742|151|0→16|
|worker8|140|573|9|32→48|
|worker10|140|573|9|32→48|
|scanner|1631|9443|227|48→0|
|pow_ok|191|818|53|32→48|

“Stack references” counts static instructions mentioning `%rsp`; it is not an executed spill counter. Those instruction counts, stack allocations, and registers are unchanged. The scanner reserves 0xb80 bytes after 64-byte stack alignment in both binaries.

## Actual hot path

For Spartan 32-byte seeds, `find_grinding_nonce` compares difficulty with 11 and calls `smallest_pow_nonce` at ≥12; the separately named parallel wrapper is inlined. That helper chooses 256-nonce chunks through 16 bits and 1024-nonce chunks thereafter. Native AVX-512 support has been constant-folded in these binaries. Rayon executes the matching chunk8/chunk10 worker closure, which calls the shared scanner. The scanner dispatches prefix 32 to its prepared eight-word kernel. All these branches and calls match.

The vector constants and all referenced five-round schedule slices were independently read from ELF load segments and compared: 52 references match in value and use, including four 640-byte schedule references. Evidence: `scanner-constant-references.json`.

## Placement differences

- The 32-byte kernel’s five-round inner loop moves from baseline 0x1804c0 (64-byte aligned) to candidate 0x181550 (offset 16 modulo 64). It remains 189 instructions with 16 bounds-check branches and one back-edge per round.
- The outer 16-nonce loop start moves from offset 53 to offset 5 modulo 64. The chunk8 worker’s main loop moves from offset 0 to 16.
- These movements could affect instruction-fetch behavior, but static alignment alone is not causal evidence for the observed 3–4% grinding delta. Some boundaries improve while others change in the opposite direction. No alignment patch is recommended.

## Concrete independent optimization opportunity

The fixed AVX-512 prepared kernel uses a runtime loop over `MSG_SCHEDULE[1..6]`. Disassembly shows 16 schedule-index loads and 16 bounds-check branches per round: 80 checks per 16-nonce vector, plus dynamic indexing into the vector message array. These are public compile-time constants. A narrowly scoped prototype can express the five rounds using a const round index, allowing those checks and dynamic loads to disappear while retaining the identical BLAKE3 G operations, digest predicate, minimum nonce, and full-hash fallback for difficulties above 32. This would remove demonstrated existing work rather than compensate for unexplained code placement. Its speed and code-size effects must be measured before any production change.

## Reproduction

Symbols were extracted with `nm -S -C`; each function was disassembled using `objdump -d -C --no-show-raw-insn --start-address=... --stop-address=...`. The stored `.asm` files retain raw virtual addresses. `.normalized` files remove only instruction addresses, replace RIP displacements with a token, and replace resolved direct target addresses while retaining symbol names/offsets. Every `.diff` file is empty. Constants were checked separately so relocation normalization does not hide changed BLAKE3 constants.

# Zinc+ row of the u32 multiplication table

`f2z_u32_mod32.rs` is a bench for [zinc-plus](https://github.com/NethermindEth/zinc-plus)
at `origin/main-beta` (609c18c). It proves this repository's u32 mod-2^32
corpus — the operands of `native-mul/mod32/inputs/v1` — as one integer
constraint per multiplication, `x·y = z + 2^32·w`, over eight int columns of
16-bit limbs, every column range-checked by a `Word { width: 16 }` GKR-LogUp
lookup, and prints the row digest it actually proved so the table can check it
against the other schemes.

It lives here, not in `protocol/benches/`, because the two crates pin
incompatible `crypto-bigint` releases and cannot be linked together. To run
it, copy it into a zinc-plus checkout; the commands are in
`BENCH_INSTRUCTIONS.md`, the design and the CHECKED-validated geometry in
`docs/native-mul-compare.md`.

# Zinc+ rows of the multiplication tables

Two benches for [zinc-plus](https://github.com/NethermindEth/zinc-plus) at
`878fbd8292472dcb13b25e2c9c0209406b5fb671` (branch `main-beta`), proving this
repository's multiplication corpora so every scheme in a table proves the
same operands:

- `bitz_u32_mod32.rs` — the u32 row: `x·y = z + 2^32·w` over eight int
  columns of 16-bit limbs (`native-mul/mod32/inputs/v1` corpus, regenerated
  in the bench from its blake3-XOF domain).
- `bitz_wide_mul.rs` — the u64 and u128 rows: `x·y = z` over 16 or 32 int
  columns of 16-bit limbs (the full product in `2L` limbs), reading the
  operands from the corpus manifest `examples/mul_corpus_export` writes and
  refusing to report unless the proved limbs recombine to the BitZ
  assignment digest of that corpus.

Every column carries a `Word { width: 16 }` GKR-LogUp lookup, Zip+/IPRS
over F65537 at rate 1/4 with the openings for 100 bits, and a 128-bit
projecting prime drawn from the transcript. Zinc+ reports no security
accounting of its own; each run prints the inverse rate, column openings,
prime width, grinding bits and the LogUp range-check term, which the
importer records with the row.

They live here, not in `protocol/benches/`, because the two crates pin
incompatible `crypto-bigint` releases (`= 0.7.0-rc.9` there, `0.7.5` in
`vendor/field`) and cannot be linked together — verified: Cargo cannot
resolve a graph containing both. `scripts/run_zinc_plus_campaign.py` pins a
checkout, copies the benches in, builds single-threaded and `parallel`
binaries, validates the integer-lane bounds once in CHECKED mode, runs one
process per case under `/usr/bin/time -l`, and imports the logs into a
`mul-bench/v2` campaign (`scripts/zinc_plus_import.py`); see
[BENCH_INSTRUCTIONS.md](../../BENCH_INSTRUCTIONS.md), campaign 5.

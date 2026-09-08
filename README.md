
# BitZ 🫜 --- README for normal humans

The core BitZ PCS proves

```
MLE[w](r) = y ∈ F_q
```

for w a vector of bits.

Run it with:


```sh
RUSTFLAGS="-C target-cpu=native" cargo run --release --features unchecked -- 24
RUSTFLAGS="-C target-cpu=native" cargo run --release --features unchecked -- \
    28 --threads 1 --reps 5 --profile custom:3:4
```

`n` is log(|w|)

This repo also contains BitZ-SNARK, a SNARK for proving R1CS constraints over the integers, using BitZ as its PCS.


## Reproducing the paper's benchmarks

### Raw performance of BitZ PCS on the core LinBitsRings relation

```sh
RUSTFLAGS="-C target-cpu=native" cargo run --release --features unchecked -- \
    --sweep 20-30 --threads 8 --reps 5 --profile custom:3:4
```

### Integer multiplication

```sh
RUSTFLAGS="-C target-cpu=native" cargo run --release --features unchecked -- \
    --mul-sweep 15-22 --threads 8 --reps 5 --profile custom:3:4 --cooldown 20
```

### MultiSwap (Limber's Table 1 workload: 4 exponentiations with 352-bit exponents modulo a 2048-bit RSA modulus + Poseidon-based hash-to-prime; the 6209-row integer Mod-R1CS from Limber's repo), λ=114

```sh
# single-threaded (RAYON_NUM_THREADS=8 for the 8-thread column)
F2Z_BENCH_LAMBDA=114 F2Z_BENCH_REPS=5 RAYON_NUM_THREADS=1 RUSTFLAGS="-C target-cpu=native" \
  cargo bench --bench multiswap --features unchecked
```

Limber and Zinc+ rows: see [MultiSwap comparison rows](#multiswap-comparison-rows-limber-zinc) under Scratch.

# Scratch

## MultiSwap comparison rows (Limber, Zinc+)

Same box (Apple M4, 4P+6E cores, 16 GB), `-C target-cpu=native`, medians of 5, 1 thread / 8 rayon threads; prover time includes commitment, excludes witness generation (< 0.15 s everywhere). LaTeX table: `paper/multiswap-table.tex`.

| System (commit) | Prove 1 thr | Prove 8 thr | Verify 1 thr | Verify 8 thr | Proof |
|---|---|---|---|---|---|
| BitZ-SNARK (`ca52890`), 114 bits | 257 ms | 103 ms | 9.8 ms | 12.0 ms | 338 KB |
| Zinc+ main-beta (`878fbd8`), 16-bit limbs + range checks, 114 bits (14 grinding bits) | 2052 ms | 563 ms | 18.2 ms | 12.7 ms | 1272 KB (847 KiB zstd) |
| Zinc+ main-beta (`878fbd8`), fat-cell mock, no range checks, 100 bits | 831 ms | 236 ms | 58.0 ms | 24.6 ms | 1415 KB |
| Limber-Brakedown (`b003684`) | 1175 ms | 559 ms | 44.2 ms | 43.3 ms | 5769 KB |
| Limber-Hyrax (`b003684`) | 1206 ms | 390 ms | 37.1 ms | 20.5 ms | 175 KB |

Limber — [albert-garreta/limber-impl](https://github.com/albert-garreta/limber-impl) `b003684` (fork of lucasxia01/limber-impl `853c6c4`; Rust ≥ 1.97). `MSCFG=paper` is required: the default `full` is a newer 2^14-row circuit, not the Table 1 statement. `RAYON_NUM_THREADS=8` for the 8-thread columns.

```sh
MSCFG=paper RAYON_NUM_THREADS=1 RUSTFLAGS="-C target-cpu=native" cargo bench --bench multiswap_modp           # Hyrax (Criterion; prove/ = commit+prove)
MSCFG=paper PSIZE=1 RAYON_NUM_THREADS=1 RUSTFLAGS="-C target-cpu=native" cargo bench --bench multiswap_modp   # Hyrax proof size
MSCFG=paper BDPCS=1 RAYON_NUM_THREADS=1 RUSTFLAGS="-C target-cpu=native" cargo bench --bench multiswap_modp   # Brakedown (one run; repeat 5×)
```

Zinc+ — [NethermindEth/zinc-plus](https://github.com/NethermindEth/zinc-plus) branch `main-beta` `878fbd8`; `Cargo.lock` is untracked, pin `crypto-primitives` to `2cf39db886a76dc3e961cbb9c86fb5ab042381ef` (`cargo update -p crypto-primitives --precise …`). For the 8-thread columns add the `parallel` feature and `RAYON_NUM_THREADS=8`. Methodology: `docs/limber-2026-1635-sound-row.md` there.

```sh
LIMB16=1 NVARS=13 REPS=5 RUSTFLAGS="-C target-cpu=native" cargo bench --features "simd unchecked iprs-rate-1-8 sec-114" --bench limber_multiswap   # range-checked, 114 bits
FOLD=1 WIDE16=1 NVARS=11 REPS=5 RUSTFLAGS="-C target-cpu=native" cargo bench --features "simd unchecked iprs-rate-1-8" --bench limber_multiswap   # fat-cell mock, 100 bits
```

## Integer R1CS with F_2 virtualization

Pick the security parameter with `F2Z_BENCH_LAMBDA`. Every bench below
honours it, so one bench can be run at exactly one λ:

| `F2Z_BENCH_LAMBDA` | profile | meaning |
|---|---|---|
| `100` | `Lambda100` | no grinding anywhere; every term this crate controls ≥ 100 bits |
| `128` | `Lambda128` | every controllable term ≥ 128 bits (two grinding bits per forest round, one at the ring switch; the GF(2^128) floor at ~126.4 still binds and is reported) |
| `114` | `Limber114` | the two-prime MultiSwap/Limber comparison target — MultiSwap only |
| `sha128-reference-schedule` | `Sha128ReferenceSchedule` | the historical SHA-256 128-bit schedule, kept for comparison |

The profile names are accepted too (`F2Z_BENCH_LAMBDA=lambda128`). Unset,
SHA-256 and u32×u32 run at λ=100, MultiSwap at 114, and the BabyBear and
`lambda_sweep` benches run every profile they know (two and three rows per
shape). A profile the bench's relation cannot instantiate — `114` outside
MultiSwap, or `100`/`128` on MultiSwap — aborts up front with the
admissible list. Each `RESULT` line carries `profile=<name>` next to
`lambda=<bits>`. `F2Z_BENCH_QUIET=1` mutes the benches' advisory
`warning:` lines (e.g. the `pcs` bench's note that it ignores
`F2Z_BENCH_LAMBDA`); errors still abort.



### SHA-256 independent compressions — λ=100 bits of security (`F2Z_BENCH_LAMBDA=128` for the 128-bit profile)
```sh
F2Z_BENCH_LAMBDA=100 F2Z_BENCH_SHAPES=14 F2Z_BENCH_REPS=3 RUSTFLAGS="-C target-cpu=native" \
  cargo bench --bench sha256_compressions --features unchecked
```

Size the same benchmark by the packed assignment domain (`MnumRows=2^n`)
instead of a power-of-two compression count with:

```sh
F2Z_BENCH_LAMBDA=100 F2Z_SHA_MNUMROWS_LOG2S="24 25" F2Z_BENCH_REPS=3 RUSTFLAGS="-C target-cpu=native" \
  cargo bench --bench sha256_compressions --features unchecked
```

This uses `floor((2^n - 1) / 20456)` compressions: one shared constant,
20,456 adjacent assignment cells per compression, and one trailing zero
suffix only.

`F2Z_SHA_OPENING_T=<t>` (compression-count shapes only) gives the opening
an explicit F2Z split of `2^t` rows × `2^(vars − t)` columns. The default
product layout pins `t = min(k, 13)` (rows = instances, columns = the 2^15
local cells), so its read-off vector — the `2^s` ~125-bit integers sent in
the clear — is 327 KB at 2^12–2^13 and doubles per step from 2^14 on. A
larger `t` shrinks that vector but crosses the one-forest cap
(`127 − t − 1 < q_bits`): the opening then runs one merged forest per
weight chunk. `F2Z_SHA_OPENING_LAYOUT=inner` (the default) takes the
inner-sumcheck path for the split; `=product` keeps the direct product
opening and transposes the product tensor instead (instance-major: the 15
local bits plus the low instance bits form the rows, the high instance bits
the columns; `t ≥ 15`), which is the cheap way to get the split. The bench
prints the layout, forest count and read-off width per shape.

### SHA-256 chain — `2^k` CHAINED compressions (a `64·2^k`-byte Merkle–Damgård chain), λ=100:
```sh
F2Z_BENCH_LAMBDA=100 F2Z_BENCH_SHAPES=14 F2Z_BENCH_REPS=3 RUSTFLAGS="-C target-cpu=native" \
  cargo bench --bench sha256_chain --features unchecked
```

The chained counterpart of `sha256_compressions`: `H_{i+1} = Compress(H_i,
M_i)` from the standard initial state, proved as ONE relation whose
intermediate chaining values are witness. Same circuit and 184 linear
constraints per compression, same runtime-prime protocol and direct product
opening; the difference is the F₂ map. Instance `i` commits only its block
and hint bits (6,888 instead of 7,144 — its 256 chaining-state bits are
read straight from instance `i − 1`'s committed output cells through the
chained virtual map, instance 0's from the initial-state constants), and
every instance carries 256 *terminal* rows that are the last instance's
output bits and structurally zero elsewhere. The public statement is the
block sequence plus the digest, bound by the same uniform public-I/O
batching (block slots at every instance; terminal slots equal to the
digest at the last instance, zero before it). The prover-side cost of the
chaining is confined to the ring switch's batching passes, which gain one
rotated-instance term over 264 columns per compression; the forest, the
PIOP and the proof bytes are the independent batch's.
`prepare_sha256_chain_batch_with_profile_and_initial_state` prepares a
chain from any public initial chaining value (a continuation).

### u32×u32 -> u64 — λ=100; exponents ≥ 15:
```sh
F2Z_BENCH_LAMBDA=100 F2Z_BENCH_SHAPES="15 20" F2Z_BENCH_REPS=5 RUSTFLAGS="-C target-cpu=native" \
  cargo bench --bench u32_mul --features unchecked
```

### Babybear mult — λ=100; exponents ≥ 15 (unset `F2Z_BENCH_LAMBDA` = a λ=100 and a λ=128 row per shape):
```sh
F2Z_BENCH_LAMBDA=100 F2Z_BENCH_SHAPES="15 20" F2Z_BENCH_REPS=5 RUSTFLAGS="-C target-cpu=native" \
  cargo bench --bench baby_bear_mul --features unchecked
```

### SHA security-profile sweep: Lambda100 / Sha128ReferenceSchedule / Lambda128 (set `F2Z_BENCH_LAMBDA` for one of them):
```sh
F2Z_BENCH_SHAPES=12 F2Z_BENCH_REPS=3 RUSTFLAGS="-C target-cpu=native" \
  cargo bench --bench lambda_sweep --features unchecked
```

### PCS-only (t:s:W triples; no IOP security profile, so `F2Z_BENCH_LAMBDA` does not apply; profiling stays opt-in here — add OBLONG_PROFILE=1 for the phase line):
```sh
F2Z_BENCH_SHAPES="17:11:1" F2Z_BENCH_REPS=5 RUSTFLAGS="-C target-cpu=native" \
  cargo bench --bench pcs --features unchecked
```

## Dependencies

- https://github.com/albert-garreta/flock-mod
- https://github.com/worldfnd/f2z-benchmark
- [Albert: I'm not sure what this is. Leaving it here just in case] **`crypto-primitives`** — vendored at `vendor/crypto-primitives` (NethermindEth, Apache-2.0; see `vendor/crypto-primitives/VENDORED.md` for
  the pinned revision and the crypto-bigint 0.7.5 / rand 0.10 port).



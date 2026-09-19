# Benchmarks in the paper

All commands run from the repository root. Measurements are serialized by
`scripts/bench_gate.py` (machine lock and swap guard). Campaigns start as soon
as the lock is available, without waiting for a CPU-idle threshold.

```sh
rustup toolchain install 1.98.1
export RUSTFLAGS="-C target-cpu=native"
```

## Campaigns

### Integer multiplication

```sh
python3 scripts/run_multiplication_benchmarks.py compare --output PerfRuns/multiplication -- \
  proof --workload u32-mod32,u64,u128 --backends all --log-n 15,17,19 \
  --threads 1,10 --reps 5 --memory rss --skip-unsupported
```

All experiment settings use the unified benchmark flags after `--`. Choose
`bitz` for standalone experiments, including `piop`, `outer`, and `bounds`.
The launcher builds once, gates measurements, and generates reports under
`<output>/reports`; omission of `--output` selects a fresh timestamped directory.
Use launcher `--dry-run` before `--` to print commands without building, or
benchmark `--dry-run` after it to build and validate the selected cases.

This is an explicit new campaign, not a reconstruction of the historical paper
matrix. Select sizes, profiles, rates, and memory limits for the intended machine;
there are no implicit per-backend size limits. The maintained tools consume only
`mul-bench/v2` results with BitZ configuration names.

### Suite: integer multiplication, SHA+ECDSA, hybrid, MultiSwap

```sh
bash scripts/run_suite_2026_09_13.sh                  # every phase, in order
bash scripts/run_suite_2026_09_13.sh sha-ecdsa        # tab:sha256-ecdsa
bash scripts/run_suite_2026_09_13.sh hybrid-witness   # tab:hybrid
bash scripts/run_suite_2026_09_13.sh hybrid-counts    # tab:hybrid-equal-counts
bash scripts/run_suite_2026_09_13.sh multiswap        # tab:multiswap (BitZ row)
```

### SHA+ECDSA over secp256k1 (Binius64 family only)

```sh
for CURVE in p256 secp256k1; do
  python3 scripts/bench_gate.py run --label "sha-ecdsa-$CURVE" --swap-grow-gb 12 -- \
    python3 scripts/run_sha256_ecdsa_compare.py \
      --output "bench_results/sha-ecdsa-$CURVE" --curve "$CURVE" \
      --methods bitz-split binius64 binius64-ligerito --exponents 4 5 6 7 \
      --binius-rates 1 3 --threads 1 10 --reps 3
done
```

Both families carry both curves; only `spartan-mc` is P-256 only. On secp256k1
BitZ takes a GLV path worth ~20% of its ECDSA verifier, and Binius uses its own
native secp256k1 circuit. `bench_gate` does not wait for sustained idle, so measure both curves
back to back in one window and repeat a slice of the first to confirm the window
held; see [the worker README](benchmarks/binius64/README.md#secp256k1) for what
the curve change does and does not isolate.

### Raw performance of the PCS (tab:bitz-raw-performance)

```sh
cargo run --release --features unchecked -- \
    --sweep 20-30 --sweep-threads 1,10 --reps 5 --profile custom:1:4 \
    --cooldown 30 --rep-cooldown 10
```

Writes `paper/raw-performance-table.tex` directly.

### fields-witch comparison (tab:fields-witch)

```sh
python3 scripts/run_fields_witch_compare.py \
    --sizes 14,16,18,20,22 --threads 1,10 --reps 5 --rates 1,3 \
    --schemes bitz,fields-witch-asm,fields-witch-bitz
```

### Zinc+ row of the u32 table

In a zinc-plus checkout at `origin/main-beta` (609c18c), with this
repository's bench copied in and the lock taken from a working checkout:

```sh
git clone https://github.com/NethermindEth/zinc-plus /tmp/zinc-609c18c
git -C /tmp/zinc-609c18c checkout 609c18c
cp ~/zinc-plus/Cargo.lock /tmp/zinc-609c18c/Cargo.lock
cp benchmarks/zinc-plus/bitz_u32_mod32.rs /tmp/zinc-609c18c/protocol/benches/
printf '\n[[bench]]\nname = "bitz_u32_mod32"\nharness = false\n' >> /tmp/zinc-609c18c/protocol/Cargo.toml
sed -i '' 's/^zstd = "0.13"$/zstd = "0.13"\nblake3 = { workspace = true }/' /tmp/zinc-609c18c/protocol/Cargo.toml

cd /tmp/zinc-609c18c/protocol
CARGO_TARGET_DIR=/tmp/zinc-u32-st cargo bench --no-run --offline \
    --bench bitz_u32_mod32 --features "simd unchecked"
CARGO_TARGET_DIR=/tmp/zinc-u32-mt cargo bench --no-run --offline \
    --bench bitz_u32_mod32 --features "simd unchecked parallel"
```

One process per (size, threads); 1 thread uses the non-parallel build, 10
threads the parallel one. `SEED` is the suite's u32 corpus seed, which the
bench checks by recomputing the table's row digest:

```sh
for E in 15 17 19 21 23; do
  EXPONENT=$E REPS=5 SEED=6139306037344403556 \
    /usr/bin/time -l /tmp/zinc-u32-st/release/deps/bitz_u32_mod32-*[0-9a-f]
  RAYON_NUM_THREADS=10 EXPONENT=$E REPS=5 SEED=6139306037344403556 \
    /usr/bin/time -l /tmp/zinc-u32-mt/release/deps/bitz_u32_mod32-*[0-9a-f]
done
```

The Zinc+ commands above reproduce historical external measurements. Retain
their raw logs separately; the current multiplication reporter does not import
that historical format.

### Zinc+ row of the SHA-256 + ECDSA table

In a zinc-plus checkout at `main-beta` (878fbd8), with this repository's
harness patch applied and the lock taken from a working checkout. `N` is the
number of chained compressions and `NV` the smallest `num_vars` with
`2^NV >= 68N + 4`; the pair must be edited into the source and rebuilt, because
upstream holds the compression count in a compile-time constant:

```sh
git clone https://github.com/NethermindEth/zinc-plus /tmp/zinc-878fbd8
git -C /tmp/zinc-878fbd8 checkout 878fbd8
cp ~/zinc-plus/Cargo.lock /tmp/zinc-878fbd8/Cargo.lock
git -C /tmp/zinc-878fbd8 apply \
    ~/f2z-pcs/benchmarks/zinc-plus/sha256-ecdsa-harness.patch

N=128; NV=14   # 2^7 compressions; 68*128+4 = 8708 rows
sed -i '' "s/^    pub const NUM_COMPRESSIONS: usize = .*/    pub const NUM_COMPRESSIONS: usize = $N;/" \
    /tmp/zinc-878fbd8/test-uair/src/sha256.rs
sed -i '' "s/^    pub const MIN_NUM_VARS: usize = .*/    pub const MIN_NUM_VARS: usize = $NV;/" \
    /tmp/zinc-878fbd8/test-uair/src/sha256.rs

cd /tmp/zinc-878fbd8/protocol
CARGO_TARGET_DIR=/tmp/zinc-sha-st RUSTFLAGS="-C target-cpu=native" \
    cargo bench --no-run --offline --bench e2e --features "simd unchecked"
CARGO_TARGET_DIR=/tmp/zinc-sha-mt RUSTFLAGS="-C target-cpu=native" \
    cargo bench --no-run --offline --bench e2e --features "simd unchecked parallel"
```

Rate 1/4 is the default; add `iprs-rate-1-8` for rate 1/8, which the IPRS NTT
allows only up to `2^13` rows (so not at 128 compressions). `MODE=folded4x`
selects the 4×-folded commitment path of the Zinc+ paper's own headline number;
it stops at `2^12` rows. One process per (size, threads):

```sh
MODE=plain SHA_NV=$NV REPS=3 \
    /usr/bin/time -l /tmp/zinc-sha-st/release/deps/e2e-*[0-9a-f]
RAYON_NUM_THREADS=10 MODE=plain SHA_NV=$NV REPS=3 \
    /usr/bin/time -l /tmp/zinc-sha-mt/release/deps/e2e-*[0-9a-f]
```

Each run prints one `ZINC_TRIAL` line per repetition and one `ZINC_RESULT`
JSON line. See [docs/zinc-plus-sha256-ecdsa.md](docs/zinc-plus-sha256-ecdsa.md)
for the measured numbers and for what the Zinc+ statement does and does not
contain — it is weaker than the one the other schemes in that table prove.

### MultiSwap rows of the other systems

```sh
# Limber (github.com/albert-garreta/limber-impl @ b003684), Brakedown and Hyrax
MSCFG=paper RAYON_NUM_THREADS=1  cargo run --release --bin multiswap
MSCFG=paper RAYON_NUM_THREADS=10 cargo run --release --bin multiswap

# Zinc+ (main-beta 878fbd8), 16-bit limbs with range checks, 114-bit target
LIMB16=1 NVARS=13 REPS=5 RAYON_NUM_THREADS=1 cargo bench --locked \
    --features "simd unchecked iprs-rate-1-8 sec-114" --bench limber_multiswap
LIMB16=1 NVARS=13 REPS=5 RAYON_NUM_THREADS=10 cargo bench --locked \
    --features "simd unchecked iprs-rate-1-8 sec-114 parallel" --bench limber_multiswap
```

## Tables

```sh
python3 scripts/mul_report.py PerfRuns/multiplication --out reports/multiplication

python3 scripts/sha256_ecdsa_table.py bench_results/suite-sha256-ecdsa-<date>

for V in witness counts; do
  python3 scripts/hybrid_table.py --variant $V \
    $(for T in 10 1; do for R in hybrid@1:bitz-r2 hybrid@3:bitz-r8 \
        all-binius@1:bin-r1 all-binius@3:bin-r3 \
        binius-ligerito@1:lig-r1 binius-ligerito@3:lig-r3; do
        echo --row "${R%%:*}:$T=PerfRuns/suite-hy-$V-${R##*:}-t$T"; done; done)
done

python3 scripts/run_fields_witch_compare.py \
    --render-latex PerfRuns/<run>/results.jsonl --latex paper/fields-witch-table.tex
```

`paper/multiswap-table.tex` is maintained by hand from the campaign logs in
`bench_results/multiswap-historical-<date>`.

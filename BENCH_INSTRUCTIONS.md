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

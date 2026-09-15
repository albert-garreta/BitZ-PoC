# Benchmarks in the paper

All commands run from the repository root. Measurements are serialized by
`scripts/bench_gate.py` (machine lock, sustained-idle wait, swap guard): run
one campaign at a time, on an otherwise idle machine.

```sh
rustup toolchain install 1.98.1
export RUSTFLAGS="-C target-cpu=native"
```

## Campaigns

### Suite: integer multiplication, SHA+ECDSA, hybrid, MultiSwap

```sh
bash scripts/run_suite_2026_09_13.sh                  # every phase, in order
bash scripts/run_suite_2026_09_13.sh sha-ecdsa        # tab:sha256-ecdsa
bash scripts/run_suite_2026_09_13.sh hybrid-witness   # tab:hybrid
bash scripts/run_suite_2026_09_13.sh hybrid-counts    # tab:hybrid-equal-counts
bash scripts/run_suite_2026_09_13.sh u32              # tab:native-mul
bash scripts/run_suite_2026_09_13.sh u64              # tab:native-mul-u64
bash scripts/run_suite_2026_09_13.sh u128             # tab:native-mul-u128
bash scripts/run_suite_2026_09_13.sh multiswap        # tab:multiswap (F2Z row)
```

### Raw performance of the PCS (tab:f2z-raw-performance)

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
    --schemes f2z,fields-witch-asm,fields-witch-f2z
```

### Zinc+ row of the u32 table

In a zinc-plus checkout at `origin/main-beta` (609c18c), with this
repository's bench copied in and the lock taken from a working checkout:

```sh
git clone https://github.com/NethermindEth/zinc-plus /tmp/zinc-609c18c
git -C /tmp/zinc-609c18c checkout 609c18c
cp ~/zinc-plus/Cargo.lock /tmp/zinc-609c18c/Cargo.lock
cp benchmarks/zinc-plus/f2z_u32_mod32.rs /tmp/zinc-609c18c/protocol/benches/
printf '\n[[bench]]\nname = "f2z_u32_mod32"\nharness = false\n' >> /tmp/zinc-609c18c/protocol/Cargo.toml
sed -i '' 's/^zstd = "0.13"$/zstd = "0.13"\nblake3 = { workspace = true }/' /tmp/zinc-609c18c/protocol/Cargo.toml

cd /tmp/zinc-609c18c/protocol
CARGO_TARGET_DIR=/tmp/zinc-u32-st cargo bench --no-run --offline \
    --bench f2z_u32_mod32 --features "simd unchecked"
CARGO_TARGET_DIR=/tmp/zinc-u32-mt cargo bench --no-run --offline \
    --bench f2z_u32_mod32 --features "simd unchecked parallel"
```

One process per (size, threads); 1 thread uses the non-parallel build, 10
threads the parallel one. `SEED` is the suite's u32 corpus seed, which the
bench checks by recomputing the table's row digest:

```sh
for E in 15 17 19 21 23; do
  EXPONENT=$E REPS=5 SEED=6139306037344403556 \
    /usr/bin/time -l /tmp/zinc-u32-st/release/deps/f2z_u32_mod32-*[0-9a-f]
  RAYON_NUM_THREADS=10 EXPONENT=$E REPS=5 SEED=6139306037344403556 \
    /usr/bin/time -l /tmp/zinc-u32-mt/release/deps/f2z_u32_mod32-*[0-9a-f]
done
```

Collect the `ZINC_RESULT` / `ZINC_TRIAL` lines and the `maximum resident set
size` of each case into `bench_results/zinc-plus-u32-<date>/results.jsonl`
and `t<threads>-e<exponent>.out`, then:

```sh
python3 scripts/zinc_plus_summary.py bench_results/zinc-plus-u32-<date> \
    --clone /tmp/zinc-609c18c --revision 609c18c --toolchain "$(rustc --version)" \
    --machine-from PerfRuns/suite-u32-f2z-r2-t1 --out PerfRuns/zinc-plus-u32
```

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
python3 scripts/native_mul_table.py \
    PerfRuns/suite-u32-{bin-r1,bin-r3,f2z-r2,f2z-r8,fri,lig-r1,lig-r3,limber}-t10 \
    PerfRuns/suite-u32-{bin-r1,bin-r3,f2z-r2,f2z-r8,fri,lig-r1,lig-r3,limber}-t1 \
    PerfRuns/zinc-plus-u32 \
    --workload u32-mod32 --exponents 15,17,19,21,23 \
    --unsupported binius64:23,binius64-ligerito-rbr:23,limber:21,limber:23 \
    --unsupported-reason 'their provers exceed the 24\,GB memory of the machine at those sizes'

python3 scripts/native_mul_table.py \
    PerfRuns/suite-u64-{bin-r1,bin-r3,f2z-r2,f2z-r8,lig-r1,lig-r3,limber}-t10 \
    PerfRuns/suite-u64-{bin-r1,bin-r3,f2z-r2,f2z-r8,lig-r1,lig-r3,limber}-t1 \
    --workload u64 --unsupported limber:21 \
    --unsupported-reason 'its prover exceeds the 24\,GB memory of the machine at that size'

python3 scripts/native_mul_table.py \
    PerfRuns/suite-u128-{bin-r1,bin-r3,f2z-r2,f2z-r8,lig-r1,lig-r3,limber}-t10 \
    PerfRuns/suite-u128-{bin-r1,bin-r3,f2z-r2,f2z-r8,lig-r1,lig-r3,limber}-t1 \
    PerfRuns/suite-u128-lig-r1-t10-s21 PerfRuns/suite-u128-lig-r1-t1-s21 \
    --workload u128 --paging binius64:21,binius64-ligerito-rbr@1:21 \
    --unsupported binius64-ligerito-rbr@3:21,limber:21 \
    --unsupported-reason 'their provers exceed the 24\,GB memory of the machine at that size'

python3 scripts/sha256_ecdsa_table.py bench_results/suite-sha256-ecdsa-<date>

for V in witness counts; do
  python3 scripts/hybrid_table.py --variant $V \
    $(for T in 10 1; do for R in hybrid@1:f2z-r2 hybrid@3:f2z-r8 \
        all-binius@1:bin-r1 all-binius@3:bin-r3 \
        binius-ligerito@1:lig-r1 binius-ligerito@3:lig-r3; do
        echo --row "${R%%:*}:$T=PerfRuns/suite-hy-$V-${R##*:}-t$T"; done; done)
done

python3 scripts/run_fields_witch_compare.py \
    --render-latex PerfRuns/<run>/results.jsonl --latex paper/fields-witch-table.tex
```

`paper/multiswap-table.tex` is maintained by hand from the campaign logs in
`bench_results/multiswap-historical-<date>`.

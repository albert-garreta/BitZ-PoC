# Sequential rerun commands

The controller executes exactly one command at a time, waits for it to exit, then waits 20 seconds before starting the next command. Each native command selects one backend. Each prover uses eight internal worker threads.

Run the prepared controller once (it has a lock and refuses to overwrite logs):

```sh

cd /Users/johnwu/code/zk/f2z-pcs

python3 /Users/johnwu/code/zk/f2z-pcs/PerfRuns/albert-sequential-9h7_or3i/run_sequential.py

```

The exact child commands are listed below. The controller also wraps each in `/usr/bin/time -l`, saves stdout/stderr and exit status, and records periodic CPU/memory/paging snapshots.

Do not execute this list separately while the controller is running.

## 1. rate-validation

```sh

env RUSTFLAGS='-C target-cpu=native' RAYON_NUM_THREADS=8 RUSTUP_TOOLCHAIN=1.98.1 PERFETTO_TRACE_PROCESSOR=/private/tmp/f2z-perfetto.F7C3rn/trace_processor_shell cargo +1.98.1 test --release --features binius64-bench --lib binius_ligerito::tests::selectable_rates_bind_both_oracles_and_reject_cross_rate_proofs -- --test-threads=1

```

## 2. binius-pcs-rate1-binius64

```sh

env RUSTFLAGS='-C target-cpu=native' RAYON_NUM_THREADS=8 PERFETTO_TRACE_PROCESSOR=/private/tmp/f2z-perfetto.F7C3rn/trace_processor_shell F2Z_BENCH_SHAPES='15 16 17 18 19 20 21 22' F2Z_BENCH_REPS=5 F2Z_MUL_COMPARE_MEMORY=1 F2Z_MUL_COMPARE_WORKLOADS=u32-mod32 F2Z_MUL_COMPARE_BACKENDS=binius64 F2Z_BINIUS_LOG_INV_RATE=1 F2Z_BINIUS_LIGERITO_LOG_INV_RATE=1 F2Z_MUL_COMPARE_OUTPUT_DIR=/Users/johnwu/code/zk/f2z-pcs/PerfRuns/albert-sequential-9h7_or3i/binius-pcs/rate1/binius64 RUSTUP_TOOLCHAIN=1.98.1 cargo +1.98.1 bench --profile release --bench mul_e2e_compare --features bench-internals,native-mul-compare,unchecked

```

## 3. binius-pcs-rate1-binius64-ligerito

```sh

env RUSTFLAGS='-C target-cpu=native' RAYON_NUM_THREADS=8 PERFETTO_TRACE_PROCESSOR=/private/tmp/f2z-perfetto.F7C3rn/trace_processor_shell F2Z_BENCH_SHAPES='15 16 17 18 19 20 21 22' F2Z_BENCH_REPS=5 F2Z_MUL_COMPARE_MEMORY=1 F2Z_MUL_COMPARE_WORKLOADS=u32-mod32 F2Z_MUL_COMPARE_BACKENDS=binius64-ligerito F2Z_BINIUS_LOG_INV_RATE=1 F2Z_BINIUS_LIGERITO_LOG_INV_RATE=1 F2Z_MUL_COMPARE_OUTPUT_DIR=/Users/johnwu/code/zk/f2z-pcs/PerfRuns/albert-sequential-9h7_or3i/binius-pcs/rate1/binius64-ligerito RUSTUP_TOOLCHAIN=1.98.1 cargo +1.98.1 bench --profile release --bench mul_e2e_compare --features bench-internals,native-mul-compare,unchecked

```

## 4. binius-pcs-rate2-binius64

```sh

env RUSTFLAGS='-C target-cpu=native' RAYON_NUM_THREADS=8 PERFETTO_TRACE_PROCESSOR=/private/tmp/f2z-perfetto.F7C3rn/trace_processor_shell F2Z_BENCH_SHAPES='15 16 17 18 19 20 21 22' F2Z_BENCH_REPS=5 F2Z_MUL_COMPARE_MEMORY=1 F2Z_MUL_COMPARE_WORKLOADS=u32-mod32 F2Z_MUL_COMPARE_BACKENDS=binius64 F2Z_BINIUS_LOG_INV_RATE=2 F2Z_BINIUS_LIGERITO_LOG_INV_RATE=2 F2Z_MUL_COMPARE_OUTPUT_DIR=/Users/johnwu/code/zk/f2z-pcs/PerfRuns/albert-sequential-9h7_or3i/binius-pcs/rate2/binius64 RUSTUP_TOOLCHAIN=1.98.1 cargo +1.98.1 bench --profile release --bench mul_e2e_compare --features bench-internals,native-mul-compare,unchecked

```

## 5. binius-pcs-rate2-binius64-ligerito

```sh

env RUSTFLAGS='-C target-cpu=native' RAYON_NUM_THREADS=8 PERFETTO_TRACE_PROCESSOR=/private/tmp/f2z-perfetto.F7C3rn/trace_processor_shell F2Z_BENCH_SHAPES='15 16 17 18 19 20 21 22' F2Z_BENCH_REPS=5 F2Z_MUL_COMPARE_MEMORY=1 F2Z_MUL_COMPARE_WORKLOADS=u32-mod32 F2Z_MUL_COMPARE_BACKENDS=binius64-ligerito F2Z_BINIUS_LOG_INV_RATE=2 F2Z_BINIUS_LIGERITO_LOG_INV_RATE=2 F2Z_MUL_COMPARE_OUTPUT_DIR=/Users/johnwu/code/zk/f2z-pcs/PerfRuns/albert-sequential-9h7_or3i/binius-pcs/rate2/binius64-ligerito RUSTUP_TOOLCHAIN=1.98.1 cargo +1.98.1 bench --profile release --bench mul_e2e_compare --features bench-internals,native-mul-compare,unchecked

```

## 6. binius-pcs-rate3-binius64

```sh

env RUSTFLAGS='-C target-cpu=native' RAYON_NUM_THREADS=8 PERFETTO_TRACE_PROCESSOR=/private/tmp/f2z-perfetto.F7C3rn/trace_processor_shell F2Z_BENCH_SHAPES='15 16 17 18 19 20 21 22' F2Z_BENCH_REPS=5 F2Z_MUL_COMPARE_MEMORY=1 F2Z_MUL_COMPARE_WORKLOADS=u32-mod32 F2Z_MUL_COMPARE_BACKENDS=binius64 F2Z_BINIUS_LOG_INV_RATE=3 F2Z_BINIUS_LIGERITO_LOG_INV_RATE=3 F2Z_MUL_COMPARE_OUTPUT_DIR=/Users/johnwu/code/zk/f2z-pcs/PerfRuns/albert-sequential-9h7_or3i/binius-pcs/rate3/binius64 RUSTUP_TOOLCHAIN=1.98.1 cargo +1.98.1 bench --profile release --bench mul_e2e_compare --features bench-internals,native-mul-compare,unchecked

```

## 7. binius-pcs-rate3-binius64-ligerito

```sh

env RUSTFLAGS='-C target-cpu=native' RAYON_NUM_THREADS=8 PERFETTO_TRACE_PROCESSOR=/private/tmp/f2z-perfetto.F7C3rn/trace_processor_shell F2Z_BENCH_SHAPES='15 16 17 18 19 20 21 22' F2Z_BENCH_REPS=5 F2Z_MUL_COMPARE_MEMORY=1 F2Z_MUL_COMPARE_WORKLOADS=u32-mod32 F2Z_MUL_COMPARE_BACKENDS=binius64-ligerito F2Z_BINIUS_LOG_INV_RATE=3 F2Z_BINIUS_LIGERITO_LOG_INV_RATE=3 F2Z_MUL_COMPARE_OUTPUT_DIR=/Users/johnwu/code/zk/f2z-pcs/PerfRuns/albert-sequential-9h7_or3i/binius-pcs/rate3/binius64-ligerito RUSTUP_TOOLCHAIN=1.98.1 cargo +1.98.1 bench --profile release --bench mul_e2e_compare --features bench-internals,native-mul-compare,unchecked

```

## 8. bitz-u32-wide-rate1

```sh

env RUSTFLAGS='-C target-cpu=native' RAYON_NUM_THREADS=8 PERFETTO_TRACE_PROCESSOR=/private/tmp/f2z-perfetto.F7C3rn/trace_processor_shell RUSTUP_TOOLCHAIN=1.98.1 cargo +1.98.1 run --bin f2z --release --features unchecked,span-metrics -- --mul-sweep 15-22 --threads 8 --reps 5 --lambda 100 --word-bits 1 --profile custom:1:4 --cooldown 20 --latex /Users/johnwu/code/zk/f2z-pcs/PerfRuns/albert-sequential-9h7_or3i/all-provers/bitz-u32-wide-rate1.tex

```

## 9. bitz-u32-wide-rate2

```sh

env RUSTFLAGS='-C target-cpu=native' RAYON_NUM_THREADS=8 PERFETTO_TRACE_PROCESSOR=/private/tmp/f2z-perfetto.F7C3rn/trace_processor_shell RUSTUP_TOOLCHAIN=1.98.1 cargo +1.98.1 run --bin f2z --release --features unchecked,span-metrics -- --mul-sweep 15-22 --threads 8 --reps 5 --lambda 100 --word-bits 1 --profile custom:2:4 --cooldown 20 --latex /Users/johnwu/code/zk/f2z-pcs/PerfRuns/albert-sequential-9h7_or3i/all-provers/bitz-u32-wide-rate2.tex

```

## 10. bitz-u32-wide-rate3

```sh

env RUSTFLAGS='-C target-cpu=native' RAYON_NUM_THREADS=8 PERFETTO_TRACE_PROCESSOR=/private/tmp/f2z-perfetto.F7C3rn/trace_processor_shell RUSTUP_TOOLCHAIN=1.98.1 cargo +1.98.1 run --bin f2z --release --features unchecked,span-metrics -- --mul-sweep 15-22 --threads 8 --reps 5 --lambda 100 --word-bits 1 --profile custom:3:4 --cooldown 20 --latex /Users/johnwu/code/zk/f2z-pcs/PerfRuns/albert-sequential-9h7_or3i/all-provers/bitz-u32-wide-rate3.tex

```

## 11. u32-mod32-rate1-f2z

```sh

env RUSTFLAGS='-C target-cpu=native' RAYON_NUM_THREADS=8 PERFETTO_TRACE_PROCESSOR=/private/tmp/f2z-perfetto.F7C3rn/trace_processor_shell F2Z_BENCH_SHAPES='15 16 17 18 19 20 21 22' F2Z_BENCH_REPS=5 F2Z_MUL_COMPARE_MEMORY=1 F2Z_MUL_COMPARE_WORKLOADS=u32-mod32 F2Z_MUL_COMPARE_BACKENDS=f2z F2Z_LIG_PROFILE=custom:1:4 F2Z_BINIUS_LOG_INV_RATE=1 F2Z_BINIUS_LIGERITO_LOG_INV_RATE=1 F2Z_PLONKY3_LOG_INV_RATE=1 F2Z_MUL_COMPARE_OUTPUT_DIR=/Users/johnwu/code/zk/f2z-pcs/PerfRuns/albert-sequential-9h7_or3i/all-provers/u32-mod32-rate1/f2z RUSTUP_TOOLCHAIN=1.98.1 cargo +1.98.1 bench --profile release --bench mul_e2e_compare --features bench-internals,native-mul-compare,unchecked

```

## 12. u32-mod32-rate1-binius64

```sh

env RUSTFLAGS='-C target-cpu=native' RAYON_NUM_THREADS=8 PERFETTO_TRACE_PROCESSOR=/private/tmp/f2z-perfetto.F7C3rn/trace_processor_shell F2Z_BENCH_SHAPES='15 16 17 18 19 20 21 22' F2Z_BENCH_REPS=5 F2Z_MUL_COMPARE_MEMORY=1 F2Z_MUL_COMPARE_WORKLOADS=u32-mod32 F2Z_MUL_COMPARE_BACKENDS=binius64 F2Z_LIG_PROFILE=custom:1:4 F2Z_BINIUS_LOG_INV_RATE=1 F2Z_BINIUS_LIGERITO_LOG_INV_RATE=1 F2Z_PLONKY3_LOG_INV_RATE=1 F2Z_MUL_COMPARE_OUTPUT_DIR=/Users/johnwu/code/zk/f2z-pcs/PerfRuns/albert-sequential-9h7_or3i/all-provers/u32-mod32-rate1/binius64 RUSTUP_TOOLCHAIN=1.98.1 cargo +1.98.1 bench --profile release --bench mul_e2e_compare --features bench-internals,native-mul-compare,unchecked

```

## 13. u32-mod32-rate1-binius64-ligerito

```sh

env RUSTFLAGS='-C target-cpu=native' RAYON_NUM_THREADS=8 PERFETTO_TRACE_PROCESSOR=/private/tmp/f2z-perfetto.F7C3rn/trace_processor_shell F2Z_BENCH_SHAPES='15 16 17 18 19 20 21 22' F2Z_BENCH_REPS=5 F2Z_MUL_COMPARE_MEMORY=1 F2Z_MUL_COMPARE_WORKLOADS=u32-mod32 F2Z_MUL_COMPARE_BACKENDS=binius64-ligerito F2Z_LIG_PROFILE=custom:1:4 F2Z_BINIUS_LOG_INV_RATE=1 F2Z_BINIUS_LIGERITO_LOG_INV_RATE=1 F2Z_PLONKY3_LOG_INV_RATE=1 F2Z_MUL_COMPARE_OUTPUT_DIR=/Users/johnwu/code/zk/f2z-pcs/PerfRuns/albert-sequential-9h7_or3i/all-provers/u32-mod32-rate1/binius64-ligerito RUSTUP_TOOLCHAIN=1.98.1 cargo +1.98.1 bench --profile release --bench mul_e2e_compare --features bench-internals,native-mul-compare,unchecked

```

## 14. u32-mod32-rate1-plonky3-fri

```sh

env RUSTFLAGS='-C target-cpu=native' RAYON_NUM_THREADS=8 PERFETTO_TRACE_PROCESSOR=/private/tmp/f2z-perfetto.F7C3rn/trace_processor_shell F2Z_BENCH_SHAPES='15 16 17 18 19 20 21 22' F2Z_BENCH_REPS=5 F2Z_MUL_COMPARE_MEMORY=1 F2Z_MUL_COMPARE_WORKLOADS=u32-mod32 F2Z_MUL_COMPARE_BACKENDS=plonky3-fri F2Z_LIG_PROFILE=custom:1:4 F2Z_BINIUS_LOG_INV_RATE=1 F2Z_BINIUS_LIGERITO_LOG_INV_RATE=1 F2Z_PLONKY3_LOG_INV_RATE=1 F2Z_MUL_COMPARE_OUTPUT_DIR=/Users/johnwu/code/zk/f2z-pcs/PerfRuns/albert-sequential-9h7_or3i/all-provers/u32-mod32-rate1/plonky3-fri RUSTUP_TOOLCHAIN=1.98.1 cargo +1.98.1 bench --profile release --bench mul_e2e_compare --features bench-internals,native-mul-compare,unchecked

```

## 15. u32-mod32-rate2-f2z

```sh

env RUSTFLAGS='-C target-cpu=native' RAYON_NUM_THREADS=8 PERFETTO_TRACE_PROCESSOR=/private/tmp/f2z-perfetto.F7C3rn/trace_processor_shell F2Z_BENCH_SHAPES='15 16 17 18 19 20 21 22' F2Z_BENCH_REPS=5 F2Z_MUL_COMPARE_MEMORY=1 F2Z_MUL_COMPARE_WORKLOADS=u32-mod32 F2Z_MUL_COMPARE_BACKENDS=f2z F2Z_LIG_PROFILE=custom:2:4 F2Z_BINIUS_LOG_INV_RATE=2 F2Z_BINIUS_LIGERITO_LOG_INV_RATE=2 F2Z_PLONKY3_LOG_INV_RATE=2 F2Z_MUL_COMPARE_OUTPUT_DIR=/Users/johnwu/code/zk/f2z-pcs/PerfRuns/albert-sequential-9h7_or3i/all-provers/u32-mod32-rate2/f2z RUSTUP_TOOLCHAIN=1.98.1 cargo +1.98.1 bench --profile release --bench mul_e2e_compare --features bench-internals,native-mul-compare,unchecked

```

## 16. u32-mod32-rate2-binius64

```sh

env RUSTFLAGS='-C target-cpu=native' RAYON_NUM_THREADS=8 PERFETTO_TRACE_PROCESSOR=/private/tmp/f2z-perfetto.F7C3rn/trace_processor_shell F2Z_BENCH_SHAPES='15 16 17 18 19 20 21 22' F2Z_BENCH_REPS=5 F2Z_MUL_COMPARE_MEMORY=1 F2Z_MUL_COMPARE_WORKLOADS=u32-mod32 F2Z_MUL_COMPARE_BACKENDS=binius64 F2Z_LIG_PROFILE=custom:2:4 F2Z_BINIUS_LOG_INV_RATE=2 F2Z_BINIUS_LIGERITO_LOG_INV_RATE=2 F2Z_PLONKY3_LOG_INV_RATE=2 F2Z_MUL_COMPARE_OUTPUT_DIR=/Users/johnwu/code/zk/f2z-pcs/PerfRuns/albert-sequential-9h7_or3i/all-provers/u32-mod32-rate2/binius64 RUSTUP_TOOLCHAIN=1.98.1 cargo +1.98.1 bench --profile release --bench mul_e2e_compare --features bench-internals,native-mul-compare,unchecked

```

## 17. u32-mod32-rate2-binius64-ligerito

```sh

env RUSTFLAGS='-C target-cpu=native' RAYON_NUM_THREADS=8 PERFETTO_TRACE_PROCESSOR=/private/tmp/f2z-perfetto.F7C3rn/trace_processor_shell F2Z_BENCH_SHAPES='15 16 17 18 19 20 21 22' F2Z_BENCH_REPS=5 F2Z_MUL_COMPARE_MEMORY=1 F2Z_MUL_COMPARE_WORKLOADS=u32-mod32 F2Z_MUL_COMPARE_BACKENDS=binius64-ligerito F2Z_LIG_PROFILE=custom:2:4 F2Z_BINIUS_LOG_INV_RATE=2 F2Z_BINIUS_LIGERITO_LOG_INV_RATE=2 F2Z_PLONKY3_LOG_INV_RATE=2 F2Z_MUL_COMPARE_OUTPUT_DIR=/Users/johnwu/code/zk/f2z-pcs/PerfRuns/albert-sequential-9h7_or3i/all-provers/u32-mod32-rate2/binius64-ligerito RUSTUP_TOOLCHAIN=1.98.1 cargo +1.98.1 bench --profile release --bench mul_e2e_compare --features bench-internals,native-mul-compare,unchecked

```

## 18. u32-mod32-rate2-plonky3-fri

```sh

env RUSTFLAGS='-C target-cpu=native' RAYON_NUM_THREADS=8 PERFETTO_TRACE_PROCESSOR=/private/tmp/f2z-perfetto.F7C3rn/trace_processor_shell F2Z_BENCH_SHAPES='15 16 17 18 19 20 21 22' F2Z_BENCH_REPS=5 F2Z_MUL_COMPARE_MEMORY=1 F2Z_MUL_COMPARE_WORKLOADS=u32-mod32 F2Z_MUL_COMPARE_BACKENDS=plonky3-fri F2Z_LIG_PROFILE=custom:2:4 F2Z_BINIUS_LOG_INV_RATE=2 F2Z_BINIUS_LIGERITO_LOG_INV_RATE=2 F2Z_PLONKY3_LOG_INV_RATE=2 F2Z_MUL_COMPARE_OUTPUT_DIR=/Users/johnwu/code/zk/f2z-pcs/PerfRuns/albert-sequential-9h7_or3i/all-provers/u32-mod32-rate2/plonky3-fri RUSTUP_TOOLCHAIN=1.98.1 cargo +1.98.1 bench --profile release --bench mul_e2e_compare --features bench-internals,native-mul-compare,unchecked

```

## 19. u32-mod32-rate3-f2z

```sh

env RUSTFLAGS='-C target-cpu=native' RAYON_NUM_THREADS=8 PERFETTO_TRACE_PROCESSOR=/private/tmp/f2z-perfetto.F7C3rn/trace_processor_shell F2Z_BENCH_SHAPES='15 16 17 18 19 20 21 22' F2Z_BENCH_REPS=5 F2Z_MUL_COMPARE_MEMORY=1 F2Z_MUL_COMPARE_WORKLOADS=u32-mod32 F2Z_MUL_COMPARE_BACKENDS=f2z F2Z_LIG_PROFILE=custom:3:4 F2Z_BINIUS_LOG_INV_RATE=3 F2Z_BINIUS_LIGERITO_LOG_INV_RATE=3 F2Z_PLONKY3_LOG_INV_RATE=3 F2Z_MUL_COMPARE_OUTPUT_DIR=/Users/johnwu/code/zk/f2z-pcs/PerfRuns/albert-sequential-9h7_or3i/all-provers/u32-mod32-rate3/f2z RUSTUP_TOOLCHAIN=1.98.1 cargo +1.98.1 bench --profile release --bench mul_e2e_compare --features bench-internals,native-mul-compare,unchecked

```

## 20. u32-mod32-rate3-binius64

```sh

env RUSTFLAGS='-C target-cpu=native' RAYON_NUM_THREADS=8 PERFETTO_TRACE_PROCESSOR=/private/tmp/f2z-perfetto.F7C3rn/trace_processor_shell F2Z_BENCH_SHAPES='15 16 17 18 19 20 21 22' F2Z_BENCH_REPS=5 F2Z_MUL_COMPARE_MEMORY=1 F2Z_MUL_COMPARE_WORKLOADS=u32-mod32 F2Z_MUL_COMPARE_BACKENDS=binius64 F2Z_LIG_PROFILE=custom:3:4 F2Z_BINIUS_LOG_INV_RATE=3 F2Z_BINIUS_LIGERITO_LOG_INV_RATE=3 F2Z_PLONKY3_LOG_INV_RATE=3 F2Z_MUL_COMPARE_OUTPUT_DIR=/Users/johnwu/code/zk/f2z-pcs/PerfRuns/albert-sequential-9h7_or3i/all-provers/u32-mod32-rate3/binius64 RUSTUP_TOOLCHAIN=1.98.1 cargo +1.98.1 bench --profile release --bench mul_e2e_compare --features bench-internals,native-mul-compare,unchecked

```

## 21. u32-mod32-rate3-binius64-ligerito

```sh

env RUSTFLAGS='-C target-cpu=native' RAYON_NUM_THREADS=8 PERFETTO_TRACE_PROCESSOR=/private/tmp/f2z-perfetto.F7C3rn/trace_processor_shell F2Z_BENCH_SHAPES='15 16 17 18 19 20 21 22' F2Z_BENCH_REPS=5 F2Z_MUL_COMPARE_MEMORY=1 F2Z_MUL_COMPARE_WORKLOADS=u32-mod32 F2Z_MUL_COMPARE_BACKENDS=binius64-ligerito F2Z_LIG_PROFILE=custom:3:4 F2Z_BINIUS_LOG_INV_RATE=3 F2Z_BINIUS_LIGERITO_LOG_INV_RATE=3 F2Z_PLONKY3_LOG_INV_RATE=3 F2Z_MUL_COMPARE_OUTPUT_DIR=/Users/johnwu/code/zk/f2z-pcs/PerfRuns/albert-sequential-9h7_or3i/all-provers/u32-mod32-rate3/binius64-ligerito RUSTUP_TOOLCHAIN=1.98.1 cargo +1.98.1 bench --profile release --bench mul_e2e_compare --features bench-internals,native-mul-compare,unchecked

```

## 22. u32-mod32-rate3-plonky3-fri

```sh

env RUSTFLAGS='-C target-cpu=native' RAYON_NUM_THREADS=8 PERFETTO_TRACE_PROCESSOR=/private/tmp/f2z-perfetto.F7C3rn/trace_processor_shell F2Z_BENCH_SHAPES='15 16 17 18 19 20 21 22' F2Z_BENCH_REPS=5 F2Z_MUL_COMPARE_MEMORY=1 F2Z_MUL_COMPARE_WORKLOADS=u32-mod32 F2Z_MUL_COMPARE_BACKENDS=plonky3-fri F2Z_LIG_PROFILE=custom:3:4 F2Z_BINIUS_LOG_INV_RATE=3 F2Z_BINIUS_LIGERITO_LOG_INV_RATE=3 F2Z_PLONKY3_LOG_INV_RATE=3 F2Z_MUL_COMPARE_OUTPUT_DIR=/Users/johnwu/code/zk/f2z-pcs/PerfRuns/albert-sequential-9h7_or3i/all-provers/u32-mod32-rate3/plonky3-fri RUSTUP_TOOLCHAIN=1.98.1 cargo +1.98.1 bench --profile release --bench mul_e2e_compare --features bench-internals,native-mul-compare,unchecked

```

## 23. u32-mod32-limber-limber

```sh

env RUSTFLAGS='-C target-cpu=native' RAYON_NUM_THREADS=8 PERFETTO_TRACE_PROCESSOR=/private/tmp/f2z-perfetto.F7C3rn/trace_processor_shell F2Z_BENCH_SHAPES='15 16 17 18 19 20 21 22' F2Z_BENCH_REPS=5 F2Z_MUL_COMPARE_MEMORY=1 F2Z_MUL_COMPARE_WORKLOADS=u32-mod32 F2Z_MUL_COMPARE_BACKENDS=limber F2Z_MUL_COMPARE_OUTPUT_DIR=/Users/johnwu/code/zk/f2z-pcs/PerfRuns/albert-sequential-9h7_or3i/all-provers/u32-mod32-limber/limber RUSTUP_TOOLCHAIN=1.98.1 cargo +1.98.1 bench --profile release --bench mul_e2e_compare --features bench-internals,native-mul-compare,unchecked

```

## 24. sha256-ecdsa-rate1

```sh

env RUSTFLAGS='-C target-cpu=native' RAYON_NUM_THREADS=8 PERFETTO_TRACE_PROCESSOR=/private/tmp/f2z-perfetto.F7C3rn/trace_processor_shell F2Z_LIG_PROFILE=custom:1:4 RUSTUP_TOOLCHAIN=1.98.1 python3 scripts/run_sha256_ecdsa_compare.py --methods f2z-split binius64 --exponents 7 --targets 100 --threads 8 --seeds 0 --reps 5 --binius-log-inv-rate 1 --output /Users/johnwu/code/zk/f2z-pcs/PerfRuns/albert-sequential-9h7_or3i/all-provers/sha256-ecdsa-rate1

```

## 25. sha256-ecdsa-rate2

```sh

env RUSTFLAGS='-C target-cpu=native' RAYON_NUM_THREADS=8 PERFETTO_TRACE_PROCESSOR=/private/tmp/f2z-perfetto.F7C3rn/trace_processor_shell F2Z_LIG_PROFILE=custom:2:4 RUSTUP_TOOLCHAIN=1.98.1 python3 scripts/run_sha256_ecdsa_compare.py --methods f2z-split binius64 --exponents 7 --targets 100 --threads 8 --seeds 0 --reps 5 --binius-log-inv-rate 2 --output /Users/johnwu/code/zk/f2z-pcs/PerfRuns/albert-sequential-9h7_or3i/all-provers/sha256-ecdsa-rate2

```

## 26. sha256-ecdsa-rate3

```sh

env RUSTFLAGS='-C target-cpu=native' RAYON_NUM_THREADS=8 PERFETTO_TRACE_PROCESSOR=/private/tmp/f2z-perfetto.F7C3rn/trace_processor_shell F2Z_LIG_PROFILE=custom:3:4 RUSTUP_TOOLCHAIN=1.98.1 python3 scripts/run_sha256_ecdsa_compare.py --methods f2z-split binius64 --exponents 7 --targets 100 --threads 8 --seeds 0 --reps 5 --binius-log-inv-rate 3 --output /Users/johnwu/code/zk/f2z-pcs/PerfRuns/albert-sequential-9h7_or3i/all-provers/sha256-ecdsa-rate3

```

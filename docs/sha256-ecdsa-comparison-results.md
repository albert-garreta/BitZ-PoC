# Initial SHA-256 / P-256 comparison measurements

**Historical implementation:** these results use Spartan2 commit `bf99f4f8`,
before the non-ZK adapter was refactored to reuse the existing optimized
NeutronNova and sumcheck kernels. They are not measurements of the current
shared-kernel implementation.

See the [current shared-kernel results](sha256-ecdsa-shared-kernels-results.md)
for the replacement measurements.

For every exponent 4 through 11 and the complete seven-metric breakdown,
see the [full-range measurements](sha256-ecdsa-i4-i11-results.md). The CSVs for
this earlier campaign also now include PIOP/IOP columns and per-sample exports.

This campaign compares the implementations described in the
[methodology guide](sha256-ecdsa-comparison.md). All 24 cases completed;
all 96 encoded proofs, including warmups, decoded and verified. Fixture
identifiers matched across methods and chunkings at each message length.

These are initial measurements on one machine, using one fixture per total
length, one warmup and three measured repetitions. Values below are medians.
F2Z uses its **100-bit economic target**, with proof of work; Spartan reports
**nominal 128-bit group security** under a different model. The columns do not
represent equal statistical security. Repeated F2Z trials reuse the same
grinding tasks; use multiple seeds for a broader performance study.

## Build and machine

- CPU: AMD Ryzen 9 9950X3D 16-Core Processor; 32 logical CPUs.
- OS: `Linux-6.17.0-41-generic-x86_64-with-glibc2.42`.
- Compiler: `rustc 1.97.1 (8bab26f4f 2026-07-14)`.
- Root base: `ec8c9f6504bddcb96d3fb2dd840a21014f3b3397`, with the comparison changes uncommitted.
- Backend: `bf99f4f828131afd153e239a4ea768ca5062601e` in the local Spartan2 fork.
- Binary SHA-256: `f13b83c12c42c500af2d3d14ad945f5a72d149fca5c7b4035be27ac07cd9c443`.
- Release build: native CPU instructions, LTO, one codegen unit.
- Fresh worker processes; Rayon fixed at the listed thread count; no CPU affinity.
- 48 GiB address-space limit and 300-second limit per case.

The backend is a new non-ZK NeutronNova/Spartan adaptation. It uses the fork's
matrix and Hyrax infrastructure but does not port every optimized kernel/cache
from the stock ZK prover. These are not the Vega paper's published timings.

```sh
python3 scripts/run_sha256_ecdsa_compare.py \
  --output bench_results/sha256-ecdsa-compare-bf99f4f8-final \
  --spartan-splits 0:3 1:2 3:0 2:3 5:0 4:3 \
  --targets 100 --threads 1 32 --reps 3 --seeds 0 --timeout 300
```

The local run used `CARGO_HOME=/tmp/bitz-sha-cargo-home` and `--offline`, with
the exact backend Git object obtained from the local checkout. Publication to
the fork is a separate step; the measured source is identified by its commit.

## Timings and complete proof material

`N` includes the final padding compression; message length is `64*(N-1)` bytes.
There is exactly one ECDSA verification. `r:c` partitions only Spartan's SHA
instances; `c=0` skips folding. F2Z rows are measured once per total N.

Prove time includes commitment and the complete proof protocol. Add witness
time for fresh-input witness-through-proof cost; its per-sample median is in
the CSV. Verification includes all application linking checks. Encoding and
decoding are timed separately. Full proof material includes commitments and
all proof-carried inputs; the 129-byte external statement is additional.

### 1 thread

| N | Method | r:c | Witness ms | Prove ms | Verify ms | Full proof KiB | Peak MiB |
| ---: | --- | --- | ---: | ---: | ---: | ---: | ---: |
| 8 | F2Z Split | — | 14.10 | 832.44 | 685.67 | 142.81 | 1223.8 |
| 8 | F2Z AllRows | — | 13.78 | 866.73 | 720.19 | 142.78 | 1223.8 |
| 8 | Spartan MC | 0:3 | 27.85 | 220.85 | 24.21 | 85.51 | 296.7 |
| 8 | Spartan MC | 1:2 | 27.72 | 243.92 | 30.95 | 84.27 | 339.6 |
| 8 | Spartan MC | 3:0 | 28.14 | 348.79 | 51.26 | 86.45 | 652.6 |
| 32 | F2Z Split | — | 17.32 | 912.45 | 737.51 | 146.53 | 1223.9 |
| 32 | F2Z AllRows | — | 17.36 | 864.94 | 691.46 | 145.87 | 1223.8 |
| 32 | Spartan MC | 2:3 | 98.97 | 499.98 | 49.76 | 112.30 | 551.9 |
| 32 | Spartan MC | 5:0 | 101.14 | 1025.66 | 160.35 | 124.07 | 2002.1 |
| 128 | F2Z Split | — | 32.85 | 1008.57 | 728.48 | 162.45 | 1221.8 |
| 128 | F2Z AllRows | — | 32.42 | 1018.52 | 736.17 | 162.26 | 1222.8 |
| 128 | Spartan MC | 4:3 | 375.31 | 1617.76 | 149.83 | 216.87 | 1577.7 |

### 32 threads

| N | Method | r:c | Witness ms | Prove ms | Verify ms | Full proof KiB | Peak MiB |
| ---: | --- | --- | ---: | ---: | ---: | ---: | ---: |
| 8 | F2Z Split | — | 4.28 | 718.68 | 680.61 | 142.81 | 1227.9 |
| 8 | F2Z AllRows | — | 4.27 | 752.06 | 722.12 | 142.78 | 1228.7 |
| 8 | Spartan MC | 0:3 | 10.20 | 52.08 | 16.82 | 85.51 | 431.2 |
| 8 | Spartan MC | 1:2 | 11.93 | 82.11 | 19.15 | 84.27 | 635.2 |
| 8 | Spartan MC | 3:0 | 29.02 | 229.89 | 32.06 | 86.45 | 1662.0 |
| 32 | F2Z Split | — | 4.49 | 776.76 | 735.54 | 146.53 | 1228.7 |
| 32 | F2Z AllRows | — | 4.49 | 714.40 | 672.62 | 145.87 | 1227.9 |
| 32 | Spartan MC | 2:3 | 18.71 | 191.62 | 26.53 | 112.30 | 967.0 |
| 32 | Spartan MC | 5:0 | 106.97 | 881.90 | 84.50 | 124.07 | 5854.6 |
| 128 | F2Z Split | — | 5.37 | 780.10 | 724.38 | 162.45 | 1228.7 |
| 128 | F2Z AllRows | — | 5.48 | 780.12 | 726.46 | 162.26 | 1225.6 |
| 128 | Spartan MC | 4:3 | 60.77 | 723.07 | 56.47 | 216.87 | 3465.5 |

Peak memory is measured over the whole worker, including setup and
verification. At these small sizes, thread overhead and padding can dominate;
the results do not establish an optimal split for longer chains.

## Artifacts and validation

The complete local campaign is in
`bench_results/sha256-ecdsa-compare-bf99f4f8-final/`: `summary.csv`, raw samples,
per-case output/error logs, `comparison.json`, and the machine/build manifest.
Setup, codec, phase breakdowns, exact security bounds and padded circuit
dimensions remain in those records. The manifest includes the dirty-source
patch and worker/runner snapshots. That directory is ignored by Git; archive
it when transferring the measurements.

Validation for this implementation includes 103 passing Spartan library tests,
the seven composed F2Z regression tests, and four campaign-runner tests.
Spartan Clippy passed with warnings denied. Tests cover invalid signatures,
broken SHA/ECDSA links, altered sumcheck/opening messages, codec failures,
high-s signatures, digest/scalar boundaries, complete point operations and
setup reuse. The default F2Z library also builds without enabling the optional
comparison backend.

An additional campaign in
`bench_results/sha256-ecdsa-compare-bf99f4f8-128-smoke/` verified all six proofs
across the three methods at `r=3,c=0,seed=1`, using F2Z's 128-bit economic
target, one thread, one warmup and one measured repetition. This checks the
other security setting and a second fixture; its timings are not included in
the tables above. Rerunning the main campaign reused all 24 completed cases.
The ordinary `cargo bench` invocation also generated and verified two proofs.

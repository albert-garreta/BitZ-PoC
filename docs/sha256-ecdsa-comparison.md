# SHA-256 / P-256 ECDSA comparison

The retained methods are `bitz-split`, `bitz-all`, `binius64`, and
`binius64-ligerito`. They prove the same signed-message relation in non-ZK mode.
BitZ reuses its composed prover; the isolated [Binius64 worker](../benchmarks/binius64/README.md)
uses [vendor/binius64](../vendor/binius64/), shared with the root adapters.
The worker keeps its own workspace, lockfile and toolchain. Local dependency
snapshots are recorded in [provenance.toml](../provenance.toml).

## Compile without executing the comparison

```sh
RUSTFLAGS="-C target-cpu=native" cargo +1.98.1 build --release --locked \
  --features sha256-ecdsa-compare --bench sha256_ecdsa_compare
RUSTFLAGS="-C target-cpu=native" cargo +1.98.1 build --release --locked \
  --manifest-path benchmarks/binius64/Cargo.toml
```

Build the worker directly through Cargo for compile-only checks: its Python
build wrapper executes the resulting binary to collect metadata. Compilation
does not establish proof correctness or performance.

## Run a campaign

The following command executes benchmarks. Use a fresh output directory when
changing the code, compiler flags, timing backend or benchmark configuration.

```sh
python3 scripts/run_sha256_ecdsa_compare.py \
  --output bench_results/sha256-p256 \
  --methods bitz-split binius64 binius64-ligerito \
  --exponents 4 5 6 7 --targets 100 --threads 1 10 --reps 5 \
  --bitz-profiles custom:1:4 custom:3:4 --binius-rates 1 3 \
  --timing perfetto
```

The default methods are `bitz-split binius64 binius64-ligerito`; default
exponents are 3, 5 and 7. Supported total compression exponents are 3..16.
Each BitZ case selects its Ligerito profile, and each Binius-family case its
commitment rate: log inverse rates 1 and 3 mean rates 1/2 and 1/8.
`binius64-ligerito` uses the fixed round-by-round 100-bit gate.
Use `--methods bitz-split bitz-all` for the BitZ-only comparison.

The runner rejects an ambient `BITZ_LIG_PROFILE`, since it sets that variable
per case. `RUSTFLAGS` defaults to `-C target-cpu=native`; `--offline` uses cached
Cargo dependencies. Perfetto timing requires `trace_processor_shell` on `PATH`
or at `PERFETTO_TRACE_PROCESSOR`. For BitZ-only wall-clock timing:

```sh
python3 scripts/run_sha256_ecdsa_compare.py \
  --output bench_results/sha256-p256-wall-clock \
  --methods bitz-split --exponents 7 --targets 100 --threads 10 \
  --reps 5 --bitz-profiles custom:1:4 --timing wall-clock
```

Wall-clock mode uses Rust's monotonic clock for setup, witness generation,
commitment, protocol, end-to-end proving, serialization and verification.
Internal phase breakdowns remain null in JSON and blank in CSV. This mode
supports `bitz-split` and `bitz-all`.

Every campaign records `samples.csv`, `summary.csv` and individual trial
artifacts. Peak RSS is the whole-worker maximum, including setup, warmup,
measured proofs and verification. To summarize saved results without running
workers, use `--summarize-only` with the original output directory.

The default limits are 48 GiB of virtual address space on Linux and one hour
per case, configurable with `--memory-gib` and `--timeout`. macOS records peak
RSS and enforces the timeout but does not enforce that address-space limit.
The runner records failed cases and resumes missing ones; `--retry-failed`
retries failures. Binary and runner hashes prevent mixing incompatible runs.

## Statement and workload

The external statement is `(i,Qx,Qy,r_sig,s_sig)`: a P-256 key, its exact
signature and the compression exponent. The four key/signature values are
canonical 32-byte big-endian values. The message and digest are witnesses;
the verifier does not receive a separately expected message.

```text
N = 2^i                 total SHA compressions, including padding
message_bytes = 64*(N-1)
signatures = 1
```

Each message has `N-1` complete blocks followed by the mandatory standard SHA
padding block. At `N=1024`, the message is 65,472 bytes. Fixtures use
`bitz/sha256-ecdsa-fixture/standard-p256/v1`. Both valid low-s and high-s
signatures are accepted, with `0 < r,s < n`. Message, key and signature are
prepared and validated outside timers, identically for each `(i,seed)` across
methods. Host checks do not replace the ECDSA circuit.

The direct Rust CLI retains `--r R --c C` as a way to supply total exponent
`i=R+C`; the Python runner supplies `R=i,C=0`. Result records leave `r,c` null,
since the retained methods do not use external SHA chunking.

| Method | Reduction | Commitment/opening |
|---|---|---|
| BitZ Split | Nonlinear P-256 rows enter the outer sumcheck; linear SHA/P-256 rows join the common inner sumcheck | One source commitment and virtual BitZ opening |
| BitZ AllRows | All original SHA/P-256 rows enter the outer sumcheck, followed by the common inner sumcheck | Same commitment and opening architecture |
| Binius64 | Fixed SHA circuit and standard P-256 gadget | Witness oracle commitment and BaseFold opening |
| Binius64 with BitZ opener | Same circuit and Binius64 reduction | All oracles committed and opened through BitZ/Ligerito |

BitZ's virtual map supplies the IV, wires adjacent states, substitutes padding,
and aliases the final SHA digest into ECDSA. Public-bit equations bind the
key/signature, including the constant-one affine equation. See the
[BitZ adapter description](sha256-ecdsa.md) for protocol details.

The Binius gadget exercises IntMul, BinMul and AND reductions. The worker
records security accounting and its actual local dependency snapshot.
Constraint counts use each system's representation and are not interchangeable.

Report tables default to `outputs/tables/sha256-ecdsa-table.tex`. The internal
BitZ prover remains part of this workspace; the external Spartan2 and
ZKPassport comparisons have been removed.

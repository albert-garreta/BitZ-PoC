# Matched 114-bit F2Z / Limber MultiSwap campaign

The campaign compares F2Z/Ligerito, Limber-Hyrax, and Limber-Brakedown on
identical copies of the RSA-2048 paper fixture. Every reported modeled check
must reach **at least 114 bits** under `per-check-round-minimum/v1` accounting.
This is a minimum over the modeled checks and rounds, not a new combined
whole-proof soundness theorem or an RSA key-strength claim.

## Statement and workload

Fix the historical workload parameter `k=0`. The default sweep proves
**1, 2, 4, 8, and 16 complete circuit copies in one proof**, corresponding to
4–64 RSA exponentiations. Copies have separate witness variables, share the
constant column, compact live rows/columns, and are padded once. Each copy
contributes 6,209 live rows and 6,204 live witness columns. One copy preserves
the existing circuit and assignment digests.

The canonical integer equations are

```text
(A z)[i] * (B z)[i] = (C z)[i] + modulus[i] * quotient[i]
```

A zero row modulus denotes an exact integer equation. Public circuit data
includes the integer matrices, row moduli, dimensions, and fixed constants.
Witness values and quotients are private unsigned integers below `2^2048`.
The application public-input vector is explicitly **count=0, values=[]**.
Padding uses zero witness values, zero quotients, and modulus two.

`f2z-limber/multiswap-statement/v2` binds the canonical matrix digest, batch
count, variable bounds and roles, constant-column and padding conventions,
and public-input count. The separate integer-assignment digest checks that
all backends benchmark the same deterministic data. F2Z's commitment layout
remaps variables and folds the quotient-modulus terms into C; structural tests
invert that remapping and compare every canonical linear form, including
exact rows and quotient terms.

This is the existing **wired MultiSwap/RSA cost model**. RSA exponentiation
chains execute, while modeled hash and Poseidon costs do not represent native
hash executions. It does not prove a complete public old-to-new accumulator
transition. The historical `k` sweep is separate: use `--batch-counts none`
with explicit `--k-values` for that experiment. Nonzero `k` is rejected in the
batch sweep.

## Security configuration

| Component | Matched setting |
|---|---|
| F2Z profile | `F2Z_BENCH_LAMBDA=114`, profile `limber114` |
| F2Z reduction | Derived per shape; ten grinding bits for every selected batch |
| F2Z Ligerito opening | Validated UDR configuration derived at target 114 |
| Comparison target | `MATCHED_SECURITY_BITS=114` |
| Limber integer commitment | `MATCHED_INTEGER_SECURITY_BITS=128` (native CRT target) |
| Limber integer challenge bound target | Native `LAMBDA_BOUND2=117` |
| Limber integer challenge width | Still 128 bits |
| Brakedown opening | `BDLAMBDA=114`, spec 4, row length 32768, direct threshold 65536 |
| Shared fingerprint | Uniform 128-bit prime sampling; roughly 114-bit conservative bound |

Limber's CRT divisor accounting uses the **minimum** sampled-prime size,
`log_p - 1`, before deriving `ceil(integer_target / bits_per_prime)`
repetitions. The 114-bit comparison preserves the native 128-bit CRT target,
128-bit challenge width, and 117-bit challenge bound target. The fingerprint
bound is the limiting modeled check for Hyrax; Brakedown targets 114 bits.
An explicit `--security-bits 112` retains the earlier integer and opening
targets for historical reproduction; existing results keep their original labels.
Limber's IntEval key format is version 2; its SNARK transcript
binds the public shape and actual serialized verifier-key configuration.
Brakedown keys also bind the runtime code/opening settings.

F2Z's new profile binds its actual Ligerito configuration and the canonical
statement into the transcript and rejects incompatible opener settings. The
legacy one-copy `limber114` transcript pin is retained. The fingerprint defect
bound stays below `2^8210` for every batch because copying does not enlarge an
individual row. The separate F2Z reduction magnitude bound grows from
`2^282` to `2^286` across the sweep and is recomputed for each shape.

Traces report the component bounds and actual parameters. The reporter
independently recomputes the modeled bounds, checks repetition counts, and
rejects bounds below target, missing range-check accounting, and parameter
drift within trials or between thread counts. Limber's conservative public
range-check size cap is checked against the actual lookup-block count for
every measured proof.

## Prepare Limber

The required published revision is
`836c50f23e674098dcfbe42a4873f583d4e0fe3f` in
`https://github.com/wu-s-john/limber-impl.git`. Both the setup helper and the
campaign runner read this pin from F2Z's `Cargo.toml` and require Python 3.11
or newer. Prepare a checkout directly from that revision:

```sh
python3 scripts/prepare_matched_limber.py /tmp/limber-matched114
```

An existing local clone containing the pinned revision can be supplied with
`--source PATH`. The helper refuses existing destinations, checks out the
published commit with detached HEAD, and prints the path, source, and revision.
It applies no patch and creates no commit. Skip preparation if the checkout
already exists at the pinned revision.

The runner defaults to `/tmp/limber-matched114`. Use `--limber-root` for another
checkout; execution rejects a revision that differs from the Cargo dependency.

## Preview and run

The default campaign has **30 configurations**: three backends × five batch
sizes × two thread counts. Each configuration has one excluded warmup and
**ten measured proofs**. The current comparison host uses 1 and 16 Rayon
threads. Threads are not affinity-pinned. Implementation order rotates by
batch ordinal and thread order alternates between batches.

Preview without compiling, running proofs, or writing campaign artifacts:

```sh
python3 scripts/run_matched_multiswap_campaign.py --dry-run \
  --limber-root /tmp/limber-matched114 --all-threads 16
```

Canonical execution requires the external `zk-proof-profiler` validator. It
is **not bundled here**: supply the actual `scripts/zk_trace.py` file through
`--profiler`. The runner fails preflight if it is absent; the repository's
comparison-specific validator does not replace it.

To collect benchmark results while that dependency is unavailable, explicitly
select `--draft`. This runs the Rust proofs, proof verification, and all
repository comparison checks. The manifest, JSON, CSV, and HTML mark canonical
validation as pending, so this does not complete the canonical acceptance gate:

```sh
python3 scripts/run_matched_multiswap_campaign.py --draft \
  --limber-root /tmp/limber-matched114 \
  --security-bits 114 --batch-counts 1,2,4,8,16 \
  --all-threads 16 --warmups 1 --samples 10
```

`--draft` and `--profiler` are mutually exclusive. For an initial smoke run,
use `--draft --batch-counts 1 --samples 1`; that runs six configurations.

```sh
python3 scripts/run_matched_multiswap_campaign.py \
  --limber-root /tmp/limber-matched114 \
  --profiler /path/to/zk-proof-profiler/scripts/zk_trace.py \
  --security-bits 114 --batch-counts 1,2,4,8,16 \
  --all-threads 16 --warmups 1 --samples 10
```

F2Z uses the repository's Rust toolchain. Limber uses
`nightly-2026-07-01`. The runner preflights both toolchains and the validator,
records their identities, overrides inherited workload/security settings,
and compiles with native CPU flags. The benchmark commands are:

```text
F2Z:    cargo bench --bench multiswap --features unchecked
Limber: rustup run nightly-2026-07-01 cargo bench --bench multiswap_modp
```

The runner supplies `MSCFG=paper`, workload, batch, target, backend, trace
path, sampling policy, and thread count for each invocation. Use
`--output-dir`, `--campaign-id`, `--f2z-root`, or `--rustflags` to override
those controls. Omit `--all-threads` to detect physical performance cores on
macOS or physical cores on Linux. Existing output directories are rejected.

## Artifacts and acceptance

```text
bench_results/<campaign>/
├── raw/                         # original JSONL traces, one per configuration
├── logs/                        # exact commands and merged stdout/stderr
├── metadata/campaign.json       # revisions, source hashes, environment, validator
├── traces/combined.jsonl        # original records in execution order
└── reports/
    ├── canonical/               # external canonical profiler report
    └── combined/
        ├── summary.json
        ├── metrics.csv
        └── intervals.html
```

Every trace is canonically validated before accepting its configuration; the
combined trace is validated again. Comparison validation requires verified
proofs, complete intervals, exact sample counts, common statement and witness
digests within each batch, identical public-input metadata and dimensions,
consistent security parameters, and source-trace SHA-256 hashes. A failure
preserves the original logs and traces and marks the manifest failed.

Reported medians are computed over measured proofs only. Per-proof totals
use overlap-safe interval unions before median and Type-7 P10/P90 aggregation.
Compilation and public setup are excluded from headline proving times.

| Report row | Definition |
|---|---|
| Witness generation | Circuit synthesis and witness/assignment materialization |
| Commitment + proving | Commitment plus all proving work |
| Combined prover including witness | Witness generation plus commitment and proving |
| Verify | Proof verification |
| Proof bytes | Commitment plus proof; analytical portions explicitly marked |
| Peak RSS | Process high-water resident memory, including setup and warmups, excluding compiler |

PCS totals include commitment plus opening. Proof sizes include serialized
commitments and openings, with analytical estimates for F2Z PIOP/bridge data
and Limber's dynamic sumchecks. Peak RSS is not a per-trial allocation count.
JSON includes the actual security parameters and statement contract; CSV and
HTML compare timings, proof sizes, and memory by batch and thread count.

Completed traces can be rechecked and rendered with the repository-specific
validator; this does not certify external canonical validation:

```sh
python3 scripts/matched_multiswap_report.py validate /path/to/metadata/campaign.json
python3 scripts/matched_multiswap_report.py report /path/to/metadata/campaign.json \
  --out-dir /tmp/matched-multiswap-report
```

## Tests

```sh
PYTHONDONTWRITEBYTECODE=1 python3 -m unittest scripts/test_matched_multiswap_campaign.py
cargo test --release --lib piop::spartan::multiswap:: -- --test-threads=1
cargo test --release --test transcript_pins multiswap -- --test-threads=1
```

The committed `scripts/fixtures/multiswap114-preflight.json` contains parameter
and statement snapshots for all 15 backend/batch pairs, not performance
measurements. The earlier `multiswap112-preflight.json` is retained to test
historical reproduction and rejection of 112-bit traces in a 114-bit campaign.
Tests cover batch report rendering, prime-count accounting,
missing metadata, altered statements, failed proofs, incompatible parameters,
measurement boundaries, and inherited configuration overrides. Full proof
smoke runs use one measured sample per configuration and must be labeled as
acceptance checks, not as the completed ten-sample performance campaign.

# SHA-256 / P-256 ECDSA comparison

The comparison benchmark calls **F2Z Split**, **F2Z AllRows**, **Spartan MC**,
and optional **Binius64** on the same signed-message relation. All modes are non-ZK. F2Z's existing
composed prover is reused. The Spartan dependency is pinned to the implementation
on the fork's `f2z-benching` branch; `Cargo.lock` records the exact revision.

The backend commit
[`3b94130453838550a1086ee75c5d280f5c25df3d`](https://github.com/wu-s-john/Spartan2/commit/3b94130453838550a1086ee75c5d280f5c25df3d)
is published on the fork's `f2z-benching` branch. Cargo can fetch the pinned
revision from GitHub; it is also available in this machine's Cargo cache.

The [earlier measurements](sha256-ecdsa-shared-kernels-results.md) cover every
exponent 4 through 11, both thread counts, and both Spartan chunking policies.

The optional [Binius64 worker](../benchmarks/binius64/README.md) uses the rebased
`sha256-chain-ecdsa-sig-verify-benching` fork through an immutable Git revision.
It has its own Cargo workspace and toolchain; existing Binius SHA/u32/hybrid
adapters keep their existing dependency versions.

ZKPassport is deprecated and is no longer an active method. Historical outputs
remain readable with `--summarize-only`.

Every new run uses `f2z/sha256-ecdsa-fixture/standard-p256/v1` fixtures. Signatures
are not normalized: valid low-s and high-s signatures are both accepted, with
`0 < r,s < n`. Export/import uses this same generator and validation. Regenerate
legacy low-s-profile fixtures for a new campaign; old results are not relabelled
or resumed into the new profile. Keys and signatures are prepared before timers.

## Run the comparison

A small campaign builds the release benchmark, runs fresh worker processes,
and records every warmup and measured proof:

```sh
python3 scripts/run_sha256_ecdsa_compare.py \
  --output bench_results/sha256-ecdsa-compare \
  --spartan-splits 0:3 1:2 3:0 --targets 100 --threads 1 --reps 3
```

Use `--offline` when dependencies are cached. `CARGO_HOME`, `CARGO_TARGET_DIR`
and compiler flags are inherited; `RUSTFLAGS` defaults to `-C target-cpu=native`.
Selecting `binius64` additionally builds its isolated worker; the root comparison feature
does not enable the older Binius, Plonky3, or Limber adapters.

To compare Spartan chunkings of a 1,024-compression chain:

```sh
python3 scripts/run_sha256_ecdsa_compare.py \
  --output bench_results/sha256-ecdsa-1024 \
  --spartan-splits 4:6 6:4 8:2 10:0 --targets 100 --threads 1 32
```

`--spartan-splits r:c` selects Spartan's SHA chunking: `2^r` compressions per
instance and `2^c` instances. F2Z uses only the total exponent `i=r+c` and runs
once per total size, method, security target, thread count, and seed.

`--exponents 3 5 7` sweeps every Spartan `r+c=i` decomposition at those total
sizes. It is mutually exclusive with `--spartan-splits`. Supported total exponents are 3..16;
large cases can exceed the selected memory or time limit and remain recorded
as failures. Defaults are exponents 3,5,7; both F2Z targets; one thread and all
logical CPUs; seed 0; one warmup and three measured repetitions. Use
`--methods f2z-split f2z-all` for the F2Z-only comparison, or `spartan-mc` for
Spartan alone. `--seeds 0 1 2` changes fixtures without changing the workload.

An already-built worker can be supplied with `--binary`. Its direct interface is:

```text
sha256_ecdsa_compare --method f2z-split|f2z-all|spartan-mc|binius64
                    --r R --c C
                    [--target 100|128] [--threads N] [--reps N] [--seed N]
                    [--fixture FILE] [--binius64-worker PATH]
```

It also accepts Cargo's automatic `--bench` flag:

```sh
cargo bench --profile release --features sha256-ecdsa-compare \
  --bench sha256_ecdsa_compare -- \
  --method spartan-mc --r 3 --c 0 --threads 1 --reps 3
```

The runner's default limits are 48 GiB of virtual address space on Linux and one
hour per case, configurable through `--memory-gib` and `--timeout`. macOS does not
support this address-space limit; it collects peak RSS and enforces the timeout.
The manifest records whether the memory limit is enforced. Rerunning resumes
missing cases; `--retry-failed` also retries recorded failures. An output
directory cannot mix binaries or runners with different SHA-256 hashes. The runner uses
Unix process/resource APIs; per-process peak RSS is collected with GNU time on
Linux and BSD time on macOS, with both normalized to bytes.

## Common statement and workload

The external statement is `(i,Qx,Qy,r_sig,s_sig)`: one P-256 key, the exact
signature, and the compression exponent. The four values are canonical 32-byte
big-endian values. The message and digest are witnesses. The verifier does not
receive a separately expected message. No method in this comparison promises
zero knowledge.

```text
B = 2^r                 compressions per Spartan SHA instance
K = 2^c                 number of Spartan SHA instances
N = B*K = 2^(r+c) = 2^i  total compressions, including padding
message_bytes = 64*(N-1)
signatures = 1
```

Each workload hashes one message of `N-1` complete blocks followed by one
mandatory standard SHA padding block. For `N=1024`, the message is 65,472 bytes.
A 2 KiB message needs 33 compressions and is a different workload.

The worker generates the same message, key and signature for each `(i,seed)`
across all methods and chunkings. It signs and validates the fixture outside the
timers. Those host checks do not replace the ECDSA circuit. The runner checks
fixture identifiers across completed cases and fails on a mismatch.

F2Z accepts only the total exponent. Its records leave `r,c` null, and the
runner measures each F2Z `(i,target,threads,seed)` case once, even when comparing
many Spartan chunkings. F2Z's packed-inner prefix is fixed at 4; this parameter
is unrelated to the Spartan chunk exponent.

## Methodologies

| Method | Arithmetization and reduction | Application commitments/opening |
| --- | --- | --- |
| F2Z Split | Boolean source map plus integer rows; 6,807 nonlinear P-256 rows enter the outer sumcheck; linear SHA/P-256 rows join the common inner sumcheck | One source commitment and one virtual F2Z opening |
| F2Z AllRows | Same F2Z arithmetization; all original SHA/P-256 rows enter the outer sumcheck, followed by the common inner sumcheck | Same commitment architecture and opening |
| Spartan MC, non-ZK | Bellpepper SHA chunks and native P-256 R1CS; NeutronNova batch folding, paired outer and inner sumchecks | K SHA commitments and one core commitment, batched into one direct Hyrax opening |
| Binius64, non-ZK | Current fixed SHA-256 circuit and standard P-256 gadget using complete arithmetic and four-bit joint scalar multiplication | Witness oracle commitment and BaseFold opening |

`AllRows` isolates the benefit of excluding linear rows from F2Z's outer
sumcheck. It remains F2Z's arithmetization and is not Bellpepper's SHA circuit.
F2Z uses no NeutronNova folding here. Constraint counts are reported in each
system's representation and are not directly interchangeable.

F2Z's virtual map supplies the IV, wires adjacent states, substitutes padding,
and aliases the final SHA digest into ECDSA. It binds the key/signature with
public-bit equations and includes the essential constant-one affine equation.
The outer terminal claims join the linear checks in one inner sumcheck; the
scaled terminal claim is discharged against the original source commitment.
See the [F2Z adapter description](sha256-ecdsa.md).

Spartan uses two circuit shapes and `K+1` instance/witness pairs. Chunks expose
blocks and input/output states as authenticated auxiliary public inputs, using
range-constrained 32-bit words. The verifier checks the IV, every adjacent
boundary, exact final padding, final digest link, and expected key/signature.
All this auxiliary data is counted in the proof size. Public auxiliary data
can reveal the message, consistent with the non-ZK requirement.

The Spartan protocol commits before challenges, folds the SHA instances while
retaining the folding residual, and checks SHA/core outer reductions with
shared challenges. Paired inner reductions produce evaluations at the same
witness point. Both claims bind before a fresh batching scalar combines the
witnesses, commitments and blinds for one direct Hyrax opening. The verifier
checks its returned evaluation against the combined claim. For `c=0`, the
sole SHA instance is used directly with target zero and no folding rounds.

The non-ZK adapter in the Spartan2 fork reuses the optimized folding kernels in
`neutronnova_zk.rs` and the batched kernels in `sumcheck.rs`. Both ZK and non-ZK
entry points call the same arithmetic; round callbacks select verifier-circuit
processing or public transcript messages. The non-ZK verifier checks the round
and terminal equations natively and uses the existing direct Hyrax opening.
Small-value cache construction is included in the measured protocol phase.
The Bellpepper witness layout and cache accounting differ from stock applications
using shared/precommitted witnesses. Measurements describe this non-ZK adapter,
not the paper's published ZK Vega timings. The optional
monolithic Spartan control is deferred; its existing IPA opening would also
need alignment before using it to isolate folding cost.

Historical reports at commit `bf99f4f8` used a separate implementation of the
folding and sumcheck arithmetic. They do not measure the shared-kernel adapter.

## Native P-256 circuit

`T256HyraxEngine` makes the constraint field equal to P-256's coordinate field.
The gadget constrains `[s_sig]R = [z]G + [r_sig]Q`, with:

- Valid finite points and complete projective addition/doubling formulas.
- Canonical, nonzero `r_sig,s_sig < n`, including valid high-s signatures.
- All 256 digest bits used as integer `z`, without reduction modulo the
  coordinate field.
- Canonical coordinates and correct integer `x(R) mod n = r_sig`, including
  an explicit bound that excludes modular wraparound in the quotient branch.

The witness generator computes `R` with RustCrypto; the circuit independently
checks the relation. Setup uses missing witness values and remains reusable
across messages, keys and signatures. Boolean SHA witnesses use Hyrax's small
value commitment path; P-256 field witnesses use its general path.

## Timing, size and security

| Metric | Definition |
| --- | --- |
| `setup_ms` | Public relation, circuit shape, and key preparation |
| `witness_ms` | Fresh witness generation, including chaining states and signature hints |
| `commit_ms` | All application witness commitments |
| `protocol_ms` | After commitments through the completed proof, including matrix products, folding, sumchecks, grinding where applicable, and opening |
| `piop_ms` | All non-opening protocol time: `protocol_ms - opening_ms`, including projection, preparation, matrix work, folding, sumchecks, transcript work and boundary overhead |
| `iop_ms` | IOP/PCS opening time: F2Z's complete virtual opening path, or Spartan's combined witness/blind construction and direct Hyrax opening; equals `opening_ms` |
| `prove_ms` | `commit_ms + protocol_ms` |
| `witness_to_proof_ms` | Per-sample `witness_ms + prove_ms` (phase sum) |
| `e2e_prover_ms` | Independently measured elapsed time from fresh witness generation through proof completion; excludes reusable setup, fixture signing, codec and verification |
| `verify_ms` | Complete application verification, including Spartan's native linking checks |
| `codec_ms` | Proof encoding and decoding, outside prover/verifier timers |
| `proof_object_bytes` | Backend proof object; F2Z excludes its separately supplied commitment |
| `proof_material_bytes` | Complete encoded proof material, including commitments, auxiliary public inputs, and framing |
| `statement_bytes` | Expected external statement, separately counted: 129 bytes |
| `peak_rss_bytes` | Whole worker peak, including setup and verification |

The F2Z wire envelope contains its source commitment and encoded proof; the
Spartan proof already contains its instances and commitments. Every warmup and
sample is decoded and verified using the decoded proof material. There are no
matrix products hidden in untimed per-message preparation. Report witness
through proof for complete fresh-input cost: some work naturally occurs during
witness generation in each implementation.

The eight comparison metrics are `commit_ms`, `witness_ms`, `piop_ms`,
`iop_ms`, `verify_ms`, `proof_material_bytes`, `peak_rss_bytes`, and `e2e_prover_ms`. The runner
derives PIOP and IOP/PCS for each original sample before aggregating. These are
disjoint accounting categories: `commit + PIOP + IOP/PCS = prove` per sample.
The PIOP column includes work outside the named sumcheck scopes and is not
just their sum. Spartan's IOP/PCS column names the corresponding PCS opening
stage; it is not a separate oracle protocol. The timed opening boundary leaves
F2Z's opening-input preparation in the non-opening category.

`samples.csv` includes every warmup and measured sample. Its peak-memory value
is the whole worker's high-water mark repeated on that worker's samples;
memory is not measured independently for each proof. `summary.csv` contains
one row per case with median times and the worker's peak memory.

F2Z targets 100/128 use **round-by-round economic Fiat–Shamir accounting with
proof of work**, and separately report a conservative statistical bound.
They do not claim statistical error `2^-100`/`2^-128`. Spartan records its T256
DLOG assumption, P-256 coordinate field, Keccak256 transcript and direct Hyrax
opening. Its nominal 128-bit group security is not the same accounting model.
Keep those labels visible when interpreting timing differences.

Repeated F2Z trials at one seed reuse the fixture and transcript, hence the
same grinding tasks. Multiple seeds provide separate grinding instances.
Outer/inner/opening timers help distinguish constraint cost from grinding;
F2Z's detailed nested timers overlap and must not be summed. Spartan reports
non-overlapping phase timers; folding is marked unavailable when `c=0`.

## Output and validation

The output directory contains `manifest.json`, `requested_cases.json`, complete
per-case JSON/stdout/stderr records, `summary.csv`, `samples.csv`, and `comparison.json`.
The manifest records the binary hash, source revision/status, compiler, build
fingerprint, machine and relevant environment. A source patch and copies of
the new worker/runner accompany dirty-root builds. Each row records its pinned
Spartan or Binius revision and a fixture identifier. The CSV takes medians of per-sample
totals, not sums of medians. Failed and timed-out cases remain visible.

Existing runs can receive the seven-column breakdown without rerunning proofs
or changing their original raw records:

```sh
python3 scripts/run_sha256_ecdsa_compare.py \
  --output bench_results/sha256-ecdsa-compare-i4-i11 --summarize-only
```

The Spartan tests cover folding counts including zero rounds, proof-message
and opening tampering, malformed codecs, setup reuse, chain/padding/digest
links, invalid signatures/keys, high-s variants, digest boundaries, integer
wraparound exclusion, and exceptional point operations. The campaign tests
cover F2Z deduplication, sample validation, fixture mismatches and aggregation.
The normal F2Z cryptographic path is unchanged apart from dimension-reporting
accessors.

Useful entrypoints are the [comparison worker](../benches/sha256_ecdsa_compare.rs),
[campaign runner](../scripts/run_sha256_ecdsa_compare.py), and
[Spartan application adapter](https://github.com/wu-s-john/Spartan2/blob/3b94130453838550a1086ee75c5d280f5c25df3d/src/sha256_ecdsa/mod.rs).
Raw `bench_results/` artifacts are ignored by Git; archive the output directory
when sharing measurements. Older SHA-only/raw-compression benchmarks and earlier
dirty-build F2Z snapshots are separate workloads or builds, not this comparison.

The [complete i=4..11 measurements](sha256-ecdsa-i4-i11-results.md) cover every
length from 16 through 2,048 compressions, with all seven requested metrics for
64 configurations and 256 verified proofs. The
[initial comparison measurements](sha256-ecdsa-comparison-results.md) preserve
the earlier 24-case campaign at 8, 32 and 128 compressions.

### Binius measurement boundaries

Binius setup builds the fixed circuit and reusable prover/verifier parameters from
only the exponent and security parameters. Every sample creates fresh witness
values and arithmetic hints. Fixed generator constants and reusable allocation
buffers belong to setup; key-specific tables and message-dependent work are
recomputed per proof. The shared fixture is held once per worker.

`witness_ms` includes circuit evaluation and witness packing. `commit_ms` covers
the witness oracle commitment. `opening_ms` includes ring switching and the
complete deferred BaseFold channel finish; `protocol_ms` includes that opening
and the remaining reductions. Stage durations are disjoint wall-clock spans.
The verifier reconstructs expected public words from `(i,Qx,Qy,r,s)` and consumes
all proof bytes; it does not read the private witness.

The worker uses SHA-256 Merkle hashing, BaseFold at rate 1/2, and explicit 100/128
FRI query targets. It records the actual query count and identifies these as
query targets, not a claim about aggregate protocol security. The mathematical
statement is shared; the security models remain separately identified.

Peak memory is the whole worker high-water mark, including setup and verification.
The runner obtains it with GNU time on Linux or BSD time on macOS; unavailable
measurements stay null.
Historical records without independent e2e timings retain an unavailable e2e field.

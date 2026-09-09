# SHA-256 and ECDSA through one F2Z commitment

Enable `ecdsa` to use `f2z::piop::spartan::ecdsa_sha256`. The statement is one
P-256 public key, one signature, and the compression exponent. Coordinates and
signature scalars are fixed 32-byte big-endian words. The message, digest, and
inverse hints belong to the witness. This adapter does not add zero knowledge.

For `N = 2^i`, `i = 3..16`, the message contains `64*(N-1)` bytes. A final,
circuit-enforced padding block brings the total to exactly N compressions.
For example, N=8 hashes one 448-byte message; N=65536 hashes one 4,194,240-byte
message. These are standard SHA-256 messages, each followed by one ECDSA check.

```rust,ignore
let relation = prepare_sha256_ecdsa(10, 100, OuterMode::Split)?;
let witness = generate_sha256_ecdsa_witness(&relation, &statement, &message)?;
let commitment = commit_sha256_ecdsa(&relation, &witness)?;
let proof = prove_sha256_ecdsa(
    &mut prover_transcript, &relation, &statement, &witness, &commitment, 4,
)?;
verify_sha256_ecdsa(
    &mut verifier_transcript, &relation, &statement, &commitment.commitment, &proof,
)?;
```

The virtual assignment is `h = lift(M f over F2)`. SHA occupies
`h[instance + N*local_wire]`; the P-256 assignment follows it. Source bits are
packed by compression, with one shared constant and no independent chaining
states. P-256's digest inputs alias the final SHA output source bits. Padding
is substituted by the map, so choosing arbitrary final-block source bits cannot
change the padded message being proved. The existing gadgets generate both
the exact constraints and packed witnesses.

`Split` partitions exact integer rows by structure. SHA's 184 rows per
compression and 254 P-256 rows are linear. Only the 6,807 nonlinear P-256 rows
enter the outer sumcheck. The linear family also contains 1,024 public-bit
equalities and the affine equation `h[0]=1`. The latter prevents the all-zero
assignment from satisfying a homogeneous version of the relation.

After the commitment, map, relation, public statement, and security schedule
are bound, the transcript samples a 113-bit prime and the outer row point.
The outer proof fixes its three terminal claims before `rho`, the linear row
point `sigma`, and `gamma` are sampled. One inner sumcheck proves

```
D = A(rx,.) + rho B(rx,.) + rho² C(rx,.) + gamma Lᵀeq(sigma,.)
<D,h> = a* + rho b* + rho² c* + gamma <eq(sigma,.),b>
```

Its terminal claim retains the scale: `D(ry) * h(ry) = value`. The F2Z opening
uses `D(ry)*eq(ry,.)` as its linear weights and ends in the existing virtual
adjoint/dual-basis bit opening. No division by a possibly zero scale is needed.
SHA's coefficients and map remain factored; the ECDSA tail is a compact
correction. The packed inner prover supports prefix widths 0 through 4.

The source commitment fixes the assignment before the projection prime and
row challenges. Over the sampled field, if an outer terminal claim disagrees
with the corresponding matrix evaluation, or a linear constraint fails, the
shared identity is a nonzero polynomial in `rho`, `sigma`, and `gamma`. Sampling those challenges
after the outer triple prevents the prover from choosing that triple to cancel
a linear defect. The shared inner sumcheck and F2Z opening then bind the identity
to the same assignment. The separate affine check `h[0]=1` is essential here.

`AllRows` is a comparison mode: all original SHA and P-256 rows enter the outer
sumcheck. It retains the same source commitment, assignment layout, affine
public checks, and one shared inner reduction. It materializes larger outer
tables and therefore has a higher memory cost.

The 100 and 128 targets use **round-by-round economic Fiat–Shamir accounting**.
They do not assert statistical error `2^-100` or `2^-128`. `security()` reports
the economic minimum and a separate conservative statistical union bound.
The exact Boolean defect norm determines the projection-prime term. Work per
challenge block is capped at 32 bits. The host forest bound deliberately uses
a conservative degree and draw-count upper bound.

Flock's zero-OOD UDR schedule has an additional atomic challenge guard. It
sums the errors of events sharing a draw block, upgrades native PoW in place,
and adds a host nonce only at blocks without a native nonce. Query retries
and their alpha vector share one block. Existing adapters retain their original
transcripts. The new proof codec rejects noncanonical fields, trailing bytes,
and excessive sumcheck lengths; verification re-derives the encoded modulus.

## Benchmarking

Build with the repository's release settings and run the resulting benchmark
executable through the campaign script:

```sh
RUSTFLAGS="-C target-cpu=native" \
  cargo build --release --locked --features ecdsa --bench sha256_ecdsa
python3 scripts/run_sha256_ecdsa_bench.py \
  --binary target/release/deps/sha256_ecdsa-<build-hash> \
  --output bench_results/sha256-ecdsa
```

The default campaign covers exponents 3..16, both modes and targets, one thread
and all available threads, one warmup and three measured repetitions. Use
`--exponents 3 7 --targets 100 --threads 1 --reps 1` for a smaller run.

Every process signs and verifies its fixture with RustCrypto before timing.
Witness timing includes inverse hints. Prover timing includes commitment;
public preparation, witness generation, and serialization are reported or
performed separately. Every measured proof is decoded and verified. JSON
records include phase timings and both security figures; process summaries
include peak RSS. Failed or timed-out cases are retained. The runner defaults
to a 48 GiB virtual-memory limit and a one-hour timeout per configuration;
both are configurable. Rerunning resumes missing cases; `--retry-failed`
also retries recorded failures.

Use the same compiler flags throughout a comparison. In particular, native CPU
features enable the x86 carry-less multiplication implementation used by F2Z.
The CSV summary reports median sample times; the JSON files retain individual
samples and nested phase timings. Nested timers overlap and should not be added
to recover total proving time. `ecdsa:boundary_grinding_prove` reports the
initial and batching proof-of-work separately from sumcheck-round grinding.
`proof_bytes` is the serialized `Sha256EcdsaProof`; the separately supplied
source commitment and public statement are not included in that size.

Repetitions use the same deterministic fixture and transcript, so they measure
runtime variation for the same proof-of-work nonces. Different modes and security
targets derive different nonces. In particular, differences in 128-target total
time can include substantial grinding variation; use the outer and shared-inner
phase timings when comparing constraint costs.

# Arithmetic Falcon witness optimization validation

The arithmetic-only Falcon-512 and Falcon-1024 relation remains public `(h,t)`
with exact supplied private `(s1,s2)`, a negacyclic ring equation, and the integer
norm bound. SHAKE-256 and HashToPoint remain outside this API.

Validation used `--locked --offline --release --features falcon-hybrid`,
`RUSTFLAGS='-C target-cpu=native'`, and
`CARGO_TARGET_DIR=/tmp/bitz-arithmetic-baseline-target`.

- Arithmetic suite: **77 passed**, 0 failed; two large qualifications ignored by
  default and then run explicitly.
- Explicit large qualifications: **2 passed**, covering batch 32 and 1024,
  Falcon-512/1024, and security targets 100/128.
- Shared integer bridge and binary source sumcheck: **27 passed**.
- Full Falcon regression `hybrid_falcon_roundtrip_and_tampering`: **1 passed**.
- Self-contained vendored field module: **13 passed**, one pre-existing ignored
  AVX512 exhaustive test. Run both with default compilation and optimized native compilation. This includes
  fixed-basis references for degrees 9, 10, and 11, randomized SIMD/scalar
  comparisons, coordinate/scalar extremes, malformed inputs, and guard sentinels.
- `git diff --check`: clean.

The arithmetic suite includes independent schoolbook ring references, exact
signed15 decoder coverage, maximum exceptions, norm boundaries, noncentered
coefficients, inactive slots, changed public inputs, changed exception metadata,
malformed proofs, inconsistent source bits, cross-signature norm borrowing, and
complete proof/transcript agreement across packing strategies and worker counts.
The protocol domain is v6; earlier arithmetic proofs are not compatible.

Reproduction commands (the test binary can be invoked directly after building):

```sh
RUSTFLAGS='-C target-cpu=native' cargo test --locked --offline --release --features falcon-hybrid --lib algebraic -- --test-threads=1
RUSTFLAGS='-C target-cpu=native' cargo test --locked --offline --release --features falcon-hybrid --lib large_batches_at_both_security_targets -- --ignored --test-threads=1
RUSTFLAGS='-C target-cpu=native' cargo test --locked --offline --release --features falcon-hybrid --lib -- hybrid::integer_bridge hybrid::joint_sumcheck --test-threads=1
RUSTFLAGS='-C target-cpu=native' cargo test --locked --offline --release --features falcon-hybrid --lib hybrid_falcon_roundtrip_and_tampering -- --test-threads=1
```

Timing comparisons are recorded separately by `benchmark.py`. The baseline
snapshot was taken before these source changes, and its binary was preserved as
`/tmp/bitz-arithmetic-baseline-binary`. Both variants use the same native release
build flags and deterministic Falcon input corpus; hashing and input generation
are excluded from prover timing.

The final matched matrix in `final/` contains 192 measured proofs and 64 warmups;
all verified, and every baseline/candidate pair had the same input digest.
All 128 final-candidate proofs also exactly matched the initial candidate's
source root, full proof Debug digest, and payload size. The final performance
changes therefore preserved the candidate protocol's proof contents.

Witness reduction is established independently of latency: maximum live bits
fall about 20%, and physical source allocation falls 18.75% above the PCS floor.
Measured payloads are 2.3–12.0% smaller. Batch-1024 single-thread prover medians
improve 1.2–3.5%; eight-thread results are approximately unchanged to 2.2% slower.
Small-batch results include a 3.3% regression. These two-seed measurements do not
establish a general proving-speed improvement; see `README.md` for every shape
and the measurement limits.

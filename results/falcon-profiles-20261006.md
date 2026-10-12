# Falcon profile comparison

Single-run validation measurements from 2026-10-06.
The full SHAKE-256 and HashToPoint statement is included. All twelve proofs
verified successfully. These are smoke measurements, not throughput claims.

- Host architecture: x86_64.
- Compiler: rustc 1.98.1 (48a229cea, 2026-09-01).
- Rust test profile, opt-level=2, debug=0; four Rayon threads.
- One signature per proof; maximum profile batch 1024.
- Upstream original-Falcon inputs, deterministic per-degree seed.
- Times exclude key/signature generation and circuit preparation.
- Proof size is the existing payload estimator; public statements are separate.
- Security estimates use the existing work-normalized algebraic/IOP model.

Command (CARGO_TARGET_DIR was set to an isolated temporary build directory):

```sh
CARGO_PROFILE_TEST_OPT_LEVEL=2 CARGO_PROFILE_TEST_DEBUG=0 RAYON_NUM_THREADS=4 \
  cargo test --offline --features falcon-hybrid --bench falcon_profiles
```

Validation completed:

- 220 Falcon-related library tests passed; three pre-existing large qualification
  or kernel benchmark tests remained ignored.
- 17 shared-opening library tests passed.
- 23 profile integration tests passed, including tiny-degree full proofs,
  both protocols, 100/128-bit targets, and padded batches.
- Four upstream interoperability tests passed.
- Two invalid-profile compile-fail doctests passed.
- The `falcon`-only library compatibility check passed.

Some library tests overlap between the two filters. Existing compiler warnings
remain in the repository and dependencies.

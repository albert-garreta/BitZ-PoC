# Validation of code commit 94c8d26

All three compiler checks completed successfully with Rust 1.98.1 and `RUSTFLAGS="-C target-cpu=native"`. They emitted warnings, not errors.

```sh
cargo +1.98.1 check --locked --offline --lib --features bench-internals,native-mul-compare,unchecked,sha256-ecdsa-compare
cargo +1.98.1 check --locked --offline --all-targets --features bench-internals,native-mul-compare,unchecked,sha256-ecdsa-compare,hybrid
cargo +1.98.1 check --locked --offline --manifest-path benchmarks/binius64/Cargo.toml --all-targets
```

The release test binary was built with `--features hybrid` and the same Rust flags. With `RAYON_NUM_THREADS=8` and `--test-threads=1`, each of these existing tests passed:

- `hybrid::tests::hybrid_roundtrip_and_tampering`
- `piop::spartan::f2z::tests::u32_mul_roundtrips_and_is_deterministic`
- `binius_ligerito::tests::selectable_rates_bind_both_oracles_and_reject_cross_rate_proofs`

The first test ran through Cargo. The other two ran directly from the just-built `target/release/deps/f2z-5860d534f5955af7` test binary to avoid another process's build lock. The queued duplicate Cargo command was cancelled; no other task's process was stopped.

`PYTHONDONTWRITEBYTECODE=1 python3 scripts/test_sha256_ecdsa_compare.py` passed all 10 tests. Staged source changes passed `git diff --cached --check`. All 291 original evidence hashes and captured JSON/JSONL records validated before this commit.

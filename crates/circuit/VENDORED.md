# Vendored circuit crates

`crates/circuit` and its `crates/field` support crate were copied from
<https://github.com/worldfnd/f2z-benchmark> at commit
`a5dc2e25cc8ad4bcc1faaad3137b4613a7926f49`.

The Rust sources and benchmark sources match that revision, apart from local
`rustfmt` import ordering. The Cargo manifests replace inherited workspace
values with the same explicit package metadata and dependency versions. In
particular, `field` retains upstream's `crypto-primitives` revision
`3c4232b`; the main `f2z` package uses an older vendored API, so unifying the
two would change the copied field implementation. The nested `Cargo.lock`
files were generated for reproducible standalone tests and benchmark builds.

The upstream project declares these crates under `MIT OR Apache-2.0` and
ships an MIT license with the repository.

Local addition (2026-09-13, not upstream): `matrix_wengert::PreparedWengertEvaluator::
apply_weighted`, the reverse pass driven by one Montgomery weight triple per row
instead of one challenge and `x`, so a row can be weighted on `C` alone. Used by
`f2z::piop::spartan::ecdsa_sha256` for both the prover's batched matrix MLE and
the verifier's evaluation of it.

Local addition (2026-09-14, not upstream): `WengertTape::prepare` reduces its
coefficients by a word-Horner kernel (`horner_reduce_2`: one Montgomery product by
`2^64·R mod q` per word) for moduli above `2^64`; `RuntimeModulus::reduce` (bit-serial
below `2^127`) stays as the test oracle.

Local addition (2026-09-14, not upstream): the forward program of the tape
(`forward_offsets`/`forward_terms`, the edges grouped by sum node) and
`PreparedWengertEvaluator::apply_forward_weighted` (`ForwardColumns`): the scalar
`Σ_row (w_A·A_row + w_B·B_row + w_C·C_row)·x` by a forward pass, without the column
vector; the reverse output buffer is allocated by the first reverse pass. Used by the
F2Z SHA-256 + ECDSA verifier.

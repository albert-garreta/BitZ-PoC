# Vendored circuit crates

`crates/circuit` and its `crates/field` support crate were copied from
<https://github.com/worldfnd/f2z-benchmark> at commit
`a5dc2e25cc8ad4bcc1faaad3137b4613a7926f49`.

The original import preserved the Rust sources apart from local `rustfmt`
import ordering and expanded workspace metadata in the Cargo manifests.
Subsequent local changes migrated the circuit to the shared `vendor/field`
implementation and fixed-width integer coefficient storage. The nested
`Cargo.lock` supports standalone tests and benchmark builds.

The upstream project declares these crates under `MIT OR Apache-2.0` and
ships an MIT license with the repository.

Local addition (2026-09-13, not upstream): `matrix_wengert::PreparedWengertEvaluator::
apply_weighted`, the reverse pass driven by one Montgomery weight triple per row
instead of one challenge and `x`, so a row can be weighted on `C` alone. Used by
`bitz::piop::spartan::ecdsa_sha256` for the prover's batched matrix MLE.

Local adaptation (2026-09-16): `WengertTape::prepare` uses the shared field's
`PreparedSignedProjection` for its coefficient words. This retains the native
projection improvement from `BitZ-PoC`'s `sha256-ecdsa-verifier-opt` branch without
reintroducing its older arithmetic implementation.

Local addition (2026-09-14, not upstream): the forward program of the tape
(`forward_offsets`/`forward_terms`, the edges grouped by sum node) and
`PreparedWengertEvaluator::apply_forward_weighted` (`ForwardColumns`): the scalar
`Σ_row (w_A·A_row + w_B·B_row + w_C·C_row)·x` by a forward pass, without the column
vector; the reverse output buffer is allocated by the first reverse pass. Used by the
BitZ SHA-256 + ECDSA verifier.

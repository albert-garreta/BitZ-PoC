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

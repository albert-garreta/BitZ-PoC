# Vendored crypto-primitives

Vendored from https://github.com/NethermindEth/crypto-primitives.git at rev
`2cf39db886a76dc3e961cbb9c86fb5ab042381ef` (the last revision with the
`PrimeField`/`FromWithConfig` trait family this crate is written against;
upstream removed it in the "No-overhead fields" redesign, whose adoption is
tracked separately). Licensed Apache-2.0 (see LICENSE).

Local modifications, all to accept crypto-bigint 0.7.5 (upstream capped at
0.7.0-rc.9) and rand 0.10:

- `Cargo.toml`: crypto-bigint `>= 0.7.0-rc.9, < 0.8`; rand `0.10`.
- subtle-based trait impls ported to crypto-bigint's own ct traits
  (`ConstantTimeEq -> CtEq`, `ConstantTimeGreater -> CtGt`,
  `ConstantTimeLess -> CtLt`, `ConditionallySelectable -> CtSelect`,
  `subtle::Choice -> crypto_bigint::Choice`); identical select semantics
  (self on FALSE, other on TRUE).
- `MontyForm`/`MontyParams` renamed to `FixedMontyForm`/`FixedMontyParams`;
  params now passed by reference; `is_zero` returns `Choice` (`.into()`).
- `Random::try_random` -> `try_random_from_rng` (`TryRngCore` -> `TryRng`).
- `ConcatMixed` bound rewritten as `Concat<RHS_LIMBS, Output = ...>`;
  `Int::FULL_MASK` const dropped (removed upstream, unused here).
- The boxed-monty field wrapper module is dropped (unused by f2z; its
  `BoxedMontyForm` API drifted).

Everything else is byte-identical to the upstream revision.

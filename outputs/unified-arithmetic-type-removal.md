# Unified arithmetic removal inventory

The declaration appendix is audited against approved baseline `6271724d`.
Historical source paths/line numbers below identify deleted declarations, not
current file locations. F2Z, circuit and local Flock now use shared arithmetic;
production dependencies on the old packages are gone. Test oracles and frozen
comparators remain permitted. Performance qualification is still outstanding.

## Current disposition

| Status | Declarations |
|---|---|
| Removed | Both legacy Fq definitions and FqDefault; BinaryFieldGF128/BinaryFieldB127/BinaryF2Poly; Flock F8/F128/F256Unreduced; WideGf128/WideB127; duplicate FixedBasePow/FixedGfMul; circuit Z and P-256 host U256/Wide |
| Removed | ProjArith, FieldArith, Arith, ArithFixed, RawMontyCtx, RuntimeModulus, WitnessScale, backend markers/accumulators and immediate/crypto-bigint production reducers |
| Removed | SpartanReductionStrategy, ProveOptions, inner-policy selectors, StoredInteger, RuntimePrime, Sha256ModQContext, MultiswapFingerprintContext, local MillerRabin, unused MBSInnerProduct/ScalarProduct |
| Shared arithmetic | Fp/FpCtx, StaticFp/StaticFpOps, Uint/Z, exact products/accumulators, Gf8/Gf128/B127/F2Poly and prepared operations |
| Retained consumer contracts | Polynomial coefficient/evaluation requirements, checked bit-vector inner-product adapter, transcript encoding adapters, sumcheck orchestration and independent test-oracle hooks |
| Retained storage/plans | SparseIntegerMatrix, CoefficientIndex, IntegerTable segments, NativeWideProducts, NativeU128Witness, NativeLimbWitness, FoldedWitness, matrix/NTT/Wengert/packing plans |
| Explicit user exception | MultiSwap 192-bit low-u128/carry-word accumulation; exact multiplication/reduction still use field |

`SpartanF2zField` is now an abbreviation for shared `Fp<2>`; `FieldConfig` names
shared `FpCtx<2>`. Neither denotes a distinct field implementation. Runtime
values carry no modulus pointer. Owners retain the context once.

Circuit and MultiSwap coefficients, generator arithmetic and private witness
checks use declared limb widths. `IntegerTable` groups `Z<L>` storage by width
instead of widening every entry. The old package was deleted after preserving
its exact baseline source and hashes as an independent test/benchmark oracle.
See [implementation status](../vendor/field/IMPLEMENTATION.md) for current
validation and unresolved performance acceptance.

## 1. Duplicate scalar, product, and preparation types

| Existing declaration | Source location | Intended replacement |
|---|---|---|
| `BinaryFieldGF128` | `src/poly/univariate/binary_gf128.rs:86` | `field::Gf128` with `Gf128Ops` |
| `BinaryFieldB127` | `src/poly/univariate/binary_b127.rs:139` | `field::B127` with `B127Ops` |
| `WideGf128` | `src/poly/univariate/binary_gf128.rs:753` | `Gf128Product` or `XorAccumulator<Gf128Product>`, according to use |
| `WideB127` | `src/poly/univariate/binary_b127.rs:862` | `B127Product` or `XorAccumulator<B127Product>` |
| `FixedGfMul` | `src/poly/univariate/binary_gf128.rs:1187` | `PreparedGf128Mul` |
| `BinaryF2Poly<W>` | `src/poly/univariate/binary_f2_wide.rs:37` | Shared `F2Poly<BITS, WORDS>` and exact carryless products |
| Flock `F128` | `vendor/flock-mod/crates/flock-core/src/field/gf2_128.rs:24` | The same shared `Gf128` used by F2Z |
| Flock `F8` | `vendor/flock-mod/crates/flock-core/src/field/gf2_8.rs:20` | Shared `Gf8` |
| Flock `F256Unreduced` | `vendor/flock-mod/crates/flock-core/src/field/gf2_128.rs:151` | Semantic shared GF128 product/accumulator |
| PCS `Fq` | `src/pcs.rs:358` | Shared static prime declaration for the existing modulus |
| Shared legacy `Fq<const Q: u128>` | `vendor/field/src/fq.rs:29` | `StaticFp<P, 2>` with a `PrimeSpec<2>` marker |
| Shared legacy `FqDefault` alias | `vendor/field/src/fq.rs:17` | Direct use of the static prime declaration; no compatibility alias |
| PCS `FixedBasePow` | `src/pcs.rs:112` | Shared provider-based fixed-base preparation |
| Legacy GF128 `FixedBasePow` | `vendor/field/src/gf128/pow.rs:134` | Consolidate with `preparation::FixedBasePow<C, E>` |
| Circuit witness `Z<LIMBS>` | `crates/circuit/src/witgen.rs:18` | `field::Z<L>` |
| P-256 host integer `U256` | `crates/circuit/src/p256.rs:682` | `field::Uint<4>` |
| P-256 host `Wide<L>` | `crates/circuit/src/p256.rs:727` | `Uint<L>` and `UintProduct<A, B>` for exact products |

Both baseline legacy `Fq` definitions are now removed. PCS uses the shared `Q100Element` declaration with the original modulus.

All duplicate scalar/product/context declarations above are removed. Local Flock
re-exports shared types. Generic consumer hooks delegate to shared arithmetic;
independent old reducers compile only in test and historical benchmark modules.

Fixed-base preparation has distinct public/private exponent contracts. Preserve
qualified public-exponent optimizations explicitly; private exponentiation cannot
reuse secret-indexed comb tables merely to retain old timings.

## 2. Contexts and reduction infrastructure that collapse

| Existing declaration(s) | Source location | Intended replacement |
|---|---|---|
| `ProjArith` | `src/ext_proj.rs:393` | Shared prime context plus mixed/canonical operations |
| `FieldArith`, `Arith`, `ArithFixed` | `src/piop/spartan/protocol/bitify.rs:80`, `:132`, `:148` | The same context across supported modulus widths; no projection/field backend enum |
| `RawMontyCtx` | `src/piop/spartan/raw_monty.rs:101` | `FpCtx<2>` |
| `RuntimeModulus<L>` | `crates/circuit/src/matrix_products.rs:22` | `ModRingCtx<L>`; use `FpCtx<L>` only when prime assurance exists |
| `OptimizedReduction`, `CryptoBigintReduction` | `src/utils/delayed_reduction.rs:49`, `:53` | Delete backend marker structs |
| `Monty128Reducer<B>` | `src/utils/delayed_reduction.rs:75` | Reduction implemented inside the shared provider |
| `OptimizedMonty128Reducer`, `CryptoBigintMonty128Reducer` | `src/utils/delayed_reduction.rs:86`, `:89` | Delete aliases to selectable reducer backends |
| `MontyProductAccumulator128` | `src/utils/delayed_reduction.rs:134` | `FpProductAcc<2>` |
| `MontyLinearAccumulator128` | `src/utils/delayed_reduction.rs:140` | `FpLinearAcc<2, N>`; `N` follows actual operand width |
| `ImmediateSumcheckReducer<F>`, `OptimizedSumcheckReducer`, `CryptoBigintSumcheckReducer` | `src/piop/spartan/sumcheck.rs:296`, `:339`, `:412` | Shared MAC, reduction, and fused kernels |
| `DelayedU32InnerArithmetic`, `SelectedU32InnerArithmetic` | `src/piop/spartan/sumcheck.rs:143`, `:208` | Direct mixed-input kernel calls |
| Local `MillerRabin` | `src/utils/primality.rs:10` | Shared public primality/sampling operations |
| `MBSInnerProduct`, `ScalarProduct`, `BooleanInnerProductAdd` | `src/utils/inner_product.rs:30`, `:93`, `:122` | Shared scalar/mixed MAC and exact integer operations |

The removed `RuntimeModulus` accepted composite and even moduli greater than one.
It cannot be globally replaced by a prime context. Similarly, replacing checked
inner-product adapters requires preserving exactness through sufficient product
and accumulator widths, not silently changing their operations to wrapping ones.

## 3. Strategy structs/enums that disappear entirely

| Declaration | Source location |
|---|---|
| `SpartanReductionStrategy` | `src/piop/spartan/piop.rs:77` |
| `ProveOptions` | `src/piop/spartan/protocol/mod.rs:449` |
| `SpartanInnerNativeFold` | `src/piop/spartan/piop.rs:90` |
| `SpartanInnerFieldAccumulation` | `src/piop/spartan/piop.rs:100` |
| `SpartanInnerPolicy` | `src/piop/spartan/piop.rs:110` |
| `NativeWitnessFoldPolicy` | `src/piop/spartan/sumcheck.rs:80` |
| `FieldCoefficientPolicy` | `src/piop/spartan/sumcheck.rs:92` |
| `WitnessScale` | `src/piop/spartan/raw_monty.rs:1705` |

`ProveOptions` has only a reduction-strategy member, so nothing remains after that
member goes. Benchmark-only policy selectors can remain in immutable historical
references, not the final production source. `WitnessScale` is replaced by typed
operands and scale-specific operations; native/folded storage distinctions remain.

## 4. Arithmetic aliases whose current definitions disappear

| Alias group | Source location | Replacement |
|---|---|---|
| `SpartanF2zField`, `FieldConfig` | `src/piop/spartan/protocol/mod.rs:84`, `:87` | Direct shared `Fp<2>` / `FpCtx<2>` types |
| `Field`, `FieldConfig`, `Raw` | `src/piop/spartan/raw_monty.rs:63`, `:64`, `:68` | Typed shared field elements, contexts, or canonical integer storage |
| `ProductPair`, `LinearPair` | `src/piop/spartan/raw_monty.rs:573`, `:574` | Arrays of shared scale-specific accumulators |
| `Field`, `FieldConfig`, `LinearAccumulator`, `ProductAccumulator`, `RawMontgomery` | `src/piop/spartan/sha256/inner_sumcheck.rs:29` | Direct shared element/context/accumulator types |
| `RawMontgomery` | `src/piop/spartan/sha256/proof.rs:87` | Shared Montgomery element |
| `Field`, `FieldConfig`, `LinearAccumulator<R>`, `ProductAccumulator<R>` | `src/piop/spartan/univariate_skip_native.rs:29` | Shared types, without reducer generics |
| `Config` | `src/piop/spartan/ecdsa_sha256/mod.rs:32` | Shared prime context |

An ergonomic import abbreviation is not a second arithmetic implementation.
The removal objective is the old representation/backend contract, not banning all
local aliases. In particular, some `Raw` values currently hold canonical witness
residues: replace each use according to its actual scale rather than renaming every
`u128` to `Fp<2>`.

## 5. Simplify owners without deleting their useful role

| Existing type | Disposition |
|---|---|
| `protocol::RuntimePrime` (`protocol/mod.rs:465`) | Can collapse into `FpCtx<2>`: it duplicates `q`, `q_bits`, config, and arithmetic context. Keep protocol validation/profile metadata with the prepared relation. |
| `Sha256ModQContext` (`sha256/prime.rs:237`) | Same consolidation, preserving its protocol/profile checks. |
| `MultiswapFingerprintContext` (`multiswap/prime.rs:177`) | Also owns a profile. Retain a profile/context owner or move that profile into the prepared relation before deleting the wrapper. |
| `RawProducts`, `RawWitness`, `PiopWitness`, `NativeProducts` | Consolidate/retype storage and remove strategy fields/obsolete variants. Preserve native borrowing and segmented/folded input layouts. |
| `ModularVector`, `MatrixProducts`, `IntegerProducts` | Retain product-table containers; replace numeric payloads/contexts. |
| `StoredInteger` (`crates/circuit/src/matrix_products.rs:301`) | Redesign heterogeneous coefficient/row storage around declared limb widths. It has no arithmetic implementation. Do not replace every entry with a giant maximum-width integer and assume memory performance is unchanged. |
| `ConstraintMatrices`, `LinearCombination`, `ConstraintGenerator` | Retain circuit abstractions; replace `BigInt` coefficients with explicit-width shared integers and checked preparation. |
| `PreparedMaterializedAbc`, `PreparedWengertEvaluator`, `ReverseContext` | Retain execution plans and reuse; replace embedded legacy arithmetic. |

Low-level errors such as `DelayedReductionError` can disappear with their backend.
Protocol errors for invalid shape, modulus, soundness parameters, or decoding still
need meaningful boundary handling even if their old wrapper enum is removed.

## 6. Move private acceleration types; retain semantic containers

- Flock `WideGhashX4` (`field/gf2_128/x86_64.rs:321`) moves with its four-lane
  VPCLMUL kernels into private shared arithmetic. It is not replaceable by four
  scalar calls without measurement.
- Flock's private ARM `UnredAcc` (`genus95_curve_code/round1.rs:2389`) moves with its
  fused kernel. It may remain an internal alias.
- Shared `Wide256` remains useful as a private binary-product payload; its public
  export can disappear after consumers use semantic products and accumulators.
- `GF128Poly<D>` remains a meaningful polynomial alias, retargeted to `Gf128`.
- `AlphaPolyBasis`, Flock NTT plans/tables, proof/claim types, and matrix layouts
  retain their protocol/preparation roles.
- `BinaryRefPoly<D>`, `BinaryU64Poly<D>`, and their iterators are not another field.
  Their mixed checked-integer/XOR semantics prevent a blind `F2Poly` substitution.
  Their inner-product marker types are further consolidation candidates only after
  preserving polynomial/coefficient semantics.
- `U32MulF2zWidth` stays: W1/W8 selects commitment-cell packing.
- `Kernel`, `PrimeStrategy`, `PrimePolicy`, `Schedule`, and `LinearProveOptions`
  retain real protocol or prefix-width choices, unlike the deleted reduction strategy.
- Circuit `UInt`, `UInt256`, `UInt32`, `P256Z`, `ShaZ`, `Elem`, `Point`,
  `WengertValue`, `PackedBits`, `PackedWitness`, and witness-generator types stay.
- `PowerRun`, `ColumnMajorPackedBits`, `TailPiece`, `TailShapeState`, and
  `RepeatedState` retain geometric metadata, packed reads, factorizations, and scratch.
- `ModQCoefficients`, `BatchedMatrixMle`, and `CompositeMultilinearExtension`
  retain evaluator ownership and geometric-run metadata.
- `RoundBoundaryPolicy`, `UngrindedRoundBoundary`, and `ProverGrindingRoundBoundary`
  retain transcript scheduling; they are not reduction-backend selectors.
- The map/storage types in `src/f2map.rs` stay; they describe commitment mapping,
  not field arithmetic.

For `BinaryF2Poly<W>`, concrete mappings such as `F2Poly<128, 2>` are straightforward.
Do not assume `F2Poly<{64 * W}, W>` is a supported stable-Rust generic alias;
propagate the bit/limb parameters explicitly or resolve the shared width interface.

## 7. External imports are not local declarations

`BigInt`, `BigUint`, `FixedMontyForm`, `FixedMontyParams`, `Odd`, and `U128` are
external types. `CryptoU256` is a renamed import. Their use disappears from the
migrated production graph, but there is no corresponding local struct to delete.

The shared `Uint<L>`, `Z<L>`, static/runtime prime values, scale-tagged products and
accumulators remain. Different product domains and Montgomery scales are useful
type distinctions, even when they share an internal limb implementation.

## 8. Deleted-package declaration appendix

The following declarations are removed with `vendor/crypto-primitives`. Package
deletion includes unused adapters; this does not claim every declaration has an
active F2Z consumer. The `Fp` in this appendix is the old Ark wrapper, not shared
`field::Fp<L>`. The old matrix structs are distinct from retained circuit/F2Z matrices.

Verified from the `6271724d` source (not inferred from current usage): **11 exported structs and 65 public aliases**, excluding test-local helpers.

| Struct | Declaration |
|---|---|
| `ArkField` | `vendor/crypto-primitives/src/field/ark_ff_field.rs:33` |
| `Fp` | `vendor/crypto-primitives/src/field/ark_ff_fp.rs:36` |
| `BoxedMontyField` | `vendor/crypto-primitives/src/field/crypto_bigint_boxed_monty.rs:21` |
| `ConstMontyField` | `vendor/crypto-primitives/src/field/crypto_bigint_const_monty.rs:27` |
| `MontyField` | `vendor/crypto-primitives/src/field/crypto_bigint_monty.rs:23` |
| `F2` | `vendor/crypto-primitives/src/field/f2.rs:25` |
| `SparseMatrix` | `vendor/crypto-primitives/src/matrix.rs:46` |
| `DenseRowMatrix` | `vendor/crypto-primitives/src/matrix.rs:134` |
| `Int` | `vendor/crypto-primitives/src/ring/crypto_bigint_int.rs:28` |
| `Boolean` | `vendor/crypto-primitives/src/semiring/boolean.rs:20` |
| `Uint` | `vendor/crypto-primitives/src/semiring/crypto_bigint_uint.rs:26` |

The 65 aliases are grouped below; the complete names are listed to make the deletion inventory explicit.

### 13 aliases in `vendor/crypto-primitives/src/field/ark_ff_fp.rs`

`Fp64`, `Fp128`, `Fp192`, `Fp256`, `Fp320`, `Fp384`, `Fp448`, `Fp512`, `Fp576`, `Fp640`, `Fp704`, `Fp768`, `Fp832`.

Declarations start at line 730 and end at line 742.

### 26 aliases in `vendor/crypto-primitives/src/field/crypto_bigint_const_monty.rs`

`F64`, `F128`, `F192`, `F256`, `F320`, `F384`, `F448`, `F512`, `F576`, `F640`, `F704`, `F768`, `F832`, `F896`, `F960`, `F1024`, `F1280`, `F1536`, `F1792`, `F2048`, `F3072`, `F4096`, `F6144`, `F8192`, `F16384`, `F32768`.

Declarations start at line 711 and end at line 736.

### 26 aliases in `vendor/crypto-primitives/src/field/crypto_bigint_monty.rs`

`F64`, `F128`, `F192`, `F256`, `F320`, `F384`, `F448`, `F512`, `F576`, `F640`, `F704`, `F768`, `F832`, `F896`, `F960`, `F1024`, `F1280`, `F1536`, `F1792`, `F2048`, `F3072`, `F4096`, `F6144`, `F8192`, `F16384`, `F32768`.

Declarations start at line 571 and end at line 596.

The package also loses `FieldError` and its traits/proc macros. Consumer polynomial and transcript requirements live in F2Z. The unused legacy matrix constructor was removed.

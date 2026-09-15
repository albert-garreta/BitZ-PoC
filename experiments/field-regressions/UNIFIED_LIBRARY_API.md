# Unified arithmetic library: construction inventory

> Superseded planning snapshot. The canonical [build plan](../../vendor/field/PLAN.md) and [API contract](../../vendor/field/API.md) now target `vendor/field`, including static field identities, plain/small representations, and fused conversion. The text below records the earlier proposal; frozen experiment evidence is unchanged.

Status: proposed implementation surface, reconciled with the existing
`docs/unified-arithmetic-plan.md` and benchmark commit `6714cf9`. This is a design
inventory, not an implemented library API. The owned implementation belongs in
`crates/field`; F2Z, `crates/circuit`, and vendored Flock consume it. Independent
comparator libraries retain their native arithmetic.

The current shared crate exports `F128`, `Fq`, `Wide256`, and related helpers.
The scoped prime types, owned integers, general exact accumulators, and common
batch traits below still need construction. Existing implementations should be
moved/adapted where their contracts fit, preserving their optimized kernels.

## 1. Value types

| Public type | Contract |
|---|---|
| `Limb = u64` | Common limb storage. |
| `Bit` | Canonical Boolean value; explicit lifting into other arithmetic. |
| `CtMask` | Opaque all-zero/all-one validity or selection mask. |
| `CtValue<T>` | Defined result plus validity mask for private checked arithmetic. |
| `Uint<const L: usize>` | Unsigned integer in exactly L little-endian limbs. |
| `Z<const L: usize>` | Signed two's-complement integer in exactly L limbs. |
| `UintRef<'a>`, `ZRef<'a>` | Borrowed limbs with a public declared width. |
| `F2` | Characteristic-two field; XOR addition and AND multiplication. |
| `Gf8` | AES polynomial-basis field, one byte. |
| `Gf128` | Flock-compatible polynomial-basis field; preserve 16-byte size/alignment. |
| `B127` | Degree-127 binary field; unused top bit is always zero. |
| `F2Poly<const BITS: usize, const WORDS: usize>` | Packed polynomial over F2; dimensions and padding bits are validated. |
| `Fp<'f, const L: usize>` | Reduced Montgomery residue, branded to one prime-field scope; exactly L limbs of numeric storage. |
| `Residue<'m, const L: usize>` | Branded canonical modular-ring residue; not a prime-field element. |

Integer operations explicitly distinguish wrapping, checked, and widening
arithmetic. A checked bounded integer is not a ring closed under multiplication.
Packed bounded polynomials likewise use widening multiplication. Neither should
be forced into `RingOps`.

`Fp` values contain no context pointer or modulus. Arithmetic that needs the
runtime modulus is called on `FpScope`, not through a context-free `Fp * Fp`
operator. Fixed binary fields can retain ordinary operator implementations that
delegate to their operation structs. Flock may re-export `Gf128` as `F128`.

## 2. Contexts and reusable preparation

| Type | What it owns or borrows |
|---|---|
| `FpCtx<L>` | Accepted public prime and reusable reduction/inversion constants. |
| `FpScope<'f, L>` | Borrow of that context with an invariant field brand. |
| `ModRingCtx<L>` | Public modulus greater than one, including composite/even moduli. |
| `ModRingScope<'m, L>` | Branded operations over the modular ring. |
| `PreparedDivisor<L>` | Public divisor and normalization/reciprocal constants. |
| `PreparedOddInverse<L>` | Fixed-schedule odd-modulus inversion; validity distinguishes nonunits. |
| `PreparedIntegerProjection<C>` | Reusable context/input-width constants for signed/unsigned projection. |
| `PreparedGf128Mul`, `PreparedB127Mul` | Repeated multiplication by a fixed scalar. |
| `FixedBasePow<C, const EXP_LIMBS: usize>` | Fixed-base powers and an explicit exponent capacity. |
| `IntegerOps`, `F2PolyOps` | Zero-size operation structs for widened integer/polynomial arithmetic. |
| `F2Ops`, `Gf8Ops`, `Gf128Ops`, `B127Ops` | Zero-size contexts for fixed binary fields. |

Scoped construction remains:

```rust
impl<const L: usize> FpCtx<L> {
    pub fn with<R>(
        &self,
        body: impl for<'f> FnOnce(FpScope<'f, L>) -> R,
    ) -> R;
}
```

The brand must actually be invariant, with private construction. Compile-fail
tests must reject mixed scopes and escaped elements. Keep the proving operation
and its compact field tables inside one scope; encode only at actual boundaries.
Remove consumer `'static` bounds that would otherwise force materialization.

## 3. Products and accumulators

| Type | Meaning |
|---|---|
| `UintProduct<A, B>` | Exact unsigned A-by-B-limb product; A+B limbs. |
| `ZProduct<A, B>` | Exact signed result of signed/signed or mixed unsigned/signed multiplication. |
| `UintAccumulator<A, B>` | Exact unsigned product sum with an additional headroom limb. |
| `ZAccumulator<A, B>` | Exact signed product sum with an additional sign/headroom limb. |
| `F2PolyProduct<A, B>` | Unreduced carryless product; A and B are input word counts. |
| `Gf8Product`, `Gf128Product`, `B127Product` | Distinct bounded binary-field products. |
| `XorAccumulator<P>` | XOR sum of binary products; no growing carry bound. |
| `MontyR`, `MontyR2` | Zero-size markers for Montgomery scaling. |
| `ScaledProduct<'f, P, Scale>` | Opaque product with field identity and scale. |
| `ScaledAccumulator<'f, A, Scale>` | Opaque accumulator with field identity and scale. |

Use split arrays rather than unstable generic expressions such as
`[u64; A + B]`. Product/accumulator fields and constructors are private.

The prime-field aliases are:

```text
FpProduct<'f, L>                  = ScaledProduct<'f, UintProduct<L,L>, MontyR2>
FpProductAcc<'f, L>               = ScaledAccumulator<'f, UintAccumulator<L,L>, MontyR2>
FpLinearProduct<'f, L, N>         = ScaledProduct<'f, UintProduct<L,N>, MontyR>
FpLinearAcc<'f, L, N>             = ScaledAccumulator<'f, UintAccumulator<L,N>, MontyR>
FpSignedLinearProduct<'f, L, N>   = ScaledProduct<'f, ZProduct<L,N>, MontyR>
FpSignedLinearAcc<'f, L, N>       = ScaledAccumulator<'f, ZAccumulator<L,N>, MontyR>
```

Field×field produces scale R²; reduction removes one Montgomery factor to
return a field element at scale R. Field×integer is already at scale R, so its
reducer preserves that scale. These are distinct types under one arithmetic
interface, not selectable prover strategies.

A full-width Montgomery residue is **unsigned**, even when its high bit is set.
Signed coefficient multiplication therefore needs an unsigned×signed kernel;
reinterpreting the residue as `Z<L>` would be wrong for full-width primes.

Batch construction establishes the complete product-count bound once. Fused
kernels count every product and accumulator merge, not just outer iterations.
Do not expose unrestricted integer/prime accumulator merging. Binary XOR
accumulators can merge freely. Checked extraction handles exact integer
narrowing; it is not an infallible `Reduce` implementation.

## 4. Traits and required implementations

The five core arithmetic traits retain the main plan's signatures:

```rust
pub trait RingOps {
    type Elem: Copy + Send + Sync + CtEq + CtSelect;

    fn zero(&self) -> Self::Elem;
    fn one(&self) -> Self::Elem;
    fn add(&self, a: &Self::Elem, b: &Self::Elem) -> Self::Elem;
    fn sub(&self, a: &Self::Elem, b: &Self::Elem) -> Self::Elem;
    fn neg(&self, a: &Self::Elem) -> Self::Elem;
    fn mul(&self, a: &Self::Elem, b: &Self::Elem) -> Self::Elem;
}

pub trait FieldOps: RingOps {
    fn square(&self, a: &Self::Elem) -> Self::Elem;
    fn inverse_ct(&self, a: &Self::Elem) -> CtValue<Self::Elem>;
    fn pow_ct<const E: usize>(
        &self, base: &Self::Elem, exponent: &Uint<E>,
    ) -> Self::Elem;
}

pub trait WideMul<Lhs, Rhs = Lhs> {
    type Product;
    fn mul_wide(&self, lhs: &Lhs, rhs: &Rhs) -> Self::Product;
}

pub trait BatchMulAcc<Lhs, Rhs = Lhs> {
    type Accumulator;
    fn batch_mul_acc(&self, lhs: &[Lhs], rhs: &[Rhs]) -> Self::Accumulator;
    fn batch_mul_acc_map(
        &self, len: usize, term: impl FnMut(usize) -> (Lhs, Rhs),
    ) -> Self::Accumulator;
}

pub trait Reduce<Input> {
    type Output;
    fn reduce(&self, input: Input) -> Self::Output;
}
```

The implementation matrix is:

| Trait | Operations | Implemented by |
|---|---|---|
| `RingOps` | zero, one, add, sub, neg, mul | All fixed-field operation structs, `FpScope`, `ModRingScope`. |
| `FieldOps: RingOps` | square, masked inverse, exponentiation | Fixed-field operation structs and `FpScope`; not general modular rings. |
| `WideMul<Lhs, Rhs>` | One typed unreduced/exact product | `IntegerOps`, `F2PolyOps`, binary-field operation structs, `FpScope`. |
| `BatchMulAcc<Lhs, Rhs>` | Slice dot accumulation and indexed/generated-term accumulation | Those same operation structs with their own typed accumulators. |
| `Reduce<Input>` | Typed polynomial/modular reduction | Binary-field operation structs, `FpScope`, and modular-ring contexts where needed. |
| `CtEq`, `CtSelect` | Masked equality/zero testing and selection | Scalar values and required products/accumulators. |
| `CtOrd` | Signed/unsigned fixed-schedule comparison | Integer values and views where needed. |
| `CheckedArithmetic` | Masked same-width add/sub/mul/div validity | `Uint<L>` and `Z<L>`. |
| `IntegerEmbedding<T>` | Explicit signed/unsigned integer projection | `FpScope`, `ModRingScope`; bit lifting uses an explicitly defined integer map. |
| `FieldEmbedding<Source>` | True field embedding | F2 into binary fields; AES-basis GF8 into GF128. |
| `CanonicalCodec<T>` | Canonical encode, public decode, masked private decode | Appropriate integer/fixed-field/scoped contexts. |
| `BatchFieldOps` | dot, elementwise mul, scaled add, pair folds, masked batch inverse | Fixed-field and prime-field operation contexts. |
| `SumcheckKernels` | Fused arithmetic round kernels | Concrete fixed-field and prime contexts, with generic default method bodies where no specialization exists. |
| `PublicRandomSource` | Public random byte stream | Consumer adapters for accepted RNG/transcript sources. |

Concrete implementation families, with method bodies omitted:

```rust
impl<const A: usize, const B: usize> WideMul<Uint<A>, Uint<B>> for IntegerOps { /* UintProduct<A,B> */ }
impl<const A: usize, const B: usize> WideMul<Z<A>, Z<B>> for IntegerOps { /* ZProduct<A,B> */ }
impl<const A: usize, const B: usize> WideMul<Uint<A>, Z<B>> for IntegerOps { /* ZProduct<A,B> */ }
// Also implement the reversed signed/unsigned operand order.
// Corresponding BatchMulAcc impls return the matching exact accumulator.

impl WideMul<Gf128> for Gf128Ops { /* Gf128Product */ }
impl BatchMulAcc<Gf128> for Gf128Ops { /* XorAccumulator<Gf128Product> */ }
impl Reduce<Gf128Product> for Gf128Ops { /* Gf128 */ }
impl Reduce<XorAccumulator<Gf128Product>> for Gf128Ops { /* Gf128 */ }

impl<'f, const L: usize> RingOps for FpScope<'f, L> { /* Elem = Fp<'f,L> */ }
impl<'f, const L: usize> FieldOps for FpScope<'f, L> { /* ... */ }
impl<'f, const L: usize> BatchMulAcc<Fp<'f,L>> for FpScope<'f,L> { /* FpProductAcc */ }
impl<'f, const L: usize, const N: usize> BatchMulAcc<Fp<'f,L>, Uint<N>> for FpScope<'f,L> { /* FpLinearAcc */ }
impl<'f, const L: usize, const N: usize> BatchMulAcc<Fp<'f,L>, Z<N>> for FpScope<'f,L> { /* FpSignedLinearAcc */ }
// Native u64 and Bit operands also get direct linear implementations.
// WideMul and Reduce cover each corresponding product and accumulator type.
```

Calls remain monomorphized. Use concrete trait implementations with default
methods and explicit overrides; a blanket implementation for every field must
not prevent specialized implementations on stable Rust.

`BatchMulAcc` checks public shape/capacity once and controls its iteration count.
Its indexed form generates sparse matrix terms without temporary operand arrays.
Private `KernelMac` performs infallible per-term updates; there is no hot-loop
`try_mac`, context-identity comparison, or allocation.

The unconditional-return signatures above have documented public shape and
capacity preconditions, checked with assertions at the batch boundary. Consumer
preparation that needs structured failures returns `ShapeError` before calling
them. These assertions do not inspect private values.

## 5. Prepared integer batches: refinement from the experiments

Add a shared prepared/bounded product capability corresponding to the tested
`PreparedProducts` API. Proposed general names are:

| Type | Responsibility |
|---|---|
| `PublicProductBounds` | Caller-declared active limb bounds for the two operands. |
| `PreparedProducts<'a, const A: usize, const B: usize>` | Validated immutable input borrows, exclusive full-width output storage, and a public kernel choice. |
| `ProductBatchError` | Public preparation failures; detailed bound errors are only for explicitly public inputs. |

Unlike the experiment's fixed `FourLimbs`/`NineLimbs` enum, the final interface
must also represent asymmetric products and larger RSA/MultiSwap widths. Exact
names/layout are a refinement of the earlier plan, not a claim that this generic
API has been implemented or benchmarked.

Preparation chooses from public bounds; it must never trim private values to
discover a faster kernel. Preserve a validated borrow so values cannot change
under that bound. `execute` is infallible after preparation, writes all output
limbs, and allocates nothing. Keep a convenience one-shot operation whose
validation cost is visible and measured.

For private inputs, scan all declared limbs in a fixed schedule and accumulate
bound validity with `CtMask`. Do not expose a per-row or per-batch bound error.
Executable validated batches are made available after the consumer's complete
validation reaches its single generic invalid-witness boundary. The experimental
constructor's ordinary bound error is therefore not the final private-witness
validation interface. Public shape errors can still use ordinary `Result`.

The seeded two-limb benchmark computes a sum **modulo 2^128**. Expose it through
an explicitly named wrapping operation, or use it internally after preparation
proves the exact result fits. It cannot implement arbitrary exact
`ZAccumulator<2,2>`, which needs five limbs. The exact widened path needs its own
correctness tests and performance measurements.

## 6. Fused kernels and architecture implementations

`BatchFieldOps` must preserve both F2Z's in-place interleaved pair folds and
Flock's out-of-place/offset folds. `SumcheckKernels` must preserve:

- Single-pair and two-pair weighted round coefficients, returning `[Elem; 3]`.
- In-place folding and fused fold-plus-round passes.
- Double-fold 3×3 grid arithmetic, returning `[Elem; 9]`.
- Intermediate field reduction where a weighted product is multiplied again.

Keep NTT plans, scheduling, matrix structure, and proof types in the consumers.
They call shared scalar, prepared, and batch kernels. Replacing specialized
passes with a generic sequence of scalar operations is not the migration goal.

Internal modules own portable, ARM PMULL, and x86 PCLMUL implementations, plus
GFNI/VPCLMUL/AVX-512 variants where available and measured. Gate all required CPU
features. Scalar multiply, wide MAC, prepared multiplication, and transforms
have separate performance decisions; a wider instruction set is not itself a
reason to select a kernel. The shared crate still needs its x86 GF implementation.

Polynomial-bit construction, integer lifting, and field embedding are distinct:
GF128 bits `2` denote polynomial X, whereas the integer 2 maps to zero in a
characteristic-two field. Preserve the actual GF8 embedding and canonical
transcript encodings; never serialize Montgomery residues directly.

## 7. Setup, errors, and private support

Public setup types are `PrimePolicy`, `ProbablePrime<L>`, and `PrimeSearchError`.
Keep `ArithmeticError`, `ContextError`, `ShapeError`, and `DecodeError` for public
boundary failures. Private arithmetic uses masked validity rather than detailed
value-dependent errors. `ProductBatchError` is the prepared-product-specific
boundary error proposed above; common shape cases can reuse `ShapeError`.

Private support consists of `Brand<'id>`, `MontgomeryParams<L>`,
`BarrettParams<L>`, `MillerRabinCtx<L>`, and `KernelMac`. Architecture vector types
stay private. Prime selection remains runtime setup work; known primes use
validated constants, not compile-time Miller–Rabin. Reuse preparation after a
candidate is accepted.

## 8. Consumer changes required to use the library

These types stay outside `crates/field`:

| Layer | Required types/traits |
|---|---|
| Circuit coefficients and expressions | `CoefficientId`, `CoefficientPool`, `CoefficientSpan`, `ZWire`, `BoolWire`, `ZExpr<L>`, `BoolExpr`. |
| Circuit bounds/storage | `IntBounds<L>`, `RowBounds`, `BoundsCertificate`, `ProductSlot`, `ProductLayout`, fixed-arena `IntegerProducts`. |
| Prepared execution | `PreparedCircuit<P>`, `WitnessExecutor<'p,P>`, `ValidatedWitness<'p,P>`, `PrepareError`, `WitnessError`, `HintOutput<T> = CtValue<T>`. |
| Circuit traits | `CircuitProgram`, revised `Circuit`, `WitnessContext`, `BoolWitness`, `BoolRepresentation`. |
| F2Z projection and validated input | `ProjectedRelation<'f,C>`, `PolynomialProjection<C,N>`, `ValidatedProverInput<'p,P,W>`. |
| F2Z transcript/failure boundary | `PublicTranscriptSink`, `PublicTranscriptPrefix`, `ProveError`, `ProveFailure`. |

Retain packed witnesses, materialized/Wengert matrix backends, polynomial/MLE
types, sparse matrices, and direct/virtualized opening types. Adapt their
arithmetic bounds and preserve borrowed native witnesses and compact tables.

Retire `SpartanReductionStrategy`, backend-specific prover/reducer wrappers, and
the now-empty strategy options. Production has one delayed-reduction path, with
Barrett/Montgomery details inside shared contexts. No one-variant strategy enum
is needed. Remove `vendor/crypto-primitives` and migrated production bigint
dependencies after callers move; independent test oracles remain available.

## 9. Construction order and acceptance

1. Scalar storage, masks, canonical codecs, and portable integer/binary kernels.
2. Prime/ring contexts, invariant brands, typed products, scales, and reducers.
3. Batch MAC, prepared public-width products, and native scalar/linear paths.
4. Preserve/import the specialized GF, folding, sumcheck, and prepared kernels.
5. Migrate F2Z, circuit, and Flock callers; consolidate delayed reduction.
6. Remove duplicate types/dependencies after correctness and performance gates.

Compile-fail tests cover mixed brands/scales and scope escape; layout tests cover
compact values; differential tests cover full-width primes, signed/unsigned
products, tails, carries, canonical decoding, and rejected bounds. Test exact
and wrapping paths independently. Measure scalar and fused consumers on ARM and
x86, including preparation, allocations, and full proofs. The passing ARM
prototype campaign establishes only its measured contracts and workloads.

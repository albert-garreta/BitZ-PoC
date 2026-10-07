# Falcon public power-basis weights

`vendor/field/src/q12289.rs` owns the fixed-basis F_12289 extension arithmetic
for degrees 9, 10, and 11, and the prepared `PowerBasis<K>` kernel. Both full
Falcon and algebraic Falcon use this field backend. Algebraic Falcon uses the
new recurrence for its ring weights; full Falcon retains its fixed-basis tensor
lift. Witness polynomial multiplication retains the existing exact integer
implementation; an NTT replacement is a separate change.

The algebraic frontend now supports both Falcon-512 and Falcon-1024. Each uses
extension degree 11, signed 15-bit coefficients in 16-bit lanes, and the existing
one-source PCS. Falcon-512 uses its norm bound 34,034,726 and 26 slack bits;
Falcon-1024 uses 70,265,242 and 27 slack bits. Source strides are 16,384 and
32,768 bits respectively. The minimum source domain remains 2^19 bits.

## Basis preparation and weight generation

For E = F_12289[theta]/(theta^11 + theta + 14), the quotient certificate is fixed
before sampling alpha outside F_12289. Since 11 is prime, alpha generates E.
Construct M with columns 1, alpha, ..., alpha^10 in the fixed theta basis.
Gaussian elimination computes M^-1 once per proof, and

```
r = M^-1 * [alpha^11]_theta.
```

`PowerBasis::encode` maps fixed-basis elements through M^-1. `advance` implements

```
u'[0] = r[0] * u[10] mod q
u'[k] = (u[k-1] + r[k] * u[10]) mod q, k = 1..10.
```

For signature i, convert only lambda_i and lambda_i h_i(alpha). Fill both
length-N weight arrays using the recurrence and retain their alpha-basis
coordinates. Public polynomial evaluations still use the existing cached-power
dot products. Convert the resulting target T into the same alpha basis.

The distinct `PowerCoordinates<K>` type prevents passing these vectors to
fixed-basis extension multiplication. It does not distinguish two runtime
alpha contexts of the same degree: callers must retain and use the context
that created their vectors. Setup uses public-data pivot selection and is not
intended to conceal a secret generator. The generic backend rejects base-field
and proper-subfield generators, including non-base elements in degrees 9/10.

## Integer lift and transcript

For every coordinate k, accumulate exact signed integers

```
S[k] = sum_{i,j} (A[i,j,k] * s1[i,j] + B[i,j,k] * s2[i,j])
carry[k] = (S[k] - T[k]) / 12289.
```

All A, B, and T coordinates are canonical integers in 0..12288. Reduction is
local to field dot products and recurrence multiply-adds; S is not reduced.
Carries are checked and absorbed before the prime-field projection challenge.
The same coordinates are used for the verifier's weights and target.

A conversion dot product is bounded by 11*12288^2 = 1,660,944,384; an advance
multiply-add by 12288 + 12288^2 = 151,007,232. Conversion uses u64 accumulators,
while advance uses u32. At the supported batch/degree limits, exact sums fit
i64. The existing carry and no-wrap bounds remain valid because the dimension
and canonical coordinate range are unchanged.

The algebraic protocol is version 3 and explicitly binds the alpha-power
coordinate convention. Its version-2 proofs are incompatible, although the
message schemas are unchanged. Serialized PCS opening sizes can vary with
transcript-dependent Merkle multiproof overlap. The certificate stays encoded
in the fixed basis. No additional commitments or proof fields are introduced.
Full Falcon's transcript and proof interpretation remain unchanged.

The recurrence replaces about K^2 coefficient products per general extension
multiplication with K multiply-adds after basis setup and start conversion.
This applies to weight generation, not to the whole prover. Packing, commitments,
norms, exact carry accumulation, projection, sumcheck, and PCS openings remain.

## Validation and measurements

The field tests compare each of 1,024 successive weights against ordinary
extension multiplication for all registered degrees, multiple generators, zero,
one, and maximal coordinates. They also check irreducibility against an
independent polynomial implementation and reject singular bases. Ring tests
compare full weight arrays, targets, and carries against a fixed-basis reference.
The shared algebraic tests exercise both signature degrees, both security targets,
non-power-of-two batches, malformed messages, tampering, and norm boundaries.

See [the benchmark report](RESULTS.md) for
baselines, optimized results, binary/source identities, commands, and raw logs.
`vendor/field/examples/q12289_power_basis.rs` isolates the public weight kernel;
it includes setup and starting-value conversions in the optimized timing.

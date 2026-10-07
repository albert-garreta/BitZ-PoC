> Historical notes from before the SharedPrime-only cleanup. These APIs,
> domains, and measurements do not describe the current implementation.

# Shared-prime Falcon V2 (historical)

Superseded by [SharedPrimeV4](SHARED_PRIME_V4.md). The measurements and
protocol details below describe this historical revision.

This describes runtime revision `4fe1d3d5`. The current API selects
[SharedPrimeV3](SHARED_PRIME_V3.md), with a larger prime and two limbs at
128 bits. V2 binaries and qualification records remain available for comparison.

`PreparedFalconHybrid::new_shared_prime(batch, target_bits)` selects the
non-ZK `SharedPrimeV2` protocol for 1–1024 signatures and targets 100 or 128.
`PreparedFalconHybrid::new` remains the native-coordinate-carry default.
V2 changes the shared proof format and transcript; old shared-prime proofs
must be regenerated. The native proof format and transcript are retained.

This document describes the implemented V2 configuration. Correctness tests
pass on Mac and will. Performance qualification failed: 100-bit discovery
exceeded the strict peak-memory gate, and 128-bit diagnostics showed a large
grinding regression. The remaining matrix and confirmation runs were skipped
under the requested regression policy. See the
[qualification report](../../../../../results/falcon-shared-prime-v2-20261006/REPORT.txt)
for evidence and scope; native remains the default.

## Ring reduction and shared binding

1. Commit the same arithmetic and two Keccak sources. The arithmetic source
   contains `C`, unsigned public-key coefficients `H`, biased bounded `S1`,
   and `S2` through its existing signed signature-bit encoding. It retains
   114,914 live bits in a 131,072-bit slot per signature.
2. Keep the original extension-field certificate and outer sumcheck. In
   `E = F_12289[theta]/(theta^11 + theta + 14)`, the degree-1022 quotient
   `D(Y)` encodes `e(Y) = (Y^1024 + 1)D(Y)`. The certificate is fixed before
   the polynomial point `beta`; the cubic signature sumcheck fixes the four
   operand evaluations before their batching challenge.
3. Apply the affine decoders directly to those batched evaluations. The
   position weights are `beta^j`; there is no coefficient-domain inner
   sumcheck. In transcript order `C,H,S2,S1`, operand weights are
   `1,lambda,lambda^3,lambda^2`. The signature weights are `eq(t,s)`, zeroed
   for padded signatures. The S1 offset is
   `-6144 * omega_S1 * sum_s(row_s) * sum_j(beta^j)`.
   The geometric sum uses no division, including at `beta = 0, 1, -1`.
   Repeated decoder slots are added in `E` before choosing canonical integer
   coordinates; the signed S2 byte permutation is unchanged.
4. Send the 21 coefficients of the unreduced integer polynomial `P(T)`.
   Check their magnitude and their read-off in `E`, then sample the shared
   prime and the fresh prime-field projection point. Canonical coordinates
   are projected individually; projection does not commute with reduction
   modulo 12289.
5. Run the existing optimized integer norm, HashToPoint rejection, ordered
   compaction, and linear proofs in that same prime. Preserve their blocking,
   compact round messages, and terminal claims. Merge those claims with the
   projected ring tensor in the existing factored/streaming source binder.
   Its terminal source query continues through the unsplit bridge below and
   the existing joint binary sumcheck and shared PCS opening.

## Integer bounds and the unsplit bridge

For padded capacity `S`, the arithmetic source has `L = 2^17 * S` bits and
the bridge uses `R = 8192` rows and `C = L/R` columns. The prime family is

```text
2^114 <= p <= 2^115 - 2^102 - 1.
```

The upper cap satisfies BitZ's conservative `(R+1)(p-1) < 2^128-1` gate.
At the maximum batch, `L = 2^27` and `C = 16384`. With extension degree 11,

```text
H_P   = 11 * L * 12288^2 = 222,928,181,554,839,552
2 H_P =                    445,856,363,109,679,104 < 2^59
H_src =                         43,013,625,445 < 2^36.
```

Thus every sampled prime exceeds `max(2 H_P, H_src)`. The residual bound
covers all decoder-allowed assignments, not only valid signatures.

Lift each terminal row weight canonically to `0 <= gamma_i < p` and send one
integer sum `eta_j = sum_i gamma_i * b_ij` per column. Check the actual sum
bound with checked arithmetic and enforce `eta_j <= sum_i gamma_i < M`,
where `M = 2^128-1`. Check the prime-field read-off of these sums. A generator
of exact order `M` then gives an injective exponent encoding on the allowed
interval. One product forest proves

```text
g^eta_j = product_i [1 + (g^gamma_i - 1) * b_ij].
```

The original bits are packed once, without a limb coordinate. Thirteen GKR
layers reduce this forest to a binary linear claim on the original row and
column slots. In characteristic two its row coefficient is
`eq(z_row,i) * (g^gamma_i + 1)` and its claimed value is `leaf_value + 1`.
The shared PCS authenticates that claim together with the other sources.

## Security and cost accounting

V2 retains the repository's computational grinding model: a raw challenge
error `e` with difficulty `g` contributes `e / 2^g` per unit of adversarial
work. This is not unconditional statistical soundness. Moving the prime
minimum from `2^125` to `2^114` multiplies a fixed prime-field error bound by
`2^11`. At the 128-bit target, V2 adds 11 bits to each retained
prime-challenge difficulty, including projection: its difficulty becomes 25
instead of 14. At the 100-bit target, integer-prime challenges retain zero
grinding and projection retains difficulty 2. That profile spends part of
the preceding protocol's security margin rather than preserving every old
prime-error bound. Binary-field and PCS schedules retain their own budgets.
Exact-integer tests verify the complete configured ledger for every batch
from 1 through 1024, including prime sampling and every remaining reduction.
The conservative reported minima are about 101.002291 bits for the 100-bit
profile and 129.070893 bits for the 128-bit profile under this model.

At the 128-bit target, an extra 11 grinding bits means 2048 times the
expected nonce-search work per affected challenge block. It is not a prediction for total prover time;
the smaller forest must be measured against that additional work.

Removing the ten quadratic inner rounds and their final extension-field
evaluation saves exactly `(10*3 + 1)*22 = 682` stored bytes. At batch 1024,
one rather than two integer sum vectors saves exactly
`16384*16 = 262144` bytes (256 KiB). These are structural component savings,
not a prediction of the total proof-size difference. Other messages and
transcript-dependent PCS encodings must be counted in the resulting proof.

The native path keeps its existing two-limb bridge. V2 has separate shared
bridge binding, root-query, and grinding domains. The higher-level shared
protocol and statement domains are `shared-prime/non-zk/v2` and
`shared-prime/statement/v2`. Retained subprotocol labels have their own
versions and do not imply old shared-proof compatibility.

# Shared-prime Falcon V3 (historical)

Superseded by [SharedPrimeV4](SHARED_PRIME_V4.md). The measurements and
protocol details below describe this historical revision.

`PreparedFalconHybrid::new_shared_prime(batch, target_bits)` prepares the
experimental `SharedPrimeV3` protocol for 1–1024 signatures. The native backend
remains the default. V3 proofs are domain-separated from V1 and V2; old shared
proofs require regeneration. Native transcript and payload behavior are retained.

## Target-selected parameters

| Target | Shared prime family (inclusive) | Column sums | Projection difficulty |
| --- | --- | --- | ---: |
| 100 | `2^114 ..= 2^115 - 2^102 - 1` | One unsplit `u128` | 2 |
| 128 | `2^125 ..= 2^126 - 1` | Two bounded `u128` limbs | 14 |

The 128-bit arithmetic schedule equals the native schedule. V2's additional
11 grinding bits are removed because the prime floor is restored. At 100 bits,
integer arithmetic still has zero grinding. Both profiles retain the binary and PCS budget policy, with bridge difficulty
recomputed from its selected geometry; they are not unground 128-bit proofs.

The prepared protocol and target select the bridge mode. Neither prover nor
verifier infers it from proof-vector lengths or the received modulus. The
statement digest binds the prime interval, bridge mode, dimensions, target
and grinding budgets. The ring header independently binds its selected interval.

## Reduction and authentication

The ring path retains the ideal quotient and cubic outer sumcheck over
`E = F_12289[theta]/(theta^11 + theta + 14)`. Fixed endpoint answers are batched
in storage order `C,H,S2,S1` with weights `1,chi,chi^3,chi^2`. Direct decoder
expansion uses `beta^j`, signed digit weights, the live-row mask and the S1
affine offset. There is no extension-field coefficient inner sumcheck.

The grouped lift sends the 21 coefficients of `P(T)` and checks their bounds
and projection into `E`. Only after `P` is fixed does the transcript select
the one shared prime, followed by projection grinding and the challenge
`alpha`. The projected ring claim and existing optimized integer proofs use
that same field. Their source claims join in the streaming prime-field binder,
then pass through the selected bridge and final shared PCS opening. All
witness-dependent endpoints still authenticate the same original source bits.

## Bridge bounds

Both profiles retain 8192 rows. The 100-bit cap satisfies the conservative
`(R+1)(p-1) < 2^128-1` guard. The verifier checks each unsplit sum against the
actual public bound `H = sum_i gamma_i < 2^128-1` before prime reconstruction.

At 128 bits, canonical weights satisfy `gamma_i < p < 2^126`. Split

```text
gamma_i = gamma_0_i + 2^113 * gamma_1_i.
eta_l_k = sum_i gamma_l_i * b_i_k.
```

The worst-case bounds are

```text
eta_0_k <= 2^126 - 8192
eta_1_k <= 2^26 - 8192 = 67,100,672.
```

The verifier enforces the tighter actual bound for each limb and checks
`sum_k c_k * (eta_0_k + 2^113 * eta_1_k) = nu (mod p)`. Products multiply
only the original rows; the limb coordinate survives in the jointly proved
forest. Its terminal coefficients accumulate back onto the same source slots.

The proof container continues to store each sum as `u128`. At batch 1024,
the sum component is 256 KiB at target 100 and 512 KiB at target 128; forest
messages are 273 and 286 respectively. A narrower high-sum codec is a separate
change. These component sizes do not predict the complete transcript-dependent
PCS payload.

## Security checks and qualification

Both prime families exceed `max(2 H_P, H_src)` for every supported layout.
At batch 1024, `2 H_P = 445,856,363,109,679,104` and
`H_src = 43,013,625,445`. The degree-20 projection and degree-three endpoint
batching terms remain; the removed ring inner sumcheck stays absent.

Exact-integer tests recertify all 2048 batch/target combinations. The
conservative complete-ledger minima are approximately 101.002291 and
129.070894 bits. These use the repository's existing work-normalized grinding
model, not unconditional statistical security. Prime sampling failure still
rejects, and the accepted-composite error remains included in the ledger.

The source also tests mode-dependent GKR error/message counts, padded two-limb
proofs, malformed vector counts, target/field mismatch, transcript agreement,
source authentication and altered proof messages. Both machines pass 124
Falcon tests. The five-seed 128-bit/batch-1024 discovery comparison recovers the V2 grinding slowdown, but strict RSS and V1
payload gates still fail; native proving-time parity remains inconclusive.
The native backend therefore remains the default. See the
[V3 qualification report](../../../../results/falcon-shared-prime-v3-20261006/REPORT.txt)
for exact timings, proof sizes, and limits of the measurements. The 100-bit
experiment still needs performance qualification.

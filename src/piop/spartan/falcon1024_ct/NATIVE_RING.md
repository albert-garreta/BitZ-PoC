# Native-ring batch Falcon prover

`PreparedFalconHybrid` uses one native-ring arithmetic path. Its statement domain
is `bitz/falcon1024-ct/hybrid/native-ring/non-zk/v3`; earlier hybrid proofs are
incompatible. There is no arithmetic-mode selector. The standalone nonhybrid
backend remains a separate existing implementation.

The public statement still contains each public key, 32-byte message, and exact
CT signature. The prover is not zero knowledge. Binary Keccak, SHAKE wiring,
three source commitments, the jointly batched two-limb bridge, and the UDR shared
PCS opening retain their existing roles.

## Source representation and counts

For N=1024, M=1311, Q=12289, decode remainders and biased s1 using
`bounded14(b) = sum(k=0..12, 2^k*b[k]) + 4097*b[13]`.
Every bit string decodes into [0,12288]; s1 is the decoded value minus6144.
The honest encoder sets the top bit exactly when the value exceeds8191.
Representations may overlap, but every source map uses the same decoder.

| Per live signature | Previous hybrid | Native ring |
| --- | ---: | ---: |
| Auxiliary arithmetic values | 18232 | 8605 |
| Auxiliary committed bits | 186062 | 87705 |
| Public/constant copy bits | 12873 | 12873 |
| Live arithmetic bits | 198935 | 100578 |
| Arithmetic allocation per capacity slot | 262144 | 131072 |
| Linear rows, live / padded | 10152 / 16384 | 4458 / 8192 |
| Rejection/product outer rows, live / padded | 5244 / 8192 | 1311 / 2048 |
| Additional cubic leaf rows, live / padded | — | 1311 / 2048 |
| Total live arithmetic and binary bits | 1030955 | 932598 |

Auxiliary columns are w(1311*16), q(1311*3), r(1311*14), d(1311),
P(1312*11), c(1024*14), v(1024*14), and T(27).
There are no committed range slacks, selector/product columns e/U/V, or ring
quotient K. Norm and forest reductions are additional obligations, not included
in the scalar-row counts. The 832020 live Keccak bits per signature are unchanged.

For capacity>=2, physical source allocation is1441792*capacity bits; singleton
allocation is1703936 bits because the four-permutation Keccak slab has minimum
capacity2. The shared virtual binary domain remains2097152*capacity bits.

## Proof sequence

1. Generate SHAKE, HashToPoint, centered s1, and nonnegative27-bit norm slack.
   Cache the negative high coefficients of the full H*S2 product moduloQ.
2. Commit the arithmetic and two Keccak sources; bind statement, geometry,
   coefficient decoder, native field, certificate/carry bounds, and schedules.
3. Use existing signature equality weights and synchronized quadratic norm
   sumchecks. Division, prefixes, and public byte constraints remain linear.
4. Prove q_bit2*q_bit0=d with one rejection outer sumcheck across the batch.
5. Sample compaction fingerprints. Retain all2B product trees and each
   signature's own root equality. Authenticate nonlinear candidate leaves using
   a new cubic sumcheck; all operands are zero on unused candidates/instances.
6. Prove the native ideal claim and its bounded coordinate lift below.
7. Batch norm, rejection, leaf, output-tree, native-ring, and linear source
   claims into one arithmetic binder. Retain indexed word templates and fused
   packed sumcheck processing. All bounded14 maps use4097 for the top bit.
8. Use the existing two-limb bridge, Keccak prefixes, SHAKE wiring, joint binary
   sumcheck, and one shared PCS opening. Falcon retains MATCHED_UDR and no initial
   OOD message.

Steps5 and6 are independent reductions. The implementation completes the
nonlinear PIOP before the native reduction, then batches all resulting claims.

## Native ideal check and source authentication

Use E=F_12289[theta]/(theta^11+theta+14). The defining polynomial is irreducible,
checked by a Rabin test. For R_s=C_s-H_s*S2_s-S1_s, sample extension equality
weights lambda_s and send one polynomial D of degree<=1022 satisfying

`sum_s lambda_s R_s(X) = (X^1024+1) D(X)`.

Absorb D before sampling alpha generating E/F_Q. This yields the source claim
`sum_(s,j) a_sj*(c_sj-s1_sj)=t`, where a_sj=lambda_s*alpha^j and
`t=(alpha^1024+1)D(alpha)+sum_s lambda_s H_s(alpha)S2_s(alpha)`.

Take canonical base-field coordinates a_bar,t_bar in[0,Q-1]. Send11 signed
integer carries h_k and check

`abs(h_k)<=22528*B*N`,
`p>(2Q-1)*22528*B*N+(Q-1)`.

After absorbing carries and the carry-grinding nonce, sample xi in F_p and form

`W_sj=sum_k xi^k*a_bar[k,s,j]`,
`t_ring=sum_k xi^k*(t_bar[k]+Q*h_k)`.

The existing binder authenticates `sum W_sj*(c_sj-s1_sj)=t_ring` against the
same source bits as the norms and HashToPoint. The intrinsic source bound is
`c-s1 in[-6144,22527]`, so the checked carry bound prevents wrap in F_p.
This is an integer congruence adapter, not a homomorphism between unrelated
finite fields. D and the11 carries are proof messages, not source columns.

## Security and measurement

Native batching/projection contributes at most
`(log2(capacity)+2046)/(12289^11-12289)`.
Carry collapse contributes `10/p` before grinding. At target128, its independent
12-bit grinding block gives work-normalized contribution below2^-133.
The security report also includes leaf instance batching and cubic rounds;
all configured batch sizes must meet the target in the complete union bound.
These are the repository's computational grinding bounds, not statistical128-bit
soundness. Every carry and certificate precedes the challenge testing it.

The v3 schedule uses equality-weight forest batching and a nonzero-vector
line bound. The fingerprint uses a fixed incorrect signature argument. Exact
regressions preserve the complete v2 bound for every supported live batch.
At batch1024, cubic, forest, and fingerprint difficulties are18,17,19 bits;
partial batches use20 fingerprint bits. See the
[security ledger](OPTIMIZATION_SECURITY.md) and
[compaction argument](COMPACTION_SOUNDNESS.md).

Run the existing benchmark at batch1024, security128, seed42, threads16,
one warmup and three measured trials with `-C target-cpu=native`. Compare total
prover time including witnesses, commitments, all native work, SHAKE, and the
shared opening. The historical matched baseline is3.160ms/signature.
Native projection/certificate/carry spans are reported separately. New proof
transcripts are not expected to match historical Debug digests.

## Initial native-ring measurement (v1)

The matched native-release benchmark measured **2.908 ms/signature** versus
**3.268 ms/signature** for the saved baseline rerun: **11.0% less proving time**.
This is also below the earlier 3.160 ms target. Batch verification measured
0.112 seconds versus 2.687 seconds. All matched proofs verified.

The full library suite passed 645 tests (seven ignored). Native release proofs
also verified for batches 3, 32, and 1024. The stored proof payload at batch1024
is 2,444,036 bytes, excluding Falcon transport framing and the public statement;
there is still no Falcon wire codec.

See [the complete benchmark report](../../../../bench_results/falcon-native-ring-20260926/results.md)
for timing boundaries, memory, payload accounting, stage profiles, and validation.

## Bottleneck optimizations (v2)

The v2 matched benchmark measured **2.282 ms/signature** versus **2.818** for
the saved v1 executable rerun: **19.0% less proving time**, or 438 signatures/s.
Batch verification decreases from 110.038 to 71.847 ms. The same 1024 distinct
signatures, seed 42, 16 threads, native release, and 128-bit target were used;
all 32 timing proofs across batches 1, 3, 32, 1024 and both diagnostic proofs verified.

The implementation splits grinding under an exact non-increasing error budget,
caches binary wiring marginals and eight-bit binder transforms, parallelizes
public binding targets, and combines PCS padding updates. Witness and constraint
counts are unchanged. The complete work-normalized security report improves
from 129.359618 to 129.365218 bits at batch 1024. These remain computational bounds
under the existing model described in [OPTIMIZATION_SECURITY.md](OPTIMIZATION_SECURITY.md).

The full library suite passed 651 tests (seven ignored), with three upstream
Falcon integration tests and the serial build check also passing. The stored
batch proof payload is 2,440,260 bytes. See [the v2 benchmark report](../../../../bench_results/falcon-bottlenecks-20260926/results.md)
for phase timings, small batches, nonce-search variability, and validation details.

## Throughput optimizations (v3)

All six planned changes are implemented. At batch1024, seed42, threads16 and
target128, the matched rerun improves from **2.311 to1.169ms/signature**
(**856 signatures/second**,49.4% less proving time). Across seeds42,43,44,
the optimized medians are1.169,1.223,1.230ms/signature; the median of those
is1.223ms/signature (818 signatures/second). The1ms target is not yet reached.

Witness and constraint counts are unchanged. The complete reported
work-normalized bound improves from129.365218 to129.375910bits. All50 benchmark
proofs verified;662 distinct library tests and three upstream tests passed.
See [THROUGHPUT.md](THROUGHPUT.md) for the implementations, full measurements,
remaining bottlenecks, small-batch results, and exact reproducibility details.

### Kernel follow-up (same v3 protocol)

The four subsequent kernel optimizations preserve exact proof transcripts and
all security parameters. A fresh matched seed-42 comparison improves from
1.195 to 1.047 ms/signature (955 signatures/second), with peak RSS falling from
4.540 to 3.532 GiB. The 1 ms objective remains unmet. Witness and constraint
counts and the reported security bound are unchanged. See
[KERNEL_THROUGHPUT.md](KERNEL_THROUGHPUT.md) for all input seeds, complete
measurements, implementation details and validation.

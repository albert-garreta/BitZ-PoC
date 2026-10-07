> Historical notes from before the SharedPrime-only cleanup. These APIs,
> domains, and measurements do not describe the current implementation.

# Native-ring batch Falcon prover

`PreparedFalconHybrid` uses one native-ring arithmetic path. Its statement domain
is `bitz/falcon1024-ct/hybrid/native-ring/non-zk/v6`; earlier proofs are
incompatible. This profile remains the default; shared-prime V4 is opt-in.

The public statement still contains each public key, 32-byte message, and exact
CT signature. The prover is not zero knowledge. Binary Keccak, SHAKE wiring,
one joint source commitment, the jointly batched two-limb bridge, and the UDR shared
PCS opening retain their existing roles.

## Source representation and counts

For N=1024, M=1311, Q=12289, decode remainders and biased s1 using
`bounded14(b) = sum(k=0..12, 2^k*b[k]) + 4097*b[13]`.
Every bit string decodes into [0,12288]; s1 is the decoded value minus6144.
The honest encoder sets the top bit exactly when the value exceeds8191.
Representations may overlap, but every source map uses the same decoder.

| Per live signature | Native ring |
| --- | ---: |
| Auxiliary arithmetic values | 8605 |
| Auxiliary committed bits | 87705 |
| Public/constant copy bits | 12873 |
| Live arithmetic bits | 100578 |
| Arithmetic allocation per capacity slot | 131072 |
| Linear rows, live / padded | 4458 / 8192 |
| Rejection outer rows, live / padded | 1311 / 2048 |
| Cubic leaf rows, live / padded | 1311 / 2048 |

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
5. Sample compaction fingerprints and a signature equality point. Reduce the
   weighted difference of each signature's candidate/output roots through a
   joint weighted quadratic forest. Its scalar candidate/output split reaches
   the committed bits through the cubic leaf proof and one affine output claim.
   The leaf proof inherits the signature point; padded trees have all-one leaves.
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
The security report includes root batching, quadratic forest rounds, scalar
line reductions, and cubic leaf rounds;
all configured batch sizes must meet the target in the complete union bound.
These are the repository's computational grinding bounds, not statistical128-bit
soundness. Every carry and certificate precedes the challenge testing it.

The current schedule uses one signed root equality, weighted quadratic forest
rounds, scalar line reductions, and a fixed incorrect signature argument for
fingerprints. Each prime group has an explicit budget. At batch 1024, cubic,
forest-round, and root/line difficulties are all 17 bits; fingerprint difficulty
is 19 bits for every batch. See the
[security ledger](../OPTIMIZATION_SECURITY.md) and
[compaction argument](../COMPACTION_SOUNDNESS.md).

Run the existing benchmark at batch1024, security128, seed42, threads16,
one warmup and three measured trials with `-C target-cpu=native`. Compare total
prover time including witnesses, commitments, all native work, SHAKE, and the
shared opening. The historical matched baseline is3.160ms/signature.
Native projection/certificate/carry spans are reported separately. New proof
transcripts are not expected to match historical Debug digests.

## Prior kernel measurements

The v3 measurement reports predate the current joint forest and grinding
allocation. They are historical measurements, not results for native v5.
See [KERNEL_THROUGHPUT.md](KERNEL_THROUGHPUT.md) for the previous kernel
measurements.

### SIMD and factored-table follow-up (same v3 protocol)

The next pass adds four-lane binary sumchecks and Karatsuba products, streams
final binder coefficients, specializes nonce hashing, and factors the cached
Keccak tensor weights. It preserves the same constraints, witness bits and
security schedule. Current matched measurements and exact-proof validation
are in [SIMD_THROUGHPUT.md](SIMD_THROUGHPUT.md).

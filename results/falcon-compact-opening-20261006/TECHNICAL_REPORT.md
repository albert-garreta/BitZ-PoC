# Compact Falcon opening: frozen V7 technical report

Correctness checks pass for frozen V7. **Timing qualification is pending.**
The subject is `/tmp/falcon-compact-opening-candidate-v7`, source-manifest
SHA-256 `cece40e49492801a1ecf8ffb33304b75c78534da4e86784e4b9994f34fdbaf7d`,
and benchmark binary SHA-256
`f784b4cdf4a26b657f5390da67c3c0bbb46aa92bb0418cd001b39f486a4c0677`.
The comparison baseline is the already qualified one-source revision
`9a401286c98db041762b97f48cfe4f1a8d85ce7c`, with the same benchmark placement
overlay. Concurrent working-tree parameterization changes are preserved and
are outside this validation. [Build](performance/candidate-build-v7.json),
[source audit](performance/v7-source-audit.json),
[working-tree divergence](performance/v7/working-tree-concurrent-audit.json).

## Encodings and soundness

Let K = GF(2^128). The source root already authenticates sixteen K elements
per encoded row. If O is the prepared geometry's ascending occupied-lane
list, the new initial message contains only E(q,O[k]). Reconstruction is

    E_full(q,l) = E_compact(q,rank_O(l))  for l in O,
                  0                       otherwise.

The occupied count is 13 at batch one and 11 at batches 2..1024. This removes
48 or 80 bytes per stored initial row. The verifier hashes the reconstructed
full sixteen-lane row against the original source root and multiproof.
Only entire structural zero lanes are omitted; inactive signatures, suffix
padding, and circuit padding inside occupied lanes do not permit omission.

At the terminal recursive commitment, the new proof stores the complete
message F(c,l), in column-major order with lane index least significant.
It is the message before the last lane folds, not a prefix of the
non-systematic encoded codeword. For the existing configured RS encoder:

    E_last(q,l) = sum_c RS_last[q,c] F(c,l),
    R_last = MerkleRoot_q H_leaf(E_last(q,0) || ... || E_last(q,L-1)),
    y(c) = sum_l eq(r_last,l) F(c,l).

The verifier checks the full recomputed root against the already-bound
recursive root and derives y. Thus the separate final residual, selected
row table and multiproof can be omitted. With 32 columns, the complete
message sizes at batches 1/3/32/1024 are respectively 128/64/64/256 K
elements, or 2,048/1,024/1,024/4,096 bytes, for both protocols and targets.

For the same normalized LCH basis Psi_c used by the RS encoder, let
a_i = eq(alpha_last,i), truncated to the existing query count. Then

    B_last(c) = sum_i a_i Psi_c(q_i),
    e_last = sum_i a_i sum_l eq(r_last,l) E_last(q_i,l)
           = sum_c y(c) B_last(c).

After full root authentication and the prescribed derivation of y, the
existing residual check therefore simplifies exactly:

    <y, B_prev + beta_last B_last> = t_prev + beta_last e_last
    iff <y, B_prev> = t_prev.

All earlier claims, padding and OOD contributions remain in B_prev. The
verifier retains the original query, alpha, beta and grinding draws, and
observes derived y at its original transcript position. Prepared dimensions
and exact lengths are checked before reconstruction/allocation. Canonical
reconstruction adds no algebraic soundness term; authentication still uses
the existing commitment assumptions. Falcon accepts the new representation
directly; unrelated generic clients keep their original representation.
[Protocol details](../../src/piop/spartan/falcon1024_ct/ONE_SOURCE.md).

## Retained implementation optimizations

These preserve proof values and transcript schedules:

- Reuse bounded, immutable NTT preparation and cancel the authenticated
  terminal induced-query term as above.
- Compact contiguous occupied prefixes by validating the omitted suffix and
  truncating; preserve the tested generic gapped-lane path.
- Keep Keccak transpose working state in contiguous physical lane order.
- Skip high identity windows in public exponentiation, preserving the
  balanced full-width product; cache the fixed public generator preparation.
  Private exponentiation and challenge/witness data are not cached.
- Compute the same compaction grinding schedule at compile time using the
  unchanged exact optimizer and tie order.
- Specialize the five fixed AVX-512 BLAKE3 rounds, preserving G order,
  minimum-nonce search, fallback paths and full-hash checks.

The parallel grinding threshold, work scheduling policy, forest engine,
security parameters and protocol domains remain unchanged. Rejected or
read-only experiments are not retained optimizations.

## Correctness evidence

Frozen V7 passed 135 Falcon tests, 16 opening tests, 10 distinct generic
hybrid tests, 10 scanner tests and 9 Spartan grinding tests. The explicit
28-proof matrix covers both protocols, both targets, and batches
1/2/3/7/8/9/1024. Vendor checks include complete-message reconstruction and
transcript equality, malformed proofs, the residual cancellation identity,
independent Keccak coefficients, and field exponentiation boundaries.
Ignored probes are not counted as passing. [Full validation](validation/REPORT.md).

All sixteen seed-42 cold candidate proofs match frozen V6 exactly in proof
debug digest, payload, source root, input digest, security ledger and
grinding metadata. This isolates V7's scanner change from proof behavior.
[Exact comparison](performance/v7/cold/v6-equivalence.json).

## Payload and pending timing verdict

Cold diagnostics show smaller payloads in every supported benchmark case.
At batch 1,024, the seed-42 byte counts are:

| Protocol | Target | Baseline | V7 | Saved |
| --- | ---: | ---: | ---: | ---: |
| NativeCarry | 100 | 767,388 | 725,860 | 41,528 |
| NativeCarry | 128 | 868,140 | 814,924 | 53,216 |
| SharedPrime | 100 | 700,466 | 659,002 | 41,464 |
| SharedPrime | 128 | 867,546 | 814,810 | 52,736 |

These are single-seed payload diagnostics, not final qualification means.
They do not establish a 0.4 MB full-proof target. [All cold cases](performance/v7/cold/REPORT.md).

**Final timing status: pending.** The fixed campaign requires all sixteen
protocol/target/batch cells, seeds 42..101, one warmup and three measured
proofs per process, and sixteen global workers. Each seed's process median
is one observation. The paired one-sided 95% bootstrap upper ratio must be
at most 1.02 for both total proving and verification time, and mean payload
must be strictly smaller in every cell. No partial or diagnostic campaign
can satisfy the gate. Final acceptance must also include source, binary,
fixture and observed CPU-affinity integrity checks.
[Live campaign report](performance/v7/qualification/REPORT.md).

## Task-owned source scope

The frozen V7 comparison contains exactly fourteen non-results file changes:
nine production/test files, one protocol document, and four benchmark/harness
files. [Exact paths and before/after hashes](task-owned-files-v7.json).
Cargo manifest and pre-existing user example work are identical between the
frozen manifests and are excluded. Concurrent parameterization edits and new
files are also excluded; no claim here validates the current working tree.

# One Falcon source commitment

SharedPrime authenticates the arithmetic and Keccak slab witnesses
through one initial Merkle tree. The logical source dimensions, arithmetic
bridge, ring switch, and recursive Ligerito commitments remain separate
concepts. There is no compatibility path for the preceding Falcon proofs.
The protocol identifier is `bitz/falcon/shared-prime/non-zk/v5`. `FalconHybridStatement::source_root` is the one
source root. Ligerito's `initial_root` field is still the statement identifier,
not that Merkle root; its authentication callback checks `source_root`.

## Projection and commitment

Let K = GF(2^128), with the implementation's polynomial basis beta_b. The
joint binary witness is W(b,l,t), with seven bit coordinates, four lane
coordinates, and the common position coordinates. For source j, lane width
d_j and aligned offset o_j, define the coordinate selection

    (P_j W)[128(d_j t + k) + b] = W[128(16t + o_j + k) + b].

For Falcon-1024 at batch 1024 the widths in A/K16/K4 order are [1,8,2], the offsets are
[10,0,8], and there are 2^20 positions. At batch one its widths are [1,8,4]
and offsets [12,0,8]. These values are derived from Geometry, not selected
by the prover. Falcon-512 uses its own slab decomposition and derived widths. Unused lanes and physical source suffixes are zero.

Pack V(t,l) = sum_b beta_b W(b,l,t), and RS encode along t:

    E(q,l) = sum_t RS[q,t] V(t,l).
    R_W = MerkleRoot_q H_leaf(E(q,0) || ... || E(q,15)).

Each leaf authenticates all sixteen canonical little-endian K elements,
including padding. The implementation encodes only populated source lanes,
gathers bounded batches of full rows for hashing, and builds one initial
tree. It does not allocate a dense padded joint codeword or three temporary
source trees. One multiproof authenticates the queried initial rows. The
proof stores their occupied lanes in ascending lane order; the verifier
restores the structural zero lanes before hashing or inducing a sumcheck.
All sixteen lanes therefore remain part of the committed alphabet.

## Compact opening messages

For Falcon-1024, the initial opening stores eleven field elements per query at batch sizes
2..1024: eight K16 lanes, two K4 lanes, and one arithmetic lane. Batch one
stores thirteen because its K4 slab needs four lanes. The prepared geometry
determines these positions and counts. Each row is expanded by

    E_full(q,l) = E_compact(q,rank(l))  if l is an occupied lane,
                  0                  otherwise.

Only entire structural zero lanes are omitted. An inactive signature, a
source's physical suffix, or a circuit padding coordinate does not justify
dropping an encoded field element from an occupied lane. The source root,
query positions, multiproof hashes, and reconstructed row bytes are unchanged.
This saves 80 bytes per initial query, or 48 bytes at batch one, before framing.

At the final recursive level, let F(c,l) be the full message of its existing
commitment, with 32 columns and L lanes. Its stored order is column-major
with the lane index in the least significant address bits. It is the witness
table before the last lane folds, not a prefix of the non-systematic RS
codeword. Previously the proof sent the folded residual y, selected encoded
rows, and their Merkle multiproof. It now sends F alone. The verifier checks

    E_last(q,l) = sum_c RS_last[q,c] F(c,l),
    R_last = MerkleRoot_q H_leaf(E_last(q,0) || ... || E_last(q,L-1)),
    y(c) = sum_l eq(r_last,l) F(c,l).

The recomputed root must equal the last recursive root already bound in the
transcript. The verifier derives the old residual y from F and retains the
residual sumcheck check. This authenticates the entire final codeword without
storing a final multiproof. For Falcon-1024 at both security targets, batches 1/3/32/1024 use respectively
128/64/64/256 field elements for F: 2,048/1,024/1,024/4,096 bytes. These are
smaller than even the former residual-plus-row payloads before their hashes
and framing are counted.

After that full root check, the last induced-query contribution cancels
exactly. For the same normalized LCH basis used by the RS encoder, write
Psi_c(q) = product_{j:c_j=1} W_hat_j(q) and a_i = eq(alpha_last,i), with
the equality table truncated to the number of queries. Then

    B_last(c) = sum_i a_i Psi_c(q_i),
    e_last = sum_i a_i sum_l eq(r_last,l) E_last(q_i,l)
           = sum_c y(c) B_last(c).

Consequently the existing final residual equation

    <y, B_prev + beta_last B_last> = t_prev + beta_last e_last

is exactly equivalent to <y,B_prev> = t_prev. The compact verifier uses
this identity to avoid materializing the final queried rows and evaluating
their induced basis. It still checks the full final root, derives y by the
prescribed lane folds, and consumes the original queries, alpha_last, and
beta_last. Earlier query, padding, and OOD contributions remain in B_prev.
Other PCS clients retain their own final encoding and explicit query contribution.

Compact encoding reconstructs the same mathematical messages and keeps the
security parameters and challenge schedule. The SharedPrime-only cleanup uses
a fresh enclosing domain, so its query positions and recursive roots may change. The derived residual y is observed at its original transcript position;
F is authenticated through R_last rather than an extra transcript observation.
The original grinding, query draws, and final verifier-only batching draws
are retained. Lengths come from prepared geometry and configuration and are
checked before reconstruction. Falcon accepts the compact shape directly;
separate-source clients retain their existing opening representation.

## Arithmetic and binary claims

For an affine decoder z = D A + d over its original field, A = P_A W:

    <u,z> = t  iff  <P_A^T D^T u,W> = t - <u,d>.

The implementation represents these coefficient pullbacks with factored
tensors and gathers, not dense matrices. The arithmetic domain remains local.
Combine coefficients of aliased bit addresses in the extension field before
canonical integer lifting. A decoder transpose does not cross fields: the
integer polynomial reduction and BitZ bridge remain necessary. The bridge
consumes the same prime-field terminal value that the source binder checks.

HashToPoint's canonical public selection masks are bound before the ring
challenges. For mask a and remainder decoder B, C=M_a B A, where M_a selects
the first N accepted candidates in order. The binder proves both the public
mask's consistency with committed rejection bits and this linear equality.
Partial polynomials reuse the same bits through these fixed maps; no
intermediate U/V coefficients are committed. The masks add proof data, not
another witness commitment. This protocol remains non-ZK.

Keccak contributes its two local AB/C linear claims for each slab. The public
nonce/input, chaining (including K16 to K4), and HashToPoint extraction checks
remain in the joint binary sumcheck. The parent transcript binds all values
before drawing their batching coefficients. Local source claims become
joint claims through P_j^T, including the lane-selector factors at arbitrary
field points; matching Boolean addresses alone is insufficient.

The joint sumcheck reduces to W evaluated at one point. The existing ring
switch then reduces this bit evaluation to a packed K-linear claim on V.
Its algebraic support-padding check is retained. The final PCS continuation
authenticates that claim against R_W and the recursive folding commitments.

## Inactive Keccak instances

For B live signatures in capacity C, constrain the constant wire of every
permutation block to mu_B(s) = [s < B]. In the existing lincheck the target is

    alpha v_L + v_R + beta MLE(mu_B)(r_signature).

The constant-column coefficient still receives beta. Evaluating the prefix
mask takes O(log C) operations and introduces no new messages or challenges.
The discrepancy between the committed constant column and the public mask
is a multilinear polynomial of degree at most the existing outer dimension,
so the existing constant-pin error allocation covers it.

For active blocks, the homogeneous circuit is ordinary Keccak. For inactive
blocks, kappa=0 forces the input to zero through S0*kappa=S0. The homogeneous
round equations then force every subsequent intermediate to zero; output
pinning and unused-row constraints force the remaining coordinates to zero.
Removing the constant pin or multiplying the whole circuit by the live mask
would not enforce this property. Tests use the actual compact constraint
walker, not the empty sparse-matrix preparation stub.

The generator executes only live permutations. Full groups use the existing
SIMD kernel; partial groups use scalar generation. Inactive z/a/b/stripe
buffers and outputs are zero, including recycled storage. Batch one's K4
capacity remains two for packing, but its second instance is all zero.

## Binding and soundness accounting

Before challenges, the statement binds R_W, public inputs, protocol version,
source geometry and offsets, packing, zero-inactive policy, security target,
prime family and bridge profile, and grinding schedule. The actual shared
prime is sampled after binding the integer lift. Each Keccak prefix additionally
binds the actual circuit identity, slab identity, local dimensions, activity
policy, and the parent statement digest. Prepared verifier metadata determines
all layouts; proof lengths do not select a profile.

The initial PCS oracle is one vector-valued RS codeword, with each sixteen-lane
row as one alphabet symbol. Apply the existing unique-decoding continuation
to this joint oracle. Do not infer joint proximity from separate proximity
of the three sources: their corrupt positions could be disjoint. Merkle
binding identifies one joint oracle, the PCS establishes its proximity, and
the padding/projection constraints identify the logical committed witness.

`PreparedFalconHybrid::security()` retains the full union-bound ledger,
including Keccak pinning, binary claim batching, support padding, and PCS
challenge blocks. The activity-mask change adds no degree beyond the old
constant-column identity test. Exact ledger tests cover every batch 1..1024
at both targets. This is the repository's computational grinding model,
separate from BLAKE3 collision security; the protocol does not provide ZK.

## Qualification

Full Falcon HashToPoint qualification against the archived baseline, including
its timing, witness-size, and process-memory accounting, is described in
[README.md](README.md).
It uses both degrees, both security targets, batch 1,024, and threads 1/2/4/8/16.
The fixed archived seed and five measured trials qualify only this input and
sampling schedule. Source roots change with the new layout; protocol version
and mask binding also change transcript challenges. Historical campaigns
remain evidence for their own revisions; they do not qualify a new build.

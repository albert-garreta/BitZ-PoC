# One Falcon source commitment

Both Falcon protocols authenticate the arithmetic, K16, and K4 witnesses
through one initial Merkle tree. The logical source dimensions, arithmetic
bridge, ring switch, and recursive Ligerito commitments remain separate
concepts. There is no compatibility path for the preceding Falcon proofs.
Native uses `native-ring/non-zk/v6`; shared prime uses
`shared-prime/non-zk/v5`. `FalconHybridStatement::source_root` is the one
source root. Ligerito's `initial_root` field is still the statement identifier,
not that Merkle root; its authentication callback checks `source_root`.

## Projection and commitment

Let K = GF(2^128), with the implementation's polynomial basis beta_b. The
joint binary witness is W(b,l,t), with seven bit coordinates, four lane
coordinates, and the common position coordinates. For source j, lane width
d_j and aligned offset o_j, define the coordinate selection

    (P_j W)[128(d_j t + k) + b] = W[128(16t + o_j + k) + b].

At batch 1024 the widths in A/K16/K4 order are [1,8,2], the offsets are
[10,0,8], and there are 2^20 positions. At batch one the widths are [1,8,4]
and offsets [12,0,8]. These values are derived from Geometry, not selected
by the prover. Unused lanes and physical source suffixes are zero.

Pack V(t,l) = sum_b beta_b W(b,l,t), and RS encode along t:

    E(q,l) = sum_t RS[q,t] V(t,l).
    R_W = MerkleRoot_q H_leaf(E(q,0) || ... || E(q,15)).

Each leaf authenticates all sixteen canonical little-endian K elements,
including padding. The implementation encodes only populated source lanes,
gathers bounded batches of full rows for hashing, and builds one initial
tree. It does not allocate a dense padded joint codeword or three temporary
source trees. Query rows are gathered in the same order as the committed
rows, and one multiproof authenticates them. Later PCS trees are unchanged.

## Arithmetic and binary claims

For an affine decoder z = D A + d over its original field, A = P_A W:

    <u,z> = t  iff  <P_A^T D^T u,W> = t - <u,d>.

The implementation represents these coefficient pullbacks with factored
tensors and gathers, not dense matrices. The arithmetic domain remains local.
Combine coefficients of aliased bit addresses in the extension field before
canonical integer lifting. A decoder transpose does not cross fields: the
integer polynomial reduction and BitZ bridge remain necessary. The bridge
consumes the same prime-field terminal value that the source binder checks.

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

Compare against frozen 51405193, including recorded benchmark instrumentation.
Both protocols, targets 100/128, batches 1/3/32/1024, and sixteen threads must
pass the fixed sixty-seed qualification. The one-sided 95% paired bootstrap
upper bound on the ratio of mean per-seed median total proving and verification
times must each be at most 1.02. An inconclusive bound does not pass.
Smaller payloads are required in every case and at least 20% reduction for
batch-1024 SharedPrime at each target. Parameter or grinding weakening is not
a way to satisfy the runtime gate. Actual results belong in the accompanying
qualification report; analytical size estimates are not measurements.

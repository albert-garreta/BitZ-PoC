# Codebase map

Goal: Explain the RSA MultiSwap path from fixed-width integers through Spartan and the committed BitZ tensor.

Entry path:
- `benches/multiswap.rs::run_once`
- `MultiswapCircuit::build_batch`
- `MultiswapAssignment::new` and `bitz_bit_rows`
- `commit_multiswap_witness`
- `prove_multiswap_mod_r1cs` -> `protocol::prove_reduced`
- `prove_piop` -> plain Spartan outer and inner sumchecks
- `bitify` -> Step 5.0 -> virtual BitZ/Ligerito opening

Core types:
- `Uint<32>`: one unsigned 2048-bit integer as 32 little-endian `u64` limbs.
- `MultiswapCircuit`: integer Mod-R1CS matrices, witness values, row moduli, and quotient advice.
- `MultiswapAssignment`: witness and quotient `Uint<32>` blocks.
- `MultiswapLayout`: four-block Spartan assignment and the 2D committed-bit layout.
- `PiopWitness::FieldAssignment`: complete assignment reduced modulo the sampled fingerprint prime and stored as raw Montgomery `u128` residues.

Key invariants:
- Integer rows satisfy `(Az)_r (Bz)_r = (Cz)_r + m_r q_r`.
- Spartan assignment order is `[constant | witness | quotient | zero]`.
- For the one-copy `k=0` shape: capacity `2^13`, assignment length `2^15`, and committed tensor `2^13 x 2^12` one-bit cells (`2^25` bits).
- Tensor position is `row = (bit_slot << 1) | gate_high`, `column = gate_low`; witness slots are `0..2048`, quotient slots `2048..4096`.
- The commitment is bound before the 128-bit fingerprint prime is sampled.
- Bitification is the adjoint of little-endian integer reconstruction; it connects Spartan's terminal assignment-MLE claim to the committed bits.

Tests/checks:
- `bit_rows_reconstruct_the_assignment_values` passed.
- `witness_first_preserves_products_piop_and_transcript` passed.
- Source tests also cover projected row satisfaction, batch embedding, corrupt quotients, and reference-circuit equivalence.

Open questions:
- None for the representation/dataflow trace. The wired benchmark is a cost-model fixture, not a full public old-to-new accumulator transition.

Recall drill:
- Derive the two modular equations used for one square-and-multiply exponent bit.
- Draw the four Spartan assignment blocks and mark which two are committed.
- For gate `g`, derive the tensor row/column containing witness bit `j`.
- Explain why reducing a 2048-bit value modulo a 128-bit prime is sound only when commitment precedes the prime draw.

---

## secp256k1 ECDSA circuit

Goal: Explain how the secp256k1 verifier is represented and where it enters the SHA-256 + ECDSA proof/benchmark paths.

Entry path:
- `EcdsaCircuit::Secp256k1BiniusMatched`
- `EcdsaCircuit::build`
- `p256::secp256k1_matched::verify_digest_circuit`
- `prepare_sha256_ecdsa_on` / `build_local` for relation synthesis
- `generate_sha256_ecdsa_witness` for witness synthesis
- `prove_sha256_ecdsa` / `verify_sha256_ecdsa`

Core types:
- `Curve` / `SECP256K1`: base/scalar moduli, `a = 0`, `b = 7`, and generator constants.
- `Circuit`: backend-neutral Boolean and signed-integer constraint interface.
- `Rep`, `Elem`, `Point`: bounded modular representatives, canonical 256-bit elements, and affine points with an infinity flag.
- `Split`: sign plus 128 magnitude bits for each GLV scalar half.
- `ConstraintGenerator`: lowers the generic circuit to Boolean map `M` and integer R1CS matrices `A`, `B`, `C`.
- `ProductWitgen`: evaluates the same circuit to source/assignment witnesses and exact row products.

Key invariants:
- Inputs are five 256-bit little-endian bit words: digest, `Q.x`, `Q.y`, `r`, `s`; external byte arrays are canonical big-endian and are converted at the boundary.
- The verifier follows Binius64's stock GLV-Straus schedule: compute `u1 = z/s`, `u2 = r/s`, split both scalars into signed 128-bit halves, build four 16-entry tables, and consume 32 four-bit windows.
- `Q` is on curve; `r` and `s` are nonzero canonical scalars; the accumulated point is finite; its base-field `x` equals `r` without a final reduction modulo the scalar order.
- Standalone shape is 1,280 inputs, 1,007,412 Boolean witness bits, and 10,228 R1CS rows.
- The SHA digest wires are aliased into the first 256 ECDSA inputs rather than duplicated, and the relation digest binds proofs to the selected circuit.
- This circuit is explicitly for the Binius64 head-to-head benchmark, not the paper's default P-256 relation.

Tests/checks:
- `ecdsa::tests::descriptors_agree_with_their_circuits` passed; it synthesizes both circuit variants and checks the pinned dimensions.
- End-to-end coverage: `secp256k1_matched_proves_verifies_and_is_bound_to_its_circuit` and `secp256k1_fixtures_prove_through_the_matched_circuit_and_bind_the_statement`.

Open questions:
- “scpek256” was interpreted as `secp256k1`, the only matching circuit name in the repository.

Recall drill:
- Name the five input words and their bit order.
- Explain why the same Rust function can produce both matrices and a witness.
- Trace the digest from the last SHA compression into the ECDSA verifier.
- Explain why the repository also has `benchmarks/binius64/src/secp256k1.rs` and why it is not the BitZ circuit implementation.

---

## Zinc+ lattice values -> BitZ integer/bit witness

Goal: Explain how an element of a lattice quotient ring can remain a polynomial in Zinc+ while being committed and opened through BitZ's integer-bit representation.

Entry path:
- Zinc+ `DensePolynomial<R, D>` stores the coefficient vector in low-degree-first order.
- `RotationIdeal<R, W>` models reduction by `(X^W - a)`; choosing `a = -1` gives the negacyclic relation `X^W + 1` when the coefficient type supports it.
- `UairTrace` separates binary-polynomial, arbitrary-polynomial, and integer columns.
- Zinc's protocol reduces a terminal witness claim through `phi_q`, MLE evaluation, and `psi_a`.
- BitZ commits the coefficient bits and `piop::spartan::protocol::bitify` applies the adjoint of integer reconstruction to the terminal claim.

Core representation:
- A ring element `f = sum_i f_i X^i` is represented by its basis coordinates `[f_0, ..., f_{N-1}]`, not by one packed integer intended to preserve multiplication.
- Choose canonical coefficient representatives `0 <= f_i < q` (or another uniquely range-checked lift).
- Decompose each coefficient `f_i = sum_j 2^j b_{i,j}` and concatenate the bits coefficient-major.
- The public reconstruction map is `L = I_N tensor [1, 2, ..., 2^(B-1)]`.
- A Zinc+ terminal claim `<v, f> = mu` becomes the BitZ bit claim `<L^T v, bits(f)> = mu`.

Key invariants:
- Packing coefficients into one radix integer is only serialization; ordinary integer multiplication does not implement polynomial or negacyclic multiplication.
- If multiplication is lowered to integer constraints, first compute signed negacyclic convolution `d_k`, then prove `d_k - c_k = q t_k` with bounded quotient witnesses `t_k`.
- Keep the semantic lattice modulus `q_lattice` distinct from BitZ/Spartan's transcript-sampled projection prime `q_proj`.
- Coefficients committed through the existing BitZ block layout should be canonical nonnegative residues. Signed auxiliary witnesses require a public bias, positive/negative split, or a new signed bit-reconstruction map.
- Witnesses and bounds must be fixed before the verifier samples the random prime/evaluation used to project integer-polynomial claims into a field.
- The present `benchmarks/zinc-plus` path imports comparable external benchmark metrics; it is not an end-to-end Zinc+-to-BitZ proof adapter.

Worked example:
- In `R_17 = Z_17[X]/(X^4 + 1)`, let `a = 1 + 2X^3` and `b = 3 + 4X`.
- Their product is `-5 + 4X + 6X^3`, whose canonical coefficient vector is `[12, 4, 0, 6]`.
- At coefficient zero the integer identity is `3 - 8 = 12 + 17(-1)`.
- Each output coefficient has five logical bits; today's BitZ layout pads that to physical `word_bits = 8` and constrains the high three bits to zero. A coefficient weight `v_i` gives the five nonzero bit weights `[v_i, 2v_i, 4v_i, 8v_i, 16v_i]`.

Tests/checks:
- BitZ `integer_storage::tests::interleaved_widths_preserve_zero_sign_and_layout` passed.
- Zinc+ source tests cover `RotationIdeal` membership for ideals such as `(X^3 - 2)`; local execution was not repeated because its pinned Git dependency was unavailable in the read-only Cargo cache.

Open questions / implementation gap:
- No current module connects a Zinc+ terminal PIOP claim to BitZ's commitment/opening APIs for a lattice quotient ring.
- The closest BitZ implementation is the extension-field coordinate opening in `ligerito_flock.rs`; a lattice adapter still needs an explicit basis/layout descriptor, coefficient bounds, transcript binding, and signed-auxiliary policy.
- Today's direct BitZ layout uses unsigned cells with physical word width a power of two at most 128; wider lattice coefficients need a limb layout and corresponding bounds.

Recall drill:
- Why does base-`B` packing fail to preserve negacyclic multiplication?
- For a five-bit coefficient and terminal weight `v_i`, derive the five adjoint bit weights.
- Which values need canonical residue ranges, and which values may need signed quotient handling?

### Detailed claim-domain ladder

Notation:
- Semantic lattice ring: `R_L = Z/q_L Z[X]/(Phi(X))`, with basis `gamma_d = X^d`.
- Ring-valued trace cell: `W[t,c] = sum_d a[t,c,d] gamma_d`.
- Coefficient bits: `a[t,c,d] = sum_j 2^j b[t,c,d,j]`.
- PIOP terminal/evaluation ring: `E`; its public map from the witness carrier is `psi`.
- BitZ small prime field: `F_p`; BitZ's binary commitment/exponent field is separate.

Claim ladder:
1. Semantic constraints in `R_L`: `P_l(x,W) = 0`.
2. Exact lifted quotient identity, when needed: `A(Y)B(Y)-C(Y)-q_L K(Y) = Phi(Y) H(Y)` in `Z[Y]`.
3. Ring-aware PIOP terminal claim: `<v, psi(W)>_E = mu`.
4. Coefficient expansion: `mu = sum_{t,c,d} v[t,c] psi(gamma_d) a[t,c,d]`.
5. Adjoint bitification: `mu = sum_{t,c,d,j} v[t,c] psi(gamma_d) 2^j b[t,c,d,j]`.
6. In matrix form, if `v[t,c] = eq(t,r) lambda_c`, the bit weights factor as `eq(t,r) * lambda_c * psi(gamma_d) * 2^j`.
7. If `E` is not already a BitZ-compatible small prime field, lift the tensor factors of the bit-weight vector to a free ring, send the exact lifted result `mu'`, check its bounds and projection back to `mu`, then sample a random morphism to `F_p`.
8. For projected tensor weight `u = u1 tensor u2`, let `gamma_i` be the canonical integer lift of `u1[i]`. BitZ proves exact integer claims `m_j = sum_i gamma_i b[i,j]`, checks `sum_j (m_j mod p) u2[j] = mu`, and certifies `g^m_j = product_i (g^gamma_i)^{b[i,j]}` without exponent wrap.

Zinc+ specialization:
- `psi` is coefficient reduction followed by polynomial evaluation: `Z[Y] -> F_p[Y] -> F_p`, `Y -> alpha`.
- Hence `psi(gamma_d) = alpha^d`, and the terminal bit weight is `eq(t,r) lambda_c alpha^d 2^j`.
- The trace remains polynomial/ring-typed; only its random fingerprint and the sumcheck arithmetic live in `F_p`.

Critical ordering:
- Commit to the coefficient bits before sampling the projection prime, polynomial evaluation point, or PIOP challenges.
- Bind the PIOP terminal claim before any optional BitZ re-projection.
- If a ring-valued terminal claim is lifted, send and range/degree-check `mu'` before sampling the fresh morphism.

Verified check:
- `cargo test bitification_is_the_adjoint_of_integer_reconstruction --offline` passed, confirming the current integer-Spartan instance of Step 5.

Checked toy ring-terminal trace:
- `R = F_17[x]/(x^4+1)`, `f0 = 3+5x+2x^2+x^3`, `f1 = 6+x+16x^2+2x^3`.
- Ring MLE at `rho=x`: `mu=(1-x)f0+x f1`.
- Exact free-ring result: `mu'=(1-X)f0_tilde+X f1_tilde = 3+8X-2X^2+15X^3+X^4`; its canonical projection to `R` is `mu=2+8x+15x^2+15x^3`.
- Toy fresh projection `p=101, X->7` gives `mu'(7)=33 mod 101`.
- Coefficient matrix `D=[[3,5,2,1],[6,1,16,2]]`, row weights `[95,7]`, and column weights `[1,7,49,40]` produce exact integer row folds `[327,482,302,109]`.
- Final read-off: `327+7*482+49*302+40*109 = 22859 = 33 mod 101`.
- The example demonstrates why the random map is applied to the free-ring lift `mu'`, not directly to the quotient-ring value `mu`.

### Design decision: native-ring PIOP vs projected-field PIOP

Recommendation:
- Use ring/polynomial-valued arithmetization and quotient-ideal constraints, then a transcript-random projection to a prime field, then run the PIOP over that field.
- Use the same supported projection prime for the Zinc+ PIOP and the direct BitZ opening when possible; current BitZ requires at most 126 bits.
- Do not expand every lattice multiplication into coefficientwise integer R1CS rows, and do not run standard sumcheck directly over a zero-divisor lattice quotient ring.

Cost model:
- Let `T*C` be the number of ring-valued trace cells, `N` the ring dimension, `P` the number of scalar operations in the projected PIOP, and `M(N)` the cost of one native ring multiplication.
- Projected field path: one-time `O(T*C*N)` coefficient reduction/Horner pass plus roughly `O(P)` field operations; interactive messages contain field elements.
- Native ring path: roughly `O(P*M(N))` arithmetic, and each transmitted ring element normally carries `N` coefficient field elements.
- Projection retains the useful ring batching: one random evaluation at `alpha` compresses an `N`-coefficient polynomial identity to one field identity with collision probability about `degree/p`, so coefficientwise expansion is unnecessary.

Soundness:
- Standard sumcheck/Schwartz--Zippel reasoning is naturally field-based.
- Lattice quotient rings often have zero divisors; a nonzero discrepancy can be annihilated, so ordinary field-PIOP soundness does not transfer automatically.
- A native ring PIOP is attractive only with a protocol proved for that exact ring, a compact ring PCS, and a demonstrated SIMD/NTT saving that exceeds its arithmetic and bandwidth costs. If the quotient is actually an extension field, this is effectively a field PIOP.

### Exact `r_0`-to-packed-bits bridge for Zinc+

Goal:
- Replace Zinc+'s Step-7 PCS opening with a BitZ opening without weakening the existing Step-6 check.

Zinc+ terminal object:
- Let `A_c(t,X) = sum_d [a[t,c,d]]_p X^d` be witness column `c` after coefficient projection to `F_p[X]`.
- Step 5 reduces all PIOP evaluation claims to one Boolean-hypercube point `r_0`.
- Step 6 announces the coefficient-wise MLE evaluation
  `bar_U_c(X) = sum_t eq(t,r_0) A_c(t,X) = sum_d u[c,d] X^d`,
  where `u[c,d] = sum_t eq(t,r_0) [a[t,c,d]]_p`.
- Zinc+ evaluates every `bar_U_c` at the earlier polynomial challenge `alpha` and checks those scalar values against the multipoint subclaim. It then absorbs every coefficient `u[c,d]` before invoking the PCS.

Drop-in BitZ opening:
1. Commit before challenge sampling to the canonical coefficient words (equivalently, their packed bit rows).
2. After Step 6 has absorbed all `u[c,d]`, derive fresh field challenges `beta[c,d]`.
3. Form the public batched claim
   `mu_pcs = sum_{c,d} beta[c,d] u[c,d]`.
4. Arrange the committed integer matrix as `D[t,k]`, with `k = flatten(c,d,limb)` and
   `a[t,c,d] = sum_limb 2^(W*limb) D[t,k]`.
5. Use BitZ row weights `R[t] = eq(t,r_0)` and column weights
   `C[k] = beta[c,d] 2^(W*limb)`.
6. BitZ proves
   `sum_{t,k} R[t] C[k] D[t,k] = mu_pcs (mod p)`
   against the original packed-bit commitment. Internally it sends exact integer row folds, binds them in the `GF(2^128)` exponent, and opens the residual bit functional with ring-switch plus Ligerito.

Why both `alpha` and `beta` are needed:
- `alpha` is the Zinc+ PIOP challenge: `bar_U_c(alpha) = sum_d u[c,d] alpha^d` ties the lifted values to the final multipoint/sumcheck subclaim.
- Fresh `beta[c,d]`, sampled only after all `u[c,d]` are fixed in the transcript, batches every claimed coefficient back to the commitment. Reusing only the `alpha` combination would not implement Zinc+'s current PCS interface, which opens the full polynomial `bar_U_c(X)`.

Tiny two-row check:
- `A(0,X)=a_00+a_01 X`, `A(1,X)=a_10+a_11 X`, and `r_0=r`.
- `bar_U(X)=(1-r)A(0,X)+r A(1,X)=u_0+u_1 X`.
- Step 6 checks `bar_U(alpha)=u_0+alpha u_1` against the PIOP subclaim.
- After absorbing `(u_0,u_1)`, draw `(beta_0,beta_1)` and ask BitZ to prove
  `beta_0 u_0+beta_1 u_1 = sum_t R[t](beta_0 D[t,0]+beta_1 D[t,1])`,
  with `R=[1-r,r]`, against the one pre-existing bit commitment.

Implementation note:
- The existing Spartan `bitify` helper assumes a four-block assignment and structured equality weights. A Zinc+ adapter should instead call the generic integer-matrix opening with its own dense `row_weights_q` and `col_weights_q`.
- Use the same at-most-126-bit projection prime for Zinc+ and BitZ when possible. Otherwise an additional exact lift and fresh-prime reduction needs its own bounds and transcript phase.

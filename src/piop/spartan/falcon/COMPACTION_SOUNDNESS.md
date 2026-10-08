# HashToPoint public selection and linear routing

Full Falcon shares one committed binary witness across SHAKE, rejection,
ordered compaction, the native ring equation, and the norm. HashToPoint has
no grand-product or layered GKR proof. BitZ's internal integer-to-binary
product GKR and the shared PCS opening remain necessary. The proof is non-ZK.

## Integer witnesses and rejection

For q=12289 and D=717/1311 draws, SHAKE links authenticate the big-endian
words t[j]=256*y[2*j]+y[2*j+1]. Constrain

    t[j] = q*u[j] + v[j].
    u[j] = u0 + 2*u1 + 4*u2.
    v[j] = sum(k=0..12, 2^k*vbit[k]) + 4097*vbit[13].
    e[j] = u2*u0.

Every remainder encoding lies in [0,q-1]. The 16-bit word and exact division
equation force u into 0..5, so e is exactly rejection. All bits are from the
original source and require BitZ authentication; native witness checks alone
do not establish these claims. No committed prefix counters are needed.

## Public mask

Each proof supplies ceil(D/8) bytes containing a public Boolean mask a, with
little-endian bits inside each byte. The verifier checks the exact byte count,
zero unused high bits, exactly N set bits, and exactly one mask per live
signature. The increasing selected positions j[0],...,j[N-1] and cutoff
J=j[N-1] are deterministic. Define the public constant d[j]=[j<=J].

Before the native ring challenge or shared-prime sampling, absorb the
canonical masks and their domain separator. On committed rejection bits,
enforce the linear rows

    d[j]*e[j] = d[j]-a[j].

Thus for every j<=J, a[j]=1-e[j]; after J, a[j]=0 by the mask's definition.
Together with its checked cardinality N, the mask selects exactly the first
N accepted draws. A malicious prover cannot skip an earlier accepted draw,
include a rejected draw, or reorder the selected indices. Masks are public
proof messages, not trusted advice or independent commitments.

## Reuse the existing bits

Let b be the committed remainder bits and B their bounded-14 decoder. Define

    M_a[k,j] = [j=j[k]].
    C = M_a B b.

The N linear routing rows bind the existing unsigned-14 C coefficients to
the selected bounded remainders. The different top-bit weights (8192 for C,
4097 for v) are preserved by each decoder. Canonical C then follows from
this equality and the remainder bounds.

A partial polynomial on the draw interval [l,r) can be recovered as

    V[l,r](X) = sum(l<=j<r, a[j]=1) v[j]*X^sum(l<=t<j, a[t]).
    U[l,r](X) = X^sum(l<=t<r, a[t]).

For fixed a every coefficient of V is a fixed linear expression in b and U
is public. The implementation needs only the final C equality: it stores no
intermediate U/V polynomials and proves no affine-composition tree. This
linear construction relies on making routing public and validating it. It
does not justify an unchecked witness-dependent decoder for a private mask.

The native ring proof consumes the committed C,S1,S2 bits. The norm consumes
the same S1 bits and the public signature's S2 coefficients; public-input
rows bind the committed S2 encoding to those coefficients.
SHAKE consumes the same output bits linked to t. Linear routing never casts
an extension-field element into the arithmetic prime field.

## Reduction to BitZ

There are D quadratic rejection rows, padded to 1,024 / 2,048. On this same
row domain let S contain S1 followed by N zeros, and let i index signatures.
One combined outer sumcheck proves

    sum(i,j, eq(rho,i)*S(i,j)^2
             + lambda*eq(tau,(j,i))*(A(i,j)*B(i,j)-C(i,j)))
      = sum(i, eq(rho,i)*(BETA_SQUARED - sum_j(public_S2[i,j]^2))) - slack.

Here A=u2, B=u0, C=e, and slack is the rho-weighted slack evaluation.
The source is fixed before rho and tau; slack is absorbed before the fresh
family-batching challenge lambda. The polynomial has degree at most three
per variable. Padded rejection rows and inactive signatures are zero.
The four terminal MLEs S,A,B,C and slack are linear functions of committed
bits. Bind these five claims before their random binder coefficients are sampled.

Merge these five claims, the native ring projection, public-input rows,
candidate divisions, mask-validity rows, routing rows, and padding checks
into one arithmetic-source sumcheck. Its source evaluation is authenticated
by the existing BitZ bridge. SHAKE, its output links, and the bridge join the
final binary opening against the single source root.

Division residuals are bounded by 98,311 on arbitrary source bits; selection
residuals by 1; routing residuals by 16,383. These are below either supported
prime family, so a zero residual in Fp is the intended integer equality.
Norm bounds remain the dominant exact-source bound. The combined sumcheck
needs norm-instance, rejection-row-point, family-batching, and cubic-round
error allocations; there is no extra
HashToPoint polynomial-evaluation challenge or projection error.

## Witness and payload

Falcon-512 / Falcon-1024 now use 52,637 / 100,482 live arithmetic bits,
removing 7,180 / 14,432 prefix-counter bits relative to the grand-product
baseline. Arithmetic strides remain 2^16 / 2^17; SHAKE sizes are unchanged.
At batch 1,024 total packed source storage remains 96 / 176 MiB. Public masks
add 90 / 164 bytes per signature (90 / 164 KiB per batch of 1,024) to the
proof payload. Other proof components change too, so total payload must be
measured independently. No additional initial Merkle tree is introduced.

Qualification compares full Falcon against matched archived prover/verifier
timings and reports process-wide peak RSS separately. Source size alone does
not establish a speedup.

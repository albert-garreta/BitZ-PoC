# Timing-repair attribution — implementation checkpoint

Base: `6271724d`. None of these changes has an accepted performance exception.
All integrated median/P95 and peak-memory gates remain unqualified. Shared `_into` allocation tests pass; these are not whole-prover memory measurements.

| Private-input operation | Previous dependency | Implemented change | Validation |
|---|---|---|---|
| P-256 wide multiply/divide | Magnitude-dependent limb work, variable-time division | Declared 9×9 exact multiplication; prepared 4-limb divisor processes 18 dividend limbs | Differential division plus composed ECDSA circuit tests |
| P-256 inversion | Variable-time inversion | Shared prepared odd inverse, fixed 128L binary-GCD steps, masked validity | Shared unit/nonunit tests, P-256 witness tests, bounded ARM probe |
| Private P-256 table/select hints | Secret table index, secret selection branch | Public-length scans and mask selection; both candidate values are evaluated | Composed witness and invalid-signature tests |
| Private product storage | Zero/sign/magnitude-dependent boxed length | Typed segments retain the declared `Z<L>` width | Interleaved widths, signed extrema, ECDSA/SHA exact matrix-product tests |
| ECDSA signed projection | Sign branch and value-trimmed input length | Declared-width Horner; prepared power correction selected by mask | Independent bigint projection tests, explicit canonical/Montgomery equality, allocation test |
| AES-subfield embedding | Private byte indexes a 256-entry table | Eight basis terms with masks | Independent binary-field differential and embedding tests |
| Wengert geometric runs | Shortcut on private zero base | Fixed public run schedule; public empty/run-boundary shortcuts retained | Canonical/weighted Wengert equivalence and run tests |
| Wengert FIOS final correction | Conditional result selection | Same multiply schedule with masked correction | Wengert equivalence, full-width moduli, bounded ARM probe |
| Circuit satisfaction checks | Value-sized BigInt operations and first-failing-row return | Preparation validates declared-width bounds; row selection is masked, products exact, all rows checked | 45 circuit tests, signed MIN, subset overflow and product-wrap rejection |
| Native u128/MultiSwap first rounds | Eager field tables and variable-width inputs | Borrowed integer segments with masked unsigned difference, fixed-width mixed MAC and fused canonical fold | Full/padded primes, extremes, dense/structured differential tests and production roundtrips |
| MultiSwap committed bit rows | Zero shortcut and set-bit iteration | Exactly 2048 public bit positions per witness/quotient entry | Bit reconstruction, deterministic proof and corrupt quotient tests |
| Bitification equality expansion | Zero-parent shortcut | Prepared factor and fixed multiply/subtract for each public table position | Bitification adjoint and field-helper tests |

MultiSwap generator and witness arithmetic now use declared-width shared
integers. Its 192-bit accumulation layout is explicitly retained by the user;
its bit scan uses fixed public bounds and masked additions. Generic binary
inversion delegates to the shared fixed schedule. Legacy unused private set-bit
projection helpers were removed; the retained four-lane projection uses masks.
The generic checked bit-vector inner product now selects every term with a mask.
These changes are not a whole-program timing certification.

Private product storage can use more payload bytes than magnitude-trimmed rows
for some P-256 values; it eliminates per-row boxed allocations but that is not a
measured peak-memory result. Further narrowing must use public declared bounds.
The new representation and any allocation/latency effects must be measured and
attributed rather than automatically excused as a timing repair.

Public coefficient deduplication still normalizes hash keys, and public generator
preparation still branches on public points. These are public-structure work,
not private-input exceptions. Remaining F2Z private-dependent shape, set-bit and
sign paths are not certified by this checkpoint.

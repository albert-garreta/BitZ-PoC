# ARM instruction review

The executable hash and individual assembly hashes are in [index.json](index.json).
These are the actual frozen confirmation kernels, compiled with Rust 1.98.1 and
`-C target-cpu=native` on the M1 Max.

| Captures | Reviewed branch/dataflow result |
|---|---|
| kernel-01/02, four-limb native MAC | Branches check the two public slice lengths, empty input and loop completion. Carry/overflow uses arithmetic flags, `cset`, `adc` and `cinc`; memory addresses depend on public iteration/lane indices. |
| kernel-05/06/07, signed projection widths 2/4/9 | The signed correction uses `CtSelect`. Conditional branches now check public lengths and loop counters; width 9 also loops over its eight remaining fixed limbs. Modulus correction no longer branches on the borrow. |
| kernel-03/04, packed polynomial widths 3×7 and 9×9 | Branches check public lengths and loop counters. Unlike the production zero-skipping control, these kernels do not skip zero coefficient words. Loads/stores follow public indices. |
| kernel-00, masked batch inversion | Prefix/reverse loops have public length branches; zero selection uses masks. The call is to `FixedMontyForm::invert`. Its returned validity is checked before use. That check always succeeds for the accepted prime-field inputs: zeros are replaced by one and every remaining factor is a unit. This assumption does not extend to composite-modulus inputs. |

No `udiv`, `sdiv` or `fdiv` instructions appeared in these eight captures.
This is a bounded source and assembly review, not a proof of constant-time
behavior for all kernels, dependency internals, microarchitectures or the prover.
The checked `Option` APIs, public-exponent powers, public fixed-scalar dispatch,
existing table embedding and variable-time controls have their separately
documented contracts.

The initial projection prototype used `RawMontyCtx::sub`. At width four, LLVM
introduced a branch on the final subtraction borrow. The candidate was changed
to explicit `CtSelect` and recompiled before confirmation; this review applies
to that corrected executable.

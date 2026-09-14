# Final ARM instruction review

The reviewed executable has SHA256 `4d16fca6e2611020cdaaf5d709e46dedf65af9889ed3414a3da620f897fe9b4d`, identical to both final performance runs. The executable and all captured assembly hashes were verified. Root reviewed the signed correction dataflow; an independent reviewer read the complete signed and grid bodies and the out-of-line grid helper. No blocker was found in these kernels.

## Signed columns

| Width | Assembly | Conditional mask selections |
|---|---|---|
| 2 limbs | [kernel-04.asm](kernel-04.asm) | `0x100038d20`, `0x100038d34` |
| 4 limbs | [kernel-05.asm](kernel-05.asm) | `0x100039068`, `0x100039070` |
| 9 limbs | [kernel-06.asm](kernel-06.asm) | `0x100039408`, `0x10003941c` |

Each implementation has two `CSEL` mask constructions. Operand signs feed conditional selection and masked arithmetic, not branches or memory addresses. Remaining conditional branches compare slice lengths, test empty length, or advance public batch counters. The nine-limb product loop additionally compares its fixed column offset with `0x90`. The assertion call is confined to invalid public length input. No operand-sign `TBZ`/`TBNZ`, secret-derived address, indirect call, or operand division occurs in these bodies.

The first revision's ordinary masks compiled into sign-dependent branches; that executable remains [rejected and recorded](../../candidate-fixes-arm-01/arm-audit/REVIEW.md). `CtSelect` prevents that transformation in this final executable. This verifies the emitted instructions of this build; a different compiler, target or build requires renewed review.

## Grid folding

[accumulate](kernel-15.asm), [fold_tile](kernel-16.asm), and [fold_quad_logical helper](grid-fold-helper.asm) were read in full. The helper starts at `0x1000098cc`; its SHA256 is `29993f95d60ebb81a07252cf9533033891761c767e1ec37c4547384472b80a62`. Its 16 checks compare public source indices with slice length, followed by straight-line PMULL arithmetic. Remaining grid branches and addresses depend on public lengths and loop indices.

Each row loads its complete 16-value source block before storing the four compacted results. Writes cannot overwrite future source blocks or prior compacted outputs. All accesses retain bounds checks; there is no unchecked indexing.

This is an instruction and dataflow review of these ARM kernels, not a universal constant-time certificate. Explicitly public-input sparse polynomial multiplication and inversion remain variable-time APIs.

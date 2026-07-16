# Session prompt: LaTeX note on the F2Z verifier (Ligerito as a black box)

**Repo:** `/Users/albertgarretafontelles/f2z-pcs` (standalone; git, commits
unsigned — `git commit --no-gpg-sign`). Written 2026-07-16, by the session
that extracted F2Z, made Ligerito its only opener, and did the packed-rows +
build-parity passes.
**Skill to load (if available):** `writing-crypto-papers`. If the zinc-plus
skills (`zinc-plus-fieldswitch`) are available they give useful background,
but this note must describe **F2Z as extracted**, not the zinc-plus
deployment — verify every claim against THIS repo's code.

## The task

Write a LaTeX note (new directory `docs/verifier-note/`, `main.tex`,
buildable with `latexmk -pdf`) that precisely describes the **F2Z verifier**
for the core mod-q opening — `verify_mle_eval_mod_q_ligerito` — **treating
the flock Ligerito PCS verifier as a correct black box**. The note's job is
to make the zinc-side verification logic reviewable: a reader holding the
note and the code should be able to check, line by line, that every stated
check exists and that no stated check is silently missing.

Explicitly OUT of scope (user constraint): the internals of the Ligerito /
ring-switch proximity verifier (flock-core). It appears in the note ONLY as
an oracle with a stated interface contract: "given commitment C, point pt,
value v (and config derived from the instance shape), it accepts iff the
committed matrix opens to v at pt" — one definition, no mechanism.

## Structure the note around the soundness chain

`docs/DESIGN.md` states it: committed bits → (opening) leaf claims →
(forest GKR) roots `α^{u}` → (generator binding) integers `u` → (chunk
recombination) `y ∈ F_q`. The verifier note walks the same chain in
verification order, one section per link:

1. **Statement and proof object.** `IntEvalParams` (t, s, W), the claim
   `MLE[INT(D)](r) = y ∈ F_q`, the chunk decomposition (`c_w = 127 − t − W`,
   `L = ⌈q_bits/c_w⌉`), and every field of `IntEvalRsLigModQProof` with its
   role. Include the serialization contract (`src/proof_codec.rs`:
   field-by-field zinc parts + length-prefixed bincode-1.3 `LigeritoProof`)
   and what `from_bytes` rejects.
2. **Fiat–Shamir schedule.** The exact transcript order — every absorb
   (with its tag) and every squeeze, in sequence, verifier mirroring
   prover. Read it out of the code; do not reconstruct from memory. State
   the invariant: everything influencing a challenge is absorbed before it.
3. **The merged-forest GKR verifier** (`src/merged_forest.rs`,
   `verify_merged_forest`): roots DERIVED from the sent folds (`α^{u}`
   recomputed — the absorb is the binding; roots are not proof data), the
   ζ entry (`mle_at(roots, ζ)`), per-layer phase-A/phase-B sumcheck
   verification via the generic `MLSumcheck` verifier, the child-pair /
   μ-line chaining, and the exit claim `(z, e_d)`.
4. **The exit-claim linkage** — the load-bearing glue: how `e_d` becomes a
   K-LINEAR functional of the committed bits (`eq(z)·(α^{w_b·2^j} − 1)`
   weights; `e_d − 1 = ⟨bits, q_rowbit⟩`) and how the η-RLC + ring-switch
   marginals (`s_v`) turn the batch of such claims into the ONE (pt, v)
   pair handed to the Ligerito oracle. Frame the computing-vs-binding
   distinction explicitly (a verifier that recomputes a value the prover
   could have chosen freely binds nothing — state for each quantity whether
   it is recomputed, opened, or oracle-certified).
5. **Integer binding in the clear** (`src/pcs.rs`): the `is_generator(α)`
   check (why injectivity of `n ↦ α^n` on `[0, 2^128−1)` needs it, with the
   squarefree factorization constant), the per-chunk `ChunkRange`
   rejections and the exact bound, and the mod-q recombination
   `y = Σ_c w′_c Σ_l 2^{c_w·l} u_c^{(l)}`.
6. **Config derivation.** `sha_lig_configs`: the verifier derives the
   Ligerito config from the instance shape, independent of the proof;
   embedded FAST profile at `m ≥ 22` is the audited regime — SCOPE THE
   NOTE'S CLAIMS to it (the ad-hoc config below is unaudited, test-only).
7. **What the tests pin** (one short section): the mod-q roundtrip's
   tamper/range/generator rejection arms, the serialization roundtrip +
   tampered-bytes rejection, the both-schedule forest byte-identity pins —
   and what they do NOT cover (FS-order mutations, codec field-coverage).
8. **Trust boundary and caveats**: the oracle assumption (flock-core, local
   path dep), the upstream-flagged Merkle leaf/node domain-separation item,
   and that the virtual-XOR / batched variants exist as pub API but are
   outside this note's scope (one paragraph; note the obligation-returning
   contract if mentioned at all).

## Read first

- `docs/DESIGN.md` (this repo) — the chain and the as-extracted design.
- `README.md` — provenance, the config boundary, reference numbers (cite
  the verify column if the note wants a performance remark: ms-class,
  1.2–15 ms across n = 16–32).
- The code, by FUNCTION NAME (line numbers have drifted):
  `verify_mle_eval_mod_q_ligerito`, `verify_rs_ligerito`,
  `verify_rs_open_ligerito` (`src/ligerito_flock.rs`);
  `verify_merged_forest`, `mle_at`, `absorb_gfs` (`src/merged_forest.rs`);
  `is_generator`, `mod_q_chunk_width`, `mod_q_num_chunks`,
  `recombine_read_off`-equivalents (`src/pcs.rs`);
  `to_bytes`/`from_bytes` (`src/proof_codec.rs`).
- Upstream exposition to borrow notation from (do NOT copy claims without
  re-verifying against F2Z):
  `zinc-plus/.claude/worktrees/f2-int-sha/documentation/f2-int-eval-doc/`
  (the 21-page as-deployed note; its §§ on the forest wire format, the
  batched transcript schedule, and mod-q chunking are the closest prior
  text). Keep K = GF(2^128), α, t/s/W, c_w, L notation aligned with it.

## Traps

1. **F2Z ≠ zinc-plus.** The extraction is single-claim-centric, RAA-free,
   and has drifted (packed-rows commit, codec). Describe what THIS repo's
   verifier does; where the upstream note says something F2Z doesn't do,
   the upstream is not authoritative.
2. **Do not describe Ligerito internals** — the user's explicit constraint.
   If a section needs "and then the opening is checked," that is the
   oracle call, full stop.
3. **The FS schedule must come from the code.** A plausible-but-wrong
   absorb order in the note is worse than no section: it would misdirect a
   review. Quote tags and order as implemented.
4. **Computing vs binding.** For every verifier-side quantity say which of
   the three it is (recomputed from public data / bound by an absorb /
   oracle-certified). This framing catches the real bug class.
5. **Scope honestly**: embedded-profile regime, core mod-q path, oracle
   assumption stated up front — the note should say exactly what a
   correctness argument for F2Z would still owe (Ligerito itself, the
   domain-separation upstream item, the unaudited ad-hoc config).

## Deliverables

- `docs/verifier-note/main.tex` (+ any section files), compiling clean
  under `latexmk -pdf`; check the PDF renders.
- A README pointer (one line in the Layout/docs section).
- Commit in this repo when green (unsigned). No zinc-plus ledger entry is
  required for f2z-only docs work — but if the writing uncovers a BUG or a
  soundness gap in the verifier, STOP writing, report it, and (if it also
  affects upstream) add the finding to
  `zinc-plus/.claude/worktrees/f2-int-sha/documentation/f2x-sha-todo.md`
  per that repo's CLAUDE.md rule.

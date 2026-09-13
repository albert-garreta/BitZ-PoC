# SHA-256 + ECDSA: the build-profile trap, and where F2Z now loses

Date: 2026-09-13 (evening). Box: Apple M5 (10 cores, fanless), 24 GB, macOS 26.6, rustc 1.98.1,
`RUSTFLAGS=-C target-cpu=native`. Workload: one SHA-256 hash of a `64(N-1)`-byte message
(`N = 2^7 = 128` compressions, padding block included, 8,128 bytes) followed by one P-256 ECDSA
verification of the digest; target 100 bits; non-ZK; the suite's fixture
`bench_results/suite-sha256-ecdsa-20260913/fixtures/i7-seed0.json` throughout.
Prompt: `docs/sha256-ecdsa-investigation-prompt.md`. Companion docs: `docs/sha256-ecdsa-comparison.md`,
`benchmarks/binius64/README.md`, `docs/binius64-ligerito.md`.

## Summary

1. **Root cause, confirmed by a 2 × 3 build grid plus two extra cells (§1).** The 2026-09-10/11
   Binius64 numbers came from the worker built under Cargo's release defaults: no cross-crate LTO
   (`-C embed-bitcode=no`) and 16 codegen units, because Cargo applies `[profile]` only from the
   workspace-root manifest and the worker's own root said nothing about LTO (Binius64's shipped
   `lto = "thin"` never applied). Same source, same lockfile, same fixture, only the profile
   changed: 1007–1015 / 149–151 ms (prover / verifier, 1 thread) under the recorded profile —
   the campaign's 1019 / 148 — against 165–169 / 12.7–13.4 ms under thin or fat LTO; 243 / 57 vs
   60–62 / 6.3 ms at 10 threads. Codegen units alone: no effect. The `f2z` path dependency and the
   lockfile change: no effect (the old source under fat LTO gives the same numbers as the new).
   Every build produced byte-identical proofs (BLAKE3 `dbec3448…`). F2Z's own bench loses only
   4 % without LTO, which is why the old table flipped.
2. **Audit (§2).** Every other table already runs every scheme at fat LTO + 1 CGU +
   `-C target-cpu=native` (in-process adapters use the root profile; Limber/Zinc+ carry it in
   their own manifests), with one documented exception: fields-witch keeps its author's
   thin-LTO profile because fat LTO regresses its SHA-NI backend. Only the SHA+ECDSA table was hit.
3. **Policy (§3).** Fat LTO + 1 CGU + `target-cpu=native` for every scheme, stated in every
   worker's root manifest, recorded in the run's provenance (the sidecar records only
   `rustflags` today, which is why the trap was invisible), enforced by the runner; the caption
   states it. Affected artifacts: the inline table `tab:sha256-ecdsa-f2z-opt` and the stale
   `paper/sha256-ecdsa-table.tex`; nothing else changes.
4. **Where F2Z now loses (§4).** With fair builds at 2^7: prover 207 vs 168 ms (1 thread), 66 vs
   60 (10 threads); verifier 45 vs 13 and 15 vs 6. The P-256 part costs the two provers the same
   (F2Z 138 vs Binius64 149 ms fixed, 1 thread; F2Z is faster at 1 KB messages), but each SHA-256
   compression costs F2Z 0.54 ms against 0.068 ms (inner sumcheck over the integer arithmetization
   of SHA at 113 bits, plus the opening): that is the whole 1-thread prover gap, and it grows to
   3.2× at 64 KB. The verifier gap is one fixed term — 30 ms evaluating the P-256 constraint
   matrices (4.24 M nonzero entries) at the sumcheck point, versus 12 ms for Binius64's
   word-granular wiring evaluation (its FRI verification is 0.4 ms) — plus 35 vs 11 µs per
   compression in the ring-switch read-off. Levers: the one-multiplication-per-index tail dot (V1, **measured**:
   verifier −8.5 %, prover −3.6 % at 1 thread, transcript-neutral, tests and pin pass), a hoisted
   field-configuration check in the inner prover (3.5 % of samples), fewer nonzero entries in the
   P-256 gadget (proportional), holographic matrix evaluation (removes the 30 ms term; protocol
   change), and the serial `outer_prove` / `mqv:wprep` / forest phase-A sections at 10 threads.
   The per-compression SHA gap cannot be closed inside the prime-field arithmetization; the
   paper's hybrid SNARK is the answer.
5. **Follow-ups done the same evening (§4.7–4.8, both transcript-neutral, applied uncommitted).**
   The tape wired into `ModQCoefficients` and a run-structured prefix kernel for the P-256 tail:
   at 1 thread the 2^7 prover went 207 → 152 ms (Binius64 168) and the verifier 45 → 23 ms
   (Binius64 12.8); at 1 KB the prover is 93 vs 148 ms. Proof bytes unchanged at every size.
6. **Paper (§5, proposals only).** Every Binius64 cell of the inline table, its before/after
   caption, the "4 KB / 2^6" prose and the intro-table row depend on the crippled build. A fair
   replacement table for 2^4–2^7 and 2^10 (48 new cases run tonight through the official runner)
   and replacement prose are drafted; F2Z keeps the proof-size (3×) and witgen (3–4×) wins and
   prover parity up to 2 KB, and loses the prover from 4 KB and the verifier everywhere.

## 1. The A/B grid and the confirmed root cause

### 1.1 Mechanism

Cargo reads `[profile.*]` **only from the workspace-root manifest**; a profile section in any
dependency is ignored. `benchmarks/binius64/Cargo.toml` declares `[workspace]`, so the worker is
its own root: Binius64's shipped profile (`lto = "thin"`, `debug = true` in the fork's
`Cargo.toml` at `bc73510`) never applied to it, and neither did f2z-pcs's fat profile. Until
`bec4835` (2026-09-13) the worker's own profile section set only `debug = 1`, so Cargo's release
defaults applied: `lto = false` and `codegen-units = 16`. `lto = false` is not "thin LTO": rustc
is invoked with `-C embed-bitcode=no`, so there is **no cross-crate LTO at all** (only rustc's
crate-local ThinLTO across the 16 codegen units of each crate). Binius64's field arithmetic lives
in `binius-field`/`binius-math` and is called from `binius-prover`, `binius-iop-prover` and
`binius-verifier`; without LTO those calls cross crate boundaries un-inlined.

There is no global override on the box: `~/.cargo/config.toml` does not exist, the repo and the
worker have no `.cargo/config.toml`, and the shell exports only `RUSTFLAGS=-C target-cpu=native`
and `CARGO_TARGET_DIR`.

Evidence from `cargo build -v` of the recorded 2026-09-10 worker source (the campaign runner keeps a
copy of the worker's `Cargo.toml`, `Cargo.lock` and `src/main.rs` in
`bench_results/sha256-ecdsa-i7-f2z-binius/source/benchmarks/binius64/`; the lock's SHA-256 is the
recorded `fdaeaeae…`), rebuilt in isolated target directories:

| variant | rustc flags of every rlib (e.g. `binius_prover`) | rustc flags of the final binary |
|---|---|---|
| as recorded 2026-09-10 (`debug = 1` only) | `-C codegen-units=16 -C embed-bitcode=no -C opt-level=3 -C target-cpu=native` | `-C codegen-units=16 -C embed-bitcode=no` |
| Binius64's own profile (`lto = "thin"`) | `-C codegen-units=16 -C linker-plugin-lto` | `-C codegen-units=16 -C lto=thin` |
| f2z-pcs profile (`lto = true`, `codegen-units = 1`) | `-C codegen-units=1 -C linker-plugin-lto` | `-C codegen-units=1 -C lto` |

### 1.2 Grid

Same source, same lockfile, same fixture, same rustc; only the profile changes. "old" is the
recorded 2026-09-10 worker (no `f2z` dependency, lock `fdaeaeae…`); "cur" is the worker at HEAD
`c0751bf` (path dependency on `f2z`, lock `aece839d…` = the suite's). Profiles are applied with
`CARGO_PROFILE_RELEASE_LTO` / `CARGO_PROFILE_RELEASE_CODEGEN_UNITS` (equivalent to
`--config profile.release.lto=…`), each into its own `CARGO_TARGET_DIR`; no tracked file was
edited. Binius64 BaseFold, rate 1/2, 2^7, target 100, medians of 5 after one warm-up, 15 s
cool-down between cells, `RAYON_NUM_THREADS = HARDWARE_CONCURRENCY = threads`. The `suite` rows
are the exact binaries the 2026-09-13 suite ran (`910e6ddc…` worker, `63505e6c…` F2Z bench).

| source, lock | profile (LTO / CGUs) | thr | setup | witgen | commit | PIOP | opening | **prove** | **verify** | proof digest (BLAKE3) |
|---|---|---:|---:|---:|---:|---:|---:|---:|---:|---|
| old (2026-09-10), `fdaeaeae` | none / 16 — as recorded 2026-09-10 | 1 | 391 | 10.0 | 59.3 | 898.9 | 48.9 | **1007.4** | **150.5** | `dbec3448…` |
| old | thin / 16 — Binius64's own | 1 | 414 | 8.9 | 13.1 | 146.3 | 9.9 | **169.3** | **13.35** | `dbec3448…` |
| old | fat / 1 — f2z-pcs | 1 | 389 | 8.7 | 13.3 | 142.2 | 9.5 | **165.1** | **12.71** | `dbec3448…` |
| cur (HEAD `c0751bf`), `aece839d` | none / 16 — Cargo default | 1 | 436 | 10.0 | 59.3 | 906.9 | 49.0 | **1015.2** | **148.8** | `dbec3448…` |
| cur | none / 1 | 1 | 417 | 9.6 | 58.8 | 892.3 | 49.2 | **1000.8** | **153.1** | `dbec3448…` |
| cur | thin / 16 — Binius64's own | 1 | 423 | 8.8 | 13.1 | 145.7 | 9.9 | **168.7** | **13.35** | `dbec3448…` |
| cur | fat / 16 | 1 | 418 | 8.8 | 13.1 | 143.4 | 10.3 | **166.8** | **12.71** | `dbec3448…` |
| cur | fat / 1 — f2z-pcs | 1 | 405 | 8.6 | 13.3 | 143.5 | 9.6 | **166.4** | **12.76** | `dbec3448…` |
| suite binary `910e6ddc` (fat / 1) | re-run tonight | 1 | 401 | 8.5 | 13.1 | 142.0 | 9.6 | **164.8** | **12.71** | (no digest line; 393,776 B) |
| old | none / 16 | 10 | 399 | 11.0 | 15.3 | 214.0 | 13.3 | **242.8** | **57.5** | `dbec3448…` |
| old | thin / 16 | 10 | 393 | 10.3 | 3.6 | 54.5 | 4.1 | **62.2** | **6.46** | `dbec3448…` |
| old | fat / 1 | 10 | 376 | 10.1 | 3.4 | 53.5 | 4.3 | **61.1** | **6.27** | `dbec3448…` |
| cur | none / 16 | 10 | 407 | 10.4 | 15.2 | 206.9 | 13.2 | **238.0** | **54.7** | `dbec3448…` |
| cur | none / 1 | 10 | 399 | 10.7 | 15.2 | 209.5 | 13.5 | **237.9** | **61.6** | `dbec3448…` |
| cur | thin / 16 | 10 | 398 | 10.3 | 3.4 | 54.2 | 4.1 | **61.9** | **6.52** | `dbec3448…` |
| cur | fat / 16 | 10 | 392 | 10.2 | 3.4 | 53.8 | 4.2 | **61.3** | **6.31** | `dbec3448…` |
| cur | fat / 1 | 10 | 381 | 10.0 | 3.4 | 52.9 | 4.1 | **60.4** | **6.25** | `dbec3448…` |
| suite binary (fat / 1) | re-run tonight | 10 | 378 | 10.1 | 3.4 | 54.7 | 4.1 | **62.6** | **6.24** | — |

Rate 1/8 (the paper's other Binius64 row), current source: none / 16 → 1241.5 / 148.6 ms at 1 thread and
277.1 / 56.9 ms at 10 threads; fat / 1 → 214.0 / 12.65 ms and 70.5 / 6.18 ms; digest `467c0a5f…` in all four.

The F2Z bench binary (`benches/sha256_ecdsa_compare.rs`, worktree of HEAD, same three profiles;
F2Z Split, rate 1/2, same fixture):

| F2Z build | thr | outer | inner | combine | evaluate | opening | **prove** | **verify** |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| none / 16 | 1 | 3.7 | 122.4 | 22.2 | 11.8 | 53.1 | **214.6** | **45.6** |
| thin / 16 | 1 | 3.6 | 116.1 | 21.8 | 11.7 | 52.8 | **207.4** | **45.6** |
| fat / 1 | 1 | 3.4 | 116.9 | 20.7 | 11.0 | 51.8 | **206.1** | **44.0** |
| suite binary `63505e6c` (fat / 1) | 1 | 3.3 | 116.3 | 20.5 | 10.9 | 51.8 | **204.1** | **43.9** |
| none / 16 | 10 | 4.2 | 29.3 | 6.0 | 3.0 | 25.7 | **70.0** | **15.6** |
| thin / 16 | 10 | 4.1 | 25.9 | 5.8 | 3.0 | 25.3 | **65.6** | **15.6** |
| fat / 1 | 10 | 3.8 | 27.9 | 5.7 | 2.8 | 25.4 | **66.1** | **15.0** |
| suite binary (fat / 1) | 10 | 3.6 | 25.9 | 5.7 | 2.8 | 25.5 | **65.0** | **15.1** |

### 1.3 Reading: confirmed root cause

- **Cross-crate LTO is the whole effect.** With the current source and lockfile, the Cargo-default
  profile (no cross-crate LTO) gives 1015 / 149 ms (prover / verifier, 1 thread) — the
  2026-09-10 campaign's 1019 / 148 within 0.5 % — and 238 / 55 ms at 10 threads (campaign: 238 /
  55.5). Thin LTO, Binius64's own shipped profile, recovers all but 1–5 % (169 / 13.4 ms); fat
  LTO with 16 and with 1 codegen unit are within noise of each other (166.8 / 12.7 vs 166.4 /
  12.8) and of the suite binary (164.8 / 12.7). Per phase, old → fair: commit 59 → 13 ms (4.5×),
  PIOP 907 → 144 ms (6.3×), opening 49 → 9.6 ms (5.1×), verify 149 → 12.8 ms (11.7×); setup and
  witness generation unchanged — exactly the campaign-to-campaign deltas the prompt lists.
- **Codegen units alone change nothing.** `none / 1` equals `none / 16` (1001 vs 1015 ms prover,
  153 vs 149 ms verifier); `fat / 16` equals `fat / 1`. The 16-CGU default is not the trap;
  the absent `lto` key is.
- **The dependency and lockfile change contributed nothing.** The current source (with the
  `f2z` path dependency and the new lock) reproduces the old numbers under the old profile, and
  the recorded old source under fat LTO gives the fair numbers (rows "old" in §1.2: 1007 / 150 ms under the recorded profile, 169 / 13.4 ms under thin LTO, 165 / 12.7 ms under fat LTO — the same three numbers as the current source, and the same proof digest `dbec3448…`).
- **Proof bytes are identical across every build and thread count** (BLAKE3 `dbec3448…` at
  rate 1/2, `467c0a5f…` at rate 1/8, 393,776 / 289,200 bytes) — the profile changes speed only.
- **F2Z is nearly insensitive to the same trap** (+4 % prover, +3.5 % verifier without LTO at 1
  thread; +6 % at 10 threads): its hot kernels are crate-local or generic and monomorphized at the
  call site, whereas Binius64's field arithmetic (`binius-field`, `binius-math`) is called across
  crate boundaries from `binius-prover`/`binius-iop-prover`/`binius-verifier`. That asymmetry is
  why the 2026-09-10 table flipped: the crippled build cost Binius64 6×, F2Z 4 %.
- **Tonight's re-runs of the suite binaries agree with the suite's recorded medians within 2 %**
  (Binius64 164.8 / 12.7 vs 167.5 / 12.8; F2Z 204.1 / 43.9 vs 206.8 / 44.7 at 1 thread), so the
  suite's SHA+ECDSA numbers stand.

## 2. Audit of the other benchmark builds

Method: for every scheme in a paper table, the manifest that is the *workspace root* of its build (Cargo reads
`[profile.*]` only from the workspace root; a dependency's profile section is ignored), the command that built it,
and the `rustflags` recorded in the artifact's Cargo fingerprint (`target/release/.fingerprint/<crate>-*/*.json`).

| Scheme / binary | Where it is built | Effective release profile | `-C target-cpu=native` evidence | Tables | Verdict |
|---|---|---|---|---|---|
| F2Z (every table) | f2z-pcs root; `cargo bench` (profile.bench) or `cargo build --release` (profile.release) | fat LTO, 1 CGU (bench profile adds `debug = 2`) | fingerprints in `~/zinc-plus/target/release/.fingerprint` | all | fair |
| Binius64, in-process adapters (`benches/mul_e2e_compare`, `hybrid_u32_sha256`, `sha256_e2e_compare`, `*_pcs_compare`) | f2z-pcs root, `cargo bench` | fat LTO, 1 CGU (above Binius64's own `lto = "thin"`, 16 CGUs) | `binius-prover` fingerprint: `-Ctarget-cpu=native` | native-mul u32/u64/u128, hybrid (both variants), SHA-256 compare | fair; the grid below quantifies thin vs fat |
| Binius64 + F2Z opener rows (same binaries as above; SHA+ECDSA rows in the worker) | as the row above / as the worker | as the row above / as the worker | idem | native-mul, hybrid, SHA+ECDSA | fair since 2026-09-13 |
| **Binius64 SHA+ECDSA worker**, `benchmarks/binius64` (own workspace) | `benchmarks/binius64/build.py`: `cargo build --release --locked` | **2026-09-10/11 campaigns: `[profile.release] debug = 1` only → Cargo defaults: `lto = false` (crate-local thin LTO), 16 CGUs.** Since `bec4835` (2026-09-13): fat LTO, 1 CGU | `.build.json` sidecar: `rustflags = -C target-cpu=native` in both campaigns | `tab:sha256-ecdsa-f2z-opt` (main.tex), stale generated `paper/sha256-ecdsa-table.tex` | **crippled before 2026-09-13**; the recorded manifest copy is in `bench_results/sha256-ecdsa-i7-f2z-binius/source/benchmarks/binius64/Cargo.toml` |
| Limber, native-mul tables | in-process git dependency (`861f10a`) of f2z-pcs, `cargo bench` | fat LTO, 1 CGU (root profile; Limber's own `lto = "fat"` is ignored as a dependency but coincides) | `limber` fingerprint: `-C target-cpu=native` | native-mul u32/u64 (Limber rows) | fair |
| Limber, MultiSwap historical row | `~/limber-impl` @ `b003684`, `cargo bench --bench multiswap_modp` (own workspace root) | its own `[profile.release] lto = "fat"` (CGUs default 16; bench profile inherits release) | fingerprint: `-C target-cpu=native` | MultiSwap table | fair (fat LTO; CGU count is second-order under fat LTO, see grid) |
| Zinc+, MultiSwap historical row | `~/zinc-plus` @ `878fbd8`, `cargo bench --bench limber_multiswap` | its own `lto = true`, `codegen-units = 1` | README command sets `RUSTFLAGS="-C target-cpu=native"` | MultiSwap table | fair |
| Plonky3 FRI | in-process git dependencies of f2z-pcs, `cargo bench` | fat LTO, 1 CGU | `p3-fri` fingerprint: `-C target-cpu=native` | native-mul u32/u64 | fair |
| Spartan2 fork (`spartan-mc`) | in-process dependency, `cargo bench` | fat LTO, 1 CGU | `spartan2` fingerprint: `-C target-cpu=native` | historical SHA+ECDSA docs only | fair |
| fields-witch (`~/fields-witch` @ `30cca8c`; the opener variant and the asm control in `~/fields-witch-f2z/target-{f2z,asm}`) | own workspace root, `cargo build --release --examples` | its own `lto = "thin"`, `codegen-units = 1` (the author documents fat LTO as a regression of its SHA-NI backend) | fingerprints in both target dirs: `-C target-cpu=native`; identical profile hash per unit kind in `target-asm` and `target-f2z` | fields-witch table | fair under "upstream default"; not fat+1 by the author's choice |
| zkpassport worker (`benchmarks/zkpassport`) | own workspace root | `opt-level = 3`, `lto = "thin"` (16 CGUs) | no binary present on the box | none (method deprecated) | n/a |
| f2z-p3-bridge (`~/f2z-p3-bridge`) | own workspace root | fat LTO, 1 CGU | docs only | none | n/a |

Two checks worth recording: the two fields-witch build directories (`target-f2z`, the opener
variant; `target-asm`, the assembly-SHA control) have identical Cargo profile hashes per unit
kind, so both ran under the same thin + 1 CGU profile with different features only; and no
`~/.cargo/config.toml`, repo or worker `.cargo/config.toml` exists, so nothing outside the
manifests shaped any build (`RUSTFLAGS=-C target-cpu=native` and `CARGO_TARGET_DIR` are the
shell's only exports).

## 3. Recommended build policy

**Policy: every scheme is built the way f2z-pcs builds itself — `opt-level = 3`, fat LTO
(`lto = true`), `codegen-units = 1`, `-C target-cpu=native` — from a workspace-root manifest that
states that profile explicitly, and the effective rustc flags are recorded in the run's
provenance.**

Why this and not "each at its upstream default":

- It is what every in-process competitor (Binius64 adapters, Limber, Plonky3, Spartan2) already
  gets today, so it changes no existing table except the SHA+ECDSA one.
- The grid shows fat LTO + 1 CGU is at least as favourable to Binius64 as its own shipped
  profile (thin LTO, 16 CGUs) on this workload, so the policy never handicaps the competitor.
- "Upstream default" is fragile: it is whatever a dependency's manifest says, which Cargo
  silently discards for non-root packages — exactly the trap that produced the 6x error.
- One documented exception: fields-witch's author states that fat LTO regresses its SHA-NI
  backend, so its rows keep its own `lto = "thin"` + `codegen-units = 1`; the caption must say
  "built with its own release profile". Under a strict reading of the policy those rows would be
  re-measured at fat LTO, which by the author's note would only make fields-witch slower.

Implementation items (proposed, not done here):

1. `benchmarks/binius64/build.py` records the effective profile in `.build.json` (parse the
   `-C lto`, `-C codegen-units`, `-C opt-level`, `-C embed-bitcode` flags of the final binary from
   `cargo build -v`, or the manifest's `[profile.release]` plus any `CARGO_PROFILE_RELEASE_*`
   override), and `scripts/run_sha256_ecdsa_compare.py` refuses a worker whose recorded profile is
   not fat LTO + 1 CGU. Today the sidecar records only `rustflags`, which is why the trap was
   invisible in the manifests.
2. Every out-of-crate worker manifest carries the profile block with a comment (done for
   `benchmarks/binius64` in `bec4835`; `benchmarks/zkpassport` still says `lto = "thin"` — fix it
   or delete the deprecated worker).
3. Captions of tables with external binaries state the build policy in one clause ("every scheme
   built with fat LTO, one codegen unit and `-C target-cpu=native`").

Tables and numbers that change under the policy:

| Artifact | Status | What changes |
|---|---|---|
| `paper/main.tex`, inline table `tab:sha256-ecdsa-f2z-opt` (lines ~2926–2961) | **crippled Binius64 rows** (2026-09-11 campaign, worker without LTO): all 24 Binius64 cells; the F2Z cells are pre-suite (`fac8c8f`-level, rate 1/8 only) | replace with the fair table (§5.3); Binius64's 1-thread prover goes ~1019 → 168 ms, verifier ~150 → 13 ms at 8 KB; F2Z no longer wins the prover or verifier columns |
| `paper/sha256-ecdsa-table.tex` (generated 2026-09-10, not `\input`) | stale on both sides (F2Z pre-`fac8c8f`, Binius64 crippled) | regenerate from the fair directories or delete |
| `paper/native-mul*-table.tex`, `hybrid-table*.tex`, `u32-mul-table.tex`, `raw-performance-table.tex`, `multiswap-table.tex` | already fat LTO + 1 CGU on every scheme | unchanged |
| `paper/fields-witch-table.tex` | fields-witch at its own thin + 1 CGU profile (documented exception) | unchanged; caption clause |
| memory `f2z-sha256-ecdsa-opt.md`, `f2z-sha256-ecdsa-i7-campaign.md` | quote the crippled Binius64 numbers (1019/148, 917–1262/130–150, 214–289/51–57 ms) | superseded by this document; the `f2z-bench-suite-methodology.md` BUILD-PROFILE TRAP entry is updated below |

## 4. Where F2Z now loses: F2Z vs Binius64 with fair builds

All numbers below are from fair builds (fat LTO, 1 CGU, `-C target-cpu=native` on both sides):
the 2026-09-13 suite at 2^7 (`bench_results/suite-sha256-ecdsa-20260913`, reps 3), the
same binaries re-run in this session (§1.2 `suite` rows, reps 5), the fair campaign at
2^4, 2^5, 2^6 and 2^10 run in this session with the same binaries through the official runner
(`bench_results/sha256-ecdsa-fair-20260913-i4-6-10`, reps 3, both rates, 1 and 10 threads,
48 cases, all verified), and the span/sample profiles of §4.3–4.4.

### 4.1 The 2^7 breakdown (rate 1/2, ms, medians; 1 thread | 10 threads)

F2Z Split, per-step scopes (`phases_seconds` / `verify_phases_seconds` of
`benches/sha256_ecdsa_compare.rs`; the nested opener scopes are inclusive):

| F2Z prover step | 1 thr | 10 thr | | F2Z verifier step | 1 thr | 10 thr |
|---|---:|---:|---|---|---:|---:|
| prover total (commit + protocol) | **206.8** | **65.8** | | verifier total | **44.7** | **15.2** |
| `ecdsa:outer_prove` (6,807 nonlinear P-256 rows) | 3.4 | 3.9 | | `ecdsa:coefficient_evaluate` (P-256 matrices at the inner point) | 30.1 | 6.8 |
| `ecdsa:coefficient_combine` (batched matrix MLE: 4.24 M-entry gather) | 21.0 | 5.7 | | `mv:rswitch` (ring-switch read-off) | 12.9 | 6.0 |
| `ecdsa:shared_inner_prove` (degree-2 inner sumcheck over 2^22 cells) | 117.8 | 26.7 | | ⤷ `mqv:vaprime` (dual-basis a′ over the source columns) | 10.1 | 1.9 |
| `ecdsa:coefficient_evaluate` (scale at the inner point) | 11.2 | 2.8 | | ⤷ `mqv:vwprep` (virtual column weights, serial) | 2.8 | 3.9 |
| `ecdsa:f2z_prove` (the F2Z opening) | 52.3 | 25.2 | | `ecdsa:matrix_projection` (3,358 residues) | 0.6 | 0.7 |
| ⤷ `mc:forest` (GKR forest; `mf:phaseA` 18.4 / 11.6, `eqf:rounds` 15.5 / 7.9) | 20.8 | 12.6 | | `mv:lig` (Ligerito verify) | 0.4 | 0.4 |
| ⤷ `mqv:hs` (ring-switch h-fold over the packed source) | 13.3 | 3.2 | | sumcheck rounds, grinding checks, rest | ~0.7 | ~1.4 |
| ⤷ `mqv:aprime` | 7.0 | 1.8 | | | | |
| ⤷ `mq:lig` (Ligerito open; `lig:grind_pow` 4.7 / 0.95) | 5.2 | 1.5 | | | | |
| ⤷ `mqv:wprep` (serial) | 2.8 | 4.0 | | | | |
| ⤷ `mqv:planes`, `mf:build_levels`, `mf:bitgen`, rest | 3.2 | 1.9 | | | | |
| `ecdsa:matrix_projection`, prime sampling, commit | 1.3 | 1.2 | | | | |

Binius64 (BaseFold, rate 1/2; the worker's four phases, plus §4.3 for the PIOP's sub-phases):

| Binius64 phase | 1 thr | 10 thr |
|---|---:|---:|
| prover total (commit + protocol) | **167.5** | **60.3** |
| commit witness (Merkle-committed Reed–Solomon codeword, SHA-256) | 13.3 | 3.3 |
| PIOP (IntMul, BinMul, BitAnd, shift reductions, ring switch) | 144.5 | 52.9 |
| opening (`[phase] PCS Opening` 4.7 / 1.0 + `[phase] Finish PCS` 5.0 / 3.0) | 9.6 | 4.1 |
| verifier total | **12.8** | **6.2** |
| witness generation (+ packing) | 9.4 | 10.3 |

Circuits: F2Z `nonlinear_rows 6,807`, `linear_rows 24,831`, assignment `3,834,159` bits, source
`2,097,071` bits; Binius64 `gates 637,313`, `bitand 332,751`, `intmul 64,801` (+45 k BMUL),
FRI message 2^20, 241 queries. Proofs: F2Z 135.3 KB (rate 1/2) / 92.4 KB (rate 1/8), Binius64
393.8 / 289.2 KB; both schemes' proofs are byte-identical across thread counts.

### 4.2 Fixed P-256 cost versus per-compression SHA cost (1 thread unless stated)

Least-squares fit `time = fixed + N · per_compression` over `N ∈ {16, 32, 64, 128, 1024}` from
the fair campaigns (fit residuals at 2^7 are within 1 ms for F2Z and within 10 ms for Binius64,
whose commit/opening step up when the FRI message grows from 2^19 to 2^20 at 2^7):

| series | fixed (ms) | per compression (µs) | measured @ 2^4 | @ 2^7 | @ 2^10 |
|---|---:|---:|---:|---:|---:|
| F2Z prover, 1 thr | 137.7 | 538.6 | 146.0 | 206.8 | 689.2 |
| Binius64 prover, 1 thr | 149.1 | 67.9 | 147.7 | 167.5 | 217.8 |
| F2Z prover, 10 thr | 50.9 | 113.7 | 51.7 | 65.8 | 167.3 |
| Binius64 prover, 10 thr | 57.1 | 15.7 | 57.3 | 60.3 | 73.1 |
| F2Z verifier, 1 thr | 39.6 | 36.3 | 39.9 | 44.7 | 76.7 |
| Binius64 verifier, 1 thr | 11.1 | 11.0 | 11.3 | 12.8 | 22.4 |
| F2Z verifier, 10 thr | 14.0 | 9.0 | 14.2 | 15.2 | 23.2 |
| Binius64 verifier, 10 thr | 5.7 | 2.6 | 5.7 | 6.2 | 8.4 |
| F2Z `shared_inner_prove`, 1 thr | 66.5 | 400.8 | 72.0 | 117.8 | 476.9 |
| F2Z `f2z_prove` (opening), 1 thr | 34.7 | 135.2 | 37.4 | 52.3 | 173.2 |
| ⤷ `mc:forest`, 1 thr | 7.4 | 110.4 | 9.6 | 20.8 | 120.6 |
| F2Z `coefficient_combine` + `evaluate` (prover), 1 thr | 32.0 | ≈0 | 31.8 | 32.2 | 32.3 |
| F2Z verifier `coefficient_evaluate`, 1 thr | 29.5 | ≈0 | 29.4 | 30.1 | 30.1 |
| F2Z verifier `mv:rswitch`, 1 thr | 8.3 | 35.2 | 8.9 | 12.9 | 44.4 |
| Binius64 PIOP, 1 thr | 135.4 | 57.9 | 135.9 | 144.5 | 194.5 |

Reading:

- **The P-256 part costs the two provers the same.** F2Z's fixed cost (138 ms) is 8 % *below*
  Binius64's (149 ms) at 1 thread and 11 % below at 10 threads (51 vs 57 ms). At 2^4 the two
  provers tie at 1 thread (146 vs 148 ms) and F2Z is 10 % faster at 10 threads (51.7 vs 57.3 ms).
- **The SHA part is where F2Z loses the prover.** Each SHA-256 compression costs F2Z 0.54 ms
  against Binius64's 0.068 ms single-threaded (7.9×; 7.2× at 10 threads). Of F2Z's 0.54 ms,
  0.40 ms is the inner sumcheck (2 × 20,457 cell-visits per compression ≈ 9.8 ns each, i.e. about
  two 113-bit Montgomery multiplications per visit) and 0.135 ms is the opening (0.11 ms of it the
  GKR forest over the compression's 2^14 source bits). This is the cost of proving SHA-256 as
  integer R1CS over a 113-bit prime; Binius64 proves it over F_2 words. At 2^7 the SHA rows
  therefore cost F2Z 69 ms against Binius64's 9 ms, which is the 1-thread gap (207 vs 168 ms)
  almost exactly (the rest is the FRI-message step in Binius64's commit/opening at 2^7).
- **The verifier gap is one fixed term plus a smaller linear one.** Of the 32 ms gap at 2^7,
  28.5 ms is fixed: F2Z's verifier spends 30 ms evaluating the batched P-256 constraint-matrix
  MLE at the inner sumcheck point (`ModQCoefficients::evaluate_batched_matrix_mle`: a gather over
  the 4.24 M nonzero entries, one Montgomery multiplication and addition each, then a
  1.2 M-column dot product with two multiplications per column ≈ 6.7 M Montgomery
  multiplications ≈ 4.5 ns each), the Spartan verifier's O(nnz) term, against Binius64's whole
  fixed verifier of 11 ms — of which 12.0 of 12.6 ms at 2^7 is the same kind of term, the
  constraint-system ("wiring") evaluation at word granularity (§4.3), and only 0.4 ms is the
  BaseFold/FRI verification. The remaining 3.5 ms of the gap is linear:
  F2Z's ring-switch read-off (`mv:rswitch`, 35 µs/compression, dominated by `mqv:vaprime`)
  against Binius64's 11 µs/compression.
- **The prover's fixed part has the same O(nnz) gathers.** `coefficient_combine` (21 ms) builds
  the same batched matrix MLE the verifier evaluates, and `coefficient_evaluate` (11 ms) is the
  same tail dot product; together 32 ms of the prover's 138 ms fixed cost.
- **10 threads.** F2Z's fixed prover cost parallelizes 2.7× (138 → 51 ms) and Binius64's 2.6×
  (149 → 57 ms); F2Z's per-compression cost parallelizes 4.7× and Binius64's 4.3×. F2Z's residual
  serial work at 10 threads is `ecdsa:outer_prove` (3.9 ms), `mqv:wprep` (4.0 ms, *slower* than
  at 1 thread) and the forest's `mf:phaseA` (11.6 ms, only 1.6× faster than at 1 thread); those
  three are 30 % of the 10-thread prover. On the verifier `mqv:vwprep` (3.9 ms) is 26 % of the
  10-thread verifier.

### 4.3 Inside Binius64's PIOP (tracing spans, `benchmarks/binius64-prof` copy of the worker)

A copy of the worker whose tracing layer times *every* span (the shipped worker keeps only four
phases); inclusive milliseconds, medians of 5, 2^7, rate 1/2. Nested spans overlap, so the rows
are not additive; the indentation follows Binius64's own `[phase]` structure.

| Binius64 prover span | 1 thr | 10 thr | note |
|---|---:|---:|---|
| `Prove` (total) | 165.2 | 62.6 | |
| `[phase] IntMul check` | 89.5 | 39.0 | the 64,801 P-256 multiplications: `Build IntMul witness` 23.6 / 9.3 (exponentiation leaves), `prover_phase_1` 22.7 / 5.9 (`build_g` 22.4), `Compute variable-base prodcheck layers` 20.2 / 6.1, `Batched selector + C-root sumcheck` 20.0 / 6.8, `logup* transparent (committed)` 17.2 / 9.1, `logup* fracadd reduction` 12.3 / 7.1, `prove_phase_2` 11.8 / 3.9, `Combined GKR` 10.5 / 6.0 |
| `[phase] Shift Reduction` | 35.0 | 10.3 | the shifted-operand wiring over all 637 k gates: `Assemble columns` 8.9 / 2.0, `build_monster_segments` 6.2 / 1.8, `run_sumcheck` 5.0 / 2.1, `Compute univariate round message` 4.6 / 1.1 |
| `[phase] BitAnd check` | 15.1 | 4.0 | 332,751 AND constraints (univariate-skip zerocheck) |
| `Commit witness` | 13.3 | 3.4 | `Reed–Solomon encode` 7.4 / —, `Merkle commit` 8.8 (`hash_leaves` 7.3, portable 4-way SHA-256, not the ARM SHA-2 instructions) |
| `[phase] Finish PCS` + `[phase] PCS Opening` | 5.0 + 4.6 | 3.0 + 1.0 | `BaseFold opening` 5.0, `Basefold MLE-check` 2.4, `Compute ring-switching partial evaluations` 3.1, `FRI Initial Fold` 1.2 |
| `[phase] BinMul check` | 2.8 | 1.1 | 45 k GHASH-field multiplications (the P-256 select lowering) |

At rate 1/8 (the paper's second Binius64 row) only `Commit witness` changes: 50.8 ms at 1 thread
(`Merkle commit` 34.9, `hash_leaves` 28.5, `Reed–Solomon encode` 27.3) — the rate-1/8 codeword
is 4× longer, and the Merkle hashing is portable SHA-256; `IntMul check` 98.5, `Finish PCS` 8.7.

The **IntMul reduction is 54 % of Binius64's PIOP** and is entirely the P-256 gadget's cost
(64,801 IntMul constraints, the same at every message size); the SHA-256 compressions enter only
through the shift and AND reductions, which is why Binius64's per-compression cost is 0.068 ms.
For F2Z the corresponding fixed cost is the 6,807 nonlinear rows' outer sumcheck (3.4 ms) plus the
P-256 share of the inner sumcheck and of the two O(nnz) gathers.

With the F2Z opener in place of BaseFold (`binius64-ligerito`, rate 1/2, 1 thread): prover 173.8
= `Commit oracles` 14.1 + `PIOP prefix` 143.1 + `Opening` 16.4; verifier 14.1 ms. The PIOP prefix
is the same 143 ms; the opener adds 7 ms of opening (Round 0 + two Johnson openings with grinding)
against BaseFold's 9.6 ms opening + cheaper commit, and its verifier is 1.4 ms slower than
BaseFold's. The Binius64-with-F2Z-opener rows therefore inherit Binius64's PIOP cost structure
exactly; they are not a path to closing F2Z-SNARK's own gaps.

**Binius64's verifier.** Its named sub-phases (`[phase] Verify IntMul/BinMul/BitAnd/Shift
Reduction`, `Verify Public Input`, `Verify PCS Opening`) total only 0.2 ms of the 12.8 ms: the
`[phase] Verify PCS Opening` span closes after the ring-switch check, before the deferred
BaseFold verification runs in `channel.finish()`, and the reductions' spans do not include the
wiring evaluation. The simple `Verifier::verify` wrapper runs `verify_statement` — the PIOP
sumcheck verifications, then `WiringEvalClaim::check_native()`, which by Binius64's own doc
comment "walks every constraint of the system" (the constraint-system wiring multilinear evaluated
at the challenge point, in GF(2^128) at word granularity: about 1.2 M operand references for the
637 k gates) — and then `channel.finish()` (241 FRI queries with SHA-256 Merkle paths and the
folding checks). Timed separately in the profiling copy (the worker's `verify` re-implemented as the three calls the simple wrapper makes, each timed):

| Binius64 verifier part (2^7, ms, medians of 5) | 1 thr, rate 1/2 | 1 thr, rate 1/8 | 10 thr, rate 1/2 | 10 thr, rate 1/8 |
|---|---:|---:|---:|---:|
| PIOP verification up to the wiring claim (`IOPVerifier::verify`: the IntMul/BinMul/BitAnd/shift sumcheck verifications) | 0.20 | 0.20 | 0.24 | 0.24 |
| wiring evaluation (`WiringEvalClaim::check_native`: the constraint-system multilinear at the challenge point) | **12.02** | **12.03** | **5.47** | **5.53** |
| BaseFold finish (`channel.finish()`: ring switch, 241 / 121 FRI queries, Merkle paths, folds) | 0.43 | 0.33 | 0.51 | 0.39 |
| total `verify` | 12.65 | 12.56 | 6.23 | 6.16 |

Binius64's verifier is therefore **95 % the wiring evaluation** — the same kind of term as F2Z's
`coefficient_evaluate`, and it parallelizes the same way (12.0 → 5.5 ms at 10 threads, F2Z's
30 → 6.8). The FRI/BaseFold verification is 0.3–0.5 ms at either rate.

So Binius64's verifier is also linear in the circuit, like F2Z's; the difference is the constant:
word-granular operands in GF(2^128) (12 ms) against F2Z's 4.24 M bit-level nonzero entries with
113-bit modular multiplications (30 ms), i.e. roughly 3.5× fewer entries and cheaper arithmetic
per entry. The "holographic" lever V3 would apply to Binius64 just as well; neither system
preprocesses its constraint system today.

### 4.4 Function-level attribution (macOS `sample`, 1 ms, 1 thread; share of busy samples)

F2Z (suite binary, `f2z-split`, rate 1/2, 40 proofs + verifications; 6,715 busy samples): the
inner sumcheck's factored prefix pass `inner_sumcheck::accumulate_factored_instance<4>` 16.2 %,
`inner_reduction::ModQCoefficients::p256_column_weight` 16.0 % (the O(nnz) gather — the prover's
`coefficient_combine` and the verifier's `coefficient_evaluate` share it), `accumulate_suffix<4>`
7.9 %, `MontyLinearAccumulator128::reduce_raw` 6.2 %, `extend_lsb_axis` 4.7 % + 1.8 %, the
parallel tail dot of `BatchedMatrixMle::evaluate` 4.3 %, `ligerito_flock::hs_scatter_block16`
4.1 %, **`crypto_bigint MontyParams::eq` 3.5 %** (the per-element field-configuration check the
inner prover performs on every coefficient it reads — a pure overhead worth hoisting; ≈7 ms at
1 thread), `folded_packed_h<4>` 3.2 %, the verifier's `evaluate_batched_matrix_mle` tail 2.9 %,
`blake3x4::first_pow_nonce` 1.8 % (grinding), `Sha256EcdsaMap::add_extra_a_prime` 1.8 %,
`VirtColumnWeights::new` 1.7 %, `fold_prefix_table` 1.3 %, the GKR forest's `eq_factored` kernels
(`dense_fused_fold_round_slices`, `dense_msg_pass_d_slices`, `dense_grid_pass_slices`,
`leaf3_round3_msg`) ≈ 2.5 % together, `dual_basis_linear_combination` 0.9 %. In sum: inner
sumcheck ≈ 46 %, the two O(nnz) gathers ≈ 19 %, the opening ≈ 20 %, consistent with the scopes.

Binius64 (fat-LTO worker, BaseFold, rate 1/2, 50 proofs + verifications; 6,655 busy samples):
the IntMul reduction's fold closures 12.4 % + 6.5 %, `witness::compute_b_leaves_parallel` 8.9 %
(the IntMul exponentiation witness), `Ghash128b::fold_with` 7.6 % (GF(2^128) folds), the
MLE-check `execute` 7.0 % + 1.2 %, `Word::eval_operand` 4.5 % (shifted-operand evaluation), the
"monster" segment construction 3.4 %, `GaoMateerPreExpanded` NTT 3.3 %, `BitSelector` 2.9 %,
portable `sha256_multi<4>` 2.7 % (Merkle leaves), `MleCheckRoundEvaluator::accumulate` 2.1 % +
1.1 %, `ParallelPseudoCompression::parallel_compress` 1.0 %, `Sha256HashSuite` compress 1.5 %,
the wiring `call_native` ≈ 1.3 %. Nothing in Binius64's profile is a single dominant kernel; the
P-256 IntMul machinery is spread over a dozen functions.

F2Z at 10 threads (80 proofs; 21,768 busy samples on 10 threads): `p256_column_weight` 14.5 % and
`accumulate_factored_instance` 14.1 % lead again, `evaluate_batched_matrix_mle` 3.6 %,
`build_batched_matrix_mle` 2.9 %, `first_pow_nonce` 2.1 %; of the 41.5 k thread-samples,
19.8 k are idle waits (`__psynch_cvwait`, `swtch_pri`): the pool is idle almost half of the
time, waiting on the serial sections (`outer_prove`, `mqv:wprep`, the forest's phase A) and on
short parallel regions with rayon dispatch overhead.

### 4.5 Levers (what could close the gaps)

Related to the levers recorded in memory (`f2z-sha256-ecdsa-opt.md`: the interned projection
and the raw-Montgomery gathers are in; "what is left" was the composite inner sumcheck from 2^9
up, the opening at ~0.17 ms/compression, and `mv:rswitch` as the verifier's linear term). The
fair data refine that list: the verifier's *fixed* term — the P-256 matrix evaluation — was not
on it and is now the largest single gap.

| # | lever | side | size of the target at 2^7 (1 thr / 10 thr) | measured? |
|---|---|---|---|---|
| V1 | `evaluate_assignment_tail`: apply the high-half equality factor once per group of `L = 2^11` consecutive assignment indices instead of once per index (one Montgomery multiplication per index instead of two); proof bytes unchanged (verifier arithmetic reordering; the prover's `scale` is the same field element) | verifier + prover `coefficient_evaluate` | 1.2 M multiplications of the 6.7 M in the verifier's 30 ms; ≈5 ms / ≈1 ms | **yes, §4.6: verifier −8.5 %, prover −3.6 % at 1 thread; −4.6 % / −1.7 % at 10 threads; proofs unchanged** |
| V2 | fewer nonzero entries in the P-256 gadget (4.24 M entries in 7,061 rows: a 494 k, b 2.17 M, c 1.58 M; 3,358 distinct coefficients, 45 % of entries 129–256 bits — the vendored `crates/circuit` bignum rows) | verifier fixed 30 ms and prover fixed 32 ms (`combine` + `evaluate`) | proportional: halving nnz saves ≈15 ms verifier and ≈10 ms prover at 1 thread | no (circuit-crate change) |
| V3 | holographic evaluation of A, B, C (commit to the matrices once; Spark-style sparse evaluation with a preprocessing step) | verifier | removes the whole fixed 30 ms term (verifier ≈ 15 ms at 1 thread, on par with Binius64's 13) at the price of a preprocessing phase and extra prover openings | no (protocol change; paper-level) |
| V4 | parallelize or hoist `mqv:vwprep` (serial; 2.8 ms at 1 thread, 3.9 ms at 10) and `ecdsa:outer_prove` (3.4 / 3.9 ms) | verifier and prover at 10 threads | ≈8 ms of the 10-thread prover (12 %), 4 ms of the 10-thread verifier (26 %) | no |
| **V0** | **adopt the vendored crate's Wengert tape** (`crates/circuit/src/matrix_wengert.rs`: the P-256 circuit's linear arithmetic recorded as a DAG; `PreparedWengertEvaluator::apply` computes `r·(A + xB + x²C)` over all 1,215,663 columns by reverse mode, with the 2^i bit lifts as doubling "power groups") in place of the expanded 4.24 M-entry gather of `ModQCoefficients::build_batched_matrix_mle` / `evaluate_batched_matrix_mle`; F2Z uses only the crate's witness-generation products today | prover `coefficient_combine` and the gather inside the verifier's `coefficient_evaluate` | **measured on the crate's own bench (§4.7): 66,412 nodes, 97,986 edges, 259 reverse levels, 1,029 coefficients; apply 2.71 ms at 1 thread, 0.82 ms at 10, against 21 / 5.7 ms today**; with V1 for the remaining tail dot the verifier's 30 ms term becomes ≈8 ms | tape: yes; integration: no (plan in §4.7) |
| P0 | hoist the per-element field-configuration check of the inner prover (`crypto_bigint MontyParams::eq`, 3.5 % of the 1-thread samples in §4.4: every coefficient the prefix/fold passes read is compared against the shared runtime configuration) | prover, all thread counts | ≈7 ms of the 1-thread prover (3 %) | no (sample evidence only) |
| P1 | the inner sumcheck's SHA share: 0.40 ms/compression ≈ 2 Montgomery multiplications per cell-visit; the K = 4 ternary prefix pass, shared block extensions and zero skipping are already in (memory Tier 3; K = 3 ≈ K = 4 measured) | prover, 1 thread | the 8× per-compression gap cannot be closed inside the 113-bit prime-field arithmetization; the paper's answer is the hybrid SNARK (SHA over F_2, §"Mod 2^32 multiplication and SHA-256 hashing") | scaling measured (§4.2) |
| P2 | the forest's `mf:phaseA` scaling (18.4 → 11.6 ms) and the opening's fixed 35 ms (Round 0 + ring switch + Ligerito with grinding) | prover, 10 threads | 12 ms of the 10-thread prover | no |

### 4.6 Lever V1 measured: one multiplication per tail index

Change (one function, `evaluate_assignment_tail` in `src/piop/spartan/ecdsa_sha256/inner_reduction.rs`,
in a worktree of HEAD; not applied to the main checkout): the equality weight of assignment index
`i` is `low[i mod L] · high[i / L]` with `L = 2^11`; instead of forming it per index, the tail
dot product accumulates `Σ low[i mod L] · v_i` over each aligned group of `L` indices and
multiplies by `high[i / L]` once per group (about 600 groups for the 1.2 M-cell P-256 tail;
rayon over groups). The verifier and the prover's `scale` evaluation share the function; the
result is the same field element, so proof bytes are unchanged.

Checks: the ECDSA module tests pass (9/9, including `streamed_matrix_evaluation_matches_prepared_mle`,
which compares the verifier's streamed evaluation with the prover's materialized MLE), the
transcript pin `sha256_ecdsa_2p3_split_transcript_is_pinned` passes, and the A/B below produced
byte-for-byte equal proof sizes (135,337 / 92,433 bytes) with every proof verifying.

A/B on the suite fixture (2^7, fat LTO, interleaved pristine/lever runs, 2 × 5 samples per cell,
15 s cool-downs; ms):

| build | thr | rate | prover | `coefficient_evaluate` (prover) | verifier | `coefficient_evaluate` (verifier) | `mv:rswitch` |
|---|---:|---|---:|---:|---:|---:|---:|
| HEAD | 1 | 1/2 | 205.4 | 10.9 | 44.17 | 29.7 | 12.8 |
| **lever** | 1 | 1/2 | **198.0** | **5.0** | **40.42** | **26.1** | 12.8 |
| HEAD | 1 | 1/8 | 201.8 | 10.9 | 43.75 | 29.5 | 12.8 |
| **lever** | 1 | 1/8 | **197.5** | **4.9** | **40.24** | **26.0** | 12.7 |
| HEAD | 10 | 1/2 | 65.1 | 2.8 | 14.98 | 6.8 | 5.8 |
| **lever** | 10 | 1/2 | **64.0** | **1.8** | **14.29** | **6.2** | 5.8 |
| HEAD | 10 | 1/8 | 64.8 | 2.8 | 14.87 | 6.8 | 5.8 |
| **lever** | 10 | 1/8 | **64.1** | **1.7** | **14.19** | **6.1** | 5.8 |

Result: the verifier's evaluation drops by 3.6 ms and the prover's by 5.9 ms at 1 thread —
verifier −8.5 % (44.2 → 40.4 ms), prover −3.6 % (205 → 198 ms); at 10 threads −4.6 % and −1.7 %.
It is a real, transcript-neutral lever, and it is small: the verifier's remaining 26 ms in this
scope are the 4.24 M-entry gather (`p256_column_weight`), which only V2 (fewer entries) or V3
(holography) can remove. Worth landing (the worktree diff is 34 lines), but it does not change
the paper's picture: with it, the 8 KB verifier is 40 vs 13 ms (3.1× instead of 3.5×).

### 4.7 The R1CS itself: the vendored crate already has the structured evaluator, and a plan

The P-256 gadget (`crates/circuit/src/p256.rs`, a port of Freigen's Lean circuit) keeps field
elements as lazy integer representatives over bit witnesses: every modular multiplication hints
521 bits (a 256-bit remainder and a 265-bit quotient), lifts them with `Σ 2^i b_i`, and asserts
one rank-1 row `x · y = r + p·q`. The `ConstraintGenerator` backend expands those lifts into
explicit matrix entries, which is where the 4.24 M nonzeros (a 494 k, b 2.17 M, c 1.58 M; 45 %
of them 129–256-bit coefficients such as `p·2^i`) come from. The same crate ships a second
backend, `matrix_wengert::WengertGenerator`, that records the circuit's Z-side linear
arithmetic as a DAG and evaluates `r · (A + xB + x²C)` by reverse mode, touching each edge
once and expanding the bit lifts by doubling. Measured with the crate's own bench
(`cargo bench -p circuit --bench p256_matrix_products`, standalone build, fat LTO,
128-bit prime, 100 samples; the tape's builder itself takes 72 ms, a one-time setup cost):

| quantity | value |
|---|---:|
| tape | 66,412 nodes, 97,986 edges, 259 reverse levels, 1,029 distinct coefficients, 7,061 rows × 1,215,663 columns, 6.2 MiB |
| `apply` (the vector `r·(A + xB + x²C)` over all columns, Montgomery form), 1 thread | **2.71 ms** |
| `apply`, 10 threads | **0.82 ms** |
| F2Z today: `coefficient_combine` (prover, the same vector), 1 / 10 threads | 21.0 / 5.7 ms |
| F2Z today: the gather inside the verifier's `coefficient_evaluate`, 1 / 10 threads | ≈21 / ≈5 ms of 30.1 / 6.8 |

F2Z's `ModQCoefficients` (`src/piop/spartan/ecdsa_sha256/inner_reduction.rs`) was written
against the expanded rows (`CompactRows` + `TailColumns`, the 2026-09-10 interning pass) and
never adopted the tape; only witness generation uses the crate's product machinery
(`ProductWitgen`). Row and column numbering coincide by construction: both backends replay
`verify_digest_circuit` in the same order, and the tape's columns are the integer-witness
indices that `TailColumns` uses.

**Phase 1 — DONE and measured (2026-09-13, 20:24).** Implemented as described below and applied
to the main checkout uncommitted for review (`docs/sha256-ecdsa-tape-phase1.patch`, 5 files:
`crates/circuit/src/matrix_wengert.rs` gains `PreparedWengertEvaluator::apply_weighted`,
`relation.rs` builds and shape-checks the tape, `inner_reduction.rs` feeds both gathers from it
and keeps the expanded gather as the test oracle, `proof.rs` makes the coefficient struct
mutable, `VENDORED.md` notes the local addition). Checks: ECDSA module tests 10/10 including the
new `tape_tail_matches_column_gather` (tape vector equals the expanded gather element by element
under both outer modes, modulus M127) and the transcript pin; proofs verify with unchanged sizes
(135,337 / 92,433 bytes). A/B on the suite fixture, interleaved pristine/tape runs, 2 × 5 samples
per cell, 15 s cool-downs (ms):

| build | thr | rate | prover | `combine` | `evaluate` (P) | verifier | `evaluate` (V) | `rswitch` |
|---|---:|---|---:|---:|---:|---:|---:|---:|
| HEAD | 1 | 1/2 | 205.9 | 20.8 | 11.1 | 44.33 | 29.8 | 12.9 |
| **tape + V1** | 1 | 1/2 | **184.5** | **5.0** | **5.1** | **23.37** | **8.1** | 12.9 |
| HEAD | 1 | 1/8 | 203.0 | 20.7 | 10.9 | 43.92 | 29.6 | 12.8 |
| **tape + V1** | 1 | 1/8 | **181.9** | **5.1** | **5.0** | **23.34** | **8.3** | 12.8 |
| HEAD | 10 | 1/2 | 64.8 | 5.6 | 2.8 | 14.97 | 6.8 | 5.9 |
| **tape + V1** | 10 | 1/2 | **61.8** | **3.0** | **1.7** | **12.18** | **3.4** | 5.9 |
| HEAD | 10 | 1/8 | 65.3 | 5.6 | 2.8 | 14.87 | 6.8 | 5.8 |
| **tape + V1** | 10 | 1/8 | **62.0** | **3.1** | **1.8** | **12.21** | **3.3** | 5.9 |

Result at 2^7: prover −10 % at 1 thread (206 → 184 ms; Binius64 168) and −5 % at 10 threads
(65 → 62 ms; Binius64 60); verifier −47 % at 1 thread (44 → 23 ms; Binius64 12.8) and −19 % at
10 threads (15 → 12.2 ms; Binius64 6.2). The verifier gap shrinks from 3.5× to 1.8×; the
10-thread prover is within 3 % of Binius64. Setup grows by the tape's construction (840 →
910 ms). What remains of the P-256 fixed cost: the inner-sumcheck tail (117 ms at 2^7, of which
≈66 ms fixed), the opening's fixed ≈35 ms, and on the verifier the ring-switch read-off (12.9 ms)
and ≈8 ms of tail dot product — the phase 2/3 targets.

**Every size, tape build vs the fair campaign** (`bench_results/sha256-ecdsa-tape-20260913-i4-10`,
F2Z rows only, official runner, same fixtures, reps 3; proof sizes identical at every cell; the
Binius64 rate-1/2 column is the fair campaign's):

| N | thr | F2Z prover before → after | F2Z verifier before → after | Binius64 prover / verifier |
|---|---:|---:|---:|---:|
| 2^4 | 1 | 146 → **125** (−14 %) | 39.9 → **19.4** (−51 %) | 148 / 11.3 |
| 2^5 | 1 | 156 → **136** (−13 %) | 40.7 → **20.1** (−51 %) | 148 / 11.4 |
| 2^6 | 1 | 171 → **149** (−13 %) | 41.9 → **21.3** (−49 %) | 150 / 11.7 |
| 2^7 | 1 | 207 → **185** (−10 %) | 44.7 → **23.5** (−47 %) | 168 / 12.8 |
| 2^10 | 1 | 689 → **664** (−4 %) | 76.7 → **56.1** (−27 %) | 218 / 22.4 |
| 2^4 | 10 | 51.7 → **49.1** (−5 %) | 14.2 → **11.2** (−21 %) | 57.3 / 5.7 |
| 2^5 | 10 | 54.9 → **51.1** (−7 %) | 14.1 → **11.3** (−20 %) | 57.6 / 5.8 |
| 2^6 | 10 | 58.5 → **55.1** (−6 %) | 14.8 → **11.9** (−20 %) | 57.2 / 5.9 |
| 2^7 | 10 | 65.8 → **62.5** (−5 %) | 15.2 → **12.4** (−19 %) | 60.3 / 6.2 |
| 2^10 | 10 | 167 → **166** (−1 %) | 23.2 → **20.1** (−13 %) | 73.1 / 8.4 |

(rate 1/8 rows within 1 % of rate 1/2.) F2Z's prover now beats Binius64's at 1 thread up to 4 KB
and at 10 threads up to 4 KB, and is within 4 % at 8 KB and 10 threads. The proposed paper table
with these F2Z rows is `docs/sha256-ecdsa-table-proposed-tape.tex`.

**Phase 1 as planned (kept for the record; transcript-neutral, proofs byte-identical).** Build the `WengertTape`
in `build_local` next to the expanded rows; `ModQCoefficients::from_relation` calls
`tape.prepare(&RuntimeModulus::<2>::new(q))` (1,029 coefficients to Montgomery, the same
class of work as today's 3,358-residue projection). The prover's `build_batched_matrix_mle`
feeds the tape one challenge per row and `x = ρ`: nonlinear rows `r = w_outer` (the tape forms
`r, rρ, rρ²` itself); linear rows carry a C-only weight in Split mode, so either pass
`r = w_lin·γ·ρ^{-2}` or, cleaner, add a per-row `[r_A, r_B, r_C]` entry point to the vendored
evaluator, which already stores exactly that triple per row. The output is the 1.2 M-entry
tail in Montgomery form (both sides use crypto-bigint `FixedMontyParams<2>`, so the
representation matches; assert it in a test). The verifier's `evaluate_batched_matrix_mle`
does the same apply and then the tail dot product with the V1 grouping. Expected at 2^7:
prover 207 → ≈185 ms and verifier 45 → ≈22 ms at 1 thread; 66 → ≈62 and 15 → ≈10 ms at 10
threads. Checks: `streamed_matrix_evaluation_matches_prepared_mle`, the 2^3 transcript pin, and
a new unit test comparing the tape's vector with the `CompactRows` gather on random
challenges. Risks: the tape's parallel thresholds (levels below 2^16 nodes run serially, so the
10-thread gain is what the bench shows, not more), the `ρ = 0` corner if the inverse trick is
used, and Montgomery-form agreement.

**Phase 2 (days).** V1 (measured, §4.6) and P0 (the hoisted configuration check). Together with
phase 1 the P-256 fixed prover cost falls from 138 to ≈110 ms, still above the 3.4 ms the
arithmetic itself costs, because the inner sumcheck's tail (66 ms fixed at 1 thread) and the
opening's fixed 35 ms remain.

**Phase 3 (one to two weeks; transcript-changing, new pins, table re-measured).** A limb-native
P-256 relation. Add a `Circuit` backend to the vendored crate that lowers the lazy integer
representatives to 64-bit limb cells instead of bit vectors: a 521-bit hint becomes nine limb
cells, `uint_from_repr` becomes a nine-term combination with `2^{64k}` coefficients, the 7,061
rows keep their shape with ≈60 k nonzeros in total, and the witness becomes ≈20 k integer cells
of the same total bit count. F2Z's per-column bit-size bound gives every limb its range for
free, which is what the bit lifts provide implicitly today; the 265-bit quotient bound is a
five-limb cell with a narrower top limb. On the F2Z side the P-256 part becomes a mod-q
integer-cell relation in the style of the u64 relation (field-valued raw-Montgomery Spartan,
`2^64` C coefficients), whose claim is batched with the SHA bit claim in one opening the way the
hybrid protocol already batches an integer-cell claim with a Binius bit claim. The inner
sumcheck's tail shrinks from 1.2 M to ≈20 k cells, the matrix terms vanish on both sides, and the
P-256 fixed prover cost should approach the opening's ≈35 ms plus small change, against
Binius64's 149 ms, which is the advantage the u32/u64/u128 tables show. The SHA per-compression
cost is untouched by all three phases.

### 4.8 The structured tail kernel — DONE and measured (2026-09-13, 21:31; user's choice over limb cells)

Prover-only, transcript-neutral. The inner sumcheck's P-256 tail is now folded through the
tape's geometric structure instead of the generic per-cell path:

- The tape exposes its power groups as runs `(start, len, base)` with `tail[start + k] = base·2^k`
  (`PreparedWengertEvaluator::power_runs`, a local addition to the vendored crate); the batched
  MLE splits them around the 1,024 public-bit cells (whose values are adjusted after the tape)
  and hands them to the composite MLE as prover-side structure (`with_tail_runs`; never read by
  `evaluation_at`, so it cannot change what is proved).
- The composite prover (`sha256/inner_sumcheck/composite.rs`) cuts the tail into *pieces*, one
  per run per 2^K prefix block plus one per uncovered column, so that on a piece
  `V = weight·2^(i−lo)`. For the K prefix rounds the ternary sums factor as
  `S(β) = Σ_shape g_shape(β) · Σ_i ext_β(i) · A[shape][i]` with
  `A[shape][i] = Σ_{pieces of that shape} weight · h[block, i]`: every tail bit is read once and
  costs field additions only (about eight per block), and the rest is a pass over at most
  (2^K+1)² shapes × 2^K bits × 3^K ternary points, constant in the tail length. The prefix fold
  becomes one multiplication per piece. Blocks are accumulated in parallel with per-thread shape
  tables. The path is taken only when the tail starts on a prefix-block boundary (N ≥ 16, i.e.
  whenever the composite path is taken at all); smaller N keeps the generic prover.
- Checks: `ternary_extension_table_matches_extend_lsb`, `structured_tail_matches_generic`
  (accumulators and folded tables equal the generic ones on a synthetic tail with straddling
  runs, single columns and a gap, K = 1..4), `tail_runs_describe_the_materialized_tail`, the 2^6
  prove-and-verify test, the 2^3 transcript pin, and identical proof sizes at every size below.
  The first attempt failed both structural tests on one bug (absolute instead of relative
  in-block exponents), caught by the prover's own "batched matrix MLE evaluation mismatch" check.

A/B at 2^7 (tape build vs tape + kernel, interleaved 2 × 5, ms):

| build | thr | rate | prover | `shared_inner_prove` | opening | verifier | proof B |
|---|---:|---|---:|---:|---:|---:|---:|
| tape | 1 | 1/2 | 182.7 | 115.8 | 51.5 | 23.2 | 135,337 |
| **tape + kernel** | 1 | 1/2 | **152.1** | **84.5** | 52.5 | 23.4 | 135,337 |
| tape | 1 | 1/8 | 180.6 | 116.1 | 47.7 | 23.0 | 92,433 |
| **tape + kernel** | 1 | 1/8 | **149.1** | **83.3** | 48.7 | 23.1 | 92,433 |
| tape | 10 | 1/2 | 61.7 | 25.9 | 25.1 | 12.3 | 135,337 |
| tape + kernel | 10 | 1/2 | 63.0 | 27.1 | 25.5 | 12.0 | 135,337 |
| tape | 10 | 1/8 | 61.7 | 25.9 | 24.4 | 12.1 | 92,433 |
| tape + kernel | 10 | 1/8 | 63.1 | 26.9 | 24.9 | 11.9 | 92,433 |

Every size (`bench_results/sha256-ecdsa-kernel-20260913-i4-10`, official runner, F2Z rows;
proof sizes identical to the tape build at every cell):

| N | thr | F2Z prover: tape → kernel | Binius64 prover | F2Z verifier | Binius64 verifier |
|---|---:|---:|---:|---:|---:|
| 2^4 | 1 | 125 → **93** (−25 %) | 148 | 19.5 | 11.3 |
| 2^5 | 1 | 136 → **103** (−24 %) | 148 | 20.0 | 11.4 |
| 2^6 | 1 | 149 → **116** (−22 %) | 150 | 21.2 | 11.7 |
| 2^7 | 1 | 185 → **152** (−18 %) | 168 | 23.3 | 12.8 |
| 2^10 | 1 | 664 → **637** (−4 %) | 218 | 55.7 | 22.4 |
| 2^4 | 10 | 49.1 → 50.9 | 57.3 | 11.3 | 5.7 |
| 2^7 | 10 | 62.5 → 63.6 | 60.3 | 12.1 | 6.2 |
| 2^10 | 10 | 165 → 165 | 73.1 | 19.2 | 8.4 |

Reading: at 1 thread F2Z's prover is now below Binius64's up to 8 KB (37 % faster at 1 KB,
9 % at 8 KB) and the P-256 fixed cost is ≈85 ms against Binius64's 149. At 10 threads the
kernel is a wash (+1–4 %, within noise): the tail's prefix pass was already parallel and small
there, and the new pass adds ≈1 ms of serial piece construction (done twice; caching it is a
small follow-up). The verifier is unchanged by design (23 vs 13 ms at 2^7; the ring-switch
read-off is now its largest term, 12.9 ms). What remains of the tail on the prover: the
post-prefix rounds over the 76 k folded tail suffixes (≈15–20 ms at 1 thread); those folded values
are still geometric across the interior blocks of each run (ratio 2^16), so the same trick can
be applied one level up — the natural next lever if the 1-thread prover matters further.

Combined patch (tape + kernel, 7 files, +875/−44), applied to the main checkout uncommitted for
review: `docs/sha256-ecdsa-tape-kernel.patch` (supersedes `sha256-ecdsa-tape-phase1.patch`).
Proposed paper table with these F2Z rows: `docs/sha256-ecdsa-table-proposed-kernel.tex`.

## 5. Paper impact (proposals only — nothing was edited)

### 5.1 Claims that depend on the old comparison

`paper/main.tex`, § "SHA-256 hashing followed by ECDSA signature verification" (line 2910 ff.):

1. **Line 2912, prose.** "proving the SHA-256 hash of a $4$KB message (a chain of $2^6$ SHA
   compressions)" — the table shows four sizes (0.96, 1.98, 4.03, 8.13 KB = 2^4 … 2^7
   compressions), the suite's headline size is 2^7 (8,128 bytes), and the fair campaign adds
   2^10 (65,472 bytes). "We compare \ftwoz-SNARK with Binius64 \cite{binius64}, Zinc+
   \cite{zincplus}" — there is no Zinc+ row; the Binius suite has four rows.
2. **Lines 2926–2961, the inline table `tab:sha256-ecdsa-f2z-opt`.** Every Binius64 cell
   (both rates, both thread counts, all four sizes) comes from the crippled worker: prover
   917–1262 ms and verifier 130–150 ms at 1 thread, 214–289 / 51–57 ms at 10 threads. Fair values
   at 8.13 KB: 167 / 12.8 ms (1 thread), 60.3 / 6.2 ms (10 threads) at rate 1/2. Consequently
   the bold marks in the Prover and Verifier columns are wrong (F2Z does not win them), the F2Z
   rows are pre-suite (`fac8c8f` level, rate 1/8 only; the suite measures both rates), the scheme
   names carry "(this work)" and `\cite` (against the 2026-09-13 naming decision), and the
   caption still describes a before/after A/B ("\emph{Before} is master 9d75b2b, \emph{after} is
   fac8c8f … interleaved runs of pinned binaries, medians of 2 × 3") that is not what the table
   shows.
3. **Line 167, intro table `tab:intro_table`.** The row "$4$KB SHA + ECDSA" (placeholders "--")
   lists \ftwoz-SNARK, Binius64 and Plonky3; no Plonky3 measurement of this statement exists.
   Its caption says "(Binius64 is at 96 bits)"; for this row Binius64 runs at the explicit
   100-bit query target (241 queries at rate 1/2).
4. **No other prose depends on it.** Line 152 ("when proving integer constraints, Binius64
   … greatly outperforms other schemes") and lines 307/915 (4.4× smaller SHA witnesses) do not
   rest on this comparison; the commented-out § "SHA-256 hashing" (line 3012 ff.) already says
   \ftwoz-SNARK does not outperform Binius64/Flock when the amount of hashing is large.

### 5.2 Proposed replacement prose (for the user to adapt; the paper is written without AI assistance)

Paragraph replacing line 2912:

> We experiment with proving the SHA-256 hash of a message of 1 KB to 8 KB (chains of $2^4$ to
> $2^7$ SHA-256 compressions, padding block included) followed by a P-256 ECDSA verification of
> the digest, a common statement in anonymous credentials; a 64 KB message ($2^{10}$
> compressions) shows the trend for longer messages. We compare \ftwoz-SNARK with Binius64
> \cite{binius64}, both with its own BaseFold opener and with the \ftwoz\ opener in its place
> (\cref{s:experiments_sha_modmul}), at rates $1/2$ and $1/8$.

Paragraph to add after the table (results):

> \ftwoz-SNARK produces proofs $3\times$ smaller than Binius64's at the same rate and generates
> its witness $3$--$4\times$ faster. Its prover matches Binius64's up to 2 KB (and is slightly
> faster at 10 threads) and falls behind as the message grows: $1.2\times$ slower at 8 KB and
> $3.2\times$ at 64 KB single-threaded. The P-256 verification costs the two provers about the
> same ($140$--$150$ ms single-threaded), but each SHA-256 compression costs \ftwoz\ about
> $0.5$ ms against Binius64's $0.07$ ms, because \ftwoz\ proves it as integer constraints over
> a $113$-bit prime while Binius64 proves it over $\FF_2$; \cref{s:experiments_sha_modmul}
> shows how the hybrid \ftwoz-SNARK removes this cost. The \ftwoz\ verifier is about $3.5\times$
> slower: $30$ ms of it evaluates the P-256 constraint matrices ($4.2$ million nonzero entries)
> at the sumcheck point, a cost linear in the circuit that a preprocessing step would remove,
> while Binius64's verifier evaluates its constraint system at word granularity.

Intro table row (10-thread values, 8.13 KB; if the row keeps "4KB", use the 4.03 KB values in
parentheses): \ftwoz-SNARK (rate 1/8): prover 65.6 (59.0) ms, verifier 15.0 (14.8) ms, proof
92 (82) KB, peak 1.22 (1.21) GB; Binius64 (rate 1/2): 60.3 (57.2) ms, 6.18 (5.89) ms, 394 (362)
KB, 0.95 (0.89) GB. Drop the Plonky3 row of this block unless it is measured, and qualify the
caption's "(Binius64 is at 96 bits)": for the SHA+ECDSA rows Binius64 runs at a 100-bit FRI
query target.

### 5.3 Proposed table

``docs/sha256-ecdsa-table-proposed.tex`` holds a drop-in replacement for the inline table in the 2026-09-13 layout
(1 thr | 10 thr sub-columns under Prover and Verifier; scheme names `\ftwoz-SNARK, rate $1/N$`,
`Binius (UDR), rate $1/N$`, `Binius (Johnson), rate $1/N$`; witgen and peak memory from the
10-thread runs; bold = best displayed value per column and size; label `tab:sha256-ecdsa`). It
was generated from `bench_results/suite-sha256-ecdsa-20260913` (2^7) and
`bench_results/sha256-ecdsa-fair-20260913-i4-6-10` (2^4, 2^5, 2^6, 2^10) by
``docs/sha256-ecdsa-table-proposed.py` (a throwaway generator in the decided layout; the repo exporter `scripts/sha256_ecdsa_table.py` still emits the old layout and `\rho` names and is the other session's to rework)`. Headline rows (ms / ms / KB):

| Msg. (KB) | scheme | prover 1 thr | prover 10 thr | verifier 1 thr | verifier 10 thr | proof |
|---|---|---:|---:|---:|---:|---:|
| 0.96 | \ftwoz-SNARK, rate 1/8 | 146 | 52.2 | 40.0 | 13.9 | 75 |
| 0.96 | Binius (UDR), rate 1/2 | 148 | 57.3 | 11.3 | 5.69 | 362 |
| 4.03 | \ftwoz-SNARK, rate 1/8 | 174 | 59.0 | 41.9 | 14.8 | 82 |
| 4.03 | Binius (UDR), rate 1/2 | 150 | 57.2 | 11.7 | 5.89 | 362 |
| 8.13 | \ftwoz-SNARK, rate 1/8 | 206 | 65.6 | 44.7 | 15.0 | 92 |
| 8.13 | Binius (UDR), rate 1/2 | 167 | 60.3 | 12.8 | 6.18 | 394 |
| 65.47 | \ftwoz-SNARK, rate 1/8 | 696 | 169 | 76.7 | 22.9 | 125 |
| 65.47 | Binius (UDR), rate 1/2 | 218 | 73.1 | 22.4 | 7.93 | 394 |

Two caveats for the user: the F2Z witgen column at 2^4/2^5 (7.2–9.4 ms at 1 thread, 3.0–3.5 ms at
10 threads) still uses the per-bit packing fallback for `N < 64` that memory Tier 2b removed in
the unmerged `worktree-ecdsa-opt`; and the 2^10 rows are the only ones where Binius64's peak
memory (1.67–1.77 GB) exceeds F2Z's (1.41 GB).

## 6. Reproduction and files

Everything ran through `scripts/bench_gate.py` (lock `/tmp/f2z-bench.lock`, idle hold, swap guard)
after the other session had stopped its raw-performance sweep and handed the machine over; nothing
in the main checkout was edited, committed, stashed or reset. Session artifacts live under the
session scratchpad `…/scratchpad/inv/` (not in the repository):

- `wt/` — `git worktree` of HEAD `c0751bf` used for every "cur" build (its worker `main.rs` and the
  untracked `benchmarks/binius64-prof/` copy carry a one-line `proof_blake3` digest and, in the
  copy, a tracing layer that times every span; the prover code is untouched);
  `old/benchmarks/binius64/` — the 2026-09-10 worker source as recorded by the campaign runner;
  `wt-lever/` — the V1 experiment (§4.6), a worktree of HEAD with the one-function change.
- `build_grid.sh`, `measure_grid.sh`, `profile.sh`, `campaign.sh`, `lever.sh`, `run_all.sh`,
  `parse.py`, `table_proposed.py` — the scripts; `logs/*.build.log` (full `cargo build -v`),
  `logs/*.flags.txt` (the extracted rustc flags per variant), `logs/*-status.txt`.
- `bin/` — one binary per variant (`w-old-none16`, `w-old-thin16`, `w-old-fat1`, `w-cur-none16`,
  `w-cur-thin16`, `w-cur-fat16`, `w-cur-none1`, `w-cur-fat1`, `w-prof-fat1`, `f-none16`,
  `f-thin16`, `f-fat1`, `f-lever`); `res/*.jsonl` — every worker/bench record (warm-up and
  samples), `res/*.time` (BSD `time -l`, peak RSS), `res/sample-*.txt` (macOS `sample`
  call trees), `res/spans-*.jsonl` (Binius64 span phases).
- `sha256-ecdsa-table-proposed.tex` — the proposed paper table (§5.3); `res/wengert-t{1,10}.txt` — the crate's
  `p256_matrix_products` bench output (§4.7), built in `t/circuit`.

Repository results written by this investigation (new directories only):
`bench_results/sha256-ecdsa-fair-20260913-i4-6-10/` (official runner, suite binaries, 48 cases).

Re-running the grid from scratch:

```sh
# recorded 2026-09-10 worker source: bench_results/sha256-ecdsa-i7-f2z-binius/source/benchmarks/binius64
# (needs ../../benches/support/sha256_ecdsa_fixture.rs next to it); current worker: benchmarks/binius64
CARGO_TARGET_DIR=/tmp/t-none16 CARGO_PROFILE_RELEASE_LTO=false CARGO_PROFILE_RELEASE_CODEGEN_UNITS=16 \
  RUSTFLAGS="-C target-cpu=native" cargo build --release --locked -v      # Cargo default (the old worker)
CARGO_TARGET_DIR=/tmp/t-thin16 CARGO_PROFILE_RELEASE_LTO=thin  CARGO_PROFILE_RELEASE_CODEGEN_UNITS=16 ...   # Binius64 upstream
CARGO_TARGET_DIR=/tmp/t-fat1   CARGO_PROFILE_RELEASE_LTO=true  CARGO_PROFILE_RELEASE_CODEGEN_UNITS=1  ...   # f2z-pcs
RAYON_NUM_THREADS=1 HARDWARE_CONCURRENCY=1 /tmp/t-*/release/binius64-sha256-ecdsa --method binius64 --r 7 --c 0 \
  --target 100 --log-inv-rate 1 --threads 1 --reps 5 --seed 0 \
  --fixture bench_results/suite-sha256-ecdsa-20260913/fixtures/i7-seed0.json
```


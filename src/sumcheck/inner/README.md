# Inner sumcheck

The shared engine proves quadratic dot products. `engine::drive` reconstructs
`q(X) = c0 + (claim - 2*c0 - c2)*X + c2*X²`, absorbs every claim's message before
sampling the common challenge, and invokes the fused fold/next-round kernel.
The field and callbacks are statically dispatched.

## Public API

- `prove_inner_sumcheck(field, transcript, claim, values: Vec<T>, weights: Vec<F::Elem>, boundary)`
  returns `InnerSumcheckOutput<F::Elem>`.
- `prove_batched_inner_sumcheck::<F, T, K>(field, transcript, claims: &[F::Elem; K],
  values: [Vec<T>; K], weights: [Vec<F::Elem>; K], boundary)` returns
  `BatchedInnerSumcheckOutput<F::Elem, K>`.
- `SumcheckProof::verify_with_round_boundary` and
  `SumcheckProof::verify_batch_with_round_boundary` verify the round reductions.
  The enclosing protocol must discharge the terminal matrix and PCS claims.

`InnerSumcheckOutput` contains `proof`, `point`, `final_claim`, and
`terminal_evaluations: [weight, value]`. The batched output has `proofs`, one
common `point`, `final_claims`, and one pair of terminal evaluations per claim.
`SumcheckProof<E, 3>` stores three coefficients, so its degree is two.

Input lengths must agree and be nonempty powers of two. `K = 0` is rejected.
Singleton inputs have no rounds and still check their terminal identity.
There is no implicit random linear combination of batch claims.

Arithmetic bounds use `FieldOps`, `BatchMulAcc`, `Reduce`, and
`PreparedLinearCombination` from `vendor/field`, plus the existing
`SpartanField` transcript contract. Streaming MAC is `BatchMulAcc::mul_acc`;
`Reduce::prepare_reduce` selects a reduction schedule once from the public
maximum total number of products, including merged workers. Mixed first-round
integer differences use two signed field weights, avoiding source-width overflow.
No `SumcheckProductReducer`, `InnerArithmetic`, or `InnerRows` trait is required.

## Optimized integrations

- `native`: borrowed native limbs, canonical integer folded storage, sparse live
  prefixes, block-selector coefficients, and fused folding with the next message.
- `packed`: packed bits, factored/composite coefficient tables, split equality,
  and prepared prefixes of ordinary rounds. Prefix lengths do not change the proof.
- `binary`: post-GKR degree-two batches. Tables fold in place. The existing
  evaluation-form codec, metadata, interpolation nodes, and challenge reabsorption
  are retained by `evaluation_form`.
- The hybrid dense tail uses the same engine and retains its compressed codec,
  packed seven-round prefix, random combination, and scratch-buffer reuse.

Legacy prover algorithms are retained only under `cfg(test)` as independent
arithmetic/transcript oracles. Old module paths are small integration facades.
Higher-degree single-group proofs still use their existing evaluation-form
proof envelope and verifier; they are not reinterpreted as quadratic claims.

## Qualification

Baseline: `50684158ab8721632cba71b0886a3311f37404c0`. Apple Silicon only.
`scripts/inner_regression.py` compares frozen executable pairs under the existing
`scripts/bench_gate.py` host gate. It retains verified samples, command lines,
process RSS, proof sizes, and failed cells, and reports a paired 95% interval
against a 1% slowdown threshold. Allocator-instrumented runs are separate from
latency runs. Results and executable hashes live under
`bench_results/inner-refactor-apple`.

`skipped_experiment` is compiled only in tests. It sends the complete prefix
polynomial, proves the quadratic tail, then proves two shared-challenge dot
products that return both original MLEs to one ordinary opening point. Its ignored
measurement includes prefix preparation, retained source tables, and the bridge.
It is not enabled in production; an ordinary prepared prefix keeps the old proof.

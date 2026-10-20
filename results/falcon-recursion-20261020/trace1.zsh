#!/bin/zsh
# Traced 1-thread runs (λ=128, batch 1024, one rep) of the old and new bench binaries.
W=/Users/albertgarretafontelles/f2z-pcs/.claude/worktrees/falcon-recursion
T=/Users/albertgarretafontelles/.claude/jobs/f60cc82e/tmp
export BITZ_FALCON_CASE_CACHE=$W/bench_results/falcon-recursion-20261018/cases
export BITZ_FALCON_GRINDING_MODEL=per-site BITZ_FALCON_PRIME=small BITZ_ARITHMETIC_RING=keyed
export BITZ_FALCON_BRIDGE_ROWS=16 BITZ_ARITHMETIC_BINDING_STRATEGY=optimized
for which in old new; do
  bin=$W/target-bench/release/deps/falcon_recursion-4f2a12645512d21d
  [ $which = old ] && bin=$W/target-bench/ab/falcon_recursion-c2c14ae
  BITZ_TRACE=1 BITZ_REC_SECURITY=128 BITZ_REC_LIG=udrg:3:4 BITZ_REC_T=18 BITZ_REC_FALCON_BATCH=1024 BITZ_REC_REPS=1 \
    RAYON_NUM_THREADS=1 $bin > $T/trace1-$which.out 2>&1
  echo "$which exit $?"
done

#!/bin/zsh
# samply profile of the one-limb BitZ setup (variables exported: samply cannot
# inject into /usr/bin/env), then the span-traced phase-time check runs.
set -u
D=/Users/albertgarretafontelles/f2z-pcs/.claude/worktrees/falcon-flamegraph/bench_results/flamegraph-20261017
W=/Users/albertgarretafontelles/f2z-pcs/.claude/worktrees/falcon-flamegraph
OUT=$D/samply
BBIN=$W/target-native/release/deps/falcon_algebraic-8edd41f39df7f30c
sleep 20
(
  export BITZ_FALCON_CASE_CACHE=$D/falcon-cases BITZ_PROFILE_IN_POOL=1 RAYON_NUM_THREADS=1
  export BITZ_FALCON_GRINDING_MODEL=per-site BITZ_FALCON_PRIME=small
  samply record --save-only --unstable-presymbolicate --rate 250 -o $OUT/bitz-onelimb.samply.json -- \
    $BBIN --degree 512 --batch 32768 --security 128 --threads 1 --warmup 0 --iterations 5 \
    > $OUT/bitz-onelimb.out 2> $OUT/bitz-onelimb.err
  echo "$(date +%T) bitz-onelimb samply exit=$? $(pmset -g batt | grep -o '[0-9]*%; [a-z ]*')" >> $OUT/clock.log
)
$D/trace-t1.zsh

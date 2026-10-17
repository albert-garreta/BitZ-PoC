#!/bin/zsh
# samply (250 Hz wall-clock timer, per-thread CPU deltas) profiles of the same
# three single-thread provers at 32768 Falcon-512 signatures, five proves each.
set -u
D=/Users/albertgarretafontelles/f2z-pcs/.claude/worktrees/falcon-flamegraph/bench_results/flamegraph-20261017
W=/Users/albertgarretafontelles/f2z-pcs/.claude/worktrees/falcon-flamegraph
OUT=$D/samply
mkdir -p $OUT
ABIN=$D/aerie/target/release/deps/falcon-6efadfd380be1a7d
BBIN=$W/target-native/release/deps/falcon_algebraic-8edd41f39df7f30c
export AERIE_HONEST_FIXTURE_DIR=$D/aerie-fixtures BITZ_FALCON_CASE_CACHE=$D/falcon-cases
export AERIE_PROFILE_IN_POOL=1 BITZ_PROFILE_IN_POOL=1 RAYON_NUM_THREADS=1
B=(--degree 512 --batch 32768 --security 128 --threads 1 --warmup 0 --iterations 5)
record() {
  local name=$1; shift
  sleep 20
  samply record --save-only --unstable-presymbolicate --rate 250 -o $OUT/$name.samply.json -- "$@" > $OUT/$name.out 2> $OUT/$name.err
  echo "$(date +%T) $name samply exit=$? $(pmset -g batt | grep -o '[0-9]*%; [a-z ]*')" >> $OUT/clock.log
}
record aerie $ABIN 32768 32768 32768 32768 32768
record bitz-shipped $BBIN $B
record bitz-onelimb env BITZ_FALCON_GRINDING_MODEL=per-site BITZ_FALCON_PRIME=small $BBIN $B
echo DONE > $OUT/DONE

#!/bin/zsh
# Separates the two changes of the sha-compressions line: the same Ch-only
# relation (16,360 cells per compression) opened by the Id ⊗ M block opener
# (BITZ_OPENER=block) and by the generic virtual opening
# (BITZ_OPENER=virtual), 2^13 compressions, λ=100, private I/O, 10 threads,
# two interleaved rounds. ALLOW_BATTERY=1 skips the wait for AC.
set -u
W=/Users/albertgarretafontelles/f2z-pcs/.claude/worktrees/falcon-recursion
OUT=$W/bench_results/falcon-recursion-vs-sha-20261021/opener
mkdir -p $OUT
BLOCK=/Users/albertgarretafontelles/f2z-pcs-shacomp/target/release/deps/sha256_compressions-ba2724b86aa629b7
echo "$BLOCK $(md5 -q $BLOCK)" > $OUT/binaries.txt
on_ac() { [ "${ALLOW_BATTERY:-0}" = 1 ] && return; until pmset -g batt | grep -q "AC Power"; do sleep 60; done; }
for round in 1 2; do
  for opener in block virtual; do
    on_ac; sleep 15
    env RAYON_NUM_THREADS=10 BITZ_OPENER=$opener BITZ_BENCH_LAMBDA=100 BITZ_BENCH_SHAPES=13 BITZ_BENCH_REPS=3 \
      BITZ_SHA_PRIVATE_IO=1 $BLOCK > $OUT/$opener-r$round.out 2> $OUT/$opener-r$round.stderr
    echo "$(date +%T) $opener-r$round exit=$? $(pmset -g batt | grep -o 'AC Power\|Battery Power')" >> $OUT/clock.log
  done
done

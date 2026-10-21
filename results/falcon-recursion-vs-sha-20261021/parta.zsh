#!/bin/zsh
# What the SHA batch of a block-opener recursion would cost on its own: the
# sha-compressions prover (Ch-only relation, Id ⊗ M block opener, private
# I/O) at the recursion's padded compression counts, 2^14 (1K signatures:
# 8,952 compressions) and 2^15 (32K: 17,897), λ=128 and λ=100, 10 threads.
# ALLOW_BATTERY=1 skips the wait for AC.
set -u
W=/Users/albertgarretafontelles/f2z-pcs/.claude/worktrees/falcon-recursion
OUT=$W/bench_results/falcon-recursion-vs-sha-20261021/parta
mkdir -p $OUT
BLOCK=/Users/albertgarretafontelles/f2z-pcs-shacomp/target/release/deps/sha256_compressions-ba2724b86aa629b7
echo "$BLOCK $(md5 -q $BLOCK)" > $OUT/binaries.txt
on_ac() { [ "${ALLOW_BATTERY:-0}" = 1 ] && return; until pmset -g batt | grep -q "AC Power"; do sleep 60; done; }
for spec in 128:14 128:15 100:14; do
  parts=(${(s/:/)spec})
  on_ac; sleep 15
  env RAYON_NUM_THREADS=10 BITZ_BENCH_LAMBDA=$parts[1] BITZ_BENCH_SHAPES=$parts[2] BITZ_BENCH_REPS=3 BITZ_SHA_PRIVATE_IO=1 \
    /usr/bin/time -l $BLOCK > $OUT/l$parts[1]-s$parts[2].out 2> $OUT/l$parts[1]-s$parts[2].stderr
  echo "$(date +%T) l$parts[1]-s$parts[2] exit=$? $(pmset -g batt | grep -o 'AC Power\|Battery Power')" >> $OUT/clock.log
done

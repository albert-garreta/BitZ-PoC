#!/bin/zsh
# The block opener ported onto falcon-recursion (sha256_compressions with the
# Ch-only relation and the Id ⊗ M block opener, recursion features) at 2^13
# compressions, private I/O, λ=128 and λ=100, interleaved with the 10-08
# sha-compressions binary (λ=100) as the parity control. 10 threads.
# ALLOW_BATTERY=1 skips the wait for AC.
set -u
W=/Users/albertgarretafontelles/f2z-pcs/.claude/worktrees/falcon-recursion
OUT=$W/bench_results/falcon-recursion-vs-sha-20261021/ported
mkdir -p $OUT
PORT=$W/target-bench/release/deps/$(ls -t $W/target-bench/release/deps | grep -E '^sha256_compressions-[0-9a-f]+$' | head -1)
CTRL=/Users/albertgarretafontelles/f2z-pcs-shacomp/target/release/deps/sha256_compressions-ba2724b86aa629b7
{ for b in $PORT $CTRL; do echo "$b $(md5 -q $b)"; done; git -C $W rev-parse HEAD; } > $OUT/binaries.txt
on_ac() { [ "${ALLOW_BATTERY:-0}" = 1 ] && return; until pmset -g batt | grep -q "AC Power"; do sleep 60; done; }
step() {
  local name=$1 bin=$2 lambda=$3
  on_ac; sleep 15
  env RAYON_NUM_THREADS=10 BITZ_BENCH_LAMBDA=$lambda BITZ_BENCH_SHAPES=13 BITZ_BENCH_REPS=3 BITZ_SHA_PRIVATE_IO=1 \
    $bin > $OUT/$name.out 2> $OUT/$name.stderr
  echo "$(date +%T) $name exit=$? $(pmset -g batt | grep -o 'AC Power\|Battery Power')" >> $OUT/clock.log
}
for round in 1 2; do
  step port-l128-r$round $PORT 128
  step port-l100-r$round $PORT 100
  step ctrl-l100-r$round $CTRL 100
done

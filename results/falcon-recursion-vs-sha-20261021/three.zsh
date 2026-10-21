#!/bin/zsh
# Three SHA-256 arithmetizations at 2^13 compressions, λ=100, 10 threads, two
# interleaved rounds in one window:
#   binary : the SHA + mod-2^32 hybrid (Binius64 SHA PIOP, shared opening),
#            2^9 multiplications (target-hybrid, checked);
#   block  : CM with Ch-only product slots and the Id ⊗ M block opener,
#            private I/O (~/f2z-pcs-shacomp LTO binary of 10-08 = the port
#            committed as 14d63cec; Blake3 transcripts and Merkle trees);
#   generic: CM through the repeated Spartan + virtual BitZ path (this
#            branch, target-bench, sha256-fs).
# ALLOW_BATTERY=1 skips the wait for AC. Run under the gate:
#   python3 scripts/bench_gate.py run --label sha-three --min-idle 75 --hold-seconds 30 -- zsh results/falcon-recursion-vs-sha-20261021/three.zsh
set -u
W=/Users/albertgarretafontelles/f2z-pcs/.claude/worktrees/falcon-recursion
OUT=$W/bench_results/falcon-recursion-vs-sha-20261021/three
mkdir -p $OUT
HY=$W/target-hybrid/release/deps/$(ls -t $W/target-hybrid/release/deps | grep -E '^hybrid_u32_sha256-[0-9a-f]+$' | head -1)
BLOCK=/Users/albertgarretafontelles/f2z-pcs-shacomp/target/release/deps/sha256_compressions-ba2724b86aa629b7
GEN=$W/target-bench/release/deps/$(ls -t $W/target-bench/release/deps | grep -E '^sha256_compressions-[0-9a-f]+$' | head -1)
{
  for b in $HY $BLOCK $GEN; do echo "$b $(md5 -q $b)"; done
  pmset -g batt | tail -1
} > $OUT/binaries.txt
on_ac() { [ "${ALLOW_BATTERY:-0}" = 1 ] && return; until pmset -g batt | grep -q "AC Power"; do sleep 60; done; }
step() {
  local name=$1; shift
  on_ac; sleep 15
  env "$@" > $OUT/$name.out 2> $OUT/$name.stderr
  echo "$(date +%T) $name exit=$? $(pmset -g batt | grep -o 'AC Power\|Battery Power')" >> $OUT/clock.log
}
for round in 1 2; do
  step binary-r$round RAYON_NUM_THREADS=10 $HY --mode hybrid --mul-log 9 --sha-log 13 --iterations 3
  step block-r$round RAYON_NUM_THREADS=10 BITZ_BENCH_LAMBDA=100 BITZ_BENCH_SHAPES=13 BITZ_BENCH_REPS=3 BITZ_SHA_PRIVATE_IO=1 $BLOCK
  step generic-r$round RAYON_NUM_THREADS=10 BITZ_BENCH_LAMBDA=100 BITZ_BENCH_SHAPES=13 BITZ_BENCH_REPS=3 $GEN
done
echo DONE > $OUT/DONE

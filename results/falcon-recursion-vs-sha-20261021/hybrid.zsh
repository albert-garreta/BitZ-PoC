#!/bin/zsh
# The binary-field SHA-256 arithmetization of the hybrid SHA + mod-2^32
# experiment (Binius64 SHA PIOP, shared BitZ/Ligerito opening) at the
# compression counts of the Falcon recursion circuit: 2^13 and 2^14
# compressions against the minimum 2^9 multiplications, plus all-Binius at
# 2^13. Hybrid proofs refuse `unchecked`, so this binary is checked
# (target-hybrid, --features recursion,falcon-hybrid,bench-internals,span-metrics).
# 10 threads, AC; run under the gate:
#   python3 scripts/bench_gate.py run --label falcon-vs-hybrid -- zsh results/falcon-recursion-vs-sha-20261021/hybrid.zsh
set -u
W=/Users/albertgarretafontelles/f2z-pcs/.claude/worktrees/falcon-recursion
OUT=$W/bench_results/falcon-recursion-vs-sha-20261021
mkdir -p $OUT
D=$W/target-hybrid/release/deps
BIN=$D/$(ls -t $D | grep -E '^hybrid_u32_sha256-[0-9a-f]+$' | head -1)
echo "hybrid $BIN $(md5 -q $BIN)" >> $OUT/binaries.txt
# ALLOW_BATTERY=1 skips the wait for AC (battery caps the clock: compare the
# interleaved cells with each other only, not with AC runs).
on_ac() { [ "${ALLOW_BATTERY:-0}" = 1 ] && return; until pmset -g batt | grep -q "AC Power"; do sleep 60; done; }
ghz() { awk '/ user /{u=$3} /cycles elapsed/{c=$1} END{if(u>0) printf "%.2f", c/u/1e9; else print 0}' $1; }
CM=$W/target-bench/release/deps/$(ls -t $W/target-bench/release/deps | grep -E '^sha256_compressions-[0-9a-f]+$' | head -1)
run() {
  local name=$1; shift
  on_ac; sleep 20
  RAYON_NUM_THREADS=10 /usr/bin/time -l $BIN "$@" > $OUT/$name.out 2> $OUT/$name.stderr
  echo "$(date +%T) $name exit=$? ghz=$(ghz $OUT/$name.stderr)" >> $OUT/clock.log
}
# CM control in the same window (the standalone CM SHA prover at λ=100).
cm() {
  local name=$1 shape=$2
  on_ac; sleep 20
  env BITZ_BENCH_LAMBDA=100 BITZ_BENCH_SHAPES=$shape BITZ_BENCH_REPS=3 BITZ_SHA_TRACE_PATH=$OUT/$name.trace.jsonl \
    RAYON_NUM_THREADS=10 /usr/bin/time -l $CM > $OUT/$name.out 2> $OUT/$name.stderr
  echo "$(date +%T) $name exit=$? ghz=$(ghz $OUT/$name.stderr)" >> $OUT/clock.log
}
run hybrid-m9-s13 --mode hybrid --mul-log 9 --sha-log 13 --iterations 3
cm cm-l100-s13-ctl 13
run hybrid-m9-s14 --mode hybrid --mul-log 9 --sha-log 14 --iterations 3
cm cm-l100-s14-ctl 14
run allbinius-m9-s13 --mode all-binius --mul-log 9 --sha-log 13 --iterations 3
echo DONE > $OUT/DONE-hybrid

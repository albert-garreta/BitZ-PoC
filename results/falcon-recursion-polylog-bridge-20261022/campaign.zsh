#!/bin/zsh
# Recursion over the logarithmic BitZ bridge: the base proof with
# BITZ_FALCON_BRIDGE=polylog (folds committed, fold opening ext:5:4) against
# the standard bridge (folds sent; the circuit takes them as public values),
# both with the ext:3:4 standard-terminal main opening; the FlockRec step at
# T = 18 and T = 16; ext:3:4 outer, λ=128, 10 threads, interleaved per batch.
# ALLOW_BATTERY=1 skips the wait for AC. Run under the gate:
#   python3 scripts/bench_gate.py run --label falcon-polylog -- zsh results/falcon-recursion-polylog-bridge-20261022/campaign.zsh
set -u
W=/Users/albertgarretafontelles/f2z-pcs/.claude/worktrees/falcon-recursion
OUT=$W/bench_results/falcon-recursion-polylog-bridge-20261022/runs
mkdir -p $OUT
D=$W/target-bench/release/deps
BIN=$D/$(ls -t $D | grep -E '^falcon_recursion-[0-9a-f]+$' | head -1)
{
  echo "bin $BIN $(md5 -q $BIN)"
  git -C $W rev-parse HEAD
  git -C $W status --short | grep -v '^?? target-' | head -20
  pmset -g batt | tail -1
} > $OUT/binaries.txt
on_ac() { [ "${ALLOW_BATTERY:-0}" = 1 ] && return; until pmset -g batt | grep -q "AC Power"; do sleep 60; done; }
ghz() { awk '/ user /{u=$3} /cycles elapsed/{c=$1} END{if(u>0) printf "%.2f", c/u/1e9; else print 0}' $1; }
G=(BITZ_FALCON_CASE_CACHE=$W/bench_results/falcon-recursion-20261018/cases BITZ_FALCON_GRINDING_MODEL=per-site
   BITZ_FALCON_PRIME=small BITZ_ARITHMETIC_RING=keyed BITZ_FALCON_BRIDGE_ROWS=16 BITZ_ARITHMETIC_BINDING_STRATEGY=optimized
   BITZ_FALCON_LIGERITO=ext:3:4 BITZ_FALCON_TERMINAL=standard
   BITZ_REC_OPENER=std BITZ_REC_SECURITY=128 BITZ_REC_LIG=ext:3:4)
run() {
  local name=$1 batch=$2 reps=$3; shift 3
  on_ac; sleep 20
  env $G "$@" BITZ_REC_FALCON_BATCH=$batch BITZ_REC_REPS=$reps RAYON_NUM_THREADS=10 /usr/bin/time -l $BIN > $OUT/$name.out 2> $OUT/$name.stderr
  echo "$(date +%T) $name exit=$? ghz=$(ghz $OUT/$name.stderr) $(pmset -g batt | grep -o 'AC Power\|Battery Power')" >> $OUT/clock.log
}
for batch in 1024 4096 32768; do
  reps=3; [ $batch = 32768 ] && reps=2
  run standard-t18-b$batch $batch $reps BITZ_REC_T=18
  run polylog-t18-b$batch $batch $reps BITZ_REC_T=18 BITZ_FALCON_BRIDGE=polylog
  run standard-t16-b$batch $batch $reps BITZ_REC_T=16
  run polylog-t16-b$batch $batch $reps BITZ_REC_T=16 BITZ_FALCON_BRIDGE=polylog
done
echo DONE > $OUT/DONE

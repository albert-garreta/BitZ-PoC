#!/bin/zsh
# The recursion step with the base proof's opening under unique decoding
# (udrg:1:4, the default) against Johnson over GF(2^256) with the standard
# terminal (BITZ_FALCON_LIGERITO=ext:3:4 BITZ_FALCON_TERMINAL=standard); the
# outer proof at ext:3:4 with the standard opener at T = 18, λ=128, 10
# threads, the two base openings interleaved per batch. ALLOW_BATTERY=1 skips
# the wait for AC. Run under the gate:
#   python3 scripts/bench_gate.py run --label falcon-ext-base -- zsh results/falcon-recursion-ext-base-20261021/campaign.zsh
set -u
W=/Users/albertgarretafontelles/f2z-pcs/.claude/worktrees/falcon-recursion
OUT=$W/bench_results/falcon-recursion-ext-base-20261021/runs
mkdir -p $OUT
D=$W/target-bench/release/deps
BIN=$D/$(ls -t $D | grep -E '^falcon_recursion-[0-9a-f]+$' | head -1)
{
  echo "bin $BIN $(md5 -q $BIN)"
  git -C $W rev-parse HEAD
  pmset -g batt | tail -1
} > $OUT/binaries.txt
on_ac() { [ "${ALLOW_BATTERY:-0}" = 1 ] && return; until pmset -g batt | grep -q "AC Power"; do sleep 60; done; }
ghz() { awk '/ user /{u=$3} /cycles elapsed/{c=$1} END{if(u>0) printf "%.2f", c/u/1e9; else print 0}' $1; }
G=(BITZ_FALCON_CASE_CACHE=$W/bench_results/falcon-recursion-20261018/cases BITZ_FALCON_GRINDING_MODEL=per-site
   BITZ_FALCON_PRIME=small BITZ_ARITHMETIC_RING=keyed BITZ_FALCON_BRIDGE_ROWS=16 BITZ_ARITHMETIC_BINDING_STRATEGY=optimized
   BITZ_REC_OPENER=std BITZ_REC_T=18 BITZ_REC_SECURITY=128 BITZ_REC_LIG=ext:3:4)
run() {
  local name=$1 batch=$2 reps=$3; shift 3
  on_ac; sleep 20
  env $G "$@" BITZ_REC_FALCON_BATCH=$batch BITZ_REC_REPS=$reps RAYON_NUM_THREADS=10 /usr/bin/time -l $BIN > $OUT/$name.out 2> $OUT/$name.stderr
  echo "$(date +%T) $name exit=$? ghz=$(ghz $OUT/$name.stderr) $(pmset -g batt | grep -o 'AC Power\|Battery Power')" >> $OUT/clock.log
}
for batch in 1024 4096 32768; do
  reps=3; [ $batch = 32768 ] && reps=2
  run udr-b$batch $batch $reps
  run ext-b$batch $batch $reps BITZ_FALCON_LIGERITO=ext:3:4 BITZ_FALCON_TERMINAL=standard
done
echo DONE > $OUT/DONE

#!/bin/zsh
# The recursion step with the split relation (the circuit's SHA-256
# compressions as one batch of the compression relation in its block layout,
# the glue as FlockRec on the stub circuit, one commitment and one opening:
# BITZ_REC_SPLIT=1) against the FlockRec step (standard opener), both with
# the ext:3:4 standard-terminal base proof and the ext:3:4 outer ladder,
# λ=128, 10 threads, interleaved per batch. Opening rows set each relation's
# prime width (at most min(113, 128 − rows) bits) and so its proof of work
# per PIOP round (17 bits at ≤ 15 rows, 20 at 18): FlockRec at T = 18 (the
# established configuration) and T = 15; the split with the batch's claim
# rows 16 or 17 (BITZ_REC_SHA_T) and the glue's 15 (BITZ_REC_GLUE_T). The
# split's compression witness generation is reported apart
# (rec_sha_witness_s). ALLOW_BATTERY=1 skips the wait for AC. Run under the
# gate:
#   python3 scripts/bench_gate.py run --label falcon-split -- zsh results/falcon-recursion-split-20261021/campaign.zsh
set -u
W=/Users/albertgarretafontelles/f2z-pcs/.claude/worktrees/falcon-recursion
OUT=$W/bench_results/falcon-recursion-split-20261021/runs
mkdir -p $OUT
D=$W/target-bench/release/deps
BIN=$D/$(ls -t $D | grep -E '^falcon_recursion-[0-9a-f]+$' | head -1)
{
  echo "bin $BIN $(md5 -q $BIN)"
  git -C $W rev-parse HEAD
  git -C $W status --short | head -20
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
  run flockrec-t18-b$batch $batch $reps BITZ_REC_T=18
  run split-16-15-b$batch $batch $reps BITZ_REC_SPLIT=1 BITZ_REC_SHA_T=16 BITZ_REC_GLUE_T=15
  run flockrec-t15-b$batch $batch $reps BITZ_REC_T=15
  run split-17-15-b$batch $batch $reps BITZ_REC_SPLIT=1 BITZ_REC_SHA_T=17 BITZ_REC_GLUE_T=15
done
echo DONE > $OUT/DONE

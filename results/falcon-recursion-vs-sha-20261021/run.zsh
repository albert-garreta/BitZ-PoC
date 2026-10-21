#!/bin/zsh
# The Falcon recursion step's FlockRec proof against the standalone SHA-256
# compressions prover (benches/sha256_compressions.rs: repeated Spartan
# relation + virtual BitZ opening) at matched compression counts, with BitZ's
# phase trace (BITZ_TRACE=1). Same features on both (recursion => sha256-fs
# transcripts and SHA-256 Merkle), 10 threads, AC power; run under the gate:
#   python3 scripts/bench_gate.py run --label falcon-vs-sha -- zsh results/falcon-recursion-vs-sha-20261021/run.zsh
set -u
W=/Users/albertgarretafontelles/f2z-pcs/.claude/worktrees/falcon-recursion
OUT=$W/bench_results/falcon-recursion-vs-sha-20261021
mkdir -p $OUT
D=$W/target-bench/release/deps
SHA=$D/$(ls -t $D | grep -E '^sha256_compressions-[0-9a-f]+$' | head -1)
REC=$D/$(ls -t $D | grep -E '^falcon_recursion-[0-9a-f]+$' | head -1)
{
  echo "sha $SHA $(md5 -q $SHA)"
  echo "rec $REC $(md5 -q $REC)"
  git -C $W rev-parse HEAD
  pmset -g batt | tail -1
} > $OUT/binaries.txt
on_ac() { until pmset -g batt | grep -q "AC Power"; do sleep 60; done; }
ghz() { awk '/ user /{u=$3} /cycles elapsed/{c=$1} END{if(u>0) printf "%.2f", c/u/1e9; else print 0}' $1; }
run() {
  local name=$1; shift
  on_ac; sleep 20
  env "$@" RAYON_NUM_THREADS=10 /usr/bin/time -l $BIN > $OUT/$name.out 2> $OUT/$name.stderr
  echo "$(date +%T) $name exit=$? ghz=$(ghz $OUT/$name.stderr)" >> $OUT/clock.log
}
# PARTS selects the provers to run (default both); the standalone bench
# rejects unknown BITZ_* variables, so it traces its spans to a jsonl file.
PARTS=${PARTS:-sha rec}
if [[ " $PARTS " == *" sha "* ]]; then
  BIN=$SHA
  for spec in l128-s13:128:13 l128-s14:128:14 l100-s13:100:13; do
    parts=(${(s/:/)spec})
    run sha-$parts[1] BITZ_BENCH_LAMBDA=$parts[2] BITZ_BENCH_SHAPES=$parts[3] BITZ_BENCH_REPS=3 \
      BITZ_SHA_TRACE_PATH=$OUT/sha-$parts[1].trace.jsonl
  done
fi
if [[ " $PARTS " == *" rec "* ]]; then
  BIN=$REC
  G=(BITZ_FALCON_CASE_CACHE=$W/bench_results/falcon-recursion-20261018/cases BITZ_FALCON_GRINDING_MODEL=per-site
     BITZ_FALCON_PRIME=small BITZ_ARITHMETIC_RING=keyed BITZ_FALCON_BRIDGE_ROWS=16 BITZ_ARITHMETIC_BINDING_STRATEGY=optimized
     BITZ_REC_OPENER=std BITZ_REC_T=18 BITZ_REC_SECURITY=128 BITZ_REC_LIG=ext:3:4 BITZ_TRACE=1)
  run rec-b1024 $G BITZ_REC_FALCON_BATCH=1024 BITZ_REC_REPS=3
  run rec-b32768 $G BITZ_REC_FALCON_BATCH=32768 BITZ_REC_REPS=2
fi
echo DONE > $OUT/DONE

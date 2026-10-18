#!/bin/zsh
# Falcon-512 recursion campaign (configuration G). Run under the bench gate:
#   python3 scripts/bench_gate.py run --label falcon-recursion -- zsh bench_results/falcon-recursion-20261018/campaign.zsh
# rec cells: falcon_recursion (sha256-fs base proof + one FlockRec recursion step + final verify);
# base cells: falcon_algebraic built without sha256-fs (the production Blake3 transcript).
set -u
W=/Users/albertgarretafontelles/f2z-pcs/.claude/worktrees/falcon-recursion
OUT=$W/bench_results/falcon-recursion-20261018/runs
mkdir -p $OUT
REC=$(ls -t $W/target-bench/release/deps/falcon_recursion-* | grep -v '\.d$' | head -1)
BASE=$(ls -t $W/target-blake3/release/deps/falcon_algebraic-* | grep -v '\.d$' | head -1)
{
  echo "$REC $(md5 -q $REC)"
  echo "$BASE $(md5 -q $BASE)"
  git -C $W rev-parse HEAD
  git -C $W status --short | grep -v '^??'
  pmset -g batt | tail -1
} > $OUT/binaries.txt
export BITZ_FALCON_CASE_CACHE=$W/bench_results/falcon-recursion-20261018/cases
export BITZ_FALCON_GRINDING_MODEL=per-site BITZ_FALCON_PRIME=small BITZ_ARITHMETIC_RING=keyed
export BITZ_FALCON_BRIDGE_ROWS=16 BITZ_ARITHMETIC_BINDING_STRATEGY=optimized
ghz() { awk '/ user /{u=$3} /cycles elapsed/{c=$1} END{if(u>0) printf "%.2f", c/u/1e9; else print 0}' $1; }
log() {
  echo "$(date +%T) $1 exit=$2 ghz=$(ghz $OUT/$1.stderr) rss=$(awk '/maximum resident set size/{printf "%.0f", $1/1048576}' $OUT/$1.stderr)MiB $(pmset -g batt | grep -o '[0-9]*%; [a-z ]*')" >> $OUT/clock.log
}
rec() {
  local name=$1 sec=$2 batch=$3 threads=$4 reps=$5 nap=$6
  local lig=udrg:3:4 t=18
  [ $sec = 100 ] && { lig=custom:6:4; t=19; }
  sleep $nap
  BITZ_REC_SECURITY=$sec BITZ_REC_LIG=$lig BITZ_REC_T=$t BITZ_REC_FALCON_BATCH=$batch BITZ_REC_REPS=$reps \
    RAYON_NUM_THREADS=$threads /usr/bin/time -l $REC > $OUT/$name.out 2> $OUT/$name.stderr
  log $name $?
}
base() {
  local name=$1 sec=$2 batch=$3 threads=$4 iters=$5 nap=$6
  sleep $nap
  RAYON_NUM_THREADS=$threads /usr/bin/time -l $BASE --degree 512 --security $sec --threads $threads \
    --batch $batch --warmup 1 --iterations $iters > $OUT/$name.jsonl 2> $OUT/$name.stderr
  log $name $?
}
for sec in 128 100; do
  for b in 1024 4096 32768; do
    reps=3; [ $b = 32768 ] && reps=2
    rec rec-s$sec-b$b-t10 $sec $b 10 $reps 15
    base base-s$sec-b$b-t10 $sec $b 10 3 15
  done
done
for sec in 128 100; do
  for b in 1024 4096 32768; do
    reps=2; [ $b = 1024 ] || reps=1
    rec rec-s$sec-b$b-t1 $sec $b 1 $reps 15
    base base-s$sec-b$b-t1 $sec $b 1 $reps 15
  done
done
echo DONE > $OUT/DONE

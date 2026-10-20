#!/bin/zsh
# Falcon-512 recursion A/B campaign (configuration G): the 2026-10-18 binary
# (c2c14ae24) against the optimized one (boxed FlockRec layout with structured
# opening weights and the lazy verifier, forked Merkle recording, shared
# recorder terms), interleaved per cell with alternating order, plus the
# production base proof. Run under the bench gate:
#   python3 scripts/bench_gate.py run --label falcon-recursion-ab -- zsh results/falcon-recursion-20261020/campaign.zsh
# Every cell waits for AC power (battery clocks are 2-3 GHz).
set -u
W=/Users/albertgarretafontelles/f2z-pcs/.claude/worktrees/falcon-recursion
OUT=$W/bench_results/falcon-recursion-20261020/runs
mkdir -p $OUT
D=$W/target-bench/release/deps
NEW=$D/$(ls -t $D | grep -E '^falcon_recursion-[0-9a-f]+$' | head -1)
OLD=$W/target-bench/ab/falcon_recursion-c2c14ae
D=$W/target-blake3/release/deps
BASE=$D/$(ls -t $D | grep -E '^falcon_algebraic-[0-9a-f]+$' | head -1)
{
  echo "new $NEW $(md5 -q $NEW)"
  echo "old $OLD $(md5 -q $OLD)"
  echo "base $BASE $(md5 -q $BASE)"
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
on_ac() {
  until pmset -g batt | grep -q "AC Power"; do
    echo "$(date +%T) waiting for AC power" >> $OUT/clock.log
    sleep 60
  done
}
rec() {
  local name=$1 bin=$2 sec=$3 batch=$4 threads=$5 reps=$6 nap=$7
  local lig=udrg:3:4 t=18
  [ $sec = 100 ] && { lig=custom:6:4; t=19; }
  on_ac
  sleep $nap
  BITZ_REC_SECURITY=$sec BITZ_REC_LIG=$lig BITZ_REC_T=$t BITZ_REC_FALCON_BATCH=$batch BITZ_REC_REPS=$reps \
    RAYON_NUM_THREADS=$threads /usr/bin/time -l $bin > $OUT/$name.out 2> $OUT/$name.stderr
  log $name $?
}
base() {
  local name=$1 sec=$2 batch=$3 threads=$4 iters=$5 nap=$6
  on_ac
  sleep $nap
  RAYON_NUM_THREADS=$threads /usr/bin/time -l $BASE --degree 512 --security $sec --threads $threads \
    --batch $batch --warmup 1 --iterations $iters > $OUT/$name.jsonl 2> $OUT/$name.stderr
  log $name $?
}
# Alternate which binary runs first, cell by cell.
cell=0
pair() {
  local sec=$1 b=$2 threads=$3 reps=$4
  if (( cell % 2 == 0 )); then
    rec rec-new-s$sec-b$b-t$threads $NEW $sec $b $threads $reps 20
    rec rec-old-s$sec-b$b-t$threads $OLD $sec $b $threads $reps 20
  else
    rec rec-old-s$sec-b$b-t$threads $OLD $sec $b $threads $reps 20
    rec rec-new-s$sec-b$b-t$threads $NEW $sec $b $threads $reps 20
  fi
  cell=$((cell + 1))
}
for sec in 128 100; do
  for b in 1024 4096 32768; do
    reps=3; [ $b = 32768 ] && reps=2
    pair $sec $b 10 $reps
    base base-s$sec-b$b-t10 $sec $b 10 3 20
  done
done
for sec in 128 100; do
  for b in 1024 4096 32768; do
    reps=2; [ $b = 1024 ] || reps=1
    pair $sec $b 1 $reps
  done
done
echo DONE > $OUT/DONE

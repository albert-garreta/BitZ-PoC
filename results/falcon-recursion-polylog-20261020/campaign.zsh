#!/bin/zsh
# Falcon-512 recursion: the outer proof opened by BitZ logarithmic at the
# reference split (BITZ_REC_OPENER=log, the default) against standard BitZ at
# T = 18/19 (BITZ_REC_OPENER=std), the same binary, interleaved per cell with
# alternating order. Run under the bench gate (launch.zsh waits for AC power):
#   zsh results/falcon-recursion-polylog-20261020/launch.zsh
set -u
W=/Users/albertgarretafontelles/f2z-pcs/.claude/worktrees/falcon-recursion
OUT=$W/bench_results/falcon-recursion-polylog-20261020/runs
mkdir -p $OUT
D=$W/target-bench/release/deps
BIN=$D/$(ls -t $D | grep -E '^falcon_recursion-[0-9a-f]+$' | head -1)
{
  echo "bin $BIN $(md5 -q $BIN)"
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
  local name=$1 opener=$2 sec=$3 batch=$4 threads=$5 reps=$6 nap=$7
  local lig=udrg:3:4 t=18
  [ $sec = 100 ] && { lig=custom:6:4; t=19; }
  on_ac
  sleep $nap
  if [ $opener = std ]; then
    BITZ_REC_OPENER=std BITZ_REC_T=$t BITZ_REC_SECURITY=$sec BITZ_REC_LIG=$lig BITZ_REC_FALCON_BATCH=$batch BITZ_REC_REPS=$reps \
      RAYON_NUM_THREADS=$threads /usr/bin/time -l $BIN > $OUT/$name.out 2> $OUT/$name.stderr
  else
    BITZ_REC_OPENER=log BITZ_REC_SECURITY=$sec BITZ_REC_LIG=$lig BITZ_REC_FALCON_BATCH=$batch BITZ_REC_REPS=$reps \
      RAYON_NUM_THREADS=$threads /usr/bin/time -l $BIN > $OUT/$name.out 2> $OUT/$name.stderr
  fi
  log $name $?
}
cell=0
pair() {
  local sec=$1 b=$2 threads=$3 reps=$4
  if (( cell % 2 == 0 )); then
    rec rec-log-s$sec-b$b-t$threads log $sec $b $threads $reps 20
    rec rec-std-s$sec-b$b-t$threads std $sec $b $threads $reps 20
  else
    rec rec-std-s$sec-b$b-t$threads std $sec $b $threads $reps 20
    rec rec-log-s$sec-b$b-t$threads log $sec $b $threads $reps 20
  fi
  cell=$((cell + 1))
}
for sec in 128 100; do
  for b in 1024 4096 32768; do
    reps=3; [ $b = 32768 ] && reps=2
    pair $sec $b 10 $reps
  done
done
for sec in 128 100; do
  for b in 1024 4096 32768; do
    reps=2; [ $b = 1024 ] || reps=1
    pair $sec $b 1 $reps
  done
done
echo DONE > $OUT/DONE

#!/bin/zsh
# Falcon-512 recursion at λ=128: the outer proof's Ligerito ladder, unique
# decoding (udrg:3:4, the current one) against Johnson over GF(2^256) at
# rates 1/8, 1/32, 1/64 (ext:3:4, ext:5:4, ext:6:4), all with the standard
# BitZ opener at T = 18; one binary, the configurations interleaved per cell
# with rotating order. Run with launch.zsh (waits for AC, then the gate).
set -u
W=/Users/albertgarretafontelles/f2z-pcs/.claude/worktrees/falcon-recursion
OUT=$W/bench_results/falcon-recursion-ext-20261021/runs
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
  local name=$1 lig=$2 batch=$3 threads=$4 reps=$5
  on_ac
  sleep 20
  BITZ_REC_OPENER=std BITZ_REC_T=18 BITZ_REC_SECURITY=128 BITZ_REC_LIG=$lig BITZ_REC_FALCON_BATCH=$batch BITZ_REC_REPS=$reps \
    RAYON_NUM_THREADS=$threads /usr/bin/time -l $BIN > $OUT/$name.out 2> $OUT/$name.stderr
  log $name $?
}
configs=(udrg:3:4 ext:3:4 ext:5:4 ext:6:4)
cell=0
round() {
  local b=$1 threads=$2 reps=$3
  local n=${#configs[@]}
  for k in $(seq 0 $((n - 1))); do
    local lig=${configs[$(( (k + cell) % n + 1 ))]}
    rec rec-${lig//:/_}-b$b-t$threads $lig $b $threads $reps
  done
  cell=$((cell + 1))
}
for b in 1024 4096 32768; do
  reps=3; [ $b = 32768 ] && reps=2
  round $b 10 $reps
done
for b in 1024 4096 32768; do
  reps=2; [ $b = 1024 ] || reps=1
  round $b 1 $reps
done
echo DONE > $OUT/DONE

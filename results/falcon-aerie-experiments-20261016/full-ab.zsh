#!/bin/zsh
# Union-bound vs per-site grinding on FULL Falcon (in-circuit HashToPoint),
# same binary, interleaved. Seed-42 inputs from one case cache.
set -u
W=/Users/albertgarretafontelles/f2z-pcs-falcon-aerie
SP=/private/tmp/claude-501/-Users-albertgarretafontelles-f2z-pcs/f86c1287-b3ca-4064-a33a-28092208da16/scratchpad
OUT=$W/bench_results/aerie-20261016/runs-full
mkdir -p $OUT
BF=$(ls -t $W/target-native/release/deps/falcon_hybrid-* | grep -v '\.d$' | head -1)
{ echo "$BF $(md5 -q $BF)"; pmset -g batt | tail -1; } > $OUT/binaries.txt
export BITZ_FALCON_CASE_CACHE=$SP/falcon-cases
ghz() { awk '/ user /{u=$3} /cycles elapsed/{c=$1} END{if(u>0) printf "%.2f", c/u/1e9; else print 0}' $1; }
run() {
  local name=$1 model=$2 t=$3 nap=$4; shift 4
  sleep $nap
  BITZ_FALCON_GRINDING_MODEL=$model RAYON_NUM_THREADS=$t /usr/bin/time -l $BF --bench "$@" --threads $t > $OUT/$name.jsonl 2> $OUT/$name.time
  echo "$(date +%T) $name ghz=$(ghz $OUT/$name.time) real=$(awk '/ real /{print $1}' $OUT/$name.time) $(pmset -g batt | grep -o '[0-9]*%; [a-z ]*')" >> $OUT/clock.log
}
echo "== reports $(date)"
for m in union per-site; do
  BITZ_FALCON_PRINT_GRINDING=1 BITZ_FALCON_GRINDING_MODEL=$m RAYON_NUM_THREADS=10 $BF --bench --degree 512 --security 128 --batch 1024 --threads 10 --warmup 0 --iterations 1 > $OUT/report-$m-512-b1024.jsonl 2> $OUT/report-$m-512-b1024.txt
done
echo "== timed $(date)"
typeset -A reps
for m in union per-site per-site union; do
  reps[$m]=$(( ${reps[$m]:-0} + 1 ))
  run full512-b1024-t10-$m-r${reps[$m]} $m 10 20 --degree 512 --security 128 --batch 1024 --warmup 1 --iterations 4
done
for m in union per-site; do
  run full512-b1-t10-$m $m 10 12 --degree 512 --security 128 --batch 1 --warmup 5 --iterations 20
done
for m in union per-site; do
  run full512-b1024-t1-$m $m 1 20 --degree 512 --security 128 --batch 1024 --warmup 1 --iterations 3
done
echo FULL-AB-FINISHED > $OUT/DONE

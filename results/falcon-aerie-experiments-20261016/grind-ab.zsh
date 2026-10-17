#!/bin/zsh
# Union-bound (John's default) vs per-site (Aerie-style) grinding on algebraic
# Falcon, same binary, interleaved. Seed-42 inputs from one case cache.
set -u
W=/Users/albertgarretafontelles/f2z-pcs-falcon-aerie
SP=/private/tmp/claude-501/-Users-albertgarretafontelles-f2z-pcs/f86c1287-b3ca-4064-a33a-28092208da16/scratchpad
OUT=$W/bench_results/aerie-20261016/runs-grind
mkdir -p $OUT
BA=$(ls -t $W/target-native/release/deps/falcon_algebraic-* | grep -v '\.d$' | head -1)
{ echo "$BA $(md5 -q $BA)"; pmset -g batt | tail -1; } > $OUT/binaries.txt
export BITZ_FALCON_CASE_CACHE=$SP/falcon-cases
mkdir -p $BITZ_FALCON_CASE_CACHE
ghz() { awk '/ user /{u=$3} /cycles elapsed/{c=$1} END{if(u>0) printf "%.2f", c/u/1e9; else print 0}' $1; }
run() {
  local name=$1 model=$2 t=$3 nap=$4; shift 4
  sleep $nap
  BITZ_FALCON_GRINDING_MODEL=$model RAYON_NUM_THREADS=$t /usr/bin/time -l $BA "$@" --threads $t > $OUT/$name.jsonl 2> $OUT/$name.time
  echo "$(date +%T) $name ghz=$(ghz $OUT/$name.time) real=$(awk '/ real /{print $1}' $OUT/$name.time) $(pmset -g batt | grep -o '[0-9]*%; [a-z ]*')" >> $OUT/clock.log
}
echo "== reports $(date)"
for m in union per-site; do
  for b in 1 1024; do
    BITZ_FALCON_PRINT_GRINDING=1 BITZ_FALCON_GRINDING_MODEL=$m RAYON_NUM_THREADS=10 $BA --degree 512 --batch $b --security 128 --threads 10 --warmup 0 --iterations 1 > $OUT/report-$m-512-b$b.jsonl 2> $OUT/report-$m-512-b$b.txt
  done
  BITZ_FALCON_PRINT_GRINDING=1 BITZ_FALCON_GRINDING_MODEL=$m RAYON_NUM_THREADS=10 $BA --degree 1024 --batch 1024 --security 128 --threads 10 --warmup 0 --iterations 1 > $OUT/report-$m-1024-b1024.jsonl 2> $OUT/report-$m-1024-b1024.txt
done
echo "== timed $(date)"
typeset -A reps
for m in union per-site per-site union; do
  reps[$m]=$(( ${reps[$m]:-0} + 1 ))
  run alg512-b1024-t10-$m-r${reps[$m]} $m 10 12 --degree 512 --batch 1024 --security 128 --warmup 2 --iterations 10
done
for m in union per-site; do
  run alg512-b1024-t1-$m $m 1 12 --degree 512 --batch 1024 --security 128 --warmup 1 --iterations 5
done
for m in union per-site per-site union; do
  reps[b1$m]=$(( ${reps[b1$m]:-0} + 1 ))
  run alg512-b1-t10-$m-r${reps[b1$m]} $m 10 8 --degree 512 --batch 1 --security 128 --warmup 5 --iterations 20
done
for m in union per-site; do
  run alg1024-b1024-t10-$m $m 10 12 --degree 1024 --batch 1024 --security 128 --warmup 1 --iterations 5
done
echo "== traces $(date)"
for m in union per-site; do
  sleep 12
  BITZ_FALCON_GRINDING_MODEL=$m RAYON_NUM_THREADS=10 $BA --degree 512 --batch 1024 --security 128 --threads 10 --warmup 2 --iterations 6 --trace > $OUT/trace-$m-b1024-t10.jsonl 2> $OUT/trace-$m-b1024-t10.stderr
  sleep 12
  BITZ_FALCON_GRINDING_MODEL=$m RAYON_NUM_THREADS=1 $BA --degree 512 --batch 1024 --security 128 --threads 1 --warmup 1 --iterations 3 --trace > $OUT/trace-$m-b1024-t1.jsonl 2> $OUT/trace-$m-b1024-t1.stderr
done
echo GRIND-AB-FINISHED > $OUT/DONE

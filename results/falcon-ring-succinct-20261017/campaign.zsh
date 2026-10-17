#!/bin/zsh
# Keyed ring check and bridge row variables (t) on falcon-ring-succinct.
# Falcon-512 @128, per-site grinding, optimized binder, 10 threads unless noted.
set -u
W=/Users/albertgarretafontelles/f2z-pcs-falcon-ring
OUT=$W/bench_results/ring-20261017/runs
mkdir -p $OUT
BIN=$(ls -t $W/target-native/release/deps/falcon_algebraic-* | grep -v '\.d$' | head -1)
{ echo "$BIN $(md5 -q $BIN)"; git -C $W rev-parse HEAD; git -C $W status --short | grep -v '^??'; pmset -g batt | tail -1; } > $OUT/binaries.txt
export BITZ_FALCON_CASE_CACHE=$W/bench_results/ring-20261017/cases
mkdir -p $BITZ_FALCON_CASE_CACHE
export BITZ_FALCON_GRINDING_MODEL=per-site BITZ_ARITHMETIC_BINDING_STRATEGY=optimized
ghz() { awk '/ user /{u=$3} /cycles elapsed/{c=$1} END{if(u>0) printf "%.2f", c/u/1e9; else print 0}' $1; }
run() {
  local name=$1 prime=$2 ring=$3 rows=$4 threads=$5 nap=$6; shift 6
  sleep $nap
  BITZ_FALCON_PRIME=$prime BITZ_ARITHMETIC_RING=$ring BITZ_FALCON_BRIDGE_ROWS=$rows \
    RAYON_NUM_THREADS=$threads /usr/bin/time -l $BIN --degree 512 --security 128 \
    --threads $threads "$@" > $OUT/$name.jsonl 2> $OUT/$name.stderr
  local rc=$?
  echo "$(date +%T) $name exit=$rc ghz=$(ghz $OUT/$name.stderr) rss=$(awk '/maximum resident set size/{printf "%.0f", $1/1048576}' $OUT/$name.stderr)MiB $(pmset -g batt | grep -o '[0-9]*%; [a-z ]*')" >> $OUT/clock.log
}
# Generate (and cache) the 32K and 1K inputs once, untimed.
for b in 1024 32768; do
  BITZ_FALCON_GRINDING_MODEL=per-site RAYON_NUM_THREADS=10 $BIN --degree 512 --security 128 --threads 10 \
    --batch $b --warmup 0 --iterations 1 > $OUT/gen-$b.jsonl 2> $OUT/gen-$b.stderr
done
B=(--batch 32768 --warmup 1 --iterations 3)
run A-default-canonical-t13 default canonical 13 10 15 $B
run B-default-keyed-t13 default keyed 13 10 15 $B
run C-small-canonical-t13 small canonical 13 10 15 $B
run D-small-keyed-t13 small keyed 13 10 15 $B
run E-small-keyed-t14 small keyed 14 10 15 $B
run F-small-keyed-t15 small keyed 15 10 15 $B
run G-small-keyed-t16 small keyed 16 10 15 $B
run H-default-keyed-t16 default keyed 16 10 15 $B
run I-default-canonical-t16 default canonical 16 10 15 $B
S=(--batch 1024 --warmup 2 --iterations 10)
run A1k-default-canonical-t13 default canonical 13 10 10 $S
run B1k-default-keyed-t13 default keyed 13 10 10 $S
run C1k-small-canonical-t13 small canonical 13 10 10 $S
run D1k-small-keyed-t13 small keyed 13 10 10 $S
T=(--batch 32768 --warmup 0 --iterations 1)
run A1t-default-canonical-t13 default canonical 13 1 15 $T
run D1t-small-keyed-t13 small keyed 13 1 15 $T
run G1t-small-keyed-t16 small keyed 16 1 15 $T
sleep 15
BITZ_FALCON_PRIME=small BITZ_ARITHMETIC_RING=keyed BITZ_FALCON_BRIDGE_ROWS=13 RAYON_NUM_THREADS=10 \
  $BIN --degree 512 --security 128 --threads 10 --batch 32768 --warmup 1 --iterations 2 --trace \
  > $OUT/trace-D.jsonl 2> $OUT/trace-D.stderr
echo "trace exit=$?" >> $OUT/clock.log
echo DONE > $OUT/DONE

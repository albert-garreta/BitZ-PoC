#!/bin/zsh
# Single-thread sampled prover profiles at 32768 Falcon-512 signatures, 128-bit:
# Aerie v0.2.0 (as published) and BitZ (John's falcon-poc e140a3f3 as shipped,
# and with per-site grinding + the one-limb small-prime bridge). Each profiled
# process proves five times under macOS `sample` (1 ms); the prover phases are
# later selected by their marker frames. Unprofiled runs of the same setups
# give the per-prove wall times the flamegraph widths are scaled to.
set -u
D=/Users/albertgarretafontelles/f2z-pcs/.claude/worktrees/falcon-flamegraph/bench_results/flamegraph-20261017
W=/Users/albertgarretafontelles/f2z-pcs/.claude/worktrees/falcon-flamegraph
OUT=$D/samples
mkdir -p $OUT
ABIN=$D/aerie/target/release/deps/falcon-6efadfd380be1a7d
BBIN=$W/target-native/release/deps/falcon_algebraic-8edd41f39df7f30c
{ echo "$ABIN $(md5 -q $ABIN)"; echo "$BBIN $(md5 -q $BBIN)"; pmset -g batt | tail -1; } > $OUT/binaries.txt
export AERIE_HONEST_FIXTURE_DIR=$D/aerie-fixtures
export BITZ_FALCON_CASE_CACHE=$D/falcon-cases
mkdir -p $BITZ_FALCON_CASE_CACHE
ghz() { awk '/ user /{u=$3} /cycles elapsed/{c=$1} END{if(u>0) printf "%.2f", c/u/1e9; else print 0}' $1; }

# Generate (and cache) the BitZ input cases once, untimed, at 10 threads.
if [ ! -e $OUT/bitz-gen.jsonl ]; then
  RAYON_NUM_THREADS=10 $BBIN --degree 512 --batch 32768 --security 128 --threads 10 --warmup 0 --iterations 1 > $OUT/bitz-gen.jsonl 2> $OUT/bitz-gen.stderr
  echo "$(date +%T) bitz case cache exit=$?" >> $OUT/clock.log
fi

export AERIE_PROFILE_IN_POOL=1 BITZ_PROFILE_IN_POOL=1 RAYON_NUM_THREADS=1
A=(32768 32768 32768 32768 32768)
B=(--degree 512 --batch 32768 --security 128 --threads 1 --warmup 0)
ONELIMB=(env BITZ_FALCON_GRINDING_MODEL=per-site BITZ_FALCON_PRIME=small)

profile() {
  local name=$1; shift
  sleep 20
  "$@" > $OUT/$name.out 2> $OUT/$name.err &
  local pid=$!
  /usr/bin/sample $pid 400 1 -mayDie -file $OUT/$name.sample.txt > $OUT/$name.sample.log 2>&1
  wait $pid
  echo "$(date +%T) $name profiled exit=$? $(pmset -g batt | grep -o '[0-9]*%; [a-z ]*')" >> $OUT/clock.log
}

clockcheck() {
  local name=$1; shift
  sleep 20
  /usr/bin/time -l "$@" > $OUT/$name.clock.out 2> $OUT/$name.clock.err
  echo "$(date +%T) $name clock exit=$? ghz=$(ghz $OUT/$name.clock.err) $(pmset -g batt | grep -o '[0-9]*%; [a-z ]*')" >> $OUT/clock.log
}

profile aerie $ABIN $A
profile bitz-shipped $BBIN $B --iterations 5
profile bitz-onelimb $ONELIMB $BBIN $B --iterations 5
clockcheck aerie $ABIN 32768 32768
clockcheck bitz-shipped $BBIN $B --iterations 2
clockcheck bitz-onelimb $ONELIMB $BBIN $B --iterations 2
echo DONE > $OUT/DONE

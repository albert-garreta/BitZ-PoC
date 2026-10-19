#!/bin/zsh
# usage: run.zsh <bin> <tag>   -- 1-thread traced (2 proves) then 10-thread (warmup 1 + 5 proves), setup G
set -u
BIN=$1; TAG=$2
R=/Users/albertgarretafontelles/f2z-pcs/.claude/worktrees/falcon-piop-opt/bench_results/piop-opt-20261019
export BITZ_FALCON_CASE_CACHE=/Users/albertgarretafontelles/f2z-pcs/.claude/worktrees/falcon-flamegraph/bench_results/flamegraph-20261017/falcon-cases
export BITZ_FALCON_GRINDING_MODEL=per-site BITZ_FALCON_PRIME=small BITZ_ARITHMETIC_RING=keyed BITZ_FALCON_BRIDGE_ROWS=16
B=(--degree 512 --batch 32768 --security 128)
ghz() { awk '/ user /{u=$3} /cycles elapsed/{c=$1} END{if(u>0) printf "%.2f", c/u/1e9; else print 0}' $1; }
export RAYON_NUM_THREADS=1
/usr/bin/time -l $BIN $B --threads 1 --warmup 0 --iterations 2 --trace > $R/t1-$TAG.out 2> $R/t1-$TAG.err
echo "$(date +%T) t1-$TAG exit=$? ghz=$(ghz $R/t1-$TAG.err) $(pmset -g batt | grep -o '[0-9]*%; [a-zA-Z ]*')" >> $R/log
sleep 5
export RAYON_NUM_THREADS=10 BITZ_ARITHMETIC_BINDING_STRATEGY=optimized
/usr/bin/time -l $BIN $B --threads 10 --warmup 1 --iterations 5 > $R/t10-$TAG.out 2> $R/t10-$TAG.err
echo "$(date +%T) t10-$TAG exit=$? ghz=$(ghz $R/t10-$TAG.err) rss=$(awk '/maximum resident set size/{printf "%.0f", $1/1048576}' $R/t10-$TAG.err)MiB" >> $R/log

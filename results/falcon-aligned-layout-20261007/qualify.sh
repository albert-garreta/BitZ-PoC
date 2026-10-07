#!/usr/bin/env bash
set -euo pipefail
cd /home/john-wu/code/BitZ-pcs
result_root=/tmp/falcon-layout-validation
mkdir -p "$result_root" /tmp/falcon-layout-candidate
test_binary=/tmp/falcon-simplify-candidate-target/release/deps/bitz-b0347a22deb10013
profile_binary=/tmp/falcon-simplify-candidate-target/release/deps/falcon_profiles-1471e718c1d4d65d
export RAYON_NUM_THREADS=16
"$test_binary" piop::spartan::falcon --test-threads=1 > "$result_root/falcon-tests-16t.log" 2>&1
"$test_binary" hybrid::joint_sumcheck --test-threads=1 > "$result_root/joint-tests-16t.log" 2>&1
"$test_binary" hybrid::integer_bridge --test-threads=1 > "$result_root/bridge-tests-16t.log" 2>&1
"$profile_binary" --test-threads=1 > "$result_root/profiles-16t.log" 2>&1
"$test_binary" large_batches_at_both_security_targets --ignored --test-threads=1 > "$result_root/algebraic-large-16t.log" 2>&1
RAYON_NUM_THREADS=1 "$test_binary" piop::spartan::falcon1024_algebraic --test-threads=1 > "$result_root/algebraic-tests-1t.log" 2>&1
RAYON_NUM_THREADS=1 "$test_binary" hybrid_falcon_128_roundtrip --test-threads=1 > "$result_root/full-roundtrip-1t.log" 2>&1
cp /tmp/falcon-simplify-candidate-target/release/examples/falcon_algebraic /tmp/falcon-layout-candidate/falcon_algebraic
cp /tmp/falcon-simplify-candidate-target/release/deps/falcon_hybrid-0d2856552ca243a9 /tmp/falcon-layout-candidate/falcon_hybrid
python3 /tmp/falcon-layout-benchmark/run_comparison.py \
  --candidate-algebraic /tmp/falcon-layout-candidate/falcon_algebraic \
  --candidate-full /tmp/falcon-layout-candidate/falcon_hybrid \
  --out /tmp/falcon-layout-results
python3 /tmp/falcon-layout-benchmark/run_binding_diagnostics.py \
  --candidate /tmp/falcon-layout-candidate \
  --out /tmp/falcon-layout-results/binding-diagnostics

#!/usr/bin/env bash
set -euo pipefail
# Run from the repository root; isolate this dependency-free field module.
task_dir=$(mktemp -d)
trap 'rm -rf "$task_dir"' EXIT
mkdir "$task_dir/q12289"
cp vendor/field/src/q12289.rs "$task_dir/q12289.rs"
cp vendor/field/src/q12289/avx512.rs "$task_dir/q12289/avx512.rs"
printf 'mod q12289;\n' > "$task_dir/field.rs"
rustc --edition 2024 --test -O -Dunsafe_op_in_unsafe_fn "$task_dir/field.rs" -o "$task_dir/generic"
"$task_dir/generic"
"$task_dir/generic" --ignored
rustc --edition 2024 --test -O -C target-cpu=native -Dunsafe_op_in_unsafe_fn "$task_dir/field.rs" -o "$task_dir/native"
"$task_dir/native"

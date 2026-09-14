// Private production baselines, extracted verbatim; no production visibility edits.
use std::{env, fs, path::PathBuf};
fn section(source: &str, start: &str, end: &str) -> String {
    let begin = source.find(start).expect("baseline start marker changed");
    let stop = source[begin..]
        .find(end)
        .expect("baseline end marker changed")
        + begin;
    source[begin..stop].to_owned()
}
fn main() {
    println!("cargo:rerun-if-env-changed=CARGO_FEATURE_ARITHMETIC_CAMPAIGN");
    if env::var_os("CARGO_FEATURE_ARITHMETIC_CAMPAIGN").is_none() {
        return;
    }
    let root = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap()).join("../..");
    let out = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    let raw_path = root.join("src/piop/spartan/raw_monty.rs");
    let p256_path = root.join("crates/circuit/src/p256.rs");
    for p in [&raw_path, &p256_path] {
        println!("cargo:rerun-if-changed={}", p.display());
    }
    let raw = fs::read_to_string(raw_path).unwrap();
    let preamble = "use crypto_bigint::modular::FixedMontyParams;\nuse crypto_primitives::{FromWithConfig, crypto_bigint_monty::MontyField, crypto_bigint_uint::Uint as FieldUint};\nuse crate::production_delayed::MontyLinearAccumulator128;\ntype Field = MontyField<2>;\ntype FieldConfig = FixedMontyParams<2>;\ntype Raw = u128;\n";
    let body = section(
        &raw,
        "#[inline(always)]\nconst fn words_to_raw",
        "// ---------------------------------------------------------------------------\n// Equality tables",
    );
    fs::write(out.join("raw_ctx.rs"), format!("{preamble}{body}")).unwrap();
    let p256 = fs::read_to_string(p256_path).unwrap();
    let body = section(&p256, "fn multiply_wide(left: Wide<9>", "fn widen_u256");
    fs::write(out.join("p256_mul.rs"), format!("#[derive(Clone, Copy)]\nstruct Wide<const L: usize>([u64; L]);\n{body}\npub fn product(a: [u64;9], b: [u64;9]) -> [u64;18] {{ multiply_wide(Wide(a), Wide(b)).0 }}\n")).unwrap();
}

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
fn function(source: &str, name: &str) -> String {
    let position = source
        .find(&format!("fn {name}("))
        .expect("function marker changed");
    let begin = source[..position].rfind('\n').unwrap() + 1;
    let body = position + source[position..].find('{').unwrap();
    let mut depth = 0;
    for (i, ch) in source[body..].char_indices() {
        if ch == '{' {
            depth += 1;
        }
        if ch == '}' {
            depth -= 1;
            if depth == 0 {
                return source[begin..body + i + 1].to_owned();
            }
        }
    }
    panic!("function end changed");
}
fn main() {
    println!("cargo:rustc-check-cfg=cfg(field_regression_probe)");
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
    let sumcheck_path = root.join("src/piop/spartan/sumcheck.rs");
    println!("cargo:rerun-if-changed={}", sumcheck_path.display());
    let sumcheck = fs::read_to_string(sumcheck_path).unwrap();
    let body = section(
        &sumcheck,
        "pub(super) fn batch_invert_nonzero",
        "#[inline]\npub(super) fn equality_coordinate_evaluation",
    );
    fs::write(out.join("batch_inverse.rs"), format!("use f2z::piop::spartan::SpartanField;\nfn mul<F: SpartanField>(a: &F, b: &F) -> F {{ a.clone() * b }}\n{body}")).unwrap();
    let gf_path = root.join("src/poly/univariate/binary_gf128.rs");
    println!("cargo:rerun-if-changed={}", gf_path.display());
    let gf = fs::read_to_string(gf_path).unwrap();
    let fixed = section(
        &gf,
        "#[derive(Clone, Copy)]\npub(crate) struct FixedGfMul",
        "// -- carryless multiplication",
    );
    let enabled = function(&gf, "fixed_scalar_enabled");
    let mut neon = String::from(
        "#[cfg(all(target_arch=\"aarch64\",target_feature=\"neon\"))]\nmod neon { use core::arch::aarch64::*; use super::BinaryFieldGF128;\n",
    );
    for name in [
        "pmull_lo",
        "pmull_hi",
        "fold_x64",
        "mul_fixed_wide",
        "fixed_mul_words",
        "mul_fixed",
        "ld",
    ] {
        neon.push_str("#[inline(always)]\n");
        neon.push_str(&function(&gf, name));
        neon.push('\n');
    }
    neon.push('}');
    // Minimal representation/visibility glue; the selected production functions
    // above are verbatim. Multiplication fallback delegates to actual F2Z.
    let glue = r#"
use crypto_primitives::crypto_bigint_uint::Uint;
use f2z::poly::univariate::binary_gf128::BinaryFieldGF128 as Native;
#[derive(Clone,Copy)] pub struct BinaryFieldGF128 { uint: Uint<2> }
impl BinaryFieldGF128 { pub fn from_words(w:[u64;2])->Self{Self{uint:Uint::from_words(w)}} pub fn words(&self)->&[u64;2]{self.uint.as_words()} }
impl core::ops::Mul for BinaryFieldGF128 { type Output=Self; fn mul(self,b:Self)->Self{Self::from_words(*(Native::from_words(*self.words())*Native::from_words(*b.words())).words())} }
fn clmul_64x64(a:u64,b:u64)->[u64;2] { let p=crate::arithmetic::clmul(a,b);[p as u64,(p>>64)as u64] }
"#;
    fs::write(out.join("fixed_gf.rs"),format!("{glue}\n#[cfg(all(target_arch=\"aarch64\",target_feature=\"neon\"))]\n{enabled}\n{fixed}\n{neon}")).unwrap();
    let ood_path = root.join("src/ligerito_flock.rs");
    println!("cargo:rerun-if-changed={}", ood_path.display());
    let ood = fs::read_to_string(ood_path).unwrap();
    let build_eq = function(&ood, "build_eq_scaled");
    let eval = function(&ood, "ood_eval");
    let preamble = r#"
use f2z::poly::univariate::binary_gf128::BinaryFieldGF128 as Gf;
use flock_core::field::F128;
use rayon::prelude::*;
fn packed_message_vars(p:&[F128])->usize{assert!(p.len().is_power_of_two());p.len().ilog2()as usize}
fn f128_to_gf(x:F128)->Gf{Gf::from_words([x.lo,x.hi])}
macro_rules! cfg_into_iter {($range:expr)=>{($range).into_par_iter()};}
"#;
    let block = section(&ood, "const OOD_BLOCK_LOG:", "\n");
    fs::write(out.join("ood.rs"),format!("{preamble}\n{block}\n{build_eq}\n{eval}\npub fn evaluate(p:&[F128],z:&[Gf])->Gf{{ood_eval(p,z)}}\n")).unwrap();
    let opening_path = root.join("src/hybrid/opening.rs");
    println!("cargo:rerun-if-changed={}", opening_path.display());
    let opening = fs::read_to_string(opening_path).unwrap();
    let pack = function(&opening, "virtual_packed");
    let geometry = r#"
use flock_core::field::F128 as F;
use rayon::prelude::*;
pub struct Geometry {pub lane_logs:[usize;2],pub virtual_lane_log:usize,pub position_log:usize}
impl Geometry {
pub fn lanes(&self)->usize{1<<self.virtual_lane_log}
pub fn packed_log(&self)->usize{self.position_log+self.virtual_lane_log}
pub fn offset(&self,b:usize)->usize{b<<(self.virtual_lane_log-1)}
"#;
    // Only macro routing changes: the benchmark selects Rayon explicitly by
    // thread count. The packing function and source indexing are unchanged.
    let pack = pack.replace(
        "crate::utils::cfg_chunks_mut!(out, lanes)",
        "out.par_chunks_mut(lanes)",
    );
    fs::write(out.join("packing.rs"), format!("{geometry}\n{pack}\n}}\n")).unwrap();
    // The production module's own tests use circuit-private witness types.
    // Keep the implementation verbatim for this crate's independent tests,
    // excluding only those original tests and converting inner doc comments.
    let projection_path = root.join("crates/circuit/src/matrix_products.rs");
    println!("cargo:rerun-if-changed={}", projection_path.display());
    let projection = fs::read_to_string(projection_path).unwrap();
    let (implementation, _) = projection
        .split_once("\n#[cfg(test)]\nmod tests {")
        .expect("projection test boundary changed");
    let implementation = implementation
        .lines()
        .map(|line| line.strip_prefix("//!").map_or(line.to_owned(), |doc| format!("//{doc}")))
        .collect::<Vec<_>>()
        .join("\n");
    fs::write(out.join("projection_test.rs"), implementation).unwrap();
}

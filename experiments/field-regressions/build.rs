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
        .or_else(|| source.find(&format!("fn {name}<")))
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
    // This benchmark-only copy includes the independent bigint reference,
    // which is test-only in production. Do not re-enable it in the F2Z graph.
    let delayed_path =
        root.join("experiments/field-regressions/reference/6271724d/delayed_reduction.rs");
    println!("cargo:rerun-if-changed={}", delayed_path.display());
    let delayed = fs::read_to_string(delayed_path).unwrap();
    let implementation = delayed
        .split_once("\n#[cfg(test)]\nmod tests {")
        .expect("delayed reduction test boundary changed")
        .0;
    let implementation = implementation
        .replace("#[cfg(test)]\n", "")
        .lines()
        .map(|line| {
            line.strip_prefix("//!")
                .map_or(line.to_owned(), |doc| format!("//{doc}"))
        })
        .collect::<Vec<_>>()
        .join("\n");
    fs::write(out.join("delayed_reference.rs"), implementation).unwrap();
    let driver_path = root.join("experiments/field-regressions/src/main.rs");
    let prime_path = root.join("experiments/field-regressions/src/campaign/prime.rs");
    println!("cargo:rerun-if-changed={}", driver_path.display());
    println!("cargo:rerun-if-changed={}", prime_path.display());
    let driver = fs::read_to_string(driver_path).unwrap();
    let shared = section(&driver, "pub struct Rng(pub u64);", "fn verify()");
    fs::write(out.join("measurement.rs"), shared).unwrap();
    let prime = fs::read_to_string(prime_path).unwrap();
    let sums = section(
        &prime,
        "#[inline(never)]\npub(super) fn product_sum",
        "#[inline(never)]\nfn eager",
    );
    fs::write(out.join("baseline_sums.rs"), sums).unwrap();
    let raw_path = root.join("experiments/field-regressions/reference/6271724d/raw_monty.rs");
    let p256_path = root.join("crates/circuit/src/p256.rs");
    for p in [&raw_path, &p256_path] {
        println!("cargo:rerun-if-changed={}", p.display());
    }
    let raw = fs::read_to_string(raw_path).unwrap();
    let preamble = "use crypto_bigint::modular::FixedMontyParams;\nuse crypto_primitives::{FromWithConfig, crypto_bigint_monty::MontyField, crypto_bigint_uint::Uint as FieldUint};\nuse crate::baseline_delayed::MontyLinearAccumulator128;\ntype Field = MontyField<2>;\ntype FieldConfig = FixedMontyParams<2>;\ntype Raw = u128;\n";
    let body = section(
        &raw,
        "#[inline(always)]\nconst fn words_to_raw",
        "// ---------------------------------------------------------------------------\n// Equality tables",
    );
    fs::write(out.join("raw_ctx.rs"), format!("{preamble}{body}")).unwrap();
    let p256 = fs::read_to_string(p256_path).unwrap();
    let body = function(&p256, "multiply_wide");
    fs::write(out.join("p256_mul.rs"), format!("use field::{{Uint,IntegerOps,WideMul}};\n{body}\npub fn product(a: [u64;9], b: [u64;9]) -> [u64;18] {{ *multiply_wide(Uint::from_words(a), Uint::from_words(b)).as_words() }}\n")).unwrap();
    let sumcheck_path = root.join("experiments/field-regressions/reference/6271724d/sumcheck.rs");
    println!("cargo:rerun-if-changed={}", sumcheck_path.display());
    let sumcheck = fs::read_to_string(sumcheck_path).unwrap();
    let body = section(
        &sumcheck,
        "pub(super) fn batch_invert_nonzero",
        "#[inline]\npub(super) fn equality_coordinate_evaluation",
    );
    let body = body
        .replace("batch_invert_nonzero<F>", "batch_invert_nonzero")
        .replace("&F::Config", "&crypto_bigint::modular::FixedMontyParams<2>")
        .replace("where\n    F: SpartanField,\n", "");
    fs::write(out.join("batch_inverse.rs"), format!("use crypto_primitives::{{FromWithConfig,PrimeField}};\ntype F=crypto_primitives::crypto_bigint_monty::MontyField<2>;\nfn mul(a:&F,b:&F)->F{{a*b}}\n{body}")).unwrap();
    let gf_path = root.join("experiments/field-regressions/reference/6271724d/binary_gf128.rs");
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
        "mul_words",
    ] {
        neon.push_str("#[inline(always)]\n");
        neon.push_str(&function(&gf, name));
        neon.push('\n');
    }
    neon.push('}');
    // Minimal representation/visibility glue; the selected production functions
    // above and all fallback arithmetic are frozen at the same baseline.
    let glue = r#"
use crypto_primitives::crypto_bigint_uint::Uint;
#[derive(Clone,Copy)] pub struct BinaryFieldGF128 { uint: Uint<2> }
impl BinaryFieldGF128 { pub fn from_words(w:[u64;2])->Self{Self{uint:Uint::from_words(w)}} pub fn words(&self)->&[u64;2]{self.uint.as_words()} }
impl core::ops::Mul for BinaryFieldGF128 { type Output=Self; fn mul(self,b:Self)->Self{Self::from_words(mul_words_gf128(self.words(),b.words()))} }
"#;
    let fallback = [
        "mul_words_gf128",
        "clmul_128x128",
        "clmul_64x64",
        "clmul_64x64_scalar",
        "word_times_g_gf128",
        "reduce_256_to_128",
    ]
    .iter()
    .map(|name| format!("#[inline]\n{}\n", function(&gf, name)))
    .collect::<String>();
    fs::write(out.join("fixed_gf.rs"),format!("{glue}\n{fallback}\n#[cfg(all(target_arch=\"aarch64\",target_feature=\"neon\"))]\n{enabled}\n{fixed}\n{neon}")).unwrap();
    let ood_path = root.join("src/ligerito_flock.rs");
    println!("cargo:rerun-if-changed={}", ood_path.display());
    let ood = fs::read_to_string(ood_path).unwrap();
    let build_eq = function(&ood, "build_eq_scaled");
    let eval = function(&ood, "ood_eval");
    let preamble = r#"
use f2z::poly::univariate::binary_gf128::Gf128 as Gf;
use flock_core::field::Gf128;
use rayon::prelude::*;
fn packed_message_vars(p:&[Gf128])->usize{assert!(p.len().is_power_of_two());p.len().ilog2()as usize}
fn f128_to_gf(x:Gf128)->Gf{Gf::from_polynomial_words([x.lo,x.hi])}
macro_rules! cfg_into_iter {($range:expr)=>{($range).into_par_iter()};}
"#;
    let block = section(&ood, "const OOD_BLOCK_LOG:", "\n");
    fs::write(out.join("ood.rs"),format!("{preamble}\n{block}\n{build_eq}\n{eval}\npub fn evaluate(p:&[Gf128],z:&[Gf])->Gf{{ood_eval(p,z)}}\n")).unwrap();
    let opening_path = root.join("src/hybrid/opening.rs");
    println!("cargo:rerun-if-changed={}", opening_path.display());
    let opening = fs::read_to_string(opening_path).unwrap();
    let pack = function(&opening, "virtual_packed");
    let geometry = r#"
use flock_core::field::Gf128 as F;
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
        .map(|line| {
            line.strip_prefix("//!")
                .map_or(line.to_owned(), |doc| format!("//{doc}"))
        })
        .collect::<Vec<_>>()
        .join("\n");
    fs::write(out.join("projection_test.rs"), implementation).unwrap();
    // The optional GF grid hand kernel is ARM-only. Use the actual production
    // dispatch and generic body on x86, including its original fold helpers.
    let grid_path = root.join("src/piop/sumcheck/eq_factored.rs");
    println!("cargo:rerun-if-changed={}", grid_path.display());
    let grid_source = fs::read_to_string(grid_path).unwrap();
    let mut grid = String::from(
        "use f2z::utils::{inner_transparent_field::InnerTransparentField,wide_mul::WideMulAcc};\nuse f2z::poly::univariate::binary_gf128::Gf128 as Gf;\n",
    );
    for name in ["fold1_at", "fold_logical", "grid_quad_acc"] {
        grid.push_str("#[inline(always)]\n");
        grid.push_str(&function(&grid_source, name));
        grid.push('\n');
    }
    for name in ["grid_finish", "dense_grid_pass_slices"] {
        grid.push_str(&function(&grid_source, name));
        grid.push('\n');
    }
    grid.push_str("pub fn evaluate(l:&mut [Gf],r:&mut [Gf],p:&[Gf],w:&[Gf],n:usize)->Option<[Gf;9]>{Some(dense_grid_pass_slices(l,r,p,w,n,&Gf::zero()))}\n");
    fs::write(out.join("grid.rs"), grid).unwrap();
}

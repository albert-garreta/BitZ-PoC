#[path = "scripts/git_revision.rs"]
mod git_revision;
fn main() {
    let root = std::path::PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").unwrap());
    println!("cargo:rustc-env=BITZ_REVISION={}", git_revision::revision(&root));
    for (name, path) in [
        ("BINIUS64_REVISION", "vendor/binius64"),
        ("PLONKY3_REVISION", "vendor/plonky3"),
        ("LIMBER_REVISION", "vendor/limber"),
    ] {
        println!("cargo:rustc-env={name}={}", git_revision::revision(&root.join(path)));
    }
}

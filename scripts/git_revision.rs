use std::{path::Path, process::Command};

pub fn revision(root: &Path) -> String {
    assert!(root.join(".git").is_dir(), "missing repository: {}", root.display());
    for path in [".git/HEAD", ".git/packed-refs", "provenance.toml"] {
        println!("cargo:rerun-if-changed={}", root.join(path).display());
    }
    let head = std::fs::read_to_string(root.join(".git/HEAD")).expect("repository HEAD");
    if let Some(reference) = head.trim().strip_prefix("ref: ") {
        println!("cargo:rerun-if-changed={}", root.join(".git").join(reference).display());
    }
    let output = Command::new("git").arg("-C").arg(root)
        .args(["rev-parse", "HEAD"]).output().expect("git is required");
    assert!(output.status.success(), "cannot read repository revision");
    let hash = String::from_utf8(output.stdout).unwrap().trim().to_owned();
    assert!(hash.len() == 40 && hash.bytes().all(|b| b.is_ascii_hexdigit()));
    hash
}

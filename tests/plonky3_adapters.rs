#![cfg(feature = "plonky3-whir-bench")]

#[path = "../benches/common/mod.rs"]
mod common;

#[path = "../benches/baby_bear_pcs_compare/whir.rs"]
mod baby_bear;
#[path = "../benches/integer_pcs_compare/whir_goldilocks.rs"]
mod goldilocks;

#[test]
fn baby_bear_roundtrip() {
    let backend = baby_bear::WhirBackend::setup(16).unwrap();
    let a = (0..16).collect::<Vec<u64>>();
    let b = vec![3; 16];
    let c = a.iter().map(|x| x * 3).collect::<Vec<_>>();
    let k = vec![0; 16];
    let witness = backend.materialize(&a, &b, &c, &k).unwrap();
    let committed = backend.commit(witness, u64::MAX);
    let ready = backend.derive_and_bind_terminal_claim(committed).unwrap();
    let opened = backend.open(ready);
    let _ = backend.verify(&opened).unwrap();
    let bytes = postcard::to_allocvec(opened.proof()).unwrap();
    let decoded: baby_bear::Proof = postcard::from_bytes(&bytes).unwrap();
    assert_eq!(postcard::to_allocvec(&decoded).unwrap(), bytes);
}

#[test]
fn goldilocks_roundtrip() {
    let backend = goldilocks::Backend::setup(16).unwrap();
    let x = vec![u32::MAX as u64; 16];
    let y = (0..16).map(|i| u32::MAX as u64 - i).collect::<Vec<_>>();
    let product = x.iter().zip(&y).map(|(x, y)| x * y).collect::<Vec<_>>();
    let witness = backend.materialize(&x, &y, &product).unwrap();
    let committed = backend.commit(witness, u64::MAX);
    let ready = backend.derive_and_bind_claim(committed).unwrap();
    let opened = backend.open(ready);
    let _ = backend.verify(&opened).unwrap();
    let bytes = postcard::to_allocvec(opened.proof()).unwrap();
    let decoded: goldilocks::Proof = postcard::from_bytes(&bytes).unwrap();
    assert_eq!(postcard::to_allocvec(&decoded).unwrap(), bytes);
}

//! Representation-independent transcript pins for every protocol the
//! benchmarks run.
//!
//! Each pin proves a fixed deterministic instance, verifies it, and records
//! two things:
//!
//! * the BLAKE3 state digest of the prover transcript after `prove` — which
//!   must equal the verifier transcript's state after `verify` — so the pin
//!   tracks the Fiat–Shamir transcript itself (every absorbed byte and every
//!   drawn challenge, in order) rather than how a proof struct happens to be
//!   laid out or `Debug`-formatted in memory;
//! * a digest of the serialized proof parts (the opener proof bytes, the
//!   grinding nonces, and any whole-proof codec), which covers the proof
//!   components that are not absorbed into the transcript (Merkle paths and
//!   the Ligerito query answers).
//!
//! Set `F2Z_RECORD_PINS=1` to print the table entries instead of asserting.
//! Update a value only in a commit that *intends* a transcript change.

use ::f2z::piop::spartan::baby_bear_mul::BabyBearMulLayout;
use ::f2z::piop::spartan::protocol;
use ::f2z::piop::spartan::protocol::PreparedRelation;
use f2z::piop::spartan::mul::{MulLayout, MulWitness};

use blake3::Hasher;
use f2z::piop::spartan::multiswap::{
    MultiswapAssignment, MultiswapCircuit, MultiswapDims, PreparedMultiswapRelation,
    commit_multiswap_witness, multiswap_lig_configs, prove_multiswap_mod_r1cs,
    verify_multiswap_mod_r1cs,
};
use f2z::piop::spartan::{
    BabyBearMulWitness, IopSecurityProfile, Lambda100, Lambda128, Sha128ReferenceSchedule,
    Sha256CompressionStatement, commit_sha256_chain_witness, commit_sha256_compression_witness,
    generate_sha256_chain_witnesses, generate_sha256_compression_witnesses,
    prepare_sha256_chain_batch_with_profile,
    prepare_sha256_compression_batch_for_assignment_rows_with_profile,
    prepare_sha256_compression_batch_with_profile, prove_sha256_chain, prove_sha256_compressions,
    verify_sha256_chain, verify_sha256_compressions,
};
use f2z::transcript::Blake3Transcript;

/// `(name, prover transcript state, verifier transcript state, serialized
/// proof parts)`. The two states differ by design: flock's Ligerito prover
/// and verifier end in different transcript states after the last level, so
/// both are pinned.
const PINS: &[(&str, &str, &str, &str)] = &[
    // Recorded after the shared arithmetic, canonical codec, and bounded sampler migration.
    // Spartan domains use v2; hybrid wire encoding uses version 6.
    (
        "baby_bear/2p15/lambda100",
        "7dcc29ecdd971d5fd508553090088f9796642c3d8219de2e659b58523c2dbb57",
        "fa9b98c61ef29a1275a5d3fa07861ea88a62724b72c7a3231b9610052dc579b1",
        "bfbfe0ae2fd33ce8f45120301b26e6efb06be0532fda8791da01c713266724e8",
    ),
    (
        "baby_bear/2p15/lambda128",
        "b01e58ba4e0476d37cdef2c43093bfac2726563c898cf8ff773d190a2ab7dcfa",
        "6c7e7911e9cf6b37abf39cbdbbc4485baad2dab790337e6c82e7ebf8349cea3d",
        "221122c7e29925247a2194f1759027e6b5aac46c46f4c5447d855da722352e9a",
    ),
    (
        "cm_and/2p15/lambda100",
        "70230d48addc57f75eb0e0cf0a40b3b55c172a9ac942a916c24c18cf09967ca8",
        "c7bdf028d7b44047ec8765debbc621f0c7e37f4c3a4ccbe0d1e1642961f37daf",
        "540df1ab634298b7159b6c69dc4ac6d9bab6b67e37b743c35c65a8316eb5dd23",
    ),
    (
        "multiswap/mini/limber114",
        "29e12bacd9891f018b07469053ca38a721b56d42916b432fa06c3e0d0122c81e",
        "3874d394dd46f2707aa77af9b12e3903a9b7da25dd25a9298525ffd231d8ae03",
        "9024409aaaca03a190455031c1c9f61cc69edea922764b57e33b431387e4a1e1",
    ),
    (
        "sha256_chain/2p7/lambda100",
        "2d5960466545f252e59540b202d1fb2bc7b718c3b90b7b58a122152cc6517851",
        "42f79939055dab67b8af40be7a5cfebb240479bad60e7a9ffdc13d897fca2313",
        "32946a29159e48e6d06b46a0a2e87ef2ea7069c666a68c1f0a0e3bdf9e999d89",
    ),
    (
        "sha256_chain/2p7/lambda128",
        "c3d62622f33cf64d4ed0c32dccbb0cbce034f250e44435c63bd4caa8ecf1fbbe",
        "26a2cdf26cbec07a09907fb765f76466973b2385e05eaae62d40cc5025485e15",
        "06db3871dcc0ae8ea0faffee2d41904435e7c3e1eb58d3bfaed79acaddda1c69",
    ),
    (
        "sha256/2p7/lambda100",
        "bc133a7a71e451b5e3b44f9d24ea6130a6107e17b6d9e6367cbf122b945551b9",
        "f6a4ab334f8e8404d9c622f9672586a99e5b069afc631b5df95e2193c8eef31d",
        "a07bcdf763b7444919fb3c8fe9b6f74df83e918e2ee083c5b3d4e96a34e01331",
    ),
    (
        "sha256/2p7/lambda128",
        "383ffaede2375e8bc48b974fc1e6f6a85f0bf440edba8fd5d8214c655ae7dc32",
        "c156b0bac460cd6ce6e81d8d235ddd163725047e443e205e046c5aeb3984a669",
        "ffa8c6cd70c85c1fc65dbb78962563c19e26248d3b37524961285cd06b6700b4",
    ),
    (
        "sha256/2p7/reference",
        "b267464171b6951d47fd73e96ec70411cf92f38b515a1087af43b0db538e62da",
        "81f017a3a18a53bf1932129cf7119e56e8b6848053a479c1ede9002b0681a4dc",
        "82591cc3ddc9aea8f87f14359c21be567cb6b1adb78901e28a1f373b0b15112f",
    ),
    (
        "sha256/legacy-rows21/lambda100",
        "cd2b12dcfcb5d015bfd7f1855091704580de7f620c4c3a44943dab75157efb89",
        "a055a1b87029f11b3f37f60c0d8674271f1e0c3c408af009db751e5fff91caf8",
        "901b246d028567f075932f18c878504e66c162ea2c3b50301a850197dc5f4d3f",
    ),
    (
        "u128_mul/2p15/lambda100",
        "15674215cffad17c9c1e79cb764af67e5605d32629f4b6c9e1916ac8b03e8120",
        "03df0bccd6ee1064fe40f9e37013c0c7ccf0df01b35914b707fa7c9bb4503ce6",
        "12393a13d68dc76fbb2295c13e4ce418bed0209dbbccf9974f223db88d6b674b",
    ),
    (
        "u32_mul/2p15/w1/lambda100",
        "a68be59cf6729e370217e45aac593e60f6b2e120b7c549ae0cc16bfdac2c2ef5",
        "cf491dcf6f4ac3303d63ffbbbf8211ec9dd00542d1f19733c5acf837795856ee",
        "5c2298fc1c6eb6b7af3c353fa06ae8eb1b8e782b9a8692eb2ec8b04b61833d34",
    ),
    (
        "u32_mul/2p15/w1/lambda128",
        "6a7c742c3b1fca90eeb6f613e07e45c22bd5fed302ff5fcd32a980096cda42e3",
        "77f785ad9654d26fcfb413c9767cc02485d0abf0219ab761eb31b5db745ac15e",
        "70bec04089e8b910d25846237715e044d55ae5b05131fd8b129426e5edef4092",
    ),
    (
        "u32_mul/2p15/w8/lambda100",
        "5c190826c3eef116f4f36dcd90afccfa0781e07a0773d5580e89f2e820610e52",
        "e1b087a6f881bcf92d1b70a5bca6282ad7bc500734f2bee9da7f3c87ef123096",
        "7d0857db55fbe8cd3129f2f52a7397961021d4d5247470d09f0dbfba057fa9f0",
    ),
    (
        "u64_mul/2p15/lambda100",
        "ce8a805e4b14d83325c5b6290e5a28355d88f3dd84116a7d0418162f702b6b4c",
        "f25509d5a5bbcd8c39a7662cb77489f21f485fc7c1cf8630b537ef944c250668",
        "fc442a6f3d3a7c29a2e95dc0df53734bed1b7c3b653cc4b45c591946f5d78868",
    ),
    (
        "u64_mul/2p15/shift+1/lambda100",
        "d26ce602ce1380396bd0c209b6be724db6d2e8b1007dd29e4fc75b776820b927",
        "234d33249ec54c319ec5c9d9d733789a4386b2fa5c806aa10a10dd4a0fb00261",
        "c8f7d41793b75f66b01402695f48a96f899cc8cc7c38fe7cf17fd22e30ae7288",
    ),
    (
        "sha256_ecdsa/2p3/allrows/lambda100",
        "4c4a89583e32a04075604e49817e0c82aedf78a8d0e513cadebbfc12b3601b91",
        "4c4a89583e32a04075604e49817e0c82aedf78a8d0e513cadebbfc12b3601b91",
        "430efbd1d38b89ef85484b32ca9c6cb1d838b81e9d896fa24f36e877ed545cd4",
    ),
    (
        "sha256_ecdsa/2p3/allrows/lambda128",
        "9c2547f228073539819a9e5d6ecf212b7deff4e7ceca07be32dacca7bae48e3e",
        "9c2547f228073539819a9e5d6ecf212b7deff4e7ceca07be32dacca7bae48e3e",
        "d51ccef03a5aad47d41869b4e3822bb079ecdf70b1628e5a3e4c281f11560201",
    ),
    (
        "sha256_ecdsa/2p3/split/lambda100",
        "ea73d3ed986cfe35f8aeb70198ae86377848540b678db0b990be03bef243e52d",
        "ea73d3ed986cfe35f8aeb70198ae86377848540b678db0b990be03bef243e52d",
        "449330377276eb7af359ff2c8647aaf30713614b672b756def484b73dbc4aa1d",
    ),
    (
        "sha256_ecdsa/2p3/split/lambda128",
        "c8fe4979314e1d84d2ae7e6e9407827c79cda5b13f3df18f1d85131a2b1ef881",
        "c8fe4979314e1d84d2ae7e6e9407827c79cda5b13f3df18f1d85131a2b1ef881",
        "3b7dd202fea9dae77af0e46a69f3b53c384bcdfdbc081232138b248352214762",
    ),
    (
        "sha256/fixed98-t13/2p14",
        "3473046c1d2541e2e71f5282d2b2f935845b7e21b157734f49b56da50f96a40e",
        "ed22514e7c8b9d3d4a8b1a5f8cb433664997b517fd897fd1ff2a187b84b823fa",
        "b471fb5b483d4cc95fc529c6432d23285cae1f61973bd9132ffd681f5941b3e2",
    ),
    (
        "hybrid/2p13x16/johnson",
        "-",
        "-",
        "9665efe426da61bbcbf978c9e9a5600c1b717c88728b9a919262688802c0ab1b",
    ),
];

fn digest_hex(parts: &[&[u8]]) -> String {
    let mut hasher = Hasher::new();
    for part in parts {
        hasher.update(&(part.len() as u64).to_le_bytes());
        hasher.update(part);
    }
    hasher.finalize().to_hex().to_string()
}

fn state_hex(transcript: &Blake3Transcript) -> String {
    blake3::Hash::from(transcript.state_digest())
        .to_hex()
        .to_string()
}

fn check(name: &str, prover_state: &str, verifier_state: &str, bytes: &str) {
    if std::env::var_os("F2Z_RECORD_PINS").is_some() {
        println!("    (\"{name}\", \"{prover_state}\", \"{verifier_state}\", \"{bytes}\"),");
        return;
    }
    let expected = PINS
        .iter()
        .find(|(pinned, _, _, _)| *pinned == name)
        .unwrap_or_else(|| panic!("{name}: no pin recorded (run with F2Z_RECORD_PINS=1)"));
    assert_eq!(
        prover_state, expected.1,
        "{name}: prover transcript state moved"
    );
    assert_eq!(
        verifier_state, expected.2,
        "{name}: verifier transcript state moved"
    );
    assert_eq!(bytes, expected.3, "{name}: serialized proof parts moved");
}

/// Checks (or records) one pin: the prover's and the verifier's final
/// transcript states plus the serialized proof parts.
fn pin(name: &str, prover: &Blake3Transcript, verifier: &Blake3Transcript, parts: &[&[u8]]) {
    check(
        name,
        &state_hex(prover),
        &state_hex(verifier),
        &digest_hex(parts),
    );
}

/// Pins a proof whose prover builds its own transcript (no state digest).
fn pin_bytes(name: &str, parts: &[&[u8]]) {
    check(name, "-", "-", &digest_hex(parts));
}

fn nonces_le(nonces: impl IntoIterator<Item = u64>) -> Vec<u8> {
    nonces.into_iter().flat_map(|n| n.to_le_bytes()).collect()
}

// ---------------------------------------------------------------- u32 mul

fn u32_witness(width: usize) -> MulWitness<u32> {
    MulWitness::<u32>::from_fn_with_word_bits(1usize << 15, width, |i| {
        let x = (i as u32).wrapping_mul(0x9e37_79b9) | 1;
        let y = (i as u32).wrapping_mul(0x85eb_ca6b) | 1;
        (x, y)
    })
    .expect("witness")
}

fn u32_pin<P: IopSecurityProfile>(name: &str, width: usize) {
    let witness = u32_witness(width);
    let prepared = PreparedRelation::<MulLayout<u32>>::new_with_profile::<P>(*witness.layout())
        .expect("prepare");
    let hint = protocol::commit(&prepared, witness.f2z_bit_rows()).expect("commit");
    let mut pt = Blake3Transcript::new();
    let proof = protocol::prove(&mut pt, &prepared, &witness, &hint).expect("prove");
    let mut vt = Blake3Transcript::new();
    protocol::verify(&mut vt, &prepared, &hint.commitment, &proof).expect("verify");
    // The grinding nonces are absorbed into the transcript, so the state
    // digest already covers them.
    let f2z_bytes = proof.f2z().to_bytes();
    pin(name, &pt, &vt, &[&hint.commitment.root, &f2z_bytes]);
}

#[test]
fn u32_mul_2p15_w1_lambda100() {
    u32_pin::<Lambda100>("u32_mul/2p15/w1/lambda100", 1);
}

#[test]
fn u32_mul_2p15_w8_lambda100() {
    u32_pin::<Lambda100>("u32_mul/2p15/w8/lambda100", 8);
}

#[test]
fn u32_mul_2p15_w1_lambda128() {
    u32_pin::<Lambda128>("u32_mul/2p15/w1/lambda128", 1);
}

// ---------------------------------------------------------------- u64 mul

fn u64_pin(name: &str, split_shift: i8) {
    let witness = MulWitness::<u64>::from_fn(1usize << 15, |i| {
        let x = (i as u64).wrapping_mul(0x9e37_79b9_7f4a_7c15) | 1;
        let y = (i as u64).wrapping_mul(0xc2b2_ae3d_27d4_eb4f) | 1;
        (x, y)
    })
    .expect("witness")
    .with_split_shift(split_shift)
    .expect("split shift");
    let prepared = PreparedRelation::<MulLayout<u64>>::new(*witness.layout()).expect("prepare");
    let hint = protocol::commit(&prepared, witness.f2z_bit_rows()).expect("commit");
    let mut pt = Blake3Transcript::new();
    let proof = protocol::prove(&mut pt, &prepared, &witness, &hint).expect("prove");
    let mut vt = Blake3Transcript::new();
    protocol::verify(&mut vt, &prepared, &hint.commitment, &proof).expect("verify");
    // The grinding nonces are absorbed into the transcript, so the state
    // digest already covers them.
    let f2z_bytes = proof.f2z().to_bytes();
    pin(name, &pt, &vt, &[&hint.commitment.root, &f2z_bytes]);
}

#[test]
fn u64_mul_2p15_lambda100() {
    u64_pin("u64_mul/2p15/lambda100", 0);
}

#[test]
fn u64_mul_2p15_shift_plus1_lambda100() {
    u64_pin("u64_mul/2p15/shift+1/lambda100", 1);
}

// ---------------------------------------------------------------- u128 mul

#[test]
fn u128_mul_2p15_lambda100() {
    let witness = MulWitness::<u128>::from_fn(1usize << 15, |i| {
        let lo = (i as u64).wrapping_mul(0x9e37_79b9_7f4a_7c15) | 1;
        let hi = (i as u64).wrapping_mul(0xc2b2_ae3d_27d4_eb4f) | 1;
        let x = (u128::from(hi) << 64) | u128::from(lo);
        let y = (u128::from(lo.rotate_left(17)) << 64) | u128::from(hi ^ 0x5851_f42d_4c95_7f2d);
        (x, y)
    })
    .expect("witness");
    let prepared = PreparedRelation::<MulLayout<u128>>::new(*witness.layout()).expect("prepare");
    let hint = protocol::commit(&prepared, witness.f2z_bit_rows()).expect("commit");
    let mut pt = Blake3Transcript::new();
    let proof = protocol::prove(&mut pt, &prepared, &witness, &hint).expect("prove");
    let mut vt = Blake3Transcript::new();
    protocol::verify(&mut vt, &prepared, &hint.commitment, &proof).expect("verify");
    let f2z_bytes = proof.f2z().to_bytes();
    pin(
        "u128_mul/2p15/lambda100",
        &pt,
        &vt,
        &[&hint.commitment.root, &f2z_bytes],
    );
}

// ---------------------------------------------------------------- BabyBear

fn baby_bear_pin<P: IopSecurityProfile>(name: &str) {
    let witness = BabyBearMulWitness::from_fn(1usize << 15, |i| {
        let a = (i as u32).wrapping_mul(0x9e37_79b9) % 2_013_265_921;
        let b = (i as u32).wrapping_mul(0x85eb_ca6b) % 2_013_265_921;
        (a, b)
    })
    .expect("witness");
    let prepared = PreparedRelation::<BabyBearMulLayout>::new_with_profile::<P>(*witness.layout())
        .expect("prepare");
    let hint = protocol::commit(&prepared, witness.f2z_bit_rows()).expect("commit");
    let mut pt = Blake3Transcript::new();
    let proof = protocol::prove(&mut pt, &prepared, &witness, &hint).expect("prove");
    let mut vt = Blake3Transcript::new();
    protocol::verify(&mut vt, &prepared, &hint.commitment, &proof).expect("verify");
    // The grinding nonces are absorbed into the transcript, so the state
    // digest already covers them.
    let f2z_bytes = proof.f2z().to_bytes();
    pin(name, &pt, &vt, &[&hint.commitment.root, &f2z_bytes]);
}

#[test]
fn baby_bear_2p15_lambda100() {
    baby_bear_pin::<Lambda100>("baby_bear/2p15/lambda100");
}

#[test]
fn baby_bear_2p15_lambda128() {
    baby_bear_pin::<Lambda128>("baby_bear/2p15/lambda128");
}

// ---------------------------------------------------------------- MultiSwap

#[test]
fn multiswap_mini_limber114() {
    let circuit = MultiswapCircuit::build(MultiswapDims::mini()).expect("build circuit");
    let prepared = PreparedMultiswapRelation::new(&circuit).expect("prepare");
    let assignment = MultiswapAssignment::new(&circuit).expect("assignment");
    let (pc, vc) = multiswap_lig_configs(prepared.params()).expect("configs");
    let hint = commit_multiswap_witness(prepared.params(), assignment.f2z_bit_rows(), &pc)
        .expect("commit");
    let mut pt = Blake3Transcript::new();
    let proof =
        prove_multiswap_mod_r1cs(&mut pt, &prepared, &assignment, &hint, &pc).expect("prove");
    let mut vt = Blake3Transcript::new();
    verify_multiswap_mod_r1cs(&mut vt, &prepared, &hint.commitment, &proof, &vc).expect("verify");
    let f2z_bytes = proof.f2z().to_bytes();
    let mu_prime =
        f2z::piop::spartan::multiswap::reduce::encode_integer_lift(proof.mu_prime().expect("lift"));
    let nonce = proof.reduction_nonce().expect("nonce").to_le_bytes();
    pin(
        "multiswap/mini/limber114",
        &pt,
        &vt,
        &[&hint.commitment.root, &f2z_bytes, &mu_prime, &nonce],
    );
}

// ---------------------------------------------------------------- SHA-256

fn sha256_inputs(instances: usize) -> Vec<([u32; 8], [u32; 16])> {
    (0..instances)
        .map(|i| {
            let word = |j: usize| (i as u32).wrapping_mul(0x9e37_79b9) ^ (j as u32);
            (
                std::array::from_fn(|j| word(j)),
                std::array::from_fn(|j| word(j + 16)),
            )
        })
        .collect()
}

fn sha256_pin(name: &str, prepared: &f2z::piop::spartan::PreparedSha256CompressionBatch) {
    let inputs = sha256_inputs(prepared.instances());
    let witness = generate_sha256_compression_witnesses(prepared, &inputs).expect("witness");
    let statements: Vec<_> = inputs
        .iter()
        .copied()
        .zip(witness.outputs().iter().copied())
        .map(|(input, output)| Sha256CompressionStatement::new(input, output))
        .collect();
    let hint = commit_sha256_compression_witness(prepared, &witness).expect("commit");
    let mut pt = Blake3Transcript::new();
    let proof =
        prove_sha256_compressions(&mut pt, prepared, &statements, &witness, &hint).expect("prove");
    let mut vt = Blake3Transcript::new();
    verify_sha256_compressions(&mut vt, prepared, &statements, &hint.commitment, &proof)
        .expect("verify");
    let f2z_bytes = proof.f2z().to_bytes();
    let nonces = nonces_le(
        proof
            .inner_nonces()
            .iter()
            .copied()
            .chain([proof.initial_nonce(), proof.terminal_nonce()]),
    );
    pin(
        name,
        &pt,
        &vt,
        &[&hint.commitment.root, &f2z_bytes, &nonces],
    );
}

#[test]
fn sha256_2p7_lambda100() {
    let prepared = prepare_sha256_compression_batch_with_profile::<Lambda100>(7).expect("prepare");
    sha256_pin("sha256/2p7/lambda100", &prepared);
}

#[test]
fn sha256_2p7_reference_schedule() {
    let prepared = prepare_sha256_compression_batch_with_profile::<Sha128ReferenceSchedule>(7)
        .expect("prepare");
    sha256_pin("sha256/2p7/reference", &prepared);
}

#[test]
fn sha256_2p7_lambda128() {
    let prepared = prepare_sha256_compression_batch_with_profile::<Lambda128>(7).expect("prepare");
    sha256_pin("sha256/2p7/lambda128", &prepared);
}

#[test]
fn sha256_legacy_inner_sumcheck_rows21_lambda100() {
    let prepared =
        prepare_sha256_compression_batch_for_assignment_rows_with_profile::<Lambda100>(21)
            .expect("prepare");
    sha256_pin("sha256/legacy-rows21/lambda100", &prepared);
}

#[cfg(feature = "bench-internals")]
#[test]
fn sha256_fixed98_product_t13_2p14() {
    let prepared =
        f2z::piop::spartan::prepare_sha256_compression_batch_for_product_t_fixed98(14, 13)
            .expect("prepare");
    sha256_pin("sha256/fixed98-t13/2p14", &prepared);
}

// ---------------------------------------------------------------- SHA-256 chain

fn sha256_chain_pin<P: IopSecurityProfile>(name: &str) {
    const EXPONENT: usize = 7;
    let prepared = prepare_sha256_chain_batch_with_profile::<P>(EXPONENT).expect("prepare");
    let blocks: Vec<[u32; 16]> = (0..1usize << EXPONENT)
        .map(|i| std::array::from_fn(|j| (i as u32).wrapping_mul(0x9e37_79b9) ^ (j as u32)))
        .collect();
    let witness = generate_sha256_chain_witnesses(&prepared, &blocks).expect("witness");
    let statement = witness.statement();
    let hint = commit_sha256_chain_witness(&prepared, &witness).expect("commit");
    let mut pt = Blake3Transcript::new();
    let proof = prove_sha256_chain(&mut pt, &prepared, &statement, &witness, &hint).expect("prove");
    let mut vt = Blake3Transcript::new();
    verify_sha256_chain(&mut vt, &prepared, &statement, &hint.commitment, &proof).expect("verify");
    let f2z_bytes = proof.f2z().to_bytes();
    let nonces = nonces_le([proof.initial_nonce(), proof.terminal_nonce()]);
    pin(
        name,
        &pt,
        &vt,
        &[&hint.commitment.root, &f2z_bytes, &nonces],
    );
}

#[test]
fn sha256_chain_2p7_lambda100() {
    sha256_chain_pin::<Lambda100>("sha256_chain/2p7/lambda100");
}

#[test]
fn sha256_chain_2p7_lambda128() {
    sha256_chain_pin::<Lambda128>("sha256_chain/2p7/lambda128");
}

// ---------------------------------------------------------------- CM-AND

#[test]
fn cm_and_2p15_lambda100() {
    use f2z::piop::spartan::cm::commit_cm_and_witness_with_config;
    use f2z::piop::spartan::{
        CmAndWitness, SpartanF2zField, prepare_cm_and_relation, prove_cm_and_f2z,
        spartan_f2z_field_config, verify_cm_and_f2z,
    };
    let field_config = spartan_f2z_field_config();
    let witness = CmAndWitness::from_fn(1usize << 15, |i| {
        let x = (i as u32).wrapping_mul(0x9e37_79b9) | 1;
        let y = (i as u32).wrapping_mul(0x85eb_ca6b) | 1;
        (x, y)
    })
    .expect("witness");
    let layout = *witness.layout();
    let relation = prepare_cm_and_relation(layout, &field_config).expect("relation");
    let pc = relation
        .ligerito_configuration()
        .expect("ligerito")
        .prover();
    let hint =
        commit_cm_and_witness_with_config(&layout, witness.f_bit_rows(), pc).expect("commit");
    let mut pt = Blake3Transcript::new();
    let proof = prove_cm_and_f2z(&mut pt, &relation, &witness, &hint).expect("prove");
    let mut vt = Blake3Transcript::new();
    verify_cm_and_f2z(&mut vt, &relation, &hint.commitment, &proof).expect("verify");
    let f2z_bytes = proof.f2z().to_bytes();
    pin(
        "cm_and/2p15/lambda100",
        &pt,
        &vt,
        &[&hint.commitment.root, &f2z_bytes],
    );
}

// ---------------------------------------------------------------- SHA-256 + ECDSA

#[cfg(feature = "ecdsa")]
fn ecdsa_pin(name: &str, lambda: u32, mode: f2z::piop::spartan::ecdsa_sha256::OuterMode) {
    use f2z::piop::spartan::ecdsa_sha256::{
        Sha256EcdsaStatement, commit_sha256_ecdsa, generate_sha256_ecdsa_witness,
        prepare_sha256_ecdsa, prove_sha256_ecdsa, verify_sha256_ecdsa,
    };
    use num_bigint::BigUint;
    let word = |value: &BigUint| {
        let bytes = value.to_bytes_be();
        let mut out = [0u8; 32];
        out[32 - bytes.len()..].copy_from_slice(&bytes);
        out
    };
    let hex = |s: &[u8]| BigUint::parse_bytes(s, 16).unwrap();
    let gx = hex(b"6b17d1f2e12c4247f8bce6e563a440f277037d812deb33a0f4a13945d898c296");
    let gy = hex(b"4fe342e2fe1a7f9b8ee7eb4a7c0f9e162bce33576b315ececbb6406837bf51f5");
    let n = hex(b"ffffffff00000000ffffffffffffffffbce6faada7179e84f3b9cac2fc632551");
    // SHA-256 of bytes[i] = i mod 256, length 448 (seven blocks plus padding).
    let digest = hex(b"afcdb4646801a7f0c78048754ff01adec0da00eb73b20dc0dde7f089c2c24640");
    let message: Vec<u8> = (0..448).map(|i| i as u8).collect();
    let statement = Sha256EcdsaStatement {
        log_compressions: 3,
        qx: word(&gx),
        qy: word(&gy),
        r: word(&gx),
        s: word(&((&digest + &gx) % n)),
    };
    let prepared = prepare_sha256_ecdsa(3, lambda, mode).unwrap();
    let witness = generate_sha256_ecdsa_witness(&prepared, &statement, &message).unwrap();
    let hint = commit_sha256_ecdsa(&prepared, &witness).unwrap();
    let mut pt = Blake3Transcript::new();
    let proof = prove_sha256_ecdsa(&mut pt, &prepared, &statement, &witness, &hint, 4).unwrap();
    let mut vt = Blake3Transcript::new();
    verify_sha256_ecdsa(&mut vt, &prepared, &statement, &hint.commitment, &proof).unwrap();
    let bytes = proof.to_bytes();
    pin(name, &pt, &vt, &[&hint.commitment.root, &bytes]);
}

#[cfg(feature = "ecdsa")]
#[test]
fn sha256_ecdsa_2p3_split_lambda100() {
    ecdsa_pin(
        "sha256_ecdsa/2p3/split/lambda100",
        100,
        f2z::piop::spartan::ecdsa_sha256::OuterMode::Split,
    );
}

#[cfg(feature = "ecdsa")]
#[test]
fn sha256_ecdsa_2p3_allrows_lambda100() {
    ecdsa_pin(
        "sha256_ecdsa/2p3/allrows/lambda100",
        100,
        f2z::piop::spartan::ecdsa_sha256::OuterMode::AllRows,
    );
}

#[cfg(feature = "ecdsa")]
#[test]
fn sha256_ecdsa_2p3_split_lambda128() {
    ecdsa_pin(
        "sha256_ecdsa/2p3/split/lambda128",
        128,
        f2z::piop::spartan::ecdsa_sha256::OuterMode::Split,
    );
}

#[cfg(feature = "ecdsa")]
#[test]
fn sha256_ecdsa_2p3_allrows_lambda128() {
    ecdsa_pin(
        "sha256_ecdsa/2p3/allrows/lambda128",
        128,
        f2z::piop::spartan::ecdsa_sha256::OuterMode::AllRows,
    );
}

// ---------------------------------------------------------------- hybrid

#[cfg(feature = "hybrid")]
#[test]
fn hybrid_2p13_muls_16_compressions_johnson() {
    use f2z::hybrid::{Parameters, PreparedHybrid};
    // The shared opener needs a committed-bit exponent of at least 20, i.e.
    // 2^13 packed words: the smallest production-like shape.
    let parameters = Parameters {
        multiplications: 1 << 13,
        sha_compressions: 16,
    };
    let prepared = PreparedHybrid::new(parameters).expect("prepare");
    let muls: Vec<(u32, u32)> = (0..parameters.multiplications)
        .map(|i| {
            let x = (i as u32).wrapping_mul(0x9e37_79b9) | 1;
            let y = (i as u32).wrapping_mul(0x85eb_ca6b) | 1;
            (x, y)
        })
        .collect();
    let blocks: Vec<[u32; 16]> = (0..parameters.sha_compressions)
        .map(|i| std::array::from_fn(|j| (i as u32).wrapping_mul(0x9e37_79b9) ^ (j as u32)))
        .collect();
    let committed = prepared.commit(&muls, &blocks).expect("commit");
    let proof = prepared.prove(&committed).expect("prove");
    prepared
        .verify(committed.statement(), &proof)
        .expect("verify");
    let bytes = proof.to_bytes();
    let roots: Vec<u8> = committed
        .statement()
        .roots
        .iter()
        .flat_map(|r| r.iter().copied())
        .collect();
    pin_bytes("hybrid/2p13x16/johnson", &[&roots, &bytes]);
}

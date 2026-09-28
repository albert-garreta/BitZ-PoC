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
//! Set `BITZ_RECORD_PINS=1` to print the table entries instead of asserting.
//! Update a value only in a commit that *intends* a transcript change.

use ::bitz::piop::spartan::baby_bear_mul::BabyBearMulLayout;
use ::bitz::piop::spartan::protocol;
use ::bitz::piop::spartan::protocol::PreparedRelation;
use bitz::piop::spartan::mul::{MulLayout, MulWitness};

use blake3::Hasher;
use bitz::piop::spartan::multiswap::{
    MultiswapAssignment, MultiswapCircuit, MultiswapDims, PreparedMultiswapRelation,
    commit_multiswap_witness, multiswap_lig_configs, prove_multiswap_mod_r1cs,
    verify_multiswap_mod_r1cs,
};
use bitz::piop::spartan::{
    BabyBearMulWitness, IopSecurityProfile, Lambda100, Lambda128, Sha128ReferenceSchedule,
    Sha256CompressionStatement, commit_sha256_compression_witness,
    generate_sha256_compression_witnesses,
    prepare_sha256_compression_batch_for_assignment_rows_with_profile,
    prepare_sha256_compression_batch_with_profile, prove_sha256_compressions,
    verify_sha256_compressions,
};
use bitz::transcript::Blake3Transcript;

/// `(name, prover transcript state, verifier transcript state, serialized
/// proof parts)`. The two states differ by design: flock's Ligerito prover
/// and verifier end in different transcript states after the last level, so
/// both are pinned.
const PINS: &[(&str, &str, &str, &str)] = &[
    // Recorded after the BitZ hashing-domain and proof-codec namespace migration.
    // Spartan domains use v2; hybrid wire encoding uses version 6.
    (
        "baby_bear/2p15/lambda100",
        "eb77e02494e687a921603fab5a7b789cec3d27e77ad41462940c66ea76062b83",
        "2aaac0fd78dc910164af92ea073a5453dd57236ff9a5b77c6411f881745ca44d",
        "33a97048356b304e28fddde2665022693d7a43fb181530acd1f41253ffe78050",
    ),
    (
        "baby_bear/2p15/lambda128",
        "272df05ae695d3576ac071629608d5904cdd3830e00f65150d32b50d9ed86c50",
        "1fed2c175959adeb6aee2e96f305f07fe42b6d8b42f752f2177b30cc1229c445",
        "7230969be62c9e19852fbee2e92554a301f01abfbad12ce1131f8bde8519fd4b",
    ),
    (
        "cm_and/2p15/lambda100",
        "8f10a23d4a65c486cf0d2e4b85d9649b8219a307d950009a29580039235d5270",
        "72878cfd1a95fbea684a3899ed26096157627a30656705bcfb95afdbfc121263",
        "9fb2f8e26c166806832bca165585c3ab1e723f00499e6a19342c5d4df3df48cb",
    ),
    (
        "multiswap/mini/limber114",
        "f31653acaa7adc17f289cb7d47eda8201b583183868241a66511097a5b34e364",
        "7c6bfbc5e95af60d7419425613ee8fd8dfebc1873e38b689eeb39d59b80df4fe",
        "c4c8e5970bfda242807c072eb9a4aa8d22fb65aeabe184ff242e83889d2f1b1c",
    ),
    (
        "sha256/2p7/lambda100",
        "d2615a8cf40432f197e84cd1092e2936c9311b50f7681e64bf4a897d0902557b",
        "a2340389d324631f299b075815f6e74868322313f7bd5b9d6789e1485f5fd46b",
        "739db1ccb4eeddc0f5b6136826a92c36475ce3bfde6ab1ad898a394e718e5861",
    ),
    (
        "sha256/2p7/lambda128",
        "410161621b922b309796510a86e707aba5fae8cf8ec85acfb3fc980dc42f18c4",
        "e54ddf6829fa7026cbe9f14e5a8efdb4ff7f2c0ffbe663b0bdb84e9d24dbdbad",
        "8d0fe7327af82a06c60e5330815c66413f70d92627a9f4f1dd8f2b47e19cdd00",
    ),
    (
        "sha256/2p7/reference",
        "8ca22aaaae01be579ab997e09b22ad8f0f3b61de7142b91d75a034841b6be0f3",
        "04cb090c72cb592fb2b8e69f2b93ced6ac4380ba76aea0447f3d03dae551585d",
        "b2738f18c6239803cc580b0ff8d71151ff1a3d051ad1b3a0910dae7453626c67",
    ),
    (
        "sha256/2p7/wfbitz/lambda100",
        "64399c38bcf1b4408081443a5285f178903b04dad7ad0c825f463bec858f1e79",
        "64399c38bcf1b4408081443a5285f178903b04dad7ad0c825f463bec858f1e79",
        "8ae0faf93d174fb0db2d71613c75ce2231ed0582dd606753686cec4d8f10e91d",
    ),
    (
        "sha256/2p7/wfbitz/private-io/lambda100",
        "0bb3629f5bf10d1c9b624f3715919330bebf6f10ffb9bffd6f6dd20caf0f9d43",
        "0bb3629f5bf10d1c9b624f3715919330bebf6f10ffb9bffd6f6dd20caf0f9d43",
        "57448e5538805d7ad8ba219f2fc5e1c4916ee60963156713b7d7f52e6de41966",
    ),
    (
        "sha256/legacy-rows24/lambda100",
        "9cba7e07e447f3d1f24f3435a4ace0b92e582956a1a1b6deafd3a7c565e27429",
        "df01a8ff3e43efd580294b929553bac8435cedc77ace17495c3b56864800967d",
        "e6b7bcdd10dca4d6f62560efde977e9c13e5325a1a505d3fc256237af6b937a9",
    ),
    // The all-products relation (`sha-all-products`: the `Σ`/`σ` parities as
    // three-factor slots, `Maj` through a shared `a_t ⊕ a_{t−1}` parity), on
    // the same instances.
    (
        "sha256/2p7/lambda100/all-products",
        "5c72bfa3dabf8ff6764e1f326e0b93f8d296a257820e96ae4436eeb2a125fba8",
        "ab524efa44cb4375f872705bd469c4eda92cb0a72d5b89f2c2d305cc6161b783",
        "bcc02f9204edd2921f652022b3e65227c1b18b462db68608db938f4ae1508080",
    ),
    (
        "sha256/2p7/lambda128/all-products",
        "290bde2555ec0723cfe494f059404f1004a7410a8b916fcd54cbb4bd9464c4be",
        "225d96b4aba8ab3edca64ad99d8a01b4cc505b78040b3109cfabd4fc6a986f3b",
        "d6c434343ff137d57e47908e62a23277a69067496f0d6ce4cfbbca28267c6741",
    ),
    (
        "sha256/2p7/reference/all-products",
        "c90f3415d728510b0fed7d38ef28f8dcea81142cf81e5a0d82c59cf4775b583d",
        "7829f775c8e4ad8c7b546bbcbfdd6f9fc22fecef82501a4935a2087623c4ed76",
        "da30997c41e6c1ceeda8341b67821e032ba4d72998b14347cab9e5790a5adb21",
    ),
    (
        "sha256/2p7/wfbitz/lambda100/all-products",
        "370341ddcbb8428e63b468f4c63abd731fb5e00bab1d044a472649ca55d56a3b",
        "370341ddcbb8428e63b468f4c63abd731fb5e00bab1d044a472649ca55d56a3b",
        "64ff67787a7f94d5997532eb31bfb60d02fb78dee6590dfed92b389bd7d4c7db",
    ),
    (
        "sha256/2p7/wfbitz/private-io/lambda100/all-products",
        "830eabbe937f938df2478268772f0578d59795e90a99b1568600ed21265a7b87",
        "830eabbe937f938df2478268772f0578d59795e90a99b1568600ed21265a7b87",
        "4998c8e08e08ebae9cb65cef049c22c4ea23592d138e39fc80acf019f3db0f47",
    ),
    (
        "sha256/legacy-rows24/lambda100/all-products",
        "cd9bd21e5edc44464eef656b985c1e7fd847d5501fdd789d8c7dcc951e72b00b",
        "0495f6a04c01a33dddc8b3c1b16784e6948909b5f45fed9653e8ef99f52fa324",
        "d88655905bbab5fe77f8fa7ea3dc8c52ed499c0d1964a034b3c7153a65d5e8f2",
    ),
    (
        "u128_mul/2p15/lambda100",
        "96dff9b7d608b46a384222ae6cf1b3a16dffde7e8b13c0c0807539ce6d176e35",
        "bb5adbe923e20a2d2c5075c452cf223fd090e6c84565cd6a82c078ee177aeeb2",
        "4f846f9e12e975c2655d8b33d8749840178987ed1c766b6af9534b0e5ce23b06",
    ),
    (
        "u32_mul/2p15/w1/lambda100",
        "5046c5c2b0608a6befd13a40e0efcccb91e1915c582da3e9ca5e47a6894eef4e",
        "c20a35085e3450006528c4cd3fc8dc95246c9923ca5cd505168ab5490e1c9322",
        "30e9fe6fe902c62f699a913196400d8c56c06531b310f215c870c7488a90c6d5",
    ),
    (
        "u32_mul/2p15/w1/lambda128",
        "d0285a82cca8f45c3b6634c1a02fdf64e28cb9b5d881c0aaaa46c93b19958716",
        "462d3ea2e828dea1b266773349e7b9833411ee1eb73b3a3b8497c20199351da6",
        "d36f6ccbaa9e7b3ab595676d76c24f842164909e5fc240b9631f66c88baf2400",
    ),
    (
        "u32_mul/2p15/w8/lambda100",
        "7d30f24ec6fe86ad0226f751a56f1bfddc62c4604d1746bf629b3fb1453a2910",
        "b3685058d4f90ce376ed7111f7fc94463af8f536a28c79bfe9bb0287bc41bd67",
        "544c084b9fb6cf2a3aa28819a5b1f3cf961649f4e506c525f6efb3142a6c05b0",
    ),
    (
        "u64_mul/2p15/lambda100",
        "4d2fa9c0cf0ae4d1783f7164a4081c4bb086fc9181e95a942ca67d3fc92f4e64",
        "7c63d15df4118bfceca698358bf386f485fb35be3462e2e7d6f78f20b839c70d",
        "7df89f1280105dc71d4c824d40c44b6aa18391e243391c5620f884e1bc5022f7",
    ),
    (
        "u64_mul/2p15/shift+1/lambda100",
        "9faf268d12f2ad1d5ff77e9bf24109238442e580d5cd4177f432fe220f5d98fa",
        "2aac021bc68f13ac60e60dba0b9fca2e545d636219c06e764036948257654a3f",
        "23ca403d4488679848b6f1cd1849a0ed276865f601db99fb52f4fa5da35b84d2",
    ),
    (
        "sha256_ecdsa/2p3/allrows/lambda100",
        "78b0b01605b58d7a1235dc1216a140a464bd6728edffee603251b5ea32970c5d",
        "78b0b01605b58d7a1235dc1216a140a464bd6728edffee603251b5ea32970c5d",
        "b0c209e2c4aa6f7d86915372efeff9d04e551d5783f216bf1e777d9325c57fba",
    ),
    (
        "sha256_ecdsa/2p3/allrows/lambda128",
        "46d72414ae99fb288b20084759e0ff1cb7e9b881c48d15ac7f611d476a1c5c0c",
        "46d72414ae99fb288b20084759e0ff1cb7e9b881c48d15ac7f611d476a1c5c0c",
        "3fc21ed3d18d036466255cdbb8e768ccd419d944312baa2869ae06762ad235d7",
    ),
    (
        "sha256_ecdsa/2p3/split/lambda100",
        "fcc100b4ce3d3e8ffd5ef75efacc12ec9f68c8465a5e231c9d4c61c495d33d2c",
        "fcc100b4ce3d3e8ffd5ef75efacc12ec9f68c8465a5e231c9d4c61c495d33d2c",
        "3c98bbe731afd0fe053fef8a17dab690e7fb00f52fe7f883cb8749635f7dc8f8",
    ),
    (
        "sha256_ecdsa/2p3/split/lambda128",
        "15c0fb99ac07d68ddd4845f5dd714816400a831471d8c0261d5fad944ea88874",
        "15c0fb99ac07d68ddd4845f5dd714816400a831471d8c0261d5fad944ea88874",
        "385d46f3a6fd6ef04cbcf7a464cd84556a3553e58099e8e9268173b2119c4957",
    ),
    (
        "sha256/fixed98-t13/2p14",
        "b287d3697862faff80a574f1b21a169721328e3c562f0f7acd08260407d6453d",
        "c2f49c0f733f29366c3a29726ee04b12af341cbeca5fc258697adef5f902a852",
        "21d419e52943caaa0f8836f11ed21436b00c9a8757a271d45d2058dbaf9792f0",
    ),
    (
        "hybrid/2p13x16/johnson",
        "-",
        "-",
        "0aefced198a8a264bd50025dbef5696fa7a37fae02412de84a57120a317903ba",
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
    if std::env::var_os("BITZ_RECORD_PINS").is_some() {
        println!("    (\"{name}\", \"{prover_state}\", \"{verifier_state}\", \"{bytes}\"),");
        return;
    }
    let expected = PINS
        .iter()
        .find(|(pinned, _, _, _)| *pinned == name)
        .unwrap_or_else(|| panic!("{name}: no pin recorded (run with BITZ_RECORD_PINS=1)"));
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
    let hint = protocol::commit(&prepared, witness.bitz_bit_rows()).expect("commit");
    let mut pt = Blake3Transcript::new();
    let proof = protocol::prove(&mut pt, &prepared, &witness, &hint).expect("prove");
    let mut vt = Blake3Transcript::new();
    protocol::verify(&mut vt, &prepared, &hint.commitment, &proof).expect("verify");
    // The grinding nonces are absorbed into the transcript, so the state
    // digest already covers them.
    let bitz_bytes = proof.bitz().to_bytes();
    pin(name, &pt, &vt, &[&hint.commitment.root, &bitz_bytes]);
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
    let hint = protocol::commit(&prepared, witness.bitz_bit_rows()).expect("commit");
    let mut pt = Blake3Transcript::new();
    let proof = protocol::prove(&mut pt, &prepared, &witness, &hint).expect("prove");
    let mut vt = Blake3Transcript::new();
    protocol::verify(&mut vt, &prepared, &hint.commitment, &proof).expect("verify");
    // The grinding nonces are absorbed into the transcript, so the state
    // digest already covers them.
    let bitz_bytes = proof.bitz().to_bytes();
    pin(name, &pt, &vt, &[&hint.commitment.root, &bitz_bytes]);
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
    let hint = protocol::commit(&prepared, witness.bitz_bit_rows()).expect("commit");
    let mut pt = Blake3Transcript::new();
    let proof = protocol::prove(&mut pt, &prepared, &witness, &hint).expect("prove");
    let mut vt = Blake3Transcript::new();
    protocol::verify(&mut vt, &prepared, &hint.commitment, &proof).expect("verify");
    let bitz_bytes = proof.bitz().to_bytes();
    pin(
        "u128_mul/2p15/lambda100",
        &pt,
        &vt,
        &[&hint.commitment.root, &bitz_bytes],
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
    let hint = protocol::commit(&prepared, witness.bitz_bit_rows()).expect("commit");
    let mut pt = Blake3Transcript::new();
    let proof = protocol::prove(&mut pt, &prepared, &witness, &hint).expect("prove");
    let mut vt = Blake3Transcript::new();
    protocol::verify(&mut vt, &prepared, &hint.commitment, &proof).expect("verify");
    // The grinding nonces are absorbed into the transcript, so the state
    // digest already covers them.
    let bitz_bytes = proof.bitz().to_bytes();
    pin(name, &pt, &vt, &[&hint.commitment.root, &bitz_bytes]);
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
    let hint = commit_multiswap_witness(prepared.params(), assignment.bitz_bit_rows(), &pc)
        .expect("commit");
    let mut pt = Blake3Transcript::new();
    let proof =
        prove_multiswap_mod_r1cs(&mut pt, &prepared, &assignment, &hint, &pc).expect("prove");
    let mut vt = Blake3Transcript::new();
    verify_multiswap_mod_r1cs(&mut vt, &prepared, &hint.commitment, &proof, &vc).expect("verify");
    let bitz_bytes = proof.bitz().to_bytes();
    let mu_prime =
        bitz::piop::spartan::multiswap::reduce::encode_integer_lift(proof.mu_prime().expect("lift"));
    let nonce = proof.reduction_nonce().expect("nonce").to_le_bytes();
    pin(
        "multiswap/mini/limber114",
        &pt,
        &vt,
        &[&hint.commitment.root, &bitz_bytes, &mu_prime, &nonce],
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

/// The SHA relation's pin name: the `Ch`-product relation by default, the
/// all-products one (`sha-all-products`) under its own names.
fn sha_relation_pin(name: &str) -> String {
    if cfg!(feature = "sha-all-products") {
        format!("{name}/all-products")
    } else {
        name.to_owned()
    }
}

fn sha256_pin(name: &str, prepared: &bitz::piop::spartan::PreparedSha256CompressionBatch) {
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
    let bitz_bytes = proof.bitz().to_bytes();
    let nonces = nonces_le(
        proof
            .inner_nonces()
            .iter()
            .copied()
            .chain([proof.initial_nonce(), proof.terminal_nonce()]),
    );
    pin(
        &sha_relation_pin(name),
        &pt,
        &vt,
        &[&hint.commitment.root, &bitz_bytes, &nonces],
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

/// The legacy inner-sumcheck path needs a non-power-of-two batch: `2^24`
/// cells hold 1025 compressions of the `2^14`-stride relation and 2054 of
/// the all-products one (`2^21`, the old choice, now holds exactly 128 of
/// the first and opens the product tensor).
#[test]
fn sha256_legacy_inner_sumcheck_rows24_lambda100() {
    let prepared =
        prepare_sha256_compression_batch_for_assignment_rows_with_profile::<Lambda100>(24)
            .expect("prepare");
    let expected = if cfg!(feature = "sha-all-products") { 2054 } else { 1025 };
    assert_eq!(prepared.instances(), expected);
    sha256_pin("sha256/legacy-rows24/lambda100", &prepared);
}

/// The default SHA opener: worldfnd/BitZ on the `Id ⊗ M` block-layout
/// commitment (the opening forks its own transcript after the claim frame).
#[cfg(feature = "bitz-parity")]
#[test]
fn sha256_2p7_wfbitz_lambda100() {
    use bitz::piop::spartan::{
        commit_sha256_compression_witness_wfbitz,
        prepare_sha256_compression_batch_wfbitz_with_profile, protocol::OpeningProof,
        prove_sha256_compressions_wfbitz, verify_sha256_compressions_wfbitz,
    };
    let prepared =
        prepare_sha256_compression_batch_wfbitz_with_profile::<Lambda100>(7).expect("prepare");
    let inputs = sha256_inputs(prepared.instances());
    let witness = generate_sha256_compression_witnesses(&prepared, &inputs).expect("witness");
    let statements: Vec<_> = inputs
        .iter()
        .copied()
        .zip(witness.outputs().iter().copied())
        .map(|(input, output)| Sha256CompressionStatement::new(input, output))
        .collect();
    let hint = commit_sha256_compression_witness_wfbitz(&prepared, &witness).expect("commit");
    let mut pt = Blake3Transcript::new();
    let proof = prove_sha256_compressions_wfbitz(&mut pt, &prepared, &statements, &witness, &hint)
        .expect("prove");
    let mut vt = Blake3Transcript::new();
    verify_sha256_compressions_wfbitz(&mut vt, &prepared, &statements, &hint.commitment, &proof)
        .expect("verify");
    let opening = proof.bitz().to_bytes();
    let nonces = nonces_le([proof.initial_nonce(), proof.terminal_nonce()]);
    pin(
        &sha_relation_pin("sha256/2p7/wfbitz/lambda100"),
        &pt,
        &vt,
        &[&hint.commitment.root, &opening, &nonces],
    );
}

/// The same batch with nothing public (Flock's `sha2_proof` statement): the
/// statement is empty, and the proof neither takes a statement nor verifies
/// as the public-I/O proof.
#[cfg(feature = "bitz-parity")]
#[test]
fn sha256_2p7_wfbitz_private_io_lambda100() {
    use bitz::piop::spartan::{
        commit_sha256_compression_witness_wfbitz,
        prepare_sha256_compression_batch_wfbitz_with_profile, protocol::OpeningProof,
        prove_sha256_compressions_wfbitz, verify_sha256_compressions_wfbitz,
    };
    let public =
        prepare_sha256_compression_batch_wfbitz_with_profile::<Lambda100>(7).expect("prepare");
    let prepared = public.clone().with_private_io();
    let inputs = sha256_inputs(prepared.instances());
    let witness = generate_sha256_compression_witnesses(&prepared, &inputs).expect("witness");
    let statements: Vec<_> = inputs
        .iter()
        .copied()
        .zip(witness.outputs().iter().copied())
        .map(|(input, output)| Sha256CompressionStatement::new(input, output))
        .collect();
    let hint = commit_sha256_compression_witness_wfbitz(&prepared, &witness).expect("commit");
    let mut rejected = Blake3Transcript::new();
    assert!(
        prove_sha256_compressions_wfbitz(&mut rejected, &prepared, &statements, &witness, &hint)
            .is_err(),
        "a private-I/O batch takes no statement"
    );
    let mut pt = Blake3Transcript::new();
    let proof = prove_sha256_compressions_wfbitz(&mut pt, &prepared, &[], &witness, &hint)
        .expect("prove");
    let mut vt = Blake3Transcript::new();
    verify_sha256_compressions_wfbitz(&mut vt, &prepared, &[], &hint.commitment, &proof)
        .expect("verify");
    let mut cross = Blake3Transcript::new();
    assert!(
        verify_sha256_compressions_wfbitz(&mut cross, &public, &statements, &hint.commitment, &proof)
            .is_err(),
        "a private-I/O proof does not verify as a public-I/O proof"
    );
    let opening = proof.bitz().to_bytes();
    let nonces = nonces_le([proof.initial_nonce(), proof.terminal_nonce()]);
    pin(
        &sha_relation_pin("sha256/2p7/wfbitz/private-io/lambda100"),
        &pt,
        &vt,
        &[&hint.commitment.root, &opening, &nonces],
    );
}

#[cfg(feature = "bench-internals")]
#[test]
fn sha256_fixed98_product_t13_2p14() {
    let prepared =
        bitz::piop::spartan::prepare_sha256_compression_batch_for_product_t_fixed98(14, 13)
            .expect("prepare");
    sha256_pin("sha256/fixed98-t13/2p14", &prepared);
}

// ---------------------------------------------------------------- CM-AND

#[test]
fn cm_and_2p15_lambda100() {
    use bitz::piop::spartan::cm::commit_cm_and_witness_with_config;
    use bitz::piop::spartan::{
        CmAndWitness, SpartanBitzField, prepare_cm_and_relation, prove_cm_and_bitz,
        spartan_bitz_field_config, verify_cm_and_bitz,
    };
    let field_config = spartan_bitz_field_config();
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
    let proof = prove_cm_and_bitz(&mut pt, &relation, &witness, &hint).expect("prove");
    let mut vt = Blake3Transcript::new();
    verify_cm_and_bitz(&mut vt, &relation, &hint.commitment, &proof).expect("verify");
    let bitz_bytes = proof.bitz().to_bytes();
    pin(
        "cm_and/2p15/lambda100",
        &pt,
        &vt,
        &[&hint.commitment.root, &bitz_bytes],
    );
}

// ---------------------------------------------------------------- SHA-256 + ECDSA

#[cfg(feature = "ecdsa")]
fn ecdsa_pin(name: &str, lambda: u32, mode: bitz::piop::spartan::ecdsa_sha256::OuterMode) {
    use bitz::piop::spartan::ecdsa_sha256::{
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
        bitz::piop::spartan::ecdsa_sha256::OuterMode::Split,
    );
}

#[cfg(feature = "ecdsa")]
#[test]
fn sha256_ecdsa_2p3_allrows_lambda100() {
    ecdsa_pin(
        "sha256_ecdsa/2p3/allrows/lambda100",
        100,
        bitz::piop::spartan::ecdsa_sha256::OuterMode::AllRows,
    );
}

#[cfg(feature = "ecdsa")]
#[test]
fn sha256_ecdsa_2p3_split_lambda128() {
    ecdsa_pin(
        "sha256_ecdsa/2p3/split/lambda128",
        128,
        bitz::piop::spartan::ecdsa_sha256::OuterMode::Split,
    );
}

#[cfg(feature = "ecdsa")]
#[test]
fn sha256_ecdsa_2p3_allrows_lambda128() {
    ecdsa_pin(
        "sha256_ecdsa/2p3/allrows/lambda128",
        128,
        bitz::piop::spartan::ecdsa_sha256::OuterMode::AllRows,
    );
}

// ---------------------------------------------------------------- hybrid

#[cfg(feature = "hybrid")]
#[test]
fn hybrid_2p13_muls_16_compressions_johnson() {
    use bitz::hybrid::{Parameters, PreparedHybrid};
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

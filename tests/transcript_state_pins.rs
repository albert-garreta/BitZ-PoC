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

use bitz::piop::spartan::multiswap::{
    MultiswapAssignment, MultiswapCircuit, MultiswapDims, PreparedMultiswapRelation,
    commit_multiswap_witness, multiswap_lig_configs, prove_multiswap_mod_r1cs,
    verify_multiswap_mod_r1cs,
};
use bitz::piop::spartan::{
    BabyBearMulWitness, IopSecurityProfile, Lambda100, Lambda128, Sha128ReferenceSchedule,
    Sha256CompressionStatement, commit_sha256_chain_witness, commit_sha256_compression_witness,
    generate_sha256_chain_witnesses, generate_sha256_compression_witnesses,
    prepare_sha256_chain_batch_with_profile,
    prepare_sha256_compression_batch_for_assignment_rows_with_profile,
    prepare_sha256_compression_batch_with_profile, prove_sha256_chain, prove_sha256_compressions,
    verify_sha256_chain, verify_sha256_compressions,
};
use bitz::transcript::Blake3Transcript;
use blake3::Hasher;

/// `(name, prover transcript state, verifier transcript state, serialized
/// proof parts)`. The two states differ by design: flock's Ligerito prover
/// and verifier end in different transcript states after the last level, so
/// both are pinned.
// Wfbitz-only native layouts and challenge schedules; concrete versioned codecs.
const PINS: &[(&str, &str, &str, &str)] = &[
    (
        "baby_bear/2p15/lambda100",
        "3d1d0aee99c441685e29a0526691e9960d2203ae350afbafe5aa7ce97265894a",
        "3d1d0aee99c441685e29a0526691e9960d2203ae350afbafe5aa7ce97265894a",
        "f555832b0bd46257ca4f8485948e0ec6e321606e7e7136ddf7fb73d6c566019f",
    ),
    (
        "baby_bear/2p15/lambda128",
        "7ddbda115406c4af36aa1ba8e80c3a26423a89f4ba17edb32f1c62c4c27da801",
        "7ddbda115406c4af36aa1ba8e80c3a26423a89f4ba17edb32f1c62c4c27da801",
        "a01e0a915eb271fe03956c330580e184394717821fa940313e1bec0c33778f5b",
    ),
    (
        "cm_and/2p15/lambda100",
        "f3cae00da0325f99ab00734692be841ddd93f7020e247a527893259a3a198f34",
        "f3cae00da0325f99ab00734692be841ddd93f7020e247a527893259a3a198f34",
        "ce8d9bf6552f077fafbd9faad8d16848d3e4d084f690939276ea3abe9ee0e5af",
    ),
    (
        "hybrid/2p13x16/johnson",
        "-",
        "-",
        "cea14db6c452edeef7675cf2331f11231b40702f65d3f4e36b434995d6c59bb9",
    ),
    (
        "multiswap/mini/limber114",
        "222c1709920b93191d1c8a32ff9090d4a3969e9c3221f610e1dc68250bed2cdf",
        "222c1709920b93191d1c8a32ff9090d4a3969e9c3221f610e1dc68250bed2cdf",
        "95c9805ae2279b7ad66a74b6dc2e52ba30fff2818933a4583b9b13fc4bdd2f79",
    ),
    (
        "sha256/2p7/lambda100",
        "af3344c170275f485fcbd389ee21d16a183e1052e7e5bee8949e7cb31f2edd65",
        "af3344c170275f485fcbd389ee21d16a183e1052e7e5bee8949e7cb31f2edd65",
        "c55ee9da0a9f499de08209333f669380f3ddffe0a4880fae19a0130513d158e3",
    ),
    (
        "sha256/2p7/lambda128",
        "42b844f34615edc18ce7ee8538eb22bfeaadef0b3e0ea171239c7257d6d1c674",
        "42b844f34615edc18ce7ee8538eb22bfeaadef0b3e0ea171239c7257d6d1c674",
        "dc4067d0985e602091b8e4ffb599d3d7e49736f2805aac4ed5854c1889c58279",
    ),
    (
        "sha256/2p7/reference",
        "008fbac72cbdce8c16b29b018ba1046168d830222fa055e8d7cacc6426c354ac",
        "008fbac72cbdce8c16b29b018ba1046168d830222fa055e8d7cacc6426c354ac",
        "78bbaf9b5f391c6019cc73bee11d70957c5ec55f5b6a0228d0795a694d7a5658",
    ),
    (
        "sha256_chain/2p7/lambda100",
        "56d2484895a33caa0e6741b60c22198eba0cd8de1b84cb8f9abe47603de3cc32",
        "56d2484895a33caa0e6741b60c22198eba0cd8de1b84cb8f9abe47603de3cc32",
        "b505a2e601d683c8921149d5c4d7423d8673b84d28e384d69a4ca67ee7e9926d",
    ),
    (
        "sha256_chain/2p7/lambda128",
        "26f9af15b1361b48b5c0ff9c491f587a0653faf461ef34fb9d4ee7d363c874c3",
        "26f9af15b1361b48b5c0ff9c491f587a0653faf461ef34fb9d4ee7d363c874c3",
        "969eb42e0f6afd72fc2b61ee697685029d2f9f5739311d7691ec08ece73d8592",
    ),
    (
        "sha256_ecdsa/2p3/allrows/lambda100",
        "beb0c077ac922ff673bec472749762712b13129b83fca4c2e3b81cd22983af38",
        "beb0c077ac922ff673bec472749762712b13129b83fca4c2e3b81cd22983af38",
        "3124213e203b7a23d1c24d9c878c8b6b3d5df1cd97620cba1abf28e7836f1cb3",
    ),
    (
        "sha256_ecdsa/2p3/allrows/lambda128",
        "e3f7c83d3d45332ea0a74fda1c04b06f4b825fc3a591190e64b9fc0923a821c4",
        "e3f7c83d3d45332ea0a74fda1c04b06f4b825fc3a591190e64b9fc0923a821c4",
        "e5ae77e68634e15f38777290204f2f0db1e8606b6e3a10fee9af27cb6462ebe2",
    ),
    (
        "sha256_ecdsa/2p3/split/lambda100",
        "811ba91713e8757e76ea4e057cf259d24c7ca156b4cd8d1bd6c5a225855dc994",
        "811ba91713e8757e76ea4e057cf259d24c7ca156b4cd8d1bd6c5a225855dc994",
        "65dedb1e368653eda27346a0565c06cfaf9d1868e097954ff4383881cabac302",
    ),
    (
        "sha256_ecdsa/2p3/split/lambda128",
        "e738e798162fec7e4d5f422404c33f117e1b2ec877e06debfbada9e732a099bd",
        "e738e798162fec7e4d5f422404c33f117e1b2ec877e06debfbada9e732a099bd",
        "afa4a24fbd2d948192064d6cc07c18fd06e1e880365a360af85324977dacdb79",
    ),
    (
        "sha256/fixed98-t13/2p14",
        "00705725b4771c66a0a8b2a7e7ddb4ecea7873744f42978ddb2637458e06554f",
        "00705725b4771c66a0a8b2a7e7ddb4ecea7873744f42978ddb2637458e06554f",
        "d03541474aa8d6c2b2448861df80d7799f045c0272dd2165a2c1076b79193d46",
    ),
    (
        "sha256/legacy-rows21/lambda100",
        "9de471457f342e9cb7a49401a0d86a681b2cde8cab003030df0945cef056ac5a",
        "9de471457f342e9cb7a49401a0d86a681b2cde8cab003030df0945cef056ac5a",
        "1782c4b68a72a8964e12c2f0399e7360c32e4e3a2e998a9925674c47d8182892",
    ),
    (
        "u128_mul/2p15/lambda100",
        "8ebfd7dcd457cd467176d6ba75599f5792cdd04a9ea1d308a48a06dd001329a2",
        "8ebfd7dcd457cd467176d6ba75599f5792cdd04a9ea1d308a48a06dd001329a2",
        "8e5bd8146365d07e3cd708455cb20ca716ff553b02942c2b2920468183b7b4db",
    ),
    (
        "u32_mul/2p15/w1/lambda100",
        "e8c37516ac423df44d1b216b47436e91bd46ecddea6a68eb2ea3783b06db52ad",
        "e8c37516ac423df44d1b216b47436e91bd46ecddea6a68eb2ea3783b06db52ad",
        "cee1b73dabd12bd9ad7e67b6e18d63b700897289a28df47a218c7800b5ef895b",
    ),
    (
        "u32_mul/2p15/w1/lambda128",
        "4f3b09248f04c5272f189073ed7615fea9243ec8c68cf7645b06ca5978599d7c",
        "4f3b09248f04c5272f189073ed7615fea9243ec8c68cf7645b06ca5978599d7c",
        "ff41236862bbcb4535e3367804164fb02c45a85163e7c296070526fb30b2ff80",
    ),
    (
        "u64_mul/2p15/lambda100",
        "379cafdd264cba81ede61010f9218ec94dbfb877c5a4c971c88cab9f261b7152",
        "379cafdd264cba81ede61010f9218ec94dbfb877c5a4c971c88cab9f261b7152",
        "05bbc11be9838bcea404b02b2860b5c2686d2c465f8aaa5995f5f4c530cc7ac1",
    ),
    (
        "u64_mul/2p15/shift+1/lambda100",
        "13d5b56720c9c9dccbe1c448551532ad8b0e9430b48ec56160b038c9b7439922",
        "13d5b56720c9c9dccbe1c448551532ad8b0e9430b48ec56160b038c9b7439922",
        "6146da9a5290c531c8309809cedbb54ac56e0917b980bac97540eb6f7251fda8",
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

fn u32_witness() -> MulWitness<u32> {
    MulWitness::<u32>::from_fn(1usize << 15, |i| {
        let x = (i as u32).wrapping_mul(0x9e37_79b9) | 1;
        let y = (i as u32).wrapping_mul(0x85eb_ca6b) | 1;
        (x, y)
    })
    .expect("witness")
}

fn u32_pin<P: IopSecurityProfile>(name: &str) {
    let witness = u32_witness();
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
    u32_pin::<Lambda100>("u32_mul/2p15/w1/lambda100");
}

#[test]
fn u32_mul_2p15_w1_lambda128() {
    u32_pin::<Lambda128>("u32_mul/2p15/w1/lambda128");
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
    let mu_prime = bitz::piop::spartan::multiswap::reduce::encode_integer_lift(
        proof.mu_prime().expect("lift"),
    );
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
        name,
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
        bitz::piop::spartan::prepare_sha256_compression_batch_for_product_t_fixed98(14, 13)
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
    let bitz_bytes = proof.bitz().to_bytes();
    let nonces = nonces_le([proof.initial_nonce(), proof.terminal_nonce()]);
    pin(
        name,
        &pt,
        &vt,
        &[&hint.commitment.root, &bitz_bytes, &nonces],
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

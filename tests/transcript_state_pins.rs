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

use blake3::Hasher;
use f2z::piop::spartan::multiswap::{
    MultiswapAssignment, MultiswapCircuit, MultiswapDims, PreparedMultiswapRelation,
    commit_multiswap_witness, multiswap_lig_configs, prove_multiswap_mod_r1cs,
    verify_multiswap_mod_r1cs,
};
use f2z::piop::spartan::{
    BabyBearMulWitness, IopSecurityProfile, Lambda100, Lambda128, PreparedBabyBearMulRelation,
    Sha128ReferenceSchedule, Sha256CompressionStatement, SpartanReductionStrategy,
    commit_baby_bear_mul_paper_witness, commit_sha256_chain_witness,
    commit_sha256_compression_witness, generate_sha256_chain_witnesses,
    generate_sha256_compression_witnesses, prepare_sha256_chain_batch_with_profile,
    prepare_sha256_compression_batch_for_assignment_rows_with_profile,
    prepare_sha256_compression_batch_with_profile, prove_baby_bear_mul_paper,
    prove_sha256_chain, prove_sha256_compressions, verify_baby_bear_mul_paper,
    verify_sha256_chain, verify_sha256_compressions,
};
use f2z::piop::spartan::{
    PreparedU32MulRelation, PreparedU64MulRelation, PreparedU128MulRelation, U32MulF2zWidth,
    U32MulWitness, U64MulWitness, U128MulWitness, commit_u32_mul_witness,
    commit_u64_mul_witness, commit_u128_mul_witness, prove_u32_mul, prove_u64_mul,
    prove_u128_mul, verify_u32_mul, verify_u64_mul, verify_u128_mul,
};
use f2z::transcript::Blake3Transcript;

/// `(name, prover transcript state, verifier transcript state, serialized
/// proof parts)`. The two states differ by design: flock's Ligerito prover
/// and verifier end in different transcript states after the last level, so
/// both are pinned.
const PINS: &[(&str, &str, &str, &str)] = &[
    // Recorded with `F2Z_RECORD_PINS=1` on the pre-unification code
    // (master 5d2aea9, 2026-09-11).
    ("baby_bear/2p15/lambda100", "36d6d70ee81300f7633094854c2ffe044113f9be4124b6b8de770ea84b4286a8", "18456aa354b4436c8caf5487c0c6e4cf2d588d7b7c76c37a7e97465f2e171922", "2fa5d981945754a943ceca1a6101cfa0fa63e8abdd5ac5a42ac14c4c2260dad4"),
    ("baby_bear/2p15/lambda128", "e18baca3215fa14537c68ecb601417d56d7d68d4b115a6956bad06023010b64a", "320aa9f05f904a1f2e9108390ce3aa7ec1d7916d2b276e651b2fe736f1a03303", "73f3cae0b33eeba6f6c2a2a8975421e55776a6f4dbb5aebf85732fe39068798e"),
    ("cm_and/2p15/lambda100", "c68268896faafdfb4b3c01e57079b6b8e282ca65f3b752844ccf316d32f2229b", "f7ff79f5e132b966361ffb40e5aedd620a12a90a72b7b5f05cbf1941ed87c767", "1dd03b16040f867ce46fc421be1a6830b9dac69f7f192a49b963f238d9cff713"),
    ("multiswap/mini/limber114", "4ab1ecce1d1acb01ac6105c56162a2c2167e206fa6f5332028456ea61513f020", "4fd000bc191c713d9cc6c1f26b702b9fba61ba02536fb1c41fbd165117119b6d", "dbc1b5d42f41f6e5d424c93fb384056676267a2926b5e2b32cdf9604c04a0087"),
    ("sha256_chain/2p7/lambda100", "54cd62218b845ed0509b8adb6a01adb7f16d3941ee84a5b983b79acf004f954a", "3d4f162f7e7f6df10b79118ecd528c94808a42ceddb3d0403a56845c5eaa2af5", "a100728ea0813ab858a2e47e7b0a0f711dc0f12ded97534be6b4627e64cab719"),
    ("sha256_chain/2p7/lambda128", "3e0f1762b0776e062217a99efca2f1c83145e91dcd65b0cc18958958cfbd069e", "d584bafd0596b14fda0b2babc9cc3578c54466be46466739c815b65e7d707c40", "bc24af2bd1fbc68762494fdb1db23f162b11a82ec0f2abc7f78bee5b5c70fdc2"),
    ("sha256/2p7/lambda100", "8dd21f4c2421721fe023feeb6a631f5be54e6b68e22b2747ea7dde55e481d1b9", "f931788ec55d652e7e54cffb3d6dbae4fa1a2fa778000ff5a0543fe24f4ec926", "c6a47f7f6a62cd10d85423f7f35a935c28eae1905f607fd629b04b3d3444f23c"),
    ("sha256/2p7/lambda128", "9174fde9a1036f1cf97bdaf36cf95f5a70e7fe30c7f5c5b98bb3f7176b7a109d", "7b90b079d4f2d5e404bcdadb45a1633662381d59cd4589125515f26f6bd352c9", "eb5a85229710b9c56fdf699dae215e06be815aa99faa53139a07909c08d8227b"),
    ("sha256/2p7/reference", "6de14a6c3ba9bb05504dc4868d45a0ae5cba14bd0ba259df0420149cf72d0066", "14190b95b59ad69e2184e95e37d05a066e6c2690170fc9466de00b88ce0c6d1f", "c907854108bfa75f047a8aec694e47d46ea0d04806942f7dba5fd47213a2c905"),
    ("sha256/legacy-rows21/lambda100", "4bac2e7de6d659003f382dbb6f58b73d57f629fd141d6c70daf4c01fa895b124", "0a8e16f81cd521f9d882107a27925874fb1afb62ba79e3b18f5cf8862ee3476a", "057a98a5df8a586a4b19f76ccbc30401bd7a865744d2b4fce68ca0836126cffa"),
    ("u128_mul/2p15/lambda100", "f0a9af270144dadd95f5acd5e49fb28a22fc6960beef6fe23cdfb84bad4f4858", "a0041e202960567ea9fea90ddced5f66c2428e2a6a7c34cae80f8f18fb3016c3", "b3b3e27140e3132e8616402dd3051eada21a56d3b036f2f36599ed7ac8d00cf7"),
    ("u32_mul/2p15/w1/lambda100", "3713fa94ff52d93283e951098919aa88c97acfd3b37181ff68c165b430600fc8", "17a12aa10604f74018b0d8978283849a62a349166fae7895d1f5ed58888aef03", "4bc343a54ad6ec4df4c915ef80df11d6120d046098b8485b1aa03f8c0d9b385a"),
    ("u32_mul/2p15/w1/lambda128", "4d8b55ce0ceb2b60d0004706d30f567c75f1c155f8d2cc8fc761adc98004d1ac", "83b6a754c046b1223a397f2d0549e3ed76ac348c15e3c2be8d7c71f2e5140458", "e10149b81e36328c089cabeeb67c5a69c246367a220d88c8aaf36ee75cebbb2f"),
    ("u32_mul/2p15/w8/lambda100", "0dba1e7de9a45e3e340e0fdb2eb3155aa7a57dc6971044d6eb186fa27356a1dd", "ba663d87a56d06b8a97c7cdb721f94a765d80e24cce392359e813cc3dcf8d902", "9de3f2a1a955c612170b13cd66e31ed10d562a4f93a80479df9f324a6b334299"),
    ("u64_mul/2p15/lambda100", "10524e78940e5f90f8275746197646780ea7e5dcf1ba3cb22bb4ac234c63fac9", "5843f4c036b615aeee94d9952b0253ded59ef0f0ebe9cffc464c27c8d60d76f7", "d06897db59f4d65e5feb4e5030479b8150e81ba956ec22555a2f2e5f33d6ee49"),
    ("u64_mul/2p15/shift+1/lambda100", "ea2a0e03edb7ce5430479bdf4bb11d0f00fb19dbcdd07788233a8acf0c4b8630", "f23bb20b0c12c4cea5e53157166511adc5fd1a2830c90bd052675e29c369de6d", "5d6b769e91d478d30dbfa656ee18b1f8020cae6757c4276aa15c97f5e1757406"),
    ("sha256_ecdsa/2p3/allrows/lambda100", "5aca8ae95c9228db846a44fddb1eeefce535a99ad9cf3e20073333e44dbe6e6b", "5aca8ae95c9228db846a44fddb1eeefce535a99ad9cf3e20073333e44dbe6e6b", "729bc1a0eccb80f5e42c3efe968e64600480d075b5adb0ad969726e5d41c1b72"),
    ("sha256_ecdsa/2p3/allrows/lambda128", "f4e9e3919b5c0d5be7bc73ca602bed71226eae8b281d0f62d81a5740d5871817", "f4e9e3919b5c0d5be7bc73ca602bed71226eae8b281d0f62d81a5740d5871817", "5b56247305180b3e46cd5ac99e521941cc1d5cbc2129d39454426a75ab771639"),
    ("sha256_ecdsa/2p3/split/lambda100", "5c08b4a910a906e9e495af348eaf253950b139724e66db5611084c5235092d3c", "5c08b4a910a906e9e495af348eaf253950b139724e66db5611084c5235092d3c", "98d86d6d85594691b2884ca40c64da36c21090281744a5194add27c4e8bcf30a"),
    ("sha256_ecdsa/2p3/split/lambda128", "942bb995a8cbb6dee834c8c6c21922c2558c4838748faa0ab16c93562fb9516e", "942bb995a8cbb6dee834c8c6c21922c2558c4838748faa0ab16c93562fb9516e", "544140f516613b9a241ac556fbfe203c396d798a0329cdbe5e8513302e07450b"),
    ("sha256/fixed98-t13/2p14", "1e003e6e7ad04324c87a52945954d31103c8555d030608a26924efba20e36900", "9aa0798d570c4ecc1cf6a60d91589f62314375a771db7359ae26adada656ad8c", "6b5fe05d55ae343c81e82ff10a7777b32abda6f72573c6885206c0d4de7ed12d"),
    ("hybrid/2p13x16/johnson", "-", "-", "7435a68c2c98260f735183cade04afac4bf73dfec22fc7553067bbb571b5b8a9"),
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
    blake3::Hash::from(transcript.state_digest()).to_hex().to_string()
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
    assert_eq!(prover_state, expected.1, "{name}: prover transcript state moved");
    assert_eq!(verifier_state, expected.2, "{name}: verifier transcript state moved");
    assert_eq!(bytes, expected.3, "{name}: serialized proof parts moved");
}

/// Checks (or records) one pin: the prover's and the verifier's final
/// transcript states plus the serialized proof parts.
fn pin(name: &str, prover: &Blake3Transcript, verifier: &Blake3Transcript, parts: &[&[u8]]) {
    check(name, &state_hex(prover), &state_hex(verifier), &digest_hex(parts));
}

/// Pins a proof whose prover builds its own transcript (no state digest).
fn pin_bytes(name: &str, parts: &[&[u8]]) {
    check(name, "-", "-", &digest_hex(parts));
}

fn nonces_le(nonces: impl IntoIterator<Item = u64>) -> Vec<u8> {
    nonces.into_iter().flat_map(|n| n.to_le_bytes()).collect()
}

// ---------------------------------------------------------------- u32 mul

fn u32_witness(width: U32MulF2zWidth) -> U32MulWitness {
    U32MulWitness::from_fn_with_f2z_width(1usize << 15, width, |i| {
        let x = (i as u32).wrapping_mul(0x9e37_79b9) | 1;
        let y = (i as u32).wrapping_mul(0x85eb_ca6b) | 1;
        (x, y)
    })
    .expect("witness")
}

fn u32_pin<P: IopSecurityProfile>(name: &str, width: U32MulF2zWidth) {
    let witness = u32_witness(width);
    let prepared = PreparedU32MulRelation::new_with_profile::<P>(*witness.layout()).expect("prepare");
    let hint = commit_u32_mul_witness(&prepared, witness.f2z_bit_rows()).expect("commit");
    let mut pt = Blake3Transcript::new();
    let proof = prove_u32_mul(&mut pt, &prepared, &witness, &hint).expect("prove");
    let mut vt = Blake3Transcript::new();
    verify_u32_mul(&mut vt, &prepared, &hint.commitment, &proof).expect("verify");
    // The grinding nonces are absorbed into the transcript, so the state
    // digest already covers them.
    let f2z_bytes = proof.f2z().to_bytes();
    pin(name, &pt, &vt, &[&hint.commitment.root, &f2z_bytes]);
}

#[test]
fn u32_mul_2p15_w1_lambda100() {
    u32_pin::<Lambda100>("u32_mul/2p15/w1/lambda100", U32MulF2zWidth::W1);
}

#[test]
fn u32_mul_2p15_w8_lambda100() {
    u32_pin::<Lambda100>("u32_mul/2p15/w8/lambda100", U32MulF2zWidth::W8);
}

#[test]
fn u32_mul_2p15_w1_lambda128() {
    u32_pin::<Lambda128>("u32_mul/2p15/w1/lambda128", U32MulF2zWidth::W1);
}

// ---------------------------------------------------------------- u64 mul

fn u64_pin(name: &str, split_shift: i8) {
    let witness = U64MulWitness::from_fn(1usize << 15, |i| {
        let x = (i as u64).wrapping_mul(0x9e37_79b9_7f4a_7c15) | 1;
        let y = (i as u64).wrapping_mul(0xc2b2_ae3d_27d4_eb4f) | 1;
        (x, y)
    })
    .expect("witness")
    .with_split_shift(split_shift)
    .expect("split shift");
    let prepared = PreparedU64MulRelation::new(*witness.layout()).expect("prepare");
    let hint = commit_u64_mul_witness(&prepared, witness.f2z_bit_rows()).expect("commit");
    let mut pt = Blake3Transcript::new();
    let proof = prove_u64_mul(&mut pt, &prepared, &witness, &hint).expect("prove");
    let mut vt = Blake3Transcript::new();
    verify_u64_mul(&mut vt, &prepared, &hint.commitment, &proof).expect("verify");
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
    let witness = U128MulWitness::from_fn(1usize << 15, |i| {
        let lo = (i as u64).wrapping_mul(0x9e37_79b9_7f4a_7c15) | 1;
        let hi = (i as u64).wrapping_mul(0xc2b2_ae3d_27d4_eb4f) | 1;
        let x = (u128::from(hi) << 64) | u128::from(lo);
        let y = (u128::from(lo.rotate_left(17)) << 64) | u128::from(hi ^ 0x5851_f42d_4c95_7f2d);
        (x, y)
    })
    .expect("witness");
    let prepared = PreparedU128MulRelation::new(*witness.layout()).expect("prepare");
    let hint = commit_u128_mul_witness(&prepared, witness.f2z_bit_rows()).expect("commit");
    let mut pt = Blake3Transcript::new();
    let proof = prove_u128_mul(&mut pt, &prepared, &witness, &hint).expect("prove");
    let mut vt = Blake3Transcript::new();
    verify_u128_mul(&mut vt, &prepared, &hint.commitment, &proof).expect("verify");
    let f2z_bytes = proof.f2z().to_bytes();
    pin("u128_mul/2p15/lambda100", &pt, &vt, &[&hint.commitment.root, &f2z_bytes]);
}

// ---------------------------------------------------------------- BabyBear

fn baby_bear_pin<P: IopSecurityProfile>(name: &str) {
    let witness = BabyBearMulWitness::from_fn(1usize << 15, |i| {
        let a = (i as u32).wrapping_mul(0x9e37_79b9) % 2_013_265_921;
        let b = (i as u32).wrapping_mul(0x85eb_ca6b) % 2_013_265_921;
        (a, b)
    })
    .expect("witness");
    let prepared =
        PreparedBabyBearMulRelation::new_with_profile::<P>(*witness.layout()).expect("prepare");
    let hint =
        commit_baby_bear_mul_paper_witness(&prepared, witness.f2z_bit_rows()).expect("commit");
    let mut pt = Blake3Transcript::new();
    let proof = prove_baby_bear_mul_paper(
        &mut pt,
        &prepared,
        &witness,
        &hint,
        SpartanReductionStrategy::DelayedBarrett,
    )
    .expect("prove");
    let mut vt = Blake3Transcript::new();
    verify_baby_bear_mul_paper(&mut vt, &prepared, &hint.commitment, &proof).expect("verify");
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
    let hint =
        commit_multiswap_witness(prepared.params(), assignment.f2z_bit_rows(), &pc).expect("commit");
    let mut pt = Blake3Transcript::new();
    let proof =
        prove_multiswap_mod_r1cs(&mut pt, &prepared, &assignment, &hint, &pc).expect("prove");
    let mut vt = Blake3Transcript::new();
    verify_multiswap_mod_r1cs(&mut vt, &prepared, &hint.commitment, &proof, &vc).expect("verify");
    let f2z_bytes = proof.f2z().to_bytes();
    let mu_prime = proof.mu_prime().to_bytes_le();
    let nonce = proof.reduction_nonce().to_le_bytes();
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

fn sha256_pin(
    name: &str,
    prepared: &f2z::piop::spartan::PreparedSha256CompressionBatch,
) {
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
    pin(name, &pt, &vt, &[&hint.commitment.root, &f2z_bytes, &nonces]);
}

#[test]
fn sha256_2p7_lambda100() {
    let prepared = prepare_sha256_compression_batch_with_profile::<Lambda100>(7).expect("prepare");
    sha256_pin("sha256/2p7/lambda100", &prepared);
}

#[test]
fn sha256_2p7_reference_schedule() {
    let prepared =
        prepare_sha256_compression_batch_with_profile::<Sha128ReferenceSchedule>(7).expect("prepare");
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
    let proof =
        prove_sha256_chain(&mut pt, &prepared, &statement, &witness, &hint).expect("prove");
    let mut vt = Blake3Transcript::new();
    verify_sha256_chain(&mut vt, &prepared, &statement, &hint.commitment, &proof).expect("verify");
    let f2z_bytes = proof.f2z().to_bytes();
    let nonces = nonces_le([proof.initial_nonce(), proof.terminal_nonce()]);
    pin(name, &pt, &vt, &[&hint.commitment.root, &f2z_bytes, &nonces]);
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
        CmAndWitness, SpartanF2zField, prepare_cm_and_relation, project_cm_and_witness,
        prove_cm_and_f2z, spartan_f2z_field_config, verify_cm_and_f2z,
    };
    let field_config = spartan_f2z_field_config();
    let witness = CmAndWitness::from_fn(1usize << 15, |i| {
        let x = (i as u32).wrapping_mul(0x9e37_79b9) | 1;
        let y = (i as u32).wrapping_mul(0x85eb_ca6b) | 1;
        (x, y)
    })
    .expect("witness");
    let layout = *witness.layout();
    let relation =
        prepare_cm_and_relation::<SpartanF2zField>(layout, &field_config).expect("relation");
    let pc = relation.ligerito_configuration().expect("ligerito").prover();
    let hint = commit_cm_and_witness_with_config(&layout, witness.f_bit_rows(), pc).expect("commit");
    let projected =
        project_cm_and_witness::<SpartanF2zField>(&witness, &field_config).expect("project");
    let mut pt = Blake3Transcript::new();
    let proof = prove_cm_and_f2z(&mut pt, &relation, projected, &hint).expect("prove");
    let mut vt = Blake3Transcript::new();
    verify_cm_and_f2z(&mut vt, &relation, &hint.commitment, &proof).expect("verify");
    let f2z_bytes = proof.f2z().to_bytes();
    pin("cm_and/2p15/lambda100", &pt, &vt, &[&hint.commitment.root, &f2z_bytes]);
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
    prepared.verify(committed.statement(), &proof).expect("verify");
    let bytes = proof.to_bytes();
    let roots: Vec<u8> = committed
        .statement()
        .roots
        .iter()
        .flat_map(|r| r.iter().copied())
        .collect();
    pin_bytes("hybrid/2p13x16/johnson", &[&roots, &bytes]);
}

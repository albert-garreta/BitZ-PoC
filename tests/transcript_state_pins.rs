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
    Sha256CompressionStatement, commit_sha256_compression_witness,
    generate_sha256_compression_witnesses,
    prepare_sha256_compression_batch_block_with_profile,
    prepare_sha256_compression_batch_for_assignment_rows_with_profile,
    prepare_sha256_compression_batch_with_profile, prove_sha256_compressions,
    verify_sha256_compressions,
};
use bitz::transcript::Blake3Transcript;
use blake3::Hasher;

/// `(name, prover transcript state, verifier transcript state, serialized
/// proof parts)`. The two states differ by design: flock's Ligerito prover
/// and verifier end in different transcript states after the last level, so
/// both are pinned.
// BitZ-only native layouts and challenge schedules; concrete versioned codecs.
const PINS: &[(&str, &str, &str, &str)] = &[
    (
        "baby_bear/2p15/lambda100",
        "8c3d9fcd869a04f202fcd031f7067b469c109e3e06f7f8cdff6e173405f901e1",
        "8c3d9fcd869a04f202fcd031f7067b469c109e3e06f7f8cdff6e173405f901e1",
        "b98bc38507d34fef2d1fe856ddfdda3bbc855eb2feb6694fa483e93e6094aa00",
    ),
    (
        "baby_bear/2p15/lambda128",
        "c530e3d67f0867b7fe599de983408abc8ce740d47408115de037493f1787cff8",
        "c530e3d67f0867b7fe599de983408abc8ce740d47408115de037493f1787cff8",
        "c8c7067be7c1ca9d2a1807491362254286ec6c6e75b761b98444e3c93fa3247c",
    ),
    (
        "cm_and/2p15/lambda100",
        "f3cae00da0325f99ab00734692be841ddd93f7020e247a527893259a3a198f34",
        "f3cae00da0325f99ab00734692be841ddd93f7020e247a527893259a3a198f34",
        "8ee32e1fcad24c2a505313b3f06e272344136b1712a12bf865b1d7d10e00dda7",
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
        "dcb8c5cb43072493814c01b2f462418acb8a1b6613d066a81f162c8ebdb801ec",
    ),
    (
        "sha256/2p7/block/lambda100",
        "373712a958fefabf7dc5bf2598f1a8c40b4fd13cbbd2237e52924f9ec61b9595",
        "373712a958fefabf7dc5bf2598f1a8c40b4fd13cbbd2237e52924f9ec61b9595",
        "2a98020af76f6c9b85995f609009042254a469b8c563414d33be7de43a608924",
    ),
    (
        "sha256/2p7/block/private-io/lambda100",
        "13d3ff0a6266b3fba1e4da888d418e754d25c35b685721eec1e25777682b7140",
        "13d3ff0a6266b3fba1e4da888d418e754d25c35b685721eec1e25777682b7140",
        "e13805039c7d227600c1703524ea36f9e41e0a0173ae6440f576d9cda44a7fb4",
    ),
    (
        "sha256/2p7/lambda100",
        "a03a3bf646962fab44b15e926982da1aa81f78acb366e90c4504b835fc54dbe8",
        "a03a3bf646962fab44b15e926982da1aa81f78acb366e90c4504b835fc54dbe8",
        "03cae73bb617b1ef208a08d13f256d0133988e302ae77737e7f482a34ad81064",
    ),
    (
        "sha256/2p7/lambda128",
        "c0e0361530c0f79ea3a59caa0a239e2e3ced46577d8188d0e7872ce42a64abb4",
        "c0e0361530c0f79ea3a59caa0a239e2e3ced46577d8188d0e7872ce42a64abb4",
        "4cc2e1883c8f78797555e9a1bee7ea294687852555bbf4d4c0fc3135898f521c",
    ),
    (
        "sha256/2p7/reference",
        "f34f5c9f6c245dbb331248662324812e600317b4efadb3cd6550c5eaf8cf6f40",
        "f34f5c9f6c245dbb331248662324812e600317b4efadb3cd6550c5eaf8cf6f40",
        "4818fd453d6847c55c183b93b4a95d9a18406e736d7c0b4c0bc9b93cad3b0b0b",
    ),
    (
        "sha256/2p9/block/private-io/lambda100",
        "f7b6d26aa0505eb24ba9636eead7bf3abbdefd07b33399f2cf60c09175d77cf3",
        "f7b6d26aa0505eb24ba9636eead7bf3abbdefd07b33399f2cf60c09175d77cf3",
        "18efc3254d9e140dbe5ff97cd296cfb2d082acea1728d604394668793faa7695",
    ),
    (
        "sha256/legacy-rows24/lambda100",
        "0a709bd5a9818613b0d43c09fec6eb4e20175e135164d53129545b1ab3a75cda",
        "0a709bd5a9818613b0d43c09fec6eb4e20175e135164d53129545b1ab3a75cda",
        "dfb72de61cbc50783bc4903e003785482f9214c63f29376882d00463105687f1",
    ),
    (
        "sha256_ecdsa/2p3/allrows/lambda100",
        "beb0c077ac922ff673bec472749762712b13129b83fca4c2e3b81cd22983af38",
        "beb0c077ac922ff673bec472749762712b13129b83fca4c2e3b81cd22983af38",
        "116d6af0111ac9add46dd7fc6fe8684323dcbf2f33bb035c20cbe18ba3f9c73c",
    ),
    (
        "sha256_ecdsa/2p3/allrows/lambda128",
        "e46a8ad781318820b226aa075334a92b9b25f2516569adc618e65338467e9c12",
        "e46a8ad781318820b226aa075334a92b9b25f2516569adc618e65338467e9c12",
        "b231900c4e6358a0d09d80ad9f7852f292b2e7bb625baa85afa906e3cba85708",
    ),
    (
        "sha256_ecdsa/2p3/split/lambda100",
        "811ba91713e8757e76ea4e057cf259d24c7ca156b4cd8d1bd6c5a225855dc994",
        "811ba91713e8757e76ea4e057cf259d24c7ca156b4cd8d1bd6c5a225855dc994",
        "fbc9f37ed48d31d545b2bef1327224cac79b01e55bb372ee607b83ce9b217169",
    ),
    (
        "sha256_ecdsa/2p3/split/lambda128",
        "3ced815cce60cd3fef31a7cefd217934adecdcd302c3d027437a6f5c74cdf11e",
        "3ced815cce60cd3fef31a7cefd217934adecdcd302c3d027437a6f5c74cdf11e",
        "10f8f719e55af0405c3b2e619d09a92a7b566202293777075e947a2bbbf5adfb",
    ),
    (
        "u128_mul/2p15/lambda100",
        "197072773b6d8425862ed0cc109c3f701bd1f653307ffc1019e0902ee6562e48",
        "197072773b6d8425862ed0cc109c3f701bd1f653307ffc1019e0902ee6562e48",
        "120a209150ef0e890846e290ef692f60ec8559b5161011ad47d60cad02e5f280",
    ),
    (
        "u32_mul/2p15/w1/lambda100",
        "016cf596a54aa99a79a6d20a007e40f1d4dd134be83254bfccb724596259af27",
        "016cf596a54aa99a79a6d20a007e40f1d4dd134be83254bfccb724596259af27",
        "664f3ae1b199f35e997e87d79e117f08f3f18a946182df578e1d8ea01b03e45d",
    ),
    (
        "u32_mul/2p15/w1/lambda128",
        "44e76de747f67a1baaae428ec8cfaebdfb4e0414288c35375cfb148cad51696a",
        "44e76de747f67a1baaae428ec8cfaebdfb4e0414288c35375cfb148cad51696a",
        "2c01e08532d1497bd9dac85000d788d89e9a0547c070bc629cc40a91ab1f4edb",
    ),
    (
        "u64_mul/2p15/lambda100",
        "430c3b2411088ae3818c681c01f2b22a99041899a30f4547e142d5b230748603",
        "430c3b2411088ae3818c681c01f2b22a99041899a30f4547e142d5b230748603",
        "1a56087eae4efd0990ecd3360dd9099d6760b78dc3594859dbf66fbac8570118",
    ),
    (
        "u64_mul/2p15/shift+1/lambda100",
        "26f5e94960932bb7251be7f833231c38dfeb4242b4f1d7c7b09df1028fbbfecc",
        "26f5e94960932bb7251be7f833231c38dfeb4242b4f1d7c7b09df1028fbbfecc",
        "f21af8b08585c0369a2e191a58b3f8e008c510eaaf6a585d1d79935acbbb2b52",
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

fn sha256_statements(
    inputs: &[([u32; 8], [u32; 16])],
    witness: &bitz::piop::spartan::Sha256CompressionWitnessBatch,
) -> Vec<Sha256CompressionStatement> {
    inputs
        .iter()
        .copied()
        .zip(witness.outputs().iter().copied())
        .map(|(input, output)| Sha256CompressionStatement::new(input, output))
        .collect()
}

/// Proves and pins one batch: the statement is empty for a private-I/O
/// batch, the inputs and outputs otherwise.
fn sha256_pin(name: &str, prepared: &bitz::piop::spartan::PreparedSha256CompressionBatch) {
    let inputs = sha256_inputs(prepared.instances());
    let witness = generate_sha256_compression_witnesses(prepared, &inputs).expect("witness");
    let statements = if prepared.private_io() {
        Vec::new()
    } else {
        sha256_statements(&inputs, &witness)
    };
    let hint = commit_sha256_compression_witness(prepared, &witness).expect("commit");
    let mut pt = Blake3Transcript::new();
    let proof =
        prove_sha256_compressions(&mut pt, prepared, &statements, &witness, &hint).expect("prove");
    let mut vt = Blake3Transcript::new();
    verify_sha256_compressions(&mut vt, prepared, &statements, &hint.commitment, &proof)
        .expect("verify");
    let bitz_bytes = proof.bitz().to_bytes();
    let product_nonces = proof.product().map_or(&[][..], |product| product.nonces());
    let nonces = nonces_le(
        proof
            .inner_nonces()
            .iter()
            .chain(product_nonces)
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

/// The legacy inner-sumcheck path needs a non-power-of-two batch: `2^24`
/// cells hold 1025 compressions of the `2^14`-stride relation (`2^21` holds
/// exactly 128 and opens the product tensor).
#[test]
fn sha256_legacy_inner_sumcheck_rows24_lambda100() {
    let prepared =
        prepare_sha256_compression_batch_for_assignment_rows_with_profile::<Lambda100>(24)
            .expect("prepare");
    assert_eq!(prepared.instances(), 1025);
    sha256_pin("sha256/legacy-rows24/lambda100", &prepared);
}

/// The SHA bench's opening: BitZ on the `Id ⊗ M` block-layout commitment
/// (the opening forks its own transcript after the claim frame).
#[test]
fn sha256_2p7_block_lambda100() {
    let prepared =
        prepare_sha256_compression_batch_block_with_profile::<Lambda100>(7).expect("prepare");
    sha256_pin("sha256/2p7/block/lambda100", &prepared);
}

/// The block opening at `2^9` compressions: the smallest SHA grid (`t + s =
/// 23`) on which the forest reads nibble rows, so the pin covers that path
/// (the `2^7` pins run below it).
#[test]
fn sha256_2p9_block_private_io_lambda100() {
    let prepared = prepare_sha256_compression_batch_block_with_profile::<Lambda100>(9)
        .expect("prepare")
        .with_private_io();
    sha256_pin("sha256/2p9/block/private-io/lambda100", &prepared);
}

/// The same batch with nothing public (Flock's `sha2_proof` statement): the
/// statement is empty, and the proof neither takes a statement nor verifies
/// as the public-I/O proof.
#[test]
fn sha256_2p7_block_private_io_lambda100() {
    let public =
        prepare_sha256_compression_batch_block_with_profile::<Lambda100>(7).expect("prepare");
    let prepared = public.clone().with_private_io();
    let inputs = sha256_inputs(prepared.instances());
    let witness = generate_sha256_compression_witnesses(&prepared, &inputs).expect("witness");
    let statements = sha256_statements(&inputs, &witness);
    let hint = commit_sha256_compression_witness(&prepared, &witness).expect("commit");
    let mut rejected = Blake3Transcript::new();
    assert!(
        prove_sha256_compressions(&mut rejected, &prepared, &statements, &witness, &hint).is_err(),
        "a private-I/O batch takes no statement"
    );
    let mut pt = Blake3Transcript::new();
    let proof =
        prove_sha256_compressions(&mut pt, &prepared, &[], &witness, &hint).expect("prove");
    let mut cross = Blake3Transcript::new();
    assert!(
        verify_sha256_compressions(&mut cross, &public, &statements, &hint.commitment, &proof)
            .is_err(),
        "a private-I/O proof does not verify as a public-I/O proof"
    );
    sha256_pin("sha256/2p7/block/private-io/lambda100", &prepared);
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

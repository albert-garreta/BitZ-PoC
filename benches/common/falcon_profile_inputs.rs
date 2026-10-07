//! Reproducible upstream original-Falcon inputs for degree-comparison experiments.
use bitz::piop::spartan::falcon_parameters::FalconParameters;
use fn_dsa::{
    DOMAIN_NONE, HASH_ID_ORIGINAL_FALCON, KeyPairGenerator, KeyPairGeneratorStandard, SigningKey,
    SigningKeyStandard, VerifyingKey, VerifyingKeyStandard, sign_key_size, signature_size,
    vrfy_key_size,
};
use rand_chacha::{ChaCha20Rng, rand_core::SeedableRng};

pub fn upstream(n: usize, batch: usize) -> Vec<(Vec<u8>, [u8; 32], Vec<u8>)> {
    let log = n.ilog2();
    let mut rng = ChaCha20Rng::seed_from_u64(0xface + n as u64);
    (0..batch)
        .map(|i| {
            let mut secret = vec![0; sign_key_size(log)];
            let mut pk = vec![0; vrfy_key_size(log)];
            KeyPairGeneratorStandard::default().keygen(log, &mut rng, &mut secret, &mut pk);
            let mut signer = SigningKeyStandard::decode(&secret).unwrap();
            let verifier = VerifyingKeyStandard::decode(&pk).unwrap();
            let mut message = [7; 32];
            message[..8].copy_from_slice(&(i as u64).to_le_bytes());
            let mut signature = vec![0; signature_size(log)];
            signer.sign(
                &mut rng,
                &DOMAIN_NONE,
                &HASH_ID_ORIGINAL_FALCON,
                &message,
                &mut signature,
            );
            assert!(verifier.verify(&signature, &DOMAIN_NONE, &HASH_ID_ORIGINAL_FALCON, &message));
            let mut s2 = vec![0i16; n];
            assert!(fn_dsa_comm::codec::comp_decode(&signature[41..], &mut s2));
            let params = FalconParameters::for_degree(n);
            let mut ct = vec![0; params.signature_bytes()];
            ct[0] = 0x50 + log as u8;
            ct[1..41].copy_from_slice(&signature[1..41]);
            for (j, &s) in s2.iter().enumerate() {
                for bit in 0..params.signature_bits {
                    let offset = j * params.signature_bits + bit;
                    ct[41 + offset / 8] |= (((s as u16) >> (params.signature_bits - 1 - bit) & 1)
                        as u8)
                        << (7 - offset % 8);
                }
            }
            (pk, message, ct)
        })
        .collect()
}

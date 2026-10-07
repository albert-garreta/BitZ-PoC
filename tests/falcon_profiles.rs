#![cfg(feature = "falcon-hybrid")]
use bitz::piop::spartan::falcon_profiles::*;
#[path = "../benches/common/falcon_profile_inputs.rs"]
mod inputs;
use inputs::upstream;
use std::{error::Error, time::Instant};

const LOGN: usize = 9;
bitz::falcon_profile! {
    pub ExpressionProfile {
        n: 1 << LOGN,
        security_bits: 100,
        max_batch: 16,
        protocol: NativeCarry,
        ring_extension: Auto,
    }
}

#[test]
fn constant_expressions_and_batch_limits() {
    assert_eq!(ExpressionProfile::N, 512);
    assert_eq!(ExpressionProfile::K, 9);
    assert!(PreparedFalconHybrid::<ExpressionProfile>::new(0).is_err());
    assert!(PreparedFalconHybrid::<ExpressionProfile>::new(17).is_err());
    assert!(PreparedFalconHybrid::<ExpressionProfile>::new(16).is_ok());
}

macro_rules! proof_case {
    ($name:ident,$profile:ident,$n:expr,$bits:expr,$protocol:ident,$selection:ident $(($k:expr))?,$batch:expr) => {
        bitz::falcon_profile! { pub $profile {n:$n,security_bits:$bits,max_batch:1024,protocol:$protocol,ring_extension:$selection $(($k))?,} }
        #[test]
        fn $name()->Result<(),Box<dyn Error>> {
            let cases=upstream($n,$batch);
            let public=FalconPublicStatement::<$profile>::from_bytes(
                &cases.iter().map(|c|c.0.as_slice()).collect::<Vec<_>>(),
                &cases.iter().map(|c|c.1.as_slice()).collect::<Vec<_>>(),
                &cases.iter().map(|c|c.2.as_slice()).collect::<Vec<_>>())?;
            let prepared=PreparedFalconHybrid::<$profile>::new($batch)?;
            assert!(prepared.security().algebraic_bits >= $bits as f64);
            let start=Instant::now();let committed=prepared.commit(public)?;let commit=start.elapsed();
            let statement=committed.statement.clone();let start=Instant::now();let proof=prepared.prove(committed)?;let prove=start.elapsed();
            let start=Instant::now();prepared.verify(&statement,&proof)?;let verify=start.elapsed();
            eprintln!("FALCON_PROFILE n={} k={} bits={} protocol={:?} batch={} security={:.3} commit_ms={} prove_ms={} verify_ms={} proof_bytes={}",
                $n,prepared.extension_degree(),$bits,$profile::PROTOCOL,$batch,prepared.security().algebraic_bits,
                commit.as_millis(),prove.as_millis(),verify.as_millis(),proof.payload_size_bytes());
            let mut changed=statement.clone();changed.public.messages[0][0]^=1;assert!(prepared.verify(&changed,&proof).is_err());
            let mut changed=statement.clone();changed.public.signatures[0].s2[0]+=1;assert!(prepared.verify(&changed,&proof).is_err());
            let different_max=<$profile as FalconProfile>::Backend::prepare($batch,$bits,$profile::PROTOCOL,1023)?;
            assert!(different_max.verify(&statement,&proof).is_err());
            assert!(PreparedFalconHybrid::<$profile>::new(1025).is_err());
            Ok(())
        }
    };
}
proof_case!(
    falcon512_native100,
    P512N100,
    512,
    100,
    NativeCarry,
    Auto,
    1
);
proof_case!(
    falcon512_shared100_padded,
    P512S100,
    512,
    100,
    SharedPrime,
    Auto,
    3
);
proof_case!(
    falcon1024_native100_padded,
    P1024N100,
    1024,
    100,
    NativeCarry,
    Auto,
    3
);
proof_case!(
    falcon1024_shared100,
    P1024S100,
    1024,
    100,
    SharedPrime,
    Auto,
    1
);
proof_case!(
    falcon512_native128,
    P512N128,
    512,
    128,
    NativeCarry,
    Auto,
    1
);
proof_case!(
    falcon512_shared128,
    P512S128,
    512,
    128,
    SharedPrime,
    Auto,
    1
);
proof_case!(
    falcon1024_native128,
    P1024N128,
    1024,
    128,
    NativeCarry,
    Auto,
    1
);
proof_case!(
    falcon1024_shared128,
    P1024S128,
    1024,
    128,
    SharedPrime,
    Auto,
    1
);
proof_case!(
    falcon512_explicit11,
    P512Explicit,
    512,
    100,
    NativeCarry,
    Explicit(11),
    1
);
proof_case!(
    falcon1024_explicit11,
    P1024Explicit,
    1024,
    100,
    NativeCarry,
    Explicit(11),
    1
);

macro_rules! small_degree {
    ($name:ident,$backend:ident) => {
        #[test]
        fn $name() -> Result<(), Box<dyn Error>> {
            use bitz::piop::spartan::falcon_profiles::$backend as b;
            let message = [3u8; 32];
            let nonce = [9u8; 40];
            let hash = b::hash_to_point_ct(&nonce, &message)?;
            let limit = (1i32 << (b::SIGNATURE_BITS - 1)) - 1;
            let s2 = std::array::from_fn(|i| {
                (-limit..=limit)
                    .min_by_key(|&s| {
                        let residue = (i32::from(hash.point[i]) - 127 * s).rem_euclid(12289);
                        let centered = if residue > 6144 {
                            residue - 12289
                        } else {
                            residue
                        };
                        centered * centered + s * s
                    })
                    .unwrap() as i16
            });
            let signature = b::FalconSignatureCt {
                nonce,
                s2: Box::new(s2),
            };
            let encoded = b::encode_signature_ct(&signature)?;
            assert_eq!(b::decode_signature_ct(&encoded)?, signature);
            let mut pk = vec![0; b::PUBLIC_KEY_BYTES];
            pk[0] = b::COEFFICIENT_LOG as u8;
            pk[1] = 1;
            pk[2] = 0xfc;
            let trace = b::verification_trace(&pk, &message, &encoded)?;
            b::check_exact_constraints(&trace)?;
            let mut changed = encoded;
            changed[0] ^= 1;
            assert!(b::decode_signature_ct(&changed).is_err());
            if b::N * b::SIGNATURE_BITS % 8 != 0 {
                let mut changed = encoded;
                *changed.last_mut().unwrap() |= 1;
                assert!(b::decode_signature_ct(&changed).is_err());
            }
            if b::N * 14 % 8 != 0 {
                let mut changed = pk.clone();
                *changed.last_mut().unwrap() |= 1;
                assert!(b::decode_public_key(&changed).is_err());
            }
            let mut invalid = signature.clone();
            invalid.s2[0] = -(1 << (b::SIGNATURE_BITS - 1));
            assert!(b::encode_signature_ct(&invalid).is_err());
            let mut invalid = trace.clone();
            invalid.s1[0] += 1;
            assert!(b::check_exact_constraints(&invalid).is_err());
            // Tiny degrees also exercise the biased prefix inside the complete proof.
            if b::N <= 4 {
                let public =
                    b::FalconPublicStatement::from_bytes(&[&pk], &[&message], &[&encoded])?;
                for protocol in [FalconProtocol::NativeCarry, FalconProtocol::SharedPrime] {
                    let prepared = b::PreparedFalconHybrid::with_protocol(1, 100, protocol)?;
                    let committed = prepared.commit(public.clone())?;
                    let statement = committed.statement.clone();
                    let proof = prepared.prove(committed)?;
                    prepared.verify(&statement, &proof)?;
                }
            }
            Ok(())
        }
    };
}
small_degree!(degree_2, n2_k9);
small_degree!(degree_4, n4_k9);
small_degree!(degree_8, n8_k9);
small_degree!(degree_16, n16_k9);
small_degree!(degree_32, n32_k9);
small_degree!(degree_64, n64_k9);
small_degree!(degree_128, n128_k9);
small_degree!(degree_256, n256_k9);
small_degree!(degree_2_k8, n2_k8);
small_degree!(degree_4_k10, n4_k10);

proof_case!(falcon512_native128_padded, P512N128Padded, 512, 128, NativeCarry, Auto, 3);
proof_case!(falcon1024_shared128_padded, P1024S128Padded, 1024, 128, SharedPrime, Auto, 3);

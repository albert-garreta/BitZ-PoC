#![cfg(feature = "falcon-hybrid")]
use bitz::piop::spartan::falcon_profiles::*;
#[path = "../benches/common/falcon_degree_inputs.rs"]
mod inputs;
use std::error::Error;

const LOGN: usize = 9;
bitz::falcon_profile! {
    pub ExpressionProfile {
        n: 1 << LOGN,
        security_bits: 100,
        max_batch: 16,
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
    assert_eq!(Falcon512_100::K, 9);
    assert_eq!(Falcon1024_100::K, 9);
    assert_eq!(Falcon512_128::K, 11);
    assert_eq!(Falcon1024_128::K, 11);
}

#[test]
fn falcon1024_three_slab_source_geometry() {
    macro_rules! check {
        ($profile:ty) => {
            for (batch, logs, widths, position_log, source_bits) in [
                (
                    1,
                    [10, 13, 13, 12],
                    [1, 4, 4, 2],
                    11,
                    [131_072, 1_048_576, 1_048_576, 524_288],
                ),
                (
                    3,
                    [12, 14, 13, 12],
                    [2, 8, 4, 2],
                    11,
                    [131_072, 524_288, 262_144, 131_072],
                ),
                (
                    32,
                    [15, 17, 16, 15],
                    [2, 8, 4, 2],
                    14,
                    [131_072, 524_288, 262_144, 131_072],
                ),
                (
                    1024,
                    [20, 22, 21, 20],
                    [2, 8, 4, 2],
                    19,
                    [131_072, 524_288, 262_144, 131_072],
                ),
            ] {
                let prepared = PreparedFalconHybrid::<$profile>::new(batch).unwrap();
                assert_eq!(prepared.pcs_source_packed_logs(), logs, "batch {batch}");
                assert_eq!(prepared.pcs_source_lane_widths(), widths, "batch {batch}");
                assert_eq!(prepared.pcs_position_log(), position_log, "batch {batch}");
                assert_eq!(prepared.pcs_virtual_lane_log(), 4);
                assert_eq!(
                    prepared.source_bits_per_signature(),
                    source_bits,
                    "batch {batch}"
                );
                assert!(prepared.security().algebraic_bits >= <$profile>::SECURITY_BITS as f64);
            }
        };
    }
    check!(Falcon1024_100);
    check!(Falcon1024_128);
}

#[test]
fn falcon512_keeps_single_permutation_slab_geometry() {
    macro_rules! check {
        ($profile:ty) => {
            for (batch, logs, widths, position_log) in [
                (1, [9, 12, 12, 12], [1, 4, 4, 4], 10),
                (3, [11, 14, 12, 12], [1, 8, 2, 2], 11),
                (32, [14, 17, 15, 14], [1, 8, 2, 1], 14),
                (1024, [19, 22, 20, 19], [1, 8, 2, 1], 19),
            ] {
                let prepared = PreparedFalconHybrid::<$profile>::new(batch).unwrap();
                assert_eq!(prepared.pcs_source_packed_logs(), logs, "batch {batch}");
                assert_eq!(prepared.pcs_source_lane_widths(), widths, "batch {batch}");
                assert_eq!(prepared.pcs_position_log(), position_log, "batch {batch}");
                assert_eq!(prepared.pcs_virtual_lane_log(), 4);
                assert!(prepared.security().algebraic_bits >= <$profile>::SECURITY_BITS as f64);
            }
        };
    }
    check!(Falcon512_100);
    check!(Falcon512_128);
}

#[test]
fn manual_profiles_cannot_bypass_validation() {
    macro_rules! invalid {
        ($name:ident,$n:expr,$k:expr,$bits:expr,$max:expr) => {
            struct $name;
            impl FalconProfile for $name {
                const N: usize = $n;
                const K: usize = $k;
                const SECURITY_BITS: usize = $bits;
                const MAX_BATCH: usize = $max;
                type Backend = BackendSelection<512, 9>;
            }
            assert!(PreparedFalconHybrid::<$name>::new(1).is_err());
        };
    }
    invalid!(WrongDegree, 1024, 9, 100, 1024);
    invalid!(WrongField, 512, 11, 100, 1024);
    invalid!(InsufficientField, 512, 9, 128, 1024);
    invalid!(WrongSecurity, 512, 9, 127, 1024);
    invalid!(OversizedBatch, 512, 9, 100, 1025);
    invalid!(EmptyBatch, 512, 9, 100, 0);
}

macro_rules! proof_cases {
    ($name:ident,$n:expr,$bits:expr,$selection:ident $(($k:expr))?) => {
        mod $name {
            use super::*;
            bitz::falcon_profile! { pub Profile {
                n:$n, security_bits:$bits, max_batch:1024,
                ring_extension:$selection $(($k))?,
            } }
            fn roundtrip(batch: usize) -> Result<(), Box<dyn Error>> {
                let cases = inputs::generate_cases($n, batch, 73)?;
                let public = FalconPublicStatement::<Profile>::from_bytes(
                    &cases.iter().map(|c| c.public_key.as_slice()).collect::<Vec<_>>(),
                    &cases.iter().map(|c| c.message.as_slice()).collect::<Vec<_>>(),
                    &cases.iter().map(|c| c.signature.as_slice()).collect::<Vec<_>>(),
                )?;
                let prepared = PreparedFalconHybrid::<Profile>::new(batch)?;
                assert!(prepared.security().algebraic_bits >= $bits as f64);
                let committed = prepared.commit(public)?;
                let statement = committed.statement.clone();
                let proof = prepared.prove(committed)?;
                prepared.verify(&statement, &proof)?;
                assert_eq!(proof.payload_size_breakdown().iter().map(|(_, n)| n).sum::<usize>(), proof.payload_size_bytes());
                let mut changed = statement.clone();
                changed.public.messages[0][0] ^= 1;
                assert!(prepared.verify(&changed, &proof).is_err());
                let mut changed = statement.clone();
                changed.public.signatures[0].s2[0] += 1;
                assert!(prepared.verify(&changed, &proof).is_err());
                let mut changed = statement.clone();
                changed.source_root[0] ^= 1;
                assert!(prepared.verify(&changed, &proof).is_err());
                let other_max = if batch < 1024 { 1023 } else { 1024 };
                if other_max != Profile::MAX_BATCH {
                    let different_max = <Profile as FalconProfile>::Backend::prepare(batch, $bits, other_max)?;
                    assert!(different_max.verify(&statement, &proof).is_err());
                }
                Ok(())
            }
            #[test]
            fn small_batches() -> Result<(), Box<dyn Error>> {
                for batch in [1, 3] { roundtrip(batch)?; }
                Ok(())
            }
            #[test]
            #[ignore = "complete correctness matrix; run in release mode serially"]
            fn qualification_batches() -> Result<(), Box<dyn Error>> {
                for batch in [2, 7, 8, 9, 1024] { roundtrip(batch)?; }
                Ok(())
            }
        }
    };
}
proof_cases!(falcon512_100, 512, 100, Auto);
proof_cases!(falcon1024_100, 1024, 100, Auto);
proof_cases!(falcon512_128, 512, 128, Auto);
proof_cases!(falcon1024_128, 1024, 128, Auto);
proof_cases!(falcon512_explicit10, 512, 100, Explicit(10));
proof_cases!(falcon1024_explicit10, 1024, 100, Explicit(10));
proof_cases!(falcon512_explicit11, 512, 100, Explicit(11));
proof_cases!(falcon1024_explicit11, 1024, 100, Explicit(11));

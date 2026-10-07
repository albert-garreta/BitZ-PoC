//! Run with `cargo bench --features falcon-hybrid --bench falcon_profiles`.
//! BITZ_FALCON_PROFILE_BATCH controls the live batch (default 1).
//! Prints CSV for both degrees/protocols: Auto at 100/128 and Explicit(11) at 100.
#[path = "common/falcon_profile_inputs.rs"]
mod inputs;
use bitz::piop::spartan::falcon_profiles::*;
use std::{error::Error, time::Instant};
macro_rules! run {
    ($n:expr,$bits:expr,$protocol:ident,$selection:ident $(($k:expr))?,$batch:expr,$cases:expr) => {{
        bitz::falcon_profile! {pub Profile {n:$n,security_bits:$bits,max_batch:1024,protocol:$protocol,ring_extension:$selection $(($k))?,}}
        let cases=$cases;
        let public=FalconPublicStatement::<Profile>::from_bytes(
            &cases.iter().map(|c|c.0.as_slice()).collect::<Vec<_>>(),
            &cases.iter().map(|c|c.1.as_slice()).collect::<Vec<_>>(),
            &cases.iter().map(|c|c.2.as_slice()).collect::<Vec<_>>())?;
        let prepared=PreparedFalconHybrid::<Profile>::new($batch)?;
        let start=Instant::now();let committed=prepared.commit(public)?;let commit=start.elapsed();
        let statement=committed.statement.clone();let start=Instant::now();let proof=prepared.prove(committed)?;let prove=start.elapsed();
        let start=Instant::now();prepared.verify(&statement,&proof)?;let verify=start.elapsed();
        println!("{},{},{},{:?},{},{},{:.6},{:.6},{:.6},{:.6},{}",$n,Profile::K,$bits,Profile::PROTOCOL,stringify!($selection),$batch,
            prepared.security().algebraic_bits,commit.as_secs_f64(),prove.as_secs_f64(),verify.as_secs_f64(),proof.payload_size_bytes());
    }};
}
fn main() -> Result<(), Box<dyn Error>> {
    let batch = std::env::var("BITZ_FALCON_PROFILE_BATCH")
        .unwrap_or_else(|_| "1".into())
        .parse::<usize>()?;
    if !(1..=1024).contains(&batch) {
        return Err("batch must be 1..=1024".into());
    }
    println!(
        "n,k,security_target,protocol,selection,batch,security_estimate,commit_s,prove_s,verify_s,proof_bytes"
    );
    let cases = inputs::upstream(512, batch);
    run!(512, 100, NativeCarry, Auto, batch, &cases);
    run!(512, 128, NativeCarry, Auto, batch, &cases);
    run!(512, 100, NativeCarry, Explicit(11), batch, &cases);
    run!(512, 100, SharedPrime, Auto, batch, &cases);
    run!(512, 128, SharedPrime, Auto, batch, &cases);
    run!(512, 100, SharedPrime, Explicit(11), batch, &cases);
    let cases = inputs::upstream(1024, batch);
    run!(1024, 100, NativeCarry, Auto, batch, &cases);
    run!(1024, 128, NativeCarry, Auto, batch, &cases);
    run!(1024, 100, NativeCarry, Explicit(11), batch, &cases);
    run!(1024, 100, SharedPrime, Auto, batch, &cases);
    run!(1024, 128, SharedPrime, Auto, batch, &cases);
    run!(1024, 100, SharedPrime, Explicit(11), batch, &cases);
    Ok(())
}

//! Writes the paper's MultiSwap Mod-R1CS instance (`k = 0`, 6,209 live
//! rows, from the crate's Limber port) in the instance-file format the
//! `mod-r1cs:<file>` circuit reads on both sides.
//!
//! `bitz_modr1cs_export <file> [--mini]` (`--mini`: the port's reduced
//! shape, which the oracle's generator can dump)
use f2z::bitz::modr1cs::Instance;
use num_traits::Zero;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let path = args.first().expect("an output path").clone();
    let instance = if args.iter().any(|a| a == "--mini") {
        Instance::multiswap_mini().expect("the mini MultiSwap instance")
    } else {
        Instance::multiswap().expect("the MultiSwap instance")
    };
    let bytes = instance.to_bytes();
    std::fs::write(&path, &bytes).expect("write");
    let modular = instance.rows.iter().filter(|row| !row.modulus.is_zero()).count();
    println!(
        "wrote {path}: {} rows ({modular} modular), {} columns ({} bits), {} bytes, digest {}",
        instance.rows.len(),
        instance.widths.len(),
        instance.widths.iter().filter(|w| **w == 1).count(),
        bytes.len(),
        instance.digest().iter().map(|b| format!("{b:02x}")).collect::<String>()
    );
}

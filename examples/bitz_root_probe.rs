//! BitZ parity probe (our side): rebuild the per-column bit rows from a packed
//! witness dumped by f2z-benchmark's `dump_commit` example, commit with the
//! SAME Ligerito shape (upstream `fast` TOML → initial_k / rate, Blake3 Merkle),
//! and print the root. Equal roots ⇒ identical packing + identical flock commit.
use f2z::ligerito_flock::commit_rs_ligerito_rows;
use f2z::pcs::IntegerMatrixLayout;
use flock_core::pcs::ligerito::LigeritoSecurityConfig;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let t: usize = args.get(1).and_then(|v| v.parse().ok()).unwrap_or(7);
    let s: usize = args.get(2).and_then(|v| v.parse().ok()).unwrap_or(15);
    let witness = args.get(3).cloned().unwrap_or_else(|| "witness.bin".to_string());
    let toml = args.get(4).cloned().expect("path to the Ligerito security TOML");

    let bytes = std::fs::read(&witness).expect("read witness");
    let packed: Vec<(u64, u64)> = bytes
        .chunks_exact(16)
        .map(|c| {
            (
                u64::from_le_bytes(c[..8].try_into().unwrap()),
                u64::from_le_bytes(c[8..].try_into().unwrap()),
            )
        })
        .collect();
    let p = IntegerMatrixLayout { row_vars: t, col_vars: s, word_bits: 1 };
    let hi_count = 1usize << (t - 7);
    assert_eq!(packed.len(), hi_count << s, "witness length vs shape");
    // Their layout: element c·2^{t−7} + i_hi holds bits (i_hi<<7 | v) of column c,
    // lo = v < 64. Our rows: row c, word 2·i_hi = lo, word 2·i_hi + 1 = hi.
    let rows: Vec<Vec<u64>> = (0..1usize << s)
        .map(|c| {
            let mut row = Vec::with_capacity(2 * hi_count);
            for i_hi in 0..hi_count {
                let (lo, hi) = packed[c * hi_count + i_hi];
                row.push(lo);
                row.push(hi);
            }
            row
        })
        .collect();

    let mut sec = LigeritoSecurityConfig::from_toml_str(&std::fs::read_to_string(&toml).expect("read toml"))
        .expect("parse toml");
    sec.hash = "blake3".into();
    let (pc, _vc) = sec.to_prover_verifier_configs().expect("configs");
    let hint = commit_rs_ligerito_rows(&p, rows, &pc);
    let hex: String = hint.root().iter().map(|b| format!("{b:02x}")).collect();
    println!(
        "our side:   t={t} s={s} m={} packed_len={} initial_k={} log_inv_rate={} hash={:?} root={hex}",
        t + s,
        packed.len(),
        pc.initial_k,
        pc.log_inv_rates[0],
        pc.merkle_hash
    );
}

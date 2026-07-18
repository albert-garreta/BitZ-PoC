//! Regenerate flock's embedded slim TOMLs at a chosen Johnson geometry:
//!
//! ```text
//! cargo run --release --example gen_lig_configs -- <log_inv_rate> <initial_k>
//! ```
//!
//! For every m = 22..=35, builds the config via
//! [`f2z::ligerito_flock::custom_johnson_config`] (flock's own
//! `paper_predicted_*` formulas, gated by
//! `LigeritoSecurityConfig::validate`) and overwrites
//! `crates/flock-core/configs/ligerito/m{m}_slim.toml` in the local flock
//! checkout (the path dependency in Cargo.toml). Rebuilding then embeds the
//! new TOMLs via flock-core's `include_str!`. Leaves flock's git alone.

use f2z::ligerito_flock::custom_johnson_config;

const FLOCK_CFG_DIR: &str =
    "/Users/albertgarretafontelles/flock/crates/flock-core/configs/ligerito";

fn main() {
    let mut args = std::env::args().skip(1);
    let r0: usize = args
        .next()
        .and_then(|x| x.parse().ok())
        .expect("usage: gen_lig_configs <log_inv_rate> <initial_k>");
    let k0: usize = args
        .next()
        .and_then(|x| x.parse().ok())
        .expect("usage: gen_lig_configs <log_inv_rate> <initial_k>");
    for m in 22usize..=35 {
        let cfg = custom_johnson_config(m, r0, k0);
        let toml = cfg.to_toml_string().expect("serialize");
        let path = format!("{FLOCK_CFG_DIR}/m{m}_slim.toml");
        std::fs::write(&path, &toml).expect("write slim toml");
        let l0 = &cfg.levels[0];
        println!(
            "m={m}: L0 rate 1/{} k={} queries {} (levels {:?}) -> {path}",
            1usize << l0.log_inv_rate,
            cfg.initial_k,
            l0.queries,
            cfg.levels.iter().map(|l| (l.log_inv_rate, l.queries)).collect::<Vec<_>>(),
        );
    }
}

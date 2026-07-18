//! Regenerate flock's embedded profile TOMLs at a chosen Johnson geometry:
//!
//! ```text
//! cargo run --release --example gen_lig_configs -- <log_inv_rate> <initial_k> <slim|fast>
//! ```
//!
//! For every m = 22..=35, builds the config via
//! [`f2z::ligerito_flock::custom_johnson_config`] (flock's own
//! `paper_predicted_*` formulas, gated by
//! `LigeritoSecurityConfig::validate`) and overwrites
//! `crates/flock-core/configs/ligerito/m{m}_{profile}.toml` in the local
//! flock checkout (the path dependency in Cargo.toml). Rebuilding then
//! embeds the new TOMLs via flock-core's `include_str!`. Leaves flock's
//! git alone. NOTE: generation uses the slim template's conventions
//! (johnson_ood, eta 0.02, 16-bit query grinding, 100-bit per-level
//! target) — regenerating `fast` moves it onto that convention (the
//! original fast generation used no query grinding).

use f2z::ligerito_flock::custom_johnson_config;

const FLOCK_CFG_DIR: &str =
    "/Users/albertgarretafontelles/flock/crates/flock-core/configs/ligerito";

fn main() {
    let mut args = std::env::args().skip(1);
    let r0: usize = args
        .next()
        .and_then(|x| x.parse().ok())
        .expect("usage: gen_lig_configs <log_inv_rate> <initial_k> <slim|fast>");
    let k0: usize = args
        .next()
        .and_then(|x| x.parse().ok())
        .expect("usage: gen_lig_configs <log_inv_rate> <initial_k> <slim|fast>");
    let profile = args.next().expect("usage: gen_lig_configs <log_inv_rate> <initial_k> <slim|fast>");
    assert!(
        profile == "slim" || profile == "fast",
        "profile must be slim or fast (secure is not generated here)"
    );
    for m in 22usize..=35 {
        let cfg = custom_johnson_config(m, r0, k0);
        let toml = cfg.to_toml_string().expect("serialize");
        let path = format!("{FLOCK_CFG_DIR}/m{m}_{profile}.toml");
        std::fs::write(&path, &toml).expect("write profile toml");
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

//! Reproducible `(t,s)` sweep for `2^14` SHA-256 compressions.
//!
//! The total product-assignment width is fixed at 29 variables. By default
//! this runs every supported one-chunk split that stays below the memory cap,
//! `t=7..=27`, with one warmup and 21 measured samples per split. `t=28` is
//! reported as skipped because its projected peak exceeds 60 GiB. Select a subset with
//! `F2Z_SHA_PRODUCT_TS="13 17"` and override samples with
//! `F2Z_BENCH_REPS=3`.

#[path = "sha256_compressions.rs"]
mod sha256_compressions;

fn main() {
    let default_sweep = std::env::var_os("F2Z_SHA_PRODUCT_TS").is_none();
    if default_sweep {
        // SAFETY: this is the first action, before the harness starts threads.
        unsafe {
            std::env::set_var(
                "F2Z_SHA_PRODUCT_TS",
                "7 8 9 10 11 12 13 14 15 16 17 18 19 20 21 22 23 24 25 26 27",
            )
        };
    }
    if std::env::var_os("F2Z_BENCH_REPS").is_none() {
        // SAFETY: this is the first action, before the harness starts threads.
        unsafe { std::env::set_var("F2Z_BENCH_REPS", "21") };
    }
    sha256_compressions::main();

    if default_sweep && let Some(path) = std::env::var_os("F2Z_SHA_RESULT_PATH") {
        use std::io::Write;

        let mut output = std::fs::OpenOptions::new()
            .append(true)
            .open(path)
            .expect("reopen SHA result output");
        for t in 1..=6 {
            writeln!(
                output,
                "STATUS product_t={t} product_s={} compressions=16384 status=unsupported reason=t_below_log_packing_7",
                29 - t,
            )
            .expect("write unsupported SHA split");
        }
        writeln!(
            output,
            "STATUS product_t=28 product_s=1 compressions=16384 status=skipped reason=projected_peak_exceeds_60_gib projected_peak_bytes=75150743216 peak_cap_bytes=64424509440"
        )
        .expect("write memory-skipped SHA split");
    }
}

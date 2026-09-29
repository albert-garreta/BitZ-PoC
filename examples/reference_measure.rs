//! Small full-proof BitZ measurements, with commitment and claim preparation
//! outside the opening timer. Run with --release --features span-metrics.
use bitz::{
    pcs::IntegerMatrixLayout,
    piop::spartan::protocol::bitz_opener::{
        BitZLigerito, BitZOpener, prove_standalone, standalone_evaluation, verify_standalone,
    },
};

fn median(mut samples: Vec<f64>) -> f64 {
    samples.sort_by(f64::total_cmp);
    samples[samples.len() / 2]
}

fn measure(t: usize, s: usize) -> Result<(), Box<dyn std::error::Error>> {
    let layout = IntegerMatrixLayout {
        row_vars: t,
        col_vars: s,
    };
    let opener = BitZOpener::new(
        layout,
        BitZLigerito::Selected(bitz::ligerito_flock::LigeritoSelection::JOHNSON),
        100,
    )?;
    let rows = (0..layout.cols())
        .map(|column| {
            let mut words = vec![0; layout.rows().div_ceil(64)];
            for row in 0..layout.rows() {
                if layout
                    .cell_index(row, column)
                    .wrapping_mul(0x9E37_79B9_7F4A_7C15)
                    & 1
                    != 0
                {
                    words[row / 64] |= 1 << (row % 64);
                }
            }
            words
        })
        .collect();
    let hint = opener.commit(rows)?;
    let claim = standalone_evaluation(&opener, &hint)?;
    let mut prover = Vec::new();
    let mut verifier = Vec::new();
    let mut bytes = 0;
    for sample in 0..6 {
        let (proof, elapsed) =
            bitz::observability::measure(tracing::info_span!("reference_measure:proof"), || {
                prove_standalone(&opener, &hint, claim)
            })
            .expect("complete measurement");
        let proof = proof?;
        let (result, verified) =
            bitz::observability::measure(tracing::info_span!("reference_measure:verify"), || {
                verify_standalone(&opener, &hint.commitment, claim, &proof)
            })
            .expect("complete measurement");
        result?;
        bytes = proof.to_bytes().len();
        if sample > 0 {
            prover.push(elapsed.as_secs_f64() * 1000.);
            verifier.push(verified.as_secs_f64() * 1000.);
        }
    }
    println!(
        "t={t} s={s}: prover {:.3} ms, verifier {:.3} ms, serialized proof {bytes} B",
        median(prover),
        median(verifier)
    );
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    bitz::observability::install()?;
    for (t, s) in [(10, 6), (12, 6), (11, 11)] {
        measure(t, s)?;
    }
    Ok(())
}

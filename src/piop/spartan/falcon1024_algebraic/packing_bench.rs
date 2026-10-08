//! Constructor measurements on a checked, prederived witness, outside proving.
use super::{FalconAlgebraicStatement, FalconError, Layout, N, Source, WitnessData};
use std::{hint::black_box, time::Instant};

fn parallel() -> Result<bool, FalconError> {
    let requested = match std::env::var("BITZ_FALCON_PACKING") {
        Ok(value) => match value.as_str() {
            "serial-word" => false,
            "parallel-word" => true,
            _ => return Err(super::error("packing must be serial-word or parallel-word")),
        },
        Err(std::env::VarError::NotPresent) => false,
        Err(e) => return Err(super::error(e)),
    };
    #[cfg(feature = "parallel")]
    let workers = rayon::current_num_threads();
    #[cfg(not(feature = "parallel"))]
    let workers = 1;
    Ok(requested && workers > 1)
}

pub(super) fn constructor() -> Result<fn(Layout, &WitnessData) -> Source, FalconError> {
    Ok(if parallel()? {
        Source::new_parallel
    } else {
        Source::new
    })
}

pub fn selected(batch: usize) -> Result<&'static str, FalconError> {
    Layout::new(batch)?;
    Ok(if parallel()? {
        "parallel-word"
    } else {
        "serial-word"
    })
}

pub fn measure(
    public: &FalconAlgebraicStatement,
    s2: Vec<[i16; N]>,
    repetitions: usize,
) -> Result<(f64, String), FalconError> {
    if repetitions == 0 {
        return Err(super::error("packing repetitions must be positive"));
    }
    let layout = Layout::new(s2.len())?;
    let data = WitnessData::from_s2(public, s2)?;
    let pack = constructor()?;
    let mut elapsed = std::time::Duration::ZERO;
    let mut last = None;
    for _ in 0..repetitions {
        drop(last.take());
        let start = Instant::now();
        let source = pack(black_box(layout), black_box(&data));
        elapsed += start.elapsed();
        last = Some(black_box(source));
    }
    let mut digest = blake3::Hasher::new();
    for word in last.unwrap().rows().iter().flatten() {
        digest.update(&word.to_le_bytes());
    }
    Ok((
        elapsed.as_secs_f64() * 1000.0 / repetitions as f64,
        digest.finalize().to_hex().to_string(),
    ))
}

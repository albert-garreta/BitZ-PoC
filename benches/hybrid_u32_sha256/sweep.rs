//! Run each workload/backend in a separate process to isolate peak RSS.
use std::{
    fs::{self, File},
    io::{BufWriter, Write},
    path::PathBuf,
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

use super::AnyError;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Shape {
    mul_log: u32,
    sha_log: u32,
}

pub fn equal_witness_shapes() -> Vec<Shape> {
    (15..=20)
        .map(|mul_log| Shape {
            mul_log,
            sha_log: mul_log - 8,
        })
        .collect()
}

pub fn parse_shapes(value: &str) -> Result<Vec<Shape>, AnyError> {
    let mut shapes = Vec::new();
    for pair in value.split(',') {
        let (mul, sha) = pair
            .trim()
            .split_once(':')
            .ok_or("--shapes expects MUL_LOG:SHA_LOG pairs, e.g. 15:7,16:8")?;
        let shape = Shape {
            mul_log: mul.trim().parse()?,
            sha_log: sha.trim().parse()?,
        };
        if !(15..=20).contains(&shape.mul_log) || !(1..=16).contains(&shape.sha_log) {
            return Err("--shapes requires multiplication logs 15..20 and SHA logs 1..16".into());
        }
        if shapes.contains(&shape) {
            return Err(format!("duplicate --shapes pair {pair}").into());
        }
        shapes.push(shape);
    }
    Ok(shapes)
}

// Keep the standalone runner's CSVs intact. The combined CSV distinguishes
// exact serialized proof sizes from the separate mode's payload estimate.
const METRICS: [&str; 16] = [
    "iteration",
    "setup_ms",
    "witness_commit_ms",
    "continuation_ms",
    "total_prover_ms",
    "verify_ms",
    "proof_bytes",
    "proof_payload_bytes_estimate",
    "peak_rss_kib",
    "piop_ms",
    "iop_ms",
    "mul_piop_ms",
    "sha_piop_ms",
    "mul_opening_ms",
    "joint_sumcheck_ms",
    "shared_opening_ms",
];

fn grouped_count(value: usize) -> String {
    let digits = value.to_string();
    let mut result = String::new();
    for (index, digit) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index).is_multiple_of(3) {
            result.push(',');
        }
        result.push(digit);
    }
    result
}

/// Produce a combined CSV record alongside a labelled terminal sample.
fn formatted_rows(
    csv: &str,
    mode: &str,
    shape: Shape,
    iterations: usize,
) -> Result<Vec<(String, String)>, AnyError> {
    let mut lines = csv.lines();
    let header: Vec<_> = lines
        .next()
        .ok_or("missing child CSV header")?
        .split(',')
        .collect();
    let expected = match mode {
        "hybrid" => {
            "mode,iteration,setup_ms,witness_commit_ms,continuation_ms,total_prover_ms,verify_ms,proof_bytes,peak_rss_kib,piop_ms,iop_ms,mul_piop_ms,sha_piop_ms,mul_opening_ms,joint_sumcheck_ms,shared_opening_ms"
        }
        "separate" => {
            "mode,iteration,setup_ms,total_prover_ms,verify_ms,proof_payload_bytes_estimate,peak_rss_kib"
        }
        _ => "mode,iteration,setup_ms,total_prover_ms,verify_ms,proof_bytes,peak_rss_kib",
    };
    if header.join(",") != expected {
        return Err(format!("unexpected {mode} child CSV header").into());
    }
    let mut rows = Vec::new();
    for (iteration, line) in lines.enumerate() {
        let values: Vec<_> = line.split(',').collect();
        if values.len() != header.len()
            || values[0] != mode
            || values[1].parse::<usize>()? != iteration
        {
            return Err(format!("invalid {mode} child CSV row {iteration}").into());
        }
        let mut row = format!(
            "{mode},u32_mod_2_32,{},{},{},{}",
            shape.mul_log,
            shape.sha_log,
            1usize << shape.mul_log,
            1usize << shape.sha_log,
        );
        for metric in &METRICS {
            row.push(',');
            if let Some(index) = header.iter().position(|name| name == metric) {
                row.push_str(values[index]);
            }
        }
        let metric = |name: &str| -> Result<&str, AnyError> {
            header
                .iter()
                .position(|key| *key == name)
                .map(|index| values[index])
                .ok_or_else(|| format!("missing {mode} metric {name}").into())
        };
        let source = format!("{mode} [mul 2^{}, SHA 2^{}]", shape.mul_log, shape.sha_log);
        let mut display = String::new();
        if iteration == 0 {
            display.push_str(&format!(
                "{source}: setup {} ms (once, excluded from prover time)\n",
                metric("setup_ms")?
            ));
        }
        display.push_str(&format!(
            "{source}: sample {}/{} — prover {} ms, verifier {} ms — VERIFIED\n",
            iteration + 1,
            iterations,
            metric("total_prover_ms")?,
            metric("verify_ms")?,
        ));
        if mode == "hybrid" {
            display.push_str(&format!(
                "  Witness + commitments: {} ms; PIOP + IOP continuation: {} ms\n",
                metric("witness_commit_ms")?,
                metric("continuation_ms")?,
            ));
            display.push_str(&format!(
                "  PIOP: {} ms (multiplication / Spartan: {} ms; SHA: {} ms)\n  IOP / PCS opening: {} ms (multiplication F2Z/GKR: {} ms; joint sumcheck: {} ms; ring switching + Ligerito: {} ms)\n",
                metric("piop_ms")?,
                metric("mul_piop_ms")?,
                metric("sha_piop_ms")?,
                metric("iop_ms")?,
                metric("mul_opening_ms")?,
                metric("joint_sumcheck_ms")?,
                metric("shared_opening_ms")?,
            ));
        }
        let (size_column, size_label) = if mode == "separate" {
            ("proof_payload_bytes_estimate", "Proof payload estimate")
        } else {
            ("proof_bytes", "Proof")
        };
        let bytes: usize = metric(size_column)?.parse()?;
        let peak_kib: u64 = metric("peak_rss_kib")?.parse()?;
        let peak = if peak_kib == 0 {
            "unavailable".to_string()
        } else {
            format!("{:.2} MiB", peak_kib as f64 / 1024.0)
        };
        display.push_str(&format!(
            "  {size_label}: {} B ({:.2} KiB); peak RSS: {peak}",
            grouped_count(bytes),
            bytes as f64 / 1024.0,
        ));
        rows.push((row, display));
    }
    if rows.len() != iterations {
        return Err(format!("expected {iterations} verified rows, got {}", rows.len()).into());
    }
    Ok(rows)
}

pub fn run(
    shapes: Vec<Shape>,
    mode: &str,
    iterations: usize,
    results_dir: Option<PathBuf>,
) -> Result<(), AnyError> {
    let modes = match mode {
        "all" => vec!["hybrid", "separate", "all-binius"],
        "hybrid" | "separate" | "all-binius" => vec![mode],
        _ => {
            return Err(
                "invalid --mode; use hybrid, separate, all-binius or all for a sweep".into(),
            );
        }
    };
    let executable = std::env::current_exe()?;
    let results_dir = if let Some(directory) = results_dir {
        directory
    } else {
        let stamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("benches/results/hybrid-u32-sha256")
            .join(format!("sweep-{stamp}-{}", std::process::id()))
    };
    if let Some(parent) = results_dir.parent().filter(|p| !p.as_os_str().is_empty()) {
        fs::create_dir_all(parent)?;
    }
    fs::create_dir(&results_dir).map_err(|error| {
        format!(
            "cannot create new results directory {}: {error}",
            results_dir.display()
        )
    })?;
    let results_dir = results_dir.canonicalize()?;
    fs::write(
        results_dir.join("run.txt"),
        format!(
            "executable={}\nprotocol=hybrid-u32-mod32-sha256-v2\nmultiplication_relation=xy=z+2^32*w (x,y,z,w are u32)\nshapes={shapes:?}\nmodes={modes:?}\niterations={iterations}\nRAYON_NUM_THREADS={}\nnon_zk=true\nsecurity_target_bits=100\n",
            executable.display(),
            std::env::var("RAYON_NUM_THREADS").unwrap_or_else(|_| "default".into()),
        ),
    )?;
    let mut summary = BufWriter::new(File::create(results_dir.join("summary.csv"))?);
    let header = format!(
        "mode,multiplication_relation,mul_log,sha_log,multiplications,sha_compressions,{}",
        METRICS.join(",")
    );
    writeln!(summary, "{header}")?;
    summary.flush()?;
    println!("SHA-256 chain + multiplication modulo 2^32 | non-ZK | 100-bit security target");
    println!(
        "Prover time includes witness generation, commitments and proof encoding; setup is separate."
    );
    println!("Peak RSS includes setup and is cumulative within each workload process.");
    println!("CSV results and detailed logs: {}", results_dir.display());
    let total = shapes.len() * modes.len();
    let mut completed = 0;
    for shape in shapes {
        for mode in &modes {
            let stem = format!("{mode}-m{}-s{}", shape.mul_log, shape.sha_log);
            let csv_path = results_dir.join(format!("{stem}.csv"));
            let log_path = results_dir.join(format!("{stem}.log"));
            let backend = match *mode {
                "hybrid" => "BitZ multiplication + Binius SHA, shared opening",
                "separate" => "BitZ multiplication + Binius SHA, separate proofs",
                _ => "Binius multiplication + SHA",
            };
            println!(
                "\n[{}/{total}] {mode} — {backend}\n  {} modular multiplications + {} chained SHA-256 compressions ({iterations} samples)",
                completed + 1,
                grouped_count(1usize << shape.mul_log),
                grouped_count(1usize << shape.sha_log),
            );
            std::io::stdout().flush()?;
            let status = Command::new(&executable)
                .args([
                    "--mode",
                    mode,
                    "--mul-log",
                    &shape.mul_log.to_string(),
                    "--sha-log",
                    &shape.sha_log.to_string(),
                    "--iterations",
                    &iterations.to_string(),
                ])
                .stdout(File::create(&csv_path)?)
                .stderr(File::create(&log_path)?)
                .status()?;
            if !status.success() {
                return Err(format!(
                    "{stem} failed ({status}); see {}. Completed results are preserved in {}",
                    log_path.display(),
                    results_dir.display(),
                )
                .into());
            }
            for (row, display) in
                formatted_rows(&fs::read_to_string(&csv_path)?, mode, shape, iterations)?
            {
                writeln!(summary, "{row}")?;
                println!("{display}");
            }
            summary.flush()?;
            completed += 1;
        }
    }
    println!(
        "\nCompleted {completed} workloads; all {} samples verified. CSV summary: {}",
        completed * iterations,
        results_dir.join("summary.csv").display()
    );
    Ok(())
}

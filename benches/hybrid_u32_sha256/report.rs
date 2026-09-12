//! Flat CSV projections shared by the child runner and the sweep validator.
use serde::Serialize;

#[derive(Serialize)]
pub(super) struct HybridRow<'a> {
    pub mode: &'a str,
    pub iteration: usize,
    #[serde(serialize_with = "super::output::csv_format::three_decimals")]
    pub setup_ms: f64,
    #[serde(serialize_with = "super::output::csv_format::three_decimals")]
    pub witness_ms: f64,
    #[serde(serialize_with = "super::output::csv_format::three_decimals")]
    pub witness_commit_ms: f64,
    #[serde(serialize_with = "super::output::csv_format::three_decimals")]
    pub continuation_ms: f64,
    #[serde(serialize_with = "super::output::csv_format::three_decimals")]
    pub total_prover_ms: f64,
    #[serde(serialize_with = "super::output::csv_format::three_decimals")]
    pub verify_ms: f64,
    pub proof_bytes: usize,
    pub peak_rss_kib: u64,
    #[serde(serialize_with = "super::output::csv_format::three_decimals")]
    pub piop_ms: f64,
    #[serde(serialize_with = "super::output::csv_format::three_decimals")]
    pub iop_ms: f64,
    #[serde(serialize_with = "super::output::csv_format::three_decimals")]
    pub mul_piop_ms: f64,
    #[serde(serialize_with = "super::output::csv_format::three_decimals")]
    pub sha_piop_ms: f64,
    #[serde(serialize_with = "super::output::csv_format::three_decimals")]
    pub mul_opening_ms: f64,
    #[serde(serialize_with = "super::output::csv_format::three_decimals")]
    pub joint_sumcheck_ms: f64,
    #[serde(serialize_with = "super::output::csv_format::three_decimals")]
    pub shared_opening_ms: f64,
    #[serde(serialize_with = "super::output::csv_format::three_decimals")]
    pub ood_round_ms: f64,
}
impl HybridRow<'_> {
    pub const HEADER: [&'static str; 18] = [
        "mode",
        "iteration",
        "setup_ms",
        "witness_ms",
        "witness_commit_ms",
        "continuation_ms",
        "total_prover_ms",
        "verify_ms",
        "proof_bytes",
        "peak_rss_kib",
        "piop_ms",
        "iop_ms",
        "mul_piop_ms",
        "sha_piop_ms",
        "mul_opening_ms",
        "joint_sumcheck_ms",
        "shared_opening_ms",
        "ood_round_ms",
    ];
}

#[derive(Serialize)]
pub(super) struct NativeRow<'a> {
    pub mode: &'a str,
    pub iteration: usize,
    #[serde(serialize_with = "super::output::csv_format::three_decimals")]
    pub setup_ms: f64,
    #[serde(serialize_with = "super::output::csv_format::three_decimals")]
    pub witness_ms: f64,
    #[serde(serialize_with = "super::output::csv_format::three_decimals")]
    pub total_prover_ms: f64,
    #[serde(serialize_with = "super::output::csv_format::three_decimals")]
    pub verify_ms: f64,
    pub proof_bytes: usize,
    pub peak_rss_kib: u64,
}
impl NativeRow<'_> {
    pub fn header(mode: &str) -> [&'static str; 8] {
        let mut header = [
            "mode",
            "iteration",
            "setup_ms",
            "witness_ms",
            "total_prover_ms",
            "verify_ms",
            "proof_bytes",
            "peak_rss_kib",
        ];
        if mode == "separate" {
            header[6] = "proof_payload_bytes_estimate";
        }
        header
    }
}

#[derive(Default, Serialize, serde::Deserialize)]
#[serde(default)]
pub(super) struct SummaryRow {
    pub mode: String,
    pub multiplication_relation: String,
    pub mul_log: u32,
    pub sha_log: u32,
    pub multiplications: usize,
    pub sha_compressions: usize,
    // Retain child numeric lexemes (including decimal precision); absent metrics stay empty.
    pub iteration: String,
    pub setup_ms: String,
    pub witness_ms: String,
    pub witness_commit_ms: String,
    pub continuation_ms: String,
    pub total_prover_ms: String,
    pub verify_ms: String,
    pub proof_bytes: String,
    pub proof_payload_bytes_estimate: String,
    pub peak_rss_kib: String,
    pub piop_ms: String,
    pub iop_ms: String,
    pub mul_piop_ms: String,
    pub sha_piop_ms: String,
    pub mul_opening_ms: String,
    pub joint_sumcheck_ms: String,
    pub shared_opening_ms: String,
    pub ood_round_ms: String,
    pub ligerito_hex: String,
}
impl SummaryRow {
    pub const HEADER: [&'static str; 25] = [
        "mode",
        "multiplication_relation",
        "mul_log",
        "sha_log",
        "multiplications",
        "sha_compressions",
        "iteration",
        "setup_ms",
        "witness_ms",
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
        "ood_round_ms",
        "ligerito_hex",
    ];
}

#[cfg(test)]
mod reporting_tests {
    use super::super::output;
    use super::*;

    #[test]
    fn csv_contract_hybrid_and_native_modes() {
        let mut csv = output::csv_writer(Vec::new());
        csv.write_record(HybridRow::HEADER).unwrap();
        csv.flush().unwrap();
        let header = "mode,iteration,setup_ms,witness_ms,witness_commit_ms,continuation_ms,total_prover_ms,verify_ms,proof_bytes,peak_rss_kib,piop_ms,iop_ms,mul_piop_ms,sha_piop_ms,mul_opening_ms,joint_sumcheck_ms,shared_opening_ms,ood_round_ms\n";
        assert_eq!(csv.get_ref(), header.as_bytes());
        csv.serialize(HybridRow {
            mode: "hybrid",
            iteration: 0,
            setup_ms: 1.2346,
            witness_ms: 2.0,
            witness_commit_ms: 3.0,
            continuation_ms: 4.0,
            total_prover_ms: 7.0,
            verify_ms: 5.0,
            proof_bytes: 1024,
            peak_rss_kib: 0,
            piop_ms: 6.0,
            iop_ms: 7.0,
            mul_piop_ms: 8.0,
            sha_piop_ms: 9.0,
            mul_opening_ms: 10.0,
            joint_sumcheck_ms: 11.0,
            shared_opening_ms: 12.0,
            ood_round_ms: 13.0,
        })
        .unwrap();
        assert_eq!(
            String::from_utf8(csv.into_inner().unwrap()).unwrap(),
            format!(
                "{header}hybrid,0,1.235,2.000,3.000,4.000,7.000,5.000,1024,0,6.000,7.000,8.000,9.000,10.000,11.000,12.000,13.000\n"
            )
        );
        for mode in ["all-binius", "binius-ligerito", "separate"] {
            let mut csv = output::csv_writer(Vec::new());
            csv.write_record(NativeRow::header(mode)).unwrap();
            let size = if mode == "separate" {
                "proof_payload_bytes_estimate"
            } else {
                "proof_bytes"
            };
            let header = format!(
                "mode,iteration,setup_ms,witness_ms,total_prover_ms,verify_ms,{size},peak_rss_kib\n"
            );
            csv.flush().unwrap();
            assert_eq!(csv.get_ref(), header.as_bytes());
            csv.serialize(NativeRow {
                mode,
                iteration: 0,
                setup_ms: 1.2346,
                witness_ms: 2.0,
                total_prover_ms: 3.0,
                verify_ms: 4.0,
                proof_bytes: 1024,
                peak_rss_kib: 0,
            })
            .unwrap();
            assert_eq!(
                String::from_utf8(csv.into_inner().unwrap()).unwrap(),
                format!("{header}{mode},0,1.235,2.000,3.000,4.000,1024,0\n")
            );
        }
    }
}

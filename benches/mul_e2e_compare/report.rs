//! Native multiplication reporting records; JSON is only an output representation.
use serde::Serialize;
use serde_json::Value;

#[derive(Clone, Copy, Debug, Serialize)]
pub(super) struct Metrics {
    pub witness_ms: f64,
    pub commit_ms: f64,
    pub piop_ms: f64,
    pub opening_ms: f64,
    pub pcs_ms: f64,
    pub online_prover_ms: f64,
    pub witness_to_proof_ms: f64,
    pub post_proof_ms: f64,
    pub verify_ms: f64,
    pub verified_trial_ms: f64,
    pub proof_bytes: usize,
}

#[derive(Clone, Copy, Debug, Serialize)]
pub(super) struct Medians {
    pub witness_ms: f64,
    pub commit_ms: f64,
    pub piop_ms: f64,
    pub opening_ms: f64,
    pub pcs_ms: f64,
    pub online_prover_ms: f64,
    pub witness_to_proof_ms: f64,
    pub post_proof_ms: f64,
    pub verify_ms: f64,
    // Historical summary schema represents all medians, including bytes, as floats.
    pub proof_bytes: f64,
}

impl Medians {
    pub fn from_samples(samples: &[Metrics]) -> Self {
        let median = |get: fn(&Metrics) -> f64| {
            super::common::median(&samples.iter().map(get).collect::<Vec<_>>())
        };
        Self {
            witness_ms: median(|m| m.witness_ms),
            commit_ms: median(|m| m.commit_ms),
            piop_ms: median(|m| m.piop_ms),
            opening_ms: median(|m| m.opening_ms),
            pcs_ms: median(|m| m.pcs_ms),
            online_prover_ms: median(|m| m.online_prover_ms),
            witness_to_proof_ms: median(|m| m.witness_to_proof_ms),
            post_proof_ms: median(|m| m.post_proof_ms),
            verify_ms: median(|m| m.verify_ms),
            proof_bytes: median(|m| m.proof_bytes as f64),
        }
    }

    pub fn values(self) -> [f64; 10] {
        [
            self.witness_ms,
            self.commit_ms,
            self.piop_ms,
            self.opening_ms,
            self.pcs_ms,
            self.online_prover_ms,
            self.witness_to_proof_ms,
            self.post_proof_ms,
            self.verify_ms,
            self.proof_bytes,
        ]
    }
}

#[derive(Clone, Copy, Serialize)]
pub(super) struct Trial {
    pub kind: &'static str,
    pub index: usize,
}
impl Trial {
    pub fn new(index: usize) -> Self {
        if index == 0 {
            Self {
                kind: "warmup",
                index: 0,
            }
        } else {
            Self {
                kind: "sample",
                index: index - 1,
            }
        }
    }
}

#[derive(Serialize)]
pub(super) struct Sample<'a> {
    pub schema: &'static str,
    pub workload: &'static str,
    pub backend: &'a str,
    pub log_multiplications: usize,
    pub multiplications: usize,
    pub corpus_digest: &'a str,
    pub threads: usize,
    pub seed: u64,
    pub trial: Trial,
    pub setup_ms: f64,
    pub config: &'a Value,
    pub measurement_policy: &'static str,
    pub proof_verified: bool,
    pub metrics: &'a Metrics,
}

#[derive(Serialize)]
#[serde(untagged)]
pub(super) enum Summary<'a> {
    Measured(MeasuredSummary<'a>),
    Ineligible(Ineligible<'a>),
}

#[derive(Serialize)]
pub(super) struct MeasuredSummary<'a> {
    pub schema: &'static str,
    pub workload: &'static str,
    pub backend: &'a str,
    pub log_multiplications: usize,
    pub multiplications: usize,
    pub samples: usize,
    pub warmups: usize,
    pub threads: usize,
    pub seed: u64,
    pub setup_ms: f64,
    pub config: Value,
    pub corpus_digest: String,
    pub measurement_policy: &'static str,
    pub proof_verified: bool,
    pub medians: Medians,
    pub peak_rss_bytes: Option<u64>,
    pub memory: Option<super::memory::Sample>,
}

#[derive(Serialize)]
pub(super) struct Ineligible<'a> {
    pub workload: &'static str,
    pub backend: &'a str,
    pub log_multiplications: usize,
    pub status: &'static str,
    pub reason: String,
}

#[derive(Serialize)]
pub(super) struct WitnessSample<'a> {
    pub schema: &'static str,
    pub workload: &'static str,
    pub backend: &'a str,
    pub log_multiplications: usize,
    pub threads: usize,
    pub seed: u64,
    pub trial: Trial,
    pub witness_generation_ms: f64,
    pub witness_digest_blake3: &'a str,
    pub expected_digest: &'a str,
    pub native_representation: &'static str,
    pub quotient_reconstructed: bool,
    pub all_rows_match: bool,
}

#[derive(Serialize)]
pub(super) struct WitnessSummary<'a> {
    pub workload: &'static str,
    pub backend: &'a str,
    pub log_multiplications: usize,
    pub samples: usize,
    pub witness_ms: f64,
    pub witness_digest_blake3: String,
    pub all_rows_match: bool,
}

#[cfg(test)]
mod reporting_tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn sample_jsonl_and_summary_have_the_historical_schema() {
        let metrics = Metrics {
            witness_ms: 1.0,
            commit_ms: 2.0,
            piop_ms: 3.0,
            opening_ms: 4.0,
            pcs_ms: 6.0,
            online_prover_ms: 9.0,
            witness_to_proof_ms: 10.0,
            post_proof_ms: 0.0,
            verify_ms: 5.0,
            verified_trial_ms: 15.0,
            proof_bytes: 1024,
        };
        let config = json!({"opaque":true});
        let sample = Sample {
            schema: "f2z-native-mul-sample/v1",
            workload: "u32",
            backend: "f2z",
            log_multiplications: 4,
            multiplications: 16,
            corpus_digest: "digest",
            threads: 1,
            seed: 7,
            trial: Trial::new(1),
            setup_ms: 8.0,
            config: &config,
            measurement_policy: "policy",
            proof_verified: true,
            metrics: &metrics,
        };
        let mut writer = super::super::common::output::JsonlWriter::new(Vec::new());
        writer.write(&sample).unwrap();
        let bytes = writer.finish().unwrap();
        assert_eq!(bytes.last(), Some(&b'\n'));
        assert_eq!(bytes.iter().filter(|&&b| b == b'\n').count(), 1);
        assert_eq!(
            serde_json::from_slice::<Value>(&bytes).unwrap(),
            json!({
                "schema":"f2z-native-mul-sample/v1","workload":"u32","backend":"f2z",
                "log_multiplications":4,"multiplications":16,"corpus_digest":"digest",
                "threads":1,"seed":7,"trial":{"kind":"sample","index":0},"setup_ms":8.0,
                "config":{"opaque":true},"measurement_policy":"policy","proof_verified":true,
                "metrics":{"witness_ms":1.0,"commit_ms":2.0,"piop_ms":3.0,"opening_ms":4.0,
                "pcs_ms":6.0,"online_prover_ms":9.0,"witness_to_proof_ms":10.0,"post_proof_ms":0.0,
                "verify_ms":5.0,"verified_trial_ms":15.0,"proof_bytes":1024}
            })
        );
        let summary = Summary::Measured(MeasuredSummary {
            schema: "summary",
            workload: "u32",
            backend: "f2z",
            log_multiplications: 4,
            multiplications: 16,
            samples: 1,
            warmups: 1,
            threads: 1,
            seed: 7,
            setup_ms: 8.0,
            config,
            corpus_digest: "digest".into(),
            measurement_policy: "policy",
            proof_verified: true,
            medians: Medians::from_samples(&[metrics]),
            peak_rss_bytes: None,
            memory: None,
        });
        let value = serde_json::to_value(summary).unwrap();
        assert!(value.get("memory").unwrap().is_null());
        assert!(value.get("peak_rss_bytes").unwrap().is_null());
        assert!(value["medians"]["proof_bytes"].is_f64());
        assert_eq!(
            serde_json::to_value(Summary::Ineligible(Ineligible {
                workload: "u32",
                backend: "f2z",
                log_multiplications: 4,
                status: "ineligible",
                reason: "budget".into()
            }))
            .unwrap(),
            json!({"workload":"u32","backend":"f2z","log_multiplications":4,
            "status":"ineligible","reason":"budget"})
        );
    }
}

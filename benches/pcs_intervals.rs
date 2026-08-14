//! Canonical `zkperf.trace/v1` capture for the opt-in PCS interval benchmark.
//!
//! This is deliberately a bench-only module: normal library and benchmark
//! builds do not link `tracing` or its subscriber. The recording layer follows
//! flock's `zkperf_sha256` bench so native flock `perf_span!` scopes and F2Z's
//! mirrored `utils::prof` scopes share one monotonic clock and parent tree.

use std::collections::HashMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use tracing::field::{Field, Visit};
use tracing::span::{Attributes, Id};
use tracing::{Subscriber, span};
use tracing_subscriber::layer::{Context, Layer, SubscriberExt};
use tracing_subscriber::registry::{LookupSpan, Registry};

#[derive(Default, Clone)]
struct Fields {
    strings: Vec<(&'static str, String)>,
    bools: Vec<(&'static str, bool)>,
    nums: Vec<(&'static str, u64)>,
}

impl Fields {
    fn string(&self, key: &str) -> Option<&str> {
        self.strings
            .iter()
            .find(|(k, _)| *k == key)
            .map(|(_, value)| value.as_str())
    }

    fn flag(&self, key: &str) -> bool {
        self.bools
            .iter()
            .find(|(k, _)| *k == key)
            .is_some_and(|(_, value)| *value)
    }

    fn num(&self, key: &str) -> Option<u64> {
        self.nums
            .iter()
            .find(|(k, _)| *k == key)
            .map(|(_, value)| *value)
    }
}

impl Visit for Fields {
    fn record_str(&mut self, field: &Field, value: &str) {
        self.strings.push((field.name(), value.to_owned()));
    }

    fn record_bool(&mut self, field: &Field, value: bool) {
        self.bools.push((field.name(), value));
    }

    fn record_u64(&mut self, field: &Field, value: u64) {
        self.nums.push((field.name(), value));
    }

    fn record_i64(&mut self, field: &Field, value: i64) {
        self.nums.push((field.name(), value.max(0) as u64));
    }

    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        self.strings.push((field.name(), format!("{value:?}")));
    }
}

struct Record {
    span_id: u64,
    parent: Option<u64>,
    name: &'static str,
    fields: Fields,
    start_ns: u128,
    end_ns: u128,
    order: u64,
    thread: String,
}

struct Open {
    fields: Fields,
    start_ns: u128,
    order: u64,
}

#[derive(Default)]
struct Shared {
    open: HashMap<u64, Open>,
    done: Vec<Record>,
    next_order: u64,
}

struct JsonlLayer {
    shared: Arc<Mutex<Shared>>,
    origin: Instant,
}

impl JsonlLayer {
    fn now_ns(&self) -> u128 {
        self.origin.elapsed().as_nanos()
    }
}

impl<S> Layer<S> for JsonlLayer
where
    S: Subscriber + for<'lookup> LookupSpan<'lookup>,
{
    fn on_new_span(&self, attrs: &Attributes<'_>, id: &Id, _ctx: Context<'_, S>) {
        let mut fields = Fields::default();
        attrs.record(&mut fields);
        let mut shared = self.shared.lock().expect("F2Z interval span map poisoned");
        let order = shared.next_order;
        shared.next_order = shared.next_order.saturating_add(1);
        shared.open.insert(
            id.into_u64(),
            Open {
                fields,
                start_ns: self.now_ns(),
                order,
            },
        );
    }

    fn on_close(&self, id: Id, ctx: Context<'_, S>) {
        let end_ns = self.now_ns();
        let parent = ctx
            .span(&id)
            .and_then(|span| span.parent().map(|p| p.id().into_u64()));
        let name = ctx.span(&id).map(|span| span.name()).unwrap_or("unknown");
        let thread = std::thread::current()
            .name()
            .map(str::to_owned)
            .unwrap_or_else(|| format!("{:?}", std::thread::current().id()));
        let mut shared = self.shared.lock().expect("F2Z interval span map poisoned");
        if let Some(open) = shared.open.remove(&id.into_u64()) {
            shared.done.push(Record {
                span_id: id.into_u64(),
                parent,
                name,
                fields: open.fields,
                start_ns: open.start_ns,
                end_ns,
                order: open.order,
                thread,
            });
        }
    }
}

fn escape_json(value: &str) -> String {
    let mut out = String::with_capacity(value.len() + 2);
    for c in value.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c => out.push(c),
        }
    }
    out
}

/// Canonical schema operations are lower-case identities separated by `.`,
/// `_`, or `-`. Legacy F2Z profiler labels use `prefix:name`; retain their
/// identity while normalizing only the separator in the serialized trace.
fn stable_operation(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    let mut separator = false;
    for c in value.chars() {
        if c.is_ascii_alphanumeric() {
            if separator && !out.is_empty() {
                out.push('.');
            }
            separator = false;
            out.push(c.to_ascii_lowercase());
        } else if matches!(c, '.' | '_' | '-') {
            if !out.is_empty() && !separator {
                out.push(c);
            }
        } else {
            separator = true;
        }
    }
    while matches!(out.as_bytes().last(), Some(b'.' | b'_' | b'-')) {
        out.pop();
    }
    if out.is_empty() {
        "unknown".to_owned()
    } else {
        out
    }
}

const TAG_FIELDS: &[(&str, &str)] = &[
    ("tag_end_to_end", "end-to-end"),
    ("tag_witness_generation", "witness-generation"),
    ("tag_preparation", "preparation"),
    ("tag_proving", "proving"),
    ("tag_commit", "commit"),
    ("tag_pcs", "pcs"),
    ("tag_opening_proof", "opening-proof"),
    ("tag_constraint_proof", "constraint-proof"),
    ("tag_sumcheck", "sumcheck"),
    ("tag_fri", "fri"),
    ("tag_verification", "verification"),
];

fn primary_phase(tags: &[&str]) -> &'static str {
    for wanted in [
        "end-to-end",
        "witness-generation",
        "commit",
        "sumcheck",
        "fri",
        "constraint-proof",
        "opening-proof",
        "pcs",
        "proving",
        "preparation",
        "verification",
    ] {
        if tags.contains(&wanted) {
            return TAG_FIELDS
                .iter()
                .map(|(_, tag)| *tag)
                .find(|tag| *tag == wanted)
                .expect("controlled trace tag");
        }
    }
    "proving"
}

fn span_depth(record: &Record, records: &HashMap<u64, &Record>, root_id: u64) -> usize {
    let mut depth = 1usize;
    let mut parent = record.parent;
    let mut remaining = records.len();
    while let Some(parent_id) = parent {
        if parent_id == root_id || remaining == 0 {
            break;
        }
        let Some(parent_record) = records.get(&parent_id) else {
            break;
        };
        depth = depth.saturating_add(1);
        parent = parent_record.parent;
        remaining -= 1;
    }
    depth
}

fn span_json(run_id: &str, root_id: u64, record: &Record, depth: usize) -> String {
    let fields = &record.fields;
    let tags: Vec<&str> = TAG_FIELDS
        .iter()
        .filter(|(field, _)| fields.flag(field))
        .map(|(_, tag)| *tag)
        .collect();
    let tags = if tags.is_empty() {
        vec!["proving"]
    } else {
        tags
    };
    let primary = primary_phase(&tags);
    let operation = stable_operation(fields.string("component").unwrap_or("unknown"));
    let parent = match record.parent {
        Some(parent_id) if parent_id != root_id => format!("s{parent_id}"),
        _ => "root".to_owned(),
    };
    let duration = record.end_ns.saturating_sub(record.start_ns);

    let mut line = String::with_capacity(800);
    let _ = write!(
        line,
        r#"{{"schema":"zkperf.trace/v1","record":"span","run_id":"{}","span_id":"s{}","parent_span_id":"{}","operation":"{}","name":"{}","primary_phase":"{}","phase_tags":[{}],"start_ns":"{}","end_ns":"{}","duration_ns":"{}""#,
        escape_json(run_id),
        record.span_id,
        parent,
        escape_json(&operation),
        escape_json(record.name),
        primary,
        tags.iter()
            .map(|tag| format!("\"{tag}\""))
            .collect::<Vec<_>>()
            .join(","),
        record.start_ns,
        record.end_ns,
        duration,
    );
    let _ = write!(
        line,
        r#", "lane":{{"process":"prover","thread":"{}"}}"#,
        escape_json(&record.thread)
    );

    let mut coordinates = Vec::new();
    for (field, key) in [
        ("occurrence_index", "occurrence_index"),
        ("occurrence_count", "occurrence_count"),
        ("round_index", "round_index"),
        ("round_count", "round_count"),
        ("recursion_depth", "recursion_depth"),
        ("recursion_instance_index", "recursion_instance_index"),
    ] {
        if let Some(value) = fields.num(field) {
            coordinates.push(format!("\"{key}\":{value}"));
        }
    }
    if !coordinates.is_empty() {
        let _ = write!(line, r#", "coordinate":{{{}}}"#, coordinates.join(","));
    }

    let mut attributes = vec![
        format!("\"primary_sequence\":{}", fields.flag("primary_sequence")),
        format!("\"order\":{}", record.order),
        format!("\"depth\":{depth}"),
    ];
    for key in ["scope_kind", "short_name", "scope_tag"] {
        if let Some(value) = fields.string(key) {
            attributes.push(format!("\"{key}\":\"{}\"", escape_json(value)));
        }
    }
    if fields.flag("overlay") {
        attributes.push("\"overlay\":true".to_owned());
    }
    let math: Vec<&str> = ["math_latex_1", "math_latex_2"]
        .iter()
        .filter_map(|key| fields.string(key))
        .collect();
    if !math.is_empty() {
        attributes.push(format!(
            "\"math_latex\":[{}]",
            math.iter()
                .map(|value| format!("\"{}\"", escape_json(value)))
                .collect::<Vec<_>>()
                .join(",")
        ));
    }
    let _ = write!(line, r#", "attributes":{{{}}}}}"#, attributes.join(","));
    line
}

#[derive(Clone, Copy)]
pub struct RunMeta<'a> {
    pub capture_id: &'a str,
    pub n: usize,
    pub t: usize,
    pub s: usize,
    pub word_bits: usize,
    pub m_p: usize,
    pub chunks: usize,
    pub requested_config: &'a str,
    pub resolved_config: &'a str,
    pub hash_id: &'a str,
    pub threads: usize,
    pub git_rev: &'a str,
    pub kind: &'static str,
    pub index: usize,
}

#[derive(Clone, Copy, Debug)]
pub struct TrialSummary {
    pub commit_ns: u128,
    pub prove_ns: u128,
    pub serialize_ns: u128,
    pub verify_ns: u128,
    pub proof_bytes: usize,
    pub proof_fnv: u64,
    pub artifact_folds_bytes: usize,
    pub artifact_merged_gkr_bytes: usize,
    pub artifact_presum_bytes: usize,
    pub artifact_ring_bytes: usize,
    pub artifact_ligerito_bytes: usize,
    pub artifact_framing_bytes: usize,
}

impl<'a> RunMeta<'a> {
    fn run_id(&self) -> String {
        format!(
            "f2z-pcs-{}-n{}-{}{}",
            self.capture_id, self.n, self.kind, self.index
        )
    }

    fn series_id(&self) -> String {
        let config: String = self
            .resolved_config
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
            .collect();
        format!(
            "f2z-pcs-{}-n{}-t{}-s{}-w{}-{config}-{}t",
            self.capture_id, self.n, self.t, self.s, self.word_bits, self.threads
        )
    }
}

fn run_json(meta: &RunMeta<'_>, summary: &TrialSummary) -> String {
    let run_id = meta.run_id();
    let series_id = meta.series_id();
    let clock_id = format!("mono-process-{run_id}");
    let trial = if meta.kind == "warmup" {
        format!(r#"{{"kind":"warmup","warmup_index":{}}}"#, meta.index)
    } else {
        format!(r#"{{"kind":"sample","sample_index":{}}}"#, meta.index)
    };
    format!(
        r#"{{"schema":"zkperf.trace/v1","record":"run","run_id":"{}","series_id":"{}","root_span_id":"root","benchmark":{{"suite":"f2z","name":"pcs-intervals","label":"2^{}-bit F2Z PCS","algorithm":"F2Z PCS {} Ligerito","implementation":"f2z","git_rev":"{}","build_profile":"bench"}},"trial":{},"clock":{{"id":"{}","kind":"monotonic","unit":"ns","source":"std::time::Instant"}},"status":"ok","trace_complete":true,"environment":{{"os":"{}","arch":"{}","cpu":"{}","threads":{}}},"measurements":{{"commit_ns":"{}","prove_ns":"{}","serialize_ns":"{}","verify_ns":"{}","proof_bytes":{},"proof_fnv":"{:016x}"}},"artifacts":{{"proof_bytes":{},"folds_bytes":{},"merged_gkr_bytes":{},"presum_bytes":{},"ring_bytes":{},"ligerito_bytes":{},"framing_bytes":{}}},"parameters":{{"input":{{"i":{},"n":{},"t":{},"s":{},"word_bits":{},"m_p":{},"chunks":{}}},"configuration":{{"requested_config":"{}","resolved_config":"{}"}},"security":{{"hash_id":"{}"}},"recursion":{{"max_depth":0,"instance_count":1}},"repetition":{{"count":1}}}}}}"#,
        escape_json(&run_id),
        escape_json(&series_id),
        meta.n,
        escape_json(meta.resolved_config),
        escape_json(meta.git_rev),
        trial,
        escape_json(&clock_id),
        std::env::consts::OS,
        std::env::consts::ARCH,
        escape_json(&std::env::var("F2Z_INTERVAL_CPU").unwrap_or_else(|_| "unknown".to_owned())),
        meta.threads,
        summary.commit_ns,
        summary.prove_ns,
        summary.serialize_ns,
        summary.verify_ns,
        summary.proof_bytes,
        summary.proof_fnv,
        summary.proof_bytes,
        summary.artifact_folds_bytes,
        summary.artifact_merged_gkr_bytes,
        summary.artifact_presum_bytes,
        summary.artifact_ring_bytes,
        summary.artifact_ligerito_bytes,
        summary.artifact_framing_bytes,
        meta.n,
        meta.n,
        meta.t,
        meta.s,
        meta.word_bits,
        meta.m_p,
        meta.chunks,
        escape_json(meta.requested_config),
        escape_json(meta.resolved_config),
        escape_json(meta.hash_id),
    )
}

pub fn capture_trial<T>(
    out_dir: &Path,
    meta: &RunMeta<'_>,
    run: impl FnOnce() -> (T, TrialSummary),
) -> (T, TrialSummary, PathBuf) {
    std::fs::create_dir_all(out_dir).expect("create F2Z_INTERVAL_OUT_DIR");
    let shared = Arc::new(Mutex::new(Shared::default()));
    let origin = Instant::now();
    let subscriber = Registry::default().with(JsonlLayer {
        shared: Arc::clone(&shared),
        origin,
    });

    let result;
    {
        let _subscriber = tracing::subscriber::set_default(subscriber);
        let root = span!(
            tracing::Level::INFO,
            "F2Z PCS interval trial",
            component = "bench.trial",
            short_name = "Trial",
            scope_kind = "scope",
            tag_end_to_end = true,
        );
        let root = root.entered();
        result = run();
        drop(root);
    }

    let (value, summary) = result;
    let mut shared = shared.lock().expect("F2Z interval span map poisoned");
    assert!(
        shared.open.is_empty(),
        "incomplete F2Z interval trace: unclosed spans"
    );
    shared.done.sort_by_key(|record| record.order);
    let root = shared
        .done
        .iter()
        .find(|record| record.fields.flag("tag_end_to_end"))
        .expect("F2Z interval trace has bench:trial root");
    let root_id = root.span_id;
    let records: HashMap<u64, &Record> = shared
        .done
        .iter()
        .map(|record| (record.span_id, record))
        .collect();
    let run_id = meta.run_id();
    let mut lines = Vec::with_capacity(shared.done.len() + 1);
    lines.push(run_json(meta, &summary));
    lines.push(format!(
        r#"{{"schema":"zkperf.trace/v1","record":"span","run_id":"{}","span_id":"root","parent_span_id":null,"operation":"bench.trial","name":"F2Z PCS interval trial","primary_phase":"end-to-end","phase_tags":["end-to-end"],"start_ns":"{}","end_ns":"{}","duration_ns":"{}","lane":{{"process":"prover","thread":"main"}},"attributes":{{"scope_kind":"scope","short_name":"bench:trial","primary_sequence":false,"order":0,"depth":0}}}}"#,
        escape_json(&run_id),
        root.start_ns,
        root.end_ns,
        root.end_ns.saturating_sub(root.start_ns),
    ));
    for record in &shared.done {
        if record.span_id == root_id {
            continue;
        }
        lines.push(span_json(
            &run_id,
            root_id,
            record,
            span_depth(record, &records, root_id),
        ));
    }

    let path = out_dir.join(format!("{run_id}.jsonl"));
    std::fs::write(&path, lines.join("\n") + "\n").expect("write F2Z interval JSONL");
    (value, summary, path)
}

pub fn commit_scope() -> tracing::span::EnteredSpan {
    tracing::info_span!(
        "Commit witness",
        component = "bench.commit",
        short_name = "Commit",
        scope_kind = "phase",
        primary_sequence = true,
        tag_commit = true,
        tag_pcs = true,
    )
    .entered()
}

pub fn prove_scope() -> tracing::span::EnteredSpan {
    tracing::info_span!(
        "Prove PCS opening",
        component = "bench.prove",
        short_name = "Prove",
        scope_kind = "phase",
        primary_sequence = true,
        tag_proving = true,
        tag_pcs = true,
        tag_opening_proof = true,
    )
    .entered()
}

pub fn serialize_scope() -> tracing::span::EnteredSpan {
    tracing::info_span!(
        "Serialize proof",
        component = "bench.serialize",
        short_name = "Serialize",
        scope_kind = "phase",
        primary_sequence = true,
        tag_preparation = true,
    )
    .entered()
}

pub fn artifact_scope() -> tracing::span::EnteredSpan {
    tracing::info_span!(
        "Account for proof artifacts",
        component = "bench.artifacts",
        short_name = "Artifacts",
        scope_kind = "procedure",
        primary_sequence = true,
        tag_preparation = true,
    )
    .entered()
}

pub fn verify_scope() -> tracing::span::EnteredSpan {
    tracing::info_span!(
        "Verify PCS opening",
        component = "bench.verify",
        short_name = "Verify",
        scope_kind = "phase",
        primary_sequence = true,
        tag_verification = true,
    )
    .entered()
}

pub fn git_revision() -> String {
    std::process::Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "unknown".to_owned())
}

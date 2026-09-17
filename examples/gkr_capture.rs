//! Phase-scoped GKR allocation and trace capture. Enable bench-peak-memory only for
//! allocation captures; its atomic accounting makes timing non-comparable.
use f2z::{piop::spartan::ecdsa_sha256::*, transcript::Blake3Transcript};
use p256::ecdsa::{Signature, SigningKey, signature::Signer};
use serde_json::json;
use std::{error::Error, path::PathBuf};

#[cfg(feature = "bench-peak-memory")]
mod allocations {
    use std::{
        alloc::{GlobalAlloc, Layout, System},
        sync::atomic::{AtomicU64, AtomicUsize, Ordering::Relaxed},
    };
    use tracing::{
        Subscriber,
        span::{Attributes, Id},
    };
    use tracing_subscriber::{Layer, layer::Context, registry::LookupSpan};
    pub const NAMES: &[&str] = &[
        "capture:setup",
        "capture:witness",
        "capture:commit",
        "capture:prove",
        "capture:verify",
        "ecdsa:outer_prove",
        "ecdsa:coefficient_combine",
        "ecdsa:shared_inner_prove",
        "ecdsa:f2z_prove",
        "mc:forest",
        "mc:pow2",
        "mf:extract_bits",
        "mf:l1tabs",
        "mf:teto",
        "mf:build_levels",
        "mf:phaseA",
        "mf:bitgen",
        "mf:phaseB",
        "eqf:rounds",
        "eqf:suffix",
        "eqf:grid",
        "eqf:fold:mats_tile",
        "eqf:leaf_tables",
        "eqf:pair2_tables",
        "eqf:leaf2_tables",
        "mc:fold_v",
        "mc:presum_tbls",
        "mc:presum_run",
    ];
    static LIVE: AtomicUsize = AtomicUsize::new(0);
    static PEAK: AtomicUsize = AtomicUsize::new(0);
    static ACTIVE: AtomicU64 = AtomicU64::new(0);
    struct Counters {
        entry: AtomicUsize,
        entry_max: AtomicUsize,
        peak: AtomicUsize,
        growth: AtomicUsize,
        exit_max: AtomicUsize,
        allocated: AtomicUsize,
        calls: AtomicUsize,
        entries: AtomicUsize,
    }
    impl Counters {
        const fn new() -> Self {
            Self {
                entry: AtomicUsize::new(0),
                entry_max: AtomicUsize::new(0),
                peak: AtomicUsize::new(0),
                growth: AtomicUsize::new(0),
                exit_max: AtomicUsize::new(0),
                allocated: AtomicUsize::new(0),
                calls: AtomicUsize::new(0),
                entries: AtomicUsize::new(0),
            }
        }
    }
    static COUNTERS: [Counters; NAMES.len()] = [const { Counters::new() }; NAMES.len()];
    pub struct Allocator;
    fn account(bytes: usize) {
        let live = LIVE.fetch_add(bytes, Relaxed) + bytes;
        PEAK.fetch_max(live, Relaxed);
        let mut active = ACTIVE.load(Relaxed);
        while active != 0 {
            let i = active.trailing_zeros() as usize;
            active &= active - 1;
            let c = &COUNTERS[i];
            c.peak.fetch_max(live, Relaxed);
            c.growth
                .fetch_max(live.saturating_sub(c.entry.load(Relaxed)), Relaxed);
            c.allocated.fetch_add(bytes, Relaxed);
            c.calls.fetch_add(1, Relaxed);
        }
    }
    // SAFETY: preserve System's original pointer/layout contract. Bookkeeping
    // uses only atomics and cannot recurse into the allocator.
    unsafe impl GlobalAlloc for Allocator {
        unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
            let p = unsafe { System.alloc(layout) };
            if !p.is_null() {
                account(layout.size());
            }
            p
        }
        unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
            let p = unsafe { System.alloc_zeroed(layout) };
            if !p.is_null() {
                account(layout.size());
            }
            p
        }
        unsafe fn dealloc(&self, p: *mut u8, layout: Layout) {
            unsafe { System.dealloc(p, layout) };
            LIVE.fetch_sub(layout.size(), Relaxed);
        }
        unsafe fn realloc(&self, p: *mut u8, layout: Layout, size: usize) -> *mut u8 {
            let q = unsafe { System.realloc(p, layout, size) };
            if !q.is_null() {
                if size >= layout.size() {
                    account(size - layout.size());
                } else {
                    LIVE.fetch_sub(layout.size() - size, Relaxed);
                }
            }
            q
        }
    }
    pub struct AllocationLayer {
        owner: std::thread::ThreadId,
    }
    struct Key(Option<usize>);
    impl AllocationLayer {
        pub fn new() -> Self {
            Self {
                owner: std::thread::current().id(),
            }
        }
    }
    impl<S: Subscriber + for<'a> LookupSpan<'a>> Layer<S> for AllocationLayer {
        fn on_new_span(&self, attrs: &Attributes<'_>, id: &Id, ctx: Context<'_, S>) {
            let key = NAMES.iter().position(|n| *n == attrs.metadata().name());
            ctx.span(id).unwrap().extensions_mut().insert(Key(key));
        }
        fn on_enter(&self, id: &Id, ctx: Context<'_, S>) {
            if std::thread::current().id() != self.owner {
                return;
            }
            let span = ctx.span(id).unwrap();
            if let Some(i) = span.extensions().get::<Key>().unwrap().0 {
                let c = &COUNTERS[i];
                let live = LIVE.load(Relaxed);
                c.entry.store(live, Relaxed);
                c.entry_max.fetch_max(live, Relaxed);
                c.peak.fetch_max(live, Relaxed);
                c.entries.fetch_add(1, Relaxed);
                let old = ACTIVE.fetch_or(1u64 << i, Relaxed);
                assert_eq!(
                    old & (1u64 << i),
                    0,
                    "selected diagnostic span recursively entered"
                );
            }
        }
        fn on_exit(&self, id: &Id, ctx: Context<'_, S>) {
            if std::thread::current().id() != self.owner {
                return;
            }
            let span = ctx.span(id).unwrap();
            if let Some(i) = span.extensions().get::<Key>().unwrap().0 {
                ACTIVE.fetch_and(!(1u64 << i), Relaxed);
                COUNTERS[i].exit_max.fetch_max(LIVE.load(Relaxed), Relaxed);
            }
        }
    }
    pub fn snapshot() -> serde_json::Value {
        let rows: Vec<_> = NAMES.iter().zip(&COUNTERS).filter(|(_, c)| c.entries.load(Relaxed)>0).map(|(name,c)|
            serde_json::json!({"phase":name, "entries":c.entries.load(Relaxed),
                "entry_live_max":c.entry_max.load(Relaxed), "peak_live_bytes":c.peak.load(Relaxed),
                "max_growth_from_entry":c.growth.load(Relaxed), "exit_live_max":c.exit_max.load(Relaxed),
                "allocated_bytes":c.allocated.load(Relaxed), "allocation_calls":c.calls.load(Relaxed)})).collect();
        serde_json::json!({"peak_rust_live_bytes":PEAK.load(Relaxed), "current_rust_live_bytes":LIVE.load(Relaxed), "phases":rows})
    }
}

#[cfg(feature = "bench-peak-memory")]
#[global_allocator]
static ALLOCATOR: allocations::Allocator = allocations::Allocator;

fn main() -> Result<(), Box<dyn Error>> {
    use tracing_subscriber::prelude::*;
    let args: Vec<_> = std::env::args().collect();
    let exponent: usize = args.get(1).ok_or("exponent")?.parse()?;
    let threads: usize = args.get(2).ok_or("threads")?.parse()?;
    let reps: usize = args.get(3).ok_or("reps")?.parse()?;
    let output = PathBuf::from(args.get(4).ok_or("output directory")?);
    assert!(
        (3..=12).contains(&exponent) && (1..=32).contains(&threads) && (1..=50).contains(&reps)
    );
    std::fs::create_dir_all(&output)?;
    rayon::ThreadPoolBuilder::new()
        .num_threads(threads)
        .build_global()?;
    #[cfg(feature = "bench-peak-memory")]
    tracing::subscriber::set_global_default(
        tracing_subscriber::registry()
            .with(f2z::observability::layer())
            .with(allocations::AllocationLayer::new()),
    )?;
    #[cfg(not(feature = "bench-peak-memory"))]
    f2z::observability::install()?;

    let setup_recording = f2z::observability::Recording::start(Vec::new())?;
    let prepared = tracing::info_span!("capture:setup").in_scope(|| {
        prepare_sha256_ecdsa(exponent, 100, OuterMode::Split)
            .and_then(|p| p.with_ligerito(f2z::ligerito_flock::LigeritoSelection::JOHNSON))
    })?;
    let setup_trace = setup_recording.into_inner()?;
    std::fs::write(output.join("setup.pftrace"), setup_trace)?;
    let message: Vec<_> = (0..prepared.message_bytes()).map(|i| i as u8).collect();
    let key = SigningKey::from_bytes((&[7u8; 32]).into())?;
    let signature: Signature = key.sign(&message);
    let point = key.verifying_key().to_encoded_point(false);
    let (r, s) = signature.split_bytes();
    let statement = Sha256EcdsaStatement {
        log_compressions: exponent as u8,
        qx: (*point.x().unwrap()).into(),
        qy: (*point.y().unwrap()).into(),
        r: r.into(),
        s: s.into(),
    };
    let mut samples = Vec::new();
    for trial in 0..reps {
        let recording = f2z::observability::Recording::start(Vec::new())?;
        let witness = tracing::info_span!("capture:witness")
            .in_scope(|| generate_sha256_ecdsa_witness(&prepared, &statement, &message))?;
        let hint = tracing::info_span!("capture:commit")
            .in_scope(|| commit_sha256_ecdsa(&prepared, &witness))?;
        let proof = tracing::info_span!("capture:prove").in_scope(|| {
            prove_sha256_ecdsa(
                &mut Blake3Transcript::new(),
                &prepared,
                &statement,
                &witness,
                &hint,
                4,
            )
        })?;
        tracing::info_span!("capture:verify").in_scope(|| {
            verify_sha256_ecdsa(
                &mut Blake3Transcript::new(),
                &prepared,
                &statement,
                &hint.commitment,
                &proof,
            )
        })?;
        let trace = recording.into_inner()?;
        std::fs::write(output.join(format!("trial-{trial}.pftrace")), &trace)?;
        let intervals = f2z::observability::TraceProcessor::from_env().intervals(&trace)?;
        let intervals_json: Vec<_> = intervals.iter().map(|s| json!({"id":s.id,"parent":s.parent,
            "track_id":s.track_id,"depth":s.depth,"name":s.label(),"start_ns":s.start_ns,"end_ns":s.end_ns})).collect();
        std::fs::write(
            output.join(format!("intervals-{trial}.json")),
            serde_json::to_vec(&intervals_json)?,
        )?;
        let h = prepared.assignment_params();
        let rows = witness.assignment_rows();
        let live_columns = rows
            .iter()
            .rposition(|col| col.iter().any(|&w| w != 0))
            .map_or(1, |i| i + 1);
        let row = json!({"trial":trial,"exponent":exponent,"threads":threads,"verified":true,
            "proof_digest":blake3::hash(&proof.to_bytes()).to_hex().to_string(),
            "row_vars":h.row_vars,"col_vars":h.col_vars,"live_columns":live_columns,
            "packed_assignment_bytes":rows.iter().map(|r|r.len()*8).sum::<usize>(),
            "phases_seconds":f2z::observability::phase_totals(&intervals,"capture:prove")?,
            "prove_ms":f2z::observability::duration(&intervals,"capture:prove")?.as_secs_f64()*1000.});
        println!("{row}");
        samples.push(row);
    }
    std::fs::write(
        output.join("samples.json"),
        serde_json::to_vec_pretty(&samples)?,
    )?;
    #[cfg(feature = "bench-peak-memory")]
    std::fs::write(
        output.join("allocations.json"),
        serde_json::to_vec_pretty(&allocations::snapshot())?,
    )?;
    Ok(())
}

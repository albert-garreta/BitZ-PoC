//! Matched, standalone linear-map qualification. Run latency and allocation
//! measurements separately; `bench-memory` instruments allocation only.
use circuit::matrix_wengert::{ForwardColumns, WengertGenerator, WengertTape};
use field::{FpCtx, ModRingCtx, RingOps, Uint};
use std::{hint::black_box, time::Instant};

#[cfg(feature = "bench-memory")]
mod allocations {
    use std::{
        alloc::{GlobalAlloc, Layout, System},
        sync::atomic::{AtomicUsize, Ordering::Relaxed},
    };
    pub struct Counted;
    pub static LIVE: AtomicUsize = AtomicUsize::new(0);
    pub static PEAK: AtomicUsize = AtomicUsize::new(0);
    pub static COUNT: AtomicUsize = AtomicUsize::new(0);
    pub static BYTES: AtomicUsize = AtomicUsize::new(0);
    fn add(n: usize) {
        COUNT.fetch_add(1, Relaxed);
        BYTES.fetch_add(n, Relaxed);
        let live = LIVE.fetch_add(n, Relaxed) + n;
        PEAK.fetch_max(live, Relaxed);
    }
    unsafe impl GlobalAlloc for Counted {
        unsafe fn alloc(&self, l: Layout) -> *mut u8 {
            let p = unsafe { System.alloc(l) };
            if !p.is_null() {
                add(l.size());
            }
            p
        }
        unsafe fn alloc_zeroed(&self, l: Layout) -> *mut u8 {
            let p = unsafe { System.alloc_zeroed(l) };
            if !p.is_null() {
                add(l.size());
            }
            p
        }
        unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
            LIVE.fetch_sub(l.size(), Relaxed);
            unsafe { System.dealloc(p, l) }
        }
        unsafe fn realloc(&self, p: *mut u8, l: Layout, n: usize) -> *mut u8 {
            let q = unsafe { System.realloc(p, l, n) };
            if !q.is_null() {
                LIVE.fetch_sub(l.size(), Relaxed);
                add(n);
            }
            q
        }
    }
    pub fn measure(f: impl FnOnce()) -> (usize, usize, usize) {
        let start = LIVE.load(Relaxed);
        PEAK.store(start, Relaxed);
        COUNT.store(0, Relaxed);
        BYTES.store(0, Relaxed);
        f();
        (
            COUNT.load(Relaxed),
            BYTES.load(Relaxed),
            PEAK.load(Relaxed).saturating_sub(start),
        )
    }
}
#[cfg(feature = "bench-memory")]
#[global_allocator]
static ALLOC: allocations::Counted = allocations::Counted;

fn build(p256: bool) -> WengertTape {
    if p256 {
        use circuit::p256::{VERIFY_DIGEST_INPUT_BITS, verify_digest_circuit};
        let mut b = WengertGenerator::new(VERIFY_DIGEST_INPUT_BITS);
        let inputs = b.take_boxed_inputs();
        verify_digest_circuit(&mut b, &inputs);
        b.finish()
    } else {
        use circuit::sha256::{SHA256_2KB_MESSAGE_BITS, sha256_2kb_circuit};
        let mut b = WengertGenerator::new(SHA256_2KB_MESSAGE_BITS);
        let inputs = b.take_boxed_inputs();
        let _ = sha256_2kb_circuit(&mut b, &inputs);
        b.finish()
    }
}
// Match the production split-equality geometric column functional, including
// an unaligned P-256 tail offset. Its preparation is outside traversal timing.
struct Columns {
    field: FpCtx<2>,
    low: Vec<field::Fp<2>>,
    high: Vec<field::Fp<2>>,
    suffix: Vec<field::Fp<2>>,
    powers: Vec<field::Fp<2>>,
    offset: usize,
}
impl Columns {
    fn new(field: FpCtx<2>, count: usize, offset: usize) -> Self {
        let vars = (count + offset).next_power_of_two().ilog2() as usize;
        let point = (0..vars)
            .map(|i| field::IntegerEmbedding::from_integer(&field, &(i as u64 + 3)))
            .collect::<Vec<_>>();
        let table = |point: &[field::Fp<2>]| {
            let mut out = field.zero_vec(1 << point.len());
            out[0] = field.one();
            for (k, r) in point.iter().enumerate() {
                for i in 0..1 << k {
                    let right = field.mul(&out[i], r);
                    out[i] = field.sub(&out[i], &right);
                    out[i + (1 << k)] = right;
                }
            }
            out
        };
        let low = table(&point[..vars / 2]);
        let high = table(&point[vars / 2..]);
        let mut suffix = field.zero_vec(low.len() + 1);
        let mut powers = vec![field.one(); low.len() + 1];
        for i in (0..low.len()).rev() {
            suffix[i] = field.add(&low[i], &field.add(&suffix[i + 1], &suffix[i + 1]));
        }
        for i in 0..low.len() {
            powers[i + 1] = field.add(&powers[i], &powers[i]);
        }
        Self {
            field,
            low,
            high,
            suffix,
            powers,
            offset,
        }
    }
}
impl ForwardColumns for Columns {
    fn scalar(&self, column: usize) -> [u64; 2] {
        let i = self.offset + column;
        *self
            .field
            .mul(
                &self.low[i % self.low.len()],
                &self.high[i / self.low.len()],
            )
            .as_montgomery_integer()
            .as_words()
    }
    fn power_sum(&self, first: usize, len: usize) -> [u64; 2] {
        let mut i = self.offset + first;
        let end = i + len;
        let mut sum = self.field.zero();
        let mut base = self.field.one();
        while i < end {
            let high = i / self.low.len();
            let lo = i % self.low.len();
            let hi = (lo + end - i).min(self.low.len());
            let part = self.field.sub(
                &self.suffix[lo],
                &self.field.mul(&self.powers[hi - lo], &self.suffix[hi]),
            );
            sum = self.field.add(
                &sum,
                &self
                    .field
                    .mul(&self.field.mul(&base, &part), &self.high[high]),
            );
            base = self.field.mul(&base, &self.powers[hi - lo]);
            i += hi - lo;
        }
        *sum.as_montgomery_integer().as_words()
    }
}
fn measure(name: &str, samples: usize, mut f: impl FnMut()) {
    if std::env::var("PHASE").is_ok_and(|phase| phase != name) {
        return;
    }
    for _ in 0..5 {
        f();
    }
    #[cfg(not(feature = "bench-memory"))]
    {
        let mut times = Vec::with_capacity(samples);
        for _ in 0..samples {
            let start = Instant::now();
            f();
            times.push(start.elapsed().as_secs_f64() * 1e6);
        }
        times.sort_by(f64::total_cmp);
        println!(
            "{name},{:.3},{:.3},{:.3}",
            times[samples / 2],
            times[samples / 10],
            times[samples * 9 / 10]
        );
    }
    #[cfg(feature = "bench-memory")]
    {
        let _ = samples;
        let _ = Instant::now();
        let (count, bytes, peak) = allocations::measure(f);
        println!("{name},{count},{bytes},{peak}");
    }
}
fn main() {
    let p256 = std::env::args().any(|s| s == "p256");
    let samples = std::env::var("SAMPLES")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(40usize);
    if std::env::args().any(|s| s == "construct") {
        measure("construct", samples, || {
            black_box(build(p256));
        });
        return;
    }
    let tape = build(p256);
    let field = field::create_prime_field(Uint::from((1u128 << 127) - 1));
    let modulus = ModRingCtx::new(*field.modulus()).unwrap();
    let encoder = tape.prepare(&modulus).unwrap();
    let weights = (0..tape.row_count())
        .map(|i| {
            [
                encoder.to_montgomery([i as u64 + 3, 0]),
                encoder.to_montgomery([i as u64 + 7, 0]),
                encoder.to_montgomery([i as u64 + 11, 0]),
            ]
        })
        .collect::<Vec<_>>();
    let columns = Columns::new(
        field,
        tape.column_count(),
        if p256 { 20_457 * 8 } else { 0 },
    );
    drop(encoder);
    let mut adjoint = tape.prepare(&modulus).unwrap();
    let mut forward = tape.prepare(&modulus).unwrap();
    println!(
        "phase,{},{}",
        if cfg!(feature = "bench-memory") {
            "allocations,allocated_bytes"
        } else {
            "median_us,p10_us"
        },
        if cfg!(feature = "bench-memory") {
            "extra_peak_bytes"
        } else {
            "p90_us"
        }
    );
    measure("prepare", samples, || {
        black_box(tape.prepare(black_box(&modulus)).unwrap());
    });
    measure("bind_reused", samples, || {
        black_box(adjoint.apply_weighted(black_box(&weights)).unwrap());
    });
    measure("terminal_reused", samples, || {
        black_box(
            forward
                .apply_forward_weighted(black_box(&weights), black_box(&columns))
                .unwrap(),
        );
    });
    measure("prepare_bind", samples, || {
        let mut p = tape.prepare(black_box(&modulus)).unwrap();
        black_box(p.apply_weighted(black_box(&weights)).unwrap());
    });
    eprintln!(
        "topology_bytes={},adjoint_workspace_bytes={},forward_workspace_bytes={}",
        tape.payload_bytes(),
        adjoint.workspace_bytes(),
        forward.workspace_bytes()
    );
}

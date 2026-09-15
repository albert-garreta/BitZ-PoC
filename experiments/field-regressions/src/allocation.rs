//! Count allocations in untimed, warmed audit passes, including Rayon workers.
use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering::Relaxed};

struct Counting;
static ACTIVE: AtomicBool = AtomicBool::new(false);
static CALLS: AtomicU64 = AtomicU64::new(0);
static BYTES: AtomicU64 = AtomicU64::new(0);

#[global_allocator]
static ALLOCATOR: Counting = Counting;

fn record(bytes: usize) {
    if ACTIVE.load(Relaxed) {
        CALLS.fetch_add(1, Relaxed);
        BYTES.fetch_add(bytes as u64, Relaxed);
    }
}
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        record(layout.size());
        unsafe { System.alloc(layout) }
    }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        record(layout.size());
        unsafe { System.alloc_zeroed(layout) }
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        record(size);
        unsafe { System.realloc(ptr, layout, size) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }
}

pub fn audit(run: &mut dyn FnMut()) -> (u64, u64) {
    let mut maximum = (0, 0);
    // Lazy pools and caches are initialized before counting.
    run();
    // The external Rayon injector periodically allocates a 31-job block.
    // Three passes can observe that allocation in only one variant by chance.
    // Cover at least two complete periods, retaining MAX allocations/bytes for
    // a single pass (not an average or an exemption for scheduler allocations).
    for _ in 0..64 {
        CALLS.store(0, Relaxed);
        BYTES.store(0, Relaxed);
        ACTIVE.store(true, Relaxed);
        run();
        ACTIVE.store(false, Relaxed);
        maximum.0 = maximum.0.max(CALLS.load(Relaxed));
        maximum.1 = maximum.1.max(BYTES.load(Relaxed));
    }
    maximum
}

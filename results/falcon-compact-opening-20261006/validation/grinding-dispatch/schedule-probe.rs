const PRIME_GROUP_MARGIN: u32 = 8;

const fn component_grinding_bits(numerator: usize) -> u32 {
    if numerator == 0 {
        return 0;
    }
    PRIME_GROUP_MARGIN + usize::BITS - (numerator - 1).leading_zeros()
}

/// Allocate one shared 2^-133 budget to cubic relations, weighted quadratic
/// forest rounds, and root/line reductions. All arithmetic is exact.
const fn compaction_grinding_bits(d: usize) -> (u32, u32, u32) {
    COMPACTION_GRINDING_BITS[d]
}

// FalconSourceLayout accepts 1..=1024 signatures and pads capacity to a
// power of two, so every constructible layout has capacity log in 0..=10.
// Run the unchanged exact optimizer at compile time, preserving its tie order
// and all security parameters without repeating its search during proofs.
const COMPACTION_GRINDING_BITS: [(u32, u32, u32); 11] = {
    let mut table = [(0, 0, 0); 11];
    let mut d = 0;
    while d < table.len() {
        table[d] = solve_compaction_grinding_bits(d);
        d += 1;
    }
    table
};

const fn solve_compaction_grinding_bits(d: usize) -> (u32, u32, u32) {
    let cubic_rounds = 2 * (11 + d);
    let forest_rounds = 55 + 11 * (d + 1);
    let claim_draws = 11 + if d == 0 { 0 } else { 1 };
    let cubic_degree = (3 * cubic_rounds) as u128;
    let forest_degree = (2 * forest_rounds) as u128;
    let claim_degree = (d + 11) as u128;
    let uniform = component_grinding_bits((cubic_degree + forest_degree + claim_degree) as usize);
    // Every group has >=11 draws and all groups together have <=230. A
    // difficulty >=uniform+5 cannot improve on the feasible uniform choice.
    let denominator_bits = uniform + 5;
    let budget = 1u128 << (denominator_bits - PRIME_GROUP_MARGIN);
    let mut best = (uniform, uniform, uniform);
    let mut work = ((cubic_rounds + forest_rounds + claim_draws) as u128) << uniform;
    let mut cubic_bits = 0;
    while cubic_bits <= denominator_bits {
        let mut forest_bits = 0;
        while forest_bits <= denominator_bits {
            let mut claim_bits = 0;
            while claim_bits <= denominator_bits {
                let error = (cubic_degree << (denominator_bits - cubic_bits))
                    + (forest_degree << (denominator_bits - forest_bits))
                    + (claim_degree << (denominator_bits - claim_bits));
                let candidate_work = ((cubic_rounds as u128) << cubic_bits)
                    + ((forest_rounds as u128) << forest_bits)
                    + ((claim_draws as u128) << claim_bits);
                if error <= budget && candidate_work < work {
                    best = (cubic_bits, forest_bits, claim_bits);
                    work = candidate_work;
                }
                claim_bits += 1;
            }
            forest_bits += 1;
        }
        cubic_bits += 1;
    }
    best
}

#[inline(never)]
fn old_runtime(d: usize) -> (u32,u32,u32) {
    solve_compaction_grinding_bits(std::hint::black_box(d))
}
#[inline(never)]
fn compiled_lookup(d: usize) -> (u32,u32,u32) {
    compaction_grinding_bits(std::hint::black_box(d))
}
fn measure(d:usize, f:fn(usize)->(u32,u32,u32))->f64 {
    let start=std::time::Instant::now();
    for _ in 0..128 {
        std::hint::black_box(f(std::hint::black_box(d)));
    }
    start.elapsed().as_secs_f64()*1e6/128.0
}
fn main(){
    let expected=[(16,16,14),(16,16,15),(17,16,15),(17,16,16),(18,16,17),(16,17,15),(16,17,16),(17,17,15),(17,17,15),(17,17,16),(17,17,17)];
    for (d,&answer) in expected.iter().enumerate(){
        assert_eq!(old_runtime(d),answer);
        assert_eq!(compiled_lookup(d),answer);
    }
    println!("All11 supported capacity logs match the unchanged runtime solver and frozen expected triples.");
    for d in [0,10]{
        for _ in 0..4{
            std::hint::black_box(measure(d,old_runtime));
            std::hint::black_box(measure(d,compiled_lookup));
        }
        let mut old=0.0;let mut new=0.0;
        for _ in 0..24{
            old+=measure(d,old_runtime);
            new+=measure(d,compiled_lookup);
            new+=measure(d,compiled_lookup);
            old+=measure(d,old_runtime);
        }
        old/=48.0;new/=48.0;
        println!("capacity_log={d} previous_us={old:.6} lookup_us={new:.6} saved_us={:.6}",old-new);
    }
}

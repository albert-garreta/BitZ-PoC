//! Actual P-256 A/B/C sparsity and coefficients, applied to bounded synthetic
//! integer vectors. This is a storage/matrix benchmark, not an ECDSA prover.
use crate::{Case, Rng, measure};
use num_bigint::{BigInt, Sign};
use num_traits::{Signed, Zero};
use std::{
    collections::HashMap,
    hint::black_box,
    io::{Read, Write},
    time::Instant,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Z9([u64; 9]);
impl Z9 {
    fn from_bigint(x: &BigInt) -> Self {
        let src = x.to_signed_bytes_le();
        assert!(src.len() <= 72);
        let mut bytes = [if x.sign() == Sign::Minus { 255 } else { 0 }; 72];
        bytes[..src.len()].copy_from_slice(&src);
        Self(std::array::from_fn(|i| {
            u64::from_le_bytes(bytes[i * 8..i * 8 + 8].try_into().unwrap())
        }))
    }
    fn bigint(self) -> BigInt {
        BigInt::from_signed_bytes_le(
            &self
                .0
                .iter()
                .flat_map(|x| x.to_le_bytes())
                .collect::<Vec<_>>(),
        )
    }
    /// Mod 2^576 arithmetic. The input-wide bound is checked once outside all
    /// timed passes, proving these row sums also equal signed exact integers.
    #[inline(always)]
    fn mac(&mut self, coefficient: Self, x: u64) {
        let mut carry = 0u128;
        for i in 0..9 {
            let sum = coefficient.0[i] as u128 * x as u128 + self.0[i] as u128 + carry;
            self.0[i] = sum as u64;
            carry = sum >> 64;
        }
    }
}
#[derive(Clone, Copy)]
#[repr(C)]
struct Entry {
    column: u32,
    id: u32,
}
#[derive(Clone, Copy)]
#[repr(C)]
struct Inline {
    column: u32,
    value: Z9,
}
struct Fixture {
    rows: Vec<usize>,
    entries: Vec<Entry>,
    pool: Vec<Z9>,
    columns: usize,
}

fn synthesize() -> Fixture {
    use circuit::{constraints::ConstraintGenerator, p256};
    let start = Instant::now();
    let mut generator = ConstraintGenerator::new(p256::VERIFY_DIGEST_INPUT_BITS);
    let inputs = generator.boxed_inputs();
    p256::verify_digest_circuit(&mut generator, &inputs);
    let matrices = generator.into_matrices();
    let mut rows = vec![0];
    let mut entries = Vec::new();
    let mut pool = Vec::new();
    let mut ids = HashMap::<BigInt, u32>::new();
    for matrix in [&matrices.a, &matrices.b, &matrices.c] {
        for row in matrix.rows() {
            for (column, value) in row.entries() {
                let id = *ids.entry(value.clone()).or_insert_with(|| {
                    let id = u32::try_from(pool.len()).unwrap();
                    pool.push(Z9::from_bigint(value));
                    id
                });
                entries.push(Entry {
                    column: u32::try_from(*column).unwrap(),
                    id,
                });
            }
            rows.push(entries.len());
        }
    }
    eprintln!(
        "P256 synthesis+interning setup: {} ns",
        start.elapsed().as_nanos()
    );
    Fixture {
        rows,
        entries,
        pool,
        columns: matrices.a.column_count(),
    }
}
fn fixture() -> Fixture {
    let Some(path) = std::env::var_os("FIELD_REGRESSION_FIXTURE") else {
        return synthesize();
    };
    if let Ok(mut file) = std::fs::File::open(&path) {
        let mut magic = [0; 8];
        file.read_exact(&mut magic).unwrap();
        assert_eq!(&magic, b"F2ZMAT01");
        fn read(f: &mut std::fs::File) -> u64 {
            let mut b = [0; 8];
            f.read_exact(&mut b).unwrap();
            u64::from_le_bytes(b)
        }
        let columns = read(&mut file) as usize;
        let nr = read(&mut file) as usize;
        let ne = read(&mut file) as usize;
        let np = read(&mut file) as usize;
        let rows = (0..nr).map(|_| read(&mut file) as usize).collect();
        let entries = (0..ne)
            .map(|_| {
                let e = read(&mut file);
                Entry {
                    column: e as u32,
                    id: (e >> 32) as u32,
                }
            })
            .collect();
        let pool = (0..np)
            .map(|_| Z9(std::array::from_fn(|_| read(&mut file))))
            .collect();
        return Fixture {
            rows,
            entries,
            pool,
            columns,
        };
    }
    let f = synthesize();
    let mut file = std::io::BufWriter::new(std::fs::File::create(path).unwrap());
    file.write_all(b"F2ZMAT01").unwrap();
    let mut write = |x: u64| file.write_all(&x.to_le_bytes()).unwrap();
    for x in [f.columns, f.rows.len(), f.entries.len(), f.pool.len()] {
        write(x as u64);
    }
    for &x in &f.rows {
        write(x as u64);
    }
    for x in &f.entries {
        write(x.column as u64 | ((x.id as u64) << 32));
    }
    for x in &f.pool {
        for &word in &x.0 {
            write(word);
        }
    }
    f
}

#[inline(never)]
fn indexed(rows: &[usize], entries: &[Entry], pool: &[Z9], x: &[u64], out: &mut [Z9]) {
    for (row, y) in rows.windows(2).zip(out) {
        let mut sum = Z9([0; 9]);
        for e in &entries[row[0]..row[1]] {
            sum.mac(pool[e.id as usize], x[e.column as usize]);
        }
        *y = sum;
    }
}
#[inline(never)]
fn compact(rows: &[usize], columns: &[u32], ids: &[u16], pool: &[Z9], x: &[u64], out: &mut [Z9]) {
    for (row, y) in rows.windows(2).zip(out) {
        let mut sum = Z9([0; 9]);
        for i in row[0]..row[1] {
            sum.mac(pool[ids[i] as usize], x[columns[i] as usize]);
        }
        *y = sum;
    }
}
#[inline(never)]
fn inline_aos(rows: &[usize], entries: &[Inline], x: &[u64], out: &mut [Z9]) {
    for (row, y) in rows.windows(2).zip(out) {
        let mut sum = Z9([0; 9]);
        for e in &entries[row[0]..row[1]] {
            sum.mac(e.value, x[e.column as usize]);
        }
        *y = sum;
    }
}
#[inline(never)]
fn inline_soa(rows: &[usize], columns: &[u32], values: &[Z9], x: &[u64], out: &mut [Z9]) {
    for (row, y) in rows.windows(2).zip(out) {
        let mut sum = Z9([0; 9]);
        for i in row[0]..row[1] {
            sum.mac(values[i], x[columns[i] as usize]);
        }
        *y = sum;
    }
}

pub fn run(samples: usize, rng: &mut Rng) {
    let fixture = fixture();
    let f = &fixture;
    eprintln!(
        "P256 fixture: rows={} entries={} coefficients={} columns={} entry_bytes={} z9_bytes={} inline_aos_bytes={}",
        f.rows.len() - 1,
        f.entries.len(),
        f.pool.len(),
        f.columns,
        std::mem::size_of::<Entry>(),
        std::mem::size_of::<Z9>(),
        std::mem::size_of::<Inline>()
    );
    assert!(
        f.pool.len() <= 65536,
        "u16 IDs must fit the public pool size"
    );
    let big_pool: Vec<_> = f.pool.iter().map(|c| c.bigint()).collect();
    let x: Vec<_> = (0..f.columns).map(|_| rng.next() & 65535).collect();
    // Validate all rows against an independent BigInt oracle, including negative coefficients.
    let mut expected = Vec::new();
    for row in f.rows.windows(2) {
        let mut sum = BigInt::zero();
        let mut bound = BigInt::zero();
        for e in &f.entries[row[0]..row[1]] {
            let c = &big_pool[e.id as usize];
            sum += c * x[e.column as usize];
            bound += c.abs() * 65535u64;
        }
        assert!(bound.bits() < 575, "row exceeds signed Z9 capacity");
        expected.push(Z9::from_bigint(&sum));
    }
    let x = &x;
    // Prefixes preserve whole rows; the final size is the complete A/B/C workload.
    for row_count in [8, 256, f.rows.len() - 1] {
        let rows = &f.rows[..=row_count];
        let n = rows[row_count];
        let entries = &f.entries[..n];
        let setup = Instant::now();
        let columns: Vec<_> = entries.iter().map(|e| e.column).collect();
        let ids: Vec<_> = entries.iter().map(|e| e.id as u16).collect();
        eprintln!(
            "P256 setup compact rows={row_count}: {} ns",
            setup.elapsed().as_nanos()
        );
        let setup = Instant::now();
        let inline: Vec<_> = entries
            .iter()
            .map(|e| Inline {
                column: e.column,
                value: f.pool[e.id as usize],
            })
            .collect();
        eprintln!(
            "P256 setup inline_aos rows={row_count}: {} ns",
            setup.elapsed().as_nanos()
        );
        let setup = Instant::now();
        let values: Vec<_> = entries.iter().map(|e| f.pool[e.id as usize]).collect();
        eprintln!(
            "P256 setup inline_soa values rows={row_count}: {} ns",
            setup.elapsed().as_nanos()
        );
        let (columns, ids, inline, values) = (&columns, &ids, &inline, &values);
        let common = rows.len() * std::mem::size_of::<usize>();
        // bytes reports matrix payload only, excluding shared input/output and fixture allocations.
        let sizes = [
            common + n * 8 + f.pool.len() * 72,
            common + n * 6 + f.pool.len() * 72,
            common + n * 80,
            common + n * 76,
        ];
        let mut cases = Vec::new();
        for variant in 0..4 {
            let mut output = vec![Z9([0; 9]); row_count];
            let apply = move |output: &mut [Z9]| match variant {
                0 => indexed(rows, entries, &f.pool, x, output),
                1 => compact(rows, columns, ids, &f.pool, x, output),
                2 => inline_aos(rows, inline, x, output),
                3 => inline_soa(rows, columns, values, x, output),
                _ => unreachable!(),
            };
            apply(&mut output);
            assert_eq!(output, expected[..row_count]);
            let name = ["indexed_u32", "indexed_u16", "inline_aos", "inline_soa"][variant];
            cases.push(Case::new(name, sizes[variant], move || {
                apply(black_box(&mut output));
                black_box(&output);
            }));
        }
        measure(
            "matrix",
            &format!("rows{row_count}_entries{n}"),
            &mut cases,
            samples,
            rng,
        );
    }
    // Same exact reduction in both representations. Public fixed coefficients only;
    // BigInt remainder is NOT suitable for secret values or a constant-time runtime API.
    let modulus = (BigInt::from(1u64) << 100) - BigInt::from(15u64);
    let reduce = |c: &BigInt| ((c % &modulus) + &modulus) % &modulus;
    let expected: Vec<_> = big_pool.iter().map(&reduce).collect();
    let mut pooled = Vec::with_capacity(f.pool.len());
    let mut expanded = Vec::with_capacity(f.entries.len());
    let mut cases = vec![
        Case::new("pool_once", f.pool.len() * 72, || {
            pooled.clear();
            for c in black_box(&big_pool) {
                pooled.push(reduce(c));
            }
            black_box(&pooled);
        }),
        Case::new("per_entry", f.entries.len() * 72, || {
            expanded.clear();
            for e in black_box(&f.entries) {
                expanded.push(reduce(&big_pool[e.id as usize]));
            }
            black_box(&expanded);
        }),
    ];
    // Calibration from per-entry prevents amplifying the 1000x cost ratio into long runs.
    cases.swap(0, 1);
    measure(
        "public_coefficient_reduction",
        "complete",
        &mut cases,
        samples.min(12),
        rng,
    );
    drop(cases);
    assert_eq!(pooled, expected);
    for (value, e) in expanded.iter().zip(&f.entries) {
        assert_eq!(value, &expected[e.id as usize]);
    }
    eprintln!("correctness: all complete P256 rows and coefficient reductions matched BigInt");
}

// H1 probe (docs/forest-speedup-ideas.md §5): can M4's SME2 binary
// outer-product (BMOPA, hw.optional.arm.SME_BI32I32=1, SVL=512) beat the
// NEON PMULL pipeline for FIXED-operand GF(2^128) multiplies?
//
// Fixed multiplier c => 128x128 F2 matrix M_c; y = M_c · x over GF(2).
// BMOPA semantics (to be verified by the built-in check): for 32-bit
// element vectors Zn, Zm and ZA tile T: T[r][c] ^= parity(Zn[r] & Zm[c]).
// So one BMOPA = 16x16 single-bit inner products over one 32-bit word of
// the contraction; a 128-bit contraction takes 4 BMOPAs per row-tile.
//
// Pipeline per 16 inputs (SVL/32 = 16 lanes):
//   4 plane loads; rows 0..63: 16 BMOPA into tiles 0..3, extract
//   (16 MOVA + 16 SLI + 1 store per tile), zero; rows 64..127 same.
// Extraction packs bit 0 of each 32-bit ZA element: acc[j] |= row_i[j]<<i.
//
// Arms:
//   neon-composed : Karatsuba 3xPMULL + 0x87 fold reduction (per mul)
//   neon-fixed    : preprocessed (R0, X^64·R0) 5-PMULL form (f2z's
//                   fixed-scalar kernel shape)
//   sme-full      : transpose-in (NEON, timed separately) + BMOPA kernel
//   sme-bmopa-only: BMOPA issue rate (no extraction)
//   sme-extract   : MOVA+SLI extraction rate only
//
// Build: clang -O3 -march=armv9.2-a+sme2 sme_bmopa_probe.c -o probe
#include <arm_neon.h>
#include <arm_sme.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>

static double now_ns(void) {
    struct timespec ts;
    clock_gettime(CLOCK_MONOTONIC_RAW, &ts);
    return (double)ts.tv_sec * 1e9 + (double)ts.tv_nsec;
}

// ---------- scalar GF(2^128) over x^128 = x^7+x^2+x+1 (0x87) ----------
typedef struct { uint64_t lo, hi; } gf128;

static gf128 gf_xtime(gf128 a) { // multiply by X
    uint64_t carry = a.hi >> 63;
    gf128 r;
    r.hi = (a.hi << 1) | (a.lo >> 63);
    r.lo = (a.lo << 1) ^ (carry ? 0x87u : 0u);
    return r;
}

// M_c columns: col k = c * X^k. Row bit view: M[i] has bit k set iff
// bit i of (c*X^k) is set. Emit rows as 4x u32 words (k-major).
static void build_matrix_rows(gf128 c, uint32_t rows[128][4]) {
    memset(rows, 0, 128 * 4 * sizeof(uint32_t));
    gf128 colv = c;
    for (int k = 0; k < 128; k++) {
        for (int i = 0; i < 128; i++) {
            uint64_t bit = (i < 64) ? ((colv.lo >> i) & 1u) : ((colv.hi >> (i - 64)) & 1u);
            if (bit) rows[i][k >> 5] |= (1u << (k & 31));
        }
        colv = gf_xtime(colv);
    }
}

static gf128 scalar_matmul(const uint32_t rows[128][4], gf128 x) {
    uint32_t xw[4] = { (uint32_t)x.lo, (uint32_t)(x.lo >> 32), (uint32_t)x.hi,
                       (uint32_t)(x.hi >> 32) };
    gf128 y = { 0, 0 };
    for (int i = 0; i < 128; i++) {
        uint32_t acc = (rows[i][0] & xw[0]) ^ (rows[i][1] & xw[1]) ^ (rows[i][2] & xw[2]) ^
                       (rows[i][3] & xw[3]);
        uint32_t bit = __builtin_popcount(acc) & 1u;
        if (i < 64) y.lo |= (uint64_t)bit << i;
        else y.hi |= (uint64_t)bit << (i - 64);
    }
    return y;
}

// ---------- NEON reference arms ----------
static inline poly64x2_t ld_p(const gf128 *p) { return vreinterpretq_p64_u64(vld1q_u64(&p->lo)); }

// composed: karatsuba 3 PMULL -> 256-bit, then 0x87 fold (2 PMULL)
static inline uint64x2_t gf_mul_neon(poly64x2_t a, poly64x2_t b) {
    poly64_t a0 = vgetq_lane_p64(a, 0), a1 = vgetq_lane_p64(a, 1);
    poly64_t b0 = vgetq_lane_p64(b, 0), b1 = vgetq_lane_p64(b, 1);
    uint64x2_t p00 = vreinterpretq_u64_p128(vmull_p64(a0, b0));
    uint64x2_t p11 = vreinterpretq_u64_p128(vmull_p64(a1, b1));
    uint64x2_t pmid = vreinterpretq_u64_p128(
        vmull_p64((poly64_t)((uint64_t)a0 ^ (uint64_t)a1), (poly64_t)((uint64_t)b0 ^ (uint64_t)b1)));
    pmid = veorq_u64(pmid, veorq_u64(p00, p11));
    uint64x2_t lo = veorq_u64(p00, vcombine_u64(vdup_n_u64(0), vget_low_u64(pmid)));
    uint64x2_t hi = veorq_u64(p11, vcombine_u64(vget_high_u64(pmid), vdup_n_u64(0)));
    // reduce hi (bits 128..255) with x^128 = 0x87: two folds
    const poly64_t G = (poly64_t)0x87u;
    uint64x2_t t = vreinterpretq_u64_p128(vmull_p64(vgetq_lane_p64(vreinterpretq_p64_u64(hi), 1), G));
    // fold hi.hi first into (hi.lo, lo.hi... ) — 191-bit then once more
    uint64x2_t mid = veorq_u64(vcombine_u64(vget_low_u64(hi), vdup_n_u64(0)), t);
    uint64x2_t t2 = vreinterpretq_u64_p128(vmull_p64(vgetq_lane_p64(vreinterpretq_p64_u64(mid), 1), G));
    uint64x2_t r = veorq_u64(lo, vcombine_u64(vget_low_u64(t2), vget_high_u64(t2)));
    r = veorq_u64(r, vcombine_u64(vget_low_u64(mid), vdup_n_u64(0)));
    (void)r;
    // NOTE: exact field constants unimportant for THROUGHPUT; the op mix
    // (5 PMULL + EOR glue) matches the repo's composed shape.
    return r;
}

// fixed-operand 5-PMULL shape: precomputed rl = [R0.lo, R1.lo],
// rh = [R0.hi, R1.hi]; product = 4 shuffle-free PMULLs + one fold PMULL.
static inline uint64x2_t gf_mul_neon_fixed(uint64x2_t av, poly64x2_t rl, poly64x2_t rh) {
    poly64x2_t ap = vreinterpretq_p64_u64(av);
    uint64x2_t tl = veorq_u64(
        vreinterpretq_u64_p128(vmull_p64(vgetq_lane_p64(ap, 0), vgetq_lane_p64(rl, 0))),
        vreinterpretq_u64_p128(vmull_high_p64(ap, rl)));
    uint64x2_t tm = veorq_u64(
        vreinterpretq_u64_p128(vmull_p64(vgetq_lane_p64(ap, 0), vgetq_lane_p64(rh, 0))),
        vreinterpretq_u64_p128(vmull_high_p64(ap, rh)));
    const poly64_t G = (poly64_t)0x87u;
    uint64x2_t f = vreinterpretq_u64_p128(vmull_p64(vgetq_lane_p64(vreinterpretq_p64_u64(tm), 1), G));
    uint64x2_t r = veorq_u64(tl, f);
    r = veorq_u64(r, vcombine_u64(vdup_n_u64(0), vget_low_u64(tm)));
    return r;
}

// ---------- SME kernel ----------
// x planes: xw[w][j] = word w of input j (SoA). y planes: 8 planes of
// u32 (16 valid bits): yp[t][j] = output bits [16t, 16t+16) of input j.
__arm_new("za") __arm_locally_streaming static void sme_matmul(
    const uint32_t rows[128][4], const uint32_t *const xw[4], uint32_t *const yp[8], size_t n) {
    const svbool_t pg = svptrue_b32();
    // row-tile lanes, tile-major: rt[t][w][i] = rows[16t+i][w]
    uint32_t rt[8][4][16];
    for (int t = 0; t < 8; t++)
        for (int w = 0; w < 4; w++)
            for (int i = 0; i < 16; i++) rt[t][w][i] = rows[16 * t + i][w];
    svzero_za();
    for (size_t j = 0; j < n; j += 16) {
        svuint32_t x0 = svld1_u32(pg, xw[0] + j);
        svuint32_t x1 = svld1_u32(pg, xw[1] + j);
        svuint32_t x2 = svld1_u32(pg, xw[2] + j);
        svuint32_t x3 = svld1_u32(pg, xw[3] + j);
        for (int half = 0; half < 2; half++) {
            const int tbase = 4 * half;
            svbmopa_za32_u32_m(0, pg, pg, svld1_u32(pg, rt[tbase + 0][0]), x0);
            svbmopa_za32_u32_m(1, pg, pg, svld1_u32(pg, rt[tbase + 1][0]), x0);
            svbmopa_za32_u32_m(2, pg, pg, svld1_u32(pg, rt[tbase + 2][0]), x0);
            svbmopa_za32_u32_m(3, pg, pg, svld1_u32(pg, rt[tbase + 3][0]), x0);
            svbmopa_za32_u32_m(0, pg, pg, svld1_u32(pg, rt[tbase + 0][1]), x1);
            svbmopa_za32_u32_m(1, pg, pg, svld1_u32(pg, rt[tbase + 1][1]), x1);
            svbmopa_za32_u32_m(2, pg, pg, svld1_u32(pg, rt[tbase + 2][1]), x1);
            svbmopa_za32_u32_m(3, pg, pg, svld1_u32(pg, rt[tbase + 3][1]), x1);
            svbmopa_za32_u32_m(0, pg, pg, svld1_u32(pg, rt[tbase + 0][2]), x2);
            svbmopa_za32_u32_m(1, pg, pg, svld1_u32(pg, rt[tbase + 1][2]), x2);
            svbmopa_za32_u32_m(2, pg, pg, svld1_u32(pg, rt[tbase + 2][2]), x2);
            svbmopa_za32_u32_m(3, pg, pg, svld1_u32(pg, rt[tbase + 3][2]), x2);
            svbmopa_za32_u32_m(0, pg, pg, svld1_u32(pg, rt[tbase + 0][3]), x3);
            svbmopa_za32_u32_m(1, pg, pg, svld1_u32(pg, rt[tbase + 1][3]), x3);
            svbmopa_za32_u32_m(2, pg, pg, svld1_u32(pg, rt[tbase + 2][3]), x3);
            svbmopa_za32_u32_m(3, pg, pg, svld1_u32(pg, rt[tbase + 3][3]), x3);
            // extract + pack: acc[j] |= row_i[j] << i (result bit is in
            // bit 0 of each 32-bit ZA element)
            svuint32_t z = svdup_n_u32(0);
            svuint32_t a0 = z, a1 = z, a2 = z, a3 = z;
            for (int i = 0; i < 16; i++) {
                a0 = svorr_u32_m(pg, a0, svlsl_n_u32_m(pg, svread_hor_za32_u32_m(z, pg, 0, i), i));
                a1 = svorr_u32_m(pg, a1, svlsl_n_u32_m(pg, svread_hor_za32_u32_m(z, pg, 1, i), i));
                a2 = svorr_u32_m(pg, a2, svlsl_n_u32_m(pg, svread_hor_za32_u32_m(z, pg, 2, i), i));
                a3 = svorr_u32_m(pg, a3, svlsl_n_u32_m(pg, svread_hor_za32_u32_m(z, pg, 3, i), i));
            }
            svst1_u32(pg, yp[tbase + 0] + j, a0);
            svst1_u32(pg, yp[tbase + 1] + j, a1);
            svst1_u32(pg, yp[tbase + 2] + j, a2);
            svst1_u32(pg, yp[tbase + 3] + j, a3);
            svzero_za();
        }
    }
}

// BMOPA-only issue rate: same op stream, no extraction.
__arm_new("za") __arm_locally_streaming static void sme_bmopa_only(
    const uint32_t rows[128][4], const uint32_t *const xw[4], size_t n) {
    const svbool_t pg = svptrue_b32();
    svuint32_t r0 = svld1_u32(pg, rows[0]), r1 = svld1_u32(pg, rows[16]);
    svuint32_t r2 = svld1_u32(pg, rows[32]), r3 = svld1_u32(pg, rows[48]);
    for (size_t j = 0; j < n; j += 16) {
        svuint32_t x0 = svld1_u32(pg, xw[0] + j);
        svuint32_t x1 = svld1_u32(pg, xw[1] + j);
        svuint32_t x2 = svld1_u32(pg, xw[2] + j);
        svuint32_t x3 = svld1_u32(pg, xw[3] + j);
        for (int rep = 0; rep < 2; rep++) {
            svbmopa_za32_u32_m(0, pg, pg, r0, x0); svbmopa_za32_u32_m(1, pg, pg, r1, x0);
            svbmopa_za32_u32_m(2, pg, pg, r2, x0); svbmopa_za32_u32_m(3, pg, pg, r3, x0);
            svbmopa_za32_u32_m(0, pg, pg, r0, x1); svbmopa_za32_u32_m(1, pg, pg, r1, x1);
            svbmopa_za32_u32_m(2, pg, pg, r2, x1); svbmopa_za32_u32_m(3, pg, pg, r3, x1);
            svbmopa_za32_u32_m(0, pg, pg, r0, x2); svbmopa_za32_u32_m(1, pg, pg, r1, x2);
            svbmopa_za32_u32_m(2, pg, pg, r2, x2); svbmopa_za32_u32_m(3, pg, pg, r3, x2);
            svbmopa_za32_u32_m(0, pg, pg, r0, x3); svbmopa_za32_u32_m(1, pg, pg, r1, x3);
            svbmopa_za32_u32_m(2, pg, pg, r2, x3); svbmopa_za32_u32_m(3, pg, pg, r3, x3);
        }
    }
}

// extraction-only rate: 8x(16 MOVA + 16 SLI-ish) per 16 inputs.
__arm_new("za") __arm_locally_streaming static void sme_extract_only(
    uint32_t *const yp[8], size_t n) {
    const svbool_t pg = svptrue_b32();
    for (size_t j = 0; j < n; j += 16) {
        for (int half = 0; half < 2; half++) {
            svuint32_t z = svdup_n_u32(0);
            svuint32_t a0 = z, a1 = z, a2 = z, a3 = z;
            for (int i = 0; i < 16; i++) {
                a0 = svorr_u32_m(pg, a0, svlsl_n_u32_m(pg, svread_hor_za32_u32_m(z, pg, 0, i), i));
                a1 = svorr_u32_m(pg, a1, svlsl_n_u32_m(pg, svread_hor_za32_u32_m(z, pg, 1, i), i));
                a2 = svorr_u32_m(pg, a2, svlsl_n_u32_m(pg, svread_hor_za32_u32_m(z, pg, 2, i), i));
                a3 = svorr_u32_m(pg, a3, svlsl_n_u32_m(pg, svread_hor_za32_u32_m(z, pg, 3, i), i));
            }
            svst1_u32(pg, yp[4 * half + 0] + j, a0);
            svst1_u32(pg, yp[4 * half + 1] + j, a1);
            svst1_u32(pg, yp[4 * half + 2] + j, a2);
            svst1_u32(pg, yp[4 * half + 3] + j, a3);
        }
    }
}

// Semantics probe: one BMOPA of known vectors, raw full-tile dump.
__arm_new("za") __arm_locally_streaming static void sme_semantics(
    const uint32_t *zn, const uint32_t *zm, uint32_t out_tile[16][16]) {
    const svbool_t pg = svptrue_b32();
    svzero_za();
    svbmopa_za32_u32_m(0, pg, pg, svld1_u32(pg, zn), svld1_u32(pg, zm));
    svuint32_t z = svdup_n_u32(0);
    for (int i = 0; i < 16; i++)
        svst1_u32(pg, out_tile[i], svread_hor_za32_u32_m(z, pg, 0, i));
}

static void unit_probe(int znlane, int znbit, int zmlane, int zmbit) {
    uint32_t zn[16] = { 0 }, zm[16] = { 0 }, tile[16][16];
    zn[znlane] = 1u << znbit;
    zm[zmlane] = 1u << zmbit;
    sme_semantics(zn, zm, tile);
    printf("zn[%d]=bit%-2d zm[%d]=bit%-2d ->", znlane, znbit, zmlane, zmbit);
    int cnt = 0;
    for (int r = 0; r < 16 && cnt < 6; r++)
        for (int c = 0; c < 16 && cnt < 6; c++)
            if (tile[r][c]) { printf(" ZA[%d][%d]=%08x", r, c, tile[r][c]); cnt++; }
    if (!cnt) printf(" (all zero)");
    printf("\n");
}

int main(int argc, char **argv) {
    if (argc > 3) { // ./probe 0 0 map  -> semantics mapping only
        unit_probe(0, 0, 0, 0);
        unit_probe(0, 1, 0, 0);
        unit_probe(0, 0, 0, 1);
        unit_probe(0, 8, 0, 8);
        unit_probe(0, 16, 0, 16);
        unit_probe(0, 31, 0, 31);
        unit_probe(1, 0, 0, 0);
        unit_probe(0, 0, 1, 0);
        unit_probe(3, 5, 7, 5);
        unit_probe(3, 5, 7, 9);
        return 0;
    }
    size_t n = (argc > 1) ? (size_t)atoll(argv[1]) : (1u << 20);
    n &= ~(size_t)15;
    int reps = (argc > 2) ? atoi(argv[2]) : 5;
    printf("SVL bytes (streaming): %u, n=%zu\n", (unsigned)svcntsb(), n);

    gf128 c = { 0x0123456789ABCDEFull, 0xFEDCBA9876543210ull };
    static uint32_t rows[128][4];
    build_matrix_rows(c, rows);

    gf128 *x = aligned_alloc(64, n * sizeof(gf128));
    gf128 *y = aligned_alloc(64, n * sizeof(gf128));
    uint64_t seed = 0x9E3779B97F4A7C15ull;
    for (size_t i = 0; i < n; i++) {
        seed = seed * 6364136223846793005ull + 1442695040888963407ull; x[i].lo = seed;
        seed = seed * 6364136223846793005ull + 1442695040888963407ull; x[i].hi = seed;
    }
    uint32_t *xw[4], *yp[8];
    for (int w = 0; w < 4; w++) xw[w] = aligned_alloc(64, n * 4);
    for (int t = 0; t < 8; t++) yp[t] = aligned_alloc(64, n * 4);

    // transpose-in AoS -> SoA planes (NEON ld4)
    double t0 = now_ns();
    for (int r = 0; r < reps; r++)
        for (size_t j = 0; j < n; j += 4) {
            uint32x4x4_t q = vld4q_u32((const uint32_t *)&x[j]);
            vst1q_u32(xw[0] + j, q.val[0]); vst1q_u32(xw[1] + j, q.val[1]);
            vst1q_u32(xw[2] + j, q.val[2]); vst1q_u32(xw[3] + j, q.val[3]);
        }
    double t_tr = (now_ns() - t0) / reps / n;

    // SME full kernel + correctness check
    t0 = now_ns();
    for (int r = 0; r < reps; r++) sme_matmul(rows, (const uint32_t *const *)xw, yp, n);
    double t_sme = (now_ns() - t0) / reps / n;
    int bad = 0;
    for (size_t j = 0; j < 256 && j < n; j++) {
        gf128 ref = scalar_matmul(rows, x[j]);
        for (int t = 0; t < 8; t++) {
            uint32_t got = yp[t][j] & 0xFFFFu;
            uint32_t want = (uint32_t)(((t < 4 ? ref.lo : ref.hi) >> (16 * (t & 3))) & 0xFFFFu);
            if (got != want) { bad++; if (bad < 4) printf("MISMATCH j=%zu t=%d got=%04x want=%04x\n", j, t, got, want); }
        }
    }
    printf("correctness: %s\n", bad ? "FAIL" : "ok");

    // attribution loops
    t0 = now_ns();
    for (int r = 0; r < reps; r++) sme_bmopa_only(rows, (const uint32_t *const *)xw, n);
    double t_bmopa = (now_ns() - t0) / reps / n;
    t0 = now_ns();
    for (int r = 0; r < reps; r++) sme_extract_only(yp, n);
    double t_extr = (now_ns() - t0) / reps / n;

    // NEON arms
    poly64x2_t cv = ld_p(&c);
    volatile uint64_t sink = 0;
    t0 = now_ns();
    for (int r = 0; r < reps; r++) {
        uint64x2_t acc = vdupq_n_u64(0);
        for (size_t j = 0; j < n; j++) acc = veorq_u64(acc, gf_mul_neon(cv, ld_p(&x[j])));
        sink ^= vgetq_lane_u64(acc, 0);
    }
    double t_neon = (now_ns() - t0) / reps / n;
    // fixed form (rl/rh precomputed once; contents don't matter for rate)
    poly64x2_t rl = vreinterpretq_p64_u64(vdupq_n_u64(c.lo)), rh = vreinterpretq_p64_u64(vdupq_n_u64(c.hi));
    t0 = now_ns();
    for (int r = 0; r < reps; r++) {
        uint64x2_t acc = vdupq_n_u64(0);
        for (size_t j = 0; j < n; j++) acc = veorq_u64(acc, gf_mul_neon_fixed(vld1q_u64(&x[j].lo), rl, rh));
        sink ^= vgetq_lane_u64(acc, 0);
    }
    double t_neonf = (now_ns() - t0) / reps / n;
    // NEON store-variant (writes y like the SME kernel does)
    t0 = now_ns();
    for (int r = 0; r < reps; r++) {
        for (size_t j = 0; j < n; j++) vst1q_u64(&y[j].lo, gf_mul_neon_fixed(vld1q_u64(&x[j].lo), rl, rh));
        sink ^= y[n - 1].lo ^ y[0].hi;
    }
    double t_neonfs = (now_ns() - t0) / reps / n;

    printf("\nns per 128-bit fixed-operand multiply (single thread):\n");
    printf("  neon composed (5 PMULL + glue, acc):   %6.3f\n", t_neon);
    printf("  neon fixed    (5 PMULL flat, acc):     %6.3f\n", t_neonf);
    printf("  neon fixed    (load+mul+store):        %6.3f\n", t_neonfs);
    printf("  sme  transpose-in (ld4/st):            %6.3f\n", t_tr);
    printf("  sme  full kernel (bmopa+extract):      %6.3f\n", t_sme);
    printf("  sme  bmopa-only  (2/input issued):     %6.3f\n", t_bmopa);
    printf("  sme  extract-only:                     %6.3f\n", t_extr);
    printf("  sme  total (transpose + kernel):       %6.3f\n", t_tr + t_sme);
    printf("\n(sink %llu)\n", (unsigned long long)sink);
    return 0;
}

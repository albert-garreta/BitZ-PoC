
/var/folders/q0/v7l8dzd13k7f_q6j80sy03lm0000gn/T/f2z-field-gated-bkk46c4y/field-regressions:	file format mach-o arm64

Disassembly of section __TEXT,__text:

0000000100002280 <field_regressions::arithmetic::wide_dot::<1>>:
100002280:     	sub	sp, sp, #0x20
100002284:     	stp	x29, x30, [sp, #0x10]
100002288:     	add	x29, sp, #0x10
10000228c:     	stp	x2, x4, [sp]
100002290:     	cmp	x2, x4
100002294:     	b.ne	0x100002320 <field_regressions::arithmetic::wide_dot::<1>+0xa0>
100002298:     	movi.2d	v0, #0000000000000000
10000229c:     	movi.2d	v2, #0000000000000000
1000022a0:     	movi.2d	v1, #0000000000000000
1000022a4:     	cbz	x2, 0x1000022f0 <field_regressions::arithmetic::wide_dot::<1>+0x70>
1000022a8:     	add	x8, x3, #0x8
1000022ac:     	add	x9, x1, #0x8
1000022b0:     	movi.2d	v3, #0000000000000000
1000022b4:     	ldp	d4, d5, [x8, #-0x8]
1000022b8:     	ldp	d6, d7, [x9, #-0x8]
1000022bc:     	pmull.1q	v16, v6, v4
1000022c0:     	pmull.1q	v17, v7, v5
1000022c4:     	pmull.1q	v5, v6, v5
1000022c8:     	pmull.1q	v4, v7, v4
1000022cc:     	eor.16b	v4, v4, v5
1000022d0:     	ext.16b	v5, v3, v4, #0x8
1000022d4:     	ext.16b	v4, v4, v3, #0x8
1000022d8:     	eor3.16b	v1, v5, v16, v1
1000022dc:     	eor3.16b	v2, v4, v17, v2
1000022e0:     	add	x8, x8, #0x10
1000022e4:     	add	x9, x9, #0x10
1000022e8:     	subs	x2, x2, #0x1
1000022ec:     	b.ne	0x1000022b4 <field_regressions::arithmetic::wide_dot::<1>+0x34>
1000022f0:     	mov	w8, #0x87               ; =135
1000022f4:     	dup.2d	v3, x8
1000022f8:     	pmull2.1q	v4, v2, v3
1000022fc:     	ext.16b	v0, v0, v4, #0x8
100002300:     	pmull2.1q	v4, v4, v3
100002304:     	pmull.1q	v2, v2, v3
100002308:     	eor.16b	v1, v1, v2
10000230c:     	eor3.16b	v0, v1, v0, v4
100002310:     	str	q0, [x0]
100002314:     	ldp	x29, x30, [sp, #0x10]
100002318:     	add	sp, sp, #0x20
10000231c:     	ret
100002320:     	adrp	x3, 0x1000b8000 <dyld_stub_binder+0x1000b8000>
100002324:     	add	x3, x3, #0x528
100002328:     	mov	x0, sp
10000232c:     	add	x1, sp, #0x8
100002330:     	mov	x2, #0x0                ; =0
100002334:     	bl	0x100088d48 <core::panicking::assert_failed::<usize, usize>>

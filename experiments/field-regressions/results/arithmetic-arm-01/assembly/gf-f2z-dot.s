
/var/folders/q0/v7l8dzd13k7f_q6j80sy03lm0000gn/T/f2z-field-gated-bkk46c4y/field-regressions:	file format mach-o arm64

Disassembly of section __TEXT,__text:

000000010001a34c <field_regressions::campaign::f2z_dot>:
10001a34c:     	sub	sp, sp, #0x20
10001a350:     	stp	x29, x30, [sp, #0x10]
10001a354:     	add	x29, sp, #0x10
10001a358:     	stp	x2, x4, [sp]
10001a35c:     	cmp	x2, x4
10001a360:     	b.ne	0x10001a3ec <field_regressions::campaign::f2z_dot+0xa0>
10001a364:     	movi.2d	v0, #0000000000000000
10001a368:     	movi.2d	v2, #0000000000000000
10001a36c:     	movi.2d	v1, #0000000000000000
10001a370:     	cbz	x2, 0x10001a3bc <field_regressions::campaign::f2z_dot+0x70>
10001a374:     	add	x8, x1, #0x8
10001a378:     	add	x9, x3, #0x8
10001a37c:     	movi.2d	v3, #0000000000000000
10001a380:     	ldp	d4, d5, [x9, #-0x8]
10001a384:     	ldp	d6, d7, [x8, #-0x8]
10001a388:     	pmull.1q	v16, v6, v4
10001a38c:     	pmull.1q	v17, v7, v5
10001a390:     	pmull.1q	v5, v6, v5
10001a394:     	pmull.1q	v4, v7, v4
10001a398:     	eor.16b	v4, v4, v5
10001a39c:     	ext.16b	v5, v3, v4, #0x8
10001a3a0:     	ext.16b	v4, v4, v3, #0x8
10001a3a4:     	eor3.16b	v1, v1, v16, v5
10001a3a8:     	eor3.16b	v2, v2, v17, v4
10001a3ac:     	add	x8, x8, #0x10
10001a3b0:     	add	x9, x9, #0x10
10001a3b4:     	subs	x2, x2, #0x1
10001a3b8:     	b.ne	0x10001a380 <field_regressions::campaign::f2z_dot+0x34>
10001a3bc:     	mov	w8, #0x87               ; =135
10001a3c0:     	dup.2d	v3, x8
10001a3c4:     	pmull2.1q	v4, v2, v3
10001a3c8:     	ext.16b	v0, v0, v4, #0x8
10001a3cc:     	pmull2.1q	v4, v4, v3
10001a3d0:     	pmull.1q	v2, v2, v3
10001a3d4:     	eor.16b	v1, v1, v2
10001a3d8:     	eor3.16b	v0, v1, v0, v4
10001a3dc:     	str	q0, [x0]
10001a3e0:     	ldp	x29, x30, [sp, #0x10]
10001a3e4:     	add	sp, sp, #0x20
10001a3e8:     	ret
10001a3ec:     	adrp	x3, 0x1000b9000 <dyld_stub_binder+0x1000b9000>
10001a3f0:     	add	x3, x3, #0x798
10001a3f4:     	mov	x0, sp
10001a3f8:     	add	x1, sp, #0x8
10001a3fc:     	mov	x2, #0x0                ; =0
10001a400:     	bl	0x100088d48 <core::panicking::assert_failed::<usize, usize>>

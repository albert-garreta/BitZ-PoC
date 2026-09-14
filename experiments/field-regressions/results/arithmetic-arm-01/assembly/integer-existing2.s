
/var/folders/q0/v7l8dzd13k7f_q6j80sy03lm0000gn/T/f2z-field-gated-bkk46c4y/field-regressions:	file format mach-o arm64

Disassembly of section __TEXT,__text:

00000001000070ac <field_regressions::campaign::integer::existing::<2>>:
1000070ac:     	sub	sp, sp, #0x20
1000070b0:     	stp	x29, x30, [sp, #0x10]
1000070b4:     	add	x29, sp, #0x10
1000070b8:     	stp	x2, x4, [sp]
1000070bc:     	cmp	x2, x4
1000070c0:     	b.ne	0x100007128 <field_regressions::campaign::integer::existing::<2>+0x7c>
1000070c4:     	cbz	x2, 0x100007110 <field_regressions::campaign::integer::existing::<2>+0x64>
1000070c8:     	mov	x8, #0x0                ; =0
1000070cc:     	mov	x9, #0x0                ; =0
1000070d0:     	add	x10, x1, #0x8
1000070d4:     	add	x11, x3, #0x8
1000070d8:     	ldp	x12, x13, [x11, #-0x8]
1000070dc:     	ldp	x14, x15, [x10, #-0x8]
1000070e0:     	mul	x16, x12, x14
1000070e4:     	umulh	x17, x12, x14
1000070e8:     	mul	x12, x12, x15
1000070ec:     	madd	x12, x13, x14, x12
1000070f0:     	add	x12, x12, x17
1000070f4:     	adds	x9, x9, x16
1000070f8:     	adc	x8, x8, x12
1000070fc:     	add	x10, x10, #0x10
100007100:     	add	x11, x11, #0x10
100007104:     	subs	x2, x2, #0x1
100007108:     	b.ne	0x1000070d8 <field_regressions::campaign::integer::existing::<2>+0x2c>
10000710c:     	b	0x100007118 <field_regressions::campaign::integer::existing::<2>+0x6c>
100007110:     	mov	x9, #0x0                ; =0
100007114:     	mov	x8, #0x0                ; =0
100007118:     	stp	x9, x8, [x0]
10000711c:     	ldp	x29, x30, [sp, #0x10]
100007120:     	add	sp, sp, #0x20
100007124:     	ret
100007128:     	adrp	x3, 0x1000b8000 <dyld_stub_binder+0x1000b8000>
10000712c:     	add	x3, x3, #0x970
100007130:     	mov	x0, sp
100007134:     	add	x1, sp, #0x8
100007138:     	mov	x2, #0x0                ; =0
10000713c:     	bl	0x100088d48 <core::panicking::assert_failed::<usize, usize>>

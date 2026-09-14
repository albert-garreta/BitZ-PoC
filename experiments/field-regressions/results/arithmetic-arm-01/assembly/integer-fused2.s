
/var/folders/q0/v7l8dzd13k7f_q6j80sy03lm0000gn/T/f2z-field-gated-bkk46c4y/field-regressions:	file format mach-o arm64

Disassembly of section __TEXT,__text:

00000001000062c8 <field_regressions::campaign::integer::mac::<2, false>>:
1000062c8:     	sub	sp, sp, #0x20
1000062cc:     	stp	x29, x30, [sp, #0x10]
1000062d0:     	add	x29, sp, #0x10
1000062d4:     	stp	x2, x4, [sp]
1000062d8:     	cmp	x2, x4
1000062dc:     	b.ne	0x10000633c <field_regressions::campaign::integer::mac::<2, false>+0x74>
1000062e0:     	cmp	x2, #0x100, lsl #12     ; =0x100000
1000062e4:     	b.hi	0x100006354 <field_regressions::campaign::integer::mac::<2, false>+0x8c>
1000062e8:     	mov	x8, #0x0                ; =0
1000062ec:     	mov	x9, #0x0                ; =0
1000062f0:     	cbz	x2, 0x10000632c <field_regressions::campaign::integer::mac::<2, false>+0x64>
1000062f4:     	add	x10, x1, #0x8
1000062f8:     	add	x11, x3, #0x8
1000062fc:     	ldp	x12, x13, [x11, #-0x8]
100006300:     	ldp	x14, x15, [x10, #-0x8]
100006304:     	umulh	x16, x12, x14
100006308:     	mul	x17, x12, x14
10000630c:     	adds	x8, x17, x8
100006310:     	adc	x9, x9, x16
100006314:     	madd	x9, x13, x14, x9
100006318:     	madd	x9, x12, x15, x9
10000631c:     	add	x10, x10, #0x10
100006320:     	add	x11, x11, #0x10
100006324:     	subs	x2, x2, #0x1
100006328:     	b.ne	0x1000062fc <field_regressions::campaign::integer::mac::<2, false>+0x34>
10000632c:     	stp	x8, x9, [x0]
100006330:     	ldp	x29, x30, [sp, #0x10]
100006334:     	add	sp, sp, #0x20
100006338:     	ret
10000633c:     	adrp	x3, 0x1000b8000 <dyld_stub_binder+0x1000b8000>
100006340:     	add	x3, x3, #0x910
100006344:     	mov	x0, sp
100006348:     	add	x1, sp, #0x8
10000634c:     	mov	x2, #0x0                ; =0
100006350:     	bl	0x100088d48 <core::panicking::assert_failed::<usize, usize>>
100006354:     	adrp	x0, 0x100096000 <GCC_except_table992+0x18>
100006358:     	add	x0, x0, #0xfc7
10000635c:     	adrp	x2, 0x1000b8000 <dyld_stub_binder+0x1000b8000>
100006360:     	add	x2, x2, #0x928
100006364:     	mov	w1, #0x24               ; =36
100006368:     	bl	0x100088e64 <core::panicking::panic>

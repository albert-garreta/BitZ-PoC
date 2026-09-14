
/private/var/folders/q0/v7l8dzd13k7f_q6j80sy03lm0000gn/T/f2z-field-gated-h_tggvge/field-regressions:	file format mach-o arm64

Disassembly of section __TEXT,__text:

000000010004c84c <__RINvNvNtNtNtCsfDw8YqMpZPC_17field_regressions8campaign10candidates6binary10polynomial5fixedKj9_KB1s_Kj12_EBa_>:
10004c84c: d102c3ff    	sub	sp, sp, #0xb0
10004c850: a90a7bfd    	stp	x29, x30, [sp, #0xa0]
10004c854: 910283fd    	add	x29, sp, #0xa0
10004c858: a90013e2    	stp	x2, x4, [sp]
10004c85c: eb04005f    	cmp	x2, x4
10004c860: 54000b01    	b.ne	0x10004c9c0 <__RINvNvNtNtNtCsfDw8YqMpZPC_17field_regressions8campaign10candidates6binary10polynomial5fixedKj9_KB1s_Kj12_EBa_+0x174>
10004c864: 6f00e400    	movi.2d	v0, #0000000000000000
10004c868: ad0083e0    	stp	q0, q0, [sp, #0x10]
10004c86c: ad0183e0    	stp	q0, q0, [sp, #0x30]
10004c870: ad0283e0    	stp	q0, q0, [sp, #0x50]
10004c874: ad0383e0    	stp	q0, q0, [sp, #0x70]
10004c878: 3d8027e0    	str	q0, [sp, #0x90]
10004c87c: b4000882    	cbz	x2, 0x10004c98c <__RINvNvNtNtNtCsfDw8YqMpZPC_17field_regressions8campaign10candidates6binary10polynomial5fixedKj9_KB1s_Kj12_EBa_+0x140>
10004c880: d2800008    	mov	x8, #0x0                ; =0
10004c884: 910043e9    	add	x9, sp, #0x10
10004c888: 9100c129    	add	x9, x9, #0x30
10004c88c: 5280090a    	mov	w10, #0x48              ; =72
10004c890: d280000b    	mov	x11, #0x0               ; =0
10004c894: 9b0a0d0c    	madd	x12, x8, x10, x3
10004c898: a942398d    	ldp	x13, x14, [x12, #0x20]
10004c89c: a943418f    	ldp	x15, x16, [x12, #0x30]
10004c8a0: a9401191    	ldp	x17, x4, [x12]
10004c8a4: 9e670220    	fmov	d0, x17
10004c8a8: a9414585    	ldp	x5, x17, [x12, #0x10]
10004c8ac: 9e670081    	fmov	d1, x4
10004c8b0: 9e6700a2    	fmov	d2, x5
10004c8b4: 9e670223    	fmov	d3, x17
10004c8b8: 9e6701a4    	fmov	d4, x13
10004c8bc: 9e6701c5    	fmov	d5, x14
10004c8c0: f940218c    	ldr	x12, [x12, #0x40]
10004c8c4: 9e6701e6    	fmov	d6, x15
10004c8c8: 9e670207    	fmov	d7, x16
10004c8cc: 9e670190    	fmov	d16, x12
10004c8d0: aa0903ec    	mov	x12, x9
10004c8d4: fc6b7831    	ldr	d17, [x1, x11, lsl #3]
10004c8d8: 0ee1e232    	pmull.1q	v18, v17, v1
10004c8dc: 0ee2e233    	pmull.1q	v19, v17, v2
10004c8e0: 0ee3e234    	pmull.1q	v20, v17, v3
10004c8e4: 4ed37a55    	zip2.2d	v21, v18, v19
10004c8e8: 6e180693    	mov.d	v19[1], v20[0]
10004c8ec: ad7f5d96    	ldp	q22, q23, [x12, #-0x20]
10004c8f0: ce154ed3    	eor3.16b	v19, v22, v21, v19
10004c8f4: 0ee4e235    	pmull.1q	v21, v17, v4
10004c8f8: 0ee5e236    	pmull.1q	v22, v17, v5
10004c8fc: 4ed57a94    	zip2.2d	v20, v20, v21
10004c900: 6e1806d5    	mov.d	v21[1], v22[0]
10004c904: ce1456f4    	eor3.16b	v20, v23, v20, v21
10004c908: 0ee6e235    	pmull.1q	v21, v17, v6
10004c90c: 0ee7e237    	pmull.1q	v23, v17, v7
10004c910: 3dc00198    	ldr	q24, [x12]
10004c914: 4ed57ad6    	zip2.2d	v22, v22, v21
10004c918: 6e1806f5    	mov.d	v21[1], v23[0]
10004c91c: ce165715    	eor3.16b	v21, v24, v22, v21
10004c920: 0ee0e236    	pmull.1q	v22, v17, v0
10004c924: 4e183ecd    	mov.d	x13, v22[1]
10004c928: f85d818e    	ldur	x14, [x12, #-0x28]
10004c92c: ca0d01cd    	eor	x13, x14, x13
10004c930: 6e180656    	mov.d	v22[1], v18[0]
10004c934: fc5d0192    	ldur	d18, [x12, #-0x30]
10004c938: 4e181db2    	mov.d	v18[1], x13
10004c93c: 6e361e52    	eor.16b	v18, v18, v22
10004c940: ad3ecd92    	stp	q18, q19, [x12, #-0x30]
10004c944: ad3fd594    	stp	q20, q21, [x12, #-0x10]
10004c948: 4e183eed    	mov.d	x13, v23[1]
10004c94c: f940098e    	ldr	x14, [x12, #0x10]
10004c950: ca0d01cd    	eor	x13, x14, x13
10004c954: 9100618e    	add	x14, x12, #0x18
10004c958: 9e6701b2    	fmov	d18, x13
10004c95c: 4d4085d2    	ld1.d	{ v18 }[1], [x14]
10004c960: 0ef0e231    	pmull.1q	v17, v17, v16
10004c964: 6e311e51    	eor.16b	v17, v18, v17
10004c968: 3d800591    	str	q17, [x12, #0x10]
10004c96c: 9100056b    	add	x11, x11, #0x1
10004c970: 9100218c    	add	x12, x12, #0x8
10004c974: f100257f    	cmp	x11, #0x9
10004c978: 54fffae1    	b.ne	0x10004c8d4 <__RINvNvNtNtNtCsfDw8YqMpZPC_17field_regressions8campaign10candidates6binary10polynomial5fixedKj9_KB1s_Kj12_EBa_+0x88>
10004c97c: 91000508    	add	x8, x8, #0x1
10004c980: 91012021    	add	x1, x1, #0x48
10004c984: eb02011f    	cmp	x8, x2
10004c988: 54fff841    	b.ne	0x10004c890 <__RINvNvNtNtNtCsfDw8YqMpZPC_17field_regressions8campaign10candidates6binary10polynomial5fixedKj9_KB1s_Kj12_EBa_+0x44>
10004c98c: ad4387e0    	ldp	q0, q1, [sp, #0x70]
10004c990: ad030400    	stp	q0, q1, [x0, #0x60]
10004c994: 3dc027e0    	ldr	q0, [sp, #0x90]
10004c998: 3d802000    	str	q0, [x0, #0x80]
10004c99c: ad4187e0    	ldp	q0, q1, [sp, #0x30]
10004c9a0: ad010400    	stp	q0, q1, [x0, #0x20]
10004c9a4: ad4283e1    	ldp	q1, q0, [sp, #0x50]
10004c9a8: ad020001    	stp	q1, q0, [x0, #0x40]
10004c9ac: ad4083e1    	ldp	q1, q0, [sp, #0x10]
10004c9b0: ad000001    	stp	q1, q0, [x0]
10004c9b4: a94a7bfd    	ldp	x29, x30, [sp, #0xa0]
10004c9b8: 9102c3ff    	add	sp, sp, #0xb0
10004c9bc: d65f03c0    	ret
10004c9c0: 90000b24    	adrp	x4, 0x1001b0000 <dyld_stub_binder+0x1001b0000>
10004c9c4: 9132c084    	add	x4, x4, #0xcb0
10004c9c8: 910003e0    	mov	x0, sp
10004c9cc: 910023e1    	add	x1, sp, #0x8
10004c9d0: d2800002    	mov	x2, #0x0                ; =0
10004c9d4: 94043fbc    	bl	0x10015c8c4 <__RINvNtCs8Mbv00yxnRz_4core9panicking13assert_failedjjEB4_>

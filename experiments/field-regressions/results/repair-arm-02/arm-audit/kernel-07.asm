
/private/var/folders/q0/v7l8dzd13k7f_q6j80sy03lm0000gn/T/f2z-field-gated-aejimxw9/field-regressions:	file format mach-o arm64

Disassembly of section __TEXT,__text:

00000001000436c4 <__RINvNvNtNtNtCsaHyC9lX8wyC_17field_regressions8campaign10candidates6binary10polynomial5fixedKj9_KB1s_Kj12_EBa_>:
1000436c4: d102c3ff    	sub	sp, sp, #0xb0
1000436c8: a90a7bfd    	stp	x29, x30, [sp, #0xa0]
1000436cc: 910283fd    	add	x29, sp, #0xa0
1000436d0: a90013e2    	stp	x2, x4, [sp]
1000436d4: eb04005f    	cmp	x2, x4
1000436d8: 54000b01    	b.ne	0x100043838 <__RINvNvNtNtNtCsaHyC9lX8wyC_17field_regressions8campaign10candidates6binary10polynomial5fixedKj9_KB1s_Kj12_EBa_+0x174>
1000436dc: 6f00e400    	movi.2d	v0, #0000000000000000
1000436e0: ad0083e0    	stp	q0, q0, [sp, #0x10]
1000436e4: ad0183e0    	stp	q0, q0, [sp, #0x30]
1000436e8: ad0283e0    	stp	q0, q0, [sp, #0x50]
1000436ec: ad0383e0    	stp	q0, q0, [sp, #0x70]
1000436f0: 3d8027e0    	str	q0, [sp, #0x90]
1000436f4: b4000882    	cbz	x2, 0x100043804 <__RINvNvNtNtNtCsaHyC9lX8wyC_17field_regressions8campaign10candidates6binary10polynomial5fixedKj9_KB1s_Kj12_EBa_+0x140>
1000436f8: d2800008    	mov	x8, #0x0                ; =0
1000436fc: 910043e9    	add	x9, sp, #0x10
100043700: 9100c129    	add	x9, x9, #0x30
100043704: 5280090a    	mov	w10, #0x48              ; =72
100043708: d280000b    	mov	x11, #0x0               ; =0
10004370c: 9b0a0d0c    	madd	x12, x8, x10, x3
100043710: a942398d    	ldp	x13, x14, [x12, #0x20]
100043714: a943418f    	ldp	x15, x16, [x12, #0x30]
100043718: a9401191    	ldp	x17, x4, [x12]
10004371c: 9e670220    	fmov	d0, x17
100043720: a9414585    	ldp	x5, x17, [x12, #0x10]
100043724: 9e670081    	fmov	d1, x4
100043728: 9e6700a2    	fmov	d2, x5
10004372c: 9e670223    	fmov	d3, x17
100043730: 9e6701a4    	fmov	d4, x13
100043734: 9e6701c5    	fmov	d5, x14
100043738: f940218c    	ldr	x12, [x12, #0x40]
10004373c: 9e6701e6    	fmov	d6, x15
100043740: 9e670207    	fmov	d7, x16
100043744: 9e670190    	fmov	d16, x12
100043748: aa0903ec    	mov	x12, x9
10004374c: fc6b7831    	ldr	d17, [x1, x11, lsl #3]
100043750: 0ee1e232    	pmull.1q	v18, v17, v1
100043754: 0ee2e233    	pmull.1q	v19, v17, v2
100043758: 0ee3e234    	pmull.1q	v20, v17, v3
10004375c: 4ed37a55    	zip2.2d	v21, v18, v19
100043760: 6e180693    	mov.d	v19[1], v20[0]
100043764: ad7f5d96    	ldp	q22, q23, [x12, #-0x20]
100043768: ce154ed3    	eor3.16b	v19, v22, v21, v19
10004376c: 0ee4e235    	pmull.1q	v21, v17, v4
100043770: 0ee5e236    	pmull.1q	v22, v17, v5
100043774: 4ed57a94    	zip2.2d	v20, v20, v21
100043778: 6e1806d5    	mov.d	v21[1], v22[0]
10004377c: ce1456f4    	eor3.16b	v20, v23, v20, v21
100043780: 0ee6e235    	pmull.1q	v21, v17, v6
100043784: 0ee7e237    	pmull.1q	v23, v17, v7
100043788: 3dc00198    	ldr	q24, [x12]
10004378c: 4ed57ad6    	zip2.2d	v22, v22, v21
100043790: 6e1806f5    	mov.d	v21[1], v23[0]
100043794: ce165715    	eor3.16b	v21, v24, v22, v21
100043798: 0ee0e236    	pmull.1q	v22, v17, v0
10004379c: 4e183ecd    	mov.d	x13, v22[1]
1000437a0: f85d818e    	ldur	x14, [x12, #-0x28]
1000437a4: ca0d01cd    	eor	x13, x14, x13
1000437a8: 6e180656    	mov.d	v22[1], v18[0]
1000437ac: fc5d0192    	ldur	d18, [x12, #-0x30]
1000437b0: 4e181db2    	mov.d	v18[1], x13
1000437b4: 6e361e52    	eor.16b	v18, v18, v22
1000437b8: ad3ecd92    	stp	q18, q19, [x12, #-0x30]
1000437bc: ad3fd594    	stp	q20, q21, [x12, #-0x10]
1000437c0: 4e183eed    	mov.d	x13, v23[1]
1000437c4: f940098e    	ldr	x14, [x12, #0x10]
1000437c8: ca0d01cd    	eor	x13, x14, x13
1000437cc: 9100618e    	add	x14, x12, #0x18
1000437d0: 9e6701b2    	fmov	d18, x13
1000437d4: 4d4085d2    	ld1.d	{ v18 }[1], [x14]
1000437d8: 0ef0e231    	pmull.1q	v17, v17, v16
1000437dc: 6e311e51    	eor.16b	v17, v18, v17
1000437e0: 3d800591    	str	q17, [x12, #0x10]
1000437e4: 9100056b    	add	x11, x11, #0x1
1000437e8: 9100218c    	add	x12, x12, #0x8
1000437ec: f100257f    	cmp	x11, #0x9
1000437f0: 54fffae1    	b.ne	0x10004374c <__RINvNvNtNtNtCsaHyC9lX8wyC_17field_regressions8campaign10candidates6binary10polynomial5fixedKj9_KB1s_Kj12_EBa_+0x88>
1000437f4: 91000508    	add	x8, x8, #0x1
1000437f8: 91012021    	add	x1, x1, #0x48
1000437fc: eb02011f    	cmp	x8, x2
100043800: 54fff841    	b.ne	0x100043708 <__RINvNvNtNtNtCsaHyC9lX8wyC_17field_regressions8campaign10candidates6binary10polynomial5fixedKj9_KB1s_Kj12_EBa_+0x44>
100043804: ad4387e0    	ldp	q0, q1, [sp, #0x70]
100043808: ad030400    	stp	q0, q1, [x0, #0x60]
10004380c: 3dc027e0    	ldr	q0, [sp, #0x90]
100043810: 3d802000    	str	q0, [x0, #0x80]
100043814: ad4187e0    	ldp	q0, q1, [sp, #0x30]
100043818: ad010400    	stp	q0, q1, [x0, #0x20]
10004381c: ad4283e1    	ldp	q1, q0, [sp, #0x50]
100043820: ad020001    	stp	q1, q0, [x0, #0x40]
100043824: ad4083e1    	ldp	q1, q0, [sp, #0x10]
100043828: ad000001    	stp	q1, q0, [x0]
10004382c: a94a7bfd    	ldp	x29, x30, [sp, #0xa0]
100043830: 9102c3ff    	add	sp, sp, #0xb0
100043834: d65f03c0    	ret
100043838: b0000ac4    	adrp	x4, 0x10019c000 <dyld_stub_binder+0x10019c000>
10004383c: 91166084    	add	x4, x4, #0x598
100043840: 910003e0    	mov	x0, sp
100043844: 910023e1    	add	x1, sp, #0x8
100043848: d2800002    	mov	x2, #0x0                ; =0
10004384c: 94042047    	bl	0x10014b968 <__RINvNtCs8Mbv00yxnRz_4core9panicking13assert_failedjjEB4_>

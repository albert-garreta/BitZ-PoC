
/private/var/folders/q0/v7l8dzd13k7f_q6j80sy03lm0000gn/T/f2z-field-gated-6lkppdze/field-regressions:	file format mach-o arm64

Disassembly of section __TEXT,__text:

0000000100030d70 <__RINvNvNtNtNtCs8IeiyMGxmWr_17field_regressions8campaign10candidates6binary10polynomial5fixedKj9_KB1s_Kj12_EBa_>:
100030d70: d102c3ff    	sub	sp, sp, #0xb0
100030d74: a90a7bfd    	stp	x29, x30, [sp, #0xa0]
100030d78: 910283fd    	add	x29, sp, #0xa0
100030d7c: a90013e2    	stp	x2, x4, [sp]
100030d80: eb04005f    	cmp	x2, x4
100030d84: 54000b01    	b.ne	0x100030ee4 <__RINvNvNtNtNtCs8IeiyMGxmWr_17field_regressions8campaign10candidates6binary10polynomial5fixedKj9_KB1s_Kj12_EBa_+0x174>
100030d88: 6f00e400    	movi.2d	v0, #0000000000000000
100030d8c: ad0083e0    	stp	q0, q0, [sp, #0x10]
100030d90: ad0183e0    	stp	q0, q0, [sp, #0x30]
100030d94: ad0283e0    	stp	q0, q0, [sp, #0x50]
100030d98: ad0383e0    	stp	q0, q0, [sp, #0x70]
100030d9c: 3d8027e0    	str	q0, [sp, #0x90]
100030da0: b4000882    	cbz	x2, 0x100030eb0 <__RINvNvNtNtNtCs8IeiyMGxmWr_17field_regressions8campaign10candidates6binary10polynomial5fixedKj9_KB1s_Kj12_EBa_+0x140>
100030da4: d2800008    	mov	x8, #0x0                ; =0
100030da8: 910043e9    	add	x9, sp, #0x10
100030dac: 9100c129    	add	x9, x9, #0x30
100030db0: 5280090a    	mov	w10, #0x48              ; =72
100030db4: d280000b    	mov	x11, #0x0               ; =0
100030db8: 9b0a0d0c    	madd	x12, x8, x10, x3
100030dbc: a942398d    	ldp	x13, x14, [x12, #0x20]
100030dc0: a943418f    	ldp	x15, x16, [x12, #0x30]
100030dc4: a9401191    	ldp	x17, x4, [x12]
100030dc8: 9e670220    	fmov	d0, x17
100030dcc: a9414585    	ldp	x5, x17, [x12, #0x10]
100030dd0: 9e670081    	fmov	d1, x4
100030dd4: 9e6700a2    	fmov	d2, x5
100030dd8: 9e670223    	fmov	d3, x17
100030ddc: 9e6701a4    	fmov	d4, x13
100030de0: 9e6701c5    	fmov	d5, x14
100030de4: f940218c    	ldr	x12, [x12, #0x40]
100030de8: 9e6701e6    	fmov	d6, x15
100030dec: 9e670207    	fmov	d7, x16
100030df0: 9e670190    	fmov	d16, x12
100030df4: aa0903ec    	mov	x12, x9
100030df8: fc6b7831    	ldr	d17, [x1, x11, lsl #3]
100030dfc: 0ee1e232    	pmull.1q	v18, v17, v1
100030e00: 0ee2e233    	pmull.1q	v19, v17, v2
100030e04: 0ee3e234    	pmull.1q	v20, v17, v3
100030e08: 4ed37a55    	zip2.2d	v21, v18, v19
100030e0c: 6e180693    	mov.d	v19[1], v20[0]
100030e10: ad7f5d96    	ldp	q22, q23, [x12, #-0x20]
100030e14: ce154ed3    	eor3.16b	v19, v22, v21, v19
100030e18: 0ee4e235    	pmull.1q	v21, v17, v4
100030e1c: 0ee5e236    	pmull.1q	v22, v17, v5
100030e20: 4ed57a94    	zip2.2d	v20, v20, v21
100030e24: 6e1806d5    	mov.d	v21[1], v22[0]
100030e28: ce1456f4    	eor3.16b	v20, v23, v20, v21
100030e2c: 0ee6e235    	pmull.1q	v21, v17, v6
100030e30: 0ee7e237    	pmull.1q	v23, v17, v7
100030e34: 3dc00198    	ldr	q24, [x12]
100030e38: 4ed57ad6    	zip2.2d	v22, v22, v21
100030e3c: 6e1806f5    	mov.d	v21[1], v23[0]
100030e40: ce165715    	eor3.16b	v21, v24, v22, v21
100030e44: 0ee0e236    	pmull.1q	v22, v17, v0
100030e48: 4e183ecd    	mov.d	x13, v22[1]
100030e4c: f85d818e    	ldur	x14, [x12, #-0x28]
100030e50: ca0d01cd    	eor	x13, x14, x13
100030e54: 6e180656    	mov.d	v22[1], v18[0]
100030e58: fc5d0192    	ldur	d18, [x12, #-0x30]
100030e5c: 4e181db2    	mov.d	v18[1], x13
100030e60: 6e361e52    	eor.16b	v18, v18, v22
100030e64: ad3ecd92    	stp	q18, q19, [x12, #-0x30]
100030e68: ad3fd594    	stp	q20, q21, [x12, #-0x10]
100030e6c: 4e183eed    	mov.d	x13, v23[1]
100030e70: f940098e    	ldr	x14, [x12, #0x10]
100030e74: ca0d01cd    	eor	x13, x14, x13
100030e78: 9100618e    	add	x14, x12, #0x18
100030e7c: 9e6701b2    	fmov	d18, x13
100030e80: 4d4085d2    	ld1.d	{ v18 }[1], [x14]
100030e84: 0ef0e231    	pmull.1q	v17, v17, v16
100030e88: 6e311e51    	eor.16b	v17, v18, v17
100030e8c: 3d800591    	str	q17, [x12, #0x10]
100030e90: 9100056b    	add	x11, x11, #0x1
100030e94: 9100218c    	add	x12, x12, #0x8
100030e98: f100257f    	cmp	x11, #0x9
100030e9c: 54fffae1    	b.ne	0x100030df8 <__RINvNvNtNtNtCs8IeiyMGxmWr_17field_regressions8campaign10candidates6binary10polynomial5fixedKj9_KB1s_Kj12_EBa_+0x88>
100030ea0: 91000508    	add	x8, x8, #0x1
100030ea4: 91012021    	add	x1, x1, #0x48
100030ea8: eb02011f    	cmp	x8, x2
100030eac: 54fff841    	b.ne	0x100030db4 <__RINvNvNtNtNtCs8IeiyMGxmWr_17field_regressions8campaign10candidates6binary10polynomial5fixedKj9_KB1s_Kj12_EBa_+0x44>
100030eb0: ad4387e0    	ldp	q0, q1, [sp, #0x70]
100030eb4: ad030400    	stp	q0, q1, [x0, #0x60]
100030eb8: 3dc027e0    	ldr	q0, [sp, #0x90]
100030ebc: 3d802000    	str	q0, [x0, #0x80]
100030ec0: ad4187e0    	ldp	q0, q1, [sp, #0x30]
100030ec4: ad010400    	stp	q0, q1, [x0, #0x20]
100030ec8: ad4283e1    	ldp	q1, q0, [sp, #0x50]
100030ecc: ad020001    	stp	q1, q0, [x0, #0x40]
100030ed0: ad4083e1    	ldp	q1, q0, [sp, #0x10]
100030ed4: ad000001    	stp	q1, q0, [x0]
100030ed8: a94a7bfd    	ldp	x29, x30, [sp, #0xa0]
100030edc: 9102c3ff    	add	sp, sp, #0xb0
100030ee0: d65f03c0    	ret
100030ee4: f0000a24    	adrp	x4, 0x100177000 <dyld_stub_binder+0x100177000>
100030ee8: 912f8084    	add	x4, x4, #0xbe0
100030eec: 910003e0    	mov	x0, sp
100030ef0: 910023e1    	add	x1, sp, #0x8
100030ef4: d2800002    	mov	x2, #0x0                ; =0
100030ef8: 9403ee1d    	bl	0x10012c76c <__RINvNtCs8Mbv00yxnRz_4core9panicking13assert_failedjjEB4_>

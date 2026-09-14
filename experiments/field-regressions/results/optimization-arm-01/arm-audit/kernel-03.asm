
/private/var/folders/q0/v7l8dzd13k7f_q6j80sy03lm0000gn/T/f2z-field-gated-6lkppdze/field-regressions:	file format mach-o arm64

Disassembly of section __TEXT,__text:

0000000100030bac <__RINvNvNtNtNtCs8IeiyMGxmWr_17field_regressions8campaign10candidates6binary10polynomial5fixedKj3_Kj7_Kja_EBa_>:
100030bac: d100c3ff    	sub	sp, sp, #0x30
100030bb0: 6d0123e9    	stp	d9, d8, [sp, #0x10]
100030bb4: a9027bfd    	stp	x29, x30, [sp, #0x20]
100030bb8: 910083fd    	add	x29, sp, #0x20
100030bbc: a90013e2    	stp	x2, x4, [sp]
100030bc0: eb04005f    	cmp	x2, x4
100030bc4: 54000ca1    	b.ne	0x100030d58 <__RINvNvNtNtNtCs8IeiyMGxmWr_17field_regressions8campaign10candidates6binary10polynomial5fixedKj3_Kj7_Kja_EBa_+0x1ac>
100030bc8: d2800009    	mov	x9, #0x0                ; =0
100030bcc: d2800008    	mov	x8, #0x0                ; =0
100030bd0: b4000a82    	cbz	x2, 0x100030d20 <__RINvNvNtNtNtCs8IeiyMGxmWr_17field_regressions8campaign10candidates6binary10polynomial5fixedKj3_Kj7_Kja_EBa_+0x174>
100030bd4: 9100402a    	add	x10, x1, #0x10
100030bd8: 6f00e400    	movi.2d	v0, #0000000000000000
100030bdc: 6f00e401    	movi.2d	v1, #0000000000000000
100030be0: 6f00e402    	movi.2d	v2, #0000000000000000
100030be4: 6f00e407    	movi.2d	v7, #0000000000000000
100030be8: 6d401464    	ldp	d4, d5, [x3]
100030bec: 6d7f0d59    	ldp	d25, d3, [x10, #-0x10]
100030bf0: 0ee4e330    	pmull.1q	v16, v25, v4
100030bf4: 0ee4e066    	pmull.1q	v6, v3, v4
100030bf8: 4ec67a12    	zip2.2d	v18, v16, v6
100030bfc: 9e66020b    	fmov	x11, d16
100030c00: ca090169    	eor	x9, x11, x9
100030c04: 0ee5e331    	pmull.1q	v17, v25, v5
100030c08: 4e183e2b    	mov.d	x11, v17[1]
100030c0c: 4e183cec    	mov.d	x12, v7[1]
100030c10: 9e6600ed    	fmov	x13, d7
100030c14: 6d411c70    	ldp	d16, d7, [x3, #0x10]
100030c18: 0ef0e334    	pmull.1q	v20, v25, v16
100030c1c: 0ee7e333    	pmull.1q	v19, v25, v7
100030c20: 4ed37a9b    	zip2.2d	v27, v20, v19
100030c24: 9e66028e    	fmov	x14, d20
100030c28: ca0d01cd    	eor	x13, x14, x13
100030c2c: ca0b01ab    	eor	x11, x13, x11
100030c30: 6d425476    	ldp	d22, d21, [x3, #0x20]
100030c34: 0ef6e338    	pmull.1q	v24, v25, v22
100030c38: 0ef5e337    	pmull.1q	v23, v25, v21
100030c3c: fd401874    	ldr	d20, [x3, #0x30]
100030c40: 0ef4e339    	pmull.1q	v25, v25, v20
100030c44: 4e183f2d    	mov.d	x13, v25[1]
100030c48: ca0d018c    	eor	x12, x12, x13
100030c4c: 0ef5e07a    	pmull.1q	v26, v3, v21
100030c50: 4e183f4d    	mov.d	x13, v26[1]
100030c54: ca0d018c    	eor	x12, x12, x13
100030c58: 9e67019c    	fmov	d28, x12
100030c5c: 6e18445c    	mov.d	v28[1], v2[1]
100030c60: 0ee5e07d    	pmull.1q	v29, v3, v5
100030c64: 4e181d62    	mov.d	v2[1], x11
100030c68: 6e1807b1    	mov.d	v17[1], v29[0]
100030c6c: 6e321c42    	eor.16b	v2, v2, v18
100030c70: 0ef0e072    	pmull.1q	v18, v3, v16
100030c74: 0ee7e07e    	pmull.1q	v30, v3, v7
100030c78: 0ef6e07f    	pmull.1q	v31, v3, v22
100030c7c: fc418548    	ldr	d8, [x10], #0x18
100030c80: 0ee4e109    	pmull.1q	v9, v8, v4
100030c84: 6e180526    	mov.d	v6[1], v9[0]
100030c88: ce111844    	eor3.16b	v4, v2, v17, v6
100030c8c: 0ee5e102    	pmull.1q	v2, v8, v5
100030c90: 6e180713    	mov.d	v19[1], v24[0]
100030c94: ce1b4c00    	eor3.16b	v0, v0, v27, v19
100030c98: 4ed27ba5    	zip2.2d	v5, v29, v18
100030c9c: 6e1807d2    	mov.d	v18[1], v30[0]
100030ca0: ce054800    	eor3.16b	v0, v0, v5, v18
100030ca4: 4ec27925    	zip2.2d	v5, v9, v2
100030ca8: 0ef0e106    	pmull.1q	v6, v8, v16
100030cac: 6e1804c2    	mov.d	v2[1], v6[0]
100030cb0: ce050800    	eor3.16b	v0, v0, v5, v2
100030cb4: 0ee7e102    	pmull.1q	v2, v8, v7
100030cb8: 4ed77b05    	zip2.2d	v5, v24, v23
100030cbc: 6e180737    	mov.d	v23[1], v25[0]
100030cc0: ce055c21    	eor3.16b	v1, v1, v5, v23
100030cc4: 4edf7bc5    	zip2.2d	v5, v30, v31
100030cc8: 6e18075f    	mov.d	v31[1], v26[0]
100030ccc: ce057c21    	eor3.16b	v1, v1, v5, v31
100030cd0: 4ec278c5    	zip2.2d	v5, v6, v2
100030cd4: 0ef6e106    	pmull.1q	v6, v8, v22
100030cd8: 6e1804c2    	mov.d	v2[1], v6[0]
100030cdc: ce050821    	eor3.16b	v1, v1, v5, v2
100030ce0: 0ef5e102    	pmull.1q	v2, v8, v21
100030ce4: 4ec278c5    	zip2.2d	v5, v6, v2
100030ce8: 0ef4e106    	pmull.1q	v6, v8, v20
100030cec: 6e1804c2    	mov.d	v2[1], v6[0]
100030cf0: 0ef4e063    	pmull.1q	v3, v3, v20
100030cf4: 6e231f83    	eor.16b	v3, v28, v3
100030cf8: ce050863    	eor3.16b	v3, v3, v5, v2
100030cfc: 4e183ccb    	mov.d	x11, v6[1]
100030d00: ca0b0108    	eor	x8, x8, x11
100030d04: 6e034087    	ext.16b	v7, v4, v3, #0x8
100030d08: 4ea41c82    	mov.16b	v2, v4
100030d0c: 6e184462    	mov.d	v2[1], v3[1]
100030d10: 9100e063    	add	x3, x3, #0x38
100030d14: f1000442    	subs	x2, x2, #0x1
100030d18: 54fff681    	b.ne	0x100030be8 <__RINvNvNtNtNtCs8IeiyMGxmWr_17field_regressions8campaign10candidates6binary10polynomial5fixedKj3_Kj7_Kja_EBa_+0x3c>
100030d1c: 14000005    	b	0x100030d30 <__RINvNvNtNtNtCs8IeiyMGxmWr_17field_regressions8campaign10candidates6binary10polynomial5fixedKj3_Kj7_Kja_EBa_+0x184>
100030d20: 6f00e404    	movi.2d	v4, #0000000000000000
100030d24: 6f00e400    	movi.2d	v0, #0000000000000000
100030d28: 6f00e401    	movi.2d	v1, #0000000000000000
100030d2c: 6f00e403    	movi.2d	v3, #0000000000000000
100030d30: f9000009    	str	x9, [x0]
100030d34: 3c808004    	stur	q4, [x0, #0x8]
100030d38: 3c818000    	stur	q0, [x0, #0x18]
100030d3c: 3c828001    	stur	q1, [x0, #0x28]
100030d40: 3c838003    	stur	q3, [x0, #0x38]
100030d44: f9002408    	str	x8, [x0, #0x48]
100030d48: a9427bfd    	ldp	x29, x30, [sp, #0x20]
100030d4c: 6d4123e9    	ldp	d9, d8, [sp, #0x10]
100030d50: 9100c3ff    	add	sp, sp, #0x30
100030d54: d65f03c0    	ret
100030d58: f0000a24    	adrp	x4, 0x100177000 <dyld_stub_binder+0x100177000>
100030d5c: 912f8084    	add	x4, x4, #0xbe0
100030d60: 910003e0    	mov	x0, sp
100030d64: 910023e1    	add	x1, sp, #0x8
100030d68: d2800002    	mov	x2, #0x0                ; =0
100030d6c: 9403ee80    	bl	0x10012c76c <__RINvNtCs8Mbv00yxnRz_4core9panicking13assert_failedjjEB4_>

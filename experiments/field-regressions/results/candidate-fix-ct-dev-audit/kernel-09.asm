
/private/tmp/f2z-arithmetic-target/release/field-regressions:	file format mach-o arm64

Disassembly of section __TEXT,__text:

000000010004b978 <__RINvNvNtNtNtCshgIkoLp8A2V_17field_regressions8campaign10candidates6binary10polynomial5fixedKj3_Kj7_Kja_EBa_>:
10004b978: d100c3ff    	sub	sp, sp, #0x30
10004b97c: 6d0123e9    	stp	d9, d8, [sp, #0x10]
10004b980: a9027bfd    	stp	x29, x30, [sp, #0x20]
10004b984: 910083fd    	add	x29, sp, #0x20
10004b988: a90013e2    	stp	x2, x4, [sp]
10004b98c: eb04005f    	cmp	x2, x4
10004b990: 54000ca1    	b.ne	0x10004bb24 <__RINvNvNtNtNtCshgIkoLp8A2V_17field_regressions8campaign10candidates6binary10polynomial5fixedKj3_Kj7_Kja_EBa_+0x1ac>
10004b994: d2800009    	mov	x9, #0x0                ; =0
10004b998: d2800008    	mov	x8, #0x0                ; =0
10004b99c: b4000a82    	cbz	x2, 0x10004baec <__RINvNvNtNtNtCshgIkoLp8A2V_17field_regressions8campaign10candidates6binary10polynomial5fixedKj3_Kj7_Kja_EBa_+0x174>
10004b9a0: 9100402a    	add	x10, x1, #0x10
10004b9a4: 6f00e400    	movi.2d	v0, #0000000000000000
10004b9a8: 6f00e401    	movi.2d	v1, #0000000000000000
10004b9ac: 6f00e402    	movi.2d	v2, #0000000000000000
10004b9b0: 6f00e407    	movi.2d	v7, #0000000000000000
10004b9b4: 6d401464    	ldp	d4, d5, [x3]
10004b9b8: 6d7f0d59    	ldp	d25, d3, [x10, #-0x10]
10004b9bc: 0ee4e330    	pmull.1q	v16, v25, v4
10004b9c0: 0ee4e066    	pmull.1q	v6, v3, v4
10004b9c4: 4ec67a12    	zip2.2d	v18, v16, v6
10004b9c8: 9e66020b    	fmov	x11, d16
10004b9cc: ca090169    	eor	x9, x11, x9
10004b9d0: 0ee5e331    	pmull.1q	v17, v25, v5
10004b9d4: 4e183e2b    	mov.d	x11, v17[1]
10004b9d8: 4e183cec    	mov.d	x12, v7[1]
10004b9dc: 9e6600ed    	fmov	x13, d7
10004b9e0: 6d411c70    	ldp	d16, d7, [x3, #0x10]
10004b9e4: 0ef0e334    	pmull.1q	v20, v25, v16
10004b9e8: 0ee7e333    	pmull.1q	v19, v25, v7
10004b9ec: 4ed37a9b    	zip2.2d	v27, v20, v19
10004b9f0: 9e66028e    	fmov	x14, d20
10004b9f4: ca0d01cd    	eor	x13, x14, x13
10004b9f8: ca0b01ab    	eor	x11, x13, x11
10004b9fc: 6d425476    	ldp	d22, d21, [x3, #0x20]
10004ba00: 0ef6e338    	pmull.1q	v24, v25, v22
10004ba04: 0ef5e337    	pmull.1q	v23, v25, v21
10004ba08: fd401874    	ldr	d20, [x3, #0x30]
10004ba0c: 0ef4e339    	pmull.1q	v25, v25, v20
10004ba10: 4e183f2d    	mov.d	x13, v25[1]
10004ba14: ca0d018c    	eor	x12, x12, x13
10004ba18: 0ef5e07a    	pmull.1q	v26, v3, v21
10004ba1c: 4e183f4d    	mov.d	x13, v26[1]
10004ba20: ca0d018c    	eor	x12, x12, x13
10004ba24: 9e67019c    	fmov	d28, x12
10004ba28: 6e18445c    	mov.d	v28[1], v2[1]
10004ba2c: 0ee5e07d    	pmull.1q	v29, v3, v5
10004ba30: 4e181d62    	mov.d	v2[1], x11
10004ba34: 6e1807b1    	mov.d	v17[1], v29[0]
10004ba38: 6e321c42    	eor.16b	v2, v2, v18
10004ba3c: 0ef0e072    	pmull.1q	v18, v3, v16
10004ba40: 0ee7e07e    	pmull.1q	v30, v3, v7
10004ba44: 0ef6e07f    	pmull.1q	v31, v3, v22
10004ba48: fc418548    	ldr	d8, [x10], #0x18
10004ba4c: 0ee4e109    	pmull.1q	v9, v8, v4
10004ba50: 6e180526    	mov.d	v6[1], v9[0]
10004ba54: ce111844    	eor3.16b	v4, v2, v17, v6
10004ba58: 0ee5e102    	pmull.1q	v2, v8, v5
10004ba5c: 6e180713    	mov.d	v19[1], v24[0]
10004ba60: ce1b4c00    	eor3.16b	v0, v0, v27, v19
10004ba64: 4ed27ba5    	zip2.2d	v5, v29, v18
10004ba68: 6e1807d2    	mov.d	v18[1], v30[0]
10004ba6c: ce054800    	eor3.16b	v0, v0, v5, v18
10004ba70: 4ec27925    	zip2.2d	v5, v9, v2
10004ba74: 0ef0e106    	pmull.1q	v6, v8, v16
10004ba78: 6e1804c2    	mov.d	v2[1], v6[0]
10004ba7c: ce050800    	eor3.16b	v0, v0, v5, v2
10004ba80: 0ee7e102    	pmull.1q	v2, v8, v7
10004ba84: 4ed77b05    	zip2.2d	v5, v24, v23
10004ba88: 6e180737    	mov.d	v23[1], v25[0]
10004ba8c: ce055c21    	eor3.16b	v1, v1, v5, v23
10004ba90: 4edf7bc5    	zip2.2d	v5, v30, v31
10004ba94: 6e18075f    	mov.d	v31[1], v26[0]
10004ba98: ce057c21    	eor3.16b	v1, v1, v5, v31
10004ba9c: 4ec278c5    	zip2.2d	v5, v6, v2
10004baa0: 0ef6e106    	pmull.1q	v6, v8, v22
10004baa4: 6e1804c2    	mov.d	v2[1], v6[0]
10004baa8: ce050821    	eor3.16b	v1, v1, v5, v2
10004baac: 0ef5e102    	pmull.1q	v2, v8, v21
10004bab0: 4ec278c5    	zip2.2d	v5, v6, v2
10004bab4: 0ef4e106    	pmull.1q	v6, v8, v20
10004bab8: 6e1804c2    	mov.d	v2[1], v6[0]
10004babc: 0ef4e063    	pmull.1q	v3, v3, v20
10004bac0: 6e231f83    	eor.16b	v3, v28, v3
10004bac4: ce050863    	eor3.16b	v3, v3, v5, v2
10004bac8: 4e183ccb    	mov.d	x11, v6[1]
10004bacc: ca0b0108    	eor	x8, x8, x11
10004bad0: 6e034087    	ext.16b	v7, v4, v3, #0x8
10004bad4: 4ea41c82    	mov.16b	v2, v4
10004bad8: 6e184462    	mov.d	v2[1], v3[1]
10004badc: 9100e063    	add	x3, x3, #0x38
10004bae0: f1000442    	subs	x2, x2, #0x1
10004bae4: 54fff681    	b.ne	0x10004b9b4 <__RINvNvNtNtNtCshgIkoLp8A2V_17field_regressions8campaign10candidates6binary10polynomial5fixedKj3_Kj7_Kja_EBa_+0x3c>
10004bae8: 14000005    	b	0x10004bafc <__RINvNvNtNtNtCshgIkoLp8A2V_17field_regressions8campaign10candidates6binary10polynomial5fixedKj3_Kj7_Kja_EBa_+0x184>
10004baec: 6f00e404    	movi.2d	v4, #0000000000000000
10004baf0: 6f00e400    	movi.2d	v0, #0000000000000000
10004baf4: 6f00e401    	movi.2d	v1, #0000000000000000
10004baf8: 6f00e403    	movi.2d	v3, #0000000000000000
10004bafc: f9000009    	str	x9, [x0]
10004bb00: 3c808004    	stur	q4, [x0, #0x8]
10004bb04: 3c818000    	stur	q0, [x0, #0x18]
10004bb08: 3c828001    	stur	q1, [x0, #0x28]
10004bb0c: 3c838003    	stur	q3, [x0, #0x38]
10004bb10: f9002408    	str	x8, [x0, #0x48]
10004bb14: a9427bfd    	ldp	x29, x30, [sp, #0x20]
10004bb18: 6d4123e9    	ldp	d9, d8, [sp, #0x10]
10004bb1c: 9100c3ff    	add	sp, sp, #0x30
10004bb20: d65f03c0    	ret
10004bb24: b0000b24    	adrp	x4, 0x1001b0000 <dyld_stub_binder+0x1001b0000>
10004bb28: 912fc084    	add	x4, x4, #0xbf0
10004bb2c: 910003e0    	mov	x0, sp
10004bb30: 910023e1    	add	x1, sp, #0x8
10004bb34: d2800002    	mov	x2, #0x0                ; =0
10004bb38: 94043ecf    	bl	0x10015b674 <__RINvNtCs8Mbv00yxnRz_4core9panicking13assert_failedjjEB4_>

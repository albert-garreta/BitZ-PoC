
/private/var/folders/q0/v7l8dzd13k7f_q6j80sy03lm0000gn/T/f2z-field-gated-aejimxw9/field-regressions:	file format mach-o arm64

Disassembly of section __TEXT,__text:

00000001000d3960 <__RNvNtNtNtCsaHyC9lX8wyC_17field_regressions8campaign10candidates9composite11serial_tree>:
1000d3960: d101c3ff    	sub	sp, sp, #0x70
1000d3964: a9016ffc    	stp	x28, x27, [sp, #0x10]
1000d3968: a90267fa    	stp	x26, x25, [sp, #0x20]
1000d396c: a9035ff8    	stp	x24, x23, [sp, #0x30]
1000d3970: a90457f6    	stp	x22, x21, [sp, #0x40]
1000d3974: a9054ff4    	stp	x20, x19, [sp, #0x50]
1000d3978: a9067bfd    	stp	x29, x30, [sp, #0x60]
1000d397c: 910183fd    	add	x29, sp, #0x60
1000d3980: aa0503f7    	mov	x23, x5
1000d3984: aa0403f3    	mov	x19, x4
1000d3988: aa0303f5    	mov	x21, x3
1000d398c: aa0203f6    	mov	x22, x2
1000d3990: aa0003f8    	mov	x24, x0
1000d3994: a940d019    	ldp	x25, x20, [x0, #0x8]
1000d3998: 5280031a    	mov	w26, #0x18              ; =24
1000d399c: 528010fb    	mov	w27, #0x87              ; =135
1000d39a0: 4e080f70    	dup.2d	v16, x27
1000d39a4: 6f00e405    	movi.2d	v5, #0000000000000000
1000d39a8: 3d8003f0    	str	q16, [sp]
1000d39ac: 1400000d    	b	0x1000d39e0 <__RNvNtNtNtCsaHyC9lX8wyC_17field_regressions8campaign10candidates9composite11serial_tree+0x80>
1000d39b0: 91000673    	add	x19, x19, #0x1
1000d39b4: d37ffae5    	lsl	x5, x23, #1
1000d39b8: aa1803e0    	mov	x0, x24
1000d39bc: aa1503e3    	mov	x3, x21
1000d39c0: aa1303e4    	mov	x4, x19
1000d39c4: 97ffffe7    	bl	0x1000d3960 <__RNvNtNtNtCsaHyC9lX8wyC_17field_regressions8campaign10candidates9composite11serial_tree>
1000d39c8: 6f00e405    	movi.2d	v5, #0000000000000000
1000d39cc: 3dc003f0    	ldr	q16, [sp]
1000d39d0: 52800028    	mov	w8, #0x1                ; =1
1000d39d4: b37ffae8    	bfi	x8, x23, #1, #63
1000d39d8: aa1c03e1    	mov	x1, x28
1000d39dc: aa0803f7    	mov	x23, x8
1000d39e0: eb1502df    	cmp	x22, x21
1000d39e4: 540012c0    	b.eq	0x1000d3c3c <__RNvNtNtNtCsaHyC9lX8wyC_17field_regressions8campaign10candidates9composite11serial_tree+0x2dc>
1000d39e8: aa3303e8    	mvn	x8, x19
1000d39ec: ab080280    	adds	x0, x20, x8
1000d39f0: 54001423    	b.lo	0x1000d3c74 <__RNvNtNtNtCsaHyC9lX8wyC_17field_regressions8campaign10candidates9composite11serial_tree+0x314>
1000d39f4: 9b1a640b    	madd	x11, x0, x26, x25
1000d39f8: f9400968    	ldr	x8, [x11, #0x10]
1000d39fc: b4001308    	cbz	x8, 0x1000d3c5c <__RNvNtNtNtCsaHyC9lX8wyC_17field_regressions8campaign10candidates9composite11serial_tree+0x2fc>
1000d3a00: d280000a    	mov	x10, #0x0               ; =0
1000d3a04: f100050c    	subs	x12, x8, #0x1
1000d3a08: 54000220    	b.eq	0x1000d3a4c <__RNvNtNtNtCsaHyC9lX8wyC_17field_regressions8campaign10candidates9composite11serial_tree+0xec>
1000d3a0c: d2800008    	mov	x8, #0x0                ; =0
1000d3a10: d2800009    	mov	x9, #0x0                ; =0
1000d3a14: f940056d    	ldr	x13, [x11, #0x8]
1000d3a18: d37ced8b    	lsl	x11, x12, #4
1000d3a1c: 910061ac    	add	x12, x13, #0x18
1000d3a20: 14000005    	b	0x1000d3a34 <__RNvNtNtNtCsaHyC9lX8wyC_17field_regressions8campaign10candidates9composite11serial_tree+0xd4>
1000d3a24: 9100418c    	add	x12, x12, #0x10
1000d3a28: 91000508    	add	x8, x8, #0x1
1000d3a2c: f100416b    	subs	x11, x11, #0x10
1000d3a30: 54000100    	b.eq	0x1000d3a50 <__RNvNtNtNtCsaHyC9lX8wyC_17field_regressions8campaign10candidates9composite11serial_tree+0xf0>
1000d3a34: 9ac826ed    	lsr	x13, x23, x8
1000d3a38: 3607ff6d    	tbz	w13, #0x0, 0x1000d3a24 <__RNvNtNtNtCsaHyC9lX8wyC_17field_regressions8campaign10candidates9composite11serial_tree+0xc4>
1000d3a3c: a97fb58e    	ldp	x14, x13, [x12, #-0x8]
1000d3a40: ca0901c9    	eor	x9, x14, x9
1000d3a44: ca0a01aa    	eor	x10, x13, x10
1000d3a48: 17fffff7    	b	0x1000d3a24 <__RNvNtNtNtCsaHyC9lX8wyC_17field_regressions8campaign10candidates9composite11serial_tree+0xc4>
1000d3a4c: d2800009    	mov	x9, #0x0                ; =0
1000d3a50: d341fec2    	lsr	x2, x22, #1
1000d3a54: 8b02103c    	add	x28, x1, x2, lsl #4
1000d3a58: cb0202d6    	sub	x22, x22, x2
1000d3a5c: eb0202df    	cmp	x22, x2
1000d3a60: 9a8232c8    	csel	x8, x22, x2, lo
1000d3a64: aa0a012b    	orr	x11, x9, x10
1000d3a68: b500020b    	cbnz	x11, 0x1000d3aa8 <__RNvNtNtNtCsaHyC9lX8wyC_17field_regressions8campaign10candidates9composite11serial_tree+0x148>
1000d3a6c: b4fffa28    	cbz	x8, 0x1000d39b0 <__RNvNtNtNtCsaHyC9lX8wyC_17field_regressions8campaign10candidates9composite11serial_tree+0x50>
1000d3a70: f100211f    	cmp	x8, #0x8
1000d3a74: 540008c2    	b.hs	0x1000d3b8c <__RNvNtNtNtCsaHyC9lX8wyC_17field_regressions8campaign10candidates9composite11serial_tree+0x22c>
1000d3a78: d2800009    	mov	x9, #0x0                ; =0
1000d3a7c: cb090108    	sub	x8, x8, x9
1000d3a80: d37cec4a    	lsl	x10, x2, #4
1000d3a84: 8b091029    	add	x9, x1, x9, lsl #4
1000d3a88: 3cea6920    	ldr	q0, [x9, x10]
1000d3a8c: 3dc00121    	ldr	q1, [x9]
1000d3a90: 6e211c00    	eor.16b	v0, v0, v1
1000d3a94: 3caa6920    	str	q0, [x9, x10]
1000d3a98: 91004129    	add	x9, x9, #0x10
1000d3a9c: f1000508    	subs	x8, x8, #0x1
1000d3aa0: 54ffff41    	b.ne	0x1000d3a88 <__RNvNtNtNtCsaHyC9lX8wyC_17field_regressions8campaign10candidates9composite11serial_tree+0x128>
1000d3aa4: 17ffffc3    	b	0x1000d39b0 <__RNvNtNtNtCsaHyC9lX8wyC_17field_regressions8campaign10candidates9composite11serial_tree+0x50>
1000d3aa8: b400048a    	cbz	x10, 0x1000d3b38 <__RNvNtNtNtCsaHyC9lX8wyC_17field_regressions8campaign10candidates9composite11serial_tree+0x1d8>
1000d3aac: b4fff828    	cbz	x8, 0x1000d39b0 <__RNvNtNtNtCsaHyC9lX8wyC_17field_regressions8campaign10candidates9composite11serial_tree+0x50>
1000d3ab0: 9e670140    	fmov	d0, x10
1000d3ab4: 9e670361    	fmov	d1, x27
1000d3ab8: 0ee1e001    	pmull.1q	v1, v0, v1
1000d3abc: 4e183c2a    	mov.d	x10, v1[1]
1000d3ac0: ca09014b    	eor	x11, x10, x9
1000d3ac4: d37cec4a    	lsl	x10, x2, #4
1000d3ac8: 9e670122    	fmov	d2, x9
1000d3acc: 9e670163    	fmov	d3, x11
1000d3ad0: aa0103e9    	mov	x9, x1
1000d3ad4: 8b0a012b    	add	x11, x9, x10
1000d3ad8: 6d401564    	ldp	d4, d5, [x11]
1000d3adc: 0ee2e086    	pmull.1q	v6, v4, v2
1000d3ae0: 0ee1e0a7    	pmull.1q	v7, v5, v1
1000d3ae4: 0ee0e084    	pmull.1q	v4, v4, v0
1000d3ae8: 0ee3e0a5    	pmull.1q	v5, v5, v3
1000d3aec: 6e241ca4    	eor.16b	v4, v5, v4
1000d3af0: 4e080f65    	dup.2d	v5, x27
1000d3af4: 4ee5e085    	pmull2.1q	v5, v4, v5
1000d3af8: ce0614e5    	eor3.16b	v5, v7, v6, v5
1000d3afc: 4e183cac    	mov.d	x12, v5[1]
1000d3b00: 9e6600ad    	fmov	x13, d5
1000d3b04: a9403d2e    	ldp	x14, x15, [x9]
1000d3b08: ca0d01cd    	eor	x13, x14, x13
1000d3b0c: 9e66008e    	fmov	x14, d4
1000d3b10: ca0e01ee    	eor	x14, x15, x14
1000d3b14: ca0c01cc    	eor	x12, x14, x12
1000d3b18: a881312d    	stp	x13, x12, [x9], #0x10
1000d3b1c: a9403d6e    	ldp	x14, x15, [x11]
1000d3b20: ca0e01ad    	eor	x13, x13, x14
1000d3b24: ca0c01ec    	eor	x12, x15, x12
1000d3b28: a900316d    	stp	x13, x12, [x11]
1000d3b2c: f1000508    	subs	x8, x8, #0x1
1000d3b30: 54fffd21    	b.ne	0x1000d3ad4 <__RNvNtNtNtCsaHyC9lX8wyC_17field_regressions8campaign10candidates9composite11serial_tree+0x174>
1000d3b34: 17ffff9f    	b	0x1000d39b0 <__RNvNtNtNtCsaHyC9lX8wyC_17field_regressions8campaign10candidates9composite11serial_tree+0x50>
1000d3b38: b4fff3c8    	cbz	x8, 0x1000d39b0 <__RNvNtNtNtCsaHyC9lX8wyC_17field_regressions8campaign10candidates9composite11serial_tree+0x50>
1000d3b3c: d37cec4a    	lsl	x10, x2, #4
1000d3b40: 9e670120    	fmov	d0, x9
1000d3b44: aa0103e9    	mov	x9, x1
1000d3b48: 8b0a012b    	add	x11, x9, x10
1000d3b4c: 6d400961    	ldp	d1, d2, [x11]
1000d3b50: 0ee0e042    	pmull.1q	v2, v2, v0
1000d3b54: 4ef0e043    	pmull2.1q	v3, v2, v16
1000d3b58: 6e0240a2    	ext.16b	v2, v5, v2, #0x8
1000d3b5c: 0ee0e021    	pmull.1q	v1, v1, v0
1000d3b60: 6e211c44    	eor.16b	v4, v2, v1
1000d3b64: ce010c41    	eor3.16b	v1, v2, v1, v3
1000d3b68: 3dc00122    	ldr	q2, [x9]
1000d3b6c: ce030883    	eor3.16b	v3, v4, v3, v2
1000d3b70: 3c810523    	str	q3, [x9], #0x10
1000d3b74: 3dc00163    	ldr	q3, [x11]
1000d3b78: ce020c21    	eor3.16b	v1, v1, v2, v3
1000d3b7c: 3d800161    	str	q1, [x11]
1000d3b80: f1000508    	subs	x8, x8, #0x1
1000d3b84: 54fffe21    	b.ne	0x1000d3b48 <__RNvNtNtNtCsaHyC9lX8wyC_17field_regressions8campaign10candidates9composite11serial_tree+0x1e8>
1000d3b88: 17ffff8a    	b	0x1000d39b0 <__RNvNtNtNtCsaHyC9lX8wyC_17field_regressions8campaign10candidates9composite11serial_tree+0x50>
1000d3b8c: d2800009    	mov	x9, #0x0                ; =0
1000d3b90: d100202b    	sub	x11, x1, #0x8
1000d3b94: d37ced0c    	lsl	x12, x8, #4
1000d3b98: d37cec4a    	lsl	x10, x2, #4
1000d3b9c: 8b0a018d    	add	x13, x12, x10
1000d3ba0: 8b0d016f    	add	x15, x11, x13
1000d3ba4: 9100202e    	add	x14, x1, #0x8
1000d3ba8: 8b0d0030    	add	x16, x1, x13
1000d3bac: 8b0c016d    	add	x13, x11, x12
1000d3bb0: eb0d039f    	cmp	x28, x13
1000d3bb4: fa4f3022    	ccmp	x1, x15, #0x2, lo
1000d3bb8: 1a9f27eb    	cset	w11, lo
1000d3bbc: 8b0c0031    	add	x17, x1, x12
1000d3bc0: eb11039f    	cmp	x28, x17
1000d3bc4: fa4f31c2    	ccmp	x14, x15, #0x2, lo
1000d3bc8: 1a9f27ec    	cset	w12, lo
1000d3bcc: 8b0a01c0    	add	x0, x14, x10
1000d3bd0: eb0d001f    	cmp	x0, x13
1000d3bd4: fa503022    	ccmp	x1, x16, #0x2, lo
1000d3bd8: 1a9f27ed    	cset	w13, lo
1000d3bdc: eb11001f    	cmp	x0, x17
1000d3be0: fa5031c2    	ccmp	x14, x16, #0x2, lo
1000d3be4: 1a9f27ee    	cset	w14, lo
1000d3be8: eb0f001f    	cmp	x0, x15
1000d3bec: fa503382    	ccmp	x28, x16, #0x2, lo
1000d3bf0: 54fff463    	b.lo	0x1000d3a7c <__RNvNtNtNtCsaHyC9lX8wyC_17field_regressions8campaign10candidates9composite11serial_tree+0x11c>
1000d3bf4: 3707f44b    	tbnz	w11, #0x0, 0x1000d3a7c <__RNvNtNtNtCsaHyC9lX8wyC_17field_regressions8campaign10candidates9composite11serial_tree+0x11c>
1000d3bf8: 3707f42c    	tbnz	w12, #0x0, 0x1000d3a7c <__RNvNtNtNtCsaHyC9lX8wyC_17field_regressions8campaign10candidates9composite11serial_tree+0x11c>
1000d3bfc: 3707f40d    	tbnz	w13, #0x0, 0x1000d3a7c <__RNvNtNtNtCsaHyC9lX8wyC_17field_regressions8campaign10candidates9composite11serial_tree+0x11c>
1000d3c00: 3707f3ee    	tbnz	w14, #0x0, 0x1000d3a7c <__RNvNtNtNtCsaHyC9lX8wyC_17field_regressions8campaign10candidates9composite11serial_tree+0x11c>
1000d3c04: 927ef109    	and	x9, x8, #0x7ffffffffffffffc
1000d3c08: 927ef10b    	and	x11, x8, #0x7ffffffffffffffc
1000d3c0c: aa0103ec    	mov	x12, x1
1000d3c10: 8b0a018d    	add	x13, x12, x10
1000d3c14: acc10580    	ldp	q0, q1, [x12], #0x20
1000d3c18: ad400da2    	ldp	q2, q3, [x13]
1000d3c1c: 6e201c40    	eor.16b	v0, v2, v0
1000d3c20: 6e211c61    	eor.16b	v1, v3, v1
1000d3c24: ad0005a0    	stp	q0, q1, [x13]
1000d3c28: f100096b    	subs	x11, x11, #0x2
1000d3c2c: 54ffff21    	b.ne	0x1000d3c10 <__RNvNtNtNtCsaHyC9lX8wyC_17field_regressions8campaign10candidates9composite11serial_tree+0x2b0>
1000d3c30: eb09011f    	cmp	x8, x9
1000d3c34: 54ffebe0    	b.eq	0x1000d39b0 <__RNvNtNtNtCsaHyC9lX8wyC_17field_regressions8campaign10candidates9composite11serial_tree+0x50>
1000d3c38: 17ffff91    	b	0x1000d3a7c <__RNvNtNtNtCsaHyC9lX8wyC_17field_regressions8campaign10candidates9composite11serial_tree+0x11c>
1000d3c3c: a9467bfd    	ldp	x29, x30, [sp, #0x60]
1000d3c40: a9454ff4    	ldp	x20, x19, [sp, #0x50]
1000d3c44: a94457f6    	ldp	x22, x21, [sp, #0x40]
1000d3c48: a9435ff8    	ldp	x24, x23, [sp, #0x30]
1000d3c4c: a94267fa    	ldp	x26, x25, [sp, #0x20]
1000d3c50: a9416ffc    	ldp	x28, x27, [sp, #0x10]
1000d3c54: 9101c3ff    	add	sp, sp, #0x70
1000d3c58: d65f03c0    	ret
1000d3c5c: b0000663    	adrp	x3, 0x1001a0000 <dyld_stub_binder+0x1001a0000>
1000d3c60: 91248063    	add	x3, x3, #0x920
1000d3c64: 52800020    	mov	w0, #0x1                ; =1
1000d3c68: d2800001    	mov	x1, #0x0                ; =0
1000d3c6c: d2800002    	mov	x2, #0x0                ; =0
1000d3c70: 9401df55    	bl	0x10014b9c4 <__RNvNtNtCs8Mbv00yxnRz_4core5slice5index16slice_index_fail>
1000d3c74: b0000662    	adrp	x2, 0x1001a0000 <dyld_stub_binder+0x1001a0000>
1000d3c78: 91242042    	add	x2, x2, #0x908
1000d3c7c: aa1403e1    	mov	x1, x20
1000d3c80: 9401df2c    	bl	0x10014b930 <__RNvNtCs8Mbv00yxnRz_4core9panicking18panic_bounds_check>


/private/var/folders/q0/v7l8dzd13k7f_q6j80sy03lm0000gn/T/f2z-field-gated-aejimxw9/field-regressions:	file format mach-o arm64

Disassembly of section __TEXT,__text:

00000001000d28fc <__RNvNtNtNtCsaHyC9lX8wyC_17field_regressions8campaign10candidates9composite10half_depth>:
1000d28fc: d10383ff    	sub	sp, sp, #0xe0
1000d2900: a9086ffc    	stp	x28, x27, [sp, #0x80]
1000d2904: a90967fa    	stp	x26, x25, [sp, #0x90]
1000d2908: a90a5ff8    	stp	x24, x23, [sp, #0xa0]
1000d290c: a90b57f6    	stp	x22, x21, [sp, #0xb0]
1000d2910: a90c4ff4    	stp	x20, x19, [sp, #0xc0]
1000d2914: a90d7bfd    	stp	x29, x30, [sp, #0xd0]
1000d2918: 910343fd    	add	x29, sp, #0xd0
1000d291c: a90093e3    	stp	x3, x4, [sp, #0x8]
1000d2920: f9000fe5    	str	x5, [sp, #0x18]
1000d2924: eb03005f    	cmp	x2, x3
1000d2928: 54000121    	b.ne	0x1000d294c <__RNvNtNtNtCsaHyC9lX8wyC_17field_regressions8campaign10candidates9composite10half_depth+0x50>
1000d292c: a94d7bfd    	ldp	x29, x30, [sp, #0xd0]
1000d2930: a94c4ff4    	ldp	x20, x19, [sp, #0xc0]
1000d2934: a94b57f6    	ldp	x22, x21, [sp, #0xb0]
1000d2938: a94a5ff8    	ldp	x24, x23, [sp, #0xa0]
1000d293c: a94967fa    	ldp	x26, x25, [sp, #0x90]
1000d2940: a9486ffc    	ldp	x28, x27, [sp, #0x80]
1000d2944: 910383ff    	add	sp, sp, #0xe0
1000d2948: d65f03c0    	ret
1000d294c: f940080a    	ldr	x10, [x0, #0x10]
1000d2950: aa2403e9    	mvn	x9, x4
1000d2954: ab090149    	adds	x9, x10, x9
1000d2958: 54001e43    	b.lo	0x1000d2d20 <__RNvNtNtNtCsaHyC9lX8wyC_17field_regressions8campaign10candidates9composite10half_depth+0x424>
1000d295c: f940040a    	ldr	x10, [x0, #0x8]
1000d2960: 5280030b    	mov	w11, #0x18              ; =24
1000d2964: 9b0b292c    	madd	x12, x9, x11, x10
1000d2968: f9400989    	ldr	x9, [x12, #0x10]
1000d296c: b4001be9    	cbz	x9, 0x1000d2ce8 <__RNvNtNtNtCsaHyC9lX8wyC_17field_regressions8campaign10candidates9composite10half_depth+0x3ec>
1000d2970: aa0203e8    	mov	x8, x2
1000d2974: d341fc42    	lsr	x2, x2, #1
1000d2978: cb020113    	sub	x19, x8, x2
1000d297c: d280000b    	mov	x11, #0x0               ; =0
1000d2980: f100052d    	subs	x13, x9, #0x1
1000d2984: 54000220    	b.eq	0x1000d29c8 <__RNvNtNtNtCsaHyC9lX8wyC_17field_regressions8campaign10candidates9composite10half_depth+0xcc>
1000d2988: d2800009    	mov	x9, #0x0                ; =0
1000d298c: d280000a    	mov	x10, #0x0               ; =0
1000d2990: f940058e    	ldr	x14, [x12, #0x8]
1000d2994: d37cedac    	lsl	x12, x13, #4
1000d2998: 910061cd    	add	x13, x14, #0x18
1000d299c: 14000005    	b	0x1000d29b0 <__RNvNtNtNtCsaHyC9lX8wyC_17field_regressions8campaign10candidates9composite10half_depth+0xb4>
1000d29a0: 910041ad    	add	x13, x13, #0x10
1000d29a4: 91000529    	add	x9, x9, #0x1
1000d29a8: f100418c    	subs	x12, x12, #0x10
1000d29ac: 54000100    	b.eq	0x1000d29cc <__RNvNtNtNtCsaHyC9lX8wyC_17field_regressions8campaign10candidates9composite10half_depth+0xd0>
1000d29b0: 9ac924ae    	lsr	x14, x5, x9
1000d29b4: 3607ff6e    	tbz	w14, #0x0, 0x1000d29a0 <__RNvNtNtNtCsaHyC9lX8wyC_17field_regressions8campaign10candidates9composite10half_depth+0xa4>
1000d29b8: a97fb9af    	ldp	x15, x14, [x13, #-0x8]
1000d29bc: ca0a01ea    	eor	x10, x15, x10
1000d29c0: ca0b01cb    	eor	x11, x14, x11
1000d29c4: 17fffff7    	b	0x1000d29a0 <__RNvNtNtNtCsaHyC9lX8wyC_17field_regressions8campaign10candidates9composite10half_depth+0xa4>
1000d29c8: d280000a    	mov	x10, #0x0               ; =0
1000d29cc: 8b021034    	add	x20, x1, x2, lsl #4
1000d29d0: eb02027f    	cmp	x19, x2
1000d29d4: 9a823269    	csel	x9, x19, x2, lo
1000d29d8: aa0b014c    	orr	x12, x10, x11
1000d29dc: b500020c    	cbnz	x12, 0x1000d2a1c <__RNvNtNtNtCsaHyC9lX8wyC_17field_regressions8campaign10candidates9composite10half_depth+0x120>
1000d29e0: b4000ec9    	cbz	x9, 0x1000d2bb8 <__RNvNtNtNtCsaHyC9lX8wyC_17field_regressions8campaign10candidates9composite10half_depth+0x2bc>
1000d29e4: f100413f    	cmp	x9, #0x10
1000d29e8: 54000942    	b.hs	0x1000d2b10 <__RNvNtNtNtCsaHyC9lX8wyC_17field_regressions8campaign10candidates9composite10half_depth+0x214>
1000d29ec: d280000a    	mov	x10, #0x0               ; =0
1000d29f0: cb0a0129    	sub	x9, x9, x10
1000d29f4: d37cec4b    	lsl	x11, x2, #4
1000d29f8: 8b0a102a    	add	x10, x1, x10, lsl #4
1000d29fc: 3ceb6940    	ldr	q0, [x10, x11]
1000d2a00: 3dc00141    	ldr	q1, [x10]
1000d2a04: 6e211c00    	eor.16b	v0, v0, v1
1000d2a08: 3cab6940    	str	q0, [x10, x11]
1000d2a0c: 9100414a    	add	x10, x10, #0x10
1000d2a10: f1000529    	subs	x9, x9, #0x1
1000d2a14: 54ffff41    	b.ne	0x1000d29fc <__RNvNtNtNtCsaHyC9lX8wyC_17field_regressions8campaign10candidates9composite10half_depth+0x100>
1000d2a18: 14000068    	b	0x1000d2bb8 <__RNvNtNtNtCsaHyC9lX8wyC_17field_regressions8campaign10candidates9composite10half_depth+0x2bc>
1000d2a1c: b40004ab    	cbz	x11, 0x1000d2ab0 <__RNvNtNtNtCsaHyC9lX8wyC_17field_regressions8campaign10candidates9composite10half_depth+0x1b4>
1000d2a20: b4000cc9    	cbz	x9, 0x1000d2bb8 <__RNvNtNtNtCsaHyC9lX8wyC_17field_regressions8campaign10candidates9composite10half_depth+0x2bc>
1000d2a24: 9e670160    	fmov	d0, x11
1000d2a28: 528010eb    	mov	w11, #0x87              ; =135
1000d2a2c: 9e670161    	fmov	d1, x11
1000d2a30: 0ee1e001    	pmull.1q	v1, v0, v1
1000d2a34: 4e183c2c    	mov.d	x12, v1[1]
1000d2a38: ca0a018d    	eor	x13, x12, x10
1000d2a3c: d37cec4c    	lsl	x12, x2, #4
1000d2a40: 9e670142    	fmov	d2, x10
1000d2a44: 9e6701a3    	fmov	d3, x13
1000d2a48: aa0103ea    	mov	x10, x1
1000d2a4c: 8b0c014d    	add	x13, x10, x12
1000d2a50: 6d4015a4    	ldp	d4, d5, [x13]
1000d2a54: 0ee2e086    	pmull.1q	v6, v4, v2
1000d2a58: 0ee1e0a7    	pmull.1q	v7, v5, v1
1000d2a5c: 0ee0e084    	pmull.1q	v4, v4, v0
1000d2a60: 0ee3e0a5    	pmull.1q	v5, v5, v3
1000d2a64: 6e241ca4    	eor.16b	v4, v5, v4
1000d2a68: 4e080d65    	dup.2d	v5, x11
1000d2a6c: 4ee5e085    	pmull2.1q	v5, v4, v5
1000d2a70: ce0614e5    	eor3.16b	v5, v7, v6, v5
1000d2a74: 4e183cae    	mov.d	x14, v5[1]
1000d2a78: 9e6600af    	fmov	x15, d5
1000d2a7c: a9404550    	ldp	x16, x17, [x10]
1000d2a80: ca0f020f    	eor	x15, x16, x15
1000d2a84: 9e660090    	fmov	x16, d4
1000d2a88: ca100230    	eor	x16, x17, x16
1000d2a8c: ca0e020e    	eor	x14, x16, x14
1000d2a90: a881394f    	stp	x15, x14, [x10], #0x10
1000d2a94: a94045b0    	ldp	x16, x17, [x13]
1000d2a98: ca1001ef    	eor	x15, x15, x16
1000d2a9c: ca0e022e    	eor	x14, x17, x14
1000d2aa0: a90039af    	stp	x15, x14, [x13]
1000d2aa4: f1000529    	subs	x9, x9, #0x1
1000d2aa8: 54fffd21    	b.ne	0x1000d2a4c <__RNvNtNtNtCsaHyC9lX8wyC_17field_regressions8campaign10candidates9composite10half_depth+0x150>
1000d2aac: 14000043    	b	0x1000d2bb8 <__RNvNtNtNtCsaHyC9lX8wyC_17field_regressions8campaign10candidates9composite10half_depth+0x2bc>
1000d2ab0: b4000849    	cbz	x9, 0x1000d2bb8 <__RNvNtNtNtCsaHyC9lX8wyC_17field_regressions8campaign10candidates9composite10half_depth+0x2bc>
1000d2ab4: d37cec4b    	lsl	x11, x2, #4
1000d2ab8: 9e670140    	fmov	d0, x10
1000d2abc: 528010ea    	mov	w10, #0x87              ; =135
1000d2ac0: 4e080d41    	dup.2d	v1, x10
1000d2ac4: 6f00e402    	movi.2d	v2, #0000000000000000
1000d2ac8: aa0103ea    	mov	x10, x1
1000d2acc: 8b0b014c    	add	x12, x10, x11
1000d2ad0: 6d401183    	ldp	d3, d4, [x12]
1000d2ad4: 0ee0e084    	pmull.1q	v4, v4, v0
1000d2ad8: 4ee1e085    	pmull2.1q	v5, v4, v1
1000d2adc: 6e044044    	ext.16b	v4, v2, v4, #0x8
1000d2ae0: 0ee0e063    	pmull.1q	v3, v3, v0
1000d2ae4: 6e231c86    	eor.16b	v6, v4, v3
1000d2ae8: ce031483    	eor3.16b	v3, v4, v3, v5
1000d2aec: 3dc00144    	ldr	q4, [x10]
1000d2af0: ce0510c5    	eor3.16b	v5, v6, v5, v4
1000d2af4: 3c810545    	str	q5, [x10], #0x10
1000d2af8: 3dc00185    	ldr	q5, [x12]
1000d2afc: ce041463    	eor3.16b	v3, v3, v4, v5
1000d2b00: 3d800183    	str	q3, [x12]
1000d2b04: f1000529    	subs	x9, x9, #0x1
1000d2b08: 54fffe21    	b.ne	0x1000d2acc <__RNvNtNtNtCsaHyC9lX8wyC_17field_regressions8campaign10candidates9composite10half_depth+0x1d0>
1000d2b0c: 1400002b    	b	0x1000d2bb8 <__RNvNtNtNtCsaHyC9lX8wyC_17field_regressions8campaign10candidates9composite10half_depth+0x2bc>
1000d2b10: d280000a    	mov	x10, #0x0               ; =0
1000d2b14: d37ced2c    	lsl	x12, x9, #4
1000d2b18: d37cec4b    	lsl	x11, x2, #4
1000d2b1c: 8b0b002d    	add	x13, x1, x11
1000d2b20: 8b0c01b0    	add	x16, x13, x12
1000d2b24: d1002211    	sub	x17, x16, #0x8
1000d2b28: 9100202f    	add	x15, x1, #0x8
1000d2b2c: 8b0c0026    	add	x6, x1, x12
1000d2b30: d10020ce    	sub	x14, x6, #0x8
1000d2b34: eb0e029f    	cmp	x20, x14
1000d2b38: fa513022    	ccmp	x1, x17, #0x2, lo
1000d2b3c: 1a9f27ec    	cset	w12, lo
1000d2b40: eb06029f    	cmp	x20, x6
1000d2b44: fa5131e2    	ccmp	x15, x17, #0x2, lo
1000d2b48: 1a9f27ed    	cset	w13, lo
1000d2b4c: 8b0b01e7    	add	x7, x15, x11
1000d2b50: eb0e00ff    	cmp	x7, x14
1000d2b54: fa503022    	ccmp	x1, x16, #0x2, lo
1000d2b58: 1a9f27ee    	cset	w14, lo
1000d2b5c: eb0600ff    	cmp	x7, x6
1000d2b60: fa5031e2    	ccmp	x15, x16, #0x2, lo
1000d2b64: 1a9f27ef    	cset	w15, lo
1000d2b68: eb1100ff    	cmp	x7, x17
1000d2b6c: fa503282    	ccmp	x20, x16, #0x2, lo
1000d2b70: 54fff403    	b.lo	0x1000d29f0 <__RNvNtNtNtCsaHyC9lX8wyC_17field_regressions8campaign10candidates9composite10half_depth+0xf4>
1000d2b74: 3707f3ec    	tbnz	w12, #0x0, 0x1000d29f0 <__RNvNtNtNtCsaHyC9lX8wyC_17field_regressions8campaign10candidates9composite10half_depth+0xf4>
1000d2b78: 3707f3cd    	tbnz	w13, #0x0, 0x1000d29f0 <__RNvNtNtNtCsaHyC9lX8wyC_17field_regressions8campaign10candidates9composite10half_depth+0xf4>
1000d2b7c: 3707f3ae    	tbnz	w14, #0x0, 0x1000d29f0 <__RNvNtNtNtCsaHyC9lX8wyC_17field_regressions8campaign10candidates9composite10half_depth+0xf4>
1000d2b80: 3707f38f    	tbnz	w15, #0x0, 0x1000d29f0 <__RNvNtNtNtCsaHyC9lX8wyC_17field_regressions8campaign10candidates9composite10half_depth+0xf4>
1000d2b84: 927edd2a    	and	x10, x9, #0x3fffffffffffffc
1000d2b88: 927edd2c    	and	x12, x9, #0x3fffffffffffffc
1000d2b8c: aa0103ed    	mov	x13, x1
1000d2b90: 8b0b01ae    	add	x14, x13, x11
1000d2b94: acc105a0    	ldp	q0, q1, [x13], #0x20
1000d2b98: ad400dc2    	ldp	q2, q3, [x14]
1000d2b9c: 6e201c40    	eor.16b	v0, v2, v0
1000d2ba0: 6e211c61    	eor.16b	v1, v3, v1
1000d2ba4: ad0005c0    	stp	q0, q1, [x14]
1000d2ba8: f100098c    	subs	x12, x12, #0x2
1000d2bac: 54ffff21    	b.ne	0x1000d2b90 <__RNvNtNtNtCsaHyC9lX8wyC_17field_regressions8campaign10candidates9composite10half_depth+0x294>
1000d2bb0: eb0a013f    	cmp	x9, x10
1000d2bb4: 54fff1e1    	b.ne	0x1000d29f0 <__RNvNtNtNtCsaHyC9lX8wyC_17field_regressions8campaign10candidates9composite10half_depth+0xf4>
1000d2bb8: d34ffd08    	lsr	x8, x8, #15
1000d2bbc: b4000688    	cbz	x8, 0x1000d2c8c <__RNvNtNtNtCsaHyC9lX8wyC_17field_regressions8campaign10candidates9composite10half_depth+0x390>
1000d2bc0: aa0003f6    	mov	x22, x0
1000d2bc4: d0000680    	adrp	x0, 0x1001a4000 <dyld_stub_binder+0x1001a4000>
1000d2bc8: 91340000    	add	x0, x0, #0xd00
1000d2bcc: f9400008    	ldr	x8, [x0]
1000d2bd0: d63f0100    	blr	x8
1000d2bd4: aa0003f5    	mov	x21, x0
1000d2bd8: f9400008    	ldr	x8, [x0]
1000d2bdc: b4000388    	cbz	x8, 0x1000d2c4c <__RNvNtNtNtCsaHyC9lX8wyC_17field_regressions8campaign10candidates9composite10half_depth+0x350>
1000d2be0: 91044100    	add	x0, x8, #0x110
1000d2be4: f9400008    	ldr	x8, [x0]
1000d2be8: f9410908    	ldr	x8, [x8, #0x210]
1000d2bec: f100091f    	cmp	x8, #0x2
1000d2bf0: aa1603e0    	mov	x0, x22
1000d2bf4: 540004c3    	b.lo	0x1000d2c8c <__RNvNtNtNtCsaHyC9lX8wyC_17field_regressions8campaign10candidates9composite10half_depth+0x390>
1000d2bf8: a90253e0    	stp	x0, x20, [sp, #0x20]
1000d2bfc: 910023e8    	add	x8, sp, #0x8
1000d2c00: a90323f3    	stp	x19, x8, [sp, #0x30]
1000d2c04: 910043e9    	add	x9, sp, #0x10
1000d2c08: 910063ea    	add	x10, sp, #0x18
1000d2c0c: a9042be9    	stp	x9, x10, [sp, #0x40]
1000d2c10: a90507e0    	stp	x0, x1, [sp, #0x50]
1000d2c14: a90623e2    	stp	x2, x8, [sp, #0x60]
1000d2c18: a9072be9    	stp	x9, x10, [sp, #0x70]
1000d2c1c: f94002a1    	ldr	x1, [x21]
1000d2c20: b5000101    	cbnz	x1, 0x1000d2c40 <__RNvNtNtNtCsaHyC9lX8wyC_17field_regressions8campaign10candidates9composite10half_depth+0x344>
1000d2c24: 94011527    	bl	0x1001180c0 <__RNvNtCs4bPT1zor9nS_10rayon_core8registry15global_registry>
1000d2c28: f9400008    	ldr	x8, [x0]
1000d2c2c: f94002a1    	ldr	x1, [x21]
1000d2c30: b4000681    	cbz	x1, 0x1000d2d00 <__RNvNtNtNtCsaHyC9lX8wyC_17field_regressions8campaign10candidates9composite10half_depth+0x404>
1000d2c34: f9408829    	ldr	x9, [x1, #0x110]
1000d2c38: eb08013f    	cmp	x9, x8
1000d2c3c: 540006a1    	b.ne	0x1000d2d10 <__RNvNtNtNtCsaHyC9lX8wyC_17field_regressions8campaign10candidates9composite10half_depth+0x414>
1000d2c40: 910083e0    	add	x0, sp, #0x20
1000d2c44: 97fdcaa2    	bl	0x1000456cc <__RNCINvNtCs4bPT1zor9nS_10rayon_core4join12join_contextNCINvNvB4_4join4calluNCNvNtNtNtCsaHyC9lX8wyC_17field_regressions8campaign10candidates9composite10half_depth0E0NCIBS_uNCB1c_s_0E0uuE0B1k_>
1000d2c48: 17ffff39    	b	0x1000d292c <__RNvNtNtNtCsaHyC9lX8wyC_17field_regressions8campaign10candidates9composite10half_depth+0x30>
1000d2c4c: aa0303f7    	mov	x23, x3
1000d2c50: aa0403fa    	mov	x26, x4
1000d2c54: aa0103f8    	mov	x24, x1
1000d2c58: aa0503fb    	mov	x27, x5
1000d2c5c: aa0203f9    	mov	x25, x2
1000d2c60: 94011518    	bl	0x1001180c0 <__RNvNtCs4bPT1zor9nS_10rayon_core8registry15global_registry>
1000d2c64: aa1903e2    	mov	x2, x25
1000d2c68: aa1b03e5    	mov	x5, x27
1000d2c6c: aa1803e1    	mov	x1, x24
1000d2c70: aa1a03e4    	mov	x4, x26
1000d2c74: aa1703e3    	mov	x3, x23
1000d2c78: f9400008    	ldr	x8, [x0]
1000d2c7c: f9410908    	ldr	x8, [x8, #0x210]
1000d2c80: f100091f    	cmp	x8, #0x2
1000d2c84: aa1603e0    	mov	x0, x22
1000d2c88: 54fffb82    	b.hs	0x1000d2bf8 <__RNvNtNtNtCsaHyC9lX8wyC_17field_regressions8campaign10candidates9composite10half_depth+0x2fc>
1000d2c8c: d37ff8a8    	lsl	x8, x5, #1
1000d2c90: aa0403f7    	mov	x23, x4
1000d2c94: 91000484    	add	x4, x4, #0x1
1000d2c98: aa0003f5    	mov	x21, x0
1000d2c9c: aa0303f6    	mov	x22, x3
1000d2ca0: aa0503f8    	mov	x24, x5
1000d2ca4: aa0803e5    	mov	x5, x8
1000d2ca8: 9400032e    	bl	0x1000d3960 <__RNvNtNtNtCsaHyC9lX8wyC_17field_regressions8campaign10candidates9composite11serial_tree>
1000d2cac: 52800025    	mov	w5, #0x1                ; =1
1000d2cb0: b37ffb05    	bfi	x5, x24, #1, #63
1000d2cb4: 910006e4    	add	x4, x23, #0x1
1000d2cb8: aa1503e0    	mov	x0, x21
1000d2cbc: aa1403e1    	mov	x1, x20
1000d2cc0: aa1303e2    	mov	x2, x19
1000d2cc4: aa1603e3    	mov	x3, x22
1000d2cc8: a94d7bfd    	ldp	x29, x30, [sp, #0xd0]
1000d2ccc: a94c4ff4    	ldp	x20, x19, [sp, #0xc0]
1000d2cd0: a94b57f6    	ldp	x22, x21, [sp, #0xb0]
1000d2cd4: a94a5ff8    	ldp	x24, x23, [sp, #0xa0]
1000d2cd8: a94967fa    	ldp	x26, x25, [sp, #0x90]
1000d2cdc: a9486ffc    	ldp	x28, x27, [sp, #0x80]
1000d2ce0: 910383ff    	add	sp, sp, #0xe0
1000d2ce4: 1400031f    	b	0x1000d3960 <__RNvNtNtNtCsaHyC9lX8wyC_17field_regressions8campaign10candidates9composite11serial_tree>
1000d2ce8: d0000663    	adrp	x3, 0x1001a0000 <dyld_stub_binder+0x1001a0000>
1000d2cec: 91248063    	add	x3, x3, #0x920
1000d2cf0: 52800020    	mov	w0, #0x1                ; =1
1000d2cf4: d2800001    	mov	x1, #0x0                ; =0
1000d2cf8: d2800002    	mov	x2, #0x0                ; =0
1000d2cfc: 9401e332    	bl	0x10014b9c4 <__RNvNtNtCs8Mbv00yxnRz_4core5slice5index16slice_index_fail>
1000d2d00: 91020100    	add	x0, x8, #0x80
1000d2d04: 910083e1    	add	x1, sp, #0x20
1000d2d08: 9401d602    	bl	0x100148510 <__RINvMs4_NtCs4bPT1zor9nS_10rayon_core8registryNtB6_8Registry14in_worker_coldNCINvNtB8_4join12join_contextNCINvNvB1g_4join4calluNCNvNtNtNtCsaHyC9lX8wyC_17field_regressions8campaign10candidates9composite10half_depth0E0NCIB1H_uNCB22_s_0E0uuE0TuuEEB2a_>
1000d2d0c: 17ffff08    	b	0x1000d292c <__RNvNtNtNtCsaHyC9lX8wyC_17field_regressions8campaign10candidates9composite10half_depth+0x30>
1000d2d10: 91020100    	add	x0, x8, #0x80
1000d2d14: 910083e2    	add	x2, sp, #0x20
1000d2d18: 9401d97e    	bl	0x100149310 <__RINvMs4_NtCs4bPT1zor9nS_10rayon_core8registryNtB6_8Registry15in_worker_crossNCINvNtB8_4join12join_contextNCINvNvB1h_4join4calluNCNvNtNtNtCsaHyC9lX8wyC_17field_regressions8campaign10candidates9composite10half_depth0E0NCIB1I_uNCB23_s_0E0uuE0TuuEEB2b_>
1000d2d1c: 17ffff04    	b	0x1000d292c <__RNvNtNtNtCsaHyC9lX8wyC_17field_regressions8campaign10candidates9composite10half_depth+0x30>
1000d2d20: d0000662    	adrp	x2, 0x1001a0000 <dyld_stub_binder+0x1001a0000>
1000d2d24: 91242042    	add	x2, x2, #0x908
1000d2d28: aa0903e0    	mov	x0, x9
1000d2d2c: aa0a03e1    	mov	x1, x10
1000d2d30: 9401e300    	bl	0x10014b930 <__RNvNtCs8Mbv00yxnRz_4core9panicking18panic_bounds_check>

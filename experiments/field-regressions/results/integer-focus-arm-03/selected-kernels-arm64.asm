
/var/folders/q0/v7l8dzd13k7f_q6j80sy03lm0000gn/T/f2z-field-gated-gtiqg0cq/field-regressions:	file format mach-o arm64

Disassembly of section __TEXT,__text:

0000000100004ca8 <field_regressions::campaign::two_limb_mac::seeded_product::<2>>:
100004ca8: d10083ff    	sub	sp, sp, #0x20
100004cac: a9017bfd    	stp	x29, x30, [sp, #0x10]
100004cb0: 910043fd    	add	x29, sp, #0x10
100004cb4: a90013e2    	stp	x2, x4, [sp]
100004cb8: eb04005f    	cmp	x2, x4
100004cbc: 540008e1    	b.ne	0x100004dd8 <field_regressions::campaign::two_limb_mac::seeded_product::<2>+0x130>
100004cc0: f100045f    	cmp	x2, #0x1
100004cc4: 54000128    	b.hi	0x100004ce8 <field_regressions::campaign::two_limb_mac::seeded_product::<2>+0x40>
100004cc8: b40007c2    	cbz	x2, 0x100004dc0 <field_regressions::campaign::two_limb_mac::seeded_product::<2>+0x118>
100004ccc: a9402029    	ldp	x9, x8, [x1]
100004cd0: a9402c6a    	ldp	x10, x11, [x3]
100004cd4: 9bc97d4c    	umulh	x12, x10, x9
100004cd8: 9b083148    	madd	x8, x10, x8, x12
100004cdc: 9b092168    	madd	x8, x11, x9, x8
100004ce0: 9b097d49    	mul	x9, x10, x9
100004ce4: 14000039    	b	0x100004dc8 <field_regressions::campaign::two_limb_mac::seeded_product::<2>+0x120>
100004ce8: a9402029    	ldp	x9, x8, [x1]
100004cec: a940286b    	ldp	x11, x10, [x3]
100004cf0: 9bc97d6c    	umulh	x12, x11, x9
100004cf4: 9b083168    	madd	x8, x11, x8, x12
100004cf8: 9b092148    	madd	x8, x10, x9, x8
100004cfc: 9b097d69    	mul	x9, x11, x9
100004d00: a9412c2a    	ldp	x10, x11, [x1, #0x10]
100004d04: a941346c    	ldp	x12, x13, [x3, #0x10]
100004d08: 9bca7d8e    	umulh	x14, x12, x10
100004d0c: 9b0b398b    	madd	x11, x12, x11, x14
100004d10: 9b0a2dab    	madd	x11, x13, x10, x11
100004d14: 9b0a7d8c    	mul	x12, x12, x10
100004d18: 927fe44a    	and	x10, x2, #0x7fffffffffffffe
100004d1c: f100095f    	cmp	x10, #0x2
100004d20: 54000320    	b.eq	0x100004d84 <field_regressions::campaign::two_limb_mac::seeded_product::<2>+0xdc>
100004d24: 5280004d    	mov	w13, #0x2               ; =2
100004d28: cb0a01ad    	sub	x13, x13, x10
100004d2c: 9100e06e    	add	x14, x3, #0x38
100004d30: 9100e02f    	add	x15, x1, #0x38
100004d34: a97ec5f0    	ldp	x16, x17, [x15, #-0x18]
100004d38: a97e95c4    	ldp	x4, x5, [x14, #-0x18]
100004d3c: 9bd07c86    	umulh	x6, x4, x16
100004d40: 9b111891    	madd	x17, x4, x17, x6
100004d44: 9b1044b1    	madd	x17, x5, x16, x17
100004d48: 9b107c90    	mul	x16, x4, x16
100004d4c: ab090209    	adds	x9, x16, x9
100004d50: 9a080228    	adc	x8, x17, x8
100004d54: a97fc5f0    	ldp	x16, x17, [x15, #-0x8]
100004d58: a97f95c4    	ldp	x4, x5, [x14, #-0x8]
100004d5c: 9bd07c86    	umulh	x6, x4, x16
100004d60: 9b111891    	madd	x17, x4, x17, x6
100004d64: 9b1044b1    	madd	x17, x5, x16, x17
100004d68: 9b107c90    	mul	x16, x4, x16
100004d6c: ab0c020c    	adds	x12, x16, x12
100004d70: 9a0b022b    	adc	x11, x17, x11
100004d74: 910081ce    	add	x14, x14, #0x20
100004d78: 910081ef    	add	x15, x15, #0x20
100004d7c: b10009ad    	adds	x13, x13, #0x2
100004d80: 54fffda1    	b.ne	0x100004d34 <field_regressions::campaign::two_limb_mac::seeded_product::<2>+0x8c>
100004d84: ab0c0129    	adds	x9, x9, x12
100004d88: 9a0b0108    	adc	x8, x8, x11
100004d8c: 360001e2    	tbz	w2, #0x0, 0x100004dc8 <field_regressions::campaign::two_limb_mac::seeded_product::<2>+0x120>
100004d90: d37ced4a    	lsl	x10, x10, #4
100004d94: 8b0a002b    	add	x11, x1, x10
100004d98: 8b0a006a    	add	x10, x3, x10
100004d9c: a9402d6c    	ldp	x12, x11, [x11]
100004da0: a940294d    	ldp	x13, x10, [x10]
100004da4: 9bcc7dae    	umulh	x14, x13, x12
100004da8: 9b0b39ab    	madd	x11, x13, x11, x14
100004dac: 9b0c2d4a    	madd	x10, x10, x12, x11
100004db0: 9b0c7dab    	mul	x11, x13, x12
100004db4: ab090169    	adds	x9, x11, x9
100004db8: 9a080148    	adc	x8, x10, x8
100004dbc: 14000003    	b	0x100004dc8 <field_regressions::campaign::two_limb_mac::seeded_product::<2>+0x120>
100004dc0: d2800009    	mov	x9, #0x0                ; =0
100004dc4: d2800008    	mov	x8, #0x0                ; =0
100004dc8: a9002009    	stp	x9, x8, [x0]
100004dcc: a9417bfd    	ldp	x29, x30, [sp, #0x10]
100004dd0: 910083ff    	add	sp, sp, #0x20
100004dd4: d65f03c0    	ret
100004dd8: 90000623    	adrp	x3, 0x1000c8000 <dyld_stub_binder+0x1000c8000>
100004ddc: 91242063    	add	x3, x3, #0x908
100004de0: 910003e0    	mov	x0, sp
100004de4: 910023e1    	add	x1, sp, #0x8
100004de8: d2800002    	mov	x2, #0x0                ; =0
100004dec: 940243d8    	bl	0x100095d4c <core::panicking::assert_failed::<usize, usize>>


/var/folders/q0/v7l8dzd13k7f_q6j80sy03lm0000gn/T/f2z-field-gated-gtiqg0cq/field-regressions:	file format mach-o arm64

Disassembly of section __TEXT,__text:

0000000100019974 <<field_regressions::campaign::bounded_product::PreparedProducts>::execute>:
100019974: a9bc67fa    	stp	x26, x25, [sp, #-0x40]!
100019978: a9015ff8    	stp	x24, x23, [sp, #0x10]
10001997c: a90257f6    	stp	x22, x21, [sp, #0x20]
100019980: a9034ff4    	stp	x20, x19, [sp, #0x30]
100019984: 3940c00c    	ldrb	w12, [x0, #0x30]
100019988: a9402009    	ldp	x9, x8, [x0]
10001998c: a941340a    	ldp	x10, x13, [x0, #0x10]
100019990: a942380b    	ldp	x11, x14, [x0, #0x20]
100019994: eb0801bf    	cmp	x13, x8
100019998: 9a8831a8    	csel	x8, x13, x8, lo
10001999c: eb0801df    	cmp	x14, x8
1000199a0: 9a8831c8    	csel	x8, x14, x8, lo
1000199a4: 7100059f    	cmp	w12, #0x1
1000199a8: 540010a1    	b.ne	0x100019bbc <<field_regressions::campaign::bounded_product::PreparedProducts>::execute+0x248>
1000199ac: b4001ce8    	cbz	x8, 0x100019d48 <<field_regressions::campaign::bounded_product::PreparedProducts>::execute+0x3d4>
1000199b0: d280000c    	mov	x12, #0x0               ; =0
1000199b4: 9100212d    	add	x13, x9, #0x8
1000199b8: 9100a16e    	add	x14, x11, #0x28
1000199bc: 5280120f    	mov	w15, #0x90              ; =144
1000199c0: d2800010    	mov	x16, #0x0               ; =0
1000199c4: 8b0c0d91    	add	x17, x12, x12, lsl #3
1000199c8: d37df231    	lsl	x17, x17, #3
1000199cc: 8b110147    	add	x7, x10, x17
1000199d0: 9b0f2d93    	madd	x19, x12, x15, x11
1000199d4: f8716934    	ldr	x20, [x9, x17]
1000199d8: a94000f1    	ldp	x17, x0, [x7]
1000199dc: 9bd47e21    	umulh	x1, x17, x20
1000199e0: 9b147e22    	mul	x2, x17, x20
1000199e4: 9bd47c03    	umulh	x3, x0, x20
1000199e8: 9b147c04    	mul	x4, x0, x20
1000199ec: ab010081    	adds	x1, x4, x1
1000199f0: 9a833463    	cinc	x3, x3, hs
1000199f4: a9000662    	stp	x2, x1, [x19]
1000199f8: a94108e1    	ldp	x1, x2, [x7, #0x10]
1000199fc: 9bd47c24    	umulh	x4, x1, x20
100019a00: 9b147c25    	mul	x5, x1, x20
100019a04: ab0300a3    	adds	x3, x5, x3
100019a08: 9a843484    	cinc	x4, x4, hs
100019a0c: 9bd47c45    	umulh	x5, x2, x20
100019a10: 9b147c46    	mul	x6, x2, x20
100019a14: ab0400c4    	adds	x4, x6, x4
100019a18: 9a8534a5    	cinc	x5, x5, hs
100019a1c: a9011263    	stp	x3, x4, [x19, #0x10]
100019a20: a94210e3    	ldp	x3, x4, [x7, #0x20]
100019a24: 9bd47c66    	umulh	x6, x3, x20
100019a28: 9b147c75    	mul	x21, x3, x20
100019a2c: ab0502a5    	adds	x5, x21, x5
100019a30: 9a8634c6    	cinc	x6, x6, hs
100019a34: 9bd47c95    	umulh	x21, x4, x20
100019a38: 9b147c96    	mul	x22, x4, x20
100019a3c: ab0602c6    	adds	x6, x22, x6
100019a40: 9a9536b5    	cinc	x21, x21, hs
100019a44: a9021a65    	stp	x5, x6, [x19, #0x20]
100019a48: a94318e5    	ldp	x5, x6, [x7, #0x30]
100019a4c: 9bd47cb6    	umulh	x22, x5, x20
100019a50: 9b147cb7    	mul	x23, x5, x20
100019a54: ab1502f5    	adds	x21, x23, x21
100019a58: 9a9636d6    	cinc	x22, x22, hs
100019a5c: 9b147cd7    	mul	x23, x6, x20
100019a60: ab1602f6    	adds	x22, x23, x22
100019a64: 9bd47cd7    	umulh	x23, x6, x20
100019a68: 9a9736f7    	cinc	x23, x23, hs
100019a6c: a9035a75    	stp	x21, x22, [x19, #0x30]
100019a70: f94020e7    	ldr	x7, [x7, #0x40]
100019a74: 9bd47cf5    	umulh	x21, x7, x20
100019a78: 9b147cf4    	mul	x20, x7, x20
100019a7c: ab170294    	adds	x20, x20, x23
100019a80: 9a9536b5    	cinc	x21, x21, hs
100019a84: a9045674    	stp	x20, x21, [x19, #0x40]
100019a88: aa0e03f4    	mov	x20, x14
100019a8c: aa0e03f3    	mov	x19, x14
100019a90: f87079b5    	ldr	x21, [x13, x16, lsl #3]
100019a94: 9bd57e36    	umulh	x22, x17, x21
100019a98: 9b157e37    	mul	x23, x17, x21
100019a9c: a97e6698    	ldp	x24, x25, [x20, #-0x20]
100019aa0: ab1802f7    	adds	x23, x23, x24
100019aa4: 9a9636d6    	cinc	x22, x22, hs
100019aa8: 9b157c18    	mul	x24, x0, x21
100019aac: ab1902d6    	adds	x22, x22, x25
100019ab0: 1a9f37f9    	cset	w25, hs
100019ab4: ab1802d6    	adds	x22, x22, x24
100019ab8: 9bd57c18    	umulh	x24, x0, x21
100019abc: 9a180338    	adc	x24, x25, x24
100019ac0: a93e5a97    	stp	x23, x22, [x20, #-0x20]
100019ac4: 9b157c36    	mul	x22, x1, x21
100019ac8: a97f6697    	ldp	x23, x25, [x20, #-0x10]
100019acc: ab170317    	adds	x23, x24, x23
100019ad0: 1a9f37f8    	cset	w24, hs
100019ad4: ab1602f6    	adds	x22, x23, x22
100019ad8: 9bd57c37    	umulh	x23, x1, x21
100019adc: 9a170317    	adc	x23, x24, x23
100019ae0: 9b157c58    	mul	x24, x2, x21
100019ae4: ab1902f7    	adds	x23, x23, x25
100019ae8: 1a9f37f9    	cset	w25, hs
100019aec: ab1802f7    	adds	x23, x23, x24
100019af0: 9bd57c58    	umulh	x24, x2, x21
100019af4: 9a180338    	adc	x24, x25, x24
100019af8: a93f5e96    	stp	x22, x23, [x20, #-0x10]
100019afc: 9bd57c76    	umulh	x22, x3, x21
100019b00: 9b157c77    	mul	x23, x3, x21
100019b04: f9400299    	ldr	x25, [x20]
100019b08: ab190318    	adds	x24, x24, x25
100019b0c: 1a9f37f9    	cset	w25, hs
100019b10: ab170317    	adds	x23, x24, x23
100019b14: 9a160336    	adc	x22, x25, x22
100019b18: f9000297    	str	x23, [x20]
100019b1c: 9bd57c97    	umulh	x23, x4, x21
100019b20: 9b157c98    	mul	x24, x4, x21
100019b24: f8408e79    	ldr	x25, [x19, #0x8]!
100019b28: ab1902d6    	adds	x22, x22, x25
100019b2c: 1a9f37f9    	cset	w25, hs
100019b30: ab1802d6    	adds	x22, x22, x24
100019b34: 9a170337    	adc	x23, x25, x23
100019b38: f9000276    	str	x22, [x19]
100019b3c: 9b157cb6    	mul	x22, x5, x21
100019b40: a9416698    	ldp	x24, x25, [x20, #0x10]
100019b44: ab1802f7    	adds	x23, x23, x24
100019b48: 1a9f37f8    	cset	w24, hs
100019b4c: ab1602f6    	adds	x22, x23, x22
100019b50: 9bd57cb7    	umulh	x23, x5, x21
100019b54: 9a170317    	adc	x23, x24, x23
100019b58: 9b157cd8    	mul	x24, x6, x21
100019b5c: ab1902f7    	adds	x23, x23, x25
100019b60: 1a9f37f9    	cset	w25, hs
100019b64: ab1802f7    	adds	x23, x23, x24
100019b68: 9bd57cd8    	umulh	x24, x6, x21
100019b6c: 9a180338    	adc	x24, x25, x24
100019b70: a9015e96    	stp	x22, x23, [x20, #0x10]
100019b74: 9bd57cf6    	umulh	x22, x7, x21
100019b78: 9b157cf5    	mul	x21, x7, x21
100019b7c: f9401297    	ldr	x23, [x20, #0x20]
100019b80: ab170317    	adds	x23, x24, x23
100019b84: 1a9f37f8    	cset	w24, hs
100019b88: ab1502f5    	adds	x21, x23, x21
100019b8c: 9a160316    	adc	x22, x24, x22
100019b90: a9025a95    	stp	x21, x22, [x20, #0x20]
100019b94: 91000610    	add	x16, x16, #0x1
100019b98: aa1303f4    	mov	x20, x19
100019b9c: f100221f    	cmp	x16, #0x8
100019ba0: 54fff781    	b.ne	0x100019a90 <<field_regressions::campaign::bounded_product::PreparedProducts>::execute+0x11c>
100019ba4: 9100058c    	add	x12, x12, #0x1
100019ba8: 910121ad    	add	x13, x13, #0x48
100019bac: 910241ce    	add	x14, x14, #0x90
100019bb0: eb08019f    	cmp	x12, x8
100019bb4: 54fff061    	b.ne	0x1000199c0 <<field_regressions::campaign::bounded_product::PreparedProducts>::execute+0x4c>
100019bb8: 14000064    	b	0x100019d48 <<field_regressions::campaign::bounded_product::PreparedProducts>::execute+0x3d4>
100019bbc: b4000c68    	cbz	x8, 0x100019d48 <<field_regressions::campaign::bounded_product::PreparedProducts>::execute+0x3d4>
100019bc0: 9100816b    	add	x11, x11, #0x20
100019bc4: 91004129    	add	x9, x9, #0x10
100019bc8: 9100414a    	add	x10, x10, #0x10
100019bcc: 6f00e400    	movi.2d	v0, #0000000000000000
100019bd0: a97f0930    	ldp	x16, x2, [x9, #-0x10]
100019bd4: a8c4b121    	ldp	x1, x12, [x9], #0x48
100019bd8: a97f4540    	ldp	x0, x17, [x10, #-0x10]
100019bdc: a8c4b54f    	ldp	x15, x13, [x10], #0x48
100019be0: 9b107c0e    	mul	x14, x0, x16
100019be4: 9bd07c03    	umulh	x3, x0, x16
100019be8: 9bd07e24    	umulh	x4, x17, x16
100019bec: 9b107e25    	mul	x5, x17, x16
100019bf0: ab050063    	adds	x3, x3, x5
100019bf4: 9bd07de5    	umulh	x5, x15, x16
100019bf8: 9a843484    	cinc	x4, x4, hs
100019bfc: 9b107de6    	mul	x6, x15, x16
100019c00: ab060084    	adds	x4, x4, x6
100019c04: 9a8534a5    	cinc	x5, x5, hs
100019c08: 9bd07da6    	umulh	x6, x13, x16
100019c0c: 9b107db0    	mul	x16, x13, x16
100019c10: ab1000a5    	adds	x5, x5, x16
100019c14: 9a8634c6    	cinc	x6, x6, hs
100019c18: 9bc27c07    	umulh	x7, x0, x2
100019c1c: 9b027c10    	mul	x16, x0, x2
100019c20: ab100070    	adds	x16, x3, x16
100019c24: 9bc27e23    	umulh	x3, x17, x2
100019c28: 9a8734e7    	cinc	x7, x7, hs
100019c2c: 9b027e33    	mul	x19, x17, x2
100019c30: ab0400e4    	adds	x4, x7, x4
100019c34: 1a9f37e7    	cset	w7, hs
100019c38: ab130084    	adds	x4, x4, x19
100019c3c: 9bc27df3    	umulh	x19, x15, x2
100019c40: 9a0300e3    	adc	x3, x7, x3
100019c44: 9b027de7    	mul	x7, x15, x2
100019c48: ab050063    	adds	x3, x3, x5
100019c4c: 1a9f37e5    	cset	w5, hs
100019c50: ab070063    	adds	x3, x3, x7
100019c54: 9bc27da7    	umulh	x7, x13, x2
100019c58: 9a1300a5    	adc	x5, x5, x19
100019c5c: 9b027da2    	mul	x2, x13, x2
100019c60: ab0600a5    	adds	x5, x5, x6
100019c64: 1a9f37e6    	cset	w6, hs
100019c68: ab0200a2    	adds	x2, x5, x2
100019c6c: 9bc17c05    	umulh	x5, x0, x1
100019c70: 9a0700c6    	adc	x6, x6, x7
100019c74: 9b017c07    	mul	x7, x0, x1
100019c78: ab070084    	adds	x4, x4, x7
100019c7c: 9a8534a5    	cinc	x5, x5, hs
100019c80: 9bc17e27    	umulh	x7, x17, x1
100019c84: 9b017e33    	mul	x19, x17, x1
100019c88: ab0300a3    	adds	x3, x5, x3
100019c8c: 1a9f37e5    	cset	w5, hs
100019c90: ab130063    	adds	x3, x3, x19
100019c94: 9a0700a5    	adc	x5, x5, x7
100019c98: 9bc17de7    	umulh	x7, x15, x1
100019c9c: 9b017df3    	mul	x19, x15, x1
100019ca0: ab0200a2    	adds	x2, x5, x2
100019ca4: 1a9f37e5    	cset	w5, hs
100019ca8: ab130042    	adds	x2, x2, x19
100019cac: 9a0700a5    	adc	x5, x5, x7
100019cb0: 9bc17da7    	umulh	x7, x13, x1
100019cb4: 9b017da1    	mul	x1, x13, x1
100019cb8: ab0600a5    	adds	x5, x5, x6
100019cbc: 1a9f37e6    	cset	w6, hs
100019cc0: ab0100a1    	adds	x1, x5, x1
100019cc4: 9a0700c5    	adc	x5, x6, x7
100019cc8: 9bcc7c06    	umulh	x6, x0, x12
100019ccc: 9b0c7c00    	mul	x0, x0, x12
100019cd0: ab000060    	adds	x0, x3, x0
100019cd4: 9a8634c3    	cinc	x3, x6, hs
100019cd8: 9bcc7e26    	umulh	x6, x17, x12
100019cdc: 9b0c7e31    	mul	x17, x17, x12
100019ce0: ab020062    	adds	x2, x3, x2
100019ce4: 1a9f37e3    	cset	w3, hs
100019ce8: ab110051    	adds	x17, x2, x17
100019cec: 9a060062    	adc	x2, x3, x6
100019cf0: 9bcc7de3    	umulh	x3, x15, x12
100019cf4: 9b0c7def    	mul	x15, x15, x12
100019cf8: ab010041    	adds	x1, x2, x1
100019cfc: 1a9f37e2    	cset	w2, hs
100019d00: ab0f002f    	adds	x15, x1, x15
100019d04: 9bcc7da1    	umulh	x1, x13, x12
100019d08: a93e416e    	stp	x14, x16, [x11, #-0x20]
100019d0c: a93f0164    	stp	x4, x0, [x11, #-0x10]
100019d10: 9a03004e    	adc	x14, x2, x3
100019d14: 9b0c7dac    	mul	x12, x13, x12
100019d18: a9003d71    	stp	x17, x15, [x11]
100019d1c: ad010160    	stp	q0, q0, [x11, #0x20]
100019d20: ab0501cd    	adds	x13, x14, x5
100019d24: 1a9f37ee    	cset	w14, hs
100019d28: ad020160    	stp	q0, q0, [x11, #0x40]
100019d2c: ab0c01ac    	adds	x12, x13, x12
100019d30: 9a0101cd    	adc	x13, x14, x1
100019d34: a901356c    	stp	x12, x13, [x11, #0x10]
100019d38: 3d801960    	str	q0, [x11, #0x60]
100019d3c: 9102416b    	add	x11, x11, #0x90
100019d40: f1000508    	subs	x8, x8, #0x1
100019d44: 54fff461    	b.ne	0x100019bd0 <<field_regressions::campaign::bounded_product::PreparedProducts>::execute+0x25c>
100019d48: a9434ff4    	ldp	x20, x19, [sp, #0x30]
100019d4c: a94257f6    	ldp	x22, x21, [sp, #0x20]
100019d50: a9415ff8    	ldp	x24, x23, [sp, #0x10]
100019d54: a8c467fa    	ldp	x26, x25, [sp], #0x40
100019d58: d65f03c0    	ret


/private/var/folders/q0/v7l8dzd13k7f_q6j80sy03lm0000gn/T/f2z-field-gated-6lkppdze/field-regressions:	file format mach-o arm64

Disassembly of section __TEXT,__text:

0000000100058ae4 <__RNvMNtNtNtCs8IeiyMGxmWr_17field_regressions8campaign10candidates7integerINtB2_10ProjectionKj4_E5batchB8_>:
100058ae4: d10143ff    	sub	sp, sp, #0x50
100058ae8: a9015ff8    	stp	x24, x23, [sp, #0x10]
100058aec: a90257f6    	stp	x22, x21, [sp, #0x20]
100058af0: a9034ff4    	stp	x20, x19, [sp, #0x30]
100058af4: a9047bfd    	stp	x29, x30, [sp, #0x40]
100058af8: 910103fd    	add	x29, sp, #0x40
100058afc: a90013e2    	stp	x2, x4, [sp]
100058b00: eb04005f    	cmp	x2, x4
100058b04: 54001c81    	b.ne	0x100058e94 <__RNvMNtNtNtCs8IeiyMGxmWr_17field_regressions8campaign10candidates7integerINtB2_10ProjectionKj4_E5batchB8_+0x3b0>
100058b08: b4001ba2    	cbz	x2, 0x100058e7c <__RNvMNtNtNtCs8IeiyMGxmWr_17field_regressions8campaign10candidates7integerINtB2_10ProjectionKj4_E5batchB8_+0x398>
100058b0c: d2800008    	mov	x8, #0x0                ; =0
100058b10: a949240a    	ldp	x10, x9, [x0, #0x90]
100058b14: a9442c0c    	ldp	x12, x11, [x0, #0x40]
100058b18: f940380d    	ldr	x13, [x0, #0x70]
100058b1c: a94a380f    	ldp	x15, x14, [x0, #0xa0]
100058b20: 91004030    	add	x16, x1, #0x10
100058b24: 91002071    	add	x17, x3, #0x8
100058b28: 910023e1    	add	x1, sp, #0x8
100058b2c: a9480003    	ldp	x3, x0, [x0, #0x80]
100058b30: a9401205    	ldp	x5, x4, [x16]
100058b34: 9b047d46    	mul	x6, x10, x4
100058b38: 9bc47d47    	umulh	x7, x10, x4
100058b3c: 9bc47d33    	umulh	x19, x9, x4
100058b40: 9b047d34    	mul	x20, x9, x4
100058b44: ab1400e7    	adds	x7, x7, x20
100058b48: ba080273    	adcs	x19, x19, x8
100058b4c: 9b067db4    	mul	x20, x13, x6
100058b50: 9b147d95    	mul	x21, x12, x20
100058b54: 9bd47d96    	umulh	x22, x12, x20
100058b58: 9bd47d77    	umulh	x23, x11, x20
100058b5c: 9b147d74    	mul	x20, x11, x20
100058b60: 1a9f37f8    	cset	w24, hs
100058b64: ab0702c7    	adds	x7, x22, x7
100058b68: 1a9f37f6    	cset	w22, hs
100058b6c: ab1400e7    	adds	x7, x7, x20
100058b70: 9a9636d4    	cinc	x20, x22, hs
100058b74: ab170273    	adds	x19, x19, x23
100058b78: 9a983716    	cinc	x22, x24, hs
100058b7c: ab0602bf    	cmn	x21, x6
100058b80: ba0800e6    	adcs	x6, x7, x8
100058b84: ba140267    	adcs	x7, x19, x20
100058b88: 9b067db3    	mul	x19, x13, x6
100058b8c: 9b137d94    	mul	x20, x12, x19
100058b90: 9bd37d95    	umulh	x21, x12, x19
100058b94: 9b137d77    	mul	x23, x11, x19
100058b98: 9bd37d73    	umulh	x19, x11, x19
100058b9c: ba160273    	adcs	x19, x19, x22
100058ba0: 1a9f37f6    	cset	w22, hs
100058ba4: ab0702a7    	adds	x7, x21, x7
100058ba8: 1a9f37f5    	cset	w21, hs
100058bac: ab1700e7    	adds	x7, x7, x23
100058bb0: 9a9536b5    	cinc	x21, x21, hs
100058bb4: ab06029f    	cmn	x20, x6
100058bb8: ba0800e6    	adcs	x6, x7, x8
100058bbc: ba150267    	adcs	x7, x19, x21
100058bc0: 9a9636d3    	cinc	x19, x22, hs
100058bc4: eb0c00df    	cmp	x6, x12
100058bc8: fa0b00ff    	sbcs	xzr, x7, x11
100058bcc: fa403a60    	ccmp	x19, #0x0, #0x0, lo
100058bd0: 9a881173    	csel	x19, x11, x8, ne
100058bd4: 9a881194    	csel	x20, x12, x8, ne
100058bd8: eb1400c6    	subs	x6, x6, x20
100058bdc: da1300e7    	sbc	x7, x7, x19
100058be0: ab0500c5    	adds	x5, x6, x5
100058be4: ba0800e6    	adcs	x6, x7, x8
100058be8: 1a9f37e7    	cset	w7, hs
100058bec: eb0c00bf    	cmp	x5, x12
100058bf0: fa0b00df    	sbcs	xzr, x6, x11
100058bf4: 1a9f34e7    	csinc	w7, w7, wzr, lo
100058bf8: 710000ff    	cmp	w7, #0x0
100058bfc: 9a881167    	csel	x7, x11, x8, ne
100058c00: 9a881193    	csel	x19, x12, x8, ne
100058c04: eb1300a5    	subs	x5, x5, x19
100058c08: da0700c6    	sbc	x6, x6, x7
100058c0c: 9b057d47    	mul	x7, x10, x5
100058c10: 9bc57d53    	umulh	x19, x10, x5
100058c14: 9bc57d34    	umulh	x20, x9, x5
100058c18: 9b057d25    	mul	x5, x9, x5
100058c1c: 9b067d55    	mul	x21, x10, x6
100058c20: 9bc67d56    	umulh	x22, x10, x6
100058c24: 9bc67d37    	umulh	x23, x9, x6
100058c28: 9b067d26    	mul	x6, x9, x6
100058c2c: ab050265    	adds	x5, x19, x5
100058c30: 1a9f37f3    	cset	w19, hs
100058c34: ab1402d4    	adds	x20, x22, x20
100058c38: 1a9f37f6    	cset	w22, hs
100058c3c: ab060286    	adds	x6, x20, x6
100058c40: 9a9636d4    	cinc	x20, x22, hs
100058c44: ab1500a5    	adds	x5, x5, x21
100058c48: ba1300c6    	adcs	x6, x6, x19
100058c4c: 9b077db3    	mul	x19, x13, x7
100058c50: 9a1402f4    	adc	x20, x23, x20
100058c54: 9b137d95    	mul	x21, x12, x19
100058c58: 9bd37d96    	umulh	x22, x12, x19
100058c5c: 9bd37d77    	umulh	x23, x11, x19
100058c60: 9b137d73    	mul	x19, x11, x19
100058c64: ab0502c5    	adds	x5, x22, x5
100058c68: 1a9f37f6    	cset	w22, hs
100058c6c: ab1300a5    	adds	x5, x5, x19
100058c70: 9a9636d3    	cinc	x19, x22, hs
100058c74: ab1700c6    	adds	x6, x6, x23
100058c78: 1a9f37f6    	cset	w22, hs
100058c7c: ab0702bf    	cmn	x21, x7
100058c80: ba0800a5    	adcs	x5, x5, x8
100058c84: ba1300c6    	adcs	x6, x6, x19
100058c88: ba160287    	adcs	x7, x20, x22
100058c8c: 1a9f37f3    	cset	w19, hs
100058c90: 9b057db4    	mul	x20, x13, x5
100058c94: 9bd47d95    	umulh	x21, x12, x20
100058c98: 9b147d96    	mul	x22, x12, x20
100058c9c: 9bd47d77    	umulh	x23, x11, x20
100058ca0: 9b147d74    	mul	x20, x11, x20
100058ca4: ab0602a6    	adds	x6, x21, x6
100058ca8: 1a9f37f5    	cset	w21, hs
100058cac: ab1400c6    	adds	x6, x6, x20
100058cb0: 9a9536b4    	cinc	x20, x21, hs
100058cb4: ab1700e7    	adds	x7, x7, x23
100058cb8: 1a9f37f5    	cset	w21, hs
100058cbc: ab0502df    	cmn	x22, x5
100058cc0: ba0800c5    	adcs	x5, x6, x8
100058cc4: ba1400e6    	adcs	x6, x7, x20
100058cc8: 9a9536a7    	cinc	x7, x21, hs
100058ccc: eb0c00bf    	cmp	x5, x12
100058cd0: fa0b00df    	sbcs	xzr, x6, x11
100058cd4: aa1300e7    	orr	x7, x7, x19
100058cd8: fa4038e0    	ccmp	x7, #0x0, #0x0, lo
100058cdc: 9a881167    	csel	x7, x11, x8, ne
100058ce0: 9a881193    	csel	x19, x12, x8, ne
100058ce4: eb1300b3    	subs	x19, x5, x19
100058ce8: da0700c6    	sbc	x6, x6, x7
100058cec: a97f1e05    	ldp	x5, x7, [x16, #-0x10]
100058cf0: ab070267    	adds	x7, x19, x7
100058cf4: ba0800c6    	adcs	x6, x6, x8
100058cf8: 1a9f37f3    	cset	w19, hs
100058cfc: eb0c00ff    	cmp	x7, x12
100058d00: fa0b00df    	sbcs	xzr, x6, x11
100058d04: 1a9f3673    	csinc	w19, w19, wzr, lo
100058d08: 7100027f    	cmp	w19, #0x0
100058d0c: 9a881173    	csel	x19, x11, x8, ne
100058d10: 9a881194    	csel	x20, x12, x8, ne
100058d14: eb1400e7    	subs	x7, x7, x20
100058d18: da1300c6    	sbc	x6, x6, x19
100058d1c: 9b077d53    	mul	x19, x10, x7
100058d20: 9bc77d54    	umulh	x20, x10, x7
100058d24: 9bc77d35    	umulh	x21, x9, x7
100058d28: 9b077d27    	mul	x7, x9, x7
100058d2c: 9b067d56    	mul	x22, x10, x6
100058d30: 9bc67d57    	umulh	x23, x10, x6
100058d34: 9bc67d38    	umulh	x24, x9, x6
100058d38: 9b067d26    	mul	x6, x9, x6
100058d3c: ab070287    	adds	x7, x20, x7
100058d40: 1a9f37f4    	cset	w20, hs
100058d44: ab1502f5    	adds	x21, x23, x21
100058d48: 1a9f37f7    	cset	w23, hs
100058d4c: ab0602a6    	adds	x6, x21, x6
100058d50: 9a9736f5    	cinc	x21, x23, hs
100058d54: ab1600e7    	adds	x7, x7, x22
100058d58: ba1400c6    	adcs	x6, x6, x20
100058d5c: 9a150314    	adc	x20, x24, x21
100058d60: 9b137db5    	mul	x21, x13, x19
100058d64: 9b157d96    	mul	x22, x12, x21
100058d68: 9bd57d97    	umulh	x23, x12, x21
100058d6c: 9bd57d78    	umulh	x24, x11, x21
100058d70: 9b157d75    	mul	x21, x11, x21
100058d74: ab0702e7    	adds	x7, x23, x7
100058d78: 1a9f37f7    	cset	w23, hs
100058d7c: ab1500e7    	adds	x7, x7, x21
100058d80: 9a9736f5    	cinc	x21, x23, hs
100058d84: ab1800c6    	adds	x6, x6, x24
100058d88: 1a9f37f7    	cset	w23, hs
100058d8c: ab1302df    	cmn	x22, x19
100058d90: ba0800e7    	adcs	x7, x7, x8
100058d94: ba1500c6    	adcs	x6, x6, x21
100058d98: ba170293    	adcs	x19, x20, x23
100058d9c: 1a9f37f4    	cset	w20, hs
100058da0: 9b077db5    	mul	x21, x13, x7
100058da4: 9b157d96    	mul	x22, x12, x21
100058da8: 9bd57d97    	umulh	x23, x12, x21
100058dac: 9bd57d78    	umulh	x24, x11, x21
100058db0: 9b157d75    	mul	x21, x11, x21
100058db4: ab0602e6    	adds	x6, x23, x6
100058db8: 1a9f37f7    	cset	w23, hs
100058dbc: ab1500c6    	adds	x6, x6, x21
100058dc0: 9a9736f5    	cinc	x21, x23, hs
100058dc4: ab180273    	adds	x19, x19, x24
100058dc8: 1a9f37f7    	cset	w23, hs
100058dcc: ab0702df    	cmn	x22, x7
100058dd0: ba0800c6    	adcs	x6, x6, x8
100058dd4: ba150267    	adcs	x7, x19, x21
100058dd8: 9a9736f3    	cinc	x19, x23, hs
100058ddc: eb0c00df    	cmp	x6, x12
100058de0: fa0b00ff    	sbcs	xzr, x7, x11
100058de4: aa140273    	orr	x19, x19, x20
100058de8: fa403a60    	ccmp	x19, #0x0, #0x0, lo
100058dec: 9a881173    	csel	x19, x11, x8, ne
100058df0: 9a881194    	csel	x20, x12, x8, ne
100058df4: eb1400c6    	subs	x6, x6, x20
100058df8: da1300e7    	sbc	x7, x7, x19
100058dfc: ab0500c5    	adds	x5, x6, x5
100058e00: ba0800e6    	adcs	x6, x7, x8
100058e04: 1a9f37e7    	cset	w7, hs
100058e08: eb0c00bf    	cmp	x5, x12
100058e0c: fa0b00df    	sbcs	xzr, x6, x11
100058e10: 1a9f34e7    	csinc	w7, w7, wzr, lo
100058e14: 710000ff    	cmp	w7, #0x0
100058e18: 9a881167    	csel	x7, x11, x8, ne
100058e1c: 9a881193    	csel	x19, x12, x8, ne
100058e20: eb1300a5    	subs	x5, x5, x19
100058e24: 937ffc84    	asr	x4, x4, #63
100058e28: 8a0e0093    	and	x19, x4, x14
100058e2c: da0700c6    	sbc	x6, x6, x7
100058e30: 8a0f0084    	and	x4, x4, x15
100058e34: eb0400a4    	subs	x4, x5, x4
100058e38: fa1300c5    	sbcs	x5, x6, x19
100058e3c: 1a9f27e6    	cset	w6, lo
100058e40: 390023e6    	strb	w6, [sp, #0x8]
100058e44: 394023e6    	ldrb	w6, [sp, #0x8]
100058e48: aa0803e7    	mov	x7, x8
100058e4c: f2401cdf    	tst	x6, #0xff
100058e50: 9a881067    	csel	x7, x3, x8, ne
100058e54: aa0803f3    	mov	x19, x8
100058e58: f2401cdf    	tst	x6, #0xff
100058e5c: 9a881013    	csel	x19, x0, x8, ne
100058e60: ab070084    	adds	x4, x4, x7
100058e64: 9a050265    	adc	x5, x19, x5
100058e68: a93f9624    	stp	x4, x5, [x17, #-0x8]
100058e6c: 91008210    	add	x16, x16, #0x20
100058e70: 91004231    	add	x17, x17, #0x10
100058e74: f1000442    	subs	x2, x2, #0x1
100058e78: 54ffe5c1    	b.ne	0x100058b30 <__RNvMNtNtNtCs8IeiyMGxmWr_17field_regressions8campaign10candidates7integerINtB2_10ProjectionKj4_E5batchB8_+0x4c>
100058e7c: a9447bfd    	ldp	x29, x30, [sp, #0x40]
100058e80: a9434ff4    	ldp	x20, x19, [sp, #0x30]
100058e84: a94257f6    	ldp	x22, x21, [sp, #0x20]
100058e88: a9415ff8    	ldp	x24, x23, [sp, #0x10]
100058e8c: 910143ff    	add	sp, sp, #0x50
100058e90: d65f03c0    	ret
100058e94: 90000904    	adrp	x4, 0x100178000 <dyld_stub_binder+0x100178000>
100058e98: 9124a084    	add	x4, x4, #0x928
100058e9c: 910003e0    	mov	x0, sp
100058ea0: 910023e1    	add	x1, sp, #0x8
100058ea4: d2800002    	mov	x2, #0x0                ; =0
100058ea8: 94034e31    	bl	0x10012c76c <__RINvNtCs8Mbv00yxnRz_4core9panicking13assert_failedjjEB4_>

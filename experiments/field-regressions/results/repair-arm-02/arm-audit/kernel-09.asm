
/private/var/folders/q0/v7l8dzd13k7f_q6j80sy03lm0000gn/T/f2z-field-gated-aejimxw9/field-regressions:	file format mach-o arm64

Disassembly of section __TEXT,__text:

0000000100070b64 <__RNvMNtNtNtCsaHyC9lX8wyC_17field_regressions8campaign10candidates7integerINtB2_10ProjectionKj4_E5batchB8_>:
100070b64: d10143ff    	sub	sp, sp, #0x50
100070b68: a9015ff8    	stp	x24, x23, [sp, #0x10]
100070b6c: a90257f6    	stp	x22, x21, [sp, #0x20]
100070b70: a9034ff4    	stp	x20, x19, [sp, #0x30]
100070b74: a9047bfd    	stp	x29, x30, [sp, #0x40]
100070b78: 910103fd    	add	x29, sp, #0x40
100070b7c: a90013e2    	stp	x2, x4, [sp]
100070b80: eb04005f    	cmp	x2, x4
100070b84: 54001c81    	b.ne	0x100070f14 <__RNvMNtNtNtCsaHyC9lX8wyC_17field_regressions8campaign10candidates7integerINtB2_10ProjectionKj4_E5batchB8_+0x3b0>
100070b88: b4001ba2    	cbz	x2, 0x100070efc <__RNvMNtNtNtCsaHyC9lX8wyC_17field_regressions8campaign10candidates7integerINtB2_10ProjectionKj4_E5batchB8_+0x398>
100070b8c: d2800008    	mov	x8, #0x0                ; =0
100070b90: a949240a    	ldp	x10, x9, [x0, #0x90]
100070b94: a9442c0c    	ldp	x12, x11, [x0, #0x40]
100070b98: f940380d    	ldr	x13, [x0, #0x70]
100070b9c: a94a380f    	ldp	x15, x14, [x0, #0xa0]
100070ba0: 91004030    	add	x16, x1, #0x10
100070ba4: 91002071    	add	x17, x3, #0x8
100070ba8: 910023e1    	add	x1, sp, #0x8
100070bac: a9480003    	ldp	x3, x0, [x0, #0x80]
100070bb0: a9401205    	ldp	x5, x4, [x16]
100070bb4: 9b047d46    	mul	x6, x10, x4
100070bb8: 9bc47d47    	umulh	x7, x10, x4
100070bbc: 9bc47d33    	umulh	x19, x9, x4
100070bc0: 9b047d34    	mul	x20, x9, x4
100070bc4: ab1400e7    	adds	x7, x7, x20
100070bc8: ba080273    	adcs	x19, x19, x8
100070bcc: 9b067db4    	mul	x20, x13, x6
100070bd0: 9b147d95    	mul	x21, x12, x20
100070bd4: 9bd47d96    	umulh	x22, x12, x20
100070bd8: 9bd47d77    	umulh	x23, x11, x20
100070bdc: 9b147d74    	mul	x20, x11, x20
100070be0: 1a9f37f8    	cset	w24, hs
100070be4: ab0702c7    	adds	x7, x22, x7
100070be8: 1a9f37f6    	cset	w22, hs
100070bec: ab1400e7    	adds	x7, x7, x20
100070bf0: 9a9636d4    	cinc	x20, x22, hs
100070bf4: ab170273    	adds	x19, x19, x23
100070bf8: 9a983716    	cinc	x22, x24, hs
100070bfc: ab0602bf    	cmn	x21, x6
100070c00: ba0800e6    	adcs	x6, x7, x8
100070c04: ba140267    	adcs	x7, x19, x20
100070c08: 9b067db3    	mul	x19, x13, x6
100070c0c: 9b137d94    	mul	x20, x12, x19
100070c10: 9bd37d95    	umulh	x21, x12, x19
100070c14: 9b137d77    	mul	x23, x11, x19
100070c18: 9bd37d73    	umulh	x19, x11, x19
100070c1c: ba160273    	adcs	x19, x19, x22
100070c20: 1a9f37f6    	cset	w22, hs
100070c24: ab0702a7    	adds	x7, x21, x7
100070c28: 1a9f37f5    	cset	w21, hs
100070c2c: ab1700e7    	adds	x7, x7, x23
100070c30: 9a9536b5    	cinc	x21, x21, hs
100070c34: ab06029f    	cmn	x20, x6
100070c38: ba0800e6    	adcs	x6, x7, x8
100070c3c: ba150267    	adcs	x7, x19, x21
100070c40: 9a9636d3    	cinc	x19, x22, hs
100070c44: eb0c00df    	cmp	x6, x12
100070c48: fa0b00ff    	sbcs	xzr, x7, x11
100070c4c: fa403a60    	ccmp	x19, #0x0, #0x0, lo
100070c50: 9a881173    	csel	x19, x11, x8, ne
100070c54: 9a881194    	csel	x20, x12, x8, ne
100070c58: eb1400c6    	subs	x6, x6, x20
100070c5c: da1300e7    	sbc	x7, x7, x19
100070c60: ab0500c5    	adds	x5, x6, x5
100070c64: ba0800e6    	adcs	x6, x7, x8
100070c68: 1a9f37e7    	cset	w7, hs
100070c6c: eb0c00bf    	cmp	x5, x12
100070c70: fa0b00df    	sbcs	xzr, x6, x11
100070c74: 1a9f34e7    	csinc	w7, w7, wzr, lo
100070c78: 710000ff    	cmp	w7, #0x0
100070c7c: 9a881167    	csel	x7, x11, x8, ne
100070c80: 9a881193    	csel	x19, x12, x8, ne
100070c84: eb1300a5    	subs	x5, x5, x19
100070c88: da0700c6    	sbc	x6, x6, x7
100070c8c: 9b057d47    	mul	x7, x10, x5
100070c90: 9bc57d53    	umulh	x19, x10, x5
100070c94: 9bc57d34    	umulh	x20, x9, x5
100070c98: 9b057d25    	mul	x5, x9, x5
100070c9c: 9b067d55    	mul	x21, x10, x6
100070ca0: 9bc67d56    	umulh	x22, x10, x6
100070ca4: 9bc67d37    	umulh	x23, x9, x6
100070ca8: 9b067d26    	mul	x6, x9, x6
100070cac: ab050265    	adds	x5, x19, x5
100070cb0: 1a9f37f3    	cset	w19, hs
100070cb4: ab1402d4    	adds	x20, x22, x20
100070cb8: 1a9f37f6    	cset	w22, hs
100070cbc: ab060286    	adds	x6, x20, x6
100070cc0: 9a9636d4    	cinc	x20, x22, hs
100070cc4: ab1500a5    	adds	x5, x5, x21
100070cc8: ba1300c6    	adcs	x6, x6, x19
100070ccc: 9b077db3    	mul	x19, x13, x7
100070cd0: 9a1402f4    	adc	x20, x23, x20
100070cd4: 9b137d95    	mul	x21, x12, x19
100070cd8: 9bd37d96    	umulh	x22, x12, x19
100070cdc: 9bd37d77    	umulh	x23, x11, x19
100070ce0: 9b137d73    	mul	x19, x11, x19
100070ce4: ab0502c5    	adds	x5, x22, x5
100070ce8: 1a9f37f6    	cset	w22, hs
100070cec: ab1300a5    	adds	x5, x5, x19
100070cf0: 9a9636d3    	cinc	x19, x22, hs
100070cf4: ab1700c6    	adds	x6, x6, x23
100070cf8: 1a9f37f6    	cset	w22, hs
100070cfc: ab0702bf    	cmn	x21, x7
100070d00: ba0800a5    	adcs	x5, x5, x8
100070d04: ba1300c6    	adcs	x6, x6, x19
100070d08: ba160287    	adcs	x7, x20, x22
100070d0c: 1a9f37f3    	cset	w19, hs
100070d10: 9b057db4    	mul	x20, x13, x5
100070d14: 9bd47d95    	umulh	x21, x12, x20
100070d18: 9b147d96    	mul	x22, x12, x20
100070d1c: 9bd47d77    	umulh	x23, x11, x20
100070d20: 9b147d74    	mul	x20, x11, x20
100070d24: ab0602a6    	adds	x6, x21, x6
100070d28: 1a9f37f5    	cset	w21, hs
100070d2c: ab1400c6    	adds	x6, x6, x20
100070d30: 9a9536b4    	cinc	x20, x21, hs
100070d34: ab1700e7    	adds	x7, x7, x23
100070d38: 1a9f37f5    	cset	w21, hs
100070d3c: ab0502df    	cmn	x22, x5
100070d40: ba0800c5    	adcs	x5, x6, x8
100070d44: ba1400e6    	adcs	x6, x7, x20
100070d48: 9a9536a7    	cinc	x7, x21, hs
100070d4c: eb0c00bf    	cmp	x5, x12
100070d50: fa0b00df    	sbcs	xzr, x6, x11
100070d54: aa1300e7    	orr	x7, x7, x19
100070d58: fa4038e0    	ccmp	x7, #0x0, #0x0, lo
100070d5c: 9a881167    	csel	x7, x11, x8, ne
100070d60: 9a881193    	csel	x19, x12, x8, ne
100070d64: eb1300b3    	subs	x19, x5, x19
100070d68: da0700c6    	sbc	x6, x6, x7
100070d6c: a97f1e05    	ldp	x5, x7, [x16, #-0x10]
100070d70: ab070267    	adds	x7, x19, x7
100070d74: ba0800c6    	adcs	x6, x6, x8
100070d78: 1a9f37f3    	cset	w19, hs
100070d7c: eb0c00ff    	cmp	x7, x12
100070d80: fa0b00df    	sbcs	xzr, x6, x11
100070d84: 1a9f3673    	csinc	w19, w19, wzr, lo
100070d88: 7100027f    	cmp	w19, #0x0
100070d8c: 9a881173    	csel	x19, x11, x8, ne
100070d90: 9a881194    	csel	x20, x12, x8, ne
100070d94: eb1400e7    	subs	x7, x7, x20
100070d98: da1300c6    	sbc	x6, x6, x19
100070d9c: 9b077d53    	mul	x19, x10, x7
100070da0: 9bc77d54    	umulh	x20, x10, x7
100070da4: 9bc77d35    	umulh	x21, x9, x7
100070da8: 9b077d27    	mul	x7, x9, x7
100070dac: 9b067d56    	mul	x22, x10, x6
100070db0: 9bc67d57    	umulh	x23, x10, x6
100070db4: 9bc67d38    	umulh	x24, x9, x6
100070db8: 9b067d26    	mul	x6, x9, x6
100070dbc: ab070287    	adds	x7, x20, x7
100070dc0: 1a9f37f4    	cset	w20, hs
100070dc4: ab1502f5    	adds	x21, x23, x21
100070dc8: 1a9f37f7    	cset	w23, hs
100070dcc: ab0602a6    	adds	x6, x21, x6
100070dd0: 9a9736f5    	cinc	x21, x23, hs
100070dd4: ab1600e7    	adds	x7, x7, x22
100070dd8: ba1400c6    	adcs	x6, x6, x20
100070ddc: 9a150314    	adc	x20, x24, x21
100070de0: 9b137db5    	mul	x21, x13, x19
100070de4: 9b157d96    	mul	x22, x12, x21
100070de8: 9bd57d97    	umulh	x23, x12, x21
100070dec: 9bd57d78    	umulh	x24, x11, x21
100070df0: 9b157d75    	mul	x21, x11, x21
100070df4: ab0702e7    	adds	x7, x23, x7
100070df8: 1a9f37f7    	cset	w23, hs
100070dfc: ab1500e7    	adds	x7, x7, x21
100070e00: 9a9736f5    	cinc	x21, x23, hs
100070e04: ab1800c6    	adds	x6, x6, x24
100070e08: 1a9f37f7    	cset	w23, hs
100070e0c: ab1302df    	cmn	x22, x19
100070e10: ba0800e7    	adcs	x7, x7, x8
100070e14: ba1500c6    	adcs	x6, x6, x21
100070e18: ba170293    	adcs	x19, x20, x23
100070e1c: 1a9f37f4    	cset	w20, hs
100070e20: 9b077db5    	mul	x21, x13, x7
100070e24: 9b157d96    	mul	x22, x12, x21
100070e28: 9bd57d97    	umulh	x23, x12, x21
100070e2c: 9bd57d78    	umulh	x24, x11, x21
100070e30: 9b157d75    	mul	x21, x11, x21
100070e34: ab0602e6    	adds	x6, x23, x6
100070e38: 1a9f37f7    	cset	w23, hs
100070e3c: ab1500c6    	adds	x6, x6, x21
100070e40: 9a9736f5    	cinc	x21, x23, hs
100070e44: ab180273    	adds	x19, x19, x24
100070e48: 1a9f37f7    	cset	w23, hs
100070e4c: ab0702df    	cmn	x22, x7
100070e50: ba0800c6    	adcs	x6, x6, x8
100070e54: ba150267    	adcs	x7, x19, x21
100070e58: 9a9736f3    	cinc	x19, x23, hs
100070e5c: eb0c00df    	cmp	x6, x12
100070e60: fa0b00ff    	sbcs	xzr, x7, x11
100070e64: aa140273    	orr	x19, x19, x20
100070e68: fa403a60    	ccmp	x19, #0x0, #0x0, lo
100070e6c: 9a881173    	csel	x19, x11, x8, ne
100070e70: 9a881194    	csel	x20, x12, x8, ne
100070e74: eb1400c6    	subs	x6, x6, x20
100070e78: da1300e7    	sbc	x7, x7, x19
100070e7c: ab0500c5    	adds	x5, x6, x5
100070e80: ba0800e6    	adcs	x6, x7, x8
100070e84: 1a9f37e7    	cset	w7, hs
100070e88: eb0c00bf    	cmp	x5, x12
100070e8c: fa0b00df    	sbcs	xzr, x6, x11
100070e90: 1a9f34e7    	csinc	w7, w7, wzr, lo
100070e94: 710000ff    	cmp	w7, #0x0
100070e98: 9a881167    	csel	x7, x11, x8, ne
100070e9c: 9a881193    	csel	x19, x12, x8, ne
100070ea0: eb1300a5    	subs	x5, x5, x19
100070ea4: 937ffc84    	asr	x4, x4, #63
100070ea8: 8a0e0093    	and	x19, x4, x14
100070eac: da0700c6    	sbc	x6, x6, x7
100070eb0: 8a0f0084    	and	x4, x4, x15
100070eb4: eb0400a4    	subs	x4, x5, x4
100070eb8: fa1300c5    	sbcs	x5, x6, x19
100070ebc: 1a9f27e6    	cset	w6, lo
100070ec0: 390023e6    	strb	w6, [sp, #0x8]
100070ec4: 394023e6    	ldrb	w6, [sp, #0x8]
100070ec8: aa0803e7    	mov	x7, x8
100070ecc: f2401cdf    	tst	x6, #0xff
100070ed0: 9a881067    	csel	x7, x3, x8, ne
100070ed4: aa0803f3    	mov	x19, x8
100070ed8: f2401cdf    	tst	x6, #0xff
100070edc: 9a881013    	csel	x19, x0, x8, ne
100070ee0: ab070084    	adds	x4, x4, x7
100070ee4: 9a050265    	adc	x5, x19, x5
100070ee8: a93f9624    	stp	x4, x5, [x17, #-0x8]
100070eec: 91008210    	add	x16, x16, #0x20
100070ef0: 91004231    	add	x17, x17, #0x10
100070ef4: f1000442    	subs	x2, x2, #0x1
100070ef8: 54ffe5c1    	b.ne	0x100070bb0 <__RNvMNtNtNtCsaHyC9lX8wyC_17field_regressions8campaign10candidates7integerINtB2_10ProjectionKj4_E5batchB8_+0x4c>
100070efc: a9447bfd    	ldp	x29, x30, [sp, #0x40]
100070f00: a9434ff4    	ldp	x20, x19, [sp, #0x30]
100070f04: a94257f6    	ldp	x22, x21, [sp, #0x20]
100070f08: a9415ff8    	ldp	x24, x23, [sp, #0x10]
100070f0c: 910143ff    	add	sp, sp, #0x50
100070f10: d65f03c0    	ret
100070f14: b0000964    	adrp	x4, 0x10019d000 <dyld_stub_binder+0x10019d000>
100070f18: 910fc084    	add	x4, x4, #0x3f0
100070f1c: 910003e0    	mov	x0, sp
100070f20: 910023e1    	add	x1, sp, #0x8
100070f24: d2800002    	mov	x2, #0x0                ; =0
100070f28: 94036a90    	bl	0x10014b968 <__RINvNtCs8Mbv00yxnRz_4core9panicking13assert_failedjjEB4_>

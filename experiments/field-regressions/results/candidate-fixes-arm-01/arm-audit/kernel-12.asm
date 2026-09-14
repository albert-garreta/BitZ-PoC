
/private/var/folders/q0/v7l8dzd13k7f_q6j80sy03lm0000gn/T/f2z-field-gated-pycl0nhx/field-regressions:	file format mach-o arm64

Disassembly of section __TEXT,__text:

000000010007be98 <__RNvMNtNtNtCsagpXUhzPPpl_17field_regressions8campaign10candidates7integerINtB2_10ProjectionKj4_E5batchB8_>:
10007be98: d10143ff    	sub	sp, sp, #0x50
10007be9c: a9015ff8    	stp	x24, x23, [sp, #0x10]
10007bea0: a90257f6    	stp	x22, x21, [sp, #0x20]
10007bea4: a9034ff4    	stp	x20, x19, [sp, #0x30]
10007bea8: a9047bfd    	stp	x29, x30, [sp, #0x40]
10007beac: 910103fd    	add	x29, sp, #0x40
10007beb0: a90013e2    	stp	x2, x4, [sp]
10007beb4: eb04005f    	cmp	x2, x4
10007beb8: 54001c81    	b.ne	0x10007c248 <__RNvMNtNtNtCsagpXUhzPPpl_17field_regressions8campaign10candidates7integerINtB2_10ProjectionKj4_E5batchB8_+0x3b0>
10007bebc: b4001ba2    	cbz	x2, 0x10007c230 <__RNvMNtNtNtCsagpXUhzPPpl_17field_regressions8campaign10candidates7integerINtB2_10ProjectionKj4_E5batchB8_+0x398>
10007bec0: d2800008    	mov	x8, #0x0                ; =0
10007bec4: a949240a    	ldp	x10, x9, [x0, #0x90]
10007bec8: a9442c0c    	ldp	x12, x11, [x0, #0x40]
10007becc: f940380d    	ldr	x13, [x0, #0x70]
10007bed0: a94a380f    	ldp	x15, x14, [x0, #0xa0]
10007bed4: 91004030    	add	x16, x1, #0x10
10007bed8: 91002071    	add	x17, x3, #0x8
10007bedc: 910023e1    	add	x1, sp, #0x8
10007bee0: a9480003    	ldp	x3, x0, [x0, #0x80]
10007bee4: a9401205    	ldp	x5, x4, [x16]
10007bee8: 9b047d46    	mul	x6, x10, x4
10007beec: 9bc47d47    	umulh	x7, x10, x4
10007bef0: 9bc47d33    	umulh	x19, x9, x4
10007bef4: 9b047d34    	mul	x20, x9, x4
10007bef8: ab1400e7    	adds	x7, x7, x20
10007befc: ba080273    	adcs	x19, x19, x8
10007bf00: 9b067db4    	mul	x20, x13, x6
10007bf04: 9b147d95    	mul	x21, x12, x20
10007bf08: 9bd47d96    	umulh	x22, x12, x20
10007bf0c: 9bd47d77    	umulh	x23, x11, x20
10007bf10: 9b147d74    	mul	x20, x11, x20
10007bf14: 1a9f37f8    	cset	w24, hs
10007bf18: ab0702c7    	adds	x7, x22, x7
10007bf1c: 1a9f37f6    	cset	w22, hs
10007bf20: ab1400e7    	adds	x7, x7, x20
10007bf24: 9a9636d4    	cinc	x20, x22, hs
10007bf28: ab170273    	adds	x19, x19, x23
10007bf2c: 9a983716    	cinc	x22, x24, hs
10007bf30: ab0602bf    	cmn	x21, x6
10007bf34: ba0800e6    	adcs	x6, x7, x8
10007bf38: ba140267    	adcs	x7, x19, x20
10007bf3c: 9b067db3    	mul	x19, x13, x6
10007bf40: 9b137d94    	mul	x20, x12, x19
10007bf44: 9bd37d95    	umulh	x21, x12, x19
10007bf48: 9b137d77    	mul	x23, x11, x19
10007bf4c: 9bd37d73    	umulh	x19, x11, x19
10007bf50: ba160273    	adcs	x19, x19, x22
10007bf54: 1a9f37f6    	cset	w22, hs
10007bf58: ab0702a7    	adds	x7, x21, x7
10007bf5c: 1a9f37f5    	cset	w21, hs
10007bf60: ab1700e7    	adds	x7, x7, x23
10007bf64: 9a9536b5    	cinc	x21, x21, hs
10007bf68: ab06029f    	cmn	x20, x6
10007bf6c: ba0800e6    	adcs	x6, x7, x8
10007bf70: ba150267    	adcs	x7, x19, x21
10007bf74: 9a9636d3    	cinc	x19, x22, hs
10007bf78: eb0c00df    	cmp	x6, x12
10007bf7c: fa0b00ff    	sbcs	xzr, x7, x11
10007bf80: fa403a60    	ccmp	x19, #0x0, #0x0, lo
10007bf84: 9a881173    	csel	x19, x11, x8, ne
10007bf88: 9a881194    	csel	x20, x12, x8, ne
10007bf8c: eb1400c6    	subs	x6, x6, x20
10007bf90: da1300e7    	sbc	x7, x7, x19
10007bf94: ab0500c5    	adds	x5, x6, x5
10007bf98: ba0800e6    	adcs	x6, x7, x8
10007bf9c: 1a9f37e7    	cset	w7, hs
10007bfa0: eb0c00bf    	cmp	x5, x12
10007bfa4: fa0b00df    	sbcs	xzr, x6, x11
10007bfa8: 1a9f34e7    	csinc	w7, w7, wzr, lo
10007bfac: 710000ff    	cmp	w7, #0x0
10007bfb0: 9a881167    	csel	x7, x11, x8, ne
10007bfb4: 9a881193    	csel	x19, x12, x8, ne
10007bfb8: eb1300a5    	subs	x5, x5, x19
10007bfbc: da0700c6    	sbc	x6, x6, x7
10007bfc0: 9b057d47    	mul	x7, x10, x5
10007bfc4: 9bc57d53    	umulh	x19, x10, x5
10007bfc8: 9bc57d34    	umulh	x20, x9, x5
10007bfcc: 9b057d25    	mul	x5, x9, x5
10007bfd0: 9b067d55    	mul	x21, x10, x6
10007bfd4: 9bc67d56    	umulh	x22, x10, x6
10007bfd8: 9bc67d37    	umulh	x23, x9, x6
10007bfdc: 9b067d26    	mul	x6, x9, x6
10007bfe0: ab050265    	adds	x5, x19, x5
10007bfe4: 1a9f37f3    	cset	w19, hs
10007bfe8: ab1402d4    	adds	x20, x22, x20
10007bfec: 1a9f37f6    	cset	w22, hs
10007bff0: ab060286    	adds	x6, x20, x6
10007bff4: 9a9636d4    	cinc	x20, x22, hs
10007bff8: ab1500a5    	adds	x5, x5, x21
10007bffc: ba1300c6    	adcs	x6, x6, x19
10007c000: 9b077db3    	mul	x19, x13, x7
10007c004: 9a1402f4    	adc	x20, x23, x20
10007c008: 9b137d95    	mul	x21, x12, x19
10007c00c: 9bd37d96    	umulh	x22, x12, x19
10007c010: 9bd37d77    	umulh	x23, x11, x19
10007c014: 9b137d73    	mul	x19, x11, x19
10007c018: ab0502c5    	adds	x5, x22, x5
10007c01c: 1a9f37f6    	cset	w22, hs
10007c020: ab1300a5    	adds	x5, x5, x19
10007c024: 9a9636d3    	cinc	x19, x22, hs
10007c028: ab1700c6    	adds	x6, x6, x23
10007c02c: 1a9f37f6    	cset	w22, hs
10007c030: ab0702bf    	cmn	x21, x7
10007c034: ba0800a5    	adcs	x5, x5, x8
10007c038: ba1300c6    	adcs	x6, x6, x19
10007c03c: ba160287    	adcs	x7, x20, x22
10007c040: 1a9f37f3    	cset	w19, hs
10007c044: 9b057db4    	mul	x20, x13, x5
10007c048: 9bd47d95    	umulh	x21, x12, x20
10007c04c: 9b147d96    	mul	x22, x12, x20
10007c050: 9bd47d77    	umulh	x23, x11, x20
10007c054: 9b147d74    	mul	x20, x11, x20
10007c058: ab0602a6    	adds	x6, x21, x6
10007c05c: 1a9f37f5    	cset	w21, hs
10007c060: ab1400c6    	adds	x6, x6, x20
10007c064: 9a9536b4    	cinc	x20, x21, hs
10007c068: ab1700e7    	adds	x7, x7, x23
10007c06c: 1a9f37f5    	cset	w21, hs
10007c070: ab0502df    	cmn	x22, x5
10007c074: ba0800c5    	adcs	x5, x6, x8
10007c078: ba1400e6    	adcs	x6, x7, x20
10007c07c: 9a9536a7    	cinc	x7, x21, hs
10007c080: eb0c00bf    	cmp	x5, x12
10007c084: fa0b00df    	sbcs	xzr, x6, x11
10007c088: aa1300e7    	orr	x7, x7, x19
10007c08c: fa4038e0    	ccmp	x7, #0x0, #0x0, lo
10007c090: 9a881167    	csel	x7, x11, x8, ne
10007c094: 9a881193    	csel	x19, x12, x8, ne
10007c098: eb1300b3    	subs	x19, x5, x19
10007c09c: da0700c6    	sbc	x6, x6, x7
10007c0a0: a97f1e05    	ldp	x5, x7, [x16, #-0x10]
10007c0a4: ab070267    	adds	x7, x19, x7
10007c0a8: ba0800c6    	adcs	x6, x6, x8
10007c0ac: 1a9f37f3    	cset	w19, hs
10007c0b0: eb0c00ff    	cmp	x7, x12
10007c0b4: fa0b00df    	sbcs	xzr, x6, x11
10007c0b8: 1a9f3673    	csinc	w19, w19, wzr, lo
10007c0bc: 7100027f    	cmp	w19, #0x0
10007c0c0: 9a881173    	csel	x19, x11, x8, ne
10007c0c4: 9a881194    	csel	x20, x12, x8, ne
10007c0c8: eb1400e7    	subs	x7, x7, x20
10007c0cc: da1300c6    	sbc	x6, x6, x19
10007c0d0: 9b077d53    	mul	x19, x10, x7
10007c0d4: 9bc77d54    	umulh	x20, x10, x7
10007c0d8: 9bc77d35    	umulh	x21, x9, x7
10007c0dc: 9b077d27    	mul	x7, x9, x7
10007c0e0: 9b067d56    	mul	x22, x10, x6
10007c0e4: 9bc67d57    	umulh	x23, x10, x6
10007c0e8: 9bc67d38    	umulh	x24, x9, x6
10007c0ec: 9b067d26    	mul	x6, x9, x6
10007c0f0: ab070287    	adds	x7, x20, x7
10007c0f4: 1a9f37f4    	cset	w20, hs
10007c0f8: ab1502f5    	adds	x21, x23, x21
10007c0fc: 1a9f37f7    	cset	w23, hs
10007c100: ab0602a6    	adds	x6, x21, x6
10007c104: 9a9736f5    	cinc	x21, x23, hs
10007c108: ab1600e7    	adds	x7, x7, x22
10007c10c: ba1400c6    	adcs	x6, x6, x20
10007c110: 9a150314    	adc	x20, x24, x21
10007c114: 9b137db5    	mul	x21, x13, x19
10007c118: 9b157d96    	mul	x22, x12, x21
10007c11c: 9bd57d97    	umulh	x23, x12, x21
10007c120: 9bd57d78    	umulh	x24, x11, x21
10007c124: 9b157d75    	mul	x21, x11, x21
10007c128: ab0702e7    	adds	x7, x23, x7
10007c12c: 1a9f37f7    	cset	w23, hs
10007c130: ab1500e7    	adds	x7, x7, x21
10007c134: 9a9736f5    	cinc	x21, x23, hs
10007c138: ab1800c6    	adds	x6, x6, x24
10007c13c: 1a9f37f7    	cset	w23, hs
10007c140: ab1302df    	cmn	x22, x19
10007c144: ba0800e7    	adcs	x7, x7, x8
10007c148: ba1500c6    	adcs	x6, x6, x21
10007c14c: ba170293    	adcs	x19, x20, x23
10007c150: 1a9f37f4    	cset	w20, hs
10007c154: 9b077db5    	mul	x21, x13, x7
10007c158: 9b157d96    	mul	x22, x12, x21
10007c15c: 9bd57d97    	umulh	x23, x12, x21
10007c160: 9bd57d78    	umulh	x24, x11, x21
10007c164: 9b157d75    	mul	x21, x11, x21
10007c168: ab0602e6    	adds	x6, x23, x6
10007c16c: 1a9f37f7    	cset	w23, hs
10007c170: ab1500c6    	adds	x6, x6, x21
10007c174: 9a9736f5    	cinc	x21, x23, hs
10007c178: ab180273    	adds	x19, x19, x24
10007c17c: 1a9f37f7    	cset	w23, hs
10007c180: ab0702df    	cmn	x22, x7
10007c184: ba0800c6    	adcs	x6, x6, x8
10007c188: ba150267    	adcs	x7, x19, x21
10007c18c: 9a9736f3    	cinc	x19, x23, hs
10007c190: eb0c00df    	cmp	x6, x12
10007c194: fa0b00ff    	sbcs	xzr, x7, x11
10007c198: aa140273    	orr	x19, x19, x20
10007c19c: fa403a60    	ccmp	x19, #0x0, #0x0, lo
10007c1a0: 9a881173    	csel	x19, x11, x8, ne
10007c1a4: 9a881194    	csel	x20, x12, x8, ne
10007c1a8: eb1400c6    	subs	x6, x6, x20
10007c1ac: da1300e7    	sbc	x7, x7, x19
10007c1b0: ab0500c5    	adds	x5, x6, x5
10007c1b4: ba0800e6    	adcs	x6, x7, x8
10007c1b8: 1a9f37e7    	cset	w7, hs
10007c1bc: eb0c00bf    	cmp	x5, x12
10007c1c0: fa0b00df    	sbcs	xzr, x6, x11
10007c1c4: 1a9f34e7    	csinc	w7, w7, wzr, lo
10007c1c8: 710000ff    	cmp	w7, #0x0
10007c1cc: 9a881167    	csel	x7, x11, x8, ne
10007c1d0: 9a881193    	csel	x19, x12, x8, ne
10007c1d4: eb1300a5    	subs	x5, x5, x19
10007c1d8: 937ffc84    	asr	x4, x4, #63
10007c1dc: 8a0e0093    	and	x19, x4, x14
10007c1e0: da0700c6    	sbc	x6, x6, x7
10007c1e4: 8a0f0084    	and	x4, x4, x15
10007c1e8: eb0400a4    	subs	x4, x5, x4
10007c1ec: fa1300c5    	sbcs	x5, x6, x19
10007c1f0: 1a9f27e6    	cset	w6, lo
10007c1f4: 390023e6    	strb	w6, [sp, #0x8]
10007c1f8: 394023e6    	ldrb	w6, [sp, #0x8]
10007c1fc: aa0803e7    	mov	x7, x8
10007c200: f2401cdf    	tst	x6, #0xff
10007c204: 9a881067    	csel	x7, x3, x8, ne
10007c208: aa0803f3    	mov	x19, x8
10007c20c: f2401cdf    	tst	x6, #0xff
10007c210: 9a881013    	csel	x19, x0, x8, ne
10007c214: ab070084    	adds	x4, x4, x7
10007c218: 9a050265    	adc	x5, x19, x5
10007c21c: a93f9624    	stp	x4, x5, [x17, #-0x8]
10007c220: 91008210    	add	x16, x16, #0x20
10007c224: 91004231    	add	x17, x17, #0x10
10007c228: f1000442    	subs	x2, x2, #0x1
10007c22c: 54ffe5c1    	b.ne	0x10007bee4 <__RNvMNtNtNtCsagpXUhzPPpl_17field_regressions8campaign10candidates7integerINtB2_10ProjectionKj4_E5batchB8_+0x4c>
10007c230: a9447bfd    	ldp	x29, x30, [sp, #0x40]
10007c234: a9434ff4    	ldp	x20, x19, [sp, #0x30]
10007c238: a94257f6    	ldp	x22, x21, [sp, #0x20]
10007c23c: a9415ff8    	ldp	x24, x23, [sp, #0x10]
10007c240: 910143ff    	add	sp, sp, #0x50
10007c244: d65f03c0    	ret
10007c248: b00009a4    	adrp	x4, 0x1001b1000 <dyld_stub_binder+0x1001b1000>
10007c24c: 912ec084    	add	x4, x4, #0xbb0
10007c250: 910003e0    	mov	x0, sp
10007c254: 910023e1    	add	x1, sp, #0x8
10007c258: d2800002    	mov	x2, #0x0                ; =0
10007c25c: 94037cfd    	bl	0x10015b650 <__RINvNtCs8Mbv00yxnRz_4core9panicking13assert_failedjjEB4_>

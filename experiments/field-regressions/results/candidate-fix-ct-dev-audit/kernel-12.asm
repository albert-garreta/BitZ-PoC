
/private/tmp/f2z-arithmetic-target/release/field-regressions:	file format mach-o arm64

Disassembly of section __TEXT,__text:

000000010007bebc <__RNvMNtNtNtCshgIkoLp8A2V_17field_regressions8campaign10candidates7integerINtB2_10ProjectionKj4_E5batchB8_>:
10007bebc: d10143ff    	sub	sp, sp, #0x50
10007bec0: a9015ff8    	stp	x24, x23, [sp, #0x10]
10007bec4: a90257f6    	stp	x22, x21, [sp, #0x20]
10007bec8: a9034ff4    	stp	x20, x19, [sp, #0x30]
10007becc: a9047bfd    	stp	x29, x30, [sp, #0x40]
10007bed0: 910103fd    	add	x29, sp, #0x40
10007bed4: a90013e2    	stp	x2, x4, [sp]
10007bed8: eb04005f    	cmp	x2, x4
10007bedc: 54001c81    	b.ne	0x10007c26c <__RNvMNtNtNtCshgIkoLp8A2V_17field_regressions8campaign10candidates7integerINtB2_10ProjectionKj4_E5batchB8_+0x3b0>
10007bee0: b4001ba2    	cbz	x2, 0x10007c254 <__RNvMNtNtNtCshgIkoLp8A2V_17field_regressions8campaign10candidates7integerINtB2_10ProjectionKj4_E5batchB8_+0x398>
10007bee4: d2800008    	mov	x8, #0x0                ; =0
10007bee8: a949240a    	ldp	x10, x9, [x0, #0x90]
10007beec: a9442c0c    	ldp	x12, x11, [x0, #0x40]
10007bef0: f940380d    	ldr	x13, [x0, #0x70]
10007bef4: a94a380f    	ldp	x15, x14, [x0, #0xa0]
10007bef8: 91004030    	add	x16, x1, #0x10
10007befc: 91002071    	add	x17, x3, #0x8
10007bf00: 910023e1    	add	x1, sp, #0x8
10007bf04: a9480003    	ldp	x3, x0, [x0, #0x80]
10007bf08: a9401205    	ldp	x5, x4, [x16]
10007bf0c: 9b047d46    	mul	x6, x10, x4
10007bf10: 9bc47d47    	umulh	x7, x10, x4
10007bf14: 9bc47d33    	umulh	x19, x9, x4
10007bf18: 9b047d34    	mul	x20, x9, x4
10007bf1c: ab1400e7    	adds	x7, x7, x20
10007bf20: ba080273    	adcs	x19, x19, x8
10007bf24: 9b067db4    	mul	x20, x13, x6
10007bf28: 9b147d95    	mul	x21, x12, x20
10007bf2c: 9bd47d96    	umulh	x22, x12, x20
10007bf30: 9bd47d77    	umulh	x23, x11, x20
10007bf34: 9b147d74    	mul	x20, x11, x20
10007bf38: 1a9f37f8    	cset	w24, hs
10007bf3c: ab0702c7    	adds	x7, x22, x7
10007bf40: 1a9f37f6    	cset	w22, hs
10007bf44: ab1400e7    	adds	x7, x7, x20
10007bf48: 9a9636d4    	cinc	x20, x22, hs
10007bf4c: ab170273    	adds	x19, x19, x23
10007bf50: 9a983716    	cinc	x22, x24, hs
10007bf54: ab0602bf    	cmn	x21, x6
10007bf58: ba0800e6    	adcs	x6, x7, x8
10007bf5c: ba140267    	adcs	x7, x19, x20
10007bf60: 9b067db3    	mul	x19, x13, x6
10007bf64: 9b137d94    	mul	x20, x12, x19
10007bf68: 9bd37d95    	umulh	x21, x12, x19
10007bf6c: 9b137d77    	mul	x23, x11, x19
10007bf70: 9bd37d73    	umulh	x19, x11, x19
10007bf74: ba160273    	adcs	x19, x19, x22
10007bf78: 1a9f37f6    	cset	w22, hs
10007bf7c: ab0702a7    	adds	x7, x21, x7
10007bf80: 1a9f37f5    	cset	w21, hs
10007bf84: ab1700e7    	adds	x7, x7, x23
10007bf88: 9a9536b5    	cinc	x21, x21, hs
10007bf8c: ab06029f    	cmn	x20, x6
10007bf90: ba0800e6    	adcs	x6, x7, x8
10007bf94: ba150267    	adcs	x7, x19, x21
10007bf98: 9a9636d3    	cinc	x19, x22, hs
10007bf9c: eb0c00df    	cmp	x6, x12
10007bfa0: fa0b00ff    	sbcs	xzr, x7, x11
10007bfa4: fa403a60    	ccmp	x19, #0x0, #0x0, lo
10007bfa8: 9a881173    	csel	x19, x11, x8, ne
10007bfac: 9a881194    	csel	x20, x12, x8, ne
10007bfb0: eb1400c6    	subs	x6, x6, x20
10007bfb4: da1300e7    	sbc	x7, x7, x19
10007bfb8: ab0500c5    	adds	x5, x6, x5
10007bfbc: ba0800e6    	adcs	x6, x7, x8
10007bfc0: 1a9f37e7    	cset	w7, hs
10007bfc4: eb0c00bf    	cmp	x5, x12
10007bfc8: fa0b00df    	sbcs	xzr, x6, x11
10007bfcc: 1a9f34e7    	csinc	w7, w7, wzr, lo
10007bfd0: 710000ff    	cmp	w7, #0x0
10007bfd4: 9a881167    	csel	x7, x11, x8, ne
10007bfd8: 9a881193    	csel	x19, x12, x8, ne
10007bfdc: eb1300a5    	subs	x5, x5, x19
10007bfe0: da0700c6    	sbc	x6, x6, x7
10007bfe4: 9b057d47    	mul	x7, x10, x5
10007bfe8: 9bc57d53    	umulh	x19, x10, x5
10007bfec: 9bc57d34    	umulh	x20, x9, x5
10007bff0: 9b057d25    	mul	x5, x9, x5
10007bff4: 9b067d55    	mul	x21, x10, x6
10007bff8: 9bc67d56    	umulh	x22, x10, x6
10007bffc: 9bc67d37    	umulh	x23, x9, x6
10007c000: 9b067d26    	mul	x6, x9, x6
10007c004: ab050265    	adds	x5, x19, x5
10007c008: 1a9f37f3    	cset	w19, hs
10007c00c: ab1402d4    	adds	x20, x22, x20
10007c010: 1a9f37f6    	cset	w22, hs
10007c014: ab060286    	adds	x6, x20, x6
10007c018: 9a9636d4    	cinc	x20, x22, hs
10007c01c: ab1500a5    	adds	x5, x5, x21
10007c020: ba1300c6    	adcs	x6, x6, x19
10007c024: 9b077db3    	mul	x19, x13, x7
10007c028: 9a1402f4    	adc	x20, x23, x20
10007c02c: 9b137d95    	mul	x21, x12, x19
10007c030: 9bd37d96    	umulh	x22, x12, x19
10007c034: 9bd37d77    	umulh	x23, x11, x19
10007c038: 9b137d73    	mul	x19, x11, x19
10007c03c: ab0502c5    	adds	x5, x22, x5
10007c040: 1a9f37f6    	cset	w22, hs
10007c044: ab1300a5    	adds	x5, x5, x19
10007c048: 9a9636d3    	cinc	x19, x22, hs
10007c04c: ab1700c6    	adds	x6, x6, x23
10007c050: 1a9f37f6    	cset	w22, hs
10007c054: ab0702bf    	cmn	x21, x7
10007c058: ba0800a5    	adcs	x5, x5, x8
10007c05c: ba1300c6    	adcs	x6, x6, x19
10007c060: ba160287    	adcs	x7, x20, x22
10007c064: 1a9f37f3    	cset	w19, hs
10007c068: 9b057db4    	mul	x20, x13, x5
10007c06c: 9bd47d95    	umulh	x21, x12, x20
10007c070: 9b147d96    	mul	x22, x12, x20
10007c074: 9bd47d77    	umulh	x23, x11, x20
10007c078: 9b147d74    	mul	x20, x11, x20
10007c07c: ab0602a6    	adds	x6, x21, x6
10007c080: 1a9f37f5    	cset	w21, hs
10007c084: ab1400c6    	adds	x6, x6, x20
10007c088: 9a9536b4    	cinc	x20, x21, hs
10007c08c: ab1700e7    	adds	x7, x7, x23
10007c090: 1a9f37f5    	cset	w21, hs
10007c094: ab0502df    	cmn	x22, x5
10007c098: ba0800c5    	adcs	x5, x6, x8
10007c09c: ba1400e6    	adcs	x6, x7, x20
10007c0a0: 9a9536a7    	cinc	x7, x21, hs
10007c0a4: eb0c00bf    	cmp	x5, x12
10007c0a8: fa0b00df    	sbcs	xzr, x6, x11
10007c0ac: aa1300e7    	orr	x7, x7, x19
10007c0b0: fa4038e0    	ccmp	x7, #0x0, #0x0, lo
10007c0b4: 9a881167    	csel	x7, x11, x8, ne
10007c0b8: 9a881193    	csel	x19, x12, x8, ne
10007c0bc: eb1300b3    	subs	x19, x5, x19
10007c0c0: da0700c6    	sbc	x6, x6, x7
10007c0c4: a97f1e05    	ldp	x5, x7, [x16, #-0x10]
10007c0c8: ab070267    	adds	x7, x19, x7
10007c0cc: ba0800c6    	adcs	x6, x6, x8
10007c0d0: 1a9f37f3    	cset	w19, hs
10007c0d4: eb0c00ff    	cmp	x7, x12
10007c0d8: fa0b00df    	sbcs	xzr, x6, x11
10007c0dc: 1a9f3673    	csinc	w19, w19, wzr, lo
10007c0e0: 7100027f    	cmp	w19, #0x0
10007c0e4: 9a881173    	csel	x19, x11, x8, ne
10007c0e8: 9a881194    	csel	x20, x12, x8, ne
10007c0ec: eb1400e7    	subs	x7, x7, x20
10007c0f0: da1300c6    	sbc	x6, x6, x19
10007c0f4: 9b077d53    	mul	x19, x10, x7
10007c0f8: 9bc77d54    	umulh	x20, x10, x7
10007c0fc: 9bc77d35    	umulh	x21, x9, x7
10007c100: 9b077d27    	mul	x7, x9, x7
10007c104: 9b067d56    	mul	x22, x10, x6
10007c108: 9bc67d57    	umulh	x23, x10, x6
10007c10c: 9bc67d38    	umulh	x24, x9, x6
10007c110: 9b067d26    	mul	x6, x9, x6
10007c114: ab070287    	adds	x7, x20, x7
10007c118: 1a9f37f4    	cset	w20, hs
10007c11c: ab1502f5    	adds	x21, x23, x21
10007c120: 1a9f37f7    	cset	w23, hs
10007c124: ab0602a6    	adds	x6, x21, x6
10007c128: 9a9736f5    	cinc	x21, x23, hs
10007c12c: ab1600e7    	adds	x7, x7, x22
10007c130: ba1400c6    	adcs	x6, x6, x20
10007c134: 9a150314    	adc	x20, x24, x21
10007c138: 9b137db5    	mul	x21, x13, x19
10007c13c: 9b157d96    	mul	x22, x12, x21
10007c140: 9bd57d97    	umulh	x23, x12, x21
10007c144: 9bd57d78    	umulh	x24, x11, x21
10007c148: 9b157d75    	mul	x21, x11, x21
10007c14c: ab0702e7    	adds	x7, x23, x7
10007c150: 1a9f37f7    	cset	w23, hs
10007c154: ab1500e7    	adds	x7, x7, x21
10007c158: 9a9736f5    	cinc	x21, x23, hs
10007c15c: ab1800c6    	adds	x6, x6, x24
10007c160: 1a9f37f7    	cset	w23, hs
10007c164: ab1302df    	cmn	x22, x19
10007c168: ba0800e7    	adcs	x7, x7, x8
10007c16c: ba1500c6    	adcs	x6, x6, x21
10007c170: ba170293    	adcs	x19, x20, x23
10007c174: 1a9f37f4    	cset	w20, hs
10007c178: 9b077db5    	mul	x21, x13, x7
10007c17c: 9b157d96    	mul	x22, x12, x21
10007c180: 9bd57d97    	umulh	x23, x12, x21
10007c184: 9bd57d78    	umulh	x24, x11, x21
10007c188: 9b157d75    	mul	x21, x11, x21
10007c18c: ab0602e6    	adds	x6, x23, x6
10007c190: 1a9f37f7    	cset	w23, hs
10007c194: ab1500c6    	adds	x6, x6, x21
10007c198: 9a9736f5    	cinc	x21, x23, hs
10007c19c: ab180273    	adds	x19, x19, x24
10007c1a0: 1a9f37f7    	cset	w23, hs
10007c1a4: ab0702df    	cmn	x22, x7
10007c1a8: ba0800c6    	adcs	x6, x6, x8
10007c1ac: ba150267    	adcs	x7, x19, x21
10007c1b0: 9a9736f3    	cinc	x19, x23, hs
10007c1b4: eb0c00df    	cmp	x6, x12
10007c1b8: fa0b00ff    	sbcs	xzr, x7, x11
10007c1bc: aa140273    	orr	x19, x19, x20
10007c1c0: fa403a60    	ccmp	x19, #0x0, #0x0, lo
10007c1c4: 9a881173    	csel	x19, x11, x8, ne
10007c1c8: 9a881194    	csel	x20, x12, x8, ne
10007c1cc: eb1400c6    	subs	x6, x6, x20
10007c1d0: da1300e7    	sbc	x7, x7, x19
10007c1d4: ab0500c5    	adds	x5, x6, x5
10007c1d8: ba0800e6    	adcs	x6, x7, x8
10007c1dc: 1a9f37e7    	cset	w7, hs
10007c1e0: eb0c00bf    	cmp	x5, x12
10007c1e4: fa0b00df    	sbcs	xzr, x6, x11
10007c1e8: 1a9f34e7    	csinc	w7, w7, wzr, lo
10007c1ec: 710000ff    	cmp	w7, #0x0
10007c1f0: 9a881167    	csel	x7, x11, x8, ne
10007c1f4: 9a881193    	csel	x19, x12, x8, ne
10007c1f8: eb1300a5    	subs	x5, x5, x19
10007c1fc: 937ffc84    	asr	x4, x4, #63
10007c200: 8a0e0093    	and	x19, x4, x14
10007c204: da0700c6    	sbc	x6, x6, x7
10007c208: 8a0f0084    	and	x4, x4, x15
10007c20c: eb0400a4    	subs	x4, x5, x4
10007c210: fa1300c5    	sbcs	x5, x6, x19
10007c214: 1a9f27e6    	cset	w6, lo
10007c218: 390023e6    	strb	w6, [sp, #0x8]
10007c21c: 394023e6    	ldrb	w6, [sp, #0x8]
10007c220: aa0803e7    	mov	x7, x8
10007c224: f2401cdf    	tst	x6, #0xff
10007c228: 9a881067    	csel	x7, x3, x8, ne
10007c22c: aa0803f3    	mov	x19, x8
10007c230: f2401cdf    	tst	x6, #0xff
10007c234: 9a881013    	csel	x19, x0, x8, ne
10007c238: ab070084    	adds	x4, x4, x7
10007c23c: 9a050265    	adc	x5, x19, x5
10007c240: a93f9624    	stp	x4, x5, [x17, #-0x8]
10007c244: 91008210    	add	x16, x16, #0x20
10007c248: 91004231    	add	x17, x17, #0x10
10007c24c: f1000442    	subs	x2, x2, #0x1
10007c250: 54ffe5c1    	b.ne	0x10007bf08 <__RNvMNtNtNtCshgIkoLp8A2V_17field_regressions8campaign10candidates7integerINtB2_10ProjectionKj4_E5batchB8_+0x4c>
10007c254: a9447bfd    	ldp	x29, x30, [sp, #0x40]
10007c258: a9434ff4    	ldp	x20, x19, [sp, #0x30]
10007c25c: a94257f6    	ldp	x22, x21, [sp, #0x20]
10007c260: a9415ff8    	ldp	x24, x23, [sp, #0x10]
10007c264: 910143ff    	add	sp, sp, #0x50
10007c268: d65f03c0    	ret
10007c26c: b00009a4    	adrp	x4, 0x1001b1000 <dyld_stub_binder+0x1001b1000>
10007c270: 912f2084    	add	x4, x4, #0xbc8
10007c274: 910003e0    	mov	x0, sp
10007c278: 910023e1    	add	x1, sp, #0x8
10007c27c: d2800002    	mov	x2, #0x0                ; =0
10007c280: 94037cfd    	bl	0x10015b674 <__RINvNtCs8Mbv00yxnRz_4core9panicking13assert_failedjjEB4_>


/private/var/folders/q0/v7l8dzd13k7f_q6j80sy03lm0000gn/T/f2z-field-gated-h_tggvge/field-regressions:	file format mach-o arm64

Disassembly of section __TEXT,__text:

000000010007cfa0 <__RNvMNtNtNtCsfDw8YqMpZPC_17field_regressions8campaign10candidates7integerINtB2_10ProjectionKj4_E5batchB8_>:
10007cfa0: d10143ff    	sub	sp, sp, #0x50
10007cfa4: a9015ff8    	stp	x24, x23, [sp, #0x10]
10007cfa8: a90257f6    	stp	x22, x21, [sp, #0x20]
10007cfac: a9034ff4    	stp	x20, x19, [sp, #0x30]
10007cfb0: a9047bfd    	stp	x29, x30, [sp, #0x40]
10007cfb4: 910103fd    	add	x29, sp, #0x40
10007cfb8: a90013e2    	stp	x2, x4, [sp]
10007cfbc: eb04005f    	cmp	x2, x4
10007cfc0: 54001c81    	b.ne	0x10007d350 <__RNvMNtNtNtCsfDw8YqMpZPC_17field_regressions8campaign10candidates7integerINtB2_10ProjectionKj4_E5batchB8_+0x3b0>
10007cfc4: b4001ba2    	cbz	x2, 0x10007d338 <__RNvMNtNtNtCsfDw8YqMpZPC_17field_regressions8campaign10candidates7integerINtB2_10ProjectionKj4_E5batchB8_+0x398>
10007cfc8: d2800008    	mov	x8, #0x0                ; =0
10007cfcc: a949240a    	ldp	x10, x9, [x0, #0x90]
10007cfd0: a9442c0c    	ldp	x12, x11, [x0, #0x40]
10007cfd4: f940380d    	ldr	x13, [x0, #0x70]
10007cfd8: a94a380f    	ldp	x15, x14, [x0, #0xa0]
10007cfdc: 91004030    	add	x16, x1, #0x10
10007cfe0: 91002071    	add	x17, x3, #0x8
10007cfe4: 910023e1    	add	x1, sp, #0x8
10007cfe8: a9480003    	ldp	x3, x0, [x0, #0x80]
10007cfec: a9401205    	ldp	x5, x4, [x16]
10007cff0: 9b047d46    	mul	x6, x10, x4
10007cff4: 9bc47d47    	umulh	x7, x10, x4
10007cff8: 9bc47d33    	umulh	x19, x9, x4
10007cffc: 9b047d34    	mul	x20, x9, x4
10007d000: ab1400e7    	adds	x7, x7, x20
10007d004: ba080273    	adcs	x19, x19, x8
10007d008: 9b067db4    	mul	x20, x13, x6
10007d00c: 9b147d95    	mul	x21, x12, x20
10007d010: 9bd47d96    	umulh	x22, x12, x20
10007d014: 9bd47d77    	umulh	x23, x11, x20
10007d018: 9b147d74    	mul	x20, x11, x20
10007d01c: 1a9f37f8    	cset	w24, hs
10007d020: ab0702c7    	adds	x7, x22, x7
10007d024: 1a9f37f6    	cset	w22, hs
10007d028: ab1400e7    	adds	x7, x7, x20
10007d02c: 9a9636d4    	cinc	x20, x22, hs
10007d030: ab170273    	adds	x19, x19, x23
10007d034: 9a983716    	cinc	x22, x24, hs
10007d038: ab0602bf    	cmn	x21, x6
10007d03c: ba0800e6    	adcs	x6, x7, x8
10007d040: ba140267    	adcs	x7, x19, x20
10007d044: 9b067db3    	mul	x19, x13, x6
10007d048: 9b137d94    	mul	x20, x12, x19
10007d04c: 9bd37d95    	umulh	x21, x12, x19
10007d050: 9b137d77    	mul	x23, x11, x19
10007d054: 9bd37d73    	umulh	x19, x11, x19
10007d058: ba160273    	adcs	x19, x19, x22
10007d05c: 1a9f37f6    	cset	w22, hs
10007d060: ab0702a7    	adds	x7, x21, x7
10007d064: 1a9f37f5    	cset	w21, hs
10007d068: ab1700e7    	adds	x7, x7, x23
10007d06c: 9a9536b5    	cinc	x21, x21, hs
10007d070: ab06029f    	cmn	x20, x6
10007d074: ba0800e6    	adcs	x6, x7, x8
10007d078: ba150267    	adcs	x7, x19, x21
10007d07c: 9a9636d3    	cinc	x19, x22, hs
10007d080: eb0c00df    	cmp	x6, x12
10007d084: fa0b00ff    	sbcs	xzr, x7, x11
10007d088: fa403a60    	ccmp	x19, #0x0, #0x0, lo
10007d08c: 9a881173    	csel	x19, x11, x8, ne
10007d090: 9a881194    	csel	x20, x12, x8, ne
10007d094: eb1400c6    	subs	x6, x6, x20
10007d098: da1300e7    	sbc	x7, x7, x19
10007d09c: ab0500c5    	adds	x5, x6, x5
10007d0a0: ba0800e6    	adcs	x6, x7, x8
10007d0a4: 1a9f37e7    	cset	w7, hs
10007d0a8: eb0c00bf    	cmp	x5, x12
10007d0ac: fa0b00df    	sbcs	xzr, x6, x11
10007d0b0: 1a9f34e7    	csinc	w7, w7, wzr, lo
10007d0b4: 710000ff    	cmp	w7, #0x0
10007d0b8: 9a881167    	csel	x7, x11, x8, ne
10007d0bc: 9a881193    	csel	x19, x12, x8, ne
10007d0c0: eb1300a5    	subs	x5, x5, x19
10007d0c4: da0700c6    	sbc	x6, x6, x7
10007d0c8: 9b057d47    	mul	x7, x10, x5
10007d0cc: 9bc57d53    	umulh	x19, x10, x5
10007d0d0: 9bc57d34    	umulh	x20, x9, x5
10007d0d4: 9b057d25    	mul	x5, x9, x5
10007d0d8: 9b067d55    	mul	x21, x10, x6
10007d0dc: 9bc67d56    	umulh	x22, x10, x6
10007d0e0: 9bc67d37    	umulh	x23, x9, x6
10007d0e4: 9b067d26    	mul	x6, x9, x6
10007d0e8: ab050265    	adds	x5, x19, x5
10007d0ec: 1a9f37f3    	cset	w19, hs
10007d0f0: ab1402d4    	adds	x20, x22, x20
10007d0f4: 1a9f37f6    	cset	w22, hs
10007d0f8: ab060286    	adds	x6, x20, x6
10007d0fc: 9a9636d4    	cinc	x20, x22, hs
10007d100: ab1500a5    	adds	x5, x5, x21
10007d104: ba1300c6    	adcs	x6, x6, x19
10007d108: 9b077db3    	mul	x19, x13, x7
10007d10c: 9a1402f4    	adc	x20, x23, x20
10007d110: 9b137d95    	mul	x21, x12, x19
10007d114: 9bd37d96    	umulh	x22, x12, x19
10007d118: 9bd37d77    	umulh	x23, x11, x19
10007d11c: 9b137d73    	mul	x19, x11, x19
10007d120: ab0502c5    	adds	x5, x22, x5
10007d124: 1a9f37f6    	cset	w22, hs
10007d128: ab1300a5    	adds	x5, x5, x19
10007d12c: 9a9636d3    	cinc	x19, x22, hs
10007d130: ab1700c6    	adds	x6, x6, x23
10007d134: 1a9f37f6    	cset	w22, hs
10007d138: ab0702bf    	cmn	x21, x7
10007d13c: ba0800a5    	adcs	x5, x5, x8
10007d140: ba1300c6    	adcs	x6, x6, x19
10007d144: ba160287    	adcs	x7, x20, x22
10007d148: 1a9f37f3    	cset	w19, hs
10007d14c: 9b057db4    	mul	x20, x13, x5
10007d150: 9bd47d95    	umulh	x21, x12, x20
10007d154: 9b147d96    	mul	x22, x12, x20
10007d158: 9bd47d77    	umulh	x23, x11, x20
10007d15c: 9b147d74    	mul	x20, x11, x20
10007d160: ab0602a6    	adds	x6, x21, x6
10007d164: 1a9f37f5    	cset	w21, hs
10007d168: ab1400c6    	adds	x6, x6, x20
10007d16c: 9a9536b4    	cinc	x20, x21, hs
10007d170: ab1700e7    	adds	x7, x7, x23
10007d174: 1a9f37f5    	cset	w21, hs
10007d178: ab0502df    	cmn	x22, x5
10007d17c: ba0800c5    	adcs	x5, x6, x8
10007d180: ba1400e6    	adcs	x6, x7, x20
10007d184: 9a9536a7    	cinc	x7, x21, hs
10007d188: eb0c00bf    	cmp	x5, x12
10007d18c: fa0b00df    	sbcs	xzr, x6, x11
10007d190: aa1300e7    	orr	x7, x7, x19
10007d194: fa4038e0    	ccmp	x7, #0x0, #0x0, lo
10007d198: 9a881167    	csel	x7, x11, x8, ne
10007d19c: 9a881193    	csel	x19, x12, x8, ne
10007d1a0: eb1300b3    	subs	x19, x5, x19
10007d1a4: da0700c6    	sbc	x6, x6, x7
10007d1a8: a97f1e05    	ldp	x5, x7, [x16, #-0x10]
10007d1ac: ab070267    	adds	x7, x19, x7
10007d1b0: ba0800c6    	adcs	x6, x6, x8
10007d1b4: 1a9f37f3    	cset	w19, hs
10007d1b8: eb0c00ff    	cmp	x7, x12
10007d1bc: fa0b00df    	sbcs	xzr, x6, x11
10007d1c0: 1a9f3673    	csinc	w19, w19, wzr, lo
10007d1c4: 7100027f    	cmp	w19, #0x0
10007d1c8: 9a881173    	csel	x19, x11, x8, ne
10007d1cc: 9a881194    	csel	x20, x12, x8, ne
10007d1d0: eb1400e7    	subs	x7, x7, x20
10007d1d4: da1300c6    	sbc	x6, x6, x19
10007d1d8: 9b077d53    	mul	x19, x10, x7
10007d1dc: 9bc77d54    	umulh	x20, x10, x7
10007d1e0: 9bc77d35    	umulh	x21, x9, x7
10007d1e4: 9b077d27    	mul	x7, x9, x7
10007d1e8: 9b067d56    	mul	x22, x10, x6
10007d1ec: 9bc67d57    	umulh	x23, x10, x6
10007d1f0: 9bc67d38    	umulh	x24, x9, x6
10007d1f4: 9b067d26    	mul	x6, x9, x6
10007d1f8: ab070287    	adds	x7, x20, x7
10007d1fc: 1a9f37f4    	cset	w20, hs
10007d200: ab1502f5    	adds	x21, x23, x21
10007d204: 1a9f37f7    	cset	w23, hs
10007d208: ab0602a6    	adds	x6, x21, x6
10007d20c: 9a9736f5    	cinc	x21, x23, hs
10007d210: ab1600e7    	adds	x7, x7, x22
10007d214: ba1400c6    	adcs	x6, x6, x20
10007d218: 9a150314    	adc	x20, x24, x21
10007d21c: 9b137db5    	mul	x21, x13, x19
10007d220: 9b157d96    	mul	x22, x12, x21
10007d224: 9bd57d97    	umulh	x23, x12, x21
10007d228: 9bd57d78    	umulh	x24, x11, x21
10007d22c: 9b157d75    	mul	x21, x11, x21
10007d230: ab0702e7    	adds	x7, x23, x7
10007d234: 1a9f37f7    	cset	w23, hs
10007d238: ab1500e7    	adds	x7, x7, x21
10007d23c: 9a9736f5    	cinc	x21, x23, hs
10007d240: ab1800c6    	adds	x6, x6, x24
10007d244: 1a9f37f7    	cset	w23, hs
10007d248: ab1302df    	cmn	x22, x19
10007d24c: ba0800e7    	adcs	x7, x7, x8
10007d250: ba1500c6    	adcs	x6, x6, x21
10007d254: ba170293    	adcs	x19, x20, x23
10007d258: 1a9f37f4    	cset	w20, hs
10007d25c: 9b077db5    	mul	x21, x13, x7
10007d260: 9b157d96    	mul	x22, x12, x21
10007d264: 9bd57d97    	umulh	x23, x12, x21
10007d268: 9bd57d78    	umulh	x24, x11, x21
10007d26c: 9b157d75    	mul	x21, x11, x21
10007d270: ab0602e6    	adds	x6, x23, x6
10007d274: 1a9f37f7    	cset	w23, hs
10007d278: ab1500c6    	adds	x6, x6, x21
10007d27c: 9a9736f5    	cinc	x21, x23, hs
10007d280: ab180273    	adds	x19, x19, x24
10007d284: 1a9f37f7    	cset	w23, hs
10007d288: ab0702df    	cmn	x22, x7
10007d28c: ba0800c6    	adcs	x6, x6, x8
10007d290: ba150267    	adcs	x7, x19, x21
10007d294: 9a9736f3    	cinc	x19, x23, hs
10007d298: eb0c00df    	cmp	x6, x12
10007d29c: fa0b00ff    	sbcs	xzr, x7, x11
10007d2a0: aa140273    	orr	x19, x19, x20
10007d2a4: fa403a60    	ccmp	x19, #0x0, #0x0, lo
10007d2a8: 9a881173    	csel	x19, x11, x8, ne
10007d2ac: 9a881194    	csel	x20, x12, x8, ne
10007d2b0: eb1400c6    	subs	x6, x6, x20
10007d2b4: da1300e7    	sbc	x7, x7, x19
10007d2b8: ab0500c5    	adds	x5, x6, x5
10007d2bc: ba0800e6    	adcs	x6, x7, x8
10007d2c0: 1a9f37e7    	cset	w7, hs
10007d2c4: eb0c00bf    	cmp	x5, x12
10007d2c8: fa0b00df    	sbcs	xzr, x6, x11
10007d2cc: 1a9f34e7    	csinc	w7, w7, wzr, lo
10007d2d0: 710000ff    	cmp	w7, #0x0
10007d2d4: 9a881167    	csel	x7, x11, x8, ne
10007d2d8: 9a881193    	csel	x19, x12, x8, ne
10007d2dc: eb1300a5    	subs	x5, x5, x19
10007d2e0: 937ffc84    	asr	x4, x4, #63
10007d2e4: 8a0e0093    	and	x19, x4, x14
10007d2e8: da0700c6    	sbc	x6, x6, x7
10007d2ec: 8a0f0084    	and	x4, x4, x15
10007d2f0: eb0400a4    	subs	x4, x5, x4
10007d2f4: fa1300c5    	sbcs	x5, x6, x19
10007d2f8: 1a9f27e6    	cset	w6, lo
10007d2fc: 390023e6    	strb	w6, [sp, #0x8]
10007d300: 394023e6    	ldrb	w6, [sp, #0x8]
10007d304: aa0803e7    	mov	x7, x8
10007d308: f2401cdf    	tst	x6, #0xff
10007d30c: 9a881067    	csel	x7, x3, x8, ne
10007d310: aa0803f3    	mov	x19, x8
10007d314: f2401cdf    	tst	x6, #0xff
10007d318: 9a881013    	csel	x19, x0, x8, ne
10007d31c: ab070084    	adds	x4, x4, x7
10007d320: 9a050265    	adc	x5, x19, x5
10007d324: a93f9624    	stp	x4, x5, [x17, #-0x8]
10007d328: 91008210    	add	x16, x16, #0x20
10007d32c: 91004231    	add	x17, x17, #0x10
10007d330: f1000442    	subs	x2, x2, #0x1
10007d334: 54ffe5c1    	b.ne	0x10007cfec <__RNvMNtNtNtCsfDw8YqMpZPC_17field_regressions8campaign10candidates7integerINtB2_10ProjectionKj4_E5batchB8_+0x4c>
10007d338: a9447bfd    	ldp	x29, x30, [sp, #0x40]
10007d33c: a9434ff4    	ldp	x20, x19, [sp, #0x30]
10007d340: a94257f6    	ldp	x22, x21, [sp, #0x20]
10007d344: a9415ff8    	ldp	x24, x23, [sp, #0x10]
10007d348: 910143ff    	add	sp, sp, #0x50
10007d34c: d65f03c0    	ret
10007d350: 900009a4    	adrp	x4, 0x1001b1000 <dyld_stub_binder+0x1001b1000>
10007d354: 91322084    	add	x4, x4, #0xc88
10007d358: 910003e0    	mov	x0, sp
10007d35c: 910023e1    	add	x1, sp, #0x8
10007d360: d2800002    	mov	x2, #0x0                ; =0
10007d364: 94037d58    	bl	0x10015c8c4 <__RINvNtCs8Mbv00yxnRz_4core9panicking13assert_failedjjEB4_>

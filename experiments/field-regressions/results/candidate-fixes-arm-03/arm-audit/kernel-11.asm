
/private/var/folders/q0/v7l8dzd13k7f_q6j80sy03lm0000gn/T/f2z-field-gated-h_tggvge/field-regressions:	file format mach-o arm64

Disassembly of section __TEXT,__text:

000000010007c93c <__RNvMNtNtNtCsfDw8YqMpZPC_17field_regressions8campaign10candidates7integerINtB2_10ProjectionKj2_E5batchB8_>:
10007c93c: d10143ff    	sub	sp, sp, #0x50
10007c940: a9015ff8    	stp	x24, x23, [sp, #0x10]
10007c944: a90257f6    	stp	x22, x21, [sp, #0x20]
10007c948: a9034ff4    	stp	x20, x19, [sp, #0x30]
10007c94c: a9047bfd    	stp	x29, x30, [sp, #0x40]
10007c950: 910103fd    	add	x29, sp, #0x40
10007c954: a90013e2    	stp	x2, x4, [sp]
10007c958: eb04005f    	cmp	x2, x4
10007c95c: 54000ba1    	b.ne	0x10007cad0 <__RNvMNtNtNtCsfDw8YqMpZPC_17field_regressions8campaign10candidates7integerINtB2_10ProjectionKj2_E5batchB8_+0x194>
10007c960: b4000ac2    	cbz	x2, 0x10007cab8 <__RNvMNtNtNtCsfDw8YqMpZPC_17field_regressions8campaign10candidates7integerINtB2_10ProjectionKj2_E5batchB8_+0x17c>
10007c964: d2800008    	mov	x8, #0x0                ; =0
10007c968: a949240a    	ldp	x10, x9, [x0, #0x90]
10007c96c: a9442c0c    	ldp	x12, x11, [x0, #0x40]
10007c970: f940380d    	ldr	x13, [x0, #0x70]
10007c974: a94a380f    	ldp	x15, x14, [x0, #0xa0]
10007c978: 91002030    	add	x16, x1, #0x8
10007c97c: 91002071    	add	x17, x3, #0x8
10007c980: 910023e1    	add	x1, sp, #0x8
10007c984: a9480003    	ldp	x3, x0, [x0, #0x80]
10007c988: a97f9205    	ldp	x5, x4, [x16, #-0x8]
10007c98c: 9bc47d26    	umulh	x6, x9, x4
10007c990: 9b047d27    	mul	x7, x9, x4
10007c994: 9b047d53    	mul	x19, x10, x4
10007c998: 9bc47d54    	umulh	x20, x10, x4
10007c99c: ab070287    	adds	x7, x20, x7
10007c9a0: ba0800c6    	adcs	x6, x6, x8
10007c9a4: 9b137db4    	mul	x20, x13, x19
10007c9a8: 1a9f37f5    	cset	w21, hs
10007c9ac: 9b147d76    	mul	x22, x11, x20
10007c9b0: 9bd47d77    	umulh	x23, x11, x20
10007c9b4: ab1700c6    	adds	x6, x6, x23
10007c9b8: 9b147d97    	mul	x23, x12, x20
10007c9bc: 9bd47d94    	umulh	x20, x12, x20
10007c9c0: 9a9536b5    	cinc	x21, x21, hs
10007c9c4: ab070287    	adds	x7, x20, x7
10007c9c8: 1a9f37f4    	cset	w20, hs
10007c9cc: ab1600e7    	adds	x7, x7, x22
10007c9d0: 9a943694    	cinc	x20, x20, hs
10007c9d4: ab1302ff    	cmn	x23, x19
10007c9d8: ba0800e7    	adcs	x7, x7, x8
10007c9dc: ba1400c6    	adcs	x6, x6, x20
10007c9e0: 9b077db3    	mul	x19, x13, x7
10007c9e4: 9b137d74    	mul	x20, x11, x19
10007c9e8: 9bd37d76    	umulh	x22, x11, x19
10007c9ec: ba1502d5    	adcs	x21, x22, x21
10007c9f0: 1a9f37f6    	cset	w22, hs
10007c9f4: 9bd37d97    	umulh	x23, x12, x19
10007c9f8: ab0602e6    	adds	x6, x23, x6
10007c9fc: 1a9f37f7    	cset	w23, hs
10007ca00: ab1400c6    	adds	x6, x6, x20
10007ca04: 9b137d93    	mul	x19, x12, x19
10007ca08: 9a9736f4    	cinc	x20, x23, hs
10007ca0c: ab07027f    	cmn	x19, x7
10007ca10: ba0800c6    	adcs	x6, x6, x8
10007ca14: ba1402a7    	adcs	x7, x21, x20
10007ca18: 9a9636d3    	cinc	x19, x22, hs
10007ca1c: eb0c00df    	cmp	x6, x12
10007ca20: fa0b00ff    	sbcs	xzr, x7, x11
10007ca24: fa403a60    	ccmp	x19, #0x0, #0x0, lo
10007ca28: 9a881173    	csel	x19, x11, x8, ne
10007ca2c: 9a881194    	csel	x20, x12, x8, ne
10007ca30: eb1400c6    	subs	x6, x6, x20
10007ca34: da1300e7    	sbc	x7, x7, x19
10007ca38: ab0500c5    	adds	x5, x6, x5
10007ca3c: ba0800e6    	adcs	x6, x7, x8
10007ca40: 1a9f37e7    	cset	w7, hs
10007ca44: eb0c00bf    	cmp	x5, x12
10007ca48: fa0b00df    	sbcs	xzr, x6, x11
10007ca4c: 1a9f34e7    	csinc	w7, w7, wzr, lo
10007ca50: 710000ff    	cmp	w7, #0x0
10007ca54: 9a881167    	csel	x7, x11, x8, ne
10007ca58: 9a881193    	csel	x19, x12, x8, ne
10007ca5c: eb1300a5    	subs	x5, x5, x19
10007ca60: 937ffc84    	asr	x4, x4, #63
10007ca64: 8a0e0093    	and	x19, x4, x14
10007ca68: da0700c6    	sbc	x6, x6, x7
10007ca6c: 8a0f0084    	and	x4, x4, x15
10007ca70: eb0400a4    	subs	x4, x5, x4
10007ca74: fa1300c5    	sbcs	x5, x6, x19
10007ca78: 1a9f27e6    	cset	w6, lo
10007ca7c: 390023e6    	strb	w6, [sp, #0x8]
10007ca80: 394023e6    	ldrb	w6, [sp, #0x8]
10007ca84: aa0803e7    	mov	x7, x8
10007ca88: f2401cdf    	tst	x6, #0xff
10007ca8c: 9a881067    	csel	x7, x3, x8, ne
10007ca90: aa0803f3    	mov	x19, x8
10007ca94: f2401cdf    	tst	x6, #0xff
10007ca98: 9a881013    	csel	x19, x0, x8, ne
10007ca9c: ab070084    	adds	x4, x4, x7
10007caa0: 9a050265    	adc	x5, x19, x5
10007caa4: a93f9624    	stp	x4, x5, [x17, #-0x8]
10007caa8: 91004210    	add	x16, x16, #0x10
10007caac: 91004231    	add	x17, x17, #0x10
10007cab0: f1000442    	subs	x2, x2, #0x1
10007cab4: 54fff6a1    	b.ne	0x10007c988 <__RNvMNtNtNtCsfDw8YqMpZPC_17field_regressions8campaign10candidates7integerINtB2_10ProjectionKj2_E5batchB8_+0x4c>
10007cab8: a9447bfd    	ldp	x29, x30, [sp, #0x40]
10007cabc: a9434ff4    	ldp	x20, x19, [sp, #0x30]
10007cac0: a94257f6    	ldp	x22, x21, [sp, #0x20]
10007cac4: a9415ff8    	ldp	x24, x23, [sp, #0x10]
10007cac8: 910143ff    	add	sp, sp, #0x50
10007cacc: d65f03c0    	ret
10007cad0: b00009a4    	adrp	x4, 0x1001b1000 <dyld_stub_binder+0x1001b1000>
10007cad4: 91322084    	add	x4, x4, #0xc88
10007cad8: 910003e0    	mov	x0, sp
10007cadc: 910023e1    	add	x1, sp, #0x8
10007cae0: d2800002    	mov	x2, #0x0                ; =0
10007cae4: 94037f78    	bl	0x10015c8c4 <__RINvNtCs8Mbv00yxnRz_4core9panicking13assert_failedjjEB4_>

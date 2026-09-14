
/private/tmp/f2z-arithmetic-target/release/field-regressions:	file format mach-o arm64

Disassembly of section __TEXT,__text:

000000010007b858 <__RNvMNtNtNtCshgIkoLp8A2V_17field_regressions8campaign10candidates7integerINtB2_10ProjectionKj2_E5batchB8_>:
10007b858: d10143ff    	sub	sp, sp, #0x50
10007b85c: a9015ff8    	stp	x24, x23, [sp, #0x10]
10007b860: a90257f6    	stp	x22, x21, [sp, #0x20]
10007b864: a9034ff4    	stp	x20, x19, [sp, #0x30]
10007b868: a9047bfd    	stp	x29, x30, [sp, #0x40]
10007b86c: 910103fd    	add	x29, sp, #0x40
10007b870: a90013e2    	stp	x2, x4, [sp]
10007b874: eb04005f    	cmp	x2, x4
10007b878: 54000ba1    	b.ne	0x10007b9ec <__RNvMNtNtNtCshgIkoLp8A2V_17field_regressions8campaign10candidates7integerINtB2_10ProjectionKj2_E5batchB8_+0x194>
10007b87c: b4000ac2    	cbz	x2, 0x10007b9d4 <__RNvMNtNtNtCshgIkoLp8A2V_17field_regressions8campaign10candidates7integerINtB2_10ProjectionKj2_E5batchB8_+0x17c>
10007b880: d2800008    	mov	x8, #0x0                ; =0
10007b884: a949240a    	ldp	x10, x9, [x0, #0x90]
10007b888: a9442c0c    	ldp	x12, x11, [x0, #0x40]
10007b88c: f940380d    	ldr	x13, [x0, #0x70]
10007b890: a94a380f    	ldp	x15, x14, [x0, #0xa0]
10007b894: 91002030    	add	x16, x1, #0x8
10007b898: 91002071    	add	x17, x3, #0x8
10007b89c: 910023e1    	add	x1, sp, #0x8
10007b8a0: a9480003    	ldp	x3, x0, [x0, #0x80]
10007b8a4: a97f9205    	ldp	x5, x4, [x16, #-0x8]
10007b8a8: 9bc47d26    	umulh	x6, x9, x4
10007b8ac: 9b047d27    	mul	x7, x9, x4
10007b8b0: 9b047d53    	mul	x19, x10, x4
10007b8b4: 9bc47d54    	umulh	x20, x10, x4
10007b8b8: ab070287    	adds	x7, x20, x7
10007b8bc: ba0800c6    	adcs	x6, x6, x8
10007b8c0: 9b137db4    	mul	x20, x13, x19
10007b8c4: 1a9f37f5    	cset	w21, hs
10007b8c8: 9b147d76    	mul	x22, x11, x20
10007b8cc: 9bd47d77    	umulh	x23, x11, x20
10007b8d0: ab1700c6    	adds	x6, x6, x23
10007b8d4: 9b147d97    	mul	x23, x12, x20
10007b8d8: 9bd47d94    	umulh	x20, x12, x20
10007b8dc: 9a9536b5    	cinc	x21, x21, hs
10007b8e0: ab070287    	adds	x7, x20, x7
10007b8e4: 1a9f37f4    	cset	w20, hs
10007b8e8: ab1600e7    	adds	x7, x7, x22
10007b8ec: 9a943694    	cinc	x20, x20, hs
10007b8f0: ab1302ff    	cmn	x23, x19
10007b8f4: ba0800e7    	adcs	x7, x7, x8
10007b8f8: ba1400c6    	adcs	x6, x6, x20
10007b8fc: 9b077db3    	mul	x19, x13, x7
10007b900: 9b137d74    	mul	x20, x11, x19
10007b904: 9bd37d76    	umulh	x22, x11, x19
10007b908: ba1502d5    	adcs	x21, x22, x21
10007b90c: 1a9f37f6    	cset	w22, hs
10007b910: 9bd37d97    	umulh	x23, x12, x19
10007b914: ab0602e6    	adds	x6, x23, x6
10007b918: 1a9f37f7    	cset	w23, hs
10007b91c: ab1400c6    	adds	x6, x6, x20
10007b920: 9b137d93    	mul	x19, x12, x19
10007b924: 9a9736f4    	cinc	x20, x23, hs
10007b928: ab07027f    	cmn	x19, x7
10007b92c: ba0800c6    	adcs	x6, x6, x8
10007b930: ba1402a7    	adcs	x7, x21, x20
10007b934: 9a9636d3    	cinc	x19, x22, hs
10007b938: eb0c00df    	cmp	x6, x12
10007b93c: fa0b00ff    	sbcs	xzr, x7, x11
10007b940: fa403a60    	ccmp	x19, #0x0, #0x0, lo
10007b944: 9a881173    	csel	x19, x11, x8, ne
10007b948: 9a881194    	csel	x20, x12, x8, ne
10007b94c: eb1400c6    	subs	x6, x6, x20
10007b950: da1300e7    	sbc	x7, x7, x19
10007b954: ab0500c5    	adds	x5, x6, x5
10007b958: ba0800e6    	adcs	x6, x7, x8
10007b95c: 1a9f37e7    	cset	w7, hs
10007b960: eb0c00bf    	cmp	x5, x12
10007b964: fa0b00df    	sbcs	xzr, x6, x11
10007b968: 1a9f34e7    	csinc	w7, w7, wzr, lo
10007b96c: 710000ff    	cmp	w7, #0x0
10007b970: 9a881167    	csel	x7, x11, x8, ne
10007b974: 9a881193    	csel	x19, x12, x8, ne
10007b978: eb1300a5    	subs	x5, x5, x19
10007b97c: 937ffc84    	asr	x4, x4, #63
10007b980: 8a0e0093    	and	x19, x4, x14
10007b984: da0700c6    	sbc	x6, x6, x7
10007b988: 8a0f0084    	and	x4, x4, x15
10007b98c: eb0400a4    	subs	x4, x5, x4
10007b990: fa1300c5    	sbcs	x5, x6, x19
10007b994: 1a9f27e6    	cset	w6, lo
10007b998: 390023e6    	strb	w6, [sp, #0x8]
10007b99c: 394023e6    	ldrb	w6, [sp, #0x8]
10007b9a0: aa0803e7    	mov	x7, x8
10007b9a4: f2401cdf    	tst	x6, #0xff
10007b9a8: 9a881067    	csel	x7, x3, x8, ne
10007b9ac: aa0803f3    	mov	x19, x8
10007b9b0: f2401cdf    	tst	x6, #0xff
10007b9b4: 9a881013    	csel	x19, x0, x8, ne
10007b9b8: ab070084    	adds	x4, x4, x7
10007b9bc: 9a050265    	adc	x5, x19, x5
10007b9c0: a93f9624    	stp	x4, x5, [x17, #-0x8]
10007b9c4: 91004210    	add	x16, x16, #0x10
10007b9c8: 91004231    	add	x17, x17, #0x10
10007b9cc: f1000442    	subs	x2, x2, #0x1
10007b9d0: 54fff6a1    	b.ne	0x10007b8a4 <__RNvMNtNtNtCshgIkoLp8A2V_17field_regressions8campaign10candidates7integerINtB2_10ProjectionKj2_E5batchB8_+0x4c>
10007b9d4: a9447bfd    	ldp	x29, x30, [sp, #0x40]
10007b9d8: a9434ff4    	ldp	x20, x19, [sp, #0x30]
10007b9dc: a94257f6    	ldp	x22, x21, [sp, #0x20]
10007b9e0: a9415ff8    	ldp	x24, x23, [sp, #0x10]
10007b9e4: 910143ff    	add	sp, sp, #0x50
10007b9e8: d65f03c0    	ret
10007b9ec: d00009a4    	adrp	x4, 0x1001b1000 <dyld_stub_binder+0x1001b1000>
10007b9f0: 912f2084    	add	x4, x4, #0xbc8
10007b9f4: 910003e0    	mov	x0, sp
10007b9f8: 910023e1    	add	x1, sp, #0x8
10007b9fc: d2800002    	mov	x2, #0x0                ; =0
10007ba00: 94037f1d    	bl	0x10015b674 <__RINvNtCs8Mbv00yxnRz_4core9panicking13assert_failedjjEB4_>

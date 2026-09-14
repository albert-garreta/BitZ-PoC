
/private/var/folders/q0/v7l8dzd13k7f_q6j80sy03lm0000gn/T/f2z-field-gated-pycl0nhx/field-regressions:	file format mach-o arm64

Disassembly of section __TEXT,__text:

000000010007b834 <__RNvMNtNtNtCsagpXUhzPPpl_17field_regressions8campaign10candidates7integerINtB2_10ProjectionKj2_E5batchB8_>:
10007b834: d10143ff    	sub	sp, sp, #0x50
10007b838: a9015ff8    	stp	x24, x23, [sp, #0x10]
10007b83c: a90257f6    	stp	x22, x21, [sp, #0x20]
10007b840: a9034ff4    	stp	x20, x19, [sp, #0x30]
10007b844: a9047bfd    	stp	x29, x30, [sp, #0x40]
10007b848: 910103fd    	add	x29, sp, #0x40
10007b84c: a90013e2    	stp	x2, x4, [sp]
10007b850: eb04005f    	cmp	x2, x4
10007b854: 54000ba1    	b.ne	0x10007b9c8 <__RNvMNtNtNtCsagpXUhzPPpl_17field_regressions8campaign10candidates7integerINtB2_10ProjectionKj2_E5batchB8_+0x194>
10007b858: b4000ac2    	cbz	x2, 0x10007b9b0 <__RNvMNtNtNtCsagpXUhzPPpl_17field_regressions8campaign10candidates7integerINtB2_10ProjectionKj2_E5batchB8_+0x17c>
10007b85c: d2800008    	mov	x8, #0x0                ; =0
10007b860: a949240a    	ldp	x10, x9, [x0, #0x90]
10007b864: a9442c0c    	ldp	x12, x11, [x0, #0x40]
10007b868: f940380d    	ldr	x13, [x0, #0x70]
10007b86c: a94a380f    	ldp	x15, x14, [x0, #0xa0]
10007b870: 91002030    	add	x16, x1, #0x8
10007b874: 91002071    	add	x17, x3, #0x8
10007b878: 910023e1    	add	x1, sp, #0x8
10007b87c: a9480003    	ldp	x3, x0, [x0, #0x80]
10007b880: a97f9205    	ldp	x5, x4, [x16, #-0x8]
10007b884: 9bc47d26    	umulh	x6, x9, x4
10007b888: 9b047d27    	mul	x7, x9, x4
10007b88c: 9b047d53    	mul	x19, x10, x4
10007b890: 9bc47d54    	umulh	x20, x10, x4
10007b894: ab070287    	adds	x7, x20, x7
10007b898: ba0800c6    	adcs	x6, x6, x8
10007b89c: 9b137db4    	mul	x20, x13, x19
10007b8a0: 1a9f37f5    	cset	w21, hs
10007b8a4: 9b147d76    	mul	x22, x11, x20
10007b8a8: 9bd47d77    	umulh	x23, x11, x20
10007b8ac: ab1700c6    	adds	x6, x6, x23
10007b8b0: 9b147d97    	mul	x23, x12, x20
10007b8b4: 9bd47d94    	umulh	x20, x12, x20
10007b8b8: 9a9536b5    	cinc	x21, x21, hs
10007b8bc: ab070287    	adds	x7, x20, x7
10007b8c0: 1a9f37f4    	cset	w20, hs
10007b8c4: ab1600e7    	adds	x7, x7, x22
10007b8c8: 9a943694    	cinc	x20, x20, hs
10007b8cc: ab1302ff    	cmn	x23, x19
10007b8d0: ba0800e7    	adcs	x7, x7, x8
10007b8d4: ba1400c6    	adcs	x6, x6, x20
10007b8d8: 9b077db3    	mul	x19, x13, x7
10007b8dc: 9b137d74    	mul	x20, x11, x19
10007b8e0: 9bd37d76    	umulh	x22, x11, x19
10007b8e4: ba1502d5    	adcs	x21, x22, x21
10007b8e8: 1a9f37f6    	cset	w22, hs
10007b8ec: 9bd37d97    	umulh	x23, x12, x19
10007b8f0: ab0602e6    	adds	x6, x23, x6
10007b8f4: 1a9f37f7    	cset	w23, hs
10007b8f8: ab1400c6    	adds	x6, x6, x20
10007b8fc: 9b137d93    	mul	x19, x12, x19
10007b900: 9a9736f4    	cinc	x20, x23, hs
10007b904: ab07027f    	cmn	x19, x7
10007b908: ba0800c6    	adcs	x6, x6, x8
10007b90c: ba1402a7    	adcs	x7, x21, x20
10007b910: 9a9636d3    	cinc	x19, x22, hs
10007b914: eb0c00df    	cmp	x6, x12
10007b918: fa0b00ff    	sbcs	xzr, x7, x11
10007b91c: fa403a60    	ccmp	x19, #0x0, #0x0, lo
10007b920: 9a881173    	csel	x19, x11, x8, ne
10007b924: 9a881194    	csel	x20, x12, x8, ne
10007b928: eb1400c6    	subs	x6, x6, x20
10007b92c: da1300e7    	sbc	x7, x7, x19
10007b930: ab0500c5    	adds	x5, x6, x5
10007b934: ba0800e6    	adcs	x6, x7, x8
10007b938: 1a9f37e7    	cset	w7, hs
10007b93c: eb0c00bf    	cmp	x5, x12
10007b940: fa0b00df    	sbcs	xzr, x6, x11
10007b944: 1a9f34e7    	csinc	w7, w7, wzr, lo
10007b948: 710000ff    	cmp	w7, #0x0
10007b94c: 9a881167    	csel	x7, x11, x8, ne
10007b950: 9a881193    	csel	x19, x12, x8, ne
10007b954: eb1300a5    	subs	x5, x5, x19
10007b958: 937ffc84    	asr	x4, x4, #63
10007b95c: 8a0e0093    	and	x19, x4, x14
10007b960: da0700c6    	sbc	x6, x6, x7
10007b964: 8a0f0084    	and	x4, x4, x15
10007b968: eb0400a4    	subs	x4, x5, x4
10007b96c: fa1300c5    	sbcs	x5, x6, x19
10007b970: 1a9f27e6    	cset	w6, lo
10007b974: 390023e6    	strb	w6, [sp, #0x8]
10007b978: 394023e6    	ldrb	w6, [sp, #0x8]
10007b97c: aa0803e7    	mov	x7, x8
10007b980: f2401cdf    	tst	x6, #0xff
10007b984: 9a881067    	csel	x7, x3, x8, ne
10007b988: aa0803f3    	mov	x19, x8
10007b98c: f2401cdf    	tst	x6, #0xff
10007b990: 9a881013    	csel	x19, x0, x8, ne
10007b994: ab070084    	adds	x4, x4, x7
10007b998: 9a050265    	adc	x5, x19, x5
10007b99c: a93f9624    	stp	x4, x5, [x17, #-0x8]
10007b9a0: 91004210    	add	x16, x16, #0x10
10007b9a4: 91004231    	add	x17, x17, #0x10
10007b9a8: f1000442    	subs	x2, x2, #0x1
10007b9ac: 54fff6a1    	b.ne	0x10007b880 <__RNvMNtNtNtCsagpXUhzPPpl_17field_regressions8campaign10candidates7integerINtB2_10ProjectionKj2_E5batchB8_+0x4c>
10007b9b0: a9447bfd    	ldp	x29, x30, [sp, #0x40]
10007b9b4: a9434ff4    	ldp	x20, x19, [sp, #0x30]
10007b9b8: a94257f6    	ldp	x22, x21, [sp, #0x20]
10007b9bc: a9415ff8    	ldp	x24, x23, [sp, #0x10]
10007b9c0: 910143ff    	add	sp, sp, #0x50
10007b9c4: d65f03c0    	ret
10007b9c8: d00009a4    	adrp	x4, 0x1001b1000 <dyld_stub_binder+0x1001b1000>
10007b9cc: 912ec084    	add	x4, x4, #0xbb0
10007b9d0: 910003e0    	mov	x0, sp
10007b9d4: 910023e1    	add	x1, sp, #0x8
10007b9d8: d2800002    	mov	x2, #0x0                ; =0
10007b9dc: 94037f1d    	bl	0x10015b650 <__RINvNtCs8Mbv00yxnRz_4core9panicking13assert_failedjjEB4_>

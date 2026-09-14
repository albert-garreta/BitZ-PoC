
/private/var/folders/q0/v7l8dzd13k7f_q6j80sy03lm0000gn/T/f2z-field-gated-aejimxw9/field-regressions:	file format mach-o arm64

Disassembly of section __TEXT,__text:

0000000100070500 <__RNvMNtNtNtCsaHyC9lX8wyC_17field_regressions8campaign10candidates7integerINtB2_10ProjectionKj2_E5batchB8_>:
100070500: d10143ff    	sub	sp, sp, #0x50
100070504: a9015ff8    	stp	x24, x23, [sp, #0x10]
100070508: a90257f6    	stp	x22, x21, [sp, #0x20]
10007050c: a9034ff4    	stp	x20, x19, [sp, #0x30]
100070510: a9047bfd    	stp	x29, x30, [sp, #0x40]
100070514: 910103fd    	add	x29, sp, #0x40
100070518: a90013e2    	stp	x2, x4, [sp]
10007051c: eb04005f    	cmp	x2, x4
100070520: 54000ba1    	b.ne	0x100070694 <__RNvMNtNtNtCsaHyC9lX8wyC_17field_regressions8campaign10candidates7integerINtB2_10ProjectionKj2_E5batchB8_+0x194>
100070524: b4000ac2    	cbz	x2, 0x10007067c <__RNvMNtNtNtCsaHyC9lX8wyC_17field_regressions8campaign10candidates7integerINtB2_10ProjectionKj2_E5batchB8_+0x17c>
100070528: d2800008    	mov	x8, #0x0                ; =0
10007052c: a949240a    	ldp	x10, x9, [x0, #0x90]
100070530: a9442c0c    	ldp	x12, x11, [x0, #0x40]
100070534: f940380d    	ldr	x13, [x0, #0x70]
100070538: a94a380f    	ldp	x15, x14, [x0, #0xa0]
10007053c: 91002030    	add	x16, x1, #0x8
100070540: 91002071    	add	x17, x3, #0x8
100070544: 910023e1    	add	x1, sp, #0x8
100070548: a9480003    	ldp	x3, x0, [x0, #0x80]
10007054c: a97f9205    	ldp	x5, x4, [x16, #-0x8]
100070550: 9bc47d26    	umulh	x6, x9, x4
100070554: 9b047d27    	mul	x7, x9, x4
100070558: 9b047d53    	mul	x19, x10, x4
10007055c: 9bc47d54    	umulh	x20, x10, x4
100070560: ab070287    	adds	x7, x20, x7
100070564: ba0800c6    	adcs	x6, x6, x8
100070568: 9b137db4    	mul	x20, x13, x19
10007056c: 1a9f37f5    	cset	w21, hs
100070570: 9b147d76    	mul	x22, x11, x20
100070574: 9bd47d77    	umulh	x23, x11, x20
100070578: ab1700c6    	adds	x6, x6, x23
10007057c: 9b147d97    	mul	x23, x12, x20
100070580: 9bd47d94    	umulh	x20, x12, x20
100070584: 9a9536b5    	cinc	x21, x21, hs
100070588: ab070287    	adds	x7, x20, x7
10007058c: 1a9f37f4    	cset	w20, hs
100070590: ab1600e7    	adds	x7, x7, x22
100070594: 9a943694    	cinc	x20, x20, hs
100070598: ab1302ff    	cmn	x23, x19
10007059c: ba0800e7    	adcs	x7, x7, x8
1000705a0: ba1400c6    	adcs	x6, x6, x20
1000705a4: 9b077db3    	mul	x19, x13, x7
1000705a8: 9b137d74    	mul	x20, x11, x19
1000705ac: 9bd37d76    	umulh	x22, x11, x19
1000705b0: ba1502d5    	adcs	x21, x22, x21
1000705b4: 1a9f37f6    	cset	w22, hs
1000705b8: 9bd37d97    	umulh	x23, x12, x19
1000705bc: ab0602e6    	adds	x6, x23, x6
1000705c0: 1a9f37f7    	cset	w23, hs
1000705c4: ab1400c6    	adds	x6, x6, x20
1000705c8: 9b137d93    	mul	x19, x12, x19
1000705cc: 9a9736f4    	cinc	x20, x23, hs
1000705d0: ab07027f    	cmn	x19, x7
1000705d4: ba0800c6    	adcs	x6, x6, x8
1000705d8: ba1402a7    	adcs	x7, x21, x20
1000705dc: 9a9636d3    	cinc	x19, x22, hs
1000705e0: eb0c00df    	cmp	x6, x12
1000705e4: fa0b00ff    	sbcs	xzr, x7, x11
1000705e8: fa403a60    	ccmp	x19, #0x0, #0x0, lo
1000705ec: 9a881173    	csel	x19, x11, x8, ne
1000705f0: 9a881194    	csel	x20, x12, x8, ne
1000705f4: eb1400c6    	subs	x6, x6, x20
1000705f8: da1300e7    	sbc	x7, x7, x19
1000705fc: ab0500c5    	adds	x5, x6, x5
100070600: ba0800e6    	adcs	x6, x7, x8
100070604: 1a9f37e7    	cset	w7, hs
100070608: eb0c00bf    	cmp	x5, x12
10007060c: fa0b00df    	sbcs	xzr, x6, x11
100070610: 1a9f34e7    	csinc	w7, w7, wzr, lo
100070614: 710000ff    	cmp	w7, #0x0
100070618: 9a881167    	csel	x7, x11, x8, ne
10007061c: 9a881193    	csel	x19, x12, x8, ne
100070620: eb1300a5    	subs	x5, x5, x19
100070624: 937ffc84    	asr	x4, x4, #63
100070628: 8a0e0093    	and	x19, x4, x14
10007062c: da0700c6    	sbc	x6, x6, x7
100070630: 8a0f0084    	and	x4, x4, x15
100070634: eb0400a4    	subs	x4, x5, x4
100070638: fa1300c5    	sbcs	x5, x6, x19
10007063c: 1a9f27e6    	cset	w6, lo
100070640: 390023e6    	strb	w6, [sp, #0x8]
100070644: 394023e6    	ldrb	w6, [sp, #0x8]
100070648: aa0803e7    	mov	x7, x8
10007064c: f2401cdf    	tst	x6, #0xff
100070650: 9a881067    	csel	x7, x3, x8, ne
100070654: aa0803f3    	mov	x19, x8
100070658: f2401cdf    	tst	x6, #0xff
10007065c: 9a881013    	csel	x19, x0, x8, ne
100070660: ab070084    	adds	x4, x4, x7
100070664: 9a050265    	adc	x5, x19, x5
100070668: a93f9624    	stp	x4, x5, [x17, #-0x8]
10007066c: 91004210    	add	x16, x16, #0x10
100070670: 91004231    	add	x17, x17, #0x10
100070674: f1000442    	subs	x2, x2, #0x1
100070678: 54fff6a1    	b.ne	0x10007054c <__RNvMNtNtNtCsaHyC9lX8wyC_17field_regressions8campaign10candidates7integerINtB2_10ProjectionKj2_E5batchB8_+0x4c>
10007067c: a9447bfd    	ldp	x29, x30, [sp, #0x40]
100070680: a9434ff4    	ldp	x20, x19, [sp, #0x30]
100070684: a94257f6    	ldp	x22, x21, [sp, #0x20]
100070688: a9415ff8    	ldp	x24, x23, [sp, #0x10]
10007068c: 910143ff    	add	sp, sp, #0x50
100070690: d65f03c0    	ret
100070694: b0000964    	adrp	x4, 0x10019d000 <dyld_stub_binder+0x10019d000>
100070698: 910fc084    	add	x4, x4, #0x3f0
10007069c: 910003e0    	mov	x0, sp
1000706a0: 910023e1    	add	x1, sp, #0x8
1000706a4: d2800002    	mov	x2, #0x0                ; =0
1000706a8: 94036cb0    	bl	0x10014b968 <__RINvNtCs8Mbv00yxnRz_4core9panicking13assert_failedjjEB4_>

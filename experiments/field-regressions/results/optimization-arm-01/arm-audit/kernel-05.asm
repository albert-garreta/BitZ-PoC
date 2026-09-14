
/private/var/folders/q0/v7l8dzd13k7f_q6j80sy03lm0000gn/T/f2z-field-gated-6lkppdze/field-regressions:	file format mach-o arm64

Disassembly of section __TEXT,__text:

0000000100058480 <__RNvMNtNtNtCs8IeiyMGxmWr_17field_regressions8campaign10candidates7integerINtB2_10ProjectionKj2_E5batchB8_>:
100058480: d10143ff    	sub	sp, sp, #0x50
100058484: a9015ff8    	stp	x24, x23, [sp, #0x10]
100058488: a90257f6    	stp	x22, x21, [sp, #0x20]
10005848c: a9034ff4    	stp	x20, x19, [sp, #0x30]
100058490: a9047bfd    	stp	x29, x30, [sp, #0x40]
100058494: 910103fd    	add	x29, sp, #0x40
100058498: a90013e2    	stp	x2, x4, [sp]
10005849c: eb04005f    	cmp	x2, x4
1000584a0: 54000ba1    	b.ne	0x100058614 <__RNvMNtNtNtCs8IeiyMGxmWr_17field_regressions8campaign10candidates7integerINtB2_10ProjectionKj2_E5batchB8_+0x194>
1000584a4: b4000ac2    	cbz	x2, 0x1000585fc <__RNvMNtNtNtCs8IeiyMGxmWr_17field_regressions8campaign10candidates7integerINtB2_10ProjectionKj2_E5batchB8_+0x17c>
1000584a8: d2800008    	mov	x8, #0x0                ; =0
1000584ac: a949240a    	ldp	x10, x9, [x0, #0x90]
1000584b0: a9442c0c    	ldp	x12, x11, [x0, #0x40]
1000584b4: f940380d    	ldr	x13, [x0, #0x70]
1000584b8: a94a380f    	ldp	x15, x14, [x0, #0xa0]
1000584bc: 91002030    	add	x16, x1, #0x8
1000584c0: 91002071    	add	x17, x3, #0x8
1000584c4: 910023e1    	add	x1, sp, #0x8
1000584c8: a9480003    	ldp	x3, x0, [x0, #0x80]
1000584cc: a97f9205    	ldp	x5, x4, [x16, #-0x8]
1000584d0: 9bc47d26    	umulh	x6, x9, x4
1000584d4: 9b047d27    	mul	x7, x9, x4
1000584d8: 9b047d53    	mul	x19, x10, x4
1000584dc: 9bc47d54    	umulh	x20, x10, x4
1000584e0: ab070287    	adds	x7, x20, x7
1000584e4: ba0800c6    	adcs	x6, x6, x8
1000584e8: 9b137db4    	mul	x20, x13, x19
1000584ec: 1a9f37f5    	cset	w21, hs
1000584f0: 9b147d76    	mul	x22, x11, x20
1000584f4: 9bd47d77    	umulh	x23, x11, x20
1000584f8: ab1700c6    	adds	x6, x6, x23
1000584fc: 9b147d97    	mul	x23, x12, x20
100058500: 9bd47d94    	umulh	x20, x12, x20
100058504: 9a9536b5    	cinc	x21, x21, hs
100058508: ab070287    	adds	x7, x20, x7
10005850c: 1a9f37f4    	cset	w20, hs
100058510: ab1600e7    	adds	x7, x7, x22
100058514: 9a943694    	cinc	x20, x20, hs
100058518: ab1302ff    	cmn	x23, x19
10005851c: ba0800e7    	adcs	x7, x7, x8
100058520: ba1400c6    	adcs	x6, x6, x20
100058524: 9b077db3    	mul	x19, x13, x7
100058528: 9b137d74    	mul	x20, x11, x19
10005852c: 9bd37d76    	umulh	x22, x11, x19
100058530: ba1502d5    	adcs	x21, x22, x21
100058534: 1a9f37f6    	cset	w22, hs
100058538: 9bd37d97    	umulh	x23, x12, x19
10005853c: ab0602e6    	adds	x6, x23, x6
100058540: 1a9f37f7    	cset	w23, hs
100058544: ab1400c6    	adds	x6, x6, x20
100058548: 9b137d93    	mul	x19, x12, x19
10005854c: 9a9736f4    	cinc	x20, x23, hs
100058550: ab07027f    	cmn	x19, x7
100058554: ba0800c6    	adcs	x6, x6, x8
100058558: ba1402a7    	adcs	x7, x21, x20
10005855c: 9a9636d3    	cinc	x19, x22, hs
100058560: eb0c00df    	cmp	x6, x12
100058564: fa0b00ff    	sbcs	xzr, x7, x11
100058568: fa403a60    	ccmp	x19, #0x0, #0x0, lo
10005856c: 9a881173    	csel	x19, x11, x8, ne
100058570: 9a881194    	csel	x20, x12, x8, ne
100058574: eb1400c6    	subs	x6, x6, x20
100058578: da1300e7    	sbc	x7, x7, x19
10005857c: ab0500c5    	adds	x5, x6, x5
100058580: ba0800e6    	adcs	x6, x7, x8
100058584: 1a9f37e7    	cset	w7, hs
100058588: eb0c00bf    	cmp	x5, x12
10005858c: fa0b00df    	sbcs	xzr, x6, x11
100058590: 1a9f34e7    	csinc	w7, w7, wzr, lo
100058594: 710000ff    	cmp	w7, #0x0
100058598: 9a881167    	csel	x7, x11, x8, ne
10005859c: 9a881193    	csel	x19, x12, x8, ne
1000585a0: eb1300a5    	subs	x5, x5, x19
1000585a4: 937ffc84    	asr	x4, x4, #63
1000585a8: 8a0e0093    	and	x19, x4, x14
1000585ac: da0700c6    	sbc	x6, x6, x7
1000585b0: 8a0f0084    	and	x4, x4, x15
1000585b4: eb0400a4    	subs	x4, x5, x4
1000585b8: fa1300c5    	sbcs	x5, x6, x19
1000585bc: 1a9f27e6    	cset	w6, lo
1000585c0: 390023e6    	strb	w6, [sp, #0x8]
1000585c4: 394023e6    	ldrb	w6, [sp, #0x8]
1000585c8: aa0803e7    	mov	x7, x8
1000585cc: f2401cdf    	tst	x6, #0xff
1000585d0: 9a881067    	csel	x7, x3, x8, ne
1000585d4: aa0803f3    	mov	x19, x8
1000585d8: f2401cdf    	tst	x6, #0xff
1000585dc: 9a881013    	csel	x19, x0, x8, ne
1000585e0: ab070084    	adds	x4, x4, x7
1000585e4: 9a050265    	adc	x5, x19, x5
1000585e8: a93f9624    	stp	x4, x5, [x17, #-0x8]
1000585ec: 91004210    	add	x16, x16, #0x10
1000585f0: 91004231    	add	x17, x17, #0x10
1000585f4: f1000442    	subs	x2, x2, #0x1
1000585f8: 54fff6a1    	b.ne	0x1000584cc <__RNvMNtNtNtCs8IeiyMGxmWr_17field_regressions8campaign10candidates7integerINtB2_10ProjectionKj2_E5batchB8_+0x4c>
1000585fc: a9447bfd    	ldp	x29, x30, [sp, #0x40]
100058600: a9434ff4    	ldp	x20, x19, [sp, #0x30]
100058604: a94257f6    	ldp	x22, x21, [sp, #0x20]
100058608: a9415ff8    	ldp	x24, x23, [sp, #0x10]
10005860c: 910143ff    	add	sp, sp, #0x50
100058610: d65f03c0    	ret
100058614: 90000904    	adrp	x4, 0x100178000 <dyld_stub_binder+0x100178000>
100058618: 9124a084    	add	x4, x4, #0x928
10005861c: 910003e0    	mov	x0, sp
100058620: 910023e1    	add	x1, sp, #0x8
100058624: d2800002    	mov	x2, #0x0                ; =0
100058628: 94035051    	bl	0x10012c76c <__RINvNtCs8Mbv00yxnRz_4core9panicking13assert_failedjjEB4_>

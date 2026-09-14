
/private/tmp/f2z-arithmetic-target/release/field-regressions:	file format mach-o arm64

Disassembly of section __TEXT,__text:

000000010007c508 <__RNvMNtNtNtCshgIkoLp8A2V_17field_regressions8campaign10candidates7integerINtB2_10ProjectionKj9_E5batchB8_>:
10007c508: d101c3ff    	sub	sp, sp, #0x70
10007c50c: a9016ffc    	stp	x28, x27, [sp, #0x10]
10007c510: a90267fa    	stp	x26, x25, [sp, #0x20]
10007c514: a9035ff8    	stp	x24, x23, [sp, #0x30]
10007c518: a90457f6    	stp	x22, x21, [sp, #0x40]
10007c51c: a9054ff4    	stp	x20, x19, [sp, #0x50]
10007c520: a9067bfd    	stp	x29, x30, [sp, #0x60]
10007c524: 910183fd    	add	x29, sp, #0x60
10007c528: a90013e2    	stp	x2, x4, [sp]
10007c52c: eb04005f    	cmp	x2, x4
10007c530: 54000ec1    	b.ne	0x10007c708 <__RNvMNtNtNtCshgIkoLp8A2V_17field_regressions8campaign10candidates7integerINtB2_10ProjectionKj9_E5batchB8_+0x200>
10007c534: b4000da2    	cbz	x2, 0x10007c6e8 <__RNvMNtNtNtCshgIkoLp8A2V_17field_regressions8campaign10candidates7integerINtB2_10ProjectionKj9_E5batchB8_+0x1e0>
10007c538: d2800008    	mov	x8, #0x0                ; =0
10007c53c: a949240a    	ldp	x10, x9, [x0, #0x90]
10007c540: a9442c0c    	ldp	x12, x11, [x0, #0x40]
10007c544: f940380d    	ldr	x13, [x0, #0x70]
10007c548: a94a380f    	ldp	x15, x14, [x0, #0xa0]
10007c54c: 52800910    	mov	w16, #0x48              ; =72
10007c550: 910023f1    	add	x17, sp, #0x8
10007c554: aa0103e4    	mov	x4, x1
10007c558: d2800006    	mov	x6, #0x0                ; =0
10007c55c: a9480005    	ldp	x5, x0, [x0, #0x80]
10007c560: d2800015    	mov	x21, #0x0               ; =0
10007c564: 9b1004c7    	madd	x7, x6, x16, x1
10007c568: f94020e7    	ldr	x7, [x7, #0x40]
10007c56c: 52800713    	mov	w19, #0x38              ; =56
10007c570: aa0703f6    	mov	x22, x7
10007c574: f8736894    	ldr	x20, [x4, x19]
10007c578: 9b167d57    	mul	x23, x10, x22
10007c57c: 9bd67d58    	umulh	x24, x10, x22
10007c580: 9bd67d39    	umulh	x25, x9, x22
10007c584: 9b167d36    	mul	x22, x9, x22
10007c588: 9b157d5a    	mul	x26, x10, x21
10007c58c: 9bd57d5b    	umulh	x27, x10, x21
10007c590: 9bd57d3c    	umulh	x28, x9, x21
10007c594: 9b157d35    	mul	x21, x9, x21
10007c598: ab160316    	adds	x22, x24, x22
10007c59c: 1a9f37f8    	cset	w24, hs
10007c5a0: ab190379    	adds	x25, x27, x25
10007c5a4: 1a9f37fb    	cset	w27, hs
10007c5a8: ab150335    	adds	x21, x25, x21
10007c5ac: 9a9b3779    	cinc	x25, x27, hs
10007c5b0: ab1a02d6    	adds	x22, x22, x26
10007c5b4: ba1802b5    	adcs	x21, x21, x24
10007c5b8: 9a190398    	adc	x24, x28, x25
10007c5bc: 9b177db9    	mul	x25, x13, x23
10007c5c0: 9b197d9a    	mul	x26, x12, x25
10007c5c4: 9bd97d9b    	umulh	x27, x12, x25
10007c5c8: 9bd97d7c    	umulh	x28, x11, x25
10007c5cc: 9b197d79    	mul	x25, x11, x25
10007c5d0: ab160376    	adds	x22, x27, x22
10007c5d4: 1a9f37fb    	cset	w27, hs
10007c5d8: ab1902d6    	adds	x22, x22, x25
10007c5dc: 9a9b3779    	cinc	x25, x27, hs
10007c5e0: ab1c02b5    	adds	x21, x21, x28
10007c5e4: 1a9f37fb    	cset	w27, hs
10007c5e8: ab17035f    	cmn	x26, x23
10007c5ec: ba0802d6    	adcs	x22, x22, x8
10007c5f0: ba1902b5    	adcs	x21, x21, x25
10007c5f4: ba1b0317    	adcs	x23, x24, x27
10007c5f8: 1a9f37f8    	cset	w24, hs
10007c5fc: 9b167db9    	mul	x25, x13, x22
10007c600: 9b197d9a    	mul	x26, x12, x25
10007c604: 9bd97d9b    	umulh	x27, x12, x25
10007c608: 9bd97d7c    	umulh	x28, x11, x25
10007c60c: 9b197d79    	mul	x25, x11, x25
10007c610: ab150375    	adds	x21, x27, x21
10007c614: 1a9f37fb    	cset	w27, hs
10007c618: ab1902b5    	adds	x21, x21, x25
10007c61c: 9a9b3779    	cinc	x25, x27, hs
10007c620: ab1c02f7    	adds	x23, x23, x28
10007c624: 1a9f37fb    	cset	w27, hs
10007c628: ab16035f    	cmn	x26, x22
10007c62c: ba0802b5    	adcs	x21, x21, x8
10007c630: ba1902f6    	adcs	x22, x23, x25
10007c634: 9a9b3777    	cinc	x23, x27, hs
10007c638: eb0c02bf    	cmp	x21, x12
10007c63c: fa0b02df    	sbcs	xzr, x22, x11
10007c640: aa1802f7    	orr	x23, x23, x24
10007c644: fa403ae0    	ccmp	x23, #0x0, #0x0, lo
10007c648: 9a881177    	csel	x23, x11, x8, ne
10007c64c: 9a881198    	csel	x24, x12, x8, ne
10007c650: eb1802b5    	subs	x21, x21, x24
10007c654: da1702d6    	sbc	x22, x22, x23
10007c658: ab1402b4    	adds	x20, x21, x20
10007c65c: ba0802d5    	adcs	x21, x22, x8
10007c660: 1a9f37f6    	cset	w22, hs
10007c664: eb0c029f    	cmp	x20, x12
10007c668: fa0b02bf    	sbcs	xzr, x21, x11
10007c66c: 1a9f36d6    	csinc	w22, w22, wzr, lo
10007c670: 710002df    	cmp	w22, #0x0
10007c674: 9a881177    	csel	x23, x11, x8, ne
10007c678: 9a881196    	csel	x22, x12, x8, ne
10007c67c: eb160296    	subs	x22, x20, x22
10007c680: da1702b5    	sbc	x21, x21, x23
10007c684: d1002273    	sub	x19, x19, #0x8
10007c688: b100227f    	cmn	x19, #0x8
10007c68c: 54fff741    	b.ne	0x10007c574 <__RNvMNtNtNtCshgIkoLp8A2V_17field_regressions8campaign10candidates7integerINtB2_10ProjectionKj9_E5batchB8_+0x6c>
10007c690: 8b061073    	add	x19, x3, x6, lsl #4
10007c694: 910004c6    	add	x6, x6, #0x1
10007c698: 937ffce7    	asr	x7, x7, #63
10007c69c: 8a0e00f4    	and	x20, x7, x14
10007c6a0: 8a0f00e7    	and	x7, x7, x15
10007c6a4: eb0702c7    	subs	x7, x22, x7
10007c6a8: fa1402b4    	sbcs	x20, x21, x20
10007c6ac: 1a9f27f5    	cset	w21, lo
10007c6b0: 390023f5    	strb	w21, [sp, #0x8]
10007c6b4: 394023f5    	ldrb	w21, [sp, #0x8]
10007c6b8: aa0803f6    	mov	x22, x8
10007c6bc: f2401ebf    	tst	x21, #0xff
10007c6c0: 9a8810b6    	csel	x22, x5, x8, ne
10007c6c4: aa0803f7    	mov	x23, x8
10007c6c8: f2401ebf    	tst	x21, #0xff
10007c6cc: 9a881017    	csel	x23, x0, x8, ne
10007c6d0: ab1600e7    	adds	x7, x7, x22
10007c6d4: 9a1402f4    	adc	x20, x23, x20
10007c6d8: a9005267    	stp	x7, x20, [x19]
10007c6dc: 91012084    	add	x4, x4, #0x48
10007c6e0: eb0200df    	cmp	x6, x2
10007c6e4: 54fff3e1    	b.ne	0x10007c560 <__RNvMNtNtNtCshgIkoLp8A2V_17field_regressions8campaign10candidates7integerINtB2_10ProjectionKj9_E5batchB8_+0x58>
10007c6e8: a9467bfd    	ldp	x29, x30, [sp, #0x60]
10007c6ec: a9454ff4    	ldp	x20, x19, [sp, #0x50]
10007c6f0: a94457f6    	ldp	x22, x21, [sp, #0x40]
10007c6f4: a9435ff8    	ldp	x24, x23, [sp, #0x30]
10007c6f8: a94267fa    	ldp	x26, x25, [sp, #0x20]
10007c6fc: a9416ffc    	ldp	x28, x27, [sp, #0x10]
10007c700: 9101c3ff    	add	sp, sp, #0x70
10007c704: d65f03c0    	ret
10007c708: b00009a4    	adrp	x4, 0x1001b1000 <dyld_stub_binder+0x1001b1000>
10007c70c: 912f2084    	add	x4, x4, #0xbc8
10007c710: 910003e0    	mov	x0, sp
10007c714: 910023e1    	add	x1, sp, #0x8
10007c718: d2800002    	mov	x2, #0x0                ; =0
10007c71c: 94037bd6    	bl	0x10015b674 <__RINvNtCs8Mbv00yxnRz_4core9panicking13assert_failedjjEB4_>

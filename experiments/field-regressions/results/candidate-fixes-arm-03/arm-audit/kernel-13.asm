
/private/var/folders/q0/v7l8dzd13k7f_q6j80sy03lm0000gn/T/f2z-field-gated-h_tggvge/field-regressions:	file format mach-o arm64

Disassembly of section __TEXT,__text:

000000010007d5ec <__RNvMNtNtNtCsfDw8YqMpZPC_17field_regressions8campaign10candidates7integerINtB2_10ProjectionKj9_E5batchB8_>:
10007d5ec: d101c3ff    	sub	sp, sp, #0x70
10007d5f0: a9016ffc    	stp	x28, x27, [sp, #0x10]
10007d5f4: a90267fa    	stp	x26, x25, [sp, #0x20]
10007d5f8: a9035ff8    	stp	x24, x23, [sp, #0x30]
10007d5fc: a90457f6    	stp	x22, x21, [sp, #0x40]
10007d600: a9054ff4    	stp	x20, x19, [sp, #0x50]
10007d604: a9067bfd    	stp	x29, x30, [sp, #0x60]
10007d608: 910183fd    	add	x29, sp, #0x60
10007d60c: a90013e2    	stp	x2, x4, [sp]
10007d610: eb04005f    	cmp	x2, x4
10007d614: 54000ec1    	b.ne	0x10007d7ec <__RNvMNtNtNtCsfDw8YqMpZPC_17field_regressions8campaign10candidates7integerINtB2_10ProjectionKj9_E5batchB8_+0x200>
10007d618: b4000da2    	cbz	x2, 0x10007d7cc <__RNvMNtNtNtCsfDw8YqMpZPC_17field_regressions8campaign10candidates7integerINtB2_10ProjectionKj9_E5batchB8_+0x1e0>
10007d61c: d2800008    	mov	x8, #0x0                ; =0
10007d620: a949240a    	ldp	x10, x9, [x0, #0x90]
10007d624: a9442c0c    	ldp	x12, x11, [x0, #0x40]
10007d628: f940380d    	ldr	x13, [x0, #0x70]
10007d62c: a94a380f    	ldp	x15, x14, [x0, #0xa0]
10007d630: 52800910    	mov	w16, #0x48              ; =72
10007d634: 910023f1    	add	x17, sp, #0x8
10007d638: aa0103e4    	mov	x4, x1
10007d63c: d2800006    	mov	x6, #0x0                ; =0
10007d640: a9480005    	ldp	x5, x0, [x0, #0x80]
10007d644: d2800015    	mov	x21, #0x0               ; =0
10007d648: 9b1004c7    	madd	x7, x6, x16, x1
10007d64c: f94020e7    	ldr	x7, [x7, #0x40]
10007d650: 52800713    	mov	w19, #0x38              ; =56
10007d654: aa0703f6    	mov	x22, x7
10007d658: f8736894    	ldr	x20, [x4, x19]
10007d65c: 9b167d57    	mul	x23, x10, x22
10007d660: 9bd67d58    	umulh	x24, x10, x22
10007d664: 9bd67d39    	umulh	x25, x9, x22
10007d668: 9b167d36    	mul	x22, x9, x22
10007d66c: 9b157d5a    	mul	x26, x10, x21
10007d670: 9bd57d5b    	umulh	x27, x10, x21
10007d674: 9bd57d3c    	umulh	x28, x9, x21
10007d678: 9b157d35    	mul	x21, x9, x21
10007d67c: ab160316    	adds	x22, x24, x22
10007d680: 1a9f37f8    	cset	w24, hs
10007d684: ab190379    	adds	x25, x27, x25
10007d688: 1a9f37fb    	cset	w27, hs
10007d68c: ab150335    	adds	x21, x25, x21
10007d690: 9a9b3779    	cinc	x25, x27, hs
10007d694: ab1a02d6    	adds	x22, x22, x26
10007d698: ba1802b5    	adcs	x21, x21, x24
10007d69c: 9a190398    	adc	x24, x28, x25
10007d6a0: 9b177db9    	mul	x25, x13, x23
10007d6a4: 9b197d9a    	mul	x26, x12, x25
10007d6a8: 9bd97d9b    	umulh	x27, x12, x25
10007d6ac: 9bd97d7c    	umulh	x28, x11, x25
10007d6b0: 9b197d79    	mul	x25, x11, x25
10007d6b4: ab160376    	adds	x22, x27, x22
10007d6b8: 1a9f37fb    	cset	w27, hs
10007d6bc: ab1902d6    	adds	x22, x22, x25
10007d6c0: 9a9b3779    	cinc	x25, x27, hs
10007d6c4: ab1c02b5    	adds	x21, x21, x28
10007d6c8: 1a9f37fb    	cset	w27, hs
10007d6cc: ab17035f    	cmn	x26, x23
10007d6d0: ba0802d6    	adcs	x22, x22, x8
10007d6d4: ba1902b5    	adcs	x21, x21, x25
10007d6d8: ba1b0317    	adcs	x23, x24, x27
10007d6dc: 1a9f37f8    	cset	w24, hs
10007d6e0: 9b167db9    	mul	x25, x13, x22
10007d6e4: 9b197d9a    	mul	x26, x12, x25
10007d6e8: 9bd97d9b    	umulh	x27, x12, x25
10007d6ec: 9bd97d7c    	umulh	x28, x11, x25
10007d6f0: 9b197d79    	mul	x25, x11, x25
10007d6f4: ab150375    	adds	x21, x27, x21
10007d6f8: 1a9f37fb    	cset	w27, hs
10007d6fc: ab1902b5    	adds	x21, x21, x25
10007d700: 9a9b3779    	cinc	x25, x27, hs
10007d704: ab1c02f7    	adds	x23, x23, x28
10007d708: 1a9f37fb    	cset	w27, hs
10007d70c: ab16035f    	cmn	x26, x22
10007d710: ba0802b5    	adcs	x21, x21, x8
10007d714: ba1902f6    	adcs	x22, x23, x25
10007d718: 9a9b3777    	cinc	x23, x27, hs
10007d71c: eb0c02bf    	cmp	x21, x12
10007d720: fa0b02df    	sbcs	xzr, x22, x11
10007d724: aa1802f7    	orr	x23, x23, x24
10007d728: fa403ae0    	ccmp	x23, #0x0, #0x0, lo
10007d72c: 9a881177    	csel	x23, x11, x8, ne
10007d730: 9a881198    	csel	x24, x12, x8, ne
10007d734: eb1802b5    	subs	x21, x21, x24
10007d738: da1702d6    	sbc	x22, x22, x23
10007d73c: ab1402b4    	adds	x20, x21, x20
10007d740: ba0802d5    	adcs	x21, x22, x8
10007d744: 1a9f37f6    	cset	w22, hs
10007d748: eb0c029f    	cmp	x20, x12
10007d74c: fa0b02bf    	sbcs	xzr, x21, x11
10007d750: 1a9f36d6    	csinc	w22, w22, wzr, lo
10007d754: 710002df    	cmp	w22, #0x0
10007d758: 9a881177    	csel	x23, x11, x8, ne
10007d75c: 9a881196    	csel	x22, x12, x8, ne
10007d760: eb160296    	subs	x22, x20, x22
10007d764: da1702b5    	sbc	x21, x21, x23
10007d768: d1002273    	sub	x19, x19, #0x8
10007d76c: b100227f    	cmn	x19, #0x8
10007d770: 54fff741    	b.ne	0x10007d658 <__RNvMNtNtNtCsfDw8YqMpZPC_17field_regressions8campaign10candidates7integerINtB2_10ProjectionKj9_E5batchB8_+0x6c>
10007d774: 8b061073    	add	x19, x3, x6, lsl #4
10007d778: 910004c6    	add	x6, x6, #0x1
10007d77c: 937ffce7    	asr	x7, x7, #63
10007d780: 8a0e00f4    	and	x20, x7, x14
10007d784: 8a0f00e7    	and	x7, x7, x15
10007d788: eb0702c7    	subs	x7, x22, x7
10007d78c: fa1402b4    	sbcs	x20, x21, x20
10007d790: 1a9f27f5    	cset	w21, lo
10007d794: 390023f5    	strb	w21, [sp, #0x8]
10007d798: 394023f5    	ldrb	w21, [sp, #0x8]
10007d79c: aa0803f6    	mov	x22, x8
10007d7a0: f2401ebf    	tst	x21, #0xff
10007d7a4: 9a8810b6    	csel	x22, x5, x8, ne
10007d7a8: aa0803f7    	mov	x23, x8
10007d7ac: f2401ebf    	tst	x21, #0xff
10007d7b0: 9a881017    	csel	x23, x0, x8, ne
10007d7b4: ab1600e7    	adds	x7, x7, x22
10007d7b8: 9a1402f4    	adc	x20, x23, x20
10007d7bc: a9005267    	stp	x7, x20, [x19]
10007d7c0: 91012084    	add	x4, x4, #0x48
10007d7c4: eb0200df    	cmp	x6, x2
10007d7c8: 54fff3e1    	b.ne	0x10007d644 <__RNvMNtNtNtCsfDw8YqMpZPC_17field_regressions8campaign10candidates7integerINtB2_10ProjectionKj9_E5batchB8_+0x58>
10007d7cc: a9467bfd    	ldp	x29, x30, [sp, #0x60]
10007d7d0: a9454ff4    	ldp	x20, x19, [sp, #0x50]
10007d7d4: a94457f6    	ldp	x22, x21, [sp, #0x40]
10007d7d8: a9435ff8    	ldp	x24, x23, [sp, #0x30]
10007d7dc: a94267fa    	ldp	x26, x25, [sp, #0x20]
10007d7e0: a9416ffc    	ldp	x28, x27, [sp, #0x10]
10007d7e4: 9101c3ff    	add	sp, sp, #0x70
10007d7e8: d65f03c0    	ret
10007d7ec: 900009a4    	adrp	x4, 0x1001b1000 <dyld_stub_binder+0x1001b1000>
10007d7f0: 91322084    	add	x4, x4, #0xc88
10007d7f4: 910003e0    	mov	x0, sp
10007d7f8: 910023e1    	add	x1, sp, #0x8
10007d7fc: d2800002    	mov	x2, #0x0                ; =0
10007d800: 94037c31    	bl	0x10015c8c4 <__RINvNtCs8Mbv00yxnRz_4core9panicking13assert_failedjjEB4_>


/private/var/folders/q0/v7l8dzd13k7f_q6j80sy03lm0000gn/T/f2z-field-gated-6lkppdze/field-regressions:	file format mach-o arm64

Disassembly of section __TEXT,__text:

0000000100059130 <__RNvMNtNtNtCs8IeiyMGxmWr_17field_regressions8campaign10candidates7integerINtB2_10ProjectionKj9_E5batchB8_>:
100059130: d101c3ff    	sub	sp, sp, #0x70
100059134: a9016ffc    	stp	x28, x27, [sp, #0x10]
100059138: a90267fa    	stp	x26, x25, [sp, #0x20]
10005913c: a9035ff8    	stp	x24, x23, [sp, #0x30]
100059140: a90457f6    	stp	x22, x21, [sp, #0x40]
100059144: a9054ff4    	stp	x20, x19, [sp, #0x50]
100059148: a9067bfd    	stp	x29, x30, [sp, #0x60]
10005914c: 910183fd    	add	x29, sp, #0x60
100059150: a90013e2    	stp	x2, x4, [sp]
100059154: eb04005f    	cmp	x2, x4
100059158: 54000ec1    	b.ne	0x100059330 <__RNvMNtNtNtCs8IeiyMGxmWr_17field_regressions8campaign10candidates7integerINtB2_10ProjectionKj9_E5batchB8_+0x200>
10005915c: b4000da2    	cbz	x2, 0x100059310 <__RNvMNtNtNtCs8IeiyMGxmWr_17field_regressions8campaign10candidates7integerINtB2_10ProjectionKj9_E5batchB8_+0x1e0>
100059160: d2800008    	mov	x8, #0x0                ; =0
100059164: a949240a    	ldp	x10, x9, [x0, #0x90]
100059168: a9442c0c    	ldp	x12, x11, [x0, #0x40]
10005916c: f940380d    	ldr	x13, [x0, #0x70]
100059170: a94a380f    	ldp	x15, x14, [x0, #0xa0]
100059174: 52800910    	mov	w16, #0x48              ; =72
100059178: 910023f1    	add	x17, sp, #0x8
10005917c: aa0103e4    	mov	x4, x1
100059180: d2800006    	mov	x6, #0x0                ; =0
100059184: a9480005    	ldp	x5, x0, [x0, #0x80]
100059188: d2800015    	mov	x21, #0x0               ; =0
10005918c: 9b1004c7    	madd	x7, x6, x16, x1
100059190: f94020e7    	ldr	x7, [x7, #0x40]
100059194: 52800713    	mov	w19, #0x38              ; =56
100059198: aa0703f6    	mov	x22, x7
10005919c: f8736894    	ldr	x20, [x4, x19]
1000591a0: 9b167d57    	mul	x23, x10, x22
1000591a4: 9bd67d58    	umulh	x24, x10, x22
1000591a8: 9bd67d39    	umulh	x25, x9, x22
1000591ac: 9b167d36    	mul	x22, x9, x22
1000591b0: 9b157d5a    	mul	x26, x10, x21
1000591b4: 9bd57d5b    	umulh	x27, x10, x21
1000591b8: 9bd57d3c    	umulh	x28, x9, x21
1000591bc: 9b157d35    	mul	x21, x9, x21
1000591c0: ab160316    	adds	x22, x24, x22
1000591c4: 1a9f37f8    	cset	w24, hs
1000591c8: ab190379    	adds	x25, x27, x25
1000591cc: 1a9f37fb    	cset	w27, hs
1000591d0: ab150335    	adds	x21, x25, x21
1000591d4: 9a9b3779    	cinc	x25, x27, hs
1000591d8: ab1a02d6    	adds	x22, x22, x26
1000591dc: ba1802b5    	adcs	x21, x21, x24
1000591e0: 9a190398    	adc	x24, x28, x25
1000591e4: 9b177db9    	mul	x25, x13, x23
1000591e8: 9b197d9a    	mul	x26, x12, x25
1000591ec: 9bd97d9b    	umulh	x27, x12, x25
1000591f0: 9bd97d7c    	umulh	x28, x11, x25
1000591f4: 9b197d79    	mul	x25, x11, x25
1000591f8: ab160376    	adds	x22, x27, x22
1000591fc: 1a9f37fb    	cset	w27, hs
100059200: ab1902d6    	adds	x22, x22, x25
100059204: 9a9b3779    	cinc	x25, x27, hs
100059208: ab1c02b5    	adds	x21, x21, x28
10005920c: 1a9f37fb    	cset	w27, hs
100059210: ab17035f    	cmn	x26, x23
100059214: ba0802d6    	adcs	x22, x22, x8
100059218: ba1902b5    	adcs	x21, x21, x25
10005921c: ba1b0317    	adcs	x23, x24, x27
100059220: 1a9f37f8    	cset	w24, hs
100059224: 9b167db9    	mul	x25, x13, x22
100059228: 9b197d9a    	mul	x26, x12, x25
10005922c: 9bd97d9b    	umulh	x27, x12, x25
100059230: 9bd97d7c    	umulh	x28, x11, x25
100059234: 9b197d79    	mul	x25, x11, x25
100059238: ab150375    	adds	x21, x27, x21
10005923c: 1a9f37fb    	cset	w27, hs
100059240: ab1902b5    	adds	x21, x21, x25
100059244: 9a9b3779    	cinc	x25, x27, hs
100059248: ab1c02f7    	adds	x23, x23, x28
10005924c: 1a9f37fb    	cset	w27, hs
100059250: ab16035f    	cmn	x26, x22
100059254: ba0802b5    	adcs	x21, x21, x8
100059258: ba1902f6    	adcs	x22, x23, x25
10005925c: 9a9b3777    	cinc	x23, x27, hs
100059260: eb0c02bf    	cmp	x21, x12
100059264: fa0b02df    	sbcs	xzr, x22, x11
100059268: aa1802f7    	orr	x23, x23, x24
10005926c: fa403ae0    	ccmp	x23, #0x0, #0x0, lo
100059270: 9a881177    	csel	x23, x11, x8, ne
100059274: 9a881198    	csel	x24, x12, x8, ne
100059278: eb1802b5    	subs	x21, x21, x24
10005927c: da1702d6    	sbc	x22, x22, x23
100059280: ab1402b4    	adds	x20, x21, x20
100059284: ba0802d5    	adcs	x21, x22, x8
100059288: 1a9f37f6    	cset	w22, hs
10005928c: eb0c029f    	cmp	x20, x12
100059290: fa0b02bf    	sbcs	xzr, x21, x11
100059294: 1a9f36d6    	csinc	w22, w22, wzr, lo
100059298: 710002df    	cmp	w22, #0x0
10005929c: 9a881177    	csel	x23, x11, x8, ne
1000592a0: 9a881196    	csel	x22, x12, x8, ne
1000592a4: eb160296    	subs	x22, x20, x22
1000592a8: da1702b5    	sbc	x21, x21, x23
1000592ac: d1002273    	sub	x19, x19, #0x8
1000592b0: b100227f    	cmn	x19, #0x8
1000592b4: 54fff741    	b.ne	0x10005919c <__RNvMNtNtNtCs8IeiyMGxmWr_17field_regressions8campaign10candidates7integerINtB2_10ProjectionKj9_E5batchB8_+0x6c>
1000592b8: 8b061073    	add	x19, x3, x6, lsl #4
1000592bc: 910004c6    	add	x6, x6, #0x1
1000592c0: 937ffce7    	asr	x7, x7, #63
1000592c4: 8a0e00f4    	and	x20, x7, x14
1000592c8: 8a0f00e7    	and	x7, x7, x15
1000592cc: eb0702c7    	subs	x7, x22, x7
1000592d0: fa1402b4    	sbcs	x20, x21, x20
1000592d4: 1a9f27f5    	cset	w21, lo
1000592d8: 390023f5    	strb	w21, [sp, #0x8]
1000592dc: 394023f5    	ldrb	w21, [sp, #0x8]
1000592e0: aa0803f6    	mov	x22, x8
1000592e4: f2401ebf    	tst	x21, #0xff
1000592e8: 9a8810b6    	csel	x22, x5, x8, ne
1000592ec: aa0803f7    	mov	x23, x8
1000592f0: f2401ebf    	tst	x21, #0xff
1000592f4: 9a881017    	csel	x23, x0, x8, ne
1000592f8: ab1600e7    	adds	x7, x7, x22
1000592fc: 9a1402f4    	adc	x20, x23, x20
100059300: a9005267    	stp	x7, x20, [x19]
100059304: 91012084    	add	x4, x4, #0x48
100059308: eb0200df    	cmp	x6, x2
10005930c: 54fff3e1    	b.ne	0x100059188 <__RNvMNtNtNtCs8IeiyMGxmWr_17field_regressions8campaign10candidates7integerINtB2_10ProjectionKj9_E5batchB8_+0x58>
100059310: a9467bfd    	ldp	x29, x30, [sp, #0x60]
100059314: a9454ff4    	ldp	x20, x19, [sp, #0x50]
100059318: a94457f6    	ldp	x22, x21, [sp, #0x40]
10005931c: a9435ff8    	ldp	x24, x23, [sp, #0x30]
100059320: a94267fa    	ldp	x26, x25, [sp, #0x20]
100059324: a9416ffc    	ldp	x28, x27, [sp, #0x10]
100059328: 9101c3ff    	add	sp, sp, #0x70
10005932c: d65f03c0    	ret
100059330: f00008e4    	adrp	x4, 0x100178000 <dyld_stub_binder+0x100178000>
100059334: 9124a084    	add	x4, x4, #0x928
100059338: 910003e0    	mov	x0, sp
10005933c: 910023e1    	add	x1, sp, #0x8
100059340: d2800002    	mov	x2, #0x0                ; =0
100059344: 94034d0a    	bl	0x10012c76c <__RINvNtCs8Mbv00yxnRz_4core9panicking13assert_failedjjEB4_>

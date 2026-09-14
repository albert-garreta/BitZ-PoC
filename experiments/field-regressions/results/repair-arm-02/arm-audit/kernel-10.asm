
/private/var/folders/q0/v7l8dzd13k7f_q6j80sy03lm0000gn/T/f2z-field-gated-aejimxw9/field-regressions:	file format mach-o arm64

Disassembly of section __TEXT,__text:

00000001000711b0 <__RNvMNtNtNtCsaHyC9lX8wyC_17field_regressions8campaign10candidates7integerINtB2_10ProjectionKj9_E5batchB8_>:
1000711b0: d101c3ff    	sub	sp, sp, #0x70
1000711b4: a9016ffc    	stp	x28, x27, [sp, #0x10]
1000711b8: a90267fa    	stp	x26, x25, [sp, #0x20]
1000711bc: a9035ff8    	stp	x24, x23, [sp, #0x30]
1000711c0: a90457f6    	stp	x22, x21, [sp, #0x40]
1000711c4: a9054ff4    	stp	x20, x19, [sp, #0x50]
1000711c8: a9067bfd    	stp	x29, x30, [sp, #0x60]
1000711cc: 910183fd    	add	x29, sp, #0x60
1000711d0: a90013e2    	stp	x2, x4, [sp]
1000711d4: eb04005f    	cmp	x2, x4
1000711d8: 54000ec1    	b.ne	0x1000713b0 <__RNvMNtNtNtCsaHyC9lX8wyC_17field_regressions8campaign10candidates7integerINtB2_10ProjectionKj9_E5batchB8_+0x200>
1000711dc: b4000da2    	cbz	x2, 0x100071390 <__RNvMNtNtNtCsaHyC9lX8wyC_17field_regressions8campaign10candidates7integerINtB2_10ProjectionKj9_E5batchB8_+0x1e0>
1000711e0: d2800008    	mov	x8, #0x0                ; =0
1000711e4: a949240a    	ldp	x10, x9, [x0, #0x90]
1000711e8: a9442c0c    	ldp	x12, x11, [x0, #0x40]
1000711ec: f940380d    	ldr	x13, [x0, #0x70]
1000711f0: a94a380f    	ldp	x15, x14, [x0, #0xa0]
1000711f4: 52800910    	mov	w16, #0x48              ; =72
1000711f8: 910023f1    	add	x17, sp, #0x8
1000711fc: aa0103e4    	mov	x4, x1
100071200: d2800006    	mov	x6, #0x0                ; =0
100071204: a9480005    	ldp	x5, x0, [x0, #0x80]
100071208: d2800015    	mov	x21, #0x0               ; =0
10007120c: 9b1004c7    	madd	x7, x6, x16, x1
100071210: f94020e7    	ldr	x7, [x7, #0x40]
100071214: 52800713    	mov	w19, #0x38              ; =56
100071218: aa0703f6    	mov	x22, x7
10007121c: f8736894    	ldr	x20, [x4, x19]
100071220: 9b167d57    	mul	x23, x10, x22
100071224: 9bd67d58    	umulh	x24, x10, x22
100071228: 9bd67d39    	umulh	x25, x9, x22
10007122c: 9b167d36    	mul	x22, x9, x22
100071230: 9b157d5a    	mul	x26, x10, x21
100071234: 9bd57d5b    	umulh	x27, x10, x21
100071238: 9bd57d3c    	umulh	x28, x9, x21
10007123c: 9b157d35    	mul	x21, x9, x21
100071240: ab160316    	adds	x22, x24, x22
100071244: 1a9f37f8    	cset	w24, hs
100071248: ab190379    	adds	x25, x27, x25
10007124c: 1a9f37fb    	cset	w27, hs
100071250: ab150335    	adds	x21, x25, x21
100071254: 9a9b3779    	cinc	x25, x27, hs
100071258: ab1a02d6    	adds	x22, x22, x26
10007125c: ba1802b5    	adcs	x21, x21, x24
100071260: 9a190398    	adc	x24, x28, x25
100071264: 9b177db9    	mul	x25, x13, x23
100071268: 9b197d9a    	mul	x26, x12, x25
10007126c: 9bd97d9b    	umulh	x27, x12, x25
100071270: 9bd97d7c    	umulh	x28, x11, x25
100071274: 9b197d79    	mul	x25, x11, x25
100071278: ab160376    	adds	x22, x27, x22
10007127c: 1a9f37fb    	cset	w27, hs
100071280: ab1902d6    	adds	x22, x22, x25
100071284: 9a9b3779    	cinc	x25, x27, hs
100071288: ab1c02b5    	adds	x21, x21, x28
10007128c: 1a9f37fb    	cset	w27, hs
100071290: ab17035f    	cmn	x26, x23
100071294: ba0802d6    	adcs	x22, x22, x8
100071298: ba1902b5    	adcs	x21, x21, x25
10007129c: ba1b0317    	adcs	x23, x24, x27
1000712a0: 1a9f37f8    	cset	w24, hs
1000712a4: 9b167db9    	mul	x25, x13, x22
1000712a8: 9b197d9a    	mul	x26, x12, x25
1000712ac: 9bd97d9b    	umulh	x27, x12, x25
1000712b0: 9bd97d7c    	umulh	x28, x11, x25
1000712b4: 9b197d79    	mul	x25, x11, x25
1000712b8: ab150375    	adds	x21, x27, x21
1000712bc: 1a9f37fb    	cset	w27, hs
1000712c0: ab1902b5    	adds	x21, x21, x25
1000712c4: 9a9b3779    	cinc	x25, x27, hs
1000712c8: ab1c02f7    	adds	x23, x23, x28
1000712cc: 1a9f37fb    	cset	w27, hs
1000712d0: ab16035f    	cmn	x26, x22
1000712d4: ba0802b5    	adcs	x21, x21, x8
1000712d8: ba1902f6    	adcs	x22, x23, x25
1000712dc: 9a9b3777    	cinc	x23, x27, hs
1000712e0: eb0c02bf    	cmp	x21, x12
1000712e4: fa0b02df    	sbcs	xzr, x22, x11
1000712e8: aa1802f7    	orr	x23, x23, x24
1000712ec: fa403ae0    	ccmp	x23, #0x0, #0x0, lo
1000712f0: 9a881177    	csel	x23, x11, x8, ne
1000712f4: 9a881198    	csel	x24, x12, x8, ne
1000712f8: eb1802b5    	subs	x21, x21, x24
1000712fc: da1702d6    	sbc	x22, x22, x23
100071300: ab1402b4    	adds	x20, x21, x20
100071304: ba0802d5    	adcs	x21, x22, x8
100071308: 1a9f37f6    	cset	w22, hs
10007130c: eb0c029f    	cmp	x20, x12
100071310: fa0b02bf    	sbcs	xzr, x21, x11
100071314: 1a9f36d6    	csinc	w22, w22, wzr, lo
100071318: 710002df    	cmp	w22, #0x0
10007131c: 9a881177    	csel	x23, x11, x8, ne
100071320: 9a881196    	csel	x22, x12, x8, ne
100071324: eb160296    	subs	x22, x20, x22
100071328: da1702b5    	sbc	x21, x21, x23
10007132c: d1002273    	sub	x19, x19, #0x8
100071330: b100227f    	cmn	x19, #0x8
100071334: 54fff741    	b.ne	0x10007121c <__RNvMNtNtNtCsaHyC9lX8wyC_17field_regressions8campaign10candidates7integerINtB2_10ProjectionKj9_E5batchB8_+0x6c>
100071338: 8b061073    	add	x19, x3, x6, lsl #4
10007133c: 910004c6    	add	x6, x6, #0x1
100071340: 937ffce7    	asr	x7, x7, #63
100071344: 8a0e00f4    	and	x20, x7, x14
100071348: 8a0f00e7    	and	x7, x7, x15
10007134c: eb0702c7    	subs	x7, x22, x7
100071350: fa1402b4    	sbcs	x20, x21, x20
100071354: 1a9f27f5    	cset	w21, lo
100071358: 390023f5    	strb	w21, [sp, #0x8]
10007135c: 394023f5    	ldrb	w21, [sp, #0x8]
100071360: aa0803f6    	mov	x22, x8
100071364: f2401ebf    	tst	x21, #0xff
100071368: 9a8810b6    	csel	x22, x5, x8, ne
10007136c: aa0803f7    	mov	x23, x8
100071370: f2401ebf    	tst	x21, #0xff
100071374: 9a881017    	csel	x23, x0, x8, ne
100071378: ab1600e7    	adds	x7, x7, x22
10007137c: 9a1402f4    	adc	x20, x23, x20
100071380: a9005267    	stp	x7, x20, [x19]
100071384: 91012084    	add	x4, x4, #0x48
100071388: eb0200df    	cmp	x6, x2
10007138c: 54fff3e1    	b.ne	0x100071208 <__RNvMNtNtNtCsaHyC9lX8wyC_17field_regressions8campaign10candidates7integerINtB2_10ProjectionKj9_E5batchB8_+0x58>
100071390: a9467bfd    	ldp	x29, x30, [sp, #0x60]
100071394: a9454ff4    	ldp	x20, x19, [sp, #0x50]
100071398: a94457f6    	ldp	x22, x21, [sp, #0x40]
10007139c: a9435ff8    	ldp	x24, x23, [sp, #0x30]
1000713a0: a94267fa    	ldp	x26, x25, [sp, #0x20]
1000713a4: a9416ffc    	ldp	x28, x27, [sp, #0x10]
1000713a8: 9101c3ff    	add	sp, sp, #0x70
1000713ac: d65f03c0    	ret
1000713b0: 90000964    	adrp	x4, 0x10019d000 <dyld_stub_binder+0x10019d000>
1000713b4: 910fc084    	add	x4, x4, #0x3f0
1000713b8: 910003e0    	mov	x0, sp
1000713bc: 910023e1    	add	x1, sp, #0x8
1000713c0: d2800002    	mov	x2, #0x0                ; =0
1000713c4: 94036969    	bl	0x10014b968 <__RINvNtCs8Mbv00yxnRz_4core9panicking13assert_failedjjEB4_>


/private/var/folders/q0/v7l8dzd13k7f_q6j80sy03lm0000gn/T/f2z-field-gated-pycl0nhx/field-regressions:	file format mach-o arm64

Disassembly of section __TEXT,__text:

000000010002c320 <__RINvNtNtNtCsagpXUhzPPpl_17field_regressions8campaign10candidates5prime5batchKb1_EB8_>:
10002c320: d10483ff    	sub	sp, sp, #0x120
10002c324: a90d67fa    	stp	x26, x25, [sp, #0xd0]
10002c328: a90e5ff8    	stp	x24, x23, [sp, #0xe0]
10002c32c: a90f57f6    	stp	x22, x21, [sp, #0xf0]
10002c330: a9104ff4    	stp	x20, x19, [sp, #0x100]
10002c334: a9117bfd    	stp	x29, x30, [sp, #0x110]
10002c338: 910443fd    	add	x29, sp, #0x110
10002c33c: a90093e2    	stp	x2, x4, [sp, #0x8]
10002c340: eb04005f    	cmp	x2, x4
10002c344: 54001e41    	b.ne	0x10002c70c <__RINvNtNtNtCsagpXUhzPPpl_17field_regressions8campaign10candidates5prime5batchKb1_EB8_+0x3ec>
10002c348: aa0203f7    	mov	x23, x2
10002c34c: f9003be2    	str	x2, [sp, #0x70]
10002c350: f9000fe6    	str	x6, [sp, #0x18]
10002c354: eb06005f    	cmp	x2, x6
10002c358: 54001e61    	b.ne	0x10002c724 <__RINvNtNtNtCsagpXUhzPPpl_17field_regressions8campaign10candidates5prime5batchKb1_EB8_+0x404>
10002c35c: aa0503f3    	mov	x19, x5
10002c360: aa0303f4    	mov	x20, x3
10002c364: aa0103f6    	mov	x22, x1
10002c368: aa0003f5    	mov	x21, x0
10002c36c: a9456019    	ldp	x25, x24, [x0, #0x50]
10002c370: b4000977    	cbz	x23, 0x10002c49c <__RINvNtNtNtCsagpXUhzPPpl_17field_regressions8campaign10candidates5prime5batchKb1_EB8_+0x17c>
10002c374: a94426aa    	ldp	x10, x9, [x21, #0x40]
10002c378: aa1403eb    	mov	x11, x20
10002c37c: aa1603ec    	mov	x12, x22
10002c380: aa1703ed    	mov	x13, x23
10002c384: aa1903ef    	mov	x15, x25
10002c388: aa1803e8    	mov	x8, x24
10002c38c: f9403aae    	ldr	x14, [x21, #0x70]
10002c390: a8c14191    	ldp	x17, x16, [x12], #0x10
10002c394: f900016f    	str	x15, [x11]
10002c398: aa100220    	orr	x0, x17, x16
10002c39c: f100001f    	cmp	x0, #0x0
10002c3a0: 9a900310    	csel	x16, x24, x16, eq
10002c3a4: 9a910331    	csel	x17, x25, x17, eq
10002c3a8: 9b0f7e20    	mul	x0, x17, x15
10002c3ac: 9bcf7e21    	umulh	x1, x17, x15
10002c3b0: 9bcf7e02    	umulh	x2, x16, x15
10002c3b4: 9b0f7e0f    	mul	x15, x16, x15
10002c3b8: 9b087e23    	mul	x3, x17, x8
10002c3bc: 9bc87e31    	umulh	x17, x17, x8
10002c3c0: 9bc87e04    	umulh	x4, x16, x8
10002c3c4: 9b087e10    	mul	x16, x16, x8
10002c3c8: ab0f002f    	adds	x15, x1, x15
10002c3cc: 1a9f37e1    	cset	w1, hs
10002c3d0: ab020231    	adds	x17, x17, x2
10002c3d4: 1a9f37e2    	cset	w2, hs
10002c3d8: ab100230    	adds	x16, x17, x16
10002c3dc: 9a823451    	cinc	x17, x2, hs
10002c3e0: ab0301ef    	adds	x15, x15, x3
10002c3e4: ba010210    	adcs	x16, x16, x1
10002c3e8: 9a110091    	adc	x17, x4, x17
10002c3ec: 9b007dc1    	mul	x1, x14, x0
10002c3f0: 9b017d42    	mul	x2, x10, x1
10002c3f4: 9bc17d43    	umulh	x3, x10, x1
10002c3f8: 9bc17d24    	umulh	x4, x9, x1
10002c3fc: 9b017d21    	mul	x1, x9, x1
10002c400: ab0f006f    	adds	x15, x3, x15
10002c404: 1a9f37e3    	cset	w3, hs
10002c408: ab0101ef    	adds	x15, x15, x1
10002c40c: 9a833461    	cinc	x1, x3, hs
10002c410: ab040210    	adds	x16, x16, x4
10002c414: 1a9f37e3    	cset	w3, hs
10002c418: ab00005f    	cmn	x2, x0
10002c41c: ba1f01ef    	adcs	x15, x15, xzr
10002c420: ba010210    	adcs	x16, x16, x1
10002c424: ba030231    	adcs	x17, x17, x3
10002c428: 1a9f37e0    	cset	w0, hs
10002c42c: 9b0f7dc1    	mul	x1, x14, x15
10002c430: 9b017d42    	mul	x2, x10, x1
10002c434: 9bc17d43    	umulh	x3, x10, x1
10002c438: 9bc17d24    	umulh	x4, x9, x1
10002c43c: 9b017d21    	mul	x1, x9, x1
10002c440: ab100070    	adds	x16, x3, x16
10002c444: 1a9f37e3    	cset	w3, hs
10002c448: ab010210    	adds	x16, x16, x1
10002c44c: 9a833461    	cinc	x1, x3, hs
10002c450: ab040231    	adds	x17, x17, x4
10002c454: 1a9f37e3    	cset	w3, hs
10002c458: ab0f005f    	cmn	x2, x15
10002c45c: ba1f020f    	adcs	x15, x16, xzr
10002c460: ba010230    	adcs	x16, x17, x1
10002c464: 9a833471    	cinc	x17, x3, hs
10002c468: eb0a01ff    	cmp	x15, x10
10002c46c: fa09021f    	sbcs	xzr, x16, x9
10002c470: aa000231    	orr	x17, x17, x0
10002c474: fa403a20    	ccmp	x17, #0x0, #0x0, lo
10002c478: 9a9f1131    	csel	x17, x9, xzr, ne
10002c47c: 9a9f1140    	csel	x0, x10, xzr, ne
10002c480: eb0001ef    	subs	x15, x15, x0
10002c484: f9000568    	str	x8, [x11, #0x8]
10002c488: da110208    	sbc	x8, x16, x17
10002c48c: 9100416b    	add	x11, x11, #0x10
10002c490: f10005ad    	subs	x13, x13, #0x1
10002c494: 54fff7e1    	b.ne	0x10002c390 <__RINvNtNtNtCsagpXUhzPPpl_17field_regressions8campaign10candidates5prime5batchKb1_EB8_+0x70>
10002c498: 14000003    	b	0x10002c4a4 <__RINvNtNtNtCsagpXUhzPPpl_17field_regressions8campaign10candidates5prime5batchKb1_EB8_+0x184>
10002c49c: aa1903ef    	mov	x15, x25
10002c4a0: aa1803e8    	mov	x8, x24
10002c4a4: a90623ef    	stp	x15, x8, [sp, #0x60]
10002c4a8: ad4006a0    	ldp	q0, q1, [x21]
10002c4ac: ad0107e0    	stp	q0, q1, [sp, #0x20]
10002c4b0: ad4106a0    	ldp	q0, q1, [x21, #0x20]
10002c4b4: ad0207e0    	stp	q0, q1, [sp, #0x40]
10002c4b8: 9101c3e0    	add	x0, sp, #0x70
10002c4bc: 910083e1    	add	x1, sp, #0x20
10002c4c0: 94013b60    	bl	0x10007b240 <__RNvMNtNtNtCs4rJR5Xg1K3s_13crypto_bigint7modular16fixed_monty_form6invertINtB4_14FixedMontyFormKj2_E6invertCsagpXUhzPPpl_17field_regressions>
10002c4c4: 394303e8    	ldrb	w8, [sp, #0xc0]
10002c4c8: 381bf3a8    	sturb	w8, [x29, #-0x41]
10002c4cc: d10107a8    	sub	x8, x29, #0x41
10002c4d0: 385bf3a8    	ldurb	w8, [x29, #-0x41]
10002c4d4: 34001348    	cbz	w8, 0x10002c73c <__RINvNtNtNtCsagpXUhzPPpl_17field_regressions8campaign10candidates5prime5batchKb1_EB8_+0x41c>
10002c4d8: b40010d7    	cbz	x23, 0x10002c6f0 <__RINvNtNtNtCsagpXUhzPPpl_17field_regressions8campaign10candidates5prime5batchKb1_EB8_+0x3d0>
10002c4dc: a94b3ff0    	ldp	x16, x15, [sp, #0xb0]
10002c4e0: d37ceee8    	lsl	x8, x23, #4
10002c4e4: a94426aa    	ldp	x10, x9, [x21, #0x40]
10002c4e8: d10042cb    	sub	x11, x22, #0x10
10002c4ec: d100428c    	sub	x12, x20, #0x10
10002c4f0: d100426d    	sub	x13, x19, #0x10
10002c4f4: f9403aae    	ldr	x14, [x21, #0x70]
10002c4f8: 8b080161    	add	x1, x11, x8
10002c4fc: 8b080182    	add	x2, x12, x8
10002c500: 8b0801b1    	add	x17, x13, x8
10002c504: a9400021    	ldp	x1, x0, [x1]
10002c508: a9400c44    	ldp	x4, x3, [x2]
10002c50c: aa000022    	orr	x2, x1, x0
10002c510: 9b107c85    	mul	x5, x4, x16
10002c514: 9bd07c86    	umulh	x6, x4, x16
10002c518: 9bd07c67    	umulh	x7, x3, x16
10002c51c: 9b107c73    	mul	x19, x3, x16
10002c520: 9b0f7c94    	mul	x20, x4, x15
10002c524: 9bcf7c84    	umulh	x4, x4, x15
10002c528: 9bcf7c75    	umulh	x21, x3, x15
10002c52c: 9b0f7c63    	mul	x3, x3, x15
10002c530: ab1300c6    	adds	x6, x6, x19
10002c534: 1a9f37f3    	cset	w19, hs
10002c538: ab070084    	adds	x4, x4, x7
10002c53c: 1a9f37e7    	cset	w7, hs
10002c540: ab030083    	adds	x3, x4, x3
10002c544: 9a8734e4    	cinc	x4, x7, hs
10002c548: ab1400c6    	adds	x6, x6, x20
10002c54c: ba130063    	adcs	x3, x3, x19
10002c550: 9a0402a4    	adc	x4, x21, x4
10002c554: 9b057dc7    	mul	x7, x14, x5
10002c558: 9b077d53    	mul	x19, x10, x7
10002c55c: 9bc77d54    	umulh	x20, x10, x7
10002c560: 9bc77d35    	umulh	x21, x9, x7
10002c564: 9b077d27    	mul	x7, x9, x7
10002c568: ab060286    	adds	x6, x20, x6
10002c56c: 1a9f37f4    	cset	w20, hs
10002c570: ab0700c6    	adds	x6, x6, x7
10002c574: 9a943687    	cinc	x7, x20, hs
10002c578: ab150063    	adds	x3, x3, x21
10002c57c: 1a9f37f4    	cset	w20, hs
10002c580: ab05027f    	cmn	x19, x5
10002c584: ba1f00c5    	adcs	x5, x6, xzr
10002c588: ba070063    	adcs	x3, x3, x7
10002c58c: ba140084    	adcs	x4, x4, x20
10002c590: 1a9f37e6    	cset	w6, hs
10002c594: 9b057dc7    	mul	x7, x14, x5
10002c598: 9b077d53    	mul	x19, x10, x7
10002c59c: 9bc77d54    	umulh	x20, x10, x7
10002c5a0: 9bc77d35    	umulh	x21, x9, x7
10002c5a4: 9b077d27    	mul	x7, x9, x7
10002c5a8: ab030283    	adds	x3, x20, x3
10002c5ac: 1a9f37f4    	cset	w20, hs
10002c5b0: ab070063    	adds	x3, x3, x7
10002c5b4: 9a943687    	cinc	x7, x20, hs
10002c5b8: ab150084    	adds	x4, x4, x21
10002c5bc: 1a9f37f4    	cset	w20, hs
10002c5c0: ab05027f    	cmn	x19, x5
10002c5c4: ba1f0063    	adcs	x3, x3, xzr
10002c5c8: ba070084    	adcs	x4, x4, x7
10002c5cc: 9a943685    	cinc	x5, x20, hs
10002c5d0: eb0a007f    	cmp	x3, x10
10002c5d4: fa09009f    	sbcs	xzr, x4, x9
10002c5d8: aa0600a5    	orr	x5, x5, x6
10002c5dc: fa4038a0    	ccmp	x5, #0x0, #0x0, lo
10002c5e0: 9a9f1125    	csel	x5, x9, xzr, ne
10002c5e4: 9a9f1146    	csel	x6, x10, xzr, ne
10002c5e8: eb060063    	subs	x3, x3, x6
10002c5ec: da050084    	sbc	x4, x4, x5
10002c5f0: f100005f    	cmp	x2, #0x0
10002c5f4: 9a8403e2    	csel	x2, xzr, x4, eq
10002c5f8: 9a8303e3    	csel	x3, xzr, x3, eq
10002c5fc: a9000a23    	stp	x3, x2, [x17]
10002c600: 9a800311    	csel	x17, x24, x0, eq
10002c604: 9a810320    	csel	x0, x25, x1, eq
10002c608: 9b107c01    	mul	x1, x0, x16
10002c60c: 9bd07c02    	umulh	x2, x0, x16
10002c610: 9bd07e23    	umulh	x3, x17, x16
10002c614: 9b107e30    	mul	x16, x17, x16
10002c618: 9b0f7c04    	mul	x4, x0, x15
10002c61c: 9bcf7c00    	umulh	x0, x0, x15
10002c620: 9bcf7e25    	umulh	x5, x17, x15
10002c624: 9b0f7e2f    	mul	x15, x17, x15
10002c628: ab100050    	adds	x16, x2, x16
10002c62c: 1a9f37f1    	cset	w17, hs
10002c630: ab030000    	adds	x0, x0, x3
10002c634: 1a9f37e2    	cset	w2, hs
10002c638: ab0f000f    	adds	x15, x0, x15
10002c63c: 9a823440    	cinc	x0, x2, hs
10002c640: ab040210    	adds	x16, x16, x4
10002c644: ba1101ef    	adcs	x15, x15, x17
10002c648: 9a0000b1    	adc	x17, x5, x0
10002c64c: 9b017dc0    	mul	x0, x14, x1
10002c650: 9b007d42    	mul	x2, x10, x0
10002c654: 9bc07d43    	umulh	x3, x10, x0
10002c658: 9bc07d24    	umulh	x4, x9, x0
10002c65c: 9b007d20    	mul	x0, x9, x0
10002c660: ab100070    	adds	x16, x3, x16
10002c664: 1a9f37e3    	cset	w3, hs
10002c668: ab000210    	adds	x16, x16, x0
10002c66c: 9a833460    	cinc	x0, x3, hs
10002c670: ab0401ef    	adds	x15, x15, x4
10002c674: 1a9f37e3    	cset	w3, hs
10002c678: ab01005f    	cmn	x2, x1
10002c67c: ba1f0210    	adcs	x16, x16, xzr
10002c680: ba0001ef    	adcs	x15, x15, x0
10002c684: ba030231    	adcs	x17, x17, x3
10002c688: 1a9f37e0    	cset	w0, hs
10002c68c: 9b107dc1    	mul	x1, x14, x16
10002c690: 9b017d42    	mul	x2, x10, x1
10002c694: 9bc17d43    	umulh	x3, x10, x1
10002c698: 9bc17d24    	umulh	x4, x9, x1
10002c69c: 9b017d21    	mul	x1, x9, x1
10002c6a0: ab0f006f    	adds	x15, x3, x15
10002c6a4: 1a9f37e3    	cset	w3, hs
10002c6a8: ab0101ef    	adds	x15, x15, x1
10002c6ac: 9a833461    	cinc	x1, x3, hs
10002c6b0: ab040231    	adds	x17, x17, x4
10002c6b4: 1a9f37e3    	cset	w3, hs
10002c6b8: ab10005f    	cmn	x2, x16
10002c6bc: ba1f01ef    	adcs	x15, x15, xzr
10002c6c0: ba010231    	adcs	x17, x17, x1
10002c6c4: 9a833470    	cinc	x16, x3, hs
10002c6c8: eb0a01ff    	cmp	x15, x10
10002c6cc: fa09023f    	sbcs	xzr, x17, x9
10002c6d0: aa000210    	orr	x16, x16, x0
10002c6d4: fa403a00    	ccmp	x16, #0x0, #0x0, lo
10002c6d8: 9a9f1120    	csel	x0, x9, xzr, ne
10002c6dc: 9a9f1150    	csel	x16, x10, xzr, ne
10002c6e0: eb1001f0    	subs	x16, x15, x16
10002c6e4: da00022f    	sbc	x15, x17, x0
10002c6e8: f1004108    	subs	x8, x8, #0x10
10002c6ec: 54fff061    	b.ne	0x10002c4f8 <__RINvNtNtNtCsagpXUhzPPpl_17field_regressions8campaign10candidates5prime5batchKb1_EB8_+0x1d8>
10002c6f0: a9517bfd    	ldp	x29, x30, [sp, #0x110]
10002c6f4: a9504ff4    	ldp	x20, x19, [sp, #0x100]
10002c6f8: a94f57f6    	ldp	x22, x21, [sp, #0xf0]
10002c6fc: a94e5ff8    	ldp	x24, x23, [sp, #0xe0]
10002c700: a94d67fa    	ldp	x26, x25, [sp, #0xd0]
10002c704: 910483ff    	add	sp, sp, #0x120
10002c708: d65f03c0    	ret
10002c70c: f0000c04    	adrp	x4, 0x1001af000 <dyld_stub_binder+0x1001af000>
10002c710: 9132c084    	add	x4, x4, #0xcb0
10002c714: 910023e0    	add	x0, sp, #0x8
10002c718: 910043e1    	add	x1, sp, #0x10
10002c71c: d2800002    	mov	x2, #0x0                ; =0
10002c720: 9404bbcc    	bl	0x10015b650 <__RINvNtCs8Mbv00yxnRz_4core9panicking13assert_failedjjEB4_>
10002c724: f0000c04    	adrp	x4, 0x1001af000 <dyld_stub_binder+0x1001af000>
10002c728: 91332084    	add	x4, x4, #0xcc8
10002c72c: 9101c3e0    	add	x0, sp, #0x70
10002c730: 910063e1    	add	x1, sp, #0x18
10002c734: d2800002    	mov	x2, #0x0                ; =0
10002c738: 9404bbc6    	bl	0x10015b650 <__RINvNtCs8Mbv00yxnRz_4core9panicking13assert_failedjjEB4_>
10002c73c: d00009c0    	adrp	x0, 0x100166000 <dyld_stub_binder+0x100166000>
10002c740: 91078c00    	add	x0, x0, #0x1e3
10002c744: f0000c02    	adrp	x2, 0x1001af000 <dyld_stub_binder+0x1001af000>
10002c748: 9133e042    	add	x2, x2, #0xcf8
10002c74c: 52801321    	mov	w1, #0x99               ; =153
10002c750: 9404bba8    	bl	0x10015b5f0 <__RNvNtCs8Mbv00yxnRz_4core9panicking9panic_fmt>

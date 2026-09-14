
/private/var/folders/q0/v7l8dzd13k7f_q6j80sy03lm0000gn/T/f2z-field-gated-aejimxw9/field-regressions:	file format mach-o arm64

Disassembly of section __TEXT,__text:

000000010002b4f4 <__RINvNtNtNtCsaHyC9lX8wyC_17field_regressions8campaign10candidates5prime5batchKb1_EB8_>:
10002b4f4: d10483ff    	sub	sp, sp, #0x120
10002b4f8: a90d67fa    	stp	x26, x25, [sp, #0xd0]
10002b4fc: a90e5ff8    	stp	x24, x23, [sp, #0xe0]
10002b500: a90f57f6    	stp	x22, x21, [sp, #0xf0]
10002b504: a9104ff4    	stp	x20, x19, [sp, #0x100]
10002b508: a9117bfd    	stp	x29, x30, [sp, #0x110]
10002b50c: 910443fd    	add	x29, sp, #0x110
10002b510: a90093e2    	stp	x2, x4, [sp, #0x8]
10002b514: eb04005f    	cmp	x2, x4
10002b518: 54001e41    	b.ne	0x10002b8e0 <__RINvNtNtNtCsaHyC9lX8wyC_17field_regressions8campaign10candidates5prime5batchKb1_EB8_+0x3ec>
10002b51c: aa0203f7    	mov	x23, x2
10002b520: f9003be2    	str	x2, [sp, #0x70]
10002b524: f9000fe6    	str	x6, [sp, #0x18]
10002b528: eb06005f    	cmp	x2, x6
10002b52c: 54001e61    	b.ne	0x10002b8f8 <__RINvNtNtNtCsaHyC9lX8wyC_17field_regressions8campaign10candidates5prime5batchKb1_EB8_+0x404>
10002b530: aa0503f3    	mov	x19, x5
10002b534: aa0303f4    	mov	x20, x3
10002b538: aa0103f6    	mov	x22, x1
10002b53c: aa0003f5    	mov	x21, x0
10002b540: a9456019    	ldp	x25, x24, [x0, #0x50]
10002b544: b4000977    	cbz	x23, 0x10002b670 <__RINvNtNtNtCsaHyC9lX8wyC_17field_regressions8campaign10candidates5prime5batchKb1_EB8_+0x17c>
10002b548: a94426aa    	ldp	x10, x9, [x21, #0x40]
10002b54c: aa1403eb    	mov	x11, x20
10002b550: aa1603ec    	mov	x12, x22
10002b554: aa1703ed    	mov	x13, x23
10002b558: aa1903ef    	mov	x15, x25
10002b55c: aa1803e8    	mov	x8, x24
10002b560: f9403aae    	ldr	x14, [x21, #0x70]
10002b564: a8c14191    	ldp	x17, x16, [x12], #0x10
10002b568: f900016f    	str	x15, [x11]
10002b56c: aa100220    	orr	x0, x17, x16
10002b570: f100001f    	cmp	x0, #0x0
10002b574: 9a900310    	csel	x16, x24, x16, eq
10002b578: 9a910331    	csel	x17, x25, x17, eq
10002b57c: 9b0f7e20    	mul	x0, x17, x15
10002b580: 9bcf7e21    	umulh	x1, x17, x15
10002b584: 9bcf7e02    	umulh	x2, x16, x15
10002b588: 9b0f7e0f    	mul	x15, x16, x15
10002b58c: 9b087e23    	mul	x3, x17, x8
10002b590: 9bc87e31    	umulh	x17, x17, x8
10002b594: 9bc87e04    	umulh	x4, x16, x8
10002b598: 9b087e10    	mul	x16, x16, x8
10002b59c: ab0f002f    	adds	x15, x1, x15
10002b5a0: 1a9f37e1    	cset	w1, hs
10002b5a4: ab020231    	adds	x17, x17, x2
10002b5a8: 1a9f37e2    	cset	w2, hs
10002b5ac: ab100230    	adds	x16, x17, x16
10002b5b0: 9a823451    	cinc	x17, x2, hs
10002b5b4: ab0301ef    	adds	x15, x15, x3
10002b5b8: ba010210    	adcs	x16, x16, x1
10002b5bc: 9a110091    	adc	x17, x4, x17
10002b5c0: 9b007dc1    	mul	x1, x14, x0
10002b5c4: 9b017d42    	mul	x2, x10, x1
10002b5c8: 9bc17d43    	umulh	x3, x10, x1
10002b5cc: 9bc17d24    	umulh	x4, x9, x1
10002b5d0: 9b017d21    	mul	x1, x9, x1
10002b5d4: ab0f006f    	adds	x15, x3, x15
10002b5d8: 1a9f37e3    	cset	w3, hs
10002b5dc: ab0101ef    	adds	x15, x15, x1
10002b5e0: 9a833461    	cinc	x1, x3, hs
10002b5e4: ab040210    	adds	x16, x16, x4
10002b5e8: 1a9f37e3    	cset	w3, hs
10002b5ec: ab00005f    	cmn	x2, x0
10002b5f0: ba1f01ef    	adcs	x15, x15, xzr
10002b5f4: ba010210    	adcs	x16, x16, x1
10002b5f8: ba030231    	adcs	x17, x17, x3
10002b5fc: 1a9f37e0    	cset	w0, hs
10002b600: 9b0f7dc1    	mul	x1, x14, x15
10002b604: 9b017d42    	mul	x2, x10, x1
10002b608: 9bc17d43    	umulh	x3, x10, x1
10002b60c: 9bc17d24    	umulh	x4, x9, x1
10002b610: 9b017d21    	mul	x1, x9, x1
10002b614: ab100070    	adds	x16, x3, x16
10002b618: 1a9f37e3    	cset	w3, hs
10002b61c: ab010210    	adds	x16, x16, x1
10002b620: 9a833461    	cinc	x1, x3, hs
10002b624: ab040231    	adds	x17, x17, x4
10002b628: 1a9f37e3    	cset	w3, hs
10002b62c: ab0f005f    	cmn	x2, x15
10002b630: ba1f020f    	adcs	x15, x16, xzr
10002b634: ba010230    	adcs	x16, x17, x1
10002b638: 9a833471    	cinc	x17, x3, hs
10002b63c: eb0a01ff    	cmp	x15, x10
10002b640: fa09021f    	sbcs	xzr, x16, x9
10002b644: aa000231    	orr	x17, x17, x0
10002b648: fa403a20    	ccmp	x17, #0x0, #0x0, lo
10002b64c: 9a9f1131    	csel	x17, x9, xzr, ne
10002b650: 9a9f1140    	csel	x0, x10, xzr, ne
10002b654: eb0001ef    	subs	x15, x15, x0
10002b658: f9000568    	str	x8, [x11, #0x8]
10002b65c: da110208    	sbc	x8, x16, x17
10002b660: 9100416b    	add	x11, x11, #0x10
10002b664: f10005ad    	subs	x13, x13, #0x1
10002b668: 54fff7e1    	b.ne	0x10002b564 <__RINvNtNtNtCsaHyC9lX8wyC_17field_regressions8campaign10candidates5prime5batchKb1_EB8_+0x70>
10002b66c: 14000003    	b	0x10002b678 <__RINvNtNtNtCsaHyC9lX8wyC_17field_regressions8campaign10candidates5prime5batchKb1_EB8_+0x184>
10002b670: aa1903ef    	mov	x15, x25
10002b674: aa1803e8    	mov	x8, x24
10002b678: a90623ef    	stp	x15, x8, [sp, #0x60]
10002b67c: ad4006a0    	ldp	q0, q1, [x21]
10002b680: ad0107e0    	stp	q0, q1, [sp, #0x20]
10002b684: ad4106a0    	ldp	q0, q1, [x21, #0x20]
10002b688: ad0207e0    	stp	q0, q1, [sp, #0x40]
10002b68c: 9101c3e0    	add	x0, sp, #0x70
10002b690: 910083e1    	add	x1, sp, #0x20
10002b694: 9401121e    	bl	0x10006ff0c <__RNvMNtNtNtCs4rJR5Xg1K3s_13crypto_bigint7modular16fixed_monty_form6invertINtB4_14FixedMontyFormKj2_E6invertCsaHyC9lX8wyC_17field_regressions>
10002b698: 394303e8    	ldrb	w8, [sp, #0xc0]
10002b69c: 381bf3a8    	sturb	w8, [x29, #-0x41]
10002b6a0: d10107a8    	sub	x8, x29, #0x41
10002b6a4: 385bf3a8    	ldurb	w8, [x29, #-0x41]
10002b6a8: 34001348    	cbz	w8, 0x10002b910 <__RINvNtNtNtCsaHyC9lX8wyC_17field_regressions8campaign10candidates5prime5batchKb1_EB8_+0x41c>
10002b6ac: b40010d7    	cbz	x23, 0x10002b8c4 <__RINvNtNtNtCsaHyC9lX8wyC_17field_regressions8campaign10candidates5prime5batchKb1_EB8_+0x3d0>
10002b6b0: a94b3ff0    	ldp	x16, x15, [sp, #0xb0]
10002b6b4: d37ceee8    	lsl	x8, x23, #4
10002b6b8: a94426aa    	ldp	x10, x9, [x21, #0x40]
10002b6bc: d10042cb    	sub	x11, x22, #0x10
10002b6c0: d100428c    	sub	x12, x20, #0x10
10002b6c4: d100426d    	sub	x13, x19, #0x10
10002b6c8: f9403aae    	ldr	x14, [x21, #0x70]
10002b6cc: 8b080161    	add	x1, x11, x8
10002b6d0: 8b080182    	add	x2, x12, x8
10002b6d4: 8b0801b1    	add	x17, x13, x8
10002b6d8: a9400021    	ldp	x1, x0, [x1]
10002b6dc: a9400c44    	ldp	x4, x3, [x2]
10002b6e0: aa000022    	orr	x2, x1, x0
10002b6e4: 9b107c85    	mul	x5, x4, x16
10002b6e8: 9bd07c86    	umulh	x6, x4, x16
10002b6ec: 9bd07c67    	umulh	x7, x3, x16
10002b6f0: 9b107c73    	mul	x19, x3, x16
10002b6f4: 9b0f7c94    	mul	x20, x4, x15
10002b6f8: 9bcf7c84    	umulh	x4, x4, x15
10002b6fc: 9bcf7c75    	umulh	x21, x3, x15
10002b700: 9b0f7c63    	mul	x3, x3, x15
10002b704: ab1300c6    	adds	x6, x6, x19
10002b708: 1a9f37f3    	cset	w19, hs
10002b70c: ab070084    	adds	x4, x4, x7
10002b710: 1a9f37e7    	cset	w7, hs
10002b714: ab030083    	adds	x3, x4, x3
10002b718: 9a8734e4    	cinc	x4, x7, hs
10002b71c: ab1400c6    	adds	x6, x6, x20
10002b720: ba130063    	adcs	x3, x3, x19
10002b724: 9a0402a4    	adc	x4, x21, x4
10002b728: 9b057dc7    	mul	x7, x14, x5
10002b72c: 9b077d53    	mul	x19, x10, x7
10002b730: 9bc77d54    	umulh	x20, x10, x7
10002b734: 9bc77d35    	umulh	x21, x9, x7
10002b738: 9b077d27    	mul	x7, x9, x7
10002b73c: ab060286    	adds	x6, x20, x6
10002b740: 1a9f37f4    	cset	w20, hs
10002b744: ab0700c6    	adds	x6, x6, x7
10002b748: 9a943687    	cinc	x7, x20, hs
10002b74c: ab150063    	adds	x3, x3, x21
10002b750: 1a9f37f4    	cset	w20, hs
10002b754: ab05027f    	cmn	x19, x5
10002b758: ba1f00c5    	adcs	x5, x6, xzr
10002b75c: ba070063    	adcs	x3, x3, x7
10002b760: ba140084    	adcs	x4, x4, x20
10002b764: 1a9f37e6    	cset	w6, hs
10002b768: 9b057dc7    	mul	x7, x14, x5
10002b76c: 9b077d53    	mul	x19, x10, x7
10002b770: 9bc77d54    	umulh	x20, x10, x7
10002b774: 9bc77d35    	umulh	x21, x9, x7
10002b778: 9b077d27    	mul	x7, x9, x7
10002b77c: ab030283    	adds	x3, x20, x3
10002b780: 1a9f37f4    	cset	w20, hs
10002b784: ab070063    	adds	x3, x3, x7
10002b788: 9a943687    	cinc	x7, x20, hs
10002b78c: ab150084    	adds	x4, x4, x21
10002b790: 1a9f37f4    	cset	w20, hs
10002b794: ab05027f    	cmn	x19, x5
10002b798: ba1f0063    	adcs	x3, x3, xzr
10002b79c: ba070084    	adcs	x4, x4, x7
10002b7a0: 9a943685    	cinc	x5, x20, hs
10002b7a4: eb0a007f    	cmp	x3, x10
10002b7a8: fa09009f    	sbcs	xzr, x4, x9
10002b7ac: aa0600a5    	orr	x5, x5, x6
10002b7b0: fa4038a0    	ccmp	x5, #0x0, #0x0, lo
10002b7b4: 9a9f1125    	csel	x5, x9, xzr, ne
10002b7b8: 9a9f1146    	csel	x6, x10, xzr, ne
10002b7bc: eb060063    	subs	x3, x3, x6
10002b7c0: da050084    	sbc	x4, x4, x5
10002b7c4: f100005f    	cmp	x2, #0x0
10002b7c8: 9a8403e2    	csel	x2, xzr, x4, eq
10002b7cc: 9a8303e3    	csel	x3, xzr, x3, eq
10002b7d0: a9000a23    	stp	x3, x2, [x17]
10002b7d4: 9a800311    	csel	x17, x24, x0, eq
10002b7d8: 9a810320    	csel	x0, x25, x1, eq
10002b7dc: 9b107c01    	mul	x1, x0, x16
10002b7e0: 9bd07c02    	umulh	x2, x0, x16
10002b7e4: 9bd07e23    	umulh	x3, x17, x16
10002b7e8: 9b107e30    	mul	x16, x17, x16
10002b7ec: 9b0f7c04    	mul	x4, x0, x15
10002b7f0: 9bcf7c00    	umulh	x0, x0, x15
10002b7f4: 9bcf7e25    	umulh	x5, x17, x15
10002b7f8: 9b0f7e2f    	mul	x15, x17, x15
10002b7fc: ab100050    	adds	x16, x2, x16
10002b800: 1a9f37f1    	cset	w17, hs
10002b804: ab030000    	adds	x0, x0, x3
10002b808: 1a9f37e2    	cset	w2, hs
10002b80c: ab0f000f    	adds	x15, x0, x15
10002b810: 9a823440    	cinc	x0, x2, hs
10002b814: ab040210    	adds	x16, x16, x4
10002b818: ba1101ef    	adcs	x15, x15, x17
10002b81c: 9a0000b1    	adc	x17, x5, x0
10002b820: 9b017dc0    	mul	x0, x14, x1
10002b824: 9b007d42    	mul	x2, x10, x0
10002b828: 9bc07d43    	umulh	x3, x10, x0
10002b82c: 9bc07d24    	umulh	x4, x9, x0
10002b830: 9b007d20    	mul	x0, x9, x0
10002b834: ab100070    	adds	x16, x3, x16
10002b838: 1a9f37e3    	cset	w3, hs
10002b83c: ab000210    	adds	x16, x16, x0
10002b840: 9a833460    	cinc	x0, x3, hs
10002b844: ab0401ef    	adds	x15, x15, x4
10002b848: 1a9f37e3    	cset	w3, hs
10002b84c: ab01005f    	cmn	x2, x1
10002b850: ba1f0210    	adcs	x16, x16, xzr
10002b854: ba0001ef    	adcs	x15, x15, x0
10002b858: ba030231    	adcs	x17, x17, x3
10002b85c: 1a9f37e0    	cset	w0, hs
10002b860: 9b107dc1    	mul	x1, x14, x16
10002b864: 9b017d42    	mul	x2, x10, x1
10002b868: 9bc17d43    	umulh	x3, x10, x1
10002b86c: 9bc17d24    	umulh	x4, x9, x1
10002b870: 9b017d21    	mul	x1, x9, x1
10002b874: ab0f006f    	adds	x15, x3, x15
10002b878: 1a9f37e3    	cset	w3, hs
10002b87c: ab0101ef    	adds	x15, x15, x1
10002b880: 9a833461    	cinc	x1, x3, hs
10002b884: ab040231    	adds	x17, x17, x4
10002b888: 1a9f37e3    	cset	w3, hs
10002b88c: ab10005f    	cmn	x2, x16
10002b890: ba1f01ef    	adcs	x15, x15, xzr
10002b894: ba010231    	adcs	x17, x17, x1
10002b898: 9a833470    	cinc	x16, x3, hs
10002b89c: eb0a01ff    	cmp	x15, x10
10002b8a0: fa09023f    	sbcs	xzr, x17, x9
10002b8a4: aa000210    	orr	x16, x16, x0
10002b8a8: fa403a00    	ccmp	x16, #0x0, #0x0, lo
10002b8ac: 9a9f1120    	csel	x0, x9, xzr, ne
10002b8b0: 9a9f1150    	csel	x16, x10, xzr, ne
10002b8b4: eb1001f0    	subs	x16, x15, x16
10002b8b8: da00022f    	sbc	x15, x17, x0
10002b8bc: f1004108    	subs	x8, x8, #0x10
10002b8c0: 54fff061    	b.ne	0x10002b6cc <__RINvNtNtNtCsaHyC9lX8wyC_17field_regressions8campaign10candidates5prime5batchKb1_EB8_+0x1d8>
10002b8c4: a9517bfd    	ldp	x29, x30, [sp, #0x110]
10002b8c8: a9504ff4    	ldp	x20, x19, [sp, #0x100]
10002b8cc: a94f57f6    	ldp	x22, x21, [sp, #0xf0]
10002b8d0: a94e5ff8    	ldp	x24, x23, [sp, #0xe0]
10002b8d4: a94d67fa    	ldp	x26, x25, [sp, #0xd0]
10002b8d8: 910483ff    	add	sp, sp, #0x120
10002b8dc: d65f03c0    	ret
10002b8e0: 90000b84    	adrp	x4, 0x10019b000 <dyld_stub_binder+0x10019b000>
10002b8e4: 912b6084    	add	x4, x4, #0xad8
10002b8e8: 910023e0    	add	x0, sp, #0x8
10002b8ec: 910043e1    	add	x1, sp, #0x10
10002b8f0: d2800002    	mov	x2, #0x0                ; =0
10002b8f4: 9404801d    	bl	0x10014b968 <__RINvNtCs8Mbv00yxnRz_4core9panicking13assert_failedjjEB4_>
10002b8f8: 90000b84    	adrp	x4, 0x10019b000 <dyld_stub_binder+0x10019b000>
10002b8fc: 912bc084    	add	x4, x4, #0xaf0
10002b900: 9101c3e0    	add	x0, sp, #0x70
10002b904: 910063e1    	add	x1, sp, #0x18
10002b908: d2800002    	mov	x2, #0x0                ; =0
10002b90c: 94048017    	bl	0x10014b968 <__RINvNtCs8Mbv00yxnRz_4core9panicking13assert_failedjjEB4_>
10002b910: f0000940    	adrp	x0, 0x100156000 <dyld_stub_binder+0x100156000>
10002b914: 91114c00    	add	x0, x0, #0x453
10002b918: 90000b82    	adrp	x2, 0x10019b000 <dyld_stub_binder+0x10019b000>
10002b91c: 912c8042    	add	x2, x2, #0xb20
10002b920: 52801321    	mov	w1, #0x99               ; =153
10002b924: 94047ff9    	bl	0x10014b908 <__RNvNtCs8Mbv00yxnRz_4core9panicking9panic_fmt>


/private/var/folders/q0/v7l8dzd13k7f_q6j80sy03lm0000gn/T/f2z-field-gated-6lkppdze/field-regressions:	file format mach-o arm64

Disassembly of section __TEXT,__text:

000000010002b56c <__RINvNtNtNtCs8IeiyMGxmWr_17field_regressions8campaign10candidates5prime5batchKb1_EB8_>:
10002b56c: d10483ff    	sub	sp, sp, #0x120
10002b570: a90d67fa    	stp	x26, x25, [sp, #0xd0]
10002b574: a90e5ff8    	stp	x24, x23, [sp, #0xe0]
10002b578: a90f57f6    	stp	x22, x21, [sp, #0xf0]
10002b57c: a9104ff4    	stp	x20, x19, [sp, #0x100]
10002b580: a9117bfd    	stp	x29, x30, [sp, #0x110]
10002b584: 910443fd    	add	x29, sp, #0x110
10002b588: a90093e2    	stp	x2, x4, [sp, #0x8]
10002b58c: eb04005f    	cmp	x2, x4
10002b590: 54001e41    	b.ne	0x10002b958 <__RINvNtNtNtCs8IeiyMGxmWr_17field_regressions8campaign10candidates5prime5batchKb1_EB8_+0x3ec>
10002b594: aa0203f7    	mov	x23, x2
10002b598: f9003be2    	str	x2, [sp, #0x70]
10002b59c: f9000fe6    	str	x6, [sp, #0x18]
10002b5a0: eb06005f    	cmp	x2, x6
10002b5a4: 54001e61    	b.ne	0x10002b970 <__RINvNtNtNtCs8IeiyMGxmWr_17field_regressions8campaign10candidates5prime5batchKb1_EB8_+0x404>
10002b5a8: aa0503f3    	mov	x19, x5
10002b5ac: aa0303f4    	mov	x20, x3
10002b5b0: aa0103f6    	mov	x22, x1
10002b5b4: aa0003f5    	mov	x21, x0
10002b5b8: a9456019    	ldp	x25, x24, [x0, #0x50]
10002b5bc: b4000977    	cbz	x23, 0x10002b6e8 <__RINvNtNtNtCs8IeiyMGxmWr_17field_regressions8campaign10candidates5prime5batchKb1_EB8_+0x17c>
10002b5c0: a94426aa    	ldp	x10, x9, [x21, #0x40]
10002b5c4: aa1403eb    	mov	x11, x20
10002b5c8: aa1603ec    	mov	x12, x22
10002b5cc: aa1703ed    	mov	x13, x23
10002b5d0: aa1903ef    	mov	x15, x25
10002b5d4: aa1803e8    	mov	x8, x24
10002b5d8: f9403aae    	ldr	x14, [x21, #0x70]
10002b5dc: a8c14191    	ldp	x17, x16, [x12], #0x10
10002b5e0: f900016f    	str	x15, [x11]
10002b5e4: aa100220    	orr	x0, x17, x16
10002b5e8: f100001f    	cmp	x0, #0x0
10002b5ec: 9a900310    	csel	x16, x24, x16, eq
10002b5f0: 9a910331    	csel	x17, x25, x17, eq
10002b5f4: 9b0f7e20    	mul	x0, x17, x15
10002b5f8: 9bcf7e21    	umulh	x1, x17, x15
10002b5fc: 9bcf7e02    	umulh	x2, x16, x15
10002b600: 9b0f7e0f    	mul	x15, x16, x15
10002b604: 9b087e23    	mul	x3, x17, x8
10002b608: 9bc87e31    	umulh	x17, x17, x8
10002b60c: 9bc87e04    	umulh	x4, x16, x8
10002b610: 9b087e10    	mul	x16, x16, x8
10002b614: ab0f002f    	adds	x15, x1, x15
10002b618: 1a9f37e1    	cset	w1, hs
10002b61c: ab020231    	adds	x17, x17, x2
10002b620: 1a9f37e2    	cset	w2, hs
10002b624: ab100230    	adds	x16, x17, x16
10002b628: 9a823451    	cinc	x17, x2, hs
10002b62c: ab0301ef    	adds	x15, x15, x3
10002b630: ba010210    	adcs	x16, x16, x1
10002b634: 9a110091    	adc	x17, x4, x17
10002b638: 9b007dc1    	mul	x1, x14, x0
10002b63c: 9b017d42    	mul	x2, x10, x1
10002b640: 9bc17d43    	umulh	x3, x10, x1
10002b644: 9bc17d24    	umulh	x4, x9, x1
10002b648: 9b017d21    	mul	x1, x9, x1
10002b64c: ab0f006f    	adds	x15, x3, x15
10002b650: 1a9f37e3    	cset	w3, hs
10002b654: ab0101ef    	adds	x15, x15, x1
10002b658: 9a833461    	cinc	x1, x3, hs
10002b65c: ab040210    	adds	x16, x16, x4
10002b660: 1a9f37e3    	cset	w3, hs
10002b664: ab00005f    	cmn	x2, x0
10002b668: ba1f01ef    	adcs	x15, x15, xzr
10002b66c: ba010210    	adcs	x16, x16, x1
10002b670: ba030231    	adcs	x17, x17, x3
10002b674: 1a9f37e0    	cset	w0, hs
10002b678: 9b0f7dc1    	mul	x1, x14, x15
10002b67c: 9b017d42    	mul	x2, x10, x1
10002b680: 9bc17d43    	umulh	x3, x10, x1
10002b684: 9bc17d24    	umulh	x4, x9, x1
10002b688: 9b017d21    	mul	x1, x9, x1
10002b68c: ab100070    	adds	x16, x3, x16
10002b690: 1a9f37e3    	cset	w3, hs
10002b694: ab010210    	adds	x16, x16, x1
10002b698: 9a833461    	cinc	x1, x3, hs
10002b69c: ab040231    	adds	x17, x17, x4
10002b6a0: 1a9f37e3    	cset	w3, hs
10002b6a4: ab0f005f    	cmn	x2, x15
10002b6a8: ba1f020f    	adcs	x15, x16, xzr
10002b6ac: ba010230    	adcs	x16, x17, x1
10002b6b0: 9a833471    	cinc	x17, x3, hs
10002b6b4: eb0a01ff    	cmp	x15, x10
10002b6b8: fa09021f    	sbcs	xzr, x16, x9
10002b6bc: aa000231    	orr	x17, x17, x0
10002b6c0: fa403a20    	ccmp	x17, #0x0, #0x0, lo
10002b6c4: 9a9f1131    	csel	x17, x9, xzr, ne
10002b6c8: 9a9f1140    	csel	x0, x10, xzr, ne
10002b6cc: eb0001ef    	subs	x15, x15, x0
10002b6d0: f9000568    	str	x8, [x11, #0x8]
10002b6d4: da110208    	sbc	x8, x16, x17
10002b6d8: 9100416b    	add	x11, x11, #0x10
10002b6dc: f10005ad    	subs	x13, x13, #0x1
10002b6e0: 54fff7e1    	b.ne	0x10002b5dc <__RINvNtNtNtCs8IeiyMGxmWr_17field_regressions8campaign10candidates5prime5batchKb1_EB8_+0x70>
10002b6e4: 14000003    	b	0x10002b6f0 <__RINvNtNtNtCs8IeiyMGxmWr_17field_regressions8campaign10candidates5prime5batchKb1_EB8_+0x184>
10002b6e8: aa1903ef    	mov	x15, x25
10002b6ec: aa1803e8    	mov	x8, x24
10002b6f0: a90623ef    	stp	x15, x8, [sp, #0x60]
10002b6f4: ad4006a0    	ldp	q0, q1, [x21]
10002b6f8: ad0107e0    	stp	q0, q1, [sp, #0x20]
10002b6fc: ad4106a0    	ldp	q0, q1, [x21, #0x20]
10002b700: ad0207e0    	stp	q0, q1, [sp, #0x40]
10002b704: 9101c3e0    	add	x0, sp, #0x70
10002b708: 910083e1    	add	x1, sp, #0x20
10002b70c: 9400b1e0    	bl	0x100057e8c <__RNvMNtNtNtCs4rJR5Xg1K3s_13crypto_bigint7modular16fixed_monty_form6invertINtB4_14FixedMontyFormKj2_E6invertCs8IeiyMGxmWr_17field_regressions>
10002b710: 394303e8    	ldrb	w8, [sp, #0xc0]
10002b714: 381bf3a8    	sturb	w8, [x29, #-0x41]
10002b718: d10107a8    	sub	x8, x29, #0x41
10002b71c: 385bf3a8    	ldurb	w8, [x29, #-0x41]
10002b720: 34001348    	cbz	w8, 0x10002b988 <__RINvNtNtNtCs8IeiyMGxmWr_17field_regressions8campaign10candidates5prime5batchKb1_EB8_+0x41c>
10002b724: b40010d7    	cbz	x23, 0x10002b93c <__RINvNtNtNtCs8IeiyMGxmWr_17field_regressions8campaign10candidates5prime5batchKb1_EB8_+0x3d0>
10002b728: a94b3ff0    	ldp	x16, x15, [sp, #0xb0]
10002b72c: d37ceee8    	lsl	x8, x23, #4
10002b730: a94426aa    	ldp	x10, x9, [x21, #0x40]
10002b734: d10042cb    	sub	x11, x22, #0x10
10002b738: d100428c    	sub	x12, x20, #0x10
10002b73c: d100426d    	sub	x13, x19, #0x10
10002b740: f9403aae    	ldr	x14, [x21, #0x70]
10002b744: 8b080161    	add	x1, x11, x8
10002b748: 8b080182    	add	x2, x12, x8
10002b74c: 8b0801b1    	add	x17, x13, x8
10002b750: a9400021    	ldp	x1, x0, [x1]
10002b754: a9400c44    	ldp	x4, x3, [x2]
10002b758: aa000022    	orr	x2, x1, x0
10002b75c: 9b107c85    	mul	x5, x4, x16
10002b760: 9bd07c86    	umulh	x6, x4, x16
10002b764: 9bd07c67    	umulh	x7, x3, x16
10002b768: 9b107c73    	mul	x19, x3, x16
10002b76c: 9b0f7c94    	mul	x20, x4, x15
10002b770: 9bcf7c84    	umulh	x4, x4, x15
10002b774: 9bcf7c75    	umulh	x21, x3, x15
10002b778: 9b0f7c63    	mul	x3, x3, x15
10002b77c: ab1300c6    	adds	x6, x6, x19
10002b780: 1a9f37f3    	cset	w19, hs
10002b784: ab070084    	adds	x4, x4, x7
10002b788: 1a9f37e7    	cset	w7, hs
10002b78c: ab030083    	adds	x3, x4, x3
10002b790: 9a8734e4    	cinc	x4, x7, hs
10002b794: ab1400c6    	adds	x6, x6, x20
10002b798: ba130063    	adcs	x3, x3, x19
10002b79c: 9a0402a4    	adc	x4, x21, x4
10002b7a0: 9b057dc7    	mul	x7, x14, x5
10002b7a4: 9b077d53    	mul	x19, x10, x7
10002b7a8: 9bc77d54    	umulh	x20, x10, x7
10002b7ac: 9bc77d35    	umulh	x21, x9, x7
10002b7b0: 9b077d27    	mul	x7, x9, x7
10002b7b4: ab060286    	adds	x6, x20, x6
10002b7b8: 1a9f37f4    	cset	w20, hs
10002b7bc: ab0700c6    	adds	x6, x6, x7
10002b7c0: 9a943687    	cinc	x7, x20, hs
10002b7c4: ab150063    	adds	x3, x3, x21
10002b7c8: 1a9f37f4    	cset	w20, hs
10002b7cc: ab05027f    	cmn	x19, x5
10002b7d0: ba1f00c5    	adcs	x5, x6, xzr
10002b7d4: ba070063    	adcs	x3, x3, x7
10002b7d8: ba140084    	adcs	x4, x4, x20
10002b7dc: 1a9f37e6    	cset	w6, hs
10002b7e0: 9b057dc7    	mul	x7, x14, x5
10002b7e4: 9b077d53    	mul	x19, x10, x7
10002b7e8: 9bc77d54    	umulh	x20, x10, x7
10002b7ec: 9bc77d35    	umulh	x21, x9, x7
10002b7f0: 9b077d27    	mul	x7, x9, x7
10002b7f4: ab030283    	adds	x3, x20, x3
10002b7f8: 1a9f37f4    	cset	w20, hs
10002b7fc: ab070063    	adds	x3, x3, x7
10002b800: 9a943687    	cinc	x7, x20, hs
10002b804: ab150084    	adds	x4, x4, x21
10002b808: 1a9f37f4    	cset	w20, hs
10002b80c: ab05027f    	cmn	x19, x5
10002b810: ba1f0063    	adcs	x3, x3, xzr
10002b814: ba070084    	adcs	x4, x4, x7
10002b818: 9a943685    	cinc	x5, x20, hs
10002b81c: eb0a007f    	cmp	x3, x10
10002b820: fa09009f    	sbcs	xzr, x4, x9
10002b824: aa0600a5    	orr	x5, x5, x6
10002b828: fa4038a0    	ccmp	x5, #0x0, #0x0, lo
10002b82c: 9a9f1125    	csel	x5, x9, xzr, ne
10002b830: 9a9f1146    	csel	x6, x10, xzr, ne
10002b834: eb060063    	subs	x3, x3, x6
10002b838: da050084    	sbc	x4, x4, x5
10002b83c: f100005f    	cmp	x2, #0x0
10002b840: 9a8403e2    	csel	x2, xzr, x4, eq
10002b844: 9a8303e3    	csel	x3, xzr, x3, eq
10002b848: a9000a23    	stp	x3, x2, [x17]
10002b84c: 9a800311    	csel	x17, x24, x0, eq
10002b850: 9a810320    	csel	x0, x25, x1, eq
10002b854: 9b107c01    	mul	x1, x0, x16
10002b858: 9bd07c02    	umulh	x2, x0, x16
10002b85c: 9bd07e23    	umulh	x3, x17, x16
10002b860: 9b107e30    	mul	x16, x17, x16
10002b864: 9b0f7c04    	mul	x4, x0, x15
10002b868: 9bcf7c00    	umulh	x0, x0, x15
10002b86c: 9bcf7e25    	umulh	x5, x17, x15
10002b870: 9b0f7e2f    	mul	x15, x17, x15
10002b874: ab100050    	adds	x16, x2, x16
10002b878: 1a9f37f1    	cset	w17, hs
10002b87c: ab030000    	adds	x0, x0, x3
10002b880: 1a9f37e2    	cset	w2, hs
10002b884: ab0f000f    	adds	x15, x0, x15
10002b888: 9a823440    	cinc	x0, x2, hs
10002b88c: ab040210    	adds	x16, x16, x4
10002b890: ba1101ef    	adcs	x15, x15, x17
10002b894: 9a0000b1    	adc	x17, x5, x0
10002b898: 9b017dc0    	mul	x0, x14, x1
10002b89c: 9b007d42    	mul	x2, x10, x0
10002b8a0: 9bc07d43    	umulh	x3, x10, x0
10002b8a4: 9bc07d24    	umulh	x4, x9, x0
10002b8a8: 9b007d20    	mul	x0, x9, x0
10002b8ac: ab100070    	adds	x16, x3, x16
10002b8b0: 1a9f37e3    	cset	w3, hs
10002b8b4: ab000210    	adds	x16, x16, x0
10002b8b8: 9a833460    	cinc	x0, x3, hs
10002b8bc: ab0401ef    	adds	x15, x15, x4
10002b8c0: 1a9f37e3    	cset	w3, hs
10002b8c4: ab01005f    	cmn	x2, x1
10002b8c8: ba1f0210    	adcs	x16, x16, xzr
10002b8cc: ba0001ef    	adcs	x15, x15, x0
10002b8d0: ba030231    	adcs	x17, x17, x3
10002b8d4: 1a9f37e0    	cset	w0, hs
10002b8d8: 9b107dc1    	mul	x1, x14, x16
10002b8dc: 9b017d42    	mul	x2, x10, x1
10002b8e0: 9bc17d43    	umulh	x3, x10, x1
10002b8e4: 9bc17d24    	umulh	x4, x9, x1
10002b8e8: 9b017d21    	mul	x1, x9, x1
10002b8ec: ab0f006f    	adds	x15, x3, x15
10002b8f0: 1a9f37e3    	cset	w3, hs
10002b8f4: ab0101ef    	adds	x15, x15, x1
10002b8f8: 9a833461    	cinc	x1, x3, hs
10002b8fc: ab040231    	adds	x17, x17, x4
10002b900: 1a9f37e3    	cset	w3, hs
10002b904: ab10005f    	cmn	x2, x16
10002b908: ba1f01ef    	adcs	x15, x15, xzr
10002b90c: ba010231    	adcs	x17, x17, x1
10002b910: 9a833470    	cinc	x16, x3, hs
10002b914: eb0a01ff    	cmp	x15, x10
10002b918: fa09023f    	sbcs	xzr, x17, x9
10002b91c: aa000210    	orr	x16, x16, x0
10002b920: fa403a00    	ccmp	x16, #0x0, #0x0, lo
10002b924: 9a9f1120    	csel	x0, x9, xzr, ne
10002b928: 9a9f1150    	csel	x16, x10, xzr, ne
10002b92c: eb1001f0    	subs	x16, x15, x16
10002b930: da00022f    	sbc	x15, x17, x0
10002b934: f1004108    	subs	x8, x8, #0x10
10002b938: 54fff061    	b.ne	0x10002b744 <__RINvNtNtNtCs8IeiyMGxmWr_17field_regressions8campaign10candidates5prime5batchKb1_EB8_+0x1d8>
10002b93c: a9517bfd    	ldp	x29, x30, [sp, #0x110]
10002b940: a9504ff4    	ldp	x20, x19, [sp, #0x100]
10002b944: a94f57f6    	ldp	x22, x21, [sp, #0xf0]
10002b948: a94e5ff8    	ldp	x24, x23, [sp, #0xe0]
10002b94c: a94d67fa    	ldp	x26, x25, [sp, #0xd0]
10002b950: 910483ff    	add	sp, sp, #0x120
10002b954: d65f03c0    	ret
10002b958: 90000a64    	adrp	x4, 0x100177000 <dyld_stub_binder+0x100177000>
10002b95c: 91112084    	add	x4, x4, #0x448
10002b960: 910023e0    	add	x0, sp, #0x8
10002b964: 910043e1    	add	x1, sp, #0x10
10002b968: d2800002    	mov	x2, #0x0                ; =0
10002b96c: 94040380    	bl	0x10012c76c <__RINvNtCs8Mbv00yxnRz_4core9panicking13assert_failedjjEB4_>
10002b970: 90000a64    	adrp	x4, 0x100177000 <dyld_stub_binder+0x100177000>
10002b974: 91118084    	add	x4, x4, #0x460
10002b978: 9101c3e0    	add	x0, sp, #0x70
10002b97c: 910063e1    	add	x1, sp, #0x18
10002b980: d2800002    	mov	x2, #0x0                ; =0
10002b984: 9404037a    	bl	0x10012c76c <__RINvNtCs8Mbv00yxnRz_4core9panicking13assert_failedjjEB4_>
10002b988: f0000840    	adrp	x0, 0x100136000 <dyld_stub_binder+0x100136000>
10002b98c: 91364c00    	add	x0, x0, #0xd93
10002b990: 90000a62    	adrp	x2, 0x100177000 <dyld_stub_binder+0x100177000>
10002b994: 91124042    	add	x2, x2, #0x490
10002b998: 52801321    	mov	w1, #0x99               ; =153
10002b99c: 9404035c    	bl	0x10012c70c <__RNvNtCs8Mbv00yxnRz_4core9panicking9panic_fmt>

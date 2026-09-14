
/private/var/folders/q0/v7l8dzd13k7f_q6j80sy03lm0000gn/T/f2z-field-gated-h_tggvge/field-regressions:	file format mach-o arm64

Disassembly of section __TEXT,__text:

000000010002c9a4 <__RINvNtNtNtCsfDw8YqMpZPC_17field_regressions8campaign10candidates5prime5batchKb1_EB8_>:
10002c9a4: d10483ff    	sub	sp, sp, #0x120
10002c9a8: a90d67fa    	stp	x26, x25, [sp, #0xd0]
10002c9ac: a90e5ff8    	stp	x24, x23, [sp, #0xe0]
10002c9b0: a90f57f6    	stp	x22, x21, [sp, #0xf0]
10002c9b4: a9104ff4    	stp	x20, x19, [sp, #0x100]
10002c9b8: a9117bfd    	stp	x29, x30, [sp, #0x110]
10002c9bc: 910443fd    	add	x29, sp, #0x110
10002c9c0: a90093e2    	stp	x2, x4, [sp, #0x8]
10002c9c4: eb04005f    	cmp	x2, x4
10002c9c8: 54001e41    	b.ne	0x10002cd90 <__RINvNtNtNtCsfDw8YqMpZPC_17field_regressions8campaign10candidates5prime5batchKb1_EB8_+0x3ec>
10002c9cc: aa0203f7    	mov	x23, x2
10002c9d0: f9003be2    	str	x2, [sp, #0x70]
10002c9d4: f9000fe6    	str	x6, [sp, #0x18]
10002c9d8: eb06005f    	cmp	x2, x6
10002c9dc: 54001e61    	b.ne	0x10002cda8 <__RINvNtNtNtCsfDw8YqMpZPC_17field_regressions8campaign10candidates5prime5batchKb1_EB8_+0x404>
10002c9e0: aa0503f3    	mov	x19, x5
10002c9e4: aa0303f4    	mov	x20, x3
10002c9e8: aa0103f6    	mov	x22, x1
10002c9ec: aa0003f5    	mov	x21, x0
10002c9f0: a9456019    	ldp	x25, x24, [x0, #0x50]
10002c9f4: b4000977    	cbz	x23, 0x10002cb20 <__RINvNtNtNtCsfDw8YqMpZPC_17field_regressions8campaign10candidates5prime5batchKb1_EB8_+0x17c>
10002c9f8: a94426aa    	ldp	x10, x9, [x21, #0x40]
10002c9fc: aa1403eb    	mov	x11, x20
10002ca00: aa1603ec    	mov	x12, x22
10002ca04: aa1703ed    	mov	x13, x23
10002ca08: aa1903ef    	mov	x15, x25
10002ca0c: aa1803e8    	mov	x8, x24
10002ca10: f9403aae    	ldr	x14, [x21, #0x70]
10002ca14: a8c14191    	ldp	x17, x16, [x12], #0x10
10002ca18: f900016f    	str	x15, [x11]
10002ca1c: aa100220    	orr	x0, x17, x16
10002ca20: f100001f    	cmp	x0, #0x0
10002ca24: 9a900310    	csel	x16, x24, x16, eq
10002ca28: 9a910331    	csel	x17, x25, x17, eq
10002ca2c: 9b0f7e20    	mul	x0, x17, x15
10002ca30: 9bcf7e21    	umulh	x1, x17, x15
10002ca34: 9bcf7e02    	umulh	x2, x16, x15
10002ca38: 9b0f7e0f    	mul	x15, x16, x15
10002ca3c: 9b087e23    	mul	x3, x17, x8
10002ca40: 9bc87e31    	umulh	x17, x17, x8
10002ca44: 9bc87e04    	umulh	x4, x16, x8
10002ca48: 9b087e10    	mul	x16, x16, x8
10002ca4c: ab0f002f    	adds	x15, x1, x15
10002ca50: 1a9f37e1    	cset	w1, hs
10002ca54: ab020231    	adds	x17, x17, x2
10002ca58: 1a9f37e2    	cset	w2, hs
10002ca5c: ab100230    	adds	x16, x17, x16
10002ca60: 9a823451    	cinc	x17, x2, hs
10002ca64: ab0301ef    	adds	x15, x15, x3
10002ca68: ba010210    	adcs	x16, x16, x1
10002ca6c: 9a110091    	adc	x17, x4, x17
10002ca70: 9b007dc1    	mul	x1, x14, x0
10002ca74: 9b017d42    	mul	x2, x10, x1
10002ca78: 9bc17d43    	umulh	x3, x10, x1
10002ca7c: 9bc17d24    	umulh	x4, x9, x1
10002ca80: 9b017d21    	mul	x1, x9, x1
10002ca84: ab0f006f    	adds	x15, x3, x15
10002ca88: 1a9f37e3    	cset	w3, hs
10002ca8c: ab0101ef    	adds	x15, x15, x1
10002ca90: 9a833461    	cinc	x1, x3, hs
10002ca94: ab040210    	adds	x16, x16, x4
10002ca98: 1a9f37e3    	cset	w3, hs
10002ca9c: ab00005f    	cmn	x2, x0
10002caa0: ba1f01ef    	adcs	x15, x15, xzr
10002caa4: ba010210    	adcs	x16, x16, x1
10002caa8: ba030231    	adcs	x17, x17, x3
10002caac: 1a9f37e0    	cset	w0, hs
10002cab0: 9b0f7dc1    	mul	x1, x14, x15
10002cab4: 9b017d42    	mul	x2, x10, x1
10002cab8: 9bc17d43    	umulh	x3, x10, x1
10002cabc: 9bc17d24    	umulh	x4, x9, x1
10002cac0: 9b017d21    	mul	x1, x9, x1
10002cac4: ab100070    	adds	x16, x3, x16
10002cac8: 1a9f37e3    	cset	w3, hs
10002cacc: ab010210    	adds	x16, x16, x1
10002cad0: 9a833461    	cinc	x1, x3, hs
10002cad4: ab040231    	adds	x17, x17, x4
10002cad8: 1a9f37e3    	cset	w3, hs
10002cadc: ab0f005f    	cmn	x2, x15
10002cae0: ba1f020f    	adcs	x15, x16, xzr
10002cae4: ba010230    	adcs	x16, x17, x1
10002cae8: 9a833471    	cinc	x17, x3, hs
10002caec: eb0a01ff    	cmp	x15, x10
10002caf0: fa09021f    	sbcs	xzr, x16, x9
10002caf4: aa000231    	orr	x17, x17, x0
10002caf8: fa403a20    	ccmp	x17, #0x0, #0x0, lo
10002cafc: 9a9f1131    	csel	x17, x9, xzr, ne
10002cb00: 9a9f1140    	csel	x0, x10, xzr, ne
10002cb04: eb0001ef    	subs	x15, x15, x0
10002cb08: f9000568    	str	x8, [x11, #0x8]
10002cb0c: da110208    	sbc	x8, x16, x17
10002cb10: 9100416b    	add	x11, x11, #0x10
10002cb14: f10005ad    	subs	x13, x13, #0x1
10002cb18: 54fff7e1    	b.ne	0x10002ca14 <__RINvNtNtNtCsfDw8YqMpZPC_17field_regressions8campaign10candidates5prime5batchKb1_EB8_+0x70>
10002cb1c: 14000003    	b	0x10002cb28 <__RINvNtNtNtCsfDw8YqMpZPC_17field_regressions8campaign10candidates5prime5batchKb1_EB8_+0x184>
10002cb20: aa1903ef    	mov	x15, x25
10002cb24: aa1803e8    	mov	x8, x24
10002cb28: a90623ef    	stp	x15, x8, [sp, #0x60]
10002cb2c: ad4006a0    	ldp	q0, q1, [x21]
10002cb30: ad0107e0    	stp	q0, q1, [sp, #0x20]
10002cb34: ad4106a0    	ldp	q0, q1, [x21, #0x20]
10002cb38: ad0207e0    	stp	q0, q1, [sp, #0x40]
10002cb3c: 9101c3e0    	add	x0, sp, #0x70
10002cb40: 910083e1    	add	x1, sp, #0x20
10002cb44: 94013e01    	bl	0x10007c348 <__RNvMNtNtNtCs4rJR5Xg1K3s_13crypto_bigint7modular16fixed_monty_form6invertINtB4_14FixedMontyFormKj2_E6invertCsfDw8YqMpZPC_17field_regressions>
10002cb48: 394303e8    	ldrb	w8, [sp, #0xc0]
10002cb4c: 381bf3a8    	sturb	w8, [x29, #-0x41]
10002cb50: d10107a8    	sub	x8, x29, #0x41
10002cb54: 385bf3a8    	ldurb	w8, [x29, #-0x41]
10002cb58: 34001348    	cbz	w8, 0x10002cdc0 <__RINvNtNtNtCsfDw8YqMpZPC_17field_regressions8campaign10candidates5prime5batchKb1_EB8_+0x41c>
10002cb5c: b40010d7    	cbz	x23, 0x10002cd74 <__RINvNtNtNtCsfDw8YqMpZPC_17field_regressions8campaign10candidates5prime5batchKb1_EB8_+0x3d0>
10002cb60: a94b3ff0    	ldp	x16, x15, [sp, #0xb0]
10002cb64: d37ceee8    	lsl	x8, x23, #4
10002cb68: a94426aa    	ldp	x10, x9, [x21, #0x40]
10002cb6c: d10042cb    	sub	x11, x22, #0x10
10002cb70: d100428c    	sub	x12, x20, #0x10
10002cb74: d100426d    	sub	x13, x19, #0x10
10002cb78: f9403aae    	ldr	x14, [x21, #0x70]
10002cb7c: 8b080161    	add	x1, x11, x8
10002cb80: 8b080182    	add	x2, x12, x8
10002cb84: 8b0801b1    	add	x17, x13, x8
10002cb88: a9400021    	ldp	x1, x0, [x1]
10002cb8c: a9400c44    	ldp	x4, x3, [x2]
10002cb90: aa000022    	orr	x2, x1, x0
10002cb94: 9b107c85    	mul	x5, x4, x16
10002cb98: 9bd07c86    	umulh	x6, x4, x16
10002cb9c: 9bd07c67    	umulh	x7, x3, x16
10002cba0: 9b107c73    	mul	x19, x3, x16
10002cba4: 9b0f7c94    	mul	x20, x4, x15
10002cba8: 9bcf7c84    	umulh	x4, x4, x15
10002cbac: 9bcf7c75    	umulh	x21, x3, x15
10002cbb0: 9b0f7c63    	mul	x3, x3, x15
10002cbb4: ab1300c6    	adds	x6, x6, x19
10002cbb8: 1a9f37f3    	cset	w19, hs
10002cbbc: ab070084    	adds	x4, x4, x7
10002cbc0: 1a9f37e7    	cset	w7, hs
10002cbc4: ab030083    	adds	x3, x4, x3
10002cbc8: 9a8734e4    	cinc	x4, x7, hs
10002cbcc: ab1400c6    	adds	x6, x6, x20
10002cbd0: ba130063    	adcs	x3, x3, x19
10002cbd4: 9a0402a4    	adc	x4, x21, x4
10002cbd8: 9b057dc7    	mul	x7, x14, x5
10002cbdc: 9b077d53    	mul	x19, x10, x7
10002cbe0: 9bc77d54    	umulh	x20, x10, x7
10002cbe4: 9bc77d35    	umulh	x21, x9, x7
10002cbe8: 9b077d27    	mul	x7, x9, x7
10002cbec: ab060286    	adds	x6, x20, x6
10002cbf0: 1a9f37f4    	cset	w20, hs
10002cbf4: ab0700c6    	adds	x6, x6, x7
10002cbf8: 9a943687    	cinc	x7, x20, hs
10002cbfc: ab150063    	adds	x3, x3, x21
10002cc00: 1a9f37f4    	cset	w20, hs
10002cc04: ab05027f    	cmn	x19, x5
10002cc08: ba1f00c5    	adcs	x5, x6, xzr
10002cc0c: ba070063    	adcs	x3, x3, x7
10002cc10: ba140084    	adcs	x4, x4, x20
10002cc14: 1a9f37e6    	cset	w6, hs
10002cc18: 9b057dc7    	mul	x7, x14, x5
10002cc1c: 9b077d53    	mul	x19, x10, x7
10002cc20: 9bc77d54    	umulh	x20, x10, x7
10002cc24: 9bc77d35    	umulh	x21, x9, x7
10002cc28: 9b077d27    	mul	x7, x9, x7
10002cc2c: ab030283    	adds	x3, x20, x3
10002cc30: 1a9f37f4    	cset	w20, hs
10002cc34: ab070063    	adds	x3, x3, x7
10002cc38: 9a943687    	cinc	x7, x20, hs
10002cc3c: ab150084    	adds	x4, x4, x21
10002cc40: 1a9f37f4    	cset	w20, hs
10002cc44: ab05027f    	cmn	x19, x5
10002cc48: ba1f0063    	adcs	x3, x3, xzr
10002cc4c: ba070084    	adcs	x4, x4, x7
10002cc50: 9a943685    	cinc	x5, x20, hs
10002cc54: eb0a007f    	cmp	x3, x10
10002cc58: fa09009f    	sbcs	xzr, x4, x9
10002cc5c: aa0600a5    	orr	x5, x5, x6
10002cc60: fa4038a0    	ccmp	x5, #0x0, #0x0, lo
10002cc64: 9a9f1125    	csel	x5, x9, xzr, ne
10002cc68: 9a9f1146    	csel	x6, x10, xzr, ne
10002cc6c: eb060063    	subs	x3, x3, x6
10002cc70: da050084    	sbc	x4, x4, x5
10002cc74: f100005f    	cmp	x2, #0x0
10002cc78: 9a8403e2    	csel	x2, xzr, x4, eq
10002cc7c: 9a8303e3    	csel	x3, xzr, x3, eq
10002cc80: a9000a23    	stp	x3, x2, [x17]
10002cc84: 9a800311    	csel	x17, x24, x0, eq
10002cc88: 9a810320    	csel	x0, x25, x1, eq
10002cc8c: 9b107c01    	mul	x1, x0, x16
10002cc90: 9bd07c02    	umulh	x2, x0, x16
10002cc94: 9bd07e23    	umulh	x3, x17, x16
10002cc98: 9b107e30    	mul	x16, x17, x16
10002cc9c: 9b0f7c04    	mul	x4, x0, x15
10002cca0: 9bcf7c00    	umulh	x0, x0, x15
10002cca4: 9bcf7e25    	umulh	x5, x17, x15
10002cca8: 9b0f7e2f    	mul	x15, x17, x15
10002ccac: ab100050    	adds	x16, x2, x16
10002ccb0: 1a9f37f1    	cset	w17, hs
10002ccb4: ab030000    	adds	x0, x0, x3
10002ccb8: 1a9f37e2    	cset	w2, hs
10002ccbc: ab0f000f    	adds	x15, x0, x15
10002ccc0: 9a823440    	cinc	x0, x2, hs
10002ccc4: ab040210    	adds	x16, x16, x4
10002ccc8: ba1101ef    	adcs	x15, x15, x17
10002cccc: 9a0000b1    	adc	x17, x5, x0
10002ccd0: 9b017dc0    	mul	x0, x14, x1
10002ccd4: 9b007d42    	mul	x2, x10, x0
10002ccd8: 9bc07d43    	umulh	x3, x10, x0
10002ccdc: 9bc07d24    	umulh	x4, x9, x0
10002cce0: 9b007d20    	mul	x0, x9, x0
10002cce4: ab100070    	adds	x16, x3, x16
10002cce8: 1a9f37e3    	cset	w3, hs
10002ccec: ab000210    	adds	x16, x16, x0
10002ccf0: 9a833460    	cinc	x0, x3, hs
10002ccf4: ab0401ef    	adds	x15, x15, x4
10002ccf8: 1a9f37e3    	cset	w3, hs
10002ccfc: ab01005f    	cmn	x2, x1
10002cd00: ba1f0210    	adcs	x16, x16, xzr
10002cd04: ba0001ef    	adcs	x15, x15, x0
10002cd08: ba030231    	adcs	x17, x17, x3
10002cd0c: 1a9f37e0    	cset	w0, hs
10002cd10: 9b107dc1    	mul	x1, x14, x16
10002cd14: 9b017d42    	mul	x2, x10, x1
10002cd18: 9bc17d43    	umulh	x3, x10, x1
10002cd1c: 9bc17d24    	umulh	x4, x9, x1
10002cd20: 9b017d21    	mul	x1, x9, x1
10002cd24: ab0f006f    	adds	x15, x3, x15
10002cd28: 1a9f37e3    	cset	w3, hs
10002cd2c: ab0101ef    	adds	x15, x15, x1
10002cd30: 9a833461    	cinc	x1, x3, hs
10002cd34: ab040231    	adds	x17, x17, x4
10002cd38: 1a9f37e3    	cset	w3, hs
10002cd3c: ab10005f    	cmn	x2, x16
10002cd40: ba1f01ef    	adcs	x15, x15, xzr
10002cd44: ba010231    	adcs	x17, x17, x1
10002cd48: 9a833470    	cinc	x16, x3, hs
10002cd4c: eb0a01ff    	cmp	x15, x10
10002cd50: fa09023f    	sbcs	xzr, x17, x9
10002cd54: aa000210    	orr	x16, x16, x0
10002cd58: fa403a00    	ccmp	x16, #0x0, #0x0, lo
10002cd5c: 9a9f1120    	csel	x0, x9, xzr, ne
10002cd60: 9a9f1150    	csel	x16, x10, xzr, ne
10002cd64: eb1001f0    	subs	x16, x15, x16
10002cd68: da00022f    	sbc	x15, x17, x0
10002cd6c: f1004108    	subs	x8, x8, #0x10
10002cd70: 54fff061    	b.ne	0x10002cb7c <__RINvNtNtNtCsfDw8YqMpZPC_17field_regressions8campaign10candidates5prime5batchKb1_EB8_+0x1d8>
10002cd74: a9517bfd    	ldp	x29, x30, [sp, #0x110]
10002cd78: a9504ff4    	ldp	x20, x19, [sp, #0x100]
10002cd7c: a94f57f6    	ldp	x22, x21, [sp, #0xf0]
10002cd80: a94e5ff8    	ldp	x24, x23, [sp, #0xe0]
10002cd84: a94d67fa    	ldp	x26, x25, [sp, #0xd0]
10002cd88: 910483ff    	add	sp, sp, #0x120
10002cd8c: d65f03c0    	ret
10002cd90: f0000c04    	adrp	x4, 0x1001af000 <dyld_stub_binder+0x1001af000>
10002cd94: 9132c084    	add	x4, x4, #0xcb0
10002cd98: 910023e0    	add	x0, sp, #0x8
10002cd9c: 910043e1    	add	x1, sp, #0x10
10002cda0: d2800002    	mov	x2, #0x0                ; =0
10002cda4: 9404bec8    	bl	0x10015c8c4 <__RINvNtCs8Mbv00yxnRz_4core9panicking13assert_failedjjEB4_>
10002cda8: f0000c04    	adrp	x4, 0x1001af000 <dyld_stub_binder+0x1001af000>
10002cdac: 91332084    	add	x4, x4, #0xcc8
10002cdb0: 9101c3e0    	add	x0, sp, #0x70
10002cdb4: 910063e1    	add	x1, sp, #0x18
10002cdb8: d2800002    	mov	x2, #0x0                ; =0
10002cdbc: 9404bec2    	bl	0x10015c8c4 <__RINvNtCs8Mbv00yxnRz_4core9panicking13assert_failedjjEB4_>
10002cdc0: d00009c0    	adrp	x0, 0x100166000 <dyld_stub_binder+0x100166000>
10002cdc4: 913ecc00    	add	x0, x0, #0xfb3
10002cdc8: f0000c02    	adrp	x2, 0x1001af000 <dyld_stub_binder+0x1001af000>
10002cdcc: 9133e042    	add	x2, x2, #0xcf8
10002cdd0: 52801321    	mov	w1, #0x99               ; =153
10002cdd4: 9404bea4    	bl	0x10015c864 <__RNvNtCs8Mbv00yxnRz_4core9panicking9panic_fmt>

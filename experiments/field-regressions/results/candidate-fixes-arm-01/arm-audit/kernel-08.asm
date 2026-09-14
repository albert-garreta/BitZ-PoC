
/private/var/folders/q0/v7l8dzd13k7f_q6j80sy03lm0000gn/T/f2z-field-gated-pycl0nhx/field-regressions:	file format mach-o arm64

Disassembly of section __TEXT,__text:

0000000100043734 <__RINvNtNtNtCsagpXUhzPPpl_17field_regressions8campaign10candidates7integer7native4Kj4_Kj2_EB8_>:
100043734: d101c3ff    	sub	sp, sp, #0x70
100043738: a9054ff4    	stp	x20, x19, [sp, #0x50]
10004373c: a9067bfd    	stp	x29, x30, [sp, #0x60]
100043740: 910183fd    	add	x29, sp, #0x60
100043744: a90093e2    	stp	x2, x4, [sp, #0x8]
100043748: eb04005f    	cmp	x2, x4
10004374c: 540009e1    	b.ne	0x100043888 <__RINvNtNtNtCsagpXUhzPPpl_17field_regressions8campaign10candidates7integer7native4Kj4_Kj2_EB8_+0x154>
100043750: 6f00e400    	movi.2d	v0, #0000000000000000
100043754: ad0183e0    	stp	q0, q0, [sp, #0x30]
100043758: ad0083e0    	stp	q0, q0, [sp, #0x10]
10004375c: d280000b    	mov	x11, #0x0               ; =0
100043760: b4000762    	cbz	x2, 0x10004384c <__RINvNtNtNtCsagpXUhzPPpl_17field_regressions8campaign10candidates7integer7native4Kj4_Kj2_EB8_+0x118>
100043764: 91004028    	add	x8, x1, #0x10
100043768: 91004069    	add	x9, x3, #0x10
10004376c: 910043ea    	add	x10, sp, #0x10
100043770: 9240016c    	and	x12, x11, #0x1
100043774: 9100056b    	add	x11, x11, #0x1
100043778: a97f392d    	ldp	x13, x14, [x9, #-0x10]
10004377c: a97f410f    	ldp	x15, x16, [x8, #-0x10]
100043780: a8c20511    	ldp	x17, x1, [x8], #0x20
100043784: 9bce7e03    	umulh	x3, x16, x14
100043788: 9b0e7e04    	mul	x4, x16, x14
10004378c: 9b0f7dc5    	mul	x5, x14, x15
100043790: 9bcf7dc6    	umulh	x6, x14, x15
100043794: ab060084    	adds	x4, x4, x6
100043798: 9a833463    	cinc	x3, x3, hs
10004379c: 9b0d7e06    	mul	x6, x16, x13
1000437a0: 9bcd7e07    	umulh	x7, x16, x13
1000437a4: ab070084    	adds	x4, x4, x7
1000437a8: 9a833463    	cinc	x3, x3, hs
1000437ac: 9b0f7da7    	mul	x7, x13, x15
1000437b0: 9bcf7db3    	umulh	x19, x13, x15
1000437b4: ab1300a5    	adds	x5, x5, x19
1000437b8: 1a9f37f3    	cset	w19, hs
1000437bc: ab0600a5    	adds	x5, x5, x6
1000437c0: ba130084    	adcs	x4, x4, x19
1000437c4: 9a833463    	cinc	x3, x3, hs
1000437c8: a8c24d26    	ldp	x6, x19, [x9], #0x20
1000437cc: 9bcf7cd4    	umulh	x20, x6, x15
1000437d0: 9b1050d0    	madd	x16, x6, x16, x20
1000437d4: 9b0f4270    	madd	x16, x19, x15, x16
1000437d8: 9b0f7ccf    	mul	x15, x6, x15
1000437dc: 9bcd7e26    	umulh	x6, x17, x13
1000437e0: 9b0e1a2e    	madd	x14, x17, x14, x6
1000437e4: 9b0d382e    	madd	x14, x1, x13, x14
1000437e8: 9b0d7e2d    	mul	x13, x17, x13
1000437ec: 8b0c154c    	add	x12, x10, x12, lsl #5
1000437f0: a9404581    	ldp	x1, x17, [x12]
1000437f4: a9411993    	ldp	x19, x6, [x12, #0x10]
1000437f8: ab0d008d    	adds	x13, x4, x13
1000437fc: 9a0e006e    	adc	x14, x3, x14
100043800: ab1301ad    	adds	x13, x13, x19
100043804: 9a0601ce    	adc	x14, x14, x6
100043808: ab0f01ad    	adds	x13, x13, x15
10004380c: 9a1001ce    	adc	x14, x14, x16
100043810: ab0100ef    	adds	x15, x7, x1
100043814: ba1100b0    	adcs	x16, x5, x17
100043818: a900418f    	stp	x15, x16, [x12]
10004381c: ba1f01ad    	adcs	x13, x13, xzr
100043820: 9a8e35ce    	cinc	x14, x14, hs
100043824: a901398d    	stp	x13, x14, [x12, #0x10]
100043828: eb0b005f    	cmp	x2, x11
10004382c: 54fffa21    	b.ne	0x100043770 <__RINvNtNtNtCsagpXUhzPPpl_17field_regressions8campaign10candidates7integer7native4Kj4_Kj2_EB8_+0x3c>
100043830: a94123e9    	ldp	x9, x8, [sp, #0x10]
100043834: a94233ed    	ldp	x13, x12, [sp, #0x20]
100043838: a9432beb    	ldp	x11, x10, [sp, #0x30]
10004383c: a9443bef    	ldp	x15, x14, [sp, #0x40]
100043840: ab0d01ed    	adds	x13, x15, x13
100043844: 9a0c01cc    	adc	x12, x14, x12
100043848: 14000006    	b	0x100043860 <__RINvNtNtNtCsagpXUhzPPpl_17field_regressions8campaign10candidates7integer7native4Kj4_Kj2_EB8_+0x12c>
10004384c: d280000a    	mov	x10, #0x0               ; =0
100043850: d2800009    	mov	x9, #0x0                ; =0
100043854: d2800008    	mov	x8, #0x0                ; =0
100043858: d280000d    	mov	x13, #0x0               ; =0
10004385c: d280000c    	mov	x12, #0x0               ; =0
100043860: ab090169    	adds	x9, x11, x9
100043864: ba080148    	adcs	x8, x10, x8
100043868: ba1f01aa    	adcs	x10, x13, xzr
10004386c: a9002009    	stp	x9, x8, [x0]
100043870: 9a8c3588    	cinc	x8, x12, hs
100043874: a901200a    	stp	x10, x8, [x0, #0x10]
100043878: a9467bfd    	ldp	x29, x30, [sp, #0x60]
10004387c: a9454ff4    	ldp	x20, x19, [sp, #0x50]
100043880: 9101c3ff    	add	sp, sp, #0x70
100043884: d65f03c0    	ret
100043888: b0000b64    	adrp	x4, 0x1001b0000 <dyld_stub_binder+0x1001b0000>
10004388c: 9115c084    	add	x4, x4, #0x570
100043890: 910023e0    	add	x0, sp, #0x8
100043894: 910043e1    	add	x1, sp, #0x10
100043898: d2800002    	mov	x2, #0x0                ; =0
10004389c: 94045f6d    	bl	0x10015b650 <__RINvNtCs8Mbv00yxnRz_4core9panicking13assert_failedjjEB4_>


/private/var/folders/q0/v7l8dzd13k7f_q6j80sy03lm0000gn/T/f2z-field-gated-6lkppdze/field-regressions:	file format mach-o arm64

Disassembly of section __TEXT,__text:

00000001000300a4 <__RINvNtNtNtCs8IeiyMGxmWr_17field_regressions8campaign10candidates7integer7native4Kj4_Kj2_EB8_>:
1000300a4: d101c3ff    	sub	sp, sp, #0x70
1000300a8: a9054ff4    	stp	x20, x19, [sp, #0x50]
1000300ac: a9067bfd    	stp	x29, x30, [sp, #0x60]
1000300b0: 910183fd    	add	x29, sp, #0x60
1000300b4: a90093e2    	stp	x2, x4, [sp, #0x8]
1000300b8: eb04005f    	cmp	x2, x4
1000300bc: 540009e1    	b.ne	0x1000301f8 <__RINvNtNtNtCs8IeiyMGxmWr_17field_regressions8campaign10candidates7integer7native4Kj4_Kj2_EB8_+0x154>
1000300c0: 6f00e400    	movi.2d	v0, #0000000000000000
1000300c4: ad0183e0    	stp	q0, q0, [sp, #0x30]
1000300c8: ad0083e0    	stp	q0, q0, [sp, #0x10]
1000300cc: d280000b    	mov	x11, #0x0               ; =0
1000300d0: b4000762    	cbz	x2, 0x1000301bc <__RINvNtNtNtCs8IeiyMGxmWr_17field_regressions8campaign10candidates7integer7native4Kj4_Kj2_EB8_+0x118>
1000300d4: 91004028    	add	x8, x1, #0x10
1000300d8: 91004069    	add	x9, x3, #0x10
1000300dc: 910043ea    	add	x10, sp, #0x10
1000300e0: 9240016c    	and	x12, x11, #0x1
1000300e4: 9100056b    	add	x11, x11, #0x1
1000300e8: a97f392d    	ldp	x13, x14, [x9, #-0x10]
1000300ec: a97f410f    	ldp	x15, x16, [x8, #-0x10]
1000300f0: a8c20511    	ldp	x17, x1, [x8], #0x20
1000300f4: 9bce7e03    	umulh	x3, x16, x14
1000300f8: 9b0e7e04    	mul	x4, x16, x14
1000300fc: 9b0f7dc5    	mul	x5, x14, x15
100030100: 9bcf7dc6    	umulh	x6, x14, x15
100030104: ab060084    	adds	x4, x4, x6
100030108: 9a833463    	cinc	x3, x3, hs
10003010c: 9b0d7e06    	mul	x6, x16, x13
100030110: 9bcd7e07    	umulh	x7, x16, x13
100030114: ab070084    	adds	x4, x4, x7
100030118: 9a833463    	cinc	x3, x3, hs
10003011c: 9b0f7da7    	mul	x7, x13, x15
100030120: 9bcf7db3    	umulh	x19, x13, x15
100030124: ab1300a5    	adds	x5, x5, x19
100030128: 1a9f37f3    	cset	w19, hs
10003012c: ab0600a5    	adds	x5, x5, x6
100030130: ba130084    	adcs	x4, x4, x19
100030134: 9a833463    	cinc	x3, x3, hs
100030138: a8c24d26    	ldp	x6, x19, [x9], #0x20
10003013c: 9bcf7cd4    	umulh	x20, x6, x15
100030140: 9b1050d0    	madd	x16, x6, x16, x20
100030144: 9b0f4270    	madd	x16, x19, x15, x16
100030148: 9b0f7ccf    	mul	x15, x6, x15
10003014c: 9bcd7e26    	umulh	x6, x17, x13
100030150: 9b0e1a2e    	madd	x14, x17, x14, x6
100030154: 9b0d382e    	madd	x14, x1, x13, x14
100030158: 9b0d7e2d    	mul	x13, x17, x13
10003015c: 8b0c154c    	add	x12, x10, x12, lsl #5
100030160: a9404581    	ldp	x1, x17, [x12]
100030164: a9411993    	ldp	x19, x6, [x12, #0x10]
100030168: ab0d008d    	adds	x13, x4, x13
10003016c: 9a0e006e    	adc	x14, x3, x14
100030170: ab1301ad    	adds	x13, x13, x19
100030174: 9a0601ce    	adc	x14, x14, x6
100030178: ab0f01ad    	adds	x13, x13, x15
10003017c: 9a1001ce    	adc	x14, x14, x16
100030180: ab0100ef    	adds	x15, x7, x1
100030184: ba1100b0    	adcs	x16, x5, x17
100030188: a900418f    	stp	x15, x16, [x12]
10003018c: ba1f01ad    	adcs	x13, x13, xzr
100030190: 9a8e35ce    	cinc	x14, x14, hs
100030194: a901398d    	stp	x13, x14, [x12, #0x10]
100030198: eb0b005f    	cmp	x2, x11
10003019c: 54fffa21    	b.ne	0x1000300e0 <__RINvNtNtNtCs8IeiyMGxmWr_17field_regressions8campaign10candidates7integer7native4Kj4_Kj2_EB8_+0x3c>
1000301a0: a94123e9    	ldp	x9, x8, [sp, #0x10]
1000301a4: a94233ed    	ldp	x13, x12, [sp, #0x20]
1000301a8: a9432beb    	ldp	x11, x10, [sp, #0x30]
1000301ac: a9443bef    	ldp	x15, x14, [sp, #0x40]
1000301b0: ab0d01ed    	adds	x13, x15, x13
1000301b4: 9a0c01cc    	adc	x12, x14, x12
1000301b8: 14000006    	b	0x1000301d0 <__RINvNtNtNtCs8IeiyMGxmWr_17field_regressions8campaign10candidates7integer7native4Kj4_Kj2_EB8_+0x12c>
1000301bc: d280000a    	mov	x10, #0x0               ; =0
1000301c0: d2800009    	mov	x9, #0x0                ; =0
1000301c4: d2800008    	mov	x8, #0x0                ; =0
1000301c8: d280000d    	mov	x13, #0x0               ; =0
1000301cc: d280000c    	mov	x12, #0x0               ; =0
1000301d0: ab090169    	adds	x9, x11, x9
1000301d4: ba080148    	adcs	x8, x10, x8
1000301d8: ba1f01aa    	adcs	x10, x13, xzr
1000301dc: a9002009    	stp	x9, x8, [x0]
1000301e0: 9a8c3588    	cinc	x8, x12, hs
1000301e4: a901200a    	stp	x10, x8, [x0, #0x10]
1000301e8: a9467bfd    	ldp	x29, x30, [sp, #0x60]
1000301ec: a9454ff4    	ldp	x20, x19, [sp, #0x50]
1000301f0: 9101c3ff    	add	sp, sp, #0x70
1000301f4: d65f03c0    	ret
1000301f8: f0000a24    	adrp	x4, 0x100177000 <dyld_stub_binder+0x100177000>
1000301fc: 911da084    	add	x4, x4, #0x768
100030200: 910023e0    	add	x0, sp, #0x8
100030204: 910043e1    	add	x1, sp, #0x10
100030208: d2800002    	mov	x2, #0x0                ; =0
10003020c: 9403f158    	bl	0x10012c76c <__RINvNtCs8Mbv00yxnRz_4core9panicking13assert_failedjjEB4_>

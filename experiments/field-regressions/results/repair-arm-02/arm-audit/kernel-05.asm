
/private/var/folders/q0/v7l8dzd13k7f_q6j80sy03lm0000gn/T/f2z-field-gated-aejimxw9/field-regressions:	file format mach-o arm64

Disassembly of section __TEXT,__text:

000000010003f598 <__RINvNtNtNtCsaHyC9lX8wyC_17field_regressions8campaign10candidates7integer7native4Kj4_Kj2_EB8_>:
10003f598: d101c3ff    	sub	sp, sp, #0x70
10003f59c: a9054ff4    	stp	x20, x19, [sp, #0x50]
10003f5a0: a9067bfd    	stp	x29, x30, [sp, #0x60]
10003f5a4: 910183fd    	add	x29, sp, #0x60
10003f5a8: a90093e2    	stp	x2, x4, [sp, #0x8]
10003f5ac: eb04005f    	cmp	x2, x4
10003f5b0: 540009e1    	b.ne	0x10003f6ec <__RINvNtNtNtCsaHyC9lX8wyC_17field_regressions8campaign10candidates7integer7native4Kj4_Kj2_EB8_+0x154>
10003f5b4: 6f00e400    	movi.2d	v0, #0000000000000000
10003f5b8: ad0183e0    	stp	q0, q0, [sp, #0x30]
10003f5bc: ad0083e0    	stp	q0, q0, [sp, #0x10]
10003f5c0: d280000b    	mov	x11, #0x0               ; =0
10003f5c4: b4000762    	cbz	x2, 0x10003f6b0 <__RINvNtNtNtCsaHyC9lX8wyC_17field_regressions8campaign10candidates7integer7native4Kj4_Kj2_EB8_+0x118>
10003f5c8: 91004028    	add	x8, x1, #0x10
10003f5cc: 91004069    	add	x9, x3, #0x10
10003f5d0: 910043ea    	add	x10, sp, #0x10
10003f5d4: 9240016c    	and	x12, x11, #0x1
10003f5d8: 9100056b    	add	x11, x11, #0x1
10003f5dc: a97f392d    	ldp	x13, x14, [x9, #-0x10]
10003f5e0: a97f410f    	ldp	x15, x16, [x8, #-0x10]
10003f5e4: a8c20511    	ldp	x17, x1, [x8], #0x20
10003f5e8: 9bce7e03    	umulh	x3, x16, x14
10003f5ec: 9b0e7e04    	mul	x4, x16, x14
10003f5f0: 9b0f7dc5    	mul	x5, x14, x15
10003f5f4: 9bcf7dc6    	umulh	x6, x14, x15
10003f5f8: ab060084    	adds	x4, x4, x6
10003f5fc: 9a833463    	cinc	x3, x3, hs
10003f600: 9b0d7e06    	mul	x6, x16, x13
10003f604: 9bcd7e07    	umulh	x7, x16, x13
10003f608: ab070084    	adds	x4, x4, x7
10003f60c: 9a833463    	cinc	x3, x3, hs
10003f610: 9b0f7da7    	mul	x7, x13, x15
10003f614: 9bcf7db3    	umulh	x19, x13, x15
10003f618: ab1300a5    	adds	x5, x5, x19
10003f61c: 1a9f37f3    	cset	w19, hs
10003f620: ab0600a5    	adds	x5, x5, x6
10003f624: ba130084    	adcs	x4, x4, x19
10003f628: 9a833463    	cinc	x3, x3, hs
10003f62c: a8c24d26    	ldp	x6, x19, [x9], #0x20
10003f630: 9bcf7cd4    	umulh	x20, x6, x15
10003f634: 9b1050d0    	madd	x16, x6, x16, x20
10003f638: 9b0f4270    	madd	x16, x19, x15, x16
10003f63c: 9b0f7ccf    	mul	x15, x6, x15
10003f640: 9bcd7e26    	umulh	x6, x17, x13
10003f644: 9b0e1a2e    	madd	x14, x17, x14, x6
10003f648: 9b0d382e    	madd	x14, x1, x13, x14
10003f64c: 9b0d7e2d    	mul	x13, x17, x13
10003f650: 8b0c154c    	add	x12, x10, x12, lsl #5
10003f654: a9404581    	ldp	x1, x17, [x12]
10003f658: a9411993    	ldp	x19, x6, [x12, #0x10]
10003f65c: ab0d008d    	adds	x13, x4, x13
10003f660: 9a0e006e    	adc	x14, x3, x14
10003f664: ab1301ad    	adds	x13, x13, x19
10003f668: 9a0601ce    	adc	x14, x14, x6
10003f66c: ab0f01ad    	adds	x13, x13, x15
10003f670: 9a1001ce    	adc	x14, x14, x16
10003f674: ab0100ef    	adds	x15, x7, x1
10003f678: ba1100b0    	adcs	x16, x5, x17
10003f67c: a900418f    	stp	x15, x16, [x12]
10003f680: ba1f01ad    	adcs	x13, x13, xzr
10003f684: 9a8e35ce    	cinc	x14, x14, hs
10003f688: a901398d    	stp	x13, x14, [x12, #0x10]
10003f68c: eb0b005f    	cmp	x2, x11
10003f690: 54fffa21    	b.ne	0x10003f5d4 <__RINvNtNtNtCsaHyC9lX8wyC_17field_regressions8campaign10candidates7integer7native4Kj4_Kj2_EB8_+0x3c>
10003f694: a94123e9    	ldp	x9, x8, [sp, #0x10]
10003f698: a94233ed    	ldp	x13, x12, [sp, #0x20]
10003f69c: a9432beb    	ldp	x11, x10, [sp, #0x30]
10003f6a0: a9443bef    	ldp	x15, x14, [sp, #0x40]
10003f6a4: ab0d01ed    	adds	x13, x15, x13
10003f6a8: 9a0c01cc    	adc	x12, x14, x12
10003f6ac: 14000006    	b	0x10003f6c4 <__RINvNtNtNtCsaHyC9lX8wyC_17field_regressions8campaign10candidates7integer7native4Kj4_Kj2_EB8_+0x12c>
10003f6b0: d280000a    	mov	x10, #0x0               ; =0
10003f6b4: d2800009    	mov	x9, #0x0                ; =0
10003f6b8: d2800008    	mov	x8, #0x0                ; =0
10003f6bc: d280000d    	mov	x13, #0x0               ; =0
10003f6c0: d280000c    	mov	x12, #0x0               ; =0
10003f6c4: ab090169    	adds	x9, x11, x9
10003f6c8: ba080148    	adcs	x8, x10, x8
10003f6cc: ba1f01aa    	adcs	x10, x13, xzr
10003f6d0: a9002009    	stp	x9, x8, [x0]
10003f6d4: 9a8c3588    	cinc	x8, x12, hs
10003f6d8: a901200a    	stp	x10, x8, [x0, #0x10]
10003f6dc: a9467bfd    	ldp	x29, x30, [sp, #0x60]
10003f6e0: a9454ff4    	ldp	x20, x19, [sp, #0x50]
10003f6e4: 9101c3ff    	add	sp, sp, #0x70
10003f6e8: d65f03c0    	ret
10003f6ec: b0000ae4    	adrp	x4, 0x10019c000 <dyld_stub_binder+0x10019c000>
10003f6f0: 9100e084    	add	x4, x4, #0x38
10003f6f4: 910023e0    	add	x0, sp, #0x8
10003f6f8: 910043e1    	add	x1, sp, #0x10
10003f6fc: d2800002    	mov	x2, #0x0                ; =0
10003f700: 9404309a    	bl	0x10014b968 <__RINvNtCs8Mbv00yxnRz_4core9panicking13assert_failedjjEB4_>

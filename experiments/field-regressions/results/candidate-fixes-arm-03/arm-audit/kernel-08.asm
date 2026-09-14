
/private/var/folders/q0/v7l8dzd13k7f_q6j80sy03lm0000gn/T/f2z-field-gated-h_tggvge/field-regressions:	file format mach-o arm64

Disassembly of section __TEXT,__text:

0000000100043eb8 <__RINvNtNtNtCsfDw8YqMpZPC_17field_regressions8campaign10candidates7integer7native4Kj4_Kj2_EB8_>:
100043eb8: d101c3ff    	sub	sp, sp, #0x70
100043ebc: a9054ff4    	stp	x20, x19, [sp, #0x50]
100043ec0: a9067bfd    	stp	x29, x30, [sp, #0x60]
100043ec4: 910183fd    	add	x29, sp, #0x60
100043ec8: a90093e2    	stp	x2, x4, [sp, #0x8]
100043ecc: eb04005f    	cmp	x2, x4
100043ed0: 540009e1    	b.ne	0x10004400c <__RINvNtNtNtCsfDw8YqMpZPC_17field_regressions8campaign10candidates7integer7native4Kj4_Kj2_EB8_+0x154>
100043ed4: 6f00e400    	movi.2d	v0, #0000000000000000
100043ed8: ad0183e0    	stp	q0, q0, [sp, #0x30]
100043edc: ad0083e0    	stp	q0, q0, [sp, #0x10]
100043ee0: d280000b    	mov	x11, #0x0               ; =0
100043ee4: b4000762    	cbz	x2, 0x100043fd0 <__RINvNtNtNtCsfDw8YqMpZPC_17field_regressions8campaign10candidates7integer7native4Kj4_Kj2_EB8_+0x118>
100043ee8: 91004028    	add	x8, x1, #0x10
100043eec: 91004069    	add	x9, x3, #0x10
100043ef0: 910043ea    	add	x10, sp, #0x10
100043ef4: 9240016c    	and	x12, x11, #0x1
100043ef8: 9100056b    	add	x11, x11, #0x1
100043efc: a97f392d    	ldp	x13, x14, [x9, #-0x10]
100043f00: a97f410f    	ldp	x15, x16, [x8, #-0x10]
100043f04: a8c20511    	ldp	x17, x1, [x8], #0x20
100043f08: 9bce7e03    	umulh	x3, x16, x14
100043f0c: 9b0e7e04    	mul	x4, x16, x14
100043f10: 9b0f7dc5    	mul	x5, x14, x15
100043f14: 9bcf7dc6    	umulh	x6, x14, x15
100043f18: ab060084    	adds	x4, x4, x6
100043f1c: 9a833463    	cinc	x3, x3, hs
100043f20: 9b0d7e06    	mul	x6, x16, x13
100043f24: 9bcd7e07    	umulh	x7, x16, x13
100043f28: ab070084    	adds	x4, x4, x7
100043f2c: 9a833463    	cinc	x3, x3, hs
100043f30: 9b0f7da7    	mul	x7, x13, x15
100043f34: 9bcf7db3    	umulh	x19, x13, x15
100043f38: ab1300a5    	adds	x5, x5, x19
100043f3c: 1a9f37f3    	cset	w19, hs
100043f40: ab0600a5    	adds	x5, x5, x6
100043f44: ba130084    	adcs	x4, x4, x19
100043f48: 9a833463    	cinc	x3, x3, hs
100043f4c: a8c24d26    	ldp	x6, x19, [x9], #0x20
100043f50: 9bcf7cd4    	umulh	x20, x6, x15
100043f54: 9b1050d0    	madd	x16, x6, x16, x20
100043f58: 9b0f4270    	madd	x16, x19, x15, x16
100043f5c: 9b0f7ccf    	mul	x15, x6, x15
100043f60: 9bcd7e26    	umulh	x6, x17, x13
100043f64: 9b0e1a2e    	madd	x14, x17, x14, x6
100043f68: 9b0d382e    	madd	x14, x1, x13, x14
100043f6c: 9b0d7e2d    	mul	x13, x17, x13
100043f70: 8b0c154c    	add	x12, x10, x12, lsl #5
100043f74: a9404581    	ldp	x1, x17, [x12]
100043f78: a9411993    	ldp	x19, x6, [x12, #0x10]
100043f7c: ab0d008d    	adds	x13, x4, x13
100043f80: 9a0e006e    	adc	x14, x3, x14
100043f84: ab1301ad    	adds	x13, x13, x19
100043f88: 9a0601ce    	adc	x14, x14, x6
100043f8c: ab0f01ad    	adds	x13, x13, x15
100043f90: 9a1001ce    	adc	x14, x14, x16
100043f94: ab0100ef    	adds	x15, x7, x1
100043f98: ba1100b0    	adcs	x16, x5, x17
100043f9c: a900418f    	stp	x15, x16, [x12]
100043fa0: ba1f01ad    	adcs	x13, x13, xzr
100043fa4: 9a8e35ce    	cinc	x14, x14, hs
100043fa8: a901398d    	stp	x13, x14, [x12, #0x10]
100043fac: eb0b005f    	cmp	x2, x11
100043fb0: 54fffa21    	b.ne	0x100043ef4 <__RINvNtNtNtCsfDw8YqMpZPC_17field_regressions8campaign10candidates7integer7native4Kj4_Kj2_EB8_+0x3c>
100043fb4: a94123e9    	ldp	x9, x8, [sp, #0x10]
100043fb8: a94233ed    	ldp	x13, x12, [sp, #0x20]
100043fbc: a9432beb    	ldp	x11, x10, [sp, #0x30]
100043fc0: a9443bef    	ldp	x15, x14, [sp, #0x40]
100043fc4: ab0d01ed    	adds	x13, x15, x13
100043fc8: 9a0c01cc    	adc	x12, x14, x12
100043fcc: 14000006    	b	0x100043fe4 <__RINvNtNtNtCsfDw8YqMpZPC_17field_regressions8campaign10candidates7integer7native4Kj4_Kj2_EB8_+0x12c>
100043fd0: d280000a    	mov	x10, #0x0               ; =0
100043fd4: d2800009    	mov	x9, #0x0                ; =0
100043fd8: d2800008    	mov	x8, #0x0                ; =0
100043fdc: d280000d    	mov	x13, #0x0               ; =0
100043fe0: d280000c    	mov	x12, #0x0               ; =0
100043fe4: ab090169    	adds	x9, x11, x9
100043fe8: ba080148    	adcs	x8, x10, x8
100043fec: ba1f01aa    	adcs	x10, x13, xzr
100043ff0: a9002009    	stp	x9, x8, [x0]
100043ff4: 9a8c3588    	cinc	x8, x12, hs
100043ff8: a901200a    	stp	x10, x8, [x0, #0x10]
100043ffc: a9467bfd    	ldp	x29, x30, [sp, #0x60]
100044000: a9454ff4    	ldp	x20, x19, [sp, #0x50]
100044004: 9101c3ff    	add	sp, sp, #0x70
100044008: d65f03c0    	ret
10004400c: 90000b64    	adrp	x4, 0x1001b0000 <dyld_stub_binder+0x1001b0000>
100044010: 91186084    	add	x4, x4, #0x618
100044014: 910023e0    	add	x0, sp, #0x8
100044018: 910043e1    	add	x1, sp, #0x10
10004401c: d2800002    	mov	x2, #0x0                ; =0
100044020: 94046229    	bl	0x10015c8c4 <__RINvNtCs8Mbv00yxnRz_4core9panicking13assert_failedjjEB4_>

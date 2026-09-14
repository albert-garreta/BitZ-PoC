
/private/var/folders/q0/v7l8dzd13k7f_q6j80sy03lm0000gn/T/f2z-field-gated-h_tggvge/field-regressions:	file format mach-o arm64

Disassembly of section __TEXT,__text:

000000010003884c <__RINvNtNtNtCsfDw8YqMpZPC_17field_regressions8campaign10candidates5words13exact_columnsKj9_Kj13_EB8_>:
10003884c: a9ba6ffc    	stp	x28, x27, [sp, #-0x60]!
100038850: a90167fa    	stp	x26, x25, [sp, #0x10]
100038854: a9025ff8    	stp	x24, x23, [sp, #0x20]
100038858: a90357f6    	stp	x22, x21, [sp, #0x30]
10003885c: a9044ff4    	stp	x20, x19, [sp, #0x40]
100038860: a9057bfd    	stp	x29, x30, [sp, #0x50]
100038864: 910143fd    	add	x29, sp, #0x50
100038868: d10683ff    	sub	sp, sp, #0x1a0
10003886c: a90593e2    	stp	x2, x4, [sp, #0x58]
100038870: eb04005f    	cmp	x2, x4
100038874: 54001d61    	b.ne	0x100038c20 <__RINvNtNtNtCsfDw8YqMpZPC_17field_regressions8campaign10candidates5words13exact_columnsKj9_Kj13_EB8_+0x3d4>
100038878: 910183e8    	add	x8, sp, #0x60
10003887c: 6f00e400    	movi.2d	v0, #0000000000000000
100038880: ad088100    	stp	q0, q0, [x8, #0x110]
100038884: ad078100    	stp	q0, q0, [x8, #0xf0]
100038888: ad068100    	stp	q0, q0, [x8, #0xd0]
10003888c: ad058100    	stp	q0, q0, [x8, #0xb0]
100038890: ad048100    	stp	q0, q0, [x8, #0x90]
100038894: 3d802100    	str	q0, [x8, #0x80]
100038898: ad0603e0    	stp	q0, q0, [sp, #0xc0]
10003889c: ad0503e0    	stp	q0, q0, [sp, #0xa0]
1000388a0: ad0403e0    	stp	q0, q0, [sp, #0x80]
1000388a4: ad0303e0    	stp	q0, q0, [sp, #0x60]
1000388a8: b40010a2    	cbz	x2, 0x100038abc <__RINvNtNtNtCsfDw8YqMpZPC_17field_regressions8campaign10candidates5words13exact_columnsKj9_Kj13_EB8_+0x270>
1000388ac: d2800008    	mov	x8, #0x0                ; =0
1000388b0: 52800909    	mov	w9, #0x48               ; =72
1000388b4: 910183ea    	add	x10, sp, #0x60
1000388b8: d280000b    	mov	x11, #0x0               ; =0
1000388bc: 9b090d06    	madd	x6, x8, x9, x3
1000388c0: a94234cc    	ldp	x12, x13, [x6, #0x20]
1000388c4: a9433cce    	ldp	x14, x15, [x6, #0x30]
1000388c8: a94044d0    	ldp	x16, x17, [x6]
1000388cc: a94110c5    	ldp	x5, x4, [x6, #0x10]
1000388d0: f94020c6    	ldr	x6, [x6, #0x40]
1000388d4: aa0103e7    	mov	x7, x1
1000388d8: f84084f4    	ldr	x20, [x7], #0x8
1000388dc: 8b0b0153    	add	x19, x10, x11
1000388e0: 9bd47e15    	umulh	x21, x16, x20
1000388e4: 9b147e16    	mul	x22, x16, x20
1000388e8: a9405e78    	ldp	x24, x23, [x19]
1000388ec: ab1802d6    	adds	x22, x22, x24
1000388f0: 9a9736f7    	cinc	x23, x23, hs
1000388f4: a9005e76    	stp	x22, x23, [x19]
1000388f8: a9415a77    	ldp	x23, x22, [x19, #0x10]
1000388fc: ab1502f5    	adds	x21, x23, x21
100038900: 9a9636d6    	cinc	x22, x22, hs
100038904: 9bd47e37    	umulh	x23, x17, x20
100038908: 9b147e38    	mul	x24, x17, x20
10003890c: ab150315    	adds	x21, x24, x21
100038910: 9a9636d6    	cinc	x22, x22, hs
100038914: a9015a75    	stp	x21, x22, [x19, #0x10]
100038918: a9425676    	ldp	x22, x21, [x19, #0x20]
10003891c: ab1702d6    	adds	x22, x22, x23
100038920: 9a9536b5    	cinc	x21, x21, hs
100038924: 9bd47cb7    	umulh	x23, x5, x20
100038928: 9b147cb8    	mul	x24, x5, x20
10003892c: ab160316    	adds	x22, x24, x22
100038930: 9a9536b5    	cinc	x21, x21, hs
100038934: a9025676    	stp	x22, x21, [x19, #0x20]
100038938: a9435676    	ldp	x22, x21, [x19, #0x30]
10003893c: ab1702d6    	adds	x22, x22, x23
100038940: 9a9536b5    	cinc	x21, x21, hs
100038944: 9bd47c97    	umulh	x23, x4, x20
100038948: 9b147c98    	mul	x24, x4, x20
10003894c: ab160316    	adds	x22, x24, x22
100038950: 9a9536b5    	cinc	x21, x21, hs
100038954: a9035676    	stp	x22, x21, [x19, #0x30]
100038958: a9445676    	ldp	x22, x21, [x19, #0x40]
10003895c: ab1702d6    	adds	x22, x22, x23
100038960: 9a9536b5    	cinc	x21, x21, hs
100038964: 9bd47d97    	umulh	x23, x12, x20
100038968: 9b147d98    	mul	x24, x12, x20
10003896c: ab160316    	adds	x22, x24, x22
100038970: 9a9536b5    	cinc	x21, x21, hs
100038974: a9045676    	stp	x22, x21, [x19, #0x40]
100038978: a9455676    	ldp	x22, x21, [x19, #0x50]
10003897c: ab1702d6    	adds	x22, x22, x23
100038980: 9a9536b5    	cinc	x21, x21, hs
100038984: 9bd47db7    	umulh	x23, x13, x20
100038988: 9b147db8    	mul	x24, x13, x20
10003898c: ab160316    	adds	x22, x24, x22
100038990: 9a9536b5    	cinc	x21, x21, hs
100038994: a9055676    	stp	x22, x21, [x19, #0x50]
100038998: a9465676    	ldp	x22, x21, [x19, #0x60]
10003899c: ab1702d6    	adds	x22, x22, x23
1000389a0: 9a9536b5    	cinc	x21, x21, hs
1000389a4: 9bd47dd7    	umulh	x23, x14, x20
1000389a8: 9b147dd8    	mul	x24, x14, x20
1000389ac: ab160316    	adds	x22, x24, x22
1000389b0: 9a9536b5    	cinc	x21, x21, hs
1000389b4: a9065676    	stp	x22, x21, [x19, #0x60]
1000389b8: a9475676    	ldp	x22, x21, [x19, #0x70]
1000389bc: ab1702d6    	adds	x22, x22, x23
1000389c0: 9a9536b5    	cinc	x21, x21, hs
1000389c4: 9bd47df7    	umulh	x23, x15, x20
1000389c8: 9b147df8    	mul	x24, x15, x20
1000389cc: ab160316    	adds	x22, x24, x22
1000389d0: 9a9536b5    	cinc	x21, x21, hs
1000389d4: a9075676    	stp	x22, x21, [x19, #0x70]
1000389d8: a9485676    	ldp	x22, x21, [x19, #0x80]
1000389dc: ab1702d6    	adds	x22, x22, x23
1000389e0: 9a9536b5    	cinc	x21, x21, hs
1000389e4: 9bd47cd7    	umulh	x23, x6, x20
1000389e8: 9b147cd4    	mul	x20, x6, x20
1000389ec: ab160294    	adds	x20, x20, x22
1000389f0: 9a9536b5    	cinc	x21, x21, hs
1000389f4: a9085674    	stp	x20, x21, [x19, #0x80]
1000389f8: a9495275    	ldp	x21, x20, [x19, #0x90]
1000389fc: ab1702b5    	adds	x21, x21, x23
100038a00: 9a943694    	cinc	x20, x20, hs
100038a04: a9095275    	stp	x21, x20, [x19, #0x90]
100038a08: 9100416b    	add	x11, x11, #0x10
100038a0c: f102417f    	cmp	x11, #0x90
100038a10: 54fff641    	b.ne	0x1000388d8 <__RINvNtNtNtCsfDw8YqMpZPC_17field_regressions8campaign10candidates5words13exact_columnsKj9_Kj13_EB8_+0x8c>
100038a14: 91000508    	add	x8, x8, #0x1
100038a18: 91012021    	add	x1, x1, #0x48
100038a1c: eb02011f    	cmp	x8, x2
100038a20: 54fff4c1    	b.ne	0x1000388b8 <__RINvNtNtNtCsfDw8YqMpZPC_17field_regressions8campaign10candidates5words13exact_columnsKj9_Kj13_EB8_+0x6c>
100038a24: a94637ec    	ldp	x12, x13, [sp, #0x60]
100038a28: a9472ff0    	ldp	x16, x11, [sp, #0x70]
100038a2c: a9482bef    	ldp	x15, x10, [sp, #0x80]
100038a30: a94953ee    	ldp	x14, x20, [sp, #0x90]
100038a34: a94a4fe9    	ldp	x9, x19, [sp, #0xa0]
100038a38: a94b1fe8    	ldp	x8, x7, [sp, #0xb0]
100038a3c: a94c47fe    	ldp	x30, x17, [sp, #0xc0]
100038a40: f90007f1    	str	x17, [sp, #0x8]
100038a44: a94d1bfc    	ldp	x28, x6, [sp, #0xd0]
100038a48: a94e17fb    	ldp	x27, x5, [sp, #0xe0]
100038a4c: a94f13fa    	ldp	x26, x4, [sp, #0xf0]
100038a50: a9500ff9    	ldp	x25, x3, [sp, #0x100]
100038a54: a9510bf8    	ldp	x24, x2, [sp, #0x110]
100038a58: a95207f7    	ldp	x23, x1, [sp, #0x120]
100038a5c: a95347f6    	ldp	x22, x17, [sp, #0x130]
100038a60: f9000bf1    	str	x17, [sp, #0x10]
100038a64: a95447f5    	ldp	x21, x17, [sp, #0x140]
100038a68: f9000ff1    	str	x17, [sp, #0x18]
100038a6c: f940aff1    	ldr	x17, [sp, #0x158]
100038a70: f90017f1    	str	x17, [sp, #0x28]
100038a74: f940abf1    	ldr	x17, [sp, #0x150]
100038a78: f90013f1    	str	x17, [sp, #0x20]
100038a7c: f940b7f1    	ldr	x17, [sp, #0x168]
100038a80: f9001ff1    	str	x17, [sp, #0x38]
100038a84: f940b3f1    	ldr	x17, [sp, #0x160]
100038a88: f9001bf1    	str	x17, [sp, #0x30]
100038a8c: f940bff1    	ldr	x17, [sp, #0x178]
100038a90: f9002bf1    	str	x17, [sp, #0x50]
100038a94: f940bbf1    	ldr	x17, [sp, #0x170]
100038a98: f90023f1    	str	x17, [sp, #0x40]
100038a9c: f940c3f1    	ldr	x17, [sp, #0x180]
100038aa0: f90027f1    	str	x17, [sp, #0x48]
100038aa4: aa0103f1    	mov	x17, x1
100038aa8: aa0203e1    	mov	x1, x2
100038aac: aa0303e2    	mov	x2, x3
100038ab0: aa0403e3    	mov	x3, x4
100038ab4: f94007e4    	ldr	x4, [sp, #0x8]
100038ab8: 14000021    	b	0x100038b3c <__RINvNtNtNtCsfDw8YqMpZPC_17field_regressions8campaign10candidates5words13exact_columnsKj9_Kj13_EB8_+0x2f0>
100038abc: a9047fff    	stp	xzr, xzr, [sp, #0x40]
100038ac0: f9002bff    	str	xzr, [sp, #0x50]
100038ac4: a9037fff    	stp	xzr, xzr, [sp, #0x30]
100038ac8: a9027fff    	stp	xzr, xzr, [sp, #0x20]
100038acc: d2800015    	mov	x21, #0x0               ; =0
100038ad0: a9017fff    	stp	xzr, xzr, [sp, #0x10]
100038ad4: d2800016    	mov	x22, #0x0               ; =0
100038ad8: d2800017    	mov	x23, #0x0               ; =0
100038adc: d2800011    	mov	x17, #0x0               ; =0
100038ae0: d2800018    	mov	x24, #0x0               ; =0
100038ae4: d2800001    	mov	x1, #0x0                ; =0
100038ae8: d2800019    	mov	x25, #0x0               ; =0
100038aec: d280001a    	mov	x26, #0x0               ; =0
100038af0: d2800003    	mov	x3, #0x0                ; =0
100038af4: d280001b    	mov	x27, #0x0               ; =0
100038af8: d2800005    	mov	x5, #0x0                ; =0
100038afc: d280001c    	mov	x28, #0x0               ; =0
100038b00: d2800006    	mov	x6, #0x0                ; =0
100038b04: d280001e    	mov	x30, #0x0               ; =0
100038b08: d2800004    	mov	x4, #0x0                ; =0
100038b0c: d2800008    	mov	x8, #0x0                ; =0
100038b10: d2800007    	mov	x7, #0x0                ; =0
100038b14: d2800009    	mov	x9, #0x0                ; =0
100038b18: d2800013    	mov	x19, #0x0               ; =0
100038b1c: d280000e    	mov	x14, #0x0               ; =0
100038b20: d2800014    	mov	x20, #0x0               ; =0
100038b24: d280000f    	mov	x15, #0x0               ; =0
100038b28: d280000a    	mov	x10, #0x0               ; =0
100038b2c: d2800010    	mov	x16, #0x0               ; =0
100038b30: d280000b    	mov	x11, #0x0               ; =0
100038b34: d280000c    	mov	x12, #0x0               ; =0
100038b38: d280000d    	mov	x13, #0x0               ; =0
100038b3c: ab1001ad    	adds	x13, x13, x16
100038b40: a900340c    	stp	x12, x13, [x0]
100038b44: 9a8b356b    	cinc	x11, x11, hs
100038b48: ab0f016b    	adds	x11, x11, x15
100038b4c: 9a8a354a    	cinc	x10, x10, hs
100038b50: ab0e014a    	adds	x10, x10, x14
100038b54: a901280b    	stp	x11, x10, [x0, #0x10]
100038b58: 9a94368a    	cinc	x10, x20, hs
100038b5c: ab090149    	adds	x9, x10, x9
100038b60: 9a93366a    	cinc	x10, x19, hs
100038b64: ab080148    	adds	x8, x10, x8
100038b68: 9a8734ea    	cinc	x10, x7, hs
100038b6c: ab1e014a    	adds	x10, x10, x30
100038b70: 9a84348b    	cinc	x11, x4, hs
100038b74: ab1c016b    	adds	x11, x11, x28
100038b78: 9a8634cc    	cinc	x12, x6, hs
100038b7c: ab1b018c    	adds	x12, x12, x27
100038b80: 9a8534ad    	cinc	x13, x5, hs
100038b84: ab1a01ad    	adds	x13, x13, x26
100038b88: 9a83346e    	cinc	x14, x3, hs
100038b8c: ab1901ce    	adds	x14, x14, x25
100038b90: 9a82344f    	cinc	x15, x2, hs
100038b94: ab1801ef    	adds	x15, x15, x24
100038b98: 9a813430    	cinc	x16, x1, hs
100038b9c: ab170210    	adds	x16, x16, x23
100038ba0: 9a913631    	cinc	x17, x17, hs
100038ba4: ab160231    	adds	x17, x17, x22
100038ba8: a9410be1    	ldp	x1, x2, [sp, #0x10]
100038bac: 9a813421    	cinc	x1, x1, hs
100038bb0: ab150021    	adds	x1, x1, x21
100038bb4: 9a823442    	cinc	x2, x2, hs
100038bb8: a9022009    	stp	x9, x8, [x0, #0x20]
100038bbc: a94227e8    	ldp	x8, x9, [sp, #0x20]
100038bc0: ab080048    	adds	x8, x2, x8
100038bc4: a9032c0a    	stp	x10, x11, [x0, #0x30]
100038bc8: 9a893529    	cinc	x9, x9, hs
100038bcc: a904340c    	stp	x12, x13, [x0, #0x40]
100038bd0: a9432beb    	ldp	x11, x10, [sp, #0x30]
100038bd4: ab0b0129    	adds	x9, x9, x11
100038bd8: a9053c0e    	stp	x14, x15, [x0, #0x50]
100038bdc: 9a8a354a    	cinc	x10, x10, hs
100038be0: a9064410    	stp	x16, x17, [x0, #0x60]
100038be4: f94023eb    	ldr	x11, [sp, #0x40]
100038be8: ab0b014a    	adds	x10, x10, x11
100038bec: a9072001    	stp	x1, x8, [x0, #0x70]
100038bf0: a944a3eb    	ldp	x11, x8, [sp, #0x48]
100038bf4: 9a080168    	adc	x8, x11, x8
100038bf8: a9082809    	stp	x9, x10, [x0, #0x80]
100038bfc: f9004808    	str	x8, [x0, #0x90]
100038c00: 910683ff    	add	sp, sp, #0x1a0
100038c04: a9457bfd    	ldp	x29, x30, [sp, #0x50]
100038c08: a9444ff4    	ldp	x20, x19, [sp, #0x40]
100038c0c: a94357f6    	ldp	x22, x21, [sp, #0x30]
100038c10: a9425ff8    	ldp	x24, x23, [sp, #0x20]
100038c14: a94167fa    	ldp	x26, x25, [sp, #0x10]
100038c18: a8c66ffc    	ldp	x28, x27, [sp], #0x60
100038c1c: d65f03c0    	ret
100038c20: f0000ba4    	adrp	x4, 0x1001af000 <dyld_stub_binder+0x1001af000>
100038c24: 91370084    	add	x4, x4, #0xdc0
100038c28: 910163e0    	add	x0, sp, #0x58
100038c2c: 910183e1    	add	x1, sp, #0x60
100038c30: d2800002    	mov	x2, #0x0                ; =0
100038c34: 94048f24    	bl	0x10015c8c4 <__RINvNtCs8Mbv00yxnRz_4core9panicking13assert_failedjjEB4_>

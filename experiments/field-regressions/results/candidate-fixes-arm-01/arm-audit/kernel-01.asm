
/private/var/folders/q0/v7l8dzd13k7f_q6j80sy03lm0000gn/T/f2z-field-gated-pycl0nhx/field-regressions:	file format mach-o arm64

Disassembly of section __TEXT,__text:

0000000100037e30 <__RINvNtNtNtCsagpXUhzPPpl_17field_regressions8campaign10candidates5words13exact_columnsKj2_Kj5_EB8_>:
100037e30: d10083ff    	sub	sp, sp, #0x20
100037e34: a9017bfd    	stp	x29, x30, [sp, #0x10]
100037e38: 910043fd    	add	x29, sp, #0x10
100037e3c: a90013e2    	stp	x2, x4, [sp]
100037e40: eb04005f    	cmp	x2, x4
100037e44: 540006c1    	b.ne	0x100037f1c <__RINvNtNtNtCsagpXUhzPPpl_17field_regressions8campaign10candidates5words13exact_columnsKj2_Kj5_EB8_+0xec>
100037e48: d280000b    	mov	x11, #0x0               ; =0
100037e4c: d280000c    	mov	x12, #0x0               ; =0
100037e50: d280000d    	mov	x13, #0x0               ; =0
100037e54: d280000f    	mov	x15, #0x0               ; =0
100037e58: d2800009    	mov	x9, #0x0                ; =0
100037e5c: d2800008    	mov	x8, #0x0                ; =0
100037e60: d280000a    	mov	x10, #0x0               ; =0
100037e64: d280000e    	mov	x14, #0x0               ; =0
100037e68: b4000422    	cbz	x2, 0x100037eec <__RINvNtNtNtCsagpXUhzPPpl_17field_regressions8campaign10candidates5words13exact_columnsKj2_Kj5_EB8_+0xbc>
100037e6c: 91002030    	add	x16, x1, #0x8
100037e70: 91002071    	add	x17, x3, #0x8
100037e74: a97f8e21    	ldp	x1, x3, [x17, #-0x8]
100037e78: a97f9604    	ldp	x4, x5, [x16, #-0x8]
100037e7c: 9b047c26    	mul	x6, x1, x4
100037e80: ab0a00ca    	adds	x10, x6, x10
100037e84: 9bc47c26    	umulh	x6, x1, x4
100037e88: 9a8e35ce    	cinc	x14, x14, hs
100037e8c: ab06016b    	adds	x11, x11, x6
100037e90: 9a8c358c    	cinc	x12, x12, hs
100037e94: 9b047c66    	mul	x6, x3, x4
100037e98: ab0b00cb    	adds	x11, x6, x11
100037e9c: 9bc47c64    	umulh	x4, x3, x4
100037ea0: 9a8c358c    	cinc	x12, x12, hs
100037ea4: ab0401ad    	adds	x13, x13, x4
100037ea8: 9a8f35ef    	cinc	x15, x15, hs
100037eac: 9bc57c24    	umulh	x4, x1, x5
100037eb0: 9b057c21    	mul	x1, x1, x5
100037eb4: ab0b002b    	adds	x11, x1, x11
100037eb8: 9a8c358c    	cinc	x12, x12, hs
100037ebc: ab0401ad    	adds	x13, x13, x4
100037ec0: 9a8f35ef    	cinc	x15, x15, hs
100037ec4: 9bc57c61    	umulh	x1, x3, x5
100037ec8: 9b057c63    	mul	x3, x3, x5
100037ecc: ab0d006d    	adds	x13, x3, x13
100037ed0: 9a8f35ef    	cinc	x15, x15, hs
100037ed4: ab010129    	adds	x9, x9, x1
100037ed8: 9a883508    	cinc	x8, x8, hs
100037edc: 91004210    	add	x16, x16, #0x10
100037ee0: 91004231    	add	x17, x17, #0x10
100037ee4: f1000442    	subs	x2, x2, #0x1
100037ee8: 54fffc61    	b.ne	0x100037e74 <__RINvNtNtNtCsagpXUhzPPpl_17field_regressions8campaign10candidates5words13exact_columnsKj2_Kj5_EB8_+0x44>
100037eec: ab0b01cb    	adds	x11, x14, x11
100037ef0: 9a8c358c    	cinc	x12, x12, hs
100037ef4: ab0d018c    	adds	x12, x12, x13
100037ef8: 9a8f35ed    	cinc	x13, x15, hs
100037efc: ab0901a9    	adds	x9, x13, x9
100037f00: a9002c0a    	stp	x10, x11, [x0]
100037f04: 9a883508    	cinc	x8, x8, hs
100037f08: a901240c    	stp	x12, x9, [x0, #0x10]
100037f0c: f9001008    	str	x8, [x0, #0x20]
100037f10: a9417bfd    	ldp	x29, x30, [sp, #0x10]
100037f14: 910083ff    	add	sp, sp, #0x20
100037f18: d65f03c0    	ret
100037f1c: 90000bc4    	adrp	x4, 0x1001af000 <dyld_stub_binder+0x1001af000>
100037f20: 91370084    	add	x4, x4, #0xdc0
100037f24: 910003e0    	mov	x0, sp
100037f28: 910023e1    	add	x1, sp, #0x8
100037f2c: d2800002    	mov	x2, #0x0                ; =0
100037f30: 94048dc8    	bl	0x10015b650 <__RINvNtCs8Mbv00yxnRz_4core9panicking13assert_failedjjEB4_>

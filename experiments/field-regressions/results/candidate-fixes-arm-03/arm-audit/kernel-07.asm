
/private/var/folders/q0/v7l8dzd13k7f_q6j80sy03lm0000gn/T/f2z-field-gated-h_tggvge/field-regressions:	file format mach-o arm64

Disassembly of section __TEXT,__text:

0000000100043d7c <__RINvNtNtNtCsfDw8YqMpZPC_17field_regressions8campaign10candidates7integer7native4Kj4_Kj1_EB8_>:
100043d7c: d10103ff    	sub	sp, sp, #0x40
100043d80: a90157f6    	stp	x22, x21, [sp, #0x10]
100043d84: a9024ff4    	stp	x20, x19, [sp, #0x20]
100043d88: a9037bfd    	stp	x29, x30, [sp, #0x30]
100043d8c: 9100c3fd    	add	x29, sp, #0x30
100043d90: a90013e2    	stp	x2, x4, [sp]
100043d94: eb04005f    	cmp	x2, x4
100043d98: 54000841    	b.ne	0x100043ea0 <__RINvNtNtNtCsfDw8YqMpZPC_17field_regressions8campaign10candidates7integer7native4Kj4_Kj1_EB8_+0x124>
100043d9c: b40006c2    	cbz	x2, 0x100043e74 <__RINvNtNtNtCsfDw8YqMpZPC_17field_regressions8campaign10candidates7integer7native4Kj4_Kj1_EB8_+0xf8>
100043da0: d280000c    	mov	x12, #0x0               ; =0
100043da4: d280000d    	mov	x13, #0x0               ; =0
100043da8: d280000a    	mov	x10, #0x0               ; =0
100043dac: d280000b    	mov	x11, #0x0               ; =0
100043db0: 91004068    	add	x8, x3, #0x10
100043db4: 91004029    	add	x9, x1, #0x10
100043db8: aa0d03ee    	mov	x14, x13
100043dbc: aa0c03ef    	mov	x15, x12
100043dc0: a97f350c    	ldp	x12, x13, [x8, #-0x10]
100043dc4: a97f4530    	ldp	x16, x17, [x9, #-0x10]
100043dc8: a8c20d21    	ldp	x1, x3, [x9], #0x20
100043dcc: 9bcd7e24    	umulh	x4, x17, x13
100043dd0: 9b0d7e25    	mul	x5, x17, x13
100043dd4: 9bd07da6    	umulh	x6, x13, x16
100043dd8: 9b107da7    	mul	x7, x13, x16
100043ddc: 9b0c7e33    	mul	x19, x17, x12
100043de0: 9bcc7e34    	umulh	x20, x17, x12
100043de4: 9b107d95    	mul	x21, x12, x16
100043de8: 9bd07d96    	umulh	x22, x12, x16
100043dec: ab1600e7    	adds	x7, x7, x22
100043df0: 1a9f37f6    	cset	w22, hs
100043df4: ab0a00ca    	adds	x10, x6, x10
100043df8: 9a8b356b    	cinc	x11, x11, hs
100043dfc: ab05014a    	adds	x10, x10, x5
100043e00: 9a04016b    	adc	x11, x11, x4
100043e04: ab14014a    	adds	x10, x10, x20
100043e08: 9a8b356b    	cinc	x11, x11, hs
100043e0c: ab1300e4    	adds	x4, x7, x19
100043e10: a8c21905    	ldp	x5, x6, [x8], #0x20
100043e14: 9bd07ca7    	umulh	x7, x5, x16
100043e18: 9b111cb1    	madd	x17, x5, x17, x7
100043e1c: 9b1044d1    	madd	x17, x6, x16, x17
100043e20: 9b107cb0    	mul	x16, x5, x16
100043e24: 9bcc7c25    	umulh	x5, x1, x12
100043e28: 9b0d142d    	madd	x13, x1, x13, x5
100043e2c: 9b0c3463    	madd	x3, x3, x12, x13
100043e30: 9b0c7c21    	mul	x1, x1, x12
100043e34: ba16014a    	adcs	x10, x10, x22
100043e38: 9a8b356b    	cinc	x11, x11, hs
100043e3c: ab0f02ac    	adds	x12, x21, x15
100043e40: 9a0e008d    	adc	x13, x4, x14
100043e44: eb0f019f    	cmp	x12, x15
100043e48: fa0e01bf    	sbcs	xzr, x13, x14
100043e4c: 1a9f27ee    	cset	w14, lo
100043e50: ab01014a    	adds	x10, x10, x1
100043e54: 9a03016b    	adc	x11, x11, x3
100043e58: ab10014a    	adds	x10, x10, x16
100043e5c: 9a11016b    	adc	x11, x11, x17
100043e60: ab0e014a    	adds	x10, x10, x14
100043e64: 9a8b356b    	cinc	x11, x11, hs
100043e68: f1000442    	subs	x2, x2, #0x1
100043e6c: 54fffa61    	b.ne	0x100043db8 <__RINvNtNtNtCsfDw8YqMpZPC_17field_regressions8campaign10candidates7integer7native4Kj4_Kj1_EB8_+0x3c>
100043e70: 14000005    	b	0x100043e84 <__RINvNtNtNtCsfDw8YqMpZPC_17field_regressions8campaign10candidates7integer7native4Kj4_Kj1_EB8_+0x108>
100043e74: d280000a    	mov	x10, #0x0               ; =0
100043e78: d280000b    	mov	x11, #0x0               ; =0
100043e7c: d280000c    	mov	x12, #0x0               ; =0
100043e80: d280000d    	mov	x13, #0x0               ; =0
100043e84: a900340c    	stp	x12, x13, [x0]
100043e88: a9012c0a    	stp	x10, x11, [x0, #0x10]
100043e8c: a9437bfd    	ldp	x29, x30, [sp, #0x30]
100043e90: a9424ff4    	ldp	x20, x19, [sp, #0x20]
100043e94: a94157f6    	ldp	x22, x21, [sp, #0x10]
100043e98: 910103ff    	add	sp, sp, #0x40
100043e9c: d65f03c0    	ret
100043ea0: b0000b64    	adrp	x4, 0x1001b0000 <dyld_stub_binder+0x1001b0000>
100043ea4: 91186084    	add	x4, x4, #0x618
100043ea8: 910003e0    	mov	x0, sp
100043eac: 910023e1    	add	x1, sp, #0x8
100043eb0: d2800002    	mov	x2, #0x0                ; =0
100043eb4: 94046284    	bl	0x10015c8c4 <__RINvNtCs8Mbv00yxnRz_4core9panicking13assert_failedjjEB4_>

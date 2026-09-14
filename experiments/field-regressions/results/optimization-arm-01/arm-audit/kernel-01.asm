
/private/var/folders/q0/v7l8dzd13k7f_q6j80sy03lm0000gn/T/f2z-field-gated-6lkppdze/field-regressions:	file format mach-o arm64

Disassembly of section __TEXT,__text:

000000010002ff68 <__RINvNtNtNtCs8IeiyMGxmWr_17field_regressions8campaign10candidates7integer7native4Kj4_Kj1_EB8_>:
10002ff68: d10103ff    	sub	sp, sp, #0x40
10002ff6c: a90157f6    	stp	x22, x21, [sp, #0x10]
10002ff70: a9024ff4    	stp	x20, x19, [sp, #0x20]
10002ff74: a9037bfd    	stp	x29, x30, [sp, #0x30]
10002ff78: 9100c3fd    	add	x29, sp, #0x30
10002ff7c: a90013e2    	stp	x2, x4, [sp]
10002ff80: eb04005f    	cmp	x2, x4
10002ff84: 54000841    	b.ne	0x10003008c <__RINvNtNtNtCs8IeiyMGxmWr_17field_regressions8campaign10candidates7integer7native4Kj4_Kj1_EB8_+0x124>
10002ff88: b40006c2    	cbz	x2, 0x100030060 <__RINvNtNtNtCs8IeiyMGxmWr_17field_regressions8campaign10candidates7integer7native4Kj4_Kj1_EB8_+0xf8>
10002ff8c: d280000c    	mov	x12, #0x0               ; =0
10002ff90: d280000d    	mov	x13, #0x0               ; =0
10002ff94: d280000a    	mov	x10, #0x0               ; =0
10002ff98: d280000b    	mov	x11, #0x0               ; =0
10002ff9c: 91004068    	add	x8, x3, #0x10
10002ffa0: 91004029    	add	x9, x1, #0x10
10002ffa4: aa0d03ee    	mov	x14, x13
10002ffa8: aa0c03ef    	mov	x15, x12
10002ffac: a97f350c    	ldp	x12, x13, [x8, #-0x10]
10002ffb0: a97f4530    	ldp	x16, x17, [x9, #-0x10]
10002ffb4: a8c20d21    	ldp	x1, x3, [x9], #0x20
10002ffb8: 9bcd7e24    	umulh	x4, x17, x13
10002ffbc: 9b0d7e25    	mul	x5, x17, x13
10002ffc0: 9bd07da6    	umulh	x6, x13, x16
10002ffc4: 9b107da7    	mul	x7, x13, x16
10002ffc8: 9b0c7e33    	mul	x19, x17, x12
10002ffcc: 9bcc7e34    	umulh	x20, x17, x12
10002ffd0: 9b107d95    	mul	x21, x12, x16
10002ffd4: 9bd07d96    	umulh	x22, x12, x16
10002ffd8: ab1600e7    	adds	x7, x7, x22
10002ffdc: 1a9f37f6    	cset	w22, hs
10002ffe0: ab0a00ca    	adds	x10, x6, x10
10002ffe4: 9a8b356b    	cinc	x11, x11, hs
10002ffe8: ab05014a    	adds	x10, x10, x5
10002ffec: 9a04016b    	adc	x11, x11, x4
10002fff0: ab14014a    	adds	x10, x10, x20
10002fff4: 9a8b356b    	cinc	x11, x11, hs
10002fff8: ab1300e4    	adds	x4, x7, x19
10002fffc: a8c21905    	ldp	x5, x6, [x8], #0x20
100030000: 9bd07ca7    	umulh	x7, x5, x16
100030004: 9b111cb1    	madd	x17, x5, x17, x7
100030008: 9b1044d1    	madd	x17, x6, x16, x17
10003000c: 9b107cb0    	mul	x16, x5, x16
100030010: 9bcc7c25    	umulh	x5, x1, x12
100030014: 9b0d142d    	madd	x13, x1, x13, x5
100030018: 9b0c3463    	madd	x3, x3, x12, x13
10003001c: 9b0c7c21    	mul	x1, x1, x12
100030020: ba16014a    	adcs	x10, x10, x22
100030024: 9a8b356b    	cinc	x11, x11, hs
100030028: ab0f02ac    	adds	x12, x21, x15
10003002c: 9a0e008d    	adc	x13, x4, x14
100030030: eb0f019f    	cmp	x12, x15
100030034: fa0e01bf    	sbcs	xzr, x13, x14
100030038: 1a9f27ee    	cset	w14, lo
10003003c: ab01014a    	adds	x10, x10, x1
100030040: 9a03016b    	adc	x11, x11, x3
100030044: ab10014a    	adds	x10, x10, x16
100030048: 9a11016b    	adc	x11, x11, x17
10003004c: ab0e014a    	adds	x10, x10, x14
100030050: 9a8b356b    	cinc	x11, x11, hs
100030054: f1000442    	subs	x2, x2, #0x1
100030058: 54fffa61    	b.ne	0x10002ffa4 <__RINvNtNtNtCs8IeiyMGxmWr_17field_regressions8campaign10candidates7integer7native4Kj4_Kj1_EB8_+0x3c>
10003005c: 14000005    	b	0x100030070 <__RINvNtNtNtCs8IeiyMGxmWr_17field_regressions8campaign10candidates7integer7native4Kj4_Kj1_EB8_+0x108>
100030060: d280000a    	mov	x10, #0x0               ; =0
100030064: d280000b    	mov	x11, #0x0               ; =0
100030068: d280000c    	mov	x12, #0x0               ; =0
10003006c: d280000d    	mov	x13, #0x0               ; =0
100030070: a900340c    	stp	x12, x13, [x0]
100030074: a9012c0a    	stp	x10, x11, [x0, #0x10]
100030078: a9437bfd    	ldp	x29, x30, [sp, #0x30]
10003007c: a9424ff4    	ldp	x20, x19, [sp, #0x20]
100030080: a94157f6    	ldp	x22, x21, [sp, #0x10]
100030084: 910103ff    	add	sp, sp, #0x40
100030088: d65f03c0    	ret
10003008c: f0000a24    	adrp	x4, 0x100177000 <dyld_stub_binder+0x100177000>
100030090: 911da084    	add	x4, x4, #0x768
100030094: 910003e0    	mov	x0, sp
100030098: 910023e1    	add	x1, sp, #0x8
10003009c: d2800002    	mov	x2, #0x0                ; =0
1000300a0: 9403f1b3    	bl	0x10012c76c <__RINvNtCs8Mbv00yxnRz_4core9panicking13assert_failedjjEB4_>


/private/tmp/f2z-arithmetic-target/release/field-regressions:	file format mach-o arm64

Disassembly of section __TEXT,__text:

000000010004361c <__RINvNtNtNtCshgIkoLp8A2V_17field_regressions8campaign10candidates7integer7native4Kj4_Kj1_EB8_>:
10004361c: d10103ff    	sub	sp, sp, #0x40
100043620: a90157f6    	stp	x22, x21, [sp, #0x10]
100043624: a9024ff4    	stp	x20, x19, [sp, #0x20]
100043628: a9037bfd    	stp	x29, x30, [sp, #0x30]
10004362c: 9100c3fd    	add	x29, sp, #0x30
100043630: a90013e2    	stp	x2, x4, [sp]
100043634: eb04005f    	cmp	x2, x4
100043638: 54000841    	b.ne	0x100043740 <__RINvNtNtNtCshgIkoLp8A2V_17field_regressions8campaign10candidates7integer7native4Kj4_Kj1_EB8_+0x124>
10004363c: b40006c2    	cbz	x2, 0x100043714 <__RINvNtNtNtCshgIkoLp8A2V_17field_regressions8campaign10candidates7integer7native4Kj4_Kj1_EB8_+0xf8>
100043640: d280000c    	mov	x12, #0x0               ; =0
100043644: d280000d    	mov	x13, #0x0               ; =0
100043648: d280000a    	mov	x10, #0x0               ; =0
10004364c: d280000b    	mov	x11, #0x0               ; =0
100043650: 91004068    	add	x8, x3, #0x10
100043654: 91004029    	add	x9, x1, #0x10
100043658: aa0d03ee    	mov	x14, x13
10004365c: aa0c03ef    	mov	x15, x12
100043660: a97f350c    	ldp	x12, x13, [x8, #-0x10]
100043664: a97f4530    	ldp	x16, x17, [x9, #-0x10]
100043668: a8c20d21    	ldp	x1, x3, [x9], #0x20
10004366c: 9bcd7e24    	umulh	x4, x17, x13
100043670: 9b0d7e25    	mul	x5, x17, x13
100043674: 9bd07da6    	umulh	x6, x13, x16
100043678: 9b107da7    	mul	x7, x13, x16
10004367c: 9b0c7e33    	mul	x19, x17, x12
100043680: 9bcc7e34    	umulh	x20, x17, x12
100043684: 9b107d95    	mul	x21, x12, x16
100043688: 9bd07d96    	umulh	x22, x12, x16
10004368c: ab1600e7    	adds	x7, x7, x22
100043690: 1a9f37f6    	cset	w22, hs
100043694: ab0a00ca    	adds	x10, x6, x10
100043698: 9a8b356b    	cinc	x11, x11, hs
10004369c: ab05014a    	adds	x10, x10, x5
1000436a0: 9a04016b    	adc	x11, x11, x4
1000436a4: ab14014a    	adds	x10, x10, x20
1000436a8: 9a8b356b    	cinc	x11, x11, hs
1000436ac: ab1300e4    	adds	x4, x7, x19
1000436b0: a8c21905    	ldp	x5, x6, [x8], #0x20
1000436b4: 9bd07ca7    	umulh	x7, x5, x16
1000436b8: 9b111cb1    	madd	x17, x5, x17, x7
1000436bc: 9b1044d1    	madd	x17, x6, x16, x17
1000436c0: 9b107cb0    	mul	x16, x5, x16
1000436c4: 9bcc7c25    	umulh	x5, x1, x12
1000436c8: 9b0d142d    	madd	x13, x1, x13, x5
1000436cc: 9b0c3463    	madd	x3, x3, x12, x13
1000436d0: 9b0c7c21    	mul	x1, x1, x12
1000436d4: ba16014a    	adcs	x10, x10, x22
1000436d8: 9a8b356b    	cinc	x11, x11, hs
1000436dc: ab0f02ac    	adds	x12, x21, x15
1000436e0: 9a0e008d    	adc	x13, x4, x14
1000436e4: eb0f019f    	cmp	x12, x15
1000436e8: fa0e01bf    	sbcs	xzr, x13, x14
1000436ec: 1a9f27ee    	cset	w14, lo
1000436f0: ab01014a    	adds	x10, x10, x1
1000436f4: 9a03016b    	adc	x11, x11, x3
1000436f8: ab10014a    	adds	x10, x10, x16
1000436fc: 9a11016b    	adc	x11, x11, x17
100043700: ab0e014a    	adds	x10, x10, x14
100043704: 9a8b356b    	cinc	x11, x11, hs
100043708: f1000442    	subs	x2, x2, #0x1
10004370c: 54fffa61    	b.ne	0x100043658 <__RINvNtNtNtCshgIkoLp8A2V_17field_regressions8campaign10candidates7integer7native4Kj4_Kj1_EB8_+0x3c>
100043710: 14000005    	b	0x100043724 <__RINvNtNtNtCshgIkoLp8A2V_17field_regressions8campaign10candidates7integer7native4Kj4_Kj1_EB8_+0x108>
100043714: d280000a    	mov	x10, #0x0               ; =0
100043718: d280000b    	mov	x11, #0x0               ; =0
10004371c: d280000c    	mov	x12, #0x0               ; =0
100043720: d280000d    	mov	x13, #0x0               ; =0
100043724: a900340c    	stp	x12, x13, [x0]
100043728: a9012c0a    	stp	x10, x11, [x0, #0x10]
10004372c: a9437bfd    	ldp	x29, x30, [sp, #0x30]
100043730: a9424ff4    	ldp	x20, x19, [sp, #0x20]
100043734: a94157f6    	ldp	x22, x21, [sp, #0x10]
100043738: 910103ff    	add	sp, sp, #0x40
10004373c: d65f03c0    	ret
100043740: b0000b64    	adrp	x4, 0x1001b0000 <dyld_stub_binder+0x1001b0000>
100043744: 9115c084    	add	x4, x4, #0x570
100043748: 910003e0    	mov	x0, sp
10004374c: 910023e1    	add	x1, sp, #0x8
100043750: d2800002    	mov	x2, #0x0                ; =0
100043754: 94045fc8    	bl	0x10015b674 <__RINvNtCs8Mbv00yxnRz_4core9panicking13assert_failedjjEB4_>

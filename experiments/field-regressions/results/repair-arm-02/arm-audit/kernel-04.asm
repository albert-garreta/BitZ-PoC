
/private/var/folders/q0/v7l8dzd13k7f_q6j80sy03lm0000gn/T/f2z-field-gated-aejimxw9/field-regressions:	file format mach-o arm64

Disassembly of section __TEXT,__text:

000000010003f45c <__RINvNtNtNtCsaHyC9lX8wyC_17field_regressions8campaign10candidates7integer7native4Kj4_Kj1_EB8_>:
10003f45c: d10103ff    	sub	sp, sp, #0x40
10003f460: a90157f6    	stp	x22, x21, [sp, #0x10]
10003f464: a9024ff4    	stp	x20, x19, [sp, #0x20]
10003f468: a9037bfd    	stp	x29, x30, [sp, #0x30]
10003f46c: 9100c3fd    	add	x29, sp, #0x30
10003f470: a90013e2    	stp	x2, x4, [sp]
10003f474: eb04005f    	cmp	x2, x4
10003f478: 54000841    	b.ne	0x10003f580 <__RINvNtNtNtCsaHyC9lX8wyC_17field_regressions8campaign10candidates7integer7native4Kj4_Kj1_EB8_+0x124>
10003f47c: b40006c2    	cbz	x2, 0x10003f554 <__RINvNtNtNtCsaHyC9lX8wyC_17field_regressions8campaign10candidates7integer7native4Kj4_Kj1_EB8_+0xf8>
10003f480: d280000c    	mov	x12, #0x0               ; =0
10003f484: d280000d    	mov	x13, #0x0               ; =0
10003f488: d280000a    	mov	x10, #0x0               ; =0
10003f48c: d280000b    	mov	x11, #0x0               ; =0
10003f490: 91004068    	add	x8, x3, #0x10
10003f494: 91004029    	add	x9, x1, #0x10
10003f498: aa0d03ee    	mov	x14, x13
10003f49c: aa0c03ef    	mov	x15, x12
10003f4a0: a97f350c    	ldp	x12, x13, [x8, #-0x10]
10003f4a4: a97f4530    	ldp	x16, x17, [x9, #-0x10]
10003f4a8: a8c20d21    	ldp	x1, x3, [x9], #0x20
10003f4ac: 9bcd7e24    	umulh	x4, x17, x13
10003f4b0: 9b0d7e25    	mul	x5, x17, x13
10003f4b4: 9bd07da6    	umulh	x6, x13, x16
10003f4b8: 9b107da7    	mul	x7, x13, x16
10003f4bc: 9b0c7e33    	mul	x19, x17, x12
10003f4c0: 9bcc7e34    	umulh	x20, x17, x12
10003f4c4: 9b107d95    	mul	x21, x12, x16
10003f4c8: 9bd07d96    	umulh	x22, x12, x16
10003f4cc: ab1600e7    	adds	x7, x7, x22
10003f4d0: 1a9f37f6    	cset	w22, hs
10003f4d4: ab0a00ca    	adds	x10, x6, x10
10003f4d8: 9a8b356b    	cinc	x11, x11, hs
10003f4dc: ab05014a    	adds	x10, x10, x5
10003f4e0: 9a04016b    	adc	x11, x11, x4
10003f4e4: ab14014a    	adds	x10, x10, x20
10003f4e8: 9a8b356b    	cinc	x11, x11, hs
10003f4ec: ab1300e4    	adds	x4, x7, x19
10003f4f0: a8c21905    	ldp	x5, x6, [x8], #0x20
10003f4f4: 9bd07ca7    	umulh	x7, x5, x16
10003f4f8: 9b111cb1    	madd	x17, x5, x17, x7
10003f4fc: 9b1044d1    	madd	x17, x6, x16, x17
10003f500: 9b107cb0    	mul	x16, x5, x16
10003f504: 9bcc7c25    	umulh	x5, x1, x12
10003f508: 9b0d142d    	madd	x13, x1, x13, x5
10003f50c: 9b0c3463    	madd	x3, x3, x12, x13
10003f510: 9b0c7c21    	mul	x1, x1, x12
10003f514: ba16014a    	adcs	x10, x10, x22
10003f518: 9a8b356b    	cinc	x11, x11, hs
10003f51c: ab0f02ac    	adds	x12, x21, x15
10003f520: 9a0e008d    	adc	x13, x4, x14
10003f524: eb0f019f    	cmp	x12, x15
10003f528: fa0e01bf    	sbcs	xzr, x13, x14
10003f52c: 1a9f27ee    	cset	w14, lo
10003f530: ab01014a    	adds	x10, x10, x1
10003f534: 9a03016b    	adc	x11, x11, x3
10003f538: ab10014a    	adds	x10, x10, x16
10003f53c: 9a11016b    	adc	x11, x11, x17
10003f540: ab0e014a    	adds	x10, x10, x14
10003f544: 9a8b356b    	cinc	x11, x11, hs
10003f548: f1000442    	subs	x2, x2, #0x1
10003f54c: 54fffa61    	b.ne	0x10003f498 <__RINvNtNtNtCsaHyC9lX8wyC_17field_regressions8campaign10candidates7integer7native4Kj4_Kj1_EB8_+0x3c>
10003f550: 14000005    	b	0x10003f564 <__RINvNtNtNtCsaHyC9lX8wyC_17field_regressions8campaign10candidates7integer7native4Kj4_Kj1_EB8_+0x108>
10003f554: d280000a    	mov	x10, #0x0               ; =0
10003f558: d280000b    	mov	x11, #0x0               ; =0
10003f55c: d280000c    	mov	x12, #0x0               ; =0
10003f560: d280000d    	mov	x13, #0x0               ; =0
10003f564: a900340c    	stp	x12, x13, [x0]
10003f568: a9012c0a    	stp	x10, x11, [x0, #0x10]
10003f56c: a9437bfd    	ldp	x29, x30, [sp, #0x30]
10003f570: a9424ff4    	ldp	x20, x19, [sp, #0x20]
10003f574: a94157f6    	ldp	x22, x21, [sp, #0x10]
10003f578: 910103ff    	add	sp, sp, #0x40
10003f57c: d65f03c0    	ret
10003f580: b0000ae4    	adrp	x4, 0x10019c000 <dyld_stub_binder+0x10019c000>
10003f584: 9100e084    	add	x4, x4, #0x38
10003f588: 910003e0    	mov	x0, sp
10003f58c: 910023e1    	add	x1, sp, #0x8
10003f590: d2800002    	mov	x2, #0x0                ; =0
10003f594: 940430f5    	bl	0x10014b968 <__RINvNtCs8Mbv00yxnRz_4core9panicking13assert_failedjjEB4_>


/private/var/folders/q0/v7l8dzd13k7f_q6j80sy03lm0000gn/T/f2z-field-gated-h_tggvge/field-regressions:	file format mach-o arm64

Disassembly of section __TEXT,__text:

00000001000384b4 <__RINvNtNtNtCsfDw8YqMpZPC_17field_regressions8campaign10candidates5words13exact_columnsKj2_Kj5_EB8_>:
1000384b4: d10083ff    	sub	sp, sp, #0x20
1000384b8: a9017bfd    	stp	x29, x30, [sp, #0x10]
1000384bc: 910043fd    	add	x29, sp, #0x10
1000384c0: a90013e2    	stp	x2, x4, [sp]
1000384c4: eb04005f    	cmp	x2, x4
1000384c8: 540006c1    	b.ne	0x1000385a0 <__RINvNtNtNtCsfDw8YqMpZPC_17field_regressions8campaign10candidates5words13exact_columnsKj2_Kj5_EB8_+0xec>
1000384cc: d280000b    	mov	x11, #0x0               ; =0
1000384d0: d280000c    	mov	x12, #0x0               ; =0
1000384d4: d280000d    	mov	x13, #0x0               ; =0
1000384d8: d280000f    	mov	x15, #0x0               ; =0
1000384dc: d2800009    	mov	x9, #0x0                ; =0
1000384e0: d2800008    	mov	x8, #0x0                ; =0
1000384e4: d280000a    	mov	x10, #0x0               ; =0
1000384e8: d280000e    	mov	x14, #0x0               ; =0
1000384ec: b4000422    	cbz	x2, 0x100038570 <__RINvNtNtNtCsfDw8YqMpZPC_17field_regressions8campaign10candidates5words13exact_columnsKj2_Kj5_EB8_+0xbc>
1000384f0: 91002030    	add	x16, x1, #0x8
1000384f4: 91002071    	add	x17, x3, #0x8
1000384f8: a97f8e21    	ldp	x1, x3, [x17, #-0x8]
1000384fc: a97f9604    	ldp	x4, x5, [x16, #-0x8]
100038500: 9b047c26    	mul	x6, x1, x4
100038504: ab0a00ca    	adds	x10, x6, x10
100038508: 9bc47c26    	umulh	x6, x1, x4
10003850c: 9a8e35ce    	cinc	x14, x14, hs
100038510: ab06016b    	adds	x11, x11, x6
100038514: 9a8c358c    	cinc	x12, x12, hs
100038518: 9b047c66    	mul	x6, x3, x4
10003851c: ab0b00cb    	adds	x11, x6, x11
100038520: 9bc47c64    	umulh	x4, x3, x4
100038524: 9a8c358c    	cinc	x12, x12, hs
100038528: ab0401ad    	adds	x13, x13, x4
10003852c: 9a8f35ef    	cinc	x15, x15, hs
100038530: 9bc57c24    	umulh	x4, x1, x5
100038534: 9b057c21    	mul	x1, x1, x5
100038538: ab0b002b    	adds	x11, x1, x11
10003853c: 9a8c358c    	cinc	x12, x12, hs
100038540: ab0401ad    	adds	x13, x13, x4
100038544: 9a8f35ef    	cinc	x15, x15, hs
100038548: 9bc57c61    	umulh	x1, x3, x5
10003854c: 9b057c63    	mul	x3, x3, x5
100038550: ab0d006d    	adds	x13, x3, x13
100038554: 9a8f35ef    	cinc	x15, x15, hs
100038558: ab010129    	adds	x9, x9, x1
10003855c: 9a883508    	cinc	x8, x8, hs
100038560: 91004210    	add	x16, x16, #0x10
100038564: 91004231    	add	x17, x17, #0x10
100038568: f1000442    	subs	x2, x2, #0x1
10003856c: 54fffc61    	b.ne	0x1000384f8 <__RINvNtNtNtCsfDw8YqMpZPC_17field_regressions8campaign10candidates5words13exact_columnsKj2_Kj5_EB8_+0x44>
100038570: ab0b01cb    	adds	x11, x14, x11
100038574: 9a8c358c    	cinc	x12, x12, hs
100038578: ab0d018c    	adds	x12, x12, x13
10003857c: 9a8f35ed    	cinc	x13, x15, hs
100038580: ab0901a9    	adds	x9, x13, x9
100038584: a9002c0a    	stp	x10, x11, [x0]
100038588: 9a883508    	cinc	x8, x8, hs
10003858c: a901240c    	stp	x12, x9, [x0, #0x10]
100038590: f9001008    	str	x8, [x0, #0x20]
100038594: a9417bfd    	ldp	x29, x30, [sp, #0x10]
100038598: 910083ff    	add	sp, sp, #0x20
10003859c: d65f03c0    	ret
1000385a0: f0000ba4    	adrp	x4, 0x1001af000 <dyld_stub_binder+0x1001af000>
1000385a4: 91370084    	add	x4, x4, #0xdc0
1000385a8: 910003e0    	mov	x0, sp
1000385ac: 910023e1    	add	x1, sp, #0x8
1000385b0: d2800002    	mov	x2, #0x0                ; =0
1000385b4: 940490c4    	bl	0x10015c8c4 <__RINvNtCs8Mbv00yxnRz_4core9panicking13assert_failedjjEB4_>

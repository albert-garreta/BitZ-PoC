
/private/var/folders/q0/v7l8dzd13k7f_q6j80sy03lm0000gn/T/f2z-field-gated-aejimxw9/field-regressions:	file format mach-o arm64

Disassembly of section __TEXT,__text:

0000000100037004 <__RINvNtNtNtCsaHyC9lX8wyC_17field_regressions8campaign10candidates5words13exact_columnsKj2_Kj5_EB8_>:
100037004: d10083ff    	sub	sp, sp, #0x20
100037008: a9017bfd    	stp	x29, x30, [sp, #0x10]
10003700c: 910043fd    	add	x29, sp, #0x10
100037010: a90013e2    	stp	x2, x4, [sp]
100037014: eb04005f    	cmp	x2, x4
100037018: 540006c1    	b.ne	0x1000370f0 <__RINvNtNtNtCsaHyC9lX8wyC_17field_regressions8campaign10candidates5words13exact_columnsKj2_Kj5_EB8_+0xec>
10003701c: d280000b    	mov	x11, #0x0               ; =0
100037020: d280000c    	mov	x12, #0x0               ; =0
100037024: d280000d    	mov	x13, #0x0               ; =0
100037028: d280000f    	mov	x15, #0x0               ; =0
10003702c: d2800009    	mov	x9, #0x0                ; =0
100037030: d2800008    	mov	x8, #0x0                ; =0
100037034: d280000a    	mov	x10, #0x0               ; =0
100037038: d280000e    	mov	x14, #0x0               ; =0
10003703c: b4000422    	cbz	x2, 0x1000370c0 <__RINvNtNtNtCsaHyC9lX8wyC_17field_regressions8campaign10candidates5words13exact_columnsKj2_Kj5_EB8_+0xbc>
100037040: 91002030    	add	x16, x1, #0x8
100037044: 91002071    	add	x17, x3, #0x8
100037048: a97f8e21    	ldp	x1, x3, [x17, #-0x8]
10003704c: a97f9604    	ldp	x4, x5, [x16, #-0x8]
100037050: 9b047c26    	mul	x6, x1, x4
100037054: ab0a00ca    	adds	x10, x6, x10
100037058: 9bc47c26    	umulh	x6, x1, x4
10003705c: 9a8e35ce    	cinc	x14, x14, hs
100037060: ab06016b    	adds	x11, x11, x6
100037064: 9a8c358c    	cinc	x12, x12, hs
100037068: 9b047c66    	mul	x6, x3, x4
10003706c: ab0b00cb    	adds	x11, x6, x11
100037070: 9bc47c64    	umulh	x4, x3, x4
100037074: 9a8c358c    	cinc	x12, x12, hs
100037078: ab0401ad    	adds	x13, x13, x4
10003707c: 9a8f35ef    	cinc	x15, x15, hs
100037080: 9bc57c24    	umulh	x4, x1, x5
100037084: 9b057c21    	mul	x1, x1, x5
100037088: ab0b002b    	adds	x11, x1, x11
10003708c: 9a8c358c    	cinc	x12, x12, hs
100037090: ab0401ad    	adds	x13, x13, x4
100037094: 9a8f35ef    	cinc	x15, x15, hs
100037098: 9bc57c61    	umulh	x1, x3, x5
10003709c: 9b057c63    	mul	x3, x3, x5
1000370a0: ab0d006d    	adds	x13, x3, x13
1000370a4: 9a8f35ef    	cinc	x15, x15, hs
1000370a8: ab010129    	adds	x9, x9, x1
1000370ac: 9a883508    	cinc	x8, x8, hs
1000370b0: 91004210    	add	x16, x16, #0x10
1000370b4: 91004231    	add	x17, x17, #0x10
1000370b8: f1000442    	subs	x2, x2, #0x1
1000370bc: 54fffc61    	b.ne	0x100037048 <__RINvNtNtNtCsaHyC9lX8wyC_17field_regressions8campaign10candidates5words13exact_columnsKj2_Kj5_EB8_+0x44>
1000370c0: ab0b01cb    	adds	x11, x14, x11
1000370c4: 9a8c358c    	cinc	x12, x12, hs
1000370c8: ab0d018c    	adds	x12, x12, x13
1000370cc: 9a8f35ed    	cinc	x13, x15, hs
1000370d0: ab0901a9    	adds	x9, x13, x9
1000370d4: a9002c0a    	stp	x10, x11, [x0]
1000370d8: 9a883508    	cinc	x8, x8, hs
1000370dc: a901240c    	stp	x12, x9, [x0, #0x10]
1000370e0: f9001008    	str	x8, [x0, #0x20]
1000370e4: a9417bfd    	ldp	x29, x30, [sp, #0x10]
1000370e8: 910083ff    	add	sp, sp, #0x20
1000370ec: d65f03c0    	ret
1000370f0: 90000b24    	adrp	x4, 0x10019b000 <dyld_stub_binder+0x10019b000>
1000370f4: 912fa084    	add	x4, x4, #0xbe8
1000370f8: 910003e0    	mov	x0, sp
1000370fc: 910023e1    	add	x1, sp, #0x8
100037100: d2800002    	mov	x2, #0x0                ; =0
100037104: 94045219    	bl	0x10014b968 <__RINvNtCs8Mbv00yxnRz_4core9panicking13assert_failedjjEB4_>

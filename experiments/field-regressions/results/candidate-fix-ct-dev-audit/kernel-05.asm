
/private/tmp/f2z-arithmetic-target/release/field-regressions:	file format mach-o arm64

Disassembly of section __TEXT,__text:

0000000100038790 <__RINvNtNtNtCshgIkoLp8A2V_17field_regressions8campaign10candidates5words20exact_signed_columnsKj4_Kj9_EB8_>:
100038790: d10283ff    	sub	sp, sp, #0xa0
100038794: a9046ffc    	stp	x28, x27, [sp, #0x40]
100038798: a90567fa    	stp	x26, x25, [sp, #0x50]
10003879c: a9065ff8    	stp	x24, x23, [sp, #0x60]
1000387a0: a90757f6    	stp	x22, x21, [sp, #0x70]
1000387a4: a9084ff4    	stp	x20, x19, [sp, #0x80]
1000387a8: a9097bfd    	stp	x29, x30, [sp, #0x90]
1000387ac: 910243fd    	add	x29, sp, #0x90
1000387b0: a90313e2    	stp	x2, x4, [sp, #0x30]
1000387b4: eb04005f    	cmp	x2, x4
1000387b8: 54001d61    	b.ne	0x100038b64 <__RINvNtNtNtCshgIkoLp8A2V_17field_regressions8campaign10candidates5words20exact_signed_columnsKj4_Kj9_EB8_+0x3d4>
1000387bc: f90007e0    	str	x0, [sp, #0x8]
1000387c0: b4001682    	cbz	x2, 0x100038a90 <__RINvNtNtNtCshgIkoLp8A2V_17field_regressions8campaign10candidates5words20exact_signed_columnsKj4_Kj9_EB8_+0x300>
1000387c4: d2800011    	mov	x17, #0x0               ; =0
1000387c8: 9100406c    	add	x12, x3, #0x10
1000387cc: d2800008    	mov	x8, #0x0                ; =0
1000387d0: d2800009    	mov	x9, #0x0                ; =0
1000387d4: d280000d    	mov	x13, #0x0               ; =0
1000387d8: d280000e    	mov	x14, #0x0               ; =0
1000387dc: d2800006    	mov	x6, #0x0                ; =0
1000387e0: d2800005    	mov	x5, #0x0                ; =0
1000387e4: d2800007    	mov	x7, #0x0                ; =0
1000387e8: d2800013    	mov	x19, #0x0               ; =0
1000387ec: d2800018    	mov	x24, #0x0               ; =0
1000387f0: d2800019    	mov	x25, #0x0               ; =0
1000387f4: d2800017    	mov	x23, #0x0               ; =0
1000387f8: d2800016    	mov	x22, #0x0               ; =0
1000387fc: d2800014    	mov	x20, #0x0               ; =0
100038800: d2800015    	mov	x21, #0x0               ; =0
100038804: d2800004    	mov	x4, #0x0                ; =0
100038808: d2800003    	mov	x3, #0x0                ; =0
10003880c: d280000a    	mov	x10, #0x0               ; =0
100038810: f9000bff    	str	xzr, [sp, #0x10]
100038814: f90017e2    	str	x2, [sp, #0x28]
100038818: a97f719b    	ldp	x27, x28, [x12, #-0x10]
10003881c: a9406820    	ldp	x0, x26, [x1]
100038820: 9bc07f6f    	umulh	x15, x27, x0
100038824: 9b007f70    	mul	x16, x27, x0
100038828: ab080208    	adds	x8, x16, x8
10003882c: 9a893522    	cinc	x2, x9, hs
100038830: ab0f01ad    	adds	x13, x13, x15
100038834: 9a8e35ce    	cinc	x14, x14, hs
100038838: 9bc07f8f    	umulh	x15, x28, x0
10003883c: 9b007f90    	mul	x16, x28, x0
100038840: ab0d020d    	adds	x13, x16, x13
100038844: 9a8e35ce    	cinc	x14, x14, hs
100038848: ab0f00c6    	adds	x6, x6, x15
10003884c: a9403d90    	ldp	x16, x15, [x12]
100038850: 9bc07e1e    	umulh	x30, x16, x0
100038854: 9a8534a5    	cinc	x5, x5, hs
100038858: 9b007e0b    	mul	x11, x16, x0
10003885c: ab06016b    	adds	x11, x11, x6
100038860: 9a8534a5    	cinc	x5, x5, hs
100038864: ab1e00e6    	adds	x6, x7, x30
100038868: 9bc07de7    	umulh	x7, x15, x0
10003886c: 9a933673    	cinc	x19, x19, hs
100038870: 9b007de0    	mul	x0, x15, x0
100038874: ab060000    	adds	x0, x0, x6
100038878: 9a933666    	cinc	x6, x19, hs
10003887c: ab070307    	adds	x7, x24, x7
100038880: 9bda7f73    	umulh	x19, x27, x26
100038884: 9a993738    	cinc	x24, x25, hs
100038888: 9b1a7f79    	mul	x25, x27, x26
10003888c: ab0d032d    	adds	x13, x25, x13
100038890: f90013ed    	str	x13, [sp, #0x20]
100038894: 9a8e35cd    	cinc	x13, x14, hs
100038898: f9000fed    	str	x13, [sp, #0x18]
10003889c: ab13016b    	adds	x11, x11, x19
1000388a0: 9bda7f93    	umulh	x19, x28, x26
1000388a4: 9a8534a5    	cinc	x5, x5, hs
1000388a8: 9b1a7f99    	mul	x25, x28, x26
1000388ac: ab0b032b    	adds	x11, x25, x11
1000388b0: 9a8534a5    	cinc	x5, x5, hs
1000388b4: ab130000    	adds	x0, x0, x19
1000388b8: 9bda7e13    	umulh	x19, x16, x26
1000388bc: 9a8634c6    	cinc	x6, x6, hs
1000388c0: 9b1a7e19    	mul	x25, x16, x26
1000388c4: ab000339    	adds	x25, x25, x0
1000388c8: 9a8634cd    	cinc	x13, x6, hs
1000388cc: ab1300e0    	adds	x0, x7, x19
1000388d0: 9bda7de6    	umulh	x6, x15, x26
1000388d4: 9a983707    	cinc	x7, x24, hs
1000388d8: 9b1a7df3    	mul	x19, x15, x26
1000388dc: ab000273    	adds	x19, x19, x0
1000388e0: 9a8734e7    	cinc	x7, x7, hs
1000388e4: ab0602f7    	adds	x23, x23, x6
1000388e8: a941003e    	ldp	x30, x0, [x1, #0x10]
1000388ec: 9a9636d6    	cinc	x22, x22, hs
1000388f0: 9bde7f78    	umulh	x24, x27, x30
1000388f4: 9b1e7f66    	mul	x6, x27, x30
1000388f8: aa0a03e9    	mov	x9, x10
1000388fc: aa0803ea    	mov	x10, x8
100038900: ab0b00c8    	adds	x8, x6, x11
100038904: 9a8534a6    	cinc	x6, x5, hs
100038908: ab18032b    	adds	x11, x25, x24
10003890c: 9a8d35ad    	cinc	x13, x13, hs
100038910: 9bde7f98    	umulh	x24, x28, x30
100038914: 9b1e7f99    	mul	x25, x28, x30
100038918: ab0b032b    	adds	x11, x25, x11
10003891c: 9a8d35ad    	cinc	x13, x13, hs
100038920: ab180273    	adds	x19, x19, x24
100038924: 9a8734e7    	cinc	x7, x7, hs
100038928: 9bde7e18    	umulh	x24, x16, x30
10003892c: 9b1e7e19    	mul	x25, x16, x30
100038930: ab130339    	adds	x25, x25, x19
100038934: 9a8734ee    	cinc	x14, x7, hs
100038938: ab1802e7    	adds	x7, x23, x24
10003893c: 9a9636d3    	cinc	x19, x22, hs
100038940: 9bde7df6    	umulh	x22, x15, x30
100038944: 9b1e7df7    	mul	x23, x15, x30
100038948: ab0702f7    	adds	x23, x23, x7
10003894c: 9a933678    	cinc	x24, x19, hs
100038950: ab160294    	adds	x20, x20, x22
100038954: 9a9536b5    	cinc	x21, x21, hs
100038958: 9bc07f76    	umulh	x22, x27, x0
10003895c: 9b007f67    	mul	x7, x27, x0
100038960: ab0b00e7    	adds	x7, x7, x11
100038964: 9a8d35b3    	cinc	x19, x13, hs
100038968: ab16032b    	adds	x11, x25, x22
10003896c: 9a8e35cd    	cinc	x13, x14, hs
100038970: 9bc07f8e    	umulh	x14, x28, x0
100038974: 9b007f96    	mul	x22, x28, x0
100038978: ab0b02cb    	adds	x11, x22, x11
10003897c: 9a8d35ad    	cinc	x13, x13, hs
100038980: ab0e02ee    	adds	x14, x23, x14
100038984: 9a983716    	cinc	x22, x24, hs
100038988: 9bc07e17    	umulh	x23, x16, x0
10003898c: 9b007e10    	mul	x16, x16, x0
100038990: ab0e020e    	adds	x14, x16, x14
100038994: 9a9636d6    	cinc	x22, x22, hs
100038998: ab170290    	adds	x16, x20, x23
10003899c: 9a9536b5    	cinc	x21, x21, hs
1000389a0: 9bc07df7    	umulh	x23, x15, x0
1000389a4: 9b007df4    	mul	x20, x15, x0
1000389a8: ab100294    	adds	x20, x20, x16
1000389ac: 9a9536b5    	cinc	x21, x21, hs
1000389b0: ab170084    	adds	x4, x4, x23
1000389b4: d37ffc10    	lsr	x16, x0, #63
1000389b8: d37ffdef    	lsr	x15, x15, #63
1000389bc: 3900e3f0    	strb	w16, [sp, #0x38]
1000389c0: 9100e3e5    	add	x5, sp, #0x38
1000389c4: 3940e3f7    	ldrb	w23, [sp, #0x38]
1000389c8: 9a833463    	cinc	x3, x3, hs
1000389cc: f9400bfc    	ldr	x28, [sp, #0x10]
1000389d0: aa1c03fb    	mov	x27, x28
1000389d4: 3900e3ef    	strb	w15, [sp, #0x38]
1000389d8: 3940e3f8    	ldrb	w24, [sp, #0x38]
1000389dc: 92800005    	mov	x5, #-0x1               ; =-1
1000389e0: f2401eff    	tst	x23, #0xff
1000389e4: 9a9c10bb    	csel	x27, x5, x28, ne
1000389e8: f2401f1f    	tst	x24, #0xff
1000389ec: 9a9c10bc    	csel	x28, x5, x28, ne
1000389f0: a97f1597    	ldp	x23, x5, [x12, #-0x10]
1000389f4: 8a1b02f7    	and	x23, x23, x27
1000389f8: f8420438    	ldr	x24, [x1], #0x20
1000389fc: 8a1c0318    	and	x24, x24, x28
100038a00: ab1802f7    	adds	x23, x23, x24
100038a04: 1a9f37f9    	cset	w25, hs
100038a08: eb170178    	subs	x24, x11, x23
100038a0c: da1901b9    	sbc	x25, x13, x25
100038a10: 8a1b00ab    	and	x11, x5, x27
100038a14: aa0603e5    	mov	x5, x6
100038a18: aa0803e6    	mov	x6, x8
100038a1c: aa0a03e8    	mov	x8, x10
100038a20: aa0903ea    	mov	x10, x9
100038a24: aa0203e9    	mov	x9, x2
100038a28: f94017e2    	ldr	x2, [sp, #0x28]
100038a2c: 8a1c034d    	and	x13, x26, x28
100038a30: ab0d016b    	adds	x11, x11, x13
100038a34: 1a9f37ed    	cset	w13, hs
100038a38: eb0b01d7    	subs	x23, x14, x11
100038a3c: da0d02d6    	sbc	x22, x22, x13
100038a40: a8c2358b    	ldp	x11, x13, [x12], #0x20
100038a44: 8a1b016b    	and	x11, x11, x27
100038a48: 8a1c03ce    	and	x14, x30, x28
100038a4c: ab0e016b    	adds	x11, x11, x14
100038a50: 1a9f37ee    	cset	w14, hs
100038a54: eb0b0294    	subs	x20, x20, x11
100038a58: da0e02b5    	sbc	x21, x21, x14
100038a5c: 8a1b01ab    	and	x11, x13, x27
100038a60: 8a1c000d    	and	x13, x0, x28
100038a64: ab0d016b    	adds	x11, x11, x13
100038a68: 1a9f37ed    	cset	w13, hs
100038a6c: eb0b0084    	subs	x4, x4, x11
100038a70: da0d0063    	sbc	x3, x3, x13
100038a74: a941b7ee    	ldp	x14, x13, [sp, #0x18]
100038a78: 8a1001eb    	and	x11, x15, x16
100038a7c: ab0b014a    	adds	x10, x10, x11
100038a80: 9a913631    	cinc	x17, x17, hs
100038a84: f1000442    	subs	x2, x2, #0x1
100038a88: 54ffec61    	b.ne	0x100038814 <__RINvNtNtNtCshgIkoLp8A2V_17field_regressions8campaign10candidates5words20exact_signed_columnsKj4_Kj9_EB8_+0x84>
100038a8c: 14000012    	b	0x100038ad4 <__RINvNtNtNtCshgIkoLp8A2V_17field_regressions8campaign10candidates5words20exact_signed_columnsKj4_Kj9_EB8_+0x344>
100038a90: d2800008    	mov	x8, #0x0                ; =0
100038a94: d2800009    	mov	x9, #0x0                ; =0
100038a98: d280000d    	mov	x13, #0x0               ; =0
100038a9c: d280000e    	mov	x14, #0x0               ; =0
100038aa0: d2800006    	mov	x6, #0x0                ; =0
100038aa4: d2800005    	mov	x5, #0x0                ; =0
100038aa8: d2800007    	mov	x7, #0x0                ; =0
100038aac: d2800013    	mov	x19, #0x0               ; =0
100038ab0: d2800018    	mov	x24, #0x0               ; =0
100038ab4: d2800019    	mov	x25, #0x0               ; =0
100038ab8: d2800017    	mov	x23, #0x0               ; =0
100038abc: d2800016    	mov	x22, #0x0               ; =0
100038ac0: d2800014    	mov	x20, #0x0               ; =0
100038ac4: d2800015    	mov	x21, #0x0               ; =0
100038ac8: d2800004    	mov	x4, #0x0                ; =0
100038acc: d2800003    	mov	x3, #0x0                ; =0
100038ad0: d280000a    	mov	x10, #0x0               ; =0
100038ad4: 937ffd2b    	asr	x11, x9, #63
100038ad8: ab0d0129    	adds	x9, x9, x13
100038adc: 9a0e016b    	adc	x11, x11, x14
100038ae0: 937ffd6c    	asr	x12, x11, #63
100038ae4: ab06016b    	adds	x11, x11, x6
100038ae8: 9a05018c    	adc	x12, x12, x5
100038aec: 937ffd8d    	asr	x13, x12, #63
100038af0: ab07018c    	adds	x12, x12, x7
100038af4: 9a1301ad    	adc	x13, x13, x19
100038af8: 937ffdae    	asr	x14, x13, #63
100038afc: ab1801ad    	adds	x13, x13, x24
100038b00: 9a1901ce    	adc	x14, x14, x25
100038b04: 937ffdcf    	asr	x15, x14, #63
100038b08: ab1701ce    	adds	x14, x14, x23
100038b0c: 9a1601ef    	adc	x15, x15, x22
100038b10: 937ffdf0    	asr	x16, x15, #63
100038b14: ab1401ef    	adds	x15, x15, x20
100038b18: 9a150210    	adc	x16, x16, x21
100038b1c: 937ffe11    	asr	x17, x16, #63
100038b20: f94007e0    	ldr	x0, [sp, #0x8]
100038b24: a9002408    	stp	x8, x9, [x0]
100038b28: ab040208    	adds	x8, x16, x4
100038b2c: a901300b    	stp	x11, x12, [x0, #0x10]
100038b30: 9a030229    	adc	x9, x17, x3
100038b34: a902380d    	stp	x13, x14, [x0, #0x20]
100038b38: 8b0a0129    	add	x9, x9, x10
100038b3c: a903200f    	stp	x15, x8, [x0, #0x30]
100038b40: f9002009    	str	x9, [x0, #0x40]
100038b44: a9497bfd    	ldp	x29, x30, [sp, #0x90]
100038b48: a9484ff4    	ldp	x20, x19, [sp, #0x80]
100038b4c: a94757f6    	ldp	x22, x21, [sp, #0x70]
100038b50: a9465ff8    	ldp	x24, x23, [sp, #0x60]
100038b54: a94567fa    	ldp	x26, x25, [sp, #0x50]
100038b58: a9446ffc    	ldp	x28, x27, [sp, #0x40]
100038b5c: 910283ff    	add	sp, sp, #0xa0
100038b60: d65f03c0    	ret
100038b64: f0000ba4    	adrp	x4, 0x1001af000 <dyld_stub_binder+0x1001af000>
100038b68: 91376084    	add	x4, x4, #0xdd8
100038b6c: 9100c3e0    	add	x0, sp, #0x30
100038b70: 9100e3e1    	add	x1, sp, #0x38
100038b74: d2800002    	mov	x2, #0x0                ; =0
100038b78: 94048abf    	bl	0x10015b674 <__RINvNtCs8Mbv00yxnRz_4core9panicking13assert_failedjjEB4_>

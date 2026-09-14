
/private/var/folders/q0/v7l8dzd13k7f_q6j80sy03lm0000gn/T/f2z-field-gated-pycl0nhx/field-regressions:	file format mach-o arm64

Disassembly of section __TEXT,__text:

0000000100038758 <__RINvNtNtNtCsagpXUhzPPpl_17field_regressions8campaign10candidates5words20exact_signed_columnsKj4_Kj9_EB8_>:
100038758: d10243ff    	sub	sp, sp, #0x90
10003875c: a9036ffc    	stp	x28, x27, [sp, #0x30]
100038760: a90467fa    	stp	x26, x25, [sp, #0x40]
100038764: a9055ff8    	stp	x24, x23, [sp, #0x50]
100038768: a90657f6    	stp	x22, x21, [sp, #0x60]
10003876c: a9074ff4    	stp	x20, x19, [sp, #0x70]
100038770: a9087bfd    	stp	x29, x30, [sp, #0x80]
100038774: 910203fd    	add	x29, sp, #0x80
100038778: a90213e2    	stp	x2, x4, [sp, #0x20]
10003877c: eb04005f    	cmp	x2, x4
100038780: 540019e1    	b.ne	0x100038abc <__RINvNtNtNtCsagpXUhzPPpl_17field_regressions8campaign10candidates5words20exact_signed_columnsKj4_Kj9_EB8_+0x364>
100038784: f90007e0    	str	x0, [sp, #0x8]
100038788: d280001e    	mov	x30, #0x0               ; =0
10003878c: d280000b    	mov	x11, #0x0               ; =0
100038790: d280000d    	mov	x13, #0x0               ; =0
100038794: d280000e    	mov	x14, #0x0               ; =0
100038798: d2800004    	mov	x4, #0x0                ; =0
10003879c: d2800005    	mov	x5, #0x0                ; =0
1000387a0: d2800013    	mov	x19, #0x0               ; =0
1000387a4: d2800014    	mov	x20, #0x0               ; =0
1000387a8: d2800015    	mov	x21, #0x0               ; =0
1000387ac: d2800016    	mov	x22, #0x0               ; =0
1000387b0: d2800006    	mov	x6, #0x0                ; =0
1000387b4: d2800007    	mov	x7, #0x0                ; =0
1000387b8: d280000f    	mov	x15, #0x0               ; =0
1000387bc: d2800010    	mov	x16, #0x0               ; =0
1000387c0: d280000c    	mov	x12, #0x0               ; =0
1000387c4: b40012e2    	cbz	x2, 0x100038a20 <__RINvNtNtNtCsagpXUhzPPpl_17field_regressions8campaign10candidates5words20exact_signed_columnsKj4_Kj9_EB8_+0x2c8>
1000387c8: f9000fff    	str	xzr, [sp, #0x18]
1000387cc: d280000a    	mov	x10, #0x0               ; =0
1000387d0: d2800011    	mov	x17, #0x0               ; =0
1000387d4: 14000021    	b	0x100038858 <__RINvNtNtNtCsagpXUhzPPpl_17field_regressions8campaign10candidates5words20exact_signed_columnsKj4_Kj9_EB8_+0x100>
1000387d8: f9400068    	ldr	x8, [x3]
1000387dc: f9400029    	ldr	x9, [x1]
1000387e0: 937ffee0    	asr	x0, x23, #63
1000387e4: 8a090009    	and	x9, x0, x9
1000387e8: ab090108    	adds	x8, x8, x9
1000387ec: 1a9f37e9    	cset	w9, hs
1000387f0: eb0802b5    	subs	x21, x21, x8
1000387f4: da0902d6    	sbc	x22, x22, x9
1000387f8: 8a1a0008    	and	x8, x0, x26
1000387fc: ab080328    	adds	x8, x25, x8
100038800: 1a9f37e9    	cset	w9, hs
100038804: eb0800c6    	subs	x6, x6, x8
100038808: da0900e7    	sbc	x7, x7, x9
10003880c: 8a1e0008    	and	x8, x0, x30
100038810: ab080368    	adds	x8, x27, x8
100038814: 1a9f37e9    	cset	w9, hs
100038818: eb0801ef    	subs	x15, x15, x8
10003881c: da090210    	sbc	x16, x16, x9
100038820: 8a180008    	and	x8, x0, x24
100038824: ab0802e8    	adds	x8, x23, x8
100038828: 1a9f37e9    	cset	w9, hs
10003882c: eb08018c    	subs	x12, x12, x8
100038830: da09039c    	sbc	x28, x28, x9
100038834: f9000ffc    	str	x28, [sp, #0x18]
100038838: 91008063    	add	x3, x3, #0x20
10003883c: 8a1802e8    	and	x8, x23, x24
100038840: ab48fd4a    	adds	x10, x10, x8, lsr #63
100038844: 9a913631    	cinc	x17, x17, hs
100038848: 91008021    	add	x1, x1, #0x20
10003884c: f1000442    	subs	x2, x2, #0x1
100038850: f9400bfe    	ldr	x30, [sp, #0x10]
100038854: 54000ea0    	b.eq	0x100038a28 <__RINvNtNtNtCsagpXUhzPPpl_17field_regressions8campaign10candidates5words20exact_signed_columnsKj4_Kj9_EB8_+0x2d0>
100038858: a940647c    	ldp	x28, x25, [x3]
10003885c: a9406838    	ldp	x24, x26, [x1]
100038860: 9bd87f97    	umulh	x23, x28, x24
100038864: 9b187f9b    	mul	x27, x28, x24
100038868: ab1e037e    	adds	x30, x27, x30
10003886c: f9000bfe    	str	x30, [sp, #0x10]
100038870: 9a8b356b    	cinc	x11, x11, hs
100038874: ab1701ad    	adds	x13, x13, x23
100038878: 9a8e35ce    	cinc	x14, x14, hs
10003887c: 9bd87f37    	umulh	x23, x25, x24
100038880: 9b187f3b    	mul	x27, x25, x24
100038884: ab0d036d    	adds	x13, x27, x13
100038888: 9a8e35ce    	cinc	x14, x14, hs
10003888c: ab170084    	adds	x4, x4, x23
100038890: 9a8534a5    	cinc	x5, x5, hs
100038894: a9415c7b    	ldp	x27, x23, [x3, #0x10]
100038898: 9bd87f7e    	umulh	x30, x27, x24
10003889c: 9b187f68    	mul	x8, x27, x24
1000388a0: ab040108    	adds	x8, x8, x4
1000388a4: 9a8534a4    	cinc	x4, x5, hs
1000388a8: ab1e0265    	adds	x5, x19, x30
1000388ac: 9a943693    	cinc	x19, x20, hs
1000388b0: 9bd87ef4    	umulh	x20, x23, x24
1000388b4: 9b187ef8    	mul	x24, x23, x24
1000388b8: ab050305    	adds	x5, x24, x5
1000388bc: 9a933673    	cinc	x19, x19, hs
1000388c0: ab1402b4    	adds	x20, x21, x20
1000388c4: 9a9636d5    	cinc	x21, x22, hs
1000388c8: 9bda7f96    	umulh	x22, x28, x26
1000388cc: 9b1a7f98    	mul	x24, x28, x26
1000388d0: ab0d030d    	adds	x13, x24, x13
1000388d4: 9a8e35ce    	cinc	x14, x14, hs
1000388d8: ab160108    	adds	x8, x8, x22
1000388dc: 9a843484    	cinc	x4, x4, hs
1000388e0: 9bda7f36    	umulh	x22, x25, x26
1000388e4: 9b1a7f38    	mul	x24, x25, x26
1000388e8: ab080308    	adds	x8, x24, x8
1000388ec: 9a843480    	cinc	x0, x4, hs
1000388f0: ab1600a4    	adds	x4, x5, x22
1000388f4: 9a933665    	cinc	x5, x19, hs
1000388f8: 9bda7f73    	umulh	x19, x27, x26
1000388fc: 9b1a7f76    	mul	x22, x27, x26
100038900: ab0402d6    	adds	x22, x22, x4
100038904: 9a8534a9    	cinc	x9, x5, hs
100038908: ab130284    	adds	x4, x20, x19
10003890c: 9a9536a5    	cinc	x5, x21, hs
100038910: 9bda7ef3    	umulh	x19, x23, x26
100038914: 9b1a7ef4    	mul	x20, x23, x26
100038918: ab040294    	adds	x20, x20, x4
10003891c: 9a8534b5    	cinc	x21, x5, hs
100038920: ab1300c6    	adds	x6, x6, x19
100038924: 9a8734e7    	cinc	x7, x7, hs
100038928: a941603e    	ldp	x30, x24, [x1, #0x10]
10003892c: 9b1e7f84    	mul	x4, x28, x30
100038930: ab080084    	adds	x4, x4, x8
100038934: 9bde7f88    	umulh	x8, x28, x30
100038938: 9a803405    	cinc	x5, x0, hs
10003893c: ab0802c8    	adds	x8, x22, x8
100038940: 9a893529    	cinc	x9, x9, hs
100038944: 9b1e7f20    	mul	x0, x25, x30
100038948: ab080008    	adds	x8, x0, x8
10003894c: 9bde7f20    	umulh	x0, x25, x30
100038950: 9a893529    	cinc	x9, x9, hs
100038954: ab000280    	adds	x0, x20, x0
100038958: 9a9536b3    	cinc	x19, x21, hs
10003895c: 9b1e7f74    	mul	x20, x27, x30
100038960: ab000280    	adds	x0, x20, x0
100038964: 9bde7f74    	umulh	x20, x27, x30
100038968: 9a933675    	cinc	x21, x19, hs
10003896c: ab1400c6    	adds	x6, x6, x20
100038970: 9a8734e7    	cinc	x7, x7, hs
100038974: 9b1e7ef3    	mul	x19, x23, x30
100038978: ab060266    	adds	x6, x19, x6
10003897c: 9bde7ef3    	umulh	x19, x23, x30
100038980: 9a8734e7    	cinc	x7, x7, hs
100038984: ab1301ef    	adds	x15, x15, x19
100038988: 9a903610    	cinc	x16, x16, hs
10003898c: 9bd87f96    	umulh	x22, x28, x24
100038990: 9b187f93    	mul	x19, x28, x24
100038994: ab080273    	adds	x19, x19, x8
100038998: 9a893534    	cinc	x20, x9, hs
10003899c: ab160008    	adds	x8, x0, x22
1000389a0: 9a9536a9    	cinc	x9, x21, hs
1000389a4: 9bd87f20    	umulh	x0, x25, x24
1000389a8: 9b187f35    	mul	x21, x25, x24
1000389ac: ab0802b5    	adds	x21, x21, x8
1000389b0: 9a893536    	cinc	x22, x9, hs
1000389b4: ab0000c8    	adds	x8, x6, x0
1000389b8: 9a8734e9    	cinc	x9, x7, hs
1000389bc: 9bd87f60    	umulh	x0, x27, x24
1000389c0: 9b187f66    	mul	x6, x27, x24
1000389c4: ab0800c6    	adds	x6, x6, x8
1000389c8: 9a893527    	cinc	x7, x9, hs
1000389cc: ab0001e8    	adds	x8, x15, x0
1000389d0: 9a903609    	cinc	x9, x16, hs
1000389d4: 9bd87ee0    	umulh	x0, x23, x24
1000389d8: 9b187eef    	mul	x15, x23, x24
1000389dc: ab0801ef    	adds	x15, x15, x8
1000389e0: 9a893530    	cinc	x16, x9, hs
1000389e4: ab00018c    	adds	x12, x12, x0
1000389e8: f9400ffc    	ldr	x28, [sp, #0x18]
1000389ec: 9a9c379c    	cinc	x28, x28, hs
1000389f0: b7ffef58    	tbnz	x24, #0x3f, 0x1000387d8 <__RINvNtNtNtCsagpXUhzPPpl_17field_regressions8campaign10candidates5words20exact_signed_columnsKj4_Kj9_EB8_+0x80>
1000389f4: b6fff217    	tbz	x23, #0x3f, 0x100038834 <__RINvNtNtNtCsagpXUhzPPpl_17field_regressions8campaign10candidates5words20exact_signed_columnsKj4_Kj9_EB8_+0xdc>
1000389f8: f9400028    	ldr	x8, [x1]
1000389fc: eb0802b5    	subs	x21, x21, x8
100038a00: da1f02d6    	sbc	x22, x22, xzr
100038a04: eb1a00c6    	subs	x6, x6, x26
100038a08: da1f00e7    	sbc	x7, x7, xzr
100038a0c: eb1e01ef    	subs	x15, x15, x30
100038a10: da1f0210    	sbc	x16, x16, xzr
100038a14: eb18018c    	subs	x12, x12, x24
100038a18: da1f039c    	sbc	x28, x28, xzr
100038a1c: 17ffff86    	b	0x100038834 <__RINvNtNtNtCsagpXUhzPPpl_17field_regressions8campaign10candidates5words20exact_signed_columnsKj4_Kj9_EB8_+0xdc>
100038a20: f9000fff    	str	xzr, [sp, #0x18]
100038a24: d280000a    	mov	x10, #0x0               ; =0
100038a28: 937ffd68    	asr	x8, x11, #63
100038a2c: ab0d0169    	adds	x9, x11, x13
100038a30: 9a0e0108    	adc	x8, x8, x14
100038a34: 937ffd0d    	asr	x13, x8, #63
100038a38: ab040108    	adds	x8, x8, x4
100038a3c: 9a0501ad    	adc	x13, x13, x5
100038a40: 937ffdae    	asr	x14, x13, #63
100038a44: ab1301ad    	adds	x13, x13, x19
100038a48: 9a1401ce    	adc	x14, x14, x20
100038a4c: 937ffdd1    	asr	x17, x14, #63
100038a50: ab1501ce    	adds	x14, x14, x21
100038a54: 9a160231    	adc	x17, x17, x22
100038a58: 937ffe20    	asr	x0, x17, #63
100038a5c: ab060231    	adds	x17, x17, x6
100038a60: 9a070000    	adc	x0, x0, x7
100038a64: 937ffc01    	asr	x1, x0, #63
100038a68: ab0f000f    	adds	x15, x0, x15
100038a6c: 9a100030    	adc	x16, x1, x16
100038a70: 937ffe00    	asr	x0, x16, #63
100038a74: f94007e1    	ldr	x1, [sp, #0x8]
100038a78: a900243e    	stp	x30, x9, [x1]
100038a7c: ab0c0209    	adds	x9, x16, x12
100038a80: a9013428    	stp	x8, x13, [x1, #0x10]
100038a84: f9400fe8    	ldr	x8, [sp, #0x18]
100038a88: 9a080008    	adc	x8, x0, x8
100038a8c: a902442e    	stp	x14, x17, [x1, #0x20]
100038a90: 8b0a0108    	add	x8, x8, x10
100038a94: a903242f    	stp	x15, x9, [x1, #0x30]
100038a98: f9002028    	str	x8, [x1, #0x40]
100038a9c: a9487bfd    	ldp	x29, x30, [sp, #0x80]
100038aa0: a9474ff4    	ldp	x20, x19, [sp, #0x70]
100038aa4: a94657f6    	ldp	x22, x21, [sp, #0x60]
100038aa8: a9455ff8    	ldp	x24, x23, [sp, #0x50]
100038aac: a94467fa    	ldp	x26, x25, [sp, #0x40]
100038ab0: a9436ffc    	ldp	x28, x27, [sp, #0x30]
100038ab4: 910243ff    	add	sp, sp, #0x90
100038ab8: d65f03c0    	ret
100038abc: f0000ba4    	adrp	x4, 0x1001af000 <dyld_stub_binder+0x1001af000>
100038ac0: 91376084    	add	x4, x4, #0xdd8
100038ac4: 910083e0    	add	x0, sp, #0x20
100038ac8: 9100a3e1    	add	x1, sp, #0x28
100038acc: d2800002    	mov	x2, #0x0                ; =0
100038ad0: 94048ae0    	bl	0x10015b650 <__RINvNtCs8Mbv00yxnRz_4core9panicking13assert_failedjjEB4_>

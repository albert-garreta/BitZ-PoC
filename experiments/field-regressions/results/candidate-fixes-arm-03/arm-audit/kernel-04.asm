
/private/var/folders/q0/v7l8dzd13k7f_q6j80sy03lm0000gn/T/f2z-field-gated-h_tggvge/field-regressions:	file format mach-o arm64

Disassembly of section __TEXT,__text:

0000000100038c38 <__RINvNtNtNtCsfDw8YqMpZPC_17field_regressions8campaign10candidates5words20exact_signed_columnsKj2_Kj5_EB8_>:
100038c38: d10183ff    	sub	sp, sp, #0x60
100038c3c: a90167fa    	stp	x26, x25, [sp, #0x10]
100038c40: a9025ff8    	stp	x24, x23, [sp, #0x20]
100038c44: a90357f6    	stp	x22, x21, [sp, #0x30]
100038c48: a9044ff4    	stp	x20, x19, [sp, #0x40]
100038c4c: a9057bfd    	stp	x29, x30, [sp, #0x50]
100038c50: 910143fd    	add	x29, sp, #0x50
100038c54: a90013e2    	stp	x2, x4, [sp]
100038c58: eb04005f    	cmp	x2, x4
100038c5c: 54000d01    	b.ne	0x100038dfc <__RINvNtNtNtCsfDw8YqMpZPC_17field_regressions8campaign10candidates5words20exact_signed_columnsKj2_Kj5_EB8_+0x1c4>
100038c60: b4000942    	cbz	x2, 0x100038d88 <__RINvNtNtNtCsfDw8YqMpZPC_17field_regressions8campaign10candidates5words20exact_signed_columnsKj2_Kj5_EB8_+0x150>
100038c64: d280000b    	mov	x11, #0x0               ; =0
100038c68: 9100202c    	add	x12, x1, #0x8
100038c6c: 9100206d    	add	x13, x3, #0x8
100038c70: 910023ee    	add	x14, sp, #0x8
100038c74: 92800011    	mov	x17, #-0x1              ; =-1
100038c78: d2800008    	mov	x8, #0x0                ; =0
100038c7c: d2800009    	mov	x9, #0x0                ; =0
100038c80: d280000f    	mov	x15, #0x0               ; =0
100038c84: d2800010    	mov	x16, #0x0               ; =0
100038c88: d2800005    	mov	x5, #0x0                ; =0
100038c8c: d2800006    	mov	x6, #0x0                ; =0
100038c90: d2800003    	mov	x3, #0x0                ; =0
100038c94: d2800004    	mov	x4, #0x0                ; =0
100038c98: d280000a    	mov	x10, #0x0               ; =0
100038c9c: d2800001    	mov	x1, #0x0                ; =0
100038ca0: a97fcda7    	ldp	x7, x19, [x13, #-0x8]
100038ca4: a97fd594    	ldp	x20, x21, [x12, #-0x8]
100038ca8: 9bd47cf6    	umulh	x22, x7, x20
100038cac: 9b147cf7    	mul	x23, x7, x20
100038cb0: ab0802e8    	adds	x8, x23, x8
100038cb4: 9a893529    	cinc	x9, x9, hs
100038cb8: ab1601ef    	adds	x15, x15, x22
100038cbc: 9a903610    	cinc	x16, x16, hs
100038cc0: 9bd47e76    	umulh	x22, x19, x20
100038cc4: 9b147e77    	mul	x23, x19, x20
100038cc8: ab0f02ef    	adds	x15, x23, x15
100038ccc: 9a903610    	cinc	x16, x16, hs
100038cd0: ab1600a5    	adds	x5, x5, x22
100038cd4: 9a8634c6    	cinc	x6, x6, hs
100038cd8: 9bd57cf6    	umulh	x22, x7, x21
100038cdc: 9b157cf7    	mul	x23, x7, x21
100038ce0: ab0f02ef    	adds	x15, x23, x15
100038ce4: 9a903610    	cinc	x16, x16, hs
100038ce8: ab1600a5    	adds	x5, x5, x22
100038cec: 9a8634c6    	cinc	x6, x6, hs
100038cf0: 9bd57e76    	umulh	x22, x19, x21
100038cf4: 9b157e77    	mul	x23, x19, x21
100038cf8: ab0502e5    	adds	x5, x23, x5
100038cfc: 9a8634c6    	cinc	x6, x6, hs
100038d00: ab160063    	adds	x3, x3, x22
100038d04: 9a843484    	cinc	x4, x4, hs
100038d08: d37ffeb6    	lsr	x22, x21, #63
100038d0c: d37ffe77    	lsr	x23, x19, #63
100038d10: 390023f6    	strb	w22, [sp, #0x8]
100038d14: 394023f8    	ldrb	w24, [sp, #0x8]
100038d18: aa0b03f9    	mov	x25, x11
100038d1c: f2401f1f    	tst	x24, #0xff
100038d20: 9a8b1239    	csel	x25, x17, x11, ne
100038d24: 390023f7    	strb	w23, [sp, #0x8]
100038d28: 394023f8    	ldrb	w24, [sp, #0x8]
100038d2c: aa0b03fa    	mov	x26, x11
100038d30: f2401f1f    	tst	x24, #0xff
100038d34: 9a8b123a    	csel	x26, x17, x11, ne
100038d38: 8a1900e7    	and	x7, x7, x25
100038d3c: 8a1a0294    	and	x20, x20, x26
100038d40: ab1400e7    	adds	x7, x7, x20
100038d44: 1a9f37f4    	cset	w20, hs
100038d48: eb0700a5    	subs	x5, x5, x7
100038d4c: da1400c6    	sbc	x6, x6, x20
100038d50: 8a190267    	and	x7, x19, x25
100038d54: 8a1a02b3    	and	x19, x21, x26
100038d58: ab1300e7    	adds	x7, x7, x19
100038d5c: 1a9f37f3    	cset	w19, hs
100038d60: eb070063    	subs	x3, x3, x7
100038d64: da130084    	sbc	x4, x4, x19
100038d68: 8a1602e7    	and	x7, x23, x22
100038d6c: ab07014a    	adds	x10, x10, x7
100038d70: 9a813421    	cinc	x1, x1, hs
100038d74: 9100418c    	add	x12, x12, #0x10
100038d78: 910041ad    	add	x13, x13, #0x10
100038d7c: f1000442    	subs	x2, x2, #0x1
100038d80: 54fff901    	b.ne	0x100038ca0 <__RINvNtNtNtCsfDw8YqMpZPC_17field_regressions8campaign10candidates5words20exact_signed_columnsKj2_Kj5_EB8_+0x68>
100038d84: 1400000a    	b	0x100038dac <__RINvNtNtNtCsfDw8YqMpZPC_17field_regressions8campaign10candidates5words20exact_signed_columnsKj2_Kj5_EB8_+0x174>
100038d88: d2800008    	mov	x8, #0x0                ; =0
100038d8c: d2800009    	mov	x9, #0x0                ; =0
100038d90: d280000f    	mov	x15, #0x0               ; =0
100038d94: d2800010    	mov	x16, #0x0               ; =0
100038d98: d2800005    	mov	x5, #0x0                ; =0
100038d9c: d2800006    	mov	x6, #0x0                ; =0
100038da0: d2800003    	mov	x3, #0x0                ; =0
100038da4: d2800004    	mov	x4, #0x0                ; =0
100038da8: d280000a    	mov	x10, #0x0               ; =0
100038dac: 937ffd2b    	asr	x11, x9, #63
100038db0: ab0f0129    	adds	x9, x9, x15
100038db4: 9a10016b    	adc	x11, x11, x16
100038db8: 937ffd6c    	asr	x12, x11, #63
100038dbc: ab05016b    	adds	x11, x11, x5
100038dc0: 9a06018c    	adc	x12, x12, x6
100038dc4: 937ffd8d    	asr	x13, x12, #63
100038dc8: ab03018c    	adds	x12, x12, x3
100038dcc: 9a0401ad    	adc	x13, x13, x4
100038dd0: a9002408    	stp	x8, x9, [x0]
100038dd4: 8b0a01a8    	add	x8, x13, x10
100038dd8: a901300b    	stp	x11, x12, [x0, #0x10]
100038ddc: f9001008    	str	x8, [x0, #0x20]
100038de0: a9457bfd    	ldp	x29, x30, [sp, #0x50]
100038de4: a9444ff4    	ldp	x20, x19, [sp, #0x40]
100038de8: a94357f6    	ldp	x22, x21, [sp, #0x30]
100038dec: a9425ff8    	ldp	x24, x23, [sp, #0x20]
100038df0: a94167fa    	ldp	x26, x25, [sp, #0x10]
100038df4: 910183ff    	add	sp, sp, #0x60
100038df8: d65f03c0    	ret
100038dfc: f0000ba4    	adrp	x4, 0x1001af000 <dyld_stub_binder+0x1001af000>
100038e00: 91376084    	add	x4, x4, #0xdd8
100038e04: 910003e0    	mov	x0, sp
100038e08: 910023e1    	add	x1, sp, #0x8
100038e0c: d2800002    	mov	x2, #0x0                ; =0
100038e10: 94048ead    	bl	0x10015c8c4 <__RINvNtCs8Mbv00yxnRz_4core9panicking13assert_failedjjEB4_>

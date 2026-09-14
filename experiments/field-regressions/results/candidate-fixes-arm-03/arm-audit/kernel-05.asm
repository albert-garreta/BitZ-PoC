
/private/var/folders/q0/v7l8dzd13k7f_q6j80sy03lm0000gn/T/f2z-field-gated-h_tggvge/field-regressions:	file format mach-o arm64

Disassembly of section __TEXT,__text:

0000000100038e14 <__RINvNtNtNtCsfDw8YqMpZPC_17field_regressions8campaign10candidates5words20exact_signed_columnsKj4_Kj9_EB8_>:
100038e14: d10283ff    	sub	sp, sp, #0xa0
100038e18: a9046ffc    	stp	x28, x27, [sp, #0x40]
100038e1c: a90567fa    	stp	x26, x25, [sp, #0x50]
100038e20: a9065ff8    	stp	x24, x23, [sp, #0x60]
100038e24: a90757f6    	stp	x22, x21, [sp, #0x70]
100038e28: a9084ff4    	stp	x20, x19, [sp, #0x80]
100038e2c: a9097bfd    	stp	x29, x30, [sp, #0x90]
100038e30: 910243fd    	add	x29, sp, #0x90
100038e34: a90313e2    	stp	x2, x4, [sp, #0x30]
100038e38: eb04005f    	cmp	x2, x4
100038e3c: 54001d61    	b.ne	0x1000391e8 <__RINvNtNtNtCsfDw8YqMpZPC_17field_regressions8campaign10candidates5words20exact_signed_columnsKj4_Kj9_EB8_+0x3d4>
100038e40: f90007e0    	str	x0, [sp, #0x8]
100038e44: b4001682    	cbz	x2, 0x100039114 <__RINvNtNtNtCsfDw8YqMpZPC_17field_regressions8campaign10candidates5words20exact_signed_columnsKj4_Kj9_EB8_+0x300>
100038e48: d2800011    	mov	x17, #0x0               ; =0
100038e4c: 9100406c    	add	x12, x3, #0x10
100038e50: d2800008    	mov	x8, #0x0                ; =0
100038e54: d2800009    	mov	x9, #0x0                ; =0
100038e58: d280000d    	mov	x13, #0x0               ; =0
100038e5c: d280000e    	mov	x14, #0x0               ; =0
100038e60: d2800006    	mov	x6, #0x0                ; =0
100038e64: d2800005    	mov	x5, #0x0                ; =0
100038e68: d2800007    	mov	x7, #0x0                ; =0
100038e6c: d2800013    	mov	x19, #0x0               ; =0
100038e70: d2800018    	mov	x24, #0x0               ; =0
100038e74: d2800019    	mov	x25, #0x0               ; =0
100038e78: d2800017    	mov	x23, #0x0               ; =0
100038e7c: d2800016    	mov	x22, #0x0               ; =0
100038e80: d2800014    	mov	x20, #0x0               ; =0
100038e84: d2800015    	mov	x21, #0x0               ; =0
100038e88: d2800004    	mov	x4, #0x0                ; =0
100038e8c: d2800003    	mov	x3, #0x0                ; =0
100038e90: d280000a    	mov	x10, #0x0               ; =0
100038e94: f9000bff    	str	xzr, [sp, #0x10]
100038e98: f90017e2    	str	x2, [sp, #0x28]
100038e9c: a97f719b    	ldp	x27, x28, [x12, #-0x10]
100038ea0: a9406820    	ldp	x0, x26, [x1]
100038ea4: 9bc07f6f    	umulh	x15, x27, x0
100038ea8: 9b007f70    	mul	x16, x27, x0
100038eac: ab080208    	adds	x8, x16, x8
100038eb0: 9a893522    	cinc	x2, x9, hs
100038eb4: ab0f01ad    	adds	x13, x13, x15
100038eb8: 9a8e35ce    	cinc	x14, x14, hs
100038ebc: 9bc07f8f    	umulh	x15, x28, x0
100038ec0: 9b007f90    	mul	x16, x28, x0
100038ec4: ab0d020d    	adds	x13, x16, x13
100038ec8: 9a8e35ce    	cinc	x14, x14, hs
100038ecc: ab0f00c6    	adds	x6, x6, x15
100038ed0: a9403d90    	ldp	x16, x15, [x12]
100038ed4: 9bc07e1e    	umulh	x30, x16, x0
100038ed8: 9a8534a5    	cinc	x5, x5, hs
100038edc: 9b007e0b    	mul	x11, x16, x0
100038ee0: ab06016b    	adds	x11, x11, x6
100038ee4: 9a8534a5    	cinc	x5, x5, hs
100038ee8: ab1e00e6    	adds	x6, x7, x30
100038eec: 9bc07de7    	umulh	x7, x15, x0
100038ef0: 9a933673    	cinc	x19, x19, hs
100038ef4: 9b007de0    	mul	x0, x15, x0
100038ef8: ab060000    	adds	x0, x0, x6
100038efc: 9a933666    	cinc	x6, x19, hs
100038f00: ab070307    	adds	x7, x24, x7
100038f04: 9bda7f73    	umulh	x19, x27, x26
100038f08: 9a993738    	cinc	x24, x25, hs
100038f0c: 9b1a7f79    	mul	x25, x27, x26
100038f10: ab0d032d    	adds	x13, x25, x13
100038f14: f90013ed    	str	x13, [sp, #0x20]
100038f18: 9a8e35cd    	cinc	x13, x14, hs
100038f1c: f9000fed    	str	x13, [sp, #0x18]
100038f20: ab13016b    	adds	x11, x11, x19
100038f24: 9bda7f93    	umulh	x19, x28, x26
100038f28: 9a8534a5    	cinc	x5, x5, hs
100038f2c: 9b1a7f99    	mul	x25, x28, x26
100038f30: ab0b032b    	adds	x11, x25, x11
100038f34: 9a8534a5    	cinc	x5, x5, hs
100038f38: ab130000    	adds	x0, x0, x19
100038f3c: 9bda7e13    	umulh	x19, x16, x26
100038f40: 9a8634c6    	cinc	x6, x6, hs
100038f44: 9b1a7e19    	mul	x25, x16, x26
100038f48: ab000339    	adds	x25, x25, x0
100038f4c: 9a8634cd    	cinc	x13, x6, hs
100038f50: ab1300e0    	adds	x0, x7, x19
100038f54: 9bda7de6    	umulh	x6, x15, x26
100038f58: 9a983707    	cinc	x7, x24, hs
100038f5c: 9b1a7df3    	mul	x19, x15, x26
100038f60: ab000273    	adds	x19, x19, x0
100038f64: 9a8734e7    	cinc	x7, x7, hs
100038f68: ab0602f7    	adds	x23, x23, x6
100038f6c: a941003e    	ldp	x30, x0, [x1, #0x10]
100038f70: 9a9636d6    	cinc	x22, x22, hs
100038f74: 9bde7f78    	umulh	x24, x27, x30
100038f78: 9b1e7f66    	mul	x6, x27, x30
100038f7c: aa0a03e9    	mov	x9, x10
100038f80: aa0803ea    	mov	x10, x8
100038f84: ab0b00c8    	adds	x8, x6, x11
100038f88: 9a8534a6    	cinc	x6, x5, hs
100038f8c: ab18032b    	adds	x11, x25, x24
100038f90: 9a8d35ad    	cinc	x13, x13, hs
100038f94: 9bde7f98    	umulh	x24, x28, x30
100038f98: 9b1e7f99    	mul	x25, x28, x30
100038f9c: ab0b032b    	adds	x11, x25, x11
100038fa0: 9a8d35ad    	cinc	x13, x13, hs
100038fa4: ab180273    	adds	x19, x19, x24
100038fa8: 9a8734e7    	cinc	x7, x7, hs
100038fac: 9bde7e18    	umulh	x24, x16, x30
100038fb0: 9b1e7e19    	mul	x25, x16, x30
100038fb4: ab130339    	adds	x25, x25, x19
100038fb8: 9a8734ee    	cinc	x14, x7, hs
100038fbc: ab1802e7    	adds	x7, x23, x24
100038fc0: 9a9636d3    	cinc	x19, x22, hs
100038fc4: 9bde7df6    	umulh	x22, x15, x30
100038fc8: 9b1e7df7    	mul	x23, x15, x30
100038fcc: ab0702f7    	adds	x23, x23, x7
100038fd0: 9a933678    	cinc	x24, x19, hs
100038fd4: ab160294    	adds	x20, x20, x22
100038fd8: 9a9536b5    	cinc	x21, x21, hs
100038fdc: 9bc07f76    	umulh	x22, x27, x0
100038fe0: 9b007f67    	mul	x7, x27, x0
100038fe4: ab0b00e7    	adds	x7, x7, x11
100038fe8: 9a8d35b3    	cinc	x19, x13, hs
100038fec: ab16032b    	adds	x11, x25, x22
100038ff0: 9a8e35cd    	cinc	x13, x14, hs
100038ff4: 9bc07f8e    	umulh	x14, x28, x0
100038ff8: 9b007f96    	mul	x22, x28, x0
100038ffc: ab0b02cb    	adds	x11, x22, x11
100039000: 9a8d35ad    	cinc	x13, x13, hs
100039004: ab0e02ee    	adds	x14, x23, x14
100039008: 9a983716    	cinc	x22, x24, hs
10003900c: 9bc07e17    	umulh	x23, x16, x0
100039010: 9b007e10    	mul	x16, x16, x0
100039014: ab0e020e    	adds	x14, x16, x14
100039018: 9a9636d6    	cinc	x22, x22, hs
10003901c: ab170290    	adds	x16, x20, x23
100039020: 9a9536b5    	cinc	x21, x21, hs
100039024: 9bc07df7    	umulh	x23, x15, x0
100039028: 9b007df4    	mul	x20, x15, x0
10003902c: ab100294    	adds	x20, x20, x16
100039030: 9a9536b5    	cinc	x21, x21, hs
100039034: ab170084    	adds	x4, x4, x23
100039038: d37ffc10    	lsr	x16, x0, #63
10003903c: d37ffdef    	lsr	x15, x15, #63
100039040: 3900e3f0    	strb	w16, [sp, #0x38]
100039044: 9100e3e5    	add	x5, sp, #0x38
100039048: 3940e3f7    	ldrb	w23, [sp, #0x38]
10003904c: 9a833463    	cinc	x3, x3, hs
100039050: f9400bfc    	ldr	x28, [sp, #0x10]
100039054: aa1c03fb    	mov	x27, x28
100039058: 3900e3ef    	strb	w15, [sp, #0x38]
10003905c: 3940e3f8    	ldrb	w24, [sp, #0x38]
100039060: 92800005    	mov	x5, #-0x1               ; =-1
100039064: f2401eff    	tst	x23, #0xff
100039068: 9a9c10bb    	csel	x27, x5, x28, ne
10003906c: f2401f1f    	tst	x24, #0xff
100039070: 9a9c10bc    	csel	x28, x5, x28, ne
100039074: a97f1597    	ldp	x23, x5, [x12, #-0x10]
100039078: 8a1b02f7    	and	x23, x23, x27
10003907c: f8420438    	ldr	x24, [x1], #0x20
100039080: 8a1c0318    	and	x24, x24, x28
100039084: ab1802f7    	adds	x23, x23, x24
100039088: 1a9f37f9    	cset	w25, hs
10003908c: eb170178    	subs	x24, x11, x23
100039090: da1901b9    	sbc	x25, x13, x25
100039094: 8a1b00ab    	and	x11, x5, x27
100039098: aa0603e5    	mov	x5, x6
10003909c: aa0803e6    	mov	x6, x8
1000390a0: aa0a03e8    	mov	x8, x10
1000390a4: aa0903ea    	mov	x10, x9
1000390a8: aa0203e9    	mov	x9, x2
1000390ac: f94017e2    	ldr	x2, [sp, #0x28]
1000390b0: 8a1c034d    	and	x13, x26, x28
1000390b4: ab0d016b    	adds	x11, x11, x13
1000390b8: 1a9f37ed    	cset	w13, hs
1000390bc: eb0b01d7    	subs	x23, x14, x11
1000390c0: da0d02d6    	sbc	x22, x22, x13
1000390c4: a8c2358b    	ldp	x11, x13, [x12], #0x20
1000390c8: 8a1b016b    	and	x11, x11, x27
1000390cc: 8a1c03ce    	and	x14, x30, x28
1000390d0: ab0e016b    	adds	x11, x11, x14
1000390d4: 1a9f37ee    	cset	w14, hs
1000390d8: eb0b0294    	subs	x20, x20, x11
1000390dc: da0e02b5    	sbc	x21, x21, x14
1000390e0: 8a1b01ab    	and	x11, x13, x27
1000390e4: 8a1c000d    	and	x13, x0, x28
1000390e8: ab0d016b    	adds	x11, x11, x13
1000390ec: 1a9f37ed    	cset	w13, hs
1000390f0: eb0b0084    	subs	x4, x4, x11
1000390f4: da0d0063    	sbc	x3, x3, x13
1000390f8: a941b7ee    	ldp	x14, x13, [sp, #0x18]
1000390fc: 8a1001eb    	and	x11, x15, x16
100039100: ab0b014a    	adds	x10, x10, x11
100039104: 9a913631    	cinc	x17, x17, hs
100039108: f1000442    	subs	x2, x2, #0x1
10003910c: 54ffec61    	b.ne	0x100038e98 <__RINvNtNtNtCsfDw8YqMpZPC_17field_regressions8campaign10candidates5words20exact_signed_columnsKj4_Kj9_EB8_+0x84>
100039110: 14000012    	b	0x100039158 <__RINvNtNtNtCsfDw8YqMpZPC_17field_regressions8campaign10candidates5words20exact_signed_columnsKj4_Kj9_EB8_+0x344>
100039114: d2800008    	mov	x8, #0x0                ; =0
100039118: d2800009    	mov	x9, #0x0                ; =0
10003911c: d280000d    	mov	x13, #0x0               ; =0
100039120: d280000e    	mov	x14, #0x0               ; =0
100039124: d2800006    	mov	x6, #0x0                ; =0
100039128: d2800005    	mov	x5, #0x0                ; =0
10003912c: d2800007    	mov	x7, #0x0                ; =0
100039130: d2800013    	mov	x19, #0x0               ; =0
100039134: d2800018    	mov	x24, #0x0               ; =0
100039138: d2800019    	mov	x25, #0x0               ; =0
10003913c: d2800017    	mov	x23, #0x0               ; =0
100039140: d2800016    	mov	x22, #0x0               ; =0
100039144: d2800014    	mov	x20, #0x0               ; =0
100039148: d2800015    	mov	x21, #0x0               ; =0
10003914c: d2800004    	mov	x4, #0x0                ; =0
100039150: d2800003    	mov	x3, #0x0                ; =0
100039154: d280000a    	mov	x10, #0x0               ; =0
100039158: 937ffd2b    	asr	x11, x9, #63
10003915c: ab0d0129    	adds	x9, x9, x13
100039160: 9a0e016b    	adc	x11, x11, x14
100039164: 937ffd6c    	asr	x12, x11, #63
100039168: ab06016b    	adds	x11, x11, x6
10003916c: 9a05018c    	adc	x12, x12, x5
100039170: 937ffd8d    	asr	x13, x12, #63
100039174: ab07018c    	adds	x12, x12, x7
100039178: 9a1301ad    	adc	x13, x13, x19
10003917c: 937ffdae    	asr	x14, x13, #63
100039180: ab1801ad    	adds	x13, x13, x24
100039184: 9a1901ce    	adc	x14, x14, x25
100039188: 937ffdcf    	asr	x15, x14, #63
10003918c: ab1701ce    	adds	x14, x14, x23
100039190: 9a1601ef    	adc	x15, x15, x22
100039194: 937ffdf0    	asr	x16, x15, #63
100039198: ab1401ef    	adds	x15, x15, x20
10003919c: 9a150210    	adc	x16, x16, x21
1000391a0: 937ffe11    	asr	x17, x16, #63
1000391a4: f94007e0    	ldr	x0, [sp, #0x8]
1000391a8: a9002408    	stp	x8, x9, [x0]
1000391ac: ab040208    	adds	x8, x16, x4
1000391b0: a901300b    	stp	x11, x12, [x0, #0x10]
1000391b4: 9a030229    	adc	x9, x17, x3
1000391b8: a902380d    	stp	x13, x14, [x0, #0x20]
1000391bc: 8b0a0129    	add	x9, x9, x10
1000391c0: a903200f    	stp	x15, x8, [x0, #0x30]
1000391c4: f9002009    	str	x9, [x0, #0x40]
1000391c8: a9497bfd    	ldp	x29, x30, [sp, #0x90]
1000391cc: a9484ff4    	ldp	x20, x19, [sp, #0x80]
1000391d0: a94757f6    	ldp	x22, x21, [sp, #0x70]
1000391d4: a9465ff8    	ldp	x24, x23, [sp, #0x60]
1000391d8: a94567fa    	ldp	x26, x25, [sp, #0x50]
1000391dc: a9446ffc    	ldp	x28, x27, [sp, #0x40]
1000391e0: 910283ff    	add	sp, sp, #0xa0
1000391e4: d65f03c0    	ret
1000391e8: d0000ba4    	adrp	x4, 0x1001af000 <dyld_stub_binder+0x1001af000>
1000391ec: 91376084    	add	x4, x4, #0xdd8
1000391f0: 9100c3e0    	add	x0, sp, #0x30
1000391f4: 9100e3e1    	add	x1, sp, #0x38
1000391f8: d2800002    	mov	x2, #0x0                ; =0
1000391fc: 94048db2    	bl	0x10015c8c4 <__RINvNtCs8Mbv00yxnRz_4core9panicking13assert_failedjjEB4_>

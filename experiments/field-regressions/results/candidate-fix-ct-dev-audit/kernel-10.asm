
/private/tmp/f2z-arithmetic-target/release/field-regressions:	file format mach-o arm64

Disassembly of section __TEXT,__text:

000000010004bb3c <__RINvNvNtNtNtCshgIkoLp8A2V_17field_regressions8campaign10candidates6binary10polynomial5fixedKj9_KB1s_Kj12_EBa_>:
10004bb3c: d102c3ff    	sub	sp, sp, #0xb0
10004bb40: a90a7bfd    	stp	x29, x30, [sp, #0xa0]
10004bb44: 910283fd    	add	x29, sp, #0xa0
10004bb48: a90013e2    	stp	x2, x4, [sp]
10004bb4c: eb04005f    	cmp	x2, x4
10004bb50: 54000b01    	b.ne	0x10004bcb0 <__RINvNvNtNtNtCshgIkoLp8A2V_17field_regressions8campaign10candidates6binary10polynomial5fixedKj9_KB1s_Kj12_EBa_+0x174>
10004bb54: 6f00e400    	movi.2d	v0, #0000000000000000
10004bb58: ad0083e0    	stp	q0, q0, [sp, #0x10]
10004bb5c: ad0183e0    	stp	q0, q0, [sp, #0x30]
10004bb60: ad0283e0    	stp	q0, q0, [sp, #0x50]
10004bb64: ad0383e0    	stp	q0, q0, [sp, #0x70]
10004bb68: 3d8027e0    	str	q0, [sp, #0x90]
10004bb6c: b4000882    	cbz	x2, 0x10004bc7c <__RINvNvNtNtNtCshgIkoLp8A2V_17field_regressions8campaign10candidates6binary10polynomial5fixedKj9_KB1s_Kj12_EBa_+0x140>
10004bb70: d2800008    	mov	x8, #0x0                ; =0
10004bb74: 910043e9    	add	x9, sp, #0x10
10004bb78: 9100c129    	add	x9, x9, #0x30
10004bb7c: 5280090a    	mov	w10, #0x48              ; =72
10004bb80: d280000b    	mov	x11, #0x0               ; =0
10004bb84: 9b0a0d0c    	madd	x12, x8, x10, x3
10004bb88: a942398d    	ldp	x13, x14, [x12, #0x20]
10004bb8c: a943418f    	ldp	x15, x16, [x12, #0x30]
10004bb90: a9401191    	ldp	x17, x4, [x12]
10004bb94: 9e670220    	fmov	d0, x17
10004bb98: a9414585    	ldp	x5, x17, [x12, #0x10]
10004bb9c: 9e670081    	fmov	d1, x4
10004bba0: 9e6700a2    	fmov	d2, x5
10004bba4: 9e670223    	fmov	d3, x17
10004bba8: 9e6701a4    	fmov	d4, x13
10004bbac: 9e6701c5    	fmov	d5, x14
10004bbb0: f940218c    	ldr	x12, [x12, #0x40]
10004bbb4: 9e6701e6    	fmov	d6, x15
10004bbb8: 9e670207    	fmov	d7, x16
10004bbbc: 9e670190    	fmov	d16, x12
10004bbc0: aa0903ec    	mov	x12, x9
10004bbc4: fc6b7831    	ldr	d17, [x1, x11, lsl #3]
10004bbc8: 0ee1e232    	pmull.1q	v18, v17, v1
10004bbcc: 0ee2e233    	pmull.1q	v19, v17, v2
10004bbd0: 0ee3e234    	pmull.1q	v20, v17, v3
10004bbd4: 4ed37a55    	zip2.2d	v21, v18, v19
10004bbd8: 6e180693    	mov.d	v19[1], v20[0]
10004bbdc: ad7f5d96    	ldp	q22, q23, [x12, #-0x20]
10004bbe0: ce154ed3    	eor3.16b	v19, v22, v21, v19
10004bbe4: 0ee4e235    	pmull.1q	v21, v17, v4
10004bbe8: 0ee5e236    	pmull.1q	v22, v17, v5
10004bbec: 4ed57a94    	zip2.2d	v20, v20, v21
10004bbf0: 6e1806d5    	mov.d	v21[1], v22[0]
10004bbf4: ce1456f4    	eor3.16b	v20, v23, v20, v21
10004bbf8: 0ee6e235    	pmull.1q	v21, v17, v6
10004bbfc: 0ee7e237    	pmull.1q	v23, v17, v7
10004bc00: 3dc00198    	ldr	q24, [x12]
10004bc04: 4ed57ad6    	zip2.2d	v22, v22, v21
10004bc08: 6e1806f5    	mov.d	v21[1], v23[0]
10004bc0c: ce165715    	eor3.16b	v21, v24, v22, v21
10004bc10: 0ee0e236    	pmull.1q	v22, v17, v0
10004bc14: 4e183ecd    	mov.d	x13, v22[1]
10004bc18: f85d818e    	ldur	x14, [x12, #-0x28]
10004bc1c: ca0d01cd    	eor	x13, x14, x13
10004bc20: 6e180656    	mov.d	v22[1], v18[0]
10004bc24: fc5d0192    	ldur	d18, [x12, #-0x30]
10004bc28: 4e181db2    	mov.d	v18[1], x13
10004bc2c: 6e361e52    	eor.16b	v18, v18, v22
10004bc30: ad3ecd92    	stp	q18, q19, [x12, #-0x30]
10004bc34: ad3fd594    	stp	q20, q21, [x12, #-0x10]
10004bc38: 4e183eed    	mov.d	x13, v23[1]
10004bc3c: f940098e    	ldr	x14, [x12, #0x10]
10004bc40: ca0d01cd    	eor	x13, x14, x13
10004bc44: 9100618e    	add	x14, x12, #0x18
10004bc48: 9e6701b2    	fmov	d18, x13
10004bc4c: 4d4085d2    	ld1.d	{ v18 }[1], [x14]
10004bc50: 0ef0e231    	pmull.1q	v17, v17, v16
10004bc54: 6e311e51    	eor.16b	v17, v18, v17
10004bc58: 3d800591    	str	q17, [x12, #0x10]
10004bc5c: 9100056b    	add	x11, x11, #0x1
10004bc60: 9100218c    	add	x12, x12, #0x8
10004bc64: f100257f    	cmp	x11, #0x9
10004bc68: 54fffae1    	b.ne	0x10004bbc4 <__RINvNvNtNtNtCshgIkoLp8A2V_17field_regressions8campaign10candidates6binary10polynomial5fixedKj9_KB1s_Kj12_EBa_+0x88>
10004bc6c: 91000508    	add	x8, x8, #0x1
10004bc70: 91012021    	add	x1, x1, #0x48
10004bc74: eb02011f    	cmp	x8, x2
10004bc78: 54fff841    	b.ne	0x10004bb80 <__RINvNvNtNtNtCshgIkoLp8A2V_17field_regressions8campaign10candidates6binary10polynomial5fixedKj9_KB1s_Kj12_EBa_+0x44>
10004bc7c: ad4387e0    	ldp	q0, q1, [sp, #0x70]
10004bc80: ad030400    	stp	q0, q1, [x0, #0x60]
10004bc84: 3dc027e0    	ldr	q0, [sp, #0x90]
10004bc88: 3d802000    	str	q0, [x0, #0x80]
10004bc8c: ad4187e0    	ldp	q0, q1, [sp, #0x30]
10004bc90: ad010400    	stp	q0, q1, [x0, #0x20]
10004bc94: ad4283e1    	ldp	q1, q0, [sp, #0x50]
10004bc98: ad020001    	stp	q1, q0, [x0, #0x40]
10004bc9c: ad4083e1    	ldp	q1, q0, [sp, #0x10]
10004bca0: ad000001    	stp	q1, q0, [x0]
10004bca4: a94a7bfd    	ldp	x29, x30, [sp, #0xa0]
10004bca8: 9102c3ff    	add	sp, sp, #0xb0
10004bcac: d65f03c0    	ret
10004bcb0: b0000b24    	adrp	x4, 0x1001b0000 <dyld_stub_binder+0x1001b0000>
10004bcb4: 912fc084    	add	x4, x4, #0xbf0
10004bcb8: 910003e0    	mov	x0, sp
10004bcbc: 910023e1    	add	x1, sp, #0x8
10004bcc0: d2800002    	mov	x2, #0x0                ; =0
10004bcc4: 94043e6c    	bl	0x10015b674 <__RINvNtCs8Mbv00yxnRz_4core9panicking13assert_failedjjEB4_>

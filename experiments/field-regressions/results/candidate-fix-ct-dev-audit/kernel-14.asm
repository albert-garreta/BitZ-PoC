
/private/tmp/f2z-arithmetic-target/release/field-regressions:	file format mach-o arm64

Disassembly of section __TEXT,__text:

00000001000e4034 <__RNvNtNtNtCshgIkoLp8A2V_17field_regressions8campaign10candidates9composite4dot3>:
1000e4034: d10083ff    	sub	sp, sp, #0x20
1000e4038: a9017bfd    	stp	x29, x30, [sp, #0x10]
1000e403c: 910043fd    	add	x29, sp, #0x10
1000e4040: a90013e2    	stp	x2, x4, [sp]
1000e4044: eb04005f    	cmp	x2, x4
1000e4048: 54000621    	b.ne	0x1000e410c <__RNvNtNtNtCshgIkoLp8A2V_17field_regressions8campaign10candidates9composite4dot3+0xd8>
1000e404c: 91002028    	add	x8, x1, #0x8
1000e4050: 6f00e400    	movi.2d	v0, #0000000000000000
1000e4054: 6f00e401    	movi.2d	v1, #0000000000000000
1000e4058: 6f00e402    	movi.2d	v2, #0000000000000000
1000e405c: a97fa909    	ldp	x9, x10, [x8, #-0x8]
1000e4060: ca09014b    	eor	x11, x10, x9
1000e4064: 9e670123    	fmov	d3, x9
1000e4068: 3cc10464    	ldr	q4, [x3], #0x10
1000e406c: 0ee4e063    	pmull.1q	v3, v3, v4
1000e4070: 6e231c00    	eor.16b	v0, v0, v3
1000e4074: 4e080d43    	dup.2d	v3, x10
1000e4078: 4ee4e063    	pmull2.1q	v3, v3, v4
1000e407c: 6e231c21    	eor.16b	v1, v1, v3
1000e4080: 4e180483    	dup.2d	v3, v4[1]
1000e4084: 2e241c63    	eor.8b	v3, v3, v4
1000e4088: 9e670164    	fmov	d4, x11
1000e408c: 0ee3e083    	pmull.1q	v3, v4, v3
1000e4090: 6e231c42    	eor.16b	v2, v2, v3
1000e4094: 91004108    	add	x8, x8, #0x10
1000e4098: f1000442    	subs	x2, x2, #0x1
1000e409c: 54fffe01    	b.ne	0x1000e405c <__RNvNtNtNtCshgIkoLp8A2V_17field_regressions8campaign10candidates9composite4dot3+0x28>
1000e40a0: ce020022    	eor3.16b	v2, v1, v2, v0
1000e40a4: 6f00e403    	movi.2d	v3, #0000000000000000
1000e40a8: 6e024064    	ext.16b	v4, v3, v2, #0x8
1000e40ac: 6e201c80    	eor.16b	v0, v4, v0
1000e40b0: 6e034042    	ext.16b	v2, v2, v3, #0x8
1000e40b4: 6e211c42    	eor.16b	v2, v2, v1
1000e40b8: 4e183c08    	mov.d	x8, v0[1]
1000e40bc: 4e183c29    	mov.d	x9, v1[1]
1000e40c0: 9e66004a    	fmov	x10, d2
1000e40c4: d37ffd2b    	lsr	x11, x9, #63
1000e40c8: ca49f96b    	eor	x11, x11, x9, lsr #62
1000e40cc: ca49e569    	eor	x9, x11, x9, lsr #57
1000e40d0: ca0a0129    	eor	x9, x9, x10
1000e40d4: ca4af908    	eor	x8, x8, x10, lsr #62
1000e40d8: ca4afd08    	eor	x8, x8, x10, lsr #63
1000e40dc: ca4ae508    	eor	x8, x8, x10, lsr #57
1000e40e0: 4e081d22    	mov.d	v2[0], x9
1000e40e4: 4ee28441    	add.2d	v1, v2, v2
1000e40e8: 4f425443    	shl.2d	v3, v2, #0x2
1000e40ec: 4f475444    	shl.2d	v4, v2, #0x7
1000e40f0: 4e181d00    	mov.d	v0[1], x8
1000e40f4: ce010c00    	eor3.16b	v0, v0, v1, v3
1000e40f8: ce040800    	eor3.16b	v0, v0, v4, v2
1000e40fc: 3d800000    	str	q0, [x0]
1000e4100: a9417bfd    	ldp	x29, x30, [sp, #0x10]
1000e4104: 910083ff    	add	sp, sp, #0x20
1000e4108: d65f03c0    	ret
1000e410c: f0000664    	adrp	x4, 0x1001b3000 <dyld_stub_binder+0x1001b3000>
1000e4110: 9129e084    	add	x4, x4, #0xa78
1000e4114: 910003e0    	mov	x0, sp
1000e4118: 910023e1    	add	x1, sp, #0x8
1000e411c: d2800002    	mov	x2, #0x0                ; =0
1000e4120: 9401dd55    	bl	0x10015b674 <__RINvNtCs8Mbv00yxnRz_4core9panicking13assert_failedjjEB4_>

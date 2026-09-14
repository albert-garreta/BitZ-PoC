
/private/var/folders/q0/v7l8dzd13k7f_q6j80sy03lm0000gn/T/f2z-field-gated-pycl0nhx/field-regressions:	file format mach-o arm64

Disassembly of section __TEXT,__text:

00000001000e3b64 <__RNvNtNtNtCsagpXUhzPPpl_17field_regressions8campaign10candidates9composite4dot3>:
1000e3b64: d10083ff    	sub	sp, sp, #0x20
1000e3b68: a9017bfd    	stp	x29, x30, [sp, #0x10]
1000e3b6c: 910043fd    	add	x29, sp, #0x10
1000e3b70: a90013e2    	stp	x2, x4, [sp]
1000e3b74: eb04005f    	cmp	x2, x4
1000e3b78: 54000621    	b.ne	0x1000e3c3c <__RNvNtNtNtCsagpXUhzPPpl_17field_regressions8campaign10candidates9composite4dot3+0xd8>
1000e3b7c: 91002028    	add	x8, x1, #0x8
1000e3b80: 6f00e400    	movi.2d	v0, #0000000000000000
1000e3b84: 6f00e401    	movi.2d	v1, #0000000000000000
1000e3b88: 6f00e402    	movi.2d	v2, #0000000000000000
1000e3b8c: a97fa909    	ldp	x9, x10, [x8, #-0x8]
1000e3b90: ca09014b    	eor	x11, x10, x9
1000e3b94: 9e670123    	fmov	d3, x9
1000e3b98: 3cc10464    	ldr	q4, [x3], #0x10
1000e3b9c: 0ee4e063    	pmull.1q	v3, v3, v4
1000e3ba0: 6e231c00    	eor.16b	v0, v0, v3
1000e3ba4: 4e080d43    	dup.2d	v3, x10
1000e3ba8: 4ee4e063    	pmull2.1q	v3, v3, v4
1000e3bac: 6e231c21    	eor.16b	v1, v1, v3
1000e3bb0: 4e180483    	dup.2d	v3, v4[1]
1000e3bb4: 2e241c63    	eor.8b	v3, v3, v4
1000e3bb8: 9e670164    	fmov	d4, x11
1000e3bbc: 0ee3e083    	pmull.1q	v3, v4, v3
1000e3bc0: 6e231c42    	eor.16b	v2, v2, v3
1000e3bc4: 91004108    	add	x8, x8, #0x10
1000e3bc8: f1000442    	subs	x2, x2, #0x1
1000e3bcc: 54fffe01    	b.ne	0x1000e3b8c <__RNvNtNtNtCsagpXUhzPPpl_17field_regressions8campaign10candidates9composite4dot3+0x28>
1000e3bd0: ce020022    	eor3.16b	v2, v1, v2, v0
1000e3bd4: 6f00e403    	movi.2d	v3, #0000000000000000
1000e3bd8: 6e024064    	ext.16b	v4, v3, v2, #0x8
1000e3bdc: 6e201c80    	eor.16b	v0, v4, v0
1000e3be0: 6e034042    	ext.16b	v2, v2, v3, #0x8
1000e3be4: 6e211c42    	eor.16b	v2, v2, v1
1000e3be8: 4e183c08    	mov.d	x8, v0[1]
1000e3bec: 4e183c29    	mov.d	x9, v1[1]
1000e3bf0: 9e66004a    	fmov	x10, d2
1000e3bf4: d37ffd2b    	lsr	x11, x9, #63
1000e3bf8: ca49f96b    	eor	x11, x11, x9, lsr #62
1000e3bfc: ca49e569    	eor	x9, x11, x9, lsr #57
1000e3c00: ca0a0129    	eor	x9, x9, x10
1000e3c04: ca4af908    	eor	x8, x8, x10, lsr #62
1000e3c08: ca4afd08    	eor	x8, x8, x10, lsr #63
1000e3c0c: ca4ae508    	eor	x8, x8, x10, lsr #57
1000e3c10: 4e081d22    	mov.d	v2[0], x9
1000e3c14: 4ee28441    	add.2d	v1, v2, v2
1000e3c18: 4f425443    	shl.2d	v3, v2, #0x2
1000e3c1c: 4f475444    	shl.2d	v4, v2, #0x7
1000e3c20: 4e181d00    	mov.d	v0[1], x8
1000e3c24: ce010c00    	eor3.16b	v0, v0, v1, v3
1000e3c28: ce040800    	eor3.16b	v0, v0, v4, v2
1000e3c2c: 3d800000    	str	q0, [x0]
1000e3c30: a9417bfd    	ldp	x29, x30, [sp, #0x10]
1000e3c34: 910083ff    	add	sp, sp, #0x20
1000e3c38: d65f03c0    	ret
1000e3c3c: 90000684    	adrp	x4, 0x1001b3000 <dyld_stub_binder+0x1001b3000>
1000e3c40: 9129e084    	add	x4, x4, #0xa78
1000e3c44: 910003e0    	mov	x0, sp
1000e3c48: 910023e1    	add	x1, sp, #0x8
1000e3c4c: d2800002    	mov	x2, #0x0                ; =0
1000e3c50: 9401de80    	bl	0x10015b650 <__RINvNtCs8Mbv00yxnRz_4core9panicking13assert_failedjjEB4_>

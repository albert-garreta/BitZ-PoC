
/private/var/folders/q0/v7l8dzd13k7f_q6j80sy03lm0000gn/T/f2z-field-gated-h_tggvge/field-regressions:	file format mach-o arm64

Disassembly of section __TEXT,__text:

00000001000e559c <__RNvNtNtNtCsfDw8YqMpZPC_17field_regressions8campaign10candidates9composite4dot3>:
1000e559c: d10083ff    	sub	sp, sp, #0x20
1000e55a0: a9017bfd    	stp	x29, x30, [sp, #0x10]
1000e55a4: 910043fd    	add	x29, sp, #0x10
1000e55a8: a90013e2    	stp	x2, x4, [sp]
1000e55ac: eb04005f    	cmp	x2, x4
1000e55b0: 54000621    	b.ne	0x1000e5674 <__RNvNtNtNtCsfDw8YqMpZPC_17field_regressions8campaign10candidates9composite4dot3+0xd8>
1000e55b4: 91002028    	add	x8, x1, #0x8
1000e55b8: 6f00e400    	movi.2d	v0, #0000000000000000
1000e55bc: 6f00e401    	movi.2d	v1, #0000000000000000
1000e55c0: 6f00e402    	movi.2d	v2, #0000000000000000
1000e55c4: a97fa909    	ldp	x9, x10, [x8, #-0x8]
1000e55c8: ca09014b    	eor	x11, x10, x9
1000e55cc: 9e670123    	fmov	d3, x9
1000e55d0: 3cc10464    	ldr	q4, [x3], #0x10
1000e55d4: 0ee4e063    	pmull.1q	v3, v3, v4
1000e55d8: 6e231c00    	eor.16b	v0, v0, v3
1000e55dc: 4e080d43    	dup.2d	v3, x10
1000e55e0: 4ee4e063    	pmull2.1q	v3, v3, v4
1000e55e4: 6e231c21    	eor.16b	v1, v1, v3
1000e55e8: 4e180483    	dup.2d	v3, v4[1]
1000e55ec: 2e241c63    	eor.8b	v3, v3, v4
1000e55f0: 9e670164    	fmov	d4, x11
1000e55f4: 0ee3e083    	pmull.1q	v3, v4, v3
1000e55f8: 6e231c42    	eor.16b	v2, v2, v3
1000e55fc: 91004108    	add	x8, x8, #0x10
1000e5600: f1000442    	subs	x2, x2, #0x1
1000e5604: 54fffe01    	b.ne	0x1000e55c4 <__RNvNtNtNtCsfDw8YqMpZPC_17field_regressions8campaign10candidates9composite4dot3+0x28>
1000e5608: ce020022    	eor3.16b	v2, v1, v2, v0
1000e560c: 6f00e403    	movi.2d	v3, #0000000000000000
1000e5610: 6e024064    	ext.16b	v4, v3, v2, #0x8
1000e5614: 6e201c80    	eor.16b	v0, v4, v0
1000e5618: 6e034042    	ext.16b	v2, v2, v3, #0x8
1000e561c: 6e211c42    	eor.16b	v2, v2, v1
1000e5620: 4e183c08    	mov.d	x8, v0[1]
1000e5624: 4e183c29    	mov.d	x9, v1[1]
1000e5628: 9e66004a    	fmov	x10, d2
1000e562c: d37ffd2b    	lsr	x11, x9, #63
1000e5630: ca49f96b    	eor	x11, x11, x9, lsr #62
1000e5634: ca49e569    	eor	x9, x11, x9, lsr #57
1000e5638: ca0a0129    	eor	x9, x9, x10
1000e563c: ca4af908    	eor	x8, x8, x10, lsr #62
1000e5640: ca4afd08    	eor	x8, x8, x10, lsr #63
1000e5644: ca4ae508    	eor	x8, x8, x10, lsr #57
1000e5648: 4e081d22    	mov.d	v2[0], x9
1000e564c: 4ee28441    	add.2d	v1, v2, v2
1000e5650: 4f425443    	shl.2d	v3, v2, #0x2
1000e5654: 4f475444    	shl.2d	v4, v2, #0x7
1000e5658: 4e181d00    	mov.d	v0[1], x8
1000e565c: ce010c00    	eor3.16b	v0, v0, v1, v3
1000e5660: ce040800    	eor3.16b	v0, v0, v4, v2
1000e5664: 3d800000    	str	q0, [x0]
1000e5668: a9417bfd    	ldp	x29, x30, [sp, #0x10]
1000e566c: 910083ff    	add	sp, sp, #0x20
1000e5670: d65f03c0    	ret
1000e5674: d0000664    	adrp	x4, 0x1001b3000 <dyld_stub_binder+0x1001b3000>
1000e5678: 912ce084    	add	x4, x4, #0xb38
1000e567c: 910003e0    	mov	x0, sp
1000e5680: 910023e1    	add	x1, sp, #0x8
1000e5684: d2800002    	mov	x2, #0x0                ; =0
1000e5688: 9401dc8f    	bl	0x10015c8c4 <__RINvNtCs8Mbv00yxnRz_4core9panicking13assert_failedjjEB4_>

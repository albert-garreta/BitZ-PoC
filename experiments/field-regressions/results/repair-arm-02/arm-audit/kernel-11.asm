
/private/var/folders/q0/v7l8dzd13k7f_q6j80sy03lm0000gn/T/f2z-field-gated-aejimxw9/field-regressions:	file format mach-o arm64

Disassembly of section __TEXT,__text:

00000001000d70b8 <__RNvNtNtNtCsaHyC9lX8wyC_17field_regressions8campaign10candidates9composite4dot3>:
1000d70b8: d10083ff    	sub	sp, sp, #0x20
1000d70bc: a9017bfd    	stp	x29, x30, [sp, #0x10]
1000d70c0: 910043fd    	add	x29, sp, #0x10
1000d70c4: a90013e2    	stp	x2, x4, [sp]
1000d70c8: eb04005f    	cmp	x2, x4
1000d70cc: 54000621    	b.ne	0x1000d7190 <__RNvNtNtNtCsaHyC9lX8wyC_17field_regressions8campaign10candidates9composite4dot3+0xd8>
1000d70d0: 91002028    	add	x8, x1, #0x8
1000d70d4: 6f00e400    	movi.2d	v0, #0000000000000000
1000d70d8: 6f00e401    	movi.2d	v1, #0000000000000000
1000d70dc: 6f00e402    	movi.2d	v2, #0000000000000000
1000d70e0: a97fa909    	ldp	x9, x10, [x8, #-0x8]
1000d70e4: ca09014b    	eor	x11, x10, x9
1000d70e8: 9e670123    	fmov	d3, x9
1000d70ec: 3cc10464    	ldr	q4, [x3], #0x10
1000d70f0: 0ee4e063    	pmull.1q	v3, v3, v4
1000d70f4: 6e231c00    	eor.16b	v0, v0, v3
1000d70f8: 4e080d43    	dup.2d	v3, x10
1000d70fc: 4ee4e063    	pmull2.1q	v3, v3, v4
1000d7100: 6e231c21    	eor.16b	v1, v1, v3
1000d7104: 4e180483    	dup.2d	v3, v4[1]
1000d7108: 2e241c63    	eor.8b	v3, v3, v4
1000d710c: 9e670164    	fmov	d4, x11
1000d7110: 0ee3e083    	pmull.1q	v3, v4, v3
1000d7114: 6e231c42    	eor.16b	v2, v2, v3
1000d7118: 91004108    	add	x8, x8, #0x10
1000d711c: f1000442    	subs	x2, x2, #0x1
1000d7120: 54fffe01    	b.ne	0x1000d70e0 <__RNvNtNtNtCsaHyC9lX8wyC_17field_regressions8campaign10candidates9composite4dot3+0x28>
1000d7124: ce020022    	eor3.16b	v2, v1, v2, v0
1000d7128: 6f00e403    	movi.2d	v3, #0000000000000000
1000d712c: 6e024064    	ext.16b	v4, v3, v2, #0x8
1000d7130: 6e201c80    	eor.16b	v0, v4, v0
1000d7134: 6e034042    	ext.16b	v2, v2, v3, #0x8
1000d7138: 6e211c42    	eor.16b	v2, v2, v1
1000d713c: 4e183c08    	mov.d	x8, v0[1]
1000d7140: 4e183c29    	mov.d	x9, v1[1]
1000d7144: 9e66004a    	fmov	x10, d2
1000d7148: d37ffd2b    	lsr	x11, x9, #63
1000d714c: ca49f96b    	eor	x11, x11, x9, lsr #62
1000d7150: ca49e569    	eor	x9, x11, x9, lsr #57
1000d7154: ca0a0129    	eor	x9, x9, x10
1000d7158: ca4af908    	eor	x8, x8, x10, lsr #62
1000d715c: ca4afd08    	eor	x8, x8, x10, lsr #63
1000d7160: ca4ae508    	eor	x8, x8, x10, lsr #57
1000d7164: 4e081d22    	mov.d	v2[0], x9
1000d7168: 4ee28441    	add.2d	v1, v2, v2
1000d716c: 4f425443    	shl.2d	v3, v2, #0x2
1000d7170: 4f475444    	shl.2d	v4, v2, #0x7
1000d7174: 4e181d00    	mov.d	v0[1], x8
1000d7178: ce010c00    	eor3.16b	v0, v0, v1, v3
1000d717c: ce040800    	eor3.16b	v0, v0, v4, v2
1000d7180: 3d800000    	str	q0, [x0]
1000d7184: a9417bfd    	ldp	x29, x30, [sp, #0x10]
1000d7188: 910083ff    	add	sp, sp, #0x20
1000d718c: d65f03c0    	ret
1000d7190: 90000644    	adrp	x4, 0x10019f000 <dyld_stub_binder+0x10019f000>
1000d7194: 9101c084    	add	x4, x4, #0x70
1000d7198: 910003e0    	mov	x0, sp
1000d719c: 910023e1    	add	x1, sp, #0x8
1000d71a0: d2800002    	mov	x2, #0x0                ; =0
1000d71a4: 9401d1f1    	bl	0x10014b968 <__RINvNtCs8Mbv00yxnRz_4core9panicking13assert_failedjjEB4_>

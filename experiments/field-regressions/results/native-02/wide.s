
/var/folders/q0/v7l8dzd13k7f_q6j80sy03lm0000gn/T/f2z-field-regressions-emqdbh47/target/release/field-regressions:	file format mach-o arm64

Disassembly of section __TEXT,__text:

0000000100001394 <field_regressions::arithmetic::wide_dot::<16>>:
100001394: 6dba3bef    	stp	d15, d14, [sp, #-0x60]!
100001398: 6d0133ed    	stp	d13, d12, [sp, #0x10]
10000139c: 6d022beb    	stp	d11, d10, [sp, #0x20]
1000013a0: 6d0323e9    	stp	d9, d8, [sp, #0x30]
1000013a4: a9046ffc    	stp	x28, x27, [sp, #0x40]
1000013a8: a9057bfd    	stp	x29, x30, [sp, #0x50]
1000013ac: 910143fd    	add	x29, sp, #0x50
1000013b0: d10843ff    	sub	sp, sp, #0x210
1000013b4: a90013e2    	stp	x2, x4, [sp]
1000013b8: eb04005f    	cmp	x2, x4
1000013bc: 54001121    	b.ne	0x1000015e0 <field_regressions::arithmetic::wide_dot::<16>+0x24c>
1000013c0: 6f00e400    	movi.2d	v0, #0000000000000000
1000013c4: ad0f83e0    	stp	q0, q0, [sp, #0x1f0]
1000013c8: ad0e83e0    	stp	q0, q0, [sp, #0x1d0]
1000013cc: ad0d83e0    	stp	q0, q0, [sp, #0x1b0]
1000013d0: ad0c83e0    	stp	q0, q0, [sp, #0x190]
1000013d4: ad0b83e0    	stp	q0, q0, [sp, #0x170]
1000013d8: ad0a83e0    	stp	q0, q0, [sp, #0x150]
1000013dc: ad0983e0    	stp	q0, q0, [sp, #0x130]
1000013e0: ad0883e0    	stp	q0, q0, [sp, #0x110]
1000013e4: ad0783e0    	stp	q0, q0, [sp, #0xf0]
1000013e8: ad0683e0    	stp	q0, q0, [sp, #0xd0]
1000013ec: ad0583e0    	stp	q0, q0, [sp, #0xb0]
1000013f0: ad0483e0    	stp	q0, q0, [sp, #0x90]
1000013f4: ad0383e0    	stp	q0, q0, [sp, #0x70]
1000013f8: ad0283e0    	stp	q0, q0, [sp, #0x50]
1000013fc: ad0183e0    	stp	q0, q0, [sp, #0x30]
100001400: d344fc49    	lsr	x9, x2, #4
100001404: 6f00e401    	movi.2d	v1, #0000000000000000
100001408: ad0083e0    	stp	q0, q0, [sp, #0x10]
10000140c: b40004c9    	cbz	x9, 0x1000014a4 <field_regressions::arithmetic::wide_dot::<16>+0x110>
100001410: d280000b    	mov	x11, #0x0               ; =0
100001414: 9100206a    	add	x10, x3, #0x8
100001418: 9100202c    	add	x12, x1, #0x8
10000141c: 910043ed    	add	x13, sp, #0x10
100001420: aa0903ee    	mov	x14, x9
100001424: aa0b03e8    	mov	x8, x11
100001428: 9100416b    	add	x11, x11, #0x10
10000142c: d10005ce    	sub	x14, x14, #0x1
100001430: aa0c03ef    	mov	x15, x12
100001434: aa0a03f0    	mov	x16, x10
100001438: 52800211    	mov	w17, #0x10              ; =16
10000143c: eb02011f    	cmp	x8, x2
100001440: 54000c42    	b.hs	0x1000015c8 <field_regressions::arithmetic::wide_dot::<16>+0x234>
100001444: 8b1101a4    	add	x4, x13, x17
100001448: 6d7f8a01    	ldp	d1, d2, [x16, #-0x8]
10000144c: 6d7f91e3    	ldp	d3, d4, [x15, #-0x8]
100001450: 0ee1e065    	pmull.1q	v5, v3, v1
100001454: 0ee2e086    	pmull.1q	v6, v4, v2
100001458: 0ee2e062    	pmull.1q	v2, v3, v2
10000145c: 0ee1e081    	pmull.1q	v1, v4, v1
100001460: 6e221c21    	eor.16b	v1, v1, v2
100001464: 6e014002    	ext.16b	v2, v0, v1, #0x8
100001468: 6e004021    	ext.16b	v1, v1, v0, #0x8
10000146c: ad7f9083    	ldp	q3, q4, [x4, #-0x10]
100001470: ce050862    	eor3.16b	v2, v3, v5, v2
100001474: ce060481    	eor3.16b	v1, v4, v6, v1
100001478: 91008231    	add	x17, x17, #0x20
10000147c: ad3f8482    	stp	q2, q1, [x4, #-0x10]
100001480: 91000508    	add	x8, x8, #0x1
100001484: 91004210    	add	x16, x16, #0x10
100001488: 910041ef    	add	x15, x15, #0x10
10000148c: f108423f    	cmp	x17, #0x210
100001490: 54fffd61    	b.ne	0x10000143c <field_regressions::arithmetic::wide_dot::<16>+0xa8>
100001494: 9104014a    	add	x10, x10, #0x100
100001498: 9104018c    	add	x12, x12, #0x100
10000149c: b5fffc4e    	cbnz	x14, 0x100001424 <field_regressions::arithmetic::wide_dot::<16>+0x90>
1000014a0: ad4083e1    	ldp	q1, q0, [sp, #0x10]
1000014a4: 927cd848    	and	x8, x2, #0x7fffffffffffff0
1000014a8: eb02011f    	cmp	x8, x2
1000014ac: 540002c0    	b.eq	0x100001504 <field_regressions::arithmetic::wide_dot::<16>+0x170>
1000014b0: 5280010a    	mov	w10, #0x8               ; =8
1000014b4: b378dd2a    	bfi	x10, x9, #8, #56
1000014b8: 8b0a0029    	add	x9, x1, x10
1000014bc: 8b0a006a    	add	x10, x3, x10
1000014c0: 6f00e402    	movi.2d	v2, #0000000000000000
1000014c4: 91000508    	add	x8, x8, #0x1
1000014c8: 6d7f9143    	ldp	d3, d4, [x10, #-0x8]
1000014cc: 6d7f9925    	ldp	d5, d6, [x9, #-0x8]
1000014d0: 0ee3e0a7    	pmull.1q	v7, v5, v3
1000014d4: 0ee4e0d0    	pmull.1q	v16, v6, v4
1000014d8: 0ee4e0a4    	pmull.1q	v4, v5, v4
1000014dc: 0ee3e0c3    	pmull.1q	v3, v6, v3
1000014e0: 6e241c63    	eor.16b	v3, v3, v4
1000014e4: 6e034044    	ext.16b	v4, v2, v3, #0x8
1000014e8: 6e024063    	ext.16b	v3, v3, v2, #0x8
1000014ec: ce071021    	eor3.16b	v1, v1, v7, v4
1000014f0: ce100c00    	eor3.16b	v0, v0, v16, v3
1000014f4: 91004129    	add	x9, x9, #0x10
1000014f8: 9100414a    	add	x10, x10, #0x10
1000014fc: eb02011f    	cmp	x8, x2
100001500: 54fffe23    	b.lo	0x1000014c4 <field_regressions::arithmetic::wide_dot::<16>+0x130>
100001504: ad418fe2    	ldp	q2, q3, [sp, #0x30]
100001508: ad4297e4    	ldp	q4, q5, [sp, #0x50]
10000150c: ad439fe6    	ldp	q6, q7, [sp, #0x70]
100001510: ad44c7f0    	ldp	q16, q17, [sp, #0x90]
100001514: ad45cff2    	ldp	q18, q19, [sp, #0xb0]
100001518: ad46d7f4    	ldp	q20, q21, [sp, #0xd0]
10000151c: ad47dff6    	ldp	q22, q23, [sp, #0xf0]
100001520: ad48e7f8    	ldp	q24, q25, [sp, #0x110]
100001524: ad49effa    	ldp	q26, q27, [sp, #0x130]
100001528: ad4af7fc    	ldp	q28, q29, [sp, #0x150]
10000152c: ad4bfffe    	ldp	q30, q31, [sp, #0x170]
100001530: ad4ca7e8    	ldp	q8, q9, [sp, #0x190]
100001534: ad4dafea    	ldp	q10, q11, [sp, #0x1b0]
100001538: ad4eb7ec    	ldp	q12, q13, [sp, #0x1d0]
10000153c: ad4fbfee    	ldp	q14, q15, [sp, #0x1f0]
100001540: 6e201c60    	eor.16b	v0, v3, v0
100001544: 6e241c42    	eor.16b	v2, v2, v4
100001548: ce001ca0    	eor3.16b	v0, v5, v0, v7
10000154c: ce064042    	eor3.16b	v2, v2, v6, v16
100001550: ce004e20    	eor3.16b	v0, v17, v0, v19
100001554: ce125042    	eor3.16b	v2, v2, v18, v20
100001558: ce005ea0    	eor3.16b	v0, v21, v0, v23
10000155c: ce166042    	eor3.16b	v2, v2, v22, v24
100001560: ce006f20    	eor3.16b	v0, v25, v0, v27
100001564: ce1a7042    	eor3.16b	v2, v2, v26, v28
100001568: ce007fa0    	eor3.16b	v0, v29, v0, v31
10000156c: ce1e2042    	eor3.16b	v2, v2, v30, v8
100001570: ce002d20    	eor3.16b	v0, v9, v0, v11
100001574: ce0a3042    	eor3.16b	v2, v2, v10, v12
100001578: ce0e0441    	eor3.16b	v1, v2, v14, v1
10000157c: ce003da0    	eor3.16b	v0, v13, v0, v15
100001580: 528010e8    	mov	w8, #0x87               ; =135
100001584: 4e080d02    	dup.2d	v2, x8
100001588: 4ee2e003    	pmull2.1q	v3, v0, v2
10000158c: 6f00e404    	movi.2d	v4, #0000000000000000
100001590: 6e034084    	ext.16b	v4, v4, v3, #0x8
100001594: 4ee2e063    	pmull2.1q	v3, v3, v2
100001598: 0ee2e000    	pmull.1q	v0, v0, v2
10000159c: 6e201c80    	eor.16b	v0, v4, v0
1000015a0: ce030400    	eor3.16b	v0, v0, v3, v1
1000015a4: 3d800000    	str	q0, [x0]
1000015a8: 910843ff    	add	sp, sp, #0x210
1000015ac: a9457bfd    	ldp	x29, x30, [sp, #0x50]
1000015b0: a9446ffc    	ldp	x28, x27, [sp, #0x40]
1000015b4: 6d4323e9    	ldp	d9, d8, [sp, #0x30]
1000015b8: 6d422beb    	ldp	d11, d10, [sp, #0x20]
1000015bc: 6d4133ed    	ldp	d13, d12, [sp, #0x10]
1000015c0: 6cc63bef    	ldp	d15, d14, [sp], #0x60
1000015c4: d65f03c0    	ret
1000015c8: f0000509    	adrp	x9, 0x1000a4000 <dyld_stub_binder+0x1000a4000>
1000015cc: 91060129    	add	x9, x9, #0x180
1000015d0: aa0803e0    	mov	x0, x8
1000015d4: aa0203e1    	mov	x1, x2
1000015d8: aa0903e2    	mov	x2, x9
1000015dc: 9401f245    	bl	0x10007def0 <core::panicking::panic_bounds_check>
1000015e0: f0000503    	adrp	x3, 0x1000a4000 <dyld_stub_binder+0x1000a4000>
1000015e4: 9105a063    	add	x3, x3, #0x168
1000015e8: 910003e0    	mov	x0, sp
1000015ec: 910023e1    	add	x1, sp, #0x8
1000015f0: d2800002    	mov	x2, #0x0                ; =0
1000015f4: 9401f24d    	bl	0x10007df28 <core::panicking::assert_failed::<usize, usize>>

00000001000015f8 <field_regressions::arithmetic::wide_dot::<1>>:
1000015f8: d10083ff    	sub	sp, sp, #0x20
1000015fc: a9017bfd    	stp	x29, x30, [sp, #0x10]
100001600: 910043fd    	add	x29, sp, #0x10
100001604: a90013e2    	stp	x2, x4, [sp]
100001608: eb04005f    	cmp	x2, x4
10000160c: 54000461    	b.ne	0x100001698 <field_regressions::arithmetic::wide_dot::<1>+0xa0>
100001610: 6f00e400    	movi.2d	v0, #0000000000000000
100001614: 6f00e402    	movi.2d	v2, #0000000000000000
100001618: 6f00e401    	movi.2d	v1, #0000000000000000
10000161c: b4000262    	cbz	x2, 0x100001668 <field_regressions::arithmetic::wide_dot::<1>+0x70>
100001620: 91002068    	add	x8, x3, #0x8
100001624: 91002029    	add	x9, x1, #0x8
100001628: 6f00e403    	movi.2d	v3, #0000000000000000
10000162c: 6d7f9504    	ldp	d4, d5, [x8, #-0x8]
100001630: 6d7f9d26    	ldp	d6, d7, [x9, #-0x8]
100001634: 0ee4e0d0    	pmull.1q	v16, v6, v4
100001638: 0ee5e0f1    	pmull.1q	v17, v7, v5
10000163c: 0ee5e0c5    	pmull.1q	v5, v6, v5
100001640: 0ee4e0e4    	pmull.1q	v4, v7, v4
100001644: 6e251c84    	eor.16b	v4, v4, v5
100001648: 6e044065    	ext.16b	v5, v3, v4, #0x8
10000164c: 6e034084    	ext.16b	v4, v4, v3, #0x8
100001650: ce1004a1    	eor3.16b	v1, v5, v16, v1
100001654: ce110882    	eor3.16b	v2, v4, v17, v2
100001658: 91004108    	add	x8, x8, #0x10
10000165c: 91004129    	add	x9, x9, #0x10
100001660: f1000442    	subs	x2, x2, #0x1
100001664: 54fffe41    	b.ne	0x10000162c <field_regressions::arithmetic::wide_dot::<1>+0x34>
100001668: 528010e8    	mov	w8, #0x87               ; =135
10000166c: 4e080d03    	dup.2d	v3, x8
100001670: 4ee3e044    	pmull2.1q	v4, v2, v3
100001674: 6e044000    	ext.16b	v0, v0, v4, #0x8
100001678: 4ee3e084    	pmull2.1q	v4, v4, v3
10000167c: 0ee3e042    	pmull.1q	v2, v2, v3
100001680: 6e221c21    	eor.16b	v1, v1, v2
100001684: ce001020    	eor3.16b	v0, v1, v0, v4
100001688: 3d800000    	str	q0, [x0]
10000168c: a9417bfd    	ldp	x29, x30, [sp, #0x10]
100001690: 910083ff    	add	sp, sp, #0x20
100001694: d65f03c0    	ret
100001698: f0000503    	adrp	x3, 0x1000a4000 <dyld_stub_binder+0x1000a4000>
10000169c: 9105a063    	add	x3, x3, #0x168
1000016a0: 910003e0    	mov	x0, sp
1000016a4: 910023e1    	add	x1, sp, #0x8
1000016a8: d2800002    	mov	x2, #0x0                ; =0
1000016ac: 9401f21f    	bl	0x10007df28 <core::panicking::assert_failed::<usize, usize>>

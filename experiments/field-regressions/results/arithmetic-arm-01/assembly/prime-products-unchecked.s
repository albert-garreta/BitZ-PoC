
/var/folders/q0/v7l8dzd13k7f_q6j80sy03lm0000gn/T/f2z-field-gated-bkk46c4y/field-regressions:	file format mach-o arm64

Disassembly of section __TEXT,__text:

0000000100005568 <field_regressions::campaign::prime::products::<false>>:
100005568:     	sub	sp, sp, #0x30
10000556c:     	stp	x29, x30, [sp, #0x20]
100005570:     	add	x29, sp, #0x20
100005574:     	str	x4, [sp, #0x8]
100005578:     	stur	x6, [x29, #-0x8]
10000557c:     	cmp	x4, x6
100005580:     	b.ne	0x10000569c <field_regressions::campaign::prime::products::<false>+0x134>
100005584:     	ldr	x8, [x29, #0x10]
100005588:     	str	x4, [sp, #0x10]
10000558c:     	stur	x8, [x29, #-0x8]
100005590:     	cmp	x4, x8
100005594:     	b.ne	0x1000056b4 <field_regressions::campaign::prime::products::<false>+0x14c>
100005598:     	cbz	x4, 0x100005690 <field_regressions::campaign::prime::products::<false>+0x128>
10000559c:     	ldp	x9, x8, [x3], #0x10
1000055a0:     	ldp	x11, x10, [x5], #0x10
1000055a4:     	mul	x12, x11, x9
1000055a8:     	umulh	x13, x11, x9
1000055ac:     	umulh	x14, x10, x9
1000055b0:     	mul	x9, x10, x9
1000055b4:     	mul	x15, x11, x8
1000055b8:     	umulh	x11, x11, x8
1000055bc:     	umulh	x16, x10, x8
1000055c0:     	mul	x8, x10, x8
1000055c4:     	adds	x9, x13, x9
1000055c8:     	cset	w10, hs
1000055cc:     	adds	x11, x11, x14
1000055d0:     	cset	w13, hs
1000055d4:     	adds	x8, x11, x8
1000055d8:     	cinc	x11, x13, hs
1000055dc:     	adds	x9, x9, x15
1000055e0:     	adcs	x8, x8, x10
1000055e4:     	adc	x10, x16, x11
1000055e8:     	mul	x11, x2, x12
1000055ec:     	umulh	x13, x0, x11
1000055f0:     	mul	x14, x0, x11
1000055f4:     	umulh	x15, x1, x11
1000055f8:     	mul	x11, x1, x11
1000055fc:     	adds	x9, x13, x9
100005600:     	cset	w13, hs
100005604:     	adds	x9, x9, x11
100005608:     	cinc	x11, x13, hs
10000560c:     	adds	x8, x8, x15
100005610:     	cset	w13, hs
100005614:     	cmn	x14, x12
100005618:     	adcs	x9, x9, xzr
10000561c:     	adcs	x8, x8, x11
100005620:     	adcs	x10, x10, x13
100005624:     	mul	x11, x2, x9
100005628:     	mul	x12, x0, x11
10000562c:     	umulh	x13, x0, x11
100005630:     	umulh	x14, x1, x11
100005634:     	mul	x11, x1, x11
100005638:     	cset	w15, hs
10000563c:     	adds	x8, x13, x8
100005640:     	cset	w13, hs
100005644:     	adds	x8, x8, x11
100005648:     	cinc	x11, x13, hs
10000564c:     	adds	x10, x10, x14
100005650:     	cset	w13, hs
100005654:     	cmn	x12, x9
100005658:     	adcs	x8, x8, xzr
10000565c:     	adcs	x9, x10, x11
100005660:     	cinc	x10, x13, hs
100005664:     	cmp	x8, x0
100005668:     	sbcs	xzr, x9, x1
10000566c:     	orr	x10, x10, x15
100005670:     	ccmp	x10, #0x0, #0x0, lo
100005674:     	csel	x10, x1, xzr, ne
100005678:     	csel	x11, x0, xzr, ne
10000567c:     	subs	x8, x8, x11
100005680:     	sbc	x9, x9, x10
100005684:     	stp	x8, x9, [x7], #0x10
100005688:     	subs	x4, x4, #0x1
10000568c:     	b.ne	0x10000559c <field_regressions::campaign::prime::products::<false>+0x34>
100005690:     	ldp	x29, x30, [sp, #0x20]
100005694:     	add	sp, sp, #0x30
100005698:     	ret
10000569c:     	adrp	x3, 0x1000b8000 <dyld_stub_binder+0x1000b8000>
1000056a0:     	add	x3, x3, #0x6e8
1000056a4:     	add	x0, sp, #0x8
1000056a8:     	sub	x1, x29, #0x8
1000056ac:     	mov	x2, #0x0                ; =0
1000056b0:     	bl	0x100088d48 <core::panicking::assert_failed::<usize, usize>>
1000056b4:     	adrp	x3, 0x1000b8000 <dyld_stub_binder+0x1000b8000>
1000056b8:     	add	x3, x3, #0x700
1000056bc:     	add	x0, sp, #0x10
1000056c0:     	sub	x1, x29, #0x8
1000056c4:     	mov	x2, #0x0                ; =0
1000056c8:     	bl	0x100088d48 <core::panicking::assert_failed::<usize, usize>>

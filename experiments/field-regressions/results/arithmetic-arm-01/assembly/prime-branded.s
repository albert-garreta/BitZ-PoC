
/var/folders/q0/v7l8dzd13k7f_q6j80sy03lm0000gn/T/f2z-field-gated-bkk46c4y/field-regressions:	file format mach-o arm64

Disassembly of section __TEXT,__text:

00000001000225c0 <field_regressions::campaign::prime::branded>:
1000225c0:     	sub	sp, sp, #0x30
1000225c4:     	stp	x29, x30, [sp, #0x20]
1000225c8:     	add	x29, sp, #0x20
1000225cc:     	str	x4, [sp, #0x8]
1000225d0:     	stur	x6, [x29, #-0x8]
1000225d4:     	cmp	x4, x6
1000225d8:     	b.ne	0x1000226f4 <field_regressions::campaign::prime::branded+0x134>
1000225dc:     	ldr	x8, [x29, #0x10]
1000225e0:     	str	x4, [sp, #0x10]
1000225e4:     	stur	x8, [x29, #-0x8]
1000225e8:     	cmp	x4, x8
1000225ec:     	b.ne	0x10002270c <field_regressions::campaign::prime::branded+0x14c>
1000225f0:     	cbz	x4, 0x1000226e8 <field_regressions::campaign::prime::branded+0x128>
1000225f4:     	ldp	x9, x8, [x3], #0x10
1000225f8:     	ldp	x11, x10, [x5], #0x10
1000225fc:     	mul	x12, x11, x9
100022600:     	umulh	x13, x11, x9
100022604:     	umulh	x14, x10, x9
100022608:     	mul	x9, x10, x9
10002260c:     	mul	x15, x11, x8
100022610:     	umulh	x11, x11, x8
100022614:     	umulh	x16, x10, x8
100022618:     	mul	x8, x10, x8
10002261c:     	adds	x9, x13, x9
100022620:     	cset	w10, hs
100022624:     	adds	x11, x11, x14
100022628:     	cset	w13, hs
10002262c:     	adds	x8, x11, x8
100022630:     	cinc	x11, x13, hs
100022634:     	adds	x9, x9, x15
100022638:     	adcs	x8, x8, x10
10002263c:     	adc	x10, x16, x11
100022640:     	mul	x11, x2, x12
100022644:     	umulh	x13, x0, x11
100022648:     	mul	x14, x0, x11
10002264c:     	umulh	x15, x1, x11
100022650:     	mul	x11, x1, x11
100022654:     	adds	x9, x13, x9
100022658:     	cset	w13, hs
10002265c:     	adds	x9, x9, x11
100022660:     	cinc	x11, x13, hs
100022664:     	adds	x8, x8, x15
100022668:     	cset	w13, hs
10002266c:     	cmn	x14, x12
100022670:     	adcs	x9, x9, xzr
100022674:     	adcs	x8, x8, x11
100022678:     	adcs	x10, x10, x13
10002267c:     	mul	x11, x2, x9
100022680:     	mul	x12, x0, x11
100022684:     	umulh	x13, x0, x11
100022688:     	umulh	x14, x1, x11
10002268c:     	mul	x11, x1, x11
100022690:     	cset	w15, hs
100022694:     	adds	x8, x13, x8
100022698:     	cset	w13, hs
10002269c:     	adds	x8, x8, x11
1000226a0:     	cinc	x11, x13, hs
1000226a4:     	adds	x10, x10, x14
1000226a8:     	cset	w13, hs
1000226ac:     	cmn	x12, x9
1000226b0:     	adcs	x8, x8, xzr
1000226b4:     	adcs	x9, x10, x11
1000226b8:     	cinc	x10, x13, hs
1000226bc:     	cmp	x8, x0
1000226c0:     	sbcs	xzr, x9, x1
1000226c4:     	orr	x10, x10, x15
1000226c8:     	ccmp	x10, #0x0, #0x0, lo
1000226cc:     	csel	x10, x1, xzr, ne
1000226d0:     	csel	x11, x0, xzr, ne
1000226d4:     	subs	x8, x8, x11
1000226d8:     	sbc	x9, x9, x10
1000226dc:     	stp	x8, x9, [x7], #0x10
1000226e0:     	subs	x4, x4, #0x1
1000226e4:     	b.ne	0x1000225f4 <field_regressions::campaign::prime::branded+0x34>
1000226e8:     	ldp	x29, x30, [sp, #0x20]
1000226ec:     	add	sp, sp, #0x30
1000226f0:     	ret
1000226f4:     	adrp	x3, 0x1000b9000 <dyld_stub_binder+0x1000b9000>
1000226f8:     	add	x3, x3, #0xb70
1000226fc:     	add	x0, sp, #0x8
100022700:     	sub	x1, x29, #0x8
100022704:     	mov	x2, #0x0                ; =0
100022708:     	bl	0x100088d48 <core::panicking::assert_failed::<usize, usize>>
10002270c:     	adrp	x3, 0x1000b9000 <dyld_stub_binder+0x1000b9000>
100022710:     	add	x3, x3, #0xb88
100022714:     	add	x0, sp, #0x10
100022718:     	sub	x1, x29, #0x8
10002271c:     	mov	x2, #0x0                ; =0
100022720:     	bl	0x100088d48 <core::panicking::assert_failed::<usize, usize>>

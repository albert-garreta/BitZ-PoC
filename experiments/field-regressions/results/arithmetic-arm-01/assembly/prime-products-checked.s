
/var/folders/q0/v7l8dzd13k7f_q6j80sy03lm0000gn/T/f2z-field-gated-bkk46c4y/field-regressions:	file format mach-o arm64

Disassembly of section __TEXT,__text:

00000001000056cc <field_regressions::campaign::prime::products::<true>>:
1000056cc:     	sub	sp, sp, #0x30
1000056d0:     	stp	x29, x30, [sp, #0x20]
1000056d4:     	add	x29, sp, #0x20
1000056d8:     	ldr	x8, [x29, #0x10]
1000056dc:     	str	x6, [sp, #0x8]
1000056e0:     	stur	x8, [x29, #-0x8]
1000056e4:     	cmp	x6, x8
1000056e8:     	b.ne	0x100005838 <field_regressions::campaign::prime::products::<true>+0x16c>
1000056ec:     	ldr	x8, [x29, #0x20]
1000056f0:     	str	x6, [sp, #0x10]
1000056f4:     	stur	x8, [x29, #-0x8]
1000056f8:     	cmp	x6, x8
1000056fc:     	b.ne	0x100005850 <field_regressions::campaign::prime::products::<true>+0x184>
100005700:     	cbz	x6, 0x100005814 <field_regressions::campaign::prime::products::<true>+0x148>
100005704:     	ldr	x8, [x29, #0x18]
100005708:     	ldp	x10, x9, [x5], #0x10
10000570c:     	cmp	x10, x3
100005710:     	sbcs	xzr, x9, x4
100005714:     	b.hs	0x100005820 <field_regressions::campaign::prime::products::<true>+0x154>
100005718:     	ldp	x12, x11, [x7], #0x10
10000571c:     	cmp	x12, x3
100005720:     	sbcs	xzr, x11, x4
100005724:     	b.hs	0x100005820 <field_regressions::campaign::prime::products::<true>+0x154>
100005728:     	mul	x13, x12, x10
10000572c:     	umulh	x14, x12, x10
100005730:     	umulh	x15, x11, x10
100005734:     	mul	x10, x11, x10
100005738:     	mul	x16, x12, x9
10000573c:     	umulh	x12, x12, x9
100005740:     	umulh	x17, x11, x9
100005744:     	mul	x9, x11, x9
100005748:     	adds	x10, x14, x10
10000574c:     	cset	w11, hs
100005750:     	adds	x12, x12, x15
100005754:     	cset	w14, hs
100005758:     	adds	x9, x12, x9
10000575c:     	cinc	x12, x14, hs
100005760:     	adds	x10, x10, x16
100005764:     	adcs	x9, x9, x11
100005768:     	adc	x11, x17, x12
10000576c:     	mul	x12, x2, x13
100005770:     	mul	x14, x0, x12
100005774:     	umulh	x15, x0, x12
100005778:     	umulh	x16, x1, x12
10000577c:     	mul	x12, x1, x12
100005780:     	adds	x10, x15, x10
100005784:     	cset	w15, hs
100005788:     	adds	x10, x10, x12
10000578c:     	cinc	x12, x15, hs
100005790:     	adds	x9, x9, x16
100005794:     	cset	w15, hs
100005798:     	cmn	x14, x13
10000579c:     	adcs	x10, x10, xzr
1000057a0:     	adcs	x9, x9, x12
1000057a4:     	adcs	x11, x11, x15
1000057a8:     	cset	w12, hs
1000057ac:     	mul	x13, x2, x10
1000057b0:     	mul	x14, x0, x13
1000057b4:     	umulh	x15, x0, x13
1000057b8:     	umulh	x16, x1, x13
1000057bc:     	mul	x13, x1, x13
1000057c0:     	adds	x9, x15, x9
1000057c4:     	cset	w15, hs
1000057c8:     	adds	x9, x9, x13
1000057cc:     	cinc	x13, x15, hs
1000057d0:     	adds	x11, x11, x16
1000057d4:     	cset	w15, hs
1000057d8:     	cmn	x14, x10
1000057dc:     	adcs	x9, x9, xzr
1000057e0:     	adcs	x10, x11, x13
1000057e4:     	cinc	x11, x15, hs
1000057e8:     	cmp	x9, x0
1000057ec:     	sbcs	xzr, x10, x1
1000057f0:     	orr	x11, x11, x12
1000057f4:     	ccmp	x11, #0x0, #0x0, lo
1000057f8:     	csel	x11, x1, xzr, ne
1000057fc:     	csel	x12, x0, xzr, ne
100005800:     	subs	x9, x9, x12
100005804:     	sbc	x10, x10, x11
100005808:     	stp	x9, x10, [x8], #0x10
10000580c:     	subs	x6, x6, #0x1
100005810:     	b.ne	0x100005708 <field_regressions::campaign::prime::products::<true>+0x3c>
100005814:     	ldp	x29, x30, [sp, #0x20]
100005818:     	add	sp, sp, #0x30
10000581c:     	ret
100005820:     	adrp	x0, 0x100096000 <GCC_except_table992+0x18>
100005824:     	add	x0, x0, #0xf72
100005828:     	adrp	x2, 0x1000b8000 <dyld_stub_binder+0x1000b8000>
10000582c:     	add	x2, x2, #0x718
100005830:     	mov	w1, #0x20               ; =32
100005834:     	bl	0x100088e64 <core::panicking::panic>
100005838:     	adrp	x3, 0x1000b8000 <dyld_stub_binder+0x1000b8000>
10000583c:     	add	x3, x3, #0x6e8
100005840:     	add	x0, sp, #0x8
100005844:     	sub	x1, x29, #0x8
100005848:     	mov	x2, #0x0                ; =0
10000584c:     	bl	0x100088d48 <core::panicking::assert_failed::<usize, usize>>
100005850:     	adrp	x3, 0x1000b8000 <dyld_stub_binder+0x1000b8000>
100005854:     	add	x3, x3, #0x700
100005858:     	add	x0, sp, #0x10
10000585c:     	sub	x1, x29, #0x8
100005860:     	mov	x2, #0x0                ; =0
100005864:     	bl	0x100088d48 <core::panicking::assert_failed::<usize, usize>>

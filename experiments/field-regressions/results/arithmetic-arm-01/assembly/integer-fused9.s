
/var/folders/q0/v7l8dzd13k7f_q6j80sy03lm0000gn/T/f2z-field-gated-bkk46c4y/field-regressions:	file format mach-o arm64

Disassembly of section __TEXT,__text:

0000000100006734 <field_regressions::campaign::integer::mac::<9, false>>:
100006734:     	sub	sp, sp, #0x60
100006738:     	stp	x26, x25, [sp, #0x10]
10000673c:     	stp	x24, x23, [sp, #0x20]
100006740:     	stp	x22, x21, [sp, #0x30]
100006744:     	stp	x20, x19, [sp, #0x40]
100006748:     	stp	x29, x30, [sp, #0x50]
10000674c:     	add	x29, sp, #0x50
100006750:     	stp	x2, x4, [sp]
100006754:     	cmp	x2, x4
100006758:     	b.ne	0x100006b5c <field_regressions::campaign::integer::mac::<9, false>+0x428>
10000675c:     	cmp	x2, #0x100, lsl #12     ; =0x100000
100006760:     	b.hi	0x100006b74 <field_regressions::campaign::integer::mac::<9, false>+0x440>
100006764:     	mov	x8, #0x0                ; =0
100006768:     	mov	x11, #0x0               ; =0
10000676c:     	mov	x14, #0x0               ; =0
100006770:     	mov	x15, #0x0               ; =0
100006774:     	mov	x4, #0x0                ; =0
100006778:     	mov	x5, #0x0                ; =0
10000677c:     	mov	x6, #0x0                ; =0
100006780:     	mov	x7, #0x0                ; =0
100006784:     	mov	x19, #0x0               ; =0
100006788:     	cbz	x2, 0x100006b2c <field_regressions::campaign::integer::mac::<9, false>+0x3f8>
10000678c:     	add	x9, x3, #0x20
100006790:     	add	x10, x1, #0x20
100006794:     	ldp	x21, x20, [x10, #-0x20]
100006798:     	ldp	x12, x13, [x9, #-0x20]
10000679c:     	umulh	x16, x12, x21
1000067a0:     	mul	x17, x12, x21
1000067a4:     	adds	x8, x17, x8
1000067a8:     	umulh	x17, x13, x21
1000067ac:     	cinc	x16, x16, hs
1000067b0:     	mul	x1, x13, x21
1000067b4:     	adds	x11, x16, x11
1000067b8:     	cset	w3, hs
1000067bc:     	adds	x11, x11, x1
1000067c0:     	adc	x1, x3, x17
1000067c4:     	ldp	x16, x17, [x9, #-0x10]
1000067c8:     	umulh	x3, x16, x21
1000067cc:     	mul	x22, x16, x21
1000067d0:     	adds	x14, x1, x14
1000067d4:     	cset	w1, hs
1000067d8:     	adds	x22, x14, x22
1000067dc:     	adc	x14, x1, x3
1000067e0:     	umulh	x1, x17, x21
1000067e4:     	mul	x3, x17, x21
1000067e8:     	adds	x14, x14, x15
1000067ec:     	cset	w15, hs
1000067f0:     	adds	x23, x14, x3
1000067f4:     	adc	x14, x15, x1
1000067f8:     	ldp	x1, x3, [x9]
1000067fc:     	umulh	x15, x1, x21
100006800:     	mul	x24, x1, x21
100006804:     	adds	x14, x14, x4
100006808:     	cset	w4, hs
10000680c:     	adds	x24, x14, x24
100006810:     	adc	x14, x4, x15
100006814:     	umulh	x4, x3, x21
100006818:     	mul	x15, x3, x21
10000681c:     	adds	x14, x14, x5
100006820:     	cset	w5, hs
100006824:     	adds	x25, x14, x15
100006828:     	ldp	x15, x14, [x9, #0x10]
10000682c:     	umulh	x26, x15, x21
100006830:     	adc	x4, x5, x4
100006834:     	mul	x5, x15, x21
100006838:     	adds	x4, x4, x6
10000683c:     	cset	w6, hs
100006840:     	adds	x5, x4, x5
100006844:     	umulh	x4, x14, x21
100006848:     	adc	x6, x6, x26
10000684c:     	mul	x26, x14, x21
100006850:     	adds	x6, x6, x7
100006854:     	cset	w7, hs
100006858:     	adds	x26, x6, x26
10000685c:     	ldr	x6, [x9, #0x20]
100006860:     	adc	x4, x7, x4
100006864:     	add	x4, x4, x19
100006868:     	umulh	x7, x12, x20
10000686c:     	mul	x19, x12, x20
100006870:     	adds	x11, x11, x19
100006874:     	cinc	x7, x7, hs
100006878:     	madd	x19, x6, x21, x4
10000687c:     	umulh	x6, x13, x20
100006880:     	mul	x4, x13, x20
100006884:     	adds	x7, x7, x22
100006888:     	cset	w21, hs
10000688c:     	adds	x4, x7, x4
100006890:     	adc	x6, x21, x6
100006894:     	umulh	x7, x16, x20
100006898:     	mul	x21, x16, x20
10000689c:     	adds	x6, x6, x23
1000068a0:     	cset	w22, hs
1000068a4:     	adds	x6, x6, x21
1000068a8:     	adc	x7, x22, x7
1000068ac:     	umulh	x21, x17, x20
1000068b0:     	mul	x22, x17, x20
1000068b4:     	adds	x7, x7, x24
1000068b8:     	cset	w23, hs
1000068bc:     	adds	x7, x7, x22
1000068c0:     	adc	x21, x23, x21
1000068c4:     	umulh	x22, x1, x20
1000068c8:     	mul	x23, x1, x20
1000068cc:     	adds	x21, x21, x25
1000068d0:     	cset	w24, hs
1000068d4:     	adds	x21, x21, x23
1000068d8:     	adc	x22, x24, x22
1000068dc:     	umulh	x23, x3, x20
1000068e0:     	mul	x24, x3, x20
1000068e4:     	adds	x5, x22, x5
1000068e8:     	cset	w25, hs
1000068ec:     	adds	x22, x5, x24
1000068f0:     	adc	x5, x25, x23
1000068f4:     	umulh	x24, x15, x20
1000068f8:     	mul	x23, x15, x20
1000068fc:     	adds	x5, x5, x26
100006900:     	cset	w25, hs
100006904:     	adds	x23, x5, x23
100006908:     	adc	x5, x25, x24
10000690c:     	add	x24, x5, x19
100006910:     	ldp	x19, x5, [x10, #-0x10]
100006914:     	umulh	x25, x12, x19
100006918:     	mul	x26, x12, x19
10000691c:     	madd	x20, x14, x20, x24
100006920:     	adds	x14, x4, x26
100006924:     	cinc	x4, x25, hs
100006928:     	umulh	x24, x13, x19
10000692c:     	mul	x25, x13, x19
100006930:     	adds	x4, x4, x6
100006934:     	cset	w6, hs
100006938:     	adds	x25, x4, x25
10000693c:     	adc	x4, x6, x24
100006940:     	umulh	x6, x16, x19
100006944:     	mul	x24, x16, x19
100006948:     	adds	x4, x4, x7
10000694c:     	cset	w7, hs
100006950:     	adds	x24, x4, x24
100006954:     	adc	x4, x7, x6
100006958:     	umulh	x6, x17, x19
10000695c:     	mul	x7, x17, x19
100006960:     	adds	x4, x4, x21
100006964:     	cset	w21, hs
100006968:     	adds	x7, x4, x7
10000696c:     	adc	x4, x21, x6
100006970:     	umulh	x6, x1, x19
100006974:     	mul	x21, x1, x19
100006978:     	adds	x4, x4, x22
10000697c:     	cset	w22, hs
100006980:     	adds	x21, x4, x21
100006984:     	adc	x4, x22, x6
100006988:     	umulh	x6, x3, x19
10000698c:     	mul	x22, x3, x19
100006990:     	adds	x4, x4, x23
100006994:     	cset	w23, hs
100006998:     	adds	x22, x4, x22
10000699c:     	adc	x4, x23, x6
1000069a0:     	add	x4, x4, x20
1000069a4:     	umulh	x6, x12, x5
1000069a8:     	mul	x20, x12, x5
1000069ac:     	madd	x4, x15, x19, x4
1000069b0:     	adds	x15, x25, x20
1000069b4:     	cinc	x6, x6, hs
1000069b8:     	umulh	x19, x13, x5
1000069bc:     	mul	x20, x13, x5
1000069c0:     	adds	x6, x6, x24
1000069c4:     	cset	w23, hs
1000069c8:     	adds	x20, x6, x20
1000069cc:     	adc	x6, x23, x19
1000069d0:     	umulh	x19, x16, x5
1000069d4:     	mul	x23, x16, x5
1000069d8:     	adds	x6, x6, x7
1000069dc:     	cset	w7, hs
1000069e0:     	adds	x23, x6, x23
1000069e4:     	adc	x6, x7, x19
1000069e8:     	umulh	x7, x17, x5
1000069ec:     	mul	x19, x17, x5
1000069f0:     	adds	x6, x6, x21
1000069f4:     	cset	w21, hs
1000069f8:     	adds	x19, x6, x19
1000069fc:     	adc	x6, x21, x7
100006a00:     	umulh	x7, x1, x5
100006a04:     	mul	x21, x1, x5
100006a08:     	adds	x6, x6, x22
100006a0c:     	cset	w22, hs
100006a10:     	adds	x21, x6, x21
100006a14:     	adc	x22, x22, x7
100006a18:     	ldp	x6, x7, [x10]
100006a1c:     	umulh	x24, x12, x6
100006a20:     	add	x22, x22, x4
100006a24:     	mul	x4, x12, x6
100006a28:     	adds	x4, x20, x4
100006a2c:     	cinc	x20, x24, hs
100006a30:     	umulh	x24, x13, x6
100006a34:     	mul	x25, x13, x6
100006a38:     	madd	x3, x3, x5, x22
100006a3c:     	adds	x5, x20, x23
100006a40:     	cset	w20, hs
100006a44:     	adds	x5, x5, x25
100006a48:     	umulh	x22, x16, x6
100006a4c:     	mul	x23, x16, x6
100006a50:     	adc	x20, x20, x24
100006a54:     	adds	x19, x20, x19
100006a58:     	cset	w20, hs
100006a5c:     	adds	x19, x19, x23
100006a60:     	umulh	x23, x17, x6
100006a64:     	mul	x24, x17, x6
100006a68:     	adc	x20, x20, x22
100006a6c:     	adds	x20, x20, x21
100006a70:     	cset	w21, hs
100006a74:     	adds	x20, x20, x24
100006a78:     	adc	x21, x21, x23
100006a7c:     	umulh	x22, x12, x7
100006a80:     	add	x3, x21, x3
100006a84:     	mul	x21, x12, x7
100006a88:     	adds	x5, x5, x21
100006a8c:     	cinc	x21, x22, hs
100006a90:     	umulh	x22, x13, x7
100006a94:     	mul	x23, x13, x7
100006a98:     	madd	x1, x1, x6, x3
100006a9c:     	adds	x3, x21, x19
100006aa0:     	cset	w6, hs
100006aa4:     	adds	x3, x3, x23
100006aa8:     	umulh	x19, x16, x7
100006aac:     	mul	x21, x16, x7
100006ab0:     	adc	x6, x6, x22
100006ab4:     	adds	x6, x6, x20
100006ab8:     	cset	w20, hs
100006abc:     	adds	x21, x6, x21
100006ac0:     	adc	x6, x20, x19
100006ac4:     	ldp	x19, x20, [x10, #0x10]
100006ac8:     	add	x1, x6, x1
100006acc:     	umulh	x22, x12, x19
100006ad0:     	mul	x6, x12, x19
100006ad4:     	adds	x6, x3, x6
100006ad8:     	cinc	x3, x22, hs
100006adc:     	madd	x17, x17, x7, x1
100006ae0:     	umulh	x1, x13, x19
100006ae4:     	mul	x7, x13, x19
100006ae8:     	adds	x3, x3, x21
100006aec:     	cset	w21, hs
100006af0:     	adds	x3, x3, x7
100006af4:     	adc	x1, x21, x1
100006af8:     	add	x17, x1, x17
100006afc:     	madd	x16, x16, x19, x17
100006b00:     	umulh	x17, x12, x20
100006b04:     	mul	x1, x12, x20
100006b08:     	adds	x7, x3, x1
100006b0c:     	adc	x16, x16, x17
100006b10:     	madd	x13, x13, x20, x16
100006b14:     	ldr	x16, [x10, #0x20]
100006b18:     	madd	x19, x12, x16, x13
100006b1c:     	add	x9, x9, #0x48
100006b20:     	add	x10, x10, #0x48
100006b24:     	subs	x2, x2, #0x1
100006b28:     	b.ne	0x100006794 <field_regressions::campaign::integer::mac::<9, false>+0x60>
100006b2c:     	stp	x8, x11, [x0]
100006b30:     	stp	x14, x15, [x0, #0x10]
100006b34:     	stp	x4, x5, [x0, #0x20]
100006b38:     	stp	x6, x7, [x0, #0x30]
100006b3c:     	str	x19, [x0, #0x40]
100006b40:     	ldp	x29, x30, [sp, #0x50]
100006b44:     	ldp	x20, x19, [sp, #0x40]
100006b48:     	ldp	x22, x21, [sp, #0x30]
100006b4c:     	ldp	x24, x23, [sp, #0x20]
100006b50:     	ldp	x26, x25, [sp, #0x10]
100006b54:     	add	sp, sp, #0x60
100006b58:     	ret
100006b5c:     	adrp	x3, 0x1000b8000 <dyld_stub_binder+0x1000b8000>
100006b60:     	add	x3, x3, #0x910
100006b64:     	mov	x0, sp
100006b68:     	add	x1, sp, #0x8
100006b6c:     	mov	x2, #0x0                ; =0
100006b70:     	bl	0x100088d48 <core::panicking::assert_failed::<usize, usize>>
100006b74:     	adrp	x0, 0x100096000 <GCC_except_table992+0x18>
100006b78:     	add	x0, x0, #0xfc7
100006b7c:     	adrp	x2, 0x1000b8000 <dyld_stub_binder+0x1000b8000>
100006b80:     	add	x2, x2, #0x928
100006b84:     	mov	w1, #0x24               ; =36
100006b88:     	bl	0x100088e64 <core::panicking::panic>

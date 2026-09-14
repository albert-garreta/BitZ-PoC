
/var/folders/q0/v7l8dzd13k7f_q6j80sy03lm0000gn/T/f2z-field-gated-bkk46c4y/field-regressions:	file format mach-o arm64

Disassembly of section __TEXT,__text:

0000000100005ce0 <field_regressions::campaign::integer::wide_products::<4>>:
100005ce0:     	sub	sp, sp, #0x40
100005ce4:     	stp	x20, x19, [sp, #0x20]
100005ce8:     	stp	x29, x30, [sp, #0x30]
100005cec:     	add	x29, sp, #0x30
100005cf0:     	str	x1, [sp, #0x8]
100005cf4:     	str	x3, [sp, #0x18]
100005cf8:     	cmp	x1, x3
100005cfc:     	b.ne	0x100005ea8 <field_regressions::campaign::integer::wide_products::<4>+0x1c8>
100005d00:     	stp	x1, x5, [sp, #0x10]
100005d04:     	cmp	x1, x5
100005d08:     	b.ne	0x100005ec0 <field_regressions::campaign::integer::wide_products::<4>+0x1e0>
100005d0c:     	cbz	x1, 0x100005e98 <field_regressions::campaign::integer::wide_products::<4>+0x1b8>
100005d10:     	add	x8, x4, #0x20
100005d14:     	add	x9, x0, #0x10
100005d18:     	add	x10, x2, #0x10
100005d1c:     	movi.2d	v0, #0000000000000000
100005d20:     	ldp	x15, x2, [x9, #-0x10]
100005d24:     	ldp	x0, x11, [x9], #0x48
100005d28:     	ldp	x17, x16, [x10, #-0x10]
100005d2c:     	ldp	x14, x12, [x10], #0x48
100005d30:     	mul	x13, x17, x15
100005d34:     	umulh	x3, x17, x15
100005d38:     	umulh	x4, x16, x15
100005d3c:     	mul	x5, x16, x15
100005d40:     	adds	x3, x3, x5
100005d44:     	umulh	x5, x14, x15
100005d48:     	cinc	x4, x4, hs
100005d4c:     	mul	x6, x14, x15
100005d50:     	adds	x4, x4, x6
100005d54:     	cinc	x5, x5, hs
100005d58:     	umulh	x6, x12, x15
100005d5c:     	mul	x15, x12, x15
100005d60:     	adds	x5, x5, x15
100005d64:     	cinc	x6, x6, hs
100005d68:     	umulh	x7, x17, x2
100005d6c:     	mul	x15, x17, x2
100005d70:     	adds	x15, x3, x15
100005d74:     	umulh	x3, x16, x2
100005d78:     	cinc	x7, x7, hs
100005d7c:     	mul	x19, x16, x2
100005d80:     	adds	x4, x7, x4
100005d84:     	cset	w7, hs
100005d88:     	adds	x4, x4, x19
100005d8c:     	umulh	x19, x14, x2
100005d90:     	adc	x3, x7, x3
100005d94:     	mul	x7, x14, x2
100005d98:     	adds	x3, x3, x5
100005d9c:     	cset	w5, hs
100005da0:     	adds	x3, x3, x7
100005da4:     	umulh	x7, x12, x2
100005da8:     	adc	x5, x5, x19
100005dac:     	mul	x2, x12, x2
100005db0:     	adds	x5, x5, x6
100005db4:     	cset	w6, hs
100005db8:     	adds	x2, x5, x2
100005dbc:     	umulh	x5, x17, x0
100005dc0:     	adc	x6, x6, x7
100005dc4:     	mul	x7, x17, x0
100005dc8:     	adds	x4, x4, x7
100005dcc:     	cinc	x5, x5, hs
100005dd0:     	umulh	x7, x16, x0
100005dd4:     	mul	x19, x16, x0
100005dd8:     	adds	x3, x5, x3
100005ddc:     	cset	w5, hs
100005de0:     	adds	x3, x3, x19
100005de4:     	adc	x5, x5, x7
100005de8:     	umulh	x7, x14, x0
100005dec:     	mul	x19, x14, x0
100005df0:     	adds	x2, x5, x2
100005df4:     	cset	w5, hs
100005df8:     	adds	x2, x2, x19
100005dfc:     	adc	x5, x5, x7
100005e00:     	umulh	x7, x12, x0
100005e04:     	mul	x0, x12, x0
100005e08:     	adds	x5, x5, x6
100005e0c:     	cset	w6, hs
100005e10:     	adds	x0, x5, x0
100005e14:     	adc	x5, x6, x7
100005e18:     	umulh	x6, x17, x11
100005e1c:     	mul	x17, x17, x11
100005e20:     	adds	x17, x3, x17
100005e24:     	cinc	x3, x6, hs
100005e28:     	umulh	x6, x16, x11
100005e2c:     	mul	x16, x16, x11
100005e30:     	adds	x2, x3, x2
100005e34:     	cset	w3, hs
100005e38:     	adds	x16, x2, x16
100005e3c:     	adc	x2, x3, x6
100005e40:     	umulh	x3, x14, x11
100005e44:     	mul	x14, x14, x11
100005e48:     	adds	x0, x2, x0
100005e4c:     	cset	w2, hs
100005e50:     	adds	x14, x0, x14
100005e54:     	umulh	x0, x12, x11
100005e58:     	stp	x13, x15, [x8, #-0x20]
100005e5c:     	stp	x4, x17, [x8, #-0x10]
100005e60:     	adc	x13, x2, x3
100005e64:     	mul	x11, x12, x11
100005e68:     	stp	x16, x14, [x8]
100005e6c:     	stp	q0, q0, [x8, #0x20]
100005e70:     	adds	x12, x13, x5
100005e74:     	cset	w13, hs
100005e78:     	stp	q0, q0, [x8, #0x40]
100005e7c:     	adds	x11, x12, x11
100005e80:     	adc	x12, x13, x0
100005e84:     	stp	x11, x12, [x8, #0x10]
100005e88:     	str	q0, [x8, #0x60]
100005e8c:     	add	x8, x8, #0x90
100005e90:     	subs	x1, x1, #0x1
100005e94:     	b.ne	0x100005d20 <field_regressions::campaign::integer::wide_products::<4>+0x40>
100005e98:     	ldp	x29, x30, [sp, #0x30]
100005e9c:     	ldp	x20, x19, [sp, #0x20]
100005ea0:     	add	sp, sp, #0x40
100005ea4:     	ret
100005ea8:     	adrp	x3, 0x1000b8000 <dyld_stub_binder+0x1000b8000>
100005eac:     	add	x3, x3, #0x8e0
100005eb0:     	add	x0, sp, #0x8
100005eb4:     	add	x1, sp, #0x18
100005eb8:     	mov	x2, #0x0                ; =0
100005ebc:     	bl	0x100088d48 <core::panicking::assert_failed::<usize, usize>>
100005ec0:     	adrp	x3, 0x1000b8000 <dyld_stub_binder+0x1000b8000>
100005ec4:     	add	x3, x3, #0x8f8
100005ec8:     	add	x0, sp, #0x10
100005ecc:     	add	x1, sp, #0x18
100005ed0:     	mov	x2, #0x0                ; =0
100005ed4:     	bl	0x100088d48 <core::panicking::assert_failed::<usize, usize>>

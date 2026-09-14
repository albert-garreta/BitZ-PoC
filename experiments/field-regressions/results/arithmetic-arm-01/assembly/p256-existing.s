
/var/folders/q0/v7l8dzd13k7f_q6j80sy03lm0000gn/T/f2z-field-gated-bkk46c4y/field-regressions:	file format mach-o arm64

Disassembly of section __TEXT,__text:

0000000100005c10 <field_regressions::campaign::integer::wide_products::<0>>:
100005c10:     	sub	sp, sp, #0xd0
100005c14:     	stp	x22, x21, [sp, #0xa0]
100005c18:     	stp	x20, x19, [sp, #0xb0]
100005c1c:     	stp	x29, x30, [sp, #0xc0]
100005c20:     	add	x29, sp, #0xc0
100005c24:     	str	x1, [sp]
100005c28:     	str	x3, [sp, #0x10]
100005c2c:     	cmp	x1, x3
100005c30:     	b.ne	0x100005cb0 <field_regressions::campaign::integer::wide_products::<0>+0xa0>
100005c34:     	mov	x21, x1
100005c38:     	stp	x1, x5, [sp, #0x8]
100005c3c:     	cmp	x1, x5
100005c40:     	b.ne	0x100005cc8 <field_regressions::campaign::integer::wide_products::<0>+0xb8>
100005c44:     	cbz	x21, 0x100005c9c <field_regressions::campaign::integer::wide_products::<0>+0x8c>
100005c48:     	mov	x19, x4
100005c4c:     	mov	x20, x2
100005c50:     	mov	x22, x0
100005c54:     	add	x0, sp, #0x10
100005c58:     	mov	x1, x22
100005c5c:     	mov	x2, x20
100005c60:     	bl	0x100019e94 <field_regressions::production_p256::product>
100005c64:     	ldp	q0, q1, [sp, #0x70]
100005c68:     	stp	q0, q1, [x19, #0x60]
100005c6c:     	ldr	q0, [sp, #0x90]
100005c70:     	str	q0, [x19, #0x80]
100005c74:     	ldp	q0, q1, [sp, #0x30]
100005c78:     	stp	q0, q1, [x19, #0x20]
100005c7c:     	ldp	q1, q0, [sp, #0x50]
100005c80:     	stp	q1, q0, [x19, #0x40]
100005c84:     	ldp	q1, q0, [sp, #0x10]
100005c88:     	stp	q1, q0, [x19], #0x90
100005c8c:     	add	x20, x20, #0x48
100005c90:     	add	x22, x22, #0x48
100005c94:     	subs	x21, x21, #0x1
100005c98:     	b.ne	0x100005c54 <field_regressions::campaign::integer::wide_products::<0>+0x44>
100005c9c:     	ldp	x29, x30, [sp, #0xc0]
100005ca0:     	ldp	x20, x19, [sp, #0xb0]
100005ca4:     	ldp	x22, x21, [sp, #0xa0]
100005ca8:     	add	sp, sp, #0xd0
100005cac:     	ret
100005cb0:     	adrp	x3, 0x1000b8000 <dyld_stub_binder+0x1000b8000>
100005cb4:     	add	x3, x3, #0x8e0
100005cb8:     	mov	x0, sp
100005cbc:     	add	x1, sp, #0x10
100005cc0:     	mov	x2, #0x0                ; =0
100005cc4:     	bl	0x100088d48 <core::panicking::assert_failed::<usize, usize>>
100005cc8:     	adrp	x3, 0x1000b8000 <dyld_stub_binder+0x1000b8000>
100005ccc:     	add	x3, x3, #0x8f8
100005cd0:     	add	x0, sp, #0x8
100005cd4:     	add	x1, sp, #0x10
100005cd8:     	mov	x2, #0x0                ; =0
100005cdc:     	bl	0x100088d48 <core::panicking::assert_failed::<usize, usize>>

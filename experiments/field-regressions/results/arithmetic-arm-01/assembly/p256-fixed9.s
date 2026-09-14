
/var/folders/q0/v7l8dzd13k7f_q6j80sy03lm0000gn/T/f2z-field-gated-bkk46c4y/field-regressions:	file format mach-o arm64

Disassembly of section __TEXT,__text:

0000000100005ed8 <field_regressions::campaign::integer::wide_products::<9>>:
100005ed8:     	sub	sp, sp, #0x150
100005edc:     	stp	x28, x27, [sp, #0xf0]
100005ee0:     	stp	x26, x25, [sp, #0x100]
100005ee4:     	stp	x24, x23, [sp, #0x110]
100005ee8:     	stp	x22, x21, [sp, #0x120]
100005eec:     	stp	x20, x19, [sp, #0x130]
100005ef0:     	stp	x29, x30, [sp, #0x140]
100005ef4:     	add	x29, sp, #0x140
100005ef8:     	str	x1, [sp, #0x8]
100005efc:     	str	x3, [sp, #0x60]
100005f00:     	cmp	x1, x3
100005f04:     	b.ne	0x1000060ec <field_regressions::campaign::integer::wide_products::<9>+0x214>
100005f08:     	str	x1, [sp, #0x10]
100005f0c:     	str	x5, [sp, #0x60]
100005f10:     	cmp	x1, x5
100005f14:     	b.ne	0x100006104 <field_regressions::campaign::integer::wide_products::<9>+0x22c>
100005f18:     	cbz	x1, 0x1000060cc <field_regressions::campaign::integer::wide_products::<9>+0x1f4>
100005f1c:     	mov	x12, #0x0               ; =0
100005f20:     	add	x8, sp, #0x60
100005f24:     	movi.2d	v0, #0000000000000000
100005f28:     	add	x9, sp, #0x10
100005f2c:     	add	x10, sp, #0x60
100005f30:     	mov	w11, #0x90              ; =144
100005f34:     	mov	x13, #0x0               ; =0
100005f38:     	add	x14, x12, x12, lsl #3
100005f3c:     	lsl	x14, x14, #3
100005f40:     	add	x15, x0, x14
100005f44:     	ldp	q1, q2, [x15, #0x20]
100005f48:     	add	x19, x2, x14
100005f4c:     	stp	q1, q2, [sp, #0x30]
100005f50:     	ldr	x14, [x15, #0x40]
100005f54:     	str	x14, [sp, #0x50]
100005f58:     	ldp	q2, q1, [x15]
100005f5c:     	stp	q2, q1, [sp, #0x10]
100005f60:     	ldp	x14, x15, [x19]
100005f64:     	ldp	x16, x17, [x19, #0x10]
100005f68:     	ldp	x3, x5, [x19, #0x20]
100005f6c:     	ldp	x6, x7, [x19, #0x30]
100005f70:     	ldr	x19, [x19, #0x40]
100005f74:     	stp	q0, q0, [x8, #0x70]
100005f78:     	stp	q0, q0, [x8, #0x50]
100005f7c:     	stp	q0, q0, [x8, #0x30]
100005f80:     	stp	q0, q0, [sp, #0x70]
100005f84:     	str	q0, [sp, #0x60]
100005f88:     	ldr	x21, [x9, x13]
100005f8c:     	add	x20, x10, x13
100005f90:     	umulh	x22, x14, x21
100005f94:     	mul	x23, x14, x21
100005f98:     	ldp	x24, x25, [x20]
100005f9c:     	adds	x23, x23, x24
100005fa0:     	mul	x24, x15, x21
100005fa4:     	cinc	x22, x22, hs
100005fa8:     	adds	x22, x22, x25
100005fac:     	cset	w25, hs
100005fb0:     	adds	x22, x22, x24
100005fb4:     	stp	x23, x22, [x20]
100005fb8:     	umulh	x23, x15, x21
100005fbc:     	adc	x23, x25, x23
100005fc0:     	umulh	x22, x16, x21
100005fc4:     	mul	x24, x16, x21
100005fc8:     	ldp	x25, x26, [x20, #0x10]
100005fcc:     	adds	x23, x23, x25
100005fd0:     	cset	w25, hs
100005fd4:     	adds	x23, x23, x24
100005fd8:     	adc	x22, x25, x22
100005fdc:     	umulh	x24, x17, x21
100005fe0:     	mul	x25, x17, x21
100005fe4:     	adds	x22, x22, x26
100005fe8:     	cset	w26, hs
100005fec:     	adds	x22, x22, x25
100005ff0:     	adc	x24, x26, x24
100005ff4:     	umulh	x25, x3, x21
100005ff8:     	stp	x23, x22, [x20, #0x10]
100005ffc:     	mul	x22, x3, x21
100006000:     	ldp	x23, x26, [x20, #0x20]
100006004:     	adds	x23, x24, x23
100006008:     	cset	w24, hs
10000600c:     	adds	x22, x23, x22
100006010:     	adc	x23, x24, x25
100006014:     	umulh	x24, x5, x21
100006018:     	mul	x25, x5, x21
10000601c:     	adds	x23, x23, x26
100006020:     	cset	w26, hs
100006024:     	adds	x23, x23, x25
100006028:     	adc	x24, x26, x24
10000602c:     	umulh	x25, x6, x21
100006030:     	mul	x26, x6, x21
100006034:     	stp	x22, x23, [x20, #0x20]
100006038:     	ldp	x27, x22, [x20, #0x30]
10000603c:     	adds	x23, x24, x27
100006040:     	cset	w24, hs
100006044:     	adds	x23, x23, x26
100006048:     	mul	x26, x7, x21
10000604c:     	adc	x24, x24, x25
100006050:     	adds	x22, x24, x22
100006054:     	cset	w24, hs
100006058:     	adds	x22, x22, x26
10000605c:     	stp	x23, x22, [x20, #0x30]
100006060:     	umulh	x23, x7, x21
100006064:     	adc	x23, x24, x23
100006068:     	umulh	x22, x19, x21
10000606c:     	mul	x21, x19, x21
100006070:     	ldr	x24, [x20, #0x40]
100006074:     	adds	x23, x23, x24
100006078:     	cset	w24, hs
10000607c:     	adds	x21, x23, x21
100006080:     	adc	x22, x24, x22
100006084:     	stp	x21, x22, [x20, #0x40]
100006088:     	add	x13, x13, #0x8
10000608c:     	cmp	x13, #0x48
100006090:     	b.ne	0x100005f88 <field_regressions::campaign::integer::wide_products::<9>+0xb0>
100006094:     	ldp	q1, q2, [x8, #0x50]
100006098:     	madd	x13, x12, x11, x4
10000609c:     	ldp	q3, q4, [x8, #0x70]
1000060a0:     	stp	q2, q3, [x13, #0x60]
1000060a4:     	str	q4, [x13, #0x80]
1000060a8:     	ldr	q2, [sp, #0x80]
1000060ac:     	ldp	q3, q4, [x8, #0x30]
1000060b0:     	stp	q2, q3, [x13, #0x20]
1000060b4:     	add	x12, x12, #0x1
1000060b8:     	stp	q4, q1, [x13, #0x40]
1000060bc:     	ldp	q2, q1, [sp, #0x60]
1000060c0:     	stp	q2, q1, [x13]
1000060c4:     	cmp	x12, x1
1000060c8:     	b.ne	0x100005f34 <field_regressions::campaign::integer::wide_products::<9>+0x5c>
1000060cc:     	ldp	x29, x30, [sp, #0x140]
1000060d0:     	ldp	x20, x19, [sp, #0x130]
1000060d4:     	ldp	x22, x21, [sp, #0x120]
1000060d8:     	ldp	x24, x23, [sp, #0x110]
1000060dc:     	ldp	x26, x25, [sp, #0x100]
1000060e0:     	ldp	x28, x27, [sp, #0xf0]
1000060e4:     	add	sp, sp, #0x150
1000060e8:     	ret
1000060ec:     	adrp	x3, 0x1000b8000 <dyld_stub_binder+0x1000b8000>
1000060f0:     	add	x3, x3, #0x8e0
1000060f4:     	add	x0, sp, #0x8
1000060f8:     	add	x1, sp, #0x60
1000060fc:     	mov	x2, #0x0                ; =0
100006100:     	bl	0x100088d48 <core::panicking::assert_failed::<usize, usize>>
100006104:     	adrp	x3, 0x1000b8000 <dyld_stub_binder+0x1000b8000>
100006108:     	add	x3, x3, #0x8f8
10000610c:     	add	x0, sp, #0x10
100006110:     	add	x1, sp, #0x60
100006114:     	mov	x2, #0x0                ; =0
100006118:     	bl	0x100088d48 <core::panicking::assert_failed::<usize, usize>>

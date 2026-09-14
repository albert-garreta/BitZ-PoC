
/var/folders/q0/v7l8dzd13k7f_q6j80sy03lm0000gn/T/f2z-field-gated-bkk46c4y/field-regressions:	file format mach-o arm64

Disassembly of section __TEXT,__text:

0000000100019e94 <field_regressions::production_p256::product>:
100019e94:     	sub	sp, sp, #0xc0
100019e98:     	stp	x24, x23, [sp, #0x90]
100019e9c:     	stp	x22, x21, [sp, #0xa0]
100019ea0:     	stp	x20, x19, [sp, #0xb0]
100019ea4:     	ldp	x8, x9, [x2]
100019ea8:     	ldp	x11, x12, [x2, #0x10]
100019eac:     	ldp	x13, x14, [x2, #0x20]
100019eb0:     	ldp	x15, x16, [x2, #0x30]
100019eb4:     	ldr	x17, [x2, #0x40]
100019eb8:     	movi.2d	v0, #0000000000000000
100019ebc:     	stp	q0, q0, [sp]
100019ec0:     	stp	q0, q0, [sp, #0x20]
100019ec4:     	stp	q0, q0, [sp, #0x40]
100019ec8:     	stp	q0, q0, [sp, #0x60]
100019ecc:     	mov	x3, sp
100019ed0:     	str	q0, [sp, #0x80]
100019ed4:     	add	x2, x3, #0x20
100019ed8:     	add	x4, x3, #0x10
100019edc:     	orr	x5, x3, #0x8
100019ee0:     	ldr	x10, [x1, #0x40]
100019ee4:     	cbz	x10, 0x100019ef0 <field_regressions::production_p256::product+0x5c>
100019ee8:     	mov	w10, #0x9               ; =9
100019eec:     	b	0x100019f6c <field_regressions::production_p256::product+0xd8>
100019ef0:     	ldr	x10, [x1, #0x38]
100019ef4:     	cbz	x10, 0x100019f00 <field_regressions::production_p256::product+0x6c>
100019ef8:     	mov	w10, #0x8               ; =8
100019efc:     	b	0x100019f6c <field_regressions::production_p256::product+0xd8>
100019f00:     	ldr	x10, [x1, #0x30]
100019f04:     	cbz	x10, 0x100019f10 <field_regressions::production_p256::product+0x7c>
100019f08:     	mov	w10, #0x7               ; =7
100019f0c:     	b	0x100019f6c <field_regressions::production_p256::product+0xd8>
100019f10:     	ldr	x10, [x1, #0x28]
100019f14:     	cbz	x10, 0x100019f20 <field_regressions::production_p256::product+0x8c>
100019f18:     	mov	w10, #0x6               ; =6
100019f1c:     	b	0x100019f6c <field_regressions::production_p256::product+0xd8>
100019f20:     	ldr	x10, [x1, #0x20]
100019f24:     	cbz	x10, 0x100019f30 <field_regressions::production_p256::product+0x9c>
100019f28:     	mov	w10, #0x5               ; =5
100019f2c:     	b	0x100019f6c <field_regressions::production_p256::product+0xd8>
100019f30:     	ldr	x10, [x1, #0x18]
100019f34:     	cbz	x10, 0x100019f40 <field_regressions::production_p256::product+0xac>
100019f38:     	mov	w10, #0x4               ; =4
100019f3c:     	b	0x100019f6c <field_regressions::production_p256::product+0xd8>
100019f40:     	ldr	x10, [x1, #0x10]
100019f44:     	cbz	x10, 0x100019f50 <field_regressions::production_p256::product+0xbc>
100019f48:     	mov	w10, #0x3               ; =3
100019f4c:     	b	0x100019f6c <field_regressions::production_p256::product+0xd8>
100019f50:     	ldr	x10, [x1, #0x8]
100019f54:     	cbz	x10, 0x100019f60 <field_regressions::production_p256::product+0xcc>
100019f58:     	mov	w10, #0x2               ; =2
100019f5c:     	b	0x100019f6c <field_regressions::production_p256::product+0xd8>
100019f60:     	ldr	x10, [x1]
100019f64:     	cmp	x10, #0x0
100019f68:     	cset	w10, ne
100019f6c:     	cbz	x17, 0x100019f98 <field_regressions::production_p256::product+0x104>
100019f70:     	mov	w22, #0x0               ; =0
100019f74:     	mov	w23, #0x0               ; =0
100019f78:     	mov	w24, #0x0               ; =0
100019f7c:     	mov	w6, #0x0                ; =0
100019f80:     	mov	w7, #0x0                ; =0
100019f84:     	mov	w19, #0x0               ; =0
100019f88:     	mov	w20, #0x0               ; =0
100019f8c:     	mov	w21, #0x0               ; =0
100019f90:     	add	x3, x3, #0x48
100019f94:     	b	0x10001a0f4 <field_regressions::production_p256::product+0x260>
100019f98:     	cbz	x16, 0x100019fc4 <field_regressions::production_p256::product+0x130>
100019f9c:     	mov	w22, #0x0               ; =0
100019fa0:     	mov	w23, #0x0               ; =0
100019fa4:     	mov	w24, #0x0               ; =0
100019fa8:     	mov	w6, #0x0                ; =0
100019fac:     	mov	w7, #0x0                ; =0
100019fb0:     	mov	w19, #0x0               ; =0
100019fb4:     	mov	w20, #0x0               ; =0
100019fb8:     	add	x3, x3, #0x40
100019fbc:     	mov	w21, #0x1               ; =1
100019fc0:     	b	0x10001a0f4 <field_regressions::production_p256::product+0x260>
100019fc4:     	cbz	x15, 0x100019ff0 <field_regressions::production_p256::product+0x15c>
100019fc8:     	mov	w22, #0x0               ; =0
100019fcc:     	mov	w23, #0x0               ; =0
100019fd0:     	mov	w24, #0x0               ; =0
100019fd4:     	mov	w6, #0x0                ; =0
100019fd8:     	mov	w7, #0x0                ; =0
100019fdc:     	mov	w19, #0x0               ; =0
100019fe0:     	mov	w21, #0x0               ; =0
100019fe4:     	add	x3, x3, #0x38
100019fe8:     	mov	w20, #0x1               ; =1
100019fec:     	b	0x10001a0f4 <field_regressions::production_p256::product+0x260>
100019ff0:     	cbz	x14, 0x10001a01c <field_regressions::production_p256::product+0x188>
100019ff4:     	mov	w22, #0x0               ; =0
100019ff8:     	mov	w23, #0x0               ; =0
100019ffc:     	mov	w24, #0x0               ; =0
10001a000:     	mov	w6, #0x0                ; =0
10001a004:     	mov	w7, #0x0                ; =0
10001a008:     	mov	w20, #0x0               ; =0
10001a00c:     	mov	w21, #0x0               ; =0
10001a010:     	add	x3, x3, #0x30
10001a014:     	mov	w19, #0x1               ; =1
10001a018:     	b	0x10001a0f4 <field_regressions::production_p256::product+0x260>
10001a01c:     	cbz	x13, 0x10001a048 <field_regressions::production_p256::product+0x1b4>
10001a020:     	mov	w22, #0x0               ; =0
10001a024:     	mov	w23, #0x0               ; =0
10001a028:     	mov	w24, #0x0               ; =0
10001a02c:     	mov	w6, #0x0                ; =0
10001a030:     	mov	w19, #0x0               ; =0
10001a034:     	mov	w20, #0x0               ; =0
10001a038:     	mov	w21, #0x0               ; =0
10001a03c:     	add	x3, x3, #0x28
10001a040:     	mov	w7, #0x1                ; =1
10001a044:     	b	0x10001a0f4 <field_regressions::production_p256::product+0x260>
10001a048:     	cbz	x12, 0x10001a074 <field_regressions::production_p256::product+0x1e0>
10001a04c:     	mov	w22, #0x0               ; =0
10001a050:     	mov	w23, #0x0               ; =0
10001a054:     	mov	w24, #0x0               ; =0
10001a058:     	mov	w7, #0x0                ; =0
10001a05c:     	mov	w19, #0x0               ; =0
10001a060:     	mov	w20, #0x0               ; =0
10001a064:     	mov	w21, #0x0               ; =0
10001a068:     	mov	w6, #0x1                ; =1
10001a06c:     	mov	x3, x2
10001a070:     	b	0x10001a0f4 <field_regressions::production_p256::product+0x260>
10001a074:     	cbz	x11, 0x10001a0a0 <field_regressions::production_p256::product+0x20c>
10001a078:     	mov	w22, #0x0               ; =0
10001a07c:     	mov	w23, #0x0               ; =0
10001a080:     	mov	w6, #0x0                ; =0
10001a084:     	mov	w7, #0x0                ; =0
10001a088:     	mov	w19, #0x0               ; =0
10001a08c:     	mov	w20, #0x0               ; =0
10001a090:     	mov	w21, #0x0               ; =0
10001a094:     	add	x3, x3, #0x18
10001a098:     	mov	w24, #0x1               ; =1
10001a09c:     	b	0x10001a0f4 <field_regressions::production_p256::product+0x260>
10001a0a0:     	cbz	x9, 0x10001a0cc <field_regressions::production_p256::product+0x238>
10001a0a4:     	mov	w22, #0x0               ; =0
10001a0a8:     	mov	w24, #0x0               ; =0
10001a0ac:     	mov	w6, #0x0                ; =0
10001a0b0:     	mov	w7, #0x0                ; =0
10001a0b4:     	mov	w19, #0x0               ; =0
10001a0b8:     	mov	w20, #0x0               ; =0
10001a0bc:     	mov	w21, #0x0               ; =0
10001a0c0:     	mov	w23, #0x1               ; =1
10001a0c4:     	mov	x3, x4
10001a0c8:     	b	0x10001a0f4 <field_regressions::production_p256::product+0x260>
10001a0cc:     	cbz	x8, 0x10001a1dc <field_regressions::production_p256::product+0x348>
10001a0d0:     	mov	w23, #0x0               ; =0
10001a0d4:     	mov	w24, #0x0               ; =0
10001a0d8:     	mov	w6, #0x0                ; =0
10001a0dc:     	mov	w7, #0x0                ; =0
10001a0e0:     	mov	w19, #0x0               ; =0
10001a0e4:     	mov	w20, #0x0               ; =0
10001a0e8:     	mov	w21, #0x0               ; =0
10001a0ec:     	mov	w22, #0x1               ; =1
10001a0f0:     	mov	x3, x5
10001a0f4:     	cbz	x10, 0x10001a1dc <field_regressions::production_p256::product+0x348>
10001a0f8:     	cbz	w22, 0x10001a12c <field_regressions::production_p256::product+0x298>
10001a0fc:     	mov	x9, sp
10001a100:     	ldr	x11, [x1], #0x8
10001a104:     	umulh	x12, x11, x8
10001a108:     	mul	x11, x11, x8
10001a10c:     	ldr	x13, [x9]
10001a110:     	adds	x11, x11, x13
10001a114:     	cinc	x12, x12, hs
10001a118:     	str	x11, [x9], #0x8
10001a11c:     	str	x12, [x3], #0x8
10001a120:     	subs	x10, x10, #0x1
10001a124:     	b.ne	0x10001a100 <field_regressions::production_p256::product+0x26c>
10001a128:     	b	0x10001a1dc <field_regressions::production_p256::product+0x348>
10001a12c:     	cbz	w23, 0x10001a178 <field_regressions::production_p256::product+0x2e4>
10001a130:     	ldr	x11, [x1], #0x8
10001a134:     	umulh	x12, x11, x8
10001a138:     	mul	x13, x11, x8
10001a13c:     	ldp	x14, x15, [x5, #-0x8]
10001a140:     	adds	x13, x13, x14
10001a144:     	cinc	x12, x12, hs
10001a148:     	mul	x14, x11, x9
10001a14c:     	adds	x12, x12, x14
10001a150:     	umulh	x11, x11, x9
10001a154:     	cinc	x11, x11, hs
10001a158:     	adds	x12, x12, x15
10001a15c:     	cinc	x11, x11, hs
10001a160:     	stp	x13, x12, [x5, #-0x8]
10001a164:     	str	x11, [x3], #0x8
10001a168:     	add	x5, x5, #0x8
10001a16c:     	subs	x10, x10, #0x1
10001a170:     	b.ne	0x10001a130 <field_regressions::production_p256::product+0x29c>
10001a174:     	b	0x10001a1dc <field_regressions::production_p256::product+0x348>
10001a178:     	tbz	w24, #0x0, 0x10001a228 <field_regressions::production_p256::product+0x394>
10001a17c:     	ldr	x12, [x1], #0x8
10001a180:     	umulh	x13, x12, x8
10001a184:     	mul	x14, x12, x8
10001a188:     	ldp	x15, x16, [x4, #-0x10]
10001a18c:     	adds	x14, x14, x15
10001a190:     	cinc	x13, x13, hs
10001a194:     	mul	x15, x12, x9
10001a198:     	adds	x13, x13, x15
10001a19c:     	umulh	x15, x12, x9
10001a1a0:     	cinc	x15, x15, hs
10001a1a4:     	adds	x13, x13, x16
10001a1a8:     	cinc	x15, x15, hs
10001a1ac:     	stp	x14, x13, [x4, #-0x10]
10001a1b0:     	umulh	x13, x12, x11
10001a1b4:     	mul	x12, x12, x11
10001a1b8:     	ldr	x14, [x4]
10001a1bc:     	adds	x12, x15, x12
10001a1c0:     	cinc	x13, x13, hs
10001a1c4:     	adds	x12, x12, x14
10001a1c8:     	cinc	x13, x13, hs
10001a1cc:     	str	x12, [x4], #0x8
10001a1d0:     	str	x13, [x3], #0x8
10001a1d4:     	subs	x10, x10, #0x1
10001a1d8:     	b.ne	0x10001a17c <field_regressions::production_p256::product+0x2e8>
10001a1dc:     	ldp	q0, q1, [sp, #0x60]
10001a1e0:     	stp	q0, q1, [x0, #0x60]
10001a1e4:     	ldr	q0, [sp, #0x80]
10001a1e8:     	str	q0, [x0, #0x80]
10001a1ec:     	ldp	q0, q1, [sp, #0x20]
10001a1f0:     	stp	q0, q1, [x0, #0x20]
10001a1f4:     	ldp	q1, q0, [sp, #0x40]
10001a1f8:     	stp	q1, q0, [x0, #0x40]
10001a1fc:     	ldp	q1, q0, [sp]
10001a200:     	stp	q1, q0, [x0]
10001a204:     	ldp	x20, x19, [sp, #0xb0]
10001a208:     	ldp	x22, x21, [sp, #0xa0]
10001a20c:     	ldp	x24, x23, [sp, #0x90]
10001a210:     	add	sp, sp, #0xc0
10001a214:     	ret
10001a218:     	str	x5, [x3], #0x8
10001a21c:     	add	x2, x2, #0x8
10001a220:     	subs	x10, x10, #0x1
10001a224:     	b.eq	0x10001a1dc <field_regressions::production_p256::product+0x348>
10001a228:     	ldr	x4, [x1], #0x8
10001a22c:     	umulh	x5, x4, x8
10001a230:     	mul	x22, x4, x8
10001a234:     	ldp	x23, x24, [x2, #-0x20]
10001a238:     	adds	x22, x22, x23
10001a23c:     	cinc	x5, x5, hs
10001a240:     	mul	x23, x4, x9
10001a244:     	adds	x5, x5, x23
10001a248:     	umulh	x23, x4, x9
10001a24c:     	cinc	x23, x23, hs
10001a250:     	adds	x5, x5, x24
10001a254:     	cinc	x23, x23, hs
10001a258:     	stp	x22, x5, [x2, #-0x20]
10001a25c:     	umulh	x5, x4, x11
10001a260:     	mul	x22, x4, x11
10001a264:     	adds	x22, x23, x22
10001a268:     	cinc	x5, x5, hs
10001a26c:     	ldp	x23, x24, [x2, #-0x10]
10001a270:     	adds	x22, x22, x23
10001a274:     	cinc	x5, x5, hs
10001a278:     	mul	x23, x4, x12
10001a27c:     	adds	x5, x5, x23
10001a280:     	umulh	x23, x4, x12
10001a284:     	cinc	x23, x23, hs
10001a288:     	adds	x24, x5, x24
10001a28c:     	cinc	x5, x23, hs
10001a290:     	stp	x22, x24, [x2, #-0x10]
10001a294:     	tbnz	w6, #0x0, 0x10001a218 <field_regressions::production_p256::product+0x384>
10001a298:     	umulh	x22, x4, x13
10001a29c:     	mul	x23, x4, x13
10001a2a0:     	ldr	x24, [x2]
10001a2a4:     	adds	x5, x5, x23
10001a2a8:     	cinc	x22, x22, hs
10001a2ac:     	adds	x23, x5, x24
10001a2b0:     	cinc	x5, x22, hs
10001a2b4:     	str	x23, [x2]
10001a2b8:     	tbnz	w7, #0x0, 0x10001a218 <field_regressions::production_p256::product+0x384>
10001a2bc:     	umulh	x22, x4, x14
10001a2c0:     	mul	x23, x4, x14
10001a2c4:     	ldr	x24, [x2, #0x8]
10001a2c8:     	adds	x5, x5, x23
10001a2cc:     	cinc	x22, x22, hs
10001a2d0:     	adds	x23, x5, x24
10001a2d4:     	cinc	x5, x22, hs
10001a2d8:     	str	x23, [x2, #0x8]
10001a2dc:     	tbnz	w19, #0x0, 0x10001a218 <field_regressions::production_p256::product+0x384>
10001a2e0:     	umulh	x22, x4, x15
10001a2e4:     	mul	x23, x4, x15
10001a2e8:     	ldr	x24, [x2, #0x10]
10001a2ec:     	adds	x5, x5, x23
10001a2f0:     	cinc	x22, x22, hs
10001a2f4:     	adds	x23, x5, x24
10001a2f8:     	cinc	x5, x22, hs
10001a2fc:     	str	x23, [x2, #0x10]
10001a300:     	tbnz	w20, #0x0, 0x10001a218 <field_regressions::production_p256::product+0x384>
10001a304:     	umulh	x22, x4, x16
10001a308:     	mul	x23, x4, x16
10001a30c:     	ldr	x24, [x2, #0x18]
10001a310:     	adds	x5, x5, x23
10001a314:     	cinc	x22, x22, hs
10001a318:     	adds	x23, x5, x24
10001a31c:     	cinc	x5, x22, hs
10001a320:     	str	x23, [x2, #0x18]
10001a324:     	tbnz	w21, #0x0, 0x10001a218 <field_regressions::production_p256::product+0x384>
10001a328:     	umulh	x22, x4, x17
10001a32c:     	mul	x4, x4, x17
10001a330:     	ldr	x23, [x2, #0x20]
10001a334:     	adds	x4, x5, x4
10001a338:     	cinc	x5, x22, hs
10001a33c:     	adds	x4, x4, x23
10001a340:     	cinc	x5, x5, hs
10001a344:     	str	x4, [x2, #0x20]
10001a348:     	b	0x10001a218 <field_regressions::production_p256::product+0x384>

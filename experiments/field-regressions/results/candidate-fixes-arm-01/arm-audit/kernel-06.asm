
/private/var/folders/q0/v7l8dzd13k7f_q6j80sy03lm0000gn/T/f2z-field-gated-pycl0nhx/field-regressions:	file format mach-o arm64

Disassembly of section __TEXT,__text:

0000000100038ad4 <__RINvNtNtNtCsagpXUhzPPpl_17field_regressions8campaign10candidates5words20exact_signed_columnsKj9_Kj13_EB8_>:
100038ad4: a9ba6ffc    	stp	x28, x27, [sp, #-0x60]!
100038ad8: a90167fa    	stp	x26, x25, [sp, #0x10]
100038adc: a9025ff8    	stp	x24, x23, [sp, #0x20]
100038ae0: a90357f6    	stp	x22, x21, [sp, #0x30]
100038ae4: a9044ff4    	stp	x20, x19, [sp, #0x40]
100038ae8: a9057bfd    	stp	x29, x30, [sp, #0x50]
100038aec: 910143fd    	add	x29, sp, #0x50
100038af0: d10683ff    	sub	sp, sp, #0x1a0
100038af4: a90593e2    	stp	x2, x4, [sp, #0x58]
100038af8: eb04005f    	cmp	x2, x4
100038afc: 54002f01    	b.ne	0x1000390dc <__RINvNtNtNtCsagpXUhzPPpl_17field_regressions8campaign10candidates5words20exact_signed_columnsKj9_Kj13_EB8_+0x608>
100038b00: 910183e8    	add	x8, sp, #0x60
100038b04: 6f00e400    	movi.2d	v0, #0000000000000000
100038b08: ad088100    	stp	q0, q0, [x8, #0x110]
100038b0c: ad078100    	stp	q0, q0, [x8, #0xf0]
100038b10: ad068100    	stp	q0, q0, [x8, #0xd0]
100038b14: ad058100    	stp	q0, q0, [x8, #0xb0]
100038b18: ad048100    	stp	q0, q0, [x8, #0x90]
100038b1c: 3d802100    	str	q0, [x8, #0x80]
100038b20: ad0603e0    	stp	q0, q0, [sp, #0xc0]
100038b24: ad0503e0    	stp	q0, q0, [sp, #0xa0]
100038b28: ad0403e0    	stp	q0, q0, [sp, #0x80]
100038b2c: ad0303e0    	stp	q0, q0, [sp, #0x60]
100038b30: b4001fe2    	cbz	x2, 0x100038f2c <__RINvNtNtNtCsagpXUhzPPpl_17field_regressions8campaign10candidates5words20exact_signed_columnsKj9_Kj13_EB8_+0x458>
100038b34: d2800008    	mov	x8, #0x0                ; =0
100038b38: 910183e9    	add	x9, sp, #0x60
100038b3c: aa0103ea    	mov	x10, x1
100038b40: 1400004e    	b	0x100038c78 <__RINvNtNtNtCsagpXUhzPPpl_17field_regressions8campaign10candidates5words20exact_signed_columnsKj9_Kj13_EB8_+0x1a4>
100038b44: 937ffd73    	asr	x19, x11, #63
100038b48: a9405594    	ldp	x20, x21, [x12]
100038b4c: 8a140274    	and	x20, x19, x20
100038b50: a94f5bf7    	ldp	x23, x22, [sp, #0xf0]
100038b54: ab1400e7    	adds	x7, x7, x20
100038b58: 1a9f37f4    	cset	w20, hs
100038b5c: eb0702e7    	subs	x7, x23, x7
100038b60: da1402d4    	sbc	x20, x22, x20
100038b64: a90f53e7    	stp	x7, x20, [sp, #0xf0]
100038b68: 8a150267    	and	x7, x19, x21
100038b6c: a95053f5    	ldp	x21, x20, [sp, #0x100]
100038b70: ab0700c6    	adds	x6, x6, x7
100038b74: 1a9f37e7    	cset	w7, hs
100038b78: eb0602a6    	subs	x6, x21, x6
100038b7c: da070287    	sbc	x7, x20, x7
100038b80: a9101fe6    	stp	x6, x7, [sp, #0x100]
100038b84: a9411d86    	ldp	x6, x7, [x12, #0x10]
100038b88: 8a060266    	and	x6, x19, x6
100038b8c: a95153f5    	ldp	x21, x20, [sp, #0x110]
100038b90: ab0600a5    	adds	x5, x5, x6
100038b94: 1a9f37e6    	cset	w6, hs
100038b98: eb0502a5    	subs	x5, x21, x5
100038b9c: da060286    	sbc	x6, x20, x6
100038ba0: a9111be5    	stp	x5, x6, [sp, #0x110]
100038ba4: 8a070265    	and	x5, x19, x7
100038ba8: a9521be7    	ldp	x7, x6, [sp, #0x120]
100038bac: ab050084    	adds	x4, x4, x5
100038bb0: 1a9f37e5    	cset	w5, hs
100038bb4: eb0400e4    	subs	x4, x7, x4
100038bb8: da0500c5    	sbc	x5, x6, x5
100038bbc: a91217e4    	stp	x4, x5, [sp, #0x120]
100038bc0: a9421584    	ldp	x4, x5, [x12, #0x20]
100038bc4: 8a040264    	and	x4, x19, x4
100038bc8: a9531be7    	ldp	x7, x6, [sp, #0x130]
100038bcc: ab040231    	adds	x17, x17, x4
100038bd0: 1a9f37e4    	cset	w4, hs
100038bd4: eb1100f1    	subs	x17, x7, x17
100038bd8: da0400c4    	sbc	x4, x6, x4
100038bdc: a91313f1    	stp	x17, x4, [sp, #0x130]
100038be0: 8a050271    	and	x17, x19, x5
100038be4: a95413e5    	ldp	x5, x4, [sp, #0x140]
100038be8: ab110210    	adds	x16, x16, x17
100038bec: 1a9f37f1    	cset	w17, hs
100038bf0: eb1000b0    	subs	x16, x5, x16
100038bf4: da110091    	sbc	x17, x4, x17
100038bf8: a91447f0    	stp	x16, x17, [sp, #0x140]
100038bfc: a9433190    	ldp	x16, x12, [x12, #0x30]
100038c00: 8a100270    	and	x16, x19, x16
100038c04: a95547e4    	ldp	x4, x17, [sp, #0x150]
100038c08: ab1001ef    	adds	x15, x15, x16
100038c0c: 1a9f37f0    	cset	w16, hs
100038c10: eb0f008f    	subs	x15, x4, x15
100038c14: da100230    	sbc	x16, x17, x16
100038c18: a91543ef    	stp	x15, x16, [sp, #0x150]
100038c1c: 8a0c026c    	and	x12, x19, x12
100038c20: a9563ff0    	ldp	x16, x15, [sp, #0x160]
100038c24: ab0c01ac    	adds	x12, x13, x12
100038c28: 1a9f37ed    	cset	w13, hs
100038c2c: eb0c020c    	subs	x12, x16, x12
100038c30: da0d01ed    	sbc	x13, x15, x13
100038c34: a91637ec    	stp	x12, x13, [sp, #0x160]
100038c38: 8a0e026c    	and	x12, x19, x14
100038c3c: a95737ef    	ldp	x15, x13, [sp, #0x170]
100038c40: ab0c016c    	adds	x12, x11, x12
100038c44: 1a9f37f0    	cset	w16, hs
100038c48: eb0c01ec    	subs	x12, x15, x12
100038c4c: da1001ad    	sbc	x13, x13, x16
100038c50: a91737ec    	stp	x12, x13, [sp, #0x170]
100038c54: 91000508    	add	x8, x8, #0x1
100038c58: 8a0e016b    	and	x11, x11, x14
100038c5c: a95833ed    	ldp	x13, x12, [sp, #0x180]
100038c60: ab4bfdab    	adds	x11, x13, x11, lsr #63
100038c64: 9a8c358c    	cinc	x12, x12, hs
100038c68: a91833eb    	stp	x11, x12, [sp, #0x180]
100038c6c: 9101214a    	add	x10, x10, #0x48
100038c70: eb02011f    	cmp	x8, x2
100038c74: 540010c0    	b.eq	0x100038e8c <__RINvNtNtNtCsagpXUhzPPpl_17field_regressions8campaign10candidates5words20exact_signed_columnsKj9_Kj13_EB8_+0x3b8>
100038c78: d280000e    	mov	x14, #0x0               ; =0
100038c7c: 8b080d0b    	add	x11, x8, x8, lsl #3
100038c80: d37df16b    	lsl	x11, x11, #3
100038c84: 8b0b002c    	add	x12, x1, x11
100038c88: 8b0b006b    	add	x11, x3, x11
100038c8c: a9424171    	ldp	x17, x16, [x11, #0x20]
100038c90: a943356f    	ldp	x15, x13, [x11, #0x30]
100038c94: a9411165    	ldp	x5, x4, [x11, #0x10]
100038c98: a9401967    	ldp	x7, x6, [x11]
100038c9c: f940216b    	ldr	x11, [x11, #0x40]
100038ca0: aa0a03f3    	mov	x19, x10
100038ca4: f8408675    	ldr	x21, [x19], #0x8
100038ca8: 8b0e0134    	add	x20, x9, x14
100038cac: 9bd57cf6    	umulh	x22, x7, x21
100038cb0: 9b157cf7    	mul	x23, x7, x21
100038cb4: a9406299    	ldp	x25, x24, [x20]
100038cb8: ab1902f7    	adds	x23, x23, x25
100038cbc: 9a983718    	cinc	x24, x24, hs
100038cc0: a9006297    	stp	x23, x24, [x20]
100038cc4: a9415e98    	ldp	x24, x23, [x20, #0x10]
100038cc8: ab160316    	adds	x22, x24, x22
100038ccc: 9a9736f7    	cinc	x23, x23, hs
100038cd0: 9bd57cd8    	umulh	x24, x6, x21
100038cd4: 9b157cd9    	mul	x25, x6, x21
100038cd8: ab160336    	adds	x22, x25, x22
100038cdc: 9a9736f7    	cinc	x23, x23, hs
100038ce0: a9015e96    	stp	x22, x23, [x20, #0x10]
100038ce4: a9425a97    	ldp	x23, x22, [x20, #0x20]
100038ce8: ab1802f7    	adds	x23, x23, x24
100038cec: 9a9636d6    	cinc	x22, x22, hs
100038cf0: 9bd57cb8    	umulh	x24, x5, x21
100038cf4: 9b157cb9    	mul	x25, x5, x21
100038cf8: ab170337    	adds	x23, x25, x23
100038cfc: 9a9636d6    	cinc	x22, x22, hs
100038d00: a9025a97    	stp	x23, x22, [x20, #0x20]
100038d04: a9435a97    	ldp	x23, x22, [x20, #0x30]
100038d08: ab1802f7    	adds	x23, x23, x24
100038d0c: 9a9636d6    	cinc	x22, x22, hs
100038d10: 9bd57c98    	umulh	x24, x4, x21
100038d14: 9b157c99    	mul	x25, x4, x21
100038d18: ab170337    	adds	x23, x25, x23
100038d1c: 9a9636d6    	cinc	x22, x22, hs
100038d20: a9035a97    	stp	x23, x22, [x20, #0x30]
100038d24: a9445a97    	ldp	x23, x22, [x20, #0x40]
100038d28: ab1802f7    	adds	x23, x23, x24
100038d2c: 9a9636d6    	cinc	x22, x22, hs
100038d30: 9bd57e38    	umulh	x24, x17, x21
100038d34: 9b157e39    	mul	x25, x17, x21
100038d38: ab170337    	adds	x23, x25, x23
100038d3c: 9a9636d6    	cinc	x22, x22, hs
100038d40: a9045a97    	stp	x23, x22, [x20, #0x40]
100038d44: a9455a97    	ldp	x23, x22, [x20, #0x50]
100038d48: ab1802f7    	adds	x23, x23, x24
100038d4c: 9a9636d6    	cinc	x22, x22, hs
100038d50: 9bd57e18    	umulh	x24, x16, x21
100038d54: 9b157e19    	mul	x25, x16, x21
100038d58: ab170337    	adds	x23, x25, x23
100038d5c: 9a9636d6    	cinc	x22, x22, hs
100038d60: a9055a97    	stp	x23, x22, [x20, #0x50]
100038d64: a9465a97    	ldp	x23, x22, [x20, #0x60]
100038d68: ab1802f7    	adds	x23, x23, x24
100038d6c: 9a9636d6    	cinc	x22, x22, hs
100038d70: 9bd57df8    	umulh	x24, x15, x21
100038d74: 9b157df9    	mul	x25, x15, x21
100038d78: ab170337    	adds	x23, x25, x23
100038d7c: 9a9636d6    	cinc	x22, x22, hs
100038d80: a9065a97    	stp	x23, x22, [x20, #0x60]
100038d84: a9475a97    	ldp	x23, x22, [x20, #0x70]
100038d88: ab1802f7    	adds	x23, x23, x24
100038d8c: 9a9636d6    	cinc	x22, x22, hs
100038d90: 9bd57db8    	umulh	x24, x13, x21
100038d94: 9b157db9    	mul	x25, x13, x21
100038d98: ab170337    	adds	x23, x25, x23
100038d9c: 9a9636d6    	cinc	x22, x22, hs
100038da0: a9075a97    	stp	x23, x22, [x20, #0x70]
100038da4: a9485a97    	ldp	x23, x22, [x20, #0x80]
100038da8: ab1802f7    	adds	x23, x23, x24
100038dac: 9a9636d6    	cinc	x22, x22, hs
100038db0: 9bd57d78    	umulh	x24, x11, x21
100038db4: 9b157d75    	mul	x21, x11, x21
100038db8: ab1702b5    	adds	x21, x21, x23
100038dbc: 9a9636d6    	cinc	x22, x22, hs
100038dc0: a9085a95    	stp	x21, x22, [x20, #0x80]
100038dc4: a9495696    	ldp	x22, x21, [x20, #0x90]
100038dc8: ab1802d6    	adds	x22, x22, x24
100038dcc: 9a9536b5    	cinc	x21, x21, hs
100038dd0: a9095696    	stp	x22, x21, [x20, #0x90]
100038dd4: 910041ce    	add	x14, x14, #0x10
100038dd8: f10241df    	cmp	x14, #0x90
100038ddc: 54fff641    	b.ne	0x100038ca4 <__RINvNtNtNtCsagpXUhzPPpl_17field_regressions8campaign10candidates5words20exact_signed_columnsKj9_Kj13_EB8_+0x1d0>
100038de0: f940218e    	ldr	x14, [x12, #0x40]
100038de4: b7ffeb0e    	tbnz	x14, #0x3f, 0x100038b44 <__RINvNtNtNtCsagpXUhzPPpl_17field_regressions8campaign10candidates5words20exact_signed_columnsKj9_Kj13_EB8_+0x70>
100038de8: b6fff36b    	tbz	x11, #0x3f, 0x100038c54 <__RINvNtNtNtCsagpXUhzPPpl_17field_regressions8campaign10candidates5words20exact_signed_columnsKj9_Kj13_EB8_+0x180>
100038dec: a94f37ef    	ldp	x15, x13, [sp, #0xf0]
100038df0: a9404590    	ldp	x16, x17, [x12]
100038df4: eb1001ef    	subs	x15, x15, x16
100038df8: da1f01ad    	sbc	x13, x13, xzr
100038dfc: a90f37ef    	stp	x15, x13, [sp, #0xf0]
100038e00: a95037ef    	ldp	x15, x13, [sp, #0x100]
100038e04: eb1101ef    	subs	x15, x15, x17
100038e08: da1f01ad    	sbc	x13, x13, xzr
100038e0c: a91037ef    	stp	x15, x13, [sp, #0x100]
100038e10: a95137ef    	ldp	x15, x13, [sp, #0x110]
100038e14: a9414590    	ldp	x16, x17, [x12, #0x10]
100038e18: eb1001ef    	subs	x15, x15, x16
100038e1c: da1f01ad    	sbc	x13, x13, xzr
100038e20: a91137ef    	stp	x15, x13, [sp, #0x110]
100038e24: a95237ef    	ldp	x15, x13, [sp, #0x120]
100038e28: eb1101ef    	subs	x15, x15, x17
100038e2c: da1f01ad    	sbc	x13, x13, xzr
100038e30: a91237ef    	stp	x15, x13, [sp, #0x120]
100038e34: a95337ef    	ldp	x15, x13, [sp, #0x130]
100038e38: a9424590    	ldp	x16, x17, [x12, #0x20]
100038e3c: eb1001ef    	subs	x15, x15, x16
100038e40: da1f01ad    	sbc	x13, x13, xzr
100038e44: a91337ef    	stp	x15, x13, [sp, #0x130]
100038e48: a95437ef    	ldp	x15, x13, [sp, #0x140]
100038e4c: eb1101ef    	subs	x15, x15, x17
100038e50: da1f01ad    	sbc	x13, x13, xzr
100038e54: a91437ef    	stp	x15, x13, [sp, #0x140]
100038e58: a95537ef    	ldp	x15, x13, [sp, #0x150]
100038e5c: a9433190    	ldp	x16, x12, [x12, #0x30]
100038e60: eb1001ef    	subs	x15, x15, x16
100038e64: da1f01ad    	sbc	x13, x13, xzr
100038e68: a91537ef    	stp	x15, x13, [sp, #0x150]
100038e6c: a95637ef    	ldp	x15, x13, [sp, #0x160]
100038e70: eb0c01ef    	subs	x15, x15, x12
100038e74: da1f01b0    	sbc	x16, x13, xzr
100038e78: a95737ec    	ldp	x12, x13, [sp, #0x170]
100038e7c: eb0e018c    	subs	x12, x12, x14
100038e80: da1f01ad    	sbc	x13, x13, xzr
100038e84: a91643ef    	stp	x15, x16, [sp, #0x160]
100038e88: 17ffff72    	b	0x100038c50 <__RINvNtNtNtCsagpXUhzPPpl_17field_regressions8campaign10candidates5words20exact_signed_columnsKj9_Kj13_EB8_+0x17c>
100038e8c: a9463be8    	ldp	x8, x14, [sp, #0x60]
100038e90: f9002be8    	str	x8, [sp, #0x50]
100038e94: a9473ff1    	ldp	x17, x15, [sp, #0x70]
100038e98: a94833f0    	ldp	x16, x12, [sp, #0x80]
100038e9c: a94927ed    	ldp	x13, x9, [sp, #0x90]
100038ea0: a94a7bea    	ldp	x10, x30, [sp, #0xa0]
100038ea4: a94bf3e8    	ldp	x8, x28, [sp, #0xb8]
100038ea8: f90007e8    	str	x8, [sp, #0x8]
100038eac: f9405be8    	ldr	x8, [sp, #0xb0]
100038eb0: a94ceffa    	ldp	x26, x27, [sp, #0xc8]
100038eb4: a94ddff8    	ldp	x24, x23, [sp, #0xd8]
100038eb8: a94edbf9    	ldp	x25, x22, [sp, #0xe8]
100038ebc: a94fd3f5    	ldp	x21, x20, [sp, #0xf8]
100038ec0: a9509ff3    	ldp	x19, x7, [sp, #0x108]
100038ec4: a95197e6    	ldp	x6, x5, [sp, #0x118]
100038ec8: a9528fe4    	ldp	x4, x3, [sp, #0x128]
100038ecc: f940a7e1    	ldr	x1, [sp, #0x148]
100038ed0: f9000fe1    	str	x1, [sp, #0x18]
100038ed4: a95387e2    	ldp	x2, x1, [sp, #0x138]
100038ed8: f9000be1    	str	x1, [sp, #0x10]
100038edc: f940afe1    	ldr	x1, [sp, #0x158]
100038ee0: f90017e1    	str	x1, [sp, #0x28]
100038ee4: f940abe1    	ldr	x1, [sp, #0x150]
100038ee8: f90013e1    	str	x1, [sp, #0x20]
100038eec: f940b7e1    	ldr	x1, [sp, #0x168]
100038ef0: f9001fe1    	str	x1, [sp, #0x38]
100038ef4: f940b3e1    	ldr	x1, [sp, #0x160]
100038ef8: f9001be1    	str	x1, [sp, #0x30]
100038efc: f940bfe1    	ldr	x1, [sp, #0x178]
100038f00: f90027e1    	str	x1, [sp, #0x48]
100038f04: f940bbe1    	ldr	x1, [sp, #0x170]
100038f08: f90023e1    	str	x1, [sp, #0x40]
100038f0c: aa0203e1    	mov	x1, x2
100038f10: aa0403e2    	mov	x2, x4
100038f14: aa0603e4    	mov	x4, x6
100038f18: aa1303e6    	mov	x6, x19
100038f1c: aa1503f3    	mov	x19, x21
100038f20: aa1903f5    	mov	x21, x25
100038f24: f94007f9    	ldr	x25, [sp, #0x8]
100038f28: 14000021    	b	0x100038fac <__RINvNtNtNtCsagpXUhzPPpl_17field_regressions8campaign10candidates5words20exact_signed_columnsKj9_Kj13_EB8_+0x4d8>
100038f2c: d280000b    	mov	x11, #0x0               ; =0
100038f30: a9047fff    	stp	xzr, xzr, [sp, #0x40]
100038f34: a9037fff    	stp	xzr, xzr, [sp, #0x30]
100038f38: a9027fff    	stp	xzr, xzr, [sp, #0x20]
100038f3c: a9017fff    	stp	xzr, xzr, [sp, #0x10]
100038f40: d2800003    	mov	x3, #0x0                ; =0
100038f44: d2800001    	mov	x1, #0x0                ; =0
100038f48: d2800005    	mov	x5, #0x0                ; =0
100038f4c: d2800007    	mov	x7, #0x0                ; =0
100038f50: d2800004    	mov	x4, #0x0                ; =0
100038f54: d2800014    	mov	x20, #0x0               ; =0
100038f58: d2800006    	mov	x6, #0x0                ; =0
100038f5c: d2800016    	mov	x22, #0x0               ; =0
100038f60: d2800013    	mov	x19, #0x0               ; =0
100038f64: d2800017    	mov	x23, #0x0               ; =0
100038f68: d2800015    	mov	x21, #0x0               ; =0
100038f6c: d280001b    	mov	x27, #0x0               ; =0
100038f70: d2800018    	mov	x24, #0x0               ; =0
100038f74: d280001c    	mov	x28, #0x0               ; =0
100038f78: d280001a    	mov	x26, #0x0               ; =0
100038f7c: d2800008    	mov	x8, #0x0                ; =0
100038f80: d2800019    	mov	x25, #0x0               ; =0
100038f84: d280000a    	mov	x10, #0x0               ; =0
100038f88: d280001e    	mov	x30, #0x0               ; =0
100038f8c: d280000d    	mov	x13, #0x0               ; =0
100038f90: d2800009    	mov	x9, #0x0                ; =0
100038f94: d2800010    	mov	x16, #0x0               ; =0
100038f98: d280000c    	mov	x12, #0x0               ; =0
100038f9c: d2800011    	mov	x17, #0x0               ; =0
100038fa0: d280000f    	mov	x15, #0x0               ; =0
100038fa4: f9002bff    	str	xzr, [sp, #0x50]
100038fa8: d280000e    	mov	x14, #0x0               ; =0
100038fac: ab1101d1    	adds	x17, x14, x17
100038fb0: 937ffdce    	asr	x14, x14, #63
100038fb4: 9a0f01cf    	adc	x15, x14, x15
100038fb8: ab1001ee    	adds	x14, x15, x16
100038fbc: 937ffdef    	asr	x15, x15, #63
100038fc0: 9a0c01ef    	adc	x15, x15, x12
100038fc4: ab0d01ec    	adds	x12, x15, x13
100038fc8: 937ffded    	asr	x13, x15, #63
100038fcc: 9a0901ad    	adc	x13, x13, x9
100038fd0: ab0a01a9    	adds	x9, x13, x10
100038fd4: 937ffdaa    	asr	x10, x13, #63
100038fd8: 9a1e014a    	adc	x10, x10, x30
100038fdc: ab080148    	adds	x8, x10, x8
100038fe0: 937ffd4a    	asr	x10, x10, #63
100038fe4: 9a19014a    	adc	x10, x10, x25
100038fe8: ab1c014d    	adds	x13, x10, x28
100038fec: 937ffd4a    	asr	x10, x10, #63
100038ff0: 9a1a014a    	adc	x10, x10, x26
100038ff4: ab1b014f    	adds	x15, x10, x27
100038ff8: 937ffd4a    	asr	x10, x10, #63
100038ffc: 9a18014a    	adc	x10, x10, x24
100039000: ab170150    	adds	x16, x10, x23
100039004: 937ffd4a    	asr	x10, x10, #63
100039008: 9a15014a    	adc	x10, x10, x21
10003900c: ab160155    	adds	x21, x10, x22
100039010: 937ffd4a    	asr	x10, x10, #63
100039014: 9a13014a    	adc	x10, x10, x19
100039018: ab140153    	adds	x19, x10, x20
10003901c: 937ffd4a    	asr	x10, x10, #63
100039020: 9a06014a    	adc	x10, x10, x6
100039024: ab070146    	adds	x6, x10, x7
100039028: 937ffd4a    	asr	x10, x10, #63
10003902c: 9a04014a    	adc	x10, x10, x4
100039030: ab050144    	adds	x4, x10, x5
100039034: 937ffd4a    	asr	x10, x10, #63
100039038: 9a02014a    	adc	x10, x10, x2
10003903c: ab030142    	adds	x2, x10, x3
100039040: 937ffd4a    	asr	x10, x10, #63
100039044: 9a01014a    	adc	x10, x10, x1
100039048: f9402be1    	ldr	x1, [sp, #0x50]
10003904c: a9004401    	stp	x1, x17, [x0]
100039050: a94107f1    	ldp	x17, x1, [sp, #0x10]
100039054: ab110151    	adds	x17, x10, x17
100039058: 937ffd4a    	asr	x10, x10, #63
10003905c: 9a01014a    	adc	x10, x10, x1
100039060: a901300e    	stp	x14, x12, [x0, #0x10]
100039064: a9022009    	stp	x9, x8, [x0, #0x20]
100039068: f94013e8    	ldr	x8, [sp, #0x20]
10003906c: ab080148    	adds	x8, x10, x8
100039070: 937ffd49    	asr	x9, x10, #63
100039074: f94017ea    	ldr	x10, [sp, #0x28]
100039078: 9a0a0129    	adc	x9, x9, x10
10003907c: a9033c0d    	stp	x13, x15, [x0, #0x30]
100039080: a9045410    	stp	x16, x21, [x0, #0x40]
100039084: a94333ea    	ldp	x10, x12, [sp, #0x30]
100039088: ab0a012a    	adds	x10, x9, x10
10003908c: 937ffd29    	asr	x9, x9, #63
100039090: 9a0c0129    	adc	x9, x9, x12
100039094: a9051813    	stp	x19, x6, [x0, #0x50]
100039098: a9060804    	stp	x4, x2, [x0, #0x60]
10003909c: a94437ec    	ldp	x12, x13, [sp, #0x40]
1000390a0: ab0c012c    	adds	x12, x9, x12
1000390a4: 937ffd29    	asr	x9, x9, #63
1000390a8: 9a0d0129    	adc	x9, x9, x13
1000390ac: a9072011    	stp	x17, x8, [x0, #0x70]
1000390b0: 8b0b0128    	add	x8, x9, x11
1000390b4: a908300a    	stp	x10, x12, [x0, #0x80]
1000390b8: f9004808    	str	x8, [x0, #0x90]
1000390bc: 910683ff    	add	sp, sp, #0x1a0
1000390c0: a9457bfd    	ldp	x29, x30, [sp, #0x50]
1000390c4: a9444ff4    	ldp	x20, x19, [sp, #0x40]
1000390c8: a94357f6    	ldp	x22, x21, [sp, #0x30]
1000390cc: a9425ff8    	ldp	x24, x23, [sp, #0x20]
1000390d0: a94167fa    	ldp	x26, x25, [sp, #0x10]
1000390d4: a8c66ffc    	ldp	x28, x27, [sp], #0x60
1000390d8: d65f03c0    	ret
1000390dc: d0000ba4    	adrp	x4, 0x1001af000 <dyld_stub_binder+0x1001af000>
1000390e0: 91376084    	add	x4, x4, #0xdd8
1000390e4: 910163e0    	add	x0, sp, #0x58
1000390e8: 910183e1    	add	x1, sp, #0x60
1000390ec: d2800002    	mov	x2, #0x0                ; =0
1000390f0: 94048958    	bl	0x10015b650 <__RINvNtCs8Mbv00yxnRz_4core9panicking13assert_failedjjEB4_>

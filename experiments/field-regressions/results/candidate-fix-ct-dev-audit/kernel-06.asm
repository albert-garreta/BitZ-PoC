
/private/tmp/f2z-arithmetic-target/release/field-regressions:	file format mach-o arm64

Disassembly of section __TEXT,__text:

0000000100038b7c <__RINvNtNtNtCshgIkoLp8A2V_17field_regressions8campaign10candidates5words20exact_signed_columnsKj9_Kj13_EB8_>:
100038b7c: a9ba6ffc    	stp	x28, x27, [sp, #-0x60]!
100038b80: a90167fa    	stp	x26, x25, [sp, #0x10]
100038b84: a9025ff8    	stp	x24, x23, [sp, #0x20]
100038b88: a90357f6    	stp	x22, x21, [sp, #0x30]
100038b8c: a9044ff4    	stp	x20, x19, [sp, #0x40]
100038b90: a9057bfd    	stp	x29, x30, [sp, #0x50]
100038b94: 910143fd    	add	x29, sp, #0x50
100038b98: d10683ff    	sub	sp, sp, #0x1a0
100038b9c: a90387e3    	stp	x3, x1, [sp, #0x38]
100038ba0: a90493e2    	stp	x2, x4, [sp, #0x48]
100038ba4: eb04005f    	cmp	x2, x4
100038ba8: 54002ac1    	b.ne	0x100039100 <__RINvNtNtNtCshgIkoLp8A2V_17field_regressions8campaign10candidates5words20exact_signed_columnsKj9_Kj13_EB8_+0x584>
100038bac: f9001be0    	str	x0, [sp, #0x30]
100038bb0: 910143e8    	add	x8, sp, #0x50
100038bb4: 6f00e400    	movi.2d	v0, #0000000000000000
100038bb8: ad088100    	stp	q0, q0, [x8, #0x110]
100038bbc: ad078100    	stp	q0, q0, [x8, #0xf0]
100038bc0: ad068100    	stp	q0, q0, [x8, #0xd0]
100038bc4: ad058100    	stp	q0, q0, [x8, #0xb0]
100038bc8: ad048100    	stp	q0, q0, [x8, #0x90]
100038bcc: 3d802100    	str	q0, [x8, #0x80]
100038bd0: ad0583e0    	stp	q0, q0, [sp, #0xb0]
100038bd4: ad0483e0    	stp	q0, q0, [sp, #0x90]
100038bd8: ad0383e0    	stp	q0, q0, [sp, #0x70]
100038bdc: ad0283e0    	stp	q0, q0, [sp, #0x50]
100038be0: b4001b22    	cbz	x2, 0x100038f44 <__RINvNtNtNtCshgIkoLp8A2V_17field_regressions8campaign10candidates5words20exact_signed_columnsKj9_Kj13_EB8_+0x3c8>
100038be4: d2800003    	mov	x3, #0x0                ; =0
100038be8: 910143ed    	add	x13, sp, #0x50
100038bec: f94023f0    	ldr	x16, [sp, #0x40]
100038bf0: d2800011    	mov	x17, #0x0               ; =0
100038bf4: d2800009    	mov	x9, #0x0                ; =0
100038bf8: 8b110e28    	add	x8, x17, x17, lsl #3
100038bfc: d37df108    	lsl	x8, x8, #3
100038c00: a943afea    	ldp	x10, x11, [sp, #0x38]
100038c04: 8b080177    	add	x23, x11, x8
100038c08: 8b08014b    	add	x11, x10, x8
100038c0c: a9424d74    	ldp	x20, x19, [x11, #0x20]
100038c10: a9436578    	ldp	x24, x25, [x11, #0x30]
100038c14: a9411964    	ldp	x4, x6, [x11, #0x10]
100038c18: a9402968    	ldp	x8, x10, [x11]
100038c1c: f940217b    	ldr	x27, [x11, #0x40]
100038c20: aa1003eb    	mov	x11, x16
100038c24: f840856e    	ldr	x14, [x11], #0x8
100038c28: 8b0901ac    	add	x12, x13, x9
100038c2c: 9bce7d0f    	umulh	x15, x8, x14
100038c30: 9b0e7d00    	mul	x0, x8, x14
100038c34: a9400585    	ldp	x5, x1, [x12]
100038c38: ab050000    	adds	x0, x0, x5
100038c3c: 9a813421    	cinc	x1, x1, hs
100038c40: a9000580    	stp	x0, x1, [x12]
100038c44: a9410181    	ldp	x1, x0, [x12, #0x10]
100038c48: ab0f002f    	adds	x15, x1, x15
100038c4c: 9a803400    	cinc	x0, x0, hs
100038c50: 9bce7d41    	umulh	x1, x10, x14
100038c54: 9b0e7d45    	mul	x5, x10, x14
100038c58: ab0f00af    	adds	x15, x5, x15
100038c5c: 9a803400    	cinc	x0, x0, hs
100038c60: a901018f    	stp	x15, x0, [x12, #0x10]
100038c64: a9423d80    	ldp	x0, x15, [x12, #0x20]
100038c68: ab010000    	adds	x0, x0, x1
100038c6c: 9a8f35ef    	cinc	x15, x15, hs
100038c70: 9bce7c81    	umulh	x1, x4, x14
100038c74: 9b0e7c85    	mul	x5, x4, x14
100038c78: ab0000a0    	adds	x0, x5, x0
100038c7c: 9a8f35ef    	cinc	x15, x15, hs
100038c80: a9023d80    	stp	x0, x15, [x12, #0x20]
100038c84: a9433d80    	ldp	x0, x15, [x12, #0x30]
100038c88: ab010000    	adds	x0, x0, x1
100038c8c: 9a8f35ef    	cinc	x15, x15, hs
100038c90: 9bce7cc1    	umulh	x1, x6, x14
100038c94: 9b0e7cc5    	mul	x5, x6, x14
100038c98: ab0000a0    	adds	x0, x5, x0
100038c9c: 9a8f35ef    	cinc	x15, x15, hs
100038ca0: a9033d80    	stp	x0, x15, [x12, #0x30]
100038ca4: a9443d80    	ldp	x0, x15, [x12, #0x40]
100038ca8: ab010000    	adds	x0, x0, x1
100038cac: 9a8f35ef    	cinc	x15, x15, hs
100038cb0: 9bce7e81    	umulh	x1, x20, x14
100038cb4: 9b0e7e85    	mul	x5, x20, x14
100038cb8: ab0000a0    	adds	x0, x5, x0
100038cbc: 9a8f35ef    	cinc	x15, x15, hs
100038cc0: a9043d80    	stp	x0, x15, [x12, #0x40]
100038cc4: a9453d80    	ldp	x0, x15, [x12, #0x50]
100038cc8: ab010000    	adds	x0, x0, x1
100038ccc: 9a8f35ef    	cinc	x15, x15, hs
100038cd0: 9bce7e61    	umulh	x1, x19, x14
100038cd4: 9b0e7e65    	mul	x5, x19, x14
100038cd8: ab0000a0    	adds	x0, x5, x0
100038cdc: 9a8f35ef    	cinc	x15, x15, hs
100038ce0: a9053d80    	stp	x0, x15, [x12, #0x50]
100038ce4: a9463d80    	ldp	x0, x15, [x12, #0x60]
100038ce8: ab010000    	adds	x0, x0, x1
100038cec: 9a8f35ef    	cinc	x15, x15, hs
100038cf0: 9bce7f01    	umulh	x1, x24, x14
100038cf4: 9b0e7f05    	mul	x5, x24, x14
100038cf8: ab0000a0    	adds	x0, x5, x0
100038cfc: 9a8f35ef    	cinc	x15, x15, hs
100038d00: a9063d80    	stp	x0, x15, [x12, #0x60]
100038d04: a9473d80    	ldp	x0, x15, [x12, #0x70]
100038d08: ab010000    	adds	x0, x0, x1
100038d0c: 9a8f35ef    	cinc	x15, x15, hs
100038d10: 9bce7f21    	umulh	x1, x25, x14
100038d14: 9b0e7f25    	mul	x5, x25, x14
100038d18: ab0000a0    	adds	x0, x5, x0
100038d1c: 9a8f35ef    	cinc	x15, x15, hs
100038d20: a9073d80    	stp	x0, x15, [x12, #0x70]
100038d24: a9483d80    	ldp	x0, x15, [x12, #0x80]
100038d28: ab010000    	adds	x0, x0, x1
100038d2c: 9a8f35ef    	cinc	x15, x15, hs
100038d30: 9bce7f61    	umulh	x1, x27, x14
100038d34: 9b0e7f6e    	mul	x14, x27, x14
100038d38: ab0001ce    	adds	x14, x14, x0
100038d3c: 9a8f35ef    	cinc	x15, x15, hs
100038d40: a9083d8e    	stp	x14, x15, [x12, #0x80]
100038d44: a949398f    	ldp	x15, x14, [x12, #0x90]
100038d48: ab0101ef    	adds	x15, x15, x1
100038d4c: 9a8e35ce    	cinc	x14, x14, hs
100038d50: a909398f    	stp	x15, x14, [x12, #0x90]
100038d54: 91004129    	add	x9, x9, #0x10
100038d58: f102413f    	cmp	x9, #0x90
100038d5c: 54fff641    	b.ne	0x100038c24 <__RINvNtNtNtCshgIkoLp8A2V_17field_regressions8campaign10candidates5words20exact_signed_columnsKj9_Kj13_EB8_+0xa8>
100038d60: f94022fc    	ldr	x28, [x23, #0x40]
100038d64: d37fff9e    	lsr	x30, x28, #63
100038d68: d37fff6e    	lsr	x14, x27, #63
100038d6c: 3819f3be    	sturb	w30, [x29, #-0x61]
100038d70: d10187ab    	sub	x11, x29, #0x61
100038d74: 3859f3a9    	ldurb	w9, [x29, #-0x61]
100038d78: aa0303ec    	mov	x12, x3
100038d7c: 9280000f    	mov	x15, #-0x1              ; =-1
100038d80: f2401d3f    	tst	x9, #0xff
100038d84: 9a8311ec    	csel	x12, x15, x3, ne
100038d88: 3819f3ae    	sturb	w14, [x29, #-0x61]
100038d8c: 3859f3a9    	ldurb	w9, [x29, #-0x61]
100038d90: aa0303e0    	mov	x0, x3
100038d94: f2401d3f    	tst	x9, #0xff
100038d98: 9a8311e0    	csel	x0, x15, x3, ne
100038d9c: 8a0c0108    	and	x8, x8, x12
100038da0: a9402ee9    	ldp	x9, x11, [x23]
100038da4: 8a000129    	and	x9, x9, x0
100038da8: a94e17e7    	ldp	x7, x5, [sp, #0xe0]
100038dac: ab090108    	adds	x8, x8, x9
100038db0: 1a9f37e9    	cset	w9, hs
100038db4: eb0800e8    	subs	x8, x7, x8
100038db8: da0900a9    	sbc	x9, x5, x9
100038dbc: a90e27e8    	stp	x8, x9, [sp, #0xe0]
100038dc0: 8a0c014a    	and	x10, x10, x12
100038dc4: 8a00016b    	and	x11, x11, x0
100038dc8: a94f17e7    	ldp	x7, x5, [sp, #0xf0]
100038dcc: ab0b014a    	adds	x10, x10, x11
100038dd0: 1a9f37eb    	cset	w11, hs
100038dd4: eb0a00ea    	subs	x10, x7, x10
100038dd8: da0b00ab    	sbc	x11, x5, x11
100038ddc: a90f2fea    	stp	x10, x11, [sp, #0xf0]
100038de0: 8a0c0084    	and	x4, x4, x12
100038de4: a9411ee5    	ldp	x5, x7, [x23, #0x10]
100038de8: 8a0000a5    	and	x5, x5, x0
100038dec: a95057f6    	ldp	x22, x21, [sp, #0x100]
100038df0: ab050084    	adds	x4, x4, x5
100038df4: 1a9f37e5    	cset	w5, hs
100038df8: eb0402c4    	subs	x4, x22, x4
100038dfc: da0502a5    	sbc	x5, x21, x5
100038e00: a91017e4    	stp	x4, x5, [sp, #0x100]
100038e04: 8a0c00c6    	and	x6, x6, x12
100038e08: 8a0000e7    	and	x7, x7, x0
100038e0c: a95157f6    	ldp	x22, x21, [sp, #0x110]
100038e10: ab0700c6    	adds	x6, x6, x7
100038e14: 1a9f37e7    	cset	w7, hs
100038e18: eb0602c6    	subs	x6, x22, x6
100038e1c: da0702a7    	sbc	x7, x21, x7
100038e20: a9111fe6    	stp	x6, x7, [sp, #0x110]
100038e24: 8a0c0294    	and	x20, x20, x12
100038e28: a9426af5    	ldp	x21, x26, [x23, #0x20]
100038e2c: 8a0002b5    	and	x21, x21, x0
100038e30: a9525be1    	ldp	x1, x22, [sp, #0x120]
100038e34: ab150294    	adds	x20, x20, x21
100038e38: 1a9f37ef    	cset	w15, hs
100038e3c: eb140035    	subs	x21, x1, x20
100038e40: da0f02d6    	sbc	x22, x22, x15
100038e44: a9125bf5    	stp	x21, x22, [sp, #0x120]
100038e48: 8a0c026f    	and	x15, x19, x12
100038e4c: 8a000341    	and	x1, x26, x0
100038e50: a95353f3    	ldp	x19, x20, [sp, #0x130]
100038e54: ab0101ef    	adds	x15, x15, x1
100038e58: 1a9f37e1    	cset	w1, hs
100038e5c: eb0f0273    	subs	x19, x19, x15
100038e60: da010294    	sbc	x20, x20, x1
100038e64: a91353f3    	stp	x19, x20, [sp, #0x130]
100038e68: 8a0c030f    	and	x15, x24, x12
100038e6c: a9436ae1    	ldp	x1, x26, [x23, #0x30]
100038e70: 8a000021    	and	x1, x1, x0
100038e74: a95463f7    	ldp	x23, x24, [sp, #0x140]
100038e78: ab0101ef    	adds	x15, x15, x1
100038e7c: 1a9f37e1    	cset	w1, hs
100038e80: eb0f02f7    	subs	x23, x23, x15
100038e84: da010318    	sbc	x24, x24, x1
100038e88: a91463f7    	stp	x23, x24, [sp, #0x140]
100038e8c: 8a0c032f    	and	x15, x25, x12
100038e90: 8a000341    	and	x1, x26, x0
100038e94: a9556bf9    	ldp	x25, x26, [sp, #0x150]
100038e98: ab0101ef    	adds	x15, x15, x1
100038e9c: 1a9f37e1    	cset	w1, hs
100038ea0: eb0f0339    	subs	x25, x25, x15
100038ea4: da01035a    	sbc	x26, x26, x1
100038ea8: a9156bf9    	stp	x25, x26, [sp, #0x150]
100038eac: 8a0c036c    	and	x12, x27, x12
100038eb0: 8a00038f    	and	x15, x28, x0
100038eb4: a95603e1    	ldp	x1, x0, [sp, #0x160]
100038eb8: ab0f018c    	adds	x12, x12, x15
100038ebc: 1a9f37ef    	cset	w15, hs
100038ec0: eb0c003b    	subs	x27, x1, x12
100038ec4: da0f001c    	sbc	x28, x0, x15
100038ec8: a91673fb    	stp	x27, x28, [sp, #0x160]
100038ecc: 91000631    	add	x17, x17, #0x1
100038ed0: 8a1e01cc    	and	x12, x14, x30
100038ed4: a9573bef    	ldp	x15, x14, [sp, #0x170]
100038ed8: ab0c01fe    	adds	x30, x15, x12
100038edc: 9a8e35cc    	cinc	x12, x14, hs
100038ee0: a91733fe    	stp	x30, x12, [sp, #0x170]
100038ee4: 91012210    	add	x16, x16, #0x48
100038ee8: eb02023f    	cmp	x17, x2
100038eec: 54ffe841    	b.ne	0x100038bf4 <__RINvNtNtNtCshgIkoLp8A2V_17field_regressions8campaign10candidates5words20exact_signed_columnsKj9_Kj13_EB8_+0x78>
100038ef0: a94543ec    	ldp	x12, x16, [sp, #0x50]
100038ef4: f90023ec    	str	x12, [sp, #0x40]
100038ef8: a94647e1    	ldp	x1, x17, [sp, #0x60]
100038efc: a9473be0    	ldp	x0, x14, [sp, #0x70]
100038f00: a94833ef    	ldp	x15, x12, [sp, #0x80]
100038f04: f9404fed    	ldr	x13, [sp, #0x98]
100038f08: f90007ed    	str	x13, [sp, #0x8]
100038f0c: f9404bed    	ldr	x13, [sp, #0x90]
100038f10: a94a0fe2    	ldp	x2, x3, [sp, #0xa0]
100038f14: f90003e2    	str	x2, [sp]
100038f18: f9405fe2    	ldr	x2, [sp, #0xb8]
100038f1c: f90017e2    	str	x2, [sp, #0x28]
100038f20: f9405be2    	ldr	x2, [sp, #0xb0]
100038f24: a9010fe2    	stp	x2, x3, [sp, #0x10]
100038f28: a94c0fe2    	ldp	x2, x3, [sp, #0xc0]
100038f2c: f90013e2    	str	x2, [sp, #0x20]
100038f30: f9406fe2    	ldr	x2, [sp, #0xd8]
100038f34: f9001fe2    	str	x2, [sp, #0x38]
100038f38: aa0303e2    	mov	x2, x3
100038f3c: f9406be3    	ldr	x3, [sp, #0xd0]
100038f40: 14000021    	b	0x100038fc4 <__RINvNtNtNtCshgIkoLp8A2V_17field_regressions8campaign10candidates5words20exact_signed_columnsKj9_Kj13_EB8_+0x448>
100038f44: d280001e    	mov	x30, #0x0               ; =0
100038f48: d280001b    	mov	x27, #0x0               ; =0
100038f4c: d280001c    	mov	x28, #0x0               ; =0
100038f50: d2800019    	mov	x25, #0x0               ; =0
100038f54: d280001a    	mov	x26, #0x0               ; =0
100038f58: d2800017    	mov	x23, #0x0               ; =0
100038f5c: d2800018    	mov	x24, #0x0               ; =0
100038f60: d2800013    	mov	x19, #0x0               ; =0
100038f64: d2800014    	mov	x20, #0x0               ; =0
100038f68: d2800015    	mov	x21, #0x0               ; =0
100038f6c: d2800016    	mov	x22, #0x0               ; =0
100038f70: d2800006    	mov	x6, #0x0                ; =0
100038f74: d2800007    	mov	x7, #0x0                ; =0
100038f78: d2800004    	mov	x4, #0x0                ; =0
100038f7c: d2800005    	mov	x5, #0x0                ; =0
100038f80: d280000a    	mov	x10, #0x0               ; =0
100038f84: d280000b    	mov	x11, #0x0               ; =0
100038f88: d2800008    	mov	x8, #0x0                ; =0
100038f8c: d2800009    	mov	x9, #0x0                ; =0
100038f90: d2800003    	mov	x3, #0x0                ; =0
100038f94: a903ffff    	stp	xzr, xzr, [sp, #0x38]
100038f98: a9027fff    	stp	xzr, xzr, [sp, #0x20]
100038f9c: a9017fff    	stp	xzr, xzr, [sp, #0x10]
100038fa0: a9007fff    	stp	xzr, xzr, [sp]
100038fa4: d280000d    	mov	x13, #0x0               ; =0
100038fa8: d280000f    	mov	x15, #0x0               ; =0
100038fac: d280000c    	mov	x12, #0x0               ; =0
100038fb0: d2800000    	mov	x0, #0x0                ; =0
100038fb4: d280000e    	mov	x14, #0x0               ; =0
100038fb8: d2800001    	mov	x1, #0x0                ; =0
100038fbc: d2800011    	mov	x17, #0x0               ; =0
100038fc0: d2800010    	mov	x16, #0x0               ; =0
100038fc4: ab010201    	adds	x1, x16, x1
100038fc8: 937ffe10    	asr	x16, x16, #63
100038fcc: 9a110211    	adc	x17, x16, x17
100038fd0: ab000230    	adds	x16, x17, x0
100038fd4: 937ffe31    	asr	x17, x17, #63
100038fd8: 9a0e0231    	adc	x17, x17, x14
100038fdc: ab0f022e    	adds	x14, x17, x15
100038fe0: 937ffe2f    	asr	x15, x17, #63
100038fe4: 9a0c01ef    	adc	x15, x15, x12
100038fe8: ab0d01ec    	adds	x12, x15, x13
100038fec: 937ffded    	asr	x13, x15, #63
100038ff0: f94007ef    	ldr	x15, [sp, #0x8]
100038ff4: 9a0f01ad    	adc	x13, x13, x15
100038ff8: f94003ef    	ldr	x15, [sp]
100038ffc: ab0f01af    	adds	x15, x13, x15
100039000: 937ffdad    	asr	x13, x13, #63
100039004: f9400ff1    	ldr	x17, [sp, #0x18]
100039008: 9a1101ad    	adc	x13, x13, x17
10003900c: f9400bf1    	ldr	x17, [sp, #0x10]
100039010: ab1101b1    	adds	x17, x13, x17
100039014: 937ffdad    	asr	x13, x13, #63
100039018: f94017e0    	ldr	x0, [sp, #0x28]
10003901c: 9a0001ad    	adc	x13, x13, x0
100039020: f94013e0    	ldr	x0, [sp, #0x20]
100039024: ab0001a0    	adds	x0, x13, x0
100039028: 937ffdad    	asr	x13, x13, #63
10003902c: 9a0201ad    	adc	x13, x13, x2
100039030: ab0301a2    	adds	x2, x13, x3
100039034: 937ffdad    	asr	x13, x13, #63
100039038: f9401fe3    	ldr	x3, [sp, #0x38]
10003903c: 9a0301ad    	adc	x13, x13, x3
100039040: ab0801a8    	adds	x8, x13, x8
100039044: 937ffdad    	asr	x13, x13, #63
100039048: 9a0901a9    	adc	x9, x13, x9
10003904c: ab0a012a    	adds	x10, x9, x10
100039050: 937ffd29    	asr	x9, x9, #63
100039054: 9a0b0129    	adc	x9, x9, x11
100039058: ab04012b    	adds	x11, x9, x4
10003905c: 937ffd29    	asr	x9, x9, #63
100039060: 9a050129    	adc	x9, x9, x5
100039064: ab06012d    	adds	x13, x9, x6
100039068: 937ffd29    	asr	x9, x9, #63
10003906c: 9a070129    	adc	x9, x9, x7
100039070: ab150123    	adds	x3, x9, x21
100039074: 937ffd29    	asr	x9, x9, #63
100039078: 9a160129    	adc	x9, x9, x22
10003907c: f9401be4    	ldr	x4, [sp, #0x30]
100039080: f94023e5    	ldr	x5, [sp, #0x40]
100039084: a9000485    	stp	x5, x1, [x4]
100039088: ab130121    	adds	x1, x9, x19
10003908c: 937ffd29    	asr	x9, x9, #63
100039090: 9a140129    	adc	x9, x9, x20
100039094: a9013890    	stp	x16, x14, [x4, #0x10]
100039098: a9023c8c    	stp	x12, x15, [x4, #0x20]
10003909c: ab17012c    	adds	x12, x9, x23
1000390a0: 937ffd29    	asr	x9, x9, #63
1000390a4: 9a180129    	adc	x9, x9, x24
1000390a8: a9030091    	stp	x17, x0, [x4, #0x30]
1000390ac: a9042082    	stp	x2, x8, [x4, #0x40]
1000390b0: ab190128    	adds	x8, x9, x25
1000390b4: 937ffd29    	asr	x9, x9, #63
1000390b8: 9a1a0129    	adc	x9, x9, x26
1000390bc: a9052c8a    	stp	x10, x11, [x4, #0x50]
1000390c0: a9060c8d    	stp	x13, x3, [x4, #0x60]
1000390c4: ab1b012a    	adds	x10, x9, x27
1000390c8: 937ffd29    	asr	x9, x9, #63
1000390cc: 9a1c0129    	adc	x9, x9, x28
1000390d0: a9073081    	stp	x1, x12, [x4, #0x70]
1000390d4: 8b1e0129    	add	x9, x9, x30
1000390d8: a9082888    	stp	x8, x10, [x4, #0x80]
1000390dc: f9004889    	str	x9, [x4, #0x90]
1000390e0: 910683ff    	add	sp, sp, #0x1a0
1000390e4: a9457bfd    	ldp	x29, x30, [sp, #0x50]
1000390e8: a9444ff4    	ldp	x20, x19, [sp, #0x40]
1000390ec: a94357f6    	ldp	x22, x21, [sp, #0x30]
1000390f0: a9425ff8    	ldp	x24, x23, [sp, #0x20]
1000390f4: a94167fa    	ldp	x26, x25, [sp, #0x10]
1000390f8: a8c66ffc    	ldp	x28, x27, [sp], #0x60
1000390fc: d65f03c0    	ret
100039100: d0000ba4    	adrp	x4, 0x1001af000 <dyld_stub_binder+0x1001af000>
100039104: 91376084    	add	x4, x4, #0xdd8
100039108: 910123e0    	add	x0, sp, #0x48
10003910c: 910143e1    	add	x1, sp, #0x50
100039110: d2800002    	mov	x2, #0x0                ; =0
100039114: 94048958    	bl	0x10015b674 <__RINvNtCs8Mbv00yxnRz_4core9panicking13assert_failedjjEB4_>

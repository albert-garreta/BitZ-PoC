
/private/var/folders/q0/v7l8dzd13k7f_q6j80sy03lm0000gn/T/f2z-field-gated-h_tggvge/field-regressions:	file format mach-o arm64

Disassembly of section __TEXT,__text:

00000001000e6620 <__RNvNtNtNtNtCsfDw8YqMpZPC_17field_regressions8campaign10candidates4grid4neon9fold_tile>:
1000e6620: a9ba6ffc    	stp	x28, x27, [sp, #-0x60]!
1000e6624: a90167fa    	stp	x26, x25, [sp, #0x10]
1000e6628: a9025ff8    	stp	x24, x23, [sp, #0x20]
1000e662c: a90357f6    	stp	x22, x21, [sp, #0x30]
1000e6630: a9044ff4    	stp	x20, x19, [sp, #0x40]
1000e6634: a9057bfd    	stp	x29, x30, [sp, #0x50]
1000e6638: 910143fd    	add	x29, sp, #0x50
1000e663c: a9a10fe5    	stp	x5, x3, [sp, #-0x1f0]!
1000e6640: eb05009f    	cmp	x4, x5
1000e6644: 54001002    	b.hs	0x1000e6844 <__RNvNtNtNtNtCsfDw8YqMpZPC_17field_regressions8campaign10candidates4grid4neon9fold_tile+0x224>
1000e6648: aa0403f6    	mov	x22, x4
1000e664c: aa0203f3    	mov	x19, x2
1000e6650: aa0003f4    	mov	x20, x0
1000e6654: ad4000c1    	ldp	q1, q0, [x6]
1000e6658: ad0507e0    	stp	q0, q1, [sp, #0xa0]
1000e665c: 52800028    	mov	w8, #0x1                ; =1
1000e6660: 52800609    	mov	w9, #0x30               ; =48
1000e6664: 528010ea    	mov	w10, #0x87              ; =135
1000e6668: ad4100c1    	ldp	q1, q0, [x6, #0x20]
1000e666c: ad0407e0    	stp	q0, q1, [sp, #0x80]
1000e6670: aa040919    	orr	x25, x8, x4, lsl #2
1000e6674: ad4200c1    	ldp	q1, q0, [x6, #0x40]
1000e6678: ad0307e0    	stp	q0, q1, [sp, #0x60]
1000e667c: 4e080d40    	dup.2d	v0, x10
1000e6680: 3d8017e0    	str	q0, [sp, #0x50]
1000e6684: aa04193b    	orr	x27, x9, x4, lsl #6
1000e6688: ad4507e0    	ldp	q0, q1, [sp, #0xa0]
1000e668c: ad0603e1    	stp	q1, q0, [sp, #0xc0]
1000e6690: ad4407e0    	ldp	q0, q1, [sp, #0x80]
1000e6694: ad0703e1    	stp	q1, q0, [sp, #0xe0]
1000e6698: ad4307e0    	ldp	q0, q1, [sp, #0x60]
1000e669c: ad0803e1    	stp	q1, q0, [sp, #0x100]
1000e66a0: 3dc017e1    	ldr	q1, [sp, #0x50]
1000e66a4: 6f00e400    	movi.2d	v0, #0000000000000000
1000e66a8: ad0903e1    	stp	q1, q0, [sp, #0x120]
1000e66ac: f81083b6    	stur	x22, [x29, #-0xf8]
1000e66b0: d103e3bc    	sub	x28, x29, #0xf8
1000e66b4: a93553bc    	stp	x28, x20, [x29, #-0xb0]
1000e66b8: 910303f5    	add	x21, sp, #0xc0
1000e66bc: a93657a1    	stp	x1, x21, [x29, #-0xa0]
1000e66c0: 910343fa    	add	x26, sp, #0xd0
1000e66c4: 910383e8    	add	x8, sp, #0xe0
1000e66c8: a93723ba    	stp	x26, x8, [x29, #-0x90]
1000e66cc: 9103c3e8    	add	x8, sp, #0xf0
1000e66d0: f81803a8    	stur	x8, [x29, #-0x80]
1000e66d4: 910403e8    	add	x8, sp, #0x100
1000e66d8: f81883a8    	stur	x8, [x29, #-0x78]
1000e66dc: 910443e8    	add	x8, sp, #0x110
1000e66e0: f81903a8    	stur	x8, [x29, #-0x70]
1000e66e4: 910483e8    	add	x8, sp, #0x120
1000e66e8: f81983a8    	stur	x8, [x29, #-0x68]
1000e66ec: 9104c3e8    	add	x8, sp, #0x130
1000e66f0: f81a03a8    	stur	x8, [x29, #-0x60]
1000e66f4: d103c3a0    	sub	x0, x29, #0xf0
1000e66f8: aa0103f7    	mov	x23, x1
1000e66fc: d102c3a1    	sub	x1, x29, #0xb0
1000e6700: 97fc8c73    	bl	0x1000098cc <__RINvNtCs8Mbv00yxnRz_4core5array11try_from_fnINtNtNtB4_3ops9try_trait17NeverShortCircuitNtNtNtNtB4_9core_arch10arm_shared4neon10uint64x2_tEKj4_NCINvMBJ_BG_10wrap_mut_1jNCNvNtNtNtNtCsfDw8YqMpZPC_17field_regressions8campaign10candidates4grid4neon17fold_quad_logicals0_0E0EB2R_>
1000e6704: ad7883a1    	ldp	q1, q0, [x29, #-0xf0]
1000e6708: ad0187e0    	stp	q0, q1, [sp, #0x30]
1000e670c: ad7983a1    	ldp	q1, q0, [x29, #-0xd0]
1000e6710: ad0087e0    	stp	q0, q1, [sp, #0x10]
1000e6714: ad4507e0    	ldp	q0, q1, [sp, #0xa0]
1000e6718: ad0603e1    	stp	q1, q0, [sp, #0xc0]
1000e671c: ad4407e0    	ldp	q0, q1, [sp, #0x80]
1000e6720: ad0703e1    	stp	q1, q0, [sp, #0xe0]
1000e6724: ad4307e0    	ldp	q0, q1, [sp, #0x60]
1000e6728: ad0803e1    	stp	q1, q0, [sp, #0x100]
1000e672c: 3dc017e1    	ldr	q1, [sp, #0x50]
1000e6730: 6f00e400    	movi.2d	v0, #0000000000000000
1000e6734: ad0903e1    	stp	q1, q0, [sp, #0x120]
1000e6738: f81083b6    	stur	x22, [x29, #-0xf8]
1000e673c: a9354fbc    	stp	x28, x19, [x29, #-0xb0]
1000e6740: aa1303f8    	mov	x24, x19
1000e6744: f94007fc    	ldr	x28, [sp, #0x8]
1000e6748: a93657bc    	stp	x28, x21, [x29, #-0xa0]
1000e674c: 910383e8    	add	x8, sp, #0xe0
1000e6750: a93723ba    	stp	x26, x8, [x29, #-0x90]
1000e6754: 9103c3e8    	add	x8, sp, #0xf0
1000e6758: f81803a8    	stur	x8, [x29, #-0x80]
1000e675c: 910403e8    	add	x8, sp, #0x100
1000e6760: f81883a8    	stur	x8, [x29, #-0x78]
1000e6764: 910443e8    	add	x8, sp, #0x110
1000e6768: f81903a8    	stur	x8, [x29, #-0x70]
1000e676c: 910483e8    	add	x8, sp, #0x120
1000e6770: f81983a8    	stur	x8, [x29, #-0x68]
1000e6774: 9104c3e8    	add	x8, sp, #0x130
1000e6778: f81a03a8    	stur	x8, [x29, #-0x60]
1000e677c: d103c3a0    	sub	x0, x29, #0xf0
1000e6780: d102c3a1    	sub	x1, x29, #0xb0
1000e6784: 97fc8c52    	bl	0x1000098cc <__RINvNtCs8Mbv00yxnRz_4core5array11try_from_fnINtNtNtB4_3ops9try_trait17NeverShortCircuitNtNtNtNtB4_9core_arch10arm_shared4neon10uint64x2_tEKj4_NCINvMBJ_BG_10wrap_mut_1jNCNvNtNtNtNtCsfDw8YqMpZPC_17field_regressions8campaign10candidates4grid4neon17fold_quad_logicals0_0E0EB2R_>
1000e6788: d1000728    	sub	x8, x25, #0x1
1000e678c: eb17011f    	cmp	x8, x23
1000e6790: 540006a2    	b.hs	0x1000e6864 <__RNvNtNtNtNtCsfDw8YqMpZPC_17field_regressions8campaign10candidates4grid4neon9fold_tile+0x244>
1000e6794: ad788ba3    	ldp	q3, q2, [x29, #-0xf0]
1000e6798: ad7983a1    	ldp	q1, q0, [x29, #-0xd0]
1000e679c: 8b1b0289    	add	x9, x20, x27
1000e67a0: 3dc013e4    	ldr	q4, [sp, #0x40]
1000e67a4: 3c9d0124    	stur	q4, [x9, #-0x30]
1000e67a8: eb1c011f    	cmp	x8, x28
1000e67ac: 54000802    	b.hs	0x1000e68ac <__RNvNtNtNtNtCsfDw8YqMpZPC_17field_regressions8campaign10candidates4grid4neon9fold_tile+0x28c>
1000e67b0: aa1703e1    	mov	x1, x23
1000e67b4: 8b1b0308    	add	x8, x24, x27
1000e67b8: 3c9d0103    	stur	q3, [x8, #-0x30]
1000e67bc: eb17033f    	cmp	x25, x23
1000e67c0: 54000542    	b.hs	0x1000e6868 <__RNvNtNtNtNtCsfDw8YqMpZPC_17field_regressions8campaign10candidates4grid4neon9fold_tile+0x248>
1000e67c4: 8b1b0288    	add	x8, x20, x27
1000e67c8: 3dc00fe3    	ldr	q3, [sp, #0x30]
1000e67cc: 3c9e0103    	stur	q3, [x8, #-0x20]
1000e67d0: eb1c033f    	cmp	x25, x28
1000e67d4: 540006e2    	b.hs	0x1000e68b0 <__RNvNtNtNtNtCsfDw8YqMpZPC_17field_regressions8campaign10candidates4grid4neon9fold_tile+0x290>
1000e67d8: aa1803f3    	mov	x19, x24
1000e67dc: 8b1b0308    	add	x8, x24, x27
1000e67e0: 3c9e0102    	stur	q2, [x8, #-0x20]
1000e67e4: 91000728    	add	x8, x25, #0x1
1000e67e8: eb01011f    	cmp	x8, x1
1000e67ec: 540003c2    	b.hs	0x1000e6864 <__RNvNtNtNtNtCsfDw8YqMpZPC_17field_regressions8campaign10candidates4grid4neon9fold_tile+0x244>
1000e67f0: 8b1b0289    	add	x9, x20, x27
1000e67f4: 3dc00be2    	ldr	q2, [sp, #0x20]
1000e67f8: 3c9f0122    	stur	q2, [x9, #-0x10]
1000e67fc: eb1c011f    	cmp	x8, x28
1000e6800: 540004a2    	b.hs	0x1000e6894 <__RNvNtNtNtNtCsfDw8YqMpZPC_17field_regressions8campaign10candidates4grid4neon9fold_tile+0x274>
1000e6804: 8b1b0268    	add	x8, x19, x27
1000e6808: 3c9f0101    	stur	q1, [x8, #-0x10]
1000e680c: 91000b28    	add	x8, x25, #0x2
1000e6810: eb01011f    	cmp	x8, x1
1000e6814: 54000282    	b.hs	0x1000e6864 <__RNvNtNtNtNtCsfDw8YqMpZPC_17field_regressions8campaign10candidates4grid4neon9fold_tile+0x244>
1000e6818: 3dc007e1    	ldr	q1, [sp, #0x10]
1000e681c: 3cbb6a81    	str	q1, [x20, x27]
1000e6820: eb1c011f    	cmp	x8, x28
1000e6824: 540002c2    	b.hs	0x1000e687c <__RNvNtNtNtNtCsfDw8YqMpZPC_17field_regressions8campaign10candidates4grid4neon9fold_tile+0x25c>
1000e6828: 910006d6    	add	x22, x22, #0x1
1000e682c: 3cbb6a60    	str	q0, [x19, x27]
1000e6830: 91001339    	add	x25, x25, #0x4
1000e6834: 9101037b    	add	x27, x27, #0x40
1000e6838: f94003e8    	ldr	x8, [sp]
1000e683c: eb16011f    	cmp	x8, x22
1000e6840: 54fff241    	b.ne	0x1000e6688 <__RNvNtNtNtNtCsfDw8YqMpZPC_17field_regressions8campaign10candidates4grid4neon9fold_tile+0x68>
1000e6844: 9107c3ff    	add	sp, sp, #0x1f0
1000e6848: a9457bfd    	ldp	x29, x30, [sp, #0x50]
1000e684c: a9444ff4    	ldp	x20, x19, [sp, #0x40]
1000e6850: a94357f6    	ldp	x22, x21, [sp, #0x30]
1000e6854: a9425ff8    	ldp	x24, x23, [sp, #0x20]
1000e6858: a94167fa    	ldp	x26, x25, [sp, #0x10]
1000e685c: a8c66ffc    	ldp	x28, x27, [sp], #0x60
1000e6860: d65f03c0    	ret
1000e6864: aa0803f9    	mov	x25, x8
1000e6868: b0000662    	adrp	x2, 0x1001b3000 <dyld_stub_binder+0x1001b3000>
1000e686c: 913c2042    	add	x2, x2, #0xf08
1000e6870: aa1903e0    	mov	x0, x25
1000e6874: aa1703e1    	mov	x1, x23
1000e6878: 9401d805    	bl	0x10015c88c <__RNvNtCs8Mbv00yxnRz_4core9panicking18panic_bounds_check>
1000e687c: 91000b39    	add	x25, x25, #0x2
1000e6880: b0000662    	adrp	x2, 0x1001b3000 <dyld_stub_binder+0x1001b3000>
1000e6884: 913c8042    	add	x2, x2, #0xf20
1000e6888: aa1903e0    	mov	x0, x25
1000e688c: f94007e1    	ldr	x1, [sp, #0x8]
1000e6890: 9401d7ff    	bl	0x10015c88c <__RNvNtCs8Mbv00yxnRz_4core9panicking18panic_bounds_check>
1000e6894: 91000739    	add	x25, x25, #0x1
1000e6898: b0000662    	adrp	x2, 0x1001b3000 <dyld_stub_binder+0x1001b3000>
1000e689c: 913c8042    	add	x2, x2, #0xf20
1000e68a0: aa1903e0    	mov	x0, x25
1000e68a4: f94007e1    	ldr	x1, [sp, #0x8]
1000e68a8: 9401d7f9    	bl	0x10015c88c <__RNvNtCs8Mbv00yxnRz_4core9panicking18panic_bounds_check>
1000e68ac: aa0803f9    	mov	x25, x8
1000e68b0: b0000662    	adrp	x2, 0x1001b3000 <dyld_stub_binder+0x1001b3000>
1000e68b4: 913c8042    	add	x2, x2, #0xf20
1000e68b8: aa1903e0    	mov	x0, x25
1000e68bc: f94007e1    	ldr	x1, [sp, #0x8]
1000e68c0: 9401d7f3    	bl	0x10015c88c <__RNvNtCs8Mbv00yxnRz_4core9panicking18panic_bounds_check>

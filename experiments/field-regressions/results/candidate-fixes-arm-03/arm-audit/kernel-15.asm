
/private/var/folders/q0/v7l8dzd13k7f_q6j80sy03lm0000gn/T/f2z-field-gated-h_tggvge/field-regressions:	file format mach-o arm64

Disassembly of section __TEXT,__text:

00000001000e62d8 <__RNvNtNtNtNtCsfDw8YqMpZPC_17field_regressions8campaign10candidates4grid4neon10accumulate>:
1000e62d8: d342fc28    	lsr	x8, x1, #2
1000e62dc: d342fc69    	lsr	x9, x3, #2
1000e62e0: eb08013f    	cmp	x9, x8
1000e62e4: 9a883128    	csel	x8, x9, x8, lo
1000e62e8: eb0800bf    	cmp	x5, x8
1000e62ec: 9a8830a8    	csel	x8, x5, x8, lo
1000e62f0: b4001968    	cbz	x8, 0x1000e661c <__RNvNtNtNtNtCsfDw8YqMpZPC_17field_regressions8campaign10candidates4grid4neon10accumulate+0x344>
1000e62f4: d10203ff    	sub	sp, sp, #0x80
1000e62f8: 6d043bef    	stp	d15, d14, [sp, #0x40]
1000e62fc: 6d0533ed    	stp	d13, d12, [sp, #0x50]
1000e6300: 6d062beb    	stp	d11, d10, [sp, #0x60]
1000e6304: 6d0723e9    	stp	d9, d8, [sp, #0x70]
1000e6308: ad4058d7    	ldp	q23, q22, [x6]
1000e630c: ad4150d5    	ldp	q21, q20, [x6, #0x20]
1000e6310: ad4248d3    	ldp	q19, q18, [x6, #0x40]
1000e6314: ad4340d1    	ldp	q17, q16, [x6, #0x60]
1000e6318: ad4418c7    	ldp	q7, q6, [x6, #0x80]
1000e631c: ad4510c5    	ldp	q5, q4, [x6, #0xa0]
1000e6320: ad4678c3    	ldp	q3, q30, [x6, #0xc0]
1000e6324: ad4770dd    	ldp	q29, q28, [x6, #0xe0]
1000e6328: 91008049    	add	x9, x2, #0x20
1000e632c: 9100208a    	add	x10, x4, #0x8
1000e6330: 9100800b    	add	x11, x0, #0x20
1000e6334: 528010ec    	mov	w12, #0x87              ; =135
1000e6338: ad4860d9    	ldp	q25, q24, [x6, #0x100]
1000e633c: 6f00e41a    	movi.2d	v26, #0000000000000000
1000e6340: 4e080d80    	dup.2d	v0, x12
1000e6344: 3d8003e0    	str	q0, [sp]
1000e6348: ad00f7fe    	stp	q30, q29, [sp, #0x10]
1000e634c: 3d800ffc    	str	q28, [sp, #0x30]
1000e6350: fd40015c    	ldr	d28, [x10]
1000e6354: 3dc003e0    	ldr	q0, [sp]
1000e6358: 0ee0e39d    	pmull.1q	v29, v28, v0
1000e635c: 4e183fac    	mov.d	x12, v29[1]
1000e6360: f85f814d    	ldur	x13, [x10, #-0x8]
1000e6364: ca0d018c    	eor	x12, x12, x13
1000e6368: 9e67019e    	fmov	d30, x12
1000e636c: 6d407d68    	ldp	d8, d31, [x11]
1000e6370: 0efee3e9    	pmull.1q	v9, v31, v30
1000e6374: 0efce10a    	pmull.1q	v10, v8, v28
1000e6378: 6e291d49    	eor.16b	v9, v10, v9
1000e637c: 6e09434a    	ext.16b	v10, v26, v9, #0x8
1000e6380: 0efde3ff    	pmull.1q	v31, v31, v29
1000e6384: 9e6701ab    	fmov	d11, x13
1000e6388: 0eebe108    	pmull.1q	v8, v8, v11
1000e638c: 6d7f316d    	ldp	d13, d12, [x11, #-0x10]
1000e6390: 4ee0e129    	pmull2.1q	v9, v9, v0
1000e6394: 0efee18e    	pmull.1q	v14, v12, v30
1000e6398: 0efce1af    	pmull.1q	v15, v13, v28
1000e639c: 6e2e1dee    	eor.16b	v14, v15, v14
1000e63a0: 6e0e434f    	ext.16b	v15, v26, v14, #0x8
1000e63a4: 0efde18c    	pmull.1q	v12, v12, v29
1000e63a8: ce1f2508    	eor3.16b	v8, v8, v31, v9
1000e63ac: 0eebe1bf    	pmull.1q	v31, v13, v11
1000e63b0: 4ee0e1c9    	pmull2.1q	v9, v14, v0
1000e63b4: 6d7e356e    	ldp	d14, d13, [x11, #-0x20]
1000e63b8: 0efee1bb    	pmull.1q	v27, v13, v30
1000e63bc: ce0c27ec    	eor3.16b	v12, v31, v12, v9
1000e63c0: 0efce1df    	pmull.1q	v31, v14, v28
1000e63c4: 6e3b1ffb    	eor.16b	v27, v31, v27
1000e63c8: 6e1b435f    	ext.16b	v31, v26, v27, #0x8
1000e63cc: 0efde1a9    	pmull.1q	v9, v13, v29
1000e63d0: 0eebe1cd    	pmull.1q	v13, v14, v11
1000e63d4: 6e281d48    	eor.16b	v8, v10, v8
1000e63d8: 4ee0e37b    	pmull2.1q	v27, v27, v0
1000e63dc: ce096da9    	eor3.16b	v9, v13, v9, v27
1000e63e0: 6d41357b    	ldp	d27, d13, [x11, #0x10]
1000e63e4: 0eebe36e    	pmull.1q	v14, v27, v11
1000e63e8: 6e2c1dea    	eor.16b	v10, v15, v12
1000e63ec: 0efde1bd    	pmull.1q	v29, v13, v29
1000e63f0: 0efce37b    	pmull.1q	v27, v27, v28
1000e63f4: 0efee1bc    	pmull.1q	v28, v13, v30
1000e63f8: 6e3b1f8b    	eor.16b	v11, v28, v27
1000e63fc: 4ee0e17b    	pmull2.1q	v27, v11, v0
1000e6400: 6e291fef    	eor.16b	v15, v31, v9
1000e6404: ce0e6fac    	eor3.16b	v12, v29, v14, v27
1000e6408: ce092bed    	eor3.16b	v13, v31, v9, v10
1000e640c: ad7f713e    	ldp	q30, q28, [x9, #-0x20]
1000e6410: 6e3e1f9d    	eor.16b	v29, v28, v30
1000e6414: 0efee1fb    	pmull.1q	v27, v15, v30
1000e6418: 4efee1ee    	pmull2.1q	v14, v15, v30
1000e641c: 4e0805e0    	dup.2d	v0, v15[0]
1000e6420: 4e0807c1    	dup.2d	v1, v30[0]
1000e6424: 4efee000    	pmull2.1q	v0, v0, v30
1000e6428: 4ee1e1e1    	pmull2.1q	v1, v15, v1
1000e642c: 6e201c20    	eor.16b	v0, v1, v0
1000e6430: 6e004341    	ext.16b	v1, v26, v0, #0x8
1000e6434: 6e1a4000    	ext.16b	v0, v0, v26, #0x8
1000e6438: 0efce14f    	pmull.1q	v15, v10, v28
1000e643c: ce1b06f7    	eor3.16b	v23, v23, v27, v1
1000e6440: 4efce141    	pmull2.1q	v1, v10, v28
1000e6444: 4e08055b    	dup.2d	v27, v10[0]
1000e6448: 4e080782    	dup.2d	v2, v28[0]
1000e644c: ce0e5816    	eor3.16b	v22, v0, v14, v22
1000e6450: 4efce360    	pmull2.1q	v0, v27, v28
1000e6454: 4ee2e142    	pmull2.1q	v2, v10, v2
1000e6458: 6e201c40    	eor.16b	v0, v2, v0
1000e645c: 6e004342    	ext.16b	v2, v26, v0, #0x8
1000e6460: 6e1a4000    	ext.16b	v0, v0, v26, #0x8
1000e6464: ce0f0ab5    	eor3.16b	v21, v21, v15, v2
1000e6468: 0efde1a2    	pmull.1q	v2, v13, v29
1000e646c: 4e0805bb    	dup.2d	v27, v13[0]
1000e6470: 4e0807ae    	dup.2d	v14, v29[0]
1000e6474: ce015014    	eor3.16b	v20, v0, v1, v20
1000e6478: 4efde360    	pmull2.1q	v0, v27, v29
1000e647c: 4eeee1a1    	pmull2.1q	v1, v13, v14
1000e6480: 6e201c20    	eor.16b	v0, v1, v0
1000e6484: 6e004341    	ext.16b	v1, v26, v0, #0x8
1000e6488: acc23d2e    	ldp	q14, q15, [x9], #0x40
1000e648c: ce020673    	eor3.16b	v19, v19, v2, v1
1000e6490: 0eeee101    	pmull.1q	v1, v8, v14
1000e6494: 4e0805c2    	dup.2d	v2, v14[0]
1000e6498: 4e08051b    	dup.2d	v27, v8[0]
1000e649c: 4eeee37b    	pmull2.1q	v27, v27, v14
1000e64a0: 4ee2e102    	pmull2.1q	v2, v8, v2
1000e64a4: 6e3b1c42    	eor.16b	v2, v2, v27
1000e64a8: 6e02435b    	ext.16b	v27, v26, v2, #0x8
1000e64ac: ce016e31    	eor3.16b	v17, v17, v1, v27
1000e64b0: 4eeee101    	pmull2.1q	v1, v8, v14
1000e64b4: 6e1a4042    	ext.16b	v2, v2, v26, #0x8
1000e64b8: ce014050    	eor3.16b	v16, v2, v1, v16
1000e64bc: 6e0b4341    	ext.16b	v1, v26, v11, #0x8
1000e64c0: 6e2c1c22    	eor.16b	v2, v1, v12
1000e64c4: ce0c282a    	eor3.16b	v10, v1, v12, v10
1000e64c8: ce0c2021    	eor3.16b	v1, v1, v12, v8
1000e64cc: ce0923fb    	eor3.16b	v27, v31, v9, v8
1000e64d0: ce08345f    	eor3.16b	v31, v2, v8, v13
1000e64d4: 4efde1a8    	pmull2.1q	v8, v13, v29
1000e64d8: 6e1a4000    	ext.16b	v0, v0, v26, #0x8
1000e64dc: 0eefe049    	pmull.1q	v9, v2, v15
1000e64e0: ce084812    	eor3.16b	v18, v0, v8, v18
1000e64e4: 4e080440    	dup.2d	v0, v2[0]
1000e64e8: 4eefe000    	pmull2.1q	v0, v0, v15
1000e64ec: 4e0805e8    	dup.2d	v8, v15[0]
1000e64f0: 4ee8e048    	pmull2.1q	v8, v2, v8
1000e64f4: 6e201d00    	eor.16b	v0, v8, v0
1000e64f8: 6e004348    	ext.16b	v8, v26, v0, #0x8
1000e64fc: ce0920e7    	eor3.16b	v7, v7, v9, v8
1000e6500: 6e2e1de8    	eor.16b	v8, v15, v14
1000e6504: 4eefe042    	pmull2.1q	v2, v2, v15
1000e6508: 6e1a4000    	ext.16b	v0, v0, v26, #0x8
1000e650c: ce021806    	eor3.16b	v6, v0, v2, v6
1000e6510: 4e080420    	dup.2d	v0, v1[0]
1000e6514: 4e080502    	dup.2d	v2, v8[0]
1000e6518: 4ee8e000    	pmull2.1q	v0, v0, v8
1000e651c: 4ee2e022    	pmull2.1q	v2, v1, v2
1000e6520: 6e201c40    	eor.16b	v0, v2, v0
1000e6524: 6e004342    	ext.16b	v2, v26, v0, #0x8
1000e6528: 0ee8e029    	pmull.1q	v9, v1, v8
1000e652c: ce0908a5    	eor3.16b	v5, v5, v9, v2
1000e6530: 6e3e1dc2    	eor.16b	v2, v14, v30
1000e6534: 4ee8e021    	pmull2.1q	v1, v1, v8
1000e6538: 6e1a4000    	ext.16b	v0, v0, v26, #0x8
1000e653c: 4e08077e    	dup.2d	v30, v27[0]
1000e6540: 4ee2e3de    	pmull2.1q	v30, v30, v2
1000e6544: ce011004    	eor3.16b	v4, v0, v1, v4
1000e6548: 4e080440    	dup.2d	v0, v2[0]
1000e654c: 4ee0e360    	pmull2.1q	v0, v27, v0
1000e6550: 6e3e1c00    	eor.16b	v0, v0, v30
1000e6554: 0ee2e361    	pmull.1q	v1, v27, v2
1000e6558: 6e00435e    	ext.16b	v30, v26, v0, #0x8
1000e655c: ce017863    	eor3.16b	v3, v3, v1, v30
1000e6560: ce0e75e1    	eor3.16b	v1, v15, v14, v29
1000e6564: ad40f7fe    	ldp	q30, q29, [sp, #0x10]
1000e6568: 6e3c1dfc    	eor.16b	v28, v15, v28
1000e656c: 4ee2e362    	pmull2.1q	v2, v27, v2
1000e6570: 6e1a4000    	ext.16b	v0, v0, v26, #0x8
1000e6574: ce02781e    	eor3.16b	v30, v0, v2, v30
1000e6578: 4e080540    	dup.2d	v0, v10[0]
1000e657c: 4efce000    	pmull2.1q	v0, v0, v28
1000e6580: 4e080782    	dup.2d	v2, v28[0]
1000e6584: 4ee2e142    	pmull2.1q	v2, v10, v2
1000e6588: 6e201c40    	eor.16b	v0, v2, v0
1000e658c: 0efce142    	pmull.1q	v2, v10, v28
1000e6590: 6e00435b    	ext.16b	v27, v26, v0, #0x8
1000e6594: ce026fbd    	eor3.16b	v29, v29, v2, v27
1000e6598: 4efce142    	pmull2.1q	v2, v10, v28
1000e659c: 3dc00ffc    	ldr	q28, [sp, #0x30]
1000e65a0: 6e1a4000    	ext.16b	v0, v0, v26, #0x8
1000e65a4: ce02701c    	eor3.16b	v28, v0, v2, v28
1000e65a8: 4e0807e0    	dup.2d	v0, v31[0]
1000e65ac: 4e080422    	dup.2d	v2, v1[0]
1000e65b0: 4ee1e000    	pmull2.1q	v0, v0, v1
1000e65b4: 4ee2e3e2    	pmull2.1q	v2, v31, v2
1000e65b8: 6e201c40    	eor.16b	v0, v2, v0
1000e65bc: 6e004342    	ext.16b	v2, v26, v0, #0x8
1000e65c0: 0ee1e3fb    	pmull.1q	v27, v31, v1
1000e65c4: ce1b0b39    	eor3.16b	v25, v25, v27, v2
1000e65c8: 4ee1e3e1    	pmull2.1q	v1, v31, v1
1000e65cc: 6e1a4000    	ext.16b	v0, v0, v26, #0x8
1000e65d0: ce016018    	eor3.16b	v24, v0, v1, v24
1000e65d4: 9100414a    	add	x10, x10, #0x10
1000e65d8: 9101016b    	add	x11, x11, #0x40
1000e65dc: f1000508    	subs	x8, x8, #0x1
1000e65e0: 54ffeb41    	b.ne	0x1000e6348 <__RNvNtNtNtNtCsfDw8YqMpZPC_17field_regressions8campaign10candidates4grid4neon10accumulate+0x70>
1000e65e4: ad0058d7    	stp	q23, q22, [x6]
1000e65e8: ad0150d5    	stp	q21, q20, [x6, #0x20]
1000e65ec: ad0248d3    	stp	q19, q18, [x6, #0x40]
1000e65f0: ad0340d1    	stp	q17, q16, [x6, #0x60]
1000e65f4: ad0418c7    	stp	q7, q6, [x6, #0x80]
1000e65f8: ad0510c5    	stp	q5, q4, [x6, #0xa0]
1000e65fc: ad0678c3    	stp	q3, q30, [x6, #0xc0]
1000e6600: ad0770dd    	stp	q29, q28, [x6, #0xe0]
1000e6604: ad0860d9    	stp	q25, q24, [x6, #0x100]
1000e6608: 6d4723e9    	ldp	d9, d8, [sp, #0x70]
1000e660c: 6d462beb    	ldp	d11, d10, [sp, #0x60]
1000e6610: 6d4533ed    	ldp	d13, d12, [sp, #0x50]
1000e6614: 6d443bef    	ldp	d15, d14, [sp, #0x40]
1000e6618: 910203ff    	add	sp, sp, #0x80
1000e661c: d65f03c0    	ret


/private/var/folders/q0/v7l8dzd13k7f_q6j80sy03lm0000gn/T/f2z-field-gated-pycl0nhx/field-regressions:	file format mach-o arm64

Disassembly of section __TEXT,__text:

00000001000e4d4c <__RNvNtNtNtNtCsagpXUhzPPpl_17field_regressions8campaign10candidates4grid4neon10accumulate>:
1000e4d4c: d342fc28    	lsr	x8, x1, #2
1000e4d50: d342fc69    	lsr	x9, x3, #2
1000e4d54: eb08013f    	cmp	x9, x8
1000e4d58: 9a883128    	csel	x8, x9, x8, lo
1000e4d5c: eb0800bf    	cmp	x5, x8
1000e4d60: 9a8830a8    	csel	x8, x5, x8, lo
1000e4d64: b4001968    	cbz	x8, 0x1000e5090 <__RNvNtNtNtNtCsagpXUhzPPpl_17field_regressions8campaign10candidates4grid4neon10accumulate+0x344>
1000e4d68: d10203ff    	sub	sp, sp, #0x80
1000e4d6c: 6d043bef    	stp	d15, d14, [sp, #0x40]
1000e4d70: 6d0533ed    	stp	d13, d12, [sp, #0x50]
1000e4d74: 6d062beb    	stp	d11, d10, [sp, #0x60]
1000e4d78: 6d0723e9    	stp	d9, d8, [sp, #0x70]
1000e4d7c: ad4058d7    	ldp	q23, q22, [x6]
1000e4d80: ad4150d5    	ldp	q21, q20, [x6, #0x20]
1000e4d84: ad4248d3    	ldp	q19, q18, [x6, #0x40]
1000e4d88: ad4340d1    	ldp	q17, q16, [x6, #0x60]
1000e4d8c: ad4418c7    	ldp	q7, q6, [x6, #0x80]
1000e4d90: ad4510c5    	ldp	q5, q4, [x6, #0xa0]
1000e4d94: ad4678c3    	ldp	q3, q30, [x6, #0xc0]
1000e4d98: ad4770dd    	ldp	q29, q28, [x6, #0xe0]
1000e4d9c: 91008049    	add	x9, x2, #0x20
1000e4da0: 9100208a    	add	x10, x4, #0x8
1000e4da4: 9100800b    	add	x11, x0, #0x20
1000e4da8: 528010ec    	mov	w12, #0x87              ; =135
1000e4dac: ad4860d9    	ldp	q25, q24, [x6, #0x100]
1000e4db0: 6f00e41a    	movi.2d	v26, #0000000000000000
1000e4db4: 4e080d80    	dup.2d	v0, x12
1000e4db8: 3d8003e0    	str	q0, [sp]
1000e4dbc: ad00f7fe    	stp	q30, q29, [sp, #0x10]
1000e4dc0: 3d800ffc    	str	q28, [sp, #0x30]
1000e4dc4: fd40015c    	ldr	d28, [x10]
1000e4dc8: 3dc003e0    	ldr	q0, [sp]
1000e4dcc: 0ee0e39d    	pmull.1q	v29, v28, v0
1000e4dd0: 4e183fac    	mov.d	x12, v29[1]
1000e4dd4: f85f814d    	ldur	x13, [x10, #-0x8]
1000e4dd8: ca0d018c    	eor	x12, x12, x13
1000e4ddc: 9e67019e    	fmov	d30, x12
1000e4de0: 6d407d68    	ldp	d8, d31, [x11]
1000e4de4: 0efee3e9    	pmull.1q	v9, v31, v30
1000e4de8: 0efce10a    	pmull.1q	v10, v8, v28
1000e4dec: 6e291d49    	eor.16b	v9, v10, v9
1000e4df0: 6e09434a    	ext.16b	v10, v26, v9, #0x8
1000e4df4: 0efde3ff    	pmull.1q	v31, v31, v29
1000e4df8: 9e6701ab    	fmov	d11, x13
1000e4dfc: 0eebe108    	pmull.1q	v8, v8, v11
1000e4e00: 6d7f316d    	ldp	d13, d12, [x11, #-0x10]
1000e4e04: 4ee0e129    	pmull2.1q	v9, v9, v0
1000e4e08: 0efee18e    	pmull.1q	v14, v12, v30
1000e4e0c: 0efce1af    	pmull.1q	v15, v13, v28
1000e4e10: 6e2e1dee    	eor.16b	v14, v15, v14
1000e4e14: 6e0e434f    	ext.16b	v15, v26, v14, #0x8
1000e4e18: 0efde18c    	pmull.1q	v12, v12, v29
1000e4e1c: ce1f2508    	eor3.16b	v8, v8, v31, v9
1000e4e20: 0eebe1bf    	pmull.1q	v31, v13, v11
1000e4e24: 4ee0e1c9    	pmull2.1q	v9, v14, v0
1000e4e28: 6d7e356e    	ldp	d14, d13, [x11, #-0x20]
1000e4e2c: 0efee1bb    	pmull.1q	v27, v13, v30
1000e4e30: ce0c27ec    	eor3.16b	v12, v31, v12, v9
1000e4e34: 0efce1df    	pmull.1q	v31, v14, v28
1000e4e38: 6e3b1ffb    	eor.16b	v27, v31, v27
1000e4e3c: 6e1b435f    	ext.16b	v31, v26, v27, #0x8
1000e4e40: 0efde1a9    	pmull.1q	v9, v13, v29
1000e4e44: 0eebe1cd    	pmull.1q	v13, v14, v11
1000e4e48: 6e281d48    	eor.16b	v8, v10, v8
1000e4e4c: 4ee0e37b    	pmull2.1q	v27, v27, v0
1000e4e50: ce096da9    	eor3.16b	v9, v13, v9, v27
1000e4e54: 6d41357b    	ldp	d27, d13, [x11, #0x10]
1000e4e58: 0eebe36e    	pmull.1q	v14, v27, v11
1000e4e5c: 6e2c1dea    	eor.16b	v10, v15, v12
1000e4e60: 0efde1bd    	pmull.1q	v29, v13, v29
1000e4e64: 0efce37b    	pmull.1q	v27, v27, v28
1000e4e68: 0efee1bc    	pmull.1q	v28, v13, v30
1000e4e6c: 6e3b1f8b    	eor.16b	v11, v28, v27
1000e4e70: 4ee0e17b    	pmull2.1q	v27, v11, v0
1000e4e74: 6e291fef    	eor.16b	v15, v31, v9
1000e4e78: ce0e6fac    	eor3.16b	v12, v29, v14, v27
1000e4e7c: ce092bed    	eor3.16b	v13, v31, v9, v10
1000e4e80: ad7f713e    	ldp	q30, q28, [x9, #-0x20]
1000e4e84: 6e3e1f9d    	eor.16b	v29, v28, v30
1000e4e88: 0efee1fb    	pmull.1q	v27, v15, v30
1000e4e8c: 4efee1ee    	pmull2.1q	v14, v15, v30
1000e4e90: 4e0805e0    	dup.2d	v0, v15[0]
1000e4e94: 4e0807c1    	dup.2d	v1, v30[0]
1000e4e98: 4efee000    	pmull2.1q	v0, v0, v30
1000e4e9c: 4ee1e1e1    	pmull2.1q	v1, v15, v1
1000e4ea0: 6e201c20    	eor.16b	v0, v1, v0
1000e4ea4: 6e004341    	ext.16b	v1, v26, v0, #0x8
1000e4ea8: 6e1a4000    	ext.16b	v0, v0, v26, #0x8
1000e4eac: 0efce14f    	pmull.1q	v15, v10, v28
1000e4eb0: ce1b06f7    	eor3.16b	v23, v23, v27, v1
1000e4eb4: 4efce141    	pmull2.1q	v1, v10, v28
1000e4eb8: 4e08055b    	dup.2d	v27, v10[0]
1000e4ebc: 4e080782    	dup.2d	v2, v28[0]
1000e4ec0: ce0e5816    	eor3.16b	v22, v0, v14, v22
1000e4ec4: 4efce360    	pmull2.1q	v0, v27, v28
1000e4ec8: 4ee2e142    	pmull2.1q	v2, v10, v2
1000e4ecc: 6e201c40    	eor.16b	v0, v2, v0
1000e4ed0: 6e004342    	ext.16b	v2, v26, v0, #0x8
1000e4ed4: 6e1a4000    	ext.16b	v0, v0, v26, #0x8
1000e4ed8: ce0f0ab5    	eor3.16b	v21, v21, v15, v2
1000e4edc: 0efde1a2    	pmull.1q	v2, v13, v29
1000e4ee0: 4e0805bb    	dup.2d	v27, v13[0]
1000e4ee4: 4e0807ae    	dup.2d	v14, v29[0]
1000e4ee8: ce015014    	eor3.16b	v20, v0, v1, v20
1000e4eec: 4efde360    	pmull2.1q	v0, v27, v29
1000e4ef0: 4eeee1a1    	pmull2.1q	v1, v13, v14
1000e4ef4: 6e201c20    	eor.16b	v0, v1, v0
1000e4ef8: 6e004341    	ext.16b	v1, v26, v0, #0x8
1000e4efc: acc23d2e    	ldp	q14, q15, [x9], #0x40
1000e4f00: ce020673    	eor3.16b	v19, v19, v2, v1
1000e4f04: 0eeee101    	pmull.1q	v1, v8, v14
1000e4f08: 4e0805c2    	dup.2d	v2, v14[0]
1000e4f0c: 4e08051b    	dup.2d	v27, v8[0]
1000e4f10: 4eeee37b    	pmull2.1q	v27, v27, v14
1000e4f14: 4ee2e102    	pmull2.1q	v2, v8, v2
1000e4f18: 6e3b1c42    	eor.16b	v2, v2, v27
1000e4f1c: 6e02435b    	ext.16b	v27, v26, v2, #0x8
1000e4f20: ce016e31    	eor3.16b	v17, v17, v1, v27
1000e4f24: 4eeee101    	pmull2.1q	v1, v8, v14
1000e4f28: 6e1a4042    	ext.16b	v2, v2, v26, #0x8
1000e4f2c: ce014050    	eor3.16b	v16, v2, v1, v16
1000e4f30: 6e0b4341    	ext.16b	v1, v26, v11, #0x8
1000e4f34: 6e2c1c22    	eor.16b	v2, v1, v12
1000e4f38: ce0c282a    	eor3.16b	v10, v1, v12, v10
1000e4f3c: ce0c2021    	eor3.16b	v1, v1, v12, v8
1000e4f40: ce0923fb    	eor3.16b	v27, v31, v9, v8
1000e4f44: ce08345f    	eor3.16b	v31, v2, v8, v13
1000e4f48: 4efde1a8    	pmull2.1q	v8, v13, v29
1000e4f4c: 6e1a4000    	ext.16b	v0, v0, v26, #0x8
1000e4f50: 0eefe049    	pmull.1q	v9, v2, v15
1000e4f54: ce084812    	eor3.16b	v18, v0, v8, v18
1000e4f58: 4e080440    	dup.2d	v0, v2[0]
1000e4f5c: 4eefe000    	pmull2.1q	v0, v0, v15
1000e4f60: 4e0805e8    	dup.2d	v8, v15[0]
1000e4f64: 4ee8e048    	pmull2.1q	v8, v2, v8
1000e4f68: 6e201d00    	eor.16b	v0, v8, v0
1000e4f6c: 6e004348    	ext.16b	v8, v26, v0, #0x8
1000e4f70: ce0920e7    	eor3.16b	v7, v7, v9, v8
1000e4f74: 6e2e1de8    	eor.16b	v8, v15, v14
1000e4f78: 4eefe042    	pmull2.1q	v2, v2, v15
1000e4f7c: 6e1a4000    	ext.16b	v0, v0, v26, #0x8
1000e4f80: ce021806    	eor3.16b	v6, v0, v2, v6
1000e4f84: 4e080420    	dup.2d	v0, v1[0]
1000e4f88: 4e080502    	dup.2d	v2, v8[0]
1000e4f8c: 4ee8e000    	pmull2.1q	v0, v0, v8
1000e4f90: 4ee2e022    	pmull2.1q	v2, v1, v2
1000e4f94: 6e201c40    	eor.16b	v0, v2, v0
1000e4f98: 6e004342    	ext.16b	v2, v26, v0, #0x8
1000e4f9c: 0ee8e029    	pmull.1q	v9, v1, v8
1000e4fa0: ce0908a5    	eor3.16b	v5, v5, v9, v2
1000e4fa4: 6e3e1dc2    	eor.16b	v2, v14, v30
1000e4fa8: 4ee8e021    	pmull2.1q	v1, v1, v8
1000e4fac: 6e1a4000    	ext.16b	v0, v0, v26, #0x8
1000e4fb0: 4e08077e    	dup.2d	v30, v27[0]
1000e4fb4: 4ee2e3de    	pmull2.1q	v30, v30, v2
1000e4fb8: ce011004    	eor3.16b	v4, v0, v1, v4
1000e4fbc: 4e080440    	dup.2d	v0, v2[0]
1000e4fc0: 4ee0e360    	pmull2.1q	v0, v27, v0
1000e4fc4: 6e3e1c00    	eor.16b	v0, v0, v30
1000e4fc8: 0ee2e361    	pmull.1q	v1, v27, v2
1000e4fcc: 6e00435e    	ext.16b	v30, v26, v0, #0x8
1000e4fd0: ce017863    	eor3.16b	v3, v3, v1, v30
1000e4fd4: ce0e75e1    	eor3.16b	v1, v15, v14, v29
1000e4fd8: ad40f7fe    	ldp	q30, q29, [sp, #0x10]
1000e4fdc: 6e3c1dfc    	eor.16b	v28, v15, v28
1000e4fe0: 4ee2e362    	pmull2.1q	v2, v27, v2
1000e4fe4: 6e1a4000    	ext.16b	v0, v0, v26, #0x8
1000e4fe8: ce02781e    	eor3.16b	v30, v0, v2, v30
1000e4fec: 4e080540    	dup.2d	v0, v10[0]
1000e4ff0: 4efce000    	pmull2.1q	v0, v0, v28
1000e4ff4: 4e080782    	dup.2d	v2, v28[0]
1000e4ff8: 4ee2e142    	pmull2.1q	v2, v10, v2
1000e4ffc: 6e201c40    	eor.16b	v0, v2, v0
1000e5000: 0efce142    	pmull.1q	v2, v10, v28
1000e5004: 6e00435b    	ext.16b	v27, v26, v0, #0x8
1000e5008: ce026fbd    	eor3.16b	v29, v29, v2, v27
1000e500c: 4efce142    	pmull2.1q	v2, v10, v28
1000e5010: 3dc00ffc    	ldr	q28, [sp, #0x30]
1000e5014: 6e1a4000    	ext.16b	v0, v0, v26, #0x8
1000e5018: ce02701c    	eor3.16b	v28, v0, v2, v28
1000e501c: 4e0807e0    	dup.2d	v0, v31[0]
1000e5020: 4e080422    	dup.2d	v2, v1[0]
1000e5024: 4ee1e000    	pmull2.1q	v0, v0, v1
1000e5028: 4ee2e3e2    	pmull2.1q	v2, v31, v2
1000e502c: 6e201c40    	eor.16b	v0, v2, v0
1000e5030: 6e004342    	ext.16b	v2, v26, v0, #0x8
1000e5034: 0ee1e3fb    	pmull.1q	v27, v31, v1
1000e5038: ce1b0b39    	eor3.16b	v25, v25, v27, v2
1000e503c: 4ee1e3e1    	pmull2.1q	v1, v31, v1
1000e5040: 6e1a4000    	ext.16b	v0, v0, v26, #0x8
1000e5044: ce016018    	eor3.16b	v24, v0, v1, v24
1000e5048: 9100414a    	add	x10, x10, #0x10
1000e504c: 9101016b    	add	x11, x11, #0x40
1000e5050: f1000508    	subs	x8, x8, #0x1
1000e5054: 54ffeb41    	b.ne	0x1000e4dbc <__RNvNtNtNtNtCsagpXUhzPPpl_17field_regressions8campaign10candidates4grid4neon10accumulate+0x70>
1000e5058: ad0058d7    	stp	q23, q22, [x6]
1000e505c: ad0150d5    	stp	q21, q20, [x6, #0x20]
1000e5060: ad0248d3    	stp	q19, q18, [x6, #0x40]
1000e5064: ad0340d1    	stp	q17, q16, [x6, #0x60]
1000e5068: ad0418c7    	stp	q7, q6, [x6, #0x80]
1000e506c: ad0510c5    	stp	q5, q4, [x6, #0xa0]
1000e5070: ad0678c3    	stp	q3, q30, [x6, #0xc0]
1000e5074: ad0770dd    	stp	q29, q28, [x6, #0xe0]
1000e5078: ad0860d9    	stp	q25, q24, [x6, #0x100]
1000e507c: 6d4723e9    	ldp	d9, d8, [sp, #0x70]
1000e5080: 6d462beb    	ldp	d11, d10, [sp, #0x60]
1000e5084: 6d4533ed    	ldp	d13, d12, [sp, #0x50]
1000e5088: 6d443bef    	ldp	d15, d14, [sp, #0x40]
1000e508c: 910203ff    	add	sp, sp, #0x80
1000e5090: d65f03c0    	ret

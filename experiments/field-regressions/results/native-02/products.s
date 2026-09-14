
/var/folders/q0/v7l8dzd13k7f_q6j80sy03lm0000gn/T/f2z-field-regressions-emqdbh47/target/release/field-regressions:	file format mach-o arm64

Disassembly of section __TEXT,__text:

000000010001e9e0 <<field_regressions::arithmetic::Kernel>::products>:
10001e9e0: d100c3ff    	sub	sp, sp, #0x30
10001e9e4: a9027bfd    	stp	x29, x30, [sp, #0x20]
10001e9e8: 910083fd    	add	x29, sp, #0x20
10001e9ec: f90007e2    	str	x2, [sp, #0x8]
10001e9f0: f81f83a4    	stur	x4, [x29, #-0x8]
10001e9f4: eb04005f    	cmp	x2, x4
10001e9f8: 54001a01    	b.ne	0x10001ed38 <<field_regressions::arithmetic::Kernel>::products+0x358>
10001e9fc: f9000be2    	str	x2, [sp, #0x10]
10001ea00: f81f83a6    	stur	x6, [x29, #-0x8]
10001ea04: eb06005f    	cmp	x2, x6
10001ea08: 54001a41    	b.ne	0x10001ed50 <<field_regressions::arithmetic::Kernel>::products+0x370>
10001ea0c: 12001c08    	and	w8, w0, #0xff
10001ea10: 7100051f    	cmp	w8, #0x1
10001ea14: 5400076d    	b.le	0x10001eb00 <<field_regressions::arithmetic::Kernel>::products+0x120>
10001ea18: 7100091f    	cmp	w8, #0x2
10001ea1c: 54000a80    	b.eq	0x10001eb6c <<field_regressions::arithmetic::Kernel>::products+0x18c>
10001ea20: 71000d1f    	cmp	w8, #0x3
10001ea24: 54000fc1    	b.ne	0x10001ec1c <<field_regressions::arithmetic::Kernel>::products+0x23c>
10001ea28: b4001822    	cbz	x2, 0x10001ed2c <<field_regressions::arithmetic::Kernel>::products+0x34c>
10001ea2c: 910020a8    	add	x8, x5, #0x8
10001ea30: 91002069    	add	x9, x3, #0x8
10001ea34: 9100202a    	add	x10, x1, #0x8
10001ea38: a97fb12b    	ldp	x11, x12, [x9, #-0x8]
10001ea3c: ca0b018d    	eor	x13, x12, x11
10001ea40: 9e670160    	fmov	d0, x11
10001ea44: a97fb94b    	ldp	x11, x14, [x10, #-0x8]
10001ea48: ca0b01cf    	eor	x15, x14, x11
10001ea4c: 9e670161    	fmov	d1, x11
10001ea50: 0ee0e020    	pmull.1q	v0, v1, v0
10001ea54: 9e670181    	fmov	d1, x12
10001ea58: 9e6701c2    	fmov	d2, x14
10001ea5c: 0ee1e041    	pmull.1q	v1, v2, v1
10001ea60: 9e6701a2    	fmov	d2, x13
10001ea64: 9e6701e3    	fmov	d3, x15
10001ea68: 0ee2e062    	pmull.1q	v2, v3, v2
10001ea6c: 4e183c0b    	mov.d	x11, v0[1]
10001ea70: ce020403    	eor3.16b	v3, v0, v2, v1
10001ea74: 9e66000c    	fmov	x12, d0
10001ea78: 4e183c2d    	mov.d	x13, v1[1]
10001ea7c: 9e66002e    	fmov	x14, d1
10001ea80: 4e183c6f    	mov.d	x15, v3[1]
10001ea84: ca0e01ef    	eor	x15, x15, x14
10001ea88: 93cffdb0    	extr	x16, x13, x15, #0x3f
10001ea8c: 9e660051    	fmov	x17, d2
10001ea90: 93cff9a0    	extr	x0, x13, x15, #0x3e
10001ea94: 93cfe5a1    	extr	x1, x13, x15, #0x39
10001ea98: d37ffda3    	lsr	x3, x13, #63
10001ea9c: ca4df863    	eor	x3, x3, x13, lsr #62
10001eaa0: ca4de463    	eor	x3, x3, x13, lsr #57
10001eaa4: d37ef464    	lsl	x4, x3, #2
10001eaa8: ca030484    	eor	x4, x4, x3, lsl #1
10001eaac: ca031c84    	eor	x4, x4, x3, lsl #7
10001eab0: ca0f0484    	eor	x4, x4, x15, lsl #1
10001eab4: ca0f0884    	eor	x4, x4, x15, lsl #2
10001eab8: ca0f1c84    	eor	x4, x4, x15, lsl #7
10001eabc: ca030183    	eor	x3, x12, x3
10001eac0: ca0f006f    	eor	x15, x3, x15
10001eac4: ca0f008f    	eor	x15, x4, x15
10001eac8: ca000231    	eor	x17, x17, x0
10001eacc: ca010210    	eor	x16, x16, x1
10001ead0: ca100230    	eor	x16, x17, x16
10001ead4: ca0e016b    	eor	x11, x11, x14
10001ead8: ca0c016b    	eor	x11, x11, x12
10001eadc: ca0d016b    	eor	x11, x11, x13
10001eae0: ca0b020b    	eor	x11, x16, x11
10001eae4: a93fad0f    	stp	x15, x11, [x8, #-0x8]
10001eae8: 91004108    	add	x8, x8, #0x10
10001eaec: 91004129    	add	x9, x9, #0x10
10001eaf0: 9100414a    	add	x10, x10, #0x10
10001eaf4: f1000442    	subs	x2, x2, #0x1
10001eaf8: 54fffa01    	b.ne	0x10001ea38 <<field_regressions::arithmetic::Kernel>::products+0x58>
10001eafc: 1400008c    	b	0x10001ed2c <<field_regressions::arithmetic::Kernel>::products+0x34c>
10001eb00: 35000e48    	cbnz	w8, 0x10001ecc8 <<field_regressions::arithmetic::Kernel>::products+0x2e8>
10001eb04: b4001142    	cbz	x2, 0x10001ed2c <<field_regressions::arithmetic::Kernel>::products+0x34c>
10001eb08: 91002068    	add	x8, x3, #0x8
10001eb0c: 91002029    	add	x9, x1, #0x8
10001eb10: 6f00e400    	movi.2d	v0, #0000000000000000
10001eb14: 528010ea    	mov	w10, #0x87              ; =135
10001eb18: 4e080d41    	dup.2d	v1, x10
10001eb1c: 6d7f8d02    	ldp	d2, d3, [x8, #-0x8]
10001eb20: 6d7f9524    	ldp	d4, d5, [x9, #-0x8]
10001eb24: 0ee3e086    	pmull.1q	v6, v4, v3
10001eb28: 0ee3e0a3    	pmull.1q	v3, v5, v3
10001eb2c: 0ee2e0a5    	pmull.1q	v5, v5, v2
10001eb30: 6e261ca5    	eor.16b	v5, v5, v6
10001eb34: 6e034006    	ext.16b	v6, v0, v3, #0x8
10001eb38: 4ee1e063    	pmull2.1q	v3, v3, v1
10001eb3c: ce050cc3    	eor3.16b	v3, v6, v5, v3
10001eb40: 6e034005    	ext.16b	v5, v0, v3, #0x8
10001eb44: 0ee2e082    	pmull.1q	v2, v4, v2
10001eb48: 6e221ca2    	eor.16b	v2, v5, v2
10001eb4c: 4ee1e063    	pmull2.1q	v3, v3, v1
10001eb50: 6e231c42    	eor.16b	v2, v2, v3
10001eb54: 3c8104a2    	str	q2, [x5], #0x10
10001eb58: 91004108    	add	x8, x8, #0x10
10001eb5c: 91004129    	add	x9, x9, #0x10
10001eb60: f1000442    	subs	x2, x2, #0x1
10001eb64: 54fffdc1    	b.ne	0x10001eb1c <<field_regressions::arithmetic::Kernel>::products+0x13c>
10001eb68: 14000071    	b	0x10001ed2c <<field_regressions::arithmetic::Kernel>::products+0x34c>
10001eb6c: b4000e02    	cbz	x2, 0x10001ed2c <<field_regressions::arithmetic::Kernel>::products+0x34c>
10001eb70: 910020a8    	add	x8, x5, #0x8
10001eb74: 91002069    	add	x9, x3, #0x8
10001eb78: 9100202a    	add	x10, x1, #0x8
10001eb7c: 6d7f8520    	ldp	d0, d1, [x9, #-0x8]
10001eb80: 6d7f8d42    	ldp	d2, d3, [x10, #-0x8]
10001eb84: 0ee1e044    	pmull.1q	v4, v2, v1
10001eb88: 0ee0e065    	pmull.1q	v5, v3, v0
10001eb8c: 6e241ca4    	eor.16b	v4, v5, v4
10001eb90: 4e180485    	dup.2d	v5, v4[1]
10001eb94: 0ee1e061    	pmull.1q	v1, v3, v1
10001eb98: 0ee0e040    	pmull.1q	v0, v2, v0
10001eb9c: 6e211ca2    	eor.16b	v2, v5, v1
10001eba0: 4e183c2b    	mov.d	x11, v1[1]
10001eba4: 4e180401    	dup.2d	v1, v0[1]
10001eba8: 9e66000c    	fmov	x12, d0
10001ebac: 6e241c20    	eor.16b	v0, v1, v4
10001ebb0: 9e66004d    	fmov	x13, d2
10001ebb4: 93cdfd6e    	extr	x14, x11, x13, #0x3f
10001ebb8: 93cdf96f    	extr	x15, x11, x13, #0x3e
10001ebbc: 9e660010    	fmov	x16, d0
10001ebc0: 93cde571    	extr	x17, x11, x13, #0x39
10001ebc4: d37ffd60    	lsr	x0, x11, #63
10001ebc8: ca4bf800    	eor	x0, x0, x11, lsr #62
10001ebcc: ca4be400    	eor	x0, x0, x11, lsr #57
10001ebd0: ca0d098c    	eor	x12, x12, x13, lsl #2
10001ebd4: ca0d058c    	eor	x12, x12, x13, lsl #1
10001ebd8: ca0d1d8c    	eor	x12, x12, x13, lsl #7
10001ebdc: ca00098c    	eor	x12, x12, x0, lsl #2
10001ebe0: ca00058c    	eor	x12, x12, x0, lsl #1
10001ebe4: ca001d8c    	eor	x12, x12, x0, lsl #7
10001ebe8: ca0001ad    	eor	x13, x13, x0
10001ebec: ca0f020f    	eor	x15, x16, x15
10001ebf0: ca1101ce    	eor	x14, x14, x17
10001ebf4: ca0e01ee    	eor	x14, x15, x14
10001ebf8: ca0b01cb    	eor	x11, x14, x11
10001ebfc: ca0d018c    	eor	x12, x12, x13
10001ec00: a93fad0c    	stp	x12, x11, [x8, #-0x8]
10001ec04: 91004108    	add	x8, x8, #0x10
10001ec08: 91004129    	add	x9, x9, #0x10
10001ec0c: 9100414a    	add	x10, x10, #0x10
10001ec10: f1000442    	subs	x2, x2, #0x1
10001ec14: 54fffb41    	b.ne	0x10001eb7c <<field_regressions::arithmetic::Kernel>::products+0x19c>
10001ec18: 14000045    	b	0x10001ed2c <<field_regressions::arithmetic::Kernel>::products+0x34c>
10001ec1c: b4000882    	cbz	x2, 0x10001ed2c <<field_regressions::arithmetic::Kernel>::products+0x34c>
10001ec20: 91002068    	add	x8, x3, #0x8
10001ec24: 91002029    	add	x9, x1, #0x8
10001ec28: 528010ea    	mov	w10, #0x87              ; =135
10001ec2c: 4e080d40    	dup.2d	v0, x10
10001ec30: 9e670141    	fmov	d1, x10
10001ec34: a97fad0a    	ldp	x10, x11, [x8, #-0x8]
10001ec38: ca0a016c    	eor	x12, x11, x10
10001ec3c: 9e670142    	fmov	d2, x10
10001ec40: a97fb52a    	ldp	x10, x13, [x9, #-0x8]
10001ec44: ca0a01ae    	eor	x14, x13, x10
10001ec48: 9e670143    	fmov	d3, x10
10001ec4c: 0ee2e062    	pmull.1q	v2, v3, v2
10001ec50: 9e670163    	fmov	d3, x11
10001ec54: 9e6701a4    	fmov	d4, x13
10001ec58: 9e670185    	fmov	d5, x12
10001ec5c: 9e6701c6    	fmov	d6, x14
10001ec60: 0ee3e083    	pmull.1q	v3, v4, v3
10001ec64: 0ee5e0c4    	pmull.1q	v4, v6, v5
10001ec68: ce040c44    	eor3.16b	v4, v2, v4, v3
10001ec6c: 4e180445    	dup.2d	v5, v2[1]
10001ec70: 4e180486    	dup.2d	v6, v4[1]
10001ec74: 4ee0e067    	pmull2.1q	v7, v3, v0
10001ec78: 6e271ca5    	eor.16b	v5, v5, v7
10001ec7c: 4e183cea    	mov.d	x10, v7[1]
10001ec80: 2e231cc3    	eor.8b	v3, v6, v3
10001ec84: 0ee1e063    	pmull.1q	v3, v3, v1
10001ec88: 6e231c42    	eor.16b	v2, v2, v3
10001ec8c: 9e66004b    	fmov	x11, d2
10001ec90: ca0a096b    	eor	x11, x11, x10, lsl #2
10001ec94: 4f4754e2    	shl.2d	v2, v7, #0x7
10001ec98: ca0a056a    	eor	x10, x11, x10, lsl #1
10001ec9c: 6e054042    	ext.16b	v2, v2, v5, #0x8
10001eca0: 4e080484    	dup.2d	v4, v4[0]
10001eca4: 4e081d44    	mov.d	v4[0], x10
10001eca8: 4ec378e3    	zip2.2d	v3, v7, v3
10001ecac: ce040c42    	eor3.16b	v2, v2, v4, v3
10001ecb0: 3c8104a2    	str	q2, [x5], #0x10
10001ecb4: 91004108    	add	x8, x8, #0x10
10001ecb8: 91004129    	add	x9, x9, #0x10
10001ecbc: f1000442    	subs	x2, x2, #0x1
10001ecc0: 54fffba1    	b.ne	0x10001ec34 <<field_regressions::arithmetic::Kernel>::products+0x254>
10001ecc4: 1400001a    	b	0x10001ed2c <<field_regressions::arithmetic::Kernel>::products+0x34c>
10001ecc8: b4000322    	cbz	x2, 0x10001ed2c <<field_regressions::arithmetic::Kernel>::products+0x34c>
10001eccc: 91002068    	add	x8, x3, #0x8
10001ecd0: 91002029    	add	x9, x1, #0x8
10001ecd4: 6f00e400    	movi.2d	v0, #0000000000000000
10001ecd8: 528010ea    	mov	w10, #0x87              ; =135
10001ecdc: 4e080d41    	dup.2d	v1, x10
10001ece0: 6d7f8d02    	ldp	d2, d3, [x8, #-0x8]
10001ece4: 6d7f9524    	ldp	d4, d5, [x9, #-0x8]
10001ece8: 0ee3e0a6    	pmull.1q	v6, v5, v3
10001ecec: 0ee3e083    	pmull.1q	v3, v4, v3
10001ecf0: 0ee2e0a5    	pmull.1q	v5, v5, v2
10001ecf4: 6e231ca3    	eor.16b	v3, v5, v3
10001ecf8: 6e064005    	ext.16b	v5, v0, v6, #0x8
10001ecfc: 4ee1e0c6    	pmull2.1q	v6, v6, v1
10001ed00: ce0318a3    	eor3.16b	v3, v5, v3, v6
10001ed04: 6e034005    	ext.16b	v5, v0, v3, #0x8
10001ed08: 0ee2e082    	pmull.1q	v2, v4, v2
10001ed0c: 6e221ca2    	eor.16b	v2, v5, v2
10001ed10: 4ee1e063    	pmull2.1q	v3, v3, v1
10001ed14: 6e231c42    	eor.16b	v2, v2, v3
10001ed18: 3c8104a2    	str	q2, [x5], #0x10
10001ed1c: 91004108    	add	x8, x8, #0x10
10001ed20: 91004129    	add	x9, x9, #0x10
10001ed24: f1000442    	subs	x2, x2, #0x1
10001ed28: 54fffdc1    	b.ne	0x10001ece0 <<field_regressions::arithmetic::Kernel>::products+0x300>
10001ed2c: a9427bfd    	ldp	x29, x30, [sp, #0x20]
10001ed30: 9100c3ff    	add	sp, sp, #0x30
10001ed34: d65f03c0    	ret
10001ed38: d0000423    	adrp	x3, 0x1000a4000 <dyld_stub_binder+0x1000a4000>
10001ed3c: 91244063    	add	x3, x3, #0x910
10001ed40: 910023e0    	add	x0, sp, #0x8
10001ed44: d10023a1    	sub	x1, x29, #0x8
10001ed48: d2800002    	mov	x2, #0x0                ; =0
10001ed4c: 94017c77    	bl	0x10007df28 <core::panicking::assert_failed::<usize, usize>>
10001ed50: d0000423    	adrp	x3, 0x1000a4000 <dyld_stub_binder+0x1000a4000>
10001ed54: 9124a063    	add	x3, x3, #0x928
10001ed58: 910043e0    	add	x0, sp, #0x10
10001ed5c: d10023a1    	sub	x1, x29, #0x8
10001ed60: d2800002    	mov	x2, #0x0                ; =0
10001ed64: 94017c71    	bl	0x10007df28 <core::panicking::assert_failed::<usize, usize>>

000000010001ed68 <<std::io::buffered::bufwriter::BufWriter<std::fs::File>>::flush_buf>:
10001ed68: a9ba6ffc    	stp	x28, x27, [sp, #-0x60]!
10001ed6c: a90167fa    	stp	x26, x25, [sp, #0x10]
10001ed70: a9025ff8    	stp	x24, x23, [sp, #0x20]
10001ed74: a90357f6    	stp	x22, x21, [sp, #0x30]
10001ed78: a9044ff4    	stp	x20, x19, [sp, #0x40]
10001ed7c: a9057bfd    	stp	x29, x30, [sp, #0x50]
10001ed80: 910143fd    	add	x29, sp, #0x50
10001ed84: f9400815    	ldr	x21, [x0, #0x10]
10001ed88: b4000535    	cbz	x21, 0x10001ee2c <<std::io::buffered::bufwriter::BufWriter<std::fs::File>>::flush_buf+0xc4>
10001ed8c: aa0003f3    	mov	x19, x0
10001ed90: d2800014    	mov	x20, #0x0               ; =0
10001ed94: 52800039    	mov	w25, #0x1               ; =1
10001ed98: 12b0001a    	mov	w26, #0x7fffffff        ; =2147483647
10001ed9c: d000047b    	adrp	x27, 0x1000ac000 <dyld_stub_binder+0x1000ac000>
10001eda0: 9000045c    	adrp	x28, 0x1000a6000 <dyld_stub_binder+0x1000a6000>
10001eda4: 9130239c    	add	x28, x28, #0xc08
10001eda8: b0000456    	adrp	x22, 0x1000a7000 <dyld_stub_binder+0x1000a7000>
10001edac: 910742d6    	add	x22, x22, #0x1d0
10001edb0: 14000006    	b	0x10001edc8 <<std::io::buffered::bufwriter::BufWriter<std::fs::File>>::flush_buf+0x60>
10001edb4: 3900627f    	strb	wzr, [x19, #0x18]
10001edb8: b4000400    	cbz	x0, 0x10001ee38 <<std::io::buffered::bufwriter::BufWriter<std::fs::File>>::flush_buf+0xd0>
10001edbc: 8b000294    	add	x20, x20, x0
10001edc0: eb15029f    	cmp	x20, x21
10001edc4: 54000382    	b.hs	0x10001ee34 <<std::io::buffered::bufwriter::BufWriter<std::fs::File>>::flush_buf+0xcc>
10001edc8: 39006279    	strb	w25, [x19, #0x18]
10001edcc: eb1402a8    	subs	x8, x21, x20
10001edd0: 540005c3    	b.lo	0x10001ee88 <<std::io::buffered::bufwriter::BufWriter<std::fs::File>>::flush_buf+0x120>
10001edd4: f9400677    	ldr	x23, [x19, #0x8]
10001edd8: b9401e60    	ldr	w0, [x19, #0x1c]
10001eddc: eb1a011f    	cmp	x8, x26
10001ede0: 9a9a3102    	csel	x2, x8, x26, lo
10001ede4: 8b1402e1    	add	x1, x23, x20
10001ede8: 9401983b    	bl	0x100084ed4 <dyld_stub_binder+0x100084ed4>
10001edec: b100041f    	cmn	x0, #0x1
10001edf0: 54fffe21    	b.ne	0x10001edb4 <<std::io::buffered::bufwriter::BufWriter<std::fs::File>>::flush_buf+0x4c>
10001edf4: 9401976f    	bl	0x100084bb0 <dyld_stub_binder+0x100084bb0>
10001edf8: b9800018    	ldrsw	x24, [x0]
10001edfc: f9463f68    	ldr	x8, [x27, #0xc78]
10001ee00: eb1c011f    	cmp	x8, x28
10001ee04: 54000101    	b.ne	0x10001ee24 <<std::io::buffered::bufwriter::BufWriter<std::fs::File>>::flush_buf+0xbc>
10001ee08: 3900627f    	strb	wzr, [x19, #0x18]
10001ee0c: f9463f68    	ldr	x8, [x27, #0xc78]
10001ee10: f9400908    	ldr	x8, [x8, #0x10]
10001ee14: aa1803e0    	mov	x0, x24
10001ee18: d63f0100    	blr	x8
10001ee1c: 3707fd20    	tbnz	w0, #0x0, 0x10001edc0 <<std::io::buffered::bufwriter::BufWriter<std::fs::File>>::flush_buf+0x58>
10001ee20: 14000016    	b	0x10001ee78 <<std::io::buffered::bufwriter::BufWriter<std::fs::File>>::flush_buf+0x110>
10001ee24: f9063f7c    	str	x28, [x27, #0xc78]
10001ee28: 17fffff8    	b	0x10001ee08 <<std::io::buffered::bufwriter::BufWriter<std::fs::File>>::flush_buf+0xa0>
10001ee2c: d2800016    	mov	x22, #0x0               ; =0
10001ee30: 1400000a    	b	0x10001ee58 <<std::io::buffered::bufwriter::BufWriter<std::fs::File>>::flush_buf+0xf0>
10001ee34: d2800016    	mov	x22, #0x0               ; =0
10001ee38: b4000114    	cbz	x20, 0x10001ee58 <<std::io::buffered::bufwriter::BufWriter<std::fs::File>>::flush_buf+0xf0>
10001ee3c: eb1402b8    	subs	x24, x21, x20
10001ee40: 54000323    	b.lo	0x10001eea4 <<std::io::buffered::bufwriter::BufWriter<std::fs::File>>::flush_buf+0x13c>
10001ee44: 8b1402e1    	add	x1, x23, x20
10001ee48: aa1703e0    	mov	x0, x23
10001ee4c: aa1803e2    	mov	x2, x24
10001ee50: 940197a6    	bl	0x100084ce8 <dyld_stub_binder+0x100084ce8>
10001ee54: f9000a78    	str	x24, [x19, #0x10]
10001ee58: aa1603e0    	mov	x0, x22
10001ee5c: a9457bfd    	ldp	x29, x30, [sp, #0x50]
10001ee60: a9444ff4    	ldp	x20, x19, [sp, #0x40]
10001ee64: a94357f6    	ldp	x22, x21, [sp, #0x30]
10001ee68: a9425ff8    	ldp	x24, x23, [sp, #0x20]
10001ee6c: a94167fa    	ldp	x26, x25, [sp, #0x10]
10001ee70: a8c66ffc    	ldp	x28, x27, [sp], #0x60
10001ee74: d65f03c0    	ret
10001ee78: 52800048    	mov	w8, #0x2                ; =2
10001ee7c: aa188116    	orr	x22, x8, x24, lsl #32
10001ee80: b5fffdf4    	cbnz	x20, 0x10001ee3c <<std::io::buffered::bufwriter::BufWriter<std::fs::File>>::flush_buf+0xd4>
10001ee84: 17fffff5    	b	0x10001ee58 <<std::io::buffered::bufwriter::BufWriter<std::fs::File>>::flush_buf+0xf0>
10001ee88: 90000443    	adrp	x3, 0x1000a6000 <dyld_stub_binder+0x1000a6000>
10001ee8c: 913cc063    	add	x3, x3, #0xf30
10001ee90: aa1403e0    	mov	x0, x20
10001ee94: aa1503e1    	mov	x1, x21
10001ee98: aa1503e2    	mov	x2, x21
10001ee9c: 94017c3d    	bl	0x10007df90 <core::slice::index::slice_index_fail>
10001eea0: d4200020    	brk	#0x1
10001eea4: b0000443    	adrp	x3, 0x1000a7000 <dyld_stub_binder+0x1000a7000>
10001eea8: 910ec063    	add	x3, x3, #0x3b0
10001eeac: aa1403e0    	mov	x0, x20
10001eeb0: aa1503e1    	mov	x1, x21
10001eeb4: aa1503e2    	mov	x2, x21
10001eeb8: 94017c36    	bl	0x10007df90 <core::slice::index::slice_index_fail>
10001eebc: 14000001    	b	0x10001eec0 <<std::io::buffered::bufwriter::BufWriter<std::fs::File>>::flush_buf+0x158>
10001eec0: aa0003f5    	mov	x21, x0
10001eec4: aa1303e0    	mov	x0, x19
10001eec8: aa1403e1    	mov	x1, x20
10001eecc: 940175e5    	bl	0x10007c660 <<<std::io::buffered::bufwriter::BufWriter<_>>::flush_buf::BufGuard as core::ops::drop::Drop>::drop>
10001eed0: aa1503e0    	mov	x0, x21
10001eed4: 9401972e    	bl	0x100084b8c <dyld_stub_binder+0x100084b8c>
10001eed8: 94017cb4    	bl	0x10007e1a8 <core::panicking::panic_in_cleanup>

000000010001eedc <<alloc::collections::btree::map::IntoIter<usize, num_bigint::bigint::BigInt>>::dying_next>:
10001eedc: a9bc5ff8    	stp	x24, x23, [sp, #-0x40]!
10001eee0: a90157f6    	stp	x22, x21, [sp, #0x10]
10001eee4: a9024ff4    	stp	x20, x19, [sp, #0x20]
10001eee8: a9037bfd    	stp	x29, x30, [sp, #0x30]
10001eeec: 9100c3fd    	add	x29, sp, #0x30
10001eef0: aa0003f3    	mov	x19, x0
10001eef4: f9402028    	ldr	x8, [x1, #0x40]
10001eef8: b40002e8    	cbz	x8, 0x10001ef54 <<alloc::collections::btree::map::IntoIter<usize, num_bigint::bigint::BigInt>>::dying_next+0x78>
10001eefc: d1000508    	sub	x8, x8, #0x1
10001ef00: f9002028    	str	x8, [x1, #0x40]
10001ef04: f9400028    	ldr	x8, [x1]
10001ef08: f100051f    	cmp	x8, #0x1
10001ef0c: 54000a41    	b.ne	0x10001f054 <<alloc::collections::btree::map::IntoIter<usize, num_bigint::bigint::BigInt>>::dying_next+0x178>
10001ef10: f9400420    	ldr	x0, [x1, #0x8]
10001ef14: b4000460    	cbz	x0, 0x10001efa0 <<alloc::collections::btree::map::IntoIter<usize, num_bigint::bigint::BigInt>>::dying_next+0xc4>
10001ef18: a9415434    	ldp	x20, x21, [x1, #0x10]
10001ef1c: 79438408    	ldrh	w8, [x0, #0x1c2]
10001ef20: eb0802bf    	cmp	x21, x8
10001ef24: 54000562    	b.hs	0x10001efd0 <<alloc::collections::btree::map::IntoIter<usize, num_bigint::bigint::BigInt>>::dying_next+0xf4>
10001ef28: aa0003f6    	mov	x22, x0
10001ef2c: b40006b4    	cbz	x20, 0x10001f000 <<alloc::collections::btree::map::IntoIter<usize, num_bigint::bigint::BigInt>>::dying_next+0x124>
10001ef30: 8b150ec8    	add	x8, x22, x21, lsl #3
10001ef34: 91074109    	add	x9, x8, #0x1d0
10001ef38: aa1403ea    	mov	x10, x20
10001ef3c: f9400128    	ldr	x8, [x9]
10001ef40: 91072109    	add	x9, x8, #0x1c8
10001ef44: f100054a    	subs	x10, x10, #0x1
10001ef48: 54ffffa1    	b.ne	0x10001ef3c <<alloc::collections::btree::map::IntoIter<usize, num_bigint::bigint::BigInt>>::dying_next+0x60>
10001ef4c: d2800009    	mov	x9, #0x0                ; =0
10001ef50: 1400002e    	b	0x10001f008 <<alloc::collections::btree::map::IntoIter<usize, num_bigint::bigint::BigInt>>::dying_next+0x12c>
10001ef54: a940002a    	ldp	x10, x0, [x1]
10001ef58: a9412029    	ldp	x9, x8, [x1, #0x10]
10001ef5c: f900003f    	str	xzr, [x1]
10001ef60: f100055f    	cmp	x10, #0x1
10001ef64: 54000621    	b.ne	0x10001f028 <<alloc::collections::btree::map::IntoIter<usize, num_bigint::bigint::BigInt>>::dying_next+0x14c>
10001ef68: b50000c0    	cbnz	x0, 0x10001ef80 <<alloc::collections::btree::map::IntoIter<usize, num_bigint::bigint::BigInt>>::dying_next+0xa4>
10001ef6c: aa0903e0    	mov	x0, x9
10001ef70: b4000088    	cbz	x8, 0x10001ef80 <<alloc::collections::btree::map::IntoIter<usize, num_bigint::bigint::BigInt>>::dying_next+0xa4>
10001ef74: f940e400    	ldr	x0, [x0, #0x1c8]
10001ef78: f1000508    	subs	x8, x8, #0x1
10001ef7c: 54ffffc1    	b.ne	0x10001ef74 <<alloc::collections::btree::map::IntoIter<usize, num_bigint::bigint::BigInt>>::dying_next+0x98>
10001ef80: f940b008    	ldr	x8, [x0, #0x160]
10001ef84: b40004c8    	cbz	x8, 0x10001f01c <<alloc::collections::btree::map::IntoIter<usize, num_bigint::bigint::BigInt>>::dying_next+0x140>
10001ef88: aa0803f4    	mov	x20, x8
10001ef8c: 94019742    	bl	0x100084c94 <dyld_stub_binder+0x100084c94>
10001ef90: f940b288    	ldr	x8, [x20, #0x160]
10001ef94: aa1403e0    	mov	x0, x20
10001ef98: b5ffff88    	cbnz	x8, 0x10001ef88 <<alloc::collections::btree::map::IntoIter<usize, num_bigint::bigint::BigInt>>::dying_next+0xac>
10001ef9c: 14000021    	b	0x10001f020 <<alloc::collections::btree::map::IntoIter<usize, num_bigint::bigint::BigInt>>::dying_next+0x144>
10001efa0: a9412020    	ldp	x0, x8, [x1, #0x10]
10001efa4: b4000088    	cbz	x8, 0x10001efb4 <<alloc::collections::btree::map::IntoIter<usize, num_bigint::bigint::BigInt>>::dying_next+0xd8>
10001efa8: f940e400    	ldr	x0, [x0, #0x1c8]
10001efac: f1000508    	subs	x8, x8, #0x1
10001efb0: 54ffffc1    	b.ne	0x10001efa8 <<alloc::collections::btree::map::IntoIter<usize, num_bigint::bigint::BigInt>>::dying_next+0xcc>
10001efb4: d2800015    	mov	x21, #0x0               ; =0
10001efb8: d2800014    	mov	x20, #0x0               ; =0
10001efbc: 52800028    	mov	w8, #0x1                ; =1
10001efc0: f9000028    	str	x8, [x1]
10001efc4: 79438408    	ldrh	w8, [x0, #0x1c2]
10001efc8: eb0802bf    	cmp	x21, x8
10001efcc: 54fffae3    	b.lo	0x10001ef28 <<alloc::collections::btree::map::IntoIter<usize, num_bigint::bigint::BigInt>>::dying_next+0x4c>
10001efd0: aa0103f7    	mov	x23, x1
10001efd4: f940b016    	ldr	x22, [x0, #0x160]
10001efd8: b4000356    	cbz	x22, 0x10001f040 <<alloc::collections::btree::map::IntoIter<usize, num_bigint::bigint::BigInt>>::dying_next+0x164>
10001efdc: 91000694    	add	x20, x20, #0x1
10001efe0: 79438015    	ldrh	w21, [x0, #0x1c0]
10001efe4: 9401972c    	bl	0x100084c94 <dyld_stub_binder+0x100084c94>
10001efe8: 794386c8    	ldrh	w8, [x22, #0x1c2]
10001efec: aa1603e0    	mov	x0, x22
10001eff0: 6b0802bf    	cmp	w21, w8
10001eff4: 54ffff02    	b.hs	0x10001efd4 <<alloc::collections::btree::map::IntoIter<usize, num_bigint::bigint::BigInt>>::dying_next+0xf8>
10001eff8: aa1703e1    	mov	x1, x23
10001effc: b5fff9b4    	cbnz	x20, 0x10001ef30 <<alloc::collections::btree::map::IntoIter<usize, num_bigint::bigint::BigInt>>::dying_next+0x54>
10001f000: 910006a9    	add	x9, x21, #0x1
10001f004: aa1603e8    	mov	x8, x22
10001f008: a900fc28    	stp	x8, xzr, [x1, #0x8]
10001f00c: f9000c29    	str	x9, [x1, #0x18]
10001f010: a9005276    	stp	x22, x20, [x19]
10001f014: f9000a75    	str	x21, [x19, #0x10]
10001f018: 14000005    	b	0x10001f02c <<alloc::collections::btree::map::IntoIter<usize, num_bigint::bigint::BigInt>>::dying_next+0x150>
10001f01c: aa0003f4    	mov	x20, x0
10001f020: aa1403e0    	mov	x0, x20
10001f024: 9401971c    	bl	0x100084c94 <dyld_stub_binder+0x100084c94>
10001f028: f900027f    	str	xzr, [x19]
10001f02c: a9437bfd    	ldp	x29, x30, [sp, #0x30]
10001f030: a9424ff4    	ldp	x20, x19, [sp, #0x20]
10001f034: a94157f6    	ldp	x22, x21, [sp, #0x10]
10001f038: a8c45ff8    	ldp	x24, x23, [sp], #0x40
10001f03c: d65f03c0    	ret
10001f040: 94019715    	bl	0x100084c94 <dyld_stub_binder+0x100084c94>
10001f044: b0000420    	adrp	x0, 0x1000a4000 <dyld_stub_binder+0x1000a4000>
10001f048: 913e6000    	add	x0, x0, #0xf98
10001f04c: 94017c0e    	bl	0x10007e084 <core::option::unwrap_failed>
10001f050: d4200020    	brk	#0x1
10001f054: b0000420    	adrp	x0, 0x1000a4000 <dyld_stub_binder+0x1000a4000>
10001f058: 913bc000    	add	x0, x0, #0xef0
10001f05c: 94017c0a    	bl	0x10007e084 <core::option::unwrap_failed>
10001f060: d4200020    	brk	#0x1

000000010001f064 <field_regressions::arithmetic::vec2_dot>:
10001f064: d10083ff    	sub	sp, sp, #0x20
10001f068: a9017bfd    	stp	x29, x30, [sp, #0x10]
10001f06c: 910043fd    	add	x29, sp, #0x10
10001f070: a90013e2    	stp	x2, x4, [sp]
10001f074: eb04005f    	cmp	x2, x4
10001f078: 54000bc1    	b.ne	0x10001f1f0 <field_regressions::arithmetic::vec2_dot+0x18c>
10001f07c: 6f00e400    	movi.2d	v0, #0000000000000000
10001f080: 3d800000    	str	q0, [x0]
10001f084: d341fc49    	lsr	x9, x2, #1
10001f088: b40007c9    	cbz	x9, 0x10001f180 <field_regressions::arithmetic::vec2_dot+0x11c>
10001f08c: d2800008    	mov	x8, #0x0                ; =0
10001f090: d37ff92a    	lsl	x10, x9, #1
10001f094: 9100402b    	add	x11, x1, #0x10
10001f098: 9100406c    	add	x12, x3, #0x10
10001f09c: eb02011f    	cmp	x8, x2
10001f0a0: 54000c22    	b.hs	0x10001f224 <field_regressions::arithmetic::vec2_dot+0x1c0>
10001f0a4: 91000509    	add	x9, x8, #0x1
10001f0a8: eb02013f    	cmp	x9, x2
10001f0ac: 54000ae2    	b.hs	0x10001f208 <field_regressions::arithmetic::vec2_dot+0x1a4>
10001f0b0: 91000908    	add	x8, x8, #0x2
10001f0b4: 6d7f0981    	ldp	d1, d2, [x12, #-0x10]
10001f0b8: 6d7f1163    	ldp	d3, d4, [x11, #-0x10]
10001f0bc: 0ee1e065    	pmull.1q	v5, v3, v1
10001f0c0: 0ee2e063    	pmull.1q	v3, v3, v2
10001f0c4: 0ee2e082    	pmull.1q	v2, v4, v2
10001f0c8: 6cc21d86    	ldp	d6, d7, [x12], #0x20
10001f0cc: 6cc24570    	ldp	d16, d17, [x11], #0x20
10001f0d0: 0ee6e212    	pmull.1q	v18, v16, v6
10001f0d4: 0ee7e210    	pmull.1q	v16, v16, v7
10001f0d8: 0ee7e227    	pmull.1q	v7, v17, v7
10001f0dc: 0ee1e081    	pmull.1q	v1, v4, v1
10001f0e0: 6e231c21    	eor.16b	v1, v1, v3
10001f0e4: 0ee6e223    	pmull.1q	v3, v17, v6
10001f0e8: 6e301c63    	eor.16b	v3, v3, v16
10001f0ec: 4ed278a4    	zip2.2d	v4, v5, v18
10001f0f0: 6e180645    	mov.d	v5[1], v18[0]
10001f0f4: 4ec37826    	zip2.2d	v6, v1, v3
10001f0f8: 6e180461    	mov.d	v1[1], v3[0]
10001f0fc: 4ec77843    	zip2.2d	v3, v2, v7
10001f100: 6e1804e2    	mov.d	v2[1], v7[0]
10001f104: 6e221cc2    	eor.16b	v2, v6, v2
10001f108: 4ee28446    	add.2d	v6, v2, v2
10001f10c: 4ee38467    	add.2d	v7, v3, v3
10001f110: 6f411447    	usra.2d	v7, v2, #0x3f
10001f114: 4f425450    	shl.2d	v16, v2, #0x2
10001f118: 4f425471    	shl.2d	v17, v3, #0x2
10001f11c: 6f421451    	usra.2d	v17, v2, #0x3e
10001f120: 4f475452    	shl.2d	v18, v2, #0x7
10001f124: 4f475473    	shl.2d	v19, v3, #0x7
10001f128: 6f471453    	usra.2d	v19, v2, #0x39
10001f12c: 6f410474    	ushr.2d	v20, v3, #0x3f
10001f130: 6f420475    	ushr.2d	v21, v3, #0x3e
10001f134: 6f470476    	ushr.2d	v22, v3, #0x39
10001f138: ce145ab4    	eor3.16b	v20, v21, v20, v22
10001f13c: 4ef48695    	add.2d	v21, v20, v20
10001f140: 4f425696    	shl.2d	v22, v20, #0x2
10001f144: 4f475697    	shl.2d	v23, v20, #0x7
10001f148: ce054a05    	eor3.16b	v5, v16, v5, v18
10001f14c: ce0658a5    	eor3.16b	v5, v5, v6, v22
10001f150: ce1754a5    	eor3.16b	v5, v5, v23, v21
10001f154: ce0250a2    	eor3.16b	v2, v5, v2, v20
10001f158: 6e211c81    	eor.16b	v1, v4, v1
10001f15c: ce114c21    	eor3.16b	v1, v1, v17, v19
10001f160: ce070c21    	eor3.16b	v1, v1, v7, v3
10001f164: 4ec17843    	zip2.2d	v3, v2, v1
10001f168: 4ec13841    	zip1.2d	v1, v2, v1
10001f16c: ce010060    	eor3.16b	v0, v3, v1, v0
10001f170: eb08015f    	cmp	x10, x8
10001f174: 54fff941    	b.ne	0x10001f09c <field_regressions::arithmetic::vec2_dot+0x38>
10001f178: 5e180401    	mov	d1, v0[1]
10001f17c: fd000401    	str	d1, [x0, #0x8]
10001f180: 927fe448    	and	x8, x2, #0x7fffffffffffffe
10001f184: fd000000    	str	d0, [x0]
10001f188: eb02011f    	cmp	x8, x2
10001f18c: 540002c0    	b.eq	0x10001f1e4 <field_regressions::arithmetic::vec2_dot+0x180>
10001f190: d37ced08    	lsl	x8, x8, #4
10001f194: 8b080029    	add	x9, x1, x8
10001f198: 8b080068    	add	x8, x3, x8
10001f19c: 6d400901    	ldp	d1, d2, [x8]
10001f1a0: 6d401123    	ldp	d3, d4, [x9]
10001f1a4: 0ee1e065    	pmull.1q	v5, v3, v1
10001f1a8: 0ee2e063    	pmull.1q	v3, v3, v2
10001f1ac: 0ee2e082    	pmull.1q	v2, v4, v2
10001f1b0: 0ee1e081    	pmull.1q	v1, v4, v1
10001f1b4: 6e231c21    	eor.16b	v1, v1, v3
10001f1b8: 6f00e403    	movi.2d	v3, #0000000000000000
10001f1bc: 6e024064    	ext.16b	v4, v3, v2, #0x8
10001f1c0: 528010e8    	mov	w8, #0x87               ; =135
10001f1c4: 4e080d06    	dup.2d	v6, x8
10001f1c8: 4ee6e042    	pmull2.1q	v2, v2, v6
10001f1cc: ce010881    	eor3.16b	v1, v4, v1, v2
10001f1d0: 6e014062    	ext.16b	v2, v3, v1, #0x8
10001f1d4: 4ee6e021    	pmull2.1q	v1, v1, v6
10001f1d8: 6e251c21    	eor.16b	v1, v1, v5
10001f1dc: ce010040    	eor3.16b	v0, v2, v1, v0
10001f1e0: 3d800000    	str	q0, [x0]
10001f1e4: a9417bfd    	ldp	x29, x30, [sp, #0x10]
10001f1e8: 910083ff    	add	sp, sp, #0x20
10001f1ec: d65f03c0    	ret
10001f1f0: b0000423    	adrp	x3, 0x1000a4000 <dyld_stub_binder+0x1000a4000>
10001f1f4: 91262063    	add	x3, x3, #0x988
10001f1f8: 910003e0    	mov	x0, sp
10001f1fc: 910023e1    	add	x1, sp, #0x8
10001f200: d2800002    	mov	x2, #0x0                ; =0
10001f204: 94017b49    	bl	0x10007df28 <core::panicking::assert_failed::<usize, usize>>
10001f208: 3d800000    	str	q0, [x0]
10001f20c: b0000428    	adrp	x8, 0x1000a4000 <dyld_stub_binder+0x1000a4000>
10001f210: 9126e108    	add	x8, x8, #0x9b8
10001f214: aa0903e0    	mov	x0, x9
10001f218: aa0203e1    	mov	x1, x2
10001f21c: aa0803e2    	mov	x2, x8
10001f220: 94017b34    	bl	0x10007def0 <core::panicking::panic_bounds_check>
10001f224: 3d800000    	str	q0, [x0]
10001f228: b0000429    	adrp	x9, 0x1000a4000 <dyld_stub_binder+0x1000a4000>
10001f22c: 91268129    	add	x9, x9, #0x9a0
10001f230: aa0803e0    	mov	x0, x8
10001f234: aa0203e1    	mov	x1, x2
10001f238: aa0903e2    	mov	x2, x9
10001f23c: 94017b2d    	bl	0x10007def0 <core::panicking::panic_bounds_check>


/var/folders/q0/v7l8dzd13k7f_q6j80sy03lm0000gn/T/f2z-field-regressions-emqdbh47/target/release/field-regressions:	file format mach-o arm64

Disassembly of section __TEXT,__text:

00000001000200c4 <<field_regressions::arithmetic::Kernel>::products>:
1000200c4: d100c3ff    	sub	sp, sp, #0x30
1000200c8: a9027bfd    	stp	x29, x30, [sp, #0x20]
1000200cc: 910083fd    	add	x29, sp, #0x20
1000200d0: f90007e2    	str	x2, [sp, #0x8]
1000200d4: f81f83a4    	stur	x4, [x29, #-0x8]
1000200d8: eb04005f    	cmp	x2, x4
1000200dc: 54001d81    	b.ne	0x10002048c <<field_regressions::arithmetic::Kernel>::products+0x3c8>
1000200e0: f9000be2    	str	x2, [sp, #0x10]
1000200e4: f81f83a6    	stur	x6, [x29, #-0x8]
1000200e8: eb06005f    	cmp	x2, x6
1000200ec: 54001dc1    	b.ne	0x1000204a4 <<field_regressions::arithmetic::Kernel>::products+0x3e0>
1000200f0: 12001c08    	and	w8, w0, #0xff
1000200f4: 7100091f    	cmp	w8, #0x2
1000200f8: 540003cc    	b.gt	0x100020170 <<field_regressions::arithmetic::Kernel>::products+0xac>
1000200fc: 34000ae8    	cbz	w8, 0x100020258 <<field_regressions::arithmetic::Kernel>::products+0x194>
100020100: 7100051f    	cmp	w8, #0x1
100020104: 54001361    	b.ne	0x100020370 <<field_regressions::arithmetic::Kernel>::products+0x2ac>
100020108: b4001bc2    	cbz	x2, 0x100020480 <<field_regressions::arithmetic::Kernel>::products+0x3bc>
10002010c: 91002068    	add	x8, x3, #0x8
100020110: 91002029    	add	x9, x1, #0x8
100020114: 6f00e400    	movi.2d	v0, #0000000000000000
100020118: 528010ea    	mov	w10, #0x87              ; =135
10002011c: 4e080d41    	dup.2d	v1, x10
100020120: 6d7f8d02    	ldp	d2, d3, [x8, #-0x8]
100020124: 6d7f9524    	ldp	d4, d5, [x9, #-0x8]
100020128: 0ee3e0a6    	pmull.1q	v6, v5, v3
10002012c: 0ee3e083    	pmull.1q	v3, v4, v3
100020130: 0ee2e0a5    	pmull.1q	v5, v5, v2
100020134: 6e231ca3    	eor.16b	v3, v5, v3
100020138: 6e064005    	ext.16b	v5, v0, v6, #0x8
10002013c: 4ee1e0c6    	pmull2.1q	v6, v6, v1
100020140: ce0318a3    	eor3.16b	v3, v5, v3, v6
100020144: 6e034005    	ext.16b	v5, v0, v3, #0x8
100020148: 0ee2e082    	pmull.1q	v2, v4, v2
10002014c: 6e221ca2    	eor.16b	v2, v5, v2
100020150: 4ee1e063    	pmull2.1q	v3, v3, v1
100020154: 6e231c42    	eor.16b	v2, v2, v3
100020158: 3c8104a2    	str	q2, [x5], #0x10
10002015c: 91004108    	add	x8, x8, #0x10
100020160: 91004129    	add	x9, x9, #0x10
100020164: f1000442    	subs	x2, x2, #0x1
100020168: 54fffdc1    	b.ne	0x100020120 <<field_regressions::arithmetic::Kernel>::products+0x5c>
10002016c: 140000c5    	b	0x100020480 <<field_regressions::arithmetic::Kernel>::products+0x3bc>
100020170: 71000d1f    	cmp	w8, #0x3
100020174: 54000a60    	b.eq	0x1000202c0 <<field_regressions::arithmetic::Kernel>::products+0x1fc>
100020178: 7100111f    	cmp	w8, #0x4
10002017c: 540012e1    	b.ne	0x1000203d8 <<field_regressions::arithmetic::Kernel>::products+0x314>
100020180: b4001802    	cbz	x2, 0x100020480 <<field_regressions::arithmetic::Kernel>::products+0x3bc>
100020184: 910020a8    	add	x8, x5, #0x8
100020188: 91002069    	add	x9, x3, #0x8
10002018c: 9100202a    	add	x10, x1, #0x8
100020190: a97fb12b    	ldp	x11, x12, [x9, #-0x8]
100020194: ca0b018d    	eor	x13, x12, x11
100020198: 9e670160    	fmov	d0, x11
10002019c: a97fb94b    	ldp	x11, x14, [x10, #-0x8]
1000201a0: ca0b01cf    	eor	x15, x14, x11
1000201a4: 9e670161    	fmov	d1, x11
1000201a8: 0ee0e020    	pmull.1q	v0, v1, v0
1000201ac: 9e670181    	fmov	d1, x12
1000201b0: 9e6701c2    	fmov	d2, x14
1000201b4: 0ee1e041    	pmull.1q	v1, v2, v1
1000201b8: 9e6701a2    	fmov	d2, x13
1000201bc: 9e6701e3    	fmov	d3, x15
1000201c0: 0ee2e062    	pmull.1q	v2, v3, v2
1000201c4: 4e183c0b    	mov.d	x11, v0[1]
1000201c8: ce020403    	eor3.16b	v3, v0, v2, v1
1000201cc: 9e66000c    	fmov	x12, d0
1000201d0: 4e183c2d    	mov.d	x13, v1[1]
1000201d4: 9e66002e    	fmov	x14, d1
1000201d8: 4e183c6f    	mov.d	x15, v3[1]
1000201dc: ca0e01ef    	eor	x15, x15, x14
1000201e0: 93cffdb0    	extr	x16, x13, x15, #0x3f
1000201e4: 9e660051    	fmov	x17, d2
1000201e8: 93cff9a0    	extr	x0, x13, x15, #0x3e
1000201ec: 93cfe5a1    	extr	x1, x13, x15, #0x39
1000201f0: d37ffda3    	lsr	x3, x13, #63
1000201f4: ca4df863    	eor	x3, x3, x13, lsr #62
1000201f8: ca4de463    	eor	x3, x3, x13, lsr #57
1000201fc: d37ef464    	lsl	x4, x3, #2
100020200: ca030484    	eor	x4, x4, x3, lsl #1
100020204: ca031c84    	eor	x4, x4, x3, lsl #7
100020208: ca0f0484    	eor	x4, x4, x15, lsl #1
10002020c: ca0f0884    	eor	x4, x4, x15, lsl #2
100020210: ca0f1c84    	eor	x4, x4, x15, lsl #7
100020214: ca030183    	eor	x3, x12, x3
100020218: ca0f006f    	eor	x15, x3, x15
10002021c: ca0f008f    	eor	x15, x4, x15
100020220: ca000231    	eor	x17, x17, x0
100020224: ca010210    	eor	x16, x16, x1
100020228: ca100230    	eor	x16, x17, x16
10002022c: ca0e016b    	eor	x11, x11, x14
100020230: ca0c016b    	eor	x11, x11, x12
100020234: ca0d016b    	eor	x11, x11, x13
100020238: ca0b020b    	eor	x11, x16, x11
10002023c: a93fad0f    	stp	x15, x11, [x8, #-0x8]
100020240: 91004108    	add	x8, x8, #0x10
100020244: 91004129    	add	x9, x9, #0x10
100020248: 9100414a    	add	x10, x10, #0x10
10002024c: f1000442    	subs	x2, x2, #0x1
100020250: 54fffa01    	b.ne	0x100020190 <<field_regressions::arithmetic::Kernel>::products+0xcc>
100020254: 1400008b    	b	0x100020480 <<field_regressions::arithmetic::Kernel>::products+0x3bc>
100020258: b4001142    	cbz	x2, 0x100020480 <<field_regressions::arithmetic::Kernel>::products+0x3bc>
10002025c: 91002068    	add	x8, x3, #0x8
100020260: 91002029    	add	x9, x1, #0x8
100020264: 6f00e400    	movi.2d	v0, #0000000000000000
100020268: 528010ea    	mov	w10, #0x87              ; =135
10002026c: 4e080d41    	dup.2d	v1, x10
100020270: 6d7f8d02    	ldp	d2, d3, [x8, #-0x8]
100020274: 6d7f9524    	ldp	d4, d5, [x9, #-0x8]
100020278: 0ee3e086    	pmull.1q	v6, v4, v3
10002027c: 0ee3e0a3    	pmull.1q	v3, v5, v3
100020280: 0ee2e0a5    	pmull.1q	v5, v5, v2
100020284: 6e261ca5    	eor.16b	v5, v5, v6
100020288: 6e034006    	ext.16b	v6, v0, v3, #0x8
10002028c: 4ee1e063    	pmull2.1q	v3, v3, v1
100020290: ce050cc3    	eor3.16b	v3, v6, v5, v3
100020294: 6e034005    	ext.16b	v5, v0, v3, #0x8
100020298: 0ee2e082    	pmull.1q	v2, v4, v2
10002029c: 6e221ca2    	eor.16b	v2, v5, v2
1000202a0: 4ee1e063    	pmull2.1q	v3, v3, v1
1000202a4: 6e231c42    	eor.16b	v2, v2, v3
1000202a8: 3c8104a2    	str	q2, [x5], #0x10
1000202ac: 91004108    	add	x8, x8, #0x10
1000202b0: 91004129    	add	x9, x9, #0x10
1000202b4: f1000442    	subs	x2, x2, #0x1
1000202b8: 54fffdc1    	b.ne	0x100020270 <<field_regressions::arithmetic::Kernel>::products+0x1ac>
1000202bc: 14000071    	b	0x100020480 <<field_regressions::arithmetic::Kernel>::products+0x3bc>
1000202c0: b4000e02    	cbz	x2, 0x100020480 <<field_regressions::arithmetic::Kernel>::products+0x3bc>
1000202c4: 910020a8    	add	x8, x5, #0x8
1000202c8: 91002069    	add	x9, x3, #0x8
1000202cc: 9100202a    	add	x10, x1, #0x8
1000202d0: 6d7f8520    	ldp	d0, d1, [x9, #-0x8]
1000202d4: 6d7f8d42    	ldp	d2, d3, [x10, #-0x8]
1000202d8: 0ee1e044    	pmull.1q	v4, v2, v1
1000202dc: 0ee0e065    	pmull.1q	v5, v3, v0
1000202e0: 6e241ca4    	eor.16b	v4, v5, v4
1000202e4: 4e180485    	dup.2d	v5, v4[1]
1000202e8: 0ee1e061    	pmull.1q	v1, v3, v1
1000202ec: 0ee0e040    	pmull.1q	v0, v2, v0
1000202f0: 6e211ca2    	eor.16b	v2, v5, v1
1000202f4: 4e183c2b    	mov.d	x11, v1[1]
1000202f8: 4e180401    	dup.2d	v1, v0[1]
1000202fc: 9e66000c    	fmov	x12, d0
100020300: 6e241c20    	eor.16b	v0, v1, v4
100020304: 9e66004d    	fmov	x13, d2
100020308: 93cdfd6e    	extr	x14, x11, x13, #0x3f
10002030c: 93cdf96f    	extr	x15, x11, x13, #0x3e
100020310: 9e660010    	fmov	x16, d0
100020314: 93cde571    	extr	x17, x11, x13, #0x39
100020318: d37ffd60    	lsr	x0, x11, #63
10002031c: ca4bf800    	eor	x0, x0, x11, lsr #62
100020320: ca4be400    	eor	x0, x0, x11, lsr #57
100020324: ca0d098c    	eor	x12, x12, x13, lsl #2
100020328: ca0d058c    	eor	x12, x12, x13, lsl #1
10002032c: ca0d1d8c    	eor	x12, x12, x13, lsl #7
100020330: ca00098c    	eor	x12, x12, x0, lsl #2
100020334: ca00058c    	eor	x12, x12, x0, lsl #1
100020338: ca001d8c    	eor	x12, x12, x0, lsl #7
10002033c: ca0001ad    	eor	x13, x13, x0
100020340: ca0f020f    	eor	x15, x16, x15
100020344: ca1101ce    	eor	x14, x14, x17
100020348: ca0e01ee    	eor	x14, x15, x14
10002034c: ca0b01cb    	eor	x11, x14, x11
100020350: ca0d018c    	eor	x12, x12, x13
100020354: a93fad0c    	stp	x12, x11, [x8, #-0x8]
100020358: 91004108    	add	x8, x8, #0x10
10002035c: 91004129    	add	x9, x9, #0x10
100020360: 9100414a    	add	x10, x10, #0x10
100020364: f1000442    	subs	x2, x2, #0x1
100020368: 54fffb41    	b.ne	0x1000202d0 <<field_regressions::arithmetic::Kernel>::products+0x20c>
10002036c: 14000045    	b	0x100020480 <<field_regressions::arithmetic::Kernel>::products+0x3bc>
100020370: b4000882    	cbz	x2, 0x100020480 <<field_regressions::arithmetic::Kernel>::products+0x3bc>
100020374: 91002068    	add	x8, x3, #0x8
100020378: 91002029    	add	x9, x1, #0x8
10002037c: 6f00e400    	movi.2d	v0, #0000000000000000
100020380: 528010ea    	mov	w10, #0x87              ; =135
100020384: 4e080d41    	dup.2d	v1, x10
100020388: 6d7f8d02    	ldp	d2, d3, [x8, #-0x8]
10002038c: 6d7f9524    	ldp	d4, d5, [x9, #-0x8]
100020390: 0ee3e086    	pmull.1q	v6, v4, v3
100020394: 0ee3e0a3    	pmull.1q	v3, v5, v3
100020398: 0ee2e0a5    	pmull.1q	v5, v5, v2
10002039c: 6e261ca5    	eor.16b	v5, v5, v6
1000203a0: 6e034006    	ext.16b	v6, v0, v3, #0x8
1000203a4: 4ee1e063    	pmull2.1q	v3, v3, v1
1000203a8: ce050cc3    	eor3.16b	v3, v6, v5, v3
1000203ac: 6e034005    	ext.16b	v5, v0, v3, #0x8
1000203b0: 0ee2e082    	pmull.1q	v2, v4, v2
1000203b4: 6e221ca2    	eor.16b	v2, v5, v2
1000203b8: 4ee1e063    	pmull2.1q	v3, v3, v1
1000203bc: 6e231c42    	eor.16b	v2, v2, v3
1000203c0: 3c8104a2    	str	q2, [x5], #0x10
1000203c4: 91004108    	add	x8, x8, #0x10
1000203c8: 91004129    	add	x9, x9, #0x10
1000203cc: f1000442    	subs	x2, x2, #0x1
1000203d0: 54fffdc1    	b.ne	0x100020388 <<field_regressions::arithmetic::Kernel>::products+0x2c4>
1000203d4: 1400002b    	b	0x100020480 <<field_regressions::arithmetic::Kernel>::products+0x3bc>
1000203d8: b4000542    	cbz	x2, 0x100020480 <<field_regressions::arithmetic::Kernel>::products+0x3bc>
1000203dc: 91002068    	add	x8, x3, #0x8
1000203e0: 91002029    	add	x9, x1, #0x8
1000203e4: 528010ea    	mov	w10, #0x87              ; =135
1000203e8: 4e080d40    	dup.2d	v0, x10
1000203ec: 9e670141    	fmov	d1, x10
1000203f0: a97fad0a    	ldp	x10, x11, [x8, #-0x8]
1000203f4: ca0a016c    	eor	x12, x11, x10
1000203f8: 9e670142    	fmov	d2, x10
1000203fc: a97fb52a    	ldp	x10, x13, [x9, #-0x8]
100020400: ca0a01ae    	eor	x14, x13, x10
100020404: 9e670143    	fmov	d3, x10
100020408: 0ee2e062    	pmull.1q	v2, v3, v2
10002040c: 9e670163    	fmov	d3, x11
100020410: 9e6701a4    	fmov	d4, x13
100020414: 9e670185    	fmov	d5, x12
100020418: 9e6701c6    	fmov	d6, x14
10002041c: 0ee3e083    	pmull.1q	v3, v4, v3
100020420: 0ee5e0c4    	pmull.1q	v4, v6, v5
100020424: ce040c44    	eor3.16b	v4, v2, v4, v3
100020428: 4e180445    	dup.2d	v5, v2[1]
10002042c: 4e180486    	dup.2d	v6, v4[1]
100020430: 4ee0e067    	pmull2.1q	v7, v3, v0
100020434: 6e271ca5    	eor.16b	v5, v5, v7
100020438: 4e183cea    	mov.d	x10, v7[1]
10002043c: 2e231cc3    	eor.8b	v3, v6, v3
100020440: 0ee1e063    	pmull.1q	v3, v3, v1
100020444: 6e231c42    	eor.16b	v2, v2, v3
100020448: 9e66004b    	fmov	x11, d2
10002044c: ca0a096b    	eor	x11, x11, x10, lsl #2
100020450: 4f4754e2    	shl.2d	v2, v7, #0x7
100020454: ca0a056a    	eor	x10, x11, x10, lsl #1
100020458: 6e054042    	ext.16b	v2, v2, v5, #0x8
10002045c: 4e080484    	dup.2d	v4, v4[0]
100020460: 4e081d44    	mov.d	v4[0], x10
100020464: 4ec378e3    	zip2.2d	v3, v7, v3
100020468: ce040c42    	eor3.16b	v2, v2, v4, v3
10002046c: 3c8104a2    	str	q2, [x5], #0x10
100020470: 91004108    	add	x8, x8, #0x10
100020474: 91004129    	add	x9, x9, #0x10
100020478: f1000442    	subs	x2, x2, #0x1
10002047c: 54fffba1    	b.ne	0x1000203f0 <<field_regressions::arithmetic::Kernel>::products+0x32c>
100020480: a9427bfd    	ldp	x29, x30, [sp, #0x20]
100020484: 9100c3ff    	add	sp, sp, #0x30
100020488: d65f03c0    	ret
10002048c: 90000443    	adrp	x3, 0x1000a8000 <dyld_stub_binder+0x1000a8000>
100020490: 91288063    	add	x3, x3, #0xa20
100020494: 910023e0    	add	x0, sp, #0x8
100020498: d10023a1    	sub	x1, x29, #0x8
10002049c: d2800002    	mov	x2, #0x0                ; =0
1000204a0: 94017b38    	bl	0x10007f180 <core::panicking::assert_failed::<usize, usize>>
1000204a4: 90000443    	adrp	x3, 0x1000a8000 <dyld_stub_binder+0x1000a8000>
1000204a8: 9128e063    	add	x3, x3, #0xa38
1000204ac: 910043e0    	add	x0, sp, #0x10
1000204b0: d10023a1    	sub	x1, x29, #0x8
1000204b4: d2800002    	mov	x2, #0x0                ; =0
1000204b8: 94017b32    	bl	0x10007f180 <core::panicking::assert_failed::<usize, usize>>

00000001000204bc <<std::io::buffered::bufwriter::BufWriter<std::fs::File>>::flush_buf>:
1000204bc: a9ba6ffc    	stp	x28, x27, [sp, #-0x60]!
1000204c0: a90167fa    	stp	x26, x25, [sp, #0x10]
1000204c4: a9025ff8    	stp	x24, x23, [sp, #0x20]
1000204c8: a90357f6    	stp	x22, x21, [sp, #0x30]
1000204cc: a9044ff4    	stp	x20, x19, [sp, #0x40]

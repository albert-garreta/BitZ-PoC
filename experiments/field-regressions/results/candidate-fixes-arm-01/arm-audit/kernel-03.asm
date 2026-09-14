
/private/var/folders/q0/v7l8dzd13k7f_q6j80sy03lm0000gn/T/f2z-field-gated-pycl0nhx/field-regressions:	file format mach-o arm64

Disassembly of section __TEXT,__text:

00000001000381c8 <__RINvNtNtNtCsagpXUhzPPpl_17field_regressions8campaign10candidates5words13exact_columnsKj9_Kj13_EB8_>:
1000381c8: a9ba6ffc    	stp	x28, x27, [sp, #-0x60]!
1000381cc: a90167fa    	stp	x26, x25, [sp, #0x10]
1000381d0: a9025ff8    	stp	x24, x23, [sp, #0x20]
1000381d4: a90357f6    	stp	x22, x21, [sp, #0x30]
1000381d8: a9044ff4    	stp	x20, x19, [sp, #0x40]
1000381dc: a9057bfd    	stp	x29, x30, [sp, #0x50]
1000381e0: 910143fd    	add	x29, sp, #0x50
1000381e4: d10683ff    	sub	sp, sp, #0x1a0
1000381e8: a90593e2    	stp	x2, x4, [sp, #0x58]
1000381ec: eb04005f    	cmp	x2, x4
1000381f0: 54001d61    	b.ne	0x10003859c <__RINvNtNtNtCsagpXUhzPPpl_17field_regressions8campaign10candidates5words13exact_columnsKj9_Kj13_EB8_+0x3d4>
1000381f4: 910183e8    	add	x8, sp, #0x60
1000381f8: 6f00e400    	movi.2d	v0, #0000000000000000
1000381fc: ad088100    	stp	q0, q0, [x8, #0x110]
100038200: ad078100    	stp	q0, q0, [x8, #0xf0]
100038204: ad068100    	stp	q0, q0, [x8, #0xd0]
100038208: ad058100    	stp	q0, q0, [x8, #0xb0]
10003820c: ad048100    	stp	q0, q0, [x8, #0x90]
100038210: 3d802100    	str	q0, [x8, #0x80]
100038214: ad0603e0    	stp	q0, q0, [sp, #0xc0]
100038218: ad0503e0    	stp	q0, q0, [sp, #0xa0]
10003821c: ad0403e0    	stp	q0, q0, [sp, #0x80]
100038220: ad0303e0    	stp	q0, q0, [sp, #0x60]
100038224: b40010a2    	cbz	x2, 0x100038438 <__RINvNtNtNtCsagpXUhzPPpl_17field_regressions8campaign10candidates5words13exact_columnsKj9_Kj13_EB8_+0x270>
100038228: d2800008    	mov	x8, #0x0                ; =0
10003822c: 52800909    	mov	w9, #0x48               ; =72
100038230: 910183ea    	add	x10, sp, #0x60
100038234: d280000b    	mov	x11, #0x0               ; =0
100038238: 9b090d06    	madd	x6, x8, x9, x3
10003823c: a94234cc    	ldp	x12, x13, [x6, #0x20]
100038240: a9433cce    	ldp	x14, x15, [x6, #0x30]
100038244: a94044d0    	ldp	x16, x17, [x6]
100038248: a94110c5    	ldp	x5, x4, [x6, #0x10]
10003824c: f94020c6    	ldr	x6, [x6, #0x40]
100038250: aa0103e7    	mov	x7, x1
100038254: f84084f4    	ldr	x20, [x7], #0x8
100038258: 8b0b0153    	add	x19, x10, x11
10003825c: 9bd47e15    	umulh	x21, x16, x20
100038260: 9b147e16    	mul	x22, x16, x20
100038264: a9405e78    	ldp	x24, x23, [x19]
100038268: ab1802d6    	adds	x22, x22, x24
10003826c: 9a9736f7    	cinc	x23, x23, hs
100038270: a9005e76    	stp	x22, x23, [x19]
100038274: a9415a77    	ldp	x23, x22, [x19, #0x10]
100038278: ab1502f5    	adds	x21, x23, x21
10003827c: 9a9636d6    	cinc	x22, x22, hs
100038280: 9bd47e37    	umulh	x23, x17, x20
100038284: 9b147e38    	mul	x24, x17, x20
100038288: ab150315    	adds	x21, x24, x21
10003828c: 9a9636d6    	cinc	x22, x22, hs
100038290: a9015a75    	stp	x21, x22, [x19, #0x10]
100038294: a9425676    	ldp	x22, x21, [x19, #0x20]
100038298: ab1702d6    	adds	x22, x22, x23
10003829c: 9a9536b5    	cinc	x21, x21, hs
1000382a0: 9bd47cb7    	umulh	x23, x5, x20
1000382a4: 9b147cb8    	mul	x24, x5, x20
1000382a8: ab160316    	adds	x22, x24, x22
1000382ac: 9a9536b5    	cinc	x21, x21, hs
1000382b0: a9025676    	stp	x22, x21, [x19, #0x20]
1000382b4: a9435676    	ldp	x22, x21, [x19, #0x30]
1000382b8: ab1702d6    	adds	x22, x22, x23
1000382bc: 9a9536b5    	cinc	x21, x21, hs
1000382c0: 9bd47c97    	umulh	x23, x4, x20
1000382c4: 9b147c98    	mul	x24, x4, x20
1000382c8: ab160316    	adds	x22, x24, x22
1000382cc: 9a9536b5    	cinc	x21, x21, hs
1000382d0: a9035676    	stp	x22, x21, [x19, #0x30]
1000382d4: a9445676    	ldp	x22, x21, [x19, #0x40]
1000382d8: ab1702d6    	adds	x22, x22, x23
1000382dc: 9a9536b5    	cinc	x21, x21, hs
1000382e0: 9bd47d97    	umulh	x23, x12, x20
1000382e4: 9b147d98    	mul	x24, x12, x20
1000382e8: ab160316    	adds	x22, x24, x22
1000382ec: 9a9536b5    	cinc	x21, x21, hs
1000382f0: a9045676    	stp	x22, x21, [x19, #0x40]
1000382f4: a9455676    	ldp	x22, x21, [x19, #0x50]
1000382f8: ab1702d6    	adds	x22, x22, x23
1000382fc: 9a9536b5    	cinc	x21, x21, hs
100038300: 9bd47db7    	umulh	x23, x13, x20
100038304: 9b147db8    	mul	x24, x13, x20
100038308: ab160316    	adds	x22, x24, x22
10003830c: 9a9536b5    	cinc	x21, x21, hs
100038310: a9055676    	stp	x22, x21, [x19, #0x50]
100038314: a9465676    	ldp	x22, x21, [x19, #0x60]
100038318: ab1702d6    	adds	x22, x22, x23
10003831c: 9a9536b5    	cinc	x21, x21, hs
100038320: 9bd47dd7    	umulh	x23, x14, x20
100038324: 9b147dd8    	mul	x24, x14, x20
100038328: ab160316    	adds	x22, x24, x22
10003832c: 9a9536b5    	cinc	x21, x21, hs
100038330: a9065676    	stp	x22, x21, [x19, #0x60]
100038334: a9475676    	ldp	x22, x21, [x19, #0x70]
100038338: ab1702d6    	adds	x22, x22, x23
10003833c: 9a9536b5    	cinc	x21, x21, hs
100038340: 9bd47df7    	umulh	x23, x15, x20
100038344: 9b147df8    	mul	x24, x15, x20
100038348: ab160316    	adds	x22, x24, x22
10003834c: 9a9536b5    	cinc	x21, x21, hs
100038350: a9075676    	stp	x22, x21, [x19, #0x70]
100038354: a9485676    	ldp	x22, x21, [x19, #0x80]
100038358: ab1702d6    	adds	x22, x22, x23
10003835c: 9a9536b5    	cinc	x21, x21, hs
100038360: 9bd47cd7    	umulh	x23, x6, x20
100038364: 9b147cd4    	mul	x20, x6, x20
100038368: ab160294    	adds	x20, x20, x22
10003836c: 9a9536b5    	cinc	x21, x21, hs
100038370: a9085674    	stp	x20, x21, [x19, #0x80]
100038374: a9495275    	ldp	x21, x20, [x19, #0x90]
100038378: ab1702b5    	adds	x21, x21, x23
10003837c: 9a943694    	cinc	x20, x20, hs
100038380: a9095275    	stp	x21, x20, [x19, #0x90]
100038384: 9100416b    	add	x11, x11, #0x10
100038388: f102417f    	cmp	x11, #0x90
10003838c: 54fff641    	b.ne	0x100038254 <__RINvNtNtNtCsagpXUhzPPpl_17field_regressions8campaign10candidates5words13exact_columnsKj9_Kj13_EB8_+0x8c>
100038390: 91000508    	add	x8, x8, #0x1
100038394: 91012021    	add	x1, x1, #0x48
100038398: eb02011f    	cmp	x8, x2
10003839c: 54fff4c1    	b.ne	0x100038234 <__RINvNtNtNtCsagpXUhzPPpl_17field_regressions8campaign10candidates5words13exact_columnsKj9_Kj13_EB8_+0x6c>
1000383a0: a94637ec    	ldp	x12, x13, [sp, #0x60]
1000383a4: a9472ff0    	ldp	x16, x11, [sp, #0x70]
1000383a8: a9482bef    	ldp	x15, x10, [sp, #0x80]
1000383ac: a94953ee    	ldp	x14, x20, [sp, #0x90]
1000383b0: a94a4fe9    	ldp	x9, x19, [sp, #0xa0]
1000383b4: a94b1fe8    	ldp	x8, x7, [sp, #0xb0]
1000383b8: a94c47fe    	ldp	x30, x17, [sp, #0xc0]
1000383bc: f90007f1    	str	x17, [sp, #0x8]
1000383c0: a94d1bfc    	ldp	x28, x6, [sp, #0xd0]
1000383c4: a94e17fb    	ldp	x27, x5, [sp, #0xe0]
1000383c8: a94f13fa    	ldp	x26, x4, [sp, #0xf0]
1000383cc: a9500ff9    	ldp	x25, x3, [sp, #0x100]
1000383d0: a9510bf8    	ldp	x24, x2, [sp, #0x110]
1000383d4: a95207f7    	ldp	x23, x1, [sp, #0x120]
1000383d8: a95347f6    	ldp	x22, x17, [sp, #0x130]
1000383dc: f9000bf1    	str	x17, [sp, #0x10]
1000383e0: a95447f5    	ldp	x21, x17, [sp, #0x140]
1000383e4: f9000ff1    	str	x17, [sp, #0x18]
1000383e8: f940aff1    	ldr	x17, [sp, #0x158]
1000383ec: f90017f1    	str	x17, [sp, #0x28]
1000383f0: f940abf1    	ldr	x17, [sp, #0x150]
1000383f4: f90013f1    	str	x17, [sp, #0x20]
1000383f8: f940b7f1    	ldr	x17, [sp, #0x168]
1000383fc: f9001ff1    	str	x17, [sp, #0x38]
100038400: f940b3f1    	ldr	x17, [sp, #0x160]
100038404: f9001bf1    	str	x17, [sp, #0x30]
100038408: f940bff1    	ldr	x17, [sp, #0x178]
10003840c: f9002bf1    	str	x17, [sp, #0x50]
100038410: f940bbf1    	ldr	x17, [sp, #0x170]
100038414: f90023f1    	str	x17, [sp, #0x40]
100038418: f940c3f1    	ldr	x17, [sp, #0x180]
10003841c: f90027f1    	str	x17, [sp, #0x48]
100038420: aa0103f1    	mov	x17, x1
100038424: aa0203e1    	mov	x1, x2
100038428: aa0303e2    	mov	x2, x3
10003842c: aa0403e3    	mov	x3, x4
100038430: f94007e4    	ldr	x4, [sp, #0x8]
100038434: 14000021    	b	0x1000384b8 <__RINvNtNtNtCsagpXUhzPPpl_17field_regressions8campaign10candidates5words13exact_columnsKj9_Kj13_EB8_+0x2f0>
100038438: a9047fff    	stp	xzr, xzr, [sp, #0x40]
10003843c: f9002bff    	str	xzr, [sp, #0x50]
100038440: a9037fff    	stp	xzr, xzr, [sp, #0x30]
100038444: a9027fff    	stp	xzr, xzr, [sp, #0x20]
100038448: d2800015    	mov	x21, #0x0               ; =0
10003844c: a9017fff    	stp	xzr, xzr, [sp, #0x10]
100038450: d2800016    	mov	x22, #0x0               ; =0
100038454: d2800017    	mov	x23, #0x0               ; =0
100038458: d2800011    	mov	x17, #0x0               ; =0
10003845c: d2800018    	mov	x24, #0x0               ; =0
100038460: d2800001    	mov	x1, #0x0                ; =0
100038464: d2800019    	mov	x25, #0x0               ; =0
100038468: d280001a    	mov	x26, #0x0               ; =0
10003846c: d2800003    	mov	x3, #0x0                ; =0
100038470: d280001b    	mov	x27, #0x0               ; =0
100038474: d2800005    	mov	x5, #0x0                ; =0
100038478: d280001c    	mov	x28, #0x0               ; =0
10003847c: d2800006    	mov	x6, #0x0                ; =0
100038480: d280001e    	mov	x30, #0x0               ; =0
100038484: d2800004    	mov	x4, #0x0                ; =0
100038488: d2800008    	mov	x8, #0x0                ; =0
10003848c: d2800007    	mov	x7, #0x0                ; =0
100038490: d2800009    	mov	x9, #0x0                ; =0
100038494: d2800013    	mov	x19, #0x0               ; =0
100038498: d280000e    	mov	x14, #0x0               ; =0
10003849c: d2800014    	mov	x20, #0x0               ; =0
1000384a0: d280000f    	mov	x15, #0x0               ; =0
1000384a4: d280000a    	mov	x10, #0x0               ; =0
1000384a8: d2800010    	mov	x16, #0x0               ; =0
1000384ac: d280000b    	mov	x11, #0x0               ; =0
1000384b0: d280000c    	mov	x12, #0x0               ; =0
1000384b4: d280000d    	mov	x13, #0x0               ; =0
1000384b8: ab1001ad    	adds	x13, x13, x16
1000384bc: a900340c    	stp	x12, x13, [x0]
1000384c0: 9a8b356b    	cinc	x11, x11, hs
1000384c4: ab0f016b    	adds	x11, x11, x15
1000384c8: 9a8a354a    	cinc	x10, x10, hs
1000384cc: ab0e014a    	adds	x10, x10, x14
1000384d0: a901280b    	stp	x11, x10, [x0, #0x10]
1000384d4: 9a94368a    	cinc	x10, x20, hs
1000384d8: ab090149    	adds	x9, x10, x9
1000384dc: 9a93366a    	cinc	x10, x19, hs
1000384e0: ab080148    	adds	x8, x10, x8
1000384e4: 9a8734ea    	cinc	x10, x7, hs
1000384e8: ab1e014a    	adds	x10, x10, x30
1000384ec: 9a84348b    	cinc	x11, x4, hs
1000384f0: ab1c016b    	adds	x11, x11, x28
1000384f4: 9a8634cc    	cinc	x12, x6, hs
1000384f8: ab1b018c    	adds	x12, x12, x27
1000384fc: 9a8534ad    	cinc	x13, x5, hs
100038500: ab1a01ad    	adds	x13, x13, x26
100038504: 9a83346e    	cinc	x14, x3, hs
100038508: ab1901ce    	adds	x14, x14, x25
10003850c: 9a82344f    	cinc	x15, x2, hs
100038510: ab1801ef    	adds	x15, x15, x24
100038514: 9a813430    	cinc	x16, x1, hs
100038518: ab170210    	adds	x16, x16, x23
10003851c: 9a913631    	cinc	x17, x17, hs
100038520: ab160231    	adds	x17, x17, x22
100038524: a9410be1    	ldp	x1, x2, [sp, #0x10]
100038528: 9a813421    	cinc	x1, x1, hs
10003852c: ab150021    	adds	x1, x1, x21
100038530: 9a823442    	cinc	x2, x2, hs
100038534: a9022009    	stp	x9, x8, [x0, #0x20]
100038538: a94227e8    	ldp	x8, x9, [sp, #0x20]
10003853c: ab080048    	adds	x8, x2, x8
100038540: a9032c0a    	stp	x10, x11, [x0, #0x30]
100038544: 9a893529    	cinc	x9, x9, hs
100038548: a904340c    	stp	x12, x13, [x0, #0x40]
10003854c: a9432beb    	ldp	x11, x10, [sp, #0x30]
100038550: ab0b0129    	adds	x9, x9, x11
100038554: a9053c0e    	stp	x14, x15, [x0, #0x50]
100038558: 9a8a354a    	cinc	x10, x10, hs
10003855c: a9064410    	stp	x16, x17, [x0, #0x60]
100038560: f94023eb    	ldr	x11, [sp, #0x40]
100038564: ab0b014a    	adds	x10, x10, x11
100038568: a9072001    	stp	x1, x8, [x0, #0x70]
10003856c: a944a3eb    	ldp	x11, x8, [sp, #0x48]
100038570: 9a080168    	adc	x8, x11, x8
100038574: a9082809    	stp	x9, x10, [x0, #0x80]
100038578: f9004808    	str	x8, [x0, #0x90]
10003857c: 910683ff    	add	sp, sp, #0x1a0
100038580: a9457bfd    	ldp	x29, x30, [sp, #0x50]
100038584: a9444ff4    	ldp	x20, x19, [sp, #0x40]
100038588: a94357f6    	ldp	x22, x21, [sp, #0x30]
10003858c: a9425ff8    	ldp	x24, x23, [sp, #0x20]
100038590: a94167fa    	ldp	x26, x25, [sp, #0x10]
100038594: a8c66ffc    	ldp	x28, x27, [sp], #0x60
100038598: d65f03c0    	ret
10003859c: f0000ba4    	adrp	x4, 0x1001af000 <dyld_stub_binder+0x1001af000>
1000385a0: 91370084    	add	x4, x4, #0xdc0
1000385a4: 910163e0    	add	x0, sp, #0x58
1000385a8: 910183e1    	add	x1, sp, #0x60
1000385ac: d2800002    	mov	x2, #0x0                ; =0
1000385b0: 94048c28    	bl	0x10015b650 <__RINvNtCs8Mbv00yxnRz_4core9panicking13assert_failedjjEB4_>

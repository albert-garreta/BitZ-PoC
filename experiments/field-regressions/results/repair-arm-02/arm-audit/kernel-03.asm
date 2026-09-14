
/private/var/folders/q0/v7l8dzd13k7f_q6j80sy03lm0000gn/T/f2z-field-gated-aejimxw9/field-regressions:	file format mach-o arm64

Disassembly of section __TEXT,__text:

000000010003739c <__RINvNtNtNtCsaHyC9lX8wyC_17field_regressions8campaign10candidates5words13exact_columnsKj9_Kj13_EB8_>:
10003739c: a9ba6ffc    	stp	x28, x27, [sp, #-0x60]!
1000373a0: a90167fa    	stp	x26, x25, [sp, #0x10]
1000373a4: a9025ff8    	stp	x24, x23, [sp, #0x20]
1000373a8: a90357f6    	stp	x22, x21, [sp, #0x30]
1000373ac: a9044ff4    	stp	x20, x19, [sp, #0x40]
1000373b0: a9057bfd    	stp	x29, x30, [sp, #0x50]
1000373b4: 910143fd    	add	x29, sp, #0x50
1000373b8: d10683ff    	sub	sp, sp, #0x1a0
1000373bc: a90593e2    	stp	x2, x4, [sp, #0x58]
1000373c0: eb04005f    	cmp	x2, x4
1000373c4: 54001d61    	b.ne	0x100037770 <__RINvNtNtNtCsaHyC9lX8wyC_17field_regressions8campaign10candidates5words13exact_columnsKj9_Kj13_EB8_+0x3d4>
1000373c8: 910183e8    	add	x8, sp, #0x60
1000373cc: 6f00e400    	movi.2d	v0, #0000000000000000
1000373d0: ad088100    	stp	q0, q0, [x8, #0x110]
1000373d4: ad078100    	stp	q0, q0, [x8, #0xf0]
1000373d8: ad068100    	stp	q0, q0, [x8, #0xd0]
1000373dc: ad058100    	stp	q0, q0, [x8, #0xb0]
1000373e0: ad048100    	stp	q0, q0, [x8, #0x90]
1000373e4: 3d802100    	str	q0, [x8, #0x80]
1000373e8: ad0603e0    	stp	q0, q0, [sp, #0xc0]
1000373ec: ad0503e0    	stp	q0, q0, [sp, #0xa0]
1000373f0: ad0403e0    	stp	q0, q0, [sp, #0x80]
1000373f4: ad0303e0    	stp	q0, q0, [sp, #0x60]
1000373f8: b40010a2    	cbz	x2, 0x10003760c <__RINvNtNtNtCsaHyC9lX8wyC_17field_regressions8campaign10candidates5words13exact_columnsKj9_Kj13_EB8_+0x270>
1000373fc: d2800008    	mov	x8, #0x0                ; =0
100037400: 52800909    	mov	w9, #0x48               ; =72
100037404: 910183ea    	add	x10, sp, #0x60
100037408: d280000b    	mov	x11, #0x0               ; =0
10003740c: 9b090d06    	madd	x6, x8, x9, x3
100037410: a94234cc    	ldp	x12, x13, [x6, #0x20]
100037414: a9433cce    	ldp	x14, x15, [x6, #0x30]
100037418: a94044d0    	ldp	x16, x17, [x6]
10003741c: a94110c5    	ldp	x5, x4, [x6, #0x10]
100037420: f94020c6    	ldr	x6, [x6, #0x40]
100037424: aa0103e7    	mov	x7, x1
100037428: f84084f4    	ldr	x20, [x7], #0x8
10003742c: 8b0b0153    	add	x19, x10, x11
100037430: 9bd47e15    	umulh	x21, x16, x20
100037434: 9b147e16    	mul	x22, x16, x20
100037438: a9405e78    	ldp	x24, x23, [x19]
10003743c: ab1802d6    	adds	x22, x22, x24
100037440: 9a9736f7    	cinc	x23, x23, hs
100037444: a9005e76    	stp	x22, x23, [x19]
100037448: a9415a77    	ldp	x23, x22, [x19, #0x10]
10003744c: ab1502f5    	adds	x21, x23, x21
100037450: 9a9636d6    	cinc	x22, x22, hs
100037454: 9bd47e37    	umulh	x23, x17, x20
100037458: 9b147e38    	mul	x24, x17, x20
10003745c: ab150315    	adds	x21, x24, x21
100037460: 9a9636d6    	cinc	x22, x22, hs
100037464: a9015a75    	stp	x21, x22, [x19, #0x10]
100037468: a9425676    	ldp	x22, x21, [x19, #0x20]
10003746c: ab1702d6    	adds	x22, x22, x23
100037470: 9a9536b5    	cinc	x21, x21, hs
100037474: 9bd47cb7    	umulh	x23, x5, x20
100037478: 9b147cb8    	mul	x24, x5, x20
10003747c: ab160316    	adds	x22, x24, x22
100037480: 9a9536b5    	cinc	x21, x21, hs
100037484: a9025676    	stp	x22, x21, [x19, #0x20]
100037488: a9435676    	ldp	x22, x21, [x19, #0x30]
10003748c: ab1702d6    	adds	x22, x22, x23
100037490: 9a9536b5    	cinc	x21, x21, hs
100037494: 9bd47c97    	umulh	x23, x4, x20
100037498: 9b147c98    	mul	x24, x4, x20
10003749c: ab160316    	adds	x22, x24, x22
1000374a0: 9a9536b5    	cinc	x21, x21, hs
1000374a4: a9035676    	stp	x22, x21, [x19, #0x30]
1000374a8: a9445676    	ldp	x22, x21, [x19, #0x40]
1000374ac: ab1702d6    	adds	x22, x22, x23
1000374b0: 9a9536b5    	cinc	x21, x21, hs
1000374b4: 9bd47d97    	umulh	x23, x12, x20
1000374b8: 9b147d98    	mul	x24, x12, x20
1000374bc: ab160316    	adds	x22, x24, x22
1000374c0: 9a9536b5    	cinc	x21, x21, hs
1000374c4: a9045676    	stp	x22, x21, [x19, #0x40]
1000374c8: a9455676    	ldp	x22, x21, [x19, #0x50]
1000374cc: ab1702d6    	adds	x22, x22, x23
1000374d0: 9a9536b5    	cinc	x21, x21, hs
1000374d4: 9bd47db7    	umulh	x23, x13, x20
1000374d8: 9b147db8    	mul	x24, x13, x20
1000374dc: ab160316    	adds	x22, x24, x22
1000374e0: 9a9536b5    	cinc	x21, x21, hs
1000374e4: a9055676    	stp	x22, x21, [x19, #0x50]
1000374e8: a9465676    	ldp	x22, x21, [x19, #0x60]
1000374ec: ab1702d6    	adds	x22, x22, x23
1000374f0: 9a9536b5    	cinc	x21, x21, hs
1000374f4: 9bd47dd7    	umulh	x23, x14, x20
1000374f8: 9b147dd8    	mul	x24, x14, x20
1000374fc: ab160316    	adds	x22, x24, x22
100037500: 9a9536b5    	cinc	x21, x21, hs
100037504: a9065676    	stp	x22, x21, [x19, #0x60]
100037508: a9475676    	ldp	x22, x21, [x19, #0x70]
10003750c: ab1702d6    	adds	x22, x22, x23
100037510: 9a9536b5    	cinc	x21, x21, hs
100037514: 9bd47df7    	umulh	x23, x15, x20
100037518: 9b147df8    	mul	x24, x15, x20
10003751c: ab160316    	adds	x22, x24, x22
100037520: 9a9536b5    	cinc	x21, x21, hs
100037524: a9075676    	stp	x22, x21, [x19, #0x70]
100037528: a9485676    	ldp	x22, x21, [x19, #0x80]
10003752c: ab1702d6    	adds	x22, x22, x23
100037530: 9a9536b5    	cinc	x21, x21, hs
100037534: 9bd47cd7    	umulh	x23, x6, x20
100037538: 9b147cd4    	mul	x20, x6, x20
10003753c: ab160294    	adds	x20, x20, x22
100037540: 9a9536b5    	cinc	x21, x21, hs
100037544: a9085674    	stp	x20, x21, [x19, #0x80]
100037548: a9495275    	ldp	x21, x20, [x19, #0x90]
10003754c: ab1702b5    	adds	x21, x21, x23
100037550: 9a943694    	cinc	x20, x20, hs
100037554: a9095275    	stp	x21, x20, [x19, #0x90]
100037558: 9100416b    	add	x11, x11, #0x10
10003755c: f102417f    	cmp	x11, #0x90
100037560: 54fff641    	b.ne	0x100037428 <__RINvNtNtNtCsaHyC9lX8wyC_17field_regressions8campaign10candidates5words13exact_columnsKj9_Kj13_EB8_+0x8c>
100037564: 91000508    	add	x8, x8, #0x1
100037568: 91012021    	add	x1, x1, #0x48
10003756c: eb02011f    	cmp	x8, x2
100037570: 54fff4c1    	b.ne	0x100037408 <__RINvNtNtNtCsaHyC9lX8wyC_17field_regressions8campaign10candidates5words13exact_columnsKj9_Kj13_EB8_+0x6c>
100037574: a94637ec    	ldp	x12, x13, [sp, #0x60]
100037578: a9472ff0    	ldp	x16, x11, [sp, #0x70]
10003757c: a9482bef    	ldp	x15, x10, [sp, #0x80]
100037580: a94953ee    	ldp	x14, x20, [sp, #0x90]
100037584: a94a4fe9    	ldp	x9, x19, [sp, #0xa0]
100037588: a94b1fe8    	ldp	x8, x7, [sp, #0xb0]
10003758c: a94c47fe    	ldp	x30, x17, [sp, #0xc0]
100037590: f90007f1    	str	x17, [sp, #0x8]
100037594: a94d1bfc    	ldp	x28, x6, [sp, #0xd0]
100037598: a94e17fb    	ldp	x27, x5, [sp, #0xe0]
10003759c: a94f13fa    	ldp	x26, x4, [sp, #0xf0]
1000375a0: a9500ff9    	ldp	x25, x3, [sp, #0x100]
1000375a4: a9510bf8    	ldp	x24, x2, [sp, #0x110]
1000375a8: a95207f7    	ldp	x23, x1, [sp, #0x120]
1000375ac: a95347f6    	ldp	x22, x17, [sp, #0x130]
1000375b0: f9000bf1    	str	x17, [sp, #0x10]
1000375b4: a95447f5    	ldp	x21, x17, [sp, #0x140]
1000375b8: f9000ff1    	str	x17, [sp, #0x18]
1000375bc: f940aff1    	ldr	x17, [sp, #0x158]
1000375c0: f90017f1    	str	x17, [sp, #0x28]
1000375c4: f940abf1    	ldr	x17, [sp, #0x150]
1000375c8: f90013f1    	str	x17, [sp, #0x20]
1000375cc: f940b7f1    	ldr	x17, [sp, #0x168]
1000375d0: f9001ff1    	str	x17, [sp, #0x38]
1000375d4: f940b3f1    	ldr	x17, [sp, #0x160]
1000375d8: f9001bf1    	str	x17, [sp, #0x30]
1000375dc: f940bff1    	ldr	x17, [sp, #0x178]
1000375e0: f9002bf1    	str	x17, [sp, #0x50]
1000375e4: f940bbf1    	ldr	x17, [sp, #0x170]
1000375e8: f90023f1    	str	x17, [sp, #0x40]
1000375ec: f940c3f1    	ldr	x17, [sp, #0x180]
1000375f0: f90027f1    	str	x17, [sp, #0x48]
1000375f4: aa0103f1    	mov	x17, x1
1000375f8: aa0203e1    	mov	x1, x2
1000375fc: aa0303e2    	mov	x2, x3
100037600: aa0403e3    	mov	x3, x4
100037604: f94007e4    	ldr	x4, [sp, #0x8]
100037608: 14000021    	b	0x10003768c <__RINvNtNtNtCsaHyC9lX8wyC_17field_regressions8campaign10candidates5words13exact_columnsKj9_Kj13_EB8_+0x2f0>
10003760c: a9047fff    	stp	xzr, xzr, [sp, #0x40]
100037610: f9002bff    	str	xzr, [sp, #0x50]
100037614: a9037fff    	stp	xzr, xzr, [sp, #0x30]
100037618: a9027fff    	stp	xzr, xzr, [sp, #0x20]
10003761c: d2800015    	mov	x21, #0x0               ; =0
100037620: a9017fff    	stp	xzr, xzr, [sp, #0x10]
100037624: d2800016    	mov	x22, #0x0               ; =0
100037628: d2800017    	mov	x23, #0x0               ; =0
10003762c: d2800011    	mov	x17, #0x0               ; =0
100037630: d2800018    	mov	x24, #0x0               ; =0
100037634: d2800001    	mov	x1, #0x0                ; =0
100037638: d2800019    	mov	x25, #0x0               ; =0
10003763c: d280001a    	mov	x26, #0x0               ; =0
100037640: d2800003    	mov	x3, #0x0                ; =0
100037644: d280001b    	mov	x27, #0x0               ; =0
100037648: d2800005    	mov	x5, #0x0                ; =0
10003764c: d280001c    	mov	x28, #0x0               ; =0
100037650: d2800006    	mov	x6, #0x0                ; =0
100037654: d280001e    	mov	x30, #0x0               ; =0
100037658: d2800004    	mov	x4, #0x0                ; =0
10003765c: d2800008    	mov	x8, #0x0                ; =0
100037660: d2800007    	mov	x7, #0x0                ; =0
100037664: d2800009    	mov	x9, #0x0                ; =0
100037668: d2800013    	mov	x19, #0x0               ; =0
10003766c: d280000e    	mov	x14, #0x0               ; =0
100037670: d2800014    	mov	x20, #0x0               ; =0
100037674: d280000f    	mov	x15, #0x0               ; =0
100037678: d280000a    	mov	x10, #0x0               ; =0
10003767c: d2800010    	mov	x16, #0x0               ; =0
100037680: d280000b    	mov	x11, #0x0               ; =0
100037684: d280000c    	mov	x12, #0x0               ; =0
100037688: d280000d    	mov	x13, #0x0               ; =0
10003768c: ab1001ad    	adds	x13, x13, x16
100037690: a900340c    	stp	x12, x13, [x0]
100037694: 9a8b356b    	cinc	x11, x11, hs
100037698: ab0f016b    	adds	x11, x11, x15
10003769c: 9a8a354a    	cinc	x10, x10, hs
1000376a0: ab0e014a    	adds	x10, x10, x14
1000376a4: a901280b    	stp	x11, x10, [x0, #0x10]
1000376a8: 9a94368a    	cinc	x10, x20, hs
1000376ac: ab090149    	adds	x9, x10, x9
1000376b0: 9a93366a    	cinc	x10, x19, hs
1000376b4: ab080148    	adds	x8, x10, x8
1000376b8: 9a8734ea    	cinc	x10, x7, hs
1000376bc: ab1e014a    	adds	x10, x10, x30
1000376c0: 9a84348b    	cinc	x11, x4, hs
1000376c4: ab1c016b    	adds	x11, x11, x28
1000376c8: 9a8634cc    	cinc	x12, x6, hs
1000376cc: ab1b018c    	adds	x12, x12, x27
1000376d0: 9a8534ad    	cinc	x13, x5, hs
1000376d4: ab1a01ad    	adds	x13, x13, x26
1000376d8: 9a83346e    	cinc	x14, x3, hs
1000376dc: ab1901ce    	adds	x14, x14, x25
1000376e0: 9a82344f    	cinc	x15, x2, hs
1000376e4: ab1801ef    	adds	x15, x15, x24
1000376e8: 9a813430    	cinc	x16, x1, hs
1000376ec: ab170210    	adds	x16, x16, x23
1000376f0: 9a913631    	cinc	x17, x17, hs
1000376f4: ab160231    	adds	x17, x17, x22
1000376f8: a9410be1    	ldp	x1, x2, [sp, #0x10]
1000376fc: 9a813421    	cinc	x1, x1, hs
100037700: ab150021    	adds	x1, x1, x21
100037704: 9a823442    	cinc	x2, x2, hs
100037708: a9022009    	stp	x9, x8, [x0, #0x20]
10003770c: a94227e8    	ldp	x8, x9, [sp, #0x20]
100037710: ab080048    	adds	x8, x2, x8
100037714: a9032c0a    	stp	x10, x11, [x0, #0x30]
100037718: 9a893529    	cinc	x9, x9, hs
10003771c: a904340c    	stp	x12, x13, [x0, #0x40]
100037720: a9432beb    	ldp	x11, x10, [sp, #0x30]
100037724: ab0b0129    	adds	x9, x9, x11
100037728: a9053c0e    	stp	x14, x15, [x0, #0x50]
10003772c: 9a8a354a    	cinc	x10, x10, hs
100037730: a9064410    	stp	x16, x17, [x0, #0x60]
100037734: f94023eb    	ldr	x11, [sp, #0x40]
100037738: ab0b014a    	adds	x10, x10, x11
10003773c: a9072001    	stp	x1, x8, [x0, #0x70]
100037740: a944a3eb    	ldp	x11, x8, [sp, #0x48]
100037744: 9a080168    	adc	x8, x11, x8
100037748: a9082809    	stp	x9, x10, [x0, #0x80]
10003774c: f9004808    	str	x8, [x0, #0x90]
100037750: 910683ff    	add	sp, sp, #0x1a0
100037754: a9457bfd    	ldp	x29, x30, [sp, #0x50]
100037758: a9444ff4    	ldp	x20, x19, [sp, #0x40]
10003775c: a94357f6    	ldp	x22, x21, [sp, #0x30]
100037760: a9425ff8    	ldp	x24, x23, [sp, #0x20]
100037764: a94167fa    	ldp	x26, x25, [sp, #0x10]
100037768: a8c66ffc    	ldp	x28, x27, [sp], #0x60
10003776c: d65f03c0    	ret
100037770: 90000b24    	adrp	x4, 0x10019b000 <dyld_stub_binder+0x10019b000>
100037774: 912fa084    	add	x4, x4, #0xbe8
100037778: 910163e0    	add	x0, sp, #0x58
10003777c: 910183e1    	add	x1, sp, #0x60
100037780: d2800002    	mov	x2, #0x0                ; =0
100037784: 94045079    	bl	0x10014b968 <__RINvNtCs8Mbv00yxnRz_4core9panicking13assert_failedjjEB4_>

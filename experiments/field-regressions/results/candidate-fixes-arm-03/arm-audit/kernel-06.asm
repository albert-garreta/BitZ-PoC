
/private/var/folders/q0/v7l8dzd13k7f_q6j80sy03lm0000gn/T/f2z-field-gated-h_tggvge/field-regressions:	file format mach-o arm64

Disassembly of section __TEXT,__text:

0000000100039200 <__RINvNtNtNtCsfDw8YqMpZPC_17field_regressions8campaign10candidates5words20exact_signed_columnsKj9_Kj13_EB8_>:
100039200: a9ba6ffc    	stp	x28, x27, [sp, #-0x60]!
100039204: a90167fa    	stp	x26, x25, [sp, #0x10]
100039208: a9025ff8    	stp	x24, x23, [sp, #0x20]
10003920c: a90357f6    	stp	x22, x21, [sp, #0x30]
100039210: a9044ff4    	stp	x20, x19, [sp, #0x40]
100039214: a9057bfd    	stp	x29, x30, [sp, #0x50]
100039218: 910143fd    	add	x29, sp, #0x50
10003921c: d10683ff    	sub	sp, sp, #0x1a0
100039220: a90387e3    	stp	x3, x1, [sp, #0x38]
100039224: a90493e2    	stp	x2, x4, [sp, #0x48]
100039228: eb04005f    	cmp	x2, x4
10003922c: 54002ac1    	b.ne	0x100039784 <__RINvNtNtNtCsfDw8YqMpZPC_17field_regressions8campaign10candidates5words20exact_signed_columnsKj9_Kj13_EB8_+0x584>
100039230: f9001be0    	str	x0, [sp, #0x30]
100039234: 910143e8    	add	x8, sp, #0x50
100039238: 6f00e400    	movi.2d	v0, #0000000000000000
10003923c: ad088100    	stp	q0, q0, [x8, #0x110]
100039240: ad078100    	stp	q0, q0, [x8, #0xf0]
100039244: ad068100    	stp	q0, q0, [x8, #0xd0]
100039248: ad058100    	stp	q0, q0, [x8, #0xb0]
10003924c: ad048100    	stp	q0, q0, [x8, #0x90]
100039250: 3d802100    	str	q0, [x8, #0x80]
100039254: ad0583e0    	stp	q0, q0, [sp, #0xb0]
100039258: ad0483e0    	stp	q0, q0, [sp, #0x90]
10003925c: ad0383e0    	stp	q0, q0, [sp, #0x70]
100039260: ad0283e0    	stp	q0, q0, [sp, #0x50]
100039264: b4001b22    	cbz	x2, 0x1000395c8 <__RINvNtNtNtCsfDw8YqMpZPC_17field_regressions8campaign10candidates5words20exact_signed_columnsKj9_Kj13_EB8_+0x3c8>
100039268: d2800003    	mov	x3, #0x0                ; =0
10003926c: 910143ed    	add	x13, sp, #0x50
100039270: f94023f0    	ldr	x16, [sp, #0x40]
100039274: d2800011    	mov	x17, #0x0               ; =0
100039278: d2800009    	mov	x9, #0x0                ; =0
10003927c: 8b110e28    	add	x8, x17, x17, lsl #3
100039280: d37df108    	lsl	x8, x8, #3
100039284: a943afea    	ldp	x10, x11, [sp, #0x38]
100039288: 8b080177    	add	x23, x11, x8
10003928c: 8b08014b    	add	x11, x10, x8
100039290: a9424d74    	ldp	x20, x19, [x11, #0x20]
100039294: a9436578    	ldp	x24, x25, [x11, #0x30]
100039298: a9411964    	ldp	x4, x6, [x11, #0x10]
10003929c: a9402968    	ldp	x8, x10, [x11]
1000392a0: f940217b    	ldr	x27, [x11, #0x40]
1000392a4: aa1003eb    	mov	x11, x16
1000392a8: f840856e    	ldr	x14, [x11], #0x8
1000392ac: 8b0901ac    	add	x12, x13, x9
1000392b0: 9bce7d0f    	umulh	x15, x8, x14
1000392b4: 9b0e7d00    	mul	x0, x8, x14
1000392b8: a9400585    	ldp	x5, x1, [x12]
1000392bc: ab050000    	adds	x0, x0, x5
1000392c0: 9a813421    	cinc	x1, x1, hs
1000392c4: a9000580    	stp	x0, x1, [x12]
1000392c8: a9410181    	ldp	x1, x0, [x12, #0x10]
1000392cc: ab0f002f    	adds	x15, x1, x15
1000392d0: 9a803400    	cinc	x0, x0, hs
1000392d4: 9bce7d41    	umulh	x1, x10, x14
1000392d8: 9b0e7d45    	mul	x5, x10, x14
1000392dc: ab0f00af    	adds	x15, x5, x15
1000392e0: 9a803400    	cinc	x0, x0, hs
1000392e4: a901018f    	stp	x15, x0, [x12, #0x10]
1000392e8: a9423d80    	ldp	x0, x15, [x12, #0x20]
1000392ec: ab010000    	adds	x0, x0, x1
1000392f0: 9a8f35ef    	cinc	x15, x15, hs
1000392f4: 9bce7c81    	umulh	x1, x4, x14
1000392f8: 9b0e7c85    	mul	x5, x4, x14
1000392fc: ab0000a0    	adds	x0, x5, x0
100039300: 9a8f35ef    	cinc	x15, x15, hs
100039304: a9023d80    	stp	x0, x15, [x12, #0x20]
100039308: a9433d80    	ldp	x0, x15, [x12, #0x30]
10003930c: ab010000    	adds	x0, x0, x1
100039310: 9a8f35ef    	cinc	x15, x15, hs
100039314: 9bce7cc1    	umulh	x1, x6, x14
100039318: 9b0e7cc5    	mul	x5, x6, x14
10003931c: ab0000a0    	adds	x0, x5, x0
100039320: 9a8f35ef    	cinc	x15, x15, hs
100039324: a9033d80    	stp	x0, x15, [x12, #0x30]
100039328: a9443d80    	ldp	x0, x15, [x12, #0x40]
10003932c: ab010000    	adds	x0, x0, x1
100039330: 9a8f35ef    	cinc	x15, x15, hs
100039334: 9bce7e81    	umulh	x1, x20, x14
100039338: 9b0e7e85    	mul	x5, x20, x14
10003933c: ab0000a0    	adds	x0, x5, x0
100039340: 9a8f35ef    	cinc	x15, x15, hs
100039344: a9043d80    	stp	x0, x15, [x12, #0x40]
100039348: a9453d80    	ldp	x0, x15, [x12, #0x50]
10003934c: ab010000    	adds	x0, x0, x1
100039350: 9a8f35ef    	cinc	x15, x15, hs
100039354: 9bce7e61    	umulh	x1, x19, x14
100039358: 9b0e7e65    	mul	x5, x19, x14
10003935c: ab0000a0    	adds	x0, x5, x0
100039360: 9a8f35ef    	cinc	x15, x15, hs
100039364: a9053d80    	stp	x0, x15, [x12, #0x50]
100039368: a9463d80    	ldp	x0, x15, [x12, #0x60]
10003936c: ab010000    	adds	x0, x0, x1
100039370: 9a8f35ef    	cinc	x15, x15, hs
100039374: 9bce7f01    	umulh	x1, x24, x14
100039378: 9b0e7f05    	mul	x5, x24, x14
10003937c: ab0000a0    	adds	x0, x5, x0
100039380: 9a8f35ef    	cinc	x15, x15, hs
100039384: a9063d80    	stp	x0, x15, [x12, #0x60]
100039388: a9473d80    	ldp	x0, x15, [x12, #0x70]
10003938c: ab010000    	adds	x0, x0, x1
100039390: 9a8f35ef    	cinc	x15, x15, hs
100039394: 9bce7f21    	umulh	x1, x25, x14
100039398: 9b0e7f25    	mul	x5, x25, x14
10003939c: ab0000a0    	adds	x0, x5, x0
1000393a0: 9a8f35ef    	cinc	x15, x15, hs
1000393a4: a9073d80    	stp	x0, x15, [x12, #0x70]
1000393a8: a9483d80    	ldp	x0, x15, [x12, #0x80]
1000393ac: ab010000    	adds	x0, x0, x1
1000393b0: 9a8f35ef    	cinc	x15, x15, hs
1000393b4: 9bce7f61    	umulh	x1, x27, x14
1000393b8: 9b0e7f6e    	mul	x14, x27, x14
1000393bc: ab0001ce    	adds	x14, x14, x0
1000393c0: 9a8f35ef    	cinc	x15, x15, hs
1000393c4: a9083d8e    	stp	x14, x15, [x12, #0x80]
1000393c8: a949398f    	ldp	x15, x14, [x12, #0x90]
1000393cc: ab0101ef    	adds	x15, x15, x1
1000393d0: 9a8e35ce    	cinc	x14, x14, hs
1000393d4: a909398f    	stp	x15, x14, [x12, #0x90]
1000393d8: 91004129    	add	x9, x9, #0x10
1000393dc: f102413f    	cmp	x9, #0x90
1000393e0: 54fff641    	b.ne	0x1000392a8 <__RINvNtNtNtCsfDw8YqMpZPC_17field_regressions8campaign10candidates5words20exact_signed_columnsKj9_Kj13_EB8_+0xa8>
1000393e4: f94022fc    	ldr	x28, [x23, #0x40]
1000393e8: d37fff9e    	lsr	x30, x28, #63
1000393ec: d37fff6e    	lsr	x14, x27, #63
1000393f0: 3819f3be    	sturb	w30, [x29, #-0x61]
1000393f4: d10187ab    	sub	x11, x29, #0x61
1000393f8: 3859f3a9    	ldurb	w9, [x29, #-0x61]
1000393fc: aa0303ec    	mov	x12, x3
100039400: 9280000f    	mov	x15, #-0x1              ; =-1
100039404: f2401d3f    	tst	x9, #0xff
100039408: 9a8311ec    	csel	x12, x15, x3, ne
10003940c: 3819f3ae    	sturb	w14, [x29, #-0x61]
100039410: 3859f3a9    	ldurb	w9, [x29, #-0x61]
100039414: aa0303e0    	mov	x0, x3
100039418: f2401d3f    	tst	x9, #0xff
10003941c: 9a8311e0    	csel	x0, x15, x3, ne
100039420: 8a0c0108    	and	x8, x8, x12
100039424: a9402ee9    	ldp	x9, x11, [x23]
100039428: 8a000129    	and	x9, x9, x0
10003942c: a94e17e7    	ldp	x7, x5, [sp, #0xe0]
100039430: ab090108    	adds	x8, x8, x9
100039434: 1a9f37e9    	cset	w9, hs
100039438: eb0800e8    	subs	x8, x7, x8
10003943c: da0900a9    	sbc	x9, x5, x9
100039440: a90e27e8    	stp	x8, x9, [sp, #0xe0]
100039444: 8a0c014a    	and	x10, x10, x12
100039448: 8a00016b    	and	x11, x11, x0
10003944c: a94f17e7    	ldp	x7, x5, [sp, #0xf0]
100039450: ab0b014a    	adds	x10, x10, x11
100039454: 1a9f37eb    	cset	w11, hs
100039458: eb0a00ea    	subs	x10, x7, x10
10003945c: da0b00ab    	sbc	x11, x5, x11
100039460: a90f2fea    	stp	x10, x11, [sp, #0xf0]
100039464: 8a0c0084    	and	x4, x4, x12
100039468: a9411ee5    	ldp	x5, x7, [x23, #0x10]
10003946c: 8a0000a5    	and	x5, x5, x0
100039470: a95057f6    	ldp	x22, x21, [sp, #0x100]
100039474: ab050084    	adds	x4, x4, x5
100039478: 1a9f37e5    	cset	w5, hs
10003947c: eb0402c4    	subs	x4, x22, x4
100039480: da0502a5    	sbc	x5, x21, x5
100039484: a91017e4    	stp	x4, x5, [sp, #0x100]
100039488: 8a0c00c6    	and	x6, x6, x12
10003948c: 8a0000e7    	and	x7, x7, x0
100039490: a95157f6    	ldp	x22, x21, [sp, #0x110]
100039494: ab0700c6    	adds	x6, x6, x7
100039498: 1a9f37e7    	cset	w7, hs
10003949c: eb0602c6    	subs	x6, x22, x6
1000394a0: da0702a7    	sbc	x7, x21, x7
1000394a4: a9111fe6    	stp	x6, x7, [sp, #0x110]
1000394a8: 8a0c0294    	and	x20, x20, x12
1000394ac: a9426af5    	ldp	x21, x26, [x23, #0x20]
1000394b0: 8a0002b5    	and	x21, x21, x0
1000394b4: a9525be1    	ldp	x1, x22, [sp, #0x120]
1000394b8: ab150294    	adds	x20, x20, x21
1000394bc: 1a9f37ef    	cset	w15, hs
1000394c0: eb140035    	subs	x21, x1, x20
1000394c4: da0f02d6    	sbc	x22, x22, x15
1000394c8: a9125bf5    	stp	x21, x22, [sp, #0x120]
1000394cc: 8a0c026f    	and	x15, x19, x12
1000394d0: 8a000341    	and	x1, x26, x0
1000394d4: a95353f3    	ldp	x19, x20, [sp, #0x130]
1000394d8: ab0101ef    	adds	x15, x15, x1
1000394dc: 1a9f37e1    	cset	w1, hs
1000394e0: eb0f0273    	subs	x19, x19, x15
1000394e4: da010294    	sbc	x20, x20, x1
1000394e8: a91353f3    	stp	x19, x20, [sp, #0x130]
1000394ec: 8a0c030f    	and	x15, x24, x12
1000394f0: a9436ae1    	ldp	x1, x26, [x23, #0x30]
1000394f4: 8a000021    	and	x1, x1, x0
1000394f8: a95463f7    	ldp	x23, x24, [sp, #0x140]
1000394fc: ab0101ef    	adds	x15, x15, x1
100039500: 1a9f37e1    	cset	w1, hs
100039504: eb0f02f7    	subs	x23, x23, x15
100039508: da010318    	sbc	x24, x24, x1
10003950c: a91463f7    	stp	x23, x24, [sp, #0x140]
100039510: 8a0c032f    	and	x15, x25, x12
100039514: 8a000341    	and	x1, x26, x0
100039518: a9556bf9    	ldp	x25, x26, [sp, #0x150]
10003951c: ab0101ef    	adds	x15, x15, x1
100039520: 1a9f37e1    	cset	w1, hs
100039524: eb0f0339    	subs	x25, x25, x15
100039528: da01035a    	sbc	x26, x26, x1
10003952c: a9156bf9    	stp	x25, x26, [sp, #0x150]
100039530: 8a0c036c    	and	x12, x27, x12
100039534: 8a00038f    	and	x15, x28, x0
100039538: a95603e1    	ldp	x1, x0, [sp, #0x160]
10003953c: ab0f018c    	adds	x12, x12, x15
100039540: 1a9f37ef    	cset	w15, hs
100039544: eb0c003b    	subs	x27, x1, x12
100039548: da0f001c    	sbc	x28, x0, x15
10003954c: a91673fb    	stp	x27, x28, [sp, #0x160]
100039550: 91000631    	add	x17, x17, #0x1
100039554: 8a1e01cc    	and	x12, x14, x30
100039558: a9573bef    	ldp	x15, x14, [sp, #0x170]
10003955c: ab0c01fe    	adds	x30, x15, x12
100039560: 9a8e35cc    	cinc	x12, x14, hs
100039564: a91733fe    	stp	x30, x12, [sp, #0x170]
100039568: 91012210    	add	x16, x16, #0x48
10003956c: eb02023f    	cmp	x17, x2
100039570: 54ffe841    	b.ne	0x100039278 <__RINvNtNtNtCsfDw8YqMpZPC_17field_regressions8campaign10candidates5words20exact_signed_columnsKj9_Kj13_EB8_+0x78>
100039574: a94543ec    	ldp	x12, x16, [sp, #0x50]
100039578: f90023ec    	str	x12, [sp, #0x40]
10003957c: a94647e1    	ldp	x1, x17, [sp, #0x60]
100039580: a9473be0    	ldp	x0, x14, [sp, #0x70]
100039584: a94833ef    	ldp	x15, x12, [sp, #0x80]
100039588: f9404fed    	ldr	x13, [sp, #0x98]
10003958c: f90007ed    	str	x13, [sp, #0x8]
100039590: f9404bed    	ldr	x13, [sp, #0x90]
100039594: a94a0fe2    	ldp	x2, x3, [sp, #0xa0]
100039598: f90003e2    	str	x2, [sp]
10003959c: f9405fe2    	ldr	x2, [sp, #0xb8]
1000395a0: f90017e2    	str	x2, [sp, #0x28]
1000395a4: f9405be2    	ldr	x2, [sp, #0xb0]
1000395a8: a9010fe2    	stp	x2, x3, [sp, #0x10]
1000395ac: a94c0fe2    	ldp	x2, x3, [sp, #0xc0]
1000395b0: f90013e2    	str	x2, [sp, #0x20]
1000395b4: f9406fe2    	ldr	x2, [sp, #0xd8]
1000395b8: f9001fe2    	str	x2, [sp, #0x38]
1000395bc: aa0303e2    	mov	x2, x3
1000395c0: f9406be3    	ldr	x3, [sp, #0xd0]
1000395c4: 14000021    	b	0x100039648 <__RINvNtNtNtCsfDw8YqMpZPC_17field_regressions8campaign10candidates5words20exact_signed_columnsKj9_Kj13_EB8_+0x448>
1000395c8: d280001e    	mov	x30, #0x0               ; =0
1000395cc: d280001b    	mov	x27, #0x0               ; =0
1000395d0: d280001c    	mov	x28, #0x0               ; =0
1000395d4: d2800019    	mov	x25, #0x0               ; =0
1000395d8: d280001a    	mov	x26, #0x0               ; =0
1000395dc: d2800017    	mov	x23, #0x0               ; =0
1000395e0: d2800018    	mov	x24, #0x0               ; =0
1000395e4: d2800013    	mov	x19, #0x0               ; =0
1000395e8: d2800014    	mov	x20, #0x0               ; =0
1000395ec: d2800015    	mov	x21, #0x0               ; =0
1000395f0: d2800016    	mov	x22, #0x0               ; =0
1000395f4: d2800006    	mov	x6, #0x0                ; =0
1000395f8: d2800007    	mov	x7, #0x0                ; =0
1000395fc: d2800004    	mov	x4, #0x0                ; =0
100039600: d2800005    	mov	x5, #0x0                ; =0
100039604: d280000a    	mov	x10, #0x0               ; =0
100039608: d280000b    	mov	x11, #0x0               ; =0
10003960c: d2800008    	mov	x8, #0x0                ; =0
100039610: d2800009    	mov	x9, #0x0                ; =0
100039614: d2800003    	mov	x3, #0x0                ; =0
100039618: a903ffff    	stp	xzr, xzr, [sp, #0x38]
10003961c: a9027fff    	stp	xzr, xzr, [sp, #0x20]
100039620: a9017fff    	stp	xzr, xzr, [sp, #0x10]
100039624: a9007fff    	stp	xzr, xzr, [sp]
100039628: d280000d    	mov	x13, #0x0               ; =0
10003962c: d280000f    	mov	x15, #0x0               ; =0
100039630: d280000c    	mov	x12, #0x0               ; =0
100039634: d2800000    	mov	x0, #0x0                ; =0
100039638: d280000e    	mov	x14, #0x0               ; =0
10003963c: d2800001    	mov	x1, #0x0                ; =0
100039640: d2800011    	mov	x17, #0x0               ; =0
100039644: d2800010    	mov	x16, #0x0               ; =0
100039648: ab010201    	adds	x1, x16, x1
10003964c: 937ffe10    	asr	x16, x16, #63
100039650: 9a110211    	adc	x17, x16, x17
100039654: ab000230    	adds	x16, x17, x0
100039658: 937ffe31    	asr	x17, x17, #63
10003965c: 9a0e0231    	adc	x17, x17, x14
100039660: ab0f022e    	adds	x14, x17, x15
100039664: 937ffe2f    	asr	x15, x17, #63
100039668: 9a0c01ef    	adc	x15, x15, x12
10003966c: ab0d01ec    	adds	x12, x15, x13
100039670: 937ffded    	asr	x13, x15, #63
100039674: f94007ef    	ldr	x15, [sp, #0x8]
100039678: 9a0f01ad    	adc	x13, x13, x15
10003967c: f94003ef    	ldr	x15, [sp]
100039680: ab0f01af    	adds	x15, x13, x15
100039684: 937ffdad    	asr	x13, x13, #63
100039688: f9400ff1    	ldr	x17, [sp, #0x18]
10003968c: 9a1101ad    	adc	x13, x13, x17
100039690: f9400bf1    	ldr	x17, [sp, #0x10]
100039694: ab1101b1    	adds	x17, x13, x17
100039698: 937ffdad    	asr	x13, x13, #63
10003969c: f94017e0    	ldr	x0, [sp, #0x28]
1000396a0: 9a0001ad    	adc	x13, x13, x0
1000396a4: f94013e0    	ldr	x0, [sp, #0x20]
1000396a8: ab0001a0    	adds	x0, x13, x0
1000396ac: 937ffdad    	asr	x13, x13, #63
1000396b0: 9a0201ad    	adc	x13, x13, x2
1000396b4: ab0301a2    	adds	x2, x13, x3
1000396b8: 937ffdad    	asr	x13, x13, #63
1000396bc: f9401fe3    	ldr	x3, [sp, #0x38]
1000396c0: 9a0301ad    	adc	x13, x13, x3
1000396c4: ab0801a8    	adds	x8, x13, x8
1000396c8: 937ffdad    	asr	x13, x13, #63
1000396cc: 9a0901a9    	adc	x9, x13, x9
1000396d0: ab0a012a    	adds	x10, x9, x10
1000396d4: 937ffd29    	asr	x9, x9, #63
1000396d8: 9a0b0129    	adc	x9, x9, x11
1000396dc: ab04012b    	adds	x11, x9, x4
1000396e0: 937ffd29    	asr	x9, x9, #63
1000396e4: 9a050129    	adc	x9, x9, x5
1000396e8: ab06012d    	adds	x13, x9, x6
1000396ec: 937ffd29    	asr	x9, x9, #63
1000396f0: 9a070129    	adc	x9, x9, x7
1000396f4: ab150123    	adds	x3, x9, x21
1000396f8: 937ffd29    	asr	x9, x9, #63
1000396fc: 9a160129    	adc	x9, x9, x22
100039700: f9401be4    	ldr	x4, [sp, #0x30]
100039704: f94023e5    	ldr	x5, [sp, #0x40]
100039708: a9000485    	stp	x5, x1, [x4]
10003970c: ab130121    	adds	x1, x9, x19
100039710: 937ffd29    	asr	x9, x9, #63
100039714: 9a140129    	adc	x9, x9, x20
100039718: a9013890    	stp	x16, x14, [x4, #0x10]
10003971c: a9023c8c    	stp	x12, x15, [x4, #0x20]
100039720: ab17012c    	adds	x12, x9, x23
100039724: 937ffd29    	asr	x9, x9, #63
100039728: 9a180129    	adc	x9, x9, x24
10003972c: a9030091    	stp	x17, x0, [x4, #0x30]
100039730: a9042082    	stp	x2, x8, [x4, #0x40]
100039734: ab190128    	adds	x8, x9, x25
100039738: 937ffd29    	asr	x9, x9, #63
10003973c: 9a1a0129    	adc	x9, x9, x26
100039740: a9052c8a    	stp	x10, x11, [x4, #0x50]
100039744: a9060c8d    	stp	x13, x3, [x4, #0x60]
100039748: ab1b012a    	adds	x10, x9, x27
10003974c: 937ffd29    	asr	x9, x9, #63
100039750: 9a1c0129    	adc	x9, x9, x28
100039754: a9073081    	stp	x1, x12, [x4, #0x70]
100039758: 8b1e0129    	add	x9, x9, x30
10003975c: a9082888    	stp	x8, x10, [x4, #0x80]
100039760: f9004889    	str	x9, [x4, #0x90]
100039764: 910683ff    	add	sp, sp, #0x1a0
100039768: a9457bfd    	ldp	x29, x30, [sp, #0x50]
10003976c: a9444ff4    	ldp	x20, x19, [sp, #0x40]
100039770: a94357f6    	ldp	x22, x21, [sp, #0x30]
100039774: a9425ff8    	ldp	x24, x23, [sp, #0x20]
100039778: a94167fa    	ldp	x26, x25, [sp, #0x10]
10003977c: a8c66ffc    	ldp	x28, x27, [sp], #0x60
100039780: d65f03c0    	ret
100039784: d0000ba4    	adrp	x4, 0x1001af000 <dyld_stub_binder+0x1001af000>
100039788: 91376084    	add	x4, x4, #0xdd8
10003978c: 910123e0    	add	x0, sp, #0x48
100039790: 910143e1    	add	x1, sp, #0x50
100039794: d2800002    	mov	x2, #0x0                ; =0
100039798: 94048c4b    	bl	0x10015c8c4 <__RINvNtCs8Mbv00yxnRz_4core9panicking13assert_failedjjEB4_>

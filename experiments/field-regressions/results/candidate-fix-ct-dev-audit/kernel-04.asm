
/private/tmp/f2z-arithmetic-target/release/field-regressions:	file format mach-o arm64

Disassembly of section __TEXT,__text:

00000001000385b4 <__RINvNtNtNtCshgIkoLp8A2V_17field_regressions8campaign10candidates5words20exact_signed_columnsKj2_Kj5_EB8_>:
1000385b4: d10183ff    	sub	sp, sp, #0x60
1000385b8: a90167fa    	stp	x26, x25, [sp, #0x10]
1000385bc: a9025ff8    	stp	x24, x23, [sp, #0x20]
1000385c0: a90357f6    	stp	x22, x21, [sp, #0x30]
1000385c4: a9044ff4    	stp	x20, x19, [sp, #0x40]
1000385c8: a9057bfd    	stp	x29, x30, [sp, #0x50]
1000385cc: 910143fd    	add	x29, sp, #0x50
1000385d0: a90013e2    	stp	x2, x4, [sp]
1000385d4: eb04005f    	cmp	x2, x4
1000385d8: 54000d01    	b.ne	0x100038778 <__RINvNtNtNtCshgIkoLp8A2V_17field_regressions8campaign10candidates5words20exact_signed_columnsKj2_Kj5_EB8_+0x1c4>
1000385dc: b4000942    	cbz	x2, 0x100038704 <__RINvNtNtNtCshgIkoLp8A2V_17field_regressions8campaign10candidates5words20exact_signed_columnsKj2_Kj5_EB8_+0x150>
1000385e0: d280000b    	mov	x11, #0x0               ; =0
1000385e4: 9100202c    	add	x12, x1, #0x8
1000385e8: 9100206d    	add	x13, x3, #0x8
1000385ec: 910023ee    	add	x14, sp, #0x8
1000385f0: 92800011    	mov	x17, #-0x1              ; =-1
1000385f4: d2800008    	mov	x8, #0x0                ; =0
1000385f8: d2800009    	mov	x9, #0x0                ; =0
1000385fc: d280000f    	mov	x15, #0x0               ; =0
100038600: d2800010    	mov	x16, #0x0               ; =0
100038604: d2800005    	mov	x5, #0x0                ; =0
100038608: d2800006    	mov	x6, #0x0                ; =0
10003860c: d2800003    	mov	x3, #0x0                ; =0
100038610: d2800004    	mov	x4, #0x0                ; =0
100038614: d280000a    	mov	x10, #0x0               ; =0
100038618: d2800001    	mov	x1, #0x0                ; =0
10003861c: a97fcda7    	ldp	x7, x19, [x13, #-0x8]
100038620: a97fd594    	ldp	x20, x21, [x12, #-0x8]
100038624: 9bd47cf6    	umulh	x22, x7, x20
100038628: 9b147cf7    	mul	x23, x7, x20
10003862c: ab0802e8    	adds	x8, x23, x8
100038630: 9a893529    	cinc	x9, x9, hs
100038634: ab1601ef    	adds	x15, x15, x22
100038638: 9a903610    	cinc	x16, x16, hs
10003863c: 9bd47e76    	umulh	x22, x19, x20
100038640: 9b147e77    	mul	x23, x19, x20
100038644: ab0f02ef    	adds	x15, x23, x15
100038648: 9a903610    	cinc	x16, x16, hs
10003864c: ab1600a5    	adds	x5, x5, x22
100038650: 9a8634c6    	cinc	x6, x6, hs
100038654: 9bd57cf6    	umulh	x22, x7, x21
100038658: 9b157cf7    	mul	x23, x7, x21
10003865c: ab0f02ef    	adds	x15, x23, x15
100038660: 9a903610    	cinc	x16, x16, hs
100038664: ab1600a5    	adds	x5, x5, x22
100038668: 9a8634c6    	cinc	x6, x6, hs
10003866c: 9bd57e76    	umulh	x22, x19, x21
100038670: 9b157e77    	mul	x23, x19, x21
100038674: ab0502e5    	adds	x5, x23, x5
100038678: 9a8634c6    	cinc	x6, x6, hs
10003867c: ab160063    	adds	x3, x3, x22
100038680: 9a843484    	cinc	x4, x4, hs
100038684: d37ffeb6    	lsr	x22, x21, #63
100038688: d37ffe77    	lsr	x23, x19, #63
10003868c: 390023f6    	strb	w22, [sp, #0x8]
100038690: 394023f8    	ldrb	w24, [sp, #0x8]
100038694: aa0b03f9    	mov	x25, x11
100038698: f2401f1f    	tst	x24, #0xff
10003869c: 9a8b1239    	csel	x25, x17, x11, ne
1000386a0: 390023f7    	strb	w23, [sp, #0x8]
1000386a4: 394023f8    	ldrb	w24, [sp, #0x8]
1000386a8: aa0b03fa    	mov	x26, x11
1000386ac: f2401f1f    	tst	x24, #0xff
1000386b0: 9a8b123a    	csel	x26, x17, x11, ne
1000386b4: 8a1900e7    	and	x7, x7, x25
1000386b8: 8a1a0294    	and	x20, x20, x26
1000386bc: ab1400e7    	adds	x7, x7, x20
1000386c0: 1a9f37f4    	cset	w20, hs
1000386c4: eb0700a5    	subs	x5, x5, x7
1000386c8: da1400c6    	sbc	x6, x6, x20
1000386cc: 8a190267    	and	x7, x19, x25
1000386d0: 8a1a02b3    	and	x19, x21, x26
1000386d4: ab1300e7    	adds	x7, x7, x19
1000386d8: 1a9f37f3    	cset	w19, hs
1000386dc: eb070063    	subs	x3, x3, x7
1000386e0: da130084    	sbc	x4, x4, x19
1000386e4: 8a1602e7    	and	x7, x23, x22
1000386e8: ab07014a    	adds	x10, x10, x7
1000386ec: 9a813421    	cinc	x1, x1, hs
1000386f0: 9100418c    	add	x12, x12, #0x10
1000386f4: 910041ad    	add	x13, x13, #0x10
1000386f8: f1000442    	subs	x2, x2, #0x1
1000386fc: 54fff901    	b.ne	0x10003861c <__RINvNtNtNtCshgIkoLp8A2V_17field_regressions8campaign10candidates5words20exact_signed_columnsKj2_Kj5_EB8_+0x68>
100038700: 1400000a    	b	0x100038728 <__RINvNtNtNtCshgIkoLp8A2V_17field_regressions8campaign10candidates5words20exact_signed_columnsKj2_Kj5_EB8_+0x174>
100038704: d2800008    	mov	x8, #0x0                ; =0
100038708: d2800009    	mov	x9, #0x0                ; =0
10003870c: d280000f    	mov	x15, #0x0               ; =0
100038710: d2800010    	mov	x16, #0x0               ; =0
100038714: d2800005    	mov	x5, #0x0                ; =0
100038718: d2800006    	mov	x6, #0x0                ; =0
10003871c: d2800003    	mov	x3, #0x0                ; =0
100038720: d2800004    	mov	x4, #0x0                ; =0
100038724: d280000a    	mov	x10, #0x0               ; =0
100038728: 937ffd2b    	asr	x11, x9, #63
10003872c: ab0f0129    	adds	x9, x9, x15
100038730: 9a10016b    	adc	x11, x11, x16
100038734: 937ffd6c    	asr	x12, x11, #63
100038738: ab05016b    	adds	x11, x11, x5
10003873c: 9a06018c    	adc	x12, x12, x6
100038740: 937ffd8d    	asr	x13, x12, #63
100038744: ab03018c    	adds	x12, x12, x3
100038748: 9a0401ad    	adc	x13, x13, x4
10003874c: a9002408    	stp	x8, x9, [x0]
100038750: 8b0a01a8    	add	x8, x13, x10
100038754: a901300b    	stp	x11, x12, [x0, #0x10]
100038758: f9001008    	str	x8, [x0, #0x20]
10003875c: a9457bfd    	ldp	x29, x30, [sp, #0x50]
100038760: a9444ff4    	ldp	x20, x19, [sp, #0x40]
100038764: a94357f6    	ldp	x22, x21, [sp, #0x30]
100038768: a9425ff8    	ldp	x24, x23, [sp, #0x20]
10003876c: a94167fa    	ldp	x26, x25, [sp, #0x10]
100038770: 910183ff    	add	sp, sp, #0x60
100038774: d65f03c0    	ret
100038778: f0000ba4    	adrp	x4, 0x1001af000 <dyld_stub_binder+0x1001af000>
10003877c: 91376084    	add	x4, x4, #0xdd8
100038780: 910003e0    	mov	x0, sp
100038784: 910023e1    	add	x1, sp, #0x8
100038788: d2800002    	mov	x2, #0x0                ; =0
10003878c: 94048bba    	bl	0x10015b674 <__RINvNtCs8Mbv00yxnRz_4core9panicking13assert_failedjjEB4_>

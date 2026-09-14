
/private/var/folders/q0/v7l8dzd13k7f_q6j80sy03lm0000gn/T/f2z-field-gated-pycl0nhx/field-regressions:	file format mach-o arm64

Disassembly of section __TEXT,__text:

00000001000385b4 <__RINvNtNtNtCsagpXUhzPPpl_17field_regressions8campaign10candidates5words20exact_signed_columnsKj2_Kj5_EB8_>:
1000385b4: d10143ff    	sub	sp, sp, #0x50
1000385b8: a9015ff8    	stp	x24, x23, [sp, #0x10]
1000385bc: a90257f6    	stp	x22, x21, [sp, #0x20]
1000385c0: a9034ff4    	stp	x20, x19, [sp, #0x30]
1000385c4: a9047bfd    	stp	x29, x30, [sp, #0x40]
1000385c8: 910103fd    	add	x29, sp, #0x40
1000385cc: a90013e2    	stp	x2, x4, [sp]
1000385d0: eb04005f    	cmp	x2, x4
1000385d4: 54000b61    	b.ne	0x100038740 <__RINvNtNtNtCsagpXUhzPPpl_17field_regressions8campaign10candidates5words20exact_signed_columnsKj2_Kj5_EB8_+0x18c>
1000385d8: d2800008    	mov	x8, #0x0                ; =0
1000385dc: d2800009    	mov	x9, #0x0                ; =0
1000385e0: d280000b    	mov	x11, #0x0               ; =0
1000385e4: d280000c    	mov	x12, #0x0               ; =0
1000385e8: d2800004    	mov	x4, #0x0                ; =0
1000385ec: d2800005    	mov	x5, #0x0                ; =0
1000385f0: d2800010    	mov	x16, #0x0               ; =0
1000385f4: d2800011    	mov	x17, #0x0               ; =0
1000385f8: d280000a    	mov	x10, #0x0               ; =0
1000385fc: b40007c2    	cbz	x2, 0x1000386f4 <__RINvNtNtNtCsagpXUhzPPpl_17field_regressions8campaign10candidates5words20exact_signed_columnsKj2_Kj5_EB8_+0x140>
100038600: d280000d    	mov	x13, #0x0               ; =0
100038604: 9100202e    	add	x14, x1, #0x8
100038608: 9100206f    	add	x15, x3, #0x8
10003860c: a97f85e6    	ldp	x6, x1, [x15, #-0x8]
100038610: a97f8dc7    	ldp	x7, x3, [x14, #-0x8]
100038614: 9bc77cd3    	umulh	x19, x6, x7
100038618: 9b077cd4    	mul	x20, x6, x7
10003861c: ab080288    	adds	x8, x20, x8
100038620: 9a893529    	cinc	x9, x9, hs
100038624: ab13016b    	adds	x11, x11, x19
100038628: 9a8c358c    	cinc	x12, x12, hs
10003862c: 9bc77c33    	umulh	x19, x1, x7
100038630: 9b077c34    	mul	x20, x1, x7
100038634: ab0b028b    	adds	x11, x20, x11
100038638: 9a8c358c    	cinc	x12, x12, hs
10003863c: ab130084    	adds	x4, x4, x19
100038640: 9a8534a5    	cinc	x5, x5, hs
100038644: 9bc37cd3    	umulh	x19, x6, x3
100038648: 9b037cd4    	mul	x20, x6, x3
10003864c: ab0b028b    	adds	x11, x20, x11
100038650: 9a8c358c    	cinc	x12, x12, hs
100038654: ab130084    	adds	x4, x4, x19
100038658: 9a8534a5    	cinc	x5, x5, hs
10003865c: 9bc37c33    	umulh	x19, x1, x3
100038660: 9b037c34    	mul	x20, x1, x3
100038664: ab040284    	adds	x4, x20, x4
100038668: 9a8534a5    	cinc	x5, x5, hs
10003866c: ab130210    	adds	x16, x16, x19
100038670: 9a913631    	cinc	x17, x17, hs
100038674: 910041ce    	add	x14, x14, #0x10
100038678: 910041ef    	add	x15, x15, #0x10
10003867c: 937ffc33    	asr	x19, x1, #63
100038680: 8a070274    	and	x20, x19, x7
100038684: ab1400c6    	adds	x6, x6, x20
100038688: 1a9f37f4    	cset	w20, hs
10003868c: eb060086    	subs	x6, x4, x6
100038690: da1400b4    	sbc	x20, x5, x20
100038694: 8a030273    	and	x19, x19, x3
100038698: ab130033    	adds	x19, x1, x19
10003869c: 1a9f37f5    	cset	w21, hs
1000386a0: eb130213    	subs	x19, x16, x19
1000386a4: da150235    	sbc	x21, x17, x21
1000386a8: eb070087    	subs	x7, x4, x7
1000386ac: da1f00b6    	sbc	x22, x5, xzr
1000386b0: eb030217    	subs	x23, x16, x3
1000386b4: da1f0238    	sbc	x24, x17, xzr
1000386b8: f241003f    	tst	x1, #0x8000000000000000
1000386bc: 9a870084    	csel	x4, x4, x7, eq
1000386c0: 9a9600a5    	csel	x5, x5, x22, eq
1000386c4: 9a970210    	csel	x16, x16, x23, eq
1000386c8: 9a980231    	csel	x17, x17, x24, eq
1000386cc: f241007f    	tst	x3, #0x8000000000000000
1000386d0: 9a8410c4    	csel	x4, x6, x4, ne
1000386d4: 9a851285    	csel	x5, x20, x5, ne
1000386d8: 9a901270    	csel	x16, x19, x16, ne
1000386dc: 9a9112b1    	csel	x17, x21, x17, ne
1000386e0: 8a030021    	and	x1, x1, x3
1000386e4: ab41fd4a    	adds	x10, x10, x1, lsr #63
1000386e8: 9a8d35ad    	cinc	x13, x13, hs
1000386ec: f1000442    	subs	x2, x2, #0x1
1000386f0: 54fff8e1    	b.ne	0x10003860c <__RINvNtNtNtCsagpXUhzPPpl_17field_regressions8campaign10candidates5words20exact_signed_columnsKj2_Kj5_EB8_+0x58>
1000386f4: 937ffd2d    	asr	x13, x9, #63
1000386f8: ab0b0129    	adds	x9, x9, x11
1000386fc: 9a0c01ab    	adc	x11, x13, x12
100038700: 937ffd6c    	asr	x12, x11, #63
100038704: ab04016b    	adds	x11, x11, x4
100038708: 9a05018c    	adc	x12, x12, x5
10003870c: 937ffd8d    	asr	x13, x12, #63
100038710: ab10018c    	adds	x12, x12, x16
100038714: 9a1101ad    	adc	x13, x13, x17
100038718: a9002408    	stp	x8, x9, [x0]
10003871c: 8b0a01a8    	add	x8, x13, x10
100038720: a901300b    	stp	x11, x12, [x0, #0x10]
100038724: f9001008    	str	x8, [x0, #0x20]
100038728: a9447bfd    	ldp	x29, x30, [sp, #0x40]
10003872c: a9434ff4    	ldp	x20, x19, [sp, #0x30]
100038730: a94257f6    	ldp	x22, x21, [sp, #0x20]
100038734: a9415ff8    	ldp	x24, x23, [sp, #0x10]
100038738: 910143ff    	add	sp, sp, #0x50
10003873c: d65f03c0    	ret
100038740: f0000ba4    	adrp	x4, 0x1001af000 <dyld_stub_binder+0x1001af000>
100038744: 91376084    	add	x4, x4, #0xdd8
100038748: 910003e0    	mov	x0, sp
10003874c: 910023e1    	add	x1, sp, #0x8
100038750: d2800002    	mov	x2, #0x0                ; =0
100038754: 94048bbf    	bl	0x10015b650 <__RINvNtCs8Mbv00yxnRz_4core9panicking13assert_failedjjEB4_>

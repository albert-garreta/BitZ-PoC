
/private/var/folders/q0/v7l8dzd13k7f_q6j80sy03lm0000gn/T/f2z-field-gated-h_tggvge/field-regressions:	file format mach-o arm64

Disassembly of section __TEXT,__text:

00000001000385b8 <__RINvNtNtNtCsfDw8YqMpZPC_17field_regressions8campaign10candidates5words13exact_columnsKj4_Kj9_EB8_>:
1000385b8: d101c3ff    	sub	sp, sp, #0x70
1000385bc: a9016ffc    	stp	x28, x27, [sp, #0x10]
1000385c0: a90267fa    	stp	x26, x25, [sp, #0x20]
1000385c4: a9035ff8    	stp	x24, x23, [sp, #0x30]
1000385c8: a90457f6    	stp	x22, x21, [sp, #0x40]
1000385cc: a9054ff4    	stp	x20, x19, [sp, #0x50]
1000385d0: a9067bfd    	stp	x29, x30, [sp, #0x60]
1000385d4: 910183fd    	add	x29, sp, #0x60
1000385d8: a90013e2    	stp	x2, x4, [sp]
1000385dc: eb04005f    	cmp	x2, x4
1000385e0: 540012a1    	b.ne	0x100038834 <__RINvNtNtNtCsfDw8YqMpZPC_17field_regressions8campaign10candidates5words13exact_columnsKj4_Kj9_EB8_+0x27c>
1000385e4: d280000b    	mov	x11, #0x0               ; =0
1000385e8: d280000c    	mov	x12, #0x0               ; =0
1000385ec: d2800004    	mov	x4, #0x0                ; =0
1000385f0: d2800005    	mov	x5, #0x0                ; =0
1000385f4: d2800013    	mov	x19, #0x0               ; =0
1000385f8: d2800014    	mov	x20, #0x0               ; =0
1000385fc: d2800015    	mov	x21, #0x0               ; =0
100038600: d2800016    	mov	x22, #0x0               ; =0
100038604: d2800006    	mov	x6, #0x0                ; =0
100038608: d2800007    	mov	x7, #0x0                ; =0
10003860c: d280000f    	mov	x15, #0x0               ; =0
100038610: d280000e    	mov	x14, #0x0               ; =0
100038614: d2800009    	mov	x9, #0x0                ; =0
100038618: d2800008    	mov	x8, #0x0                ; =0
10003861c: d280000a    	mov	x10, #0x0               ; =0
100038620: d280000d    	mov	x13, #0x0               ; =0
100038624: b4000d22    	cbz	x2, 0x1000387c8 <__RINvNtNtNtCsfDw8YqMpZPC_17field_regressions8campaign10candidates5words13exact_columnsKj4_Kj9_EB8_+0x210>
100038628: 91004070    	add	x16, x3, #0x10
10003862c: 91004031    	add	x17, x1, #0x10
100038630: a97f0e01    	ldp	x1, x3, [x16, #-0x10]
100038634: a97f6a39    	ldp	x25, x26, [x17, #-0x10]
100038638: 9bd97c37    	umulh	x23, x1, x25
10003863c: 9b197c38    	mul	x24, x1, x25
100038640: ab0a030a    	adds	x10, x24, x10
100038644: 9a8d35ad    	cinc	x13, x13, hs
100038648: ab17016b    	adds	x11, x11, x23
10003864c: 9bd97c77    	umulh	x23, x3, x25
100038650: 9a8c358c    	cinc	x12, x12, hs
100038654: 9b197c78    	mul	x24, x3, x25
100038658: ab0b030b    	adds	x11, x24, x11
10003865c: 9a8c358c    	cinc	x12, x12, hs
100038660: ab170084    	adds	x4, x4, x23
100038664: a8c25e18    	ldp	x24, x23, [x16], #0x20
100038668: 9a8534a5    	cinc	x5, x5, hs
10003866c: 9bd97f1b    	umulh	x27, x24, x25
100038670: 9b197f1c    	mul	x28, x24, x25
100038674: ab040384    	adds	x4, x28, x4
100038678: 9a8534a5    	cinc	x5, x5, hs
10003867c: ab1b0273    	adds	x19, x19, x27
100038680: 9a943694    	cinc	x20, x20, hs
100038684: 9bd97efb    	umulh	x27, x23, x25
100038688: 9b197ef9    	mul	x25, x23, x25
10003868c: ab130333    	adds	x19, x25, x19
100038690: 9a943694    	cinc	x20, x20, hs
100038694: ab1b02b5    	adds	x21, x21, x27
100038698: 9a9636d6    	cinc	x22, x22, hs
10003869c: 9bda7c39    	umulh	x25, x1, x26
1000386a0: 9b1a7c3b    	mul	x27, x1, x26
1000386a4: ab0b036b    	adds	x11, x27, x11
1000386a8: 9a8c358c    	cinc	x12, x12, hs
1000386ac: ab190084    	adds	x4, x4, x25
1000386b0: 9a8534a5    	cinc	x5, x5, hs
1000386b4: 9bda7c79    	umulh	x25, x3, x26
1000386b8: 9b1a7c7b    	mul	x27, x3, x26
1000386bc: ab040364    	adds	x4, x27, x4
1000386c0: 9a8534a5    	cinc	x5, x5, hs
1000386c4: ab190273    	adds	x19, x19, x25
1000386c8: 9a943694    	cinc	x20, x20, hs
1000386cc: 9bda7f19    	umulh	x25, x24, x26
1000386d0: 9b1a7f1b    	mul	x27, x24, x26
1000386d4: ab130373    	adds	x19, x27, x19
1000386d8: 9a943694    	cinc	x20, x20, hs
1000386dc: ab1902b5    	adds	x21, x21, x25
1000386e0: 9a9636d6    	cinc	x22, x22, hs
1000386e4: 9bda7ef9    	umulh	x25, x23, x26
1000386e8: 9b1a7efa    	mul	x26, x23, x26
1000386ec: ab150355    	adds	x21, x26, x21
1000386f0: 9a9636d6    	cinc	x22, x22, hs
1000386f4: ab1900c6    	adds	x6, x6, x25
1000386f8: 9a8734e7    	cinc	x7, x7, hs
1000386fc: a8c26a39    	ldp	x25, x26, [x17], #0x20
100038700: 9bd97c3b    	umulh	x27, x1, x25
100038704: 9b197c3c    	mul	x28, x1, x25
100038708: ab040384    	adds	x4, x28, x4
10003870c: 9a8534a5    	cinc	x5, x5, hs
100038710: ab1b0273    	adds	x19, x19, x27
100038714: 9a943694    	cinc	x20, x20, hs
100038718: 9bd97c7b    	umulh	x27, x3, x25
10003871c: 9b197c7c    	mul	x28, x3, x25
100038720: ab130393    	adds	x19, x28, x19
100038724: 9a943694    	cinc	x20, x20, hs
100038728: ab1b02b5    	adds	x21, x21, x27
10003872c: 9a9636d6    	cinc	x22, x22, hs
100038730: 9bd97f1b    	umulh	x27, x24, x25
100038734: 9b197f1c    	mul	x28, x24, x25
100038738: ab150395    	adds	x21, x28, x21
10003873c: 9a9636d6    	cinc	x22, x22, hs
100038740: ab1b00c6    	adds	x6, x6, x27
100038744: 9a8734e7    	cinc	x7, x7, hs
100038748: 9bd97efb    	umulh	x27, x23, x25
10003874c: 9b197ef9    	mul	x25, x23, x25
100038750: ab060326    	adds	x6, x25, x6
100038754: 9a8734e7    	cinc	x7, x7, hs
100038758: ab1b01ef    	adds	x15, x15, x27
10003875c: 9a8e35ce    	cinc	x14, x14, hs
100038760: 9bda7c39    	umulh	x25, x1, x26
100038764: 9b1a7c21    	mul	x1, x1, x26
100038768: ab130033    	adds	x19, x1, x19
10003876c: 9a943694    	cinc	x20, x20, hs
100038770: ab1902a1    	adds	x1, x21, x25
100038774: 9a9636d6    	cinc	x22, x22, hs
100038778: 9bda7c79    	umulh	x25, x3, x26
10003877c: 9b1a7c63    	mul	x3, x3, x26
100038780: ab010075    	adds	x21, x3, x1
100038784: 9a9636d6    	cinc	x22, x22, hs
100038788: ab1900c1    	adds	x1, x6, x25
10003878c: 9a8734e3    	cinc	x3, x7, hs
100038790: 9bda7f19    	umulh	x25, x24, x26
100038794: 9b1a7f06    	mul	x6, x24, x26
100038798: ab0100c6    	adds	x6, x6, x1
10003879c: 9a833467    	cinc	x7, x3, hs
1000387a0: ab1901ef    	adds	x15, x15, x25
1000387a4: 9a8e35ce    	cinc	x14, x14, hs
1000387a8: 9bda7ee1    	umulh	x1, x23, x26
1000387ac: 9b1a7ee3    	mul	x3, x23, x26
1000387b0: ab0f006f    	adds	x15, x3, x15
1000387b4: 9a8e35ce    	cinc	x14, x14, hs
1000387b8: ab010129    	adds	x9, x9, x1
1000387bc: 9a883508    	cinc	x8, x8, hs
1000387c0: f1000442    	subs	x2, x2, #0x1
1000387c4: 54fff361    	b.ne	0x100038630 <__RINvNtNtNtCsfDw8YqMpZPC_17field_regressions8campaign10candidates5words13exact_columnsKj4_Kj9_EB8_+0x78>
1000387c8: ab0b01ab    	adds	x11, x13, x11
1000387cc: 9a8c358c    	cinc	x12, x12, hs
1000387d0: ab04018c    	adds	x12, x12, x4
1000387d4: 9a8534ad    	cinc	x13, x5, hs
1000387d8: ab1301ad    	adds	x13, x13, x19
1000387dc: 9a943690    	cinc	x16, x20, hs
1000387e0: ab150210    	adds	x16, x16, x21
1000387e4: 9a9636d1    	cinc	x17, x22, hs
1000387e8: ab060231    	adds	x17, x17, x6
1000387ec: 9a8734e1    	cinc	x1, x7, hs
1000387f0: ab0f002f    	adds	x15, x1, x15
1000387f4: a9002c0a    	stp	x10, x11, [x0]
1000387f8: 9a8e35ca    	cinc	x10, x14, hs
1000387fc: a901340c    	stp	x12, x13, [x0, #0x10]
100038800: ab090149    	adds	x9, x10, x9
100038804: a9024410    	stp	x16, x17, [x0, #0x20]
100038808: 9a883508    	cinc	x8, x8, hs
10003880c: a903240f    	stp	x15, x9, [x0, #0x30]
100038810: f9002008    	str	x8, [x0, #0x40]
100038814: a9467bfd    	ldp	x29, x30, [sp, #0x60]
100038818: a9454ff4    	ldp	x20, x19, [sp, #0x50]
10003881c: a94457f6    	ldp	x22, x21, [sp, #0x40]
100038820: a9435ff8    	ldp	x24, x23, [sp, #0x30]
100038824: a94267fa    	ldp	x26, x25, [sp, #0x20]
100038828: a9416ffc    	ldp	x28, x27, [sp, #0x10]
10003882c: 9101c3ff    	add	sp, sp, #0x70
100038830: d65f03c0    	ret
100038834: f0000ba4    	adrp	x4, 0x1001af000 <dyld_stub_binder+0x1001af000>
100038838: 91370084    	add	x4, x4, #0xdc0
10003883c: 910003e0    	mov	x0, sp
100038840: 910023e1    	add	x1, sp, #0x8
100038844: d2800002    	mov	x2, #0x0                ; =0
100038848: 9404901f    	bl	0x10015c8c4 <__RINvNtCs8Mbv00yxnRz_4core9panicking13assert_failedjjEB4_>


/private/var/folders/q0/v7l8dzd13k7f_q6j80sy03lm0000gn/T/f2z-field-gated-aejimxw9/field-regressions:	file format mach-o arm64

Disassembly of section __TEXT,__text:

0000000100037108 <__RINvNtNtNtCsaHyC9lX8wyC_17field_regressions8campaign10candidates5words13exact_columnsKj4_Kj9_EB8_>:
100037108: d101c3ff    	sub	sp, sp, #0x70
10003710c: a9016ffc    	stp	x28, x27, [sp, #0x10]
100037110: a90267fa    	stp	x26, x25, [sp, #0x20]
100037114: a9035ff8    	stp	x24, x23, [sp, #0x30]
100037118: a90457f6    	stp	x22, x21, [sp, #0x40]
10003711c: a9054ff4    	stp	x20, x19, [sp, #0x50]
100037120: a9067bfd    	stp	x29, x30, [sp, #0x60]
100037124: 910183fd    	add	x29, sp, #0x60
100037128: a90013e2    	stp	x2, x4, [sp]
10003712c: eb04005f    	cmp	x2, x4
100037130: 540012a1    	b.ne	0x100037384 <__RINvNtNtNtCsaHyC9lX8wyC_17field_regressions8campaign10candidates5words13exact_columnsKj4_Kj9_EB8_+0x27c>
100037134: d280000b    	mov	x11, #0x0               ; =0
100037138: d280000c    	mov	x12, #0x0               ; =0
10003713c: d2800004    	mov	x4, #0x0                ; =0
100037140: d2800005    	mov	x5, #0x0                ; =0
100037144: d2800013    	mov	x19, #0x0               ; =0
100037148: d2800014    	mov	x20, #0x0               ; =0
10003714c: d2800015    	mov	x21, #0x0               ; =0
100037150: d2800016    	mov	x22, #0x0               ; =0
100037154: d2800006    	mov	x6, #0x0                ; =0
100037158: d2800007    	mov	x7, #0x0                ; =0
10003715c: d280000f    	mov	x15, #0x0               ; =0
100037160: d280000e    	mov	x14, #0x0               ; =0
100037164: d2800009    	mov	x9, #0x0                ; =0
100037168: d2800008    	mov	x8, #0x0                ; =0
10003716c: d280000a    	mov	x10, #0x0               ; =0
100037170: d280000d    	mov	x13, #0x0               ; =0
100037174: b4000d22    	cbz	x2, 0x100037318 <__RINvNtNtNtCsaHyC9lX8wyC_17field_regressions8campaign10candidates5words13exact_columnsKj4_Kj9_EB8_+0x210>
100037178: 91004070    	add	x16, x3, #0x10
10003717c: 91004031    	add	x17, x1, #0x10
100037180: a97f0e01    	ldp	x1, x3, [x16, #-0x10]
100037184: a97f6a39    	ldp	x25, x26, [x17, #-0x10]
100037188: 9bd97c37    	umulh	x23, x1, x25
10003718c: 9b197c38    	mul	x24, x1, x25
100037190: ab0a030a    	adds	x10, x24, x10
100037194: 9a8d35ad    	cinc	x13, x13, hs
100037198: ab17016b    	adds	x11, x11, x23
10003719c: 9bd97c77    	umulh	x23, x3, x25
1000371a0: 9a8c358c    	cinc	x12, x12, hs
1000371a4: 9b197c78    	mul	x24, x3, x25
1000371a8: ab0b030b    	adds	x11, x24, x11
1000371ac: 9a8c358c    	cinc	x12, x12, hs
1000371b0: ab170084    	adds	x4, x4, x23
1000371b4: a8c25e18    	ldp	x24, x23, [x16], #0x20
1000371b8: 9a8534a5    	cinc	x5, x5, hs
1000371bc: 9bd97f1b    	umulh	x27, x24, x25
1000371c0: 9b197f1c    	mul	x28, x24, x25
1000371c4: ab040384    	adds	x4, x28, x4
1000371c8: 9a8534a5    	cinc	x5, x5, hs
1000371cc: ab1b0273    	adds	x19, x19, x27
1000371d0: 9a943694    	cinc	x20, x20, hs
1000371d4: 9bd97efb    	umulh	x27, x23, x25
1000371d8: 9b197ef9    	mul	x25, x23, x25
1000371dc: ab130333    	adds	x19, x25, x19
1000371e0: 9a943694    	cinc	x20, x20, hs
1000371e4: ab1b02b5    	adds	x21, x21, x27
1000371e8: 9a9636d6    	cinc	x22, x22, hs
1000371ec: 9bda7c39    	umulh	x25, x1, x26
1000371f0: 9b1a7c3b    	mul	x27, x1, x26
1000371f4: ab0b036b    	adds	x11, x27, x11
1000371f8: 9a8c358c    	cinc	x12, x12, hs
1000371fc: ab190084    	adds	x4, x4, x25
100037200: 9a8534a5    	cinc	x5, x5, hs
100037204: 9bda7c79    	umulh	x25, x3, x26
100037208: 9b1a7c7b    	mul	x27, x3, x26
10003720c: ab040364    	adds	x4, x27, x4
100037210: 9a8534a5    	cinc	x5, x5, hs
100037214: ab190273    	adds	x19, x19, x25
100037218: 9a943694    	cinc	x20, x20, hs
10003721c: 9bda7f19    	umulh	x25, x24, x26
100037220: 9b1a7f1b    	mul	x27, x24, x26
100037224: ab130373    	adds	x19, x27, x19
100037228: 9a943694    	cinc	x20, x20, hs
10003722c: ab1902b5    	adds	x21, x21, x25
100037230: 9a9636d6    	cinc	x22, x22, hs
100037234: 9bda7ef9    	umulh	x25, x23, x26
100037238: 9b1a7efa    	mul	x26, x23, x26
10003723c: ab150355    	adds	x21, x26, x21
100037240: 9a9636d6    	cinc	x22, x22, hs
100037244: ab1900c6    	adds	x6, x6, x25
100037248: 9a8734e7    	cinc	x7, x7, hs
10003724c: a8c26a39    	ldp	x25, x26, [x17], #0x20
100037250: 9bd97c3b    	umulh	x27, x1, x25
100037254: 9b197c3c    	mul	x28, x1, x25
100037258: ab040384    	adds	x4, x28, x4
10003725c: 9a8534a5    	cinc	x5, x5, hs
100037260: ab1b0273    	adds	x19, x19, x27
100037264: 9a943694    	cinc	x20, x20, hs
100037268: 9bd97c7b    	umulh	x27, x3, x25
10003726c: 9b197c7c    	mul	x28, x3, x25
100037270: ab130393    	adds	x19, x28, x19
100037274: 9a943694    	cinc	x20, x20, hs
100037278: ab1b02b5    	adds	x21, x21, x27
10003727c: 9a9636d6    	cinc	x22, x22, hs
100037280: 9bd97f1b    	umulh	x27, x24, x25
100037284: 9b197f1c    	mul	x28, x24, x25
100037288: ab150395    	adds	x21, x28, x21
10003728c: 9a9636d6    	cinc	x22, x22, hs
100037290: ab1b00c6    	adds	x6, x6, x27
100037294: 9a8734e7    	cinc	x7, x7, hs
100037298: 9bd97efb    	umulh	x27, x23, x25
10003729c: 9b197ef9    	mul	x25, x23, x25
1000372a0: ab060326    	adds	x6, x25, x6
1000372a4: 9a8734e7    	cinc	x7, x7, hs
1000372a8: ab1b01ef    	adds	x15, x15, x27
1000372ac: 9a8e35ce    	cinc	x14, x14, hs
1000372b0: 9bda7c39    	umulh	x25, x1, x26
1000372b4: 9b1a7c21    	mul	x1, x1, x26
1000372b8: ab130033    	adds	x19, x1, x19
1000372bc: 9a943694    	cinc	x20, x20, hs
1000372c0: ab1902a1    	adds	x1, x21, x25
1000372c4: 9a9636d6    	cinc	x22, x22, hs
1000372c8: 9bda7c79    	umulh	x25, x3, x26
1000372cc: 9b1a7c63    	mul	x3, x3, x26
1000372d0: ab010075    	adds	x21, x3, x1
1000372d4: 9a9636d6    	cinc	x22, x22, hs
1000372d8: ab1900c1    	adds	x1, x6, x25
1000372dc: 9a8734e3    	cinc	x3, x7, hs
1000372e0: 9bda7f19    	umulh	x25, x24, x26
1000372e4: 9b1a7f06    	mul	x6, x24, x26
1000372e8: ab0100c6    	adds	x6, x6, x1
1000372ec: 9a833467    	cinc	x7, x3, hs
1000372f0: ab1901ef    	adds	x15, x15, x25
1000372f4: 9a8e35ce    	cinc	x14, x14, hs
1000372f8: 9bda7ee1    	umulh	x1, x23, x26
1000372fc: 9b1a7ee3    	mul	x3, x23, x26
100037300: ab0f006f    	adds	x15, x3, x15
100037304: 9a8e35ce    	cinc	x14, x14, hs
100037308: ab010129    	adds	x9, x9, x1
10003730c: 9a883508    	cinc	x8, x8, hs
100037310: f1000442    	subs	x2, x2, #0x1
100037314: 54fff361    	b.ne	0x100037180 <__RINvNtNtNtCsaHyC9lX8wyC_17field_regressions8campaign10candidates5words13exact_columnsKj4_Kj9_EB8_+0x78>
100037318: ab0b01ab    	adds	x11, x13, x11
10003731c: 9a8c358c    	cinc	x12, x12, hs
100037320: ab04018c    	adds	x12, x12, x4
100037324: 9a8534ad    	cinc	x13, x5, hs
100037328: ab1301ad    	adds	x13, x13, x19
10003732c: 9a943690    	cinc	x16, x20, hs
100037330: ab150210    	adds	x16, x16, x21
100037334: 9a9636d1    	cinc	x17, x22, hs
100037338: ab060231    	adds	x17, x17, x6
10003733c: 9a8734e1    	cinc	x1, x7, hs
100037340: ab0f002f    	adds	x15, x1, x15
100037344: a9002c0a    	stp	x10, x11, [x0]
100037348: 9a8e35ca    	cinc	x10, x14, hs
10003734c: a901340c    	stp	x12, x13, [x0, #0x10]
100037350: ab090149    	adds	x9, x10, x9
100037354: a9024410    	stp	x16, x17, [x0, #0x20]
100037358: 9a883508    	cinc	x8, x8, hs
10003735c: a903240f    	stp	x15, x9, [x0, #0x30]
100037360: f9002008    	str	x8, [x0, #0x40]
100037364: a9467bfd    	ldp	x29, x30, [sp, #0x60]
100037368: a9454ff4    	ldp	x20, x19, [sp, #0x50]
10003736c: a94457f6    	ldp	x22, x21, [sp, #0x40]
100037370: a9435ff8    	ldp	x24, x23, [sp, #0x30]
100037374: a94267fa    	ldp	x26, x25, [sp, #0x20]
100037378: a9416ffc    	ldp	x28, x27, [sp, #0x10]
10003737c: 9101c3ff    	add	sp, sp, #0x70
100037380: d65f03c0    	ret
100037384: 90000b24    	adrp	x4, 0x10019b000 <dyld_stub_binder+0x10019b000>
100037388: 912fa084    	add	x4, x4, #0xbe8
10003738c: 910003e0    	mov	x0, sp
100037390: 910023e1    	add	x1, sp, #0x8
100037394: d2800002    	mov	x2, #0x0                ; =0
100037398: 94045174    	bl	0x10014b968 <__RINvNtCs8Mbv00yxnRz_4core9panicking13assert_failedjjEB4_>

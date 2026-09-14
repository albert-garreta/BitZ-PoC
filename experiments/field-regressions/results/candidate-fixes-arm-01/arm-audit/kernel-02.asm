
/private/var/folders/q0/v7l8dzd13k7f_q6j80sy03lm0000gn/T/f2z-field-gated-pycl0nhx/field-regressions:	file format mach-o arm64

Disassembly of section __TEXT,__text:

0000000100037f34 <__RINvNtNtNtCsagpXUhzPPpl_17field_regressions8campaign10candidates5words13exact_columnsKj4_Kj9_EB8_>:
100037f34: d101c3ff    	sub	sp, sp, #0x70
100037f38: a9016ffc    	stp	x28, x27, [sp, #0x10]
100037f3c: a90267fa    	stp	x26, x25, [sp, #0x20]
100037f40: a9035ff8    	stp	x24, x23, [sp, #0x30]
100037f44: a90457f6    	stp	x22, x21, [sp, #0x40]
100037f48: a9054ff4    	stp	x20, x19, [sp, #0x50]
100037f4c: a9067bfd    	stp	x29, x30, [sp, #0x60]
100037f50: 910183fd    	add	x29, sp, #0x60
100037f54: a90013e2    	stp	x2, x4, [sp]
100037f58: eb04005f    	cmp	x2, x4
100037f5c: 540012a1    	b.ne	0x1000381b0 <__RINvNtNtNtCsagpXUhzPPpl_17field_regressions8campaign10candidates5words13exact_columnsKj4_Kj9_EB8_+0x27c>
100037f60: d280000b    	mov	x11, #0x0               ; =0
100037f64: d280000c    	mov	x12, #0x0               ; =0
100037f68: d2800004    	mov	x4, #0x0                ; =0
100037f6c: d2800005    	mov	x5, #0x0                ; =0
100037f70: d2800013    	mov	x19, #0x0               ; =0
100037f74: d2800014    	mov	x20, #0x0               ; =0
100037f78: d2800015    	mov	x21, #0x0               ; =0
100037f7c: d2800016    	mov	x22, #0x0               ; =0
100037f80: d2800006    	mov	x6, #0x0                ; =0
100037f84: d2800007    	mov	x7, #0x0                ; =0
100037f88: d280000f    	mov	x15, #0x0               ; =0
100037f8c: d280000e    	mov	x14, #0x0               ; =0
100037f90: d2800009    	mov	x9, #0x0                ; =0
100037f94: d2800008    	mov	x8, #0x0                ; =0
100037f98: d280000a    	mov	x10, #0x0               ; =0
100037f9c: d280000d    	mov	x13, #0x0               ; =0
100037fa0: b4000d22    	cbz	x2, 0x100038144 <__RINvNtNtNtCsagpXUhzPPpl_17field_regressions8campaign10candidates5words13exact_columnsKj4_Kj9_EB8_+0x210>
100037fa4: 91004070    	add	x16, x3, #0x10
100037fa8: 91004031    	add	x17, x1, #0x10
100037fac: a97f0e01    	ldp	x1, x3, [x16, #-0x10]
100037fb0: a97f6a39    	ldp	x25, x26, [x17, #-0x10]
100037fb4: 9bd97c37    	umulh	x23, x1, x25
100037fb8: 9b197c38    	mul	x24, x1, x25
100037fbc: ab0a030a    	adds	x10, x24, x10
100037fc0: 9a8d35ad    	cinc	x13, x13, hs
100037fc4: ab17016b    	adds	x11, x11, x23
100037fc8: 9bd97c77    	umulh	x23, x3, x25
100037fcc: 9a8c358c    	cinc	x12, x12, hs
100037fd0: 9b197c78    	mul	x24, x3, x25
100037fd4: ab0b030b    	adds	x11, x24, x11
100037fd8: 9a8c358c    	cinc	x12, x12, hs
100037fdc: ab170084    	adds	x4, x4, x23
100037fe0: a8c25e18    	ldp	x24, x23, [x16], #0x20
100037fe4: 9a8534a5    	cinc	x5, x5, hs
100037fe8: 9bd97f1b    	umulh	x27, x24, x25
100037fec: 9b197f1c    	mul	x28, x24, x25
100037ff0: ab040384    	adds	x4, x28, x4
100037ff4: 9a8534a5    	cinc	x5, x5, hs
100037ff8: ab1b0273    	adds	x19, x19, x27
100037ffc: 9a943694    	cinc	x20, x20, hs
100038000: 9bd97efb    	umulh	x27, x23, x25
100038004: 9b197ef9    	mul	x25, x23, x25
100038008: ab130333    	adds	x19, x25, x19
10003800c: 9a943694    	cinc	x20, x20, hs
100038010: ab1b02b5    	adds	x21, x21, x27
100038014: 9a9636d6    	cinc	x22, x22, hs
100038018: 9bda7c39    	umulh	x25, x1, x26
10003801c: 9b1a7c3b    	mul	x27, x1, x26
100038020: ab0b036b    	adds	x11, x27, x11
100038024: 9a8c358c    	cinc	x12, x12, hs
100038028: ab190084    	adds	x4, x4, x25
10003802c: 9a8534a5    	cinc	x5, x5, hs
100038030: 9bda7c79    	umulh	x25, x3, x26
100038034: 9b1a7c7b    	mul	x27, x3, x26
100038038: ab040364    	adds	x4, x27, x4
10003803c: 9a8534a5    	cinc	x5, x5, hs
100038040: ab190273    	adds	x19, x19, x25
100038044: 9a943694    	cinc	x20, x20, hs
100038048: 9bda7f19    	umulh	x25, x24, x26
10003804c: 9b1a7f1b    	mul	x27, x24, x26
100038050: ab130373    	adds	x19, x27, x19
100038054: 9a943694    	cinc	x20, x20, hs
100038058: ab1902b5    	adds	x21, x21, x25
10003805c: 9a9636d6    	cinc	x22, x22, hs
100038060: 9bda7ef9    	umulh	x25, x23, x26
100038064: 9b1a7efa    	mul	x26, x23, x26
100038068: ab150355    	adds	x21, x26, x21
10003806c: 9a9636d6    	cinc	x22, x22, hs
100038070: ab1900c6    	adds	x6, x6, x25
100038074: 9a8734e7    	cinc	x7, x7, hs
100038078: a8c26a39    	ldp	x25, x26, [x17], #0x20
10003807c: 9bd97c3b    	umulh	x27, x1, x25
100038080: 9b197c3c    	mul	x28, x1, x25
100038084: ab040384    	adds	x4, x28, x4
100038088: 9a8534a5    	cinc	x5, x5, hs
10003808c: ab1b0273    	adds	x19, x19, x27
100038090: 9a943694    	cinc	x20, x20, hs
100038094: 9bd97c7b    	umulh	x27, x3, x25
100038098: 9b197c7c    	mul	x28, x3, x25
10003809c: ab130393    	adds	x19, x28, x19
1000380a0: 9a943694    	cinc	x20, x20, hs
1000380a4: ab1b02b5    	adds	x21, x21, x27
1000380a8: 9a9636d6    	cinc	x22, x22, hs
1000380ac: 9bd97f1b    	umulh	x27, x24, x25
1000380b0: 9b197f1c    	mul	x28, x24, x25
1000380b4: ab150395    	adds	x21, x28, x21
1000380b8: 9a9636d6    	cinc	x22, x22, hs
1000380bc: ab1b00c6    	adds	x6, x6, x27
1000380c0: 9a8734e7    	cinc	x7, x7, hs
1000380c4: 9bd97efb    	umulh	x27, x23, x25
1000380c8: 9b197ef9    	mul	x25, x23, x25
1000380cc: ab060326    	adds	x6, x25, x6
1000380d0: 9a8734e7    	cinc	x7, x7, hs
1000380d4: ab1b01ef    	adds	x15, x15, x27
1000380d8: 9a8e35ce    	cinc	x14, x14, hs
1000380dc: 9bda7c39    	umulh	x25, x1, x26
1000380e0: 9b1a7c21    	mul	x1, x1, x26
1000380e4: ab130033    	adds	x19, x1, x19
1000380e8: 9a943694    	cinc	x20, x20, hs
1000380ec: ab1902a1    	adds	x1, x21, x25
1000380f0: 9a9636d6    	cinc	x22, x22, hs
1000380f4: 9bda7c79    	umulh	x25, x3, x26
1000380f8: 9b1a7c63    	mul	x3, x3, x26
1000380fc: ab010075    	adds	x21, x3, x1
100038100: 9a9636d6    	cinc	x22, x22, hs
100038104: ab1900c1    	adds	x1, x6, x25
100038108: 9a8734e3    	cinc	x3, x7, hs
10003810c: 9bda7f19    	umulh	x25, x24, x26
100038110: 9b1a7f06    	mul	x6, x24, x26
100038114: ab0100c6    	adds	x6, x6, x1
100038118: 9a833467    	cinc	x7, x3, hs
10003811c: ab1901ef    	adds	x15, x15, x25
100038120: 9a8e35ce    	cinc	x14, x14, hs
100038124: 9bda7ee1    	umulh	x1, x23, x26
100038128: 9b1a7ee3    	mul	x3, x23, x26
10003812c: ab0f006f    	adds	x15, x3, x15
100038130: 9a8e35ce    	cinc	x14, x14, hs
100038134: ab010129    	adds	x9, x9, x1
100038138: 9a883508    	cinc	x8, x8, hs
10003813c: f1000442    	subs	x2, x2, #0x1
100038140: 54fff361    	b.ne	0x100037fac <__RINvNtNtNtCsagpXUhzPPpl_17field_regressions8campaign10candidates5words13exact_columnsKj4_Kj9_EB8_+0x78>
100038144: ab0b01ab    	adds	x11, x13, x11
100038148: 9a8c358c    	cinc	x12, x12, hs
10003814c: ab04018c    	adds	x12, x12, x4
100038150: 9a8534ad    	cinc	x13, x5, hs
100038154: ab1301ad    	adds	x13, x13, x19
100038158: 9a943690    	cinc	x16, x20, hs
10003815c: ab150210    	adds	x16, x16, x21
100038160: 9a9636d1    	cinc	x17, x22, hs
100038164: ab060231    	adds	x17, x17, x6
100038168: 9a8734e1    	cinc	x1, x7, hs
10003816c: ab0f002f    	adds	x15, x1, x15
100038170: a9002c0a    	stp	x10, x11, [x0]
100038174: 9a8e35ca    	cinc	x10, x14, hs
100038178: a901340c    	stp	x12, x13, [x0, #0x10]
10003817c: ab090149    	adds	x9, x10, x9
100038180: a9024410    	stp	x16, x17, [x0, #0x20]
100038184: 9a883508    	cinc	x8, x8, hs
100038188: a903240f    	stp	x15, x9, [x0, #0x30]
10003818c: f9002008    	str	x8, [x0, #0x40]
100038190: a9467bfd    	ldp	x29, x30, [sp, #0x60]
100038194: a9454ff4    	ldp	x20, x19, [sp, #0x50]
100038198: a94457f6    	ldp	x22, x21, [sp, #0x40]
10003819c: a9435ff8    	ldp	x24, x23, [sp, #0x30]
1000381a0: a94267fa    	ldp	x26, x25, [sp, #0x20]
1000381a4: a9416ffc    	ldp	x28, x27, [sp, #0x10]
1000381a8: 9101c3ff    	add	sp, sp, #0x70
1000381ac: d65f03c0    	ret
1000381b0: f0000ba4    	adrp	x4, 0x1001af000 <dyld_stub_binder+0x1001af000>
1000381b4: 91370084    	add	x4, x4, #0xdc0
1000381b8: 910003e0    	mov	x0, sp
1000381bc: 910023e1    	add	x1, sp, #0x8
1000381c0: d2800002    	mov	x2, #0x0                ; =0
1000381c4: 94048d23    	bl	0x10015b650 <__RINvNtCs8Mbv00yxnRz_4core9panicking13assert_failedjjEB4_>


/private/var/folders/q0/v7l8dzd13k7f_q6j80sy03lm0000gn/T/f2z-field-gated-aejimxw9/field-regressions:	file format mach-o arm64

Disassembly of section __TEXT,__text:

0000000100043500 <__RINvNvNtNtNtCsaHyC9lX8wyC_17field_regressions8campaign10candidates6binary10polynomial5fixedKj3_Kj7_Kja_EBa_>:
100043500: d100c3ff    	sub	sp, sp, #0x30
100043504: 6d0123e9    	stp	d9, d8, [sp, #0x10]
100043508: a9027bfd    	stp	x29, x30, [sp, #0x20]
10004350c: 910083fd    	add	x29, sp, #0x20
100043510: a90013e2    	stp	x2, x4, [sp]
100043514: eb04005f    	cmp	x2, x4
100043518: 54000ca1    	b.ne	0x1000436ac <__RINvNvNtNtNtCsaHyC9lX8wyC_17field_regressions8campaign10candidates6binary10polynomial5fixedKj3_Kj7_Kja_EBa_+0x1ac>
10004351c: d2800009    	mov	x9, #0x0                ; =0
100043520: d2800008    	mov	x8, #0x0                ; =0
100043524: b4000a82    	cbz	x2, 0x100043674 <__RINvNvNtNtNtCsaHyC9lX8wyC_17field_regressions8campaign10candidates6binary10polynomial5fixedKj3_Kj7_Kja_EBa_+0x174>
100043528: 9100402a    	add	x10, x1, #0x10
10004352c: 6f00e400    	movi.2d	v0, #0000000000000000
100043530: 6f00e401    	movi.2d	v1, #0000000000000000
100043534: 6f00e402    	movi.2d	v2, #0000000000000000
100043538: 6f00e407    	movi.2d	v7, #0000000000000000
10004353c: 6d401464    	ldp	d4, d5, [x3]
100043540: 6d7f0d59    	ldp	d25, d3, [x10, #-0x10]
100043544: 0ee4e330    	pmull.1q	v16, v25, v4
100043548: 0ee4e066    	pmull.1q	v6, v3, v4
10004354c: 4ec67a12    	zip2.2d	v18, v16, v6
100043550: 9e66020b    	fmov	x11, d16
100043554: ca090169    	eor	x9, x11, x9
100043558: 0ee5e331    	pmull.1q	v17, v25, v5
10004355c: 4e183e2b    	mov.d	x11, v17[1]
100043560: 4e183cec    	mov.d	x12, v7[1]
100043564: 9e6600ed    	fmov	x13, d7
100043568: 6d411c70    	ldp	d16, d7, [x3, #0x10]
10004356c: 0ef0e334    	pmull.1q	v20, v25, v16
100043570: 0ee7e333    	pmull.1q	v19, v25, v7
100043574: 4ed37a9b    	zip2.2d	v27, v20, v19
100043578: 9e66028e    	fmov	x14, d20
10004357c: ca0d01cd    	eor	x13, x14, x13
100043580: ca0b01ab    	eor	x11, x13, x11
100043584: 6d425476    	ldp	d22, d21, [x3, #0x20]
100043588: 0ef6e338    	pmull.1q	v24, v25, v22
10004358c: 0ef5e337    	pmull.1q	v23, v25, v21
100043590: fd401874    	ldr	d20, [x3, #0x30]
100043594: 0ef4e339    	pmull.1q	v25, v25, v20
100043598: 4e183f2d    	mov.d	x13, v25[1]
10004359c: ca0d018c    	eor	x12, x12, x13
1000435a0: 0ef5e07a    	pmull.1q	v26, v3, v21
1000435a4: 4e183f4d    	mov.d	x13, v26[1]
1000435a8: ca0d018c    	eor	x12, x12, x13
1000435ac: 9e67019c    	fmov	d28, x12
1000435b0: 6e18445c    	mov.d	v28[1], v2[1]
1000435b4: 0ee5e07d    	pmull.1q	v29, v3, v5
1000435b8: 4e181d62    	mov.d	v2[1], x11
1000435bc: 6e1807b1    	mov.d	v17[1], v29[0]
1000435c0: 6e321c42    	eor.16b	v2, v2, v18
1000435c4: 0ef0e072    	pmull.1q	v18, v3, v16
1000435c8: 0ee7e07e    	pmull.1q	v30, v3, v7
1000435cc: 0ef6e07f    	pmull.1q	v31, v3, v22
1000435d0: fc418548    	ldr	d8, [x10], #0x18
1000435d4: 0ee4e109    	pmull.1q	v9, v8, v4
1000435d8: 6e180526    	mov.d	v6[1], v9[0]
1000435dc: ce111844    	eor3.16b	v4, v2, v17, v6
1000435e0: 0ee5e102    	pmull.1q	v2, v8, v5
1000435e4: 6e180713    	mov.d	v19[1], v24[0]
1000435e8: ce1b4c00    	eor3.16b	v0, v0, v27, v19
1000435ec: 4ed27ba5    	zip2.2d	v5, v29, v18
1000435f0: 6e1807d2    	mov.d	v18[1], v30[0]
1000435f4: ce054800    	eor3.16b	v0, v0, v5, v18
1000435f8: 4ec27925    	zip2.2d	v5, v9, v2
1000435fc: 0ef0e106    	pmull.1q	v6, v8, v16
100043600: 6e1804c2    	mov.d	v2[1], v6[0]
100043604: ce050800    	eor3.16b	v0, v0, v5, v2
100043608: 0ee7e102    	pmull.1q	v2, v8, v7
10004360c: 4ed77b05    	zip2.2d	v5, v24, v23
100043610: 6e180737    	mov.d	v23[1], v25[0]
100043614: ce055c21    	eor3.16b	v1, v1, v5, v23
100043618: 4edf7bc5    	zip2.2d	v5, v30, v31
10004361c: 6e18075f    	mov.d	v31[1], v26[0]
100043620: ce057c21    	eor3.16b	v1, v1, v5, v31
100043624: 4ec278c5    	zip2.2d	v5, v6, v2
100043628: 0ef6e106    	pmull.1q	v6, v8, v22
10004362c: 6e1804c2    	mov.d	v2[1], v6[0]
100043630: ce050821    	eor3.16b	v1, v1, v5, v2
100043634: 0ef5e102    	pmull.1q	v2, v8, v21
100043638: 4ec278c5    	zip2.2d	v5, v6, v2
10004363c: 0ef4e106    	pmull.1q	v6, v8, v20
100043640: 6e1804c2    	mov.d	v2[1], v6[0]
100043644: 0ef4e063    	pmull.1q	v3, v3, v20
100043648: 6e231f83    	eor.16b	v3, v28, v3
10004364c: ce050863    	eor3.16b	v3, v3, v5, v2
100043650: 4e183ccb    	mov.d	x11, v6[1]
100043654: ca0b0108    	eor	x8, x8, x11
100043658: 6e034087    	ext.16b	v7, v4, v3, #0x8
10004365c: 4ea41c82    	mov.16b	v2, v4
100043660: 6e184462    	mov.d	v2[1], v3[1]
100043664: 9100e063    	add	x3, x3, #0x38
100043668: f1000442    	subs	x2, x2, #0x1
10004366c: 54fff681    	b.ne	0x10004353c <__RINvNvNtNtNtCsaHyC9lX8wyC_17field_regressions8campaign10candidates6binary10polynomial5fixedKj3_Kj7_Kja_EBa_+0x3c>
100043670: 14000005    	b	0x100043684 <__RINvNvNtNtNtCsaHyC9lX8wyC_17field_regressions8campaign10candidates6binary10polynomial5fixedKj3_Kj7_Kja_EBa_+0x184>
100043674: 6f00e404    	movi.2d	v4, #0000000000000000
100043678: 6f00e400    	movi.2d	v0, #0000000000000000
10004367c: 6f00e401    	movi.2d	v1, #0000000000000000
100043680: 6f00e403    	movi.2d	v3, #0000000000000000
100043684: f9000009    	str	x9, [x0]
100043688: 3c808004    	stur	q4, [x0, #0x8]
10004368c: 3c818000    	stur	q0, [x0, #0x18]
100043690: 3c828001    	stur	q1, [x0, #0x28]
100043694: 3c838003    	stur	q3, [x0, #0x38]
100043698: f9002408    	str	x8, [x0, #0x48]
10004369c: a9427bfd    	ldp	x29, x30, [sp, #0x20]
1000436a0: 6d4123e9    	ldp	d9, d8, [sp, #0x10]
1000436a4: 9100c3ff    	add	sp, sp, #0x30
1000436a8: d65f03c0    	ret
1000436ac: b0000ac4    	adrp	x4, 0x10019c000 <dyld_stub_binder+0x10019c000>
1000436b0: 91166084    	add	x4, x4, #0x598
1000436b4: 910003e0    	mov	x0, sp
1000436b8: 910023e1    	add	x1, sp, #0x8
1000436bc: d2800002    	mov	x2, #0x0                ; =0
1000436c0: 940420aa    	bl	0x10014b968 <__RINvNtCs8Mbv00yxnRz_4core9panicking13assert_failedjjEB4_>

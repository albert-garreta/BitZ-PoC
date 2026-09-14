
/private/var/folders/q0/v7l8dzd13k7f_q6j80sy03lm0000gn/T/f2z-field-gated-aejimxw9/field-regressions:	file format mach-o arm64

Disassembly of section __TEXT,__text:

00000001000e8544 <__RNvXs2_NtCs4bPT1zor9nS_10rayon_core3jobINtB5_8StackJobNtNtB7_5latch9SpinLatchNCINvMs4_NtB7_8registryNtB1m_8Registry15in_worker_crossNCINvNtB7_4join12join_contextNCINvNvB2b_4join4calluNCNvNtNtNtCsaHyC9lX8wyC_17field_regressions8campaign10candidates9composite10half_depth0E0NCIB2C_uNCB2X_s_0E0uuE0TuuEE0B4J_ENtB5_3Job7executeB35_>:
1000e8544: d10283ff    	sub	sp, sp, #0xa0
1000e8548: a9065ff8    	stp	x24, x23, [sp, #0x60]
1000e854c: a90757f6    	stp	x22, x21, [sp, #0x70]
1000e8550: a9084ff4    	stp	x20, x19, [sp, #0x80]
1000e8554: a9097bfd    	stp	x29, x30, [sp, #0x90]
1000e8558: 910243fd    	add	x29, sp, #0x90
1000e855c: 3dc00000    	ldr	q0, [x0]
1000e8560: aa0003e8    	mov	x8, x0
1000e8564: f801051f    	str	xzr, [x8], #0x10
1000e8568: 9e660009    	fmov	x9, d0
1000e856c: b40008a9    	cbz	x9, 0x1000e8680 <__RNvXs2_NtCs4bPT1zor9nS_10rayon_core3jobINtB5_8StackJobNtNtB7_5latch9SpinLatchNCINvMs4_NtB7_8registryNtB1m_8Registry15in_worker_crossNCINvNtB7_4join12join_contextNCINvNvB2b_4join4calluNCNvNtNtNtCsaHyC9lX8wyC_17field_regressions8campaign10candidates9composite10half_depth0E0NCIB2C_uNCB2X_s_0E0uuE0TuuEE0B4J_ENtB5_3Job7executeB35_+0x13c>
1000e8570: aa0003f3    	mov	x19, x0
1000e8574: 900005e0    	adrp	x0, 0x1001a4000 <dyld_stub_binder+0x1001a4000>
1000e8578: 91340000    	add	x0, x0, #0xd00
1000e857c: f9400009    	ldr	x9, [x0]
1000e8580: d63f0120    	blr	x9
1000e8584: f9400001    	ldr	x1, [x0]
1000e8588: b40006e1    	cbz	x1, 0x1000e8664 <__RNvXs2_NtCs4bPT1zor9nS_10rayon_core3jobINtB5_8StackJobNtNtB7_5latch9SpinLatchNCINvMs4_NtB7_8registryNtB1m_8Registry15in_worker_crossNCINvNtB7_4join12join_contextNCINvNvB2b_4join4calluNCNvNtNtNtCsaHyC9lX8wyC_17field_regressions8campaign10candidates9composite10half_depth0E0NCIB2C_uNCB2X_s_0E0uuE0TuuEE0B4J_ENtB5_3Job7executeB35_+0x120>
1000e858c: ad410901    	ldp	q1, q2, [x8, #0x20]
1000e8590: ad400d04    	ldp	q4, q3, [x8]
1000e8594: ad0107e3    	stp	q3, q1, [sp, #0x20]
1000e8598: 3dc01101    	ldr	q1, [x8, #0x40]
1000e859c: ad0207e2    	stp	q2, q1, [sp, #0x40]
1000e85a0: ad0013e0    	stp	q0, q4, [sp]
1000e85a4: 910003e0    	mov	x0, sp
1000e85a8: 97fd7449    	bl	0x1000456cc <__RNCINvNtCs4bPT1zor9nS_10rayon_core4join12join_contextNCINvNvB4_4join4calluNCNvNtNtNtCsaHyC9lX8wyC_17field_regressions8campaign10candidates9composite10half_depth0E0NCIBS_uNCB1c_s_0E0uuE0B1k_>
1000e85ac: 52800037    	mov	w23, #0x1               ; =1
1000e85b0: f9403268    	ldr	x8, [x19, #0x60]
1000e85b4: f100091f    	cmp	x8, #0x2
1000e85b8: 54000143    	b.lo	0x1000e85e0 <__RNvXs2_NtCs4bPT1zor9nS_10rayon_core3jobINtB5_8StackJobNtNtB7_5latch9SpinLatchNCINvMs4_NtB7_8registryNtB1m_8Registry15in_worker_crossNCINvNtB7_4join12join_contextNCINvNvB2b_4join4calluNCNvNtNtNtCsaHyC9lX8wyC_17field_regressions8campaign10candidates9composite10half_depth0E0NCIB2C_uNCB2X_s_0E0uuE0TuuEE0B4J_ENtB5_3Job7executeB35_+0x9c>
1000e85bc: a946e276    	ldp	x22, x24, [x19, #0x68]
1000e85c0: f9400308    	ldr	x8, [x24]
1000e85c4: b4000068    	cbz	x8, 0x1000e85d0 <__RNvXs2_NtCs4bPT1zor9nS_10rayon_core3jobINtB5_8StackJobNtNtB7_5latch9SpinLatchNCINvMs4_NtB7_8registryNtB1m_8Registry15in_worker_crossNCINvNtB7_4join12join_contextNCINvNvB2b_4join4calluNCNvNtNtNtCsaHyC9lX8wyC_17field_regressions8campaign10candidates9composite10half_depth0E0NCIB2C_uNCB2X_s_0E0uuE0TuuEE0B4J_ENtB5_3Job7executeB35_+0x8c>
1000e85c8: aa1603e0    	mov	x0, x22
1000e85cc: d63f0100    	blr	x8
1000e85d0: f9400708    	ldr	x8, [x24, #0x8]
1000e85d4: b4000068    	cbz	x8, 0x1000e85e0 <__RNvXs2_NtCs4bPT1zor9nS_10rayon_core3jobINtB5_8StackJobNtNtB7_5latch9SpinLatchNCINvMs4_NtB7_8registryNtB1m_8Registry15in_worker_crossNCINvNtB7_4join12join_contextNCINvNvB2b_4join4calluNCNvNtNtNtCsaHyC9lX8wyC_17field_regressions8campaign10candidates9composite10half_depth0E0NCIB2C_uNCB2X_s_0E0uuE0TuuEE0B4J_ENtB5_3Job7executeB35_+0x9c>
1000e85d8: aa1603e0    	mov	x0, x22
1000e85dc: 9401af81    	bl	0x1001543e0 <dyld_stub_binder+0x1001543e0>
1000e85e0: a9065677    	stp	x23, x21, [x19, #0x60]
1000e85e4: f9003a74    	str	x20, [x19, #0x70]
1000e85e8: 39424275    	ldrb	w21, [x19, #0x90]
1000e85ec: f9403e68    	ldr	x8, [x19, #0x78]
1000e85f0: f9400114    	ldr	x20, [x8]
1000e85f4: 710006bf    	cmp	w21, #0x1
1000e85f8: 540000a1    	b.ne	0x1000e860c <__RNvXs2_NtCs4bPT1zor9nS_10rayon_core3jobINtB5_8StackJobNtNtB7_5latch9SpinLatchNCINvMs4_NtB7_8registryNtB1m_8Registry15in_worker_crossNCINvNtB7_4join12join_contextNCINvNvB2b_4join4calluNCNvNtNtNtCsaHyC9lX8wyC_17field_regressions8campaign10candidates9composite10half_depth0E0NCIB2C_uNCB2X_s_0E0uuE0TuuEE0B4J_ENtB5_3Job7executeB35_+0xc8>
1000e85fc: 52800029    	mov	w9, #0x1                ; =1
1000e8600: f8290289    	ldadd	x9, x9, [x20]
1000e8604: b7f806a9    	tbnz	x9, #0x3f, 0x1000e86d8 <__RNvXs2_NtCs4bPT1zor9nS_10rayon_core3jobINtB5_8StackJobNtNtB7_5latch9SpinLatchNCINvMs4_NtB7_8registryNtB1m_8Registry15in_worker_crossNCINvNtB7_4join12join_contextNCINvNvB2b_4join4calluNCNvNtNtNtCsaHyC9lX8wyC_17field_regressions8campaign10candidates9composite10half_depth0E0NCIB2C_uNCB2X_s_0E0uuE0TuuEE0B4J_ENtB5_3Job7executeB35_+0x194>
1000e8608: f9400114    	ldr	x20, [x8]
1000e860c: f9404661    	ldr	x1, [x19, #0x88]
1000e8610: 91020268    	add	x8, x19, #0x80
1000e8614: 52800069    	mov	w9, #0x3                ; =3
1000e8618: f8e98108    	swpal	x9, x8, [x8]
1000e861c: f100091f    	cmp	x8, #0x2
1000e8620: 54000061    	b.ne	0x1000e862c <__RNvXs2_NtCs4bPT1zor9nS_10rayon_core3jobINtB5_8StackJobNtNtB7_5latch9SpinLatchNCINvMs4_NtB7_8registryNtB1m_8Registry15in_worker_crossNCINvNtB7_4join12join_contextNCINvNvB2b_4join4calluNCNvNtNtNtCsaHyC9lX8wyC_17field_regressions8campaign10candidates9composite10half_depth0E0NCIB2C_uNCB2X_s_0E0uuE0TuuEE0B4J_ENtB5_3Job7executeB35_+0xe8>
1000e8624: 91078280    	add	x0, x20, #0x1e0
1000e8628: 9400af50    	bl	0x100114368 <__RNvMNtCs4bPT1zor9nS_10rayon_core5sleepNtB2_5Sleep20wake_specific_thread>
1000e862c: 36000115    	tbz	w21, #0x0, 0x1000e864c <__RNvXs2_NtCs4bPT1zor9nS_10rayon_core3jobINtB5_8StackJobNtNtB7_5latch9SpinLatchNCINvMs4_NtB7_8registryNtB1m_8Registry15in_worker_crossNCINvNtB7_4join12join_contextNCINvNvB2b_4join4calluNCNvNtNtNtCsaHyC9lX8wyC_17field_regressions8campaign10candidates9composite10half_depth0E0NCIB2C_uNCB2X_s_0E0uuE0TuuEE0B4J_ENtB5_3Job7executeB35_+0x108>
1000e8630: 92800008    	mov	x8, #-0x1               ; =-1
1000e8634: f8680288    	ldaddl	x8, x8, [x20]
1000e8638: f100051f    	cmp	x8, #0x1
1000e863c: 54000081    	b.ne	0x1000e864c <__RNvXs2_NtCs4bPT1zor9nS_10rayon_core3jobINtB5_8StackJobNtNtB7_5latch9SpinLatchNCINvMs4_NtB7_8registryNtB1m_8Registry15in_worker_crossNCINvNtB7_4join12join_contextNCINvNvB2b_4join4calluNCNvNtNtNtCsaHyC9lX8wyC_17field_regressions8campaign10candidates9composite10half_depth0E0NCIB2C_uNCB2X_s_0E0uuE0TuuEE0B4J_ENtB5_3Job7executeB35_+0x108>
1000e8640: d50339bf    	dmb	ishld
1000e8644: aa1403e0    	mov	x0, x20
1000e8648: 9400b109    	bl	0x100114a6c <__RNvMsn_NtCshxvaOLs88l5_5alloc4syncINtB5_3ArcNtNtCs4bPT1zor9nS_10rayon_core8registry8RegistryE9drop_slowBK_>
1000e864c: a9497bfd    	ldp	x29, x30, [sp, #0x90]
1000e8650: a9484ff4    	ldp	x20, x19, [sp, #0x80]
1000e8654: a94757f6    	ldp	x22, x21, [sp, #0x70]
1000e8658: a9465ff8    	ldp	x24, x23, [sp, #0x60]
1000e865c: 910283ff    	add	sp, sp, #0xa0
1000e8660: d65f03c0    	ret
1000e8664: f0000380    	adrp	x0, 0x10015b000 <__RNvNtNtNtNtNtCs8Mbv00yxnRz_4core3num3imp7flt2dec8strategy6dragon9POW5TO256+0x2e4>
1000e8668: 91008000    	add	x0, x0, #0x20
1000e866c: 900005c2    	adrp	x2, 0x1001a0000 <dyld_stub_binder+0x1001a0000>
1000e8670: 911cc042    	add	x2, x2, #0x730
1000e8674: 528006c1    	mov	w1, #0x36               ; =54
1000e8678: 94018d00    	bl	0x10014ba78 <__RNvNtCs8Mbv00yxnRz_4core9panicking5panic>
1000e867c: 14000017    	b	0x1000e86d8 <__RNvXs2_NtCs4bPT1zor9nS_10rayon_core3jobINtB5_8StackJobNtNtB7_5latch9SpinLatchNCINvMs4_NtB7_8registryNtB1m_8Registry15in_worker_crossNCINvNtB7_4join12join_contextNCINvNvB2b_4join4calluNCNvNtNtNtCsaHyC9lX8wyC_17field_regressions8campaign10candidates9composite10half_depth0E0NCIB2C_uNCB2X_s_0E0uuE0TuuEE0B4J_ENtB5_3Job7executeB35_+0x194>
1000e8680: 900005c0    	adrp	x0, 0x1001a0000 <dyld_stub_binder+0x1001a0000>
1000e8684: 91338000    	add	x0, x0, #0xce0
1000e8688: 94018d0b    	bl	0x10014bab4 <__RNvNtCs8Mbv00yxnRz_4core6option13unwrap_failed>
1000e868c: 14000013    	b	0x1000e86d8 <__RNvXs2_NtCs4bPT1zor9nS_10rayon_core3jobINtB5_8StackJobNtNtB7_5latch9SpinLatchNCINvMs4_NtB7_8registryNtB1m_8Registry15in_worker_crossNCINvNtB7_4join12join_contextNCINvNvB2b_4join4calluNCNvNtNtNtCsaHyC9lX8wyC_17field_regressions8campaign10candidates9composite10half_depth0E0NCIB2C_uNCB2X_s_0E0uuE0TuuEE0B4J_ENtB5_3Job7executeB35_+0x194>
1000e8690: f9400708    	ldr	x8, [x24, #0x8]
1000e8694: b4000068    	cbz	x8, 0x1000e86a0 <__RNvXs2_NtCs4bPT1zor9nS_10rayon_core3jobINtB5_8StackJobNtNtB7_5latch9SpinLatchNCINvMs4_NtB7_8registryNtB1m_8Registry15in_worker_crossNCINvNtB7_4join12join_contextNCINvNvB2b_4join4calluNCNvNtNtNtCsaHyC9lX8wyC_17field_regressions8campaign10candidates9composite10half_depth0E0NCIB2C_uNCB2X_s_0E0uuE0TuuEE0B4J_ENtB5_3Job7executeB35_+0x15c>
1000e8698: aa1603e0    	mov	x0, x22
1000e869c: 9401af51    	bl	0x1001543e0 <dyld_stub_binder+0x1001543e0>
1000e86a0: a9065677    	stp	x23, x21, [x19, #0x60]
1000e86a4: f9003a74    	str	x20, [x19, #0x70]
1000e86a8: 1400000b    	b	0x1000e86d4 <__RNvXs2_NtCs4bPT1zor9nS_10rayon_core3jobINtB5_8StackJobNtNtB7_5latch9SpinLatchNCINvMs4_NtB7_8registryNtB1m_8Registry15in_worker_crossNCINvNtB7_4join12join_contextNCINvNvB2b_4join4calluNCNvNtNtNtCsaHyC9lX8wyC_17field_regressions8campaign10candidates9composite10half_depth0E0NCIB2C_uNCB2X_s_0E0uuE0TuuEE0B4J_ENtB5_3Job7executeB35_+0x190>
1000e86ac: 36000155    	tbz	w21, #0x0, 0x1000e86d4 <__RNvXs2_NtCs4bPT1zor9nS_10rayon_core3jobINtB5_8StackJobNtNtB7_5latch9SpinLatchNCINvMs4_NtB7_8registryNtB1m_8Registry15in_worker_crossNCINvNtB7_4join12join_contextNCINvNvB2b_4join4calluNCNvNtNtNtCsaHyC9lX8wyC_17field_regressions8campaign10candidates9composite10half_depth0E0NCIB2C_uNCB2X_s_0E0uuE0TuuEE0B4J_ENtB5_3Job7executeB35_+0x190>
1000e86b0: 92800008    	mov	x8, #-0x1               ; =-1
1000e86b4: f8680288    	ldaddl	x8, x8, [x20]
1000e86b8: f100051f    	cmp	x8, #0x1
1000e86bc: 540000c1    	b.ne	0x1000e86d4 <__RNvXs2_NtCs4bPT1zor9nS_10rayon_core3jobINtB5_8StackJobNtNtB7_5latch9SpinLatchNCINvMs4_NtB7_8registryNtB1m_8Registry15in_worker_crossNCINvNtB7_4join12join_contextNCINvNvB2b_4join4calluNCNvNtNtNtCsaHyC9lX8wyC_17field_regressions8campaign10candidates9composite10half_depth0E0NCIB2C_uNCB2X_s_0E0uuE0TuuEE0B4J_ENtB5_3Job7executeB35_+0x190>
1000e86c0: d50339bf    	dmb	ishld
1000e86c4: aa1403e0    	mov	x0, x20
1000e86c8: 9400b0e9    	bl	0x100114a6c <__RNvMsn_NtCshxvaOLs88l5_5alloc4syncINtB5_3ArcNtNtCs4bPT1zor9nS_10rayon_core8registry8RegistryE9drop_slowBK_>
1000e86cc: 14000002    	b	0x1000e86d4 <__RNvXs2_NtCs4bPT1zor9nS_10rayon_core3jobINtB5_8StackJobNtNtB7_5latch9SpinLatchNCINvMs4_NtB7_8registryNtB1m_8Registry15in_worker_crossNCINvNtB7_4join12join_contextNCINvNvB2b_4join4calluNCNvNtNtNtCsaHyC9lX8wyC_17field_regressions8campaign10candidates9composite10half_depth0E0NCIB2C_uNCB2X_s_0E0uuE0TuuEE0B4J_ENtB5_3Job7executeB35_+0x190>
1000e86d0: 94018d42    	bl	0x10014bbd8 <__RNvNtCs8Mbv00yxnRz_4core9panicking16panic_in_cleanup>
1000e86d4: 940198b6    	bl	0x10014e9ac <__RNvXNtCs4bPT1zor9nS_10rayon_core6unwindNtB2_12AbortIfPanicNtNtNtCs8Mbv00yxnRz_4core3ops4drop4Drop4drop>
1000e86d8: d4200020    	brk	#0x1
1000e86dc: 94018d3f    	bl	0x10014bbd8 <__RNvNtCs8Mbv00yxnRz_4core9panicking16panic_in_cleanup>
1000e86e0: 9400abf6    	bl	0x1001136b8 <__RNvCs1njKG4L9aB3_7___rustc20___rust_panic_cleanup>
1000e86e4: aa0003f5    	mov	x21, x0
1000e86e8: aa0103f4    	mov	x20, x1
1000e86ec: 900005e8    	adrp	x8, 0x1001a4000 <dyld_stub_binder+0x1001a4000>
1000e86f0: 913c0108    	add	x8, x8, #0xf00
1000e86f4: 92800009    	mov	x9, #-0x1               ; =-1
1000e86f8: f8290108    	ldadd	x9, x8, [x8]
1000e86fc: 900005e0    	adrp	x0, 0x1001a4000 <dyld_stub_binder+0x1001a4000>
1000e8700: 91352000    	add	x0, x0, #0xd48
1000e8704: f9400008    	ldr	x8, [x0]
1000e8708: d63f0100    	blr	x8
1000e870c: f9400008    	ldr	x8, [x0]
1000e8710: d1000508    	sub	x8, x8, #0x1
1000e8714: f9000008    	str	x8, [x0]
1000e8718: 3900201f    	strb	wzr, [x0, #0x8]
1000e871c: 52800057    	mov	w23, #0x2               ; =2
1000e8720: f9403268    	ldr	x8, [x19, #0x60]
1000e8724: f100091f    	cmp	x8, #0x2
1000e8728: 54fff4a2    	b.hs	0x1000e85bc <__RNvXs2_NtCs4bPT1zor9nS_10rayon_core3jobINtB5_8StackJobNtNtB7_5latch9SpinLatchNCINvMs4_NtB7_8registryNtB1m_8Registry15in_worker_crossNCINvNtB7_4join12join_contextNCINvNvB2b_4join4calluNCNvNtNtNtCsaHyC9lX8wyC_17field_regressions8campaign10candidates9composite10half_depth0E0NCIB2C_uNCB2X_s_0E0uuE0TuuEE0B4J_ENtB5_3Job7executeB35_+0x78>
1000e872c: 17ffffad    	b	0x1000e85e0 <__RNvXs2_NtCs4bPT1zor9nS_10rayon_core3jobINtB5_8StackJobNtNtB7_5latch9SpinLatchNCINvMs4_NtB7_8registryNtB1m_8Registry15in_worker_crossNCINvNtB7_4join12join_contextNCINvNvB2b_4join4calluNCNvNtNtNtCsaHyC9lX8wyC_17field_regressions8campaign10candidates9composite10half_depth0E0NCIB2C_uNCB2X_s_0E0uuE0TuuEE0B4J_ENtB5_3Job7executeB35_+0x9c>


/private/var/folders/q0/v7l8dzd13k7f_q6j80sy03lm0000gn/T/f2z-field-gated-aejimxw9/field-regressions:	file format mach-o arm64

Disassembly of section __TEXT,__text:

00000001000ea9e0 <__RNvXs2_NtCs4bPT1zor9nS_10rayon_core3jobINtB5_8StackJobNtNtB7_5latch9SpinLatchNCINvNvNtB7_4join12join_context6call_buNCINvNvB1k_4join4calluNCNvNtNtNtCsaHyC9lX8wyC_17field_regressions8campaign10candidates9composite10half_depths_0E0E0uENtB5_3Job7executeB2m_>:
1000ea9e0: a9bc5ff8    	stp	x24, x23, [sp, #-0x40]!
1000ea9e4: a90157f6    	stp	x22, x21, [sp, #0x10]
1000ea9e8: a9024ff4    	stp	x20, x19, [sp, #0x20]
1000ea9ec: a9037bfd    	stp	x29, x30, [sp, #0x30]
1000ea9f0: 9100c3fd    	add	x29, sp, #0x30
1000ea9f4: aa0003f3    	mov	x19, x0
1000ea9f8: f9400000    	ldr	x0, [x0]
1000ea9fc: a9408a61    	ldp	x1, x2, [x19, #0x8]
1000eaa00: a941a668    	ldp	x8, x9, [x19, #0x18]
1000eaa04: f940166a    	ldr	x10, [x19, #0x28]
1000eaa08: f900027f    	str	xzr, [x19]
1000eaa0c: b4000680    	cbz	x0, 0x1000eaadc <__RNvXs2_NtCs4bPT1zor9nS_10rayon_core3jobINtB5_8StackJobNtNtB7_5latch9SpinLatchNCINvNvNtB7_4join12join_context6call_buNCINvNvB1k_4join4calluNCNvNtNtNtCsaHyC9lX8wyC_17field_regressions8campaign10candidates9composite10half_depths_0E0E0uENtB5_3Job7executeB2m_+0xfc>
1000eaa10: f9400103    	ldr	x3, [x8]
1000eaa14: f9400128    	ldr	x8, [x9]
1000eaa18: f9400149    	ldr	x9, [x10]
1000eaa1c: 52800037    	mov	w23, #0x1               ; =1
1000eaa20: aa0906e5    	orr	x5, x23, x9, lsl #1
1000eaa24: 91000504    	add	x4, x8, #0x1
1000eaa28: 97ff9fb5    	bl	0x1000d28fc <__RNvNtNtNtCsaHyC9lX8wyC_17field_regressions8campaign10candidates9composite10half_depth>
1000eaa2c: f9401a68    	ldr	x8, [x19, #0x30]
1000eaa30: f100091f    	cmp	x8, #0x2
1000eaa34: 54000143    	b.lo	0x1000eaa5c <__RNvXs2_NtCs4bPT1zor9nS_10rayon_core3jobINtB5_8StackJobNtNtB7_5latch9SpinLatchNCINvNvNtB7_4join12join_context6call_buNCINvNvB1k_4join4calluNCNvNtNtNtCsaHyC9lX8wyC_17field_regressions8campaign10candidates9composite10half_depths_0E0E0uENtB5_3Job7executeB2m_+0x7c>
1000eaa38: a943e276    	ldp	x22, x24, [x19, #0x38]
1000eaa3c: f9400308    	ldr	x8, [x24]
1000eaa40: b4000068    	cbz	x8, 0x1000eaa4c <__RNvXs2_NtCs4bPT1zor9nS_10rayon_core3jobINtB5_8StackJobNtNtB7_5latch9SpinLatchNCINvNvNtB7_4join12join_context6call_buNCINvNvB1k_4join4calluNCNvNtNtNtCsaHyC9lX8wyC_17field_regressions8campaign10candidates9composite10half_depths_0E0E0uENtB5_3Job7executeB2m_+0x6c>
1000eaa44: aa1603e0    	mov	x0, x22
1000eaa48: d63f0100    	blr	x8
1000eaa4c: f9400708    	ldr	x8, [x24, #0x8]
1000eaa50: b4000068    	cbz	x8, 0x1000eaa5c <__RNvXs2_NtCs4bPT1zor9nS_10rayon_core3jobINtB5_8StackJobNtNtB7_5latch9SpinLatchNCINvNvNtB7_4join12join_context6call_buNCINvNvB1k_4join4calluNCNvNtNtNtCsaHyC9lX8wyC_17field_regressions8campaign10candidates9composite10half_depths_0E0E0uENtB5_3Job7executeB2m_+0x7c>
1000eaa54: aa1603e0    	mov	x0, x22
1000eaa58: 9401a662    	bl	0x1001543e0 <dyld_stub_binder+0x1001543e0>
1000eaa5c: a9035677    	stp	x23, x21, [x19, #0x30]
1000eaa60: f9002274    	str	x20, [x19, #0x40]
1000eaa64: 39418275    	ldrb	w21, [x19, #0x60]
1000eaa68: f9402668    	ldr	x8, [x19, #0x48]
1000eaa6c: f9400114    	ldr	x20, [x8]
1000eaa70: 710006bf    	cmp	w21, #0x1
1000eaa74: 540000a1    	b.ne	0x1000eaa88 <__RNvXs2_NtCs4bPT1zor9nS_10rayon_core3jobINtB5_8StackJobNtNtB7_5latch9SpinLatchNCINvNvNtB7_4join12join_context6call_buNCINvNvB1k_4join4calluNCNvNtNtNtCsaHyC9lX8wyC_17field_regressions8campaign10candidates9composite10half_depths_0E0E0uENtB5_3Job7executeB2m_+0xa8>
1000eaa78: 52800029    	mov	w9, #0x1                ; =1
1000eaa7c: f8290289    	ldadd	x9, x9, [x20]
1000eaa80: b7f80829    	tbnz	x9, #0x3f, 0x1000eab84 <__RNvXs2_NtCs4bPT1zor9nS_10rayon_core3jobINtB5_8StackJobNtNtB7_5latch9SpinLatchNCINvNvNtB7_4join12join_context6call_buNCINvNvB1k_4join4calluNCNvNtNtNtCsaHyC9lX8wyC_17field_regressions8campaign10candidates9composite10half_depths_0E0E0uENtB5_3Job7executeB2m_+0x1a4>
1000eaa84: f9400114    	ldr	x20, [x8]
1000eaa88: f9402e61    	ldr	x1, [x19, #0x58]
1000eaa8c: 91014268    	add	x8, x19, #0x50
1000eaa90: 52800069    	mov	w9, #0x3                ; =3
1000eaa94: f8e98108    	swpal	x9, x8, [x8]
1000eaa98: f100091f    	cmp	x8, #0x2
1000eaa9c: 54000061    	b.ne	0x1000eaaa8 <__RNvXs2_NtCs4bPT1zor9nS_10rayon_core3jobINtB5_8StackJobNtNtB7_5latch9SpinLatchNCINvNvNtB7_4join12join_context6call_buNCINvNvB1k_4join4calluNCNvNtNtNtCsaHyC9lX8wyC_17field_regressions8campaign10candidates9composite10half_depths_0E0E0uENtB5_3Job7executeB2m_+0xc8>
1000eaaa0: 91078280    	add	x0, x20, #0x1e0
1000eaaa4: 9400a631    	bl	0x100114368 <__RNvMNtCs4bPT1zor9nS_10rayon_core5sleepNtB2_5Sleep20wake_specific_thread>
1000eaaa8: 36000115    	tbz	w21, #0x0, 0x1000eaac8 <__RNvXs2_NtCs4bPT1zor9nS_10rayon_core3jobINtB5_8StackJobNtNtB7_5latch9SpinLatchNCINvNvNtB7_4join12join_context6call_buNCINvNvB1k_4join4calluNCNvNtNtNtCsaHyC9lX8wyC_17field_regressions8campaign10candidates9composite10half_depths_0E0E0uENtB5_3Job7executeB2m_+0xe8>
1000eaaac: 92800008    	mov	x8, #-0x1               ; =-1
1000eaab0: f8680288    	ldaddl	x8, x8, [x20]
1000eaab4: f100051f    	cmp	x8, #0x1
1000eaab8: 54000081    	b.ne	0x1000eaac8 <__RNvXs2_NtCs4bPT1zor9nS_10rayon_core3jobINtB5_8StackJobNtNtB7_5latch9SpinLatchNCINvNvNtB7_4join12join_context6call_buNCINvNvB1k_4join4calluNCNvNtNtNtCsaHyC9lX8wyC_17field_regressions8campaign10candidates9composite10half_depths_0E0E0uENtB5_3Job7executeB2m_+0xe8>
1000eaabc: d50339bf    	dmb	ishld
1000eaac0: aa1403e0    	mov	x0, x20
1000eaac4: 9400a7ea    	bl	0x100114a6c <__RNvMsn_NtCshxvaOLs88l5_5alloc4syncINtB5_3ArcNtNtCs4bPT1zor9nS_10rayon_core8registry8RegistryE9drop_slowBK_>
1000eaac8: a9437bfd    	ldp	x29, x30, [sp, #0x30]
1000eaacc: a9424ff4    	ldp	x20, x19, [sp, #0x20]
1000eaad0: a94157f6    	ldp	x22, x21, [sp, #0x10]
1000eaad4: a8c45ff8    	ldp	x24, x23, [sp], #0x40
1000eaad8: d65f03c0    	ret
1000eaadc: d00005a0    	adrp	x0, 0x1001a0000 <dyld_stub_binder+0x1001a0000>
1000eaae0: 91338000    	add	x0, x0, #0xce0
1000eaae4: 940183f4    	bl	0x10014bab4 <__RNvNtCs8Mbv00yxnRz_4core6option13unwrap_failed>
1000eaae8: 14000027    	b	0x1000eab84 <__RNvXs2_NtCs4bPT1zor9nS_10rayon_core3jobINtB5_8StackJobNtNtB7_5latch9SpinLatchNCINvNvNtB7_4join12join_context6call_buNCINvNvB1k_4join4calluNCNvNtNtNtCsaHyC9lX8wyC_17field_regressions8campaign10candidates9composite10half_depths_0E0E0uENtB5_3Job7executeB2m_+0x1a4>
1000eaaec: f9400708    	ldr	x8, [x24, #0x8]
1000eaaf0: b4000068    	cbz	x8, 0x1000eaafc <__RNvXs2_NtCs4bPT1zor9nS_10rayon_core3jobINtB5_8StackJobNtNtB7_5latch9SpinLatchNCINvNvNtB7_4join12join_context6call_buNCINvNvB1k_4join4calluNCNvNtNtNtCsaHyC9lX8wyC_17field_regressions8campaign10candidates9composite10half_depths_0E0E0uENtB5_3Job7executeB2m_+0x11c>
1000eaaf4: aa1603e0    	mov	x0, x22
1000eaaf8: 9401a63a    	bl	0x1001543e0 <dyld_stub_binder+0x1001543e0>
1000eaafc: a9035677    	stp	x23, x21, [x19, #0x30]
1000eab00: f9002274    	str	x20, [x19, #0x40]
1000eab04: 1400001f    	b	0x1000eab80 <__RNvXs2_NtCs4bPT1zor9nS_10rayon_core3jobINtB5_8StackJobNtNtB7_5latch9SpinLatchNCINvNvNtB7_4join12join_context6call_buNCINvNvB1k_4join4calluNCNvNtNtNtCsaHyC9lX8wyC_17field_regressions8campaign10candidates9composite10half_depths_0E0E0uENtB5_3Job7executeB2m_+0x1a0>
1000eab08: 360003d5    	tbz	w21, #0x0, 0x1000eab80 <__RNvXs2_NtCs4bPT1zor9nS_10rayon_core3jobINtB5_8StackJobNtNtB7_5latch9SpinLatchNCINvNvNtB7_4join12join_context6call_buNCINvNvB1k_4join4calluNCNvNtNtNtCsaHyC9lX8wyC_17field_regressions8campaign10candidates9composite10half_depths_0E0E0uENtB5_3Job7executeB2m_+0x1a0>
1000eab0c: 92800008    	mov	x8, #-0x1               ; =-1
1000eab10: f8680288    	ldaddl	x8, x8, [x20]
1000eab14: f100051f    	cmp	x8, #0x1
1000eab18: 54000341    	b.ne	0x1000eab80 <__RNvXs2_NtCs4bPT1zor9nS_10rayon_core3jobINtB5_8StackJobNtNtB7_5latch9SpinLatchNCINvNvNtB7_4join12join_context6call_buNCINvNvB1k_4join4calluNCNvNtNtNtCsaHyC9lX8wyC_17field_regressions8campaign10candidates9composite10half_depths_0E0E0uENtB5_3Job7executeB2m_+0x1a0>
1000eab1c: d50339bf    	dmb	ishld
1000eab20: aa1403e0    	mov	x0, x20
1000eab24: 9400a7d2    	bl	0x100114a6c <__RNvMsn_NtCshxvaOLs88l5_5alloc4syncINtB5_3ArcNtNtCs4bPT1zor9nS_10rayon_core8registry8RegistryE9drop_slowBK_>
1000eab28: 14000016    	b	0x1000eab80 <__RNvXs2_NtCs4bPT1zor9nS_10rayon_core3jobINtB5_8StackJobNtNtB7_5latch9SpinLatchNCINvNvNtB7_4join12join_context6call_buNCINvNvB1k_4join4calluNCNvNtNtNtCsaHyC9lX8wyC_17field_regressions8campaign10candidates9composite10half_depths_0E0E0uENtB5_3Job7executeB2m_+0x1a0>
1000eab2c: 9401842b    	bl	0x10014bbd8 <__RNvNtCs8Mbv00yxnRz_4core9panicking16panic_in_cleanup>
1000eab30: 9400a2e2    	bl	0x1001136b8 <__RNvCs1njKG4L9aB3_7___rustc20___rust_panic_cleanup>
1000eab34: aa0003f5    	mov	x21, x0
1000eab38: aa0103f4    	mov	x20, x1
1000eab3c: d00005c8    	adrp	x8, 0x1001a4000 <dyld_stub_binder+0x1001a4000>
1000eab40: 913c0108    	add	x8, x8, #0xf00
1000eab44: 92800009    	mov	x9, #-0x1               ; =-1
1000eab48: f8290108    	ldadd	x9, x8, [x8]
1000eab4c: d00005c0    	adrp	x0, 0x1001a4000 <dyld_stub_binder+0x1001a4000>
1000eab50: 91352000    	add	x0, x0, #0xd48
1000eab54: f9400008    	ldr	x8, [x0]
1000eab58: d63f0100    	blr	x8
1000eab5c: f9400008    	ldr	x8, [x0]
1000eab60: d1000508    	sub	x8, x8, #0x1
1000eab64: f9000008    	str	x8, [x0]
1000eab68: 3900201f    	strb	wzr, [x0, #0x8]
1000eab6c: 52800057    	mov	w23, #0x2               ; =2
1000eab70: f9401a68    	ldr	x8, [x19, #0x30]
1000eab74: f100091f    	cmp	x8, #0x2
1000eab78: 54fff602    	b.hs	0x1000eaa38 <__RNvXs2_NtCs4bPT1zor9nS_10rayon_core3jobINtB5_8StackJobNtNtB7_5latch9SpinLatchNCINvNvNtB7_4join12join_context6call_buNCINvNvB1k_4join4calluNCNvNtNtNtCsaHyC9lX8wyC_17field_regressions8campaign10candidates9composite10half_depths_0E0E0uENtB5_3Job7executeB2m_+0x58>
1000eab7c: 17ffffb8    	b	0x1000eaa5c <__RNvXs2_NtCs4bPT1zor9nS_10rayon_core3jobINtB5_8StackJobNtNtB7_5latch9SpinLatchNCINvNvNtB7_4join12join_context6call_buNCINvNvB1k_4join4calluNCNvNtNtNtCsaHyC9lX8wyC_17field_regressions8campaign10candidates9composite10half_depths_0E0E0uENtB5_3Job7executeB2m_+0x7c>
1000eab80: 94018f8b    	bl	0x10014e9ac <__RNvXNtCs4bPT1zor9nS_10rayon_core6unwindNtB2_12AbortIfPanicNtNtNtCs8Mbv00yxnRz_4core3ops4drop4Drop4drop>
1000eab84: d4200020    	brk	#0x1
1000eab88: 94018414    	bl	0x10014bbd8 <__RNvNtCs8Mbv00yxnRz_4core9panicking16panic_in_cleanup>

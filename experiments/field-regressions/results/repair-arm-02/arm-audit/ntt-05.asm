
/private/var/folders/q0/v7l8dzd13k7f_q6j80sy03lm0000gn/T/f2z-field-gated-aejimxw9/field-regressions:	file format mach-o arm64

Disassembly of section __TEXT,__text:

00000001000e70a8 <__RNvXs2_NtCs4bPT1zor9nS_10rayon_core3jobINtB5_8StackJobINtNtB7_5latch8LatchRefNtBT_9LockLatchENCNCINvMs4_NtB7_8registryNtB1E_8Registry14in_worker_coldNCINvNtB7_4join12join_contextNCINvNvB2s_4join4calluNCNvNtNtNtCsaHyC9lX8wyC_17field_regressions8campaign10candidates9composite10half_depth0E0NCIB2T_uNCB3e_s_0E0uuE0TuuEE00B50_ENtB5_3Job7executeB3m_>:
1000e70a8: d10283ff    	sub	sp, sp, #0xa0
1000e70ac: a9065ff8    	stp	x24, x23, [sp, #0x60]
1000e70b0: a90757f6    	stp	x22, x21, [sp, #0x70]
1000e70b4: a9084ff4    	stp	x20, x19, [sp, #0x80]
1000e70b8: a9097bfd    	stp	x29, x30, [sp, #0x90]
1000e70bc: 910243fd    	add	x29, sp, #0x90
1000e70c0: 3dc00000    	ldr	q0, [x0]
1000e70c4: aa0003e8    	mov	x8, x0
1000e70c8: f801051f    	str	xzr, [x8], #0x10
1000e70cc: 9e660009    	fmov	x9, d0
1000e70d0: b40005c9    	cbz	x9, 0x1000e7188 <__RNvXs2_NtCs4bPT1zor9nS_10rayon_core3jobINtB5_8StackJobINtNtB7_5latch8LatchRefNtBT_9LockLatchENCNCINvMs4_NtB7_8registryNtB1E_8Registry14in_worker_coldNCINvNtB7_4join12join_contextNCINvNvB2s_4join4calluNCNvNtNtNtCsaHyC9lX8wyC_17field_regressions8campaign10candidates9composite10half_depth0E0NCIB2T_uNCB3e_s_0E0uuE0TuuEE00B50_ENtB5_3Job7executeB3m_+0xe0>
1000e70d4: aa0003f3    	mov	x19, x0
1000e70d8: b00005e0    	adrp	x0, 0x1001a4000 <dyld_stub_binder+0x1001a4000>
1000e70dc: 91340000    	add	x0, x0, #0xd00
1000e70e0: f9400009    	ldr	x9, [x0]
1000e70e4: d63f0120    	blr	x9
1000e70e8: f9400001    	ldr	x1, [x0]
1000e70ec: b4000401    	cbz	x1, 0x1000e716c <__RNvXs2_NtCs4bPT1zor9nS_10rayon_core3jobINtB5_8StackJobINtNtB7_5latch8LatchRefNtBT_9LockLatchENCNCINvMs4_NtB7_8registryNtB1E_8Registry14in_worker_coldNCINvNtB7_4join12join_contextNCINvNvB2s_4join4calluNCNvNtNtNtCsaHyC9lX8wyC_17field_regressions8campaign10candidates9composite10half_depth0E0NCIB2T_uNCB3e_s_0E0uuE0TuuEE00B50_ENtB5_3Job7executeB3m_+0xc4>
1000e70f0: ad410901    	ldp	q1, q2, [x8, #0x20]
1000e70f4: ad400d04    	ldp	q4, q3, [x8]
1000e70f8: ad0107e3    	stp	q3, q1, [sp, #0x20]
1000e70fc: 3dc01101    	ldr	q1, [x8, #0x40]
1000e7100: ad0207e2    	stp	q2, q1, [sp, #0x40]
1000e7104: ad0013e0    	stp	q0, q4, [sp]
1000e7108: 910003e0    	mov	x0, sp
1000e710c: 97fd7970    	bl	0x1000456cc <__RNCINvNtCs4bPT1zor9nS_10rayon_core4join12join_contextNCINvNvB4_4join4calluNCNvNtNtNtCsaHyC9lX8wyC_17field_regressions8campaign10candidates9composite10half_depth0E0NCIBS_uNCB1c_s_0E0uuE0B1k_>
1000e7110: 52800037    	mov	w23, #0x1               ; =1
1000e7114: f9403668    	ldr	x8, [x19, #0x68]
1000e7118: f100091f    	cmp	x8, #0x2
1000e711c: 54000143    	b.lo	0x1000e7144 <__RNvXs2_NtCs4bPT1zor9nS_10rayon_core3jobINtB5_8StackJobINtNtB7_5latch8LatchRefNtBT_9LockLatchENCNCINvMs4_NtB7_8registryNtB1E_8Registry14in_worker_coldNCINvNtB7_4join12join_contextNCINvNvB2s_4join4calluNCNvNtNtNtCsaHyC9lX8wyC_17field_regressions8campaign10candidates9composite10half_depth0E0NCIB2T_uNCB3e_s_0E0uuE0TuuEE00B50_ENtB5_3Job7executeB3m_+0x9c>
1000e7120: a9476276    	ldp	x22, x24, [x19, #0x70]
1000e7124: f9400308    	ldr	x8, [x24]
1000e7128: b4000068    	cbz	x8, 0x1000e7134 <__RNvXs2_NtCs4bPT1zor9nS_10rayon_core3jobINtB5_8StackJobINtNtB7_5latch8LatchRefNtBT_9LockLatchENCNCINvMs4_NtB7_8registryNtB1E_8Registry14in_worker_coldNCINvNtB7_4join12join_contextNCINvNvB2s_4join4calluNCNvNtNtNtCsaHyC9lX8wyC_17field_regressions8campaign10candidates9composite10half_depth0E0NCIB2T_uNCB3e_s_0E0uuE0TuuEE00B50_ENtB5_3Job7executeB3m_+0x8c>
1000e712c: aa1603e0    	mov	x0, x22
1000e7130: d63f0100    	blr	x8
1000e7134: f9400708    	ldr	x8, [x24, #0x8]
1000e7138: b4000068    	cbz	x8, 0x1000e7144 <__RNvXs2_NtCs4bPT1zor9nS_10rayon_core3jobINtB5_8StackJobINtNtB7_5latch8LatchRefNtBT_9LockLatchENCNCINvMs4_NtB7_8registryNtB1E_8Registry14in_worker_coldNCINvNtB7_4join12join_contextNCINvNvB2s_4join4calluNCNvNtNtNtCsaHyC9lX8wyC_17field_regressions8campaign10candidates9composite10half_depth0E0NCIB2T_uNCB3e_s_0E0uuE0TuuEE00B50_ENtB5_3Job7executeB3m_+0x9c>
1000e713c: aa1603e0    	mov	x0, x22
1000e7140: 9401b4a8    	bl	0x1001543e0 <dyld_stub_binder+0x1001543e0>
1000e7144: a906d677    	stp	x23, x21, [x19, #0x68]
1000e7148: f9003e74    	str	x20, [x19, #0x78]
1000e714c: f9403260    	ldr	x0, [x19, #0x60]
1000e7150: 940013d8    	bl	0x1000ec0b0 <__RNvXs4_NtCs4bPT1zor9nS_10rayon_core5latchNtB5_9LockLatchNtB5_5Latch3set>
1000e7154: a9497bfd    	ldp	x29, x30, [sp, #0x90]
1000e7158: a9484ff4    	ldp	x20, x19, [sp, #0x80]
1000e715c: a94757f6    	ldp	x22, x21, [sp, #0x70]
1000e7160: a9465ff8    	ldp	x24, x23, [sp, #0x60]
1000e7164: 910283ff    	add	sp, sp, #0xa0
1000e7168: d65f03c0    	ret
1000e716c: 900003a0    	adrp	x0, 0x10015b000 <__RNvNtNtNtNtNtCs8Mbv00yxnRz_4core3num3imp7flt2dec8strategy6dragon9POW5TO256+0x2e4>
1000e7170: 91008000    	add	x0, x0, #0x20
1000e7174: b00005c2    	adrp	x2, 0x1001a0000 <dyld_stub_binder+0x1001a0000>
1000e7178: 911d2042    	add	x2, x2, #0x748
1000e717c: 528006c1    	mov	w1, #0x36               ; =54
1000e7180: 9401923e    	bl	0x10014ba78 <__RNvNtCs8Mbv00yxnRz_4core9panicking5panic>
1000e7184: 1400000d    	b	0x1000e71b8 <__RNvXs2_NtCs4bPT1zor9nS_10rayon_core3jobINtB5_8StackJobINtNtB7_5latch8LatchRefNtBT_9LockLatchENCNCINvMs4_NtB7_8registryNtB1E_8Registry14in_worker_coldNCINvNtB7_4join12join_contextNCINvNvB2s_4join4calluNCNvNtNtNtCsaHyC9lX8wyC_17field_regressions8campaign10candidates9composite10half_depth0E0NCIB2T_uNCB3e_s_0E0uuE0TuuEE00B50_ENtB5_3Job7executeB3m_+0x110>
1000e7188: b00005c0    	adrp	x0, 0x1001a0000 <dyld_stub_binder+0x1001a0000>
1000e718c: 91338000    	add	x0, x0, #0xce0
1000e7190: 94019249    	bl	0x10014bab4 <__RNvNtCs8Mbv00yxnRz_4core6option13unwrap_failed>
1000e7194: 14000009    	b	0x1000e71b8 <__RNvXs2_NtCs4bPT1zor9nS_10rayon_core3jobINtB5_8StackJobINtNtB7_5latch8LatchRefNtBT_9LockLatchENCNCINvMs4_NtB7_8registryNtB1E_8Registry14in_worker_coldNCINvNtB7_4join12join_contextNCINvNvB2s_4join4calluNCNvNtNtNtCsaHyC9lX8wyC_17field_regressions8campaign10candidates9composite10half_depth0E0NCIB2T_uNCB3e_s_0E0uuE0TuuEE00B50_ENtB5_3Job7executeB3m_+0x110>
1000e7198: f9400708    	ldr	x8, [x24, #0x8]
1000e719c: b4000068    	cbz	x8, 0x1000e71a8 <__RNvXs2_NtCs4bPT1zor9nS_10rayon_core3jobINtB5_8StackJobINtNtB7_5latch8LatchRefNtBT_9LockLatchENCNCINvMs4_NtB7_8registryNtB1E_8Registry14in_worker_coldNCINvNtB7_4join12join_contextNCINvNvB2s_4join4calluNCNvNtNtNtCsaHyC9lX8wyC_17field_regressions8campaign10candidates9composite10half_depth0E0NCIB2T_uNCB3e_s_0E0uuE0TuuEE00B50_ENtB5_3Job7executeB3m_+0x100>
1000e71a0: aa1603e0    	mov	x0, x22
1000e71a4: 9401b48f    	bl	0x1001543e0 <dyld_stub_binder+0x1001543e0>
1000e71a8: a906d677    	stp	x23, x21, [x19, #0x68]
1000e71ac: f9003e74    	str	x20, [x19, #0x78]
1000e71b0: 14000001    	b	0x1000e71b4 <__RNvXs2_NtCs4bPT1zor9nS_10rayon_core3jobINtB5_8StackJobINtNtB7_5latch8LatchRefNtBT_9LockLatchENCNCINvMs4_NtB7_8registryNtB1E_8Registry14in_worker_coldNCINvNtB7_4join12join_contextNCINvNvB2s_4join4calluNCNvNtNtNtCsaHyC9lX8wyC_17field_regressions8campaign10candidates9composite10half_depth0E0NCIB2T_uNCB3e_s_0E0uuE0TuuEE00B50_ENtB5_3Job7executeB3m_+0x10c>
1000e71b4: 94019dfe    	bl	0x10014e9ac <__RNvXNtCs4bPT1zor9nS_10rayon_core6unwindNtB2_12AbortIfPanicNtNtNtCs8Mbv00yxnRz_4core3ops4drop4Drop4drop>
1000e71b8: d4200020    	brk	#0x1
1000e71bc: 94019287    	bl	0x10014bbd8 <__RNvNtCs8Mbv00yxnRz_4core9panicking16panic_in_cleanup>
1000e71c0: 9400b13e    	bl	0x1001136b8 <__RNvCs1njKG4L9aB3_7___rustc20___rust_panic_cleanup>
1000e71c4: aa0003f5    	mov	x21, x0
1000e71c8: aa0103f4    	mov	x20, x1
1000e71cc: b00005e8    	adrp	x8, 0x1001a4000 <dyld_stub_binder+0x1001a4000>
1000e71d0: 913c0108    	add	x8, x8, #0xf00
1000e71d4: 92800009    	mov	x9, #-0x1               ; =-1
1000e71d8: f8290108    	ldadd	x9, x8, [x8]
1000e71dc: b00005e0    	adrp	x0, 0x1001a4000 <dyld_stub_binder+0x1001a4000>
1000e71e0: 91352000    	add	x0, x0, #0xd48
1000e71e4: f9400008    	ldr	x8, [x0]
1000e71e8: d63f0100    	blr	x8
1000e71ec: f9400008    	ldr	x8, [x0]
1000e71f0: d1000508    	sub	x8, x8, #0x1
1000e71f4: f9000008    	str	x8, [x0]
1000e71f8: 3900201f    	strb	wzr, [x0, #0x8]
1000e71fc: 52800057    	mov	w23, #0x2               ; =2
1000e7200: f9403668    	ldr	x8, [x19, #0x68]
1000e7204: f100091f    	cmp	x8, #0x2
1000e7208: 54fff8c2    	b.hs	0x1000e7120 <__RNvXs2_NtCs4bPT1zor9nS_10rayon_core3jobINtB5_8StackJobINtNtB7_5latch8LatchRefNtBT_9LockLatchENCNCINvMs4_NtB7_8registryNtB1E_8Registry14in_worker_coldNCINvNtB7_4join12join_contextNCINvNvB2s_4join4calluNCNvNtNtNtCsaHyC9lX8wyC_17field_regressions8campaign10candidates9composite10half_depth0E0NCIB2T_uNCB3e_s_0E0uuE0TuuEE00B50_ENtB5_3Job7executeB3m_+0x78>
1000e720c: 17ffffce    	b	0x1000e7144 <__RNvXs2_NtCs4bPT1zor9nS_10rayon_core3jobINtB5_8StackJobINtNtB7_5latch8LatchRefNtBT_9LockLatchENCNCINvMs4_NtB7_8registryNtB1E_8Registry14in_worker_coldNCINvNtB7_4join12join_contextNCINvNvB2s_4join4calluNCNvNtNtNtCsaHyC9lX8wyC_17field_regressions8campaign10candidates9composite10half_depth0E0NCIB2T_uNCB3e_s_0E0uuE0TuuEE00B50_ENtB5_3Job7executeB3m_+0x9c>

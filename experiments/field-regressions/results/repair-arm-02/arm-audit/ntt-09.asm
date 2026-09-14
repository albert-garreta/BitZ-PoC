
/private/var/folders/q0/v7l8dzd13k7f_q6j80sy03lm0000gn/T/f2z-field-gated-aejimxw9/field-regressions:	file format mach-o arm64

Disassembly of section __TEXT,__text:

0000000100149310 <__RINvMs4_NtCs4bPT1zor9nS_10rayon_core8registryNtB6_8Registry15in_worker_crossNCINvNtB8_4join12join_contextNCINvNvB1h_4join4calluNCNvNtNtNtCsaHyC9lX8wyC_17field_regressions8campaign10candidates9composite10half_depth0E0NCIB1I_uNCB23_s_0E0uuE0TuuEEB2b_>:
100149310: d10303ff    	sub	sp, sp, #0xc0
100149314: a90a4ff4    	stp	x20, x19, [sp, #0xa0]
100149318: a90b7bfd    	stp	x29, x30, [sp, #0xb0]
10014931c: 9102c3fd    	add	x29, sp, #0xb0
100149320: aa0103f3    	mov	x19, x1
100149324: f9408028    	ldr	x8, [x1, #0x100]
100149328: 91044029    	add	x9, x1, #0x110
10014932c: a90823ff    	stp	xzr, x8, [sp, #0x80]
100149330: 52800028    	mov	w8, #0x1                ; =1
100149334: 390243e8    	strb	w8, [sp, #0x90]
100149338: 910003f4    	mov	x20, sp
10014933c: ad410440    	ldp	q0, q1, [x2, #0x20]
100149340: ad0107e0    	stp	q0, q1, [sp, #0x20]
100149344: ad420440    	ldp	q0, q1, [x2, #0x40]
100149348: ad0207e0    	stp	q0, q1, [sp, #0x40]
10014934c: ad400440    	ldp	q0, q1, [x2]
100149350: ad0007e0    	stp	q0, q1, [sp]
100149354: f9003fe9    	str	x9, [sp, #0x78]
100149358: f90033ff    	str	xzr, [sp, #0x60]
10014935c: f0fffce1    	adrp	x1, 0x1000e8000 <__RNvXs2_NtCs4bPT1zor9nS_10rayon_core3jobINtB5_8StackJobINtNtB7_5latch8LatchRefNtBT_9LockLatchENCNCINvMs4_NtB7_8registryNtB1E_8Registry14in_worker_coldNCINvNtB7_4join12join_contextNCINvNvNtNtCseQgOPG8ZrnX_5rayon4iter8plumbing24bridge_producer_consumer6helperINtNtB31_5range12IterProducerjEINtNtB2Z_3map11MapConsumerINtNtNtB2Z_7collect8consumer15CollectConsumerNtNtNtNtCs6sYy3Ligu9V_3f2z4poly10univariate12binary_gf12816BinaryFieldGF128ENCNvNtCsaHyC9lX8wyC_17field_regressions14production_ood8ood_eval0EE0NCB2S_s_0INtB54_13CollectResultB5K_EB8d_E0TB8d_B8d_EE00B8K_ENtB5_3Job7executeB74_+0x190>
100149360: 91151021    	add	x1, x1, #0x544
100149364: 910003e2    	mov	x2, sp
100149368: 97ff3caa    	bl	0x100118610 <__RNvMs4_NtCs4bPT1zor9nS_10rayon_core8registryNtB5_8Registry6inject>
10014936c: d94803e8    	ldapur	x8, [sp, #0x80]
100149370: f1000d1f    	cmp	x8, #0x3
100149374: 54000181    	b.ne	0x1001493a4 <__RINvMs4_NtCs4bPT1zor9nS_10rayon_core8registryNtB6_8Registry15in_worker_crossNCINvNtB8_4join12join_contextNCINvNvB1h_4join4calluNCNvNtNtNtCsaHyC9lX8wyC_17field_regressions8campaign10candidates9composite10half_depth0E0NCIB1I_uNCB23_s_0E0uuE0TuuEEB2b_+0x94>
100149378: f94033e8    	ldr	x8, [sp, #0x60]
10014937c: f100051f    	cmp	x8, #0x1
100149380: 540000a0    	b.eq	0x100149394 <__RINvMs4_NtCs4bPT1zor9nS_10rayon_core8registryNtB6_8Registry15in_worker_crossNCINvNtB8_4join12join_contextNCINvNvB1h_4join4calluNCNvNtNtNtCsaHyC9lX8wyC_17field_regressions8campaign10candidates9composite10half_depth0E0NCIB1I_uNCB23_s_0E0uuE0TuuEEB2b_+0x84>
100149384: f100091f    	cmp	x8, #0x2
100149388: 54000161    	b.ne	0x1001493b4 <__RINvMs4_NtCs4bPT1zor9nS_10rayon_core8registryNtB6_8Registry15in_worker_crossNCINvNtB8_4join12join_contextNCINvNvB1h_4join4calluNCNvNtNtNtCsaHyC9lX8wyC_17field_regressions8campaign10candidates9composite10half_depth0E0NCIB1I_uNCB23_s_0E0uuE0TuuEEB2b_+0xa4>
10014938c: a94687e0    	ldp	x0, x1, [sp, #0x68]
100149390: 97ff3c27    	bl	0x10011842c <__RNvNtCs4bPT1zor9nS_10rayon_core6unwind16resume_unwinding>
100149394: a94b7bfd    	ldp	x29, x30, [sp, #0xb0]
100149398: a94a4ff4    	ldp	x20, x19, [sp, #0xa0]
10014939c: 910303ff    	add	sp, sp, #0xc0
1001493a0: d65f03c0    	ret
1001493a4: 91020281    	add	x1, x20, #0x80
1001493a8: aa1303e0    	mov	x0, x19
1001493ac: 940011d7    	bl	0x10014db08 <__RNvMs8_NtCs4bPT1zor9nS_10rayon_core8registryNtB5_12WorkerThread15wait_until_cold>
1001493b0: 17fffff2    	b	0x100149378 <__RINvMs4_NtCs4bPT1zor9nS_10rayon_core8registryNtB6_8Registry15in_worker_crossNCINvNtB8_4join12join_contextNCINvNvB1h_4join4calluNCNvNtNtNtCsaHyC9lX8wyC_17field_regressions8campaign10candidates9composite10half_depth0E0NCIB1I_uNCB23_s_0E0uuE0TuuEEB2b_+0x68>
1001493b4: b00000a0    	adrp	x0, 0x10015e000 <__RNvNtCsejePs28XOyx_10serde_json4read4HEX1+0x1040>
1001493b8: 912b9800    	add	x0, x0, #0xae6
1001493bc: f00002a2    	adrp	x2, 0x1001a0000 <dyld_stub_binder+0x1001a0000>
1001493c0: 9125a042    	add	x2, x2, #0x968
1001493c4: 52800501    	mov	w1, #0x28               ; =40
1001493c8: 940009ac    	bl	0x10014ba78 <__RNvNtCs8Mbv00yxnRz_4core9panicking5panic>
1001493cc: aa0003f3    	mov	x19, x0
1001493d0: 910003e0    	mov	x0, sp
1001493d4: 97fafc0c    	bl	0x100008404 <__RINvNtCs8Mbv00yxnRz_4core3ptr9drop_glueINtNtCs4bPT1zor9nS_10rayon_core3job8StackJobNtNtBG_5latch9SpinLatchNCINvMs4_NtBG_8registryNtB1P_8Registry15in_worker_crossNCINvNtBG_4join12join_contextNCINvNvB2E_4join4calluNCNvNtNtNtCsaHyC9lX8wyC_17field_regressions8campaign10candidates9composite10half_depth0E0NCIB35_uNCB3q_s_0E0uuE0TuuEE0B5c_EEB3y_>
1001493d8: aa1303e0    	mov	x0, x19
1001493dc: 94002bbf    	bl	0x1001542d8 <dyld_stub_binder+0x1001542d8>
1001493e0: 940009fe    	bl	0x10014bbd8 <__RNvNtCs8Mbv00yxnRz_4core9panicking16panic_in_cleanup>

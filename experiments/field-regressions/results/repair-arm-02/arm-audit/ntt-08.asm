
/private/var/folders/q0/v7l8dzd13k7f_q6j80sy03lm0000gn/T/f2z-field-gated-aejimxw9/field-regressions:	file format mach-o arm64

Disassembly of section __TEXT,__text:

0000000100148510 <__RINvMs4_NtCs4bPT1zor9nS_10rayon_core8registryNtB6_8Registry14in_worker_coldNCINvNtB8_4join12join_contextNCINvNvB1g_4join4calluNCNvNtNtNtCsaHyC9lX8wyC_17field_regressions8campaign10candidates9composite10half_depth0E0NCIB1H_uNCB22_s_0E0uuE0TuuEEB2a_>:
100148510: d102c3ff    	sub	sp, sp, #0xb0
100148514: a9094ff4    	stp	x20, x19, [sp, #0x90]
100148518: a90a7bfd    	stp	x29, x30, [sp, #0xa0]
10014851c: 910283fd    	add	x29, sp, #0xa0
100148520: aa0003f3    	mov	x19, x0
100148524: 900002e0    	adrp	x0, 0x1001a4000 <dyld_stub_binder+0x1001a4000>
100148528: 91346000    	add	x0, x0, #0xd18
10014852c: f9400008    	ldr	x8, [x0]
100148530: d63f0100    	blr	x8
100148534: 39408008    	ldrb	w8, [x0, #0x20]
100148538: 35000348    	cbnz	w8, 0x1001485a0 <__RINvMs4_NtCs4bPT1zor9nS_10rayon_core8registryNtB6_8Registry14in_worker_coldNCINvNtB8_4join12join_contextNCINvNvB1g_4join4calluNCNvNtNtNtCsaHyC9lX8wyC_17field_regressions8campaign10candidates9composite10half_depth0E0NCIB1H_uNCB22_s_0E0uuE0TuuEEB2a_+0x90>
10014853c: ad410420    	ldp	q0, q1, [x1, #0x20]
100148540: ad0187e0    	stp	q0, q1, [sp, #0x30]
100148544: ad420420    	ldp	q0, q1, [x1, #0x40]
100148548: ad0287e0    	stp	q0, q1, [sp, #0x50]
10014854c: ad400420    	ldp	q0, q1, [x1]
100148550: ad0087e0    	stp	q0, q1, [sp, #0x10]
100148554: a9077fe0    	stp	x0, xzr, [sp, #0x70]
100148558: f0fffce1    	adrp	x1, 0x1000e7000 <__RNvXs1q_NtCs8Mbv00yxnRz_4core3fmtRNtNtNtCs4rJR5Xg1K3s_13crypto_bigint4uint8ref_type7UintRefNtB6_8UpperHex3fmtCsaHyC9lX8wyC_17field_regressions+0x34>
10014855c: 9102a021    	add	x1, x1, #0xa8
100148560: 910043e2    	add	x2, sp, #0x10
100148564: aa1303e0    	mov	x0, x19
100148568: 97ff402a    	bl	0x100118610 <__RNvMs4_NtCs4bPT1zor9nS_10rayon_core8registryNtB5_8Registry6inject>
10014856c: f9403be0    	ldr	x0, [sp, #0x70]
100148570: 97ff3fb2    	bl	0x100118438 <__RNvMs3_NtCs4bPT1zor9nS_10rayon_core5latchNtB5_9LockLatch14wait_and_reset>
100148574: f9403fe8    	ldr	x8, [sp, #0x78]
100148578: f100051f    	cmp	x8, #0x1
10014857c: 540000a0    	b.eq	0x100148590 <__RINvMs4_NtCs4bPT1zor9nS_10rayon_core8registryNtB6_8Registry14in_worker_coldNCINvNtB8_4join12join_contextNCINvNvB1g_4join4calluNCNvNtNtNtCsaHyC9lX8wyC_17field_regressions8campaign10candidates9composite10half_depth0E0NCIB1H_uNCB22_s_0E0uuE0TuuEEB2a_+0x80>
100148580: f100091f    	cmp	x8, #0x2
100148584: 54000221    	b.ne	0x1001485c8 <__RINvMs4_NtCs4bPT1zor9nS_10rayon_core8registryNtB6_8Registry14in_worker_coldNCINvNtB8_4join12join_contextNCINvNvB1g_4join4calluNCNvNtNtNtCsaHyC9lX8wyC_17field_regressions8campaign10candidates9composite10half_depth0E0NCIB1H_uNCB22_s_0E0uuE0TuuEEB2a_+0xb8>
100148588: a94807e0    	ldp	x0, x1, [sp, #0x80]
10014858c: 97ff3fa8    	bl	0x10011842c <__RNvNtCs4bPT1zor9nS_10rayon_core6unwind16resume_unwinding>
100148590: a94a7bfd    	ldp	x29, x30, [sp, #0xa0]
100148594: a9494ff4    	ldp	x20, x19, [sp, #0x90]
100148598: 9102c3ff    	add	sp, sp, #0xb0
10014859c: d65f03c0    	ret
1001485a0: 7100051f    	cmp	w8, #0x1
1001485a4: 540001e1    	b.ne	0x1001485e0 <__RINvMs4_NtCs4bPT1zor9nS_10rayon_core8registryNtB6_8Registry14in_worker_coldNCINvNtB8_4join12join_contextNCINvNvB1g_4join4calluNCNvNtNtNtCsaHyC9lX8wyC_17field_regressions8campaign10candidates9composite10half_depth0E0NCIB1H_uNCB22_s_0E0uuE0TuuEEB2a_+0xd0>
1001485a8: d0fffda8    	adrp	x8, 0x1000fe000 <__RNCINvMs0_NtNtCs82bWklYMk3w_3std4sync4onceNtB8_4Once15call_once_forceNCINvMNtBa_9once_lockINtB1b_8OnceLockNtNtCs4bPT1zor9nS_10rayon_core11thread_pool10ThreadPoolE10initializeNCINvB1a_11get_or_initNCNvCs2sbcTSnSTHy_10flock_core13all_core_pool0E0zE0E0B3c_+0xbb0>
1001485ac: 913f2108    	add	x8, x8, #0xfc8
1001485b0: a90003e1    	stp	x1, x0, [sp]
1001485b4: aa0803e1    	mov	x1, x8
1001485b8: 97ffe4a0    	bl	0x100141838 <__RNvNtNtNtNtCs82bWklYMk3w_3std3sys12thread_local11destructors4list8register>
1001485bc: a94003e1    	ldp	x1, x0, [sp]
1001485c0: 3900801f    	strb	wzr, [x0, #0x20]
1001485c4: 17ffffde    	b	0x10014853c <__RINvMs4_NtCs4bPT1zor9nS_10rayon_core8registryNtB6_8Registry14in_worker_coldNCINvNtB8_4join12join_contextNCINvNvB1g_4join4calluNCNvNtNtNtCsaHyC9lX8wyC_17field_regressions8campaign10candidates9composite10half_depth0E0NCIB1H_uNCB22_s_0E0uuE0TuuEEB2a_+0x2c>
1001485c8: d00000a0    	adrp	x0, 0x10015e000 <__RNvNtCsejePs28XOyx_10serde_json4read4HEX1+0x1040>
1001485cc: 912b9800    	add	x0, x0, #0xae6
1001485d0: 900002c2    	adrp	x2, 0x1001a0000 <dyld_stub_binder+0x1001a0000>
1001485d4: 9125a042    	add	x2, x2, #0x968
1001485d8: 52800501    	mov	w1, #0x28               ; =40
1001485dc: 94000d27    	bl	0x10014ba78 <__RNvNtCs8Mbv00yxnRz_4core9panicking5panic>
1001485e0: 900002c0    	adrp	x0, 0x1001a0000 <dyld_stub_binder+0x1001a0000>
1001485e4: 911c0000    	add	x0, x0, #0x700
1001485e8: 94002ead    	bl	0x10015409c <__RNvNtNtCs82bWklYMk3w_3std6thread5local18panic_access_error>
1001485ec: aa0003f3    	mov	x19, x0
1001485f0: 910043e0    	add	x0, sp, #0x10
1001485f4: 97fafec0    	bl	0x1000080f4 <__RINvNtCs8Mbv00yxnRz_4core3ptr9drop_glueINtNtCs4bPT1zor9nS_10rayon_core3job8StackJobINtNtBG_5latch8LatchRefNtB1m_9LockLatchENCNCINvMs4_NtBG_8registryNtB28_8Registry14in_worker_coldNCINvNtBG_4join12join_contextNCINvNvB2W_4join4calluNCNvNtNtNtCsaHyC9lX8wyC_17field_regressions8campaign10candidates9composite10half_depth0E0NCIB3n_uNCB3I_s_0E0uuE0TuuEE00B5u_EEB3Q_>
1001485f8: aa1303e0    	mov	x0, x19
1001485fc: 94002f37    	bl	0x1001542d8 <dyld_stub_binder+0x1001542d8>
100148600: 94000d76    	bl	0x10014bbd8 <__RNvNtCs8Mbv00yxnRz_4core9panicking16panic_in_cleanup>

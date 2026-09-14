
/var/folders/q0/v7l8dzd13k7f_q6j80sy03lm0000gn/T/f2z-field-gated-bkk46c4y/field-regressions:	file format mach-o arm64

Disassembly of section __TEXT,__text:

0000000100007274 <field_regressions::campaign::integer::existing::<9>>:
100007274:     	sub	sp, sp, #0xf0
100007278:     	stp	x28, x27, [sp, #0x90]
10000727c:     	stp	x26, x25, [sp, #0xa0]
100007280:     	stp	x24, x23, [sp, #0xb0]
100007284:     	stp	x22, x21, [sp, #0xc0]
100007288:     	stp	x20, x19, [sp, #0xd0]
10000728c:     	stp	x29, x30, [sp, #0xe0]
100007290:     	add	x29, sp, #0xe0
100007294:     	stp	x2, x4, [x29, #-0x60]
100007298:     	cmp	x2, x4
10000729c:     	b.ne	0x100007768 <field_regressions::campaign::integer::existing::<9>+0x4f4>
1000072a0:     	str	x0, [sp, #0x8]
1000072a4:     	cbz	x2, 0x10000770c <field_regressions::campaign::integer::existing::<9>+0x498>
1000072a8:     	mov	x9, #0x0                ; =0
1000072ac:     	mov	x10, #0x0               ; =0
1000072b0:     	mov	x12, #0x0               ; =0
1000072b4:     	mov	x16, #0x0               ; =0
1000072b8:     	mov	x0, #0x0                ; =0
1000072bc:     	mov	x6, #0x0                ; =0
1000072c0:     	mov	x5, #0x0                ; =0
1000072c4:     	mov	x7, #0x0                ; =0
1000072c8:     	mov	x19, #0x0               ; =0
1000072cc:     	add	x14, x3, #0x20
1000072d0:     	add	x15, x1, #0x20
1000072d4:     	str	x9, [sp, #0x10]
1000072d8:     	stp	x16, x12, [sp, #0x20]
1000072dc:     	stp	x0, x5, [sp, #0x48]
1000072e0:     	stp	x6, x2, [sp, #0x60]
1000072e4:     	str	x19, [sp, #0x70]
1000072e8:     	stur	x7, [x29, #-0x68]
1000072ec:     	ldp	x1, x6, [x14, #-0x20]
1000072f0:     	ldp	x7, x19, [x14, #-0x10]
1000072f4:     	ldp	x3, x28, [x15, #-0x20]
1000072f8:     	ldp	x23, x25, [x14]
1000072fc:     	ldp	x26, x24, [x15, #-0x10]
100007300:     	ldp	x22, x21, [x15]
100007304:     	umulh	x8, x1, x3
100007308:     	umulh	x11, x6, x3
10000730c:     	mul	x0, x6, x3
100007310:     	adds	x8, x8, x0
100007314:     	cinc	x11, x11, hs
100007318:     	umulh	x0, x28, x1
10000731c:     	mul	x5, x28, x1
100007320:     	adds	x8, x5, x8
100007324:     	str	x8, [sp, #0x58]
100007328:     	cinc	x8, x0, hs
10000732c:     	umulh	x0, x7, x3
100007330:     	mul	x5, x7, x3
100007334:     	adds	x11, x11, x5
100007338:     	umulh	x5, x28, x6
10000733c:     	cinc	x0, x0, hs
100007340:     	mul	x20, x28, x6
100007344:     	adds	x11, x20, x11
100007348:     	cinc	x5, x5, hs
10000734c:     	adds	x8, x11, x8
100007350:     	umulh	x11, x26, x1
100007354:     	cinc	x5, x5, hs
100007358:     	mul	x20, x26, x1
10000735c:     	adds	x8, x8, x20
100007360:     	str	x8, [sp, #0x40]
100007364:     	cinc	x8, x11, hs
100007368:     	umulh	x11, x19, x3
10000736c:     	mul	x20, x19, x3
100007370:     	adds	x0, x0, x20
100007374:     	cinc	x11, x11, hs
100007378:     	umulh	x20, x28, x7
10000737c:     	mul	x27, x28, x7
100007380:     	adds	x0, x27, x0
100007384:     	cinc	x20, x20, hs
100007388:     	adds	x0, x0, x5
10000738c:     	cinc	x5, x20, hs
100007390:     	umulh	x20, x26, x6
100007394:     	mul	x27, x26, x6
100007398:     	adds	x8, x8, x27
10000739c:     	cinc	x20, x20, hs
1000073a0:     	adds	x8, x8, x0
1000073a4:     	cinc	x0, x20, hs
1000073a8:     	umulh	x20, x24, x1
1000073ac:     	mul	x27, x24, x1
1000073b0:     	adds	x8, x8, x27
1000073b4:     	str	x8, [sp, #0x38]
1000073b8:     	umulh	x8, x23, x3
1000073bc:     	cinc	x20, x20, hs
1000073c0:     	mul	x27, x23, x3
1000073c4:     	adds	x11, x11, x27
1000073c8:     	cinc	x8, x8, hs
1000073cc:     	umulh	x27, x28, x19
1000073d0:     	mul	x30, x28, x19
1000073d4:     	adds	x11, x30, x11
1000073d8:     	cinc	x27, x27, hs
1000073dc:     	adds	x11, x11, x5
1000073e0:     	cinc	x5, x27, hs
1000073e4:     	umulh	x27, x26, x7
1000073e8:     	mul	x30, x26, x7
1000073ec:     	adds	x11, x11, x30
1000073f0:     	cinc	x27, x27, hs
1000073f4:     	adds	x11, x11, x0
1000073f8:     	cinc	x0, x27, hs
1000073fc:     	umulh	x27, x24, x6
100007400:     	mul	x30, x24, x6
100007404:     	adds	x20, x20, x30
100007408:     	cinc	x27, x27, hs
10000740c:     	adds	x11, x20, x11
100007410:     	cinc	x20, x27, hs
100007414:     	umulh	x27, x22, x1
100007418:     	mul	x30, x22, x1
10000741c:     	adds	x11, x11, x30
100007420:     	str	x11, [sp, #0x30]
100007424:     	cinc	x11, x27, hs
100007428:     	umulh	x27, x25, x3
10000742c:     	mul	x12, x25, x3
100007430:     	adds	x8, x8, x12
100007434:     	umulh	x12, x28, x23
100007438:     	cinc	x13, x27, hs
10000743c:     	mul	x27, x28, x23
100007440:     	adds	x8, x8, x27
100007444:     	cinc	x12, x12, hs
100007448:     	adds	x8, x8, x5
10000744c:     	umulh	x5, x26, x19
100007450:     	cinc	x12, x12, hs
100007454:     	mul	x27, x26, x19
100007458:     	adds	x8, x8, x27
10000745c:     	cinc	x5, x5, hs
100007460:     	adds	x8, x8, x0
100007464:     	umulh	x0, x24, x7
100007468:     	cinc	x5, x5, hs
10000746c:     	mul	x27, x24, x7
100007470:     	adds	x8, x8, x27
100007474:     	cinc	x0, x0, hs
100007478:     	adds	x8, x8, x20
10000747c:     	umulh	x20, x22, x6
100007480:     	cinc	x0, x0, hs
100007484:     	mul	x27, x22, x6
100007488:     	adds	x11, x11, x27
10000748c:     	cinc	x20, x20, hs
100007490:     	adds	x8, x11, x8
100007494:     	umulh	x11, x21, x1
100007498:     	cinc	x16, x20, hs
10000749c:     	mul	x20, x21, x1
1000074a0:     	adds	x8, x8, x20
1000074a4:     	str	x8, [sp, #0x18]
1000074a8:     	cinc	x11, x11, hs
1000074ac:     	ldp	x20, x27, [x14, #0x10]
1000074b0:     	mul	x17, x20, x3
1000074b4:     	adds	x13, x13, x17
1000074b8:     	umulh	x17, x20, x3
1000074bc:     	cinc	x17, x17, hs
1000074c0:     	mul	x30, x28, x25
1000074c4:     	adds	x13, x13, x30
1000074c8:     	umulh	x30, x28, x25
1000074cc:     	cinc	x30, x30, hs
1000074d0:     	adds	x12, x13, x12
1000074d4:     	cinc	x13, x30, hs
1000074d8:     	mul	x30, x26, x23
1000074dc:     	adds	x12, x12, x30
1000074e0:     	umulh	x30, x26, x23
1000074e4:     	cinc	x30, x30, hs
1000074e8:     	adds	x12, x12, x5
1000074ec:     	cinc	x30, x30, hs
1000074f0:     	mul	x5, x24, x19
1000074f4:     	adds	x12, x12, x5
1000074f8:     	umulh	x5, x24, x19
1000074fc:     	cinc	x5, x5, hs
100007500:     	adds	x12, x12, x0
100007504:     	cinc	x4, x5, hs
100007508:     	mul	x0, x22, x7
10000750c:     	adds	x12, x12, x0
100007510:     	umulh	x0, x22, x7
100007514:     	cinc	x0, x0, hs
100007518:     	adds	x12, x12, x16
10000751c:     	cinc	x16, x0, hs
100007520:     	mul	x0, x21, x6
100007524:     	adds	x11, x11, x0
100007528:     	umulh	x0, x21, x6
10000752c:     	cinc	x0, x0, hs
100007530:     	adds	x12, x11, x12
100007534:     	cinc	x2, x0, hs
100007538:     	ldp	x8, x0, [x15, #0x10]
10000753c:     	mul	x5, x8, x1
100007540:     	adds	x5, x12, x5
100007544:     	umulh	x12, x8, x1
100007548:     	cinc	x12, x12, hs
10000754c:     	mul	x9, x27, x3
100007550:     	adds	x9, x17, x9
100007554:     	ldr	x17, [x14, #0x20]
100007558:     	mul	x17, x17, x3
10000755c:     	madd	x17, x28, x27, x17
100007560:     	mov	x11, x10
100007564:     	umulh	x10, x28, x20
100007568:     	mul	x28, x28, x20
10000756c:     	madd	x17, x26, x20, x17
100007570:     	umulh	x20, x27, x3
100007574:     	madd	x17, x24, x25, x17
100007578:     	madd	x17, x22, x23, x17
10000757c:     	madd	x17, x21, x19, x17
100007580:     	madd	x17, x8, x7, x17
100007584:     	adc	x17, x17, x20
100007588:     	adds	x9, x9, x28
10000758c:     	cinc	x10, x10, hs
100007590:     	adds	x9, x9, x13
100007594:     	umulh	x13, x26, x25
100007598:     	mul	x20, x26, x25
10000759c:     	madd	x17, x0, x6, x17
1000075a0:     	ldr	x25, [x15, #0x20]
1000075a4:     	madd	x17, x25, x1, x17
1000075a8:     	adc	x10, x17, x10
1000075ac:     	adds	x9, x9, x20
1000075b0:     	cinc	x13, x13, hs
1000075b4:     	adds	x9, x9, x30
1000075b8:     	umulh	x17, x24, x23
1000075bc:     	mul	x20, x24, x23
1000075c0:     	adc	x10, x10, x13
1000075c4:     	adds	x9, x9, x20
1000075c8:     	cinc	x13, x17, hs
1000075cc:     	adds	x9, x9, x4
1000075d0:     	umulh	x17, x22, x19
1000075d4:     	mul	x4, x22, x19
1000075d8:     	adc	x10, x10, x13
1000075dc:     	adds	x9, x9, x4
1000075e0:     	cinc	x13, x17, hs
1000075e4:     	adds	x9, x9, x16
1000075e8:     	umulh	x16, x21, x7
1000075ec:     	mul	x17, x21, x7
1000075f0:     	ldur	x7, [x29, #-0x68]
1000075f4:     	adc	x10, x10, x13
1000075f8:     	adds	x9, x9, x17
1000075fc:     	umulh	x13, x8, x6
100007600:     	mul	x8, x8, x6
100007604:     	cinc	x16, x16, hs
100007608:     	adds	x8, x12, x8
10000760c:     	cinc	x12, x13, hs
100007610:     	adds	x9, x9, x2
100007614:     	ldp	x2, x19, [sp, #0x68]
100007618:     	adc	x10, x10, x16
10000761c:     	adds	x8, x8, x9
100007620:     	adc	x9, x10, x12
100007624:     	mul	x10, x0, x1
100007628:     	adds	x8, x8, x10
10000762c:     	umulh	x10, x0, x1
100007630:     	adc	x9, x9, x10
100007634:     	adds	x8, x11, x8
100007638:     	mul	x10, x1, x3
10000763c:     	cset	w11, hs
100007640:     	ldp	x16, x12, [sp, #0x20]
100007644:     	adds	x12, x12, x5
100007648:     	cset	w13, hs
10000764c:     	ldr	x17, [sp, #0x18]
100007650:     	adds	x16, x16, x17
100007654:     	cset	w17, hs
100007658:     	ldp	x0, x5, [sp, #0x48]
10000765c:     	ldp	x1, x4, [sp, #0x30]
100007660:     	adds	x0, x0, x1
100007664:     	cset	w1, hs
100007668:     	ldr	x3, [sp, #0x60]
10000766c:     	adds	x3, x3, x4
100007670:     	cset	w4, hs
100007674:     	ldr	x6, [sp, #0x40]
100007678:     	adds	x5, x5, x6
10000767c:     	cset	w6, hs
100007680:     	adds	x19, x19, x10
100007684:     	ldr	x10, [sp, #0x58]
100007688:     	adcs	x7, x7, x10
10000768c:     	adcs	x5, x5, xzr
100007690:     	cset	w10, hs
100007694:     	orr	w10, w6, w10
100007698:     	and	x10, x10, #0x1
10000769c:     	adds	x6, x3, x10
1000076a0:     	cset	w10, hs
1000076a4:     	orr	w10, w4, w10
1000076a8:     	and	x10, x10, #0x1
1000076ac:     	adds	x0, x0, x10
1000076b0:     	cset	w10, hs
1000076b4:     	orr	w10, w1, w10
1000076b8:     	and	x10, x10, #0x1
1000076bc:     	adds	x16, x16, x10
1000076c0:     	cset	w10, hs
1000076c4:     	orr	w10, w17, w10
1000076c8:     	and	x10, x10, #0x1
1000076cc:     	adds	x12, x12, x10
1000076d0:     	cset	w10, hs
1000076d4:     	orr	w10, w13, w10
1000076d8:     	and	x10, x10, #0x1
1000076dc:     	adds	x10, x8, x10
1000076e0:     	cset	w8, hs
1000076e4:     	orr	w8, w11, w8
1000076e8:     	and	x8, x8, #0x1
1000076ec:     	ldr	x11, [sp, #0x10]
1000076f0:     	add	x8, x11, x8
1000076f4:     	add	x9, x8, x9
1000076f8:     	add	x14, x14, #0x48
1000076fc:     	add	x15, x15, #0x48
100007700:     	subs	x2, x2, #0x1
100007704:     	b.ne	0x1000072d4 <field_regressions::campaign::integer::existing::<9>+0x60>
100007708:     	b	0x100007730 <field_regressions::campaign::integer::existing::<9>+0x4bc>
10000770c:     	mov	x19, #0x0               ; =0
100007710:     	mov	x7, #0x0                ; =0
100007714:     	mov	x5, #0x0                ; =0
100007718:     	mov	x6, #0x0                ; =0
10000771c:     	mov	x0, #0x0                ; =0
100007720:     	mov	x16, #0x0               ; =0
100007724:     	mov	x12, #0x0               ; =0
100007728:     	mov	x10, #0x0               ; =0
10000772c:     	mov	x9, #0x0                ; =0
100007730:     	ldr	x8, [sp, #0x8]
100007734:     	stp	x19, x7, [x8]
100007738:     	stp	x5, x6, [x8, #0x10]
10000773c:     	stp	x0, x16, [x8, #0x20]
100007740:     	stp	x12, x10, [x8, #0x30]
100007744:     	str	x9, [x8, #0x40]
100007748:     	ldp	x29, x30, [sp, #0xe0]
10000774c:     	ldp	x20, x19, [sp, #0xd0]
100007750:     	ldp	x22, x21, [sp, #0xc0]
100007754:     	ldp	x24, x23, [sp, #0xb0]
100007758:     	ldp	x26, x25, [sp, #0xa0]
10000775c:     	ldp	x28, x27, [sp, #0x90]
100007760:     	add	sp, sp, #0xf0
100007764:     	ret
100007768:     	adrp	x3, 0x1000b8000 <dyld_stub_binder+0x1000b8000>
10000776c:     	add	x3, x3, #0x970
100007770:     	sub	x0, x29, #0x60
100007774:     	sub	x1, x29, #0x58
100007778:     	mov	x2, #0x0                ; =0
10000777c:     	bl	0x100088d48 <core::panicking::assert_failed::<usize, usize>>


results/falcon-compact-opening-20261006/validation/blake3-fixed-rounds/bench:     file format elf64-x86-64


Disassembly of section .text:

0000000000029220 <bench::original::first_pow_nonce>:
   29220:	push   %rbp
   29221:	mov    %rsp,%rbp
   29224:	push   %r15
   29226:	push   %r14
   29228:	push   %r13
   2922a:	push   %r12
   2922c:	push   %rbx
   2922d:	and    $0xffffffffffffffc0,%rsp
   29231:	sub    $0xb80,%rsp
   29238:	mov    %r8d,0x14(%rsp)
   2923d:	mov    %rdi,0x40(%rsp)
   29242:	cmp    $0x39,%rsi
   29246:	jae    2b694 <bench::original::first_pow_nonce+0x2474>
   2924c:	mov    %rdx,%rbx
   2924f:	mov    %rsi,0x18(%rsp)
   29254:	test   $0x3,%sil
   29258:	je     292fb <bench::original::first_pow_nonce+0xdb>
   2925e:	cmp    %rbx,%rcx
   29261:	lea    0x8(%rsi),%rax
   29265:	mov    %rbx,%r14
   29268:	cmovbe %rbx,%rcx
   2926c:	mov    %rax,0x20(%rsp)
   29271:	data16 data16 data16 data16 data16 cs nopw 0x0(%rax,%rax,1)
   29280:	mov    %r14,%r12
   29283:	cmp    %r14,%rcx
   29286:	je     29741 <bench::original::first_pow_nonce+0x521>
   2928c:	mov    0x18(%rsp),%rbx
   29291:	mov    0x40(%rsp),%rsi
   29296:	lea    0x100(%rsp),%r15
   2929e:	vpxor  %xmm0,%xmm0,%xmm0
   292a2:	lea    0x1(%r12),%r14
   292a7:	vmovdqa64 %zmm0,0x100(%rsp)
   292af:	mov    %rcx,%r13
   292b2:	mov    %r15,%rdi
   292b5:	mov    %rbx,%rdx
   292b8:	vzeroupper
   292bb:	call   *0x6461f(%rip)        # 8d8e0 <memcpy@GLIBC_2.14>
   292c1:	mov    0x20(%rsp),%rdx
   292c6:	mov    %r12,0x100(%rsp,%rbx,1)
   292ce:	lea    0x60(%rsp),%rbx
   292d3:	mov    %r15,%rsi
   292d6:	mov    %rbx,%rdi
   292d9:	call   *0x64701(%rip)        # 8d9e0 <_DYNAMIC+0x3e8>
   292df:	mov    %rbx,%rdi
   292e2:	call   2b9c0 <bench::original::leading_zero_bits>
   292e7:	mov    %r13,%rcx
   292ea:	cmp    0x14(%rsp),%eax
   292ee:	jb     29280 <bench::original::first_pow_nonce+0x60>
   292f0:	dec    %r14
   292f3:	mov    %r14,%r13
   292f6:	jmp    29744 <bench::original::first_pow_nonce+0x524>
   292fb:	mov    %rsi,%rdi
   292fe:	shr    $0x2,%rdi
   29302:	vpxor  %xmm0,%xmm0,%xmm0
   29306:	vmovdqu %ymm0,0x78(%rsp)
   2930c:	vmovdqa %ymm0,0x60(%rsp)
   29312:	mov    %rcx,0x58(%rsp)
   29317:	test   %rsi,%rsi
   2931a:	je     29751 <bench::original::first_pow_nonce+0x531>
   29320:	mov    0x40(%rsp),%rsi
   29325:	mov    0x18(%rsp),%rdx
   2932a:	mov    %rdi,%r14
   2932d:	lea    0x60(%rsp),%rdi
   29332:	vzeroupper
   29335:	call   *0x645a5(%rip)        # 8d8e0 <memcpy@GLIBC_2.14>
   2933b:	cmp    $0x4,%r14
   2933f:	je     2a025 <bench::original::first_pow_nonce+0xe05>
   29345:	mov    0x58(%rsp),%rcx
   2934a:	mov    %r14,%rdi
   2934d:	cmp    $0x8,%r14
   29351:	jne    29751 <bench::original::first_pow_nonce+0x531>
   29357:	xor    %esi,%esi
   29359:	mov    $0x20,%eax
   2935e:	sub    0x14(%rsp),%eax
   29362:	mov    $0xffffffff,%edx
   29367:	cmovb  %esi,%eax
   2936a:	cmp    $0x20,%eax
   2936d:	shlx   %eax,%edx,%eax
   29372:	mov    %rcx,%rdx
   29375:	bswap  %eax
   29377:	cmovae %esi,%eax
   2937a:	sub    %rbx,%rdx
   2937d:	cmovae %rdx,%rsi
   29381:	cmp    $0xf,%rsi
   29385:	jbe    2b4df <bench::original::first_pow_nonce+0x22bf>
   2938b:	vpbroadcastd 0x60(%rsp),%zmm25
   29393:	vpbroadcastd 0x68(%rsp),%zmm27
   2939b:	vpbroadcastd 0x64(%rsp),%zmm26
   293a3:	vpbroadcastd 0x6c(%rsp),%zmm28
   293ab:	vpbroadcastd 0x78(%rsp),%zmm31
   293b3:	vpbroadcastd 0x7c(%rsp),%zmm14
   293bb:	vpbroadcastd 0x70(%rsp),%zmm29
   293c3:	vpbroadcastd 0x74(%rsp),%zmm30
   293cb:	mov    0x18(%rsp),%rcx
   293d0:	lea    0x380(%rsp),%rdx
   293d8:	lea    -0x1b207(%rip),%rdi        # e1d8 <anon.6f182dc8ab90d5ac6fa66ca75edd0c6c.1.llvm.16074353281485304474+0x1ae>
   293df:	vpaddd -0x1dcc5(%rip){1to16},%zmm25,%zmm0        # b724 <GCC_except_table20+0x368>
   293e9:	vpaddd -0x1dc8b(%rip){1to16},%zmm27,%zmm8        # b768 <GCC_except_table20+0x3ac>
   293f3:	vpaddd -0x1dd1d(%rip){1to16},%zmm31,%zmm11        # b6e0 <GCC_except_table20+0x324>
   293fd:	vpaddd -0x1dc7f(%rip){1to16},%zmm29,%zmm17        # b788 <GCC_except_table20+0x3cc>
   29407:	vmovdqa64 %zmm25,0x580(%rsp)
   2940f:	vmovdqa64 %zmm26,0x540(%rsp)
   29417:	vmovdqa64 %zmm27,0x500(%rsp)
   2941f:	vmovdqa64 %zmm28,0x8c0(%rsp)
   29427:	vmovdqa64 %zmm29,0x880(%rsp)
   2942f:	vmovdqa64 %zmm30,0x840(%rsp)
   29437:	vmovdqa64 %zmm31,0x800(%rsp)
   2943f:	vmovdqa64 %zmm14,0x7c0(%rsp)
   29447:	vprold $0x10,%zmm0,%zmm1
   2944e:	vpaddd -0x1dd98(%rip){1to16},%zmm1,%zmm2        # b6c0 <GCC_except_table20+0x304>
   29458:	vprold $0x10,%zmm8,%zmm5
   2945f:	vpaddd -0x1dcfd(%rip){1to16},%zmm5,%zmm6        # b76c <GCC_except_table20+0x3b0>
   29469:	vpaddd %zmm26,%zmm0,%zmm0
   2946f:	vpxord -0x1dd29(%rip){1to16},%zmm2,%zmm3        # b750 <GCC_except_table20+0x394>
   29479:	vpxord -0x1dd6b(%rip){1to16},%zmm6,%zmm7        # b718 <GCC_except_table20+0x35c>
   29483:	vprold $0x14,%zmm3,%zmm3
   2948a:	vprold $0x14,%zmm7,%zmm7
   29491:	vpaddd %zmm3,%zmm0,%zmm0
   29497:	vpxord %zmm1,%zmm0,%zmm1
   2949d:	vprold $0x18,%zmm1,%zmm4
   294a4:	vpaddd %zmm2,%zmm4,%zmm2
   294aa:	vpxord %zmm3,%zmm2,%zmm1
   294b0:	vpaddd %zmm28,%zmm8,%zmm3
   294b6:	vpxord -0x1dd68(%rip){1to16},%zmm11,%zmm8        # b758 <GCC_except_table20+0x39c>
   294c0:	vpaddd %zmm7,%zmm3,%zmm3
   294c6:	vprold $0x19,%zmm1,%zmm1
   294cd:	vpxord %zmm5,%zmm3,%zmm5
   294d3:	vprold $0x18,%zmm5,%zmm5
   294da:	vpaddd %zmm6,%zmm5,%zmm6
   294e0:	vpxord %zmm7,%zmm6,%zmm7
   294e6:	vprold $0x19,%zmm7,%zmm15
   294ed:	vpaddd %zmm14,%zmm11,%zmm7
   294f3:	vpxord -0x1de31(%rip){1to16},%zmm17,%zmm11        # b6cc <GCC_except_table20+0x310>
   294fd:	vprold $0x10,%zmm8,%zmm8
   29504:	vpaddd -0x1ddd6(%rip){1to16},%zmm8,%zmm9        # b738 <GCC_except_table20+0x37c>
   2950e:	vpxord -0x1ddcc(%rip){1to16},%zmm9,%zmm10        # b74c <GCC_except_table20+0x390>
   29518:	vmovdqa64 %zmm15,0x780(%rsp)
   29520:	vprold $0x10,%zmm11,%zmm11
   29527:	vpaddd -0x1ddc1(%rip){1to16},%zmm11,%zmm12        # b770 <GCC_except_table20+0x3b4>
   29531:	vpxord -0x1de37(%rip){1to16},%zmm12,%zmm13        # b704 <GCC_except_table20+0x348>
   2953b:	vprold $0x14,%zmm10,%zmm10
   29542:	vpaddd %zmm10,%zmm7,%zmm7
   29548:	vpxord %zmm8,%zmm7,%zmm8
   2954e:	vprold $0x18,%zmm8,%zmm16
   29555:	vpaddd %zmm9,%zmm16,%zmm8
   2955b:	vmovdqa64 %zmm16,0x740(%rsp)
   29563:	vprold $0x14,%zmm13,%zmm13
   2956a:	vpxord %zmm10,%zmm8,%zmm9
   29570:	vpaddd %zmm30,%zmm17,%zmm10
   29576:	vprold $0x19,%zmm9,%zmm9
   2957d:	vpaddd %zmm13,%zmm10,%zmm10
   29583:	vpxord %zmm11,%zmm10,%zmm11
   29589:	vpaddd %zmm10,%zmm9,%zmm10
   2958f:	vprold $0x18,%zmm11,%zmm11
   29596:	vpaddd %zmm12,%zmm11,%zmm17
   2959c:	vpxord %zmm13,%zmm17,%zmm12
   295a2:	vmovdqa64 %zmm17,0x700(%rsp)
   295aa:	vprold $0x19,%zmm12,%zmm12
   295b1:	vpaddd %zmm3,%zmm12,%zmm3
   295b7:	vpxord %zmm4,%zmm3,%zmm4
   295bd:	vprold $0x10,%zmm4,%zmm4
   295c4:	vpaddd %zmm8,%zmm4,%zmm8
   295ca:	vpxord %zmm12,%zmm8,%zmm12
   295d0:	vprold $0x14,%zmm12,%zmm12
   295d7:	vpaddd %zmm3,%zmm12,%zmm13
   295dd:	vpxord %zmm4,%zmm13,%zmm3
   295e3:	vmovdqa64 %zmm13,0x6c0(%rsp)
   295eb:	vprold $0x18,%zmm3,%zmm3
   295f2:	vpaddd %zmm8,%zmm3,%zmm4
   295f8:	vmovdqa64 %zmm3,%zmm8
   295fe:	vmovdqa64 %zmm8,0x680(%rsp)
   29606:	vpxord %zmm12,%zmm4,%zmm3
   2960c:	vmovdqa64 %zmm4,%zmm12
   29612:	vpxord %zmm5,%zmm10,%zmm4
   29618:	vmovdqa64 %zmm12,0x640(%rsp)
   29620:	vprold $0x10,%zmm4,%zmm4
   29627:	vprold $0x19,%zmm3,%zmm18
   2962e:	vpaddd %zmm2,%zmm4,%zmm2
   29634:	vmovdqa64 %zmm18,0x600(%rsp)
   2963c:	vpxord %zmm9,%zmm2,%zmm5
   29642:	vprold $0x14,%zmm5,%zmm5
   29649:	vpaddd %zmm10,%zmm5,%zmm9
   2964f:	vpxord %zmm4,%zmm9,%zmm3
   29655:	vmovdqa64 %zmm9,0xb00(%rsp)
   2965d:	vprold $0x18,%zmm3,%zmm10
   29664:	vpaddd %zmm2,%zmm10,%zmm19
   2966a:	vmovdqa64 %zmm10,0xac0(%rsp)
   29672:	vpxord %zmm5,%zmm19,%zmm2
   29678:	vpaddd %zmm1,%zmm7,%zmm5
   2967e:	vmovdqa64 %zmm19,0xa80(%rsp)
   29686:	vpxord %zmm11,%zmm5,%zmm3
   2968c:	vprold $0x19,%zmm2,%zmm20
   29693:	vprold $0x10,%zmm3,%zmm3
   2969a:	vmovdqa64 %zmm20,0xa40(%rsp)
   296a2:	vpaddd %zmm6,%zmm3,%zmm4
   296a8:	vpxord %zmm1,%zmm4,%zmm1
   296ae:	vprold $0x14,%zmm1,%zmm1
   296b5:	vpaddd %zmm5,%zmm1,%zmm11
   296bb:	vpxord %zmm3,%zmm11,%zmm2
   296c1:	vpbroadcastd %eax,%zmm3
   296c7:	lea    0x8(%rcx),%rax
   296cb:	vmovdqa64 %zmm11,0xa00(%rsp)
   296d3:	vmovdqa64 %zmm3,0xc0(%rsp)
   296db:	vpaddd %zmm0,%zmm15,%zmm3
   296e1:	vpaddd %zmm25,%zmm30,%zmm0
   296e7:	vprold $0x18,%zmm2,%zmm21
   296ee:	mov    %rax,0x38(%rsp)
   296f3:	vpxor  %xmm2,%xmm2,%xmm2
   296f7:	vmovdqa64 %zmm0,0x5c0(%rsp)
   296ff:	vmovdqa64 -0x1e249(%rip),%zmm0        # b4c0 <GCC_except_table20+0x104>
   29709:	vmovdqa64 %zmm3,0x900(%rsp)
   29711:	vpaddd %zmm4,%zmm21,%zmm22
   29717:	vmovdqa64 %zmm21,0x9c0(%rsp)
   2971f:	vpxord %zmm1,%zmm22,%zmm1
   29725:	vmovdqa64 %zmm22,0x980(%rsp)
   2972d:	vprold $0x19,%zmm1,%zmm23
   29734:	vmovdqa64 %zmm23,0x940(%rsp)
   2973c:	jmp    2a35e <bench::original::first_pow_nonce+0x113e>
   29741:	mov    %rcx,%r13
   29744:	xor    %eax,%eax
   29746:	cmp    %r12,%rcx
   29749:	setne  %al
   2974c:	jmp    2b64c <bench::original::first_pow_nonce+0x242c>
   29751:	xor    %esi,%esi
   29753:	mov    $0x20,%eax
   29758:	sub    0x14(%rsp),%eax
   2975c:	mov    $0xffffffff,%edx
   29761:	cmovb  %esi,%eax
   29764:	cmp    $0x20,%eax
   29767:	shlx   %eax,%edx,%eax
   2976c:	mov    %rcx,%rdx
   2976f:	bswap  %eax
   29771:	cmovae %esi,%eax
   29774:	sub    %rbx,%rdx
   29777:	cmovae %rdx,%rsi
   2977b:	cmp    $0xf,%rsi
   2977f:	jbe    29f84 <bench::original::first_pow_nonce+0xd64>
   29785:	vpbroadcastd %eax,%zmm0
   2978b:	mov    %rdi,%rax
   2978e:	shl    $0x6,%rax
   29792:	lea    0x8(,%rdi,4),%ecx
   29799:	mov    %edi,%r12d
   2979c:	and    $0x7,%r12d
   297a0:	and    $0x8,%edi
   297a3:	lea    -0x1b652(%rip),%r8        # e158 <anon.6f182dc8ab90d5ac6fa66ca75edd0c6c.1.llvm.16074353281485304474+0x12e>
   297aa:	vpxord %xmm23,%xmm23,%xmm23
   297b0:	mov    %rbx,%r14
   297b3:	lea    0x100(%rsp,%rax,1),%rax
   297bb:	vmovdqa64 %zmm0,0x540(%rsp)
   297c3:	mov    %rdi,0x500(%rsp)
   297cb:	mov    %r12,0x38(%rsp)
   297d0:	vpbroadcastd %ecx,%zmm0
   297d6:	mov    %rax,0x5c0(%rsp)
   297de:	mov    0x18(%rsp),%rax
   297e3:	vmovdqa64 %zmm0,0x580(%rsp)
   297eb:	lea    0x8(%rax),%rcx
   297ef:	mov    %rcx,0xc0(%rsp)
   297f7:	jmp    29836 <bench::original::first_pow_nonce+0x616>
   297f9:	nopl   0x0(%rax)
   29800:	mov    0x58(%rsp),%rcx
   29805:	add    $0x10,%r14
   29809:	mov    $0x0,%edx
   2980e:	mov    0x500(%rsp),%rdi
   29816:	lea    -0x1b6c5(%rip),%r8        # e158 <anon.6f182dc8ab90d5ac6fa66ca75edd0c6c.1.llvm.16074353281485304474+0x12e>
   2981d:	mov    %rcx,%rax
   29820:	sub    %r14,%rax
   29823:	cmovb  %rdx,%rax
   29827:	cmp    $0xf,%rax
   2982b:	mov    0x18(%rsp),%rax
   29830:	jbe    29f98 <bench::original::first_pow_nonce+0xd78>
   29836:	mov    %r14,0x50(%rsp)
   2983b:	vmovdqa64 %zmm23,0x4c0(%rsp)
   29843:	vmovdqa64 %zmm23,0x480(%rsp)
   2984b:	vmovdqa64 %zmm23,0x440(%rsp)
   29853:	vmovdqa64 %zmm23,0x400(%rsp)
   2985b:	vmovdqa64 %zmm23,0x3c0(%rsp)
   29863:	vmovdqa64 %zmm23,0x380(%rsp)
   2986b:	vmovdqa64 %zmm23,0x340(%rsp)
   29873:	vmovdqa64 %zmm23,0x300(%rsp)
   2987b:	vmovdqa64 %zmm23,0x2c0(%rsp)
   29883:	vmovdqa64 %zmm23,0x280(%rsp)
   2988b:	vmovdqa64 %zmm23,0x240(%rsp)
   29893:	vmovdqa64 %zmm23,0x200(%rsp)
   2989b:	vmovdqa64 %zmm23,0x1c0(%rsp)
   298a3:	vmovdqa64 %zmm23,0x180(%rsp)
   298ab:	vmovdqa64 %zmm23,0x140(%rsp)
   298b3:	vmovdqa64 %zmm23,0x100(%rsp)
   298bb:	test   %rax,%rax
   298be:	je     299a9 <bench::original::first_pow_nonce+0x789>
   298c4:	cmp    $0x20,%rax
   298c8:	jae    298d1 <bench::original::first_pow_nonce+0x6b1>
   298ca:	xor    %eax,%eax
   298cc:	jmp    29970 <bench::original::first_pow_nonce+0x750>
   298d1:	lea    0x2c0(%rsp),%rcx
   298d9:	xor    %eax,%eax
   298db:	nopl   0x0(%rax,%rax,1)
   298e0:	vpbroadcastd 0x60(%rsp,%rax,4),%zmm0
   298e8:	vbroadcastss 0x64(%rsp,%rax,4),%zmm1
   298f0:	vbroadcastss 0x68(%rsp,%rax,4),%zmm2
   298f8:	vmovdqa64 %zmm0,-0x1c0(%rcx)
   298ff:	vmovaps %zmm1,-0x180(%rcx)
   29906:	vbroadcastss 0x6c(%rsp,%rax,4),%zmm1
   2990e:	vmovaps %zmm2,-0x140(%rcx)
   29915:	vbroadcastss 0x70(%rsp,%rax,4),%zmm2
   2991d:	vmovaps %zmm1,-0x100(%rcx)
   29924:	vbroadcastss 0x74(%rsp,%rax,4),%zmm1
   2992c:	vmovaps %zmm2,-0xc0(%rcx)
   29933:	vpbroadcastd 0x78(%rsp,%rax,4),%zmm2
   2993b:	vmovaps %zmm1,-0x80(%rcx)
   29942:	vpbroadcastd 0x7c(%rsp,%rax,4),%zmm1
   2994a:	vmovdqa64 %zmm2,-0x40(%rcx)
   29951:	add    $0x8,%rax
   29955:	vmovdqa64 %zmm1,(%rcx)
   2995b:	add    $0x200,%rcx
   29962:	cmp    %rax,%rdi
   29965:	jne    298e0 <bench::original::first_pow_nonce+0x6c0>
   2996b:	test   %r12,%r12
   2996e:	je     299a9 <bench::original::first_pow_nonce+0x789>
   29970:	lea    0x60(%rsp,%rax,4),%rcx
   29975:	shl    $0x6,%rax
   29979:	xor    %edx,%edx
   2997b:	lea    0x100(%rsp,%rax,1),%rax
   29983:	data16 data16 data16 cs nopw 0x0(%rax,%rax,1)
   29990:	vpbroadcastd (%rcx,%rdx,4),%zmm0
   29997:	inc    %rdx
   2999a:	vmovdqa64 %zmm0,(%rax)
   299a0:	add    $0x40,%rax
   299a4:	cmp    %rdx,%r12
   299a7:	jne    29990 <bench::original::first_pow_nonce+0x770>
   299a9:	mov    0x50(%rsp),%rax
   299ae:	mov    0x5c0(%rsp),%rcx
   299b6:	vpbroadcastd -0x1e250(%rip),%zmm13        # b770 <GCC_except_table20+0x3b4>
   299c0:	vpbroadcastd -0x1e25e(%rip),%zmm14        # b76c <GCC_except_table20+0x3b0>
   299ca:	vpbroadcastd -0x1e314(%rip),%zmm15        # b6c0 <GCC_except_table20+0x304>
   299d4:	vpbroadcastd -0x1e2da(%rip),%zmm4        # b704 <GCC_except_table20+0x348>
   299de:	vpbroadcastd -0x1e2d0(%rip),%zmm5        # b718 <GCC_except_table20+0x35c>
   299e8:	vpbroadcastd -0x1e2a2(%rip),%zmm11        # b750 <GCC_except_table20+0x394>
   299f2:	vpbroadcastd -0x1e2c4(%rip),%zmm12        # b738 <GCC_except_table20+0x37c>
   299fc:	vmovdqa64 0x580(%rsp),%zmm6
   29a04:	mov    $0x78,%edx
   29a09:	vpxor  %xmm7,%xmm7,%xmm7
   29a0d:	vpxor  %xmm8,%xmm8,%xmm8
   29a12:	vpbroadcastd %eax,%zmm0
   29a18:	vpaddd -0x1e562(%rip),%zmm0,%zmm1        # b4c0 <GCC_except_table20+0x104>
   29a22:	shr    $0x20,%rax
   29a26:	vpbroadcastd %eax,%zmm2
   29a2c:	vmovdqa64 %zmm13,%zmm3
   29a32:	vmovdqa64 %zmm14,%zmm10
   29a38:	vmovdqa64 %zmm15,%zmm9
   29a3e:	vpcmpltud %zmm0,%zmm1,%k0
   29a45:	vmovdqa64 %zmm1,(%rcx)
   29a4b:	vpbroadcastd -0x1e2fd(%rip),%zmm1        # b758 <GCC_except_table20+0x39c>
   29a55:	vpmovm2d %k0,%zmm0
   29a5b:	vpsubd %zmm0,%zmm2,%zmm0
   29a61:	vpbroadcastd -0x1e31f(%rip),%zmm2        # b74c <GCC_except_table20+0x390>
   29a6b:	vmovdqa64 %zmm0,0x40(%rcx)
   29a72:	vmovdqa64 %zmm12,%zmm0
   29a78:	nopl   0x0(%rax,%rax,1)
   29a80:	mov    -0x78(%rdx,%r8,1),%rdi
   29a85:	cmp    $0xf,%rdi
   29a89:	ja     2b759 <bench::original::first_pow_nonce+0x2539>
   29a8f:	mov    -0x70(%rdx,%r8,1),%rcx
   29a94:	cmp    $0x10,%rcx
   29a98:	jae    2b81a <bench::original::first_pow_nonce+0x25fa>
   29a9e:	mov    -0x68(%rdx,%r8,1),%rax
   29aa3:	cmp    $0x10,%rax
   29aa7:	jae    2b741 <bench::original::first_pow_nonce+0x2521>
   29aad:	mov    -0x60(%rdx,%r8,1),%r14
   29ab2:	cmp    $0x10,%r14
   29ab6:	jae    2b802 <bench::original::first_pow_nonce+0x25e2>
   29abc:	mov    -0x58(%rdx,%r8,1),%r12
   29ac1:	cmp    $0x10,%r12
   29ac5:	jae    2b729 <bench::original::first_pow_nonce+0x2509>
   29acb:	mov    -0x50(%rdx,%r8,1),%r15
   29ad0:	cmp    $0x10,%r15
   29ad4:	jae    2b7ea <bench::original::first_pow_nonce+0x25ca>
   29ada:	mov    -0x48(%rdx,%r8,1),%r13
   29adf:	cmp    $0x10,%r13
   29ae3:	jae    2b711 <bench::original::first_pow_nonce+0x24f1>
   29ae9:	mov    -0x40(%rdx,%r8,1),%rbx
   29aee:	cmp    $0x10,%rbx
   29af2:	jae    2b7d2 <bench::original::first_pow_nonce+0x25b2>
   29af8:	mov    -0x38(%rdx,%r8,1),%r11
   29afd:	cmp    $0x10,%r11
   29b01:	jae    2b6f9 <bench::original::first_pow_nonce+0x24d9>
   29b07:	mov    -0x30(%rdx,%r8,1),%r10
   29b0c:	cmp    $0x10,%r10
   29b10:	jae    2b7ba <bench::original::first_pow_nonce+0x259a>
   29b16:	mov    -0x28(%rdx,%r8,1),%r9
   29b1b:	cmp    $0x10,%r9
   29b1f:	jae    2b6e1 <bench::original::first_pow_nonce+0x24c1>
   29b25:	mov    -0x20(%rdx,%r8,1),%rsi
   29b2a:	mov    %rsi,0x20(%rsp)
   29b2f:	cmp    $0x10,%rsi
   29b33:	jae    2b7a0 <bench::original::first_pow_nonce+0x2580>
   29b39:	mov    -0x18(%rdx,%r8,1),%rsi
   29b3e:	mov    %rsi,0x30(%rsp)
   29b43:	cmp    $0x10,%rsi
   29b47:	jae    2b6c7 <bench::original::first_pow_nonce+0x24a7>
   29b4d:	mov    -0x10(%rdx,%r8,1),%rsi
   29b52:	mov    %rsi,0x28(%rsp)
   29b57:	cmp    $0x10,%rsi
   29b5b:	jae    2b786 <bench::original::first_pow_nonce+0x2566>
   29b61:	mov    -0x8(%rdx,%r8,1),%rsi
   29b66:	mov    %rsi,0x48(%rsp)
   29b6b:	cmp    $0x10,%rsi
   29b6f:	jae    2b6ad <bench::original::first_pow_nonce+0x248d>
   29b75:	mov    (%rdx,%r8,1),%r8
   29b79:	cmp    $0x10,%r8
   29b7d:	jae    2b76e <bench::original::first_pow_nonce+0x254e>
   29b83:	shl    $0x6,%rax
   29b87:	shl    $0x6,%r12
   29b8b:	vpaddd %zmm5,%zmm14,%zmm14
   29b91:	vpaddd %zmm4,%zmm13,%zmm13
   29b97:	shl    $0x6,%rdi
   29b9b:	shl    $0x6,%r14
   29b9f:	shl    $0x6,%r15
   29ba3:	shl    $0x6,%r13
   29ba7:	vpaddd %zmm11,%zmm15,%zmm15
   29bad:	vpaddd %zmm2,%zmm12,%zmm12
   29bb3:	shl    $0x6,%rbx
   29bb7:	shl    $0x6,%rcx
   29bbb:	shl    $0x6,%r9
   29bbf:	shl    $0x6,%r11
   29bc3:	shl    $0x6,%r10
   29bc7:	shl    $0x6,%r8
   29bcb:	sub    $0xffffffffffffff80,%rdx
   29bcf:	vpaddd 0x100(%rsp,%rax,1),%zmm14,%zmm19
   29bd7:	vpaddd 0x100(%rsp,%r12,1),%zmm13,%zmm21
   29bdf:	vpaddd 0x100(%rsp,%rdi,1),%zmm15,%zmm16
   29be7:	vpaddd 0x100(%rsp,%r13,1),%zmm12,%zmm15
   29bef:	mov    0x20(%rsp),%rax
   29bf4:	mov    0x38(%rsp),%r12
   29bf9:	vpaddd 0x100(%rsp,%r14,1),%zmm19,%zmm20
   29c01:	vpaddd 0x100(%rsp,%r15,1),%zmm21,%zmm22
   29c09:	vpaddd 0x100(%rsp,%rbx,1),%zmm15,%zmm17
   29c11:	vpaddd 0x100(%rsp,%rcx,1),%zmm16,%zmm18
   29c19:	shl    $0x6,%rax
   29c1d:	vpxord %zmm7,%zmm19,%zmm7
   29c23:	vpxord %zmm6,%zmm21,%zmm6
   29c29:	vpxord %zmm1,%zmm15,%zmm1
   29c2f:	vpxord %zmm8,%zmm16,%zmm8
   29c35:	vprold $0x10,%zmm7,%zmm7
   29c3c:	vprold $0x10,%zmm6,%zmm6
   29c43:	vprold $0x10,%zmm1,%zmm1
   29c4a:	vprold $0x10,%zmm8,%zmm12
   29c51:	vpaddd %zmm10,%zmm7,%zmm10
   29c57:	vpaddd %zmm3,%zmm6,%zmm3
   29c5d:	vpaddd %zmm0,%zmm1,%zmm0
   29c63:	vpaddd %zmm9,%zmm12,%zmm8
   29c69:	vpxord %zmm5,%zmm10,%zmm5
   29c6f:	vpxord %zmm4,%zmm3,%zmm4
   29c75:	vpxord %zmm2,%zmm0,%zmm2
   29c7b:	vpxord %zmm11,%zmm8,%zmm9
   29c81:	vprold $0x14,%zmm5,%zmm5
   29c88:	vprold $0x14,%zmm4,%zmm4
   29c8f:	vprold $0x14,%zmm2,%zmm2
   29c96:	vprold $0x14,%zmm9,%zmm9
   29c9d:	vpaddd %zmm5,%zmm20,%zmm16
   29ca3:	vpaddd %zmm4,%zmm22,%zmm20
   29ca9:	vpaddd %zmm2,%zmm17,%zmm15
   29caf:	vpaddd %zmm9,%zmm18,%zmm14
   29cb5:	vpxord %zmm6,%zmm20,%zmm6
   29cbb:	vpxord %zmm7,%zmm16,%zmm7
   29cc1:	vpxord %zmm1,%zmm15,%zmm1
   29cc7:	vpxord %zmm12,%zmm14,%zmm12
   29ccd:	vprold $0x18,%zmm6,%zmm6
   29cd4:	vprold $0x18,%zmm1,%zmm1
   29cdb:	vprold $0x18,%zmm7,%zmm7
   29ce2:	vprold $0x18,%zmm12,%zmm12
   29ce9:	vpaddd %zmm3,%zmm6,%zmm3
   29cef:	vpaddd %zmm0,%zmm1,%zmm0
   29cf5:	vpaddd %zmm10,%zmm7,%zmm10
   29cfb:	vpxord %zmm4,%zmm3,%zmm4
   29d01:	vpxord %zmm2,%zmm0,%zmm2
   29d07:	vpxord %zmm5,%zmm10,%zmm5
   29d0d:	vprold $0x19,%zmm4,%zmm4
   29d14:	vprold $0x19,%zmm2,%zmm2
   29d1b:	vprold $0x19,%zmm5,%zmm5
   29d22:	vpaddd %zmm16,%zmm4,%zmm16
   29d28:	vpaddd 0x100(%rsp,%r9,1),%zmm16,%zmm19
   29d30:	vpaddd %zmm20,%zmm2,%zmm17
   29d36:	vpaddd %zmm8,%zmm12,%zmm20
   29d3c:	vpaddd %zmm14,%zmm5,%zmm11
   29d42:	vpaddd 0x100(%rsp,%r11,1),%zmm11,%zmm13
   29d4a:	vpaddd 0x100(%rsp,%rax,1),%zmm19,%zmm16
   29d52:	mov    0x30(%rsp),%rax
   29d57:	vpxord %zmm9,%zmm20,%zmm8
   29d5d:	vpaddd 0x100(%rsp,%r10,1),%zmm13,%zmm11
   29d65:	vprold $0x19,%zmm8,%zmm21
   29d6c:	vpaddd %zmm21,%zmm15,%zmm8
   29d72:	shl    $0x6,%rax
   29d76:	vpaddd 0x100(%rsp,%rax,1),%zmm17,%zmm17
   29d7e:	mov    0x28(%rsp),%rax
   29d83:	vpxord %zmm1,%zmm13,%zmm1
   29d89:	vpxord %zmm12,%zmm19,%zmm9
   29d8f:	vprold $0x10,%zmm1,%zmm1
   29d96:	vpaddd %zmm3,%zmm1,%zmm3
   29d9c:	shl    $0x6,%rax
   29da0:	vpxord %zmm5,%zmm3,%zmm5
   29da6:	vpaddd 0x100(%rsp,%rax,1),%zmm17,%zmm18
   29dae:	mov    0x48(%rsp),%rax
   29db3:	vprold $0x14,%zmm5,%zmm5
   29dba:	vpxord %zmm7,%zmm17,%zmm7
   29dc0:	vpaddd %zmm5,%zmm11,%zmm15
   29dc6:	vprold $0x10,%zmm7,%zmm7
   29dcd:	vpxord %zmm1,%zmm15,%zmm1
   29dd3:	vprold $0x18,%zmm1,%zmm1
   29dda:	shl    $0x6,%rax
   29dde:	vpaddd 0x100(%rsp,%rax,1),%zmm8,%zmm8
   29de6:	vpaddd %zmm3,%zmm1,%zmm3
   29dec:	vpaddd 0x100(%rsp,%r8,1),%zmm8,%zmm22
   29df4:	vpxord %zmm5,%zmm3,%zmm5
   29dfa:	lea    -0x1bca9(%rip),%r8        # e158 <anon.6f182dc8ab90d5ac6fa66ca75edd0c6c.1.llvm.16074353281485304474+0x12e>
   29e01:	vprold $0x19,%zmm5,%zmm5
   29e08:	vpxord %zmm6,%zmm8,%zmm6
   29e0e:	vprold $0x10,%zmm9,%zmm8
   29e15:	vpaddd %zmm20,%zmm7,%zmm9
   29e1b:	vprold $0x10,%zmm6,%zmm6
   29e22:	vpxord %zmm2,%zmm9,%zmm2
   29e28:	vpaddd %zmm0,%zmm8,%zmm0
   29e2e:	vprold $0x14,%zmm2,%zmm2
   29e35:	vpaddd %zmm10,%zmm6,%zmm10
   29e3b:	vpxord %zmm4,%zmm0,%zmm4
   29e41:	vpxord %zmm21,%zmm10,%zmm11
   29e47:	vprold $0x14,%zmm4,%zmm4
   29e4e:	vpaddd %zmm2,%zmm18,%zmm13
   29e54:	vprold $0x14,%zmm11,%zmm11
   29e5b:	vpxord %zmm7,%zmm13,%zmm7
   29e61:	vpaddd %zmm4,%zmm16,%zmm14
   29e67:	vprold $0x18,%zmm7,%zmm7
   29e6e:	vpaddd %zmm11,%zmm22,%zmm12
   29e74:	vpxord %zmm8,%zmm14,%zmm8
   29e7a:	vpxord %zmm6,%zmm12,%zmm6
   29e80:	vprold $0x18,%zmm8,%zmm8
   29e87:	vpaddd %zmm9,%zmm7,%zmm9
   29e8d:	vprold $0x18,%zmm6,%zmm6
   29e94:	vpxord %zmm2,%zmm9,%zmm2
   29e9a:	vpaddd %zmm0,%zmm8,%zmm0
   29ea0:	vprold $0x19,%zmm2,%zmm2
   29ea7:	vpaddd %zmm10,%zmm6,%zmm10
   29ead:	vpxord %zmm4,%zmm0,%zmm4
   29eb3:	vpxord %zmm11,%zmm10,%zmm11
   29eb9:	vprold $0x19,%zmm4,%zmm4
   29ec0:	vprold $0x19,%zmm11,%zmm11
   29ec7:	cmp    $0x3f8,%rdx
   29ece:	jne    29a80 <bench::original::first_pow_nonce+0x860>
   29ed4:	vpxord %zmm15,%zmm9,%zmm0
   29eda:	vptestnmd 0x540(%rsp),%zmm0,%k0
   29ee2:	mov    0x50(%rsp),%r14
   29ee7:	kortestw %k0,%k0
   29eeb:	je     29800 <bench::original::first_pow_nonce+0x5e0>
   29ef1:	cmpl   $0x21,0x14(%rsp)
   29ef6:	kmovd  %k0,%ebx
   29efa:	jb     2b661 <bench::original::first_pow_nonce+0x2441>
   29f00:	tzcnt  %ebx,%r13d
   29f05:	mov    %r14,%r15
   29f08:	mov    0x40(%rsp),%rsi
   29f0d:	lea    0x100(%rsp),%rdi
   29f15:	vmovdqa64 %zmm23,0x100(%rsp)
   29f1d:	add    %r14,%r13
   29f20:	mov    0x18(%rsp),%r14
   29f25:	mov    %r14,%rdx
   29f28:	vzeroupper
   29f2b:	call   *0x639af(%rip)        # 8d8e0 <memcpy@GLIBC_2.14>
   29f31:	mov    0xc0(%rsp),%rdx
   29f39:	mov    %r13,0x100(%rsp,%r14,1)
   29f41:	lea    0xa0(%rsp),%r14
   29f49:	lea    0x100(%rsp),%rsi
   29f51:	mov    %r14,%rdi
   29f54:	call   *0x63a86(%rip)        # 8d9e0 <_DYNAMIC+0x3e8>
   29f5a:	mov    %r14,%rdi
   29f5d:	call   2b9c0 <bench::original::leading_zero_bits>
   29f62:	cmp    0x14(%rsp),%eax
   29f66:	jae    2b4d5 <bench::original::first_pow_nonce+0x22b5>
   29f6c:	lea    -0x1(%rbx),%eax
   29f6f:	mov    %r15,%r14
   29f72:	vpxord %xmm23,%xmm23,%xmm23
   29f78:	and    %bx,%ax
   29f7b:	mov    %eax,%ebx
   29f7d:	jne    29f00 <bench::original::first_pow_nonce+0xce0>
   29f7f:	jmp    29800 <bench::original::first_pow_nonce+0x5e0>
   29f84:	mov    0x18(%rsp),%rax
   29f89:	mov    %rbx,%r14
   29f8c:	add    $0x8,%rax
   29f90:	mov    %rax,0xc0(%rsp)
   29f98:	mov    0x18(%rsp),%r13
   29f9d:	cmp    %r14,%rcx
   29fa0:	cmovbe %r14,%rcx
   29fa4:	data16 data16 cs nopw 0x0(%rax,%rax,1)
   29fb0:	cmp    %r14,%rcx
   29fb3:	je     2b579 <bench::original::first_pow_nonce+0x2359>
   29fb9:	mov    0x40(%rsp),%rsi
   29fbe:	lea    0x100(%rsp),%r15
   29fc6:	vpxor  %xmm0,%xmm0,%xmm0
   29fca:	lea    0x1(%r14),%rbx
   29fce:	vmovdqa64 %zmm0,0x100(%rsp)
   29fd6:	mov    %r13,%rdx
   29fd9:	mov    %rcx,%r12
   29fdc:	mov    %r15,%rdi
   29fdf:	vzeroupper
   29fe2:	call   *0x638f8(%rip)        # 8d8e0 <memcpy@GLIBC_2.14>
   29fe8:	mov    0xc0(%rsp),%rdx
   29ff0:	mov    %r14,0x100(%rsp,%r13,1)
   29ff8:	lea    0xa0(%rsp),%r14
   2a000:	mov    %r15,%rsi
   2a003:	mov    %r14,%rdi
   2a006:	call   *0x639d4(%rip)        # 8d9e0 <_DYNAMIC+0x3e8>
   2a00c:	mov    %r14,%rdi
   2a00f:	call   2b9c0 <bench::original::leading_zero_bits>
   2a014:	mov    %r12,%rcx
   2a017:	mov    %rbx,%r14
   2a01a:	cmp    0x14(%rsp),%eax
   2a01e:	jb     29fb0 <bench::original::first_pow_nonce+0xd90>
   2a020:	jmp    2b569 <bench::original::first_pow_nonce+0x2349>
   2a025:	xor    %ecx,%ecx
   2a027:	mov    $0x20,%eax
   2a02c:	sub    0x14(%rsp),%eax
   2a030:	mov    0x58(%rsp),%r8
   2a035:	mov    $0xffffffff,%edx
   2a03a:	mov    %rbx,%r13
   2a03d:	cmovb  %ecx,%eax
   2a040:	cmp    $0x20,%eax
   2a043:	shlx   %eax,%edx,%eax
   2a048:	mov    %r8,%rdx
   2a04b:	bswap  %eax
   2a04d:	cmovae %ecx,%eax
   2a050:	sub    %rbx,%rdx
   2a053:	cmovae %rdx,%rcx
   2a057:	cmp    $0xf,%rcx
   2a05b:	jbe    2b583 <bench::original::first_pow_nonce+0x2363>
   2a061:	vpbroadcastd 0x60(%rsp),%zmm25
   2a069:	vpbroadcastd 0x64(%rsp),%zmm26
   2a071:	vpbroadcastd 0x68(%rsp),%zmm27
   2a079:	vpbroadcastd 0x6c(%rsp),%zmm28
   2a081:	vpbroadcastd -0x1e91b(%rip),%zmm8        # b770 <GCC_except_table20+0x3b4>
   2a08b:	vpbroadcastd -0x1e991(%rip),%zmm11        # b704 <GCC_except_table20+0x348>
   2a095:	vpbroadcastd -0x1e913(%rip),%zmm12        # b78c <GCC_except_table20+0x3d0>
   2a09f:	vpbroadcastd -0x1e969(%rip),%zmm14        # b740 <GCC_except_table20+0x384>
   2a0a9:	mov    $0x40,%edx
   2a0ae:	vpaddd -0x1e994(%rip){1to16},%zmm25,%zmm0        # b724 <GCC_except_table20+0x368>
   2a0b8:	vpaddd -0x1e95a(%rip){1to16},%zmm27,%zmm5        # b768 <GCC_except_table20+0x3ac>
   2a0c2:	vmovdqa64 %zmm25,0x540(%rsp)
   2a0ca:	vmovdqa64 %zmm26,0x500(%rsp)
   2a0d2:	vmovdqa64 %zmm27,0x8c0(%rsp)
   2a0da:	vmovdqa64 %zmm28,0x880(%rsp)
   2a0e2:	vprold $0x10,%zmm0,%zmm1
   2a0e9:	vpaddd -0x1ea33(%rip){1to16},%zmm1,%zmm2        # b6c0 <GCC_except_table20+0x304>
   2a0f3:	vpaddd %zmm26,%zmm0,%zmm0
   2a0f9:	vpxord -0x1e9b3(%rip){1to16},%zmm2,%zmm3        # b750 <GCC_except_table20+0x394>
   2a103:	vprold $0x14,%zmm3,%zmm3
   2a10a:	vpaddd %zmm3,%zmm0,%zmm0
   2a110:	vpxord %zmm1,%zmm0,%zmm1
   2a116:	vprold $0x18,%zmm1,%zmm29
   2a11d:	vpaddd %zmm2,%zmm29,%zmm30
   2a123:	vprold $0x10,%zmm5,%zmm2
   2a12a:	vmovdqa64 %zmm29,0x840(%rsp)
   2a132:	vpxord %zmm3,%zmm30,%zmm1
   2a138:	vpaddd -0x1e9d6(%rip){1to16},%zmm2,%zmm3        # b76c <GCC_except_table20+0x3b0>
   2a142:	vmovdqa64 %zmm30,0x800(%rsp)
   2a14a:	vpxord -0x1ea3c(%rip){1to16},%zmm3,%zmm4        # b718 <GCC_except_table20+0x35c>
   2a154:	vprold $0x19,%zmm1,%zmm31
   2a15b:	vpaddd %zmm28,%zmm5,%zmm1
   2a161:	vpaddd -0x1ea2f(%rip){1to16},%zmm31,%zmm18        # b73c <GCC_except_table20+0x380>
   2a16b:	vpbroadcastd -0x1ea21(%rip),%zmm5        # b754 <GCC_except_table20+0x398>
   2a175:	vmovdqa64 %zmm31,0x7c0(%rsp)
   2a17d:	vprold $0x14,%zmm4,%zmm4
   2a184:	vmovdqa64 %zmm18,0x640(%rsp)
   2a18c:	vpaddd %zmm4,%zmm1,%zmm6
   2a192:	vpxord %zmm2,%zmm6,%zmm1
   2a198:	vmovdqa64 %zmm6,0x780(%rsp)
   2a1a0:	vprold $0x18,%zmm1,%zmm13
   2a1a7:	vpaddd %zmm3,%zmm13,%zmm17
   2a1ad:	vpxor  %xmm3,%xmm3,%xmm3
   2a1b1:	vmovdqa64 %zmm13,0x740(%rsp)
   2a1b9:	vpxord %zmm4,%zmm17,%zmm1
   2a1bf:	vmovdqa64 -0x1ecc9(%rip),%zmm4        # b500 <GCC_except_table20+0x144>
   2a1c9:	vmovdqa64 %zmm17,0x700(%rsp)
   2a1d1:	vprold $0x19,%zmm1,%zmm7
   2a1d8:	vpbroadcastd %eax,%zmm1
   2a1de:	mov    0x18(%rsp),%rax
   2a1e3:	vmovdqa64 %zmm1,0x580(%rsp)
   2a1eb:	vpaddd %zmm0,%zmm7,%zmm9
   2a1f1:	vpxord -0x1eb23(%rip){1to16},%zmm9,%zmm0        # b6d8 <GCC_except_table20+0x31c>
   2a1fb:	vmovdqa64 %zmm7,0x6c0(%rsp)
   2a203:	vmovdqa64 %zmm9,0x680(%rsp)
   2a20b:	lea    0x8(%rax),%rcx
   2a20f:	sub    %rax,%rdx
   2a212:	lea    0x100(%rsp,%rax,1),%rax
   2a21a:	mov    %rcx,0x38(%rsp)
   2a21f:	mov    %rdx,0xc0(%rsp)
   2a227:	lea    0x280(%rsp),%rcx
   2a22f:	lea    -0x1c05e(%rip),%rdx        # e1d8 <anon.6f182dc8ab90d5ac6fa66ca75edd0c6c.1.llvm.16074353281485304474+0x1ae>
   2a236:	mov    %rax,0x5c0(%rsp)
   2a23e:	vprold $0x10,%zmm0,%zmm10
   2a245:	vmovdqa64 -0x1ed8f(%rip),%zmm0        # b4c0 <GCC_except_table20+0x104>
   2a24f:	vmovdqa64 %zmm10,0x600(%rsp)
   2a257:	jmp    2ac30 <bench::original::first_pow_nonce+0x1a10>
   2a25c:	vmovdqa64 0x580(%rsp),%zmm25
   2a264:	vmovdqa64 0x540(%rsp),%zmm26
   2a26c:	vmovdqa64 0x500(%rsp),%zmm27
   2a274:	vmovdqa64 0x8c0(%rsp),%zmm28
   2a27c:	vmovdqa64 0x880(%rsp),%zmm29
   2a284:	vmovdqa64 0x840(%rsp),%zmm30
   2a28c:	vmovdqa64 0x800(%rsp),%zmm31
   2a294:	vmovdqa64 0x7c0(%rsp),%zmm14
   2a29c:	vmovdqa64 0x780(%rsp),%zmm15
   2a2a4:	vmovdqa64 0x740(%rsp),%zmm16
   2a2ac:	vmovdqa64 0x700(%rsp),%zmm17
   2a2b4:	vmovdqa64 0x6c0(%rsp),%zmm13
   2a2bc:	vmovdqa64 0x680(%rsp),%zmm8
   2a2c4:	vmovdqa64 0x640(%rsp),%zmm12
   2a2cc:	vmovdqa64 0x600(%rsp),%zmm18
   2a2d4:	vmovdqa64 0xb00(%rsp),%zmm9
   2a2dc:	vmovdqa64 0xac0(%rsp),%zmm10
   2a2e4:	vmovdqa64 0xa80(%rsp),%zmm19
   2a2ec:	vmovdqa64 0xa40(%rsp),%zmm20
   2a2f4:	vmovdqa64 0xa00(%rsp),%zmm11
   2a2fc:	vmovdqa64 0x9c0(%rsp),%zmm21
   2a304:	vmovdqa64 0x980(%rsp),%zmm22
   2a30c:	vmovdqa64 0x940(%rsp),%zmm23
   2a314:	vmovdqa64 0x900(%rsp),%zmm3
   2a31c:	vmovdqa64 -0x1ee66(%rip),%zmm0        # b4c0 <GCC_except_table20+0x104>
   2a326:	mov    0x58(%rsp),%rcx
   2a32b:	add    $0x10,%rdx
   2a32f:	lea    -0x1c15e(%rip),%rdi        # e1d8 <anon.6f182dc8ab90d5ac6fa66ca75edd0c6c.1.llvm.16074353281485304474+0x1ae>
   2a336:	vpxor  %xmm2,%xmm2,%xmm2
   2a33a:	mov    %rdx,%rbx
   2a33d:	mov    %rcx,%rax
   2a340:	sub    %rdx,%rax
   2a343:	mov    $0x0,%edx
   2a348:	cmovb  %rdx,%rax
   2a34c:	lea    0x380(%rsp),%rdx
   2a354:	cmp    $0xf,%rax
   2a358:	jbe    2b4ed <bench::original::first_pow_nonce+0x22cd>
   2a35e:	vpbroadcastd %ebx,%zmm1
   2a364:	mov    %rbx,0x50(%rsp)
   2a369:	shr    $0x20,%rbx
   2a36d:	vmovdqa64 %zmm25,0x100(%rsp)
   2a375:	vmovdqa64 %zmm26,0x140(%rsp)
   2a37d:	vmovdqa64 %zmm27,0x180(%rsp)
   2a385:	vmovdqa64 %zmm28,0x1c0(%rsp)
   2a38d:	vmovdqa64 %zmm29,0x200(%rsp)
   2a395:	vmovdqa64 %zmm30,0x240(%rsp)
   2a39d:	vmovdqa64 %zmm31,0x280(%rsp)
   2a3a5:	vmovdqa64 %zmm14,0x2c0(%rsp)
   2a3ad:	vmovdqa64 %zmm2,0x140(%rdx)
   2a3b4:	vmovdqa64 %zmm2,0x100(%rdx)
   2a3bb:	vmovdqa64 %zmm2,0xc0(%rdx)
   2a3c2:	vmovdqa64 %zmm2,0x80(%rdx)
   2a3c9:	vmovdqa64 %zmm2,0x40(%rdx)
   2a3d0:	vmovdqa64 %zmm2,(%rdx)
   2a3d6:	mov    %rdi,%rdx
   2a3d9:	vmovdqa64 %zmm19,%zmm14
   2a3df:	vpaddd %zmm0,%zmm1,%zmm0
   2a3e5:	vpaddd %zmm0,%zmm3,%zmm7
   2a3eb:	vpcmpltud %zmm1,%zmm0,%k0
   2a3f2:	vpbroadcastd %ebx,%zmm1
   2a3f8:	vmovdqa64 %zmm0,0x300(%rsp)
   2a400:	vpxord %zmm16,%zmm7,%zmm3
   2a406:	vpmovm2d %k0,%zmm2
   2a40c:	vpsubd %zmm2,%zmm1,%zmm1
   2a412:	vmovdqa64 %zmm13,%zmm16
   2a418:	vmovdqa64 %zmm9,%zmm13
   2a41e:	vmovdqa64 %zmm11,%zmm9
   2a424:	vmovdqa64 %zmm22,%zmm11
   2a42a:	vprold $0x10,%zmm3,%zmm3
   2a431:	vpaddd %zmm7,%zmm1,%zmm2
   2a437:	vmovdqa64 %zmm1,0x340(%rsp)
   2a43f:	vpaddd %zmm17,%zmm3,%zmm4
   2a445:	vpxord %zmm15,%zmm4,%zmm5
   2a44b:	vmovdqa64 %zmm23,%zmm15
   2a451:	vprold $0x14,%zmm5,%zmm6
   2a458:	vpaddd %zmm6,%zmm2,%zmm17
   2a45e:	vpxord %zmm3,%zmm17,%zmm2
   2a464:	vprold $0x18,%zmm2,%zmm2
   2a46b:	vpaddd %zmm4,%zmm2,%zmm5
   2a471:	vmovdqa64 %zmm20,%zmm4
   2a477:	vpxord %zmm6,%zmm5,%zmm3
   2a47d:	vmovdqa64 %zmm18,%zmm6
   2a483:	vprold $0x19,%zmm3,%zmm7
   2a48a:	vmovdqa64 %zmm12,%zmm3
   2a490:	vmovdqa64 %zmm8,%zmm12
   2a496:	vmovdqa64 %zmm21,%zmm8
   2a49c:	nopl   0x0(%rax)
   2a4a0:	mov    (%rdx),%rdi
   2a4a3:	cmp    $0xf,%rdi
   2a4a7:	ja     2b8de <bench::original::first_pow_nonce+0x26be>
   2a4ad:	mov    0x8(%rdx),%rcx
   2a4b1:	cmp    $0x10,%rcx
   2a4b5:	jae    2b99f <bench::original::first_pow_nonce+0x277f>
   2a4bb:	mov    0x10(%rdx),%rax
   2a4bf:	cmp    $0x10,%rax
   2a4c3:	jae    2b8c6 <bench::original::first_pow_nonce+0x26a6>
   2a4c9:	mov    0x18(%rdx),%r12
   2a4cd:	cmp    $0x10,%r12
   2a4d1:	jae    2b987 <bench::original::first_pow_nonce+0x2767>
   2a4d7:	mov    0x20(%rdx),%r15
   2a4db:	cmp    $0x10,%r15
   2a4df:	jae    2b8ae <bench::original::first_pow_nonce+0x268e>
   2a4e5:	mov    0x28(%rdx),%r14
   2a4e9:	cmp    $0x10,%r14
   2a4ed:	jae    2b96f <bench::original::first_pow_nonce+0x274f>
   2a4f3:	mov    0x30(%rdx),%r13
   2a4f7:	cmp    $0x10,%r13
   2a4fb:	jae    2b896 <bench::original::first_pow_nonce+0x2676>
   2a501:	mov    0x38(%rdx),%rbx
   2a505:	cmp    $0x10,%rbx
   2a509:	jae    2b957 <bench::original::first_pow_nonce+0x2737>
   2a50f:	mov    0x40(%rdx),%r11
   2a513:	cmp    $0x10,%r11
   2a517:	jae    2b87e <bench::original::first_pow_nonce+0x265e>
   2a51d:	mov    0x48(%rdx),%r10
   2a521:	cmp    $0x10,%r10
   2a525:	jae    2b93f <bench::original::first_pow_nonce+0x271f>
   2a52b:	mov    0x50(%rdx),%r9
   2a52f:	cmp    $0x10,%r9
   2a533:	jae    2b866 <bench::original::first_pow_nonce+0x2646>
   2a539:	mov    0x58(%rdx),%r8
   2a53d:	cmp    $0x10,%r8
   2a541:	jae    2b927 <bench::original::first_pow_nonce+0x2707>
   2a547:	mov    0x60(%rdx),%rsi
   2a54b:	mov    %rsi,0x20(%rsp)
   2a550:	cmp    $0x10,%rsi
   2a554:	jae    2b84c <bench::original::first_pow_nonce+0x262c>
   2a55a:	mov    0x68(%rdx),%rsi
   2a55e:	mov    %rsi,0x30(%rsp)
   2a563:	cmp    $0x10,%rsi
   2a567:	jae    2b8f3 <bench::original::first_pow_nonce+0x26d3>
   2a56d:	mov    0x70(%rdx),%rsi
   2a571:	mov    %rsi,0x28(%rsp)
   2a576:	cmp    $0x10,%rsi
   2a57a:	jae    2b832 <bench::original::first_pow_nonce+0x2612>
   2a580:	mov    0x78(%rdx),%rsi
   2a584:	mov    %rsi,0x48(%rsp)
   2a589:	cmp    $0x10,%rsi
   2a58d:	jae    2b90d <bench::original::first_pow_nonce+0x26ed>
   2a593:	shl    $0x6,%rdi
   2a597:	shl    $0x6,%r13
   2a59b:	vpaddd %zmm17,%zmm15,%zmm17
   2a5a1:	vpaddd %zmm9,%zmm4,%zmm9
   2a5a7:	shl    $0x6,%r15
   2a5ab:	shl    $0x6,%rbx
   2a5af:	vpaddd %zmm13,%zmm6,%zmm13
   2a5b5:	shl    $0x6,%rcx
   2a5b9:	shl    $0x6,%r14
   2a5bd:	shl    $0x6,%rax
   2a5c1:	vpaddd %zmm16,%zmm7,%zmm16
   2a5c7:	shl    $0x6,%r12
   2a5cb:	shl    $0x6,%r9
   2a5cf:	shl    $0x6,%r11
   2a5d3:	shl    $0x6,%r8
   2a5d7:	shl    $0x6,%r10
   2a5db:	sub    $0xffffffffffffff80,%rdx
   2a5df:	vpaddd 0x100(%rsp,%rdi,1),%zmm17,%zmm18
   2a5e7:	vpaddd 0x100(%rsp,%r13,1),%zmm9,%zmm17
   2a5ef:	vpaddd 0x100(%rsp,%r15,1),%zmm13,%zmm23
   2a5f7:	vpaddd 0x100(%rsp,%rax,1),%zmm16,%zmm21
   2a5ff:	mov    0x20(%rsp),%rax
   2a604:	vpaddd 0x100(%rsp,%rbx,1),%zmm17,%zmm19
   2a60c:	vpaddd 0x100(%rsp,%rcx,1),%zmm18,%zmm20
   2a614:	vpaddd 0x100(%rsp,%r14,1),%zmm23,%zmm24
   2a61c:	vpaddd 0x100(%rsp,%r12,1),%zmm21,%zmm22
   2a624:	shl    $0x6,%rax
   2a628:	vpxord %zmm2,%zmm17,%zmm2
   2a62e:	vpxord %zmm8,%zmm23,%zmm8
   2a634:	vpxord %zmm12,%zmm18,%zmm9
   2a63a:	vpxord %zmm10,%zmm21,%zmm10
   2a640:	vprold $0x10,%zmm2,%zmm2
   2a647:	vprold $0x10,%zmm9,%zmm13
   2a64e:	vprold $0x10,%zmm8,%zmm8
   2a655:	vpaddd %zmm3,%zmm2,%zmm3
   2a65b:	vpaddd %zmm14,%zmm13,%zmm12
   2a661:	vpaddd %zmm5,%zmm8,%zmm5
   2a667:	vpxord %zmm4,%zmm3,%zmm4
   2a66d:	vpxord %zmm6,%zmm5,%zmm6
   2a673:	vpxord %zmm15,%zmm12,%zmm9
   2a679:	vprold $0x14,%zmm4,%zmm4
   2a680:	vprold $0x14,%zmm9,%zmm14
   2a687:	vprold $0x14,%zmm6,%zmm6
   2a68e:	vprold $0x10,%zmm10,%zmm9
   2a695:	vpaddd %zmm4,%zmm19,%zmm17
   2a69b:	vpaddd %zmm14,%zmm20,%zmm16
   2a6a1:	vpaddd %zmm6,%zmm24,%zmm20
   2a6a7:	vpaddd %zmm11,%zmm9,%zmm11
   2a6ad:	vpxord %zmm2,%zmm17,%zmm2
   2a6b3:	vpxord %zmm7,%zmm11,%zmm7
   2a6b9:	vpxord %zmm8,%zmm20,%zmm8
   2a6bf:	vpxord %zmm13,%zmm16,%zmm13
   2a6c5:	vprold $0x18,%zmm2,%zmm2
   2a6cc:	vprold $0x14,%zmm7,%zmm7
   2a6d3:	vprold $0x18,%zmm8,%zmm8
   2a6da:	vprold $0x18,%zmm13,%zmm13
   2a6e1:	vpaddd %zmm3,%zmm2,%zmm3
   2a6e7:	vpaddd %zmm7,%zmm22,%zmm18
   2a6ed:	vpaddd %zmm5,%zmm8,%zmm5
   2a6f3:	vpaddd %zmm12,%zmm13,%zmm22
   2a6f9:	vpxord %zmm4,%zmm3,%zmm4
   2a6ff:	vpxord %zmm6,%zmm5,%zmm6
   2a705:	vpxord %zmm9,%zmm18,%zmm9
   2a70b:	vpxord %zmm14,%zmm22,%zmm12
   2a711:	vprold $0x19,%zmm4,%zmm4
   2a718:	vprold $0x18,%zmm9,%zmm10
   2a71f:	vprold $0x19,%zmm6,%zmm6
   2a726:	vprold $0x19,%zmm12,%zmm23
   2a72d:	vpaddd %zmm20,%zmm4,%zmm19
   2a733:	vpaddd 0x100(%rsp,%rax,1),%zmm19,%zmm19
   2a73b:	mov    0x30(%rsp),%rax
   2a740:	vpaddd %zmm11,%zmm10,%zmm9
   2a746:	vpaddd %zmm18,%zmm6,%zmm18
   2a74c:	vpaddd 0x100(%rsp,%r9,1),%zmm18,%zmm21
   2a754:	vpaddd %zmm23,%zmm17,%zmm12
   2a75a:	vpxord %zmm7,%zmm9,%zmm7
   2a760:	vpaddd 0x100(%rsp,%r8,1),%zmm21,%zmm18
   2a768:	vprold $0x19,%zmm7,%zmm7
   2a76f:	vpaddd %zmm16,%zmm7,%zmm11
   2a775:	vpaddd 0x100(%rsp,%r11,1),%zmm11,%zmm15
   2a77d:	shl    $0x6,%rax
   2a781:	vpaddd 0x100(%rsp,%r10,1),%zmm15,%zmm11
   2a789:	vpaddd 0x100(%rsp,%rax,1),%zmm19,%zmm20
   2a791:	mov    0x28(%rsp),%rax
   2a796:	vpxord %zmm13,%zmm21,%zmm13
   2a79c:	vpxord %zmm10,%zmm19,%zmm10
   2a7a2:	vprold $0x10,%zmm10,%zmm10
   2a7a9:	shl    $0x6,%rax
   2a7ad:	vpaddd %zmm22,%zmm10,%zmm14
   2a7b3:	vpaddd 0x100(%rsp,%rax,1),%zmm12,%zmm12
   2a7bb:	mov    0x48(%rsp),%rax
   2a7c0:	vpxord %zmm2,%zmm15,%zmm2
   2a7c6:	vpxord %zmm4,%zmm14,%zmm4
   2a7cc:	vprold $0x10,%zmm2,%zmm2
   2a7d3:	vprold $0x14,%zmm4,%zmm4
   2a7da:	vpaddd %zmm5,%zmm2,%zmm5
   2a7e0:	vpxord %zmm7,%zmm5,%zmm7
   2a7e6:	shl    $0x6,%rax
   2a7ea:	vprold $0x14,%zmm7,%zmm7
   2a7f1:	vpaddd 0x100(%rsp,%rax,1),%zmm12,%zmm24
   2a7f9:	lea    -0x1c3a8(%rip),%rax        # e458 <anon.6f182dc8ab90d5ac6fa66ca75edd0c6c.1.llvm.16074353281485304474+0x42e>
   2a800:	vpaddd %zmm7,%zmm11,%zmm17
   2a806:	vpxord %zmm8,%zmm12,%zmm8
   2a80c:	vprold $0x10,%zmm13,%zmm12
   2a813:	vpaddd %zmm4,%zmm20,%zmm13
   2a819:	vpxord %zmm2,%zmm17,%zmm2
   2a81f:	vprold $0x10,%zmm8,%zmm8
   2a826:	vpxord %zmm10,%zmm13,%zmm10
   2a82c:	vprold $0x18,%zmm2,%zmm2
   2a833:	vpaddd %zmm3,%zmm12,%zmm3
   2a839:	vprold $0x18,%zmm10,%zmm10
   2a840:	vpxord %zmm6,%zmm3,%zmm6
   2a846:	vpaddd %zmm5,%zmm2,%zmm5
   2a84c:	vprold $0x14,%zmm6,%zmm6
   2a853:	vpaddd %zmm14,%zmm10,%zmm14
   2a859:	vpxord %zmm7,%zmm5,%zmm7
   2a85f:	vpxord %zmm4,%zmm14,%zmm4
   2a865:	vprold $0x19,%zmm7,%zmm7
   2a86c:	vpaddd %zmm6,%zmm18,%zmm16
   2a872:	vpaddd %zmm9,%zmm8,%zmm18
   2a878:	vprold $0x19,%zmm4,%zmm4
   2a87f:	vpxord %zmm23,%zmm18,%zmm9
   2a885:	vpxord %zmm12,%zmm16,%zmm11
   2a88b:	vprold $0x14,%zmm9,%zmm15
   2a892:	vprold $0x18,%zmm11,%zmm12
   2a899:	vpaddd %zmm15,%zmm24,%zmm9
   2a89f:	vpaddd %zmm3,%zmm12,%zmm3
   2a8a5:	vpxord %zmm8,%zmm9,%zmm8
   2a8ab:	vpxord %zmm6,%zmm3,%zmm6
   2a8b1:	vprold $0x18,%zmm8,%zmm8
   2a8b8:	vprold $0x19,%zmm6,%zmm6
   2a8bf:	vpaddd %zmm18,%zmm8,%zmm11
   2a8c5:	vpxord %zmm15,%zmm11,%zmm15
   2a8cb:	vprold $0x19,%zmm15,%zmm15
   2a8d2:	cmp    %rax,%rdx
   2a8d5:	jne    2a4a0 <bench::original::first_pow_nonce+0x1280>
   2a8db:	vpaddd %zmm17,%zmm15,%zmm17
   2a8e1:	vpaddd %zmm0,%zmm4,%zmm0
   2a8e7:	mov    0x50(%rsp),%rdx
   2a8ec:	vpxord %zmm12,%zmm17,%zmm12
   2a8f2:	vpaddd %zmm9,%zmm0,%zmm0
   2a8f8:	vprold $0x10,%zmm12,%zmm12
   2a8ff:	vpxord %zmm2,%zmm0,%zmm2
   2a905:	vpaddd %zmm31,%zmm0,%zmm0
   2a90b:	vprold $0x10,%zmm2,%zmm2
   2a912:	vpaddd %zmm14,%zmm12,%zmm14
   2a918:	vpxord %zmm15,%zmm14,%zmm15
   2a91e:	vpaddd %zmm3,%zmm2,%zmm3
   2a924:	vprold $0x14,%zmm15,%zmm15
   2a92b:	vpxord %zmm4,%zmm3,%zmm4
   2a931:	vprold $0x14,%zmm4,%zmm4
   2a938:	vpaddd %zmm17,%zmm15,%zmm15
   2a93e:	vpaddd %zmm7,%zmm16,%zmm17
   2a944:	vpxord %zmm12,%zmm15,%zmm12
   2a94a:	vpaddd %zmm30,%zmm17,%zmm16
   2a950:	vpaddd %zmm4,%zmm0,%zmm0
   2a956:	vprold $0x18,%zmm12,%zmm12
   2a95d:	vpxord %zmm16,%zmm10,%zmm10
   2a963:	vpxord %zmm2,%zmm0,%zmm0
   2a969:	vprold $0x10,%zmm10,%zmm10
   2a970:	vprold $0x18,%zmm0,%zmm0
   2a977:	vpaddd %zmm14,%zmm12,%zmm12
   2a97d:	vpaddd 0x5c0(%rsp),%zmm17,%zmm14
   2a985:	vpaddd %zmm10,%zmm11,%zmm11
   2a98b:	vpaddd %zmm3,%zmm0,%zmm2
   2a991:	vpxord %zmm7,%zmm11,%zmm7
   2a997:	vpxord %zmm4,%zmm2,%zmm2
   2a99d:	vprold $0x14,%zmm7,%zmm7
   2a9a4:	vprold $0x19,%zmm2,%zmm2
   2a9ab:	vpaddd %zmm7,%zmm14,%zmm14
   2a9b1:	vpxord %zmm10,%zmm14,%zmm10
   2a9b7:	vpaddd %zmm26,%zmm6,%zmm14
   2a9bd:	vprold $0x18,%zmm10,%zmm10
   2a9c4:	vpaddd %zmm11,%zmm10,%zmm11
   2a9ca:	vpxord %zmm7,%zmm11,%zmm7
   2a9d0:	vpaddd %zmm13,%zmm14,%zmm11
   2a9d6:	vpxord %zmm11,%zmm8,%zmm8
   2a9dc:	vprold $0x19,%zmm7,%zmm7
   2a9e3:	vpaddd %zmm1,%zmm11,%zmm1
   2a9e9:	vprold $0x10,%zmm8,%zmm8
   2a9f0:	vpaddd %zmm7,%zmm15,%zmm3
   2a9f6:	vpaddd %zmm5,%zmm8,%zmm5
   2a9fc:	vpxord %zmm0,%zmm3,%zmm0
   2aa02:	vpxord %zmm6,%zmm5,%zmm6
   2aa08:	vprold $0x10,%zmm0,%zmm0
   2aa0f:	vprold $0x14,%zmm6,%zmm6
   2aa16:	vpaddd %zmm6,%zmm1,%zmm1
   2aa1c:	vpxord %zmm8,%zmm1,%zmm6
   2aa22:	vpaddd %zmm28,%zmm1,%zmm1
   2aa28:	vprold $0x18,%zmm6,%zmm6
   2aa2f:	vpaddd %zmm2,%zmm1,%zmm1
   2aa35:	vpxord %zmm10,%zmm1,%zmm4
   2aa3b:	vpaddd %zmm29,%zmm1,%zmm1
   2aa41:	vpaddd %zmm5,%zmm6,%zmm5
   2aa47:	vpaddd %zmm0,%zmm5,%zmm0
   2aa4d:	vpxord %zmm7,%zmm0,%zmm0
   2aa53:	vprold $0x14,%zmm0,%zmm0
   2aa5a:	vpaddd %zmm3,%zmm0,%zmm0
   2aa60:	vprold $0x10,%zmm4,%zmm3
   2aa67:	vpaddd %zmm3,%zmm12,%zmm4
   2aa6d:	vpxord %zmm2,%zmm4,%zmm2
   2aa73:	vprold $0x14,%zmm2,%zmm2
   2aa7a:	vpaddd %zmm2,%zmm1,%zmm1
   2aa80:	vpxord %zmm3,%zmm1,%zmm1
   2aa86:	vprold $0x18,%zmm1,%zmm1
   2aa8d:	vpaddd %zmm4,%zmm1,%zmm1
   2aa93:	vpxord %zmm0,%zmm1,%zmm0
   2aa99:	vptestnmd 0xc0(%rsp),%zmm0,%k0
   2aaa1:	kortestw %k0,%k0
   2aaa5:	je     2a25c <bench::original::first_pow_nonce+0x103c>
   2aaab:	cmpl   $0x21,0x14(%rsp)
   2aab0:	kmovd  %k0,%ebx
   2aab4:	jb     2b672 <bench::original::first_pow_nonce+0x2452>
   2aaba:	nopw   0x0(%rax,%rax,1)
   2aac0:	mov    0x40(%rsp),%rsi
   2aac5:	mov    0x18(%rsp),%r14
   2aaca:	tzcnt  %ebx,%r13d
   2aacf:	lea    0x100(%rsp),%r12
   2aad7:	vpxor  %xmm0,%xmm0,%xmm0
   2aadb:	mov    %rdx,%r15
   2aade:	vmovdqa64 %zmm0,0x100(%rsp)
   2aae6:	add    %rdx,%r13
   2aae9:	mov    %r12,%rdi
   2aaec:	mov    %r14,%rdx
   2aaef:	vzeroupper
   2aaf2:	call   *0x62de8(%rip)        # 8d8e0 <memcpy@GLIBC_2.14>
   2aaf8:	mov    0x38(%rsp),%rdx
   2aafd:	mov    %r13,0x100(%rsp,%r14,1)
   2ab05:	lea    0xa0(%rsp),%r14
   2ab0d:	mov    %r12,%rsi
   2ab10:	mov    %r14,%rdi
   2ab13:	call   *0x62ec7(%rip)        # 8d9e0 <_DYNAMIC+0x3e8>
   2ab19:	mov    %r14,%rdi
   2ab1c:	call   2b9c0 <bench::original::leading_zero_bits>
   2ab21:	cmp    0x14(%rsp),%eax
   2ab25:	jae    2b4d5 <bench::original::first_pow_nonce+0x22b5>
   2ab2b:	lea    -0x1(%rbx),%eax
   2ab2e:	mov    %r15,%rdx
   2ab31:	and    %bx,%ax
   2ab34:	mov    %eax,%ebx
   2ab36:	jne    2aac0 <bench::original::first_pow_nonce+0x18a0>
   2ab38:	jmp    2a25c <bench::original::first_pow_nonce+0x103c>
   2ab3d:	mov    0x58(%rsp),%r8
   2ab42:	add    $0x10,%rcx
   2ab46:	vmovdqa64 0x540(%rsp),%zmm25
   2ab4e:	vmovdqa64 0x500(%rsp),%zmm26
   2ab56:	vmovdqa64 0x8c0(%rsp),%zmm27
   2ab5e:	vmovdqa64 0x880(%rsp),%zmm28
   2ab66:	vmovdqa64 0x840(%rsp),%zmm29
   2ab6e:	vmovdqa64 0x800(%rsp),%zmm30
   2ab76:	vmovdqa64 0x7c0(%rsp),%zmm31
   2ab7e:	vmovdqa64 0x780(%rsp),%zmm6
   2ab86:	vmovdqa64 0x740(%rsp),%zmm13
   2ab8e:	vmovdqa64 0x700(%rsp),%zmm17
   2ab96:	vmovdqa64 0x6c0(%rsp),%zmm7
   2ab9e:	vmovdqa64 0x680(%rsp),%zmm9
   2aba6:	vmovdqa64 0x640(%rsp),%zmm18
   2abae:	vmovdqa64 0x600(%rsp),%zmm10
   2abb6:	vmovdqa64 -0x1f700(%rip),%zmm0        # b4c0 <GCC_except_table20+0x104>
   2abc0:	vmovdqa64 -0x1f6ca(%rip),%zmm4        # b500 <GCC_except_table20+0x144>
   2abca:	vpbroadcastd -0x1f480(%rip),%zmm5        # b754 <GCC_except_table20+0x398>
   2abd4:	vpbroadcastd -0x1f46e(%rip),%zmm8        # b770 <GCC_except_table20+0x3b4>
   2abde:	vpbroadcastd -0x1f4e4(%rip),%zmm11        # b704 <GCC_except_table20+0x348>
   2abe8:	vpbroadcastd -0x1f466(%rip),%zmm12        # b78c <GCC_except_table20+0x3d0>
   2abf2:	vpbroadcastd -0x1f4bc(%rip),%zmm14        # b740 <GCC_except_table20+0x384>
   2abfc:	vpxor  %xmm3,%xmm3,%xmm3
   2ac00:	lea    -0x1ca2f(%rip),%rdx        # e1d8 <anon.6f182dc8ab90d5ac6fa66ca75edd0c6c.1.llvm.16074353281485304474+0x1ae>
   2ac07:	mov    %rcx,%r13
   2ac0a:	mov    %r8,%rax
   2ac0d:	sub    %rcx,%rax
   2ac10:	mov    $0x0,%ecx
   2ac15:	cmovb  %rcx,%rax
   2ac19:	lea    0x280(%rsp),%rcx
   2ac21:	cmp    $0xf,%rax
   2ac25:	mov    0x18(%rsp),%rax
   2ac2a:	jbe    2b591 <bench::original::first_pow_nonce+0x2371>
   2ac30:	vpbroadcastd %r13d,%zmm2
   2ac36:	mov    %r13,0x50(%rsp)
   2ac3b:	shr    $0x20,%r13
   2ac3f:	vmovdqa64 %zmm25,0x100(%rsp)
   2ac47:	vmovdqa64 %zmm26,0x140(%rsp)
   2ac4f:	vmovdqa64 %zmm27,0x180(%rsp)
   2ac57:	vmovdqa64 %zmm28,0x1c0(%rsp)
   2ac5f:	vmovdqa64 %zmm3,0x240(%rcx)
   2ac66:	vmovdqa64 %zmm3,0x200(%rcx)
   2ac6d:	vmovdqa64 %zmm3,0x1c0(%rcx)
   2ac74:	vmovdqa64 %zmm3,0x180(%rcx)
   2ac7b:	vmovdqa64 %zmm3,0x140(%rcx)
   2ac82:	vmovdqa64 %zmm3,0x100(%rcx)
   2ac89:	vmovdqa64 %zmm3,0xc0(%rcx)
   2ac90:	vmovdqa64 %zmm3,0x80(%rcx)
   2ac97:	vmovdqa64 %zmm3,0x40(%rcx)
   2ac9e:	vmovdqa64 %zmm3,(%rcx)
   2aca4:	vpaddd %zmm0,%zmm2,%zmm0
   2acaa:	vpbroadcastd %r13d,%zmm1
   2acb0:	vpcmpltud %zmm2,%zmm0,%k0
   2acb7:	vpaddd %zmm4,%zmm2,%zmm2
   2acbd:	vmovdqa64 %zmm0,0x200(%rsp)
   2acc5:	vpxord %zmm5,%zmm2,%zmm4
   2accb:	vpmovm2d %k0,%zmm3
   2acd1:	vpsubd %zmm3,%zmm1,%zmm1
   2acd7:	vprold $0x10,%zmm4,%zmm3
   2acde:	vpaddd %zmm2,%zmm1,%zmm2
   2ace4:	vmovdqa64 %zmm1,0x240(%rsp)
   2acec:	vpaddd %zmm8,%zmm3,%zmm4
   2acf2:	vpxord %zmm11,%zmm4,%zmm5
   2acf8:	vprold $0x14,%zmm5,%zmm5
   2acff:	vpaddd %zmm5,%zmm2,%zmm8
   2ad05:	vpxord %zmm3,%zmm8,%zmm2
   2ad0b:	vpaddd %zmm14,%zmm8,%zmm19
   2ad11:	vprold $0x18,%zmm2,%zmm11
   2ad18:	vpxord %zmm13,%zmm19,%zmm8
   2ad1e:	vprold $0x10,%zmm8,%zmm8
   2ad25:	vpaddd %zmm4,%zmm11,%zmm2
   2ad2b:	vpaddd %zmm10,%zmm2,%zmm4
   2ad31:	vpxord %zmm5,%zmm2,%zmm3
   2ad37:	vpxord %zmm7,%zmm4,%zmm2
   2ad3d:	vprold $0x19,%zmm3,%zmm3
   2ad44:	vprold $0x14,%zmm2,%zmm5
   2ad4b:	vpaddd %zmm5,%zmm9,%zmm16
   2ad51:	vpaddd %zmm3,%zmm6,%zmm9
   2ad57:	vpxord %zmm29,%zmm9,%zmm6
   2ad5d:	vpxord %zmm10,%zmm16,%zmm2
   2ad63:	vprold $0x10,%zmm6,%zmm6
   2ad6a:	vprold $0x18,%zmm2,%zmm2
   2ad71:	vpaddd %zmm12,%zmm6,%zmm10
   2ad77:	vpaddd %zmm4,%zmm2,%zmm4
   2ad7d:	vpxord %zmm3,%zmm10,%zmm3
   2ad83:	vpxord %zmm5,%zmm4,%zmm5
   2ad89:	vprold $0x14,%zmm3,%zmm12
   2ad90:	vprold $0x19,%zmm5,%zmm7
   2ad97:	vpaddd %zmm12,%zmm9,%zmm15
   2ad9d:	vpxord %zmm6,%zmm15,%zmm3
   2ada3:	vprold $0x18,%zmm3,%zmm9
   2adaa:	vpaddd %zmm10,%zmm9,%zmm3
   2adb0:	vpxord %zmm12,%zmm3,%zmm5
   2adb6:	vpaddd %zmm30,%zmm8,%zmm12
   2adbc:	vpxord %zmm14,%zmm12,%zmm10
   2adc2:	vprold $0x19,%zmm5,%zmm6
   2adc9:	vprold $0x14,%zmm10,%zmm14
   2add0:	vpaddd %zmm14,%zmm19,%zmm13
   2add6:	vpxord %zmm8,%zmm13,%zmm5
   2addc:	vpxord %zmm11,%zmm18,%zmm8
   2ade2:	vprold $0x10,%zmm8,%zmm8
   2ade9:	vprold $0x18,%zmm5,%zmm10
   2adf0:	vpaddd %zmm17,%zmm8,%zmm19
   2adf6:	vpaddd %zmm12,%zmm10,%zmm12
   2adfc:	vpxord %zmm31,%zmm19,%zmm11
   2ae02:	vpxord %zmm14,%zmm12,%zmm5
   2ae08:	vprold $0x14,%zmm11,%zmm17
   2ae0f:	vprold $0x19,%zmm5,%zmm5
   2ae16:	vpaddd %zmm17,%zmm18,%zmm11
   2ae1c:	vpxord %zmm8,%zmm11,%zmm8
   2ae22:	vprold $0x18,%zmm8,%zmm8
   2ae29:	vpaddd %zmm19,%zmm8,%zmm14
   2ae2f:	vpxord %zmm17,%zmm14,%zmm17
   2ae35:	vprold $0x19,%zmm17,%zmm17
   2ae3c:	nopl   0x0(%rax)
   2ae40:	mov    (%rdx),%rdi
   2ae43:	cmp    $0xf,%rdi
   2ae47:	ja     2b8de <bench::original::first_pow_nonce+0x26be>
   2ae4d:	mov    0x8(%rdx),%rcx
   2ae51:	cmp    $0x10,%rcx
   2ae55:	jae    2b99f <bench::original::first_pow_nonce+0x277f>
   2ae5b:	mov    0x10(%rdx),%rax
   2ae5f:	cmp    $0x10,%rax
   2ae63:	jae    2b8c6 <bench::original::first_pow_nonce+0x26a6>
   2ae69:	mov    0x18(%rdx),%r12
   2ae6d:	cmp    $0x10,%r12
   2ae71:	jae    2b987 <bench::original::first_pow_nonce+0x2767>
   2ae77:	mov    0x20(%rdx),%r15
   2ae7b:	cmp    $0x10,%r15
   2ae7f:	jae    2b8ae <bench::original::first_pow_nonce+0x268e>
   2ae85:	mov    0x28(%rdx),%r14
   2ae89:	cmp    $0x10,%r14
   2ae8d:	jae    2b96f <bench::original::first_pow_nonce+0x274f>
   2ae93:	mov    0x30(%rdx),%r13
   2ae97:	cmp    $0x10,%r13
   2ae9b:	jae    2b896 <bench::original::first_pow_nonce+0x2676>
   2aea1:	mov    0x38(%rdx),%rbx
   2aea5:	cmp    $0x10,%rbx
   2aea9:	jae    2b957 <bench::original::first_pow_nonce+0x2737>
   2aeaf:	mov    0x40(%rdx),%r11
   2aeb3:	cmp    $0x10,%r11
   2aeb7:	jae    2b87e <bench::original::first_pow_nonce+0x265e>
   2aebd:	mov    0x48(%rdx),%r10
   2aec1:	cmp    $0x10,%r10
   2aec5:	jae    2b93f <bench::original::first_pow_nonce+0x271f>
   2aecb:	mov    0x50(%rdx),%r9
   2aecf:	cmp    $0x10,%r9
   2aed3:	jae    2b866 <bench::original::first_pow_nonce+0x2646>
   2aed9:	mov    0x58(%rdx),%r8
   2aedd:	cmp    $0x10,%r8
   2aee1:	jae    2b927 <bench::original::first_pow_nonce+0x2707>
   2aee7:	mov    0x60(%rdx),%rsi
   2aeeb:	mov    %rsi,0x20(%rsp)
   2aef0:	cmp    $0x10,%rsi
   2aef4:	jae    2b84c <bench::original::first_pow_nonce+0x262c>
   2aefa:	mov    0x68(%rdx),%rsi
   2aefe:	mov    %rsi,0x30(%rsp)
   2af03:	cmp    $0x10,%rsi
   2af07:	jae    2b8f3 <bench::original::first_pow_nonce+0x26d3>
   2af0d:	mov    0x70(%rdx),%rsi
   2af11:	mov    %rsi,0x28(%rsp)
   2af16:	cmp    $0x10,%rsi
   2af1a:	jae    2b832 <bench::original::first_pow_nonce+0x2612>
   2af20:	mov    0x78(%rdx),%rsi
   2af24:	mov    %rsi,0x48(%rsp)
   2af29:	cmp    $0x10,%rsi
   2af2d:	jae    2b90d <bench::original::first_pow_nonce+0x26ed>
   2af33:	shl    $0x6,%r13
   2af37:	vpaddd %zmm11,%zmm5,%zmm11
   2af3d:	shl    $0x6,%rdi
   2af41:	vpaddd %zmm16,%zmm17,%zmm16
   2af47:	shl    $0x6,%rax
   2af4b:	vpaddd %zmm15,%zmm7,%zmm15
   2af51:	shl    $0x6,%r15
   2af55:	shl    $0x6,%rbx
   2af59:	vpaddd %zmm13,%zmm6,%zmm13
   2af5f:	shl    $0x6,%r12
   2af63:	shl    $0x6,%rcx
   2af67:	shl    $0x6,%r14
   2af6b:	shl    $0x6,%r11
   2af6f:	shl    $0x6,%r9
   2af73:	shl    $0x6,%r10
   2af77:	shl    $0x6,%r8
   2af7b:	sub    $0xffffffffffffff80,%rdx
   2af7f:	vpaddd 0x100(%rsp,%r13,1),%zmm11,%zmm18
   2af87:	vpaddd 0x100(%rsp,%rdi,1),%zmm16,%zmm16
   2af8f:	vpaddd 0x100(%rsp,%rax,1),%zmm15,%zmm21
   2af97:	vpaddd 0x100(%rsp,%r15,1),%zmm13,%zmm23
   2af9f:	mov    0x20(%rsp),%rax
   2afa4:	vpaddd 0x100(%rsp,%rbx,1),%zmm18,%zmm19
   2afac:	vpaddd 0x100(%rsp,%r12,1),%zmm21,%zmm22
   2afb4:	vpaddd 0x100(%rsp,%rcx,1),%zmm16,%zmm20
   2afbc:	vpaddd 0x100(%rsp,%r14,1),%zmm23,%zmm24
   2afc4:	shl    $0x6,%rax
   2afc8:	vpxord %zmm2,%zmm18,%zmm2
   2afce:	vpxord %zmm9,%zmm16,%zmm9
   2afd4:	vpxord %zmm8,%zmm23,%zmm8
   2afda:	vprold $0x10,%zmm2,%zmm2
   2afe1:	vprold $0x10,%zmm9,%zmm13
   2afe8:	vprold $0x10,%zmm8,%zmm8
   2afef:	vpaddd %zmm3,%zmm2,%zmm3
   2aff5:	vpaddd %zmm12,%zmm13,%zmm12
   2affb:	vpaddd %zmm4,%zmm8,%zmm4
   2b001:	vpxord %zmm5,%zmm3,%zmm5
   2b007:	vpxord %zmm17,%zmm12,%zmm9
   2b00d:	vpxord %zmm6,%zmm4,%zmm6
   2b013:	vprold $0x14,%zmm5,%zmm5
   2b01a:	vprold $0x14,%zmm9,%zmm15
   2b021:	vpxord %zmm10,%zmm21,%zmm9
   2b027:	vprold $0x14,%zmm6,%zmm6
   2b02e:	vprold $0x10,%zmm9,%zmm9
   2b035:	vpaddd %zmm5,%zmm19,%zmm18
   2b03b:	vpaddd %zmm15,%zmm20,%zmm16
   2b041:	vpaddd %zmm6,%zmm24,%zmm20
   2b047:	vpxord %zmm2,%zmm18,%zmm2
   2b04d:	vpaddd %zmm14,%zmm9,%zmm10
   2b053:	vpxord %zmm8,%zmm20,%zmm8
   2b059:	vpxord %zmm13,%zmm16,%zmm13
   2b05f:	vprold $0x18,%zmm2,%zmm2
   2b066:	vpxord %zmm7,%zmm10,%zmm7
   2b06c:	vprold $0x18,%zmm8,%zmm8
   2b073:	vprold $0x18,%zmm13,%zmm13
   2b07a:	vprold $0x14,%zmm7,%zmm7
   2b081:	vpaddd %zmm3,%zmm2,%zmm3
   2b087:	vpaddd %zmm4,%zmm8,%zmm4
   2b08d:	vpaddd %zmm12,%zmm13,%zmm12
   2b093:	vpxord %zmm5,%zmm3,%zmm5
   2b099:	vpaddd %zmm7,%zmm22,%zmm17
   2b09f:	vpxord %zmm6,%zmm4,%zmm6
   2b0a5:	vpxord %zmm15,%zmm12,%zmm15
   2b0ab:	vprold $0x19,%zmm5,%zmm5
   2b0b2:	vpxord %zmm9,%zmm17,%zmm9
   2b0b8:	vprold $0x19,%zmm6,%zmm6
   2b0bf:	vprold $0x19,%zmm15,%zmm22
   2b0c6:	vprold $0x18,%zmm9,%zmm9
   2b0cd:	vpaddd %zmm20,%zmm5,%zmm19
   2b0d3:	vpaddd 0x100(%rsp,%rax,1),%zmm19,%zmm19
   2b0db:	mov    0x30(%rsp),%rax
   2b0e0:	vpaddd %zmm17,%zmm6,%zmm17
   2b0e6:	vpaddd 0x100(%rsp,%r9,1),%zmm17,%zmm21
   2b0ee:	vpaddd %zmm22,%zmm18,%zmm15
   2b0f4:	vpaddd %zmm10,%zmm9,%zmm11
   2b0fa:	vpaddd 0x100(%rsp,%r8,1),%zmm21,%zmm17
   2b102:	vpxord %zmm7,%zmm11,%zmm7
   2b108:	vprold $0x19,%zmm7,%zmm7
   2b10f:	shl    $0x6,%rax
   2b113:	vpaddd %zmm16,%zmm7,%zmm10
   2b119:	vpaddd 0x100(%rsp,%r11,1),%zmm10,%zmm14
   2b121:	vpaddd 0x100(%rsp,%rax,1),%zmm19,%zmm20
   2b129:	mov    0x28(%rsp),%rax
   2b12e:	vpaddd 0x100(%rsp,%r10,1),%zmm14,%zmm10
   2b136:	vpxord %zmm13,%zmm21,%zmm13
   2b13c:	shl    $0x6,%rax
   2b140:	vpaddd 0x100(%rsp,%rax,1),%zmm15,%zmm15
   2b148:	mov    0x48(%rsp),%rax
   2b14d:	vpxord %zmm2,%zmm14,%zmm2
   2b153:	vpxord %zmm9,%zmm19,%zmm14
   2b159:	vprold $0x10,%zmm13,%zmm9
   2b160:	vprold $0x10,%zmm2,%zmm2
   2b167:	vpaddd %zmm3,%zmm9,%zmm3
   2b16d:	vpaddd %zmm4,%zmm2,%zmm4
   2b173:	vpxord %zmm6,%zmm3,%zmm6
   2b179:	vpxord %zmm7,%zmm4,%zmm7
   2b17f:	shl    $0x6,%rax
   2b183:	vprold $0x14,%zmm6,%zmm6
   2b18a:	vpaddd 0x100(%rsp,%rax,1),%zmm15,%zmm18
   2b192:	vprold $0x14,%zmm7,%zmm7
   2b199:	lea    -0x1cd48(%rip),%rax        # e458 <anon.6f182dc8ab90d5ac6fa66ca75edd0c6c.1.llvm.16074353281485304474+0x42e>
   2b1a0:	vpxord %zmm8,%zmm15,%zmm8
   2b1a6:	vpaddd %zmm7,%zmm10,%zmm16
   2b1ac:	vprold $0x10,%zmm14,%zmm10
   2b1b3:	vpaddd %zmm6,%zmm17,%zmm15
   2b1b9:	vprold $0x10,%zmm8,%zmm8
   2b1c0:	vpxord %zmm2,%zmm16,%zmm2
   2b1c6:	vpxord %zmm9,%zmm15,%zmm9
   2b1cc:	vpaddd %zmm12,%zmm10,%zmm12
   2b1d2:	vprold $0x18,%zmm2,%zmm2
   2b1d9:	vprold $0x18,%zmm9,%zmm9
   2b1e0:	vpaddd %zmm11,%zmm8,%zmm14
   2b1e6:	vpxord %zmm5,%zmm12,%zmm5
   2b1ec:	vpxord %zmm22,%zmm14,%zmm11
   2b1f2:	vprold $0x14,%zmm5,%zmm5
   2b1f9:	vpaddd %zmm4,%zmm2,%zmm4
   2b1ff:	vpaddd %zmm3,%zmm9,%zmm3
   2b205:	vprold $0x14,%zmm11,%zmm17
   2b20c:	vpxord %zmm7,%zmm4,%zmm7
   2b212:	vpxord %zmm6,%zmm3,%zmm6
   2b218:	vpaddd %zmm5,%zmm20,%zmm13
   2b21e:	vprold $0x19,%zmm7,%zmm7
   2b225:	vprold $0x19,%zmm6,%zmm6
   2b22c:	vpaddd %zmm17,%zmm18,%zmm11
   2b232:	vpxord %zmm10,%zmm13,%zmm10
   2b238:	vpxord %zmm8,%zmm11,%zmm8
   2b23e:	vprold $0x18,%zmm10,%zmm10
   2b245:	vprold $0x18,%zmm8,%zmm8
   2b24c:	vpaddd %zmm12,%zmm10,%zmm12
   2b252:	vpaddd %zmm14,%zmm8,%zmm14
   2b258:	vpxord %zmm5,%zmm12,%zmm5
   2b25e:	vpxord %zmm17,%zmm14,%zmm17
   2b264:	vprold $0x19,%zmm5,%zmm5
   2b26b:	vprold $0x19,%zmm17,%zmm17
   2b272:	cmp    %rax,%rdx
   2b275:	jne    2ae40 <bench::original::first_pow_nonce+0x1c20>
   2b27b:	vpaddd %zmm1,%zmm7,%zmm1
   2b281:	vpaddd %zmm17,%zmm16,%zmm16
   2b287:	vpaddd %zmm11,%zmm5,%zmm11
   2b28d:	mov    0x50(%rsp),%rcx
   2b292:	vpaddd %zmm15,%zmm1,%zmm1
   2b298:	vpxord %zmm9,%zmm16,%zmm9
   2b29e:	vpxord %zmm2,%zmm11,%zmm2
   2b2a4:	vpxord %zmm1,%zmm10,%zmm10
   2b2aa:	vprold $0x10,%zmm9,%zmm9
   2b2b1:	vpaddd %zmm25,%zmm1,%zmm1
   2b2b7:	vprold $0x10,%zmm2,%zmm2
   2b2be:	vprold $0x10,%zmm10,%zmm10
   2b2c5:	vpaddd %zmm12,%zmm9,%zmm12
   2b2cb:	vpaddd %zmm3,%zmm2,%zmm3
   2b2d1:	vpaddd %zmm10,%zmm14,%zmm14
   2b2d7:	vpxord %zmm17,%zmm12,%zmm17
   2b2dd:	vpxord %zmm5,%zmm3,%zmm5
   2b2e3:	vpxord %zmm7,%zmm14,%zmm7
   2b2e9:	vprold $0x14,%zmm17,%zmm17
   2b2f0:	vprold $0x14,%zmm5,%zmm5
   2b2f7:	vprold $0x14,%zmm7,%zmm7
   2b2fe:	vpaddd %zmm17,%zmm16,%zmm16
   2b304:	vpaddd %zmm7,%zmm1,%zmm1
   2b30a:	vpxord %zmm9,%zmm16,%zmm9
   2b310:	vpxord %zmm10,%zmm1,%zmm1
   2b316:	vprold $0x18,%zmm9,%zmm9
   2b31d:	vprold $0x18,%zmm1,%zmm1
   2b324:	vpaddd %zmm12,%zmm9,%zmm9
   2b32a:	vpaddd %zmm26,%zmm6,%zmm12
   2b330:	vpaddd %zmm14,%zmm1,%zmm10
   2b336:	vpxord %zmm7,%zmm10,%zmm7
   2b33c:	vpaddd %zmm13,%zmm12,%zmm10
   2b342:	vpxord %zmm10,%zmm8,%zmm8
   2b348:	vprold $0x19,%zmm7,%zmm7
   2b34f:	vprold $0x10,%zmm8,%zmm8
   2b356:	vpaddd %zmm4,%zmm8,%zmm4
   2b35c:	vpxord %zmm6,%zmm4,%zmm6
   2b362:	vprold $0x14,%zmm6,%zmm6
   2b369:	vpaddd %zmm6,%zmm10,%zmm6
   2b36f:	vpaddd %zmm11,%zmm5,%zmm10
   2b375:	vpxord %zmm2,%zmm10,%zmm2
   2b37b:	vpxord %zmm8,%zmm6,%zmm8
   2b381:	vpaddd %zmm7,%zmm16,%zmm10
   2b387:	vprold $0x18,%zmm2,%zmm2
   2b38e:	vprold $0x18,%zmm8,%zmm8
   2b395:	vpaddd %zmm3,%zmm2,%zmm3
   2b39b:	vpxord %zmm2,%zmm10,%zmm2
   2b3a1:	vpaddd %zmm4,%zmm8,%zmm4
   2b3a7:	vpxord %zmm5,%zmm3,%zmm3
   2b3ad:	vprold $0x10,%zmm2,%zmm2
   2b3b4:	vpaddd %zmm28,%zmm6,%zmm5
   2b3ba:	vprold $0x19,%zmm3,%zmm3
   2b3c1:	vpaddd %zmm2,%zmm4,%zmm2
   2b3c7:	vpaddd %zmm3,%zmm5,%zmm4
   2b3cd:	vpxord %zmm7,%zmm2,%zmm2
   2b3d3:	vpxord %zmm1,%zmm4,%zmm1
   2b3d9:	vpaddd %zmm0,%zmm4,%zmm0
   2b3df:	vprold $0x14,%zmm2,%zmm2
   2b3e6:	vprold $0x10,%zmm1,%zmm1
   2b3ed:	vpaddd %zmm2,%zmm10,%zmm2
   2b3f3:	vpaddd %zmm1,%zmm9,%zmm5
   2b3f9:	vpxord %zmm3,%zmm5,%zmm3
   2b3ff:	vprold $0x14,%zmm3,%zmm3
   2b406:	vpaddd %zmm3,%zmm0,%zmm0
   2b40c:	vpxord %zmm1,%zmm0,%zmm0
   2b412:	vprold $0x18,%zmm0,%zmm0
   2b419:	vpaddd %zmm5,%zmm0,%zmm0
   2b41f:	vpxord %zmm2,%zmm0,%zmm0
   2b425:	vptestnmd 0x580(%rsp),%zmm0,%k0
   2b42d:	kortestw %k0,%k0
   2b431:	je     2ab3d <bench::original::first_pow_nonce+0x191d>
   2b437:	cmpl   $0x21,0x14(%rsp)
   2b43c:	kmovd  %k0,%ebx
   2b440:	jb     2b683 <bench::original::first_pow_nonce+0x2463>
   2b446:	cs nopw 0x0(%rax,%rax,1)
   2b450:	mov    0x5c0(%rsp),%rdi
   2b458:	mov    0xc0(%rsp),%rdx
   2b460:	tzcnt  %ebx,%r13d
   2b465:	xor    %esi,%esi
   2b467:	mov    %rcx,%r15
   2b46a:	add    %rcx,%r13
   2b46d:	vzeroupper
   2b470:	call   *0x62622(%rip)        # 8da98 <memset@GLIBC_2.2.5>
   2b476:	mov    0x18(%rsp),%r14
   2b47b:	mov    0x40(%rsp),%rsi
   2b480:	lea    0x100(%rsp),%r12
   2b488:	mov    %r12,%rdi
   2b48b:	mov    %r14,%rdx
   2b48e:	call   *0x6244c(%rip)        # 8d8e0 <memcpy@GLIBC_2.14>
   2b494:	mov    0x38(%rsp),%rdx
   2b499:	mov    %r13,0x100(%rsp,%r14,1)
   2b4a1:	lea    0xa0(%rsp),%r14
   2b4a9:	mov    %r12,%rsi
   2b4ac:	mov    %r14,%rdi
   2b4af:	call   *0x6252b(%rip)        # 8d9e0 <_DYNAMIC+0x3e8>
   2b4b5:	mov    %r14,%rdi
   2b4b8:	call   2b9c0 <bench::original::leading_zero_bits>
   2b4bd:	cmp    0x14(%rsp),%eax
   2b4c1:	jae    2b4d5 <bench::original::first_pow_nonce+0x22b5>
   2b4c3:	lea    -0x1(%rbx),%eax
   2b4c6:	mov    %r15,%rcx
   2b4c9:	and    %bx,%ax
   2b4cc:	mov    %eax,%ebx
   2b4ce:	jne    2b450 <bench::original::first_pow_nonce+0x2230>
   2b4d0:	jmp    2ab3d <bench::original::first_pow_nonce+0x191d>
   2b4d5:	mov    $0x1,%eax
   2b4da:	jmp    2b64c <bench::original::first_pow_nonce+0x242c>
   2b4df:	mov    0x18(%rsp),%rax
   2b4e4:	add    $0x8,%rax
   2b4e8:	mov    %rax,0x38(%rsp)
   2b4ed:	mov    0x18(%rsp),%r13
   2b4f2:	cmp    %rbx,%rcx
   2b4f5:	mov    %rbx,%r14
   2b4f8:	cmovbe %rbx,%rcx
   2b4fc:	nopl   0x0(%rax)
   2b500:	cmp    %r14,%rcx
   2b503:	je     2b579 <bench::original::first_pow_nonce+0x2359>
   2b505:	mov    0x40(%rsp),%rsi
   2b50a:	lea    0x100(%rsp),%r15
   2b512:	vpxor  %xmm0,%xmm0,%xmm0
   2b516:	lea    0x1(%r14),%rbx
   2b51a:	vmovdqa64 %zmm0,0x100(%rsp)
   2b522:	mov    %r13,%rdx
   2b525:	mov    %rcx,%r12
   2b528:	mov    %r15,%rdi
   2b52b:	vzeroupper
   2b52e:	call   *0x623ac(%rip)        # 8d8e0 <memcpy@GLIBC_2.14>
   2b534:	mov    0x38(%rsp),%rdx
   2b539:	mov    %r14,0x100(%rsp,%r13,1)
   2b541:	lea    0xa0(%rsp),%r14
   2b549:	mov    %r15,%rsi
   2b54c:	mov    %r14,%rdi
   2b54f:	call   *0x6248b(%rip)        # 8d9e0 <_DYNAMIC+0x3e8>
   2b555:	mov    %r14,%rdi
   2b558:	call   2b9c0 <bench::original::leading_zero_bits>
   2b55d:	mov    %r12,%rcx
   2b560:	mov    %rbx,%r14
   2b563:	cmp    0x14(%rsp),%eax
   2b567:	jb     2b500 <bench::original::first_pow_nonce+0x22e0>
   2b569:	dec    %rbx
   2b56c:	mov    $0x1,%eax
   2b571:	mov    %rbx,%r13
   2b574:	jmp    2b64c <bench::original::first_pow_nonce+0x242c>
   2b579:	xor    %eax,%eax
   2b57b:	mov    %rcx,%r13
   2b57e:	jmp    2b64c <bench::original::first_pow_nonce+0x242c>
   2b583:	mov    0x18(%rsp),%rax
   2b588:	lea    0x8(%rax),%rcx
   2b58c:	mov    %rcx,0x38(%rsp)
   2b591:	cmp    %r13,%r8
   2b594:	mov    $0x40,%ecx
   2b599:	movq   $0x0,0x28(%rsp)
   2b5a2:	cmovbe %r13,%r8
   2b5a6:	sub    %rax,%rcx
   2b5a9:	lea    0x100(%rsp,%rax,1),%rax
   2b5b1:	mov    %rcx,0x30(%rsp)
   2b5b6:	mov    %rax,0x20(%rsp)
   2b5bb:	nopl   0x0(%rax,%rax,1)
   2b5c0:	cmp    %r13,%r8
   2b5c3:	je     2b644 <bench::original::first_pow_nonce+0x2424>
   2b5c5:	mov    0x20(%rsp),%rdi
   2b5ca:	mov    0x30(%rsp),%rdx
   2b5cf:	xor    %esi,%esi
   2b5d1:	lea    0x1(%r13),%r15
   2b5d5:	mov    %r8,%rbx
   2b5d8:	vzeroupper
   2b5db:	call   *0x624b7(%rip)        # 8da98 <memset@GLIBC_2.2.5>
   2b5e1:	mov    0x18(%rsp),%r12
   2b5e6:	mov    0x40(%rsp),%rsi
   2b5eb:	lea    0x100(%rsp),%r14
   2b5f3:	mov    %r14,%rdi
   2b5f6:	mov    %r12,%rdx
   2b5f9:	call   *0x622e1(%rip)        # 8d8e0 <memcpy@GLIBC_2.14>
   2b5ff:	mov    0x38(%rsp),%rdx
   2b604:	mov    %r13,0x100(%rsp,%r12,1)
   2b60c:	lea    0xa0(%rsp),%r12
   2b614:	mov    %r14,%rsi
   2b617:	mov    %r12,%rdi
   2b61a:	call   *0x623c0(%rip)        # 8d9e0 <_DYNAMIC+0x3e8>
   2b620:	mov    %r12,%rdi
   2b623:	call   2b9c0 <bench::original::leading_zero_bits>
   2b628:	mov    %rbx,%r8
   2b62b:	mov    %r15,%r13
   2b62e:	cmp    0x14(%rsp),%eax
   2b632:	jb     2b5c0 <bench::original::first_pow_nonce+0x23a0>
   2b634:	dec    %r15
   2b637:	mov    $0x1,%eax
   2b63c:	mov    %r15,%r8
   2b63f:	mov    %rax,0x28(%rsp)
   2b644:	mov    0x28(%rsp),%rax
   2b649:	mov    %r8,%r13
   2b64c:	mov    %r13,%rdx
   2b64f:	lea    -0x28(%rbp),%rsp
   2b653:	pop    %rbx
   2b654:	pop    %r12
   2b656:	pop    %r13
   2b658:	pop    %r14
   2b65a:	pop    %r15
   2b65c:	pop    %rbp
   2b65d:	vzeroupper
   2b660:	ret
   2b661:	tzcnt  %ebx,%eax
   2b665:	add    %rax,%r14
   2b668:	mov    $0x1,%eax
   2b66d:	mov    %r14,%r13
   2b670:	jmp    2b64c <bench::original::first_pow_nonce+0x242c>
   2b672:	tzcnt  %ebx,%eax
   2b676:	add    %rax,%rdx
   2b679:	mov    $0x1,%eax
   2b67e:	mov    %rdx,%r13
   2b681:	jmp    2b64c <bench::original::first_pow_nonce+0x242c>
   2b683:	tzcnt  %ebx,%eax
   2b687:	add    %rax,%rcx
   2b68a:	mov    $0x1,%eax
   2b68f:	mov    %rcx,%r13
   2b692:	jmp    2b64c <bench::original::first_pow_nonce+0x242c>
   2b694:	lea    -0x1cf89(%rip),%rdi        # e712 <anon.6f182dc8ab90d5ac6fa66ca75edd0c6c.1.llvm.16074353281485304474+0x6e8>
   2b69b:	lea    0x5f4be(%rip),%rdx        # 8ab60 <__frame_dummy_init_array_entry+0x630>
   2b6a2:	mov    $0x4d,%esi
   2b6a7:	call   *0x622cb(%rip)        # 8d978 <_DYNAMIC+0x380>
   2b6ad:	lea    0x5f4f4(%rip),%rdx        # 8aba8 <__frame_dummy_init_array_entry+0x678>
   2b6b4:	mov    0x48(%rsp),%rdi
   2b6b9:	mov    $0x10,%esi
   2b6be:	vzeroupper
   2b6c1:	call   *0x62261(%rip)        # 8d928 <_DYNAMIC+0x330>
   2b6c7:	lea    0x5f4da(%rip),%rdx        # 8aba8 <__frame_dummy_init_array_entry+0x678>
   2b6ce:	mov    0x30(%rsp),%rdi
   2b6d3:	mov    $0x10,%esi
   2b6d8:	vzeroupper
   2b6db:	call   *0x62247(%rip)        # 8d928 <_DYNAMIC+0x330>
   2b6e1:	lea    0x5f4c0(%rip),%rdx        # 8aba8 <__frame_dummy_init_array_entry+0x678>
   2b6e8:	mov    $0x10,%esi
   2b6ed:	mov    %r9,%rdi
   2b6f0:	vzeroupper
   2b6f3:	call   *0x6222f(%rip)        # 8d928 <_DYNAMIC+0x330>
   2b6f9:	lea    0x5f4a8(%rip),%rdx        # 8aba8 <__frame_dummy_init_array_entry+0x678>
   2b700:	mov    $0x10,%esi
   2b705:	mov    %r11,%rdi
   2b708:	vzeroupper
   2b70b:	call   *0x62217(%rip)        # 8d928 <_DYNAMIC+0x330>
   2b711:	lea    0x5f490(%rip),%rdx        # 8aba8 <__frame_dummy_init_array_entry+0x678>
   2b718:	mov    $0x10,%esi
   2b71d:	mov    %r13,%rdi
   2b720:	vzeroupper
   2b723:	call   *0x621ff(%rip)        # 8d928 <_DYNAMIC+0x330>
   2b729:	lea    0x5f478(%rip),%rdx        # 8aba8 <__frame_dummy_init_array_entry+0x678>
   2b730:	mov    $0x10,%esi
   2b735:	mov    %r12,%rdi
   2b738:	vzeroupper
   2b73b:	call   *0x621e7(%rip)        # 8d928 <_DYNAMIC+0x330>
   2b741:	lea    0x5f460(%rip),%rdx        # 8aba8 <__frame_dummy_init_array_entry+0x678>
   2b748:	mov    $0x10,%esi
   2b74d:	mov    %rax,%rdi
   2b750:	vzeroupper
   2b753:	call   *0x621cf(%rip)        # 8d928 <_DYNAMIC+0x330>
   2b759:	lea    0x5f448(%rip),%rdx        # 8aba8 <__frame_dummy_init_array_entry+0x678>
   2b760:	mov    $0x10,%esi
   2b765:	vzeroupper
   2b768:	call   *0x621ba(%rip)        # 8d928 <_DYNAMIC+0x330>
   2b76e:	lea    0x5f433(%rip),%rdx        # 8aba8 <__frame_dummy_init_array_entry+0x678>
   2b775:	mov    $0x10,%esi
   2b77a:	mov    %r8,%rdi
   2b77d:	vzeroupper
   2b780:	call   *0x621a2(%rip)        # 8d928 <_DYNAMIC+0x330>
   2b786:	lea    0x5f41b(%rip),%rdx        # 8aba8 <__frame_dummy_init_array_entry+0x678>
   2b78d:	mov    0x28(%rsp),%rdi
   2b792:	mov    $0x10,%esi
   2b797:	vzeroupper
   2b79a:	call   *0x62188(%rip)        # 8d928 <_DYNAMIC+0x330>
   2b7a0:	lea    0x5f401(%rip),%rdx        # 8aba8 <__frame_dummy_init_array_entry+0x678>
   2b7a7:	mov    0x20(%rsp),%rdi
   2b7ac:	mov    $0x10,%esi
   2b7b1:	vzeroupper
   2b7b4:	call   *0x6216e(%rip)        # 8d928 <_DYNAMIC+0x330>
   2b7ba:	lea    0x5f3e7(%rip),%rdx        # 8aba8 <__frame_dummy_init_array_entry+0x678>
   2b7c1:	mov    $0x10,%esi
   2b7c6:	mov    %r10,%rdi
   2b7c9:	vzeroupper
   2b7cc:	call   *0x62156(%rip)        # 8d928 <_DYNAMIC+0x330>
   2b7d2:	lea    0x5f3cf(%rip),%rdx        # 8aba8 <__frame_dummy_init_array_entry+0x678>
   2b7d9:	mov    $0x10,%esi
   2b7de:	mov    %rbx,%rdi
   2b7e1:	vzeroupper
   2b7e4:	call   *0x6213e(%rip)        # 8d928 <_DYNAMIC+0x330>
   2b7ea:	lea    0x5f3b7(%rip),%rdx        # 8aba8 <__frame_dummy_init_array_entry+0x678>
   2b7f1:	mov    $0x10,%esi
   2b7f6:	mov    %r15,%rdi
   2b7f9:	vzeroupper
   2b7fc:	call   *0x62126(%rip)        # 8d928 <_DYNAMIC+0x330>
   2b802:	lea    0x5f39f(%rip),%rdx        # 8aba8 <__frame_dummy_init_array_entry+0x678>
   2b809:	mov    $0x10,%esi
   2b80e:	mov    %r14,%rdi
   2b811:	vzeroupper
   2b814:	call   *0x6210e(%rip)        # 8d928 <_DYNAMIC+0x330>
   2b81a:	lea    0x5f387(%rip),%rdx        # 8aba8 <__frame_dummy_init_array_entry+0x678>
   2b821:	mov    $0x10,%esi
   2b826:	mov    %rcx,%rdi
   2b829:	vzeroupper
   2b82c:	call   *0x620f6(%rip)        # 8d928 <_DYNAMIC+0x330>
   2b832:	lea    0x5ed17(%rip),%rdx        # 8a550 <__frame_dummy_init_array_entry+0x20>
   2b839:	mov    0x28(%rsp),%rdi
   2b83e:	mov    $0x10,%esi
   2b843:	vzeroupper
   2b846:	call   *0x620dc(%rip)        # 8d928 <_DYNAMIC+0x330>
   2b84c:	lea    0x5ecfd(%rip),%rdx        # 8a550 <__frame_dummy_init_array_entry+0x20>
   2b853:	mov    0x20(%rsp),%rdi
   2b858:	mov    $0x10,%esi
   2b85d:	vzeroupper
   2b860:	call   *0x620c2(%rip)        # 8d928 <_DYNAMIC+0x330>
   2b866:	lea    0x5ece3(%rip),%rdx        # 8a550 <__frame_dummy_init_array_entry+0x20>
   2b86d:	mov    $0x10,%esi
   2b872:	mov    %r9,%rdi
   2b875:	vzeroupper
   2b878:	call   *0x620aa(%rip)        # 8d928 <_DYNAMIC+0x330>
   2b87e:	lea    0x5eccb(%rip),%rdx        # 8a550 <__frame_dummy_init_array_entry+0x20>
   2b885:	mov    $0x10,%esi
   2b88a:	mov    %r11,%rdi
   2b88d:	vzeroupper
   2b890:	call   *0x62092(%rip)        # 8d928 <_DYNAMIC+0x330>
   2b896:	lea    0x5ecb3(%rip),%rdx        # 8a550 <__frame_dummy_init_array_entry+0x20>
   2b89d:	mov    $0x10,%esi
   2b8a2:	mov    %r13,%rdi
   2b8a5:	vzeroupper
   2b8a8:	call   *0x6207a(%rip)        # 8d928 <_DYNAMIC+0x330>
   2b8ae:	lea    0x5ec9b(%rip),%rdx        # 8a550 <__frame_dummy_init_array_entry+0x20>
   2b8b5:	mov    $0x10,%esi
   2b8ba:	mov    %r15,%rdi
   2b8bd:	vzeroupper
   2b8c0:	call   *0x62062(%rip)        # 8d928 <_DYNAMIC+0x330>
   2b8c6:	lea    0x5ec83(%rip),%rdx        # 8a550 <__frame_dummy_init_array_entry+0x20>
   2b8cd:	mov    $0x10,%esi
   2b8d2:	mov    %rax,%rdi
   2b8d5:	vzeroupper
   2b8d8:	call   *0x6204a(%rip)        # 8d928 <_DYNAMIC+0x330>
   2b8de:	lea    0x5ec6b(%rip),%rdx        # 8a550 <__frame_dummy_init_array_entry+0x20>
   2b8e5:	mov    $0x10,%esi
   2b8ea:	vzeroupper
   2b8ed:	call   *0x62035(%rip)        # 8d928 <_DYNAMIC+0x330>
   2b8f3:	lea    0x5ec56(%rip),%rdx        # 8a550 <__frame_dummy_init_array_entry+0x20>
   2b8fa:	mov    0x30(%rsp),%rdi
   2b8ff:	mov    $0x10,%esi
   2b904:	vzeroupper
   2b907:	call   *0x6201b(%rip)        # 8d928 <_DYNAMIC+0x330>
   2b90d:	lea    0x5ec3c(%rip),%rdx        # 8a550 <__frame_dummy_init_array_entry+0x20>
   2b914:	mov    0x48(%rsp),%rdi
   2b919:	mov    $0x10,%esi
   2b91e:	vzeroupper
   2b921:	call   *0x62001(%rip)        # 8d928 <_DYNAMIC+0x330>
   2b927:	lea    0x5ec22(%rip),%rdx        # 8a550 <__frame_dummy_init_array_entry+0x20>
   2b92e:	mov    $0x10,%esi
   2b933:	mov    %r8,%rdi
   2b936:	vzeroupper
   2b939:	call   *0x61fe9(%rip)        # 8d928 <_DYNAMIC+0x330>
   2b93f:	lea    0x5ec0a(%rip),%rdx        # 8a550 <__frame_dummy_init_array_entry+0x20>
   2b946:	mov    $0x10,%esi
   2b94b:	mov    %r10,%rdi
   2b94e:	vzeroupper
   2b951:	call   *0x61fd1(%rip)        # 8d928 <_DYNAMIC+0x330>
   2b957:	lea    0x5ebf2(%rip),%rdx        # 8a550 <__frame_dummy_init_array_entry+0x20>
   2b95e:	mov    $0x10,%esi
   2b963:	mov    %rbx,%rdi
   2b966:	vzeroupper
   2b969:	call   *0x61fb9(%rip)        # 8d928 <_DYNAMIC+0x330>
   2b96f:	lea    0x5ebda(%rip),%rdx        # 8a550 <__frame_dummy_init_array_entry+0x20>
   2b976:	mov    $0x10,%esi
   2b97b:	mov    %r14,%rdi
   2b97e:	vzeroupper
   2b981:	call   *0x61fa1(%rip)        # 8d928 <_DYNAMIC+0x330>
   2b987:	lea    0x5ebc2(%rip),%rdx        # 8a550 <__frame_dummy_init_array_entry+0x20>
   2b98e:	mov    $0x10,%esi
   2b993:	mov    %r12,%rdi
   2b996:	vzeroupper
   2b999:	call   *0x61f89(%rip)        # 8d928 <_DYNAMIC+0x330>
   2b99f:	lea    0x5ebaa(%rip),%rdx        # 8a550 <__frame_dummy_init_array_entry+0x20>
   2b9a6:	mov    $0x10,%esi
   2b9ab:	mov    %rcx,%rdi
   2b9ae:	vzeroupper
   2b9b1:	call   *0x61f71(%rip)        # 8d928 <_DYNAMIC+0x330>


/tmp/falcon-compact-opening-binaries/baseline-v4:     file format elf64-x86-64


Disassembly of section .text:

000000000017f370 <bitz::utils::blake3x4::first_pow_nonce>:
  17f370:	push   %rbp
  17f371:	mov    %rsp,%rbp
  17f374:	push   %r15
  17f376:	push   %r14
  17f378:	push   %r13
  17f37a:	push   %r12
  17f37c:	push   %rbx
  17f37d:	and    $0xffffffffffffffc0,%rsp
  17f381:	sub    $0xb80,%rsp
  17f388:	mov    %r8d,0x3c(%rsp)
  17f38d:	mov    %rdi,0x48(%rsp)
  17f392:	cmp    $0x39,%rsi
  17f396:	jae    181551 <bitz::utils::blake3x4::first_pow_nonce+0x21e1>
  17f39c:	mov    %rcx,%rbx
  17f39f:	mov    %rsi,%r13
  17f3a2:	test   $0x3,%r13b
  17f3a6:	je     17f3ee <bitz::utils::blake3x4::first_pow_nonce+0x7e>
  17f3a8:	cmp    %rdx,%rbx
  17f3ab:	cmovbe %rdx,%rbx
  17f3af:	dec    %rdx
  17f3b2:	mov    %rbx,%r15
  17f3b5:	neg    %r15
  17f3b8:	mov    %rbx,%r12
  17f3bb:	mov    %rdx,%r14
  17f3be:	xchg   %ax,%ax
  17f3c0:	lea    (%r15,%r14,1),%rbx
  17f3c4:	cmp    $0xffffffffffffffff,%rbx
  17f3c8:	je     17f833 <bitz::utils::blake3x4::first_pow_nonce+0x4c3>
  17f3ce:	mov    0x48(%rsp),%rdi
  17f3d3:	mov    0x3c(%rsp),%ecx
  17f3d7:	inc    %r14
  17f3da:	mov    %r13,%rsi
  17f3dd:	mov    %r14,%rdx
  17f3e0:	call   17eca0 <bitz::utils::blake3x4::pow_ok>
  17f3e5:	test   %al,%al
  17f3e7:	je     17f3c0 <bitz::utils::blake3x4::first_pow_nonce+0x50>
  17f3e9:	jmp    17f836 <bitz::utils::blake3x4::first_pow_nonce+0x4c6>
  17f3ee:	mov    %r13,%r14
  17f3f1:	shr    $0x2,%r14
  17f3f5:	vpxor  %xmm0,%xmm0,%xmm0
  17f3f9:	vmovdqu %ymm0,0x98(%rsp)
  17f402:	mov    %rdx,0x40(%rsp)
  17f407:	vmovdqa %ymm0,0x80(%rsp)
  17f410:	mov    %rbx,0x70(%rsp)
  17f415:	mov    %r13,0x78(%rsp)
  17f41a:	test   %r13,%r13
  17f41d:	je     17f841 <bitz::utils::blake3x4::first_pow_nonce+0x4d1>
  17f423:	mov    0x48(%rsp),%rsi
  17f428:	lea    0x80(%rsp),%rdi
  17f430:	mov    %r13,%rdx
  17f433:	vzeroupper
  17f436:	call   *0x4349fc(%rip)        # 5b3e38 <memcpy@GLIBC_2.14>
  17f43c:	cmp    $0x4,%r14
  17f440:	je     180071 <bitz::utils::blake3x4::first_pow_nonce+0xd01>
  17f446:	cmp    $0x8,%r14
  17f44a:	jne    17f841 <bitz::utils::blake3x4::first_pow_nonce+0x4d1>
  17f450:	xor    %ecx,%ecx
  17f452:	mov    $0x20,%eax
  17f457:	sub    0x3c(%rsp),%eax
  17f45b:	mov    $0xffffffff,%edx
  17f460:	cmovb  %ecx,%eax
  17f463:	cmp    $0x20,%eax
  17f466:	shlx   %eax,%edx,%eax
  17f46b:	mov    %rbx,%rdx
  17f46e:	bswap  %eax
  17f470:	cmovae %ecx,%eax
  17f473:	sub    0x40(%rsp),%rdx
  17f478:	cmovae %rdx,%rcx
  17f47c:	cmp    $0x10,%rcx
  17f480:	jb     18147e <bitz::utils::blake3x4::first_pow_nonce+0x210e>
  17f486:	vpbroadcastd 0x80(%rsp),%zmm25
  17f48e:	vpbroadcastd 0x88(%rsp),%zmm27
  17f496:	vpbroadcastd 0x84(%rsp),%zmm26
  17f49e:	vpbroadcastd 0x8c(%rsp),%zmm28
  17f4a6:	vpbroadcastd 0x98(%rsp),%zmm31
  17f4ae:	vpbroadcastd 0x9c(%rsp),%zmm14
  17f4b6:	vpbroadcastd 0x90(%rsp),%zmm29
  17f4be:	vpbroadcastd 0x94(%rsp),%zmm30
  17f4c6:	mov    0x40(%rsp),%r15
  17f4cb:	lea    0x480(%rsp),%r12
  17f4d3:	lea    -0x11275a(%rip),%rdx        # 6cd80 <perfetto::base::(anonymous namespace)::kHexmap+0x106a0>
  17f4da:	vpaddd -0x122b68(%rip){1to16},%zmm25,%zmm0        # 5c97c <perfetto::base::(anonymous namespace)::kHexmap+0x29c>
  17f4e4:	vpaddd -0x122ae6(%rip){1to16},%zmm27,%zmm8        # 5ca08 <perfetto::base::(anonymous namespace)::kHexmap+0x328>
  17f4ee:	vpaddd -0x122c30(%rip){1to16},%zmm31,%zmm11        # 5c8c8 <perfetto::base::(anonymous namespace)::kHexmap+0x1e8>
  17f4f8:	vpaddd -0x122ace(%rip){1to16},%zmm29,%zmm17        # 5ca34 <perfetto::base::(anonymous namespace)::kHexmap+0x354>
  17f502:	vmovdqa64 %zmm25,0x140(%rsp)
  17f50a:	vmovdqa64 %zmm26,0x100(%rsp)
  17f512:	vmovdqa64 %zmm27,0xc0(%rsp)
  17f51a:	vmovdqa64 %zmm28,0x840(%rsp)
  17f522:	vmovdqa64 %zmm29,0x800(%rsp)
  17f52a:	vmovdqa64 %zmm30,0x7c0(%rsp)
  17f532:	vmovdqa64 %zmm31,0x780(%rsp)
  17f53a:	vmovdqa64 %zmm14,0x740(%rsp)
  17f542:	vprold $0x10,%zmm0,%zmm1
  17f549:	vpaddd -0x122cd7(%rip){1to16},%zmm1,%zmm2        # 5c87c <perfetto::base::(anonymous namespace)::kHexmap+0x19c>
  17f553:	vprold $0x10,%zmm8,%zmm5
  17f55a:	vpaddd -0x122b58(%rip){1to16},%zmm5,%zmm6        # 5ca0c <perfetto::base::(anonymous namespace)::kHexmap+0x32c>
  17f564:	vpaddd %zmm26,%zmm0,%zmm0
  17f56a:	vpxord -0x122b94(%rip){1to16},%zmm2,%zmm3        # 5c9e0 <perfetto::base::(anonymous namespace)::kHexmap+0x300>
  17f574:	vpxord -0x122c1a(%rip){1to16},%zmm6,%zmm7        # 5c964 <perfetto::base::(anonymous namespace)::kHexmap+0x284>
  17f57e:	vprold $0x14,%zmm3,%zmm3
  17f585:	vprold $0x14,%zmm7,%zmm7
  17f58c:	vpaddd %zmm3,%zmm0,%zmm0
  17f592:	vpxord %zmm1,%zmm0,%zmm1
  17f598:	vprold $0x18,%zmm1,%zmm4
  17f59f:	vpaddd %zmm2,%zmm4,%zmm2
  17f5a5:	vpxord %zmm3,%zmm2,%zmm1
  17f5ab:	vpaddd %zmm28,%zmm8,%zmm3
  17f5b1:	vpxord -0x122bcf(%rip){1to16},%zmm11,%zmm8        # 5c9ec <perfetto::base::(anonymous namespace)::kHexmap+0x30c>
  17f5bb:	vpaddd %zmm7,%zmm3,%zmm3
  17f5c1:	vprold $0x19,%zmm1,%zmm1
  17f5c8:	vpxord %zmm5,%zmm3,%zmm5
  17f5ce:	vprold $0x18,%zmm5,%zmm5
  17f5d5:	vpaddd %zmm6,%zmm5,%zmm6
  17f5db:	vpxord %zmm7,%zmm6,%zmm7
  17f5e1:	vprold $0x19,%zmm7,%zmm15
  17f5e8:	vpaddd %zmm14,%zmm11,%zmm7
  17f5ee:	vpxord -0x122d60(%rip){1to16},%zmm17,%zmm11        # 5c898 <perfetto::base::(anonymous namespace)::kHexmap+0x1b8>
  17f5f8:	vprold $0x10,%zmm8,%zmm8
  17f5ff:	vpaddd -0x122c55(%rip){1to16},%zmm8,%zmm9        # 5c9b4 <perfetto::base::(anonymous namespace)::kHexmap+0x2d4>
  17f609:	vpxord -0x122c3f(%rip){1to16},%zmm9,%zmm10        # 5c9d4 <perfetto::base::(anonymous namespace)::kHexmap+0x2f4>
  17f613:	vmovdqa64 %zmm15,0x700(%rsp)
  17f61b:	vprold $0x10,%zmm11,%zmm11
  17f622:	vpaddd -0x122c1c(%rip){1to16},%zmm11,%zmm12        # 5ca10 <perfetto::base::(anonymous namespace)::kHexmap+0x330>
  17f62c:	vpxord -0x122d26(%rip){1to16},%zmm12,%zmm13        # 5c910 <perfetto::base::(anonymous namespace)::kHexmap+0x230>
  17f636:	vprold $0x14,%zmm10,%zmm10
  17f63d:	vpaddd %zmm10,%zmm7,%zmm7
  17f643:	vpxord %zmm8,%zmm7,%zmm8
  17f649:	vprold $0x18,%zmm8,%zmm16
  17f650:	vpaddd %zmm9,%zmm16,%zmm8
  17f656:	vmovdqa64 %zmm16,0x6c0(%rsp)
  17f65e:	vprold $0x14,%zmm13,%zmm13
  17f665:	vpxord %zmm10,%zmm8,%zmm9
  17f66b:	vpaddd %zmm30,%zmm17,%zmm10
  17f671:	vprold $0x19,%zmm9,%zmm9
  17f678:	vpaddd %zmm13,%zmm10,%zmm10
  17f67e:	vpxord %zmm11,%zmm10,%zmm11
  17f684:	vpaddd %zmm10,%zmm9,%zmm10
  17f68a:	vprold $0x18,%zmm11,%zmm11
  17f691:	vpaddd %zmm12,%zmm11,%zmm17
  17f697:	vpxord %zmm13,%zmm17,%zmm12
  17f69d:	vmovdqa64 %zmm17,0x680(%rsp)
  17f6a5:	vprold $0x19,%zmm12,%zmm12
  17f6ac:	vpaddd %zmm3,%zmm12,%zmm3
  17f6b2:	vpxord %zmm4,%zmm3,%zmm4
  17f6b8:	vprold $0x10,%zmm4,%zmm4
  17f6bf:	vpaddd %zmm8,%zmm4,%zmm8
  17f6c5:	vpxord %zmm12,%zmm8,%zmm12
  17f6cb:	vprold $0x14,%zmm12,%zmm12
  17f6d2:	vpaddd %zmm3,%zmm12,%zmm13
  17f6d8:	vpxord %zmm4,%zmm13,%zmm3
  17f6de:	vmovdqa64 %zmm13,0x640(%rsp)
  17f6e6:	vprold $0x18,%zmm3,%zmm3
  17f6ed:	vpaddd %zmm8,%zmm3,%zmm4
  17f6f3:	vmovdqa64 %zmm3,%zmm8
  17f6f9:	vmovdqa64 %zmm8,0x600(%rsp)
  17f701:	vpxord %zmm12,%zmm4,%zmm3
  17f707:	vmovdqa64 %zmm4,%zmm12
  17f70d:	vpxord %zmm5,%zmm10,%zmm4
  17f713:	vmovdqa64 %zmm12,0xb00(%rsp)
  17f71b:	vprold $0x10,%zmm4,%zmm4
  17f722:	vprold $0x19,%zmm3,%zmm18
  17f729:	vpaddd %zmm2,%zmm4,%zmm2
  17f72f:	vmovdqa64 %zmm18,0xac0(%rsp)
  17f737:	vpxord %zmm9,%zmm2,%zmm5
  17f73d:	vprold $0x14,%zmm5,%zmm5
  17f744:	vpaddd %zmm10,%zmm5,%zmm9
  17f74a:	vpxord %zmm4,%zmm9,%zmm3
  17f750:	vmovdqa64 %zmm9,0xa80(%rsp)
  17f758:	vprold $0x18,%zmm3,%zmm10
  17f75f:	vpaddd %zmm2,%zmm10,%zmm19
  17f765:	vmovdqa64 %zmm10,0xa40(%rsp)
  17f76d:	vpxord %zmm5,%zmm19,%zmm2
  17f773:	vpaddd %zmm1,%zmm7,%zmm5
  17f779:	vmovdqa64 %zmm19,0xa00(%rsp)
  17f781:	vpxord %zmm11,%zmm5,%zmm3
  17f787:	vprold $0x19,%zmm2,%zmm20
  17f78e:	vprold $0x10,%zmm3,%zmm3
  17f795:	vmovdqa64 %zmm20,0x9c0(%rsp)
  17f79d:	vpaddd %zmm6,%zmm3,%zmm4
  17f7a3:	vpxord %zmm1,%zmm4,%zmm1
  17f7a9:	vprold $0x14,%zmm1,%zmm1
  17f7b0:	vpaddd %zmm5,%zmm1,%zmm11
  17f7b6:	vpxord %zmm3,%zmm11,%zmm2
  17f7bc:	vpaddd %zmm0,%zmm15,%zmm3
  17f7c2:	vmovdqa64 -0x12a78c(%rip),%zmm0        # 55040 <GCC_except_table0+0xafc>
  17f7cc:	vmovdqa64 %zmm11,0x980(%rsp)
  17f7d4:	vprold $0x18,%zmm2,%zmm21
  17f7db:	vpxor  %xmm2,%xmm2,%xmm2
  17f7df:	vmovdqa64 %zmm3,0x880(%rsp)
  17f7e7:	vpaddd %zmm4,%zmm21,%zmm22
  17f7ed:	vmovdqa64 %zmm21,0x940(%rsp)
  17f7f5:	vpxord %zmm1,%zmm22,%zmm1
  17f7fb:	vmovdqa64 %zmm22,0x900(%rsp)
  17f803:	vprold $0x19,%zmm1,%zmm23
  17f80a:	vpbroadcastd %eax,%zmm1
  17f810:	vmovdqa64 %zmm1,0x1c0(%rsp)
  17f818:	vpaddd %zmm25,%zmm30,%zmm1
  17f81e:	vmovdqa64 %zmm1,0x180(%rsp)
  17f826:	vmovdqa64 %zmm23,0x8c0(%rsp)
  17f82e:	jmp    180375 <bitz::utils::blake3x4::first_pow_nonce+0x1005>
  17f833:	mov    %r12,%r14
  17f836:	xor    %eax,%eax
  17f838:	cmp    $0xffffffffffffffff,%rbx
  17f83c:	jmp    181526 <bitz::utils::blake3x4::first_pow_nonce+0x21b6>
  17f841:	xor    %ecx,%ecx
  17f843:	mov    $0x20,%eax
  17f848:	sub    0x3c(%rsp),%eax
  17f84c:	mov    $0xffffffff,%edx
  17f851:	cmovb  %ecx,%eax
  17f854:	cmp    $0x20,%eax
  17f857:	shlx   %eax,%edx,%eax
  17f85c:	mov    %rbx,%rdx
  17f85f:	bswap  %eax
  17f861:	cmovae %ecx,%eax
  17f864:	sub    0x40(%rsp),%rdx
  17f869:	cmovae %rdx,%rcx
  17f86d:	cmp    $0x10,%rcx
  17f871:	jb     180028 <bitz::utils::blake3x4::first_pow_nonce+0xcb8>
  17f877:	vpbroadcastd %eax,%zmm0
  17f87d:	mov    %r14,%rax
  17f880:	shl    $0x6,%rax
  17f884:	lea    0x8(,%r14,4),%ecx
  17f88c:	mov    %r14,%r15
  17f88f:	mov    %r15d,%edi
  17f892:	and    $0x7,%edi
  17f895:	and    $0x8,%r15d
  17f899:	lea    -0x112ba0(%rip),%r8        # 6cd00 <perfetto::base::(anonymous namespace)::kHexmap+0x10620>
  17f8a0:	lea    0x200(%rsp,%rax,1),%rax
  17f8a8:	vmovdqa64 %zmm0,0x140(%rsp)
  17f8b0:	mov    %rdi,0x100(%rsp)
  17f8b8:	mov    %r15,0xc0(%rsp)
  17f8c0:	vpbroadcastd %ecx,%zmm0
  17f8c6:	vmovdqa64 %zmm0,0x180(%rsp)
  17f8ce:	vpxor  %xmm0,%xmm0,%xmm0
  17f8d2:	mov    %rax,0x1c0(%rsp)
  17f8da:	jmp    17f91f <bitz::utils::blake3x4::first_pow_nonce+0x5af>
  17f8dc:	nopl   0x0(%rax)
  17f8e0:	mov    0x40(%rsp),%rcx
  17f8e5:	mov    0x70(%rsp),%rbx
  17f8ea:	mov    0x100(%rsp),%rdi
  17f8f2:	lea    -0x112bf9(%rip),%r8        # 6cd00 <perfetto::base::(anonymous namespace)::kHexmap+0x10620>
  17f8f9:	vpxor  %xmm0,%xmm0,%xmm0
  17f8fd:	add    $0x10,%rcx
  17f901:	mov    %rbx,%rax
  17f904:	mov    %rcx,0x40(%rsp)
  17f909:	sub    %rcx,%rax
  17f90c:	mov    $0x0,%ecx
  17f911:	cmovb  %rcx,%rax
  17f915:	cmp    $0xf,%rax
  17f919:	jbe    180028 <bitz::utils::blake3x4::first_pow_nonce+0xcb8>
  17f91f:	vmovdqa64 %zmm0,0x5c0(%rsp)
  17f927:	vmovdqa64 %zmm0,0x580(%rsp)
  17f92f:	vmovdqa64 %zmm0,0x540(%rsp)
  17f937:	vmovdqa64 %zmm0,0x500(%rsp)
  17f93f:	vmovdqa64 %zmm0,0x4c0(%rsp)
  17f947:	vmovdqa64 %zmm0,0x480(%rsp)
  17f94f:	vmovdqa64 %zmm0,0x440(%rsp)
  17f957:	vmovdqa64 %zmm0,0x400(%rsp)
  17f95f:	vmovdqa64 %zmm0,0x3c0(%rsp)
  17f967:	vmovdqa64 %zmm0,0x380(%rsp)
  17f96f:	vmovdqa64 %zmm0,0x340(%rsp)
  17f977:	vmovdqa64 %zmm0,0x300(%rsp)
  17f97f:	vmovdqa64 %zmm0,0x2c0(%rsp)
  17f987:	vmovdqa64 %zmm0,0x280(%rsp)
  17f98f:	vmovdqa64 %zmm0,0x240(%rsp)
  17f997:	vmovdqa64 %zmm0,0x200(%rsp)
  17f99f:	test   %r13,%r13
  17f9a2:	je     17fa99 <bitz::utils::blake3x4::first_pow_nonce+0x729>
  17f9a8:	cmp    $0x20,%r13
  17f9ac:	jae    17f9b5 <bitz::utils::blake3x4::first_pow_nonce+0x645>
  17f9ae:	xor    %eax,%eax
  17f9b0:	jmp    17fa60 <bitz::utils::blake3x4::first_pow_nonce+0x6f0>
  17f9b5:	mov    0xc0(%rsp),%rdx
  17f9bd:	lea    0x3c0(%rsp),%rcx
  17f9c5:	xor    %eax,%eax
  17f9c7:	nopw   0x0(%rax,%rax,1)
  17f9d0:	vpbroadcastd 0x80(%rsp,%rax,4),%zmm0
  17f9d8:	vbroadcastss 0x84(%rsp,%rax,4),%zmm1
  17f9e0:	vbroadcastss 0x88(%rsp,%rax,4),%zmm2
  17f9e8:	vmovdqa64 %zmm0,-0x1c0(%rcx)
  17f9ef:	vmovaps %zmm1,-0x180(%rcx)
  17f9f6:	vbroadcastss 0x8c(%rsp,%rax,4),%zmm1
  17f9fe:	vmovaps %zmm2,-0x140(%rcx)
  17fa05:	vbroadcastss 0x90(%rsp,%rax,4),%zmm2
  17fa0d:	vmovaps %zmm1,-0x100(%rcx)
  17fa14:	vbroadcastss 0x94(%rsp,%rax,4),%zmm1
  17fa1c:	vmovaps %zmm2,-0xc0(%rcx)
  17fa23:	vpbroadcastd 0x98(%rsp,%rax,4),%zmm2
  17fa2b:	vmovaps %zmm1,-0x80(%rcx)
  17fa32:	vpbroadcastd 0x9c(%rsp,%rax,4),%zmm1
  17fa3a:	vmovdqa64 %zmm2,-0x40(%rcx)
  17fa41:	add    $0x8,%rax
  17fa45:	vmovdqa64 %zmm1,(%rcx)
  17fa4b:	add    $0x200,%rcx
  17fa52:	cmp    %rax,%rdx
  17fa55:	jne    17f9d0 <bitz::utils::blake3x4::first_pow_nonce+0x660>
  17fa5b:	test   %rdi,%rdi
  17fa5e:	je     17fa99 <bitz::utils::blake3x4::first_pow_nonce+0x729>
  17fa60:	lea    0x80(%rsp,%rax,4),%rcx
  17fa68:	shl    $0x6,%rax
  17fa6c:	xor    %edx,%edx
  17fa6e:	lea    0x200(%rsp,%rax,1),%rax
  17fa76:	cs nopw 0x0(%rax,%rax,1)
  17fa80:	vpbroadcastd (%rcx,%rdx,4),%zmm0
  17fa87:	inc    %rdx
  17fa8a:	vmovdqa64 %zmm0,(%rax)
  17fa90:	add    $0x40,%rax
  17fa94:	cmp    %rdx,%rdi
  17fa97:	jne    17fa80 <bitz::utils::blake3x4::first_pow_nonce+0x710>
  17fa99:	mov    0x40(%rsp),%rax
  17fa9e:	mov    0x1c0(%rsp),%rcx
  17faa6:	vpbroadcastd -0x1230a0(%rip),%zmm13        # 5ca10 <perfetto::base::(anonymous namespace)::kHexmap+0x330>
  17fab0:	vpbroadcastd -0x1230ae(%rip),%zmm14        # 5ca0c <perfetto::base::(anonymous namespace)::kHexmap+0x32c>
  17faba:	vpbroadcastd -0x123248(%rip),%zmm15        # 5c87c <perfetto::base::(anonymous namespace)::kHexmap+0x19c>
  17fac4:	vpbroadcastd -0x12311a(%rip),%zmm11        # 5c9b4 <perfetto::base::(anonymous namespace)::kHexmap+0x2d4>
  17face:	vpbroadcastd -0x1231c8(%rip),%zmm4        # 5c910 <perfetto::base::(anonymous namespace)::kHexmap+0x230>
  17fad8:	vpbroadcastd -0x12317e(%rip),%zmm5        # 5c964 <perfetto::base::(anonymous namespace)::kHexmap+0x284>
  17fae2:	vpbroadcastd -0x12310c(%rip),%zmm12        # 5c9e0 <perfetto::base::(anonymous namespace)::kHexmap+0x300>
  17faec:	vmovdqa64 0x180(%rsp),%zmm6
  17faf4:	mov    $0x78,%edx
  17faf9:	vpxor  %xmm7,%xmm7,%xmm7
  17fafd:	vpxor  %xmm8,%xmm8,%xmm8
  17fb02:	vpbroadcastd %eax,%zmm0
  17fb08:	vpaddd -0x12aad2(%rip),%zmm0,%zmm1        # 55040 <GCC_except_table0+0xafc>
  17fb12:	shr    $0x20,%rax
  17fb16:	vpbroadcastd %eax,%zmm2
  17fb1c:	vmovdqa64 %zmm13,%zmm3
  17fb22:	vmovdqa64 %zmm14,%zmm10
  17fb28:	vmovdqa64 %zmm15,%zmm9
  17fb2e:	vpcmpltud %zmm0,%zmm1,%k0
  17fb35:	vmovdqa64 %zmm1,(%rcx)
  17fb3b:	vpbroadcastd -0x123159(%rip),%zmm1        # 5c9ec <perfetto::base::(anonymous namespace)::kHexmap+0x30c>
  17fb45:	vpmovm2d %k0,%zmm0
  17fb4b:	vpsubd %zmm0,%zmm2,%zmm0
  17fb51:	vpbroadcastd -0x123187(%rip),%zmm2        # 5c9d4 <perfetto::base::(anonymous namespace)::kHexmap+0x2f4>
  17fb5b:	vmovdqa64 %zmm0,0x40(%rcx)
  17fb62:	vmovdqa64 %zmm11,%zmm0
  17fb68:	nopl   0x0(%rax,%rax,1)
  17fb70:	mov    -0x78(%rdx,%r8,1),%rdi
  17fb75:	cmp    $0xf,%rdi
  17fb79:	ja     18160e <bitz::utils::blake3x4::first_pow_nonce+0x229e>
  17fb7f:	mov    -0x70(%rdx,%r8,1),%rcx
  17fb84:	cmp    $0x10,%rcx
  17fb88:	jae    1816c7 <bitz::utils::blake3x4::first_pow_nonce+0x2357>
  17fb8e:	mov    -0x68(%rdx,%r8,1),%rax
  17fb93:	cmp    $0x10,%rax
  17fb97:	jae    1815f7 <bitz::utils::blake3x4::first_pow_nonce+0x2287>
  17fb9d:	mov    -0x60(%rdx,%r8,1),%r14
  17fba2:	cmp    $0x10,%r14
  17fba6:	jae    1816b0 <bitz::utils::blake3x4::first_pow_nonce+0x2340>
  17fbac:	mov    -0x58(%rdx,%r8,1),%rbx
  17fbb1:	cmp    $0x10,%rbx
  17fbb5:	jae    1815e0 <bitz::utils::blake3x4::first_pow_nonce+0x2270>
  17fbbb:	mov    -0x50(%rdx,%r8,1),%r12
  17fbc0:	cmp    $0x10,%r12
  17fbc4:	jae    181699 <bitz::utils::blake3x4::first_pow_nonce+0x2329>
  17fbca:	mov    -0x48(%rdx,%r8,1),%r15
  17fbcf:	cmp    $0x10,%r15
  17fbd3:	jae    1815c9 <bitz::utils::blake3x4::first_pow_nonce+0x2259>
  17fbd9:	mov    -0x40(%rdx,%r8,1),%r13
  17fbde:	cmp    $0x10,%r13
  17fbe2:	jae    181682 <bitz::utils::blake3x4::first_pow_nonce+0x2312>
  17fbe8:	mov    -0x38(%rdx,%r8,1),%r11
  17fbed:	cmp    $0x10,%r11
  17fbf1:	jae    1815b2 <bitz::utils::blake3x4::first_pow_nonce+0x2242>
  17fbf7:	mov    -0x30(%rdx,%r8,1),%r10
  17fbfc:	cmp    $0x10,%r10
  17fc00:	jae    18166b <bitz::utils::blake3x4::first_pow_nonce+0x22fb>
  17fc06:	mov    -0x28(%rdx,%r8,1),%r9
  17fc0b:	cmp    $0x10,%r9
  17fc0f:	jae    18159b <bitz::utils::blake3x4::first_pow_nonce+0x222b>
  17fc15:	mov    -0x20(%rdx,%r8,1),%rsi
  17fc1a:	mov    %rsi,0x68(%rsp)
  17fc1f:	cmp    $0x10,%rsi
  17fc23:	jae    181652 <bitz::utils::blake3x4::first_pow_nonce+0x22e2>
  17fc29:	mov    -0x18(%rdx,%r8,1),%rsi
  17fc2e:	mov    %rsi,0x60(%rsp)
  17fc33:	cmp    $0x10,%rsi
  17fc37:	jae    181582 <bitz::utils::blake3x4::first_pow_nonce+0x2212>
  17fc3d:	mov    -0x10(%rdx,%r8,1),%rsi
  17fc42:	mov    %rsi,0x58(%rsp)
  17fc47:	cmp    $0x10,%rsi
  17fc4b:	jae    181639 <bitz::utils::blake3x4::first_pow_nonce+0x22c9>
  17fc51:	mov    -0x8(%rdx,%r8,1),%rsi
  17fc56:	mov    %rsi,0x50(%rsp)
  17fc5b:	cmp    $0x10,%rsi
  17fc5f:	jae    181569 <bitz::utils::blake3x4::first_pow_nonce+0x21f9>
  17fc65:	mov    (%rdx,%r8,1),%r8
  17fc69:	cmp    $0x10,%r8
  17fc6d:	jae    181622 <bitz::utils::blake3x4::first_pow_nonce+0x22b2>
  17fc73:	shl    $0x6,%rax
  17fc77:	shl    $0x6,%rbx
  17fc7b:	vpaddd %zmm5,%zmm14,%zmm14
  17fc81:	vpaddd %zmm4,%zmm13,%zmm13
  17fc87:	shl    $0x6,%rdi
  17fc8b:	shl    $0x6,%r14
  17fc8f:	shl    $0x6,%r12
  17fc93:	shl    $0x6,%r15
  17fc97:	vpaddd %zmm12,%zmm15,%zmm15
  17fc9d:	vpaddd %zmm2,%zmm11,%zmm11
  17fca3:	shl    $0x6,%r13
  17fca7:	shl    $0x6,%rcx
  17fcab:	shl    $0x6,%r9
  17fcaf:	shl    $0x6,%r11
  17fcb3:	shl    $0x6,%r10
  17fcb7:	shl    $0x6,%r8
  17fcbb:	sub    $0xffffffffffffff80,%rdx
  17fcbf:	vpaddd 0x200(%rsp,%rax,1),%zmm14,%zmm19
  17fcc7:	vpaddd 0x200(%rsp,%rbx,1),%zmm13,%zmm21
  17fccf:	vpaddd 0x200(%rsp,%rdi,1),%zmm15,%zmm16
  17fcd7:	vpaddd 0x200(%rsp,%r15,1),%zmm11,%zmm15
  17fcdf:	mov    0x68(%rsp),%rax
  17fce4:	vpaddd 0x200(%rsp,%r14,1),%zmm19,%zmm20
  17fcec:	vpaddd 0x200(%rsp,%r12,1),%zmm21,%zmm22
  17fcf4:	vpaddd 0x200(%rsp,%r13,1),%zmm15,%zmm17
  17fcfc:	vpaddd 0x200(%rsp,%rcx,1),%zmm16,%zmm18
  17fd04:	mov    0x78(%rsp),%r13
  17fd09:	shl    $0x6,%rax
  17fd0d:	vpxord %zmm7,%zmm19,%zmm7
  17fd13:	vpxord %zmm6,%zmm21,%zmm6
  17fd19:	vpxord %zmm1,%zmm15,%zmm1
  17fd1f:	vpxord %zmm8,%zmm16,%zmm8
  17fd25:	vprold $0x10,%zmm7,%zmm7
  17fd2c:	vprold $0x10,%zmm6,%zmm6
  17fd33:	vprold $0x10,%zmm1,%zmm1
  17fd3a:	vprold $0x10,%zmm8,%zmm11
  17fd41:	vpaddd %zmm10,%zmm7,%zmm10
  17fd47:	vpaddd %zmm3,%zmm6,%zmm3
  17fd4d:	vpaddd %zmm0,%zmm1,%zmm0
  17fd53:	vpaddd %zmm9,%zmm11,%zmm8
  17fd59:	vpxord %zmm5,%zmm10,%zmm5
  17fd5f:	vpxord %zmm4,%zmm3,%zmm4
  17fd65:	vpxord %zmm2,%zmm0,%zmm2
  17fd6b:	vpxord %zmm12,%zmm8,%zmm9
  17fd71:	vprold $0x14,%zmm5,%zmm5
  17fd78:	vprold $0x14,%zmm4,%zmm4
  17fd7f:	vprold $0x14,%zmm2,%zmm2
  17fd86:	vprold $0x14,%zmm9,%zmm9
  17fd8d:	vpaddd %zmm5,%zmm20,%zmm16
  17fd93:	vpaddd %zmm4,%zmm22,%zmm20
  17fd99:	vpaddd %zmm2,%zmm17,%zmm15
  17fd9f:	vpaddd %zmm9,%zmm18,%zmm14
  17fda5:	vpxord %zmm6,%zmm20,%zmm6
  17fdab:	vpxord %zmm7,%zmm16,%zmm7
  17fdb1:	vpxord %zmm1,%zmm15,%zmm1
  17fdb7:	vpxord %zmm11,%zmm14,%zmm11
  17fdbd:	vprold $0x18,%zmm6,%zmm6
  17fdc4:	vprold $0x18,%zmm1,%zmm1
  17fdcb:	vprold $0x18,%zmm7,%zmm7
  17fdd2:	vprold $0x18,%zmm11,%zmm11
  17fdd9:	vpaddd %zmm3,%zmm6,%zmm3
  17fddf:	vpaddd %zmm0,%zmm1,%zmm0
  17fde5:	vpaddd %zmm10,%zmm7,%zmm10
  17fdeb:	vpxord %zmm4,%zmm3,%zmm4
  17fdf1:	vpxord %zmm2,%zmm0,%zmm2
  17fdf7:	vpxord %zmm5,%zmm10,%zmm5
  17fdfd:	vprold $0x19,%zmm4,%zmm4
  17fe04:	vprold $0x19,%zmm2,%zmm2
  17fe0b:	vprold $0x19,%zmm5,%zmm5
  17fe12:	vpaddd %zmm16,%zmm4,%zmm16
  17fe18:	vpaddd 0x200(%rsp,%r9,1),%zmm16,%zmm19
  17fe20:	vpaddd %zmm20,%zmm2,%zmm17
  17fe26:	vpaddd %zmm8,%zmm11,%zmm20
  17fe2c:	vpaddd %zmm14,%zmm5,%zmm12
  17fe32:	vpaddd 0x200(%rsp,%r11,1),%zmm12,%zmm13
  17fe3a:	vpaddd 0x200(%rsp,%rax,1),%zmm19,%zmm16
  17fe42:	mov    0x60(%rsp),%rax
  17fe47:	vpxord %zmm9,%zmm20,%zmm8
  17fe4d:	vpaddd 0x200(%rsp,%r10,1),%zmm13,%zmm12
  17fe55:	vprold $0x19,%zmm8,%zmm21
  17fe5c:	vpaddd %zmm21,%zmm15,%zmm8
  17fe62:	shl    $0x6,%rax
  17fe66:	vpaddd 0x200(%rsp,%rax,1),%zmm17,%zmm17
  17fe6e:	mov    0x58(%rsp),%rax
  17fe73:	vpxord %zmm1,%zmm13,%zmm1
  17fe79:	vpxord %zmm11,%zmm19,%zmm9
  17fe7f:	vprold $0x10,%zmm1,%zmm1
  17fe86:	vpaddd %zmm3,%zmm1,%zmm3
  17fe8c:	shl    $0x6,%rax
  17fe90:	vpxord %zmm5,%zmm3,%zmm5
  17fe96:	vpaddd 0x200(%rsp,%rax,1),%zmm17,%zmm18
  17fe9e:	mov    0x50(%rsp),%rax
  17fea3:	vprold $0x14,%zmm5,%zmm5
  17feaa:	vpxord %zmm7,%zmm17,%zmm7
  17feb0:	vpaddd %zmm5,%zmm12,%zmm15
  17feb6:	vprold $0x10,%zmm7,%zmm7
  17febd:	vpxord %zmm1,%zmm15,%zmm1
  17fec3:	vprold $0x18,%zmm1,%zmm1
  17feca:	shl    $0x6,%rax
  17fece:	vpaddd 0x200(%rsp,%rax,1),%zmm8,%zmm8
  17fed6:	vpaddd %zmm3,%zmm1,%zmm3
  17fedc:	vpaddd 0x200(%rsp,%r8,1),%zmm8,%zmm22
  17fee4:	vpxord %zmm5,%zmm3,%zmm5
  17feea:	lea    -0x1131f1(%rip),%r8        # 6cd00 <perfetto::base::(anonymous namespace)::kHexmap+0x10620>
  17fef1:	vprold $0x19,%zmm5,%zmm5
  17fef8:	vpxord %zmm6,%zmm8,%zmm6
  17fefe:	vprold $0x10,%zmm9,%zmm8
  17ff05:	vpaddd %zmm20,%zmm7,%zmm9
  17ff0b:	vprold $0x10,%zmm6,%zmm6
  17ff12:	vpxord %zmm2,%zmm9,%zmm2
  17ff18:	vpaddd %zmm0,%zmm8,%zmm0
  17ff1e:	vprold $0x14,%zmm2,%zmm2
  17ff25:	vpaddd %zmm10,%zmm6,%zmm10
  17ff2b:	vpxord %zmm4,%zmm0,%zmm4
  17ff31:	vpxord %zmm21,%zmm10,%zmm11
  17ff37:	vprold $0x14,%zmm4,%zmm4
  17ff3e:	vpaddd %zmm2,%zmm18,%zmm13
  17ff44:	vprold $0x14,%zmm11,%zmm12
  17ff4b:	vpxord %zmm7,%zmm13,%zmm7
  17ff51:	vpaddd %zmm4,%zmm16,%zmm14
  17ff57:	vprold $0x18,%zmm7,%zmm7
  17ff5e:	vpaddd %zmm12,%zmm22,%zmm11
  17ff64:	vpxord %zmm8,%zmm14,%zmm8
  17ff6a:	vpxord %zmm6,%zmm11,%zmm6
  17ff70:	vprold $0x18,%zmm8,%zmm8
  17ff77:	vpaddd %zmm9,%zmm7,%zmm9
  17ff7d:	vprold $0x18,%zmm6,%zmm6
  17ff84:	vpxord %zmm2,%zmm9,%zmm2
  17ff8a:	vpaddd %zmm0,%zmm8,%zmm0
  17ff90:	vprold $0x19,%zmm2,%zmm2
  17ff97:	vpaddd %zmm10,%zmm6,%zmm10
  17ff9d:	vpxord %zmm4,%zmm0,%zmm4
  17ffa3:	vpxord %zmm12,%zmm10,%zmm12
  17ffa9:	vprold $0x19,%zmm4,%zmm4
  17ffb0:	vprold $0x19,%zmm12,%zmm12
  17ffb7:	cmp    $0x3f8,%rdx
  17ffbe:	jne    17fb70 <bitz::utils::blake3x4::first_pow_nonce+0x800>
  17ffc4:	vpxord %zmm15,%zmm9,%zmm0
  17ffca:	vptestnmd 0x140(%rsp),%zmm0,%k0
  17ffd2:	kortestw %k0,%k0
  17ffd6:	je     17f8e0 <bitz::utils::blake3x4::first_pow_nonce+0x570>
  17ffdc:	cmpl   $0x21,0x3c(%rsp)
  17ffe1:	kmovd  %k0,%ebx
  17ffe5:	jb     18153e <bitz::utils::blake3x4::first_pow_nonce+0x21ce>
  17ffeb:	nopl   0x0(%rax,%rax,1)
  17fff0:	tzcnt  %ebx,%r14d
  17fff5:	add    0x40(%rsp),%r14
  17fffa:	mov    0x48(%rsp),%rdi
  17ffff:	mov    0x3c(%rsp),%ecx
  180003:	mov    %r13,%rsi
  180006:	mov    %r14,%rdx
  180009:	vzeroupper
  18000c:	call   17eca0 <bitz::utils::blake3x4::pow_ok>
  180011:	test   %al,%al
  180013:	jne    181474 <bitz::utils::blake3x4::first_pow_nonce+0x2104>
  180019:	lea    -0x1(%rbx),%eax
  18001c:	and    %bx,%ax
  18001f:	mov    %eax,%ebx
  180021:	jne    17fff0 <bitz::utils::blake3x4::first_pow_nonce+0xc80>
  180023:	jmp    17f8e0 <bitz::utils::blake3x4::first_pow_nonce+0x570>
  180028:	mov    0x40(%rsp),%r15
  18002d:	cmp    %r15,%rbx
  180030:	cmovbe %r15,%rbx
  180034:	dec    %r15
  180037:	mov    %rbx,%r14
  18003a:	neg    %r14
  18003d:	mov    %rbx,%r12
  180040:	lea    (%r14,%r15,1),%rbx
  180044:	cmp    $0xffffffffffffffff,%rbx
  180048:	je     18151a <bitz::utils::blake3x4::first_pow_nonce+0x21aa>
  18004e:	mov    0x48(%rsp),%rdi
  180053:	mov    0x3c(%rsp),%ecx
  180057:	inc    %r15
  18005a:	mov    %r13,%rsi
  18005d:	mov    %r15,%rdx
  180060:	vzeroupper
  180063:	call   17eca0 <bitz::utils::blake3x4::pow_ok>
  180068:	test   %al,%al
  18006a:	je     180040 <bitz::utils::blake3x4::first_pow_nonce+0xcd0>
  18006c:	jmp    18151d <bitz::utils::blake3x4::first_pow_nonce+0x21ad>
  180071:	xor    %ecx,%ecx
  180073:	mov    $0x20,%eax
  180078:	sub    0x3c(%rsp),%eax
  18007c:	mov    $0xffffffff,%edx
  180081:	cmovb  %ecx,%eax
  180084:	cmp    $0x20,%eax
  180087:	shlx   %eax,%edx,%eax
  18008c:	mov    %rbx,%rdx
  18008f:	bswap  %eax
  180091:	cmovae %ecx,%eax
  180094:	sub    0x40(%rsp),%rdx
  180099:	cmovae %rdx,%rcx
  18009d:	cmp    $0x10,%rcx
  1800a1:	jb     1814ca <bitz::utils::blake3x4::first_pow_nonce+0x215a>
  1800a7:	vpbroadcastd 0x80(%rsp),%zmm25
  1800af:	vpbroadcastd 0x84(%rsp),%zmm26
  1800b7:	vpbroadcastd 0x88(%rsp),%zmm27
  1800bf:	vpbroadcastd 0x8c(%rsp),%zmm28
  1800c7:	vpbroadcastd -0x1236c1(%rip),%zmm8        # 5ca10 <perfetto::base::(anonymous namespace)::kHexmap+0x330>
  1800d1:	vpbroadcastd -0x1237cb(%rip),%zmm11        # 5c910 <perfetto::base::(anonymous namespace)::kHexmap+0x230>
  1800db:	vpbroadcastd -0x1236ad(%rip),%zmm12        # 5ca38 <perfetto::base::(anonymous namespace)::kHexmap+0x358>
  1800e5:	vpbroadcastd -0x123733(%rip),%zmm14        # 5c9bc <perfetto::base::(anonymous namespace)::kHexmap+0x2dc>
  1800ef:	mov    0x40(%rsp),%r15
  1800f4:	lea    0x380(%rsp),%r12
  1800fc:	lea    -0x113383(%rip),%rdx        # 6cd80 <perfetto::base::(anonymous namespace)::kHexmap+0x106a0>
  180103:	vpaddd -0x123791(%rip){1to16},%zmm25,%zmm0        # 5c97c <perfetto::base::(anonymous namespace)::kHexmap+0x29c>
  18010d:	vpaddd -0x12370f(%rip){1to16},%zmm27,%zmm5        # 5ca08 <perfetto::base::(anonymous namespace)::kHexmap+0x328>
  180117:	vmovdqa64 %zmm25,0x180(%rsp)
  18011f:	vmovdqa64 %zmm26,0x140(%rsp)
  180127:	vmovdqa64 %zmm27,0x100(%rsp)
  18012f:	vmovdqa64 %zmm28,0xc0(%rsp)
  180137:	vprold $0x10,%zmm0,%zmm1
  18013e:	vpaddd -0x1238cc(%rip){1to16},%zmm1,%zmm2        # 5c87c <perfetto::base::(anonymous namespace)::kHexmap+0x19c>
  180148:	vpaddd %zmm26,%zmm0,%zmm0
  18014e:	vpxord -0x123778(%rip){1to16},%zmm2,%zmm3        # 5c9e0 <perfetto::base::(anonymous namespace)::kHexmap+0x300>
  180158:	vprold $0x14,%zmm3,%zmm3
  18015f:	vpaddd %zmm3,%zmm0,%zmm0
  180165:	vpxord %zmm1,%zmm0,%zmm1
  18016b:	vprold $0x18,%zmm1,%zmm29
  180172:	vpaddd %zmm2,%zmm29,%zmm30
  180178:	vprold $0x10,%zmm5,%zmm2
  18017f:	vmovdqa64 %zmm29,0x840(%rsp)
  180187:	vpxord %zmm3,%zmm30,%zmm1
  18018d:	vpaddd -0x12378b(%rip){1to16},%zmm2,%zmm3        # 5ca0c <perfetto::base::(anonymous namespace)::kHexmap+0x32c>
  180197:	vmovdqa64 %zmm30,0x800(%rsp)
  18019f:	vpxord -0x123845(%rip){1to16},%zmm3,%zmm4        # 5c964 <perfetto::base::(anonymous namespace)::kHexmap+0x284>
  1801a9:	vprold $0x19,%zmm1,%zmm31
  1801b0:	vpaddd %zmm28,%zmm5,%zmm1
  1801b6:	vpaddd -0x123808(%rip){1to16},%zmm31,%zmm18        # 5c9b8 <perfetto::base::(anonymous namespace)::kHexmap+0x2d8>
  1801c0:	vpbroadcastd -0x1237ee(%rip),%zmm5        # 5c9dc <perfetto::base::(anonymous namespace)::kHexmap+0x2fc>
  1801ca:	vmovdqa64 %zmm31,0x7c0(%rsp)
  1801d2:	vprold $0x14,%zmm4,%zmm4
  1801d9:	vmovdqa64 %zmm18,0x640(%rsp)
  1801e1:	vpaddd %zmm4,%zmm1,%zmm6
  1801e7:	vpxord %zmm2,%zmm6,%zmm1
  1801ed:	vmovdqa64 %zmm6,0x780(%rsp)
  1801f5:	vprold $0x18,%zmm1,%zmm13
  1801fc:	vpaddd %zmm3,%zmm13,%zmm17
  180202:	vpxor  %xmm3,%xmm3,%xmm3
  180206:	vmovdqa64 %zmm13,0x740(%rsp)
  18020e:	vpxord %zmm4,%zmm17,%zmm1
  180214:	vmovdqa64 -0x12b19e(%rip),%zmm4        # 55080 <GCC_except_table0+0xb3c>
  18021e:	vmovdqa64 %zmm17,0x700(%rsp)
  180226:	vprold $0x19,%zmm1,%zmm7
  18022d:	vpbroadcastd %eax,%zmm1
  180233:	vmovdqa64 %zmm1,0x1c0(%rsp)
  18023b:	vpaddd %zmm0,%zmm7,%zmm9
  180241:	vpxord -0x123993(%rip){1to16},%zmm9,%zmm0        # 5c8b8 <perfetto::base::(anonymous namespace)::kHexmap+0x1d8>
  18024b:	vmovdqa64 %zmm7,0x6c0(%rsp)
  180253:	vmovdqa64 %zmm9,0x680(%rsp)
  18025b:	vprold $0x10,%zmm0,%zmm10
  180262:	vmovdqa64 -0x12b22c(%rip),%zmm0        # 55040 <GCC_except_table0+0xafc>
  18026c:	vmovdqa64 %zmm10,0x600(%rsp)
  180274:	jmp    180c10 <bitz::utils::blake3x4::first_pow_nonce+0x18a0>
  180279:	mov    0x40(%rsp),%r15
  18027e:	vmovdqa64 0x140(%rsp),%zmm25
  180286:	vmovdqa64 0x100(%rsp),%zmm26
  18028e:	vmovdqa64 0xc0(%rsp),%zmm27
  180296:	vmovdqa64 0x840(%rsp),%zmm28
  18029e:	vmovdqa64 0x800(%rsp),%zmm29
  1802a6:	vmovdqa64 0x7c0(%rsp),%zmm30
  1802ae:	vmovdqa64 0x780(%rsp),%zmm31
  1802b6:	vmovdqa64 0x740(%rsp),%zmm14
  1802be:	vmovdqa64 0x700(%rsp),%zmm15
  1802c6:	vmovdqa64 0x6c0(%rsp),%zmm16
  1802ce:	vmovdqa64 0x680(%rsp),%zmm17
  1802d6:	vmovdqa64 0x640(%rsp),%zmm13
  1802de:	vmovdqa64 0x600(%rsp),%zmm8
  1802e6:	vmovdqa64 0xb00(%rsp),%zmm12
  1802ee:	vmovdqa64 0xac0(%rsp),%zmm18
  1802f6:	vmovdqa64 0xa80(%rsp),%zmm9
  1802fe:	vmovdqa64 0xa40(%rsp),%zmm10
  180306:	vmovdqa64 0xa00(%rsp),%zmm19
  18030e:	vmovdqa64 0x9c0(%rsp),%zmm20
  180316:	vmovdqa64 0x980(%rsp),%zmm11
  18031e:	vmovdqa64 0x940(%rsp),%zmm21
  180326:	vmovdqa64 0x900(%rsp),%zmm22
  18032e:	vmovdqa64 0x8c0(%rsp),%zmm23
  180336:	vmovdqa64 0x880(%rsp),%zmm3
  18033e:	vmovdqa64 -0x12b308(%rip),%zmm0        # 55040 <GCC_except_table0+0xafc>
  180348:	mov    0x70(%rsp),%rbx
  18034d:	mov    $0x0,%ecx
  180352:	lea    -0x1135d9(%rip),%rdx        # 6cd80 <perfetto::base::(anonymous namespace)::kHexmap+0x106a0>
  180359:	vpxor  %xmm2,%xmm2,%xmm2
  18035d:	add    $0x10,%r15
  180361:	mov    %rbx,%rax
  180364:	sub    %r15,%rax
  180367:	cmovb  %rcx,%rax
  18036b:	cmp    $0xf,%rax
  18036f:	jbe    181483 <bitz::utils::blake3x4::first_pow_nonce+0x2113>
  180375:	vpbroadcastd %r15d,%zmm1
  18037b:	mov    %r15,0x40(%rsp)
  180380:	shr    $0x20,%r15
  180384:	vmovdqa64 %zmm25,0x200(%rsp)
  18038c:	vmovdqa64 %zmm26,0x240(%rsp)
  180394:	vmovdqa64 %zmm27,0x280(%rsp)
  18039c:	vmovdqa64 %zmm28,0x2c0(%rsp)
  1803a4:	vmovdqa64 %zmm29,0x300(%rsp)
  1803ac:	vmovdqa64 %zmm30,0x340(%rsp)
  1803b4:	vmovdqa64 %zmm31,0x380(%rsp)
  1803bc:	vmovdqa64 %zmm14,0x3c0(%rsp)
  1803c4:	vmovdqa64 %zmm2,0x140(%r12)
  1803cc:	vmovdqa64 %zmm2,0x100(%r12)
  1803d4:	vmovdqa64 %zmm2,0xc0(%r12)
  1803dc:	vmovdqa64 %zmm2,0x80(%r12)
  1803e4:	vmovdqa64 %zmm2,0x40(%r12)
  1803ec:	vmovdqa64 %zmm2,(%r12)
  1803f3:	vmovdqa64 %zmm19,%zmm14
  1803f9:	vpaddd %zmm0,%zmm1,%zmm0
  1803ff:	vpaddd %zmm0,%zmm3,%zmm7
  180405:	vpcmpltud %zmm1,%zmm0,%k0
  18040c:	vpbroadcastd %r15d,%zmm1
  180412:	vmovdqa64 %zmm0,0x400(%rsp)
  18041a:	vpxord %zmm16,%zmm7,%zmm3
  180420:	vpmovm2d %k0,%zmm2
  180426:	vpsubd %zmm2,%zmm1,%zmm1
  18042c:	vmovdqa64 %zmm13,%zmm16
  180432:	vmovdqa64 %zmm9,%zmm13
  180438:	vmovdqa64 %zmm11,%zmm9
  18043e:	vmovdqa64 %zmm22,%zmm11
  180444:	vprold $0x10,%zmm3,%zmm3
  18044b:	vpaddd %zmm7,%zmm1,%zmm2
  180451:	vmovdqa64 %zmm1,0x440(%rsp)
  180459:	vpaddd %zmm17,%zmm3,%zmm4
  18045f:	vpxord %zmm15,%zmm4,%zmm5
  180465:	vmovdqa64 %zmm23,%zmm15
  18046b:	vprold $0x14,%zmm5,%zmm6
  180472:	vpaddd %zmm6,%zmm2,%zmm17
  180478:	vpxord %zmm3,%zmm17,%zmm2
  18047e:	vprold $0x18,%zmm2,%zmm2
  180485:	vpaddd %zmm4,%zmm2,%zmm5
  18048b:	vmovdqa64 %zmm20,%zmm4
  180491:	vpxord %zmm6,%zmm5,%zmm3
  180497:	vmovdqa64 %zmm18,%zmm6
  18049d:	vprold $0x19,%zmm3,%zmm7
  1804a4:	vmovdqa64 %zmm12,%zmm3
  1804aa:	vmovdqa64 %zmm8,%zmm12
  1804b0:	vmovdqa64 %zmm21,%zmm8
  1804b6:	cs nopw 0x0(%rax,%rax,1)
  1804c0:	mov    (%rdx),%rdi
  1804c3:	cmp    $0xf,%rdi
  1804c7:	ja     181783 <bitz::utils::blake3x4::first_pow_nonce+0x2413>
  1804cd:	mov    0x8(%rdx),%rcx
  1804d1:	cmp    $0x10,%rcx
  1804d5:	jae    18183c <bitz::utils::blake3x4::first_pow_nonce+0x24cc>
  1804db:	mov    0x10(%rdx),%rax
  1804df:	cmp    $0x10,%rax
  1804e3:	jae    18176c <bitz::utils::blake3x4::first_pow_nonce+0x23fc>
  1804e9:	mov    0x18(%rdx),%rbx
  1804ed:	cmp    $0x10,%rbx
  1804f1:	jae    181825 <bitz::utils::blake3x4::first_pow_nonce+0x24b5>
  1804f7:	mov    0x20(%rdx),%r12
  1804fb:	cmp    $0x10,%r12
  1804ff:	jae    181755 <bitz::utils::blake3x4::first_pow_nonce+0x23e5>
  180505:	mov    0x28(%rdx),%r15
  180509:	cmp    $0x10,%r15
  18050d:	jae    18180e <bitz::utils::blake3x4::first_pow_nonce+0x249e>
  180513:	mov    0x30(%rdx),%r13
  180517:	cmp    $0x10,%r13
  18051b:	jae    18173e <bitz::utils::blake3x4::first_pow_nonce+0x23ce>
  180521:	mov    0x38(%rdx),%r14
  180525:	cmp    $0x10,%r14
  180529:	jae    1817e0 <bitz::utils::blake3x4::first_pow_nonce+0x2470>
  18052f:	mov    0x40(%rdx),%r11
  180533:	cmp    $0x10,%r11
  180537:	jae    181727 <bitz::utils::blake3x4::first_pow_nonce+0x23b7>
  18053d:	mov    0x48(%rdx),%r10
  180541:	cmp    $0x10,%r10
  180545:	jae    1817f7 <bitz::utils::blake3x4::first_pow_nonce+0x2487>
  18054b:	mov    0x50(%rdx),%r9
  18054f:	cmp    $0x10,%r9
  180553:	jae    1816f7 <bitz::utils::blake3x4::first_pow_nonce+0x2387>
  180559:	mov    0x58(%rdx),%r8
  18055d:	cmp    $0x10,%r8
  180561:	jae    1817b0 <bitz::utils::blake3x4::first_pow_nonce+0x2440>
  180567:	mov    0x60(%rdx),%rsi
  18056b:	mov    %rsi,0x68(%rsp)
  180570:	cmp    $0x10,%rsi
  180574:	jae    1816de <bitz::utils::blake3x4::first_pow_nonce+0x236e>
  18057a:	mov    0x68(%rdx),%rsi
  18057e:	mov    %rsi,0x60(%rsp)
  180583:	cmp    $0x10,%rsi
  180587:	jae    1817c7 <bitz::utils::blake3x4::first_pow_nonce+0x2457>
  18058d:	mov    0x70(%rdx),%rsi
  180591:	mov    %rsi,0x58(%rsp)
  180596:	cmp    $0x10,%rsi
  18059a:	jae    18170e <bitz::utils::blake3x4::first_pow_nonce+0x239e>
  1805a0:	mov    0x78(%rdx),%rsi
  1805a4:	mov    %rsi,0x50(%rsp)
  1805a9:	cmp    $0x10,%rsi
  1805ad:	jae    181797 <bitz::utils::blake3x4::first_pow_nonce+0x2427>
  1805b3:	shl    $0x6,%rdi
  1805b7:	shl    $0x6,%r13
  1805bb:	vpaddd %zmm17,%zmm15,%zmm17
  1805c1:	vpaddd %zmm9,%zmm4,%zmm9
  1805c7:	shl    $0x6,%r12
  1805cb:	shl    $0x6,%r14
  1805cf:	vpaddd %zmm13,%zmm6,%zmm13
  1805d5:	shl    $0x6,%rcx
  1805d9:	shl    $0x6,%r15
  1805dd:	shl    $0x6,%rax
  1805e1:	vpaddd %zmm16,%zmm7,%zmm16
  1805e7:	shl    $0x6,%rbx
  1805eb:	shl    $0x6,%r9
  1805ef:	shl    $0x6,%r11
  1805f3:	shl    $0x6,%r8
  1805f7:	shl    $0x6,%r10
  1805fb:	sub    $0xffffffffffffff80,%rdx
  1805ff:	vpaddd 0x200(%rsp,%rdi,1),%zmm17,%zmm18
  180607:	vpaddd 0x200(%rsp,%r13,1),%zmm9,%zmm17
  18060f:	vpaddd 0x200(%rsp,%r12,1),%zmm13,%zmm23
  180617:	vpaddd 0x200(%rsp,%rax,1),%zmm16,%zmm21
  18061f:	mov    0x68(%rsp),%rax
  180624:	mov    0x78(%rsp),%r13
  180629:	vpaddd 0x200(%rsp,%r14,1),%zmm17,%zmm19
  180631:	vpaddd 0x200(%rsp,%rcx,1),%zmm18,%zmm20
  180639:	vpaddd 0x200(%rsp,%r15,1),%zmm23,%zmm24
  180641:	vpaddd 0x200(%rsp,%rbx,1),%zmm21,%zmm22
  180649:	shl    $0x6,%rax
  18064d:	vpxord %zmm2,%zmm17,%zmm2
  180653:	vpxord %zmm8,%zmm23,%zmm8
  180659:	vpxord %zmm12,%zmm18,%zmm9
  18065f:	vpxord %zmm10,%zmm21,%zmm10
  180665:	vprold $0x10,%zmm2,%zmm2
  18066c:	vprold $0x10,%zmm9,%zmm13
  180673:	vprold $0x10,%zmm8,%zmm8
  18067a:	vpaddd %zmm3,%zmm2,%zmm3
  180680:	vpaddd %zmm14,%zmm13,%zmm12
  180686:	vpaddd %zmm5,%zmm8,%zmm5
  18068c:	vpxord %zmm4,%zmm3,%zmm4
  180692:	vpxord %zmm6,%zmm5,%zmm6
  180698:	vpxord %zmm15,%zmm12,%zmm9
  18069e:	vprold $0x14,%zmm4,%zmm4
  1806a5:	vprold $0x14,%zmm9,%zmm14
  1806ac:	vprold $0x14,%zmm6,%zmm6
  1806b3:	vprold $0x10,%zmm10,%zmm9
  1806ba:	vpaddd %zmm4,%zmm19,%zmm17
  1806c0:	vpaddd %zmm14,%zmm20,%zmm16
  1806c6:	vpaddd %zmm6,%zmm24,%zmm20
  1806cc:	vpaddd %zmm11,%zmm9,%zmm11
  1806d2:	vpxord %zmm2,%zmm17,%zmm2
  1806d8:	vpxord %zmm7,%zmm11,%zmm7
  1806de:	vpxord %zmm8,%zmm20,%zmm8
  1806e4:	vpxord %zmm13,%zmm16,%zmm13
  1806ea:	vprold $0x18,%zmm2,%zmm2
  1806f1:	vprold $0x14,%zmm7,%zmm7
  1806f8:	vprold $0x18,%zmm8,%zmm8
  1806ff:	vprold $0x18,%zmm13,%zmm13
  180706:	vpaddd %zmm3,%zmm2,%zmm3
  18070c:	vpaddd %zmm7,%zmm22,%zmm18
  180712:	vpaddd %zmm5,%zmm8,%zmm5
  180718:	vpaddd %zmm12,%zmm13,%zmm22
  18071e:	vpxord %zmm4,%zmm3,%zmm4
  180724:	vpxord %zmm6,%zmm5,%zmm6
  18072a:	vpxord %zmm9,%zmm18,%zmm9
  180730:	vpxord %zmm14,%zmm22,%zmm12
  180736:	vprold $0x19,%zmm4,%zmm4
  18073d:	vprold $0x18,%zmm9,%zmm10
  180744:	vprold $0x19,%zmm6,%zmm6
  18074b:	vprold $0x19,%zmm12,%zmm23
  180752:	vpaddd %zmm20,%zmm4,%zmm19
  180758:	vpaddd 0x200(%rsp,%rax,1),%zmm19,%zmm19
  180760:	mov    0x60(%rsp),%rax
  180765:	vpaddd %zmm11,%zmm10,%zmm9
  18076b:	vpaddd %zmm18,%zmm6,%zmm18
  180771:	vpaddd 0x200(%rsp,%r9,1),%zmm18,%zmm21
  180779:	vpaddd %zmm23,%zmm17,%zmm12
  18077f:	vpxord %zmm7,%zmm9,%zmm7
  180785:	vpaddd 0x200(%rsp,%r8,1),%zmm21,%zmm18
  18078d:	vprold $0x19,%zmm7,%zmm7
  180794:	vpaddd %zmm16,%zmm7,%zmm11
  18079a:	vpaddd 0x200(%rsp,%r11,1),%zmm11,%zmm15
  1807a2:	shl    $0x6,%rax
  1807a6:	vpaddd 0x200(%rsp,%r10,1),%zmm15,%zmm11
  1807ae:	vpaddd 0x200(%rsp,%rax,1),%zmm19,%zmm20
  1807b6:	mov    0x58(%rsp),%rax
  1807bb:	vpxord %zmm13,%zmm21,%zmm13
  1807c1:	vpxord %zmm10,%zmm19,%zmm10
  1807c7:	vprold $0x10,%zmm10,%zmm10
  1807ce:	shl    $0x6,%rax
  1807d2:	vpaddd %zmm22,%zmm10,%zmm14
  1807d8:	vpaddd 0x200(%rsp,%rax,1),%zmm12,%zmm12
  1807e0:	mov    0x50(%rsp),%rax
  1807e5:	vpxord %zmm2,%zmm15,%zmm2
  1807eb:	vpxord %zmm4,%zmm14,%zmm4
  1807f1:	vprold $0x10,%zmm2,%zmm2
  1807f8:	vprold $0x14,%zmm4,%zmm4
  1807ff:	vpaddd %zmm5,%zmm2,%zmm5
  180805:	vpxord %zmm7,%zmm5,%zmm7
  18080b:	shl    $0x6,%rax
  18080f:	vprold $0x14,%zmm7,%zmm7
  180816:	vpaddd 0x200(%rsp,%rax,1),%zmm12,%zmm24
  18081e:	lea    -0x113825(%rip),%rax        # 6d000 <perfetto::base::(anonymous namespace)::kHexmap+0x10920>
  180825:	vpaddd %zmm7,%zmm11,%zmm17
  18082b:	vpxord %zmm8,%zmm12,%zmm8
  180831:	vprold $0x10,%zmm13,%zmm12
  180838:	vpaddd %zmm4,%zmm20,%zmm13
  18083e:	vpxord %zmm2,%zmm17,%zmm2
  180844:	vprold $0x10,%zmm8,%zmm8
  18084b:	vpxord %zmm10,%zmm13,%zmm10
  180851:	vprold $0x18,%zmm2,%zmm2
  180858:	vpaddd %zmm3,%zmm12,%zmm3
  18085e:	vprold $0x18,%zmm10,%zmm10
  180865:	vpxord %zmm6,%zmm3,%zmm6
  18086b:	vpaddd %zmm5,%zmm2,%zmm5
  180871:	vprold $0x14,%zmm6,%zmm6
  180878:	vpaddd %zmm14,%zmm10,%zmm14
  18087e:	vpxord %zmm7,%zmm5,%zmm7
  180884:	vpxord %zmm4,%zmm14,%zmm4
  18088a:	vprold $0x19,%zmm7,%zmm7
  180891:	vpaddd %zmm6,%zmm18,%zmm16
  180897:	vpaddd %zmm9,%zmm8,%zmm18
  18089d:	vprold $0x19,%zmm4,%zmm4
  1808a4:	vpxord %zmm23,%zmm18,%zmm9
  1808aa:	vpxord %zmm12,%zmm16,%zmm11
  1808b0:	vprold $0x14,%zmm9,%zmm15
  1808b7:	vprold $0x18,%zmm11,%zmm12
  1808be:	vpaddd %zmm15,%zmm24,%zmm9
  1808c4:	vpaddd %zmm3,%zmm12,%zmm3
  1808ca:	vpxord %zmm8,%zmm9,%zmm8
  1808d0:	vpxord %zmm6,%zmm3,%zmm6
  1808d6:	vprold $0x18,%zmm8,%zmm8
  1808dd:	vprold $0x19,%zmm6,%zmm6
  1808e4:	vpaddd %zmm18,%zmm8,%zmm11
  1808ea:	vpxord %zmm15,%zmm11,%zmm15
  1808f0:	vprold $0x19,%zmm15,%zmm15
  1808f7:	cmp    %rax,%rdx
  1808fa:	jne    1804c0 <bitz::utils::blake3x4::first_pow_nonce+0x1150>
  180900:	vpaddd %zmm17,%zmm15,%zmm17
  180906:	vpaddd %zmm0,%zmm4,%zmm0
  18090c:	lea    0x480(%rsp),%r12
  180914:	vpxord %zmm12,%zmm17,%zmm12
  18091a:	vpaddd %zmm9,%zmm0,%zmm0
  180920:	vprold $0x10,%zmm12,%zmm12
  180927:	vpxord %zmm2,%zmm0,%zmm2
  18092d:	vpaddd %zmm31,%zmm0,%zmm0
  180933:	vprold $0x10,%zmm2,%zmm2
  18093a:	vpaddd %zmm14,%zmm12,%zmm14
  180940:	vpxord %zmm15,%zmm14,%zmm15
  180946:	vpaddd %zmm3,%zmm2,%zmm3
  18094c:	vprold $0x14,%zmm15,%zmm15
  180953:	vpxord %zmm4,%zmm3,%zmm4
  180959:	vprold $0x14,%zmm4,%zmm4
  180960:	vpaddd %zmm17,%zmm15,%zmm15
  180966:	vpaddd %zmm7,%zmm16,%zmm17
  18096c:	vpxord %zmm12,%zmm15,%zmm12
  180972:	vpaddd %zmm30,%zmm17,%zmm16
  180978:	vpaddd %zmm4,%zmm0,%zmm0
  18097e:	vprold $0x18,%zmm12,%zmm12
  180985:	vpxord %zmm16,%zmm10,%zmm10
  18098b:	vpxord %zmm2,%zmm0,%zmm0
  180991:	vprold $0x10,%zmm10,%zmm10
  180998:	vprold $0x18,%zmm0,%zmm0
  18099f:	vpaddd %zmm14,%zmm12,%zmm12
  1809a5:	vpaddd 0x180(%rsp),%zmm17,%zmm14
  1809ad:	vpaddd %zmm10,%zmm11,%zmm11
  1809b3:	vpaddd %zmm3,%zmm0,%zmm2
  1809b9:	vpxord %zmm7,%zmm11,%zmm7
  1809bf:	vpxord %zmm4,%zmm2,%zmm2
  1809c5:	vprold $0x14,%zmm7,%zmm7
  1809cc:	vprold $0x19,%zmm2,%zmm2
  1809d3:	vpaddd %zmm7,%zmm14,%zmm14
  1809d9:	vpxord %zmm10,%zmm14,%zmm10
  1809df:	vpaddd %zmm26,%zmm6,%zmm14
  1809e5:	vprold $0x18,%zmm10,%zmm10
  1809ec:	vpaddd %zmm11,%zmm10,%zmm11
  1809f2:	vpxord %zmm7,%zmm11,%zmm7
  1809f8:	vpaddd %zmm13,%zmm14,%zmm11
  1809fe:	vpxord %zmm11,%zmm8,%zmm8
  180a04:	vprold $0x19,%zmm7,%zmm7
  180a0b:	vpaddd %zmm1,%zmm11,%zmm1
  180a11:	vprold $0x10,%zmm8,%zmm8
  180a18:	vpaddd %zmm7,%zmm15,%zmm3
  180a1e:	vpaddd %zmm5,%zmm8,%zmm5
  180a24:	vpxord %zmm0,%zmm3,%zmm0
  180a2a:	vpxord %zmm6,%zmm5,%zmm6
  180a30:	vprold $0x10,%zmm0,%zmm0
  180a37:	vprold $0x14,%zmm6,%zmm6
  180a3e:	vpaddd %zmm6,%zmm1,%zmm1
  180a44:	vpxord %zmm8,%zmm1,%zmm6
  180a4a:	vpaddd %zmm28,%zmm1,%zmm1
  180a50:	vprold $0x18,%zmm6,%zmm6
  180a57:	vpaddd %zmm2,%zmm1,%zmm1
  180a5d:	vpxord %zmm10,%zmm1,%zmm4
  180a63:	vpaddd %zmm29,%zmm1,%zmm1
  180a69:	vpaddd %zmm5,%zmm6,%zmm5
  180a6f:	vpaddd %zmm0,%zmm5,%zmm0
  180a75:	vpxord %zmm7,%zmm0,%zmm0
  180a7b:	vprold $0x14,%zmm0,%zmm0
  180a82:	vpaddd %zmm3,%zmm0,%zmm0
  180a88:	vprold $0x10,%zmm4,%zmm3
  180a8f:	vpaddd %zmm3,%zmm12,%zmm4
  180a95:	vpxord %zmm2,%zmm4,%zmm2
  180a9b:	vprold $0x14,%zmm2,%zmm2
  180aa2:	vpaddd %zmm2,%zmm1,%zmm1
  180aa8:	vpxord %zmm3,%zmm1,%zmm1
  180aae:	vprold $0x18,%zmm1,%zmm1
  180ab5:	vpaddd %zmm4,%zmm1,%zmm1
  180abb:	vpxord %zmm0,%zmm1,%zmm0
  180ac1:	vptestnmd 0x1c0(%rsp),%zmm0,%k0
  180ac9:	kortestw %k0,%k0
  180acd:	je     180279 <bitz::utils::blake3x4::first_pow_nonce+0xf09>
  180ad3:	cmpl   $0x21,0x3c(%rsp)
  180ad8:	kmovd  %k0,%ebx
  180adc:	jb     18153e <bitz::utils::blake3x4::first_pow_nonce+0x21ce>
  180ae2:	data16 data16 data16 data16 cs nopw 0x0(%rax,%rax,1)
  180af0:	tzcnt  %ebx,%r14d
  180af5:	add    0x40(%rsp),%r14
  180afa:	mov    0x48(%rsp),%rdi
  180aff:	mov    0x3c(%rsp),%ecx
  180b03:	mov    %r13,%rsi
  180b06:	mov    %r14,%rdx
  180b09:	vzeroupper
  180b0c:	call   17eca0 <bitz::utils::blake3x4::pow_ok>
  180b11:	test   %al,%al
  180b13:	jne    181474 <bitz::utils::blake3x4::first_pow_nonce+0x2104>
  180b19:	lea    -0x1(%rbx),%eax
  180b1c:	and    %bx,%ax
  180b1f:	mov    %eax,%ebx
  180b21:	jne    180af0 <bitz::utils::blake3x4::first_pow_nonce+0x1780>
  180b23:	jmp    180279 <bitz::utils::blake3x4::first_pow_nonce+0xf09>
  180b28:	mov    0x40(%rsp),%r15
  180b2d:	vmovdqa64 0x180(%rsp),%zmm25
  180b35:	vmovdqa64 0x140(%rsp),%zmm26
  180b3d:	vmovdqa64 0x100(%rsp),%zmm27
  180b45:	vmovdqa64 0xc0(%rsp),%zmm28
  180b4d:	vmovdqa64 0x840(%rsp),%zmm29
  180b55:	vmovdqa64 0x800(%rsp),%zmm30
  180b5d:	vmovdqa64 0x7c0(%rsp),%zmm31
  180b65:	vmovdqa64 0x780(%rsp),%zmm6
  180b6d:	vmovdqa64 0x740(%rsp),%zmm13
  180b75:	vmovdqa64 0x700(%rsp),%zmm17
  180b7d:	vmovdqa64 0x6c0(%rsp),%zmm7
  180b85:	vmovdqa64 0x680(%rsp),%zmm9
  180b8d:	vmovdqa64 0x640(%rsp),%zmm18
  180b95:	vmovdqa64 0x600(%rsp),%zmm10
  180b9d:	vmovdqa64 -0x12bb67(%rip),%zmm0        # 55040 <GCC_except_table0+0xafc>
  180ba7:	vmovdqa64 -0x12bb31(%rip),%zmm4        # 55080 <GCC_except_table0+0xb3c>
  180bb1:	vpbroadcastd -0x1241df(%rip),%zmm5        # 5c9dc <perfetto::base::(anonymous namespace)::kHexmap+0x2fc>
  180bbb:	vpbroadcastd -0x1241b5(%rip),%zmm8        # 5ca10 <perfetto::base::(anonymous namespace)::kHexmap+0x330>
  180bc5:	vpbroadcastd -0x1242bf(%rip),%zmm11        # 5c910 <perfetto::base::(anonymous namespace)::kHexmap+0x230>
  180bcf:	vpbroadcastd -0x1241a1(%rip),%zmm12        # 5ca38 <perfetto::base::(anonymous namespace)::kHexmap+0x358>
  180bd9:	vpbroadcastd -0x124227(%rip),%zmm14        # 5c9bc <perfetto::base::(anonymous namespace)::kHexmap+0x2dc>
  180be3:	mov    0x70(%rsp),%rbx
  180be8:	mov    $0x0,%ecx
  180bed:	lea    -0x113e74(%rip),%rdx        # 6cd80 <perfetto::base::(anonymous namespace)::kHexmap+0x106a0>
  180bf4:	vpxor  %xmm3,%xmm3,%xmm3
  180bf8:	add    $0x10,%r15
  180bfc:	mov    %rbx,%rax
  180bff:	sub    %r15,%rax
  180c02:	cmovb  %rcx,%rax
  180c06:	cmp    $0xf,%rax
  180c0a:	jbe    1814cf <bitz::utils::blake3x4::first_pow_nonce+0x215f>
  180c10:	vpbroadcastd %r15d,%zmm2
  180c16:	mov    %r15,0x40(%rsp)
  180c1b:	shr    $0x20,%r15
  180c1f:	vmovdqa64 %zmm25,0x200(%rsp)
  180c27:	vmovdqa64 %zmm26,0x240(%rsp)
  180c2f:	vmovdqa64 %zmm27,0x280(%rsp)
  180c37:	vmovdqa64 %zmm28,0x2c0(%rsp)
  180c3f:	vmovdqa64 %zmm3,0x240(%r12)
  180c47:	vmovdqa64 %zmm3,0x200(%r12)
  180c4f:	vmovdqa64 %zmm3,0x1c0(%r12)
  180c57:	vmovdqa64 %zmm3,0x180(%r12)
  180c5f:	vmovdqa64 %zmm3,0x140(%r12)
  180c67:	vmovdqa64 %zmm3,0x100(%r12)
  180c6f:	vmovdqa64 %zmm3,0xc0(%r12)
  180c77:	vmovdqa64 %zmm3,0x80(%r12)
  180c7f:	vmovdqa64 %zmm3,0x40(%r12)
  180c87:	vmovdqa64 %zmm3,(%r12)
  180c8e:	vpaddd %zmm0,%zmm2,%zmm0
  180c94:	vpbroadcastd %r15d,%zmm1
  180c9a:	vpcmpltud %zmm2,%zmm0,%k0
  180ca1:	vpaddd %zmm4,%zmm2,%zmm2
  180ca7:	vmovdqa64 %zmm0,0x300(%rsp)
  180caf:	vpxord %zmm5,%zmm2,%zmm4
  180cb5:	vpmovm2d %k0,%zmm3
  180cbb:	vpsubd %zmm3,%zmm1,%zmm1
  180cc1:	vprold $0x10,%zmm4,%zmm3
  180cc8:	vpaddd %zmm2,%zmm1,%zmm2
  180cce:	vmovdqa64 %zmm1,0x340(%rsp)
  180cd6:	vpaddd %zmm8,%zmm3,%zmm4
  180cdc:	vpxord %zmm11,%zmm4,%zmm5
  180ce2:	vprold $0x14,%zmm5,%zmm5
  180ce9:	vpaddd %zmm5,%zmm2,%zmm8
  180cef:	vpxord %zmm3,%zmm8,%zmm2
  180cf5:	vpaddd %zmm14,%zmm8,%zmm19
  180cfb:	vprold $0x18,%zmm2,%zmm11
  180d02:	vpxord %zmm13,%zmm19,%zmm8
  180d08:	vprold $0x10,%zmm8,%zmm8
  180d0f:	vpaddd %zmm4,%zmm11,%zmm2
  180d15:	vpaddd %zmm10,%zmm2,%zmm4
  180d1b:	vpxord %zmm5,%zmm2,%zmm3
  180d21:	vpxord %zmm7,%zmm4,%zmm2
  180d27:	vprold $0x19,%zmm3,%zmm3
  180d2e:	vprold $0x14,%zmm2,%zmm5
  180d35:	vpaddd %zmm5,%zmm9,%zmm16
  180d3b:	vpaddd %zmm3,%zmm6,%zmm9
  180d41:	vpxord %zmm29,%zmm9,%zmm6
  180d47:	vpxord %zmm10,%zmm16,%zmm2
  180d4d:	vprold $0x10,%zmm6,%zmm6
  180d54:	vprold $0x18,%zmm2,%zmm2
  180d5b:	vpaddd %zmm12,%zmm6,%zmm10
  180d61:	vpaddd %zmm4,%zmm2,%zmm4
  180d67:	vpxord %zmm3,%zmm10,%zmm3
  180d6d:	vpxord %zmm5,%zmm4,%zmm5
  180d73:	vprold $0x14,%zmm3,%zmm12
  180d7a:	vprold $0x19,%zmm5,%zmm7
  180d81:	vpaddd %zmm12,%zmm9,%zmm15
  180d87:	vpxord %zmm6,%zmm15,%zmm3
  180d8d:	vprold $0x18,%zmm3,%zmm9
  180d94:	vpaddd %zmm10,%zmm9,%zmm3
  180d9a:	vpxord %zmm12,%zmm3,%zmm5
  180da0:	vpaddd %zmm30,%zmm8,%zmm12
  180da6:	vpxord %zmm14,%zmm12,%zmm10
  180dac:	vprold $0x19,%zmm5,%zmm6
  180db3:	vprold $0x14,%zmm10,%zmm14
  180dba:	vpaddd %zmm14,%zmm19,%zmm13
  180dc0:	vpxord %zmm8,%zmm13,%zmm5
  180dc6:	vpxord %zmm11,%zmm18,%zmm8
  180dcc:	vprold $0x10,%zmm8,%zmm8
  180dd3:	vprold $0x18,%zmm5,%zmm10
  180dda:	vpaddd %zmm17,%zmm8,%zmm19
  180de0:	vpaddd %zmm12,%zmm10,%zmm12
  180de6:	vpxord %zmm31,%zmm19,%zmm11
  180dec:	vpxord %zmm14,%zmm12,%zmm5
  180df2:	vprold $0x14,%zmm11,%zmm17
  180df9:	vprold $0x19,%zmm5,%zmm5
  180e00:	vpaddd %zmm17,%zmm18,%zmm11
  180e06:	vpxord %zmm8,%zmm11,%zmm8
  180e0c:	vprold $0x18,%zmm8,%zmm8
  180e13:	vpaddd %zmm19,%zmm8,%zmm14
  180e19:	vpxord %zmm17,%zmm14,%zmm17
  180e1f:	vprold $0x19,%zmm17,%zmm17
  180e26:	cs nopw 0x0(%rax,%rax,1)
  180e30:	mov    (%rdx),%rdi
  180e33:	cmp    $0xf,%rdi
  180e37:	ja     181783 <bitz::utils::blake3x4::first_pow_nonce+0x2413>
  180e3d:	mov    0x8(%rdx),%rcx
  180e41:	cmp    $0x10,%rcx
  180e45:	jae    18183c <bitz::utils::blake3x4::first_pow_nonce+0x24cc>
  180e4b:	mov    0x10(%rdx),%rax
  180e4f:	cmp    $0x10,%rax
  180e53:	jae    18176c <bitz::utils::blake3x4::first_pow_nonce+0x23fc>
  180e59:	mov    0x18(%rdx),%rbx
  180e5d:	cmp    $0x10,%rbx
  180e61:	jae    181825 <bitz::utils::blake3x4::first_pow_nonce+0x24b5>
  180e67:	mov    0x20(%rdx),%r12
  180e6b:	cmp    $0x10,%r12
  180e6f:	jae    181755 <bitz::utils::blake3x4::first_pow_nonce+0x23e5>
  180e75:	mov    0x28(%rdx),%r15
  180e79:	cmp    $0x10,%r15
  180e7d:	jae    18180e <bitz::utils::blake3x4::first_pow_nonce+0x249e>
  180e83:	mov    0x30(%rdx),%r13
  180e87:	cmp    $0x10,%r13
  180e8b:	jae    18173e <bitz::utils::blake3x4::first_pow_nonce+0x23ce>
  180e91:	mov    0x38(%rdx),%r14
  180e95:	cmp    $0x10,%r14
  180e99:	jae    1817e0 <bitz::utils::blake3x4::first_pow_nonce+0x2470>
  180e9f:	mov    0x40(%rdx),%r11
  180ea3:	cmp    $0x10,%r11
  180ea7:	jae    181727 <bitz::utils::blake3x4::first_pow_nonce+0x23b7>
  180ead:	mov    0x48(%rdx),%r10
  180eb1:	cmp    $0x10,%r10
  180eb5:	jae    1817f7 <bitz::utils::blake3x4::first_pow_nonce+0x2487>
  180ebb:	mov    0x50(%rdx),%r9
  180ebf:	cmp    $0x10,%r9
  180ec3:	jae    1816f7 <bitz::utils::blake3x4::first_pow_nonce+0x2387>
  180ec9:	mov    0x58(%rdx),%r8
  180ecd:	cmp    $0x10,%r8
  180ed1:	jae    1817b0 <bitz::utils::blake3x4::first_pow_nonce+0x2440>
  180ed7:	mov    0x60(%rdx),%rsi
  180edb:	mov    %rsi,0x68(%rsp)
  180ee0:	cmp    $0x10,%rsi
  180ee4:	jae    1816de <bitz::utils::blake3x4::first_pow_nonce+0x236e>
  180eea:	mov    0x68(%rdx),%rsi
  180eee:	mov    %rsi,0x60(%rsp)
  180ef3:	cmp    $0x10,%rsi
  180ef7:	jae    1817c7 <bitz::utils::blake3x4::first_pow_nonce+0x2457>
  180efd:	mov    0x70(%rdx),%rsi
  180f01:	mov    %rsi,0x58(%rsp)
  180f06:	cmp    $0x10,%rsi
  180f0a:	jae    18170e <bitz::utils::blake3x4::first_pow_nonce+0x239e>
  180f10:	mov    0x78(%rdx),%rsi
  180f14:	mov    %rsi,0x50(%rsp)
  180f19:	cmp    $0x10,%rsi
  180f1d:	jae    181797 <bitz::utils::blake3x4::first_pow_nonce+0x2427>
  180f23:	shl    $0x6,%r13
  180f27:	vpaddd %zmm11,%zmm5,%zmm11
  180f2d:	shl    $0x6,%rdi
  180f31:	vpaddd %zmm16,%zmm17,%zmm16
  180f37:	shl    $0x6,%rax
  180f3b:	vpaddd %zmm15,%zmm7,%zmm15
  180f41:	shl    $0x6,%r12
  180f45:	shl    $0x6,%r14
  180f49:	vpaddd %zmm13,%zmm6,%zmm13
  180f4f:	shl    $0x6,%rbx
  180f53:	shl    $0x6,%rcx
  180f57:	shl    $0x6,%r15
  180f5b:	shl    $0x6,%r11
  180f5f:	shl    $0x6,%r9
  180f63:	shl    $0x6,%r10
  180f67:	shl    $0x6,%r8
  180f6b:	sub    $0xffffffffffffff80,%rdx
  180f6f:	vpaddd 0x200(%rsp,%r13,1),%zmm11,%zmm18
  180f77:	vpaddd 0x200(%rsp,%rdi,1),%zmm16,%zmm16
  180f7f:	vpaddd 0x200(%rsp,%rax,1),%zmm15,%zmm21
  180f87:	vpaddd 0x200(%rsp,%r12,1),%zmm13,%zmm23
  180f8f:	mov    0x68(%rsp),%rax
  180f94:	mov    0x78(%rsp),%r13
  180f99:	vpaddd 0x200(%rsp,%r14,1),%zmm18,%zmm19
  180fa1:	vpaddd 0x200(%rsp,%rbx,1),%zmm21,%zmm22
  180fa9:	vpaddd 0x200(%rsp,%rcx,1),%zmm16,%zmm20
  180fb1:	vpaddd 0x200(%rsp,%r15,1),%zmm23,%zmm24
  180fb9:	shl    $0x6,%rax
  180fbd:	vpxord %zmm2,%zmm18,%zmm2
  180fc3:	vpxord %zmm9,%zmm16,%zmm9
  180fc9:	vpxord %zmm8,%zmm23,%zmm8
  180fcf:	vprold $0x10,%zmm2,%zmm2
  180fd6:	vprold $0x10,%zmm9,%zmm13
  180fdd:	vprold $0x10,%zmm8,%zmm8
  180fe4:	vpaddd %zmm3,%zmm2,%zmm3
  180fea:	vpaddd %zmm12,%zmm13,%zmm12
  180ff0:	vpaddd %zmm4,%zmm8,%zmm4
  180ff6:	vpxord %zmm5,%zmm3,%zmm5
  180ffc:	vpxord %zmm17,%zmm12,%zmm9
  181002:	vpxord %zmm6,%zmm4,%zmm6
  181008:	vprold $0x14,%zmm5,%zmm5
  18100f:	vprold $0x14,%zmm9,%zmm15
  181016:	vpxord %zmm10,%zmm21,%zmm9
  18101c:	vprold $0x14,%zmm6,%zmm6
  181023:	vprold $0x10,%zmm9,%zmm9
  18102a:	vpaddd %zmm5,%zmm19,%zmm18
  181030:	vpaddd %zmm15,%zmm20,%zmm16
  181036:	vpaddd %zmm6,%zmm24,%zmm20
  18103c:	vpxord %zmm2,%zmm18,%zmm2
  181042:	vpaddd %zmm14,%zmm9,%zmm10
  181048:	vpxord %zmm8,%zmm20,%zmm8
  18104e:	vpxord %zmm13,%zmm16,%zmm13
  181054:	vprold $0x18,%zmm2,%zmm2
  18105b:	vpxord %zmm7,%zmm10,%zmm7
  181061:	vprold $0x18,%zmm8,%zmm8
  181068:	vprold $0x18,%zmm13,%zmm13
  18106f:	vprold $0x14,%zmm7,%zmm7
  181076:	vpaddd %zmm3,%zmm2,%zmm3
  18107c:	vpaddd %zmm4,%zmm8,%zmm4
  181082:	vpaddd %zmm12,%zmm13,%zmm12
  181088:	vpxord %zmm5,%zmm3,%zmm5
  18108e:	vpaddd %zmm7,%zmm22,%zmm17
  181094:	vpxord %zmm6,%zmm4,%zmm6
  18109a:	vpxord %zmm15,%zmm12,%zmm15
  1810a0:	vprold $0x19,%zmm5,%zmm5
  1810a7:	vpxord %zmm9,%zmm17,%zmm9
  1810ad:	vprold $0x19,%zmm6,%zmm6
  1810b4:	vprold $0x19,%zmm15,%zmm22
  1810bb:	vprold $0x18,%zmm9,%zmm9
  1810c2:	vpaddd %zmm20,%zmm5,%zmm19
  1810c8:	vpaddd 0x200(%rsp,%rax,1),%zmm19,%zmm19
  1810d0:	mov    0x60(%rsp),%rax
  1810d5:	vpaddd %zmm17,%zmm6,%zmm17
  1810db:	vpaddd 0x200(%rsp,%r9,1),%zmm17,%zmm21
  1810e3:	vpaddd %zmm22,%zmm18,%zmm15
  1810e9:	vpaddd %zmm10,%zmm9,%zmm11
  1810ef:	vpaddd 0x200(%rsp,%r8,1),%zmm21,%zmm17
  1810f7:	vpxord %zmm7,%zmm11,%zmm7
  1810fd:	vprold $0x19,%zmm7,%zmm7
  181104:	shl    $0x6,%rax
  181108:	vpaddd %zmm16,%zmm7,%zmm10
  18110e:	vpaddd 0x200(%rsp,%r11,1),%zmm10,%zmm14
  181116:	vpaddd 0x200(%rsp,%rax,1),%zmm19,%zmm20
  18111e:	mov    0x58(%rsp),%rax
  181123:	vpaddd 0x200(%rsp,%r10,1),%zmm14,%zmm10
  18112b:	vpxord %zmm13,%zmm21,%zmm13
  181131:	shl    $0x6,%rax
  181135:	vpaddd 0x200(%rsp,%rax,1),%zmm15,%zmm15
  18113d:	mov    0x50(%rsp),%rax
  181142:	vpxord %zmm2,%zmm14,%zmm2
  181148:	vpxord %zmm9,%zmm19,%zmm14
  18114e:	vprold $0x10,%zmm13,%zmm9
  181155:	vprold $0x10,%zmm2,%zmm2
  18115c:	vpaddd %zmm3,%zmm9,%zmm3
  181162:	vpaddd %zmm4,%zmm2,%zmm4
  181168:	vpxord %zmm6,%zmm3,%zmm6
  18116e:	vpxord %zmm7,%zmm4,%zmm7
  181174:	shl    $0x6,%rax
  181178:	vprold $0x14,%zmm6,%zmm6
  18117f:	vpaddd 0x200(%rsp,%rax,1),%zmm15,%zmm18
  181187:	vprold $0x14,%zmm7,%zmm7
  18118e:	lea    -0x114195(%rip),%rax        # 6d000 <perfetto::base::(anonymous namespace)::kHexmap+0x10920>
  181195:	vpxord %zmm8,%zmm15,%zmm8
  18119b:	vpaddd %zmm7,%zmm10,%zmm16
  1811a1:	vprold $0x10,%zmm14,%zmm10
  1811a8:	vpaddd %zmm6,%zmm17,%zmm15
  1811ae:	vprold $0x10,%zmm8,%zmm8
  1811b5:	vpxord %zmm2,%zmm16,%zmm2
  1811bb:	vpxord %zmm9,%zmm15,%zmm9
  1811c1:	vpaddd %zmm12,%zmm10,%zmm12
  1811c7:	vprold $0x18,%zmm2,%zmm2
  1811ce:	vprold $0x18,%zmm9,%zmm9
  1811d5:	vpaddd %zmm11,%zmm8,%zmm14
  1811db:	vpxord %zmm5,%zmm12,%zmm5
  1811e1:	vpxord %zmm22,%zmm14,%zmm11
  1811e7:	vprold $0x14,%zmm5,%zmm5
  1811ee:	vpaddd %zmm4,%zmm2,%zmm4
  1811f4:	vpaddd %zmm3,%zmm9,%zmm3
  1811fa:	vprold $0x14,%zmm11,%zmm17
  181201:	vpxord %zmm7,%zmm4,%zmm7
  181207:	vpxord %zmm6,%zmm3,%zmm6
  18120d:	vpaddd %zmm5,%zmm20,%zmm13
  181213:	vprold $0x19,%zmm7,%zmm7
  18121a:	vprold $0x19,%zmm6,%zmm6
  181221:	vpaddd %zmm17,%zmm18,%zmm11
  181227:	vpxord %zmm10,%zmm13,%zmm10
  18122d:	vpxord %zmm8,%zmm11,%zmm8
  181233:	vprold $0x18,%zmm10,%zmm10
  18123a:	vprold $0x18,%zmm8,%zmm8
  181241:	vpaddd %zmm12,%zmm10,%zmm12
  181247:	vpaddd %zmm14,%zmm8,%zmm14
  18124d:	vpxord %zmm5,%zmm12,%zmm5
  181253:	vpxord %zmm17,%zmm14,%zmm17
  181259:	vprold $0x19,%zmm5,%zmm5
  181260:	vprold $0x19,%zmm17,%zmm17
  181267:	cmp    %rax,%rdx
  18126a:	jne    180e30 <bitz::utils::blake3x4::first_pow_nonce+0x1ac0>
  181270:	vpaddd %zmm1,%zmm7,%zmm1
  181276:	vpaddd %zmm17,%zmm16,%zmm16
  18127c:	vpaddd %zmm11,%zmm5,%zmm11
  181282:	lea    0x380(%rsp),%r12
  18128a:	vpaddd %zmm15,%zmm1,%zmm1
  181290:	vpxord %zmm9,%zmm16,%zmm9
  181296:	vpxord %zmm2,%zmm11,%zmm2
  18129c:	vpxord %zmm1,%zmm10,%zmm10
  1812a2:	vprold $0x10,%zmm9,%zmm9
  1812a9:	vpaddd %zmm25,%zmm1,%zmm1
  1812af:	vprold $0x10,%zmm2,%zmm2
  1812b6:	vprold $0x10,%zmm10,%zmm10
  1812bd:	vpaddd %zmm12,%zmm9,%zmm12
  1812c3:	vpaddd %zmm3,%zmm2,%zmm3
  1812c9:	vpaddd %zmm10,%zmm14,%zmm14
  1812cf:	vpxord %zmm17,%zmm12,%zmm17
  1812d5:	vpxord %zmm5,%zmm3,%zmm5
  1812db:	vpxord %zmm7,%zmm14,%zmm7
  1812e1:	vprold $0x14,%zmm17,%zmm17
  1812e8:	vprold $0x14,%zmm5,%zmm5
  1812ef:	vprold $0x14,%zmm7,%zmm7
  1812f6:	vpaddd %zmm17,%zmm16,%zmm16
  1812fc:	vpaddd %zmm7,%zmm1,%zmm1
  181302:	vpxord %zmm9,%zmm16,%zmm9
  181308:	vpxord %zmm10,%zmm1,%zmm1
  18130e:	vprold $0x18,%zmm9,%zmm9
  181315:	vprold $0x18,%zmm1,%zmm1
  18131c:	vpaddd %zmm12,%zmm9,%zmm9
  181322:	vpaddd %zmm26,%zmm6,%zmm12
  181328:	vpaddd %zmm14,%zmm1,%zmm10
  18132e:	vpxord %zmm7,%zmm10,%zmm7
  181334:	vpaddd %zmm13,%zmm12,%zmm10
  18133a:	vpxord %zmm10,%zmm8,%zmm8
  181340:	vprold $0x19,%zmm7,%zmm7
  181347:	vprold $0x10,%zmm8,%zmm8
  18134e:	vpaddd %zmm4,%zmm8,%zmm4
  181354:	vpxord %zmm6,%zmm4,%zmm6
  18135a:	vprold $0x14,%zmm6,%zmm6
  181361:	vpaddd %zmm6,%zmm10,%zmm6
  181367:	vpaddd %zmm11,%zmm5,%zmm10
  18136d:	vpxord %zmm2,%zmm10,%zmm2
  181373:	vpxord %zmm8,%zmm6,%zmm8
  181379:	vpaddd %zmm7,%zmm16,%zmm10
  18137f:	vprold $0x18,%zmm2,%zmm2
  181386:	vprold $0x18,%zmm8,%zmm8
  18138d:	vpaddd %zmm3,%zmm2,%zmm3
  181393:	vpxord %zmm2,%zmm10,%zmm2
  181399:	vpaddd %zmm4,%zmm8,%zmm4
  18139f:	vpxord %zmm5,%zmm3,%zmm3
  1813a5:	vprold $0x10,%zmm2,%zmm2
  1813ac:	vpaddd %zmm28,%zmm6,%zmm5
  1813b2:	vprold $0x19,%zmm3,%zmm3
  1813b9:	vpaddd %zmm2,%zmm4,%zmm2
  1813bf:	vpaddd %zmm3,%zmm5,%zmm4
  1813c5:	vpxord %zmm7,%zmm2,%zmm2
  1813cb:	vpxord %zmm1,%zmm4,%zmm1
  1813d1:	vpaddd %zmm0,%zmm4,%zmm0
  1813d7:	vprold $0x14,%zmm2,%zmm2
  1813de:	vprold $0x10,%zmm1,%zmm1
  1813e5:	vpaddd %zmm2,%zmm10,%zmm2
  1813eb:	vpaddd %zmm1,%zmm9,%zmm5
  1813f1:	vpxord %zmm3,%zmm5,%zmm3
  1813f7:	vprold $0x14,%zmm3,%zmm3
  1813fe:	vpaddd %zmm3,%zmm0,%zmm0
  181404:	vpxord %zmm1,%zmm0,%zmm0
  18140a:	vprold $0x18,%zmm0,%zmm0
  181411:	vpaddd %zmm5,%zmm0,%zmm0
  181417:	vpxord %zmm2,%zmm0,%zmm0
  18141d:	vptestnmd 0x1c0(%rsp),%zmm0,%k0
  181425:	kortestw %k0,%k0
  181429:	je     180b28 <bitz::utils::blake3x4::first_pow_nonce+0x17b8>
  18142f:	cmpl   $0x21,0x3c(%rsp)
  181434:	kmovd  %k0,%ebx
  181438:	jb     18153e <bitz::utils::blake3x4::first_pow_nonce+0x21ce>
  18143e:	xchg   %ax,%ax
  181440:	tzcnt  %ebx,%r14d
  181445:	add    0x40(%rsp),%r14
  18144a:	mov    0x48(%rsp),%rdi
  18144f:	mov    0x3c(%rsp),%ecx
  181453:	mov    %r13,%rsi
  181456:	mov    %r14,%rdx
  181459:	vzeroupper
  18145c:	call   17eca0 <bitz::utils::blake3x4::pow_ok>
  181461:	test   %al,%al
  181463:	jne    181474 <bitz::utils::blake3x4::first_pow_nonce+0x2104>
  181465:	lea    -0x1(%rbx),%eax
  181468:	and    %bx,%ax
  18146b:	mov    %eax,%ebx
  18146d:	jne    181440 <bitz::utils::blake3x4::first_pow_nonce+0x20d0>
  18146f:	jmp    180b28 <bitz::utils::blake3x4::first_pow_nonce+0x17b8>
  181474:	mov    $0x1,%eax
  181479:	jmp    181529 <bitz::utils::blake3x4::first_pow_nonce+0x21b9>
  18147e:	mov    0x40(%rsp),%r15
  181483:	cmp    %r15,%rbx
  181486:	cmovbe %r15,%rbx
  18148a:	dec    %r15
  18148d:	mov    %rbx,%r14
  181490:	neg    %r14
  181493:	mov    %rbx,%r12
  181496:	cs nopw 0x0(%rax,%rax,1)
  1814a0:	lea    (%r14,%r15,1),%rbx
  1814a4:	cmp    $0xffffffffffffffff,%rbx
  1814a8:	je     18151a <bitz::utils::blake3x4::first_pow_nonce+0x21aa>
  1814aa:	mov    0x48(%rsp),%rdi
  1814af:	mov    0x3c(%rsp),%ecx
  1814b3:	inc    %r15
  1814b6:	mov    %r13,%rsi
  1814b9:	mov    %r15,%rdx
  1814bc:	vzeroupper
  1814bf:	call   17eca0 <bitz::utils::blake3x4::pow_ok>
  1814c4:	test   %al,%al
  1814c6:	je     1814a0 <bitz::utils::blake3x4::first_pow_nonce+0x2130>
  1814c8:	jmp    18151d <bitz::utils::blake3x4::first_pow_nonce+0x21ad>
  1814ca:	mov    0x40(%rsp),%r15
  1814cf:	cmp    %r15,%rbx
  1814d2:	cmovbe %r15,%rbx
  1814d6:	dec    %r15
  1814d9:	mov    %rbx,%r14
  1814dc:	neg    %r14
  1814df:	mov    %rbx,%r12
  1814e2:	data16 data16 data16 data16 cs nopw 0x0(%rax,%rax,1)
  1814f0:	lea    (%r14,%r15,1),%rbx
  1814f4:	cmp    $0xffffffffffffffff,%rbx
  1814f8:	je     18151a <bitz::utils::blake3x4::first_pow_nonce+0x21aa>
  1814fa:	mov    0x48(%rsp),%rdi
  1814ff:	mov    0x3c(%rsp),%ecx
  181503:	inc    %r15
  181506:	mov    %r13,%rsi
  181509:	mov    %r15,%rdx
  18150c:	vzeroupper
  18150f:	call   17eca0 <bitz::utils::blake3x4::pow_ok>
  181514:	test   %al,%al
  181516:	je     1814f0 <bitz::utils::blake3x4::first_pow_nonce+0x2180>
  181518:	jmp    18151d <bitz::utils::blake3x4::first_pow_nonce+0x21ad>
  18151a:	mov    %r12,%r15
  18151d:	xor    %eax,%eax
  18151f:	cmp    $0xffffffffffffffff,%rbx
  181523:	mov    %r15,%r14
  181526:	setne  %al
  181529:	mov    %r14,%rdx
  18152c:	lea    -0x28(%rbp),%rsp
  181530:	pop    %rbx
  181531:	pop    %r12
  181533:	pop    %r13
  181535:	pop    %r14
  181537:	pop    %r15
  181539:	pop    %rbp
  18153a:	vzeroupper
  18153d:	ret
  18153e:	mov    0x40(%rsp),%r14
  181543:	tzcnt  %ebx,%eax
  181547:	add    %rax,%r14
  18154a:	mov    $0x1,%eax
  18154f:	jmp    181529 <bitz::utils::blake3x4::first_pow_nonce+0x21b9>
  181551:	lea    -0x111684(%rip),%rdi        # 6fed4 <perfetto::base::(anonymous namespace)::kHexmap+0x137f4>
  181558:	lea    0x423831(%rip),%rdx        # 5a4d90 <falcon_hybrid::main::__CALLSITE::META+0x60f8>
  18155f:	mov    $0x4d,%esi
  181564:	call   3cc4e0 <core::panicking::panic_fmt>
  181569:	lea    0x423b98(%rip),%rdx        # 5a5108 <falcon_hybrid::main::__CALLSITE::META+0x6470>
  181570:	mov    0x50(%rsp),%rdi
  181575:	mov    $0x10,%esi
  18157a:	vzeroupper
  18157d:	call   3cd58b <core::panicking::panic_bounds_check>
  181582:	lea    0x423b7f(%rip),%rdx        # 5a5108 <falcon_hybrid::main::__CALLSITE::META+0x6470>
  181589:	mov    0x60(%rsp),%rdi
  18158e:	mov    $0x10,%esi
  181593:	vzeroupper
  181596:	call   3cd58b <core::panicking::panic_bounds_check>
  18159b:	lea    0x423b66(%rip),%rdx        # 5a5108 <falcon_hybrid::main::__CALLSITE::META+0x6470>
  1815a2:	mov    $0x10,%esi
  1815a7:	mov    %r9,%rdi
  1815aa:	vzeroupper
  1815ad:	call   3cd58b <core::panicking::panic_bounds_check>
  1815b2:	lea    0x423b4f(%rip),%rdx        # 5a5108 <falcon_hybrid::main::__CALLSITE::META+0x6470>
  1815b9:	mov    $0x10,%esi
  1815be:	mov    %r11,%rdi
  1815c1:	vzeroupper
  1815c4:	call   3cd58b <core::panicking::panic_bounds_check>
  1815c9:	lea    0x423b38(%rip),%rdx        # 5a5108 <falcon_hybrid::main::__CALLSITE::META+0x6470>
  1815d0:	mov    $0x10,%esi
  1815d5:	mov    %r15,%rdi
  1815d8:	vzeroupper
  1815db:	call   3cd58b <core::panicking::panic_bounds_check>
  1815e0:	lea    0x423b21(%rip),%rdx        # 5a5108 <falcon_hybrid::main::__CALLSITE::META+0x6470>
  1815e7:	mov    $0x10,%esi
  1815ec:	mov    %rbx,%rdi
  1815ef:	vzeroupper
  1815f2:	call   3cd58b <core::panicking::panic_bounds_check>
  1815f7:	lea    0x423b0a(%rip),%rdx        # 5a5108 <falcon_hybrid::main::__CALLSITE::META+0x6470>
  1815fe:	mov    $0x10,%esi
  181603:	mov    %rax,%rdi
  181606:	vzeroupper
  181609:	call   3cd58b <core::panicking::panic_bounds_check>
  18160e:	lea    0x423af3(%rip),%rdx        # 5a5108 <falcon_hybrid::main::__CALLSITE::META+0x6470>
  181615:	mov    $0x10,%esi
  18161a:	vzeroupper
  18161d:	call   3cd58b <core::panicking::panic_bounds_check>
  181622:	lea    0x423adf(%rip),%rdx        # 5a5108 <falcon_hybrid::main::__CALLSITE::META+0x6470>
  181629:	mov    $0x10,%esi
  18162e:	mov    %r8,%rdi
  181631:	vzeroupper
  181634:	call   3cd58b <core::panicking::panic_bounds_check>
  181639:	lea    0x423ac8(%rip),%rdx        # 5a5108 <falcon_hybrid::main::__CALLSITE::META+0x6470>
  181640:	mov    0x58(%rsp),%rdi
  181645:	mov    $0x10,%esi
  18164a:	vzeroupper
  18164d:	call   3cd58b <core::panicking::panic_bounds_check>
  181652:	lea    0x423aaf(%rip),%rdx        # 5a5108 <falcon_hybrid::main::__CALLSITE::META+0x6470>
  181659:	mov    0x68(%rsp),%rdi
  18165e:	mov    $0x10,%esi
  181663:	vzeroupper
  181666:	call   3cd58b <core::panicking::panic_bounds_check>
  18166b:	lea    0x423a96(%rip),%rdx        # 5a5108 <falcon_hybrid::main::__CALLSITE::META+0x6470>
  181672:	mov    $0x10,%esi
  181677:	mov    %r10,%rdi
  18167a:	vzeroupper
  18167d:	call   3cd58b <core::panicking::panic_bounds_check>
  181682:	lea    0x423a7f(%rip),%rdx        # 5a5108 <falcon_hybrid::main::__CALLSITE::META+0x6470>
  181689:	mov    $0x10,%esi
  18168e:	mov    %r13,%rdi
  181691:	vzeroupper
  181694:	call   3cd58b <core::panicking::panic_bounds_check>
  181699:	lea    0x423a68(%rip),%rdx        # 5a5108 <falcon_hybrid::main::__CALLSITE::META+0x6470>
  1816a0:	mov    $0x10,%esi
  1816a5:	mov    %r12,%rdi
  1816a8:	vzeroupper
  1816ab:	call   3cd58b <core::panicking::panic_bounds_check>
  1816b0:	lea    0x423a51(%rip),%rdx        # 5a5108 <falcon_hybrid::main::__CALLSITE::META+0x6470>
  1816b7:	mov    $0x10,%esi
  1816bc:	mov    %r14,%rdi
  1816bf:	vzeroupper
  1816c2:	call   3cd58b <core::panicking::panic_bounds_check>
  1816c7:	lea    0x423a3a(%rip),%rdx        # 5a5108 <falcon_hybrid::main::__CALLSITE::META+0x6470>
  1816ce:	mov    $0x10,%esi
  1816d3:	mov    %rcx,%rdi
  1816d6:	vzeroupper
  1816d9:	call   3cd58b <core::panicking::panic_bounds_check>
  1816de:	lea    0x41e2db(%rip),%rdx        # 59f9c0 <falcon_hybrid::main::__CALLSITE::META+0xd28>
  1816e5:	mov    0x68(%rsp),%rdi
  1816ea:	mov    $0x10,%esi
  1816ef:	vzeroupper
  1816f2:	call   3cd58b <core::panicking::panic_bounds_check>
  1816f7:	lea    0x41e2c2(%rip),%rdx        # 59f9c0 <falcon_hybrid::main::__CALLSITE::META+0xd28>
  1816fe:	mov    $0x10,%esi
  181703:	mov    %r9,%rdi
  181706:	vzeroupper
  181709:	call   3cd58b <core::panicking::panic_bounds_check>
  18170e:	lea    0x41e2ab(%rip),%rdx        # 59f9c0 <falcon_hybrid::main::__CALLSITE::META+0xd28>
  181715:	mov    0x58(%rsp),%rdi
  18171a:	mov    $0x10,%esi
  18171f:	vzeroupper
  181722:	call   3cd58b <core::panicking::panic_bounds_check>
  181727:	lea    0x41e292(%rip),%rdx        # 59f9c0 <falcon_hybrid::main::__CALLSITE::META+0xd28>
  18172e:	mov    $0x10,%esi
  181733:	mov    %r11,%rdi
  181736:	vzeroupper
  181739:	call   3cd58b <core::panicking::panic_bounds_check>
  18173e:	lea    0x41e27b(%rip),%rdx        # 59f9c0 <falcon_hybrid::main::__CALLSITE::META+0xd28>
  181745:	mov    $0x10,%esi
  18174a:	mov    %r13,%rdi
  18174d:	vzeroupper
  181750:	call   3cd58b <core::panicking::panic_bounds_check>
  181755:	lea    0x41e264(%rip),%rdx        # 59f9c0 <falcon_hybrid::main::__CALLSITE::META+0xd28>
  18175c:	mov    $0x10,%esi
  181761:	mov    %r12,%rdi
  181764:	vzeroupper
  181767:	call   3cd58b <core::panicking::panic_bounds_check>
  18176c:	lea    0x41e24d(%rip),%rdx        # 59f9c0 <falcon_hybrid::main::__CALLSITE::META+0xd28>
  181773:	mov    $0x10,%esi
  181778:	mov    %rax,%rdi
  18177b:	vzeroupper
  18177e:	call   3cd58b <core::panicking::panic_bounds_check>
  181783:	lea    0x41e236(%rip),%rdx        # 59f9c0 <falcon_hybrid::main::__CALLSITE::META+0xd28>
  18178a:	mov    $0x10,%esi
  18178f:	vzeroupper
  181792:	call   3cd58b <core::panicking::panic_bounds_check>
  181797:	lea    0x41e222(%rip),%rdx        # 59f9c0 <falcon_hybrid::main::__CALLSITE::META+0xd28>
  18179e:	mov    0x50(%rsp),%rdi
  1817a3:	mov    $0x10,%esi
  1817a8:	vzeroupper
  1817ab:	call   3cd58b <core::panicking::panic_bounds_check>
  1817b0:	lea    0x41e209(%rip),%rdx        # 59f9c0 <falcon_hybrid::main::__CALLSITE::META+0xd28>
  1817b7:	mov    $0x10,%esi
  1817bc:	mov    %r8,%rdi
  1817bf:	vzeroupper
  1817c2:	call   3cd58b <core::panicking::panic_bounds_check>
  1817c7:	lea    0x41e1f2(%rip),%rdx        # 59f9c0 <falcon_hybrid::main::__CALLSITE::META+0xd28>
  1817ce:	mov    0x60(%rsp),%rdi
  1817d3:	mov    $0x10,%esi
  1817d8:	vzeroupper
  1817db:	call   3cd58b <core::panicking::panic_bounds_check>
  1817e0:	lea    0x41e1d9(%rip),%rdx        # 59f9c0 <falcon_hybrid::main::__CALLSITE::META+0xd28>
  1817e7:	mov    $0x10,%esi
  1817ec:	mov    %r14,%rdi
  1817ef:	vzeroupper
  1817f2:	call   3cd58b <core::panicking::panic_bounds_check>
  1817f7:	lea    0x41e1c2(%rip),%rdx        # 59f9c0 <falcon_hybrid::main::__CALLSITE::META+0xd28>
  1817fe:	mov    $0x10,%esi
  181803:	mov    %r10,%rdi
  181806:	vzeroupper
  181809:	call   3cd58b <core::panicking::panic_bounds_check>
  18180e:	lea    0x41e1ab(%rip),%rdx        # 59f9c0 <falcon_hybrid::main::__CALLSITE::META+0xd28>
  181815:	mov    $0x10,%esi
  18181a:	mov    %r15,%rdi
  18181d:	vzeroupper
  181820:	call   3cd58b <core::panicking::panic_bounds_check>
  181825:	lea    0x41e194(%rip),%rdx        # 59f9c0 <falcon_hybrid::main::__CALLSITE::META+0xd28>
  18182c:	mov    $0x10,%esi
  181831:	mov    %rbx,%rdi
  181834:	vzeroupper
  181837:	call   3cd58b <core::panicking::panic_bounds_check>
  18183c:	lea    0x41e17d(%rip),%rdx        # 59f9c0 <falcon_hybrid::main::__CALLSITE::META+0xd28>
  181843:	mov    $0x10,%esi
  181848:	mov    %rcx,%rdi
  18184b:	vzeroupper
  18184e:	call   3cd58b <core::panicking::panic_bounds_check>

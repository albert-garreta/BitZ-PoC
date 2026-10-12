
/tmp/falcon-compact-opening-binaries/candidate-v6:     file format elf64-x86-64


Disassembly of section .text:

0000000000180400 <bitz::utils::blake3x4::first_pow_nonce>:
  180400:	push   %rbp
  180401:	mov    %rsp,%rbp
  180404:	push   %r15
  180406:	push   %r14
  180408:	push   %r13
  18040a:	push   %r12
  18040c:	push   %rbx
  18040d:	and    $0xffffffffffffffc0,%rsp
  180411:	sub    $0xb80,%rsp
  180418:	mov    %r8d,0x3c(%rsp)
  18041d:	mov    %rdi,0x48(%rsp)
  180422:	cmp    $0x39,%rsi
  180426:	jae    1825e1 <bitz::utils::blake3x4::first_pow_nonce+0x21e1>
  18042c:	mov    %rcx,%rbx
  18042f:	mov    %rsi,%r13
  180432:	test   $0x3,%r13b
  180436:	je     18047e <bitz::utils::blake3x4::first_pow_nonce+0x7e>
  180438:	cmp    %rdx,%rbx
  18043b:	cmovbe %rdx,%rbx
  18043f:	dec    %rdx
  180442:	mov    %rbx,%r15
  180445:	neg    %r15
  180448:	mov    %rbx,%r12
  18044b:	mov    %rdx,%r14
  18044e:	xchg   %ax,%ax
  180450:	lea    (%r15,%r14,1),%rbx
  180454:	cmp    $0xffffffffffffffff,%rbx
  180458:	je     1808c3 <bitz::utils::blake3x4::first_pow_nonce+0x4c3>
  18045e:	mov    0x48(%rsp),%rdi
  180463:	mov    0x3c(%rsp),%ecx
  180467:	inc    %r14
  18046a:	mov    %r13,%rsi
  18046d:	mov    %r14,%rdx
  180470:	call   17fd30 <bitz::utils::blake3x4::pow_ok>
  180475:	test   %al,%al
  180477:	je     180450 <bitz::utils::blake3x4::first_pow_nonce+0x50>
  180479:	jmp    1808c6 <bitz::utils::blake3x4::first_pow_nonce+0x4c6>
  18047e:	mov    %r13,%r14
  180481:	shr    $0x2,%r14
  180485:	vpxor  %xmm0,%xmm0,%xmm0
  180489:	vmovdqu %ymm0,0x98(%rsp)
  180492:	mov    %rdx,0x40(%rsp)
  180497:	vmovdqa %ymm0,0x80(%rsp)
  1804a0:	mov    %rbx,0x70(%rsp)
  1804a5:	mov    %r13,0x78(%rsp)
  1804aa:	test   %r13,%r13
  1804ad:	je     1808d1 <bitz::utils::blake3x4::first_pow_nonce+0x4d1>
  1804b3:	mov    0x48(%rsp),%rsi
  1804b8:	lea    0x80(%rsp),%rdi
  1804c0:	mov    %r13,%rdx
  1804c3:	vzeroupper
  1804c6:	call   *0x435b8c(%rip)        # 5b6058 <memcpy@GLIBC_2.14>
  1804cc:	cmp    $0x4,%r14
  1804d0:	je     181101 <bitz::utils::blake3x4::first_pow_nonce+0xd01>
  1804d6:	cmp    $0x8,%r14
  1804da:	jne    1808d1 <bitz::utils::blake3x4::first_pow_nonce+0x4d1>
  1804e0:	xor    %ecx,%ecx
  1804e2:	mov    $0x20,%eax
  1804e7:	sub    0x3c(%rsp),%eax
  1804eb:	mov    $0xffffffff,%edx
  1804f0:	cmovb  %ecx,%eax
  1804f3:	cmp    $0x20,%eax
  1804f6:	shlx   %eax,%edx,%eax
  1804fb:	mov    %rbx,%rdx
  1804fe:	bswap  %eax
  180500:	cmovae %ecx,%eax
  180503:	sub    0x40(%rsp),%rdx
  180508:	cmovae %rdx,%rcx
  18050c:	cmp    $0x10,%rcx
  180510:	jb     18250e <bitz::utils::blake3x4::first_pow_nonce+0x210e>
  180516:	vpbroadcastd 0x80(%rsp),%zmm25
  18051e:	vpbroadcastd 0x88(%rsp),%zmm27
  180526:	vpbroadcastd 0x84(%rsp),%zmm26
  18052e:	vpbroadcastd 0x8c(%rsp),%zmm28
  180536:	vpbroadcastd 0x98(%rsp),%zmm31
  18053e:	vpbroadcastd 0x9c(%rsp),%zmm14
  180546:	vpbroadcastd 0x90(%rsp),%zmm29
  18054e:	vpbroadcastd 0x94(%rsp),%zmm30
  180556:	mov    0x40(%rsp),%r15
  18055b:	lea    0x480(%rsp),%r12
  180563:	lea    -0x113882(%rip),%rdx        # 6cce8 <perfetto::base::(anonymous namespace)::kHexmap+0x10688>
  18056a:	vpaddd -0x123c78(%rip){1to16},%zmm25,%zmm0        # 5c8fc <perfetto::base::(anonymous namespace)::kHexmap+0x29c>
  180574:	vpaddd -0x123bf6(%rip){1to16},%zmm27,%zmm8        # 5c988 <perfetto::base::(anonymous namespace)::kHexmap+0x328>
  18057e:	vpaddd -0x123d40(%rip){1to16},%zmm31,%zmm11        # 5c848 <perfetto::base::(anonymous namespace)::kHexmap+0x1e8>
  180588:	vpaddd -0x123bde(%rip){1to16},%zmm29,%zmm17        # 5c9b4 <perfetto::base::(anonymous namespace)::kHexmap+0x354>
  180592:	vmovdqa64 %zmm25,0x140(%rsp)
  18059a:	vmovdqa64 %zmm26,0x100(%rsp)
  1805a2:	vmovdqa64 %zmm27,0xc0(%rsp)
  1805aa:	vmovdqa64 %zmm28,0x840(%rsp)
  1805b2:	vmovdqa64 %zmm29,0x800(%rsp)
  1805ba:	vmovdqa64 %zmm30,0x7c0(%rsp)
  1805c2:	vmovdqa64 %zmm31,0x780(%rsp)
  1805ca:	vmovdqa64 %zmm14,0x740(%rsp)
  1805d2:	vprold $0x10,%zmm0,%zmm1
  1805d9:	vpaddd -0x123de7(%rip){1to16},%zmm1,%zmm2        # 5c7fc <perfetto::base::(anonymous namespace)::kHexmap+0x19c>
  1805e3:	vprold $0x10,%zmm8,%zmm5
  1805ea:	vpaddd -0x123c68(%rip){1to16},%zmm5,%zmm6        # 5c98c <perfetto::base::(anonymous namespace)::kHexmap+0x32c>
  1805f4:	vpaddd %zmm26,%zmm0,%zmm0
  1805fa:	vpxord -0x123ca4(%rip){1to16},%zmm2,%zmm3        # 5c960 <perfetto::base::(anonymous namespace)::kHexmap+0x300>
  180604:	vpxord -0x123d2a(%rip){1to16},%zmm6,%zmm7        # 5c8e4 <perfetto::base::(anonymous namespace)::kHexmap+0x284>
  18060e:	vprold $0x14,%zmm3,%zmm3
  180615:	vprold $0x14,%zmm7,%zmm7
  18061c:	vpaddd %zmm3,%zmm0,%zmm0
  180622:	vpxord %zmm1,%zmm0,%zmm1
  180628:	vprold $0x18,%zmm1,%zmm4
  18062f:	vpaddd %zmm2,%zmm4,%zmm2
  180635:	vpxord %zmm3,%zmm2,%zmm1
  18063b:	vpaddd %zmm28,%zmm8,%zmm3
  180641:	vpxord -0x123cdf(%rip){1to16},%zmm11,%zmm8        # 5c96c <perfetto::base::(anonymous namespace)::kHexmap+0x30c>
  18064b:	vpaddd %zmm7,%zmm3,%zmm3
  180651:	vprold $0x19,%zmm1,%zmm1
  180658:	vpxord %zmm5,%zmm3,%zmm5
  18065e:	vprold $0x18,%zmm5,%zmm5
  180665:	vpaddd %zmm6,%zmm5,%zmm6
  18066b:	vpxord %zmm7,%zmm6,%zmm7
  180671:	vprold $0x19,%zmm7,%zmm15
  180678:	vpaddd %zmm14,%zmm11,%zmm7
  18067e:	vpxord -0x123e70(%rip){1to16},%zmm17,%zmm11        # 5c818 <perfetto::base::(anonymous namespace)::kHexmap+0x1b8>
  180688:	vprold $0x10,%zmm8,%zmm8
  18068f:	vpaddd -0x123d65(%rip){1to16},%zmm8,%zmm9        # 5c934 <perfetto::base::(anonymous namespace)::kHexmap+0x2d4>
  180699:	vpxord -0x123d4f(%rip){1to16},%zmm9,%zmm10        # 5c954 <perfetto::base::(anonymous namespace)::kHexmap+0x2f4>
  1806a3:	vmovdqa64 %zmm15,0x700(%rsp)
  1806ab:	vprold $0x10,%zmm11,%zmm11
  1806b2:	vpaddd -0x123d2c(%rip){1to16},%zmm11,%zmm12        # 5c990 <perfetto::base::(anonymous namespace)::kHexmap+0x330>
  1806bc:	vpxord -0x123e36(%rip){1to16},%zmm12,%zmm13        # 5c890 <perfetto::base::(anonymous namespace)::kHexmap+0x230>
  1806c6:	vprold $0x14,%zmm10,%zmm10
  1806cd:	vpaddd %zmm10,%zmm7,%zmm7
  1806d3:	vpxord %zmm8,%zmm7,%zmm8
  1806d9:	vprold $0x18,%zmm8,%zmm16
  1806e0:	vpaddd %zmm9,%zmm16,%zmm8
  1806e6:	vmovdqa64 %zmm16,0x6c0(%rsp)
  1806ee:	vprold $0x14,%zmm13,%zmm13
  1806f5:	vpxord %zmm10,%zmm8,%zmm9
  1806fb:	vpaddd %zmm30,%zmm17,%zmm10
  180701:	vprold $0x19,%zmm9,%zmm9
  180708:	vpaddd %zmm13,%zmm10,%zmm10
  18070e:	vpxord %zmm11,%zmm10,%zmm11
  180714:	vpaddd %zmm10,%zmm9,%zmm10
  18071a:	vprold $0x18,%zmm11,%zmm11
  180721:	vpaddd %zmm12,%zmm11,%zmm17
  180727:	vpxord %zmm13,%zmm17,%zmm12
  18072d:	vmovdqa64 %zmm17,0x680(%rsp)
  180735:	vprold $0x19,%zmm12,%zmm12
  18073c:	vpaddd %zmm3,%zmm12,%zmm3
  180742:	vpxord %zmm4,%zmm3,%zmm4
  180748:	vprold $0x10,%zmm4,%zmm4
  18074f:	vpaddd %zmm8,%zmm4,%zmm8
  180755:	vpxord %zmm12,%zmm8,%zmm12
  18075b:	vprold $0x14,%zmm12,%zmm12
  180762:	vpaddd %zmm3,%zmm12,%zmm13
  180768:	vpxord %zmm4,%zmm13,%zmm3
  18076e:	vmovdqa64 %zmm13,0x640(%rsp)
  180776:	vprold $0x18,%zmm3,%zmm3
  18077d:	vpaddd %zmm8,%zmm3,%zmm4
  180783:	vmovdqa64 %zmm3,%zmm8
  180789:	vmovdqa64 %zmm8,0x600(%rsp)
  180791:	vpxord %zmm12,%zmm4,%zmm3
  180797:	vmovdqa64 %zmm4,%zmm12
  18079d:	vpxord %zmm5,%zmm10,%zmm4
  1807a3:	vmovdqa64 %zmm12,0xb00(%rsp)
  1807ab:	vprold $0x10,%zmm4,%zmm4
  1807b2:	vprold $0x19,%zmm3,%zmm18
  1807b9:	vpaddd %zmm2,%zmm4,%zmm2
  1807bf:	vmovdqa64 %zmm18,0xac0(%rsp)
  1807c7:	vpxord %zmm9,%zmm2,%zmm5
  1807cd:	vprold $0x14,%zmm5,%zmm5
  1807d4:	vpaddd %zmm10,%zmm5,%zmm9
  1807da:	vpxord %zmm4,%zmm9,%zmm3
  1807e0:	vmovdqa64 %zmm9,0xa80(%rsp)
  1807e8:	vprold $0x18,%zmm3,%zmm10
  1807ef:	vpaddd %zmm2,%zmm10,%zmm19
  1807f5:	vmovdqa64 %zmm10,0xa40(%rsp)
  1807fd:	vpxord %zmm5,%zmm19,%zmm2
  180803:	vpaddd %zmm1,%zmm7,%zmm5
  180809:	vmovdqa64 %zmm19,0xa00(%rsp)
  180811:	vpxord %zmm11,%zmm5,%zmm3
  180817:	vprold $0x19,%zmm2,%zmm20
  18081e:	vprold $0x10,%zmm3,%zmm3
  180825:	vmovdqa64 %zmm20,0x9c0(%rsp)
  18082d:	vpaddd %zmm6,%zmm3,%zmm4
  180833:	vpxord %zmm1,%zmm4,%zmm1
  180839:	vprold $0x14,%zmm1,%zmm1
  180840:	vpaddd %zmm5,%zmm1,%zmm11
  180846:	vpxord %zmm3,%zmm11,%zmm2
  18084c:	vpaddd %zmm0,%zmm15,%zmm3
  180852:	vmovdqa64 -0x12b5dc(%rip),%zmm0        # 55280 <GCC_except_table0+0xb00>
  18085c:	vmovdqa64 %zmm11,0x980(%rsp)
  180864:	vprold $0x18,%zmm2,%zmm21
  18086b:	vpxor  %xmm2,%xmm2,%xmm2
  18086f:	vmovdqa64 %zmm3,0x880(%rsp)
  180877:	vpaddd %zmm4,%zmm21,%zmm22
  18087d:	vmovdqa64 %zmm21,0x940(%rsp)
  180885:	vpxord %zmm1,%zmm22,%zmm1
  18088b:	vmovdqa64 %zmm22,0x900(%rsp)
  180893:	vprold $0x19,%zmm1,%zmm23
  18089a:	vpbroadcastd %eax,%zmm1
  1808a0:	vmovdqa64 %zmm1,0x1c0(%rsp)
  1808a8:	vpaddd %zmm25,%zmm30,%zmm1
  1808ae:	vmovdqa64 %zmm1,0x180(%rsp)
  1808b6:	vmovdqa64 %zmm23,0x8c0(%rsp)
  1808be:	jmp    181405 <bitz::utils::blake3x4::first_pow_nonce+0x1005>
  1808c3:	mov    %r12,%r14
  1808c6:	xor    %eax,%eax
  1808c8:	cmp    $0xffffffffffffffff,%rbx
  1808cc:	jmp    1825b6 <bitz::utils::blake3x4::first_pow_nonce+0x21b6>
  1808d1:	xor    %ecx,%ecx
  1808d3:	mov    $0x20,%eax
  1808d8:	sub    0x3c(%rsp),%eax
  1808dc:	mov    $0xffffffff,%edx
  1808e1:	cmovb  %ecx,%eax
  1808e4:	cmp    $0x20,%eax
  1808e7:	shlx   %eax,%edx,%eax
  1808ec:	mov    %rbx,%rdx
  1808ef:	bswap  %eax
  1808f1:	cmovae %ecx,%eax
  1808f4:	sub    0x40(%rsp),%rdx
  1808f9:	cmovae %rdx,%rcx
  1808fd:	cmp    $0x10,%rcx
  180901:	jb     1810b8 <bitz::utils::blake3x4::first_pow_nonce+0xcb8>
  180907:	vpbroadcastd %eax,%zmm0
  18090d:	mov    %r14,%rax
  180910:	shl    $0x6,%rax
  180914:	lea    0x8(,%r14,4),%ecx
  18091c:	mov    %r14,%r15
  18091f:	mov    %r15d,%edi
  180922:	and    $0x7,%edi
  180925:	and    $0x8,%r15d
  180929:	lea    -0x113cc8(%rip),%r8        # 6cc68 <perfetto::base::(anonymous namespace)::kHexmap+0x10608>
  180930:	lea    0x200(%rsp,%rax,1),%rax
  180938:	vmovdqa64 %zmm0,0x140(%rsp)
  180940:	mov    %rdi,0x100(%rsp)
  180948:	mov    %r15,0xc0(%rsp)
  180950:	vpbroadcastd %ecx,%zmm0
  180956:	vmovdqa64 %zmm0,0x180(%rsp)
  18095e:	vpxor  %xmm0,%xmm0,%xmm0
  180962:	mov    %rax,0x1c0(%rsp)
  18096a:	jmp    1809af <bitz::utils::blake3x4::first_pow_nonce+0x5af>
  18096c:	nopl   0x0(%rax)
  180970:	mov    0x40(%rsp),%rcx
  180975:	mov    0x70(%rsp),%rbx
  18097a:	mov    0x100(%rsp),%rdi
  180982:	lea    -0x113d21(%rip),%r8        # 6cc68 <perfetto::base::(anonymous namespace)::kHexmap+0x10608>
  180989:	vpxor  %xmm0,%xmm0,%xmm0
  18098d:	add    $0x10,%rcx
  180991:	mov    %rbx,%rax
  180994:	mov    %rcx,0x40(%rsp)
  180999:	sub    %rcx,%rax
  18099c:	mov    $0x0,%ecx
  1809a1:	cmovb  %rcx,%rax
  1809a5:	cmp    $0xf,%rax
  1809a9:	jbe    1810b8 <bitz::utils::blake3x4::first_pow_nonce+0xcb8>
  1809af:	vmovdqa64 %zmm0,0x5c0(%rsp)
  1809b7:	vmovdqa64 %zmm0,0x580(%rsp)
  1809bf:	vmovdqa64 %zmm0,0x540(%rsp)
  1809c7:	vmovdqa64 %zmm0,0x500(%rsp)
  1809cf:	vmovdqa64 %zmm0,0x4c0(%rsp)
  1809d7:	vmovdqa64 %zmm0,0x480(%rsp)
  1809df:	vmovdqa64 %zmm0,0x440(%rsp)
  1809e7:	vmovdqa64 %zmm0,0x400(%rsp)
  1809ef:	vmovdqa64 %zmm0,0x3c0(%rsp)
  1809f7:	vmovdqa64 %zmm0,0x380(%rsp)
  1809ff:	vmovdqa64 %zmm0,0x340(%rsp)
  180a07:	vmovdqa64 %zmm0,0x300(%rsp)
  180a0f:	vmovdqa64 %zmm0,0x2c0(%rsp)
  180a17:	vmovdqa64 %zmm0,0x280(%rsp)
  180a1f:	vmovdqa64 %zmm0,0x240(%rsp)
  180a27:	vmovdqa64 %zmm0,0x200(%rsp)
  180a2f:	test   %r13,%r13
  180a32:	je     180b29 <bitz::utils::blake3x4::first_pow_nonce+0x729>
  180a38:	cmp    $0x20,%r13
  180a3c:	jae    180a45 <bitz::utils::blake3x4::first_pow_nonce+0x645>
  180a3e:	xor    %eax,%eax
  180a40:	jmp    180af0 <bitz::utils::blake3x4::first_pow_nonce+0x6f0>
  180a45:	mov    0xc0(%rsp),%rdx
  180a4d:	lea    0x3c0(%rsp),%rcx
  180a55:	xor    %eax,%eax
  180a57:	nopw   0x0(%rax,%rax,1)
  180a60:	vpbroadcastd 0x80(%rsp,%rax,4),%zmm0
  180a68:	vbroadcastss 0x84(%rsp,%rax,4),%zmm1
  180a70:	vbroadcastss 0x88(%rsp,%rax,4),%zmm2
  180a78:	vmovdqa64 %zmm0,-0x1c0(%rcx)
  180a7f:	vmovaps %zmm1,-0x180(%rcx)
  180a86:	vbroadcastss 0x8c(%rsp,%rax,4),%zmm1
  180a8e:	vmovaps %zmm2,-0x140(%rcx)
  180a95:	vbroadcastss 0x90(%rsp,%rax,4),%zmm2
  180a9d:	vmovaps %zmm1,-0x100(%rcx)
  180aa4:	vbroadcastss 0x94(%rsp,%rax,4),%zmm1
  180aac:	vmovaps %zmm2,-0xc0(%rcx)
  180ab3:	vpbroadcastd 0x98(%rsp,%rax,4),%zmm2
  180abb:	vmovaps %zmm1,-0x80(%rcx)
  180ac2:	vpbroadcastd 0x9c(%rsp,%rax,4),%zmm1
  180aca:	vmovdqa64 %zmm2,-0x40(%rcx)
  180ad1:	add    $0x8,%rax
  180ad5:	vmovdqa64 %zmm1,(%rcx)
  180adb:	add    $0x200,%rcx
  180ae2:	cmp    %rax,%rdx
  180ae5:	jne    180a60 <bitz::utils::blake3x4::first_pow_nonce+0x660>
  180aeb:	test   %rdi,%rdi
  180aee:	je     180b29 <bitz::utils::blake3x4::first_pow_nonce+0x729>
  180af0:	lea    0x80(%rsp,%rax,4),%rcx
  180af8:	shl    $0x6,%rax
  180afc:	xor    %edx,%edx
  180afe:	lea    0x200(%rsp,%rax,1),%rax
  180b06:	cs nopw 0x0(%rax,%rax,1)
  180b10:	vpbroadcastd (%rcx,%rdx,4),%zmm0
  180b17:	inc    %rdx
  180b1a:	vmovdqa64 %zmm0,(%rax)
  180b20:	add    $0x40,%rax
  180b24:	cmp    %rdx,%rdi
  180b27:	jne    180b10 <bitz::utils::blake3x4::first_pow_nonce+0x710>
  180b29:	mov    0x40(%rsp),%rax
  180b2e:	mov    0x1c0(%rsp),%rcx
  180b36:	vpbroadcastd -0x1241b0(%rip),%zmm13        # 5c990 <perfetto::base::(anonymous namespace)::kHexmap+0x330>
  180b40:	vpbroadcastd -0x1241be(%rip),%zmm14        # 5c98c <perfetto::base::(anonymous namespace)::kHexmap+0x32c>
  180b4a:	vpbroadcastd -0x124358(%rip),%zmm15        # 5c7fc <perfetto::base::(anonymous namespace)::kHexmap+0x19c>
  180b54:	vpbroadcastd -0x12422a(%rip),%zmm11        # 5c934 <perfetto::base::(anonymous namespace)::kHexmap+0x2d4>
  180b5e:	vpbroadcastd -0x1242d8(%rip),%zmm4        # 5c890 <perfetto::base::(anonymous namespace)::kHexmap+0x230>
  180b68:	vpbroadcastd -0x12428e(%rip),%zmm5        # 5c8e4 <perfetto::base::(anonymous namespace)::kHexmap+0x284>
  180b72:	vpbroadcastd -0x12421c(%rip),%zmm12        # 5c960 <perfetto::base::(anonymous namespace)::kHexmap+0x300>
  180b7c:	vmovdqa64 0x180(%rsp),%zmm6
  180b84:	mov    $0x78,%edx
  180b89:	vpxor  %xmm7,%xmm7,%xmm7
  180b8d:	vpxor  %xmm8,%xmm8,%xmm8
  180b92:	vpbroadcastd %eax,%zmm0
  180b98:	vpaddd -0x12b922(%rip),%zmm0,%zmm1        # 55280 <GCC_except_table0+0xb00>
  180ba2:	shr    $0x20,%rax
  180ba6:	vpbroadcastd %eax,%zmm2
  180bac:	vmovdqa64 %zmm13,%zmm3
  180bb2:	vmovdqa64 %zmm14,%zmm10
  180bb8:	vmovdqa64 %zmm15,%zmm9
  180bbe:	vpcmpltud %zmm0,%zmm1,%k0
  180bc5:	vmovdqa64 %zmm1,(%rcx)
  180bcb:	vpbroadcastd -0x124269(%rip),%zmm1        # 5c96c <perfetto::base::(anonymous namespace)::kHexmap+0x30c>
  180bd5:	vpmovm2d %k0,%zmm0
  180bdb:	vpsubd %zmm0,%zmm2,%zmm0
  180be1:	vpbroadcastd -0x124297(%rip),%zmm2        # 5c954 <perfetto::base::(anonymous namespace)::kHexmap+0x2f4>
  180beb:	vmovdqa64 %zmm0,0x40(%rcx)
  180bf2:	vmovdqa64 %zmm11,%zmm0
  180bf8:	nopl   0x0(%rax,%rax,1)
  180c00:	mov    -0x78(%rdx,%r8,1),%rdi
  180c05:	cmp    $0xf,%rdi
  180c09:	ja     18269e <bitz::utils::blake3x4::first_pow_nonce+0x229e>
  180c0f:	mov    -0x70(%rdx,%r8,1),%rcx
  180c14:	cmp    $0x10,%rcx
  180c18:	jae    182757 <bitz::utils::blake3x4::first_pow_nonce+0x2357>
  180c1e:	mov    -0x68(%rdx,%r8,1),%rax
  180c23:	cmp    $0x10,%rax
  180c27:	jae    182687 <bitz::utils::blake3x4::first_pow_nonce+0x2287>
  180c2d:	mov    -0x60(%rdx,%r8,1),%r14
  180c32:	cmp    $0x10,%r14
  180c36:	jae    182740 <bitz::utils::blake3x4::first_pow_nonce+0x2340>
  180c3c:	mov    -0x58(%rdx,%r8,1),%rbx
  180c41:	cmp    $0x10,%rbx
  180c45:	jae    182670 <bitz::utils::blake3x4::first_pow_nonce+0x2270>
  180c4b:	mov    -0x50(%rdx,%r8,1),%r12
  180c50:	cmp    $0x10,%r12
  180c54:	jae    182729 <bitz::utils::blake3x4::first_pow_nonce+0x2329>
  180c5a:	mov    -0x48(%rdx,%r8,1),%r15
  180c5f:	cmp    $0x10,%r15
  180c63:	jae    182659 <bitz::utils::blake3x4::first_pow_nonce+0x2259>
  180c69:	mov    -0x40(%rdx,%r8,1),%r13
  180c6e:	cmp    $0x10,%r13
  180c72:	jae    182712 <bitz::utils::blake3x4::first_pow_nonce+0x2312>
  180c78:	mov    -0x38(%rdx,%r8,1),%r11
  180c7d:	cmp    $0x10,%r11
  180c81:	jae    182642 <bitz::utils::blake3x4::first_pow_nonce+0x2242>
  180c87:	mov    -0x30(%rdx,%r8,1),%r10
  180c8c:	cmp    $0x10,%r10
  180c90:	jae    1826fb <bitz::utils::blake3x4::first_pow_nonce+0x22fb>
  180c96:	mov    -0x28(%rdx,%r8,1),%r9
  180c9b:	cmp    $0x10,%r9
  180c9f:	jae    18262b <bitz::utils::blake3x4::first_pow_nonce+0x222b>
  180ca5:	mov    -0x20(%rdx,%r8,1),%rsi
  180caa:	mov    %rsi,0x68(%rsp)
  180caf:	cmp    $0x10,%rsi
  180cb3:	jae    1826e2 <bitz::utils::blake3x4::first_pow_nonce+0x22e2>
  180cb9:	mov    -0x18(%rdx,%r8,1),%rsi
  180cbe:	mov    %rsi,0x60(%rsp)
  180cc3:	cmp    $0x10,%rsi
  180cc7:	jae    182612 <bitz::utils::blake3x4::first_pow_nonce+0x2212>
  180ccd:	mov    -0x10(%rdx,%r8,1),%rsi
  180cd2:	mov    %rsi,0x58(%rsp)
  180cd7:	cmp    $0x10,%rsi
  180cdb:	jae    1826c9 <bitz::utils::blake3x4::first_pow_nonce+0x22c9>
  180ce1:	mov    -0x8(%rdx,%r8,1),%rsi
  180ce6:	mov    %rsi,0x50(%rsp)
  180ceb:	cmp    $0x10,%rsi
  180cef:	jae    1825f9 <bitz::utils::blake3x4::first_pow_nonce+0x21f9>
  180cf5:	mov    (%rdx,%r8,1),%r8
  180cf9:	cmp    $0x10,%r8
  180cfd:	jae    1826b2 <bitz::utils::blake3x4::first_pow_nonce+0x22b2>
  180d03:	shl    $0x6,%rax
  180d07:	shl    $0x6,%rbx
  180d0b:	vpaddd %zmm5,%zmm14,%zmm14
  180d11:	vpaddd %zmm4,%zmm13,%zmm13
  180d17:	shl    $0x6,%rdi
  180d1b:	shl    $0x6,%r14
  180d1f:	shl    $0x6,%r12
  180d23:	shl    $0x6,%r15
  180d27:	vpaddd %zmm12,%zmm15,%zmm15
  180d2d:	vpaddd %zmm2,%zmm11,%zmm11
  180d33:	shl    $0x6,%r13
  180d37:	shl    $0x6,%rcx
  180d3b:	shl    $0x6,%r9
  180d3f:	shl    $0x6,%r11
  180d43:	shl    $0x6,%r10
  180d47:	shl    $0x6,%r8
  180d4b:	sub    $0xffffffffffffff80,%rdx
  180d4f:	vpaddd 0x200(%rsp,%rax,1),%zmm14,%zmm19
  180d57:	vpaddd 0x200(%rsp,%rbx,1),%zmm13,%zmm21
  180d5f:	vpaddd 0x200(%rsp,%rdi,1),%zmm15,%zmm16
  180d67:	vpaddd 0x200(%rsp,%r15,1),%zmm11,%zmm15
  180d6f:	mov    0x68(%rsp),%rax
  180d74:	vpaddd 0x200(%rsp,%r14,1),%zmm19,%zmm20
  180d7c:	vpaddd 0x200(%rsp,%r12,1),%zmm21,%zmm22
  180d84:	vpaddd 0x200(%rsp,%r13,1),%zmm15,%zmm17
  180d8c:	vpaddd 0x200(%rsp,%rcx,1),%zmm16,%zmm18
  180d94:	mov    0x78(%rsp),%r13
  180d99:	shl    $0x6,%rax
  180d9d:	vpxord %zmm7,%zmm19,%zmm7
  180da3:	vpxord %zmm6,%zmm21,%zmm6
  180da9:	vpxord %zmm1,%zmm15,%zmm1
  180daf:	vpxord %zmm8,%zmm16,%zmm8
  180db5:	vprold $0x10,%zmm7,%zmm7
  180dbc:	vprold $0x10,%zmm6,%zmm6
  180dc3:	vprold $0x10,%zmm1,%zmm1
  180dca:	vprold $0x10,%zmm8,%zmm11
  180dd1:	vpaddd %zmm10,%zmm7,%zmm10
  180dd7:	vpaddd %zmm3,%zmm6,%zmm3
  180ddd:	vpaddd %zmm0,%zmm1,%zmm0
  180de3:	vpaddd %zmm9,%zmm11,%zmm8
  180de9:	vpxord %zmm5,%zmm10,%zmm5
  180def:	vpxord %zmm4,%zmm3,%zmm4
  180df5:	vpxord %zmm2,%zmm0,%zmm2
  180dfb:	vpxord %zmm12,%zmm8,%zmm9
  180e01:	vprold $0x14,%zmm5,%zmm5
  180e08:	vprold $0x14,%zmm4,%zmm4
  180e0f:	vprold $0x14,%zmm2,%zmm2
  180e16:	vprold $0x14,%zmm9,%zmm9
  180e1d:	vpaddd %zmm5,%zmm20,%zmm16
  180e23:	vpaddd %zmm4,%zmm22,%zmm20
  180e29:	vpaddd %zmm2,%zmm17,%zmm15
  180e2f:	vpaddd %zmm9,%zmm18,%zmm14
  180e35:	vpxord %zmm6,%zmm20,%zmm6
  180e3b:	vpxord %zmm7,%zmm16,%zmm7
  180e41:	vpxord %zmm1,%zmm15,%zmm1
  180e47:	vpxord %zmm11,%zmm14,%zmm11
  180e4d:	vprold $0x18,%zmm6,%zmm6
  180e54:	vprold $0x18,%zmm1,%zmm1
  180e5b:	vprold $0x18,%zmm7,%zmm7
  180e62:	vprold $0x18,%zmm11,%zmm11
  180e69:	vpaddd %zmm3,%zmm6,%zmm3
  180e6f:	vpaddd %zmm0,%zmm1,%zmm0
  180e75:	vpaddd %zmm10,%zmm7,%zmm10
  180e7b:	vpxord %zmm4,%zmm3,%zmm4
  180e81:	vpxord %zmm2,%zmm0,%zmm2
  180e87:	vpxord %zmm5,%zmm10,%zmm5
  180e8d:	vprold $0x19,%zmm4,%zmm4
  180e94:	vprold $0x19,%zmm2,%zmm2
  180e9b:	vprold $0x19,%zmm5,%zmm5
  180ea2:	vpaddd %zmm16,%zmm4,%zmm16
  180ea8:	vpaddd 0x200(%rsp,%r9,1),%zmm16,%zmm19
  180eb0:	vpaddd %zmm20,%zmm2,%zmm17
  180eb6:	vpaddd %zmm8,%zmm11,%zmm20
  180ebc:	vpaddd %zmm14,%zmm5,%zmm12
  180ec2:	vpaddd 0x200(%rsp,%r11,1),%zmm12,%zmm13
  180eca:	vpaddd 0x200(%rsp,%rax,1),%zmm19,%zmm16
  180ed2:	mov    0x60(%rsp),%rax
  180ed7:	vpxord %zmm9,%zmm20,%zmm8
  180edd:	vpaddd 0x200(%rsp,%r10,1),%zmm13,%zmm12
  180ee5:	vprold $0x19,%zmm8,%zmm21
  180eec:	vpaddd %zmm21,%zmm15,%zmm8
  180ef2:	shl    $0x6,%rax
  180ef6:	vpaddd 0x200(%rsp,%rax,1),%zmm17,%zmm17
  180efe:	mov    0x58(%rsp),%rax
  180f03:	vpxord %zmm1,%zmm13,%zmm1
  180f09:	vpxord %zmm11,%zmm19,%zmm9
  180f0f:	vprold $0x10,%zmm1,%zmm1
  180f16:	vpaddd %zmm3,%zmm1,%zmm3
  180f1c:	shl    $0x6,%rax
  180f20:	vpxord %zmm5,%zmm3,%zmm5
  180f26:	vpaddd 0x200(%rsp,%rax,1),%zmm17,%zmm18
  180f2e:	mov    0x50(%rsp),%rax
  180f33:	vprold $0x14,%zmm5,%zmm5
  180f3a:	vpxord %zmm7,%zmm17,%zmm7
  180f40:	vpaddd %zmm5,%zmm12,%zmm15
  180f46:	vprold $0x10,%zmm7,%zmm7
  180f4d:	vpxord %zmm1,%zmm15,%zmm1
  180f53:	vprold $0x18,%zmm1,%zmm1
  180f5a:	shl    $0x6,%rax
  180f5e:	vpaddd 0x200(%rsp,%rax,1),%zmm8,%zmm8
  180f66:	vpaddd %zmm3,%zmm1,%zmm3
  180f6c:	vpaddd 0x200(%rsp,%r8,1),%zmm8,%zmm22
  180f74:	vpxord %zmm5,%zmm3,%zmm5
  180f7a:	lea    -0x114319(%rip),%r8        # 6cc68 <perfetto::base::(anonymous namespace)::kHexmap+0x10608>
  180f81:	vprold $0x19,%zmm5,%zmm5
  180f88:	vpxord %zmm6,%zmm8,%zmm6
  180f8e:	vprold $0x10,%zmm9,%zmm8
  180f95:	vpaddd %zmm20,%zmm7,%zmm9
  180f9b:	vprold $0x10,%zmm6,%zmm6
  180fa2:	vpxord %zmm2,%zmm9,%zmm2
  180fa8:	vpaddd %zmm0,%zmm8,%zmm0
  180fae:	vprold $0x14,%zmm2,%zmm2
  180fb5:	vpaddd %zmm10,%zmm6,%zmm10
  180fbb:	vpxord %zmm4,%zmm0,%zmm4
  180fc1:	vpxord %zmm21,%zmm10,%zmm11
  180fc7:	vprold $0x14,%zmm4,%zmm4
  180fce:	vpaddd %zmm2,%zmm18,%zmm13
  180fd4:	vprold $0x14,%zmm11,%zmm12
  180fdb:	vpxord %zmm7,%zmm13,%zmm7
  180fe1:	vpaddd %zmm4,%zmm16,%zmm14
  180fe7:	vprold $0x18,%zmm7,%zmm7
  180fee:	vpaddd %zmm12,%zmm22,%zmm11
  180ff4:	vpxord %zmm8,%zmm14,%zmm8
  180ffa:	vpxord %zmm6,%zmm11,%zmm6
  181000:	vprold $0x18,%zmm8,%zmm8
  181007:	vpaddd %zmm9,%zmm7,%zmm9
  18100d:	vprold $0x18,%zmm6,%zmm6
  181014:	vpxord %zmm2,%zmm9,%zmm2
  18101a:	vpaddd %zmm0,%zmm8,%zmm0
  181020:	vprold $0x19,%zmm2,%zmm2
  181027:	vpaddd %zmm10,%zmm6,%zmm10
  18102d:	vpxord %zmm4,%zmm0,%zmm4
  181033:	vpxord %zmm12,%zmm10,%zmm12
  181039:	vprold $0x19,%zmm4,%zmm4
  181040:	vprold $0x19,%zmm12,%zmm12
  181047:	cmp    $0x3f8,%rdx
  18104e:	jne    180c00 <bitz::utils::blake3x4::first_pow_nonce+0x800>
  181054:	vpxord %zmm15,%zmm9,%zmm0
  18105a:	vptestnmd 0x140(%rsp),%zmm0,%k0
  181062:	kortestw %k0,%k0
  181066:	je     180970 <bitz::utils::blake3x4::first_pow_nonce+0x570>
  18106c:	cmpl   $0x21,0x3c(%rsp)
  181071:	kmovd  %k0,%ebx
  181075:	jb     1825ce <bitz::utils::blake3x4::first_pow_nonce+0x21ce>
  18107b:	nopl   0x0(%rax,%rax,1)
  181080:	tzcnt  %ebx,%r14d
  181085:	add    0x40(%rsp),%r14
  18108a:	mov    0x48(%rsp),%rdi
  18108f:	mov    0x3c(%rsp),%ecx
  181093:	mov    %r13,%rsi
  181096:	mov    %r14,%rdx
  181099:	vzeroupper
  18109c:	call   17fd30 <bitz::utils::blake3x4::pow_ok>
  1810a1:	test   %al,%al
  1810a3:	jne    182504 <bitz::utils::blake3x4::first_pow_nonce+0x2104>
  1810a9:	lea    -0x1(%rbx),%eax
  1810ac:	and    %bx,%ax
  1810af:	mov    %eax,%ebx
  1810b1:	jne    181080 <bitz::utils::blake3x4::first_pow_nonce+0xc80>
  1810b3:	jmp    180970 <bitz::utils::blake3x4::first_pow_nonce+0x570>
  1810b8:	mov    0x40(%rsp),%r15
  1810bd:	cmp    %r15,%rbx
  1810c0:	cmovbe %r15,%rbx
  1810c4:	dec    %r15
  1810c7:	mov    %rbx,%r14
  1810ca:	neg    %r14
  1810cd:	mov    %rbx,%r12
  1810d0:	lea    (%r14,%r15,1),%rbx
  1810d4:	cmp    $0xffffffffffffffff,%rbx
  1810d8:	je     1825aa <bitz::utils::blake3x4::first_pow_nonce+0x21aa>
  1810de:	mov    0x48(%rsp),%rdi
  1810e3:	mov    0x3c(%rsp),%ecx
  1810e7:	inc    %r15
  1810ea:	mov    %r13,%rsi
  1810ed:	mov    %r15,%rdx
  1810f0:	vzeroupper
  1810f3:	call   17fd30 <bitz::utils::blake3x4::pow_ok>
  1810f8:	test   %al,%al
  1810fa:	je     1810d0 <bitz::utils::blake3x4::first_pow_nonce+0xcd0>
  1810fc:	jmp    1825ad <bitz::utils::blake3x4::first_pow_nonce+0x21ad>
  181101:	xor    %ecx,%ecx
  181103:	mov    $0x20,%eax
  181108:	sub    0x3c(%rsp),%eax
  18110c:	mov    $0xffffffff,%edx
  181111:	cmovb  %ecx,%eax
  181114:	cmp    $0x20,%eax
  181117:	shlx   %eax,%edx,%eax
  18111c:	mov    %rbx,%rdx
  18111f:	bswap  %eax
  181121:	cmovae %ecx,%eax
  181124:	sub    0x40(%rsp),%rdx
  181129:	cmovae %rdx,%rcx
  18112d:	cmp    $0x10,%rcx
  181131:	jb     18255a <bitz::utils::blake3x4::first_pow_nonce+0x215a>
  181137:	vpbroadcastd 0x80(%rsp),%zmm25
  18113f:	vpbroadcastd 0x84(%rsp),%zmm26
  181147:	vpbroadcastd 0x88(%rsp),%zmm27
  18114f:	vpbroadcastd 0x8c(%rsp),%zmm28
  181157:	vpbroadcastd -0x1247d1(%rip),%zmm8        # 5c990 <perfetto::base::(anonymous namespace)::kHexmap+0x330>
  181161:	vpbroadcastd -0x1248db(%rip),%zmm11        # 5c890 <perfetto::base::(anonymous namespace)::kHexmap+0x230>
  18116b:	vpbroadcastd -0x1247bd(%rip),%zmm12        # 5c9b8 <perfetto::base::(anonymous namespace)::kHexmap+0x358>
  181175:	vpbroadcastd -0x124843(%rip),%zmm14        # 5c93c <perfetto::base::(anonymous namespace)::kHexmap+0x2dc>
  18117f:	mov    0x40(%rsp),%r15
  181184:	lea    0x380(%rsp),%r12
  18118c:	lea    -0x1144ab(%rip),%rdx        # 6cce8 <perfetto::base::(anonymous namespace)::kHexmap+0x10688>
  181193:	vpaddd -0x1248a1(%rip){1to16},%zmm25,%zmm0        # 5c8fc <perfetto::base::(anonymous namespace)::kHexmap+0x29c>
  18119d:	vpaddd -0x12481f(%rip){1to16},%zmm27,%zmm5        # 5c988 <perfetto::base::(anonymous namespace)::kHexmap+0x328>
  1811a7:	vmovdqa64 %zmm25,0x180(%rsp)
  1811af:	vmovdqa64 %zmm26,0x140(%rsp)
  1811b7:	vmovdqa64 %zmm27,0x100(%rsp)
  1811bf:	vmovdqa64 %zmm28,0xc0(%rsp)
  1811c7:	vprold $0x10,%zmm0,%zmm1
  1811ce:	vpaddd -0x1249dc(%rip){1to16},%zmm1,%zmm2        # 5c7fc <perfetto::base::(anonymous namespace)::kHexmap+0x19c>
  1811d8:	vpaddd %zmm26,%zmm0,%zmm0
  1811de:	vpxord -0x124888(%rip){1to16},%zmm2,%zmm3        # 5c960 <perfetto::base::(anonymous namespace)::kHexmap+0x300>
  1811e8:	vprold $0x14,%zmm3,%zmm3
  1811ef:	vpaddd %zmm3,%zmm0,%zmm0
  1811f5:	vpxord %zmm1,%zmm0,%zmm1
  1811fb:	vprold $0x18,%zmm1,%zmm29
  181202:	vpaddd %zmm2,%zmm29,%zmm30
  181208:	vprold $0x10,%zmm5,%zmm2
  18120f:	vmovdqa64 %zmm29,0x840(%rsp)
  181217:	vpxord %zmm3,%zmm30,%zmm1
  18121d:	vpaddd -0x12489b(%rip){1to16},%zmm2,%zmm3        # 5c98c <perfetto::base::(anonymous namespace)::kHexmap+0x32c>
  181227:	vmovdqa64 %zmm30,0x800(%rsp)
  18122f:	vpxord -0x124955(%rip){1to16},%zmm3,%zmm4        # 5c8e4 <perfetto::base::(anonymous namespace)::kHexmap+0x284>
  181239:	vprold $0x19,%zmm1,%zmm31
  181240:	vpaddd %zmm28,%zmm5,%zmm1
  181246:	vpaddd -0x124918(%rip){1to16},%zmm31,%zmm18        # 5c938 <perfetto::base::(anonymous namespace)::kHexmap+0x2d8>
  181250:	vpbroadcastd -0x1248fe(%rip),%zmm5        # 5c95c <perfetto::base::(anonymous namespace)::kHexmap+0x2fc>
  18125a:	vmovdqa64 %zmm31,0x7c0(%rsp)
  181262:	vprold $0x14,%zmm4,%zmm4
  181269:	vmovdqa64 %zmm18,0x640(%rsp)
  181271:	vpaddd %zmm4,%zmm1,%zmm6
  181277:	vpxord %zmm2,%zmm6,%zmm1
  18127d:	vmovdqa64 %zmm6,0x780(%rsp)
  181285:	vprold $0x18,%zmm1,%zmm13
  18128c:	vpaddd %zmm3,%zmm13,%zmm17
  181292:	vpxor  %xmm3,%xmm3,%xmm3
  181296:	vmovdqa64 %zmm13,0x740(%rsp)
  18129e:	vpxord %zmm4,%zmm17,%zmm1
  1812a4:	vmovdqa64 -0x12bfee(%rip),%zmm4        # 552c0 <GCC_except_table0+0xb40>
  1812ae:	vmovdqa64 %zmm17,0x700(%rsp)
  1812b6:	vprold $0x19,%zmm1,%zmm7
  1812bd:	vpbroadcastd %eax,%zmm1
  1812c3:	vmovdqa64 %zmm1,0x1c0(%rsp)
  1812cb:	vpaddd %zmm0,%zmm7,%zmm9
  1812d1:	vpxord -0x124aa3(%rip){1to16},%zmm9,%zmm0        # 5c838 <perfetto::base::(anonymous namespace)::kHexmap+0x1d8>
  1812db:	vmovdqa64 %zmm7,0x6c0(%rsp)
  1812e3:	vmovdqa64 %zmm9,0x680(%rsp)
  1812eb:	vprold $0x10,%zmm0,%zmm10
  1812f2:	vmovdqa64 -0x12c07c(%rip),%zmm0        # 55280 <GCC_except_table0+0xb00>
  1812fc:	vmovdqa64 %zmm10,0x600(%rsp)
  181304:	jmp    181ca0 <bitz::utils::blake3x4::first_pow_nonce+0x18a0>
  181309:	mov    0x40(%rsp),%r15
  18130e:	vmovdqa64 0x140(%rsp),%zmm25
  181316:	vmovdqa64 0x100(%rsp),%zmm26
  18131e:	vmovdqa64 0xc0(%rsp),%zmm27
  181326:	vmovdqa64 0x840(%rsp),%zmm28
  18132e:	vmovdqa64 0x800(%rsp),%zmm29
  181336:	vmovdqa64 0x7c0(%rsp),%zmm30
  18133e:	vmovdqa64 0x780(%rsp),%zmm31
  181346:	vmovdqa64 0x740(%rsp),%zmm14
  18134e:	vmovdqa64 0x700(%rsp),%zmm15
  181356:	vmovdqa64 0x6c0(%rsp),%zmm16
  18135e:	vmovdqa64 0x680(%rsp),%zmm17
  181366:	vmovdqa64 0x640(%rsp),%zmm13
  18136e:	vmovdqa64 0x600(%rsp),%zmm8
  181376:	vmovdqa64 0xb00(%rsp),%zmm12
  18137e:	vmovdqa64 0xac0(%rsp),%zmm18
  181386:	vmovdqa64 0xa80(%rsp),%zmm9
  18138e:	vmovdqa64 0xa40(%rsp),%zmm10
  181396:	vmovdqa64 0xa00(%rsp),%zmm19
  18139e:	vmovdqa64 0x9c0(%rsp),%zmm20
  1813a6:	vmovdqa64 0x980(%rsp),%zmm11
  1813ae:	vmovdqa64 0x940(%rsp),%zmm21
  1813b6:	vmovdqa64 0x900(%rsp),%zmm22
  1813be:	vmovdqa64 0x8c0(%rsp),%zmm23
  1813c6:	vmovdqa64 0x880(%rsp),%zmm3
  1813ce:	vmovdqa64 -0x12c158(%rip),%zmm0        # 55280 <GCC_except_table0+0xb00>
  1813d8:	mov    0x70(%rsp),%rbx
  1813dd:	mov    $0x0,%ecx
  1813e2:	lea    -0x114701(%rip),%rdx        # 6cce8 <perfetto::base::(anonymous namespace)::kHexmap+0x10688>
  1813e9:	vpxor  %xmm2,%xmm2,%xmm2
  1813ed:	add    $0x10,%r15
  1813f1:	mov    %rbx,%rax
  1813f4:	sub    %r15,%rax
  1813f7:	cmovb  %rcx,%rax
  1813fb:	cmp    $0xf,%rax
  1813ff:	jbe    182513 <bitz::utils::blake3x4::first_pow_nonce+0x2113>
  181405:	vpbroadcastd %r15d,%zmm1
  18140b:	mov    %r15,0x40(%rsp)
  181410:	shr    $0x20,%r15
  181414:	vmovdqa64 %zmm25,0x200(%rsp)
  18141c:	vmovdqa64 %zmm26,0x240(%rsp)
  181424:	vmovdqa64 %zmm27,0x280(%rsp)
  18142c:	vmovdqa64 %zmm28,0x2c0(%rsp)
  181434:	vmovdqa64 %zmm29,0x300(%rsp)
  18143c:	vmovdqa64 %zmm30,0x340(%rsp)
  181444:	vmovdqa64 %zmm31,0x380(%rsp)
  18144c:	vmovdqa64 %zmm14,0x3c0(%rsp)
  181454:	vmovdqa64 %zmm2,0x140(%r12)
  18145c:	vmovdqa64 %zmm2,0x100(%r12)
  181464:	vmovdqa64 %zmm2,0xc0(%r12)
  18146c:	vmovdqa64 %zmm2,0x80(%r12)
  181474:	vmovdqa64 %zmm2,0x40(%r12)
  18147c:	vmovdqa64 %zmm2,(%r12)
  181483:	vmovdqa64 %zmm19,%zmm14
  181489:	vpaddd %zmm0,%zmm1,%zmm0
  18148f:	vpaddd %zmm0,%zmm3,%zmm7
  181495:	vpcmpltud %zmm1,%zmm0,%k0
  18149c:	vpbroadcastd %r15d,%zmm1
  1814a2:	vmovdqa64 %zmm0,0x400(%rsp)
  1814aa:	vpxord %zmm16,%zmm7,%zmm3
  1814b0:	vpmovm2d %k0,%zmm2
  1814b6:	vpsubd %zmm2,%zmm1,%zmm1
  1814bc:	vmovdqa64 %zmm13,%zmm16
  1814c2:	vmovdqa64 %zmm9,%zmm13
  1814c8:	vmovdqa64 %zmm11,%zmm9
  1814ce:	vmovdqa64 %zmm22,%zmm11
  1814d4:	vprold $0x10,%zmm3,%zmm3
  1814db:	vpaddd %zmm7,%zmm1,%zmm2
  1814e1:	vmovdqa64 %zmm1,0x440(%rsp)
  1814e9:	vpaddd %zmm17,%zmm3,%zmm4
  1814ef:	vpxord %zmm15,%zmm4,%zmm5
  1814f5:	vmovdqa64 %zmm23,%zmm15
  1814fb:	vprold $0x14,%zmm5,%zmm6
  181502:	vpaddd %zmm6,%zmm2,%zmm17
  181508:	vpxord %zmm3,%zmm17,%zmm2
  18150e:	vprold $0x18,%zmm2,%zmm2
  181515:	vpaddd %zmm4,%zmm2,%zmm5
  18151b:	vmovdqa64 %zmm20,%zmm4
  181521:	vpxord %zmm6,%zmm5,%zmm3
  181527:	vmovdqa64 %zmm18,%zmm6
  18152d:	vprold $0x19,%zmm3,%zmm7
  181534:	vmovdqa64 %zmm12,%zmm3
  18153a:	vmovdqa64 %zmm8,%zmm12
  181540:	vmovdqa64 %zmm21,%zmm8
  181546:	cs nopw 0x0(%rax,%rax,1)
  181550:	mov    (%rdx),%rdi
  181553:	cmp    $0xf,%rdi
  181557:	ja     182813 <bitz::utils::blake3x4::first_pow_nonce+0x2413>
  18155d:	mov    0x8(%rdx),%rcx
  181561:	cmp    $0x10,%rcx
  181565:	jae    1828cc <bitz::utils::blake3x4::first_pow_nonce+0x24cc>
  18156b:	mov    0x10(%rdx),%rax
  18156f:	cmp    $0x10,%rax
  181573:	jae    1827fc <bitz::utils::blake3x4::first_pow_nonce+0x23fc>
  181579:	mov    0x18(%rdx),%rbx
  18157d:	cmp    $0x10,%rbx
  181581:	jae    1828b5 <bitz::utils::blake3x4::first_pow_nonce+0x24b5>
  181587:	mov    0x20(%rdx),%r12
  18158b:	cmp    $0x10,%r12
  18158f:	jae    1827e5 <bitz::utils::blake3x4::first_pow_nonce+0x23e5>
  181595:	mov    0x28(%rdx),%r15
  181599:	cmp    $0x10,%r15
  18159d:	jae    18289e <bitz::utils::blake3x4::first_pow_nonce+0x249e>
  1815a3:	mov    0x30(%rdx),%r13
  1815a7:	cmp    $0x10,%r13
  1815ab:	jae    1827ce <bitz::utils::blake3x4::first_pow_nonce+0x23ce>
  1815b1:	mov    0x38(%rdx),%r14
  1815b5:	cmp    $0x10,%r14
  1815b9:	jae    182870 <bitz::utils::blake3x4::first_pow_nonce+0x2470>
  1815bf:	mov    0x40(%rdx),%r11
  1815c3:	cmp    $0x10,%r11
  1815c7:	jae    1827b7 <bitz::utils::blake3x4::first_pow_nonce+0x23b7>
  1815cd:	mov    0x48(%rdx),%r10
  1815d1:	cmp    $0x10,%r10
  1815d5:	jae    182887 <bitz::utils::blake3x4::first_pow_nonce+0x2487>
  1815db:	mov    0x50(%rdx),%r9
  1815df:	cmp    $0x10,%r9
  1815e3:	jae    182787 <bitz::utils::blake3x4::first_pow_nonce+0x2387>
  1815e9:	mov    0x58(%rdx),%r8
  1815ed:	cmp    $0x10,%r8
  1815f1:	jae    182840 <bitz::utils::blake3x4::first_pow_nonce+0x2440>
  1815f7:	mov    0x60(%rdx),%rsi
  1815fb:	mov    %rsi,0x68(%rsp)
  181600:	cmp    $0x10,%rsi
  181604:	jae    18276e <bitz::utils::blake3x4::first_pow_nonce+0x236e>
  18160a:	mov    0x68(%rdx),%rsi
  18160e:	mov    %rsi,0x60(%rsp)
  181613:	cmp    $0x10,%rsi
  181617:	jae    182857 <bitz::utils::blake3x4::first_pow_nonce+0x2457>
  18161d:	mov    0x70(%rdx),%rsi
  181621:	mov    %rsi,0x58(%rsp)
  181626:	cmp    $0x10,%rsi
  18162a:	jae    18279e <bitz::utils::blake3x4::first_pow_nonce+0x239e>
  181630:	mov    0x78(%rdx),%rsi
  181634:	mov    %rsi,0x50(%rsp)
  181639:	cmp    $0x10,%rsi
  18163d:	jae    182827 <bitz::utils::blake3x4::first_pow_nonce+0x2427>
  181643:	shl    $0x6,%rdi
  181647:	shl    $0x6,%r13
  18164b:	vpaddd %zmm17,%zmm15,%zmm17
  181651:	vpaddd %zmm9,%zmm4,%zmm9
  181657:	shl    $0x6,%r12
  18165b:	shl    $0x6,%r14
  18165f:	vpaddd %zmm13,%zmm6,%zmm13
  181665:	shl    $0x6,%rcx
  181669:	shl    $0x6,%r15
  18166d:	shl    $0x6,%rax
  181671:	vpaddd %zmm16,%zmm7,%zmm16
  181677:	shl    $0x6,%rbx
  18167b:	shl    $0x6,%r9
  18167f:	shl    $0x6,%r11
  181683:	shl    $0x6,%r8
  181687:	shl    $0x6,%r10
  18168b:	sub    $0xffffffffffffff80,%rdx
  18168f:	vpaddd 0x200(%rsp,%rdi,1),%zmm17,%zmm18
  181697:	vpaddd 0x200(%rsp,%r13,1),%zmm9,%zmm17
  18169f:	vpaddd 0x200(%rsp,%r12,1),%zmm13,%zmm23
  1816a7:	vpaddd 0x200(%rsp,%rax,1),%zmm16,%zmm21
  1816af:	mov    0x68(%rsp),%rax
  1816b4:	mov    0x78(%rsp),%r13
  1816b9:	vpaddd 0x200(%rsp,%r14,1),%zmm17,%zmm19
  1816c1:	vpaddd 0x200(%rsp,%rcx,1),%zmm18,%zmm20
  1816c9:	vpaddd 0x200(%rsp,%r15,1),%zmm23,%zmm24
  1816d1:	vpaddd 0x200(%rsp,%rbx,1),%zmm21,%zmm22
  1816d9:	shl    $0x6,%rax
  1816dd:	vpxord %zmm2,%zmm17,%zmm2
  1816e3:	vpxord %zmm8,%zmm23,%zmm8
  1816e9:	vpxord %zmm12,%zmm18,%zmm9
  1816ef:	vpxord %zmm10,%zmm21,%zmm10
  1816f5:	vprold $0x10,%zmm2,%zmm2
  1816fc:	vprold $0x10,%zmm9,%zmm13
  181703:	vprold $0x10,%zmm8,%zmm8
  18170a:	vpaddd %zmm3,%zmm2,%zmm3
  181710:	vpaddd %zmm14,%zmm13,%zmm12
  181716:	vpaddd %zmm5,%zmm8,%zmm5
  18171c:	vpxord %zmm4,%zmm3,%zmm4
  181722:	vpxord %zmm6,%zmm5,%zmm6
  181728:	vpxord %zmm15,%zmm12,%zmm9
  18172e:	vprold $0x14,%zmm4,%zmm4
  181735:	vprold $0x14,%zmm9,%zmm14
  18173c:	vprold $0x14,%zmm6,%zmm6
  181743:	vprold $0x10,%zmm10,%zmm9
  18174a:	vpaddd %zmm4,%zmm19,%zmm17
  181750:	vpaddd %zmm14,%zmm20,%zmm16
  181756:	vpaddd %zmm6,%zmm24,%zmm20
  18175c:	vpaddd %zmm11,%zmm9,%zmm11
  181762:	vpxord %zmm2,%zmm17,%zmm2
  181768:	vpxord %zmm7,%zmm11,%zmm7
  18176e:	vpxord %zmm8,%zmm20,%zmm8
  181774:	vpxord %zmm13,%zmm16,%zmm13
  18177a:	vprold $0x18,%zmm2,%zmm2
  181781:	vprold $0x14,%zmm7,%zmm7
  181788:	vprold $0x18,%zmm8,%zmm8
  18178f:	vprold $0x18,%zmm13,%zmm13
  181796:	vpaddd %zmm3,%zmm2,%zmm3
  18179c:	vpaddd %zmm7,%zmm22,%zmm18
  1817a2:	vpaddd %zmm5,%zmm8,%zmm5
  1817a8:	vpaddd %zmm12,%zmm13,%zmm22
  1817ae:	vpxord %zmm4,%zmm3,%zmm4
  1817b4:	vpxord %zmm6,%zmm5,%zmm6
  1817ba:	vpxord %zmm9,%zmm18,%zmm9
  1817c0:	vpxord %zmm14,%zmm22,%zmm12
  1817c6:	vprold $0x19,%zmm4,%zmm4
  1817cd:	vprold $0x18,%zmm9,%zmm10
  1817d4:	vprold $0x19,%zmm6,%zmm6
  1817db:	vprold $0x19,%zmm12,%zmm23
  1817e2:	vpaddd %zmm20,%zmm4,%zmm19
  1817e8:	vpaddd 0x200(%rsp,%rax,1),%zmm19,%zmm19
  1817f0:	mov    0x60(%rsp),%rax
  1817f5:	vpaddd %zmm11,%zmm10,%zmm9
  1817fb:	vpaddd %zmm18,%zmm6,%zmm18
  181801:	vpaddd 0x200(%rsp,%r9,1),%zmm18,%zmm21
  181809:	vpaddd %zmm23,%zmm17,%zmm12
  18180f:	vpxord %zmm7,%zmm9,%zmm7
  181815:	vpaddd 0x200(%rsp,%r8,1),%zmm21,%zmm18
  18181d:	vprold $0x19,%zmm7,%zmm7
  181824:	vpaddd %zmm16,%zmm7,%zmm11
  18182a:	vpaddd 0x200(%rsp,%r11,1),%zmm11,%zmm15
  181832:	shl    $0x6,%rax
  181836:	vpaddd 0x200(%rsp,%r10,1),%zmm15,%zmm11
  18183e:	vpaddd 0x200(%rsp,%rax,1),%zmm19,%zmm20
  181846:	mov    0x58(%rsp),%rax
  18184b:	vpxord %zmm13,%zmm21,%zmm13
  181851:	vpxord %zmm10,%zmm19,%zmm10
  181857:	vprold $0x10,%zmm10,%zmm10
  18185e:	shl    $0x6,%rax
  181862:	vpaddd %zmm22,%zmm10,%zmm14
  181868:	vpaddd 0x200(%rsp,%rax,1),%zmm12,%zmm12
  181870:	mov    0x50(%rsp),%rax
  181875:	vpxord %zmm2,%zmm15,%zmm2
  18187b:	vpxord %zmm4,%zmm14,%zmm4
  181881:	vprold $0x10,%zmm2,%zmm2
  181888:	vprold $0x14,%zmm4,%zmm4
  18188f:	vpaddd %zmm5,%zmm2,%zmm5
  181895:	vpxord %zmm7,%zmm5,%zmm7
  18189b:	shl    $0x6,%rax
  18189f:	vprold $0x14,%zmm7,%zmm7
  1818a6:	vpaddd 0x200(%rsp,%rax,1),%zmm12,%zmm24
  1818ae:	lea    -0x11494d(%rip),%rax        # 6cf68 <perfetto::base::(anonymous namespace)::kHexmap+0x10908>
  1818b5:	vpaddd %zmm7,%zmm11,%zmm17
  1818bb:	vpxord %zmm8,%zmm12,%zmm8
  1818c1:	vprold $0x10,%zmm13,%zmm12
  1818c8:	vpaddd %zmm4,%zmm20,%zmm13
  1818ce:	vpxord %zmm2,%zmm17,%zmm2
  1818d4:	vprold $0x10,%zmm8,%zmm8
  1818db:	vpxord %zmm10,%zmm13,%zmm10
  1818e1:	vprold $0x18,%zmm2,%zmm2
  1818e8:	vpaddd %zmm3,%zmm12,%zmm3
  1818ee:	vprold $0x18,%zmm10,%zmm10
  1818f5:	vpxord %zmm6,%zmm3,%zmm6
  1818fb:	vpaddd %zmm5,%zmm2,%zmm5
  181901:	vprold $0x14,%zmm6,%zmm6
  181908:	vpaddd %zmm14,%zmm10,%zmm14
  18190e:	vpxord %zmm7,%zmm5,%zmm7
  181914:	vpxord %zmm4,%zmm14,%zmm4
  18191a:	vprold $0x19,%zmm7,%zmm7
  181921:	vpaddd %zmm6,%zmm18,%zmm16
  181927:	vpaddd %zmm9,%zmm8,%zmm18
  18192d:	vprold $0x19,%zmm4,%zmm4
  181934:	vpxord %zmm23,%zmm18,%zmm9
  18193a:	vpxord %zmm12,%zmm16,%zmm11
  181940:	vprold $0x14,%zmm9,%zmm15
  181947:	vprold $0x18,%zmm11,%zmm12
  18194e:	vpaddd %zmm15,%zmm24,%zmm9
  181954:	vpaddd %zmm3,%zmm12,%zmm3
  18195a:	vpxord %zmm8,%zmm9,%zmm8
  181960:	vpxord %zmm6,%zmm3,%zmm6
  181966:	vprold $0x18,%zmm8,%zmm8
  18196d:	vprold $0x19,%zmm6,%zmm6
  181974:	vpaddd %zmm18,%zmm8,%zmm11
  18197a:	vpxord %zmm15,%zmm11,%zmm15
  181980:	vprold $0x19,%zmm15,%zmm15
  181987:	cmp    %rax,%rdx
  18198a:	jne    181550 <bitz::utils::blake3x4::first_pow_nonce+0x1150>
  181990:	vpaddd %zmm17,%zmm15,%zmm17
  181996:	vpaddd %zmm0,%zmm4,%zmm0
  18199c:	lea    0x480(%rsp),%r12
  1819a4:	vpxord %zmm12,%zmm17,%zmm12
  1819aa:	vpaddd %zmm9,%zmm0,%zmm0
  1819b0:	vprold $0x10,%zmm12,%zmm12
  1819b7:	vpxord %zmm2,%zmm0,%zmm2
  1819bd:	vpaddd %zmm31,%zmm0,%zmm0
  1819c3:	vprold $0x10,%zmm2,%zmm2
  1819ca:	vpaddd %zmm14,%zmm12,%zmm14
  1819d0:	vpxord %zmm15,%zmm14,%zmm15
  1819d6:	vpaddd %zmm3,%zmm2,%zmm3
  1819dc:	vprold $0x14,%zmm15,%zmm15
  1819e3:	vpxord %zmm4,%zmm3,%zmm4
  1819e9:	vprold $0x14,%zmm4,%zmm4
  1819f0:	vpaddd %zmm17,%zmm15,%zmm15
  1819f6:	vpaddd %zmm7,%zmm16,%zmm17
  1819fc:	vpxord %zmm12,%zmm15,%zmm12
  181a02:	vpaddd %zmm30,%zmm17,%zmm16
  181a08:	vpaddd %zmm4,%zmm0,%zmm0
  181a0e:	vprold $0x18,%zmm12,%zmm12
  181a15:	vpxord %zmm16,%zmm10,%zmm10
  181a1b:	vpxord %zmm2,%zmm0,%zmm0
  181a21:	vprold $0x10,%zmm10,%zmm10
  181a28:	vprold $0x18,%zmm0,%zmm0
  181a2f:	vpaddd %zmm14,%zmm12,%zmm12
  181a35:	vpaddd 0x180(%rsp),%zmm17,%zmm14
  181a3d:	vpaddd %zmm10,%zmm11,%zmm11
  181a43:	vpaddd %zmm3,%zmm0,%zmm2
  181a49:	vpxord %zmm7,%zmm11,%zmm7
  181a4f:	vpxord %zmm4,%zmm2,%zmm2
  181a55:	vprold $0x14,%zmm7,%zmm7
  181a5c:	vprold $0x19,%zmm2,%zmm2
  181a63:	vpaddd %zmm7,%zmm14,%zmm14
  181a69:	vpxord %zmm10,%zmm14,%zmm10
  181a6f:	vpaddd %zmm26,%zmm6,%zmm14
  181a75:	vprold $0x18,%zmm10,%zmm10
  181a7c:	vpaddd %zmm11,%zmm10,%zmm11
  181a82:	vpxord %zmm7,%zmm11,%zmm7
  181a88:	vpaddd %zmm13,%zmm14,%zmm11
  181a8e:	vpxord %zmm11,%zmm8,%zmm8
  181a94:	vprold $0x19,%zmm7,%zmm7
  181a9b:	vpaddd %zmm1,%zmm11,%zmm1
  181aa1:	vprold $0x10,%zmm8,%zmm8
  181aa8:	vpaddd %zmm7,%zmm15,%zmm3
  181aae:	vpaddd %zmm5,%zmm8,%zmm5
  181ab4:	vpxord %zmm0,%zmm3,%zmm0
  181aba:	vpxord %zmm6,%zmm5,%zmm6
  181ac0:	vprold $0x10,%zmm0,%zmm0
  181ac7:	vprold $0x14,%zmm6,%zmm6
  181ace:	vpaddd %zmm6,%zmm1,%zmm1
  181ad4:	vpxord %zmm8,%zmm1,%zmm6
  181ada:	vpaddd %zmm28,%zmm1,%zmm1
  181ae0:	vprold $0x18,%zmm6,%zmm6
  181ae7:	vpaddd %zmm2,%zmm1,%zmm1
  181aed:	vpxord %zmm10,%zmm1,%zmm4
  181af3:	vpaddd %zmm29,%zmm1,%zmm1
  181af9:	vpaddd %zmm5,%zmm6,%zmm5
  181aff:	vpaddd %zmm0,%zmm5,%zmm0
  181b05:	vpxord %zmm7,%zmm0,%zmm0
  181b0b:	vprold $0x14,%zmm0,%zmm0
  181b12:	vpaddd %zmm3,%zmm0,%zmm0
  181b18:	vprold $0x10,%zmm4,%zmm3
  181b1f:	vpaddd %zmm3,%zmm12,%zmm4
  181b25:	vpxord %zmm2,%zmm4,%zmm2
  181b2b:	vprold $0x14,%zmm2,%zmm2
  181b32:	vpaddd %zmm2,%zmm1,%zmm1
  181b38:	vpxord %zmm3,%zmm1,%zmm1
  181b3e:	vprold $0x18,%zmm1,%zmm1
  181b45:	vpaddd %zmm4,%zmm1,%zmm1
  181b4b:	vpxord %zmm0,%zmm1,%zmm0
  181b51:	vptestnmd 0x1c0(%rsp),%zmm0,%k0
  181b59:	kortestw %k0,%k0
  181b5d:	je     181309 <bitz::utils::blake3x4::first_pow_nonce+0xf09>
  181b63:	cmpl   $0x21,0x3c(%rsp)
  181b68:	kmovd  %k0,%ebx
  181b6c:	jb     1825ce <bitz::utils::blake3x4::first_pow_nonce+0x21ce>
  181b72:	data16 data16 data16 data16 cs nopw 0x0(%rax,%rax,1)
  181b80:	tzcnt  %ebx,%r14d
  181b85:	add    0x40(%rsp),%r14
  181b8a:	mov    0x48(%rsp),%rdi
  181b8f:	mov    0x3c(%rsp),%ecx
  181b93:	mov    %r13,%rsi
  181b96:	mov    %r14,%rdx
  181b99:	vzeroupper
  181b9c:	call   17fd30 <bitz::utils::blake3x4::pow_ok>
  181ba1:	test   %al,%al
  181ba3:	jne    182504 <bitz::utils::blake3x4::first_pow_nonce+0x2104>
  181ba9:	lea    -0x1(%rbx),%eax
  181bac:	and    %bx,%ax
  181baf:	mov    %eax,%ebx
  181bb1:	jne    181b80 <bitz::utils::blake3x4::first_pow_nonce+0x1780>
  181bb3:	jmp    181309 <bitz::utils::blake3x4::first_pow_nonce+0xf09>
  181bb8:	mov    0x40(%rsp),%r15
  181bbd:	vmovdqa64 0x180(%rsp),%zmm25
  181bc5:	vmovdqa64 0x140(%rsp),%zmm26
  181bcd:	vmovdqa64 0x100(%rsp),%zmm27
  181bd5:	vmovdqa64 0xc0(%rsp),%zmm28
  181bdd:	vmovdqa64 0x840(%rsp),%zmm29
  181be5:	vmovdqa64 0x800(%rsp),%zmm30
  181bed:	vmovdqa64 0x7c0(%rsp),%zmm31
  181bf5:	vmovdqa64 0x780(%rsp),%zmm6
  181bfd:	vmovdqa64 0x740(%rsp),%zmm13
  181c05:	vmovdqa64 0x700(%rsp),%zmm17
  181c0d:	vmovdqa64 0x6c0(%rsp),%zmm7
  181c15:	vmovdqa64 0x680(%rsp),%zmm9
  181c1d:	vmovdqa64 0x640(%rsp),%zmm18
  181c25:	vmovdqa64 0x600(%rsp),%zmm10
  181c2d:	vmovdqa64 -0x12c9b7(%rip),%zmm0        # 55280 <GCC_except_table0+0xb00>
  181c37:	vmovdqa64 -0x12c981(%rip),%zmm4        # 552c0 <GCC_except_table0+0xb40>
  181c41:	vpbroadcastd -0x1252ef(%rip),%zmm5        # 5c95c <perfetto::base::(anonymous namespace)::kHexmap+0x2fc>
  181c4b:	vpbroadcastd -0x1252c5(%rip),%zmm8        # 5c990 <perfetto::base::(anonymous namespace)::kHexmap+0x330>
  181c55:	vpbroadcastd -0x1253cf(%rip),%zmm11        # 5c890 <perfetto::base::(anonymous namespace)::kHexmap+0x230>
  181c5f:	vpbroadcastd -0x1252b1(%rip),%zmm12        # 5c9b8 <perfetto::base::(anonymous namespace)::kHexmap+0x358>
  181c69:	vpbroadcastd -0x125337(%rip),%zmm14        # 5c93c <perfetto::base::(anonymous namespace)::kHexmap+0x2dc>
  181c73:	mov    0x70(%rsp),%rbx
  181c78:	mov    $0x0,%ecx
  181c7d:	lea    -0x114f9c(%rip),%rdx        # 6cce8 <perfetto::base::(anonymous namespace)::kHexmap+0x10688>
  181c84:	vpxor  %xmm3,%xmm3,%xmm3
  181c88:	add    $0x10,%r15
  181c8c:	mov    %rbx,%rax
  181c8f:	sub    %r15,%rax
  181c92:	cmovb  %rcx,%rax
  181c96:	cmp    $0xf,%rax
  181c9a:	jbe    18255f <bitz::utils::blake3x4::first_pow_nonce+0x215f>
  181ca0:	vpbroadcastd %r15d,%zmm2
  181ca6:	mov    %r15,0x40(%rsp)
  181cab:	shr    $0x20,%r15
  181caf:	vmovdqa64 %zmm25,0x200(%rsp)
  181cb7:	vmovdqa64 %zmm26,0x240(%rsp)
  181cbf:	vmovdqa64 %zmm27,0x280(%rsp)
  181cc7:	vmovdqa64 %zmm28,0x2c0(%rsp)
  181ccf:	vmovdqa64 %zmm3,0x240(%r12)
  181cd7:	vmovdqa64 %zmm3,0x200(%r12)
  181cdf:	vmovdqa64 %zmm3,0x1c0(%r12)
  181ce7:	vmovdqa64 %zmm3,0x180(%r12)
  181cef:	vmovdqa64 %zmm3,0x140(%r12)
  181cf7:	vmovdqa64 %zmm3,0x100(%r12)
  181cff:	vmovdqa64 %zmm3,0xc0(%r12)
  181d07:	vmovdqa64 %zmm3,0x80(%r12)
  181d0f:	vmovdqa64 %zmm3,0x40(%r12)
  181d17:	vmovdqa64 %zmm3,(%r12)
  181d1e:	vpaddd %zmm0,%zmm2,%zmm0
  181d24:	vpbroadcastd %r15d,%zmm1
  181d2a:	vpcmpltud %zmm2,%zmm0,%k0
  181d31:	vpaddd %zmm4,%zmm2,%zmm2
  181d37:	vmovdqa64 %zmm0,0x300(%rsp)
  181d3f:	vpxord %zmm5,%zmm2,%zmm4
  181d45:	vpmovm2d %k0,%zmm3
  181d4b:	vpsubd %zmm3,%zmm1,%zmm1
  181d51:	vprold $0x10,%zmm4,%zmm3
  181d58:	vpaddd %zmm2,%zmm1,%zmm2
  181d5e:	vmovdqa64 %zmm1,0x340(%rsp)
  181d66:	vpaddd %zmm8,%zmm3,%zmm4
  181d6c:	vpxord %zmm11,%zmm4,%zmm5
  181d72:	vprold $0x14,%zmm5,%zmm5
  181d79:	vpaddd %zmm5,%zmm2,%zmm8
  181d7f:	vpxord %zmm3,%zmm8,%zmm2
  181d85:	vpaddd %zmm14,%zmm8,%zmm19
  181d8b:	vprold $0x18,%zmm2,%zmm11
  181d92:	vpxord %zmm13,%zmm19,%zmm8
  181d98:	vprold $0x10,%zmm8,%zmm8
  181d9f:	vpaddd %zmm4,%zmm11,%zmm2
  181da5:	vpaddd %zmm10,%zmm2,%zmm4
  181dab:	vpxord %zmm5,%zmm2,%zmm3
  181db1:	vpxord %zmm7,%zmm4,%zmm2
  181db7:	vprold $0x19,%zmm3,%zmm3
  181dbe:	vprold $0x14,%zmm2,%zmm5
  181dc5:	vpaddd %zmm5,%zmm9,%zmm16
  181dcb:	vpaddd %zmm3,%zmm6,%zmm9
  181dd1:	vpxord %zmm29,%zmm9,%zmm6
  181dd7:	vpxord %zmm10,%zmm16,%zmm2
  181ddd:	vprold $0x10,%zmm6,%zmm6
  181de4:	vprold $0x18,%zmm2,%zmm2
  181deb:	vpaddd %zmm12,%zmm6,%zmm10
  181df1:	vpaddd %zmm4,%zmm2,%zmm4
  181df7:	vpxord %zmm3,%zmm10,%zmm3
  181dfd:	vpxord %zmm5,%zmm4,%zmm5
  181e03:	vprold $0x14,%zmm3,%zmm12
  181e0a:	vprold $0x19,%zmm5,%zmm7
  181e11:	vpaddd %zmm12,%zmm9,%zmm15
  181e17:	vpxord %zmm6,%zmm15,%zmm3
  181e1d:	vprold $0x18,%zmm3,%zmm9
  181e24:	vpaddd %zmm10,%zmm9,%zmm3
  181e2a:	vpxord %zmm12,%zmm3,%zmm5
  181e30:	vpaddd %zmm30,%zmm8,%zmm12
  181e36:	vpxord %zmm14,%zmm12,%zmm10
  181e3c:	vprold $0x19,%zmm5,%zmm6
  181e43:	vprold $0x14,%zmm10,%zmm14
  181e4a:	vpaddd %zmm14,%zmm19,%zmm13
  181e50:	vpxord %zmm8,%zmm13,%zmm5
  181e56:	vpxord %zmm11,%zmm18,%zmm8
  181e5c:	vprold $0x10,%zmm8,%zmm8
  181e63:	vprold $0x18,%zmm5,%zmm10
  181e6a:	vpaddd %zmm17,%zmm8,%zmm19
  181e70:	vpaddd %zmm12,%zmm10,%zmm12
  181e76:	vpxord %zmm31,%zmm19,%zmm11
  181e7c:	vpxord %zmm14,%zmm12,%zmm5
  181e82:	vprold $0x14,%zmm11,%zmm17
  181e89:	vprold $0x19,%zmm5,%zmm5
  181e90:	vpaddd %zmm17,%zmm18,%zmm11
  181e96:	vpxord %zmm8,%zmm11,%zmm8
  181e9c:	vprold $0x18,%zmm8,%zmm8
  181ea3:	vpaddd %zmm19,%zmm8,%zmm14
  181ea9:	vpxord %zmm17,%zmm14,%zmm17
  181eaf:	vprold $0x19,%zmm17,%zmm17
  181eb6:	cs nopw 0x0(%rax,%rax,1)
  181ec0:	mov    (%rdx),%rdi
  181ec3:	cmp    $0xf,%rdi
  181ec7:	ja     182813 <bitz::utils::blake3x4::first_pow_nonce+0x2413>
  181ecd:	mov    0x8(%rdx),%rcx
  181ed1:	cmp    $0x10,%rcx
  181ed5:	jae    1828cc <bitz::utils::blake3x4::first_pow_nonce+0x24cc>
  181edb:	mov    0x10(%rdx),%rax
  181edf:	cmp    $0x10,%rax
  181ee3:	jae    1827fc <bitz::utils::blake3x4::first_pow_nonce+0x23fc>
  181ee9:	mov    0x18(%rdx),%rbx
  181eed:	cmp    $0x10,%rbx
  181ef1:	jae    1828b5 <bitz::utils::blake3x4::first_pow_nonce+0x24b5>
  181ef7:	mov    0x20(%rdx),%r12
  181efb:	cmp    $0x10,%r12
  181eff:	jae    1827e5 <bitz::utils::blake3x4::first_pow_nonce+0x23e5>
  181f05:	mov    0x28(%rdx),%r15
  181f09:	cmp    $0x10,%r15
  181f0d:	jae    18289e <bitz::utils::blake3x4::first_pow_nonce+0x249e>
  181f13:	mov    0x30(%rdx),%r13
  181f17:	cmp    $0x10,%r13
  181f1b:	jae    1827ce <bitz::utils::blake3x4::first_pow_nonce+0x23ce>
  181f21:	mov    0x38(%rdx),%r14
  181f25:	cmp    $0x10,%r14
  181f29:	jae    182870 <bitz::utils::blake3x4::first_pow_nonce+0x2470>
  181f2f:	mov    0x40(%rdx),%r11
  181f33:	cmp    $0x10,%r11
  181f37:	jae    1827b7 <bitz::utils::blake3x4::first_pow_nonce+0x23b7>
  181f3d:	mov    0x48(%rdx),%r10
  181f41:	cmp    $0x10,%r10
  181f45:	jae    182887 <bitz::utils::blake3x4::first_pow_nonce+0x2487>
  181f4b:	mov    0x50(%rdx),%r9
  181f4f:	cmp    $0x10,%r9
  181f53:	jae    182787 <bitz::utils::blake3x4::first_pow_nonce+0x2387>
  181f59:	mov    0x58(%rdx),%r8
  181f5d:	cmp    $0x10,%r8
  181f61:	jae    182840 <bitz::utils::blake3x4::first_pow_nonce+0x2440>
  181f67:	mov    0x60(%rdx),%rsi
  181f6b:	mov    %rsi,0x68(%rsp)
  181f70:	cmp    $0x10,%rsi
  181f74:	jae    18276e <bitz::utils::blake3x4::first_pow_nonce+0x236e>
  181f7a:	mov    0x68(%rdx),%rsi
  181f7e:	mov    %rsi,0x60(%rsp)
  181f83:	cmp    $0x10,%rsi
  181f87:	jae    182857 <bitz::utils::blake3x4::first_pow_nonce+0x2457>
  181f8d:	mov    0x70(%rdx),%rsi
  181f91:	mov    %rsi,0x58(%rsp)
  181f96:	cmp    $0x10,%rsi
  181f9a:	jae    18279e <bitz::utils::blake3x4::first_pow_nonce+0x239e>
  181fa0:	mov    0x78(%rdx),%rsi
  181fa4:	mov    %rsi,0x50(%rsp)
  181fa9:	cmp    $0x10,%rsi
  181fad:	jae    182827 <bitz::utils::blake3x4::first_pow_nonce+0x2427>
  181fb3:	shl    $0x6,%r13
  181fb7:	vpaddd %zmm11,%zmm5,%zmm11
  181fbd:	shl    $0x6,%rdi
  181fc1:	vpaddd %zmm16,%zmm17,%zmm16
  181fc7:	shl    $0x6,%rax
  181fcb:	vpaddd %zmm15,%zmm7,%zmm15
  181fd1:	shl    $0x6,%r12
  181fd5:	shl    $0x6,%r14
  181fd9:	vpaddd %zmm13,%zmm6,%zmm13
  181fdf:	shl    $0x6,%rbx
  181fe3:	shl    $0x6,%rcx
  181fe7:	shl    $0x6,%r15
  181feb:	shl    $0x6,%r11
  181fef:	shl    $0x6,%r9
  181ff3:	shl    $0x6,%r10
  181ff7:	shl    $0x6,%r8
  181ffb:	sub    $0xffffffffffffff80,%rdx
  181fff:	vpaddd 0x200(%rsp,%r13,1),%zmm11,%zmm18
  182007:	vpaddd 0x200(%rsp,%rdi,1),%zmm16,%zmm16
  18200f:	vpaddd 0x200(%rsp,%rax,1),%zmm15,%zmm21
  182017:	vpaddd 0x200(%rsp,%r12,1),%zmm13,%zmm23
  18201f:	mov    0x68(%rsp),%rax
  182024:	mov    0x78(%rsp),%r13
  182029:	vpaddd 0x200(%rsp,%r14,1),%zmm18,%zmm19
  182031:	vpaddd 0x200(%rsp,%rbx,1),%zmm21,%zmm22
  182039:	vpaddd 0x200(%rsp,%rcx,1),%zmm16,%zmm20
  182041:	vpaddd 0x200(%rsp,%r15,1),%zmm23,%zmm24
  182049:	shl    $0x6,%rax
  18204d:	vpxord %zmm2,%zmm18,%zmm2
  182053:	vpxord %zmm9,%zmm16,%zmm9
  182059:	vpxord %zmm8,%zmm23,%zmm8
  18205f:	vprold $0x10,%zmm2,%zmm2
  182066:	vprold $0x10,%zmm9,%zmm13
  18206d:	vprold $0x10,%zmm8,%zmm8
  182074:	vpaddd %zmm3,%zmm2,%zmm3
  18207a:	vpaddd %zmm12,%zmm13,%zmm12
  182080:	vpaddd %zmm4,%zmm8,%zmm4
  182086:	vpxord %zmm5,%zmm3,%zmm5
  18208c:	vpxord %zmm17,%zmm12,%zmm9
  182092:	vpxord %zmm6,%zmm4,%zmm6
  182098:	vprold $0x14,%zmm5,%zmm5
  18209f:	vprold $0x14,%zmm9,%zmm15
  1820a6:	vpxord %zmm10,%zmm21,%zmm9
  1820ac:	vprold $0x14,%zmm6,%zmm6
  1820b3:	vprold $0x10,%zmm9,%zmm9
  1820ba:	vpaddd %zmm5,%zmm19,%zmm18
  1820c0:	vpaddd %zmm15,%zmm20,%zmm16
  1820c6:	vpaddd %zmm6,%zmm24,%zmm20
  1820cc:	vpxord %zmm2,%zmm18,%zmm2
  1820d2:	vpaddd %zmm14,%zmm9,%zmm10
  1820d8:	vpxord %zmm8,%zmm20,%zmm8
  1820de:	vpxord %zmm13,%zmm16,%zmm13
  1820e4:	vprold $0x18,%zmm2,%zmm2
  1820eb:	vpxord %zmm7,%zmm10,%zmm7
  1820f1:	vprold $0x18,%zmm8,%zmm8
  1820f8:	vprold $0x18,%zmm13,%zmm13
  1820ff:	vprold $0x14,%zmm7,%zmm7
  182106:	vpaddd %zmm3,%zmm2,%zmm3
  18210c:	vpaddd %zmm4,%zmm8,%zmm4
  182112:	vpaddd %zmm12,%zmm13,%zmm12
  182118:	vpxord %zmm5,%zmm3,%zmm5
  18211e:	vpaddd %zmm7,%zmm22,%zmm17
  182124:	vpxord %zmm6,%zmm4,%zmm6
  18212a:	vpxord %zmm15,%zmm12,%zmm15
  182130:	vprold $0x19,%zmm5,%zmm5
  182137:	vpxord %zmm9,%zmm17,%zmm9
  18213d:	vprold $0x19,%zmm6,%zmm6
  182144:	vprold $0x19,%zmm15,%zmm22
  18214b:	vprold $0x18,%zmm9,%zmm9
  182152:	vpaddd %zmm20,%zmm5,%zmm19
  182158:	vpaddd 0x200(%rsp,%rax,1),%zmm19,%zmm19
  182160:	mov    0x60(%rsp),%rax
  182165:	vpaddd %zmm17,%zmm6,%zmm17
  18216b:	vpaddd 0x200(%rsp,%r9,1),%zmm17,%zmm21
  182173:	vpaddd %zmm22,%zmm18,%zmm15
  182179:	vpaddd %zmm10,%zmm9,%zmm11
  18217f:	vpaddd 0x200(%rsp,%r8,1),%zmm21,%zmm17
  182187:	vpxord %zmm7,%zmm11,%zmm7
  18218d:	vprold $0x19,%zmm7,%zmm7
  182194:	shl    $0x6,%rax
  182198:	vpaddd %zmm16,%zmm7,%zmm10
  18219e:	vpaddd 0x200(%rsp,%r11,1),%zmm10,%zmm14
  1821a6:	vpaddd 0x200(%rsp,%rax,1),%zmm19,%zmm20
  1821ae:	mov    0x58(%rsp),%rax
  1821b3:	vpaddd 0x200(%rsp,%r10,1),%zmm14,%zmm10
  1821bb:	vpxord %zmm13,%zmm21,%zmm13
  1821c1:	shl    $0x6,%rax
  1821c5:	vpaddd 0x200(%rsp,%rax,1),%zmm15,%zmm15
  1821cd:	mov    0x50(%rsp),%rax
  1821d2:	vpxord %zmm2,%zmm14,%zmm2
  1821d8:	vpxord %zmm9,%zmm19,%zmm14
  1821de:	vprold $0x10,%zmm13,%zmm9
  1821e5:	vprold $0x10,%zmm2,%zmm2
  1821ec:	vpaddd %zmm3,%zmm9,%zmm3
  1821f2:	vpaddd %zmm4,%zmm2,%zmm4
  1821f8:	vpxord %zmm6,%zmm3,%zmm6
  1821fe:	vpxord %zmm7,%zmm4,%zmm7
  182204:	shl    $0x6,%rax
  182208:	vprold $0x14,%zmm6,%zmm6
  18220f:	vpaddd 0x200(%rsp,%rax,1),%zmm15,%zmm18
  182217:	vprold $0x14,%zmm7,%zmm7
  18221e:	lea    -0x1152bd(%rip),%rax        # 6cf68 <perfetto::base::(anonymous namespace)::kHexmap+0x10908>
  182225:	vpxord %zmm8,%zmm15,%zmm8
  18222b:	vpaddd %zmm7,%zmm10,%zmm16
  182231:	vprold $0x10,%zmm14,%zmm10
  182238:	vpaddd %zmm6,%zmm17,%zmm15
  18223e:	vprold $0x10,%zmm8,%zmm8
  182245:	vpxord %zmm2,%zmm16,%zmm2
  18224b:	vpxord %zmm9,%zmm15,%zmm9
  182251:	vpaddd %zmm12,%zmm10,%zmm12
  182257:	vprold $0x18,%zmm2,%zmm2
  18225e:	vprold $0x18,%zmm9,%zmm9
  182265:	vpaddd %zmm11,%zmm8,%zmm14
  18226b:	vpxord %zmm5,%zmm12,%zmm5
  182271:	vpxord %zmm22,%zmm14,%zmm11
  182277:	vprold $0x14,%zmm5,%zmm5
  18227e:	vpaddd %zmm4,%zmm2,%zmm4
  182284:	vpaddd %zmm3,%zmm9,%zmm3
  18228a:	vprold $0x14,%zmm11,%zmm17
  182291:	vpxord %zmm7,%zmm4,%zmm7
  182297:	vpxord %zmm6,%zmm3,%zmm6
  18229d:	vpaddd %zmm5,%zmm20,%zmm13
  1822a3:	vprold $0x19,%zmm7,%zmm7
  1822aa:	vprold $0x19,%zmm6,%zmm6
  1822b1:	vpaddd %zmm17,%zmm18,%zmm11
  1822b7:	vpxord %zmm10,%zmm13,%zmm10
  1822bd:	vpxord %zmm8,%zmm11,%zmm8
  1822c3:	vprold $0x18,%zmm10,%zmm10
  1822ca:	vprold $0x18,%zmm8,%zmm8
  1822d1:	vpaddd %zmm12,%zmm10,%zmm12
  1822d7:	vpaddd %zmm14,%zmm8,%zmm14
  1822dd:	vpxord %zmm5,%zmm12,%zmm5
  1822e3:	vpxord %zmm17,%zmm14,%zmm17
  1822e9:	vprold $0x19,%zmm5,%zmm5
  1822f0:	vprold $0x19,%zmm17,%zmm17
  1822f7:	cmp    %rax,%rdx
  1822fa:	jne    181ec0 <bitz::utils::blake3x4::first_pow_nonce+0x1ac0>
  182300:	vpaddd %zmm1,%zmm7,%zmm1
  182306:	vpaddd %zmm17,%zmm16,%zmm16
  18230c:	vpaddd %zmm11,%zmm5,%zmm11
  182312:	lea    0x380(%rsp),%r12
  18231a:	vpaddd %zmm15,%zmm1,%zmm1
  182320:	vpxord %zmm9,%zmm16,%zmm9
  182326:	vpxord %zmm2,%zmm11,%zmm2
  18232c:	vpxord %zmm1,%zmm10,%zmm10
  182332:	vprold $0x10,%zmm9,%zmm9
  182339:	vpaddd %zmm25,%zmm1,%zmm1
  18233f:	vprold $0x10,%zmm2,%zmm2
  182346:	vprold $0x10,%zmm10,%zmm10
  18234d:	vpaddd %zmm12,%zmm9,%zmm12
  182353:	vpaddd %zmm3,%zmm2,%zmm3
  182359:	vpaddd %zmm10,%zmm14,%zmm14
  18235f:	vpxord %zmm17,%zmm12,%zmm17
  182365:	vpxord %zmm5,%zmm3,%zmm5
  18236b:	vpxord %zmm7,%zmm14,%zmm7
  182371:	vprold $0x14,%zmm17,%zmm17
  182378:	vprold $0x14,%zmm5,%zmm5
  18237f:	vprold $0x14,%zmm7,%zmm7
  182386:	vpaddd %zmm17,%zmm16,%zmm16
  18238c:	vpaddd %zmm7,%zmm1,%zmm1
  182392:	vpxord %zmm9,%zmm16,%zmm9
  182398:	vpxord %zmm10,%zmm1,%zmm1
  18239e:	vprold $0x18,%zmm9,%zmm9
  1823a5:	vprold $0x18,%zmm1,%zmm1
  1823ac:	vpaddd %zmm12,%zmm9,%zmm9
  1823b2:	vpaddd %zmm26,%zmm6,%zmm12
  1823b8:	vpaddd %zmm14,%zmm1,%zmm10
  1823be:	vpxord %zmm7,%zmm10,%zmm7
  1823c4:	vpaddd %zmm13,%zmm12,%zmm10
  1823ca:	vpxord %zmm10,%zmm8,%zmm8
  1823d0:	vprold $0x19,%zmm7,%zmm7
  1823d7:	vprold $0x10,%zmm8,%zmm8
  1823de:	vpaddd %zmm4,%zmm8,%zmm4
  1823e4:	vpxord %zmm6,%zmm4,%zmm6
  1823ea:	vprold $0x14,%zmm6,%zmm6
  1823f1:	vpaddd %zmm6,%zmm10,%zmm6
  1823f7:	vpaddd %zmm11,%zmm5,%zmm10
  1823fd:	vpxord %zmm2,%zmm10,%zmm2
  182403:	vpxord %zmm8,%zmm6,%zmm8
  182409:	vpaddd %zmm7,%zmm16,%zmm10
  18240f:	vprold $0x18,%zmm2,%zmm2
  182416:	vprold $0x18,%zmm8,%zmm8
  18241d:	vpaddd %zmm3,%zmm2,%zmm3
  182423:	vpxord %zmm2,%zmm10,%zmm2
  182429:	vpaddd %zmm4,%zmm8,%zmm4
  18242f:	vpxord %zmm5,%zmm3,%zmm3
  182435:	vprold $0x10,%zmm2,%zmm2
  18243c:	vpaddd %zmm28,%zmm6,%zmm5
  182442:	vprold $0x19,%zmm3,%zmm3
  182449:	vpaddd %zmm2,%zmm4,%zmm2
  18244f:	vpaddd %zmm3,%zmm5,%zmm4
  182455:	vpxord %zmm7,%zmm2,%zmm2
  18245b:	vpxord %zmm1,%zmm4,%zmm1
  182461:	vpaddd %zmm0,%zmm4,%zmm0
  182467:	vprold $0x14,%zmm2,%zmm2
  18246e:	vprold $0x10,%zmm1,%zmm1
  182475:	vpaddd %zmm2,%zmm10,%zmm2
  18247b:	vpaddd %zmm1,%zmm9,%zmm5
  182481:	vpxord %zmm3,%zmm5,%zmm3
  182487:	vprold $0x14,%zmm3,%zmm3
  18248e:	vpaddd %zmm3,%zmm0,%zmm0
  182494:	vpxord %zmm1,%zmm0,%zmm0
  18249a:	vprold $0x18,%zmm0,%zmm0
  1824a1:	vpaddd %zmm5,%zmm0,%zmm0
  1824a7:	vpxord %zmm2,%zmm0,%zmm0
  1824ad:	vptestnmd 0x1c0(%rsp),%zmm0,%k0
  1824b5:	kortestw %k0,%k0
  1824b9:	je     181bb8 <bitz::utils::blake3x4::first_pow_nonce+0x17b8>
  1824bf:	cmpl   $0x21,0x3c(%rsp)
  1824c4:	kmovd  %k0,%ebx
  1824c8:	jb     1825ce <bitz::utils::blake3x4::first_pow_nonce+0x21ce>
  1824ce:	xchg   %ax,%ax
  1824d0:	tzcnt  %ebx,%r14d
  1824d5:	add    0x40(%rsp),%r14
  1824da:	mov    0x48(%rsp),%rdi
  1824df:	mov    0x3c(%rsp),%ecx
  1824e3:	mov    %r13,%rsi
  1824e6:	mov    %r14,%rdx
  1824e9:	vzeroupper
  1824ec:	call   17fd30 <bitz::utils::blake3x4::pow_ok>
  1824f1:	test   %al,%al
  1824f3:	jne    182504 <bitz::utils::blake3x4::first_pow_nonce+0x2104>
  1824f5:	lea    -0x1(%rbx),%eax
  1824f8:	and    %bx,%ax
  1824fb:	mov    %eax,%ebx
  1824fd:	jne    1824d0 <bitz::utils::blake3x4::first_pow_nonce+0x20d0>
  1824ff:	jmp    181bb8 <bitz::utils::blake3x4::first_pow_nonce+0x17b8>
  182504:	mov    $0x1,%eax
  182509:	jmp    1825b9 <bitz::utils::blake3x4::first_pow_nonce+0x21b9>
  18250e:	mov    0x40(%rsp),%r15
  182513:	cmp    %r15,%rbx
  182516:	cmovbe %r15,%rbx
  18251a:	dec    %r15
  18251d:	mov    %rbx,%r14
  182520:	neg    %r14
  182523:	mov    %rbx,%r12
  182526:	cs nopw 0x0(%rax,%rax,1)
  182530:	lea    (%r14,%r15,1),%rbx
  182534:	cmp    $0xffffffffffffffff,%rbx
  182538:	je     1825aa <bitz::utils::blake3x4::first_pow_nonce+0x21aa>
  18253a:	mov    0x48(%rsp),%rdi
  18253f:	mov    0x3c(%rsp),%ecx
  182543:	inc    %r15
  182546:	mov    %r13,%rsi
  182549:	mov    %r15,%rdx
  18254c:	vzeroupper
  18254f:	call   17fd30 <bitz::utils::blake3x4::pow_ok>
  182554:	test   %al,%al
  182556:	je     182530 <bitz::utils::blake3x4::first_pow_nonce+0x2130>
  182558:	jmp    1825ad <bitz::utils::blake3x4::first_pow_nonce+0x21ad>
  18255a:	mov    0x40(%rsp),%r15
  18255f:	cmp    %r15,%rbx
  182562:	cmovbe %r15,%rbx
  182566:	dec    %r15
  182569:	mov    %rbx,%r14
  18256c:	neg    %r14
  18256f:	mov    %rbx,%r12
  182572:	data16 data16 data16 data16 cs nopw 0x0(%rax,%rax,1)
  182580:	lea    (%r14,%r15,1),%rbx
  182584:	cmp    $0xffffffffffffffff,%rbx
  182588:	je     1825aa <bitz::utils::blake3x4::first_pow_nonce+0x21aa>
  18258a:	mov    0x48(%rsp),%rdi
  18258f:	mov    0x3c(%rsp),%ecx
  182593:	inc    %r15
  182596:	mov    %r13,%rsi
  182599:	mov    %r15,%rdx
  18259c:	vzeroupper
  18259f:	call   17fd30 <bitz::utils::blake3x4::pow_ok>
  1825a4:	test   %al,%al
  1825a6:	je     182580 <bitz::utils::blake3x4::first_pow_nonce+0x2180>
  1825a8:	jmp    1825ad <bitz::utils::blake3x4::first_pow_nonce+0x21ad>
  1825aa:	mov    %r12,%r15
  1825ad:	xor    %eax,%eax
  1825af:	cmp    $0xffffffffffffffff,%rbx
  1825b3:	mov    %r15,%r14
  1825b6:	setne  %al
  1825b9:	mov    %r14,%rdx
  1825bc:	lea    -0x28(%rbp),%rsp
  1825c0:	pop    %rbx
  1825c1:	pop    %r12
  1825c3:	pop    %r13
  1825c5:	pop    %r14
  1825c7:	pop    %r15
  1825c9:	pop    %rbp
  1825ca:	vzeroupper
  1825cd:	ret
  1825ce:	mov    0x40(%rsp),%r14
  1825d3:	tzcnt  %ebx,%eax
  1825d7:	add    %rax,%r14
  1825da:	mov    $0x1,%eax
  1825df:	jmp    1825b9 <bitz::utils::blake3x4::first_pow_nonce+0x21b9>
  1825e1:	lea    -0x1126a4(%rip),%rdi        # 6ff44 <perfetto::base::(anonymous namespace)::kHexmap+0x138e4>
  1825e8:	lea    0x4249e1(%rip),%rdx        # 5a6fd0 <falcon_hybrid::main::__CALLSITE::META+0x6220>
  1825ef:	mov    $0x4d,%esi
  1825f4:	call   3cd5a0 <core::panicking::panic_fmt>
  1825f9:	lea    0x424d48(%rip),%rdx        # 5a7348 <falcon_hybrid::main::__CALLSITE::META+0x6598>
  182600:	mov    0x50(%rsp),%rdi
  182605:	mov    $0x10,%esi
  18260a:	vzeroupper
  18260d:	call   3ce64b <core::panicking::panic_bounds_check>
  182612:	lea    0x424d2f(%rip),%rdx        # 5a7348 <falcon_hybrid::main::__CALLSITE::META+0x6598>
  182619:	mov    0x60(%rsp),%rdi
  18261e:	mov    $0x10,%esi
  182623:	vzeroupper
  182626:	call   3ce64b <core::panicking::panic_bounds_check>
  18262b:	lea    0x424d16(%rip),%rdx        # 5a7348 <falcon_hybrid::main::__CALLSITE::META+0x6598>
  182632:	mov    $0x10,%esi
  182637:	mov    %r9,%rdi
  18263a:	vzeroupper
  18263d:	call   3ce64b <core::panicking::panic_bounds_check>
  182642:	lea    0x424cff(%rip),%rdx        # 5a7348 <falcon_hybrid::main::__CALLSITE::META+0x6598>
  182649:	mov    $0x10,%esi
  18264e:	mov    %r11,%rdi
  182651:	vzeroupper
  182654:	call   3ce64b <core::panicking::panic_bounds_check>
  182659:	lea    0x424ce8(%rip),%rdx        # 5a7348 <falcon_hybrid::main::__CALLSITE::META+0x6598>
  182660:	mov    $0x10,%esi
  182665:	mov    %r15,%rdi
  182668:	vzeroupper
  18266b:	call   3ce64b <core::panicking::panic_bounds_check>
  182670:	lea    0x424cd1(%rip),%rdx        # 5a7348 <falcon_hybrid::main::__CALLSITE::META+0x6598>
  182677:	mov    $0x10,%esi
  18267c:	mov    %rbx,%rdi
  18267f:	vzeroupper
  182682:	call   3ce64b <core::panicking::panic_bounds_check>
  182687:	lea    0x424cba(%rip),%rdx        # 5a7348 <falcon_hybrid::main::__CALLSITE::META+0x6598>
  18268e:	mov    $0x10,%esi
  182693:	mov    %rax,%rdi
  182696:	vzeroupper
  182699:	call   3ce64b <core::panicking::panic_bounds_check>
  18269e:	lea    0x424ca3(%rip),%rdx        # 5a7348 <falcon_hybrid::main::__CALLSITE::META+0x6598>
  1826a5:	mov    $0x10,%esi
  1826aa:	vzeroupper
  1826ad:	call   3ce64b <core::panicking::panic_bounds_check>
  1826b2:	lea    0x424c8f(%rip),%rdx        # 5a7348 <falcon_hybrid::main::__CALLSITE::META+0x6598>
  1826b9:	mov    $0x10,%esi
  1826be:	mov    %r8,%rdi
  1826c1:	vzeroupper
  1826c4:	call   3ce64b <core::panicking::panic_bounds_check>
  1826c9:	lea    0x424c78(%rip),%rdx        # 5a7348 <falcon_hybrid::main::__CALLSITE::META+0x6598>
  1826d0:	mov    0x58(%rsp),%rdi
  1826d5:	mov    $0x10,%esi
  1826da:	vzeroupper
  1826dd:	call   3ce64b <core::panicking::panic_bounds_check>
  1826e2:	lea    0x424c5f(%rip),%rdx        # 5a7348 <falcon_hybrid::main::__CALLSITE::META+0x6598>
  1826e9:	mov    0x68(%rsp),%rdi
  1826ee:	mov    $0x10,%esi
  1826f3:	vzeroupper
  1826f6:	call   3ce64b <core::panicking::panic_bounds_check>
  1826fb:	lea    0x424c46(%rip),%rdx        # 5a7348 <falcon_hybrid::main::__CALLSITE::META+0x6598>
  182702:	mov    $0x10,%esi
  182707:	mov    %r10,%rdi
  18270a:	vzeroupper
  18270d:	call   3ce64b <core::panicking::panic_bounds_check>
  182712:	lea    0x424c2f(%rip),%rdx        # 5a7348 <falcon_hybrid::main::__CALLSITE::META+0x6598>
  182719:	mov    $0x10,%esi
  18271e:	mov    %r13,%rdi
  182721:	vzeroupper
  182724:	call   3ce64b <core::panicking::panic_bounds_check>
  182729:	lea    0x424c18(%rip),%rdx        # 5a7348 <falcon_hybrid::main::__CALLSITE::META+0x6598>
  182730:	mov    $0x10,%esi
  182735:	mov    %r12,%rdi
  182738:	vzeroupper
  18273b:	call   3ce64b <core::panicking::panic_bounds_check>
  182740:	lea    0x424c01(%rip),%rdx        # 5a7348 <falcon_hybrid::main::__CALLSITE::META+0x6598>
  182747:	mov    $0x10,%esi
  18274c:	mov    %r14,%rdi
  18274f:	vzeroupper
  182752:	call   3ce64b <core::panicking::panic_bounds_check>
  182757:	lea    0x424bea(%rip),%rdx        # 5a7348 <falcon_hybrid::main::__CALLSITE::META+0x6598>
  18275e:	mov    $0x10,%esi
  182763:	mov    %rcx,%rdi
  182766:	vzeroupper
  182769:	call   3ce64b <core::panicking::panic_bounds_check>
  18276e:	lea    0x41f373(%rip),%rdx        # 5a1ae8 <falcon_hybrid::main::__CALLSITE::META+0xd38>
  182775:	mov    0x68(%rsp),%rdi
  18277a:	mov    $0x10,%esi
  18277f:	vzeroupper
  182782:	call   3ce64b <core::panicking::panic_bounds_check>
  182787:	lea    0x41f35a(%rip),%rdx        # 5a1ae8 <falcon_hybrid::main::__CALLSITE::META+0xd38>
  18278e:	mov    $0x10,%esi
  182793:	mov    %r9,%rdi
  182796:	vzeroupper
  182799:	call   3ce64b <core::panicking::panic_bounds_check>
  18279e:	lea    0x41f343(%rip),%rdx        # 5a1ae8 <falcon_hybrid::main::__CALLSITE::META+0xd38>
  1827a5:	mov    0x58(%rsp),%rdi
  1827aa:	mov    $0x10,%esi
  1827af:	vzeroupper
  1827b2:	call   3ce64b <core::panicking::panic_bounds_check>
  1827b7:	lea    0x41f32a(%rip),%rdx        # 5a1ae8 <falcon_hybrid::main::__CALLSITE::META+0xd38>
  1827be:	mov    $0x10,%esi
  1827c3:	mov    %r11,%rdi
  1827c6:	vzeroupper
  1827c9:	call   3ce64b <core::panicking::panic_bounds_check>
  1827ce:	lea    0x41f313(%rip),%rdx        # 5a1ae8 <falcon_hybrid::main::__CALLSITE::META+0xd38>
  1827d5:	mov    $0x10,%esi
  1827da:	mov    %r13,%rdi
  1827dd:	vzeroupper
  1827e0:	call   3ce64b <core::panicking::panic_bounds_check>
  1827e5:	lea    0x41f2fc(%rip),%rdx        # 5a1ae8 <falcon_hybrid::main::__CALLSITE::META+0xd38>
  1827ec:	mov    $0x10,%esi
  1827f1:	mov    %r12,%rdi
  1827f4:	vzeroupper
  1827f7:	call   3ce64b <core::panicking::panic_bounds_check>
  1827fc:	lea    0x41f2e5(%rip),%rdx        # 5a1ae8 <falcon_hybrid::main::__CALLSITE::META+0xd38>
  182803:	mov    $0x10,%esi
  182808:	mov    %rax,%rdi
  18280b:	vzeroupper
  18280e:	call   3ce64b <core::panicking::panic_bounds_check>
  182813:	lea    0x41f2ce(%rip),%rdx        # 5a1ae8 <falcon_hybrid::main::__CALLSITE::META+0xd38>
  18281a:	mov    $0x10,%esi
  18281f:	vzeroupper
  182822:	call   3ce64b <core::panicking::panic_bounds_check>
  182827:	lea    0x41f2ba(%rip),%rdx        # 5a1ae8 <falcon_hybrid::main::__CALLSITE::META+0xd38>
  18282e:	mov    0x50(%rsp),%rdi
  182833:	mov    $0x10,%esi
  182838:	vzeroupper
  18283b:	call   3ce64b <core::panicking::panic_bounds_check>
  182840:	lea    0x41f2a1(%rip),%rdx        # 5a1ae8 <falcon_hybrid::main::__CALLSITE::META+0xd38>
  182847:	mov    $0x10,%esi
  18284c:	mov    %r8,%rdi
  18284f:	vzeroupper
  182852:	call   3ce64b <core::panicking::panic_bounds_check>
  182857:	lea    0x41f28a(%rip),%rdx        # 5a1ae8 <falcon_hybrid::main::__CALLSITE::META+0xd38>
  18285e:	mov    0x60(%rsp),%rdi
  182863:	mov    $0x10,%esi
  182868:	vzeroupper
  18286b:	call   3ce64b <core::panicking::panic_bounds_check>
  182870:	lea    0x41f271(%rip),%rdx        # 5a1ae8 <falcon_hybrid::main::__CALLSITE::META+0xd38>
  182877:	mov    $0x10,%esi
  18287c:	mov    %r14,%rdi
  18287f:	vzeroupper
  182882:	call   3ce64b <core::panicking::panic_bounds_check>
  182887:	lea    0x41f25a(%rip),%rdx        # 5a1ae8 <falcon_hybrid::main::__CALLSITE::META+0xd38>
  18288e:	mov    $0x10,%esi
  182893:	mov    %r10,%rdi
  182896:	vzeroupper
  182899:	call   3ce64b <core::panicking::panic_bounds_check>
  18289e:	lea    0x41f243(%rip),%rdx        # 5a1ae8 <falcon_hybrid::main::__CALLSITE::META+0xd38>
  1828a5:	mov    $0x10,%esi
  1828aa:	mov    %r15,%rdi
  1828ad:	vzeroupper
  1828b0:	call   3ce64b <core::panicking::panic_bounds_check>
  1828b5:	lea    0x41f22c(%rip),%rdx        # 5a1ae8 <falcon_hybrid::main::__CALLSITE::META+0xd38>
  1828bc:	mov    $0x10,%esi
  1828c1:	mov    %rbx,%rdi
  1828c4:	vzeroupper
  1828c7:	call   3ce64b <core::panicking::panic_bounds_check>
  1828cc:	lea    0x41f215(%rip),%rdx        # 5a1ae8 <falcon_hybrid::main::__CALLSITE::META+0xd38>
  1828d3:	mov    $0x10,%esi
  1828d8:	mov    %rcx,%rdi
  1828db:	vzeroupper
  1828de:	call   3ce64b <core::panicking::panic_bounds_check>

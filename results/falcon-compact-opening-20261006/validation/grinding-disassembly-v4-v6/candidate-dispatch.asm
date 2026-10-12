
/tmp/falcon-compact-opening-binaries/candidate-v6:     file format elf64-x86-64


Disassembly of section .text:

000000000018d150 <bitz::piop::spartan::grinding::find_grinding_nonce>:
  18d150:	push   %rbp
  18d151:	push   %r15
  18d153:	push   %r14
  18d155:	push   %r13
  18d157:	push   %r12
  18d159:	push   %rbx
  18d15a:	sub    $0x78,%rsp
  18d15e:	mov    %edx,0xc(%rsp)
  18d162:	mov    %rsi,%r12
  18d165:	mov    %rdi,%r15
  18d168:	mov    0x447e49(%rip),%rbx        # 5d4fb8 <tracing_core::metadata::MAX_LEVEL>
  18d16f:	cmp    $0x2,%rbx
  18d173:	jbe    18d18e <bitz::piop::spartan::grinding::find_grinding_nonce+0x3e>
  18d175:	mov    0xc(%rsp),%r13d
  18d17a:	lea    -0x1(%r13),%eax
  18d17e:	cmp    $0x100,%eax
  18d183:	jae    18d2e5 <bitz::piop::spartan::grinding::find_grinding_nonce+0x195>
  18d189:	jmp    18d31a <bitz::piop::spartan::grinding::find_grinding_nonce+0x1ca>
  18d18e:	mov    0x447e23(%rip),%rax        # 5d4fb8 <tracing_core::metadata::MAX_LEVEL>
  18d195:	cmp    $0x2,%rax
  18d199:	ja     18d2f2 <bitz::piop::spartan::grinding::find_grinding_nonce+0x1a2>
  18d19f:	movzbl 0x42a99a(%rip),%eax        # 5b7b40 <bitz::piop::spartan::grinding::find_grinding_nonce::__CALLSITE+0x10>
  18d1a6:	test   %al,%al
  18d1a8:	je     18d2f2 <bitz::piop::spartan::grinding::find_grinding_nonce+0x1a2>
  18d1ae:	movzbl %al,%eax
  18d1b1:	cmp    $0x1,%eax
  18d1b4:	je     18d1d7 <bitz::piop::spartan::grinding::find_grinding_nonce+0x87>
  18d1b6:	cmp    $0x2,%eax
  18d1b9:	je     18d212 <bitz::piop::spartan::grinding::find_grinding_nonce+0xc2>
  18d1bb:	lea    0x42a96e(%rip),%rdi        # 5b7b30 <bitz::piop::spartan::grinding::find_grinding_nonce::__CALLSITE>
  18d1c2:	call   4e14c0 <<tracing_core::callsite::DefaultCallsite>::register>
  18d1c7:	test   %al,%al
  18d1c9:	je     18d2f2 <bitz::piop::spartan::grinding::find_grinding_nonce+0x1a2>
  18d1cf:	movzbl %al,%eax
  18d1d2:	cmp    $0x2,%eax
  18d1d5:	je     18d212 <bitz::piop::spartan::grinding::find_grinding_nonce+0xc2>
  18d1d7:	mov    0x42a952(%rip),%rsi        # 5b7b30 <bitz::piop::spartan::grinding::find_grinding_nonce::__CALLSITE>
  18d1de:	mov    0x4487c3(%rip),%rax        # 5d59a8 <tracing_core::dispatcher::GLOBAL_INIT>
  18d1e5:	lea    -0xdc87d(%rip),%rdi        # b096f <tracing_core::dispatcher::NO_SUBSCRIBER>
  18d1ec:	cmp    $0x2,%rax
  18d1f0:	lea    0x4259a9(%rip),%rax        # 5b2ba0 <<bitz::piop::spartan::falcon1024_ct::hybrid_sumcheck::State<_> as bitz::sumcheck::inner::input::State<field::binary::Gf128Ops>>::fold::__CALLSITE::META+0x91f0>
  18d1f7:	cmove  0x447da9(%rip),%rdi        # 5d4fa8 <tracing_core::dispatcher::GLOBAL_DISPATCH>
  18d1ff:	cmove  0x447da9(%rip),%rax        # 5d4fb0 <tracing_core::dispatcher::GLOBAL_DISPATCH>
  18d207:	call   *0x28(%rax)
  18d20a:	test   %al,%al
  18d20c:	je     18d2f2 <bitz::piop::spartan::grinding::find_grinding_nonce+0x1a2>
  18d212:	mov    0x42a917(%rip),%r14        # 5b7b30 <bitz::piop::spartan::grinding::find_grinding_nonce::__CALLSITE>
  18d219:	lea    0xc(%rsp),%rcx
  18d21e:	movq   $0x1,0x58(%rsp)
  18d227:	lea    -0xdc8bf(%rip),%r13        # b096f <tracing_core::dispatcher::NO_SUBSCRIBER>
  18d22e:	lea    0x42596b(%rip),%rbp        # 5b2ba0 <<bitz::piop::spartan::falcon1024_ct::hybrid_sumcheck::State<_> as bitz::sumcheck::inner::input::State<field::binary::Gf128Ops>>::fold::__CALLSITE::META+0x91f0>
  18d235:	lea    0x10(%rsp),%rsi
  18d23a:	mov    %rcx,0x48(%rsp)
  18d23f:	lea    0x425a0a(%rip),%rcx        # 5b2c50 <<bitz::piop::spartan::falcon1024_ct::hybrid_sumcheck::State<_> as bitz::sumcheck::inner::input::State<field::binary::Gf128Ops>>::fold::__CALLSITE::META+0x92a0>
  18d246:	mov    %rcx,0x50(%rsp)
  18d24b:	lea    0x48(%rsp),%rcx
  18d250:	mov    %rcx,0x60(%rsp)
  18d255:	movq   $0x1,0x68(%rsp)
  18d25e:	lea    0x30(%r14),%rax
  18d262:	mov    %rax,0x70(%rsp)
  18d267:	mov    0x44873a(%rip),%rax        # 5d59a8 <tracing_core::dispatcher::GLOBAL_INIT>
  18d26e:	mov    %r14,0x20(%rsp)
  18d273:	cmp    $0x2,%rax
  18d277:	lea    0x58(%rsp),%rax
  18d27c:	cmove  0x447d24(%rip),%r13        # 5d4fa8 <tracing_core::dispatcher::GLOBAL_DISPATCH>
  18d284:	cmove  0x447d24(%rip),%rbp        # 5d4fb0 <tracing_core::dispatcher::GLOBAL_DISPATCH>
  18d28c:	mov    %rax,0x28(%rsp)
  18d291:	movq   $0x1,0x10(%rsp)
  18d29a:	mov    %r13,%rdi
  18d29d:	call   *0x38(%rbp)
  18d2a0:	movq   $0x0,0x10(%rsp)
  18d2a9:	mov    %r13,0x18(%rsp)
  18d2ae:	mov    %rbp,0x20(%rsp)
  18d2b3:	mov    %rax,0x28(%rsp)
  18d2b8:	lea    0x28(%rsp),%rsi
  18d2bd:	mov    %rax,0x38(%rsp)
  18d2c2:	mov    %r14,0x30(%rsp)
  18d2c7:	mov    %r13,%rdi
  18d2ca:	mov    %rbp,0x40(%rsp)
  18d2cf:	call   *0x60(%rbp)
  18d2d2:	xor    %r14d,%r14d
  18d2d5:	mov    0xc(%rsp),%r13d
  18d2da:	lea    -0x1(%r13),%eax
  18d2de:	cmp    $0x100,%eax
  18d2e3:	jb     18d31a <bitz::piop::spartan::grinding::find_grinding_nonce+0x1ca>
  18d2e5:	movl   $0x0,(%r15)
  18d2ec:	mov    %r13d,0x4(%r15)
  18d2f0:	jmp    18d34b <bitz::piop::spartan::grinding::find_grinding_nonce+0x1fb>
  18d2f2:	mov    $0x2,%r14d
  18d2f8:	movq   $0x2,0x10(%rsp)
  18d301:	movq   $0x0,0x30(%rsp)
  18d30a:	mov    0xc(%rsp),%r13d
  18d30f:	lea    -0x1(%r13),%eax
  18d313:	cmp    $0x100,%eax
  18d318:	jae    18d2e5 <bitz::piop::spartan::grinding::find_grinding_nonce+0x195>
  18d31a:	cmp    $0xb,%r13d
  18d31e:	jbe    18d3b4 <bitz::piop::spartan::grinding::find_grinding_nonce+0x264>
  18d324:	mov    $0x20,%esi
  18d329:	mov    %r12,%rdi
  18d32c:	mov    %r13d,%edx
  18d32f:	call   17ec50 <bitz::utils::blake3x4::smallest_pow_nonce>
  18d334:	mov    $0x2,%ecx
  18d339:	cmp    $0x1,%rax
  18d33d:	jne    18d348 <bitz::piop::spartan::grinding::find_grinding_nonce+0x1f8>
  18d33f:	mov    $0xffffffff,%ecx
  18d344:	mov    %rdx,0x8(%r15)
  18d348:	mov    %ecx,(%r15)
  18d34b:	cmp    $0x3,%rbx
  18d34f:	mov    0x40(%rsp),%rbx
  18d354:	setae  %al
  18d357:	cmp    $0x2,%r14
  18d35b:	sete   %cl
  18d35e:	or     %al,%cl
  18d360:	jne    18d486 <bitz::piop::spartan::grinding::find_grinding_nonce+0x336>
  18d366:	mov    0x18(%rsp),%rdi
  18d36b:	lea    0x28(%rsp),%rsi
  18d370:	cmp    $0x1,%r14
  18d374:	jne    18d386 <bitz::piop::spartan::grinding::find_grinding_nonce+0x236>
  18d376:	mov    0x10(%rbx),%rax
  18d37a:	dec    %rax
  18d37d:	and    $0xfffffffffffffff0,%rax
  18d381:	lea    0x10(%rdi,%rax,1),%rdi
  18d386:	call   *0x68(%rbx)
  18d389:	mov    0x18(%rsp),%rdi
  18d38e:	test   $0x1,%r14b
  18d392:	je     18d3a4 <bitz::piop::spartan::grinding::find_grinding_nonce+0x254>
  18d394:	mov    0x10(%rbx),%rax
  18d398:	dec    %rax
  18d39b:	and    $0xfffffffffffffff0,%rax
  18d39f:	lea    0x10(%rdi,%rax,1),%rdi
  18d3a4:	mov    0x38(%rsp),%rsi
  18d3a9:	call   *0x80(%rbx)
  18d3af:	jmp    18d469 <bitz::piop::spartan::grinding::find_grinding_nonce+0x319>
  18d3b4:	mov    $0x20,%esi
  18d3b9:	mov    $0xffffffffffffffff,%rcx
  18d3c0:	mov    %r12,%rdi
  18d3c3:	xor    %edx,%edx
  18d3c5:	mov    %r13d,%r8d
  18d3c8:	call   180400 <bitz::utils::blake3x4::first_pow_nonce>
  18d3cd:	test   $0x1,%al
  18d3cf:	je     18d3d6 <bitz::piop::spartan::grinding::find_grinding_nonce+0x286>
  18d3d1:	mov    %rdx,%rbp
  18d3d4:	jmp    18d3fd <bitz::piop::spartan::grinding::find_grinding_nonce+0x2ad>
  18d3d6:	mov    $0x20,%esi
  18d3db:	mov    $0xffffffffffffffff,%rdx
  18d3e2:	mov    $0xffffffffffffffff,%rbp
  18d3e9:	mov    %r12,%rdi
  18d3ec:	mov    %r13d,%ecx
  18d3ef:	call   17fd30 <bitz::utils::blake3x4::pow_ok>
  18d3f4:	mov    $0x2,%ecx
  18d3f9:	test   %al,%al
  18d3fb:	je     18d406 <bitz::piop::spartan::grinding::find_grinding_nonce+0x2b6>
  18d3fd:	mov    $0xffffffff,%ecx
  18d402:	mov    %rbp,0x8(%r15)
  18d406:	cmp    $0x3,%rbx
  18d40a:	mov    0x40(%rsp),%rbx
  18d40f:	mov    %ecx,(%r15)
  18d412:	setae  %al
  18d415:	cmp    $0x2,%r14
  18d419:	sete   %cl
  18d41c:	or     %al,%cl
  18d41e:	jne    18d486 <bitz::piop::spartan::grinding::find_grinding_nonce+0x336>
  18d420:	mov    0x18(%rsp),%rdi
  18d425:	lea    0x28(%rsp),%rsi
  18d42a:	cmp    $0x1,%r14
  18d42e:	jne    18d440 <bitz::piop::spartan::grinding::find_grinding_nonce+0x2f0>
  18d430:	mov    0x10(%rbx),%rax
  18d434:	dec    %rax
  18d437:	and    $0xfffffffffffffff0,%rax
  18d43b:	lea    0x10(%rdi,%rax,1),%rdi
  18d440:	call   *0x68(%rbx)
  18d443:	mov    0x18(%rsp),%rdi
  18d448:	test   $0x1,%r14b
  18d44c:	je     18d45e <bitz::piop::spartan::grinding::find_grinding_nonce+0x30e>
  18d44e:	mov    0x10(%rbx),%rax
  18d452:	dec    %rax
  18d455:	and    $0xfffffffffffffff0,%rax
  18d459:	lea    0x10(%rdi,%rax,1),%rdi
  18d45e:	mov    0x38(%rsp),%rsi
  18d463:	call   *0x80(%rbx)
  18d469:	test   %r14,%r14
  18d46c:	je     18d486 <bitz::piop::spartan::grinding::find_grinding_nonce+0x336>
  18d46e:	mov    0x18(%rsp),%rax
  18d473:	lock decq (%rax)
  18d477:	jne    18d486 <bitz::piop::spartan::grinding::find_grinding_nonce+0x336>
  18d479:	mov    0x18(%rsp),%rdi
  18d47e:	mov    %rbx,%rsi
  18d481:	call   4e1400 <<alloc::sync::Arc<dyn tracing_core::subscriber::Subscriber + core::marker::Send + core::marker::Sync>>::drop_slow>
  18d486:	add    $0x78,%rsp
  18d48a:	pop    %rbx
  18d48b:	pop    %r12
  18d48d:	pop    %r13
  18d48f:	pop    %r14
  18d491:	pop    %r15
  18d493:	pop    %rbp
  18d494:	ret
  18d495:	mov    %rax,%r15
  18d498:	lea    0x10(%rsp),%rdi
  18d49d:	call   18a5b0 <core::ptr::drop_glue::<core::option::Option<tracing::span::Inner>>>
  18d4a2:	jmp    18d50a <bitz::piop::spartan::grinding::find_grinding_nonce+0x3ba>
  18d4a4:	call   3d1d63 <core::panicking::panic_in_cleanup>
  18d4a9:	jmp    18d4bf <bitz::piop::spartan::grinding::find_grinding_nonce+0x36f>
  18d4ab:	mov    %rax,%r15
  18d4ae:	lea    0x10(%rsp),%rdi
  18d4b3:	call   18a5b0 <core::ptr::drop_glue::<core::option::Option<tracing::span::Inner>>>
  18d4b8:	jmp    18d50a <bitz::piop::spartan::grinding::find_grinding_nonce+0x3ba>
  18d4ba:	call   3d1d63 <core::panicking::panic_in_cleanup>
  18d4bf:	mov    %rax,%r15
  18d4c2:	jmp    18d500 <bitz::piop::spartan::grinding::find_grinding_nonce+0x3b0>
  18d4c4:	mov    %rax,%r15
  18d4c7:	cmp    $0x2,%rbx
  18d4cb:	ja     18d50a <bitz::piop::spartan::grinding::find_grinding_nonce+0x3ba>
  18d4cd:	cmp    $0x2,%r14
  18d4d1:	je     18d500 <bitz::piop::spartan::grinding::find_grinding_nonce+0x3b0>
  18d4d3:	mov    0x18(%rsp),%rdi
  18d4d8:	lea    0x28(%rsp),%rsi
  18d4dd:	cmp    $0x1,%r14
  18d4e1:	jne    18d4f8 <bitz::piop::spartan::grinding::find_grinding_nonce+0x3a8>
  18d4e3:	mov    0x40(%rsp),%rax
  18d4e8:	mov    0x10(%rax),%rax
  18d4ec:	dec    %rax
  18d4ef:	and    $0xfffffffffffffff0,%rax
  18d4f3:	lea    0x10(%rdi,%rax,1),%rdi
  18d4f8:	mov    0x40(%rsp),%rax
  18d4fd:	call   *0x68(%rax)
  18d500:	lea    0x10(%rsp),%rdi
  18d505:	call   18a530 <core::ptr::drop_glue::<tracing::span::Span>>
  18d50a:	mov    %r15,%rdi
  18d50d:	call   59e0f0 <_Unwind_Resume@plt>
  18d512:	call   3d1d63 <core::panicking::panic_in_cleanup>

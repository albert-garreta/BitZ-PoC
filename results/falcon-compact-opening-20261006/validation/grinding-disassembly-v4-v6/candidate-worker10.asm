
/tmp/falcon-compact-opening-binaries/candidate-v6:     file format elf64-x86-64


Disassembly of section .text:

000000000017faf0 <<rayon_core::job::StackJob<rayon_core::latch::LatchRef<rayon_core::latch::CountLatch>, &rayon_core::broadcast::broadcast_in<bitz::utils::blake3x4::smallest_pow_nonce_with<10, bitz::utils::blake3x4::first_pow_nonce>::{closure#0}, ()>::{closure#0}, ()> as rayon_core::job::Job>::execute>:
  17faf0:	push   %rbp
  17faf1:	push   %r15
  17faf3:	push   %r14
  17faf5:	push   %r13
  17faf7:	push   %r12
  17faf9:	push   %rbx
  17fafa:	sub    $0x18,%rsp
  17fafe:	mov    0x8(%rdi),%rax
  17fb02:	movq   $0x0,0x8(%rdi)
  17fb0a:	test   %rax,%rax
  17fb0d:	je     17fc9b <<rayon_core::job::StackJob<rayon_core::latch::LatchRef<rayon_core::latch::CountLatch>, &rayon_core::broadcast::broadcast_in<bitz::utils::blake3x4::smallest_pow_nonce_with<10, bitz::utils::blake3x4::first_pow_nonce>::{closure#0}, ()>::{closure#0}, ()> as rayon_core::job::Job>::execute+0x1ab>
  17fb13:	cmpq   $0x0,%fs:0xffffffffffffff60
  17fb1d:	mov    %rdi,0x10(%rsp)
  17fb22:	je     17fca9 <<rayon_core::job::StackJob<rayon_core::latch::LatchRef<rayon_core::latch::CountLatch>, &rayon_core::broadcast::broadcast_in<bitz::utils::blake3x4::smallest_pow_nonce_with<10, bitz::utils::blake3x4::first_pow_nonce>::{closure#0}, ()>::{closure#0}, ()> as rayon_core::job::Job>::execute+0x1b9>
  17fb28:	mov    (%rax),%rbp
  17fb2b:	mov    $0x1,%edx
  17fb30:	lock xadd %rdx,0x0(%rbp)
  17fb36:	mov    %rdx,%rcx
  17fb39:	shr    $0x36,%rcx
  17fb3d:	je     17fbec <<rayon_core::job::StackJob<rayon_core::latch::LatchRef<rayon_core::latch::CountLatch>, &rayon_core::broadcast::broadcast_in<bitz::utils::blake3x4::smallest_pow_nonce_with<10, bitz::utils::blake3x4::first_pow_nonce>::{closure#0}, ()>::{closure#0}, ()> as rayon_core::job::Job>::execute+0xfc>
  17fb43:	mov    $0x1,%ebx
  17fb48:	mov    0x10(%rsp),%r15
  17fb4d:	mov    0x8(%rsp),%r13
  17fb52:	mov    (%rsp),%rbp
  17fb56:	cmpl   $0x2,0x10(%r15)
  17fb5b:	jb     17fb82 <<rayon_core::job::StackJob<rayon_core::latch::LatchRef<rayon_core::latch::CountLatch>, &rayon_core::broadcast::broadcast_in<bitz::utils::blake3x4::smallest_pow_nonce_with<10, bitz::utils::blake3x4::first_pow_nonce>::{closure#0}, ()>::{closure#0}, ()> as rayon_core::job::Job>::execute+0x92>
  17fb5d:	mov    0x20(%r15),%r14
  17fb61:	mov    0x18(%r15),%r12
  17fb65:	mov    (%r14),%rax
  17fb68:	test   %rax,%rax
  17fb6b:	je     17fb72 <<rayon_core::job::StackJob<rayon_core::latch::LatchRef<rayon_core::latch::CountLatch>, &rayon_core::broadcast::broadcast_in<bitz::utils::blake3x4::smallest_pow_nonce_with<10, bitz::utils::blake3x4::first_pow_nonce>::{closure#0}, ()>::{closure#0}, ()> as rayon_core::job::Job>::execute+0x82>
  17fb6d:	mov    %r12,%rdi
  17fb70:	call   *%rax
  17fb72:	cmpq   $0x0,0x8(%r14)
  17fb77:	je     17fb82 <<rayon_core::job::StackJob<rayon_core::latch::LatchRef<rayon_core::latch::CountLatch>, &rayon_core::broadcast::broadcast_in<bitz::utils::blake3x4::smallest_pow_nonce_with<10, bitz::utils::blake3x4::first_pow_nonce>::{closure#0}, ()>::{closure#0}, ()> as rayon_core::job::Job>::execute+0x92>
  17fb79:	mov    %r12,%rdi
  17fb7c:	call   *0x4364de(%rip)        # 5b6060 <free@GLIBC_2.2.5>
  17fb82:	mov    %rbx,0x10(%r15)
  17fb86:	mov    %rbp,0x18(%r15)
  17fb8a:	mov    %r13,0x20(%r15)
  17fb8e:	mov    (%r15),%rdi
  17fb91:	lock decq (%rdi)
  17fb95:	jne    17fc8c <<rayon_core::job::StackJob<rayon_core::latch::LatchRef<rayon_core::latch::CountLatch>, &rayon_core::broadcast::broadcast_in<bitz::utils::blake3x4::smallest_pow_nonce_with<10, bitz::utils::blake3x4::first_pow_nonce>::{closure#0}, ()>::{closure#0}, ()> as rayon_core::job::Job>::execute+0x19c>
  17fb9b:	mov    0x8(%rdi),%rax
  17fb9f:	test   %rax,%rax
  17fba2:	je     17fc83 <<rayon_core::job::StackJob<rayon_core::latch::LatchRef<rayon_core::latch::CountLatch>, &rayon_core::broadcast::broadcast_in<bitz::utils::blake3x4::smallest_pow_nonce_with<10, bitz::utils::blake3x4::first_pow_nonce>::{closure#0}, ()>::{closure#0}, ()> as rayon_core::job::Job>::execute+0x193>
  17fba8:	mov    0x18(%rdi),%rsi
  17fbac:	lock incq (%rax)
  17fbb0:	jle    17fd26 <<rayon_core::job::StackJob<rayon_core::latch::LatchRef<rayon_core::latch::CountLatch>, &rayon_core::broadcast::broadcast_in<bitz::utils::blake3x4::smallest_pow_nonce_with<10, bitz::utils::blake3x4::first_pow_nonce>::{closure#0}, ()>::{closure#0}, ()> as rayon_core::job::Job>::execute+0x236>
  17fbb6:	mov    $0x3,%eax
  17fbbb:	mov    0x8(%rdi),%rbx
  17fbbf:	xchg   %rax,0x10(%rdi)
  17fbc3:	cmp    $0x2,%rax
  17fbc7:	jne    17fbd5 <<rayon_core::job::StackJob<rayon_core::latch::LatchRef<rayon_core::latch::CountLatch>, &rayon_core::broadcast::broadcast_in<bitz::utils::blake3x4::smallest_pow_nonce_with<10, bitz::utils::blake3x4::first_pow_nonce>::{closure#0}, ()>::{closure#0}, ()> as rayon_core::job::Job>::execute+0xe5>
  17fbc9:	lea    0x1d8(%rbx),%rdi
  17fbd0:	call   481e60 <<rayon_core::sleep::Sleep>::wake_specific_thread>
  17fbd5:	lock decq (%rbx)
  17fbd9:	jne    17fc8c <<rayon_core::job::StackJob<rayon_core::latch::LatchRef<rayon_core::latch::CountLatch>, &rayon_core::broadcast::broadcast_in<bitz::utils::blake3x4::smallest_pow_nonce_with<10, bitz::utils::blake3x4::first_pow_nonce>::{closure#0}, ()>::{closure#0}, ()> as rayon_core::job::Job>::execute+0x19c>
  17fbdf:	mov    %rbx,%rdi
  17fbe2:	call   482b40 <<alloc::sync::Arc<rayon_core::registry::Registry>>::drop_slow>
  17fbe7:	jmp    17fc8c <<rayon_core::job::StackJob<rayon_core::latch::LatchRef<rayon_core::latch::CountLatch>, &rayon_core::broadcast::broadcast_in<bitz::utils::blake3x4::smallest_pow_nonce_with<10, bitz::utils::blake3x4::first_pow_nonce>::{closure#0}, ()>::{closure#0}, ()> as rayon_core::job::Job>::execute+0x19c>
  17fbec:	mov    0x8(%rax),%r15
  17fbf0:	mov    0x18(%rax),%r12
  17fbf4:	mov    0x20(%rax),%r13
  17fbf8:	mov    0x28(%rax),%r14
  17fbfc:	mov    $0x1,%ebx
  17fc01:	jmp    17fc2e <<rayon_core::job::StackJob<rayon_core::latch::LatchRef<rayon_core::latch::CountLatch>, &rayon_core::broadcast::broadcast_in<bitz::utils::blake3x4::smallest_pow_nonce_with<10, bitz::utils::blake3x4::first_pow_nonce>::{closure#0}, ()>::{closure#0}, ()> as rayon_core::job::Job>::execute+0x13e>
  17fc03:	data16 data16 data16 cs nopw 0x0(%rax,%rax,1)
  17fc10:	mov    $0x1,%edx
  17fc15:	movabs $0x3fffffffffffff,%rax
  17fc1f:	lock xadd %rdx,0x0(%rbp)
  17fc25:	cmp    %rax,%rdx
  17fc28:	ja     17fb48 <<rayon_core::job::StackJob<rayon_core::latch::LatchRef<rayon_core::latch::CountLatch>, &rayon_core::broadcast::broadcast_in<bitz::utils::blake3x4::smallest_pow_nonce_with<10, bitz::utils::blake3x4::first_pow_nonce>::{closure#0}, ()>::{closure#0}, ()> as rayon_core::job::Job>::execute+0x58>
  17fc2e:	mov    (%r15),%rax
  17fc31:	shl    $0xa,%rdx
  17fc35:	cmp    %rax,%rdx
  17fc38:	jae    17fb43 <<rayon_core::job::StackJob<rayon_core::latch::LatchRef<rayon_core::latch::CountLatch>, &rayon_core::broadcast::broadcast_in<bitz::utils::blake3x4::smallest_pow_nonce_with<10, bitz::utils::blake3x4::first_pow_nonce>::{closure#0}, ()>::{closure#0}, ()> as rayon_core::job::Job>::execute+0x53>
  17fc3e:	mov    (%r14),%r8d
  17fc41:	mov    %rdx,%rcx
  17fc44:	mov    $0xffffffffffffffff,%rax
  17fc4b:	add    $0x400,%rcx
  17fc52:	cmovb  %rax,%rcx
  17fc56:	mov    %r12,%rdi
  17fc59:	mov    %r13,%rsi
  17fc5c:	call   180400 <bitz::utils::blake3x4::first_pow_nonce>
  17fc61:	cmp    $0x1,%rax
  17fc65:	jne    17fc10 <<rayon_core::job::StackJob<rayon_core::latch::LatchRef<rayon_core::latch::CountLatch>, &rayon_core::broadcast::broadcast_in<bitz::utils::blake3x4::smallest_pow_nonce_with<10, bitz::utils::blake3x4::first_pow_nonce>::{closure#0}, ()>::{closure#0}, ()> as rayon_core::job::Job>::execute+0x120>
  17fc67:	mov    (%r15),%rax
  17fc6a:	nopw   0x0(%rax,%rax,1)
  17fc70:	cmp    %rdx,%rax
  17fc73:	mov    %rdx,%rcx
  17fc76:	cmovbe %rax,%rcx
  17fc7a:	lock cmpxchg %rcx,(%r15)
  17fc7f:	jne    17fc70 <<rayon_core::job::StackJob<rayon_core::latch::LatchRef<rayon_core::latch::CountLatch>, &rayon_core::broadcast::broadcast_in<bitz::utils::blake3x4::smallest_pow_nonce_with<10, bitz::utils::blake3x4::first_pow_nonce>::{closure#0}, ()>::{closure#0}, ()> as rayon_core::job::Job>::execute+0x180>
  17fc81:	jmp    17fc10 <<rayon_core::job::StackJob<rayon_core::latch::LatchRef<rayon_core::latch::CountLatch>, &rayon_core::broadcast::broadcast_in<bitz::utils::blake3x4::smallest_pow_nonce_with<10, bitz::utils::blake3x4::first_pow_nonce>::{closure#0}, ()>::{closure#0}, ()> as rayon_core::job::Job>::execute+0x120>
  17fc83:	add    $0x10,%rdi
  17fc87:	call   17ead0 <<rayon_core::latch::LockLatch as rayon_core::latch::Latch>::set>
  17fc8c:	add    $0x18,%rsp
  17fc90:	pop    %rbx
  17fc91:	pop    %r12
  17fc93:	pop    %r13
  17fc95:	pop    %r14
  17fc97:	pop    %r15
  17fc99:	pop    %rbp
  17fc9a:	ret
  17fc9b:	lea    0x42e08e(%rip),%rdi        # 5add30 <<bitz::piop::spartan::falcon1024_ct::hybrid_sumcheck::State<_> as bitz::sumcheck::inner::input::State<field::binary::Gf128Ops>>::fold::__CALLSITE::META+0x4380>
  17fca2:	call   3d1ba0 <core::option::unwrap_failed>
  17fca7:	jmp    17fd26 <<rayon_core::job::StackJob<rayon_core::latch::LatchRef<rayon_core::latch::CountLatch>, &rayon_core::broadcast::broadcast_in<bitz::utils::blake3x4::smallest_pow_nonce_with<10, bitz::utils::blake3x4::first_pow_nonce>::{closure#0}, ()>::{closure#0}, ()> as rayon_core::job::Job>::execute+0x236>
  17fca9:	lea    -0x113090(%rip),%rdi        # 6cc20 <perfetto::base::(anonymous namespace)::kHexmap+0x105c0>
  17fcb0:	lea    0x421d99(%rip),%rdx        # 5a1a50 <falcon_hybrid::main::__CALLSITE::META+0xca0>
  17fcb7:	mov    $0x2a,%esi
  17fcbc:	call   3cf670 <core::panicking::panic>
  17fcc1:	jmp    17fd26 <<rayon_core::job::StackJob<rayon_core::latch::LatchRef<rayon_core::latch::CountLatch>, &rayon_core::broadcast::broadcast_in<bitz::utils::blake3x4::smallest_pow_nonce_with<10, bitz::utils::blake3x4::first_pow_nonce>::{closure#0}, ()>::{closure#0}, ()> as rayon_core::job::Job>::execute+0x236>
  17fcc3:	lock decq (%rbx)
  17fcc7:	jne    17fd21 <<rayon_core::job::StackJob<rayon_core::latch::LatchRef<rayon_core::latch::CountLatch>, &rayon_core::broadcast::broadcast_in<bitz::utils::blake3x4::smallest_pow_nonce_with<10, bitz::utils::blake3x4::first_pow_nonce>::{closure#0}, ()>::{closure#0}, ()> as rayon_core::job::Job>::execute+0x231>
  17fcc9:	mov    %rbx,%rdi
  17fccc:	call   482b40 <<alloc::sync::Arc<rayon_core::registry::Registry>>::drop_slow>
  17fcd1:	jmp    17fd21 <<rayon_core::job::StackJob<rayon_core::latch::LatchRef<rayon_core::latch::CountLatch>, &rayon_core::broadcast::broadcast_in<bitz::utils::blake3x4::smallest_pow_nonce_with<10, bitz::utils::blake3x4::first_pow_nonce>::{closure#0}, ()>::{closure#0}, ()> as rayon_core::job::Job>::execute+0x231>
  17fcd3:	call   3d1d63 <core::panicking::panic_in_cleanup>
  17fcd8:	cmpq   $0x0,0x8(%r14)
  17fcdd:	je     17fce8 <<rayon_core::job::StackJob<rayon_core::latch::LatchRef<rayon_core::latch::CountLatch>, &rayon_core::broadcast::broadcast_in<bitz::utils::blake3x4::smallest_pow_nonce_with<10, bitz::utils::blake3x4::first_pow_nonce>::{closure#0}, ()>::{closure#0}, ()> as rayon_core::job::Job>::execute+0x1f8>
  17fcdf:	mov    %r12,%rdi
  17fce2:	call   *0x436378(%rip)        # 5b6060 <free@GLIBC_2.2.5>
  17fce8:	mov    0x10(%rsp),%rax
  17fced:	mov    (%rsp),%rcx
  17fcf1:	mov    0x8(%rsp),%rdx
  17fcf6:	mov    %rbx,0x10(%rax)
  17fcfa:	mov    %rcx,0x18(%rax)
  17fcfe:	mov    %rdx,0x20(%rax)
  17fd02:	jmp    17fd21 <<rayon_core::job::StackJob<rayon_core::latch::LatchRef<rayon_core::latch::CountLatch>, &rayon_core::broadcast::broadcast_in<bitz::utils::blake3x4::smallest_pow_nonce_with<10, bitz::utils::blake3x4::first_pow_nonce>::{closure#0}, ()>::{closure#0}, ()> as rayon_core::job::Job>::execute+0x231>
  17fd04:	jmp    17fd06 <<rayon_core::job::StackJob<rayon_core::latch::LatchRef<rayon_core::latch::CountLatch>, &rayon_core::broadcast::broadcast_in<bitz::utils::blake3x4::smallest_pow_nonce_with<10, bitz::utils::blake3x4::first_pow_nonce>::{closure#0}, ()>::{closure#0}, ()> as rayon_core::job::Job>::execute+0x216>
  17fd06:	mov    %rax,%rdi
  17fd09:	call   4c3eaa <std::panicking::catch_unwind::cleanup>
  17fd0e:	mov    $0x2,%ebx
  17fd13:	mov    %rax,(%rsp)
  17fd17:	mov    %rdx,0x8(%rsp)
  17fd1c:	jmp    17fb48 <<rayon_core::job::StackJob<rayon_core::latch::LatchRef<rayon_core::latch::CountLatch>, &rayon_core::broadcast::broadcast_in<bitz::utils::blake3x4::smallest_pow_nonce_with<10, bitz::utils::blake3x4::first_pow_nonce>::{closure#0}, ()>::{closure#0}, ()> as rayon_core::job::Job>::execute+0x58>
  17fd21:	call   486d90 <<rayon_core::unwind::AbortIfPanic as core::ops::drop::Drop>::drop>
  17fd26:	ud2
  17fd28:	call   3d1d63 <core::panicking::panic_in_cleanup>

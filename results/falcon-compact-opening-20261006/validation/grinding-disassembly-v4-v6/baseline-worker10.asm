
/tmp/falcon-compact-opening-binaries/baseline-v4:     file format elf64-x86-64


Disassembly of section .text:

000000000017ea60 <<rayon_core::job::StackJob<rayon_core::latch::LatchRef<rayon_core::latch::CountLatch>, &rayon_core::broadcast::broadcast_in<bitz::utils::blake3x4::smallest_pow_nonce_with<10, bitz::utils::blake3x4::first_pow_nonce>::{closure#0}, ()>::{closure#0}, ()> as rayon_core::job::Job>::execute>:
  17ea60:	push   %rbp
  17ea61:	push   %r15
  17ea63:	push   %r14
  17ea65:	push   %r13
  17ea67:	push   %r12
  17ea69:	push   %rbx
  17ea6a:	sub    $0x18,%rsp
  17ea6e:	mov    0x8(%rdi),%rax
  17ea72:	movq   $0x0,0x8(%rdi)
  17ea7a:	test   %rax,%rax
  17ea7d:	je     17ec0b <<rayon_core::job::StackJob<rayon_core::latch::LatchRef<rayon_core::latch::CountLatch>, &rayon_core::broadcast::broadcast_in<bitz::utils::blake3x4::smallest_pow_nonce_with<10, bitz::utils::blake3x4::first_pow_nonce>::{closure#0}, ()>::{closure#0}, ()> as rayon_core::job::Job>::execute+0x1ab>
  17ea83:	cmpq   $0x0,%fs:0xffffffffffffff60
  17ea8d:	mov    %rdi,0x10(%rsp)
  17ea92:	je     17ec19 <<rayon_core::job::StackJob<rayon_core::latch::LatchRef<rayon_core::latch::CountLatch>, &rayon_core::broadcast::broadcast_in<bitz::utils::blake3x4::smallest_pow_nonce_with<10, bitz::utils::blake3x4::first_pow_nonce>::{closure#0}, ()>::{closure#0}, ()> as rayon_core::job::Job>::execute+0x1b9>
  17ea98:	mov    (%rax),%rbp
  17ea9b:	mov    $0x1,%edx
  17eaa0:	lock xadd %rdx,0x0(%rbp)
  17eaa6:	mov    %rdx,%rcx
  17eaa9:	shr    $0x36,%rcx
  17eaad:	je     17eb5c <<rayon_core::job::StackJob<rayon_core::latch::LatchRef<rayon_core::latch::CountLatch>, &rayon_core::broadcast::broadcast_in<bitz::utils::blake3x4::smallest_pow_nonce_with<10, bitz::utils::blake3x4::first_pow_nonce>::{closure#0}, ()>::{closure#0}, ()> as rayon_core::job::Job>::execute+0xfc>
  17eab3:	mov    $0x1,%ebx
  17eab8:	mov    0x10(%rsp),%r15
  17eabd:	mov    0x8(%rsp),%r13
  17eac2:	mov    (%rsp),%rbp
  17eac6:	cmpl   $0x2,0x10(%r15)
  17eacb:	jb     17eaf2 <<rayon_core::job::StackJob<rayon_core::latch::LatchRef<rayon_core::latch::CountLatch>, &rayon_core::broadcast::broadcast_in<bitz::utils::blake3x4::smallest_pow_nonce_with<10, bitz::utils::blake3x4::first_pow_nonce>::{closure#0}, ()>::{closure#0}, ()> as rayon_core::job::Job>::execute+0x92>
  17eacd:	mov    0x20(%r15),%r14
  17ead1:	mov    0x18(%r15),%r12
  17ead5:	mov    (%r14),%rax
  17ead8:	test   %rax,%rax
  17eadb:	je     17eae2 <<rayon_core::job::StackJob<rayon_core::latch::LatchRef<rayon_core::latch::CountLatch>, &rayon_core::broadcast::broadcast_in<bitz::utils::blake3x4::smallest_pow_nonce_with<10, bitz::utils::blake3x4::first_pow_nonce>::{closure#0}, ()>::{closure#0}, ()> as rayon_core::job::Job>::execute+0x82>
  17eadd:	mov    %r12,%rdi
  17eae0:	call   *%rax
  17eae2:	cmpq   $0x0,0x8(%r14)
  17eae7:	je     17eaf2 <<rayon_core::job::StackJob<rayon_core::latch::LatchRef<rayon_core::latch::CountLatch>, &rayon_core::broadcast::broadcast_in<bitz::utils::blake3x4::smallest_pow_nonce_with<10, bitz::utils::blake3x4::first_pow_nonce>::{closure#0}, ()>::{closure#0}, ()> as rayon_core::job::Job>::execute+0x92>
  17eae9:	mov    %r12,%rdi
  17eaec:	call   *0x43534e(%rip)        # 5b3e40 <free@GLIBC_2.2.5>
  17eaf2:	mov    %rbx,0x10(%r15)
  17eaf6:	mov    %rbp,0x18(%r15)
  17eafa:	mov    %r13,0x20(%r15)
  17eafe:	mov    (%r15),%rdi
  17eb01:	lock decq (%rdi)
  17eb05:	jne    17ebfc <<rayon_core::job::StackJob<rayon_core::latch::LatchRef<rayon_core::latch::CountLatch>, &rayon_core::broadcast::broadcast_in<bitz::utils::blake3x4::smallest_pow_nonce_with<10, bitz::utils::blake3x4::first_pow_nonce>::{closure#0}, ()>::{closure#0}, ()> as rayon_core::job::Job>::execute+0x19c>
  17eb0b:	mov    0x8(%rdi),%rax
  17eb0f:	test   %rax,%rax
  17eb12:	je     17ebf3 <<rayon_core::job::StackJob<rayon_core::latch::LatchRef<rayon_core::latch::CountLatch>, &rayon_core::broadcast::broadcast_in<bitz::utils::blake3x4::smallest_pow_nonce_with<10, bitz::utils::blake3x4::first_pow_nonce>::{closure#0}, ()>::{closure#0}, ()> as rayon_core::job::Job>::execute+0x193>
  17eb18:	mov    0x18(%rdi),%rsi
  17eb1c:	lock incq (%rax)
  17eb20:	jle    17ec96 <<rayon_core::job::StackJob<rayon_core::latch::LatchRef<rayon_core::latch::CountLatch>, &rayon_core::broadcast::broadcast_in<bitz::utils::blake3x4::smallest_pow_nonce_with<10, bitz::utils::blake3x4::first_pow_nonce>::{closure#0}, ()>::{closure#0}, ()> as rayon_core::job::Job>::execute+0x236>
  17eb26:	mov    $0x3,%eax
  17eb2b:	mov    0x8(%rdi),%rbx
  17eb2f:	xchg   %rax,0x10(%rdi)
  17eb33:	cmp    $0x2,%rax
  17eb37:	jne    17eb45 <<rayon_core::job::StackJob<rayon_core::latch::LatchRef<rayon_core::latch::CountLatch>, &rayon_core::broadcast::broadcast_in<bitz::utils::blake3x4::smallest_pow_nonce_with<10, bitz::utils::blake3x4::first_pow_nonce>::{closure#0}, ()>::{closure#0}, ()> as rayon_core::job::Job>::execute+0xe5>
  17eb39:	lea    0x1d8(%rbx),%rdi
  17eb40:	call   480450 <<rayon_core::sleep::Sleep>::wake_specific_thread>
  17eb45:	lock decq (%rbx)
  17eb49:	jne    17ebfc <<rayon_core::job::StackJob<rayon_core::latch::LatchRef<rayon_core::latch::CountLatch>, &rayon_core::broadcast::broadcast_in<bitz::utils::blake3x4::smallest_pow_nonce_with<10, bitz::utils::blake3x4::first_pow_nonce>::{closure#0}, ()>::{closure#0}, ()> as rayon_core::job::Job>::execute+0x19c>
  17eb4f:	mov    %rbx,%rdi
  17eb52:	call   480d30 <<alloc::sync::Arc<rayon_core::registry::Registry>>::drop_slow>
  17eb57:	jmp    17ebfc <<rayon_core::job::StackJob<rayon_core::latch::LatchRef<rayon_core::latch::CountLatch>, &rayon_core::broadcast::broadcast_in<bitz::utils::blake3x4::smallest_pow_nonce_with<10, bitz::utils::blake3x4::first_pow_nonce>::{closure#0}, ()>::{closure#0}, ()> as rayon_core::job::Job>::execute+0x19c>
  17eb5c:	mov    0x8(%rax),%r15
  17eb60:	mov    0x18(%rax),%r12
  17eb64:	mov    0x20(%rax),%r13
  17eb68:	mov    0x28(%rax),%r14
  17eb6c:	mov    $0x1,%ebx
  17eb71:	jmp    17eb9e <<rayon_core::job::StackJob<rayon_core::latch::LatchRef<rayon_core::latch::CountLatch>, &rayon_core::broadcast::broadcast_in<bitz::utils::blake3x4::smallest_pow_nonce_with<10, bitz::utils::blake3x4::first_pow_nonce>::{closure#0}, ()>::{closure#0}, ()> as rayon_core::job::Job>::execute+0x13e>
  17eb73:	data16 data16 data16 cs nopw 0x0(%rax,%rax,1)
  17eb80:	mov    $0x1,%edx
  17eb85:	movabs $0x3fffffffffffff,%rax
  17eb8f:	lock xadd %rdx,0x0(%rbp)
  17eb95:	cmp    %rax,%rdx
  17eb98:	ja     17eab8 <<rayon_core::job::StackJob<rayon_core::latch::LatchRef<rayon_core::latch::CountLatch>, &rayon_core::broadcast::broadcast_in<bitz::utils::blake3x4::smallest_pow_nonce_with<10, bitz::utils::blake3x4::first_pow_nonce>::{closure#0}, ()>::{closure#0}, ()> as rayon_core::job::Job>::execute+0x58>
  17eb9e:	mov    (%r15),%rax
  17eba1:	shl    $0xa,%rdx
  17eba5:	cmp    %rax,%rdx
  17eba8:	jae    17eab3 <<rayon_core::job::StackJob<rayon_core::latch::LatchRef<rayon_core::latch::CountLatch>, &rayon_core::broadcast::broadcast_in<bitz::utils::blake3x4::smallest_pow_nonce_with<10, bitz::utils::blake3x4::first_pow_nonce>::{closure#0}, ()>::{closure#0}, ()> as rayon_core::job::Job>::execute+0x53>
  17ebae:	mov    (%r14),%r8d
  17ebb1:	mov    %rdx,%rcx
  17ebb4:	mov    $0xffffffffffffffff,%rax
  17ebbb:	add    $0x400,%rcx
  17ebc2:	cmovb  %rax,%rcx
  17ebc6:	mov    %r12,%rdi
  17ebc9:	mov    %r13,%rsi
  17ebcc:	call   17f370 <bitz::utils::blake3x4::first_pow_nonce>
  17ebd1:	cmp    $0x1,%rax
  17ebd5:	jne    17eb80 <<rayon_core::job::StackJob<rayon_core::latch::LatchRef<rayon_core::latch::CountLatch>, &rayon_core::broadcast::broadcast_in<bitz::utils::blake3x4::smallest_pow_nonce_with<10, bitz::utils::blake3x4::first_pow_nonce>::{closure#0}, ()>::{closure#0}, ()> as rayon_core::job::Job>::execute+0x120>
  17ebd7:	mov    (%r15),%rax
  17ebda:	nopw   0x0(%rax,%rax,1)
  17ebe0:	cmp    %rdx,%rax
  17ebe3:	mov    %rdx,%rcx
  17ebe6:	cmovbe %rax,%rcx
  17ebea:	lock cmpxchg %rcx,(%r15)
  17ebef:	jne    17ebe0 <<rayon_core::job::StackJob<rayon_core::latch::LatchRef<rayon_core::latch::CountLatch>, &rayon_core::broadcast::broadcast_in<bitz::utils::blake3x4::smallest_pow_nonce_with<10, bitz::utils::blake3x4::first_pow_nonce>::{closure#0}, ()>::{closure#0}, ()> as rayon_core::job::Job>::execute+0x180>
  17ebf1:	jmp    17eb80 <<rayon_core::job::StackJob<rayon_core::latch::LatchRef<rayon_core::latch::CountLatch>, &rayon_core::broadcast::broadcast_in<bitz::utils::blake3x4::smallest_pow_nonce_with<10, bitz::utils::blake3x4::first_pow_nonce>::{closure#0}, ()>::{closure#0}, ()> as rayon_core::job::Job>::execute+0x120>
  17ebf3:	add    $0x10,%rdi
  17ebf7:	call   17da40 <<rayon_core::latch::LockLatch as rayon_core::latch::Latch>::set>
  17ebfc:	add    $0x18,%rsp
  17ec00:	pop    %rbx
  17ec01:	pop    %r12
  17ec03:	pop    %r13
  17ec05:	pop    %r14
  17ec07:	pop    %r15
  17ec09:	pop    %rbp
  17ec0a:	ret
  17ec0b:	lea    0x42ce56(%rip),%rdi        # 5aba68 <<bitz::piop::spartan::falcon1024_ct::hybrid_sumcheck::State<_> as bitz::sumcheck::inner::input::State<field::binary::Gf128Ops>>::fold::__CALLSITE::META+0x42f8>
  17ec12:	call   3d0ae0 <core::option::unwrap_failed>
  17ec17:	jmp    17ec96 <<rayon_core::job::StackJob<rayon_core::latch::LatchRef<rayon_core::latch::CountLatch>, &rayon_core::broadcast::broadcast_in<bitz::utils::blake3x4::smallest_pow_nonce_with<10, bitz::utils::blake3x4::first_pow_nonce>::{closure#0}, ()>::{closure#0}, ()> as rayon_core::job::Job>::execute+0x236>
  17ec19:	lea    -0x111f64(%rip),%rdi        # 6ccbc <perfetto::base::(anonymous namespace)::kHexmap+0x105dc>
  17ec20:	lea    0x420d01(%rip),%rdx        # 59f928 <falcon_hybrid::main::__CALLSITE::META+0xc90>
  17ec27:	mov    $0x2a,%esi
  17ec2c:	call   3ce5b0 <core::panicking::panic>
  17ec31:	jmp    17ec96 <<rayon_core::job::StackJob<rayon_core::latch::LatchRef<rayon_core::latch::CountLatch>, &rayon_core::broadcast::broadcast_in<bitz::utils::blake3x4::smallest_pow_nonce_with<10, bitz::utils::blake3x4::first_pow_nonce>::{closure#0}, ()>::{closure#0}, ()> as rayon_core::job::Job>::execute+0x236>
  17ec33:	lock decq (%rbx)
  17ec37:	jne    17ec91 <<rayon_core::job::StackJob<rayon_core::latch::LatchRef<rayon_core::latch::CountLatch>, &rayon_core::broadcast::broadcast_in<bitz::utils::blake3x4::smallest_pow_nonce_with<10, bitz::utils::blake3x4::first_pow_nonce>::{closure#0}, ()>::{closure#0}, ()> as rayon_core::job::Job>::execute+0x231>
  17ec39:	mov    %rbx,%rdi
  17ec3c:	call   480d30 <<alloc::sync::Arc<rayon_core::registry::Registry>>::drop_slow>
  17ec41:	jmp    17ec91 <<rayon_core::job::StackJob<rayon_core::latch::LatchRef<rayon_core::latch::CountLatch>, &rayon_core::broadcast::broadcast_in<bitz::utils::blake3x4::smallest_pow_nonce_with<10, bitz::utils::blake3x4::first_pow_nonce>::{closure#0}, ()>::{closure#0}, ()> as rayon_core::job::Job>::execute+0x231>
  17ec43:	call   3d0ca3 <core::panicking::panic_in_cleanup>
  17ec48:	cmpq   $0x0,0x8(%r14)
  17ec4d:	je     17ec58 <<rayon_core::job::StackJob<rayon_core::latch::LatchRef<rayon_core::latch::CountLatch>, &rayon_core::broadcast::broadcast_in<bitz::utils::blake3x4::smallest_pow_nonce_with<10, bitz::utils::blake3x4::first_pow_nonce>::{closure#0}, ()>::{closure#0}, ()> as rayon_core::job::Job>::execute+0x1f8>
  17ec4f:	mov    %r12,%rdi
  17ec52:	call   *0x4351e8(%rip)        # 5b3e40 <free@GLIBC_2.2.5>
  17ec58:	mov    0x10(%rsp),%rax
  17ec5d:	mov    (%rsp),%rcx
  17ec61:	mov    0x8(%rsp),%rdx
  17ec66:	mov    %rbx,0x10(%rax)
  17ec6a:	mov    %rcx,0x18(%rax)
  17ec6e:	mov    %rdx,0x20(%rax)
  17ec72:	jmp    17ec91 <<rayon_core::job::StackJob<rayon_core::latch::LatchRef<rayon_core::latch::CountLatch>, &rayon_core::broadcast::broadcast_in<bitz::utils::blake3x4::smallest_pow_nonce_with<10, bitz::utils::blake3x4::first_pow_nonce>::{closure#0}, ()>::{closure#0}, ()> as rayon_core::job::Job>::execute+0x231>
  17ec74:	jmp    17ec76 <<rayon_core::job::StackJob<rayon_core::latch::LatchRef<rayon_core::latch::CountLatch>, &rayon_core::broadcast::broadcast_in<bitz::utils::blake3x4::smallest_pow_nonce_with<10, bitz::utils::blake3x4::first_pow_nonce>::{closure#0}, ()>::{closure#0}, ()> as rayon_core::job::Job>::execute+0x216>
  17ec76:	mov    %rax,%rdi
  17ec79:	call   4c209a <std::panicking::catch_unwind::cleanup>
  17ec7e:	mov    $0x2,%ebx
  17ec83:	mov    %rax,(%rsp)
  17ec87:	mov    %rdx,0x8(%rsp)
  17ec8c:	jmp    17eab8 <<rayon_core::job::StackJob<rayon_core::latch::LatchRef<rayon_core::latch::CountLatch>, &rayon_core::broadcast::broadcast_in<bitz::utils::blake3x4::smallest_pow_nonce_with<10, bitz::utils::blake3x4::first_pow_nonce>::{closure#0}, ()>::{closure#0}, ()> as rayon_core::job::Job>::execute+0x58>
  17ec91:	call   484f80 <<rayon_core::unwind::AbortIfPanic as core::ops::drop::Drop>::drop>
  17ec96:	ud2
  17ec98:	call   3d0ca3 <core::panicking::panic_in_cleanup>

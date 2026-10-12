
/tmp/falcon-compact-opening-binaries/candidate-v6:     file format elf64-x86-64


Disassembly of section .text:

0000000000180070 <<rayon_core::job::StackJob<rayon_core::latch::LatchRef<rayon_core::latch::CountLatch>, &rayon_core::broadcast::broadcast_in<bitz::utils::blake3x4::smallest_pow_nonce_with<8, bitz::utils::blake3x4::first_pow_nonce>::{closure#0}, ()>::{closure#0}, ()> as rayon_core::job::Job>::execute>:
  180070:	push   %rbp
  180071:	push   %r15
  180073:	push   %r14
  180075:	push   %r13
  180077:	push   %r12
  180079:	push   %rbx
  18007a:	sub    $0x18,%rsp
  18007e:	mov    0x8(%rdi),%rax
  180082:	movq   $0x0,0x8(%rdi)
  18008a:	test   %rax,%rax
  18008d:	je     18021b <<rayon_core::job::StackJob<rayon_core::latch::LatchRef<rayon_core::latch::CountLatch>, &rayon_core::broadcast::broadcast_in<bitz::utils::blake3x4::smallest_pow_nonce_with<8, bitz::utils::blake3x4::first_pow_nonce>::{closure#0}, ()>::{closure#0}, ()> as rayon_core::job::Job>::execute+0x1ab>
  180093:	cmpq   $0x0,%fs:0xffffffffffffff60
  18009d:	mov    %rdi,0x10(%rsp)
  1800a2:	je     180229 <<rayon_core::job::StackJob<rayon_core::latch::LatchRef<rayon_core::latch::CountLatch>, &rayon_core::broadcast::broadcast_in<bitz::utils::blake3x4::smallest_pow_nonce_with<8, bitz::utils::blake3x4::first_pow_nonce>::{closure#0}, ()>::{closure#0}, ()> as rayon_core::job::Job>::execute+0x1b9>
  1800a8:	mov    (%rax),%rbp
  1800ab:	mov    $0x1,%edx
  1800b0:	lock xadd %rdx,0x0(%rbp)
  1800b6:	mov    %rdx,%rcx
  1800b9:	shr    $0x38,%rcx
  1800bd:	je     18016c <<rayon_core::job::StackJob<rayon_core::latch::LatchRef<rayon_core::latch::CountLatch>, &rayon_core::broadcast::broadcast_in<bitz::utils::blake3x4::smallest_pow_nonce_with<8, bitz::utils::blake3x4::first_pow_nonce>::{closure#0}, ()>::{closure#0}, ()> as rayon_core::job::Job>::execute+0xfc>
  1800c3:	mov    $0x1,%ebx
  1800c8:	mov    0x10(%rsp),%r15
  1800cd:	mov    0x8(%rsp),%r13
  1800d2:	mov    (%rsp),%rbp
  1800d6:	cmpl   $0x2,0x10(%r15)
  1800db:	jb     180102 <<rayon_core::job::StackJob<rayon_core::latch::LatchRef<rayon_core::latch::CountLatch>, &rayon_core::broadcast::broadcast_in<bitz::utils::blake3x4::smallest_pow_nonce_with<8, bitz::utils::blake3x4::first_pow_nonce>::{closure#0}, ()>::{closure#0}, ()> as rayon_core::job::Job>::execute+0x92>
  1800dd:	mov    0x20(%r15),%r14
  1800e1:	mov    0x18(%r15),%r12
  1800e5:	mov    (%r14),%rax
  1800e8:	test   %rax,%rax
  1800eb:	je     1800f2 <<rayon_core::job::StackJob<rayon_core::latch::LatchRef<rayon_core::latch::CountLatch>, &rayon_core::broadcast::broadcast_in<bitz::utils::blake3x4::smallest_pow_nonce_with<8, bitz::utils::blake3x4::first_pow_nonce>::{closure#0}, ()>::{closure#0}, ()> as rayon_core::job::Job>::execute+0x82>
  1800ed:	mov    %r12,%rdi
  1800f0:	call   *%rax
  1800f2:	cmpq   $0x0,0x8(%r14)
  1800f7:	je     180102 <<rayon_core::job::StackJob<rayon_core::latch::LatchRef<rayon_core::latch::CountLatch>, &rayon_core::broadcast::broadcast_in<bitz::utils::blake3x4::smallest_pow_nonce_with<8, bitz::utils::blake3x4::first_pow_nonce>::{closure#0}, ()>::{closure#0}, ()> as rayon_core::job::Job>::execute+0x92>
  1800f9:	mov    %r12,%rdi
  1800fc:	call   *0x435f5e(%rip)        # 5b6060 <free@GLIBC_2.2.5>
  180102:	mov    %rbx,0x10(%r15)
  180106:	mov    %rbp,0x18(%r15)
  18010a:	mov    %r13,0x20(%r15)
  18010e:	mov    (%r15),%rdi
  180111:	lock decq (%rdi)
  180115:	jne    18020c <<rayon_core::job::StackJob<rayon_core::latch::LatchRef<rayon_core::latch::CountLatch>, &rayon_core::broadcast::broadcast_in<bitz::utils::blake3x4::smallest_pow_nonce_with<8, bitz::utils::blake3x4::first_pow_nonce>::{closure#0}, ()>::{closure#0}, ()> as rayon_core::job::Job>::execute+0x19c>
  18011b:	mov    0x8(%rdi),%rax
  18011f:	test   %rax,%rax
  180122:	je     180203 <<rayon_core::job::StackJob<rayon_core::latch::LatchRef<rayon_core::latch::CountLatch>, &rayon_core::broadcast::broadcast_in<bitz::utils::blake3x4::smallest_pow_nonce_with<8, bitz::utils::blake3x4::first_pow_nonce>::{closure#0}, ()>::{closure#0}, ()> as rayon_core::job::Job>::execute+0x193>
  180128:	mov    0x18(%rdi),%rsi
  18012c:	lock incq (%rax)
  180130:	jle    1802a6 <<rayon_core::job::StackJob<rayon_core::latch::LatchRef<rayon_core::latch::CountLatch>, &rayon_core::broadcast::broadcast_in<bitz::utils::blake3x4::smallest_pow_nonce_with<8, bitz::utils::blake3x4::first_pow_nonce>::{closure#0}, ()>::{closure#0}, ()> as rayon_core::job::Job>::execute+0x236>
  180136:	mov    $0x3,%eax
  18013b:	mov    0x8(%rdi),%rbx
  18013f:	xchg   %rax,0x10(%rdi)
  180143:	cmp    $0x2,%rax
  180147:	jne    180155 <<rayon_core::job::StackJob<rayon_core::latch::LatchRef<rayon_core::latch::CountLatch>, &rayon_core::broadcast::broadcast_in<bitz::utils::blake3x4::smallest_pow_nonce_with<8, bitz::utils::blake3x4::first_pow_nonce>::{closure#0}, ()>::{closure#0}, ()> as rayon_core::job::Job>::execute+0xe5>
  180149:	lea    0x1d8(%rbx),%rdi
  180150:	call   481e60 <<rayon_core::sleep::Sleep>::wake_specific_thread>
  180155:	lock decq (%rbx)
  180159:	jne    18020c <<rayon_core::job::StackJob<rayon_core::latch::LatchRef<rayon_core::latch::CountLatch>, &rayon_core::broadcast::broadcast_in<bitz::utils::blake3x4::smallest_pow_nonce_with<8, bitz::utils::blake3x4::first_pow_nonce>::{closure#0}, ()>::{closure#0}, ()> as rayon_core::job::Job>::execute+0x19c>
  18015f:	mov    %rbx,%rdi
  180162:	call   482b40 <<alloc::sync::Arc<rayon_core::registry::Registry>>::drop_slow>
  180167:	jmp    18020c <<rayon_core::job::StackJob<rayon_core::latch::LatchRef<rayon_core::latch::CountLatch>, &rayon_core::broadcast::broadcast_in<bitz::utils::blake3x4::smallest_pow_nonce_with<8, bitz::utils::blake3x4::first_pow_nonce>::{closure#0}, ()>::{closure#0}, ()> as rayon_core::job::Job>::execute+0x19c>
  18016c:	mov    0x8(%rax),%r15
  180170:	mov    0x18(%rax),%r12
  180174:	mov    0x20(%rax),%r13
  180178:	mov    0x28(%rax),%r14
  18017c:	mov    $0x1,%ebx
  180181:	jmp    1801ae <<rayon_core::job::StackJob<rayon_core::latch::LatchRef<rayon_core::latch::CountLatch>, &rayon_core::broadcast::broadcast_in<bitz::utils::blake3x4::smallest_pow_nonce_with<8, bitz::utils::blake3x4::first_pow_nonce>::{closure#0}, ()>::{closure#0}, ()> as rayon_core::job::Job>::execute+0x13e>
  180183:	data16 data16 data16 cs nopw 0x0(%rax,%rax,1)
  180190:	mov    $0x1,%edx
  180195:	movabs $0xffffffffffffff,%rax
  18019f:	lock xadd %rdx,0x0(%rbp)
  1801a5:	cmp    %rax,%rdx
  1801a8:	ja     1800c8 <<rayon_core::job::StackJob<rayon_core::latch::LatchRef<rayon_core::latch::CountLatch>, &rayon_core::broadcast::broadcast_in<bitz::utils::blake3x4::smallest_pow_nonce_with<8, bitz::utils::blake3x4::first_pow_nonce>::{closure#0}, ()>::{closure#0}, ()> as rayon_core::job::Job>::execute+0x58>
  1801ae:	mov    (%r15),%rax
  1801b1:	shl    $0x8,%rdx
  1801b5:	cmp    %rax,%rdx
  1801b8:	jae    1800c3 <<rayon_core::job::StackJob<rayon_core::latch::LatchRef<rayon_core::latch::CountLatch>, &rayon_core::broadcast::broadcast_in<bitz::utils::blake3x4::smallest_pow_nonce_with<8, bitz::utils::blake3x4::first_pow_nonce>::{closure#0}, ()>::{closure#0}, ()> as rayon_core::job::Job>::execute+0x53>
  1801be:	mov    (%r14),%r8d
  1801c1:	mov    %rdx,%rcx
  1801c4:	mov    $0xffffffffffffffff,%rax
  1801cb:	add    $0x100,%rcx
  1801d2:	cmovb  %rax,%rcx
  1801d6:	mov    %r12,%rdi
  1801d9:	mov    %r13,%rsi
  1801dc:	call   180400 <bitz::utils::blake3x4::first_pow_nonce>
  1801e1:	cmp    $0x1,%rax
  1801e5:	jne    180190 <<rayon_core::job::StackJob<rayon_core::latch::LatchRef<rayon_core::latch::CountLatch>, &rayon_core::broadcast::broadcast_in<bitz::utils::blake3x4::smallest_pow_nonce_with<8, bitz::utils::blake3x4::first_pow_nonce>::{closure#0}, ()>::{closure#0}, ()> as rayon_core::job::Job>::execute+0x120>
  1801e7:	mov    (%r15),%rax
  1801ea:	nopw   0x0(%rax,%rax,1)
  1801f0:	cmp    %rdx,%rax
  1801f3:	mov    %rdx,%rcx
  1801f6:	cmovbe %rax,%rcx
  1801fa:	lock cmpxchg %rcx,(%r15)
  1801ff:	jne    1801f0 <<rayon_core::job::StackJob<rayon_core::latch::LatchRef<rayon_core::latch::CountLatch>, &rayon_core::broadcast::broadcast_in<bitz::utils::blake3x4::smallest_pow_nonce_with<8, bitz::utils::blake3x4::first_pow_nonce>::{closure#0}, ()>::{closure#0}, ()> as rayon_core::job::Job>::execute+0x180>
  180201:	jmp    180190 <<rayon_core::job::StackJob<rayon_core::latch::LatchRef<rayon_core::latch::CountLatch>, &rayon_core::broadcast::broadcast_in<bitz::utils::blake3x4::smallest_pow_nonce_with<8, bitz::utils::blake3x4::first_pow_nonce>::{closure#0}, ()>::{closure#0}, ()> as rayon_core::job::Job>::execute+0x120>
  180203:	add    $0x10,%rdi
  180207:	call   17ead0 <<rayon_core::latch::LockLatch as rayon_core::latch::Latch>::set>
  18020c:	add    $0x18,%rsp
  180210:	pop    %rbx
  180211:	pop    %r12
  180213:	pop    %r13
  180215:	pop    %r14
  180217:	pop    %r15
  180219:	pop    %rbp
  18021a:	ret
  18021b:	lea    0x42db0e(%rip),%rdi        # 5add30 <<bitz::piop::spartan::falcon1024_ct::hybrid_sumcheck::State<_> as bitz::sumcheck::inner::input::State<field::binary::Gf128Ops>>::fold::__CALLSITE::META+0x4380>
  180222:	call   3d1ba0 <core::option::unwrap_failed>
  180227:	jmp    1802a6 <<rayon_core::job::StackJob<rayon_core::latch::LatchRef<rayon_core::latch::CountLatch>, &rayon_core::broadcast::broadcast_in<bitz::utils::blake3x4::smallest_pow_nonce_with<8, bitz::utils::blake3x4::first_pow_nonce>::{closure#0}, ()>::{closure#0}, ()> as rayon_core::job::Job>::execute+0x236>
  180229:	lea    -0x113610(%rip),%rdi        # 6cc20 <perfetto::base::(anonymous namespace)::kHexmap+0x105c0>
  180230:	lea    0x421819(%rip),%rdx        # 5a1a50 <falcon_hybrid::main::__CALLSITE::META+0xca0>
  180237:	mov    $0x2a,%esi
  18023c:	call   3cf670 <core::panicking::panic>
  180241:	jmp    1802a6 <<rayon_core::job::StackJob<rayon_core::latch::LatchRef<rayon_core::latch::CountLatch>, &rayon_core::broadcast::broadcast_in<bitz::utils::blake3x4::smallest_pow_nonce_with<8, bitz::utils::blake3x4::first_pow_nonce>::{closure#0}, ()>::{closure#0}, ()> as rayon_core::job::Job>::execute+0x236>
  180243:	lock decq (%rbx)
  180247:	jne    1802a1 <<rayon_core::job::StackJob<rayon_core::latch::LatchRef<rayon_core::latch::CountLatch>, &rayon_core::broadcast::broadcast_in<bitz::utils::blake3x4::smallest_pow_nonce_with<8, bitz::utils::blake3x4::first_pow_nonce>::{closure#0}, ()>::{closure#0}, ()> as rayon_core::job::Job>::execute+0x231>
  180249:	mov    %rbx,%rdi
  18024c:	call   482b40 <<alloc::sync::Arc<rayon_core::registry::Registry>>::drop_slow>
  180251:	jmp    1802a1 <<rayon_core::job::StackJob<rayon_core::latch::LatchRef<rayon_core::latch::CountLatch>, &rayon_core::broadcast::broadcast_in<bitz::utils::blake3x4::smallest_pow_nonce_with<8, bitz::utils::blake3x4::first_pow_nonce>::{closure#0}, ()>::{closure#0}, ()> as rayon_core::job::Job>::execute+0x231>
  180253:	call   3d1d63 <core::panicking::panic_in_cleanup>
  180258:	cmpq   $0x0,0x8(%r14)
  18025d:	je     180268 <<rayon_core::job::StackJob<rayon_core::latch::LatchRef<rayon_core::latch::CountLatch>, &rayon_core::broadcast::broadcast_in<bitz::utils::blake3x4::smallest_pow_nonce_with<8, bitz::utils::blake3x4::first_pow_nonce>::{closure#0}, ()>::{closure#0}, ()> as rayon_core::job::Job>::execute+0x1f8>
  18025f:	mov    %r12,%rdi
  180262:	call   *0x435df8(%rip)        # 5b6060 <free@GLIBC_2.2.5>
  180268:	mov    0x10(%rsp),%rax
  18026d:	mov    (%rsp),%rcx
  180271:	mov    0x8(%rsp),%rdx
  180276:	mov    %rbx,0x10(%rax)
  18027a:	mov    %rcx,0x18(%rax)
  18027e:	mov    %rdx,0x20(%rax)
  180282:	jmp    1802a1 <<rayon_core::job::StackJob<rayon_core::latch::LatchRef<rayon_core::latch::CountLatch>, &rayon_core::broadcast::broadcast_in<bitz::utils::blake3x4::smallest_pow_nonce_with<8, bitz::utils::blake3x4::first_pow_nonce>::{closure#0}, ()>::{closure#0}, ()> as rayon_core::job::Job>::execute+0x231>
  180284:	jmp    180286 <<rayon_core::job::StackJob<rayon_core::latch::LatchRef<rayon_core::latch::CountLatch>, &rayon_core::broadcast::broadcast_in<bitz::utils::blake3x4::smallest_pow_nonce_with<8, bitz::utils::blake3x4::first_pow_nonce>::{closure#0}, ()>::{closure#0}, ()> as rayon_core::job::Job>::execute+0x216>
  180286:	mov    %rax,%rdi
  180289:	call   4c3eaa <std::panicking::catch_unwind::cleanup>
  18028e:	mov    $0x2,%ebx
  180293:	mov    %rax,(%rsp)
  180297:	mov    %rdx,0x8(%rsp)
  18029c:	jmp    1800c8 <<rayon_core::job::StackJob<rayon_core::latch::LatchRef<rayon_core::latch::CountLatch>, &rayon_core::broadcast::broadcast_in<bitz::utils::blake3x4::smallest_pow_nonce_with<8, bitz::utils::blake3x4::first_pow_nonce>::{closure#0}, ()>::{closure#0}, ()> as rayon_core::job::Job>::execute+0x58>
  1802a1:	call   486d90 <<rayon_core::unwind::AbortIfPanic as core::ops::drop::Drop>::drop>
  1802a6:	ud2
  1802a8:	call   3d1d63 <core::panicking::panic_in_cleanup>

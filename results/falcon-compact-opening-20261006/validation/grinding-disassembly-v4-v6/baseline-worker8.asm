
/tmp/falcon-compact-opening-binaries/baseline-v4:     file format elf64-x86-64


Disassembly of section .text:

000000000017efe0 <<rayon_core::job::StackJob<rayon_core::latch::LatchRef<rayon_core::latch::CountLatch>, &rayon_core::broadcast::broadcast_in<bitz::utils::blake3x4::smallest_pow_nonce_with<8, bitz::utils::blake3x4::first_pow_nonce>::{closure#0}, ()>::{closure#0}, ()> as rayon_core::job::Job>::execute>:
  17efe0:	push   %rbp
  17efe1:	push   %r15
  17efe3:	push   %r14
  17efe5:	push   %r13
  17efe7:	push   %r12
  17efe9:	push   %rbx
  17efea:	sub    $0x18,%rsp
  17efee:	mov    0x8(%rdi),%rax
  17eff2:	movq   $0x0,0x8(%rdi)
  17effa:	test   %rax,%rax
  17effd:	je     17f18b <<rayon_core::job::StackJob<rayon_core::latch::LatchRef<rayon_core::latch::CountLatch>, &rayon_core::broadcast::broadcast_in<bitz::utils::blake3x4::smallest_pow_nonce_with<8, bitz::utils::blake3x4::first_pow_nonce>::{closure#0}, ()>::{closure#0}, ()> as rayon_core::job::Job>::execute+0x1ab>
  17f003:	cmpq   $0x0,%fs:0xffffffffffffff60
  17f00d:	mov    %rdi,0x10(%rsp)
  17f012:	je     17f199 <<rayon_core::job::StackJob<rayon_core::latch::LatchRef<rayon_core::latch::CountLatch>, &rayon_core::broadcast::broadcast_in<bitz::utils::blake3x4::smallest_pow_nonce_with<8, bitz::utils::blake3x4::first_pow_nonce>::{closure#0}, ()>::{closure#0}, ()> as rayon_core::job::Job>::execute+0x1b9>
  17f018:	mov    (%rax),%rbp
  17f01b:	mov    $0x1,%edx
  17f020:	lock xadd %rdx,0x0(%rbp)
  17f026:	mov    %rdx,%rcx
  17f029:	shr    $0x38,%rcx
  17f02d:	je     17f0dc <<rayon_core::job::StackJob<rayon_core::latch::LatchRef<rayon_core::latch::CountLatch>, &rayon_core::broadcast::broadcast_in<bitz::utils::blake3x4::smallest_pow_nonce_with<8, bitz::utils::blake3x4::first_pow_nonce>::{closure#0}, ()>::{closure#0}, ()> as rayon_core::job::Job>::execute+0xfc>
  17f033:	mov    $0x1,%ebx
  17f038:	mov    0x10(%rsp),%r15
  17f03d:	mov    0x8(%rsp),%r13
  17f042:	mov    (%rsp),%rbp
  17f046:	cmpl   $0x2,0x10(%r15)
  17f04b:	jb     17f072 <<rayon_core::job::StackJob<rayon_core::latch::LatchRef<rayon_core::latch::CountLatch>, &rayon_core::broadcast::broadcast_in<bitz::utils::blake3x4::smallest_pow_nonce_with<8, bitz::utils::blake3x4::first_pow_nonce>::{closure#0}, ()>::{closure#0}, ()> as rayon_core::job::Job>::execute+0x92>
  17f04d:	mov    0x20(%r15),%r14
  17f051:	mov    0x18(%r15),%r12
  17f055:	mov    (%r14),%rax
  17f058:	test   %rax,%rax
  17f05b:	je     17f062 <<rayon_core::job::StackJob<rayon_core::latch::LatchRef<rayon_core::latch::CountLatch>, &rayon_core::broadcast::broadcast_in<bitz::utils::blake3x4::smallest_pow_nonce_with<8, bitz::utils::blake3x4::first_pow_nonce>::{closure#0}, ()>::{closure#0}, ()> as rayon_core::job::Job>::execute+0x82>
  17f05d:	mov    %r12,%rdi
  17f060:	call   *%rax
  17f062:	cmpq   $0x0,0x8(%r14)
  17f067:	je     17f072 <<rayon_core::job::StackJob<rayon_core::latch::LatchRef<rayon_core::latch::CountLatch>, &rayon_core::broadcast::broadcast_in<bitz::utils::blake3x4::smallest_pow_nonce_with<8, bitz::utils::blake3x4::first_pow_nonce>::{closure#0}, ()>::{closure#0}, ()> as rayon_core::job::Job>::execute+0x92>
  17f069:	mov    %r12,%rdi
  17f06c:	call   *0x434dce(%rip)        # 5b3e40 <free@GLIBC_2.2.5>
  17f072:	mov    %rbx,0x10(%r15)
  17f076:	mov    %rbp,0x18(%r15)
  17f07a:	mov    %r13,0x20(%r15)
  17f07e:	mov    (%r15),%rdi
  17f081:	lock decq (%rdi)
  17f085:	jne    17f17c <<rayon_core::job::StackJob<rayon_core::latch::LatchRef<rayon_core::latch::CountLatch>, &rayon_core::broadcast::broadcast_in<bitz::utils::blake3x4::smallest_pow_nonce_with<8, bitz::utils::blake3x4::first_pow_nonce>::{closure#0}, ()>::{closure#0}, ()> as rayon_core::job::Job>::execute+0x19c>
  17f08b:	mov    0x8(%rdi),%rax
  17f08f:	test   %rax,%rax
  17f092:	je     17f173 <<rayon_core::job::StackJob<rayon_core::latch::LatchRef<rayon_core::latch::CountLatch>, &rayon_core::broadcast::broadcast_in<bitz::utils::blake3x4::smallest_pow_nonce_with<8, bitz::utils::blake3x4::first_pow_nonce>::{closure#0}, ()>::{closure#0}, ()> as rayon_core::job::Job>::execute+0x193>
  17f098:	mov    0x18(%rdi),%rsi
  17f09c:	lock incq (%rax)
  17f0a0:	jle    17f216 <<rayon_core::job::StackJob<rayon_core::latch::LatchRef<rayon_core::latch::CountLatch>, &rayon_core::broadcast::broadcast_in<bitz::utils::blake3x4::smallest_pow_nonce_with<8, bitz::utils::blake3x4::first_pow_nonce>::{closure#0}, ()>::{closure#0}, ()> as rayon_core::job::Job>::execute+0x236>
  17f0a6:	mov    $0x3,%eax
  17f0ab:	mov    0x8(%rdi),%rbx
  17f0af:	xchg   %rax,0x10(%rdi)
  17f0b3:	cmp    $0x2,%rax
  17f0b7:	jne    17f0c5 <<rayon_core::job::StackJob<rayon_core::latch::LatchRef<rayon_core::latch::CountLatch>, &rayon_core::broadcast::broadcast_in<bitz::utils::blake3x4::smallest_pow_nonce_with<8, bitz::utils::blake3x4::first_pow_nonce>::{closure#0}, ()>::{closure#0}, ()> as rayon_core::job::Job>::execute+0xe5>
  17f0b9:	lea    0x1d8(%rbx),%rdi
  17f0c0:	call   480450 <<rayon_core::sleep::Sleep>::wake_specific_thread>
  17f0c5:	lock decq (%rbx)
  17f0c9:	jne    17f17c <<rayon_core::job::StackJob<rayon_core::latch::LatchRef<rayon_core::latch::CountLatch>, &rayon_core::broadcast::broadcast_in<bitz::utils::blake3x4::smallest_pow_nonce_with<8, bitz::utils::blake3x4::first_pow_nonce>::{closure#0}, ()>::{closure#0}, ()> as rayon_core::job::Job>::execute+0x19c>
  17f0cf:	mov    %rbx,%rdi
  17f0d2:	call   480d30 <<alloc::sync::Arc<rayon_core::registry::Registry>>::drop_slow>
  17f0d7:	jmp    17f17c <<rayon_core::job::StackJob<rayon_core::latch::LatchRef<rayon_core::latch::CountLatch>, &rayon_core::broadcast::broadcast_in<bitz::utils::blake3x4::smallest_pow_nonce_with<8, bitz::utils::blake3x4::first_pow_nonce>::{closure#0}, ()>::{closure#0}, ()> as rayon_core::job::Job>::execute+0x19c>
  17f0dc:	mov    0x8(%rax),%r15
  17f0e0:	mov    0x18(%rax),%r12
  17f0e4:	mov    0x20(%rax),%r13
  17f0e8:	mov    0x28(%rax),%r14
  17f0ec:	mov    $0x1,%ebx
  17f0f1:	jmp    17f11e <<rayon_core::job::StackJob<rayon_core::latch::LatchRef<rayon_core::latch::CountLatch>, &rayon_core::broadcast::broadcast_in<bitz::utils::blake3x4::smallest_pow_nonce_with<8, bitz::utils::blake3x4::first_pow_nonce>::{closure#0}, ()>::{closure#0}, ()> as rayon_core::job::Job>::execute+0x13e>
  17f0f3:	data16 data16 data16 cs nopw 0x0(%rax,%rax,1)
  17f100:	mov    $0x1,%edx
  17f105:	movabs $0xffffffffffffff,%rax
  17f10f:	lock xadd %rdx,0x0(%rbp)
  17f115:	cmp    %rax,%rdx
  17f118:	ja     17f038 <<rayon_core::job::StackJob<rayon_core::latch::LatchRef<rayon_core::latch::CountLatch>, &rayon_core::broadcast::broadcast_in<bitz::utils::blake3x4::smallest_pow_nonce_with<8, bitz::utils::blake3x4::first_pow_nonce>::{closure#0}, ()>::{closure#0}, ()> as rayon_core::job::Job>::execute+0x58>
  17f11e:	mov    (%r15),%rax
  17f121:	shl    $0x8,%rdx
  17f125:	cmp    %rax,%rdx
  17f128:	jae    17f033 <<rayon_core::job::StackJob<rayon_core::latch::LatchRef<rayon_core::latch::CountLatch>, &rayon_core::broadcast::broadcast_in<bitz::utils::blake3x4::smallest_pow_nonce_with<8, bitz::utils::blake3x4::first_pow_nonce>::{closure#0}, ()>::{closure#0}, ()> as rayon_core::job::Job>::execute+0x53>
  17f12e:	mov    (%r14),%r8d
  17f131:	mov    %rdx,%rcx
  17f134:	mov    $0xffffffffffffffff,%rax
  17f13b:	add    $0x100,%rcx
  17f142:	cmovb  %rax,%rcx
  17f146:	mov    %r12,%rdi
  17f149:	mov    %r13,%rsi
  17f14c:	call   17f370 <bitz::utils::blake3x4::first_pow_nonce>
  17f151:	cmp    $0x1,%rax
  17f155:	jne    17f100 <<rayon_core::job::StackJob<rayon_core::latch::LatchRef<rayon_core::latch::CountLatch>, &rayon_core::broadcast::broadcast_in<bitz::utils::blake3x4::smallest_pow_nonce_with<8, bitz::utils::blake3x4::first_pow_nonce>::{closure#0}, ()>::{closure#0}, ()> as rayon_core::job::Job>::execute+0x120>
  17f157:	mov    (%r15),%rax
  17f15a:	nopw   0x0(%rax,%rax,1)
  17f160:	cmp    %rdx,%rax
  17f163:	mov    %rdx,%rcx
  17f166:	cmovbe %rax,%rcx
  17f16a:	lock cmpxchg %rcx,(%r15)
  17f16f:	jne    17f160 <<rayon_core::job::StackJob<rayon_core::latch::LatchRef<rayon_core::latch::CountLatch>, &rayon_core::broadcast::broadcast_in<bitz::utils::blake3x4::smallest_pow_nonce_with<8, bitz::utils::blake3x4::first_pow_nonce>::{closure#0}, ()>::{closure#0}, ()> as rayon_core::job::Job>::execute+0x180>
  17f171:	jmp    17f100 <<rayon_core::job::StackJob<rayon_core::latch::LatchRef<rayon_core::latch::CountLatch>, &rayon_core::broadcast::broadcast_in<bitz::utils::blake3x4::smallest_pow_nonce_with<8, bitz::utils::blake3x4::first_pow_nonce>::{closure#0}, ()>::{closure#0}, ()> as rayon_core::job::Job>::execute+0x120>
  17f173:	add    $0x10,%rdi
  17f177:	call   17da40 <<rayon_core::latch::LockLatch as rayon_core::latch::Latch>::set>
  17f17c:	add    $0x18,%rsp
  17f180:	pop    %rbx
  17f181:	pop    %r12
  17f183:	pop    %r13
  17f185:	pop    %r14
  17f187:	pop    %r15
  17f189:	pop    %rbp
  17f18a:	ret
  17f18b:	lea    0x42c8d6(%rip),%rdi        # 5aba68 <<bitz::piop::spartan::falcon1024_ct::hybrid_sumcheck::State<_> as bitz::sumcheck::inner::input::State<field::binary::Gf128Ops>>::fold::__CALLSITE::META+0x42f8>
  17f192:	call   3d0ae0 <core::option::unwrap_failed>
  17f197:	jmp    17f216 <<rayon_core::job::StackJob<rayon_core::latch::LatchRef<rayon_core::latch::CountLatch>, &rayon_core::broadcast::broadcast_in<bitz::utils::blake3x4::smallest_pow_nonce_with<8, bitz::utils::blake3x4::first_pow_nonce>::{closure#0}, ()>::{closure#0}, ()> as rayon_core::job::Job>::execute+0x236>
  17f199:	lea    -0x1124e4(%rip),%rdi        # 6ccbc <perfetto::base::(anonymous namespace)::kHexmap+0x105dc>
  17f1a0:	lea    0x420781(%rip),%rdx        # 59f928 <falcon_hybrid::main::__CALLSITE::META+0xc90>
  17f1a7:	mov    $0x2a,%esi
  17f1ac:	call   3ce5b0 <core::panicking::panic>
  17f1b1:	jmp    17f216 <<rayon_core::job::StackJob<rayon_core::latch::LatchRef<rayon_core::latch::CountLatch>, &rayon_core::broadcast::broadcast_in<bitz::utils::blake3x4::smallest_pow_nonce_with<8, bitz::utils::blake3x4::first_pow_nonce>::{closure#0}, ()>::{closure#0}, ()> as rayon_core::job::Job>::execute+0x236>
  17f1b3:	lock decq (%rbx)
  17f1b7:	jne    17f211 <<rayon_core::job::StackJob<rayon_core::latch::LatchRef<rayon_core::latch::CountLatch>, &rayon_core::broadcast::broadcast_in<bitz::utils::blake3x4::smallest_pow_nonce_with<8, bitz::utils::blake3x4::first_pow_nonce>::{closure#0}, ()>::{closure#0}, ()> as rayon_core::job::Job>::execute+0x231>
  17f1b9:	mov    %rbx,%rdi
  17f1bc:	call   480d30 <<alloc::sync::Arc<rayon_core::registry::Registry>>::drop_slow>
  17f1c1:	jmp    17f211 <<rayon_core::job::StackJob<rayon_core::latch::LatchRef<rayon_core::latch::CountLatch>, &rayon_core::broadcast::broadcast_in<bitz::utils::blake3x4::smallest_pow_nonce_with<8, bitz::utils::blake3x4::first_pow_nonce>::{closure#0}, ()>::{closure#0}, ()> as rayon_core::job::Job>::execute+0x231>
  17f1c3:	call   3d0ca3 <core::panicking::panic_in_cleanup>
  17f1c8:	cmpq   $0x0,0x8(%r14)
  17f1cd:	je     17f1d8 <<rayon_core::job::StackJob<rayon_core::latch::LatchRef<rayon_core::latch::CountLatch>, &rayon_core::broadcast::broadcast_in<bitz::utils::blake3x4::smallest_pow_nonce_with<8, bitz::utils::blake3x4::first_pow_nonce>::{closure#0}, ()>::{closure#0}, ()> as rayon_core::job::Job>::execute+0x1f8>
  17f1cf:	mov    %r12,%rdi
  17f1d2:	call   *0x434c68(%rip)        # 5b3e40 <free@GLIBC_2.2.5>
  17f1d8:	mov    0x10(%rsp),%rax
  17f1dd:	mov    (%rsp),%rcx
  17f1e1:	mov    0x8(%rsp),%rdx
  17f1e6:	mov    %rbx,0x10(%rax)
  17f1ea:	mov    %rcx,0x18(%rax)
  17f1ee:	mov    %rdx,0x20(%rax)
  17f1f2:	jmp    17f211 <<rayon_core::job::StackJob<rayon_core::latch::LatchRef<rayon_core::latch::CountLatch>, &rayon_core::broadcast::broadcast_in<bitz::utils::blake3x4::smallest_pow_nonce_with<8, bitz::utils::blake3x4::first_pow_nonce>::{closure#0}, ()>::{closure#0}, ()> as rayon_core::job::Job>::execute+0x231>
  17f1f4:	jmp    17f1f6 <<rayon_core::job::StackJob<rayon_core::latch::LatchRef<rayon_core::latch::CountLatch>, &rayon_core::broadcast::broadcast_in<bitz::utils::blake3x4::smallest_pow_nonce_with<8, bitz::utils::blake3x4::first_pow_nonce>::{closure#0}, ()>::{closure#0}, ()> as rayon_core::job::Job>::execute+0x216>
  17f1f6:	mov    %rax,%rdi
  17f1f9:	call   4c209a <std::panicking::catch_unwind::cleanup>
  17f1fe:	mov    $0x2,%ebx
  17f203:	mov    %rax,(%rsp)
  17f207:	mov    %rdx,0x8(%rsp)
  17f20c:	jmp    17f038 <<rayon_core::job::StackJob<rayon_core::latch::LatchRef<rayon_core::latch::CountLatch>, &rayon_core::broadcast::broadcast_in<bitz::utils::blake3x4::smallest_pow_nonce_with<8, bitz::utils::blake3x4::first_pow_nonce>::{closure#0}, ()>::{closure#0}, ()> as rayon_core::job::Job>::execute+0x58>
  17f211:	call   484f80 <<rayon_core::unwind::AbortIfPanic as core::ops::drop::Drop>::drop>
  17f216:	ud2
  17f218:	call   3d0ca3 <core::panicking::panic_in_cleanup>

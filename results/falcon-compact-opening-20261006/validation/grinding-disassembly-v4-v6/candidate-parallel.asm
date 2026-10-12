
/tmp/falcon-compact-opening-binaries/candidate-v6:     file format elf64-x86-64


Disassembly of section .text:

000000000017ec50 <bitz::utils::blake3x4::smallest_pow_nonce>:
  17ec50:	push   %rbp
  17ec51:	push   %r15
  17ec53:	push   %r14
  17ec55:	push   %r13
  17ec57:	push   %r12
  17ec59:	push   %rbx
  17ec5a:	sub    $0xd8,%rsp
  17ec61:	cmp    $0x11,%edx
  17ec64:	mov    %rsi,%rbx
  17ec67:	mov    %rdi,%r14
  17ec6a:	mov    %rsi,(%rsp)
  17ec6e:	mov    %rdi,0x88(%rsp)
  17ec76:	setb   %al
  17ec79:	test   $0x3,%bl
  17ec7c:	sete   %cl
  17ec7f:	test   %cl,%al
  17ec81:	je     17ecbe <bitz::utils::blake3x4::smallest_pow_nonce+0x6e>
  17ec83:	mov    %fs:0xffffffffffffff60,%rax
  17ec8c:	mov    %edx,0x14(%rsp)
  17ec90:	movq   $0x0,0x80(%rsp)
  17ec9c:	movq   $0xffffffffffffffff,0x78(%rsp)
  17eca5:	test   %rax,%rax
  17eca8:	je     17ed01 <bitz::utils::blake3x4::smallest_pow_nonce+0xb1>
  17ecaa:	add    $0x110,%rax
  17ecb0:	mov    (%rax),%rcx
  17ecb3:	lock incq (%rcx)
  17ecb7:	jg     17ed13 <bitz::utils::blake3x4::smallest_pow_nonce+0xc3>
  17ecb9:	jmp    17f865 <bitz::utils::blake3x4::smallest_pow_nonce+0xc15>
  17ecbe:	mov    %fs:0xffffffffffffff60,%rax
  17ecc7:	mov    %edx,0x14(%rsp)
  17eccb:	movq   $0x0,0x80(%rsp)
  17ecd7:	movq   $0xffffffffffffffff,0x78(%rsp)
  17ece0:	test   %rax,%rax
  17ece3:	je     17ed9f <bitz::utils::blake3x4::smallest_pow_nonce+0x14f>
  17ece9:	add    $0x110,%rax
  17ecef:	mov    (%rax),%rcx
  17ecf2:	lock incq (%rcx)
  17ecf6:	jg     17edb1 <bitz::utils::blake3x4::smallest_pow_nonce+0x161>
  17ecfc:	jmp    17f865 <bitz::utils::blake3x4::smallest_pow_nonce+0xc15>
  17ed01:	call   4864b0 <rayon_core::registry::global_registry>
  17ed06:	mov    (%rax),%rcx
  17ed09:	lock incq (%rcx)
  17ed0d:	jle    17f865 <bitz::utils::blake3x4::smallest_pow_nonce+0xc15>
  17ed13:	mov    (%rax),%r13
  17ed16:	lea    0x80(%rsp),%rdx
  17ed1e:	lea    0x78(%rsp),%rcx
  17ed23:	mov    %rdx,0x90(%rsp)
  17ed2b:	mov    %rcx,0x98(%rsp)
  17ed33:	lea    0x13(%rsp),%rdx
  17ed38:	lea    0x14(%rsp),%rcx
  17ed3d:	mov    %rdx,0xa0(%rsp)
  17ed45:	mov    %r14,0xa8(%rsp)
  17ed4d:	mov    %rbx,0xb0(%rsp)
  17ed55:	mov    %rcx,0xb8(%rsp)
  17ed5d:	mov    %fs:0xffffffffffffff60,%rdx
  17ed66:	mov    0x208(%r13),%r12
  17ed6d:	mov    %rdx,0x58(%rsp)
  17ed72:	test   %rdx,%rdx
  17ed75:	je     17ee36 <bitz::utils::blake3x4::smallest_pow_nonce+0x1e6>
  17ed7b:	mov    0x110(%rdx),%rax
  17ed82:	lock incq (%rax)
  17ed86:	jle    17f865 <bitz::utils::blake3x4::smallest_pow_nonce+0xc15>
  17ed8c:	mov    0x100(%rdx),%rcx
  17ed93:	mov    0x110(%rdx),%rax
  17ed9a:	jmp    17ee3a <bitz::utils::blake3x4::smallest_pow_nonce+0x1ea>
  17ed9f:	call   4864b0 <rayon_core::registry::global_registry>
  17eda4:	mov    (%rax),%rcx
  17eda7:	lock incq (%rcx)
  17edab:	jle    17f865 <bitz::utils::blake3x4::smallest_pow_nonce+0xc15>
  17edb1:	mov    (%rax),%r13
  17edb4:	lea    0x80(%rsp),%rdx
  17edbc:	lea    0x78(%rsp),%rcx
  17edc1:	mov    %rdx,0x90(%rsp)
  17edc9:	mov    %rcx,0x98(%rsp)
  17edd1:	lea    0x13(%rsp),%rdx
  17edd6:	lea    0x14(%rsp),%rcx
  17eddb:	mov    %rdx,0xa0(%rsp)
  17ede3:	mov    %r14,0xa8(%rsp)
  17edeb:	mov    %rbx,0xb0(%rsp)
  17edf3:	mov    %rcx,0xb8(%rsp)
  17edfb:	mov    %fs:0xffffffffffffff60,%rdx
  17ee04:	mov    0x208(%r13),%r12
  17ee0b:	mov    %rdx,0x58(%rsp)
  17ee10:	test   %rdx,%rdx
  17ee13:	je     17ee85 <bitz::utils::blake3x4::smallest_pow_nonce+0x235>
  17ee15:	mov    0x110(%rdx),%rax
  17ee1c:	lock incq (%rax)
  17ee20:	jle    17f865 <bitz::utils::blake3x4::smallest_pow_nonce+0xc15>
  17ee26:	mov    0x100(%rdx),%rcx
  17ee2d:	mov    0x110(%rdx),%rax
  17ee34:	jmp    17ee89 <bitz::utils::blake3x4::smallest_pow_nonce+0x239>
  17ee36:	xor    %eax,%eax
  17ee38:	xor    %ecx,%ecx
  17ee3a:	mov    %r12,0x38(%rsp)
  17ee3f:	mov    %rax,0x40(%rsp)
  17ee44:	lea    0x0(,%r12,8),%rax
  17ee4c:	movq   $0x0,0x48(%rsp)
  17ee55:	mov    %rcx,0x50(%rsp)
  17ee5a:	lea    (%rax,%rax,4),%rbp
  17ee5e:	test   %r12,%r12
  17ee61:	je     17eed8 <bitz::utils::blake3x4::smallest_pow_nonce+0x288>
  17ee63:	mov    %rbp,%rdi
  17ee66:	call   *0x4371e4(%rip)        # 5b6050 <malloc@GLIBC_2.2.5>
  17ee6c:	test   %rax,%rax
  17ee6f:	je     17f849 <bitz::utils::blake3x4::smallest_pow_nonce+0xbf9>
  17ee75:	mov    %rax,%r15
  17ee78:	cmp    $0x8,%r12
  17ee7c:	jae    17eeee <bitz::utils::blake3x4::smallest_pow_nonce+0x29e>
  17ee7e:	xor    %eax,%eax
  17ee80:	jmp    17ef7a <bitz::utils::blake3x4::smallest_pow_nonce+0x32a>
  17ee85:	xor    %eax,%eax
  17ee87:	xor    %ecx,%ecx
  17ee89:	mov    %r12,0x38(%rsp)
  17ee8e:	mov    %rax,0x40(%rsp)
  17ee93:	lea    0x0(,%r12,8),%rax
  17ee9b:	movq   $0x0,0x48(%rsp)
  17eea4:	mov    %rcx,0x50(%rsp)
  17eea9:	lea    (%rax,%rax,4),%rbp
  17eead:	test   %r12,%r12
  17eeb0:	je     17eee3 <bitz::utils::blake3x4::smallest_pow_nonce+0x293>
  17eeb2:	mov    %rbp,%rdi
  17eeb5:	call   *0x437195(%rip)        # 5b6050 <malloc@GLIBC_2.2.5>
  17eebb:	test   %rax,%rax
  17eebe:	je     17f858 <bitz::utils::blake3x4::smallest_pow_nonce+0xc08>
  17eec4:	mov    %rax,%r15
  17eec7:	cmp    $0x8,%r12
  17eecb:	jae    17f223 <bitz::utils::blake3x4::smallest_pow_nonce+0x5d3>
  17eed1:	xor    %eax,%eax
  17eed3:	jmp    17f2aa <bitz::utils::blake3x4::smallest_pow_nonce+0x65a>
  17eed8:	mov    $0x8,%r15d
  17eede:	jmp    17efb8 <bitz::utils::blake3x4::smallest_pow_nonce+0x368>
  17eee3:	mov    $0x8,%r15d
  17eee9:	jmp    17f2e8 <bitz::utils::blake3x4::smallest_pow_nonce+0x698>
  17eeee:	vmovdqa64 -0x129cb8(%rip),%zmm2        # 55240 <GCC_except_table0+0xac0>
  17eef8:	vpbroadcastq -0x12a6d2(%rip),%zmm3        # 54830 <GCC_except_table0+0xb0>
  17ef02:	vpbroadcastq -0x12a74c(%rip),%zmm5        # 547c0 <GCC_except_table0+0x40>
  17ef0c:	lea    0x38(%rsp),%rcx
  17ef11:	movabs $0x3fffffffffffff8,%rax
  17ef1b:	vpxor  %xmm4,%xmm4,%xmm4
  17ef1f:	vpbroadcastq %rcx,%zmm0
  17ef25:	lea    0x90(%rsp),%rcx
  17ef2d:	and    %r12,%rax
  17ef30:	vpbroadcastq %rcx,%zmm1
  17ef36:	mov    %rax,%rcx
  17ef39:	nopl   0x0(%rax)
  17ef40:	vpmullq %zmm3,%zmm2,%zmm6
  17ef46:	kxnorb %k0,%k0,%k1
  17ef4a:	vpaddq %zmm5,%zmm2,%zmm2
  17ef50:	add    $0xfffffffffffffff8,%rcx
  17ef54:	vpscatterqq %zmm0,(%r15,%zmm6,1){%k1}
  17ef5b:	kxnorb %k0,%k0,%k1
  17ef5f:	vpscatterqq %zmm1,0x8(%r15,%zmm6,1){%k1}
  17ef67:	kxnorb %k0,%k0,%k1
  17ef6b:	vpscatterqq %zmm4,0x10(%r15,%zmm6,1){%k1}
  17ef73:	jne    17ef40 <bitz::utils::blake3x4::smallest_pow_nonce+0x2f0>
  17ef75:	cmp    %rax,%r12
  17ef78:	je     17efb8 <bitz::utils::blake3x4::smallest_pow_nonce+0x368>
  17ef7a:	lea    (%rax,%rax,4),%rcx
  17ef7e:	mov    %r12,%rdx
  17ef81:	sub    %rax,%rdx
  17ef84:	lea    0x38(%rsp),%rax
  17ef89:	lea    0x90(%rsp),%rsi
  17ef91:	lea    0x10(%r15,%rcx,8),%rcx
  17ef96:	cs nopw 0x0(%rax,%rax,1)
  17efa0:	mov    %rax,-0x10(%rcx)
  17efa4:	mov    %rsi,-0x8(%rcx)
  17efa8:	movq   $0x0,(%rcx)
  17efaf:	add    $0x28,%rcx
  17efb3:	dec    %rdx
  17efb6:	jne    17efa0 <bitz::utils::blake3x4::smallest_pow_nonce+0x350>
  17efb8:	mov    %r12,0xc0(%rsp)
  17efc0:	mov    %r15,0xc8(%rsp)
  17efc8:	mov    %r12,0xd0(%rsp)
  17efd0:	mov    0x208(%r13),%rax
  17efd7:	mov    %r12,0x28(%rsp)
  17efdc:	mov    %rax,0x20(%rsp)
  17efe1:	cmp    %r12,%rax
  17efe4:	jne    17f5af <bitz::utils::blake3x4::smallest_pow_nonce+0x95f>
  17efea:	lea    0x180(%r13),%rax
  17eff1:	mov    $0x1,%ecx
  17eff6:	mov    %rax,0x60(%rsp)
  17effb:	xor    %eax,%eax
  17effd:	lock cmpxchg %ecx,0x180(%r13)
  17f006:	jne    17f5f3 <bitz::utils::blake3x4::smallest_pow_nonce+0x9a3>
  17f00c:	mov    0x45699d(%rip),%rax        # 5d59b0 <std::panicking::panic_count::GLOBAL_PANIC_COUNT>
  17f013:	shl    $1,%rax
  17f016:	test   %rax,%rax
  17f019:	jne    17f615 <bitz::utils::blake3x4::smallest_pow_nonce+0x9c5>
  17f01f:	movl   $0x0,0xc(%rsp)
  17f027:	movzbl 0x184(%r13),%eax
  17f02f:	test   %al,%al
  17f031:	jne    17f633 <bitz::utils::blake3x4::smallest_pow_nonce+0x9e3>
  17f037:	mov    0x198(%r13),%rax
  17f03e:	mov    %r12,0x28(%rsp)
  17f043:	mov    %rax,0x20(%rsp)
  17f048:	cmp    %r12,%rax
  17f04b:	jne    17f6f3 <bitz::utils::blake3x4::smallest_pow_nonce+0xaa3>
  17f051:	mov    %rbp,0x68(%rsp)
  17f056:	mov    %r13,0x18(%rsp)
  17f05b:	mov    %r12,0x70(%rsp)
  17f060:	test   %r12,%r12
  17f063:	je     17f0f7 <bitz::utils::blake3x4::smallest_pow_nonce+0x4a7>
  17f069:	mov    0x18(%rsp),%rax
  17f06e:	mov    0x70(%rsp),%rbp
  17f073:	lea    0xff6(%rip),%r14        # 180070 <<rayon_core::job::StackJob<rayon_core::latch::LatchRef<rayon_core::latch::CountLatch>, &rayon_core::broadcast::broadcast_in<bitz::utils::blake3x4::smallest_pow_nonce_with<8, bitz::utils::blake3x4::first_pow_nonce>::{closure#0}, ()>::{closure#0}, ()> as rayon_core::job::Job>::execute>
  17f07a:	mov    %r15,%r12
  17f07d:	mov    0x190(%rax),%rbx
  17f084:	data16 data16 cs nopw 0x0(%rax,%rax,1)
  17f090:	mov    (%rbx),%rax
  17f093:	mov    0x108(%rax),%r13
  17f09a:	mov    0x100(%rax),%rax
  17f0a1:	mov    0x10(%rbx),%rsi
  17f0a5:	mov    %r13,%rcx
  17f0a8:	sub    %rax,%rcx
  17f0ab:	cmp    %rsi,%rcx
  17f0ae:	jge    17f0e3 <bitz::utils::blake3x4::smallest_pow_nonce+0x493>
  17f0b0:	mov    0x8(%rbx),%rax
  17f0b4:	dec    %rsi
  17f0b7:	and    %r13,%rsi
  17f0ba:	inc    %r13
  17f0bd:	shl    $0x4,%rsi
  17f0c1:	mov    %r14,(%rax,%rsi,1)
  17f0c5:	mov    %r12,0x8(%rax,%rsi,1)
  17f0ca:	mov    (%rbx),%rcx
  17f0cd:	add    $0x28,%r12
  17f0d1:	add    $0x20,%rbx
  17f0d5:	dec    %rbp
  17f0d8:	mov    %r13,0x108(%rcx)
  17f0df:	jne    17f090 <bitz::utils::blake3x4::smallest_pow_nonce+0x440>
  17f0e1:	jmp    17f0f7 <bitz::utils::blake3x4::smallest_pow_nonce+0x4a7>
  17f0e3:	add    %rsi,%rsi
  17f0e6:	mov    %rbx,%rdi
  17f0e9:	vzeroupper
  17f0ec:	call   17dde0 <<crossbeam_deque::deque::Worker<rayon_core::job::JobRef>>::resize>
  17f0f1:	mov    0x10(%rbx),%rsi
  17f0f5:	jmp    17f0b0 <bitz::utils::blake3x4::smallest_pow_nonce+0x460>
  17f0f7:	cmpb   $0x0,0xc(%rsp)
  17f0fc:	mov    0x18(%rsp),%r13
  17f101:	jne    17f116 <bitz::utils::blake3x4::smallest_pow_nonce+0x4c6>
  17f103:	mov    0x4568a6(%rip),%rax        # 5d59b0 <std::panicking::panic_count::GLOBAL_PANIC_COUNT>
  17f10a:	shl    $1,%rax
  17f10d:	test   %rax,%rax
  17f110:	jne    17f80f <bitz::utils::blake3x4::smallest_pow_nonce+0xbbf>
  17f116:	mov    0x60(%rsp),%rsi
  17f11b:	xor    %eax,%eax
  17f11d:	xchg   %eax,(%rsi)
  17f11f:	mov    0x70(%rsp),%rbp
  17f124:	cmp    $0x2,%eax
  17f127:	je     17f737 <bitz::utils::blake3x4::smallest_pow_nonce+0xae7>
  17f12d:	mov    0x208(%r13),%r12
  17f134:	test   %r12,%r12
  17f137:	je     17f166 <bitz::utils::blake3x4::smallest_pow_nonce+0x516>
  17f139:	lea    0x1d8(%r13),%rbx
  17f140:	xor    %r14d,%r14d
  17f143:	data16 data16 data16 cs nopw 0x0(%rax,%rax,1)
  17f150:	mov    %rbx,%rdi
  17f153:	mov    %r14,%rsi
  17f156:	vzeroupper
  17f159:	call   481e60 <<rayon_core::sleep::Sleep>::wake_specific_thread>
  17f15e:	inc    %r14
  17f161:	cmp    %r14,%r12
  17f164:	jne    17f150 <bitz::utils::blake3x4::smallest_pow_nonce+0x500>
  17f166:	cmpq   $0x0,0x40(%rsp)
  17f16c:	lea    0x48(%rsp),%rsi
  17f171:	je     17f199 <bitz::utils::blake3x4::smallest_pow_nonce+0x549>
  17f173:	cmpq   $0x0,0x58(%rsp)
  17f179:	je     17f795 <bitz::utils::blake3x4::smallest_pow_nonce+0xb45>
  17f17f:	mov    0x48(%rsp),%rax
  17f184:	cmp    $0x3,%rax
  17f188:	je     17f1a4 <bitz::utils::blake3x4::smallest_pow_nonce+0x554>
  17f18a:	mov    0x58(%rsp),%rdi
  17f18f:	vzeroupper
  17f192:	call   4811f0 <<rayon_core::registry::WorkerThread>::wait_until_cold>
  17f197:	jmp    17f1a4 <bitz::utils::blake3x4::smallest_pow_nonce+0x554>
  17f199:	mov    %rsi,%rdi
  17f19c:	vzeroupper
  17f19f:	call   482fc0 <<rayon_core::latch::LockLatch>::wait>
  17f1a4:	test   %rbp,%rbp
  17f1a7:	je     17f1da <bitz::utils::blake3x4::smallest_pow_nonce+0x58a>
  17f1a9:	mov    0x68(%rsp),%rax
  17f1ae:	lea    (%r15,%rax,1),%rbp
  17f1b2:	mov    %r15,%rax
  17f1b5:	mov    0x10(%rax),%rcx
  17f1b9:	lea    0x28(%rax),%r14
  17f1bd:	cmp    $0x1,%rcx
  17f1c1:	jne    17f204 <bitz::utils::blake3x4::smallest_pow_nonce+0x5b4>
  17f1c3:	addq   $0xffffffffffffffd8,0x68(%rsp)
  17f1c9:	mov    %r14,%rax
  17f1cc:	jne    17f1b5 <bitz::utils::blake3x4::smallest_pow_nonce+0x565>
  17f1ce:	mov    %r15,%rdi
  17f1d1:	vzeroupper
  17f1d4:	call   *0x436e86(%rip)        # 5b6060 <free@GLIBC_2.2.5>
  17f1da:	mov    0x40(%rsp),%rax
  17f1df:	test   %rax,%rax
  17f1e2:	je     17f52b <bitz::utils::blake3x4::smallest_pow_nonce+0x8db>
  17f1e8:	lock decq (%rax)
  17f1ec:	jne    17f52b <bitz::utils::blake3x4::smallest_pow_nonce+0x8db>
  17f1f2:	mov    0x40(%rsp),%rdi
  17f1f7:	vzeroupper
  17f1fa:	call   482b40 <<alloc::sync::Arc<rayon_core::registry::Registry>>::drop_slow>
  17f1ff:	jmp    17f52b <bitz::utils::blake3x4::smallest_pow_nonce+0x8db>
  17f204:	cmp    $0x2,%rcx
  17f208:	jne    17f7d5 <bitz::utils::blake3x4::smallest_pow_nonce+0xb85>
  17f20e:	mov    0x18(%rax),%rdi
  17f212:	mov    0x20(%rax),%rsi
  17f216:	vzeroupper
  17f219:	call   486820 <rayon_core::unwind::resume_unwinding>
  17f21e:	jmp    17f865 <bitz::utils::blake3x4::smallest_pow_nonce+0xc15>
  17f223:	vmovdqa64 -0x129fed(%rip),%zmm2        # 55240 <GCC_except_table0+0xac0>
  17f22d:	vpbroadcastq -0x12aa07(%rip),%zmm3        # 54830 <GCC_except_table0+0xb0>
  17f237:	vpbroadcastq -0x12aa81(%rip),%zmm5        # 547c0 <GCC_except_table0+0x40>
  17f241:	lea    0x38(%rsp),%rcx
  17f246:	movabs $0x3fffffffffffff8,%rax
  17f250:	vpxor  %xmm4,%xmm4,%xmm4
  17f254:	vpbroadcastq %rcx,%zmm0
  17f25a:	lea    0x90(%rsp),%rcx
  17f262:	and    %r12,%rax
  17f265:	vpbroadcastq %rcx,%zmm1
  17f26b:	mov    %rax,%rcx
  17f26e:	xchg   %ax,%ax
  17f270:	vpmullq %zmm3,%zmm2,%zmm6
  17f276:	kxnorb %k0,%k0,%k1
  17f27a:	vpaddq %zmm5,%zmm2,%zmm2
  17f280:	add    $0xfffffffffffffff8,%rcx
  17f284:	vpscatterqq %zmm0,(%r15,%zmm6,1){%k1}
  17f28b:	kxnorb %k0,%k0,%k1
  17f28f:	vpscatterqq %zmm1,0x8(%r15,%zmm6,1){%k1}
  17f297:	kxnorb %k0,%k0,%k1
  17f29b:	vpscatterqq %zmm4,0x10(%r15,%zmm6,1){%k1}
  17f2a3:	jne    17f270 <bitz::utils::blake3x4::smallest_pow_nonce+0x620>
  17f2a5:	cmp    %rax,%r12
  17f2a8:	je     17f2e8 <bitz::utils::blake3x4::smallest_pow_nonce+0x698>
  17f2aa:	lea    (%rax,%rax,4),%rcx
  17f2ae:	mov    %r12,%rdx
  17f2b1:	sub    %rax,%rdx
  17f2b4:	lea    0x38(%rsp),%rax
  17f2b9:	lea    0x90(%rsp),%rsi
  17f2c1:	lea    0x10(%r15,%rcx,8),%rcx
  17f2c6:	cs nopw 0x0(%rax,%rax,1)
  17f2d0:	mov    %rax,-0x10(%rcx)
  17f2d4:	mov    %rsi,-0x8(%rcx)
  17f2d8:	movq   $0x0,(%rcx)
  17f2df:	add    $0x28,%rcx
  17f2e3:	dec    %rdx
  17f2e6:	jne    17f2d0 <bitz::utils::blake3x4::smallest_pow_nonce+0x680>
  17f2e8:	mov    %r12,0xc0(%rsp)
  17f2f0:	mov    %r15,0xc8(%rsp)
  17f2f8:	mov    %r12,0xd0(%rsp)
  17f300:	mov    0x208(%r13),%rax
  17f307:	mov    %r12,0x28(%rsp)
  17f30c:	mov    %rax,0x20(%rsp)
  17f311:	cmp    %r12,%rax
  17f314:	jne    17f5d1 <bitz::utils::blake3x4::smallest_pow_nonce+0x981>
  17f31a:	lea    0x180(%r13),%rax
  17f321:	mov    $0x1,%ecx
  17f326:	mov    %rax,0x60(%rsp)
  17f32b:	xor    %eax,%eax
  17f32d:	lock cmpxchg %ecx,0x180(%r13)
  17f336:	jne    17f673 <bitz::utils::blake3x4::smallest_pow_nonce+0xa23>
  17f33c:	mov    0x45666d(%rip),%rax        # 5d59b0 <std::panicking::panic_count::GLOBAL_PANIC_COUNT>
  17f343:	shl    $1,%rax
  17f346:	test   %rax,%rax
  17f349:	jne    17f695 <bitz::utils::blake3x4::smallest_pow_nonce+0xa45>
  17f34f:	movl   $0x0,0xc(%rsp)
  17f357:	movzbl 0x184(%r13),%eax
  17f35f:	test   %al,%al
  17f361:	jne    17f6b3 <bitz::utils::blake3x4::smallest_pow_nonce+0xa63>
  17f367:	mov    0x198(%r13),%rax
  17f36e:	mov    %r12,0x28(%rsp)
  17f373:	mov    %rax,0x20(%rsp)
  17f378:	cmp    %r12,%rax
  17f37b:	jne    17f715 <bitz::utils::blake3x4::smallest_pow_nonce+0xac5>
  17f381:	mov    %rbp,0x68(%rsp)
  17f386:	mov    %r13,0x18(%rsp)
  17f38b:	mov    %r12,0x70(%rsp)
  17f390:	test   %r12,%r12
  17f393:	je     17f427 <bitz::utils::blake3x4::smallest_pow_nonce+0x7d7>
  17f399:	mov    0x18(%rsp),%rax
  17f39e:	mov    0x70(%rsp),%rbp
  17f3a3:	lea    0x746(%rip),%r14        # 17faf0 <<rayon_core::job::StackJob<rayon_core::latch::LatchRef<rayon_core::latch::CountLatch>, &rayon_core::broadcast::broadcast_in<bitz::utils::blake3x4::smallest_pow_nonce_with<10, bitz::utils::blake3x4::first_pow_nonce>::{closure#0}, ()>::{closure#0}, ()> as rayon_core::job::Job>::execute>
  17f3aa:	mov    %r15,%r12
  17f3ad:	mov    0x190(%rax),%rbx
  17f3b4:	data16 data16 cs nopw 0x0(%rax,%rax,1)
  17f3c0:	mov    (%rbx),%rax
  17f3c3:	mov    0x108(%rax),%r13
  17f3ca:	mov    0x100(%rax),%rax
  17f3d1:	mov    0x10(%rbx),%rsi
  17f3d5:	mov    %r13,%rcx
  17f3d8:	sub    %rax,%rcx
  17f3db:	cmp    %rsi,%rcx
  17f3de:	jge    17f413 <bitz::utils::blake3x4::smallest_pow_nonce+0x7c3>
  17f3e0:	mov    0x8(%rbx),%rax
  17f3e4:	dec    %rsi
  17f3e7:	and    %r13,%rsi
  17f3ea:	inc    %r13
  17f3ed:	shl    $0x4,%rsi
  17f3f1:	mov    %r14,(%rax,%rsi,1)
  17f3f5:	mov    %r12,0x8(%rax,%rsi,1)
  17f3fa:	mov    (%rbx),%rcx
  17f3fd:	add    $0x28,%r12
  17f401:	add    $0x20,%rbx
  17f405:	dec    %rbp
  17f408:	mov    %r13,0x108(%rcx)
  17f40f:	jne    17f3c0 <bitz::utils::blake3x4::smallest_pow_nonce+0x770>
  17f411:	jmp    17f427 <bitz::utils::blake3x4::smallest_pow_nonce+0x7d7>
  17f413:	add    %rsi,%rsi
  17f416:	mov    %rbx,%rdi
  17f419:	vzeroupper
  17f41c:	call   17dde0 <<crossbeam_deque::deque::Worker<rayon_core::job::JobRef>>::resize>
  17f421:	mov    0x10(%rbx),%rsi
  17f425:	jmp    17f3e0 <bitz::utils::blake3x4::smallest_pow_nonce+0x790>
  17f427:	cmpb   $0x0,0xc(%rsp)
  17f42c:	mov    0x18(%rsp),%r13
  17f431:	jne    17f446 <bitz::utils::blake3x4::smallest_pow_nonce+0x7f6>
  17f433:	mov    0x456576(%rip),%rax        # 5d59b0 <std::panicking::panic_count::GLOBAL_PANIC_COUNT>
  17f43a:	shl    $1,%rax
  17f43d:	test   %rax,%rax
  17f440:	jne    17f82c <bitz::utils::blake3x4::smallest_pow_nonce+0xbdc>
  17f446:	mov    0x60(%rsp),%rsi
  17f44b:	xor    %eax,%eax
  17f44d:	xchg   %eax,(%rsi)
  17f44f:	mov    0x70(%rsp),%rbp
  17f454:	cmp    $0x2,%eax
  17f457:	je     17f766 <bitz::utils::blake3x4::smallest_pow_nonce+0xb16>
  17f45d:	mov    0x208(%r13),%r12
  17f464:	test   %r12,%r12
  17f467:	je     17f496 <bitz::utils::blake3x4::smallest_pow_nonce+0x846>
  17f469:	lea    0x1d8(%r13),%rbx
  17f470:	xor    %r14d,%r14d
  17f473:	data16 data16 data16 cs nopw 0x0(%rax,%rax,1)
  17f480:	mov    %rbx,%rdi
  17f483:	mov    %r14,%rsi
  17f486:	vzeroupper
  17f489:	call   481e60 <<rayon_core::sleep::Sleep>::wake_specific_thread>
  17f48e:	inc    %r14
  17f491:	cmp    %r14,%r12
  17f494:	jne    17f480 <bitz::utils::blake3x4::smallest_pow_nonce+0x830>
  17f496:	cmpq   $0x0,0x40(%rsp)
  17f49c:	lea    0x48(%rsp),%rsi
  17f4a1:	je     17f4c9 <bitz::utils::blake3x4::smallest_pow_nonce+0x879>
  17f4a3:	cmpq   $0x0,0x58(%rsp)
  17f4a9:	je     17f7b5 <bitz::utils::blake3x4::smallest_pow_nonce+0xb65>
  17f4af:	mov    0x48(%rsp),%rax
  17f4b4:	cmp    $0x3,%rax
  17f4b8:	je     17f4d4 <bitz::utils::blake3x4::smallest_pow_nonce+0x884>
  17f4ba:	mov    0x58(%rsp),%rdi
  17f4bf:	vzeroupper
  17f4c2:	call   4811f0 <<rayon_core::registry::WorkerThread>::wait_until_cold>
  17f4c7:	jmp    17f4d4 <bitz::utils::blake3x4::smallest_pow_nonce+0x884>
  17f4c9:	mov    %rsi,%rdi
  17f4cc:	vzeroupper
  17f4cf:	call   482fc0 <<rayon_core::latch::LockLatch>::wait>
  17f4d4:	test   %rbp,%rbp
  17f4d7:	je     17f50e <bitz::utils::blake3x4::smallest_pow_nonce+0x8be>
  17f4d9:	mov    0x68(%rsp),%rax
  17f4de:	lea    (%r15,%rax,1),%rbp
  17f4e2:	mov    %r15,%rax
  17f4e5:	mov    0x10(%rax),%rcx
  17f4e9:	lea    0x28(%rax),%r14
  17f4ed:	cmp    $0x1,%rcx
  17f4f1:	jne    17f590 <bitz::utils::blake3x4::smallest_pow_nonce+0x940>
  17f4f7:	addq   $0xffffffffffffffd8,0x68(%rsp)
  17f4fd:	mov    %r14,%rax
  17f500:	jne    17f4e5 <bitz::utils::blake3x4::smallest_pow_nonce+0x895>
  17f502:	mov    %r15,%rdi
  17f505:	vzeroupper
  17f508:	call   *0x436b52(%rip)        # 5b6060 <free@GLIBC_2.2.5>
  17f50e:	mov    0x40(%rsp),%rax
  17f513:	test   %rax,%rax
  17f516:	je     17f52b <bitz::utils::blake3x4::smallest_pow_nonce+0x8db>
  17f518:	lock decq (%rax)
  17f51c:	jne    17f52b <bitz::utils::blake3x4::smallest_pow_nonce+0x8db>
  17f51e:	mov    0x40(%rsp),%rdi
  17f523:	vzeroupper
  17f526:	call   482b40 <<alloc::sync::Arc<rayon_core::registry::Registry>>::drop_slow>
  17f52b:	lock decq 0x0(%r13)
  17f530:	mov    (%rsp),%rbx
  17f534:	mov    0x88(%rsp),%r14
  17f53c:	jne    17f549 <bitz::utils::blake3x4::smallest_pow_nonce+0x8f9>
  17f53e:	mov    %r13,%rdi
  17f541:	vzeroupper
  17f544:	call   482b40 <<alloc::sync::Arc<rayon_core::registry::Registry>>::drop_slow>
  17f549:	mov    0x78(%rsp),%rdx
  17f54e:	mov    $0x1,%eax
  17f553:	cmp    $0xffffffffffffffff,%rdx
  17f557:	jne    17f57b <bitz::utils::blake3x4::smallest_pow_nonce+0x92b>
  17f559:	mov    0x14(%rsp),%ecx
  17f55d:	mov    %rbx,%rsi
  17f560:	mov    %rdx,%rbx
  17f563:	mov    $0xffffffffffffffff,%rdx
  17f56a:	mov    %r14,%rdi
  17f56d:	vzeroupper
  17f570:	call   17fd30 <bitz::utils::blake3x4::pow_ok>
  17f575:	movzbl %al,%eax
  17f578:	mov    %rbx,%rdx
  17f57b:	add    $0xd8,%rsp
  17f582:	pop    %rbx
  17f583:	pop    %r12
  17f585:	pop    %r13
  17f587:	pop    %r14
  17f589:	pop    %r15
  17f58b:	pop    %rbp
  17f58c:	vzeroupper
  17f58f:	ret
  17f590:	cmp    $0x2,%rcx
  17f594:	jne    17f7f2 <bitz::utils::blake3x4::smallest_pow_nonce+0xba2>
  17f59a:	mov    0x18(%rax),%rdi
  17f59e:	mov    0x20(%rax),%rsi
  17f5a2:	vzeroupper
  17f5a5:	call   486820 <rayon_core::unwind::resume_unwinding>
  17f5aa:	jmp    17f865 <bitz::utils::blake3x4::smallest_pow_nonce+0xc15>
  17f5af:	lea    0x423012(%rip),%r9        # 5a25c8 <falcon_hybrid::main::__CALLSITE::META+0x1818>
  17f5b6:	lea    0x20(%rsp),%rsi
  17f5bb:	lea    0x28(%rsp),%rdx
  17f5c0:	xor    %edi,%edi
  17f5c2:	xor    %ecx,%ecx
  17f5c4:	vzeroupper
  17f5c7:	call   3ce9f2 <core::panicking::assert_failed::<usize, usize>>
  17f5cc:	jmp    17f865 <bitz::utils::blake3x4::smallest_pow_nonce+0xc15>
  17f5d1:	lea    0x422ff0(%rip),%r9        # 5a25c8 <falcon_hybrid::main::__CALLSITE::META+0x1818>
  17f5d8:	lea    0x20(%rsp),%rsi
  17f5dd:	lea    0x28(%rsp),%rdx
  17f5e2:	xor    %edi,%edi
  17f5e4:	xor    %ecx,%ecx
  17f5e6:	vzeroupper
  17f5e9:	call   3ce9f2 <core::panicking::assert_failed::<usize, usize>>
  17f5ee:	jmp    17f865 <bitz::utils::blake3x4::smallest_pow_nonce+0xc15>
  17f5f3:	lea    0x180(%r13),%rdi
  17f5fa:	vzeroupper
  17f5fd:	call   48ff20 <<std::sys::sync::mutex::futex::Mutex>::lock_contended>
  17f602:	mov    0x4563a7(%rip),%rax        # 5d59b0 <std::panicking::panic_count::GLOBAL_PANIC_COUNT>
  17f609:	shl    $1,%rax
  17f60c:	test   %rax,%rax
  17f60f:	je     17f01f <bitz::utils::blake3x4::smallest_pow_nonce+0x3cf>
  17f615:	vzeroupper
  17f618:	call   48fde0 <std::panicking::panic_count::is_zero_slow_path>
  17f61d:	xor    $0x1,%al
  17f61f:	mov    %eax,0xc(%rsp)
  17f623:	movzbl 0x184(%r13),%eax
  17f62b:	test   %al,%al
  17f62d:	je     17f037 <bitz::utils::blake3x4::smallest_pow_nonce+0x3e7>
  17f633:	lea    0x180(%r13),%rax
  17f63a:	mov    %rax,0x28(%rsp)
  17f63f:	mov    0xc(%rsp),%eax
  17f643:	mov    %al,0x30(%rsp)
  17f647:	lea    -0xcec59(%rip),%rdi        # b09f5 <tracing_core::dispatcher::NO_SUBSCRIBER+0x86>
  17f64e:	lea    0x4263f3(%rip),%rcx        # 5a5a48 <falcon_hybrid::main::__CALLSITE::META+0x4c98>
  17f655:	lea    0x422f84(%rip),%r8        # 5a25e0 <falcon_hybrid::main::__CALLSITE::META+0x1830>
  17f65c:	lea    0x28(%rsp),%rdx
  17f661:	mov    $0x2b,%esi
  17f666:	vzeroupper
  17f669:	call   3d1ca0 <core::result::unwrap_failed>
  17f66e:	jmp    17f865 <bitz::utils::blake3x4::smallest_pow_nonce+0xc15>
  17f673:	lea    0x180(%r13),%rdi
  17f67a:	vzeroupper
  17f67d:	call   48ff20 <<std::sys::sync::mutex::futex::Mutex>::lock_contended>
  17f682:	mov    0x456327(%rip),%rax        # 5d59b0 <std::panicking::panic_count::GLOBAL_PANIC_COUNT>
  17f689:	shl    $1,%rax
  17f68c:	test   %rax,%rax
  17f68f:	je     17f34f <bitz::utils::blake3x4::smallest_pow_nonce+0x6ff>
  17f695:	vzeroupper
  17f698:	call   48fde0 <std::panicking::panic_count::is_zero_slow_path>
  17f69d:	xor    $0x1,%al
  17f69f:	mov    %eax,0xc(%rsp)
  17f6a3:	movzbl 0x184(%r13),%eax
  17f6ab:	test   %al,%al
  17f6ad:	je     17f367 <bitz::utils::blake3x4::smallest_pow_nonce+0x717>
  17f6b3:	lea    0x180(%r13),%rax
  17f6ba:	mov    %rax,0x28(%rsp)
  17f6bf:	mov    0xc(%rsp),%eax
  17f6c3:	mov    %al,0x30(%rsp)
  17f6c7:	lea    -0xcecd9(%rip),%rdi        # b09f5 <tracing_core::dispatcher::NO_SUBSCRIBER+0x86>
  17f6ce:	lea    0x426373(%rip),%rcx        # 5a5a48 <falcon_hybrid::main::__CALLSITE::META+0x4c98>
  17f6d5:	lea    0x422f04(%rip),%r8        # 5a25e0 <falcon_hybrid::main::__CALLSITE::META+0x1830>
  17f6dc:	lea    0x28(%rsp),%rdx
  17f6e1:	mov    $0x2b,%esi
  17f6e6:	vzeroupper
  17f6e9:	call   3d1ca0 <core::result::unwrap_failed>
  17f6ee:	jmp    17f865 <bitz::utils::blake3x4::smallest_pow_nonce+0xc15>
  17f6f3:	lea    0x422efe(%rip),%r9        # 5a25f8 <falcon_hybrid::main::__CALLSITE::META+0x1848>
  17f6fa:	lea    0x20(%rsp),%rsi
  17f6ff:	lea    0x28(%rsp),%rdx
  17f704:	xor    %edi,%edi
  17f706:	xor    %ecx,%ecx
  17f708:	vzeroupper
  17f70b:	call   3ce9f2 <core::panicking::assert_failed::<usize, usize>>
  17f710:	jmp    17f865 <bitz::utils::blake3x4::smallest_pow_nonce+0xc15>
  17f715:	lea    0x422edc(%rip),%r9        # 5a25f8 <falcon_hybrid::main::__CALLSITE::META+0x1848>
  17f71c:	lea    0x20(%rsp),%rsi
  17f721:	lea    0x28(%rsp),%rdx
  17f726:	xor    %edi,%edi
  17f728:	xor    %ecx,%ecx
  17f72a:	vzeroupper
  17f72d:	call   3ce9f2 <core::panicking::assert_failed::<usize, usize>>
  17f732:	jmp    17f865 <bitz::utils::blake3x4::smallest_pow_nonce+0xc15>
  17f737:	mov    $0xca,%edi
  17f73c:	mov    $0x81,%edx
  17f741:	mov    $0x1,%ecx
  17f746:	xor    %eax,%eax
  17f748:	vzeroupper
  17f74b:	call   *0x436927(%rip)        # 5b6078 <syscall@GLIBC_2.2.5>
  17f751:	mov    0x208(%r13),%r12
  17f758:	test   %r12,%r12
  17f75b:	jne    17f139 <bitz::utils::blake3x4::smallest_pow_nonce+0x4e9>
  17f761:	jmp    17f166 <bitz::utils::blake3x4::smallest_pow_nonce+0x516>
  17f766:	mov    $0xca,%edi
  17f76b:	mov    $0x81,%edx
  17f770:	mov    $0x1,%ecx
  17f775:	xor    %eax,%eax
  17f777:	vzeroupper
  17f77a:	call   *0x4368f8(%rip)        # 5b6078 <syscall@GLIBC_2.2.5>
  17f780:	mov    0x208(%r13),%r12
  17f787:	test   %r12,%r12
  17f78a:	jne    17f469 <bitz::utils::blake3x4::smallest_pow_nonce+0x819>
  17f790:	jmp    17f496 <bitz::utils::blake3x4::smallest_pow_nonce+0x846>
  17f795:	lea    -0xd0c2d(%rip),%rdi        # aeb6f <flock_core::genus95_curve_code::sage_data::GAMMA_SLOT_MASKS+0x331e7>
  17f79c:	lea    0x4311c5(%rip),%rdx        # 5b0968 <<bitz::piop::spartan::falcon1024_ct::hybrid_sumcheck::State<_> as bitz::sumcheck::inner::input::State<field::binary::Gf128Ops>>::fold::__CALLSITE::META+0x6fb8>
  17f7a3:	mov    $0xc,%esi
  17f7a8:	vzeroupper
  17f7ab:	call   3d1c60 <core::option::expect_failed>
  17f7b0:	jmp    17f865 <bitz::utils::blake3x4::smallest_pow_nonce+0xc15>
  17f7b5:	lea    -0xd0c4d(%rip),%rdi        # aeb6f <flock_core::genus95_curve_code::sage_data::GAMMA_SLOT_MASKS+0x331e7>
  17f7bc:	lea    0x4311a5(%rip),%rdx        # 5b0968 <<bitz::piop::spartan::falcon1024_ct::hybrid_sumcheck::State<_> as bitz::sumcheck::inner::input::State<field::binary::Gf128Ops>>::fold::__CALLSITE::META+0x6fb8>
  17f7c3:	mov    $0xc,%esi
  17f7c8:	vzeroupper
  17f7cb:	call   3d1c60 <core::option::expect_failed>
  17f7d0:	jmp    17f865 <bitz::utils::blake3x4::smallest_pow_nonce+0xc15>
  17f7d5:	lea    -0xcf485(%rip),%rdi        # b0357 <core::unicode::unicode_data::white_space::WHITESPACE_MAP+0x483>
  17f7dc:	lea    0x42e3fd(%rip),%rdx        # 5adbe0 <<bitz::piop::spartan::falcon1024_ct::hybrid_sumcheck::State<_> as bitz::sumcheck::inner::input::State<field::binary::Gf128Ops>>::fold::__CALLSITE::META+0x4230>
  17f7e3:	mov    $0x28,%esi
  17f7e8:	vzeroupper
  17f7eb:	call   3cf670 <core::panicking::panic>
  17f7f0:	jmp    17f865 <bitz::utils::blake3x4::smallest_pow_nonce+0xc15>
  17f7f2:	lea    -0xcf4a2(%rip),%rdi        # b0357 <core::unicode::unicode_data::white_space::WHITESPACE_MAP+0x483>
  17f7f9:	lea    0x42e3e0(%rip),%rdx        # 5adbe0 <<bitz::piop::spartan::falcon1024_ct::hybrid_sumcheck::State<_> as bitz::sumcheck::inner::input::State<field::binary::Gf128Ops>>::fold::__CALLSITE::META+0x4230>
  17f800:	mov    $0x28,%esi
  17f805:	vzeroupper
  17f808:	call   3cf670 <core::panicking::panic>
  17f80d:	jmp    17f865 <bitz::utils::blake3x4::smallest_pow_nonce+0xc15>
  17f80f:	vzeroupper
  17f812:	call   48fde0 <std::panicking::panic_count::is_zero_slow_path>
  17f817:	test   %al,%al
  17f819:	jne    17f116 <bitz::utils::blake3x4::smallest_pow_nonce+0x4c6>
  17f81f:	movb   $0x1,0x184(%r13)
  17f827:	jmp    17f116 <bitz::utils::blake3x4::smallest_pow_nonce+0x4c6>
  17f82c:	vzeroupper
  17f82f:	call   48fde0 <std::panicking::panic_count::is_zero_slow_path>
  17f834:	test   %al,%al
  17f836:	jne    17f446 <bitz::utils::blake3x4::smallest_pow_nonce+0x7f6>
  17f83c:	movb   $0x1,0x184(%r13)
  17f844:	jmp    17f446 <bitz::utils::blake3x4::smallest_pow_nonce+0x7f6>
  17f849:	mov    $0x8,%edi
  17f84e:	mov    %rbp,%rsi
  17f851:	call   17370b <alloc::raw_vec::handle_error>
  17f856:	jmp    17f865 <bitz::utils::blake3x4::smallest_pow_nonce+0xc15>
  17f858:	mov    $0x8,%edi
  17f85d:	mov    %rbp,%rsi
  17f860:	call   17370b <alloc::raw_vec::handle_error>
  17f865:	ud2
  17f867:	mov    0x18(%rsp),%r13
  17f86c:	mov    0x60(%rsp),%rdi
  17f871:	jmp    17f8ae <bitz::utils::blake3x4::smallest_pow_nonce+0xc5e>
  17f873:	mov    0x18(%rsp),%r13
  17f878:	mov    0x60(%rsp),%rdi
  17f87d:	jmp    17f8c5 <bitz::utils::blake3x4::smallest_pow_nonce+0xc75>
  17f87f:	mov    %rax,(%rsp)
  17f883:	jmp    17f961 <bitz::utils::blake3x4::smallest_pow_nonce+0xd11>
  17f888:	mov    %rax,(%rsp)
  17f88c:	jmp    17fa47 <bitz::utils::blake3x4::smallest_pow_nonce+0xdf7>
  17f891:	mov    %rax,(%rsp)
  17f895:	jmp    17f947 <bitz::utils::blake3x4::smallest_pow_nonce+0xcf7>
  17f89a:	mov    %rax,(%rsp)
  17f89e:	jmp    17fa2d <bitz::utils::blake3x4::smallest_pow_nonce+0xddd>
  17f8a3:	jmp    17f8f5 <bitz::utils::blake3x4::smallest_pow_nonce+0xca5>
  17f8a5:	jmp    17f908 <bitz::utils::blake3x4::smallest_pow_nonce+0xcb8>
  17f8a7:	lea    0x180(%r13),%rdi
  17f8ae:	mov    %rax,(%rsp)
  17f8b2:	movzbl 0xc(%rsp),%esi
  17f8b7:	call   2142e0 <core::ptr::drop_glue::<std::sync::poison::PoisonError<std::sync::poison::mutex::MutexGuard<bitz::hybrid::sumcheck::Scratch>>>>
  17f8bc:	jmp    17f8f9 <bitz::utils::blake3x4::smallest_pow_nonce+0xca9>
  17f8be:	lea    0x180(%r13),%rdi
  17f8c5:	mov    %rax,(%rsp)
  17f8c9:	movzbl 0xc(%rsp),%esi
  17f8ce:	call   2142e0 <core::ptr::drop_glue::<std::sync::poison::PoisonError<std::sync::poison::mutex::MutexGuard<bitz::hybrid::sumcheck::Scratch>>>>
  17f8d3:	jmp    17f90c <bitz::utils::blake3x4::smallest_pow_nonce+0xcbc>
  17f8d5:	lea    0x28(%rsp),%rdi
  17f8da:	mov    %rax,(%rsp)
  17f8de:	call   4e3110 <core::ptr::drop_glue::<std::sync::poison::PoisonError<std::sync::poison::mutex::MutexGuard<[core::option::Option<alloc::boxed::Box<dyn core::ops::function::FnMut<(u32, bool, bool), Output = ()> + core::marker::Send + core::marker::Sync>>; 1]>>>>
  17f8e3:	jmp    17f8f9 <bitz::utils::blake3x4::smallest_pow_nonce+0xca9>
  17f8e5:	lea    0x28(%rsp),%rdi
  17f8ea:	mov    %rax,(%rsp)
  17f8ee:	call   4e3110 <core::ptr::drop_glue::<std::sync::poison::PoisonError<std::sync::poison::mutex::MutexGuard<[core::option::Option<alloc::boxed::Box<dyn core::ops::function::FnMut<(u32, bool, bool), Output = ()> + core::marker::Send + core::marker::Sync>>; 1]>>>>
  17f8f3:	jmp    17f90c <bitz::utils::blake3x4::smallest_pow_nonce+0xcbc>
  17f8f5:	mov    %rax,(%rsp)
  17f8f9:	lea    0xc0(%rsp),%rdi
  17f901:	call   1802b0 <core::ptr::drop_glue::<alloc::vec::Vec<rayon_core::job::StackJob<rayon_core::latch::LatchRef<rayon_core::latch::CountLatch>, &rayon_core::broadcast::broadcast_in<bitz::utils::blake3x4::smallest_pow_nonce_with<8, bitz::utils::blake3x4::first_pow_nonce>::{closure#0}, ()>::{closure#0}, ()>>>>
  17f906:	jmp    17f947 <bitz::utils::blake3x4::smallest_pow_nonce+0xcf7>
  17f908:	mov    %rax,(%rsp)
  17f90c:	lea    0xc0(%rsp),%rdi
  17f914:	call   1802b0 <core::ptr::drop_glue::<alloc::vec::Vec<rayon_core::job::StackJob<rayon_core::latch::LatchRef<rayon_core::latch::CountLatch>, &rayon_core::broadcast::broadcast_in<bitz::utils::blake3x4::smallest_pow_nonce_with<8, bitz::utils::blake3x4::first_pow_nonce>::{closure#0}, ()>::{closure#0}, ()>>>>
  17f919:	jmp    17fa2d <bitz::utils::blake3x4::smallest_pow_nonce+0xddd>
  17f91e:	mov    %rax,(%rsp)
  17f922:	sub    %r14,%rbp
  17f925:	movabs $0xcccccccccccccccd,%rax
  17f92f:	mov    %rbp,%rdx
  17f932:	mulx   %rax,%rbp,%rbp
  17f937:	jne    17f979 <bitz::utils::blake3x4::smallest_pow_nonce+0xd29>
  17f939:	mov    %r15,%rdi
  17f93c:	call   *0x43671e(%rip)        # 5b6060 <free@GLIBC_2.2.5>
  17f942:	mov    0x18(%rsp),%r13
  17f947:	mov    0x40(%rsp),%rax
  17f94c:	test   %rax,%rax
  17f94f:	je     17f961 <bitz::utils::blake3x4::smallest_pow_nonce+0xd11>
  17f951:	lock decq (%rax)
  17f955:	jne    17f961 <bitz::utils::blake3x4::smallest_pow_nonce+0xd11>
  17f957:	mov    0x40(%rsp),%rdi
  17f95c:	call   482b40 <<alloc::sync::Arc<rayon_core::registry::Registry>>::drop_slow>
  17f961:	lock decq 0x0(%r13)
  17f966:	jne    17fa56 <bitz::utils::blake3x4::smallest_pow_nonce+0xe06>
  17f96c:	mov    %r13,%rdi
  17f96f:	call   482b40 <<alloc::sync::Arc<rayon_core::registry::Registry>>::drop_slow>
  17f974:	jmp    17fa56 <bitz::utils::blake3x4::smallest_pow_nonce+0xe06>
  17f979:	mov    0x4366e0(%rip),%r12        # 5b6060 <free@GLIBC_2.2.5>
  17f980:	shr    $0x5,%rbp
  17f984:	add    $0x28,%r14
  17f988:	jmp    17f999 <bitz::utils::blake3x4::smallest_pow_nonce+0xd49>
  17f98a:	nopw   0x0(%rax,%rax,1)
  17f990:	add    $0x28,%r14
  17f994:	dec    %rbp
  17f997:	je     17f939 <bitz::utils::blake3x4::smallest_pow_nonce+0xce9>
  17f999:	cmpl   $0x2,-0x18(%r14)
  17f99e:	jb     17f990 <bitz::utils::blake3x4::smallest_pow_nonce+0xd40>
  17f9a0:	mov    -0x8(%r14),%rbx
  17f9a4:	mov    -0x10(%r14),%r13
  17f9a8:	mov    (%rbx),%rax
  17f9ab:	test   %rax,%rax
  17f9ae:	je     17f9b5 <bitz::utils::blake3x4::smallest_pow_nonce+0xd65>
  17f9b0:	mov    %r13,%rdi
  17f9b3:	call   *%rax
  17f9b5:	cmpq   $0x0,0x8(%rbx)
  17f9ba:	je     17f990 <bitz::utils::blake3x4::smallest_pow_nonce+0xd40>
  17f9bc:	mov    %r13,%rdi
  17f9bf:	call   *%r12
  17f9c2:	jmp    17f990 <bitz::utils::blake3x4::smallest_pow_nonce+0xd40>
  17f9c4:	call   3d1d63 <core::panicking::panic_in_cleanup>
  17f9c9:	cmpq   $0x0,0x8(%rbx)
  17f9ce:	je     17f9e0 <bitz::utils::blake3x4::smallest_pow_nonce+0xd90>
  17f9d0:	mov    %r13,%rdi
  17f9d3:	call   *0x436687(%rip)        # 5b6060 <free@GLIBC_2.2.5>
  17f9d9:	nopl   0x0(%rax)
  17f9e0:	dec    %rbp
  17f9e3:	je     17fad6 <bitz::utils::blake3x4::smallest_pow_nonce+0xe86>
  17f9e9:	mov    %r14,%rdi
  17f9ec:	lea    0x28(%r14),%rbx
  17f9f0:	call   1803a0 <core::ptr::drop_glue::<rayon_core::job::StackJob<rayon_core::latch::LatchRef<rayon_core::latch::CountLatch>, &rayon_core::broadcast::broadcast_in<bitz::utils::blake3x4::smallest_pow_nonce_with<8, bitz::utils::blake3x4::first_pow_nonce>::{closure#0}, ()>::{closure#0}, ()>>>
  17f9f5:	mov    %rbx,%r14
  17f9f8:	jmp    17f9e0 <bitz::utils::blake3x4::smallest_pow_nonce+0xd90>
  17f9fa:	call   3d1d63 <core::panicking::panic_in_cleanup>
  17f9ff:	call   3d1d63 <core::panicking::panic_in_cleanup>
  17fa04:	mov    %rax,(%rsp)
  17fa08:	sub    %r14,%rbp
  17fa0b:	movabs $0xcccccccccccccccd,%rax
  17fa15:	mov    %rbp,%rdx
  17fa18:	mulx   %rax,%rbp,%rbp
  17fa1d:	jne    17fa5f <bitz::utils::blake3x4::smallest_pow_nonce+0xe0f>
  17fa1f:	mov    %r15,%rdi
  17fa22:	call   *0x436638(%rip)        # 5b6060 <free@GLIBC_2.2.5>
  17fa28:	mov    0x18(%rsp),%r13
  17fa2d:	mov    0x40(%rsp),%rax
  17fa32:	test   %rax,%rax
  17fa35:	je     17fa47 <bitz::utils::blake3x4::smallest_pow_nonce+0xdf7>
  17fa37:	lock decq (%rax)
  17fa3b:	jne    17fa47 <bitz::utils::blake3x4::smallest_pow_nonce+0xdf7>
  17fa3d:	mov    0x40(%rsp),%rdi
  17fa42:	call   482b40 <<alloc::sync::Arc<rayon_core::registry::Registry>>::drop_slow>
  17fa47:	lock decq 0x0(%r13)
  17fa4c:	jne    17fa56 <bitz::utils::blake3x4::smallest_pow_nonce+0xe06>
  17fa4e:	mov    %r13,%rdi
  17fa51:	call   482b40 <<alloc::sync::Arc<rayon_core::registry::Registry>>::drop_slow>
  17fa56:	mov    (%rsp),%rdi
  17fa5a:	call   59e0f0 <_Unwind_Resume@plt>
  17fa5f:	mov    0x4365fa(%rip),%r12        # 5b6060 <free@GLIBC_2.2.5>
  17fa66:	shr    $0x5,%rbp
  17fa6a:	add    $0x28,%r14
  17fa6e:	jmp    17fa79 <bitz::utils::blake3x4::smallest_pow_nonce+0xe29>
  17fa70:	add    $0x28,%r14
  17fa74:	dec    %rbp
  17fa77:	je     17fa1f <bitz::utils::blake3x4::smallest_pow_nonce+0xdcf>
  17fa79:	cmpl   $0x2,-0x18(%r14)
  17fa7e:	jb     17fa70 <bitz::utils::blake3x4::smallest_pow_nonce+0xe20>
  17fa80:	mov    -0x8(%r14),%rbx
  17fa84:	mov    -0x10(%r14),%r13
  17fa88:	mov    (%rbx),%rax
  17fa8b:	test   %rax,%rax
  17fa8e:	je     17fa95 <bitz::utils::blake3x4::smallest_pow_nonce+0xe45>
  17fa90:	mov    %r13,%rdi
  17fa93:	call   *%rax
  17fa95:	cmpq   $0x0,0x8(%rbx)
  17fa9a:	je     17fa70 <bitz::utils::blake3x4::smallest_pow_nonce+0xe20>
  17fa9c:	mov    %r13,%rdi
  17fa9f:	call   *%r12
  17faa2:	jmp    17fa70 <bitz::utils::blake3x4::smallest_pow_nonce+0xe20>
  17faa4:	call   3d1d63 <core::panicking::panic_in_cleanup>
  17faa9:	cmpq   $0x0,0x8(%rbx)
  17faae:	je     17fac0 <bitz::utils::blake3x4::smallest_pow_nonce+0xe70>
  17fab0:	mov    %r13,%rdi
  17fab3:	call   *0x4365a7(%rip)        # 5b6060 <free@GLIBC_2.2.5>
  17fab9:	nopl   0x0(%rax)
  17fac0:	dec    %rbp
  17fac3:	je     17fad6 <bitz::utils::blake3x4::smallest_pow_nonce+0xe86>
  17fac5:	mov    %r14,%rdi
  17fac8:	lea    0x28(%r14),%rbx
  17facc:	call   1803a0 <core::ptr::drop_glue::<rayon_core::job::StackJob<rayon_core::latch::LatchRef<rayon_core::latch::CountLatch>, &rayon_core::broadcast::broadcast_in<bitz::utils::blake3x4::smallest_pow_nonce_with<8, bitz::utils::blake3x4::first_pow_nonce>::{closure#0}, ()>::{closure#0}, ()>>>
  17fad1:	mov    %rbx,%r14
  17fad4:	jmp    17fac0 <bitz::utils::blake3x4::smallest_pow_nonce+0xe70>
  17fad6:	mov    %r15,%rdi
  17fad9:	call   *0x436581(%rip)        # 5b6060 <free@GLIBC_2.2.5>
  17fadf:	call   3d1d63 <core::panicking::panic_in_cleanup>
  17fae4:	call   3d1d63 <core::panicking::panic_in_cleanup>
  17fae9:	call   3d1d63 <core::panicking::panic_in_cleanup>

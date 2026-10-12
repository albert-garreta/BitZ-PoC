
/tmp/falcon-compact-opening-binaries/baseline-v4:     file format elf64-x86-64


Disassembly of section .text:

000000000017dbc0 <bitz::utils::blake3x4::smallest_pow_nonce>:
  17dbc0:	push   %rbp
  17dbc1:	push   %r15
  17dbc3:	push   %r14
  17dbc5:	push   %r13
  17dbc7:	push   %r12
  17dbc9:	push   %rbx
  17dbca:	sub    $0xd8,%rsp
  17dbd1:	cmp    $0x11,%edx
  17dbd4:	mov    %rsi,%rbx
  17dbd7:	mov    %rdi,%r14
  17dbda:	mov    %rsi,(%rsp)
  17dbde:	mov    %rdi,0x88(%rsp)
  17dbe6:	setb   %al
  17dbe9:	test   $0x3,%bl
  17dbec:	sete   %cl
  17dbef:	test   %cl,%al
  17dbf1:	je     17dc2e <bitz::utils::blake3x4::smallest_pow_nonce+0x6e>
  17dbf3:	mov    %fs:0xffffffffffffff60,%rax
  17dbfc:	mov    %edx,0x14(%rsp)
  17dc00:	movq   $0x0,0x80(%rsp)
  17dc0c:	movq   $0xffffffffffffffff,0x78(%rsp)
  17dc15:	test   %rax,%rax
  17dc18:	je     17dc71 <bitz::utils::blake3x4::smallest_pow_nonce+0xb1>
  17dc1a:	add    $0x110,%rax
  17dc20:	mov    (%rax),%rcx
  17dc23:	lock incq (%rcx)
  17dc27:	jg     17dc83 <bitz::utils::blake3x4::smallest_pow_nonce+0xc3>
  17dc29:	jmp    17e7d5 <bitz::utils::blake3x4::smallest_pow_nonce+0xc15>
  17dc2e:	mov    %fs:0xffffffffffffff60,%rax
  17dc37:	mov    %edx,0x14(%rsp)
  17dc3b:	movq   $0x0,0x80(%rsp)
  17dc47:	movq   $0xffffffffffffffff,0x78(%rsp)
  17dc50:	test   %rax,%rax
  17dc53:	je     17dd0f <bitz::utils::blake3x4::smallest_pow_nonce+0x14f>
  17dc59:	add    $0x110,%rax
  17dc5f:	mov    (%rax),%rcx
  17dc62:	lock incq (%rcx)
  17dc66:	jg     17dd21 <bitz::utils::blake3x4::smallest_pow_nonce+0x161>
  17dc6c:	jmp    17e7d5 <bitz::utils::blake3x4::smallest_pow_nonce+0xc15>
  17dc71:	call   4846a0 <rayon_core::registry::global_registry>
  17dc76:	mov    (%rax),%rcx
  17dc79:	lock incq (%rcx)
  17dc7d:	jle    17e7d5 <bitz::utils::blake3x4::smallest_pow_nonce+0xc15>
  17dc83:	mov    (%rax),%r13
  17dc86:	lea    0x80(%rsp),%rdx
  17dc8e:	lea    0x78(%rsp),%rcx
  17dc93:	mov    %rdx,0x90(%rsp)
  17dc9b:	mov    %rcx,0x98(%rsp)
  17dca3:	lea    0x13(%rsp),%rdx
  17dca8:	lea    0x14(%rsp),%rcx
  17dcad:	mov    %rdx,0xa0(%rsp)
  17dcb5:	mov    %r14,0xa8(%rsp)
  17dcbd:	mov    %rbx,0xb0(%rsp)
  17dcc5:	mov    %rcx,0xb8(%rsp)
  17dccd:	mov    %fs:0xffffffffffffff60,%rdx
  17dcd6:	mov    0x208(%r13),%r12
  17dcdd:	mov    %rdx,0x58(%rsp)
  17dce2:	test   %rdx,%rdx
  17dce5:	je     17dda6 <bitz::utils::blake3x4::smallest_pow_nonce+0x1e6>
  17dceb:	mov    0x110(%rdx),%rax
  17dcf2:	lock incq (%rax)
  17dcf6:	jle    17e7d5 <bitz::utils::blake3x4::smallest_pow_nonce+0xc15>
  17dcfc:	mov    0x100(%rdx),%rcx
  17dd03:	mov    0x110(%rdx),%rax
  17dd0a:	jmp    17ddaa <bitz::utils::blake3x4::smallest_pow_nonce+0x1ea>
  17dd0f:	call   4846a0 <rayon_core::registry::global_registry>
  17dd14:	mov    (%rax),%rcx
  17dd17:	lock incq (%rcx)
  17dd1b:	jle    17e7d5 <bitz::utils::blake3x4::smallest_pow_nonce+0xc15>
  17dd21:	mov    (%rax),%r13
  17dd24:	lea    0x80(%rsp),%rdx
  17dd2c:	lea    0x78(%rsp),%rcx
  17dd31:	mov    %rdx,0x90(%rsp)
  17dd39:	mov    %rcx,0x98(%rsp)
  17dd41:	lea    0x13(%rsp),%rdx
  17dd46:	lea    0x14(%rsp),%rcx
  17dd4b:	mov    %rdx,0xa0(%rsp)
  17dd53:	mov    %r14,0xa8(%rsp)
  17dd5b:	mov    %rbx,0xb0(%rsp)
  17dd63:	mov    %rcx,0xb8(%rsp)
  17dd6b:	mov    %fs:0xffffffffffffff60,%rdx
  17dd74:	mov    0x208(%r13),%r12
  17dd7b:	mov    %rdx,0x58(%rsp)
  17dd80:	test   %rdx,%rdx
  17dd83:	je     17ddf5 <bitz::utils::blake3x4::smallest_pow_nonce+0x235>
  17dd85:	mov    0x110(%rdx),%rax
  17dd8c:	lock incq (%rax)
  17dd90:	jle    17e7d5 <bitz::utils::blake3x4::smallest_pow_nonce+0xc15>
  17dd96:	mov    0x100(%rdx),%rcx
  17dd9d:	mov    0x110(%rdx),%rax
  17dda4:	jmp    17ddf9 <bitz::utils::blake3x4::smallest_pow_nonce+0x239>
  17dda6:	xor    %eax,%eax
  17dda8:	xor    %ecx,%ecx
  17ddaa:	mov    %r12,0x38(%rsp)
  17ddaf:	mov    %rax,0x40(%rsp)
  17ddb4:	lea    0x0(,%r12,8),%rax
  17ddbc:	movq   $0x0,0x48(%rsp)
  17ddc5:	mov    %rcx,0x50(%rsp)
  17ddca:	lea    (%rax,%rax,4),%rbp
  17ddce:	test   %r12,%r12
  17ddd1:	je     17de48 <bitz::utils::blake3x4::smallest_pow_nonce+0x288>
  17ddd3:	mov    %rbp,%rdi
  17ddd6:	call   *0x436054(%rip)        # 5b3e30 <malloc@GLIBC_2.2.5>
  17dddc:	test   %rax,%rax
  17dddf:	je     17e7b9 <bitz::utils::blake3x4::smallest_pow_nonce+0xbf9>
  17dde5:	mov    %rax,%r15
  17dde8:	cmp    $0x8,%r12
  17ddec:	jae    17de5e <bitz::utils::blake3x4::smallest_pow_nonce+0x29e>
  17ddee:	xor    %eax,%eax
  17ddf0:	jmp    17deea <bitz::utils::blake3x4::smallest_pow_nonce+0x32a>
  17ddf5:	xor    %eax,%eax
  17ddf7:	xor    %ecx,%ecx
  17ddf9:	mov    %r12,0x38(%rsp)
  17ddfe:	mov    %rax,0x40(%rsp)
  17de03:	lea    0x0(,%r12,8),%rax
  17de0b:	movq   $0x0,0x48(%rsp)
  17de14:	mov    %rcx,0x50(%rsp)
  17de19:	lea    (%rax,%rax,4),%rbp
  17de1d:	test   %r12,%r12
  17de20:	je     17de53 <bitz::utils::blake3x4::smallest_pow_nonce+0x293>
  17de22:	mov    %rbp,%rdi
  17de25:	call   *0x436005(%rip)        # 5b3e30 <malloc@GLIBC_2.2.5>
  17de2b:	test   %rax,%rax
  17de2e:	je     17e7c8 <bitz::utils::blake3x4::smallest_pow_nonce+0xc08>
  17de34:	mov    %rax,%r15
  17de37:	cmp    $0x8,%r12
  17de3b:	jae    17e193 <bitz::utils::blake3x4::smallest_pow_nonce+0x5d3>
  17de41:	xor    %eax,%eax
  17de43:	jmp    17e21a <bitz::utils::blake3x4::smallest_pow_nonce+0x65a>
  17de48:	mov    $0x8,%r15d
  17de4e:	jmp    17df28 <bitz::utils::blake3x4::smallest_pow_nonce+0x368>
  17de53:	mov    $0x8,%r15d
  17de59:	jmp    17e258 <bitz::utils::blake3x4::smallest_pow_nonce+0x698>
  17de5e:	vmovdqa64 -0x128e68(%rip),%zmm2        # 55000 <GCC_except_table0+0xabc>
  17de68:	vpbroadcastq -0x129882(%rip),%zmm3        # 545f0 <GCC_except_table0+0xac>
  17de72:	vpbroadcastq -0x1298fc(%rip),%zmm5        # 54580 <GCC_except_table0+0x3c>
  17de7c:	lea    0x38(%rsp),%rcx
  17de81:	movabs $0x3fffffffffffff8,%rax
  17de8b:	vpxor  %xmm4,%xmm4,%xmm4
  17de8f:	vpbroadcastq %rcx,%zmm0
  17de95:	lea    0x90(%rsp),%rcx
  17de9d:	and    %r12,%rax
  17dea0:	vpbroadcastq %rcx,%zmm1
  17dea6:	mov    %rax,%rcx
  17dea9:	nopl   0x0(%rax)
  17deb0:	vpmullq %zmm3,%zmm2,%zmm6
  17deb6:	kxnorb %k0,%k0,%k1
  17deba:	vpaddq %zmm5,%zmm2,%zmm2
  17dec0:	add    $0xfffffffffffffff8,%rcx
  17dec4:	vpscatterqq %zmm0,(%r15,%zmm6,1){%k1}
  17decb:	kxnorb %k0,%k0,%k1
  17decf:	vpscatterqq %zmm1,0x8(%r15,%zmm6,1){%k1}
  17ded7:	kxnorb %k0,%k0,%k1
  17dedb:	vpscatterqq %zmm4,0x10(%r15,%zmm6,1){%k1}
  17dee3:	jne    17deb0 <bitz::utils::blake3x4::smallest_pow_nonce+0x2f0>
  17dee5:	cmp    %rax,%r12
  17dee8:	je     17df28 <bitz::utils::blake3x4::smallest_pow_nonce+0x368>
  17deea:	lea    (%rax,%rax,4),%rcx
  17deee:	mov    %r12,%rdx
  17def1:	sub    %rax,%rdx
  17def4:	lea    0x38(%rsp),%rax
  17def9:	lea    0x90(%rsp),%rsi
  17df01:	lea    0x10(%r15,%rcx,8),%rcx
  17df06:	cs nopw 0x0(%rax,%rax,1)
  17df10:	mov    %rax,-0x10(%rcx)
  17df14:	mov    %rsi,-0x8(%rcx)
  17df18:	movq   $0x0,(%rcx)
  17df1f:	add    $0x28,%rcx
  17df23:	dec    %rdx
  17df26:	jne    17df10 <bitz::utils::blake3x4::smallest_pow_nonce+0x350>
  17df28:	mov    %r12,0xc0(%rsp)
  17df30:	mov    %r15,0xc8(%rsp)
  17df38:	mov    %r12,0xd0(%rsp)
  17df40:	mov    0x208(%r13),%rax
  17df47:	mov    %r12,0x28(%rsp)
  17df4c:	mov    %rax,0x20(%rsp)
  17df51:	cmp    %r12,%rax
  17df54:	jne    17e51f <bitz::utils::blake3x4::smallest_pow_nonce+0x95f>
  17df5a:	lea    0x180(%r13),%rax
  17df61:	mov    $0x1,%ecx
  17df66:	mov    %rax,0x60(%rsp)
  17df6b:	xor    %eax,%eax
  17df6d:	lock cmpxchg %ecx,0x180(%r13)
  17df76:	jne    17e563 <bitz::utils::blake3x4::smallest_pow_nonce+0x9a3>
  17df7c:	mov    0x45552d(%rip),%rax        # 5d34b0 <std::panicking::panic_count::GLOBAL_PANIC_COUNT>
  17df83:	shl    $1,%rax
  17df86:	test   %rax,%rax
  17df89:	jne    17e585 <bitz::utils::blake3x4::smallest_pow_nonce+0x9c5>
  17df8f:	movl   $0x0,0xc(%rsp)
  17df97:	movzbl 0x184(%r13),%eax
  17df9f:	test   %al,%al
  17dfa1:	jne    17e5a3 <bitz::utils::blake3x4::smallest_pow_nonce+0x9e3>
  17dfa7:	mov    0x198(%r13),%rax
  17dfae:	mov    %r12,0x28(%rsp)
  17dfb3:	mov    %rax,0x20(%rsp)
  17dfb8:	cmp    %r12,%rax
  17dfbb:	jne    17e663 <bitz::utils::blake3x4::smallest_pow_nonce+0xaa3>
  17dfc1:	mov    %rbp,0x68(%rsp)
  17dfc6:	mov    %r13,0x18(%rsp)
  17dfcb:	mov    %r12,0x70(%rsp)
  17dfd0:	test   %r12,%r12
  17dfd3:	je     17e067 <bitz::utils::blake3x4::smallest_pow_nonce+0x4a7>
  17dfd9:	mov    0x18(%rsp),%rax
  17dfde:	mov    0x70(%rsp),%rbp
  17dfe3:	lea    0xff6(%rip),%r14        # 17efe0 <<rayon_core::job::StackJob<rayon_core::latch::LatchRef<rayon_core::latch::CountLatch>, &rayon_core::broadcast::broadcast_in<bitz::utils::blake3x4::smallest_pow_nonce_with<8, bitz::utils::blake3x4::first_pow_nonce>::{closure#0}, ()>::{closure#0}, ()> as rayon_core::job::Job>::execute>
  17dfea:	mov    %r15,%r12
  17dfed:	mov    0x190(%rax),%rbx
  17dff4:	data16 data16 cs nopw 0x0(%rax,%rax,1)
  17e000:	mov    (%rbx),%rax
  17e003:	mov    0x108(%rax),%r13
  17e00a:	mov    0x100(%rax),%rax
  17e011:	mov    0x10(%rbx),%rsi
  17e015:	mov    %r13,%rcx
  17e018:	sub    %rax,%rcx
  17e01b:	cmp    %rsi,%rcx
  17e01e:	jge    17e053 <bitz::utils::blake3x4::smallest_pow_nonce+0x493>
  17e020:	mov    0x8(%rbx),%rax
  17e024:	dec    %rsi
  17e027:	and    %r13,%rsi
  17e02a:	inc    %r13
  17e02d:	shl    $0x4,%rsi
  17e031:	mov    %r14,(%rax,%rsi,1)
  17e035:	mov    %r12,0x8(%rax,%rsi,1)
  17e03a:	mov    (%rbx),%rcx
  17e03d:	add    $0x28,%r12
  17e041:	add    $0x20,%rbx
  17e045:	dec    %rbp
  17e048:	mov    %r13,0x108(%rcx)
  17e04f:	jne    17e000 <bitz::utils::blake3x4::smallest_pow_nonce+0x440>
  17e051:	jmp    17e067 <bitz::utils::blake3x4::smallest_pow_nonce+0x4a7>
  17e053:	add    %rsi,%rsi
  17e056:	mov    %rbx,%rdi
  17e059:	vzeroupper
  17e05c:	call   17cd50 <<crossbeam_deque::deque::Worker<rayon_core::job::JobRef>>::resize>
  17e061:	mov    0x10(%rbx),%rsi
  17e065:	jmp    17e020 <bitz::utils::blake3x4::smallest_pow_nonce+0x460>
  17e067:	cmpb   $0x0,0xc(%rsp)
  17e06c:	mov    0x18(%rsp),%r13
  17e071:	jne    17e086 <bitz::utils::blake3x4::smallest_pow_nonce+0x4c6>
  17e073:	mov    0x455436(%rip),%rax        # 5d34b0 <std::panicking::panic_count::GLOBAL_PANIC_COUNT>
  17e07a:	shl    $1,%rax
  17e07d:	test   %rax,%rax
  17e080:	jne    17e77f <bitz::utils::blake3x4::smallest_pow_nonce+0xbbf>
  17e086:	mov    0x60(%rsp),%rsi
  17e08b:	xor    %eax,%eax
  17e08d:	xchg   %eax,(%rsi)
  17e08f:	mov    0x70(%rsp),%rbp
  17e094:	cmp    $0x2,%eax
  17e097:	je     17e6a7 <bitz::utils::blake3x4::smallest_pow_nonce+0xae7>
  17e09d:	mov    0x208(%r13),%r12
  17e0a4:	test   %r12,%r12
  17e0a7:	je     17e0d6 <bitz::utils::blake3x4::smallest_pow_nonce+0x516>
  17e0a9:	lea    0x1d8(%r13),%rbx
  17e0b0:	xor    %r14d,%r14d
  17e0b3:	data16 data16 data16 cs nopw 0x0(%rax,%rax,1)
  17e0c0:	mov    %rbx,%rdi
  17e0c3:	mov    %r14,%rsi
  17e0c6:	vzeroupper
  17e0c9:	call   480450 <<rayon_core::sleep::Sleep>::wake_specific_thread>
  17e0ce:	inc    %r14
  17e0d1:	cmp    %r14,%r12
  17e0d4:	jne    17e0c0 <bitz::utils::blake3x4::smallest_pow_nonce+0x500>
  17e0d6:	cmpq   $0x0,0x40(%rsp)
  17e0dc:	lea    0x48(%rsp),%rsi
  17e0e1:	je     17e109 <bitz::utils::blake3x4::smallest_pow_nonce+0x549>
  17e0e3:	cmpq   $0x0,0x58(%rsp)
  17e0e9:	je     17e705 <bitz::utils::blake3x4::smallest_pow_nonce+0xb45>
  17e0ef:	mov    0x48(%rsp),%rax
  17e0f4:	cmp    $0x3,%rax
  17e0f8:	je     17e114 <bitz::utils::blake3x4::smallest_pow_nonce+0x554>
  17e0fa:	mov    0x58(%rsp),%rdi
  17e0ff:	vzeroupper
  17e102:	call   47f7e0 <<rayon_core::registry::WorkerThread>::wait_until_cold>
  17e107:	jmp    17e114 <bitz::utils::blake3x4::smallest_pow_nonce+0x554>
  17e109:	mov    %rsi,%rdi
  17e10c:	vzeroupper
  17e10f:	call   4811b0 <<rayon_core::latch::LockLatch>::wait>
  17e114:	test   %rbp,%rbp
  17e117:	je     17e14a <bitz::utils::blake3x4::smallest_pow_nonce+0x58a>
  17e119:	mov    0x68(%rsp),%rax
  17e11e:	lea    (%r15,%rax,1),%rbp
  17e122:	mov    %r15,%rax
  17e125:	mov    0x10(%rax),%rcx
  17e129:	lea    0x28(%rax),%r14
  17e12d:	cmp    $0x1,%rcx
  17e131:	jne    17e174 <bitz::utils::blake3x4::smallest_pow_nonce+0x5b4>
  17e133:	addq   $0xffffffffffffffd8,0x68(%rsp)
  17e139:	mov    %r14,%rax
  17e13c:	jne    17e125 <bitz::utils::blake3x4::smallest_pow_nonce+0x565>
  17e13e:	mov    %r15,%rdi
  17e141:	vzeroupper
  17e144:	call   *0x435cf6(%rip)        # 5b3e40 <free@GLIBC_2.2.5>
  17e14a:	mov    0x40(%rsp),%rax
  17e14f:	test   %rax,%rax
  17e152:	je     17e49b <bitz::utils::blake3x4::smallest_pow_nonce+0x8db>
  17e158:	lock decq (%rax)
  17e15c:	jne    17e49b <bitz::utils::blake3x4::smallest_pow_nonce+0x8db>
  17e162:	mov    0x40(%rsp),%rdi
  17e167:	vzeroupper
  17e16a:	call   480d30 <<alloc::sync::Arc<rayon_core::registry::Registry>>::drop_slow>
  17e16f:	jmp    17e49b <bitz::utils::blake3x4::smallest_pow_nonce+0x8db>
  17e174:	cmp    $0x2,%rcx
  17e178:	jne    17e745 <bitz::utils::blake3x4::smallest_pow_nonce+0xb85>
  17e17e:	mov    0x18(%rax),%rdi
  17e182:	mov    0x20(%rax),%rsi
  17e186:	vzeroupper
  17e189:	call   484a10 <rayon_core::unwind::resume_unwinding>
  17e18e:	jmp    17e7d5 <bitz::utils::blake3x4::smallest_pow_nonce+0xc15>
  17e193:	vmovdqa64 -0x12919d(%rip),%zmm2        # 55000 <GCC_except_table0+0xabc>
  17e19d:	vpbroadcastq -0x129bb7(%rip),%zmm3        # 545f0 <GCC_except_table0+0xac>
  17e1a7:	vpbroadcastq -0x129c31(%rip),%zmm5        # 54580 <GCC_except_table0+0x3c>
  17e1b1:	lea    0x38(%rsp),%rcx
  17e1b6:	movabs $0x3fffffffffffff8,%rax
  17e1c0:	vpxor  %xmm4,%xmm4,%xmm4
  17e1c4:	vpbroadcastq %rcx,%zmm0
  17e1ca:	lea    0x90(%rsp),%rcx
  17e1d2:	and    %r12,%rax
  17e1d5:	vpbroadcastq %rcx,%zmm1
  17e1db:	mov    %rax,%rcx
  17e1de:	xchg   %ax,%ax
  17e1e0:	vpmullq %zmm3,%zmm2,%zmm6
  17e1e6:	kxnorb %k0,%k0,%k1
  17e1ea:	vpaddq %zmm5,%zmm2,%zmm2
  17e1f0:	add    $0xfffffffffffffff8,%rcx
  17e1f4:	vpscatterqq %zmm0,(%r15,%zmm6,1){%k1}
  17e1fb:	kxnorb %k0,%k0,%k1
  17e1ff:	vpscatterqq %zmm1,0x8(%r15,%zmm6,1){%k1}
  17e207:	kxnorb %k0,%k0,%k1
  17e20b:	vpscatterqq %zmm4,0x10(%r15,%zmm6,1){%k1}
  17e213:	jne    17e1e0 <bitz::utils::blake3x4::smallest_pow_nonce+0x620>
  17e215:	cmp    %rax,%r12
  17e218:	je     17e258 <bitz::utils::blake3x4::smallest_pow_nonce+0x698>
  17e21a:	lea    (%rax,%rax,4),%rcx
  17e21e:	mov    %r12,%rdx
  17e221:	sub    %rax,%rdx
  17e224:	lea    0x38(%rsp),%rax
  17e229:	lea    0x90(%rsp),%rsi
  17e231:	lea    0x10(%r15,%rcx,8),%rcx
  17e236:	cs nopw 0x0(%rax,%rax,1)
  17e240:	mov    %rax,-0x10(%rcx)
  17e244:	mov    %rsi,-0x8(%rcx)
  17e248:	movq   $0x0,(%rcx)
  17e24f:	add    $0x28,%rcx
  17e253:	dec    %rdx
  17e256:	jne    17e240 <bitz::utils::blake3x4::smallest_pow_nonce+0x680>
  17e258:	mov    %r12,0xc0(%rsp)
  17e260:	mov    %r15,0xc8(%rsp)
  17e268:	mov    %r12,0xd0(%rsp)
  17e270:	mov    0x208(%r13),%rax
  17e277:	mov    %r12,0x28(%rsp)
  17e27c:	mov    %rax,0x20(%rsp)
  17e281:	cmp    %r12,%rax
  17e284:	jne    17e541 <bitz::utils::blake3x4::smallest_pow_nonce+0x981>
  17e28a:	lea    0x180(%r13),%rax
  17e291:	mov    $0x1,%ecx
  17e296:	mov    %rax,0x60(%rsp)
  17e29b:	xor    %eax,%eax
  17e29d:	lock cmpxchg %ecx,0x180(%r13)
  17e2a6:	jne    17e5e3 <bitz::utils::blake3x4::smallest_pow_nonce+0xa23>
  17e2ac:	mov    0x4551fd(%rip),%rax        # 5d34b0 <std::panicking::panic_count::GLOBAL_PANIC_COUNT>
  17e2b3:	shl    $1,%rax
  17e2b6:	test   %rax,%rax
  17e2b9:	jne    17e605 <bitz::utils::blake3x4::smallest_pow_nonce+0xa45>
  17e2bf:	movl   $0x0,0xc(%rsp)
  17e2c7:	movzbl 0x184(%r13),%eax
  17e2cf:	test   %al,%al
  17e2d1:	jne    17e623 <bitz::utils::blake3x4::smallest_pow_nonce+0xa63>
  17e2d7:	mov    0x198(%r13),%rax
  17e2de:	mov    %r12,0x28(%rsp)
  17e2e3:	mov    %rax,0x20(%rsp)
  17e2e8:	cmp    %r12,%rax
  17e2eb:	jne    17e685 <bitz::utils::blake3x4::smallest_pow_nonce+0xac5>
  17e2f1:	mov    %rbp,0x68(%rsp)
  17e2f6:	mov    %r13,0x18(%rsp)
  17e2fb:	mov    %r12,0x70(%rsp)
  17e300:	test   %r12,%r12
  17e303:	je     17e397 <bitz::utils::blake3x4::smallest_pow_nonce+0x7d7>
  17e309:	mov    0x18(%rsp),%rax
  17e30e:	mov    0x70(%rsp),%rbp
  17e313:	lea    0x746(%rip),%r14        # 17ea60 <<rayon_core::job::StackJob<rayon_core::latch::LatchRef<rayon_core::latch::CountLatch>, &rayon_core::broadcast::broadcast_in<bitz::utils::blake3x4::smallest_pow_nonce_with<10, bitz::utils::blake3x4::first_pow_nonce>::{closure#0}, ()>::{closure#0}, ()> as rayon_core::job::Job>::execute>
  17e31a:	mov    %r15,%r12
  17e31d:	mov    0x190(%rax),%rbx
  17e324:	data16 data16 cs nopw 0x0(%rax,%rax,1)
  17e330:	mov    (%rbx),%rax
  17e333:	mov    0x108(%rax),%r13
  17e33a:	mov    0x100(%rax),%rax
  17e341:	mov    0x10(%rbx),%rsi
  17e345:	mov    %r13,%rcx
  17e348:	sub    %rax,%rcx
  17e34b:	cmp    %rsi,%rcx
  17e34e:	jge    17e383 <bitz::utils::blake3x4::smallest_pow_nonce+0x7c3>
  17e350:	mov    0x8(%rbx),%rax
  17e354:	dec    %rsi
  17e357:	and    %r13,%rsi
  17e35a:	inc    %r13
  17e35d:	shl    $0x4,%rsi
  17e361:	mov    %r14,(%rax,%rsi,1)
  17e365:	mov    %r12,0x8(%rax,%rsi,1)
  17e36a:	mov    (%rbx),%rcx
  17e36d:	add    $0x28,%r12
  17e371:	add    $0x20,%rbx
  17e375:	dec    %rbp
  17e378:	mov    %r13,0x108(%rcx)
  17e37f:	jne    17e330 <bitz::utils::blake3x4::smallest_pow_nonce+0x770>
  17e381:	jmp    17e397 <bitz::utils::blake3x4::smallest_pow_nonce+0x7d7>
  17e383:	add    %rsi,%rsi
  17e386:	mov    %rbx,%rdi
  17e389:	vzeroupper
  17e38c:	call   17cd50 <<crossbeam_deque::deque::Worker<rayon_core::job::JobRef>>::resize>
  17e391:	mov    0x10(%rbx),%rsi
  17e395:	jmp    17e350 <bitz::utils::blake3x4::smallest_pow_nonce+0x790>
  17e397:	cmpb   $0x0,0xc(%rsp)
  17e39c:	mov    0x18(%rsp),%r13
  17e3a1:	jne    17e3b6 <bitz::utils::blake3x4::smallest_pow_nonce+0x7f6>
  17e3a3:	mov    0x455106(%rip),%rax        # 5d34b0 <std::panicking::panic_count::GLOBAL_PANIC_COUNT>
  17e3aa:	shl    $1,%rax
  17e3ad:	test   %rax,%rax
  17e3b0:	jne    17e79c <bitz::utils::blake3x4::smallest_pow_nonce+0xbdc>
  17e3b6:	mov    0x60(%rsp),%rsi
  17e3bb:	xor    %eax,%eax
  17e3bd:	xchg   %eax,(%rsi)
  17e3bf:	mov    0x70(%rsp),%rbp
  17e3c4:	cmp    $0x2,%eax
  17e3c7:	je     17e6d6 <bitz::utils::blake3x4::smallest_pow_nonce+0xb16>
  17e3cd:	mov    0x208(%r13),%r12
  17e3d4:	test   %r12,%r12
  17e3d7:	je     17e406 <bitz::utils::blake3x4::smallest_pow_nonce+0x846>
  17e3d9:	lea    0x1d8(%r13),%rbx
  17e3e0:	xor    %r14d,%r14d
  17e3e3:	data16 data16 data16 cs nopw 0x0(%rax,%rax,1)
  17e3f0:	mov    %rbx,%rdi
  17e3f3:	mov    %r14,%rsi
  17e3f6:	vzeroupper
  17e3f9:	call   480450 <<rayon_core::sleep::Sleep>::wake_specific_thread>
  17e3fe:	inc    %r14
  17e401:	cmp    %r14,%r12
  17e404:	jne    17e3f0 <bitz::utils::blake3x4::smallest_pow_nonce+0x830>
  17e406:	cmpq   $0x0,0x40(%rsp)
  17e40c:	lea    0x48(%rsp),%rsi
  17e411:	je     17e439 <bitz::utils::blake3x4::smallest_pow_nonce+0x879>
  17e413:	cmpq   $0x0,0x58(%rsp)
  17e419:	je     17e725 <bitz::utils::blake3x4::smallest_pow_nonce+0xb65>
  17e41f:	mov    0x48(%rsp),%rax
  17e424:	cmp    $0x3,%rax
  17e428:	je     17e444 <bitz::utils::blake3x4::smallest_pow_nonce+0x884>
  17e42a:	mov    0x58(%rsp),%rdi
  17e42f:	vzeroupper
  17e432:	call   47f7e0 <<rayon_core::registry::WorkerThread>::wait_until_cold>
  17e437:	jmp    17e444 <bitz::utils::blake3x4::smallest_pow_nonce+0x884>
  17e439:	mov    %rsi,%rdi
  17e43c:	vzeroupper
  17e43f:	call   4811b0 <<rayon_core::latch::LockLatch>::wait>
  17e444:	test   %rbp,%rbp
  17e447:	je     17e47e <bitz::utils::blake3x4::smallest_pow_nonce+0x8be>
  17e449:	mov    0x68(%rsp),%rax
  17e44e:	lea    (%r15,%rax,1),%rbp
  17e452:	mov    %r15,%rax
  17e455:	mov    0x10(%rax),%rcx
  17e459:	lea    0x28(%rax),%r14
  17e45d:	cmp    $0x1,%rcx
  17e461:	jne    17e500 <bitz::utils::blake3x4::smallest_pow_nonce+0x940>
  17e467:	addq   $0xffffffffffffffd8,0x68(%rsp)
  17e46d:	mov    %r14,%rax
  17e470:	jne    17e455 <bitz::utils::blake3x4::smallest_pow_nonce+0x895>
  17e472:	mov    %r15,%rdi
  17e475:	vzeroupper
  17e478:	call   *0x4359c2(%rip)        # 5b3e40 <free@GLIBC_2.2.5>
  17e47e:	mov    0x40(%rsp),%rax
  17e483:	test   %rax,%rax
  17e486:	je     17e49b <bitz::utils::blake3x4::smallest_pow_nonce+0x8db>
  17e488:	lock decq (%rax)
  17e48c:	jne    17e49b <bitz::utils::blake3x4::smallest_pow_nonce+0x8db>
  17e48e:	mov    0x40(%rsp),%rdi
  17e493:	vzeroupper
  17e496:	call   480d30 <<alloc::sync::Arc<rayon_core::registry::Registry>>::drop_slow>
  17e49b:	lock decq 0x0(%r13)
  17e4a0:	mov    (%rsp),%rbx
  17e4a4:	mov    0x88(%rsp),%r14
  17e4ac:	jne    17e4b9 <bitz::utils::blake3x4::smallest_pow_nonce+0x8f9>
  17e4ae:	mov    %r13,%rdi
  17e4b1:	vzeroupper
  17e4b4:	call   480d30 <<alloc::sync::Arc<rayon_core::registry::Registry>>::drop_slow>
  17e4b9:	mov    0x78(%rsp),%rdx
  17e4be:	mov    $0x1,%eax
  17e4c3:	cmp    $0xffffffffffffffff,%rdx
  17e4c7:	jne    17e4eb <bitz::utils::blake3x4::smallest_pow_nonce+0x92b>
  17e4c9:	mov    0x14(%rsp),%ecx
  17e4cd:	mov    %rbx,%rsi
  17e4d0:	mov    %rdx,%rbx
  17e4d3:	mov    $0xffffffffffffffff,%rdx
  17e4da:	mov    %r14,%rdi
  17e4dd:	vzeroupper
  17e4e0:	call   17eca0 <bitz::utils::blake3x4::pow_ok>
  17e4e5:	movzbl %al,%eax
  17e4e8:	mov    %rbx,%rdx
  17e4eb:	add    $0xd8,%rsp
  17e4f2:	pop    %rbx
  17e4f3:	pop    %r12
  17e4f5:	pop    %r13
  17e4f7:	pop    %r14
  17e4f9:	pop    %r15
  17e4fb:	pop    %rbp
  17e4fc:	vzeroupper
  17e4ff:	ret
  17e500:	cmp    $0x2,%rcx
  17e504:	jne    17e762 <bitz::utils::blake3x4::smallest_pow_nonce+0xba2>
  17e50a:	mov    0x18(%rax),%rdi
  17e50e:	mov    0x20(%rax),%rsi
  17e512:	vzeroupper
  17e515:	call   484a10 <rayon_core::unwind::resume_unwinding>
  17e51a:	jmp    17e7d5 <bitz::utils::blake3x4::smallest_pow_nonce+0xc15>
  17e51f:	lea    0x421ef2(%rip),%r9        # 5a0418 <falcon_hybrid::main::__CALLSITE::META+0x1780>
  17e526:	lea    0x20(%rsp),%rsi
  17e52b:	lea    0x28(%rsp),%rdx
  17e530:	xor    %edi,%edi
  17e532:	xor    %ecx,%ecx
  17e534:	vzeroupper
  17e537:	call   3cd932 <core::panicking::assert_failed::<usize, usize>>
  17e53c:	jmp    17e7d5 <bitz::utils::blake3x4::smallest_pow_nonce+0xc15>
  17e541:	lea    0x421ed0(%rip),%r9        # 5a0418 <falcon_hybrid::main::__CALLSITE::META+0x1780>
  17e548:	lea    0x20(%rsp),%rsi
  17e54d:	lea    0x28(%rsp),%rdx
  17e552:	xor    %edi,%edi
  17e554:	xor    %ecx,%ecx
  17e556:	vzeroupper
  17e559:	call   3cd932 <core::panicking::assert_failed::<usize, usize>>
  17e55e:	jmp    17e7d5 <bitz::utils::blake3x4::smallest_pow_nonce+0xc15>
  17e563:	lea    0x180(%r13),%rdi
  17e56a:	vzeroupper
  17e56d:	call   48e110 <<std::sys::sync::mutex::futex::Mutex>::lock_contended>
  17e572:	mov    0x454f37(%rip),%rax        # 5d34b0 <std::panicking::panic_count::GLOBAL_PANIC_COUNT>
  17e579:	shl    $1,%rax
  17e57c:	test   %rax,%rax
  17e57f:	je     17df8f <bitz::utils::blake3x4::smallest_pow_nonce+0x3cf>
  17e585:	vzeroupper
  17e588:	call   48dfd0 <std::panicking::panic_count::is_zero_slow_path>
  17e58d:	xor    $0x1,%al
  17e58f:	mov    %eax,0xc(%rsp)
  17e593:	movzbl 0x184(%r13),%eax
  17e59b:	test   %al,%al
  17e59d:	je     17dfa7 <bitz::utils::blake3x4::smallest_pow_nonce+0x3e7>
  17e5a3:	lea    0x180(%r13),%rax
  17e5aa:	mov    %rax,0x28(%rsp)
  17e5af:	mov    0xc(%rsp),%eax
  17e5b3:	mov    %al,0x30(%rsp)
  17e5b7:	lea    -0xcdc49(%rip),%rdi        # b0975 <tracing_core::dispatcher::NO_SUBSCRIBER+0x86>
  17e5be:	lea    0x4251e3(%rip),%rcx        # 5a37a8 <falcon_hybrid::main::__CALLSITE::META+0x4b10>
  17e5c5:	lea    0x421e64(%rip),%r8        # 5a0430 <falcon_hybrid::main::__CALLSITE::META+0x1798>
  17e5cc:	lea    0x28(%rsp),%rdx
  17e5d1:	mov    $0x2b,%esi
  17e5d6:	vzeroupper
  17e5d9:	call   3d0be0 <core::result::unwrap_failed>
  17e5de:	jmp    17e7d5 <bitz::utils::blake3x4::smallest_pow_nonce+0xc15>
  17e5e3:	lea    0x180(%r13),%rdi
  17e5ea:	vzeroupper
  17e5ed:	call   48e110 <<std::sys::sync::mutex::futex::Mutex>::lock_contended>
  17e5f2:	mov    0x454eb7(%rip),%rax        # 5d34b0 <std::panicking::panic_count::GLOBAL_PANIC_COUNT>
  17e5f9:	shl    $1,%rax
  17e5fc:	test   %rax,%rax
  17e5ff:	je     17e2bf <bitz::utils::blake3x4::smallest_pow_nonce+0x6ff>
  17e605:	vzeroupper
  17e608:	call   48dfd0 <std::panicking::panic_count::is_zero_slow_path>
  17e60d:	xor    $0x1,%al
  17e60f:	mov    %eax,0xc(%rsp)
  17e613:	movzbl 0x184(%r13),%eax
  17e61b:	test   %al,%al
  17e61d:	je     17e2d7 <bitz::utils::blake3x4::smallest_pow_nonce+0x717>
  17e623:	lea    0x180(%r13),%rax
  17e62a:	mov    %rax,0x28(%rsp)
  17e62f:	mov    0xc(%rsp),%eax
  17e633:	mov    %al,0x30(%rsp)
  17e637:	lea    -0xcdcc9(%rip),%rdi        # b0975 <tracing_core::dispatcher::NO_SUBSCRIBER+0x86>
  17e63e:	lea    0x425163(%rip),%rcx        # 5a37a8 <falcon_hybrid::main::__CALLSITE::META+0x4b10>
  17e645:	lea    0x421de4(%rip),%r8        # 5a0430 <falcon_hybrid::main::__CALLSITE::META+0x1798>
  17e64c:	lea    0x28(%rsp),%rdx
  17e651:	mov    $0x2b,%esi
  17e656:	vzeroupper
  17e659:	call   3d0be0 <core::result::unwrap_failed>
  17e65e:	jmp    17e7d5 <bitz::utils::blake3x4::smallest_pow_nonce+0xc15>
  17e663:	lea    0x421dde(%rip),%r9        # 5a0448 <falcon_hybrid::main::__CALLSITE::META+0x17b0>
  17e66a:	lea    0x20(%rsp),%rsi
  17e66f:	lea    0x28(%rsp),%rdx
  17e674:	xor    %edi,%edi
  17e676:	xor    %ecx,%ecx
  17e678:	vzeroupper
  17e67b:	call   3cd932 <core::panicking::assert_failed::<usize, usize>>
  17e680:	jmp    17e7d5 <bitz::utils::blake3x4::smallest_pow_nonce+0xc15>
  17e685:	lea    0x421dbc(%rip),%r9        # 5a0448 <falcon_hybrid::main::__CALLSITE::META+0x17b0>
  17e68c:	lea    0x20(%rsp),%rsi
  17e691:	lea    0x28(%rsp),%rdx
  17e696:	xor    %edi,%edi
  17e698:	xor    %ecx,%ecx
  17e69a:	vzeroupper
  17e69d:	call   3cd932 <core::panicking::assert_failed::<usize, usize>>
  17e6a2:	jmp    17e7d5 <bitz::utils::blake3x4::smallest_pow_nonce+0xc15>
  17e6a7:	mov    $0xca,%edi
  17e6ac:	mov    $0x81,%edx
  17e6b1:	mov    $0x1,%ecx
  17e6b6:	xor    %eax,%eax
  17e6b8:	vzeroupper
  17e6bb:	call   *0x435797(%rip)        # 5b3e58 <syscall@GLIBC_2.2.5>
  17e6c1:	mov    0x208(%r13),%r12
  17e6c8:	test   %r12,%r12
  17e6cb:	jne    17e0a9 <bitz::utils::blake3x4::smallest_pow_nonce+0x4e9>
  17e6d1:	jmp    17e0d6 <bitz::utils::blake3x4::smallest_pow_nonce+0x516>
  17e6d6:	mov    $0xca,%edi
  17e6db:	mov    $0x81,%edx
  17e6e0:	mov    $0x1,%ecx
  17e6e5:	xor    %eax,%eax
  17e6e7:	vzeroupper
  17e6ea:	call   *0x435768(%rip)        # 5b3e58 <syscall@GLIBC_2.2.5>
  17e6f0:	mov    0x208(%r13),%r12
  17e6f7:	test   %r12,%r12
  17e6fa:	jne    17e3d9 <bitz::utils::blake3x4::smallest_pow_nonce+0x819>
  17e700:	jmp    17e406 <bitz::utils::blake3x4::smallest_pow_nonce+0x846>
  17e705:	lea    -0xcfc1d(%rip),%rdi        # aeaef <flock_core::genus95_curve_code::sage_data::GAMMA_SLOT_MASKS+0x331d7>
  17e70c:	lea    0x42ff8d(%rip),%rdx        # 5ae6a0 <<bitz::piop::spartan::falcon1024_ct::hybrid_sumcheck::State<_> as bitz::sumcheck::inner::input::State<field::binary::Gf128Ops>>::fold::__CALLSITE::META+0x6f30>
  17e713:	mov    $0xc,%esi
  17e718:	vzeroupper
  17e71b:	call   3d0ba0 <core::option::expect_failed>
  17e720:	jmp    17e7d5 <bitz::utils::blake3x4::smallest_pow_nonce+0xc15>
  17e725:	lea    -0xcfc3d(%rip),%rdi        # aeaef <flock_core::genus95_curve_code::sage_data::GAMMA_SLOT_MASKS+0x331d7>
  17e72c:	lea    0x42ff6d(%rip),%rdx        # 5ae6a0 <<bitz::piop::spartan::falcon1024_ct::hybrid_sumcheck::State<_> as bitz::sumcheck::inner::input::State<field::binary::Gf128Ops>>::fold::__CALLSITE::META+0x6f30>
  17e733:	mov    $0xc,%esi
  17e738:	vzeroupper
  17e73b:	call   3d0ba0 <core::option::expect_failed>
  17e740:	jmp    17e7d5 <bitz::utils::blake3x4::smallest_pow_nonce+0xc15>
  17e745:	lea    -0xce475(%rip),%rdi        # b02d7 <core::unicode::unicode_data::white_space::WHITESPACE_MAP+0x483>
  17e74c:	lea    0x42d17d(%rip),%rdx        # 5ab8d0 <<bitz::piop::spartan::falcon1024_ct::hybrid_sumcheck::State<_> as bitz::sumcheck::inner::input::State<field::binary::Gf128Ops>>::fold::__CALLSITE::META+0x4160>
  17e753:	mov    $0x28,%esi
  17e758:	vzeroupper
  17e75b:	call   3ce5b0 <core::panicking::panic>
  17e760:	jmp    17e7d5 <bitz::utils::blake3x4::smallest_pow_nonce+0xc15>
  17e762:	lea    -0xce492(%rip),%rdi        # b02d7 <core::unicode::unicode_data::white_space::WHITESPACE_MAP+0x483>
  17e769:	lea    0x42d160(%rip),%rdx        # 5ab8d0 <<bitz::piop::spartan::falcon1024_ct::hybrid_sumcheck::State<_> as bitz::sumcheck::inner::input::State<field::binary::Gf128Ops>>::fold::__CALLSITE::META+0x4160>
  17e770:	mov    $0x28,%esi
  17e775:	vzeroupper
  17e778:	call   3ce5b0 <core::panicking::panic>
  17e77d:	jmp    17e7d5 <bitz::utils::blake3x4::smallest_pow_nonce+0xc15>
  17e77f:	vzeroupper
  17e782:	call   48dfd0 <std::panicking::panic_count::is_zero_slow_path>
  17e787:	test   %al,%al
  17e789:	jne    17e086 <bitz::utils::blake3x4::smallest_pow_nonce+0x4c6>
  17e78f:	movb   $0x1,0x184(%r13)
  17e797:	jmp    17e086 <bitz::utils::blake3x4::smallest_pow_nonce+0x4c6>
  17e79c:	vzeroupper
  17e79f:	call   48dfd0 <std::panicking::panic_count::is_zero_slow_path>
  17e7a4:	test   %al,%al
  17e7a6:	jne    17e3b6 <bitz::utils::blake3x4::smallest_pow_nonce+0x7f6>
  17e7ac:	movb   $0x1,0x184(%r13)
  17e7b4:	jmp    17e3b6 <bitz::utils::blake3x4::smallest_pow_nonce+0x7f6>
  17e7b9:	mov    $0x8,%edi
  17e7be:	mov    %rbp,%rsi
  17e7c1:	call   17306b <alloc::raw_vec::handle_error>
  17e7c6:	jmp    17e7d5 <bitz::utils::blake3x4::smallest_pow_nonce+0xc15>
  17e7c8:	mov    $0x8,%edi
  17e7cd:	mov    %rbp,%rsi
  17e7d0:	call   17306b <alloc::raw_vec::handle_error>
  17e7d5:	ud2
  17e7d7:	mov    0x18(%rsp),%r13
  17e7dc:	mov    0x60(%rsp),%rdi
  17e7e1:	jmp    17e81e <bitz::utils::blake3x4::smallest_pow_nonce+0xc5e>
  17e7e3:	mov    0x18(%rsp),%r13
  17e7e8:	mov    0x60(%rsp),%rdi
  17e7ed:	jmp    17e835 <bitz::utils::blake3x4::smallest_pow_nonce+0xc75>
  17e7ef:	mov    %rax,(%rsp)
  17e7f3:	jmp    17e8d1 <bitz::utils::blake3x4::smallest_pow_nonce+0xd11>
  17e7f8:	mov    %rax,(%rsp)
  17e7fc:	jmp    17e9b7 <bitz::utils::blake3x4::smallest_pow_nonce+0xdf7>
  17e801:	mov    %rax,(%rsp)
  17e805:	jmp    17e8b7 <bitz::utils::blake3x4::smallest_pow_nonce+0xcf7>
  17e80a:	mov    %rax,(%rsp)
  17e80e:	jmp    17e99d <bitz::utils::blake3x4::smallest_pow_nonce+0xddd>
  17e813:	jmp    17e865 <bitz::utils::blake3x4::smallest_pow_nonce+0xca5>
  17e815:	jmp    17e878 <bitz::utils::blake3x4::smallest_pow_nonce+0xcb8>
  17e817:	lea    0x180(%r13),%rdi
  17e81e:	mov    %rax,(%rsp)
  17e822:	movzbl 0xc(%rsp),%esi
  17e827:	call   213b80 <core::ptr::drop_glue::<std::sync::poison::PoisonError<std::sync::poison::mutex::MutexGuard<bitz::hybrid::sumcheck::Scratch>>>>
  17e82c:	jmp    17e869 <bitz::utils::blake3x4::smallest_pow_nonce+0xca9>
  17e82e:	lea    0x180(%r13),%rdi
  17e835:	mov    %rax,(%rsp)
  17e839:	movzbl 0xc(%rsp),%esi
  17e83e:	call   213b80 <core::ptr::drop_glue::<std::sync::poison::PoisonError<std::sync::poison::mutex::MutexGuard<bitz::hybrid::sumcheck::Scratch>>>>
  17e843:	jmp    17e87c <bitz::utils::blake3x4::smallest_pow_nonce+0xcbc>
  17e845:	lea    0x28(%rsp),%rdi
  17e84a:	mov    %rax,(%rsp)
  17e84e:	call   4e1010 <core::ptr::drop_glue::<std::sync::poison::PoisonError<std::sync::poison::mutex::MutexGuard<[core::option::Option<alloc::boxed::Box<dyn core::ops::function::FnMut<(u32, bool, bool), Output = ()> + core::marker::Send + core::marker::Sync>>; 1]>>>>
  17e853:	jmp    17e869 <bitz::utils::blake3x4::smallest_pow_nonce+0xca9>
  17e855:	lea    0x28(%rsp),%rdi
  17e85a:	mov    %rax,(%rsp)
  17e85e:	call   4e1010 <core::ptr::drop_glue::<std::sync::poison::PoisonError<std::sync::poison::mutex::MutexGuard<[core::option::Option<alloc::boxed::Box<dyn core::ops::function::FnMut<(u32, bool, bool), Output = ()> + core::marker::Send + core::marker::Sync>>; 1]>>>>
  17e863:	jmp    17e87c <bitz::utils::blake3x4::smallest_pow_nonce+0xcbc>
  17e865:	mov    %rax,(%rsp)
  17e869:	lea    0xc0(%rsp),%rdi
  17e871:	call   17f220 <core::ptr::drop_glue::<alloc::vec::Vec<rayon_core::job::StackJob<rayon_core::latch::LatchRef<rayon_core::latch::CountLatch>, &rayon_core::broadcast::broadcast_in<bitz::utils::blake3x4::smallest_pow_nonce_with<8, bitz::utils::blake3x4::first_pow_nonce>::{closure#0}, ()>::{closure#0}, ()>>>>
  17e876:	jmp    17e8b7 <bitz::utils::blake3x4::smallest_pow_nonce+0xcf7>
  17e878:	mov    %rax,(%rsp)
  17e87c:	lea    0xc0(%rsp),%rdi
  17e884:	call   17f220 <core::ptr::drop_glue::<alloc::vec::Vec<rayon_core::job::StackJob<rayon_core::latch::LatchRef<rayon_core::latch::CountLatch>, &rayon_core::broadcast::broadcast_in<bitz::utils::blake3x4::smallest_pow_nonce_with<8, bitz::utils::blake3x4::first_pow_nonce>::{closure#0}, ()>::{closure#0}, ()>>>>
  17e889:	jmp    17e99d <bitz::utils::blake3x4::smallest_pow_nonce+0xddd>
  17e88e:	mov    %rax,(%rsp)
  17e892:	sub    %r14,%rbp
  17e895:	movabs $0xcccccccccccccccd,%rax
  17e89f:	mov    %rbp,%rdx
  17e8a2:	mulx   %rax,%rbp,%rbp
  17e8a7:	jne    17e8e9 <bitz::utils::blake3x4::smallest_pow_nonce+0xd29>
  17e8a9:	mov    %r15,%rdi
  17e8ac:	call   *0x43558e(%rip)        # 5b3e40 <free@GLIBC_2.2.5>
  17e8b2:	mov    0x18(%rsp),%r13
  17e8b7:	mov    0x40(%rsp),%rax
  17e8bc:	test   %rax,%rax
  17e8bf:	je     17e8d1 <bitz::utils::blake3x4::smallest_pow_nonce+0xd11>
  17e8c1:	lock decq (%rax)
  17e8c5:	jne    17e8d1 <bitz::utils::blake3x4::smallest_pow_nonce+0xd11>
  17e8c7:	mov    0x40(%rsp),%rdi
  17e8cc:	call   480d30 <<alloc::sync::Arc<rayon_core::registry::Registry>>::drop_slow>
  17e8d1:	lock decq 0x0(%r13)
  17e8d6:	jne    17e9c6 <bitz::utils::blake3x4::smallest_pow_nonce+0xe06>
  17e8dc:	mov    %r13,%rdi
  17e8df:	call   480d30 <<alloc::sync::Arc<rayon_core::registry::Registry>>::drop_slow>
  17e8e4:	jmp    17e9c6 <bitz::utils::blake3x4::smallest_pow_nonce+0xe06>
  17e8e9:	mov    0x435550(%rip),%r12        # 5b3e40 <free@GLIBC_2.2.5>
  17e8f0:	shr    $0x5,%rbp
  17e8f4:	add    $0x28,%r14
  17e8f8:	jmp    17e909 <bitz::utils::blake3x4::smallest_pow_nonce+0xd49>
  17e8fa:	nopw   0x0(%rax,%rax,1)
  17e900:	add    $0x28,%r14
  17e904:	dec    %rbp
  17e907:	je     17e8a9 <bitz::utils::blake3x4::smallest_pow_nonce+0xce9>
  17e909:	cmpl   $0x2,-0x18(%r14)
  17e90e:	jb     17e900 <bitz::utils::blake3x4::smallest_pow_nonce+0xd40>
  17e910:	mov    -0x8(%r14),%rbx
  17e914:	mov    -0x10(%r14),%r13
  17e918:	mov    (%rbx),%rax
  17e91b:	test   %rax,%rax
  17e91e:	je     17e925 <bitz::utils::blake3x4::smallest_pow_nonce+0xd65>
  17e920:	mov    %r13,%rdi
  17e923:	call   *%rax
  17e925:	cmpq   $0x0,0x8(%rbx)
  17e92a:	je     17e900 <bitz::utils::blake3x4::smallest_pow_nonce+0xd40>
  17e92c:	mov    %r13,%rdi
  17e92f:	call   *%r12
  17e932:	jmp    17e900 <bitz::utils::blake3x4::smallest_pow_nonce+0xd40>
  17e934:	call   3d0ca3 <core::panicking::panic_in_cleanup>
  17e939:	cmpq   $0x0,0x8(%rbx)
  17e93e:	je     17e950 <bitz::utils::blake3x4::smallest_pow_nonce+0xd90>
  17e940:	mov    %r13,%rdi
  17e943:	call   *0x4354f7(%rip)        # 5b3e40 <free@GLIBC_2.2.5>
  17e949:	nopl   0x0(%rax)
  17e950:	dec    %rbp
  17e953:	je     17ea46 <bitz::utils::blake3x4::smallest_pow_nonce+0xe86>
  17e959:	mov    %r14,%rdi
  17e95c:	lea    0x28(%r14),%rbx
  17e960:	call   17f310 <core::ptr::drop_glue::<rayon_core::job::StackJob<rayon_core::latch::LatchRef<rayon_core::latch::CountLatch>, &rayon_core::broadcast::broadcast_in<bitz::utils::blake3x4::smallest_pow_nonce_with<8, bitz::utils::blake3x4::first_pow_nonce>::{closure#0}, ()>::{closure#0}, ()>>>
  17e965:	mov    %rbx,%r14
  17e968:	jmp    17e950 <bitz::utils::blake3x4::smallest_pow_nonce+0xd90>
  17e96a:	call   3d0ca3 <core::panicking::panic_in_cleanup>
  17e96f:	call   3d0ca3 <core::panicking::panic_in_cleanup>
  17e974:	mov    %rax,(%rsp)
  17e978:	sub    %r14,%rbp
  17e97b:	movabs $0xcccccccccccccccd,%rax
  17e985:	mov    %rbp,%rdx
  17e988:	mulx   %rax,%rbp,%rbp
  17e98d:	jne    17e9cf <bitz::utils::blake3x4::smallest_pow_nonce+0xe0f>
  17e98f:	mov    %r15,%rdi
  17e992:	call   *0x4354a8(%rip)        # 5b3e40 <free@GLIBC_2.2.5>
  17e998:	mov    0x18(%rsp),%r13
  17e99d:	mov    0x40(%rsp),%rax
  17e9a2:	test   %rax,%rax
  17e9a5:	je     17e9b7 <bitz::utils::blake3x4::smallest_pow_nonce+0xdf7>
  17e9a7:	lock decq (%rax)
  17e9ab:	jne    17e9b7 <bitz::utils::blake3x4::smallest_pow_nonce+0xdf7>
  17e9ad:	mov    0x40(%rsp),%rdi
  17e9b2:	call   480d30 <<alloc::sync::Arc<rayon_core::registry::Registry>>::drop_slow>
  17e9b7:	lock decq 0x0(%r13)
  17e9bc:	jne    17e9c6 <bitz::utils::blake3x4::smallest_pow_nonce+0xe06>
  17e9be:	mov    %r13,%rdi
  17e9c1:	call   480d30 <<alloc::sync::Arc<rayon_core::registry::Registry>>::drop_slow>
  17e9c6:	mov    (%rsp),%rdi
  17e9ca:	call   59bff0 <_Unwind_Resume@plt>
  17e9cf:	mov    0x43546a(%rip),%r12        # 5b3e40 <free@GLIBC_2.2.5>
  17e9d6:	shr    $0x5,%rbp
  17e9da:	add    $0x28,%r14
  17e9de:	jmp    17e9e9 <bitz::utils::blake3x4::smallest_pow_nonce+0xe29>
  17e9e0:	add    $0x28,%r14
  17e9e4:	dec    %rbp
  17e9e7:	je     17e98f <bitz::utils::blake3x4::smallest_pow_nonce+0xdcf>
  17e9e9:	cmpl   $0x2,-0x18(%r14)
  17e9ee:	jb     17e9e0 <bitz::utils::blake3x4::smallest_pow_nonce+0xe20>
  17e9f0:	mov    -0x8(%r14),%rbx
  17e9f4:	mov    -0x10(%r14),%r13
  17e9f8:	mov    (%rbx),%rax
  17e9fb:	test   %rax,%rax
  17e9fe:	je     17ea05 <bitz::utils::blake3x4::smallest_pow_nonce+0xe45>
  17ea00:	mov    %r13,%rdi
  17ea03:	call   *%rax
  17ea05:	cmpq   $0x0,0x8(%rbx)
  17ea0a:	je     17e9e0 <bitz::utils::blake3x4::smallest_pow_nonce+0xe20>
  17ea0c:	mov    %r13,%rdi
  17ea0f:	call   *%r12
  17ea12:	jmp    17e9e0 <bitz::utils::blake3x4::smallest_pow_nonce+0xe20>
  17ea14:	call   3d0ca3 <core::panicking::panic_in_cleanup>
  17ea19:	cmpq   $0x0,0x8(%rbx)
  17ea1e:	je     17ea30 <bitz::utils::blake3x4::smallest_pow_nonce+0xe70>
  17ea20:	mov    %r13,%rdi
  17ea23:	call   *0x435417(%rip)        # 5b3e40 <free@GLIBC_2.2.5>
  17ea29:	nopl   0x0(%rax)
  17ea30:	dec    %rbp
  17ea33:	je     17ea46 <bitz::utils::blake3x4::smallest_pow_nonce+0xe86>
  17ea35:	mov    %r14,%rdi
  17ea38:	lea    0x28(%r14),%rbx
  17ea3c:	call   17f310 <core::ptr::drop_glue::<rayon_core::job::StackJob<rayon_core::latch::LatchRef<rayon_core::latch::CountLatch>, &rayon_core::broadcast::broadcast_in<bitz::utils::blake3x4::smallest_pow_nonce_with<8, bitz::utils::blake3x4::first_pow_nonce>::{closure#0}, ()>::{closure#0}, ()>>>
  17ea41:	mov    %rbx,%r14
  17ea44:	jmp    17ea30 <bitz::utils::blake3x4::smallest_pow_nonce+0xe70>
  17ea46:	mov    %r15,%rdi
  17ea49:	call   *0x4353f1(%rip)        # 5b3e40 <free@GLIBC_2.2.5>
  17ea4f:	call   3d0ca3 <core::panicking::panic_in_cleanup>
  17ea54:	call   3d0ca3 <core::panicking::panic_in_cleanup>
  17ea59:	call   3d0ca3 <core::panicking::panic_in_cleanup>


/tmp/falcon-compact-opening-binaries/baseline-v4:     file format elf64-x86-64


Disassembly of section .text:

000000000018cc60 <bitz::piop::spartan::grinding::find_grinding_nonce>:
  18cc60:	push   %rbp
  18cc61:	push   %r15
  18cc63:	push   %r14
  18cc65:	push   %r13
  18cc67:	push   %r12
  18cc69:	push   %rbx
  18cc6a:	sub    $0x78,%rsp
  18cc6e:	mov    %edx,0xc(%rsp)
  18cc72:	mov    %rsi,%r12
  18cc75:	mov    %rdi,%r15
  18cc78:	mov    0x445e39(%rip),%rbx        # 5d2ab8 <tracing_core::metadata::MAX_LEVEL>
  18cc7f:	cmp    $0x2,%rbx
  18cc83:	jbe    18cc9e <bitz::piop::spartan::grinding::find_grinding_nonce+0x3e>
  18cc85:	mov    0xc(%rsp),%r13d
  18cc8a:	lea    -0x1(%r13),%eax
  18cc8e:	cmp    $0x100,%eax
  18cc93:	jae    18cdf5 <bitz::piop::spartan::grinding::find_grinding_nonce+0x195>
  18cc99:	jmp    18ce2a <bitz::piop::spartan::grinding::find_grinding_nonce+0x1ca>
  18cc9e:	mov    0x445e13(%rip),%rax        # 5d2ab8 <tracing_core::metadata::MAX_LEVEL>
  18cca5:	cmp    $0x2,%rax
  18cca9:	ja     18ce02 <bitz::piop::spartan::grinding::find_grinding_nonce+0x1a2>
  18ccaf:	movzbl 0x428c6a(%rip),%eax        # 5b5920 <bitz::piop::spartan::grinding::find_grinding_nonce::__CALLSITE+0x10>
  18ccb6:	test   %al,%al
  18ccb8:	je     18ce02 <bitz::piop::spartan::grinding::find_grinding_nonce+0x1a2>
  18ccbe:	movzbl %al,%eax
  18ccc1:	cmp    $0x1,%eax
  18ccc4:	je     18cce7 <bitz::piop::spartan::grinding::find_grinding_nonce+0x87>
  18ccc6:	cmp    $0x2,%eax
  18ccc9:	je     18cd22 <bitz::piop::spartan::grinding::find_grinding_nonce+0xc2>
  18cccb:	lea    0x428c3e(%rip),%rdi        # 5b5910 <bitz::piop::spartan::grinding::find_grinding_nonce::__CALLSITE>
  18ccd2:	call   4df3c0 <<tracing_core::callsite::DefaultCallsite>::register>
  18ccd7:	test   %al,%al
  18ccd9:	je     18ce02 <bitz::piop::spartan::grinding::find_grinding_nonce+0x1a2>
  18ccdf:	movzbl %al,%eax
  18cce2:	cmp    $0x2,%eax
  18cce5:	je     18cd22 <bitz::piop::spartan::grinding::find_grinding_nonce+0xc2>
  18cce7:	mov    0x428c22(%rip),%rsi        # 5b5910 <bitz::piop::spartan::grinding::find_grinding_nonce::__CALLSITE>
  18ccee:	mov    0x4467b3(%rip),%rax        # 5d34a8 <tracing_core::dispatcher::GLOBAL_INIT>
  18ccf5:	lea    -0xdc40d(%rip),%rdi        # b08ef <tracing_core::dispatcher::NO_SUBSCRIBER>
  18ccfc:	cmp    $0x2,%rax
  18cd00:	lea    0x423c79(%rip),%rax        # 5b0980 <<bitz::piop::spartan::falcon1024_ct::hybrid_sumcheck::State<_> as bitz::sumcheck::inner::input::State<field::binary::Gf128Ops>>::fold::__CALLSITE::META+0x9210>
  18cd07:	cmove  0x445d99(%rip),%rdi        # 5d2aa8 <tracing_core::dispatcher::GLOBAL_DISPATCH>
  18cd0f:	cmove  0x445d99(%rip),%rax        # 5d2ab0 <tracing_core::dispatcher::GLOBAL_DISPATCH>
  18cd17:	call   *0x28(%rax)
  18cd1a:	test   %al,%al
  18cd1c:	je     18ce02 <bitz::piop::spartan::grinding::find_grinding_nonce+0x1a2>
  18cd22:	mov    0x428be7(%rip),%r14        # 5b5910 <bitz::piop::spartan::grinding::find_grinding_nonce::__CALLSITE>
  18cd29:	lea    0xc(%rsp),%rcx
  18cd2e:	movq   $0x1,0x58(%rsp)
  18cd37:	lea    -0xdc44f(%rip),%r13        # b08ef <tracing_core::dispatcher::NO_SUBSCRIBER>
  18cd3e:	lea    0x423c3b(%rip),%rbp        # 5b0980 <<bitz::piop::spartan::falcon1024_ct::hybrid_sumcheck::State<_> as bitz::sumcheck::inner::input::State<field::binary::Gf128Ops>>::fold::__CALLSITE::META+0x9210>
  18cd45:	lea    0x10(%rsp),%rsi
  18cd4a:	mov    %rcx,0x48(%rsp)
  18cd4f:	lea    0x423cda(%rip),%rcx        # 5b0a30 <<bitz::piop::spartan::falcon1024_ct::hybrid_sumcheck::State<_> as bitz::sumcheck::inner::input::State<field::binary::Gf128Ops>>::fold::__CALLSITE::META+0x92c0>
  18cd56:	mov    %rcx,0x50(%rsp)
  18cd5b:	lea    0x48(%rsp),%rcx
  18cd60:	mov    %rcx,0x60(%rsp)
  18cd65:	movq   $0x1,0x68(%rsp)
  18cd6e:	lea    0x30(%r14),%rax
  18cd72:	mov    %rax,0x70(%rsp)
  18cd77:	mov    0x44672a(%rip),%rax        # 5d34a8 <tracing_core::dispatcher::GLOBAL_INIT>
  18cd7e:	mov    %r14,0x20(%rsp)
  18cd83:	cmp    $0x2,%rax
  18cd87:	lea    0x58(%rsp),%rax
  18cd8c:	cmove  0x445d14(%rip),%r13        # 5d2aa8 <tracing_core::dispatcher::GLOBAL_DISPATCH>
  18cd94:	cmove  0x445d14(%rip),%rbp        # 5d2ab0 <tracing_core::dispatcher::GLOBAL_DISPATCH>
  18cd9c:	mov    %rax,0x28(%rsp)
  18cda1:	movq   $0x1,0x10(%rsp)
  18cdaa:	mov    %r13,%rdi
  18cdad:	call   *0x38(%rbp)
  18cdb0:	movq   $0x0,0x10(%rsp)
  18cdb9:	mov    %r13,0x18(%rsp)
  18cdbe:	mov    %rbp,0x20(%rsp)
  18cdc3:	mov    %rax,0x28(%rsp)
  18cdc8:	lea    0x28(%rsp),%rsi
  18cdcd:	mov    %rax,0x38(%rsp)
  18cdd2:	mov    %r14,0x30(%rsp)
  18cdd7:	mov    %r13,%rdi
  18cdda:	mov    %rbp,0x40(%rsp)
  18cddf:	call   *0x60(%rbp)
  18cde2:	xor    %r14d,%r14d
  18cde5:	mov    0xc(%rsp),%r13d
  18cdea:	lea    -0x1(%r13),%eax
  18cdee:	cmp    $0x100,%eax
  18cdf3:	jb     18ce2a <bitz::piop::spartan::grinding::find_grinding_nonce+0x1ca>
  18cdf5:	movl   $0x0,(%r15)
  18cdfc:	mov    %r13d,0x4(%r15)
  18ce00:	jmp    18ce5b <bitz::piop::spartan::grinding::find_grinding_nonce+0x1fb>
  18ce02:	mov    $0x2,%r14d
  18ce08:	movq   $0x2,0x10(%rsp)
  18ce11:	movq   $0x0,0x30(%rsp)
  18ce1a:	mov    0xc(%rsp),%r13d
  18ce1f:	lea    -0x1(%r13),%eax
  18ce23:	cmp    $0x100,%eax
  18ce28:	jae    18cdf5 <bitz::piop::spartan::grinding::find_grinding_nonce+0x195>
  18ce2a:	cmp    $0xb,%r13d
  18ce2e:	jbe    18cec4 <bitz::piop::spartan::grinding::find_grinding_nonce+0x264>
  18ce34:	mov    $0x20,%esi
  18ce39:	mov    %r12,%rdi
  18ce3c:	mov    %r13d,%edx
  18ce3f:	call   17dbc0 <bitz::utils::blake3x4::smallest_pow_nonce>
  18ce44:	mov    $0x2,%ecx
  18ce49:	cmp    $0x1,%rax
  18ce4d:	jne    18ce58 <bitz::piop::spartan::grinding::find_grinding_nonce+0x1f8>
  18ce4f:	mov    $0xffffffff,%ecx
  18ce54:	mov    %rdx,0x8(%r15)
  18ce58:	mov    %ecx,(%r15)
  18ce5b:	cmp    $0x3,%rbx
  18ce5f:	mov    0x40(%rsp),%rbx
  18ce64:	setae  %al
  18ce67:	cmp    $0x2,%r14
  18ce6b:	sete   %cl
  18ce6e:	or     %al,%cl
  18ce70:	jne    18cf96 <bitz::piop::spartan::grinding::find_grinding_nonce+0x336>
  18ce76:	mov    0x18(%rsp),%rdi
  18ce7b:	lea    0x28(%rsp),%rsi
  18ce80:	cmp    $0x1,%r14
  18ce84:	jne    18ce96 <bitz::piop::spartan::grinding::find_grinding_nonce+0x236>
  18ce86:	mov    0x10(%rbx),%rax
  18ce8a:	dec    %rax
  18ce8d:	and    $0xfffffffffffffff0,%rax
  18ce91:	lea    0x10(%rdi,%rax,1),%rdi
  18ce96:	call   *0x68(%rbx)
  18ce99:	mov    0x18(%rsp),%rdi
  18ce9e:	test   $0x1,%r14b
  18cea2:	je     18ceb4 <bitz::piop::spartan::grinding::find_grinding_nonce+0x254>
  18cea4:	mov    0x10(%rbx),%rax
  18cea8:	dec    %rax
  18ceab:	and    $0xfffffffffffffff0,%rax
  18ceaf:	lea    0x10(%rdi,%rax,1),%rdi
  18ceb4:	mov    0x38(%rsp),%rsi
  18ceb9:	call   *0x80(%rbx)
  18cebf:	jmp    18cf79 <bitz::piop::spartan::grinding::find_grinding_nonce+0x319>
  18cec4:	mov    $0x20,%esi
  18cec9:	mov    $0xffffffffffffffff,%rcx
  18ced0:	mov    %r12,%rdi
  18ced3:	xor    %edx,%edx
  18ced5:	mov    %r13d,%r8d
  18ced8:	call   17f370 <bitz::utils::blake3x4::first_pow_nonce>
  18cedd:	test   $0x1,%al
  18cedf:	je     18cee6 <bitz::piop::spartan::grinding::find_grinding_nonce+0x286>
  18cee1:	mov    %rdx,%rbp
  18cee4:	jmp    18cf0d <bitz::piop::spartan::grinding::find_grinding_nonce+0x2ad>
  18cee6:	mov    $0x20,%esi
  18ceeb:	mov    $0xffffffffffffffff,%rdx
  18cef2:	mov    $0xffffffffffffffff,%rbp
  18cef9:	mov    %r12,%rdi
  18cefc:	mov    %r13d,%ecx
  18ceff:	call   17eca0 <bitz::utils::blake3x4::pow_ok>
  18cf04:	mov    $0x2,%ecx
  18cf09:	test   %al,%al
  18cf0b:	je     18cf16 <bitz::piop::spartan::grinding::find_grinding_nonce+0x2b6>
  18cf0d:	mov    $0xffffffff,%ecx
  18cf12:	mov    %rbp,0x8(%r15)
  18cf16:	cmp    $0x3,%rbx
  18cf1a:	mov    0x40(%rsp),%rbx
  18cf1f:	mov    %ecx,(%r15)
  18cf22:	setae  %al
  18cf25:	cmp    $0x2,%r14
  18cf29:	sete   %cl
  18cf2c:	or     %al,%cl
  18cf2e:	jne    18cf96 <bitz::piop::spartan::grinding::find_grinding_nonce+0x336>
  18cf30:	mov    0x18(%rsp),%rdi
  18cf35:	lea    0x28(%rsp),%rsi
  18cf3a:	cmp    $0x1,%r14
  18cf3e:	jne    18cf50 <bitz::piop::spartan::grinding::find_grinding_nonce+0x2f0>
  18cf40:	mov    0x10(%rbx),%rax
  18cf44:	dec    %rax
  18cf47:	and    $0xfffffffffffffff0,%rax
  18cf4b:	lea    0x10(%rdi,%rax,1),%rdi
  18cf50:	call   *0x68(%rbx)
  18cf53:	mov    0x18(%rsp),%rdi
  18cf58:	test   $0x1,%r14b
  18cf5c:	je     18cf6e <bitz::piop::spartan::grinding::find_grinding_nonce+0x30e>
  18cf5e:	mov    0x10(%rbx),%rax
  18cf62:	dec    %rax
  18cf65:	and    $0xfffffffffffffff0,%rax
  18cf69:	lea    0x10(%rdi,%rax,1),%rdi
  18cf6e:	mov    0x38(%rsp),%rsi
  18cf73:	call   *0x80(%rbx)
  18cf79:	test   %r14,%r14
  18cf7c:	je     18cf96 <bitz::piop::spartan::grinding::find_grinding_nonce+0x336>
  18cf7e:	mov    0x18(%rsp),%rax
  18cf83:	lock decq (%rax)
  18cf87:	jne    18cf96 <bitz::piop::spartan::grinding::find_grinding_nonce+0x336>
  18cf89:	mov    0x18(%rsp),%rdi
  18cf8e:	mov    %rbx,%rsi
  18cf91:	call   4df300 <<alloc::sync::Arc<dyn tracing_core::subscriber::Subscriber + core::marker::Send + core::marker::Sync>>::drop_slow>
  18cf96:	add    $0x78,%rsp
  18cf9a:	pop    %rbx
  18cf9b:	pop    %r12
  18cf9d:	pop    %r13
  18cf9f:	pop    %r14
  18cfa1:	pop    %r15
  18cfa3:	pop    %rbp
  18cfa4:	ret
  18cfa5:	mov    %rax,%r15
  18cfa8:	lea    0x10(%rsp),%rdi
  18cfad:	call   18a0c0 <core::ptr::drop_glue::<core::option::Option<tracing::span::Inner>>>
  18cfb2:	jmp    18d01a <bitz::piop::spartan::grinding::find_grinding_nonce+0x3ba>
  18cfb4:	call   3d0ca3 <core::panicking::panic_in_cleanup>
  18cfb9:	jmp    18cfcf <bitz::piop::spartan::grinding::find_grinding_nonce+0x36f>
  18cfbb:	mov    %rax,%r15
  18cfbe:	lea    0x10(%rsp),%rdi
  18cfc3:	call   18a0c0 <core::ptr::drop_glue::<core::option::Option<tracing::span::Inner>>>
  18cfc8:	jmp    18d01a <bitz::piop::spartan::grinding::find_grinding_nonce+0x3ba>
  18cfca:	call   3d0ca3 <core::panicking::panic_in_cleanup>
  18cfcf:	mov    %rax,%r15
  18cfd2:	jmp    18d010 <bitz::piop::spartan::grinding::find_grinding_nonce+0x3b0>
  18cfd4:	mov    %rax,%r15
  18cfd7:	cmp    $0x2,%rbx
  18cfdb:	ja     18d01a <bitz::piop::spartan::grinding::find_grinding_nonce+0x3ba>
  18cfdd:	cmp    $0x2,%r14
  18cfe1:	je     18d010 <bitz::piop::spartan::grinding::find_grinding_nonce+0x3b0>
  18cfe3:	mov    0x18(%rsp),%rdi
  18cfe8:	lea    0x28(%rsp),%rsi
  18cfed:	cmp    $0x1,%r14
  18cff1:	jne    18d008 <bitz::piop::spartan::grinding::find_grinding_nonce+0x3a8>
  18cff3:	mov    0x40(%rsp),%rax
  18cff8:	mov    0x10(%rax),%rax
  18cffc:	dec    %rax
  18cfff:	and    $0xfffffffffffffff0,%rax
  18d003:	lea    0x10(%rdi,%rax,1),%rdi
  18d008:	mov    0x40(%rsp),%rax
  18d00d:	call   *0x68(%rax)
  18d010:	lea    0x10(%rsp),%rdi
  18d015:	call   18a040 <core::ptr::drop_glue::<tracing::span::Span>>
  18d01a:	mov    %r15,%rdi
  18d01d:	call   59bff0 <_Unwind_Resume@plt>
  18d022:	call   3d0ca3 <core::panicking::panic_in_cleanup>

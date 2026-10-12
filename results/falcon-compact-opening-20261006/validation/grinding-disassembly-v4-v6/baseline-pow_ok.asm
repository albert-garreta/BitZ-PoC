
/tmp/falcon-compact-opening-binaries/baseline-v4:     file format elf64-x86-64


Disassembly of section .text:

000000000017eca0 <bitz::utils::blake3x4::pow_ok>:
  17eca0:	push   %r15
  17eca2:	push   %r14
  17eca4:	push   %r12
  17eca6:	push   %rbx
  17eca7:	sub    $0x128,%rsp
  17ecae:	lea    0xe0(%rsp),%r12
  17ecb6:	mov    %rsi,%r15
  17ecb9:	mov    %rdx,%r14
  17ecbc:	mov    %rdi,%rsi
  17ecbf:	vxorps %xmm0,%xmm0,%xmm0
  17ecc3:	mov    %ecx,%ebx
  17ecc5:	vmovups %zmm0,0xe0(%rsp)
  17ecd0:	mov    %r15,%rdx
  17ecd3:	mov    %r12,%rdi
  17ecd6:	vzeroupper
  17ecd9:	call   *0x435159(%rip)        # 5b3e38 <memcpy@GLIBC_2.14>
  17ecdf:	vmovups -0x123b07(%rip),%ymm1        # 5b1e0 <GCC_except_table0+0x6c9c>
  17ece7:	mov    %r14,0xe0(%rsp,%r15,1)
  17ecef:	add    $0x8,%r15
  17ecf3:	vxorps %xmm0,%xmm0,%xmm0
  17ecf7:	vmovups %zmm0,(%rsp)
  17ecfe:	mov    %rsp,%rdi
  17ed01:	mov    %r12,%rsi
  17ed04:	mov    %r15,%rdx
  17ed07:	vmovups %ymm1,0x40(%rsp)
  17ed0d:	movq   $0x0,0x60(%rsp)
  17ed16:	movl   $0x0,0x67(%rsp)
  17ed1e:	movb   $0x4,0x6b(%rsp)
  17ed23:	vzeroupper
  17ed26:	call   3c5eb0 <<blake3::ChunkState>::update>
  17ed2b:	cmpb   $0x0,0x69(%rsp)
  17ed30:	vmovups 0x40(%rsp),%ymm0
  17ed36:	vmovups (%rsp),%zmm1
  17ed3d:	movzbl 0x68(%rsp),%ecx
  17ed42:	mov    0x60(%rsp),%rdx
  17ed47:	movzbl 0x6b(%rsp),%esi
  17ed4c:	mov    %rsp,%rdi
  17ed4f:	sete   %al
  17ed52:	or     0x6a(%rsp),%al
  17ed56:	vmovups %ymm0,0xb0(%rsp)
  17ed5f:	vmovups %zmm1,0x70(%rsp)
  17ed6a:	mov    %cl,0xd8(%rsp)
  17ed71:	mov    %rdx,0xd0(%rsp)
  17ed79:	or     $0x2,%al
  17ed7b:	mov    %al,0xd9(%rsp)
  17ed82:	mov    %sil,0xda(%rsp)
  17ed8a:	lea    0x70(%rsp),%rsi
  17ed8f:	vzeroupper
  17ed92:	call   3c60c0 <<blake3::Output>::root_hash>
  17ed97:	movzbl (%rsp),%eax
  17ed9b:	xor    %ecx,%ecx
  17ed9d:	test   %al,%al
  17ed9f:	jne    17efab <bitz::utils::blake3x4::pow_ok+0x30b>
  17eda5:	movzbl 0x1(%rsp),%eax
  17edaa:	mov    $0x8,%ecx
  17edaf:	test   %al,%al
  17edb1:	jne    17efab <bitz::utils::blake3x4::pow_ok+0x30b>
  17edb7:	movzbl 0x2(%rsp),%eax
  17edbc:	mov    $0x10,%ecx
  17edc1:	test   %al,%al
  17edc3:	jne    17efab <bitz::utils::blake3x4::pow_ok+0x30b>
  17edc9:	movzbl 0x3(%rsp),%eax
  17edce:	mov    $0x18,%ecx
  17edd3:	test   %al,%al
  17edd5:	jne    17efab <bitz::utils::blake3x4::pow_ok+0x30b>
  17eddb:	movzbl 0x4(%rsp),%eax
  17ede0:	mov    $0x20,%ecx
  17ede5:	test   %al,%al
  17ede7:	jne    17efab <bitz::utils::blake3x4::pow_ok+0x30b>
  17eded:	movzbl 0x5(%rsp),%eax
  17edf2:	mov    $0x28,%ecx
  17edf7:	test   %al,%al
  17edf9:	jne    17efab <bitz::utils::blake3x4::pow_ok+0x30b>
  17edff:	movzbl 0x6(%rsp),%eax
  17ee04:	mov    $0x30,%ecx
  17ee09:	test   %al,%al
  17ee0b:	jne    17efab <bitz::utils::blake3x4::pow_ok+0x30b>
  17ee11:	movzbl 0x7(%rsp),%eax
  17ee16:	mov    $0x38,%ecx
  17ee1b:	test   %al,%al
  17ee1d:	jne    17efab <bitz::utils::blake3x4::pow_ok+0x30b>
  17ee23:	movzbl 0x8(%rsp),%eax
  17ee28:	mov    $0x40,%ecx
  17ee2d:	test   %al,%al
  17ee2f:	jne    17efab <bitz::utils::blake3x4::pow_ok+0x30b>
  17ee35:	movzbl 0x9(%rsp),%eax
  17ee3a:	mov    $0x48,%ecx
  17ee3f:	test   %al,%al
  17ee41:	jne    17efab <bitz::utils::blake3x4::pow_ok+0x30b>
  17ee47:	movzbl 0xa(%rsp),%eax
  17ee4c:	mov    $0x50,%ecx
  17ee51:	test   %al,%al
  17ee53:	jne    17efab <bitz::utils::blake3x4::pow_ok+0x30b>
  17ee59:	movzbl 0xb(%rsp),%eax
  17ee5e:	mov    $0x58,%ecx
  17ee63:	test   %al,%al
  17ee65:	jne    17efab <bitz::utils::blake3x4::pow_ok+0x30b>
  17ee6b:	movzbl 0xc(%rsp),%eax
  17ee70:	mov    $0x60,%ecx
  17ee75:	test   %al,%al
  17ee77:	jne    17efab <bitz::utils::blake3x4::pow_ok+0x30b>
  17ee7d:	movzbl 0xd(%rsp),%eax
  17ee82:	mov    $0x68,%ecx
  17ee87:	test   %al,%al
  17ee89:	jne    17efab <bitz::utils::blake3x4::pow_ok+0x30b>
  17ee8f:	movzbl 0xe(%rsp),%eax
  17ee94:	mov    $0x70,%ecx
  17ee99:	test   %al,%al
  17ee9b:	jne    17efab <bitz::utils::blake3x4::pow_ok+0x30b>
  17eea1:	movzbl 0xf(%rsp),%eax
  17eea6:	mov    $0x78,%ecx
  17eeab:	test   %al,%al
  17eead:	jne    17efab <bitz::utils::blake3x4::pow_ok+0x30b>
  17eeb3:	movzbl 0x10(%rsp),%eax
  17eeb8:	mov    $0x80,%ecx
  17eebd:	test   %al,%al
  17eebf:	jne    17efab <bitz::utils::blake3x4::pow_ok+0x30b>
  17eec5:	movzbl 0x11(%rsp),%eax
  17eeca:	mov    $0x88,%ecx
  17eecf:	test   %al,%al
  17eed1:	jne    17efab <bitz::utils::blake3x4::pow_ok+0x30b>
  17eed7:	movzbl 0x12(%rsp),%eax
  17eedc:	mov    $0x90,%ecx
  17eee1:	test   %al,%al
  17eee3:	jne    17efab <bitz::utils::blake3x4::pow_ok+0x30b>
  17eee9:	movzbl 0x13(%rsp),%eax
  17eeee:	mov    $0x98,%ecx
  17eef3:	test   %al,%al
  17eef5:	jne    17efab <bitz::utils::blake3x4::pow_ok+0x30b>
  17eefb:	movzbl 0x14(%rsp),%eax
  17ef00:	mov    $0xa0,%ecx
  17ef05:	test   %al,%al
  17ef07:	jne    17efab <bitz::utils::blake3x4::pow_ok+0x30b>
  17ef0d:	movzbl 0x15(%rsp),%eax
  17ef12:	mov    $0xa8,%ecx
  17ef17:	test   %al,%al
  17ef19:	jne    17efab <bitz::utils::blake3x4::pow_ok+0x30b>
  17ef1f:	movzbl 0x16(%rsp),%eax
  17ef24:	mov    $0xb0,%ecx
  17ef29:	test   %al,%al
  17ef2b:	jne    17efab <bitz::utils::blake3x4::pow_ok+0x30b>
  17ef2d:	movzbl 0x17(%rsp),%eax
  17ef32:	mov    $0xb8,%ecx
  17ef37:	test   %al,%al
  17ef39:	jne    17efab <bitz::utils::blake3x4::pow_ok+0x30b>
  17ef3b:	movzbl 0x18(%rsp),%eax
  17ef40:	mov    $0xc0,%ecx
  17ef45:	test   %al,%al
  17ef47:	jne    17efab <bitz::utils::blake3x4::pow_ok+0x30b>
  17ef49:	movzbl 0x19(%rsp),%eax
  17ef4e:	mov    $0xc8,%ecx
  17ef53:	test   %al,%al
  17ef55:	jne    17efab <bitz::utils::blake3x4::pow_ok+0x30b>
  17ef57:	movzbl 0x1a(%rsp),%eax
  17ef5c:	mov    $0xd0,%ecx
  17ef61:	test   %al,%al
  17ef63:	jne    17efab <bitz::utils::blake3x4::pow_ok+0x30b>
  17ef65:	movzbl 0x1b(%rsp),%eax
  17ef6a:	mov    $0xd8,%ecx
  17ef6f:	test   %al,%al
  17ef71:	jne    17efab <bitz::utils::blake3x4::pow_ok+0x30b>
  17ef73:	movzbl 0x1c(%rsp),%eax
  17ef78:	mov    $0xe0,%ecx
  17ef7d:	test   %al,%al
  17ef7f:	jne    17efab <bitz::utils::blake3x4::pow_ok+0x30b>
  17ef81:	movzbl 0x1d(%rsp),%eax
  17ef86:	mov    $0xe8,%ecx
  17ef8b:	test   %al,%al
  17ef8d:	jne    17efab <bitz::utils::blake3x4::pow_ok+0x30b>
  17ef8f:	movzbl 0x1e(%rsp),%eax
  17ef94:	mov    $0xf0,%ecx
  17ef99:	test   %al,%al
  17ef9b:	jne    17efab <bitz::utils::blake3x4::pow_ok+0x30b>
  17ef9d:	movzbl 0x1f(%rsp),%eax
  17efa2:	mov    $0xf8,%ecx
  17efa7:	test   %al,%al
  17efa9:	je     17efcb <bitz::utils::blake3x4::pow_ok+0x32b>
  17efab:	movzbl %al,%eax
  17efae:	shl    $0x18,%eax
  17efb1:	lzcnt  %eax,%eax
  17efb5:	or     %ecx,%eax
  17efb7:	cmp    %ebx,%eax
  17efb9:	setae  %al
  17efbc:	add    $0x128,%rsp
  17efc3:	pop    %rbx
  17efc4:	pop    %r12
  17efc6:	pop    %r14
  17efc8:	pop    %r15
  17efca:	ret
  17efcb:	mov    $0x100,%eax
  17efd0:	jmp    17efb7 <bitz::utils::blake3x4::pow_ok+0x317>

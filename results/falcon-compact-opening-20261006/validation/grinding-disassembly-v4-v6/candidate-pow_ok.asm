
/tmp/falcon-compact-opening-binaries/candidate-v6:     file format elf64-x86-64


Disassembly of section .text:

000000000017fd30 <bitz::utils::blake3x4::pow_ok>:
  17fd30:	push   %r15
  17fd32:	push   %r14
  17fd34:	push   %r12
  17fd36:	push   %rbx
  17fd37:	sub    $0x128,%rsp
  17fd3e:	lea    0xe0(%rsp),%r12
  17fd46:	mov    %rsi,%r15
  17fd49:	mov    %rdx,%r14
  17fd4c:	mov    %rdi,%rsi
  17fd4f:	vxorps %xmm0,%xmm0,%xmm0
  17fd53:	mov    %ecx,%ebx
  17fd55:	vmovups %zmm0,0xe0(%rsp)
  17fd60:	mov    %r15,%rdx
  17fd63:	mov    %r12,%rdi
  17fd66:	vzeroupper
  17fd69:	call   *0x4362e9(%rip)        # 5b6058 <memcpy@GLIBC_2.14>
  17fd6f:	vmovups -0x124bf7(%rip),%ymm1        # 5b180 <GCC_except_table0+0x6a00>
  17fd77:	mov    %r14,0xe0(%rsp,%r15,1)
  17fd7f:	add    $0x8,%r15
  17fd83:	vxorps %xmm0,%xmm0,%xmm0
  17fd87:	vmovups %zmm0,(%rsp)
  17fd8e:	mov    %rsp,%rdi
  17fd91:	mov    %r12,%rsi
  17fd94:	mov    %r15,%rdx
  17fd97:	vmovups %ymm1,0x40(%rsp)
  17fd9d:	movq   $0x0,0x60(%rsp)
  17fda6:	movl   $0x0,0x67(%rsp)
  17fdae:	movb   $0x4,0x6b(%rsp)
  17fdb3:	vzeroupper
  17fdb6:	call   3c6c80 <<blake3::ChunkState>::update>
  17fdbb:	cmpb   $0x0,0x69(%rsp)
  17fdc0:	vmovups 0x40(%rsp),%ymm0
  17fdc6:	vmovups (%rsp),%zmm1
  17fdcd:	movzbl 0x68(%rsp),%ecx
  17fdd2:	mov    0x60(%rsp),%rdx
  17fdd7:	movzbl 0x6b(%rsp),%esi
  17fddc:	mov    %rsp,%rdi
  17fddf:	sete   %al
  17fde2:	or     0x6a(%rsp),%al
  17fde6:	vmovups %ymm0,0xb0(%rsp)
  17fdef:	vmovups %zmm1,0x70(%rsp)
  17fdfa:	mov    %cl,0xd8(%rsp)
  17fe01:	mov    %rdx,0xd0(%rsp)
  17fe09:	or     $0x2,%al
  17fe0b:	mov    %al,0xd9(%rsp)
  17fe12:	mov    %sil,0xda(%rsp)
  17fe1a:	lea    0x70(%rsp),%rsi
  17fe1f:	vzeroupper
  17fe22:	call   3c6e90 <<blake3::Output>::root_hash>
  17fe27:	movzbl (%rsp),%eax
  17fe2b:	xor    %ecx,%ecx
  17fe2d:	test   %al,%al
  17fe2f:	jne    18003b <bitz::utils::blake3x4::pow_ok+0x30b>
  17fe35:	movzbl 0x1(%rsp),%eax
  17fe3a:	mov    $0x8,%ecx
  17fe3f:	test   %al,%al
  17fe41:	jne    18003b <bitz::utils::blake3x4::pow_ok+0x30b>
  17fe47:	movzbl 0x2(%rsp),%eax
  17fe4c:	mov    $0x10,%ecx
  17fe51:	test   %al,%al
  17fe53:	jne    18003b <bitz::utils::blake3x4::pow_ok+0x30b>
  17fe59:	movzbl 0x3(%rsp),%eax
  17fe5e:	mov    $0x18,%ecx
  17fe63:	test   %al,%al
  17fe65:	jne    18003b <bitz::utils::blake3x4::pow_ok+0x30b>
  17fe6b:	movzbl 0x4(%rsp),%eax
  17fe70:	mov    $0x20,%ecx
  17fe75:	test   %al,%al
  17fe77:	jne    18003b <bitz::utils::blake3x4::pow_ok+0x30b>
  17fe7d:	movzbl 0x5(%rsp),%eax
  17fe82:	mov    $0x28,%ecx
  17fe87:	test   %al,%al
  17fe89:	jne    18003b <bitz::utils::blake3x4::pow_ok+0x30b>
  17fe8f:	movzbl 0x6(%rsp),%eax
  17fe94:	mov    $0x30,%ecx
  17fe99:	test   %al,%al
  17fe9b:	jne    18003b <bitz::utils::blake3x4::pow_ok+0x30b>
  17fea1:	movzbl 0x7(%rsp),%eax
  17fea6:	mov    $0x38,%ecx
  17feab:	test   %al,%al
  17fead:	jne    18003b <bitz::utils::blake3x4::pow_ok+0x30b>
  17feb3:	movzbl 0x8(%rsp),%eax
  17feb8:	mov    $0x40,%ecx
  17febd:	test   %al,%al
  17febf:	jne    18003b <bitz::utils::blake3x4::pow_ok+0x30b>
  17fec5:	movzbl 0x9(%rsp),%eax
  17feca:	mov    $0x48,%ecx
  17fecf:	test   %al,%al
  17fed1:	jne    18003b <bitz::utils::blake3x4::pow_ok+0x30b>
  17fed7:	movzbl 0xa(%rsp),%eax
  17fedc:	mov    $0x50,%ecx
  17fee1:	test   %al,%al
  17fee3:	jne    18003b <bitz::utils::blake3x4::pow_ok+0x30b>
  17fee9:	movzbl 0xb(%rsp),%eax
  17feee:	mov    $0x58,%ecx
  17fef3:	test   %al,%al
  17fef5:	jne    18003b <bitz::utils::blake3x4::pow_ok+0x30b>
  17fefb:	movzbl 0xc(%rsp),%eax
  17ff00:	mov    $0x60,%ecx
  17ff05:	test   %al,%al
  17ff07:	jne    18003b <bitz::utils::blake3x4::pow_ok+0x30b>
  17ff0d:	movzbl 0xd(%rsp),%eax
  17ff12:	mov    $0x68,%ecx
  17ff17:	test   %al,%al
  17ff19:	jne    18003b <bitz::utils::blake3x4::pow_ok+0x30b>
  17ff1f:	movzbl 0xe(%rsp),%eax
  17ff24:	mov    $0x70,%ecx
  17ff29:	test   %al,%al
  17ff2b:	jne    18003b <bitz::utils::blake3x4::pow_ok+0x30b>
  17ff31:	movzbl 0xf(%rsp),%eax
  17ff36:	mov    $0x78,%ecx
  17ff3b:	test   %al,%al
  17ff3d:	jne    18003b <bitz::utils::blake3x4::pow_ok+0x30b>
  17ff43:	movzbl 0x10(%rsp),%eax
  17ff48:	mov    $0x80,%ecx
  17ff4d:	test   %al,%al
  17ff4f:	jne    18003b <bitz::utils::blake3x4::pow_ok+0x30b>
  17ff55:	movzbl 0x11(%rsp),%eax
  17ff5a:	mov    $0x88,%ecx
  17ff5f:	test   %al,%al
  17ff61:	jne    18003b <bitz::utils::blake3x4::pow_ok+0x30b>
  17ff67:	movzbl 0x12(%rsp),%eax
  17ff6c:	mov    $0x90,%ecx
  17ff71:	test   %al,%al
  17ff73:	jne    18003b <bitz::utils::blake3x4::pow_ok+0x30b>
  17ff79:	movzbl 0x13(%rsp),%eax
  17ff7e:	mov    $0x98,%ecx
  17ff83:	test   %al,%al
  17ff85:	jne    18003b <bitz::utils::blake3x4::pow_ok+0x30b>
  17ff8b:	movzbl 0x14(%rsp),%eax
  17ff90:	mov    $0xa0,%ecx
  17ff95:	test   %al,%al
  17ff97:	jne    18003b <bitz::utils::blake3x4::pow_ok+0x30b>
  17ff9d:	movzbl 0x15(%rsp),%eax
  17ffa2:	mov    $0xa8,%ecx
  17ffa7:	test   %al,%al
  17ffa9:	jne    18003b <bitz::utils::blake3x4::pow_ok+0x30b>
  17ffaf:	movzbl 0x16(%rsp),%eax
  17ffb4:	mov    $0xb0,%ecx
  17ffb9:	test   %al,%al
  17ffbb:	jne    18003b <bitz::utils::blake3x4::pow_ok+0x30b>
  17ffbd:	movzbl 0x17(%rsp),%eax
  17ffc2:	mov    $0xb8,%ecx
  17ffc7:	test   %al,%al
  17ffc9:	jne    18003b <bitz::utils::blake3x4::pow_ok+0x30b>
  17ffcb:	movzbl 0x18(%rsp),%eax
  17ffd0:	mov    $0xc0,%ecx
  17ffd5:	test   %al,%al
  17ffd7:	jne    18003b <bitz::utils::blake3x4::pow_ok+0x30b>
  17ffd9:	movzbl 0x19(%rsp),%eax
  17ffde:	mov    $0xc8,%ecx
  17ffe3:	test   %al,%al
  17ffe5:	jne    18003b <bitz::utils::blake3x4::pow_ok+0x30b>
  17ffe7:	movzbl 0x1a(%rsp),%eax
  17ffec:	mov    $0xd0,%ecx
  17fff1:	test   %al,%al
  17fff3:	jne    18003b <bitz::utils::blake3x4::pow_ok+0x30b>
  17fff5:	movzbl 0x1b(%rsp),%eax
  17fffa:	mov    $0xd8,%ecx
  17ffff:	test   %al,%al
  180001:	jne    18003b <bitz::utils::blake3x4::pow_ok+0x30b>
  180003:	movzbl 0x1c(%rsp),%eax
  180008:	mov    $0xe0,%ecx
  18000d:	test   %al,%al
  18000f:	jne    18003b <bitz::utils::blake3x4::pow_ok+0x30b>
  180011:	movzbl 0x1d(%rsp),%eax
  180016:	mov    $0xe8,%ecx
  18001b:	test   %al,%al
  18001d:	jne    18003b <bitz::utils::blake3x4::pow_ok+0x30b>
  18001f:	movzbl 0x1e(%rsp),%eax
  180024:	mov    $0xf0,%ecx
  180029:	test   %al,%al
  18002b:	jne    18003b <bitz::utils::blake3x4::pow_ok+0x30b>
  18002d:	movzbl 0x1f(%rsp),%eax
  180032:	mov    $0xf8,%ecx
  180037:	test   %al,%al
  180039:	je     18005b <bitz::utils::blake3x4::pow_ok+0x32b>
  18003b:	movzbl %al,%eax
  18003e:	shl    $0x18,%eax
  180041:	lzcnt  %eax,%eax
  180045:	or     %ecx,%eax
  180047:	cmp    %ebx,%eax
  180049:	setae  %al
  18004c:	add    $0x128,%rsp
  180053:	pop    %rbx
  180054:	pop    %r12
  180056:	pop    %r14
  180058:	pop    %r15
  18005a:	ret
  18005b:	mov    $0x100,%eax
  180060:	jmp    180047 <bitz::utils::blake3x4::pow_ok+0x317>

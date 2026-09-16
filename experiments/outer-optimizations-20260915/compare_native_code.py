#!/usr/bin/env python3
"""Compare the executable code sections of two aarch64 Mach-O benchmark binaries."""
import hashlib,struct,sys
from pathlib import Path

def text_section(path):
 with Path(path).open('rb') as f:
  header=f.read(32)
  if struct.unpack_from('<I',header)[0]!=0xfeedfacf:raise ValueError('expected little-endian 64-bit Mach-O')
  for _ in range(struct.unpack_from('<I',header,16)[0]):
   start=f.tell();command,size=struct.unpack('<II',f.read(8));f.seek(start);data=f.read(size)
   if command!=0x19:continue
   for j in range(struct.unpack_from('<I',data,64)[0]):
    section=data[72+80*j:72+80*(j+1)]
    if section[:16].rstrip(b'\0')==b'__text':
     size=struct.unpack_from('<Q',section,40)[0];offset=struct.unpack_from('<I',section,48)[0]
     f.seek(offset);return f.read(size)
 raise ValueError('missing __text')

left,right=map(text_section,sys.argv[1:3])
for path,code in zip(sys.argv[1:3],(left,right)):print(path,len(code),hashlib.sha256(code).hexdigest())
print('identical:',left==right)
raise SystemExit(0 if left==right else 1)

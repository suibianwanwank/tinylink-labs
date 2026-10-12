#!/usr/bin/env python3
"""Inspect an LLD ad-hoc CodeDirectory on Linux. This is not Darwin policy validation."""
import argparse
import hashlib
import struct
from pathlib import Path

p = argparse.ArgumentParser()
p.add_argument('file', type=Path)
p.add_argument('--mutate', type=Path)
a = p.parse_args()
b = a.file.read_bytes()

def unpack(fmt, off):
    size = struct.calcsize(fmt)
    if off < 0 or off + size > len(b):
        raise ValueError('truncated structure')
    return struct.unpack_from(fmt, b, off)

magic, _, _, _, ncmds, sizeofcmds, _, _ = unpack('<8I', 0)
if magic != 0xfeedfacf:
    raise ValueError('requires little-endian Mach-O64')
o = 32
signature = None
text_offset = None
for _ in range(ncmds):
    cmd, size = unpack('<2I', o)
    if size < 8 or o + size > 32 + sizeofcmds:
        raise ValueError('invalid load command')
    if cmd == 0x1d:
        signature = unpack('<2I', o + 8)
    if cmd == 0x19:
        nsects = unpack('<I', o + 64)[0]
        for i in range(nsects):
            sec = o + 72 + i * 80
            if b[sec:sec+16].rstrip(b'\0') == b'__text':
                text_offset = unpack('<I', sec + 48)[0]
    o += size
if signature is None:
    print(f'{a.file.name}: no LC_CODE_SIGNATURE')
    raise SystemExit(0)
sigoff, sigsize = signature
magic, length, count = unpack('>3I', sigoff)
if magic != 0xfade0cc0 or length > sigsize or sigoff + sigsize > len(b):
    raise ValueError('invalid signature SuperBlob')
for i in range(count):
    slot, off = unpack('>2I', sigoff + 12 + i * 8)
    if slot != 0:
        continue
    cd = sigoff + off
    (magic, cdlen, version, flags, hash_off, ident_off, special,
     slots, code_limit) = unpack('>9I', cd)
    hash_size, hash_type, _, page_log2 = unpack('>4B', cd + 36)
    if magic != 0xfade0c02 or hash_type != 2 or hash_size != 32:
        raise ValueError('this exercise expects SHA-256 CodeDirectory')
    if cd + cdlen > sigoff + length or hash_off + slots * hash_size > cdlen:
        raise ValueError('hash table out of bounds')
    page_size = 1 << page_log2
    mismatches = []
    for j in range(slots):
        page = b[j * page_size:min((j + 1) * page_size, code_limit)]
        expected = b[cd + hash_off + j * hash_size:cd + hash_off + (j + 1) * hash_size]
        if hashlib.sha256(page).digest() != expected:
            mismatches.append(j)
    print(f'{a.file.name}: flags={flags:#x}, codeLimit={code_limit:#x}, pageSize={page_size}, slots={slots}, mismatches={mismatches}')
    if a.mutate:
        if text_offset is None or text_offset >= code_limit:
            raise ValueError('no covered text byte')
        damaged = bytearray(b)
        damaged[text_offset] ^= 1
        a.mutate.write_bytes(damaged)
        print(f'mutated file offset {text_offset:#x}; signed page {text_offset // page_size}')
    raise SystemExit(bool(mismatches))
raise ValueError('no primary CodeDirectory')

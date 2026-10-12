#!/usr/bin/env python3
"""Check the chapter's byte-region diagrams and the odd.s epilogue CFI."""
import argparse
from pathlib import Path
import re
import struct
import subprocess


def check_macho(path):
    data = path.read_bytes()
    u32 = lambda at: struct.unpack_from('<I', data, at)[0]
    assert u32(0) == 0xfeedfacf and u32(4) == 0x0100000c and len(data) == 848
    assert (u32(16), u32(20)) == (5, 456)
    cursor = 32
    commands, sections = [], []
    for _ in range(u32(16)):
        kind, size = u32(cursor), u32(cursor + 4)
        assert size >= 8 and cursor + size <= 32 + u32(20)
        commands.append((cursor, kind, size))
        if kind == 0x19:
            count = u32(cursor + 64)
            assert 72 + count * 80 <= size
            for index in range(count):
                at = cursor + 72 + index * 80
                name = data[at:at + 16].rstrip(b'\0')
                length = struct.unpack_from('<Q', data, at + 40)[0]
                offset, reloc, nreloc = u32(at + 48), u32(at + 56), u32(at + 60)
                assert offset + length <= len(data)
                assert reloc + nreloc * 8 <= len(data)
                sections.append((at, name, length, offset, reloc, nreloc))
        cursor += size
    assert cursor == 488
    assert commands == [(32, 0x19, 312), (344, 0x32, 24), (368, 0x2e, 16),
                        (384, 2, 24), (408, 0xb, 80)]
    assert sections == [(104, b'__text', 64, 488, 592, 6),
                        (184, b'__cstring', 4, 552, 0, 0),
                        (264, b'__compact_unwind', 32, 560, 640, 1)]
    assert (u32(376), u32(380)) == (648, 16)
    assert (u32(392), u32(396), u32(400), u32(404)) == (664, 8, 792, 56)
    print('Mach-O diagram: header, commands, records, contents and tables agree')


def check_pe(path):
    data = path.read_bytes()
    u16 = lambda at: struct.unpack_from('<H', data, at)[0]
    u32 = lambda at: struct.unpack_from('<I', data, at)[0]
    signature = u32(0x3c)
    assert data[:2] == b'MZ' and data[signature:signature + 4] == b'PE\0\0'
    assert signature == 0x78 and len(data) == 0xe00
    assert u16(signature + 4) == 0x8664
    assert (u16(signature + 6), u16(signature + 20)) == (5, 240)
    optional = signature + 24
    assert optional == 0x90 and u16(optional) == 0x20b
    base = struct.unpack_from('<Q', data, optional + 24)[0]
    assert base == 0x140000000
    assert (u32(optional + 32), u32(optional + 36), u32(optional + 56)) == (4096, 512, 0x6000)
    table = optional + u16(signature + 20)
    assert table == 0x180 and table + 5 * 40 == 0x248
    rows = []
    for index in range(5):
        at = table + index * 40
        name = data[at:at + 8].rstrip(b'\0')
        virtual_size, rva, raw_size, raw_offset = struct.unpack_from('<4I', data, at + 8)
        assert raw_offset + raw_size <= len(data)
        rows.append((name, virtual_size, rva, raw_size, raw_offset))
    assert rows == [(b'.text', 0x36, 0x1000, 512, 0x400),
                    (b'.rdata', 0x84, 0x2000, 512, 0x600),
                    (b'.data', 0x10, 0x3000, 512, 0x800),
                    (b'.pdata', 0xc, 0x4000, 512, 0xa00),
                    (b'.reloc', 0xc, 0x5000, 512, 0xc00)]
    import_rva = u32(optional + 112 + 8)
    assert import_rva == 0x2000
    lookup_rva = u32(0x600)
    assert lookup_rva == 0x2028
    delta = lookup_rva - rows[1][2]
    assert 0 <= delta and delta + 8 <= rows[1][3]
    file_offset = rows[1][4] + delta
    assert file_offset == 0x628 and base + lookup_rva == 0x140002028
    assert struct.unpack_from('<Q', data, file_offset)[0] == 0x2058
    print('PE diagram: headers, raw regions, RVA conversion and lookup entry agree')


def check_unwind(path):
    dump = subprocess.run(['llvm-dwarfdump', '--eh-frame', str(path)],
                          check=True, capture_output=True, text=True).stdout
    match = re.search(r'FDE cie=[0-9a-f]+ pc=([0-9a-f]+)\.\.\.([0-9a-f]+)', dump)
    assert match, 'odd.s must have a DWARF fallback FDE'
    start, end = (int(value, 16) for value in match.groups())
    assert end - start == 20
    # ldp is at +12; the following ret at +16 uses the restored SP/registers.
    ret_row = rf'^\s*0x{start + 16:x}: CFA=WSP\s*$'
    assert re.search(ret_row, dump, re.MULTILINE), 'ret must not retain CFA=W19+16'
    for register in ['W19', 'W30']:
        assert f'DW_CFA_restore: {register}' in dump
    print('CFI: the ret row uses restored SP, x19 and x30')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('macho_object', type=Path)
    parser.add_argument('pe_image', type=Path)
    parser.add_argument('odd_object', type=Path)
    args = parser.parse_args()
    check_macho(args.macho_object)
    check_pe(args.pe_image)
    check_unwind(args.odd_object)


if __name__ == '__main__':
    main()

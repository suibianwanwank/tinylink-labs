#!/usr/bin/env python3
"""One PC-relative defect observed at several independent testing boundaries."""
import json
import platform
import resource
import signal
import struct
import subprocess
import tempfile
from pathlib import Path


def run(*args, cwd):
    return subprocess.run(args, cwd=cwd, check=True, capture_output=True, text=True, timeout=30).stdout


def load_segments(data):
    assert data[:7] == b'\x7fELF\x02\x01\x01'
    entry, phoff = struct.unpack_from('<QQ', data, 24)
    phentsize, phnum = struct.unpack_from('<HH', data, 54)
    assert phentsize == 56 and phoff + phnum * phentsize <= len(data)
    segments = []
    for index in range(phnum):
        kind, flags, offset, address, _, filesz, memsz, alignment = struct.unpack_from(
            '<IIQQQQQQ', data, phoff + index * phentsize
        )
        if kind != 1:
            continue
        assert filesz <= memsz and offset + filesz <= len(data)
        assert alignment in (0, 1) or (
            alignment & (alignment - 1) == 0 and offset % alignment == address % alignment
        )
        segments.append((flags, offset, address, filesz, memsz))
    assert any(flags & 1 and address <= entry < address + memsz
               for flags, _, address, _, memsz in segments)
    return entry, segments


def main():
    assert platform.system() == 'Linux' and platform.machine() == 'x86_64', (
        'Run this example on native x86-64 Linux'
    )
    resource.setrlimit(resource.RLIMIT_CORE, (0, 0))
    source = Path(__file__).resolve().parent
    work = Path(tempfile.mkdtemp(prefix='linker-relocation-evidence.'))
    run('cc', '-c', str(source / 'start.s'), '-o', 'start.o', cwd=work)
    run('cc', '-c', str(source / 'answer.s'), '-o', 'answer.o', cwd=work)
    run('ld', '-Ttext=0x401000', '-o', 'correct', 'start.o', 'answer.o', cwd=work)
    original = (work / 'correct').read_bytes()
    run('ld', '-Ttext=0x401000', '-o', 'correct-repeat', 'start.o', 'answer.o', cwd=work)
    assert (work / 'correct-repeat').read_bytes() == original
    relocations = run('readelf', '-rW', 'start.o', cwd=work)
    assert 'R_X86_64_PLT32' in relocations and 'answer - 4' in relocations
    entry, segments = load_segments(original)
    assert entry == 0x401000
    start = next(offset + entry - address
                 for _, offset, address, filesz, _ in segments
                 if address <= entry < address + filesz)
    assert original[start:start + 5] == bytes.fromhex('e8 1b 00 00 00')
    assert original[start + 28:start + 32] == b'\xcc' * 4
    assert original[start + 32:start + 38] == bytes.fromhex('b8 2a 00 00 00 c3')
    mutated = bytearray(original)
    mutated[start + 1:start + 5] = (0x17).to_bytes(4, 'little')
    assert load_segments(mutated) == (entry, segments)
    wrong = work / 'wrong'
    wrong.write_bytes(mutated)
    wrong.chmod(0o755)
    outcomes = [subprocess.run([str(work / name)], cwd=work, capture_output=True, timeout=30).returncode
                for name in ('correct', 'wrong')]
    assert outcomes == [42, -signal.SIGTRAP], outcomes
    print(json.dumps({
        'work': str(work), 'structure_checks': 'both pass',
        'repeated_link_bytes': 'identical', 'input_relocation': 'PLT32 answer - 4',
        'correct_call': original[start:start + 5].hex(' '),
        'wrong_call': bytes(mutated[start:start + 5]).hex(' '),
        'correct_target': hex(entry + 5 + 0x1b),
        'wrong_target': hex(entry + 5 + 0x17),
        'correct_exit': outcomes[0], 'wrong_signal': 'SIGTRAP',
        'observed_wrong_returncode': outcomes[1],
    }, indent=2))


if __name__ == '__main__':
    main()

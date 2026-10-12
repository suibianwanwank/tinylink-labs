#!/usr/bin/env python3
"""Reproduce cross-file LTO on native x86-64 Linux, without a macOS sysroot."""
from pathlib import Path
import platform
import shutil
import subprocess
import tempfile

if platform.system() != 'Linux' or platform.machine() != 'x86_64':
    raise SystemExit('requires native x86-64 Linux')
source = Path(__file__).resolve().parent / 'puzzle'
work = Path(tempfile.mkdtemp(prefix='linker-ch12-lto-'))
print('Evidence:', work, flush=True)
for name in ['main.c', 'util.c']:
    shutil.copy(source / name, work / name)

def command(*args):
    p = subprocess.run(args, cwd=work, capture_output=True, text=True)
    print('$', ' '.join(args), '\n' + p.stdout + p.stderr, flush=True)
    p.check_returncode()
    return p.stdout

command('uname', '-srmo')
command('clang', '--version')
command('ld.lld', '--version')
command('gcc', '--version')
for compiler in ['clang', 'gcc']:
    for mode in ['plain', 'lto']:
        flags = ['-O2'] + (['-flto'] if mode == 'lto' else [])
        objects = []
        for name in ['main', 'util']:
            obj = f'{compiler}-{mode}-{name}.o'
            command(compiler, *flags, '-c', name + '.c', '-o', obj)
            objects.append(obj)
        exe = f'{compiler}-{mode}'
        command(compiler, *flags, *(['-fuse-ld=lld'] if compiler == 'clang' else []), *objects, '-o', exe)
        symbols = command('nm', '--defined-only', exe)
        names = {line.split()[-1] for line in symbols.splitlines() if line.split()}
        assert 'main' in names
        if mode == 'plain':
            assert {'scale', 'twice', 'checksum'} <= names
        else:
            assert not {'scale', 'twice', 'checksum'} & names
        command('size', exe)
    for args in [[], ['a'], ['a', 'b']]:
        results = []
        for mode in ['plain', 'lto']:
            run = subprocess.run([str(work / f'{compiler}-{mode}'), *args], capture_output=True, timeout=15)
            assert run.returncode >= 0, (compiler, mode, run)
            results.append((run.returncode, run.stdout, run.stderr))
        assert results[0] == results[1], (compiler, args, results)
        print('PASS', compiler, 'argc', len(args) + 1, 'exit', results[0][0], flush=True)
assert (work / 'clang-lto-main.o').read_bytes()[:4] == b'BC\xc0\xde'
print('PASS: separate objects, LLVM bitcode, symbol elimination and runtime equivalence')

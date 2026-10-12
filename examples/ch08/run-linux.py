#!/usr/bin/env python3
"""Native Linux backtrace/symbolization checks, with addresses read from each build."""
import os
import json
from pathlib import Path
import platform
import re
import shutil
import subprocess
import tempfile

source = Path(__file__).resolve().parent
if platform.system() != 'Linux' or platform.machine() != 'x86_64':
    raise SystemExit('Run on native x86-64 Linux with glibc, GCC, binutils and Rust installed')
work = Path(tempfile.mkdtemp(prefix='linker-ch08-'))
print('work:', work, flush=True)
shutil.copy(source / 'bt.c', work / 'bt.c')

records = {}

def run(*args):
    p = subprocess.run(args, cwd=work, text=True, stdout=subprocess.PIPE, stderr=subprocess.STDOUT)
    print('$', ' '.join(map(str, args)), '\n' + p.stdout, flush=True)
    records[' '.join(map(str,args))] = p.stdout
    p.check_returncode()
    return p.stdout

run('uname', '-srmo')
run('gcc', '--version')
run('ld', '--version')
run('ldd', '--version')
for name, flags in [('bt', []), ('bt_rd', ['-rdynamic']), ('bt_static', ['-static', '-rdynamic']), ('bt_nohdr', ['-Wl,--no-eh-frame-hdr'])]:
    run('gcc', '-O1', '-g', '-fno-optimize-sibling-calls', '-fdebug-prefix-map='+str(work)+'=/src', 'bt.c', '-o', name, *flags)
for original, name, flags in [('bt', 'bt_nodebug', ['--strip-debug']), ('bt', 'bt_strip', []), ('bt_rd', 'bt_rd_strip', [])]:
    shutil.copy(work / original, work / name)
    run('strip', *flags, name)

outputs = {name: run('./' + name) for name in ['bt', 'bt_rd', 'bt_nodebug', 'bt_strip', 'bt_rd_strip', 'bt_static', 'bt_nohdr']}
for name in ['bt_rd', 'bt_rd_strip']:
    for symbol in ['leaf', 'middle', 'main']:
        assert '(' + symbol + '+' in outputs[name], (name, symbol)
    assert '(report+' not in outputs[name], 'static report must not be dynamically exported'
for name in ['bt', 'bt_strip', 'bt_nodebug']:
    assert '(leaf+' not in outputs[name], name

# Read addresses from the current ELF, never reuse another architecture's offsets.
nm = run('nm', '-n', 'bt')
address = re.search(r'^([0-9a-f]+) [tT] report$', nm, re.M).group(1)
for name in ['bt', 'bt_nodebug', 'bt_strip']:
    text = run('addr2line', '-f', '-e', name, '0x' + address)
    if name == 'bt':
        assert text.startswith('report\n') and 'bt.c:' in text
    elif name == 'bt_nodebug':
        assert text.startswith('report\n') and '??:' in text
    else:
        assert text.startswith('??\n') and '??:' in text
assert 'GNU_EH_FRAME' in run('readelf', '-lW', 'bt')
assert 'GNU_EH_FRAME' not in run('readelf', '-lW', 'bt_nohdr')
run('readelf', '--debug-dump=frames', 'bt')
# Keep build products outside the source tree.
os.environ['CARGO_TARGET_DIR'] = str(work / 'cargo-target')
run('cargo', 'build', '--offline', '--release', '--manifest-path', str(source / 'ehhdr/Cargo.toml'))
run(str(work / 'cargo-target/release/ehhdr'), 'bt', '--check')
print('PASS: exported names, stripping, source lookup and unwind header consistency')
print('Static and no-header backtrace depth is observational; it depends on the runtime lookup path.')

for args in [('readelf','--dyn-syms','-W','bt_rd'), ('readelf','-sW','bt_rd'), ('readelf','--debug-dump=decodedline','bt'), ('objdump','-d','bt'), ('objdump','-s','-j','.eh_frame_hdr','-j','.eh_frame','bt'), ('readelf','-SW','bt'), ('nm','-n','bt_rd')]:
    run(*args)
print('sizes', {n:(work/n).stat().st_size for n in ['bt','bt_nodebug','bt_strip']})
(work/'evidence.json').write_text(json.dumps(records,indent=2))

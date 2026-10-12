# 改写 ELF64 小端文件头或程序头里的一个字段。
# 用法: python3 patch.py 源文件 目标文件 字段 新值 [程序头下标]
# 字段: e_entry e_machine p_flags p_offset p_vaddr p_filesz p_memsz
import shutil, struct, sys

src, dst, field, val = sys.argv[1:5]
idx = int(sys.argv[5]) if len(sys.argv) > 5 else None
val = int(val, 0)
b = bytearray(open(src, 'rb').read())
if field == 'e_entry':
    struct.pack_into('<Q', b, 24, val)
elif field == 'e_machine':
    struct.pack_into('<H', b, 18, val)
else:
    phoff, = struct.unpack_from('<Q', b, 32)
    # Elf64_Phdr: p_type(4) p_flags(4) p_offset p_vaddr p_paddr p_filesz p_memsz p_align
    if field == 'p_flags':
        struct.pack_into('<I', b, phoff + 56 * idx + 4, val)
    else:
        off = {'p_offset': 8, 'p_vaddr': 16, 'p_filesz': 32, 'p_memsz': 40}[field]
        struct.pack_into('<Q', b, phoff + 56 * idx + off, val)
open(dst, 'wb').write(b)
shutil.copymode(src, dst)

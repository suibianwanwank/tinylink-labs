import struct, sys

data = open(sys.argv[1], "rb").read()
# ELF64 头：e_shoff 在偏移 0x28，e_shentsize/e_shnum/e_shstrndx 在 0x3a 起
shoff = struct.unpack_from("<Q", data, 0x28)[0]
shentsize, shnum, shstrndx = struct.unpack_from("<HHH", data, 0x3a)

TYPES = {0: "NULL", 1: "PROGBITS", 2: "SYMTAB", 3: "STRTAB", 4: "RELA",
         8: "NOBITS", 0x70000001: "X86_64_UNWIND", 0x6fff4c03: "LLVM_ADDRSIG"}
FLAGS = [(0x1, "W"), (0x2, "A"), (0x4, "X"), (0x10, "M"), (0x20, "S"), (0x40, "I")]

def shdr(i):
    # sh_name, sh_type, sh_flags, sh_addr, sh_offset, sh_size,
    # sh_link, sh_info, sh_addralign, sh_entsize
    return struct.unpack_from("<IIQQQQIIQQ", data, shoff + i * shentsize)

names_off = shdr(shstrndx)[4]
def name(off):
    end = data.index(b"\0", names_off + off)
    return data[names_off + off:end].decode()

print("Nr Name             Type          Flg Off    Size   Lk Inf Al ES")
for i in range(shnum):
    n, t, f, _, off, size, link, info, align, es = shdr(i)
    flg = "".join(c for bit, c in FLAGS if f & bit)
    print(f"{i:2} {name(n):16} {TYPES.get(t, hex(t)):13} {flg:3} "
          f"{off:06x} {size:06x} {link:2} {info:3} {align:2} {es:2}")

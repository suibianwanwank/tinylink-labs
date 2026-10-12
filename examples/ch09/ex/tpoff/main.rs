use std::{env, fs};

fn u16_at(b: &[u8], o: usize) -> u64 { u16::from_le_bytes(b[o..o + 2].try_into().unwrap()) as u64 }
fn u32_at(b: &[u8], o: usize) -> u64 { u32::from_le_bytes(b[o..o + 4].try_into().unwrap()) as u64 }
fn u64_at(b: &[u8], o: usize) -> u64 { u64::from_le_bytes(b[o..o + 8].try_into().unwrap()) }
fn round_up(x: u64, a: u64) -> u64 { if a <= 1 { x } else { (x + a - 1) / a * a } }

fn cstr(b: &[u8], o: usize) -> &str {
    let end = b[o..].iter().position(|&c| c == 0).unwrap();
    std::str::from_utf8(&b[o..o + end]).unwrap_or("?")
}

fn main() {
    let path = env::args().nth(1).expect("usage: tpoff <ELF>");
    let b = fs::read(&path).expect("read");
    assert!(&b[..4] == b"\x7fELF" && b[4] == 2 && b[5] == 1, "only ELF64 little-endian");
    let machine = u16_at(&b, 18);

    // 程序头：找 PT_TLS（类型 7）
    let (phoff, phentsize, phnum) = (u64_at(&b, 32) as usize, u16_at(&b, 54) as usize, u16_at(&b, 56) as usize);
    let tls = (0..phnum).map(|i| phoff + i * phentsize).find(|&p| u32_at(&b, p) == 7)
        .expect("no PT_TLS");
    let (vaddr, filesz, memsz, align) =
        (u64_at(&b, tls + 16), u64_at(&b, tls + 32), u64_at(&b, tls + 40), u64_at(&b, tls + 48));
    assert!(vaddr % align.max(1) == 0, "p_vaddr not aligned to p_align: not handled");
    println!("PT_TLS filesz={filesz:#x} memsz={memsz:#x} align={align:#x}");

    // 线程指针到"块内偏移 0"的距离
    let base: i64 = match machine {
        62 => -(round_up(memsz, align) as i64),  // x86-64, Variant II
        183 => round_up(16, align) as i64,       // AArch64, Variant I, TCB 16 字节
        m => panic!("e_machine {m} not handled"),
    };

    // 节头：找 SHT_SYMTAB（类型 2），它的 sh_link 是字符串表
    let (shoff, shentsize, shnum) = (u64_at(&b, 40) as usize, u16_at(&b, 58) as usize, u16_at(&b, 60) as usize);
    let sh = |i: usize| shoff + i * shentsize;
    let symtab = (0..shnum).map(sh).find(|&s| u32_at(&b, s + 4) == 2).expect("no .symtab (stripped?)");
    let (symoff, symsize) = (u64_at(&b, symtab + 24) as usize, u64_at(&b, symtab + 32) as usize);
    let stroff = u64_at(&b, sh(u32_at(&b, symtab + 40) as usize) + 24) as usize;

    for s in (symoff..symoff + symsize).step_by(24) {
        let (info, shndx, value) = (b[s + 4], u16_at(&b, s + 6), u64_at(&b, s + 8));
        let name = cstr(&b, stroff + u32_at(&b, s) as usize);
        if info & 0xf != 6 || shndx == 0 || name.is_empty() || name.starts_with('$') { continue; }
        let off = base + value as i64;
        let sign = if off < 0 { "-" } else { "+" };
        println!("{name:<12} st_value={value:#06x}  tpoff={sign}{:#x}", off.unsigned_abs());
    }
}

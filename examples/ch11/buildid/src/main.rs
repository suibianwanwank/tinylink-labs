//! 第 11 章练习：读 ELF64 小端文件的 build-id，拼出分离调试文件的路径；
//! 顺带列出带 SHF_COMPRESSED 标志的节和它们的 Elf64_Chdr。
//! 用法：buildid <ELF 文件> [调试目录，默认 /usr/lib/debug]
use std::{env, fs, process};

const SHT_NOTE: u32 = 7;
const SHF_COMPRESSED: u64 = 0x800;
const NT_GNU_BUILD_ID: u32 = 3;

fn u16_at(b: &[u8], o: usize) -> u16 { u16::from_le_bytes(b[o..o + 2].try_into().unwrap()) }
fn u32_at(b: &[u8], o: usize) -> u32 { u32::from_le_bytes(b[o..o + 4].try_into().unwrap()) }
fn u64_at(b: &[u8], o: usize) -> u64 { u64::from_le_bytes(b[o..o + 8].try_into().unwrap()) }
fn align4(x: usize) -> usize { (x + 3) & !3 }

struct Shdr { name: u32, ty: u32, flags: u64, offset: u64, size: u64 }

fn section_headers(b: &[u8]) -> Result<Vec<Shdr>, String> {
    if b.len() < 64 || &b[..4] != b"\x7fELF" { return Err("不是 ELF 文件".into()); }
    if b[4] != 2 || b[5] != 1 { return Err("只支持 ELF64 小端".into()); }
    let shoff = u64_at(b, 0x28) as usize;
    let shentsize = u16_at(b, 0x3a) as usize;
    let shnum = u16_at(b, 0x3c) as usize;
    (0..shnum).map(|i| {
        let o = shoff + i * shentsize;
        if o + 64 > b.len() { return Err(format!("节头 {i} 越界")); }
        Ok(Shdr { name: u32_at(b, o), ty: u32_at(b, o + 4), flags: u64_at(b, o + 8),
                  offset: u64_at(b, o + 24), size: u64_at(b, o + 32) })
    }).collect()
}

fn section_name<'a>(b: &'a [u8], shstr: &Shdr, s: &Shdr) -> &'a str {
    let start = shstr.offset as usize + s.name as usize;
    let end = b[start..].iter().position(|&c| c == 0).map_or(b.len(), |n| start + n);
    std::str::from_utf8(&b[start..end]).unwrap_or("?")
}

/// 在一个 SHT_NOTE 节里逐条找 "GNU" 名下、类型为 3 的附注，返回它的描述字节。
fn find_build_id<'a>(b: &'a [u8], s: &Shdr) -> Option<&'a [u8]> {
    let (mut p, end) = (s.offset as usize, (s.offset + s.size) as usize);
    while p + 12 <= end {
        let (namesz, descsz, ty) = (u32_at(b, p) as usize, u32_at(b, p + 4) as usize, u32_at(b, p + 8));
        let name = &b[p + 12..p + 12 + namesz];
        let desc = p + 12 + align4(namesz);
        if ty == NT_GNU_BUILD_ID && name == b"GNU\0" { return Some(&b[desc..desc + descsz]); }
        p = desc + align4(descsz);
    }
    None
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let Some(path) = args.get(1) else { eprintln!("用法：buildid <ELF 文件> [调试目录]"); process::exit(2) };
    let dir = args.get(2).map_or("/usr/lib/debug", |s| s.as_str());
    let b = fs::read(path).unwrap_or_else(|e| { eprintln!("{path}: {e}"); process::exit(1) });
    let shdrs = section_headers(&b).unwrap_or_else(|e| { eprintln!("{path}: {e}"); process::exit(1) });
    let shstr = &shdrs[u16_at(&b, 0x3e) as usize];

    let mut found = false;
    for s in &shdrs {
        let name = section_name(&b, shstr, s);
        if s.ty == SHT_NOTE {
            if let Some(id) = find_build_id(&b, s) {
                let hex: String = id.iter().map(|x| format!("{x:02x}")).collect();
                println!("{name}: build-id {hex}（{} 字节）", id.len());
                println!("{dir}/.build-id/{}/{}.debug", &hex[..2], &hex[2..]);
                found = true;
            }
        }
        if s.flags & SHF_COMPRESSED != 0 {
            let o = s.offset as usize;
            let kind = match u32_at(&b, o) { 1 => "zlib", 2 => "zstd", _ => "未知" };
            println!("{name}: 压缩 {kind}，文件里 {} 字节，解压后 {} 字节，对齐 {}",
                     s.size, u64_at(&b, o + 8), u64_at(&b, o + 16));
        }
    }
    if !found { println!("没有 build-id"); }
}

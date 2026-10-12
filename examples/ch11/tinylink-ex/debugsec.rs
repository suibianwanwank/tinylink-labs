//! 第 11 章练习四：在实验 2 的 ElfFile 上读压缩头，并给调试节的重定位目标分类。
//! 用法：cargo run --example debugsec -- <目标文件> [根符号，默认 _start]
use std::collections::BTreeSet;
use tinylink::elf::types::{SectionFlags, SectionIndex};
use tinylink::elf::{ElfFile, Section, SymbolTable};

/// `Elf64_Chdr`：压缩节开头的 24 字节。
#[derive(Debug)]
pub struct Chdr {
    pub kind: u32, // 1 = zlib，2 = zstd
    pub size: u64, // 解压后的大小
    pub align: u64,
}

pub fn chdr(s: &Section) -> Option<Chdr> {
    if !s.header.flags.contains(SectionFlags::COMPRESSED) {
        return None;
    }
    let d = s.data.get(..24)?;
    let u32_at = |o: usize| u32::from_le_bytes(d[o..o + 4].try_into().unwrap());
    let u64_at = |o: usize| u64::from_le_bytes(d[o..o + 8].try_into().unwrap());
    Some(Chdr { kind: u32_at(0), size: u64_at(8), align: u64_at(16) })
}

fn name(s: &Section) -> String {
    String::from_utf8_lossy(s.name.as_bytes()).into_owned()
}

fn is_alloc(s: &Section) -> bool {
    s.header.flags.contains(SectionFlags::ALLOC)
}

/// 一条重定位指向哪个节。未定义、绝对符号返回 None。
fn target(symtab: &SymbolTable, sym: u32) -> Option<usize> {
    match symtab.symbols.get(sym as usize)?.section {
        SectionIndex::Index(i) => Some(i),
        _ => None,
    }
}

/// 第 5 章的标记过程，只在一个文件内部做：从定义根符号的节出发，
/// 只沿加载节里的重定位走，调试节的引用不算数。
fn live_sections(f: &ElfFile, root: &str) -> tinylink::elf::Result<BTreeSet<usize>> {
    let mut live = BTreeSet::new();
    let mut work = Vec::new();
    if let Some(symtab) = f.symbol_table()? {
        for s in &symtab.symbols {
            if s.name.as_bytes() == root.as_bytes()
                && let SectionIndex::Index(i) = s.section
            {
                work.push(i);
            }
        }
    }
    while let Some(i) = work.pop() {
        if !live.insert(i) {
            continue;
        }
        for rela in f.rela_sections()? {
            if rela.header.info as usize != i {
                continue;
            }
            let symtab = f.linked_symbols(rela)?;
            for r in f.relocations(rela)? {
                if let Some(t) = target(&symtab, r.symbol)
                    && f.section(t).is_some_and(is_alloc)
                {
                    work.push(t);
                }
            }
        }
    }
    Ok(live)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    let data = std::fs::read(args.get(1).ok_or("用法：debugsec <目标文件> [根符号]")?)?;
    let root = args.get(2).map_or("_start", |s| s.as_str());
    let f = ElfFile::parse(&data)?;

    for s in f.sections() {
        if let Some(c) = chdr(s) {
            let alg = match c.kind { 1 => "zlib", 2 => "zstd", _ => "未知" };
            println!("{}: {alg}，{} -> {} 字节，对齐 {}", name(s), s.header.size, c.size, c.align);
        }
    }

    let live = live_sections(&f, root)?;
    for rela in f.rela_sections()? {
        let Some(dst) = f.section(rela.header.info as usize) else { continue };
        if !name(dst).starts_with(".debug") {
            continue;
        }
        let symtab = f.linked_symbols(rela)?;
        let (mut alloc, mut nonalloc, mut other) = (0, 0, 0);
        let mut dead = Vec::new();
        for r in f.relocations(rela)? {
            match target(&symtab, r.symbol).and_then(|t| f.section(t)) {
                Some(t) if !is_alloc(t) => nonalloc += 1,
                Some(t) if live.contains(&t.index) => alloc += 1,
                Some(t) => dead.push(format!("{}+{:#x}", name(t), r.offset)),
                None => other += 1,
            }
        }
        println!(
            "{}: 加载节 {alloc}，不加载节 {nonalloc}，会被 GC 的 {} {:?}，其他 {other}",
            name(rela), dead.len(), dead
        );
    }
    Ok(())
}

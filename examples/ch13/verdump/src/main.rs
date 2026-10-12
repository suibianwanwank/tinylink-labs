//! verdump：打印每个动态符号的版本，输出和 `readelf --dyn-syms -W` 的 Name 列对得上。
//!
//! 读三个节（第 7 章"符号版本"一节）：
//! - `.gnu.version`（SHT_GNU_versym）：和 `.dynsym` 一一对应的 16 位索引，最高位是"隐藏"；
//! - `.gnu.version_d`（SHT_GNU_verdef）：本模块定义的版本，索引 → 名字；
//! - `.gnu.version_r`（SHT_GNU_verneed）：本模块需要的版本，索引 → 名字和来源文件。

use std::collections::BTreeMap;
use std::process::ExitCode;

use tinylink::elf::read::Cursor;
use tinylink::elf::types::{SectionIndex, SectionType, SymbolType};
use tinylink::elf::{ElfError, ElfFile, Place, Section};

const SHT_GNU_VERDEF: u32 = 0x6fff_fffd;
const SHT_GNU_VERNEED: u32 = 0x6fff_fffe;
const SHT_GNU_VERSYM: u32 = 0x6fff_ffff;
const VERSYM_HIDDEN: u16 = 0x8000;

type Result<T> = std::result::Result<T, String>;

fn err(e: ElfError) -> String {
    e.to_string()
}

fn find<'a>(elf: &ElfFile<'a>, kind: u32) -> Option<Section<'a>> {
    elf.sections()
        .iter()
        .find(|s| s.header.kind == SectionType::Other(kind))
        .copied()
}

/// sh_link 指向的字符串表里，偏移 `off` 处以 NUL 结尾的字符串。
fn str_at(elf: &ElfFile<'_>, sec: &Section<'_>, off: u32) -> Result<String> {
    let strtab = elf
        .section(sec.header.link as usize)
        .ok_or_else(|| format!("section {}: bad sh_link {}", sec.index, sec.header.link))?;
    let tail = strtab
        .data
        .get(off as usize..)
        .ok_or_else(|| format!("string offset {off:#x} outside section {}", strtab.index))?;
    let end = tail
        .iter()
        .position(|&b| b == 0)
        .ok_or_else(|| format!("unterminated string at {off:#x}"))?;
    Ok(String::from_utf8_lossy(&tail[..end]).into_owned())
}

/// 从节内偏移 `at` 处读，越界时报出是哪个节。
fn cursor<'a>(sec: &Section<'a>, at: u64) -> Result<Cursor<'a>> {
    let bytes = sec
        .data
        .get(at as usize..)
        .ok_or_else(|| format!("offset {at:#x} outside section {}", sec.index))?;
    Ok(Cursor::new(bytes, sec.header.offset + at, Place::SectionData(sec.index)))
}

/// `.gnu.version_d`：Elf64_Verdef 链表，每项后面挂 vd_cnt 个 Elf64_Verdaux，第一个是版本名。
fn verdefs(elf: &ElfFile<'_>, sec: &Section<'_>) -> Result<BTreeMap<u16, String>> {
    let mut out = BTreeMap::new();
    let mut at = 0u64;
    // sh_info 是表项个数
    for _ in 0..sec.header.info {
        let mut c = cursor(sec, at)?;
        let _version = c.u16().map_err(err)?;
        let _flags = c.u16().map_err(err)?;
        let ndx = c.u16().map_err(err)?;
        let _cnt = c.u16().map_err(err)?;
        let _hash = c.u32().map_err(err)?;
        let aux = c.u32().map_err(err)?;
        let next = c.u32().map_err(err)?;
        let name = cursor(sec, at + u64::from(aux))?.u32().map_err(err)?;
        out.insert(ndx, str_at(elf, sec, name)?);
        if next == 0 {
            break;
        }
        at += u64::from(next);
    }
    Ok(out)
}

/// `.gnu.version_r`：Elf64_Verneed 链表（每个依赖的文件一项），下挂 Elf64_Vernaux（每个版本一项）。
/// vna_other 就是 `.gnu.version` 里引用它的索引。
fn verneeds(elf: &ElfFile<'_>, sec: &Section<'_>) -> Result<BTreeMap<u16, (String, String)>> {
    let mut out = BTreeMap::new();
    let mut at = 0u64;
    for _ in 0..sec.header.info {
        let mut c = cursor(sec, at)?;
        let _version = c.u16().map_err(err)?;
        let cnt = c.u16().map_err(err)?;
        let file = str_at(elf, sec, c.u32().map_err(err)?)?;
        let aux = c.u32().map_err(err)?;
        let next = c.u32().map_err(err)?;
        let mut a = at + u64::from(aux);
        for _ in 0..cnt {
            let mut c = cursor(sec, a)?;
            let _hash = c.u32().map_err(err)?;
            let _flags = c.u16().map_err(err)?;
            let other = c.u16().map_err(err)?;
            let name = str_at(elf, sec, c.u32().map_err(err)?)?;
            let anext = c.u32().map_err(err)?;
            out.insert(other, (name, file.clone()));
            if anext == 0 {
                break;
            }
            a += u64::from(anext);
        }
        if next == 0 {
            break;
        }
        at += u64::from(next);
    }
    Ok(out)
}

fn dump(path: &str) -> Result<()> {
    let data = std::fs::read(path).map_err(|e| format!("{path}: {e}"))?;
    let elf = ElfFile::parse(&data).map_err(err)?;
    let dynsym = elf
        .sections()
        .iter()
        .find(|s| s.header.kind == SectionType::Dynsym)
        .ok_or_else(|| format!("{path}: no .dynsym"))?;
    let syms = elf.symbols_in(dynsym).map_err(err)?;
    let defs = match find(&elf, SHT_GNU_VERDEF) {
        Some(s) => verdefs(&elf, &s)?,
        None => BTreeMap::new(),
    };
    let needs = match find(&elf, SHT_GNU_VERNEED) {
        Some(s) => verneeds(&elf, &s)?,
        None => BTreeMap::new(),
    };
    let versym = find(&elf, SHT_GNU_VERSYM);

    for (i, sym) in syms.symbols.iter().enumerate() {
        let raw = match &versym {
            Some(s) => cursor(s, 2 * i as u64)?.u16().map_err(err)?,
            None => 1, // 没有版本信息：等同于 VER_NDX_GLOBAL
        };
        let ndx = raw & !VERSYM_HIDDEN;
        let undef = sym.section == SectionIndex::Undef;
        // 0 是 *local*，1 是 *global*（基础版本，不加后缀）
        let suffix = match ndx {
            0 | 1 => String::new(),
            n if undef => match needs.get(&n) {
                Some((v, _file)) => format!("@{v} ({n})"),
                None => format!("@<unknown {n}>"),
            },
            n => {
                // 定义：可见的是默认版本 @@，隐藏的是兼容版本 @
                let at = if raw & VERSYM_HIDDEN != 0 { "@" } else { "@@" };
                match defs.get(&n).or_else(|| needs.get(&n).map(|(v, _)| v)) {
                    // GNU ld 给每个版本名定义一个同名的 ABS 符号，readelf 不给它加后缀
                    Some(v) if sym.name == v.as_str() => String::new(),
                    Some(v) => format!("{at}{v}"),
                    None => format!("{at}<unknown {n}>"),
                }
            }
        };
        // 节符号没有名字，readelf 显示它所在节的节名
        let name = match (sym.kind, sym.section) {
            (SymbolType::Section, SectionIndex::Index(n)) => elf
                .section(n)
                .map(|s| s.name.to_string())
                .unwrap_or_default(),
            _ => sym.name.to_string(),
        };
        println!("{i:6}: {name}{suffix}");
    }
    Ok(())
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() {
        eprintln!("usage: verdump FILE...");
        return ExitCode::from(2);
    }
    let mut status = ExitCode::SUCCESS;
    for path in &args {
        if args.len() > 1 {
            println!("{path}:");
        }
        if let Err(e) = dump(path) {
            eprintln!("verdump: {e}");
            status = ExitCode::FAILURE;
        }
    }
    status
}

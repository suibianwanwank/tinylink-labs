//! M2：把一个没有重定位的目标文件写成可执行文件。
//!
//! 输出只有三样东西：ELF 头、一个程序头、紧跟其后的各个可分配节的内容。
//! 唯一的程序头是 `PT_LOAD`，权限 R+X，从文件偏移 0 开始，把整个文件映射到
//! `BASE` 处。ELF 头和程序头本身也落在这个段里，这和 GNU ld 默认产物的第一个段一样。
//! 没有节头表：内核加载程序只看程序头（第 2 章"两种视图"一节）。

use std::fmt;

use crate::elf::types::{
    FileType, Machine, SectionFlags, SectionIndex, SectionType, SegmentFlags, SegmentType,
    SymbolBinding,
};
use crate::elf::{EHDR_SIZE, ElfError, ElfFile, Header, PHDR_SIZE, ProgramHeader, Section};

/// 段的加载地址。x86-64 和 AArch64 上 GNU ld 的默认值都是 0x400000（4 MiB）。
pub const BASE: u64 = 0x40_0000;

/// 入口符号的名字。
pub const ENTRY: &str = "_start";

/// 输出文件的大小上限。输入是不可信的，一个离谱的对齐值就可能要求补上几个 GB 的零。
pub const MAX_OUTPUT: u64 = 1 << 30;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LinkError {
    /// 输入文件本身解析失败。
    Elf(ElfError),
    NotRelocatable(FileType),
    UnsupportedMachine(Machine),
    /// 输入里有重定位，M2 还不会处理。
    HasRelocations {
        section: String,
        count: u64,
    },
    UnsupportedSection {
        section: String,
        reason: &'static str,
    },
    NoEntry,
    EntryOutsideSection {
        value: u64,
        size: u64,
    },
    TooLarge,
}

/// 有了这个实现，`?` 会把 `ElfError` 自动转换成 `LinkError::Elf`。
impl From<ElfError> for LinkError {
    fn from(e: ElfError) -> Self {
        LinkError::Elf(e)
    }
}

impl fmt::Display for LinkError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LinkError::Elf(e) => write!(f, "{e}"),
            LinkError::NotRelocatable(kind) => {
                write!(f, "input is {kind}, not a relocatable object file")
            }
            LinkError::UnsupportedMachine(m) => {
                write!(
                    f,
                    "unsupported machine {m}; only x86-64 and AArch64 are supported"
                )
            }
            LinkError::HasRelocations { section, count } => write!(
                f,
                "relocations are not supported yet: section '{section}' has {count} {}",
                if *count == 1 { "entry" } else { "entries" }
            ),
            LinkError::UnsupportedSection { section, reason } => {
                write!(f, "section '{section}' is not supported yet: {reason}")
            }
            LinkError::NoEntry => write!(
                f,
                "entry symbol '{ENTRY}' is not defined (it must be a global symbol in an allocated section)"
            ),
            LinkError::EntryOutsideSection { value, size } => write!(
                f,
                "entry symbol '{ENTRY}' has value {value:#x}, outside its section of {size:#x} bytes"
            ),
            LinkError::TooLarge => write!(f, "output would be larger than {MAX_OUTPUT:#x} bytes"),
        }
    }
}

// 不实现 source：`Display` 已经把内层的 ElfError 拼了进去。std::error::Error 的文档建议
// 二者只选其一，都做的话，打印错误链的代码会把同一句话打两遍。
impl std::error::Error for LinkError {}

/// 每种 CPU 的页大小上限。段的文件偏移和虚拟地址必须对它同余。
fn max_page_size(machine: Machine) -> u64 {
    match machine {
        // AArch64 的 Linux 内核可以配置成 4 KiB、16 KiB 或 64 KiB 的页，按最大的来。
        Machine::AArch64 => 0x1_0000,
        _ => 0x1000,
    }
}

/// 一个被放进输出文件的节。
struct Placed<'f, 'a> {
    section: &'f Section<'a>,
    offset: u64,
}

/// 把 `value` 向上取整到 `align` 的倍数。`align` 必须是 2 的幂；溢出时返回 `None`。
fn align_up(value: u64, align: u64) -> Option<u64> {
    let mask = align.checked_sub(1)?;
    Some(value.checked_add(mask)? & !mask)
}

/// 检查输入，决定每个节放在输出文件的什么位置。
fn layout<'f, 'a>(file: &'f ElfFile<'a>) -> Result<Vec<Placed<'f, 'a>>, LinkError> {
    let header = file.header();
    if header.kind != FileType::Rel {
        return Err(LinkError::NotRelocatable(header.kind));
    }
    if !matches!(header.machine, Machine::X86_64 | Machine::AArch64) {
        return Err(LinkError::UnsupportedMachine(header.machine));
    }
    for s in file.sections() {
        let is_reloc = matches!(s.header.kind, SectionType::Rela | SectionType::Rel);
        if is_reloc && !s.data.is_empty() {
            let entsize = s.header.entsize.max(1);
            return Err(LinkError::HasRelocations {
                section: s.name.to_string(),
                count: s.header.size / entsize,
            });
        }
    }

    let mut offset = EHDR_SIZE + PHDR_SIZE;
    let mut placed = Vec::new();
    for s in file.sections() {
        let flags = s.header.flags;
        if !flags.contains(SectionFlags::ALLOC) || s.header.size == 0 {
            continue;
        }
        let unsupported = |reason| LinkError::UnsupportedSection {
            section: s.name.to_string(),
            reason,
        };
        if flags.contains(SectionFlags::WRITE) {
            return Err(unsupported(
                "it is writable, and the only segment is read-only",
            ));
        }
        if !matches!(s.header.kind, SectionType::Progbits | SectionType::Note) {
            return Err(unsupported("only PROGBITS and NOTE sections can be copied"));
        }
        let align = s.header.addralign.max(1);
        if !align.is_power_of_two() {
            return Err(unsupported("sh_addralign is not a power of two"));
        }
        if align > max_page_size(header.machine) {
            return Err(unsupported("sh_addralign is larger than the page size"));
        }
        offset = align_up(offset, align).ok_or(LinkError::TooLarge)?;
        placed.push(Placed { section: s, offset });
        offset = offset
            .checked_add(s.header.size)
            .ok_or(LinkError::TooLarge)?;
    }
    if offset > MAX_OUTPUT {
        return Err(LinkError::TooLarge);
    }
    Ok(placed)
}

/// 链接一个目标文件，返回可执行文件的全部字节。
pub fn link(file: &ElfFile<'_>) -> Result<Vec<u8>, LinkError> {
    let placed = layout(file)?;

    // 入口：全局的 `_start`，定义在某个被放进输出的节里。
    let symbols = file.symbol_table()?.map(|t| t.symbols).unwrap_or_default();
    let (start, place) = symbols
        .iter()
        .filter(|s| s.name == ENTRY && s.binding != SymbolBinding::Local)
        .find_map(|s| match s.section {
            SectionIndex::Index(i) => placed.iter().find(|p| p.section.index == i).map(|p| (s, p)),
            _ => None,
        })
        .ok_or(LinkError::NoEntry)?;
    let section_size = place.section.header.size;
    if start.value >= section_size {
        return Err(LinkError::EntryOutsideSection {
            value: start.value,
            size: section_size,
        });
    }
    // layout() 保证了 offset 不超过 MAX_OUTPUT，value 又小于节的大小，这里的加法不会溢出。
    let entry = BASE + place.offset + start.value;

    let file_size = placed
        .last()
        .map_or(EHDR_SIZE + PHDR_SIZE, |p| p.offset + p.section.header.size);
    let machine = file.header().machine;
    let header = Header {
        ident: [0x7f, b'E', b'L', b'F', 2, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0],
        kind: FileType::Exec,
        machine,
        version: 1,
        entry,
        phoff: EHDR_SIZE, // 程序头紧跟在 ELF 头后面
        shoff: 0,         // 没有节头表
        flags: file.header().flags,
        // 64 和 56 是常量，放得进 u16。
        ehsize: EHDR_SIZE as u16,
        phentsize: PHDR_SIZE as u16,
        phnum: 1,
        shentsize: 0,
        shnum: 0,
        shstrndx: 0,
    };
    // 一个覆盖整个文件的 PT_LOAD。
    // p_offset = 0 与 p_vaddr = BASE 对页大小同余，内核才能按页把文件映射到这个地址。
    let segment = ProgramHeader {
        kind: SegmentType::Load,
        flags: SegmentFlags(SegmentFlags::R | SegmentFlags::X),
        offset: 0,
        vaddr: BASE,
        paddr: BASE,
        filesz: file_size,
        memsz: file_size,
        align: max_page_size(machine),
    };
    let mut out = Vec::new();
    header.write(&mut out);
    segment.write(&mut out);

    // 各节内容。节之间按对齐要求补零。
    for p in &placed {
        let offset = usize::try_from(p.offset).map_err(|_| LinkError::TooLarge)?;
        out.resize(offset, 0);
        out.extend_from_slice(p.section.data);
    }
    debug_assert_eq!(out.len() as u64, file_size);
    Ok(out)
}

//! 把没有重定位的目标文件写成可执行文件（M2，实验 3 起可以有多个输入）。
//!
//! 输出只有三样东西：ELF 头、一个程序头、紧跟其后的各个可分配节的内容。
//! 唯一的程序头是 `PT_LOAD`，权限 R+X，从文件偏移 0 开始，把整个文件映射到
//! `BASE` 处。ELF 头和程序头本身也落在这个段里，这和 GNU ld 默认产物的第一个段一样。
//! 没有节头表：内核加载程序只看程序头（第 2 章"两种视图"一节）。
//!
//! 多个输入时，各输入的节按命令行顺序、每个文件内按节号依次排下去。调用者先做符号解析
//! （`crate::resolve`），解析没有错误才调用 `link`。文件之间要互相引用，就得有重定位，
//! 那是实验 4 的事；这里遇到重定位只报"尚不支持"。

use std::fmt;

use crate::elf::types::{
    FileType, Machine, SectionFlags, SectionIndex, SectionType, SegmentFlags, SegmentType,
};
use crate::elf::{EHDR_SIZE, Header, PHDR_SIZE, ProgramHeader, Section};
use crate::resolve::{Definition, InputFile, Resolution};

/// 段的加载地址。x86-64 和 AArch64 上 GNU ld 的默认值都是 0x400000（4 MiB）。
pub const BASE: u64 = 0x40_0000;

/// 入口符号的名字。
pub const ENTRY: &str = "_start";

/// 输出文件的大小上限。输入是不可信的，一个离谱的对齐值就可能要求补上几个 GB 的零。
pub const MAX_OUTPUT: u64 = 1 << 30;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LinkError {
    UnsupportedMachine(Machine),
    /// 输入里有重定位，实验 4 才处理。
    HasRelocations {
        section: String,
        count: u64,
    },
    UnsupportedSection {
        section: String,
        reason: &'static str,
    },
    /// COMMON 符号要在 `.bss` 里分配空间，现在的单个只读段放不下它。
    Common {
        name: String,
    },
    EntryOutsideSection {
        value: u64,
        size: u64,
    },
    TooLarge,
}

impl fmt::Display for LinkError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
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
            LinkError::Common { name } => write!(
                f,
                "common symbol '{name}' needs space in .bss, which is not supported yet"
            ),
            LinkError::EntryOutsideSection { value, size } => write!(
                f,
                "entry symbol '{ENTRY}' has value {value:#x}, outside its section of {size:#x} bytes"
            ),
            LinkError::TooLarge => write!(f, "output would be larger than {MAX_OUTPUT:#x} bytes"),
        }
    }
}

impl std::error::Error for LinkError {}

/// 一个链接错误，和它出在哪个输入上（`None` 表示和具体输入无关）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkDiag {
    pub file: Option<usize>,
    pub error: LinkError,
}

/// 链接器的警告：不影响产物的生成。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkWarning {
    /// 没有可用的 `_start`。和 lld 一样，入口地址写 0（GNU ld 则改用 `.text` 的起点）。
    NoEntry,
}

impl fmt::Display for LinkWarning {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LinkWarning::NoEntry => write!(
                f,
                "cannot find entry symbol {ENTRY}; not setting start address"
            ),
        }
    }
}

/// 链接的产物：可执行文件的全部字节，和过程中的警告。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Linked {
    pub image: Vec<u8>,
    pub warnings: Vec<LinkWarning>,
}

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
    file: usize,
    section: &'f Section<'a>,
    offset: u64,
}

/// 把 `value` 向上取整到 `align` 的倍数。`align` 必须是 2 的幂；溢出时返回 `None`。
fn align_up(value: u64, align: u64) -> Option<u64> {
    let mask = align.checked_sub(1)?;
    Some(value.checked_add(mask)? & !mask)
}

/// 检查一个输入里不支持的东西：重定位，以及放不进只读段的节。
fn check(file: usize, input: &InputFile<'_>, diags: &mut Vec<LinkDiag>) {
    let mut fail = |error| {
        diags.push(LinkDiag {
            file: Some(file),
            error,
        })
    };
    for s in input.elf.sections() {
        let is_reloc = matches!(s.header.kind, SectionType::Rela | SectionType::Rel);
        if is_reloc && !s.data.is_empty() {
            let entsize = s.header.entsize.max(1);
            fail(LinkError::HasRelocations {
                section: s.name.to_string(),
                count: s.header.size / entsize,
            });
        }
    }
    for s in input.elf.sections() {
        let flags = s.header.flags;
        if !flags.contains(SectionFlags::ALLOC) || s.header.size == 0 {
            continue;
        }
        let mut unsupported = |reason| {
            fail(LinkError::UnsupportedSection {
                section: s.name.to_string(),
                reason,
            })
        };
        let align = s.header.addralign.max(1);
        if flags.contains(SectionFlags::WRITE) {
            unsupported("it is writable, and the only segment is read-only");
        } else if !matches!(s.header.kind, SectionType::Progbits | SectionType::Note) {
            unsupported("only PROGBITS and NOTE sections can be copied");
        } else if !align.is_power_of_two() {
            unsupported("sh_addralign is not a power of two");
        } else if align > max_page_size(input.elf.header().machine) {
            unsupported("sh_addralign is larger than the page size");
        }
    }
}

/// 决定每个节放在输出文件的什么位置。`check` 已经保证每个可分配节都能放。
fn layout<'f, 'a>(inputs: &'f [InputFile<'a>]) -> Result<Vec<Placed<'f, 'a>>, LinkError> {
    let mut offset = EHDR_SIZE + PHDR_SIZE;
    let mut placed = Vec::new();
    for (file, input) in inputs.iter().enumerate() {
        for s in input.elf.sections() {
            if !s.header.flags.contains(SectionFlags::ALLOC) || s.header.size == 0 {
                continue;
            }
            offset = align_up(offset, s.header.addralign.max(1)).ok_or(LinkError::TooLarge)?;
            placed.push(Placed {
                file,
                section: s,
                offset,
            });
            offset = offset
                .checked_add(s.header.size)
                .ok_or(LinkError::TooLarge)?;
        }
    }
    if offset > MAX_OUTPUT {
        return Err(LinkError::TooLarge);
    }
    Ok(placed)
}

/// 入口地址。`_start` 要是一个放进了输出的节里的定义，或者一个绝对符号；
/// 都不是时返回 `Ok(None)`，由调用者发出警告。
fn entry(resolution: &Resolution<'_>, placed: &[Placed<'_, '_>]) -> Result<Option<u64>, LinkDiag> {
    let Some(id) = resolution.table.lookup(ENTRY.as_bytes()) else {
        return Ok(None);
    };
    let Definition::Defined {
        at, section, value, ..
    } = resolution.table.get(id).definition
    else {
        return Ok(None);
    };
    let index = match section {
        SectionIndex::Abs => return Ok(Some(value)),
        SectionIndex::Index(i) => i,
        _ => return Ok(None),
    };
    let Some(place) = placed
        .iter()
        .find(|p| p.file == at.file && p.section.index == index)
    else {
        return Ok(None);
    };
    let size = place.section.header.size;
    if value >= size {
        return Err(LinkDiag {
            file: Some(at.file),
            error: LinkError::EntryOutsideSection { value, size },
        });
    }
    // layout() 保证了 offset 不超过 MAX_OUTPUT，value 又小于节的大小，这里的加法不会溢出。
    Ok(Some(BASE + place.offset + value))
}

/// 把一组已经解析过符号的输入链接成可执行文件。所有问题一次报完。
pub fn link(
    inputs: &[InputFile<'_>],
    resolution: &Resolution<'_>,
) -> Result<Linked, Vec<LinkDiag>> {
    let mut diags = Vec::new();
    let Some(first) = inputs.first() else {
        return Err(diags);
    };
    let machine = first.elf.header().machine;
    if !matches!(machine, Machine::X86_64 | Machine::AArch64) {
        diags.push(LinkDiag {
            file: Some(0),
            error: LinkError::UnsupportedMachine(machine),
        });
    }
    for (file, input) in inputs.iter().enumerate() {
        check(file, input, &mut diags);
    }
    for (_, symbol) in resolution.table.iter() {
        if let Definition::Common { .. } = symbol.definition {
            diags.push(LinkDiag {
                file: None,
                error: LinkError::Common {
                    name: symbol.name.to_string(),
                },
            });
        }
    }
    if !diags.is_empty() {
        return Err(diags);
    }
    let placed = layout(inputs).map_err(|error| vec![LinkDiag { file: None, error }])?;
    let mut warnings = Vec::new();
    let entry = match entry(resolution, &placed).map_err(|d| vec![d])? {
        Some(address) => address,
        None => {
            warnings.push(LinkWarning::NoEntry);
            0
        }
    };

    let file_size = placed
        .last()
        .map_or(EHDR_SIZE + PHDR_SIZE, |p| p.offset + p.section.header.size);
    let header = Header {
        ident: [0x7f, b'E', b'L', b'F', 2, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0],
        kind: FileType::Exec,
        machine,
        version: 1,
        entry,
        phoff: EHDR_SIZE, // 程序头紧跟在 ELF 头后面
        shoff: 0,         // 没有节头表
        flags: first.elf.header().flags,
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
        let offset = usize::try_from(p.offset).map_err(|_| {
            vec![LinkDiag {
                file: None,
                error: LinkError::TooLarge,
            }]
        })?;
        out.resize(offset, 0);
        out.extend_from_slice(p.section.data);
    }
    debug_assert_eq!(out.len() as u64, file_size);
    Ok(Linked {
        image: out,
        warnings,
    })
}

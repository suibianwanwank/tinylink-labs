//! 解析 ELF 时可能遇到的错误。
//!
//! 输入文件是不可信的：任何一个偏移、大小、下标都可能是错的。每一种错误都说明
//! 出在文件的哪个结构上（`Place`），涉及的字段，以及具体的数值，方便对照十六进制转储排查。
//! 这里的错误只描述"文件里哪里不对"，不包含文件是谁；文件的身份由 `crate::input::InputError` 加上。

use std::fmt;

use super::types::SectionType;

/// 出错的位置：文件里的哪个结构。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Place {
    FileHeader,
    SectionHeaderTable,
    SectionHeader(usize),
    SectionData(usize),
    ProgramHeaderTable,
    ProgramHeader(usize),
    Symbol(usize),
    Relocation { section: usize, index: usize },
}

impl fmt::Display for Place {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Place::FileHeader => write!(f, "ELF header"),
            Place::SectionHeaderTable => write!(f, "section header table"),
            Place::SectionHeader(i) => write!(f, "section header {i}"),
            Place::SectionData(i) => write!(f, "contents of section {i}"),
            Place::ProgramHeaderTable => write!(f, "program header table"),
            Place::ProgramHeader(i) => write!(f, "program header {i}"),
            Place::Symbol(i) => write!(f, "symbol {i}"),
            Place::Relocation { section, index } => {
                write!(f, "relocation {index} in section {section}")
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ElfError {
    /// 要读的字节范围超出了文件末尾。
    OutOfBounds {
        place: Place,
        offset: u64,
        size: u64,
        file_len: u64,
    },
    /// 在一段已经取出的结构内部读过了头：结构的大小算错了，属于 tinylink 自己的 bug。
    PastEnd {
        place: Place,
        offset: u64,
        size: u64,
        end: u64,
    },
    /// 文件开头不是 `\x7fELF`。
    BadMagic { found: Vec<u8> },
    /// 字段的值合法，但 tinylink 还不支持。
    Unsupported {
        place: Place,
        field: &'static str,
        value: u64,
    },
    /// 节数或节名字符串表的下标放不进 ELF 头，改放在 0 号节头里（gABI 的扩展编号）。
    ExtendedNumbering {
        shnum: u16,
        shoff: u64,
        shstrndx: u16,
    },
    /// 一个下标指向了不存在的东西，比如 `sh_link` 指向第 99 个节。
    BadIndex {
        place: Place,
        field: &'static str,
        index: u64,
        count: u64,
    },
    /// 一个下标指向的节类型不对，比如符号表的 `sh_link` 指向的不是字符串表。
    WrongType {
        place: Place,
        field: &'static str,
        index: usize,
        expected: SectionType,
        found: SectionType,
    },
    /// 字符串表里的偏移不对：越界，或者没有结尾的 NUL。
    BadString {
        place: Place,
        field: &'static str,
        offset: u64,
        problem: StringProblem,
    },
    /// 表项大小（`sh_entsize`）和规范规定的不一致。
    BadEntrySize {
        section: usize,
        entsize: u64,
        expected: u64,
    },
    /// 节的大小不是表项大小的整数倍，最后一项不完整。
    NotMultipleOfEntry {
        section: usize,
        size: u64,
        entsize: u64,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StringProblem {
    /// 偏移超出了字符串表，表的大小附在后面。
    OutsideTable { table_size: u64 },
    /// 从偏移处一直到表尾都没有 NUL。
    Unterminated,
}

pub type Result<T> = std::result::Result<T, ElfError>;

impl fmt::Display for ElfError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ElfError::OutOfBounds {
                place,
                offset,
                size,
                file_len,
            } => write!(
                f,
                "{place}: bytes {offset:#x}..{:#x} are outside the file ({file_len:#x} bytes)",
                offset.saturating_add(*size)
            ),
            ElfError::PastEnd {
                place,
                offset,
                size,
                end,
            } => write!(
                f,
                "{place}: reading {size} bytes at {offset:#x} goes past the end of the structure at {end:#x}"
            ),
            ElfError::BadMagic { found } => {
                write!(f, "not an ELF file: magic is")?;
                for b in found {
                    write!(f, " {b:02x}")?;
                }
                write!(f, ", expected 7f 45 4c 46")
            }
            ElfError::Unsupported {
                place,
                field,
                value,
            } => write!(f, "{place}: unsupported {field} {value:#x}"),
            ElfError::ExtendedNumbering {
                shnum,
                shoff,
                shstrndx,
            } => {
                write!(
                    f,
                    "ELF header: extended section numbering is not supported yet ("
                )?;
                if *shstrndx == 0xffff {
                    write!(f, "e_shstrndx is 0xffff, SHN_XINDEX")?;
                } else {
                    write!(
                        f,
                        "e_shnum is {shnum} but e_shoff is {shoff:#x}, so the count is in sh_size of section header 0"
                    )?;
                }
                write!(f, ")")
            }
            ElfError::BadIndex {
                place,
                field,
                index,
                count,
            } => write!(
                f,
                "{place}: {field} {index} is out of range (there are {count})"
            ),
            ElfError::WrongType {
                place,
                field,
                index,
                expected,
                found,
            } => write!(
                f,
                "{place}: {field} points to section {index} of type {found}, expected {expected}"
            ),
            ElfError::BadString {
                place,
                field,
                offset,
                problem,
            } => {
                write!(f, "{place}: {field} {offset:#x} ")?;
                match problem {
                    StringProblem::OutsideTable { table_size } => {
                        write!(f, "is outside the string table ({table_size:#x} bytes)")
                    }
                    StringProblem::Unterminated => {
                        write!(f, "points to a string with no terminating NUL")
                    }
                }
            }
            ElfError::BadEntrySize {
                section,
                entsize,
                expected,
            } => write!(
                f,
                "section header {section}: sh_entsize is {entsize}, expected {expected}"
            ),
            ElfError::NotMultipleOfEntry {
                section,
                size,
                entsize,
            } => write!(
                f,
                "section header {section}: sh_size {size:#x} is not a multiple of sh_entsize {entsize}"
            ),
        }
    }
}

impl std::error::Error for ElfError {}

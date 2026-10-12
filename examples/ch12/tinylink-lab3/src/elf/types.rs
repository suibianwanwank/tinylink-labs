//! ELF 里各种编号的含义。
//!
//! 文件里存的是整数，这里把它们变成 enum。规范没有列出的值落进 `Other`，
//! 解析从不因为"不认识"而失败；只有在真要用到它的地方，才决定支持还是报错。
//! `Display` 输出的名字和 GNU readelf 一致，方便对拍。

use std::fmt;

/// `e_type`：文件的种类。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileType {
    None,
    Rel,
    Exec,
    Dyn,
    Core,
    Other(u16),
}

impl From<u16> for FileType {
    fn from(v: u16) -> Self {
        match v {
            0 => FileType::None,
            1 => FileType::Rel,
            2 => FileType::Exec,
            3 => FileType::Dyn,
            4 => FileType::Core,
            other => FileType::Other(other),
        }
    }
}

impl From<FileType> for u16 {
    fn from(t: FileType) -> u16 {
        match t {
            FileType::None => 0,
            FileType::Rel => 1,
            FileType::Exec => 2,
            FileType::Dyn => 3,
            FileType::Core => 4,
            FileType::Other(v) => v,
        }
    }
}

impl fmt::Display for FileType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FileType::None => write!(f, "NONE (None)"),
            FileType::Rel => write!(f, "REL (Relocatable file)"),
            FileType::Exec => write!(f, "EXEC (Executable file)"),
            FileType::Dyn => write!(f, "DYN (Shared object file)"),
            FileType::Core => write!(f, "CORE (Core file)"),
            FileType::Other(v) => write!(f, "<unknown>: {v:x}"),
        }
    }
}

/// `e_machine`：目标 CPU。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Machine {
    X86_64,
    AArch64,
    RiscV,
    Other(u16),
}

impl From<u16> for Machine {
    fn from(v: u16) -> Self {
        match v {
            62 => Machine::X86_64,
            183 => Machine::AArch64,
            243 => Machine::RiscV,
            other => Machine::Other(other),
        }
    }
}

impl From<Machine> for u16 {
    fn from(m: Machine) -> u16 {
        match m {
            Machine::X86_64 => 62,
            Machine::AArch64 => 183,
            Machine::RiscV => 243,
            Machine::Other(v) => v,
        }
    }
}

impl fmt::Display for Machine {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Machine::X86_64 => write!(f, "Advanced Micro Devices X86-64"),
            Machine::AArch64 => write!(f, "AArch64"),
            Machine::RiscV => write!(f, "RISC-V"),
            Machine::Other(v) => write!(f, "<unknown>: 0x{v:x}"),
        }
    }
}

/// `sh_type`：节的种类。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SectionType {
    Null,
    Progbits,
    Symtab,
    Strtab,
    Rela,
    Hash,
    Dynamic,
    Note,
    Nobits,
    Rel,
    Dynsym,
    InitArray,
    FiniArray,
    PreinitArray,
    Group,
    SymtabShndx,
    LlvmAddrsig,
    Other(u32),
}

impl From<u32> for SectionType {
    fn from(v: u32) -> Self {
        match v {
            0 => SectionType::Null,
            1 => SectionType::Progbits,
            2 => SectionType::Symtab,
            3 => SectionType::Strtab,
            4 => SectionType::Rela,
            5 => SectionType::Hash,
            6 => SectionType::Dynamic,
            7 => SectionType::Note,
            8 => SectionType::Nobits,
            9 => SectionType::Rel,
            11 => SectionType::Dynsym,
            14 => SectionType::InitArray,
            15 => SectionType::FiniArray,
            16 => SectionType::PreinitArray,
            17 => SectionType::Group,
            18 => SectionType::SymtabShndx,
            0x6fff_4c03 => SectionType::LlvmAddrsig,
            other => SectionType::Other(other),
        }
    }
}

impl SectionType {
    /// GNU readelf 显示的类型名。处理器相关的编号要结合 `e_machine` 才知道含义。
    pub fn name(self, machine: Machine) -> String {
        let s = match self {
            SectionType::Null => "NULL",
            SectionType::Progbits => "PROGBITS",
            SectionType::Symtab => "SYMTAB",
            SectionType::Strtab => "STRTAB",
            SectionType::Rela => "RELA",
            SectionType::Hash => "HASH",
            SectionType::Dynamic => "DYNAMIC",
            SectionType::Note => "NOTE",
            SectionType::Nobits => "NOBITS",
            SectionType::Rel => "REL",
            SectionType::Dynsym => "DYNSYM",
            SectionType::InitArray => "INIT_ARRAY",
            SectionType::FiniArray => "FINI_ARRAY",
            SectionType::PreinitArray => "PREINIT_ARRAY",
            SectionType::Group => "GROUP",
            SectionType::SymtabShndx => "SYMTAB SECTION INDICES",
            SectionType::LlvmAddrsig => "LLVM_ADDRSIG",
            SectionType::Other(0x7000_0001) if machine == Machine::X86_64 => "X86_64_UNWIND",
            SectionType::Other(v @ 0x7000_0000..=0x7fff_ffff) => {
                return format!("LOPROC+{:#x}", v - 0x7000_0000);
            }
            SectionType::Other(v @ 0x6000_0000..=0x6fff_ffff) => {
                return format!("LOOS+{:#x}", v - 0x6000_0000);
            }
            SectionType::Other(v) => return format!("{v:08x}: <unknown>"),
        };
        s.to_owned()
    }
}

/// 不依赖 `e_machine` 的类型名，用在错误信息里。
impl fmt::Display for SectionType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SectionType::Other(v) => write!(f, "{v:#x}"),
            other => f.write_str(&other.name(Machine::Other(0))),
        }
    }
}

/// `sh_flags`：一组位标志。用一个包着 u64 的新类型，而不是把每一位拆成 bool。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SectionFlags(pub u64);

impl SectionFlags {
    pub const WRITE: u64 = 0x1;
    pub const ALLOC: u64 = 0x2;
    pub const EXECINSTR: u64 = 0x4;

    pub fn contains(self, bit: u64) -> bool {
        self.0 & bit != 0
    }

    /// GNU readelf `-S` 的 Flg 列：按位从低到高，每个认识的位一个字母。
    pub fn letters(self, machine: Machine) -> String {
        let mut known: Vec<(u64, char)> = vec![
            (0x1, 'W'),
            (0x2, 'A'),
            (0x4, 'X'),
            (0x10, 'M'),
            (0x20, 'S'),
            (0x40, 'I'),
            (0x80, 'L'),
            (0x100, 'O'),
            (0x200, 'G'),
            (0x400, 'T'),
            (0x800, 'C'),
        ];
        if machine == Machine::X86_64 {
            known.push((0x1000_0000, 'l'));
        }
        known.push((0x8000_0000, 'E'));
        known
            .into_iter()
            .filter(|&(bit, _)| self.contains(bit))
            .map(|(_, c)| c)
            .collect()
    }
}

/// 符号的绑定（`st_info` 的高 4 位）：这个名字在多大范围内有效。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SymbolBinding {
    Local,
    Global,
    Weak,
    Other(u8),
}

impl From<u8> for SymbolBinding {
    fn from(v: u8) -> Self {
        match v {
            0 => SymbolBinding::Local,
            1 => SymbolBinding::Global,
            2 => SymbolBinding::Weak,
            other => SymbolBinding::Other(other),
        }
    }
}

impl fmt::Display for SymbolBinding {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SymbolBinding::Local => f.pad("LOCAL"),
            SymbolBinding::Global => f.pad("GLOBAL"),
            SymbolBinding::Weak => f.pad("WEAK"),
            SymbolBinding::Other(10) => f.pad("UNIQUE"),
            SymbolBinding::Other(v) => f.pad(&format!("<unknown>: {v}")),
        }
    }
}

/// 符号的类型（`st_info` 的低 4 位）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SymbolType {
    NoType,
    Object,
    Func,
    Section,
    File,
    Common,
    Tls,
    Other(u8),
}

impl From<u8> for SymbolType {
    fn from(v: u8) -> Self {
        match v {
            0 => SymbolType::NoType,
            1 => SymbolType::Object,
            2 => SymbolType::Func,
            3 => SymbolType::Section,
            4 => SymbolType::File,
            5 => SymbolType::Common,
            6 => SymbolType::Tls,
            other => SymbolType::Other(other),
        }
    }
}

impl fmt::Display for SymbolType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // `f.pad` 会照顾调用方写的宽度和对齐，比如 `{:<7}`。
        match self {
            SymbolType::NoType => f.pad("NOTYPE"),
            SymbolType::Object => f.pad("OBJECT"),
            SymbolType::Func => f.pad("FUNC"),
            SymbolType::Section => f.pad("SECTION"),
            SymbolType::File => f.pad("FILE"),
            SymbolType::Common => f.pad("COMMON"),
            SymbolType::Tls => f.pad("TLS"),
            SymbolType::Other(10) => f.pad("IFUNC"),
            SymbolType::Other(v) => f.pad(&format!("<unknown>: {v}")),
        }
    }
}

/// 符号的可见性（`st_other` 的低 2 位）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Visibility {
    Default,
    Internal,
    Hidden,
    Protected,
}

impl From<u8> for Visibility {
    fn from(v: u8) -> Self {
        match v & 0x3 {
            0 => Visibility::Default,
            1 => Visibility::Internal,
            2 => Visibility::Hidden,
            _ => Visibility::Protected,
        }
    }
}

impl fmt::Display for Visibility {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.pad(match self {
            Visibility::Default => "DEFAULT",
            Visibility::Internal => "INTERNAL",
            Visibility::Hidden => "HIDDEN",
            Visibility::Protected => "PROTECTED",
        })
    }
}

/// `st_shndx`：符号定义在哪个节。几个保留值有特殊含义。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SectionIndex {
    /// 0：未定义，要由别的文件提供。
    Undef,
    /// 0xfff1：绝对值，不随任何节移动。
    Abs,
    /// 0xfff2：common 符号，第 3 章讲。
    Common,
    /// 普通的节号。由 `ElfFile` 构造的值都检查过：这个节号一定存在。
    Index(usize),
}

/// `p_type`：段的种类。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SegmentType {
    Null,
    Load,
    Dynamic,
    Interp,
    Note,
    Phdr,
    Tls,
    GnuEhFrame,
    GnuStack,
    GnuRelro,
    GnuProperty,
    Other(u32),
}

impl From<u32> for SegmentType {
    fn from(v: u32) -> Self {
        match v {
            0 => SegmentType::Null,
            1 => SegmentType::Load,
            2 => SegmentType::Dynamic,
            3 => SegmentType::Interp,
            4 => SegmentType::Note,
            6 => SegmentType::Phdr,
            7 => SegmentType::Tls,
            0x6474_e550 => SegmentType::GnuEhFrame,
            0x6474_e551 => SegmentType::GnuStack,
            0x6474_e552 => SegmentType::GnuRelro,
            0x6474_e553 => SegmentType::GnuProperty,
            other => SegmentType::Other(other),
        }
    }
}

impl From<SegmentType> for u32 {
    fn from(t: SegmentType) -> u32 {
        match t {
            SegmentType::Null => 0,
            SegmentType::Load => 1,
            SegmentType::Dynamic => 2,
            SegmentType::Interp => 3,
            SegmentType::Note => 4,
            SegmentType::Phdr => 6,
            SegmentType::Tls => 7,
            SegmentType::GnuEhFrame => 0x6474_e550,
            SegmentType::GnuStack => 0x6474_e551,
            SegmentType::GnuRelro => 0x6474_e552,
            SegmentType::GnuProperty => 0x6474_e553,
            SegmentType::Other(v) => v,
        }
    }
}

impl fmt::Display for SegmentType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            SegmentType::Null => "NULL",
            SegmentType::Load => "LOAD",
            SegmentType::Dynamic => "DYNAMIC",
            SegmentType::Interp => "INTERP",
            SegmentType::Note => "NOTE",
            SegmentType::Phdr => "PHDR",
            SegmentType::Tls => "TLS",
            SegmentType::GnuEhFrame => "GNU_EH_FRAME",
            SegmentType::GnuStack => "GNU_STACK",
            SegmentType::GnuRelro => "GNU_RELRO",
            SegmentType::GnuProperty => "GNU_PROPERTY",
            SegmentType::Other(v) => return f.pad(&format!("{v:#x}")),
        };
        f.pad(name)
    }
}

/// `p_flags`：段的权限。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SegmentFlags(pub u32);

impl SegmentFlags {
    pub const X: u32 = 0x1;
    pub const W: u32 = 0x2;
    pub const R: u32 = 0x4;
}

impl fmt::Display for SegmentFlags {
    /// readelf 的 Flg 列：固定三个字符，依次是 R、W、E，没有的位写空格。
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let bit = |b: u32, c: char| if self.0 & b != 0 { c } else { ' ' };
        write!(
            f,
            "{}{}{}",
            bit(Self::R, 'R'),
            bit(Self::W, 'W'),
            bit(Self::X, 'E')
        )
    }
}

/// x86-64 重定位类型的名字（x86-64 psABI 第 4.4 节）。不认识的返回 `None`。
pub fn x86_64_reloc_name(kind: u32) -> Option<&'static str> {
    Some(match kind {
        0 => "R_X86_64_NONE",
        1 => "R_X86_64_64",
        2 => "R_X86_64_PC32",
        3 => "R_X86_64_GOT32",
        4 => "R_X86_64_PLT32",
        5 => "R_X86_64_COPY",
        6 => "R_X86_64_GLOB_DAT",
        7 => "R_X86_64_JUMP_SLOT",
        8 => "R_X86_64_RELATIVE",
        9 => "R_X86_64_GOTPCREL",
        10 => "R_X86_64_32",
        11 => "R_X86_64_32S",
        12 => "R_X86_64_16",
        13 => "R_X86_64_PC16",
        14 => "R_X86_64_8",
        15 => "R_X86_64_PC8",
        16 => "R_X86_64_DTPMOD64",
        17 => "R_X86_64_DTPOFF64",
        18 => "R_X86_64_TPOFF64",
        19 => "R_X86_64_TLSGD",
        20 => "R_X86_64_TLSLD",
        21 => "R_X86_64_DTPOFF32",
        22 => "R_X86_64_GOTTPOFF",
        23 => "R_X86_64_TPOFF32",
        24 => "R_X86_64_PC64",
        25 => "R_X86_64_GOTOFF64",
        26 => "R_X86_64_GOTPC32",
        32 => "R_X86_64_SIZE32",
        33 => "R_X86_64_SIZE64",
        41 => "R_X86_64_GOTPCRELX",
        42 => "R_X86_64_REX_GOTPCRELX",
        _ => return None,
    })
}

//! 参加链接的一个输入文件：解析好的 ELF、它的符号和重定位。

use std::error::Error;
use std::fmt;

use crate::elf::types::FileType;
use crate::elf::{ElfError, ElfFile, Rela, Symbol};
use crate::input::InputName;

/// 读入输入时的错误。出错的文件不参加解析，其余文件照常处理。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoadError {
    Elf(ElfError),
    /// 只接受可重定位文件（`ET_REL`）。
    NotRelocatable(FileType),
}

impl From<ElfError> for LoadError {
    fn from(e: ElfError) -> Self {
        LoadError::Elf(e)
    }
}

impl fmt::Display for LoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LoadError::Elf(e) => write!(f, "{e}"),
            LoadError::NotRelocatable(kind) => {
                write!(f, "input is {kind}, not a relocatable object file")
            }
        }
    }
}

// 不实现 source：`Display` 已经写出了里面的 ElfError，和 LinkError、InputError 的做法一致。
impl Error for LoadError {}

/// 一条重定位和它要修改的节。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Relocation {
    /// 被修改的节的编号，来自重定位节的 `sh_info`。
    pub section: usize,
    pub rela: Rela,
}

/// 一个输入文件。字段在 `parse` 里一次解析完，后面的步骤只读不写。
#[derive(Debug, Clone)]
pub struct InputFile<'a> {
    pub name: InputName,
    pub elf: ElfFile<'a>,
    /// `.symtab` 里的全部符号，下标就是符号编号，0 号是空项。没有符号表时为空。
    pub symbols: Vec<Symbol<'a>>,
    pub relocations: Vec<Relocation>,
}

impl<'a> InputFile<'a> {
    pub fn parse(name: InputName, data: &'a [u8]) -> Result<Self, LoadError> {
        let elf = ElfFile::parse(data)?;
        if elf.header().kind != FileType::Rel {
            return Err(LoadError::NotRelocatable(elf.header().kind));
        }
        let symbols = elf.symbol_table()?.map(|t| t.symbols).unwrap_or_default();
        let mut relocations = Vec::new();
        for section in elf.rela_sections()? {
            // sh_info 是被修改的节。u32 放得进 64 位机器上的 usize。
            let target = section.header.info as usize;
            for rela in elf.relocations(section)? {
                relocations.push(Relocation {
                    section: target,
                    rela,
                });
            }
        }
        Ok(InputFile {
            name,
            elf,
            symbols,
            relocations,
        })
    }

    /// 节的名字，报错时用。编号不存在时显示编号本身。
    pub fn section_name(&self, index: usize) -> String {
        match self.elf.section(index) {
            Some(s) => s.name.to_string(),
            None => format!("<section {index}>"),
        }
    }
}

/// 某个输入文件某个节里的一个位置，按 lld 的写法显示成 `a.o:(.text+0x1)`。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Location {
    /// 输入文件的下标，指向 `resolve` 收到的那个切片。
    pub file: usize,
    pub section: usize,
    pub offset: u64,
}

impl Location {
    pub fn display<'f, 'a>(&self, inputs: &'f [InputFile<'a>]) -> LocationView<'f, 'a> {
        LocationView {
            location: *self,
            inputs,
        }
    }
}

/// `Location` 显示时需要输入文件的名字和节名，这些都在 `inputs` 里。
pub struct LocationView<'f, 'a> {
    location: Location,
    inputs: &'f [InputFile<'a>],
}

impl fmt::Display for LocationView<'_, '_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Location {
            file,
            section,
            offset,
        } = self.location;
        match self.inputs.get(file) {
            Some(input) => write!(
                f,
                "{}:({}+{offset:#x})",
                input.name,
                input.section_name(section)
            ),
            None => write!(f, "<input {file}>:(<section {section}>+{offset:#x})"),
        }
    }
}

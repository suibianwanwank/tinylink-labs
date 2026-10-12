//! 把一个 ELF64 小端文件解析成结构，以及把 ELF 头、程序头写回字节。
//!
//! `ElfFile<'a>` 借用文件的字节：节名、符号名是指向字符串表的 `Name<'a>`，
//! 节的内容是指向文件的 `&'a [u8]`，解析过程中不复制任何数据。
//! 节头表、程序头表和每个节的内容范围在 `parse` 里一次检查完，之后取 `data` 不会再越界。

use std::collections::BTreeMap;
use std::collections::btree_map::Entry;

use super::error::{ElfError, Place, Result, StringProblem};
use super::name::Name;
use super::read::{Bytes, Cursor};
use super::types::{
    FileType, Machine, SectionFlags, SectionIndex, SectionType, SegmentFlags, SegmentType,
    SymbolBinding, SymbolType, Visibility,
};

pub const EHDR_SIZE: u64 = 64;
pub const SHDR_SIZE: u64 = 64;
pub const PHDR_SIZE: u64 = 56;
pub const SYM_SIZE: usize = 24;
pub const RELA_SIZE: usize = 24;

const SHN_UNDEF: u16 = 0;
const SHN_LORESERVE: u16 = 0xff00;
const SHN_ABS: u16 = 0xfff1;
const SHN_COMMON: u16 = 0xfff2;
const SHN_XINDEX: u16 = 0xffff;

/// 往 `Vec<u8>` 里追加小端整数。
trait PutLe {
    fn put_u16(&mut self, v: u16);
    fn put_u32(&mut self, v: u32);
    fn put_u64(&mut self, v: u64);
}

impl PutLe for Vec<u8> {
    fn put_u16(&mut self, v: u16) {
        self.extend_from_slice(&v.to_le_bytes());
    }
    fn put_u32(&mut self, v: u32) {
        self.extend_from_slice(&v.to_le_bytes());
    }
    fn put_u64(&mut self, v: u64) {
        self.extend_from_slice(&v.to_le_bytes());
    }
}

/// ELF 头：文件开头的 64 个字节（第 2 章"ELF 头"一节逐字段拆过）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Header {
    pub ident: [u8; 16],
    pub kind: FileType,
    pub machine: Machine,
    pub version: u32,
    pub entry: u64,
    pub phoff: u64,
    pub shoff: u64,
    pub flags: u32,
    pub ehsize: u16,
    pub phentsize: u16,
    pub phnum: u16,
    pub shentsize: u16,
    pub shnum: u16,
    pub shstrndx: u16,
}

impl Header {
    fn parse(bytes: Bytes<'_>) -> Result<Header> {
        // 先看魔数：一个只有几个字节的文本文件，应该报"不是 ELF"，不该报"ELF 头不完整"。
        let magic = bytes.prefix(4);
        if magic != b"\x7fELF" {
            return Err(ElfError::BadMagic {
                found: magic.to_vec(),
            });
        }
        let mut c = bytes.cursor(Place::FileHeader, 0, EHDR_SIZE)?;
        let ident: [u8; 16] = c.array()?;
        // 数组上的切片模式：`_` 跳过魔数（上面查过了），按位置取出三个字节，`..` 吃掉其余的。
        let [_, _, _, _, class, data, version, ..] = ident;
        let unsupported = |field, value: u8| ElfError::Unsupported {
            place: Place::FileHeader,
            field,
            value: value.into(),
        };
        if class != 2 {
            return Err(unsupported("EI_CLASS (only 64-bit is supported)", class));
        }
        if data != 1 {
            return Err(unsupported(
                "EI_DATA (only little-endian is supported)",
                data,
            ));
        }
        if version != 1 {
            return Err(unsupported("EI_VERSION", version));
        }
        let header = Header {
            ident,
            kind: c.u16()?.into(),
            machine: c.u16()?.into(),
            version: c.u32()?,
            entry: c.u64()?,
            phoff: c.u64()?,
            shoff: c.u64()?,
            flags: c.u32()?,
            ehsize: c.u16()?,
            phentsize: c.u16()?,
            phnum: c.u16()?,
            shentsize: c.u16()?,
            shnum: c.u16()?,
            shstrndx: c.u16()?,
        };
        if header.shnum > 0 && u64::from(header.shentsize) != SHDR_SIZE {
            return Err(ElfError::Unsupported {
                place: Place::FileHeader,
                field: "e_shentsize",
                value: header.shentsize.into(),
            });
        }
        if header.phnum > 0 && u64::from(header.phentsize) != PHDR_SIZE {
            return Err(ElfError::Unsupported {
                place: Place::FileHeader,
                field: "e_phentsize",
                value: header.phentsize.into(),
            });
        }
        // 节数达到 0xff00 时，e_shnum 写 0，真正的节数放在 0 号节头的 sh_size 里；
        // 字符串表的下标放不下时，e_shstrndx 写 0xffff，真正的值放在 0 号节头的 sh_link 里。
        if (header.shnum == 0 && header.shoff != 0) || header.shstrndx == SHN_XINDEX {
            return Err(ElfError::ExtendedNumbering {
                shnum: header.shnum,
                shoff: header.shoff,
                shstrndx: header.shstrndx,
            });
        }
        Ok(header)
    }

    /// 写出 64 字节的 ELF 头。
    pub fn write(&self, out: &mut Vec<u8>) {
        out.extend_from_slice(&self.ident);
        out.put_u16(self.kind.into());
        out.put_u16(self.machine.into());
        out.put_u32(self.version);
        out.put_u64(self.entry);
        out.put_u64(self.phoff);
        out.put_u64(self.shoff);
        out.put_u32(self.flags);
        out.put_u16(self.ehsize);
        out.put_u16(self.phentsize);
        out.put_u16(self.phnum);
        out.put_u16(self.shentsize);
        out.put_u16(self.shnum);
        out.put_u16(self.shstrndx);
    }
}

/// 节头表里的一项，原样保存 64 个字节解出来的字段。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SectionHeader {
    pub name: u32,
    pub kind: SectionType,
    pub flags: SectionFlags,
    pub addr: u64,
    pub offset: u64,
    pub size: u64,
    pub link: u32,
    pub info: u32,
    pub addralign: u64,
    pub entsize: u64,
}

impl SectionHeader {
    fn parse(c: &mut Cursor<'_>) -> Result<SectionHeader> {
        Ok(SectionHeader {
            name: c.u32()?,
            kind: c.u32()?.into(),
            flags: SectionFlags(c.u64()?),
            addr: c.u64()?,
            offset: c.u64()?,
            size: c.u64()?,
            link: c.u32()?,
            info: c.u32()?,
            addralign: c.u64()?,
            entsize: c.u64()?,
        })
    }
}

/// 程序头表里的一项（`Elf64_Phdr`，56 字节）：一个段。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProgramHeader {
    pub kind: SegmentType,
    pub flags: SegmentFlags,
    pub offset: u64,
    pub vaddr: u64,
    pub paddr: u64,
    pub filesz: u64,
    pub memsz: u64,
    pub align: u64,
}

impl ProgramHeader {
    fn parse(c: &mut Cursor<'_>) -> Result<ProgramHeader> {
        Ok(ProgramHeader {
            kind: c.u32()?.into(),
            flags: SegmentFlags(c.u32()?),
            offset: c.u64()?,
            vaddr: c.u64()?,
            paddr: c.u64()?,
            filesz: c.u64()?,
            memsz: c.u64()?,
            align: c.u64()?,
        })
    }

    /// 写出 56 字节的程序头，字段顺序和 `parse` 读的一样。
    pub fn write(&self, out: &mut Vec<u8>) {
        out.put_u32(self.kind.into());
        out.put_u32(self.flags.0);
        out.put_u64(self.offset);
        out.put_u64(self.vaddr);
        out.put_u64(self.paddr);
        out.put_u64(self.filesz);
        out.put_u64(self.memsz);
        out.put_u64(self.align);
    }
}

/// 一个节：编号、名字、节头和内容。名字和内容都借用自文件。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Section<'a> {
    pub index: usize,
    /// 节名。文件没有节名字符串表（`e_shstrndx` 为 0）时是空的。
    pub name: Name<'a>,
    pub header: SectionHeader,
    /// 节在文件里的字节。`NOBITS` 节（比如 `.bss`）不占文件空间，这里是空切片。
    pub data: &'a [u8],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Symbol<'a> {
    pub name: Name<'a>,
    pub value: u64,
    pub size: u64,
    pub kind: SymbolType,
    pub binding: SymbolBinding,
    pub visibility: Visibility,
    /// 定义所在的节。`SectionIndex::Index(i)` 里的 `i` 在解析时检查过，
    /// 产生这个符号的 `ElfFile` 一定有第 `i` 个节。
    pub section: SectionIndex,
}

/// 一张符号表：它所在的节，加上解出来的符号。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SymbolTable<'a> {
    pub section: usize,
    pub symbols: Vec<Symbol<'a>>,
}

/// `.rela.*` 里的一条重定位（第 2 章"那些零字节：重定位表"一节）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rela {
    pub offset: u64,
    /// `r_info` 的高 32 位：符号表里的编号。
    pub symbol: u32,
    /// `r_info` 的低 32 位：重定位类型。
    pub kind: u32,
    pub addend: i64,
}

impl Rela {
    pub fn info(&self) -> u64 {
        (u64::from(self.symbol) << 32) | u64::from(self.kind)
    }
}

/// 解析好的 ELF 文件。字段都是私有的，通过方法访问：构造之后它们满足 `parse` 检查过的不变量。
#[derive(Debug, Clone)]
pub struct ElfFile<'a> {
    header: Header,
    sections: Vec<Section<'a>>,
    program_headers: Vec<ProgramHeader>,
    has_section_names: bool,
}

/// 在字符串表里取以 NUL 结尾的字符串。
fn string_at<'a>(
    table: &'a [u8],
    offset: u32,
    place: Place,
    field: &'static str,
) -> Result<Name<'a>> {
    let bad = |problem| ElfError::BadString {
        place,
        field,
        offset: offset.into(),
        problem,
    };
    let tail = usize::try_from(offset)
        .ok()
        .and_then(|start| table.get(start..))
        .ok_or_else(|| {
            bad(StringProblem::OutsideTable {
                table_size: table.len() as u64,
            })
        })?;
    let end = tail
        .iter()
        .position(|&b| b == 0)
        .ok_or_else(|| bad(StringProblem::Unterminated))?;
    Ok(Name::new(&tail[..end]))
}

/// 检查一张表的表项大小，返回表项个数。
fn entry_count(section: &Section<'_>, expected: usize) -> Result<usize> {
    let expected_u64 = expected as u64;
    if section.header.entsize != expected_u64 {
        return Err(ElfError::BadEntrySize {
            section: section.index,
            entsize: section.header.entsize,
            expected: expected_u64,
        });
    }
    // data 的长度就是 sh_size（parse 里检查过范围），用它判断能否整除。
    if !section.data.len().is_multiple_of(expected) {
        return Err(ElfError::NotMultipleOfEntry {
            section: section.index,
            size: section.header.size,
            entsize: section.header.entsize,
        });
    }
    Ok(section.data.len() / expected)
}

impl<'a> ElfFile<'a> {
    pub fn parse(data: &'a [u8]) -> Result<ElfFile<'a>> {
        let bytes = Bytes::new(data);
        let header = Header::parse(bytes)?;

        // 先把整张节头表的范围检查一遍，再逐项解析。节数是 u16，乘以 64 不会溢出 u64。
        let table_size = u64::from(header.shnum) * SHDR_SIZE;
        let table = bytes.range(Place::SectionHeaderTable, header.shoff, table_size)?;
        let headers = table
            .as_chunks::<{ SHDR_SIZE as usize }>()
            .0
            .iter()
            .enumerate()
            .map(|(i, entry)| {
                let offset = header.shoff + i as u64 * SHDR_SIZE;
                SectionHeader::parse(&mut Cursor::new(entry, offset, Place::SectionHeader(i)))
            })
            .collect::<Result<Vec<_>>>()?;

        // 程序头表同理。可重定位文件通常没有程序头表，phnum 为 0。
        let table_size = u64::from(header.phnum) * PHDR_SIZE;
        let table = bytes.range(Place::ProgramHeaderTable, header.phoff, table_size)?;
        let program_headers = table
            .as_chunks::<{ PHDR_SIZE as usize }>()
            .0
            .iter()
            .enumerate()
            .map(|(i, entry)| {
                let offset = header.phoff + i as u64 * PHDR_SIZE;
                ProgramHeader::parse(&mut Cursor::new(entry, offset, Place::ProgramHeader(i)))
            })
            .collect::<Result<Vec<_>>>()?;

        // 每个节的内容范围也在这里检查一次。
        let contents = headers
            .iter()
            .enumerate()
            .map(|(i, h)| match h.kind {
                SectionType::Nobits | SectionType::Null => Ok(&data[..0]),
                _ => bytes.range(Place::SectionData(i), h.offset, h.size),
            })
            .collect::<Result<Vec<_>>>()?;

        // e_shstrndx 为 0（SHN_UNDEF）表示文件没有节名字符串表，gABI 允许这样做。
        let names: Option<&'a [u8]> = match header.shstrndx {
            SHN_UNDEF => None,
            index => {
                let index = usize::from(index);
                let Some(section) = headers.get(index) else {
                    return Err(ElfError::BadIndex {
                        place: Place::FileHeader,
                        field: "e_shstrndx",
                        index: index as u64,
                        count: headers.len() as u64,
                    });
                };
                if section.kind != SectionType::Strtab {
                    return Err(ElfError::WrongType {
                        place: Place::FileHeader,
                        field: "e_shstrndx",
                        index,
                        expected: SectionType::Strtab,
                        found: section.kind,
                    });
                }
                // index 小于 headers.len()，contents 和 headers 一样长。
                Some(contents[index])
            }
        };

        let sections = headers
            .into_iter()
            .zip(contents)
            .enumerate()
            .map(|(index, (header, data))| {
                let name = match names {
                    Some(table) => {
                        string_at(table, header.name, Place::SectionHeader(index), "sh_name")?
                    }
                    None => Name::default(),
                };
                Ok(Section {
                    index,
                    name,
                    header,
                    data,
                })
            })
            .collect::<Result<Vec<_>>>()?;

        Ok(ElfFile {
            header,
            sections,
            program_headers,
            has_section_names: names.is_some(),
        })
    }

    pub fn header(&self) -> &Header {
        &self.header
    }

    pub fn sections(&self) -> &[Section<'a>] {
        &self.sections
    }

    pub fn section(&self, index: usize) -> Option<&Section<'a>> {
        self.sections.get(index)
    }

    pub fn program_headers(&self) -> &[ProgramHeader] {
        &self.program_headers
    }

    /// 文件有没有节名字符串表。没有时 readelf 把节名显示成 `<no-strings>`。
    pub fn has_section_names(&self) -> bool {
        self.has_section_names
    }

    /// `sh_link` 指向的节，并检查它的类型。
    fn linked(&self, section: &Section<'a>, expected: SectionType) -> Result<&Section<'a>> {
        let place = Place::SectionHeader(section.index);
        let link = section.header.link;
        let target = usize::try_from(link)
            .ok()
            .and_then(|i| self.sections.get(i))
            .ok_or(ElfError::BadIndex {
                place,
                field: "sh_link",
                index: link.into(),
                count: self.sections.len() as u64,
            })?;
        if target.header.kind != expected {
            return Err(ElfError::WrongType {
                place,
                field: "sh_link",
                index: target.index,
                expected,
                found: target.header.kind,
            });
        }
        Ok(target)
    }

    /// 第一个 `SHT_SYMTAB` 节里的符号表。gABI 规定一个文件最多有一个 `.symtab`。
    pub fn symbol_table(&self) -> Result<Option<SymbolTable<'a>>> {
        match self
            .sections
            .iter()
            .find(|s| s.header.kind == SectionType::Symtab)
        {
            Some(symtab) => self.symbols_in(symtab).map(Some),
            None => Ok(None),
        }
    }

    /// 解析一个符号表节。它的 `sh_link` 必须指向字符串表。
    pub fn symbols_in(&self, symtab: &Section<'a>) -> Result<SymbolTable<'a>> {
        if !matches!(
            symtab.header.kind,
            SectionType::Symtab | SectionType::Dynsym
        ) {
            return Err(ElfError::WrongType {
                place: Place::SectionHeader(symtab.index),
                field: "symbol table",
                index: symtab.index,
                expected: SectionType::Symtab,
                found: symtab.header.kind,
            });
        }
        entry_count(symtab, SYM_SIZE)?;
        let strtab = self.linked(symtab, SectionType::Strtab)?.data;

        let mut symbols = Vec::new();
        // as_chunks 把字节切成定长数组 `&[[u8; 24]]`；entry_count 检查过没有零头。
        let (entries, _) = symtab.data.as_chunks::<SYM_SIZE>();
        for (i, entry) in entries.iter().enumerate() {
            let place = Place::Symbol(i);
            let offset = symtab.header.offset + (i * SYM_SIZE) as u64;
            let mut c = Cursor::new(entry, offset, place);
            let name = c.u32()?;
            let info = c.u8()?;
            let other = c.u8()?;
            let shndx = c.u16()?;
            let value = c.u64()?;
            let size = c.u64()?;

            let section = match shndx {
                SHN_UNDEF => SectionIndex::Undef,
                SHN_ABS => SectionIndex::Abs,
                SHN_COMMON => SectionIndex::Common,
                n if n >= SHN_LORESERVE => {
                    return Err(ElfError::Unsupported {
                        place,
                        field: "st_shndx",
                        value: n.into(),
                    });
                }
                n if usize::from(n) < self.sections.len() => SectionIndex::Index(n.into()),
                n => {
                    return Err(ElfError::BadIndex {
                        place,
                        field: "st_shndx",
                        index: n.into(),
                        count: self.sections.len() as u64,
                    });
                }
            };
            symbols.push(Symbol {
                name: string_at(strtab, name, place, "st_name")?,
                value,
                size,
                kind: (info & 0xf).into(),
                binding: (info >> 4).into(),
                visibility: other.into(),
                section,
            });
        }
        Ok(SymbolTable {
            section: symtab.index,
            symbols,
        })
    }

    /// 所有 `SHT_RELA` 节。x86-64 和 AArch64 只用 `SHT_RELA`；遇到 `SHT_REL`
    /// 明确报不支持，不能悄悄跳过：跳过了，readelf -r 和链接器都会少看一批重定位。
    pub fn rela_sections(&self) -> Result<Vec<&Section<'a>>> {
        let mut out = Vec::new();
        for s in &self.sections {
            match s.header.kind {
                SectionType::Rela => out.push(s),
                SectionType::Rel => {
                    return Err(ElfError::Unsupported {
                        place: Place::SectionHeader(s.index),
                        field: "sh_type SHT_REL (only SHT_RELA is supported)",
                        value: 9,
                    });
                }
                _ => {}
            }
        }
        Ok(out)
    }

    /// 重定位节的 `sh_link` 指向的符号表节。指向 `.dynsym` 是合法的 ELF，
    /// 只是 tinylink 还不读动态符号表，报不支持，不报类型错误。
    fn linked_symtab(&self, rela: &Section<'a>) -> Result<&Section<'a>> {
        let link = rela.header.link;
        if let Some(target) = usize::try_from(link)
            .ok()
            .and_then(|i| self.sections.get(i))
            && target.header.kind == SectionType::Dynsym
        {
            return Err(ElfError::Unsupported {
                place: Place::SectionHeader(rela.index),
                field: "sh_link to SHT_DYNSYM",
                value: link.into(),
            });
        }
        self.linked(rela, SectionType::Symtab)
    }

    /// 一个重定位节通过 `sh_link` 指向的符号表。
    pub fn linked_symbols(&self, rela: &Section<'a>) -> Result<SymbolTable<'a>> {
        self.symbols_in(self.linked_symtab(rela)?)
    }

    /// 解析一个 `SHT_RELA` 节，并检查每条记录的符号编号确实在它链接的符号表里。
    pub fn relocations(&self, section: &Section<'a>) -> Result<Vec<Rela>> {
        entry_count(section, RELA_SIZE)?;
        let symtab = self.linked_symtab(section)?;
        let symbol_count = entry_count(symtab, SYM_SIZE)? as u64;

        section
            .data
            .as_chunks::<RELA_SIZE>()
            .0
            .iter()
            .enumerate()
            .map(|(index, entry)| {
                let place = Place::Relocation {
                    section: section.index,
                    index,
                };
                let offset = section.header.offset + (index * RELA_SIZE) as u64;
                let mut c = Cursor::new(entry, offset, place);
                let r_offset = c.u64()?;
                let info = c.u64()?;
                let addend = c.i64()?;
                let rela = Rela {
                    offset: r_offset,
                    // 右移 32 位之后一定放得进 u32；低 32 位用 `as` 截取，截掉高位正是本意。
                    symbol: (info >> 32) as u32,
                    kind: info as u32,
                    addend,
                };
                if u64::from(rela.symbol) >= symbol_count {
                    return Err(ElfError::BadIndex {
                        place,
                        field: "symbol index",
                        index: rela.symbol.into(),
                        count: symbol_count,
                    });
                }
                Ok(rela)
            })
            .collect()
    }

    /// 每个重定位节链接的符号表，按符号表所在的节号缓存。readelf -r 用它取符号名。
    /// 先只看 `sh_link` 拿到节号，缓存里没有时才解析，几个重定位节共用的符号表只解析一次。
    pub fn symbol_tables_for_relocations(&self) -> Result<BTreeMap<usize, SymbolTable<'a>>> {
        let mut tables = BTreeMap::new();
        for rela in self.rela_sections()? {
            let symtab = self.linked_symtab(rela)?;
            if let Entry::Vacant(slot) = tables.entry(symtab.index) {
                slot.insert(self.symbols_in(symtab)?);
            }
        }
        Ok(tables)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn header_round_trips() {
        // 每个字段都用互不相同的值：write 里任意两个字段写反、写错宽度，读回来就对不上。
        // ELF 头里有节头表，parse 才会去读 shentsize、shnum、shstrndx 指向的东西。
        let shoff = 0x80;
        let strtab_offset = shoff + 3 * SHDR_SIZE;
        let header = Header {
            ident: [0x7f, b'E', b'L', b'F', 2, 1, 1, 3, 9, 0, 0, 0, 0, 0, 0, 0],
            kind: FileType::Exec,
            machine: Machine::AArch64,
            version: 7,
            entry: 0x40_0078,
            phoff: 0x48,
            shoff,
            flags: 0x5,
            // parse 不检查 ehsize，这里故意用一个和其他字段不同的值。
            ehsize: 0x41,
            phentsize: 56,
            phnum: 1,
            shentsize: 64,
            shnum: 3,
            shstrndx: 2,
        };
        let phdr = ProgramHeader {
            kind: SegmentType::Load,
            flags: SegmentFlags(SegmentFlags::R | SegmentFlags::X),
            offset: 0x10,
            vaddr: 0x40_1000,
            paddr: 0x50_2000,
            filesz: 0x30,
            memsz: 0x60,
            align: 0x1_0000,
        };
        let mut bytes = Vec::new();
        header.write(&mut bytes);
        bytes.resize(0x48, 0);
        phdr.write(&mut bytes);
        bytes.resize(shoff as usize, 0);
        // 三个节头：0 号空项、一个空的 PROGBITS、节名字符串表。
        let names = b"\0.shstrtab\0";
        bytes.resize(bytes.len() + 64, 0);
        let section = |out: &mut Vec<u8>, kind: u32, name: u32, offset: u64, size: u64| {
            out.put_u32(name);
            out.put_u32(kind);
            out.put_u64(0); // sh_flags
            out.put_u64(0); // sh_addr
            out.put_u64(offset);
            out.put_u64(size);
            out.put_u32(0); // sh_link
            out.put_u32(0); // sh_info
            out.put_u64(1); // sh_addralign
            out.put_u64(0); // sh_entsize
        };
        section(&mut bytes, 1, 0, 0, 0);
        section(&mut bytes, 3, 1, strtab_offset, names.len() as u64);
        bytes.extend_from_slice(names);

        let file = ElfFile::parse(&bytes).expect("parse what we wrote");
        assert_eq!(file.header(), &header);
        assert_eq!(file.program_headers(), &[phdr]);
        assert!(file.section(2).is_some_and(|s| s.name == ".shstrtab"));
    }
}

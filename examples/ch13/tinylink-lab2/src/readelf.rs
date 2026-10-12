//! `tinylink readelf`：按 GNU readelf `-W` 的格式打印 ELF 头、节头表、符号表和重定位。
//!
//! `readelf -l` 也在这里，格式同样对齐 GNU readelf。
//!
//! 每个视图分两步：`new` 做所有可能失败的解析，返回 `Result`；`Display` 只负责排版，
//! 不会失败。格式和 GNU binutils 2.45 的 `readelf -W` 逐字一致，测试拿两者对拍。
//! 有意不支持的情形列在 `tests/elf_snapshots.rs` 开头。

use std::fmt;

use std::collections::BTreeMap;

use crate::elf::types::{
    Machine, SectionFlags, SectionIndex, SectionType, SymbolType, x86_64_reloc_name,
};
use crate::elf::{ElfFile, Name, ProgramHeader, Rela, Result, Section, Symbol, SymbolTable};

/// `readelf -h`
pub struct HeaderView<'f, 'a>(pub &'f ElfFile<'a>);

impl fmt::Display for HeaderView<'_, '_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let h = self.0.header();
        let line = |f: &mut fmt::Formatter<'_>, label: &str, value: &dyn fmt::Display| {
            writeln!(f, "  {label:<35}{value}")
        };
        writeln!(f, "ELF Header:")?;
        write!(f, "  Magic:  ")?;
        for b in h.ident {
            write!(f, " {b:02x}")?;
        }
        writeln!(f, " ")?;
        line(f, "Class:", &"ELF64")?;
        line(f, "Data:", &"2's complement, little endian")?;
        line(f, "Version:", &"1 (current)")?;
        let osabi = match h.ident[7] {
            0 => "UNIX - System V".to_owned(),
            3 => "UNIX - GNU".to_owned(),
            v => format!("<unknown: {v:x}>"),
        };
        line(f, "OS/ABI:", &osabi)?;
        line(f, "ABI Version:", &h.ident[8])?;
        line(f, "Type:", &h.kind)?;
        line(f, "Machine:", &h.machine)?;
        line(f, "Version:", &format_args!("{:#x}", h.version))?;
        line(f, "Entry point address:", &format_args!("{:#x}", h.entry))?;
        line(
            f,
            "Start of program headers:",
            &format_args!("{} (bytes into file)", h.phoff),
        )?;
        line(
            f,
            "Start of section headers:",
            &format_args!("{} (bytes into file)", h.shoff),
        )?;
        line(f, "Flags:", &format_args!("{:#x}", h.flags))?;
        line(
            f,
            "Size of this header:",
            &format_args!("{} (bytes)", h.ehsize),
        )?;
        line(
            f,
            "Size of program headers:",
            &format_args!("{} (bytes)", h.phentsize),
        )?;
        line(f, "Number of program headers:", &h.phnum)?;
        line(
            f,
            "Size of section headers:",
            &format_args!("{} (bytes)", h.shentsize),
        )?;
        line(f, "Number of section headers:", &h.shnum)?;
        line(f, "Section header string table index:", &h.shstrndx)
    }
}

/// `readelf -S`
pub struct SectionsView<'f, 'a>(pub &'f ElfFile<'a>);

impl fmt::Display for SectionsView<'_, '_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let file = self.0;
        let machine = file.header().machine;
        let n = file.sections().len();
        if n == 0 {
            return writeln!(f, "\nThere are no sections in this file.");
        }
        let (verb, noun) = if n == 1 {
            ("is", "header")
        } else {
            ("are", "headers")
        };
        writeln!(
            f,
            "There {verb} {n} section {noun}, starting at offset {:#x}:",
            file.header().shoff
        )?;
        writeln!(f)?;
        writeln!(f, "Section Headers:")?;
        writeln!(
            f,
            "  [Nr] Name              Type            Address          Off    Size   ES Flg Lk Inf Al"
        )?;
        for s in file.sections() {
            let h = &s.header;
            writeln!(
                f,
                "  [{:>2}] {:<17} {:<15} {:016x} {:06x} {:06x} {:02x} {:>3} {:>2} {:>3} {:>2}",
                s.index,
                section_name(file, s),
                h.kind.name(machine),
                h.addr,
                h.offset,
                h.size,
                h.entsize,
                h.flags.letters(machine),
                h.link,
                h.info,
                h.addralign
            )?;
        }
        writeln!(f, "Key to Flags:")?;
        writeln!(
            f,
            "  W (write), A (alloc), X (execute), M (merge), S (strings), I (info),"
        )?;
        writeln!(
            f,
            "  L (link order), O (extra OS processing required), G (group), T (TLS),"
        )?;
        writeln!(
            f,
            "  C (compressed), x (unknown), o (OS specific), E (exclude),"
        )?;
        if machine == Machine::X86_64 {
            writeln!(f, "  D (mbind), l (large), p (processor specific)")
        } else {
            writeln!(f, "  D (mbind), p (processor specific)")
        }
    }
}

/// GNU readelf 在文件没有节名字符串表时显示的名字。
const NO_STRINGS: Name<'static> = Name::new(b"<no-strings>");

/// 节在 readelf 里显示的名字。
fn section_name<'a>(file: &ElfFile<'a>, section: &Section<'a>) -> Name<'a> {
    if file.has_section_names() {
        section.name
    } else {
        NO_STRINGS
    }
}

/// 符号在 readelf 里显示的名字：节符号没有名字，显示它所在节的名字。
fn display_name<'a>(symbol: &Symbol<'a>, file: &ElfFile<'a>) -> Name<'a> {
    match (symbol.kind, symbol.section) {
        (SymbolType::Section, SectionIndex::Index(i)) if symbol.name.is_empty() => file
            .section(i)
            .map_or(symbol.name, |s| section_name(file, s)),
        _ => symbol.name,
    }
}

/// `readelf -s`
pub struct SymbolsView<'f, 'a> {
    file: &'f ElfFile<'a>,
    table: Option<SymbolTable<'a>>,
}

impl<'f, 'a> SymbolsView<'f, 'a> {
    pub fn new(file: &'f ElfFile<'a>) -> Result<Self> {
        Ok(SymbolsView {
            file,
            table: file.symbol_table()?,
        })
    }
}

impl fmt::Display for SymbolsView<'_, '_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Some(table) = &self.table else {
            // 和 GNU readelf 2.45 一样：没有节头表的文件只能指望动态符号表，tinylink 不读它。
            if self.file.sections().is_empty() {
                writeln!(f)?;
                writeln!(
                    f,
                    "Dynamic symbol information is not available for displaying symbols."
                )?;
            }
            return Ok(());
        };
        let file = self.file;
        let table_name = file
            .section(table.section)
            .map_or(NO_STRINGS, |s| section_name(file, s));
        writeln!(f)?;
        writeln!(
            f,
            "Symbol table '{table_name}' contains {} {}:",
            table.symbols.len(),
            if table.symbols.len() == 1 {
                "entry"
            } else {
                "entries"
            }
        )?;
        writeln!(
            f,
            "   Num:    Value          Size Type    Bind   Vis      Ndx Name"
        )?;
        for (i, sym) in table.symbols.iter().enumerate() {
            let ndx = match sym.section {
                SectionIndex::Undef => "UND".to_owned(),
                SectionIndex::Abs => "ABS".to_owned(),
                SectionIndex::Common => "COM".to_owned(),
                SectionIndex::Index(n) => n.to_string(),
            };
            writeln!(
                f,
                "{i:>6}: {:016x} {:>5} {:<7} {:<6} {:<7} {ndx:>4} {}",
                sym.value,
                sym.size,
                sym.kind,
                sym.binding,
                sym.visibility,
                display_name(sym, file)
            )?;
        }
        Ok(())
    }
}

/// 一个重定位节和解析出来的记录。
struct RelaTable<'f, 'a> {
    section: &'f Section<'a>,
    /// `sh_link`：这些记录里的符号编号指向哪一张符号表。
    symtab: usize,
    relas: Vec<Rela>,
}

/// `readelf -r`
pub struct RelocsView<'f, 'a> {
    file: &'f ElfFile<'a>,
    /// 按符号表所在的节号缓存：几个重定位节通常共用同一张 `.symtab`，
    /// `symbol_tables_for_relocations` 每张只解析一次。
    symtabs: BTreeMap<usize, SymbolTable<'a>>,
    tables: Vec<RelaTable<'f, 'a>>,
}

impl<'f, 'a> RelocsView<'f, 'a> {
    pub fn new(file: &'f ElfFile<'a>) -> Result<Self> {
        let symtabs = file.symbol_tables_for_relocations()?;
        let tables = file
            .rela_sections()?
            .into_iter()
            .map(|section| {
                Ok(RelaTable {
                    section,
                    // symbol_tables_for_relocations 已经检查过这个 sh_link。
                    symtab: section.header.link as usize,
                    relas: file.relocations(section)?,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        Ok(RelocsView {
            file,
            symtabs,
            tables,
        })
    }
}

impl fmt::Display for RelocsView<'_, '_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.tables.is_empty() {
            return writeln!(f, "\nThere are no relocations in this file.");
        }
        let file = self.file;
        let machine = file.header().machine;
        for table in &self.tables {
            let section = table.section;
            let relas = &table.relas;
            writeln!(f)?;
            // 没有节名字符串表时，GNU readelf 在这里打印 sh_name 的数值，不加引号。
            if file.has_section_names() {
                write!(f, "Relocation section '{}'", section.name)?;
            } else {
                write!(f, "Relocation section {}", section.header.name)?;
            }
            writeln!(
                f,
                " at offset {:#x} contains {} {}:",
                section.header.offset,
                relas.len(),
                if relas.len() == 1 { "entry" } else { "entries" }
            )?;
            writeln!(
                f,
                "    Offset             Info             Type               Symbol's Value  Symbol's Name + Addend"
            )?;
            // 不变量：new() 里 symtabs 收齐了每个重定位节的 sh_link 指向的符号表，
            // 这里一定取得到。取不到说明 new() 写错了。
            let symbols = &self
                .symtabs
                .get(&table.symtab)
                .expect("symbol table of every relocation section is cached")
                .symbols;
            for r in relas {
                let kind = match (machine, x86_64_reloc_name(r.kind)) {
                    (Machine::X86_64, Some(name)) => name.to_owned(),
                    _ => format!("unrecognized: {:x}", r.kind),
                };
                let (sign, magnitude) = if r.addend < 0 {
                    ('-', r.addend.unsigned_abs())
                } else {
                    ('+', r.addend.unsigned_abs())
                };
                // 0 号符号是空项，readelf 不显示符号列。
                let symbol = usize::try_from(r.symbol)
                    .ok()
                    .filter(|&i| i != 0)
                    .and_then(|i| symbols.get(i));
                match symbol {
                    Some(sym) => writeln!(
                        f,
                        "{:016x}  {:016x} {kind:<22} {:016x} {} {sign} {magnitude:x}",
                        r.offset,
                        r.info(),
                        sym.value,
                        display_name(sym, file)
                    )?,
                    None => writeln!(
                        f,
                        "{:016x}  {:016x} {kind:<22}                    {magnitude:x}",
                        r.offset,
                        r.info()
                    )?,
                }
            }
        }
        Ok(())
    }
}

/// `readelf -l`
pub struct SegmentsView<'f, 'a>(pub &'f ElfFile<'a>);

/// 一个节是否落在一个段里，`readelf -l` 的"Section to Segment mapping"用它。
///
/// 这是 binutils `ELF_SECTION_IN_SEGMENT` 的简化版：只看可分配的节，
/// 不处理 TLS 节和 `PT_DYNAMIC` 的特殊规则（tests/elf_snapshots.rs 开头列了这些差异）。
fn in_segment(section: &Section<'_>, segment: &ProgramHeader) -> bool {
    let h = &section.header;
    if !h.flags.contains(SectionFlags::ALLOC) || h.kind == SectionType::Null {
        return false;
    }
    let inside = |start: u64, size: u64, base: u64, len: u64| {
        // 大小为 0 的节只要起点严格落在段内就算；否则整个范围都要在段里。
        start >= base
            && match size {
                0 => start < base.saturating_add(len),
                _ => start - base < len && size <= len - (start - base),
            }
    };
    let in_memory = inside(h.addr, h.size, segment.vaddr, segment.memsz);
    if h.kind == SectionType::Nobits {
        return in_memory;
    }
    in_memory && inside(h.offset, h.size, segment.offset, segment.filesz)
}

impl fmt::Display for SegmentsView<'_, '_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let file = self.0;
        let h = file.header();
        let phdrs = file.program_headers();
        if phdrs.is_empty() {
            return writeln!(f, "\nThere are no program headers in this file.");
        }
        writeln!(f)?;
        writeln!(f, "Elf file type is {}", h.kind)?;
        writeln!(f, "Entry point {:#x}", h.entry)?;
        let (verb, noun) = if phdrs.len() == 1 {
            ("is", "header")
        } else {
            ("are", "headers")
        };
        writeln!(
            f,
            "There {verb} {} program {noun}, starting at offset {}",
            phdrs.len(),
            h.phoff
        )?;
        writeln!(f)?;
        writeln!(f, "Program Headers:")?;
        writeln!(
            f,
            "  Type           Offset   VirtAddr           PhysAddr           FileSiz  MemSiz   Flg Align"
        )?;
        for p in phdrs {
            let align = match p.align {
                0 => "0".to_owned(),
                a => format!("{a:#x}"),
            };
            writeln!(
                f,
                "  {:<14} {:#08x} {:#018x} {:#018x} {:#08x} {:#08x} {} {align}",
                p.kind, p.offset, p.vaddr, p.paddr, p.filesz, p.memsz, p.flags
            )?;
        }
        // 没有节头表的文件（比如 M2 的产物）没有这一段；没有节名字符串表时 GNU 也不打印它。
        if file.sections().is_empty() || !file.has_section_names() {
            return Ok(());
        }
        writeln!(f)?;
        writeln!(f, " Section to Segment mapping:")?;
        writeln!(f, "  Segment Sections...")?;
        for (i, p) in phdrs.iter().enumerate() {
            write!(f, "   {i:02}     ")?;
            for s in file.sections().iter().filter(|s| in_segment(s, p)) {
                write!(f, "{} ", section_name(file, s))?;
            }
            writeln!(f)?;
        }
        Ok(())
    }
}

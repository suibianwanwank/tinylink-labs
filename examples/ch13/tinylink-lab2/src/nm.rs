//! `tinylink nm`：按 GNU nm 的默认格式列出符号，每行"值 类型字母 名字"，按名字排序。

use std::fmt;

use crate::elf::types::{SectionFlags, SectionIndex, SectionType, SymbolBinding, SymbolType};
use crate::elf::{ElfFile, Name, Result, Symbol};

/// nm 的类型字母（第 2 章练习四的答案里列过规则）。大写是全局，小写是局部。
fn letter(symbol: &Symbol<'_>, file: &ElfFile<'_>) -> char {
    let weak = symbol.binding == SymbolBinding::Weak;
    let object = symbol.kind == SymbolType::Object;
    let c = match (weak, symbol.section) {
        (true, SectionIndex::Undef) => return if object { 'v' } else { 'w' },
        (false, SectionIndex::Undef) => return 'U',
        (true, _) => return if object { 'V' } else { 'W' },
        (false, SectionIndex::Common) => return 'C',
        (false, SectionIndex::Abs) => 'a',
        (false, SectionIndex::Index(i)) => {
            // ElfFile 构造的 Index 都检查过范围，取不到只可能是用法错误，按"其他"处理。
            let Some(section) = file.section(i) else {
                return '?';
            };
            let flags = section.header.flags;
            if section.header.kind == SectionType::Nobits {
                'b'
            } else if flags.contains(SectionFlags::EXECINSTR) {
                't'
            } else if flags.contains(SectionFlags::WRITE) {
                'd'
            } else if flags.contains(SectionFlags::ALLOC) {
                'r'
            } else {
                'n'
            }
        }
    };
    if symbol.binding == SymbolBinding::Local {
        c
    } else {
        c.to_ascii_uppercase()
    }
}

/// 一行 nm 输出。
struct Line<'a> {
    /// 第一列。未定义符号没有值，显示成空白；COMMON 符号显示大小（st_value 存的是对齐）。
    value: Option<u64>,
    letter: char,
    name: Name<'a>,
}

pub struct NmView<'a> {
    lines: Vec<Line<'a>>,
}

impl<'a> NmView<'a> {
    pub fn new(file: &ElfFile<'a>) -> Result<Self> {
        let Some(table) = file.symbol_table()? else {
            return Ok(NmView { lines: Vec::new() });
        };
        let mut lines: Vec<Line<'a>> = table
            .symbols
            .iter()
            .skip(1) // 0 号符号是规范要求的空项
            .filter(|s| !matches!(s.kind, SymbolType::File | SymbolType::Section))
            .map(|s| Line {
                value: match s.section {
                    SectionIndex::Undef => None,
                    SectionIndex::Common => Some(s.size),
                    _ => Some(s.value),
                },
                letter: letter(s, file),
                name: s.name,
            })
            .collect();
        // sort_by 是稳定排序：同名的符号保持符号表里的先后顺序。
        lines.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(NmView { lines })
    }

    pub fn is_empty(&self) -> bool {
        self.lines.is_empty()
    }
}

impl fmt::Display for NmView<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for line in &self.lines {
            match line.value {
                Some(v) => writeln!(f, "{v:016x} {} {}", line.letter, line.name)?,
                None => writeln!(f, "{:16} {} {}", "", line.letter, line.name)?,
            }
        }
        Ok(())
    }
}

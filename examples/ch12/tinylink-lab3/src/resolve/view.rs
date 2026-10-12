//! `tinylink resolve` 的输出：每个全局名字一段，先列出每个提到它的输入，最后一行是结论。
//!
//! ```text
//! counter
//!     w.o  .data  WEAK    OBJECT  DEFAULT  size 4
//!     a.o  .data  GLOBAL  OBJECT  DEFAULT  size 4
//!   => a.o:(.data+0x0), size 4
//! ```
//!
//! 结论那一行的写法是固定的，`tests/resolve_vs_linkers.rs` 拿它和 ld.lld、GNU ld 的
//! 链接映射文件（`-Map`）里的结果比较。

use std::fmt;

use super::Resolution;
use super::input::{InputFile, Location};
use super::table::{Definition, GlobalSymbol, SymbolTable, Synthetic};
use crate::elf::types::{SectionIndex, Visibility};

pub struct ResolveView<'r, 'a> {
    inputs: &'r [InputFile<'a>],
    table: &'r SymbolTable<'a>,
}

impl<'r, 'a> ResolveView<'r, 'a> {
    pub fn new(inputs: &'r [InputFile<'a>], resolution: &'r Resolution<'a>) -> Self {
        ResolveView {
            inputs,
            table: &resolution.table,
        }
    }

    /// 一个同名符号所在的节，按 readelf 的习惯写 `UND`、`ABS`、`COM`。
    fn section(&self, file: usize, section: SectionIndex) -> String {
        match section {
            SectionIndex::Undef => "UND".to_owned(),
            SectionIndex::Abs => "ABS".to_owned(),
            SectionIndex::Common => "COM".to_owned(),
            SectionIndex::Index(i) => self
                .inputs
                .get(file)
                .map_or_else(|| format!("<section {i}>"), |input| input.section_name(i)),
        }
    }

    fn file(&self, file: usize) -> String {
        self.inputs
            .get(file)
            .map_or_else(|| format!("<input {file}>"), |i| i.name.to_string())
    }

    fn symbol(&self, f: &mut fmt::Formatter<'_>, entry: &GlobalSymbol<'_>) -> fmt::Result {
        writeln!(f, "{}", entry.name)?;
        let rows: Vec<[String; 6]> = entry
            .occurrences
            .iter()
            .filter_map(|at| {
                let s = self.inputs.get(at.file)?.symbols.get(at.index)?;
                let extra = match s.section {
                    SectionIndex::Common => format!("size {} align {}", s.size, s.value),
                    SectionIndex::Abs => format!("value {:#x}", s.value),
                    SectionIndex::Undef => String::new(),
                    SectionIndex::Index(_) => format!("size {}", s.size),
                };
                Some([
                    self.file(at.file),
                    self.section(at.file, s.section),
                    s.binding.to_string(),
                    s.kind.to_string(),
                    s.visibility.to_string(),
                    extra,
                ])
            })
            .collect();
        // 每一列按这一段里最宽的值对齐。
        let mut widths = [0; 6];
        for row in &rows {
            for (w, cell) in widths.iter_mut().zip(row) {
                *w = (*w).max(cell.len());
            }
        }
        for row in &rows {
            let mut line = String::from("    ");
            for (cell, w) in row.iter().zip(widths) {
                line.push_str(&format!("{cell:<w$}  "));
            }
            // 最后一列可能是空的，去掉行尾的空白。
            writeln!(f, "{}", line.trim_end())?;
        }

        write!(f, "  => ")?;
        match entry.definition {
            Definition::Defined {
                at,
                section: SectionIndex::Index(section),
                value,
                size,
                ..
            } => {
                let here = Location {
                    file: at.file,
                    section,
                    offset: value,
                };
                write!(f, "{}, size {size}", here.display(self.inputs))?;
            }
            Definition::Defined { at, value, .. } => {
                write!(f, "{}: absolute {value:#x}", self.file(at.file))?;
            }
            Definition::Common { at, size, align } => write!(
                f,
                "COMMON from {}, size {size}, align {align}",
                self.file(at.file)
            )?,
            Definition::Synthetic(s) => write!(f, "defined by the linker ({})", synthetic(s))?,
            Definition::Undefined if !entry.strong_reference => {
                write!(f, "undefined weak, value 0")?;
            }
            Definition::Undefined => write!(f, "undefined")?,
        }
        if entry.visibility != Visibility::Default {
            write!(f, ", {}", entry.visibility)?;
        }
        writeln!(f)
    }
}

fn synthetic(s: Synthetic) -> &'static str {
    match s {
        Synthetic::ImageStart => "start of the image",
        Synthetic::TextEnd => "end of the executable segment",
        Synthetic::DataEnd => "end of initialized data",
        Synthetic::BssStart => "start of .bss",
        Synthetic::End => "end of the image",
    }
}

impl fmt::Display for ResolveView<'_, '_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (_, entry) in self.table.iter() {
            self.symbol(f, entry)?;
        }
        Ok(())
    }
}

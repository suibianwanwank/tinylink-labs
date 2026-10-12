//! 符号解析中发现的问题。
//!
//! `Diag` 里只存编号（输入文件的下标、全局表的 `SymbolId`），不存字符串：
//! 发现问题时手里的信息最全，可显示时要的文件名、节名都在别处，到打印时再查。
//!
//! 文字的写法照着 lld 21（`ELF/Symbols.cpp` 的 `reportDuplicate`、`ELF/Relocations.cpp` 的
//! `reportUndefinedSymbol`）：第一行说是什么问题，下面每行 `>>>` 给一个位置。位置分两行：
//! 源文件（lld 先查 DWARF 行号表，查不到才用 `STT_FILE` 符号；tinylink 只用后者），
//! 和 `文件:(符号)` 或 `文件:(节+偏移)`。没有源文件名时只有后一行。

use std::fmt;

use super::Resolution;
use super::input::{InputFile, Location};
use super::table::{Definition, Duplicate, SymRef, SymbolId};
use crate::elf::Name;
use crate::elf::types::{SectionIndex, SymbolType, Visibility};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Diag {
    /// 两个强定义。
    Duplicate(Duplicate),
    /// 有强引用、被可分配节里的重定位用到、却没有定义。
    Undefined {
        symbol: SymbolId,
        references: Vec<Location>,
    },
    /// 输入的 `e_machine` 和第一个输入不同。
    Incompatible { file: usize, first: usize },
    /// `STB_GNU_UNIQUE` 之类 tinylink 还不认识的绑定。
    UnsupportedBinding { at: SymRef, binding: u8 },
}

/// lld 对同一个未定义符号最多列出 3 处引用，其余的只报个数。
const MAX_REFERENCES: usize = 3;

impl Diag {
    pub fn display<'r, 'a>(
        &'r self,
        inputs: &'r [InputFile<'a>],
        resolution: &'r Resolution<'a>,
    ) -> DiagView<'r, 'a> {
        DiagView {
            diag: self,
            inputs,
            resolution,
        }
    }
}

/// 打印一个 `Diag` 需要的全部上下文。
pub struct DiagView<'r, 'a> {
    diag: &'r Diag,
    inputs: &'r [InputFile<'a>],
    resolution: &'r Resolution<'a>,
}

impl<'a> DiagView<'_, 'a> {
    fn file(&self, file: usize) -> String {
        self.inputs
            .get(file)
            .map_or_else(|| format!("<input {file}>"), |i| i.name.to_string())
    }

    /// 文件里的第一个 `STT_FILE` 符号的名字，也就是编译它的源文件。汇编出来的目标文件没有。
    fn source(&self, file: usize) -> Option<Name<'a>> {
        self.inputs
            .get(file)?
            .symbols
            .iter()
            .find(|s| s.kind == SymbolType::File)
            .map(|s| s.name)
    }

    /// 文件 `file` 里覆盖 `section` 偏移 `offset` 的那个符号（lld 的 `getEnclosingSymbol`）。
    /// 局部符号按它自己的定义算；全局符号只有最后的定义就在这个文件里时才算。
    fn enclosing(&self, file: usize, section: usize, offset: u64) -> Option<Name<'a>> {
        let input = self.inputs.get(file)?;
        let ids = self.resolution.ids.get(file)?;
        input.symbols.iter().enumerate().skip(1).find_map(|(i, s)| {
            let (at_file, symbol) = match ids.get(i).copied().flatten() {
                None => (file, s),
                Some(id) => match self.resolution.table.get(id).definition {
                    Definition::Defined { at, .. } => {
                        (at.file, self.inputs.get(at.file)?.symbols.get(at.index)?)
                    }
                    _ => return None,
                },
            };
            let covers = at_file == file
                && symbol.section == SectionIndex::Index(section)
                && symbol.value <= offset
                && offset - symbol.value < symbol.size;
            covers.then_some(symbol.name)
        })
    }

    /// 一个位置：`>>> {verb} 源文件` 换行 `>>>   文件:(符号)`；没有源文件名时只写后者。
    /// `enclosing` 为 false 时不找覆盖它的符号，只写 `节+偏移`。
    fn location(
        &self,
        f: &mut fmt::Formatter<'_>,
        verb: &str,
        here: Location,
        enclosing: bool,
    ) -> fmt::Result {
        write!(f, ">>> {verb} ")?;
        if let Some(source) = self.source(here.file) {
            // 第二行的缩进让 `文件:` 和上一行的源文件名对齐。
            write!(f, "{source}\n>>> {:width$} ", "", width = verb.len())?;
        }
        let symbol = enclosing
            .then(|| self.enclosing(here.file, here.section, here.offset))
            .flatten();
        match symbol {
            Some(name) => writeln!(f, "{}:({name})", self.file(here.file)),
            None => writeln!(f, "{}", here.display(self.inputs)),
        }
    }

    /// 一个定义所在的位置。绝对符号不在任何节里，返回 `None`。
    fn defined_at(&self, at: SymRef) -> Option<Location> {
        let symbol = self.inputs.get(at.file)?.symbols.get(at.index)?;
        match symbol.section {
            SectionIndex::Index(section) => Some(Location {
                file: at.file,
                section,
                offset: symbol.value,
            }),
            _ => None,
        }
    }

    fn duplicate(&self, f: &mut fmt::Formatter<'_>, d: &Duplicate) -> fmt::Result {
        writeln!(
            f,
            "duplicate symbol: {}",
            self.resolution.table.get(d.symbol).name
        )?;
        match (self.defined_at(d.first), self.defined_at(d.second)) {
            (Some(first), Some(second)) => {
                // lld 发现撞车时，新文件的局部符号还没有建好，第二个位置找不到覆盖它的符号，
                // 总是写成 `节+偏移`。tinylink 照样写，输出才和 lld 一样。
                self.location(f, "defined at", first, true)?;
                self.location(f, "defined at", second, false)
            }
            // 有一个是绝对符号，没有"节+偏移"可写，lld 两行都只写文件名。
            _ => {
                writeln!(f, ">>> defined in {}", self.file(d.first.file))?;
                writeln!(f, ">>> defined in {}", self.file(d.second.file))
            }
        }
    }
}

impl fmt::Display for DiagView<'_, '_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // 符号解析发现的问题都是错误。链接器的警告（比如找不到入口）在 link 里。
        write!(f, "error: ")?;
        match self.diag {
            Diag::Duplicate(d) => self.duplicate(f, d),
            Diag::Undefined { symbol, references } => {
                let entry = self.resolution.table.get(*symbol);
                let visibility = match entry.visibility {
                    Visibility::Default => "",
                    Visibility::Protected => "protected ",
                    Visibility::Hidden => "hidden ",
                    Visibility::Internal => "internal ",
                };
                writeln!(f, "undefined {visibility}symbol: {}", entry.name)?;
                for r in references.iter().take(MAX_REFERENCES) {
                    self.location(f, "referenced by", *r, true)?;
                }
                if references.len() > MAX_REFERENCES {
                    writeln!(
                        f,
                        ">>> referenced {} more times",
                        references.len() - MAX_REFERENCES
                    )?;
                }
                Ok(())
            }
            Diag::Incompatible { file, first } => writeln!(
                f,
                "{} is incompatible with {}",
                self.file(*file),
                self.file(*first)
            ),
            Diag::UnsupportedBinding { at, binding } => {
                let name = self
                    .inputs
                    .get(at.file)
                    .and_then(|i| i.symbols.get(at.index))
                    .map_or_else(String::new, |s| s.name.to_string());
                writeln!(
                    f,
                    "{}: symbol '{name}' (number {}) has unsupported binding {binding}",
                    self.file(at.file),
                    at.index
                )
            }
        }
    }
}

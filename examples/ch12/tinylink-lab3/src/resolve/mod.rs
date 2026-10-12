//! M4 的前半：多个目标文件之间的符号解析（第 3 章）。
//!
//! - `input`：一个输入文件，解析好的 ELF、符号和重定位；
//! - `table`：全局符号表和强、弱、COMMON 的合并规则；
//! - `diag`：解析中发现的问题，报错的写法参照 lld；
//! - `view`：`tinylink resolve` 打印的解析结果表。
//!
//! 和实验 1 一样，问题不在第一次出现时就停下：所有输入读完，所有问题一次报完。

mod diag;
mod input;
mod table;
mod view;

use std::collections::BTreeMap;

pub use diag::{Diag, DiagView};
pub use input::{InputFile, LoadError, Location, LocationView, Relocation};
pub use table::{Definition, Duplicate, GlobalSymbol, SymRef, SymbolId, SymbolTable, Synthetic};
pub use view::ResolveView;

use crate::elf::types::{Machine, SectionFlags, SymbolBinding};

/// 一次符号解析的结果。
#[derive(Debug, Clone)]
pub struct Resolution<'a> {
    pub table: SymbolTable<'a>,
    /// `ids[file][index]`：第 `file` 个输入的第 `index` 个符号在全局表里的编号，
    /// 局部符号和 0 号空项是 `None`。实验 4 处理重定位时按它找到每个符号的定义。
    pub ids: Vec<Vec<Option<SymbolId>>>,
    pub diags: Vec<Diag>,
}

impl Resolution<'_> {
    pub fn has_errors(&self) -> bool {
        !self.diags.is_empty()
    }
}

/// 收集输入的阶段。`add_file` 一个个读入输入文件，`finish` 结束收集，交出 `Resolution`。
///
/// 两个阶段用类型分开：`finish` 拿走 `Resolver` 的所有权，之后再也不能 `add_file`；
/// 合成符号只在 `finish` 里补上，不会出现"先填了合成符号、后来的真实定义被挡住"。
/// 实验 7 读静态库时，在两次 `add_file` 之间用 `strong_undefined` 决定还要拉哪些成员。
#[derive(Debug, Default)]
pub struct Resolver<'a> {
    table: SymbolTable<'a>,
    ids: Vec<Vec<Option<SymbolId>>>,
    diags: Vec<Diag>,
    machine: Option<Machine>,
}

impl<'a> Resolver<'a> {
    pub fn new() -> Self {
        Self::default()
    }

    /// 读入下一个输入。`input` 必须是 `finish` 时传入的切片里的第 `ids.len()` 个。
    pub fn add_file(&mut self, input: &InputFile<'a>) {
        let file = self.ids.len();
        let machine = input.elf.header().machine;
        let mut file_ids = vec![None; input.symbols.len()];
        if *self.machine.get_or_insert(machine) != machine {
            self.diags.push(Diag::Incompatible { file, first: 0 });
            self.ids.push(file_ids);
            return;
        }
        // 0 号符号是规范要求的空项。
        for (index, symbol) in input.symbols.iter().enumerate().skip(1) {
            let at = SymRef { file, index };
            match symbol.binding {
                // 局部符号只在自己的文件里有意义，不进全局表。
                SymbolBinding::Local => continue,
                SymbolBinding::Global | SymbolBinding::Weak => {}
                // STB_GNU_UNIQUE（10）之类的扩展绑定还不支持。
                SymbolBinding::Other(binding) => {
                    self.diags.push(Diag::UnsupportedBinding { at, binding });
                    continue;
                }
            }
            let (id, duplicate) = self.table.add(at, symbol);
            file_ids[index] = Some(id);
            if let Some(d) = duplicate {
                self.diags.push(Diag::Duplicate(d));
            }
        }
        self.ids.push(file_ids);
    }

    /// 目前有强引用、还没有定义的名字。实验 7 按它们去静态库里找成员。
    pub fn strong_undefined(&self) -> impl Iterator<Item = SymbolId> + '_ {
        self.table
            .iter()
            .filter(|(_, s)| s.definition == Definition::Undefined && s.strong_reference)
            .map(|(id, _)| id)
    }

    /// 结束收集：补上合成符号，找出未定义的名字。`inputs` 是读入过的全部输入，顺序不变。
    pub fn finish(mut self, inputs: &[InputFile<'a>]) -> Resolution<'a> {
        debug_assert_eq!(inputs.len(), self.ids.len(), "every input was added once");
        self.table.define_synthetic();

        // 找不到定义的名字，只有被某条重定位用到、并且有强引用时才算错。
        // 一个只在符号表里出现、没有被用到的未定义符号不影响链接，GNU ld 和 lld 都这样。
        // 不可分配的节（比如 .debug_info）里的重定位不算：lld 不为它们报错，GNU ld 报。
        let mut references: BTreeMap<SymbolId, Vec<Location>> = BTreeMap::new();
        for (file, input) in inputs.iter().enumerate() {
            for r in &input.relocations {
                let allocated = input
                    .elf
                    .section(r.section)
                    .is_some_and(|s| s.header.flags.contains(SectionFlags::ALLOC));
                // 符号编号在解析重定位时检查过，`get` 一定取得到；取到 `None` 说明是局部符号。
                let Some(Some(id)) = self.ids[file].get(r.rela.symbol as usize).copied() else {
                    continue;
                };
                let entry = self.table.get(id);
                if allocated && entry.definition == Definition::Undefined && entry.strong_reference
                {
                    references.entry(id).or_default().push(Location {
                        file,
                        section: r.section,
                        offset: r.rela.offset,
                    });
                }
            }
        }
        self.diags.extend(
            references
                .into_iter()
                .map(|(symbol, references)| Diag::Undefined { symbol, references }),
        );

        Resolution {
            table: self.table,
            ids: self.ids,
            diags: self.diags,
        }
    }
}

/// 对一组输入做符号解析。输入的顺序就是命令行的顺序，它决定了弱定义之间、
/// 同样大小的 COMMON 之间选谁。
pub fn resolve<'a>(inputs: &[InputFile<'a>]) -> Resolution<'a> {
    let mut resolver = Resolver::new();
    for input in inputs {
        resolver.add_file(input);
    }
    resolver.finish(inputs)
}

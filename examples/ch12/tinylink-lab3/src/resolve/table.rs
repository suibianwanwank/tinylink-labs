//! 全局符号表：每个名字一项，记录到目前为止它的定义是谁。
//!
//! 表项里不存对输入文件里 `Symbol` 的引用，存的是下标 `SymRef { file, index }`。
//! 表本身因此不借用任何输入文件的结构，只借用名字的字节（`Name<'a>`），
//! 往表里添项、改项时也不会和"正在读的那个输入文件"的借用打架。

use std::collections::HashMap;
use std::collections::hash_map::Entry;

use crate::elf::Name;
use crate::elf::Symbol;
use crate::elf::types::{SectionIndex, SymbolBinding, SymbolType, Visibility};

/// 全局符号表里的一项的编号。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SymbolId(usize);

/// 某个输入文件符号表里的第几个符号。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SymRef {
    pub file: usize,
    pub index: usize,
}

/// 链接器自己定义的符号。只有在有人引用、所有输入都没有定义时才生效，
/// 相当于 GNU ld 默认链接器脚本里的 `PROVIDE`。值要等布局（实验 5）定下来才知道，
/// 这里只占住名字，让引用它们的程序不报"未定义"。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Synthetic {
    /// `__ehdr_start`、`__executable_start`：第一个段的起点，ELF 头就在那里。
    ImageStart,
    /// `etext`、`_etext`、`__etext`：可执行段的末尾。
    TextEnd,
    /// `edata`、`_edata`：已初始化数据的末尾。
    DataEnd,
    /// `__bss_start`：`.bss` 的起点。
    BssStart,
    /// `end`、`_end`：整个映像（含 `.bss`）的末尾。
    End,
}

impl Synthetic {
    /// 实验 4、5 用得上的几个名字。`_GLOBAL_OFFSET_TABLE_` 要等有了 GOT（第 7 章）再加。
    pub fn for_name(name: &[u8]) -> Option<Synthetic> {
        Some(match name {
            b"__ehdr_start" | b"__executable_start" => Synthetic::ImageStart,
            b"etext" | b"_etext" | b"__etext" => Synthetic::TextEnd,
            b"edata" | b"_edata" => Synthetic::DataEnd,
            b"__bss_start" => Synthetic::BssStart,
            b"end" | b"_end" => Synthetic::End,
            _ => return None,
        })
    }
}

/// 一个名字目前的定义。每读到一个同名符号，就按第 3 章的规则和它合并一次。
/// 实验 7 读静态库时会加一个 Lazy 变体：定义在某个还没拉进来的库成员里。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Definition {
    /// 只见到过引用。
    Undefined,
    /// 一个普通定义：在某个节里，或者是 `SHN_ABS` 的绝对值。
    Defined {
        at: SymRef,
        binding: SymbolBinding,
        kind: SymbolType,
        section: SectionIndex,
        value: u64,
        size: u64,
    },
    /// COMMON 符号。`size` 和 `align` 是所有同名 COMMON 里的最大值；
    /// `at` 是尺寸最大的那一个，尺寸相同时取先读到的，和 GNU ld、lld 一致。
    Common { at: SymRef, size: u64, align: u64 },
    /// 链接器合成的符号。
    Synthetic(Synthetic),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GlobalSymbol<'a> {
    pub name: Name<'a>,
    pub definition: Definition,
    /// 所有同名符号里最严的可见性（gABI 的规则）。实验 8 起写共享库时才用到（实验 7 是静态库）。
    pub visibility: Visibility,
    /// 每个提到这个名字的全局符号，按读入的顺序。
    pub occurrences: Vec<SymRef>,
    /// 是否有输入以 `STB_GLOBAL` 引用它。只有弱引用的名字找不到定义时值为 0，不报错。
    pub strong_reference: bool,
}

/// 两个强定义撞车。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Duplicate {
    pub symbol: SymbolId,
    /// 表里原有的定义。
    pub first: SymRef,
    /// 新读到的定义，它不会被采用。
    pub second: SymRef,
}

/// 可见性从宽到严：DEFAULT < PROTECTED < HIDDEN < INTERNAL。
fn strictness(v: Visibility) -> u8 {
    match v {
        Visibility::Default => 0,
        Visibility::Protected => 1,
        Visibility::Hidden => 2,
        Visibility::Internal => 3,
    }
}

/// 新读到的符号在合并规则里属于哪一种。
#[derive(Debug, Clone, Copy)]
enum Incoming {
    Undefined,
    Defined,
    Common { size: u64, align: u64 },
}

impl Incoming {
    fn of(symbol: &Symbol<'_>) -> Incoming {
        match symbol.section {
            SectionIndex::Undef => Incoming::Undefined,
            // COMMON 符号的 st_value 存的是对齐要求。
            SectionIndex::Common => Incoming::Common {
                size: symbol.size,
                align: symbol.value,
            },
            SectionIndex::Abs | SectionIndex::Index(_) => Incoming::Defined,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct SymbolTable<'a> {
    symbols: Vec<GlobalSymbol<'a>>,
    by_name: HashMap<Name<'a>, SymbolId>,
}

impl<'a> SymbolTable<'a> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn get(&self, id: SymbolId) -> &GlobalSymbol<'a> {
        // SymbolId 只由这张表发出，下标一定有效。
        &self.symbols[id.0]
    }

    pub fn lookup(&self, name: &[u8]) -> Option<SymbolId> {
        self.by_name.get(&Name::new(name)).copied()
    }

    /// 按第一次出现的顺序遍历。
    pub fn iter(&self) -> impl Iterator<Item = (SymbolId, &GlobalSymbol<'a>)> {
        self.symbols
            .iter()
            .enumerate()
            .map(|(i, s)| (SymbolId(i), s))
    }

    pub fn len(&self) -> usize {
        self.symbols.len()
    }

    pub fn is_empty(&self) -> bool {
        self.symbols.is_empty()
    }

    /// 把一个全局符号（GLOBAL 或 WEAK）并进表里。返回它在表里的编号；
    /// 和已有的强定义撞车时，另外返回一个 `Duplicate`，表里保留原来的定义。
    pub(super) fn add(&mut self, at: SymRef, symbol: &Symbol<'a>) -> (SymbolId, Option<Duplicate>) {
        debug_assert!(
            symbol.binding != SymbolBinding::Local,
            "local symbols never enter the global table"
        );
        // entry 只查一次哈希表：名字已经在表里就取出编号，不在就当场插入新的一项。
        let id = match self.by_name.entry(symbol.name) {
            Entry::Occupied(e) => *e.get(),
            Entry::Vacant(e) => {
                let id = SymbolId(self.symbols.len());
                self.symbols.push(GlobalSymbol {
                    name: symbol.name,
                    definition: Definition::Undefined,
                    visibility: Visibility::Default,
                    occurrences: Vec::new(),
                    strong_reference: false,
                });
                *e.insert(id)
            }
        };
        let entry = &mut self.symbols[id.0];
        entry.occurrences.push(at);
        if strictness(symbol.visibility) > strictness(entry.visibility) {
            entry.visibility = symbol.visibility;
        }
        let weak = symbol.binding == SymbolBinding::Weak;
        let defined = Definition::Defined {
            at,
            binding: symbol.binding,
            kind: symbol.kind,
            section: symbol.section,
            value: symbol.value,
            size: symbol.size,
        };

        let duplicate = match (entry.definition, Incoming::of(symbol)) {
            (_, Incoming::Undefined) => {
                if !weak {
                    entry.strong_reference = true;
                }
                None
            }
            // 第一个定义，不管强弱、是不是 COMMON，都先记下。
            (Definition::Undefined, Incoming::Defined) => {
                entry.definition = defined;
                None
            }
            (Definition::Undefined, Incoming::Common { size, align }) => {
                entry.definition = Definition::Common { at, size, align };
                None
            }
            // 弱定义从不替换已有的定义，也不和谁冲突；两个弱定义保留先读到的。
            // 已有 COMMON 时也一样，gABI 写明此时采用 COMMON。
            (_, Incoming::Defined) if weak => None,
            // 强定义替换弱定义和 COMMON。
            (
                Definition::Defined {
                    binding: SymbolBinding::Weak,
                    ..
                },
                Incoming::Defined,
            ) => {
                entry.definition = defined;
                None
            }
            (Definition::Common { .. }, Incoming::Defined) => {
                entry.definition = defined;
                None
            }
            // 两个强定义。例外是值相同的两个绝对符号：GNU ld 一律放过，lld 只在值不为 0 时放过，
            // tinylink 跟 GNU ld。
            (
                Definition::Defined {
                    at: first,
                    section,
                    value,
                    ..
                },
                Incoming::Defined,
            ) => {
                let same_absolute = section == SectionIndex::Abs
                    && symbol.section == SectionIndex::Abs
                    && value == symbol.value;
                (!same_absolute).then_some(Duplicate {
                    symbol: id,
                    first,
                    second: at,
                })
            }
            // COMMON 遇上强定义：保留强定义。遇上弱定义：COMMON 取而代之，两个链接器都这样做。
            (Definition::Defined { binding, .. }, Incoming::Common { size, align }) => {
                if binding == SymbolBinding::Weak {
                    entry.definition = Definition::Common { at, size, align };
                }
                None
            }
            // 两个 COMMON 合并：尺寸、对齐都取最大值。
            (
                Definition::Common {
                    at: old_at,
                    size: old_size,
                    align: old_align,
                },
                Incoming::Common { size, align },
            ) => {
                entry.definition = Definition::Common {
                    at: if size > old_size { at } else { old_at },
                    size: size.max(old_size),
                    align: align.max(old_align),
                };
                None
            }
            // 合成符号只在全部输入读完之后才填进来。
            (Definition::Synthetic(_), _) => None,
        };
        (id, duplicate)
    }

    /// 全部输入读完之后调用：没有定义、名字又是保留的合成符号，就由链接器定义。
    pub(super) fn define_synthetic(&mut self) {
        for entry in &mut self.symbols {
            if entry.definition == Definition::Undefined
                && let Some(s) = Synthetic::for_name(entry.name.as_bytes())
            {
                entry.definition = Definition::Synthetic(s);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 造一个名叫 n 的符号。`section` 决定它是定义、引用还是 COMMON。
    fn sym(
        binding: SymbolBinding,
        section: SectionIndex,
        value: u64,
        size: u64,
    ) -> Symbol<'static> {
        Symbol {
            name: Name::new(b"n"),
            value,
            size,
            kind: SymbolType::Object,
            binding,
            visibility: Visibility::Default,
            section,
        }
    }

    const GLOBAL: SymbolBinding = SymbolBinding::Global;
    const WEAK: SymbolBinding = SymbolBinding::Weak;
    const DATA: SectionIndex = SectionIndex::Index(2);

    /// 按顺序把一串符号加进表里，每个来自不同的文件。返回最后的定义和撞车的次数。
    fn merge(symbols: &[Symbol<'static>]) -> (Definition, usize) {
        let mut table = SymbolTable::new();
        let mut duplicates = 0;
        let mut id = None;
        for (file, s) in symbols.iter().enumerate() {
            let (i, dup) = table.add(SymRef { file, index: 1 }, s);
            id = Some(i);
            duplicates += usize::from(dup.is_some());
        }
        (
            table.get(id.expect("at least one symbol")).definition,
            duplicates,
        )
    }

    fn from_file(d: Definition) -> usize {
        match d {
            Definition::Defined { at, .. } | Definition::Common { at, .. } => at.file,
            other => panic!("not a definition: {other:?}"),
        }
    }

    #[test]
    fn strong_beats_weak_in_either_order() {
        let strong = sym(GLOBAL, DATA, 0, 4);
        let weak = sym(WEAK, DATA, 0, 4);
        assert_eq!(from_file(merge(&[weak, strong]).0), 1);
        assert_eq!(from_file(merge(&[strong, weak]).0), 0);
    }

    #[test]
    fn first_weak_wins_and_two_strong_collide() {
        let weak = sym(WEAK, DATA, 0, 4);
        assert_eq!(merge(&[weak, weak]), (merge(&[weak]).0, 0));
        let strong = sym(GLOBAL, DATA, 0, 4);
        assert_eq!(merge(&[strong, strong, strong]).1, 2);
    }

    #[test]
    fn commons_merge_to_the_largest_size_and_alignment() {
        let c16 = sym(GLOBAL, SectionIndex::Common, 16, 4);
        let c8 = sym(GLOBAL, SectionIndex::Common, 8, 8);
        let (d, _) = merge(&[c16, c8]);
        assert_eq!(
            d,
            Definition::Common {
                at: SymRef { file: 1, index: 1 },
                size: 8,
                align: 16
            }
        );
    }

    #[test]
    fn common_loses_to_strong_and_beats_weak() {
        let common = sym(GLOBAL, SectionIndex::Common, 4, 4);
        let strong = sym(GLOBAL, DATA, 0, 4);
        let weak = sym(WEAK, DATA, 0, 4);
        assert!(matches!(
            merge(&[common, strong]).0,
            Definition::Defined { .. }
        ));
        assert!(matches!(
            merge(&[strong, common]).0,
            Definition::Defined { .. }
        ));
        assert!(matches!(
            merge(&[common, weak]).0,
            Definition::Common { .. }
        ));
        assert!(matches!(
            merge(&[weak, common]).0,
            Definition::Common { .. }
        ));
    }

    #[test]
    fn equal_absolute_values_are_not_duplicates() {
        let k42 = sym(GLOBAL, SectionIndex::Abs, 42, 0);
        let k43 = sym(GLOBAL, SectionIndex::Abs, 43, 0);
        assert_eq!(merge(&[k42, k42]).1, 0);
        assert_eq!(merge(&[k42, k43]).1, 1);
    }

    /// 两个可见性的同名符号按顺序加进表里，返回最后的可见性。
    fn merged_visibility(first: Visibility, second: Visibility) -> Visibility {
        let mut table = SymbolTable::new();
        let mut a = sym(GLOBAL, SectionIndex::Undef, 0, 0);
        a.visibility = first;
        let mut b = sym(GLOBAL, DATA, 0, 4);
        b.visibility = second;
        table.add(SymRef { file: 0, index: 1 }, &a);
        let (id, _) = table.add(SymRef { file: 1, index: 1 }, &b);
        table.get(id).visibility
    }

    #[test]
    fn strictest_visibility_wins_in_either_order() {
        use Visibility::{Default, Hidden, Protected};
        assert_eq!(merged_visibility(Protected, Hidden), Hidden);
        assert_eq!(merged_visibility(Hidden, Protected), Hidden);
        assert_eq!(merged_visibility(Hidden, Default), Hidden);
        assert_eq!(merged_visibility(Default, Protected), Protected);
    }

    #[test]
    fn larger_common_first_keeps_its_file() {
        // 先大后小：8 字节、8 对齐，然后 4 字节、64 对齐。尺寸取 8、对齐取 64，归第一个文件。
        let big = sym(GLOBAL, SectionIndex::Common, 8, 8);
        let small = sym(GLOBAL, SectionIndex::Common, 64, 4);
        assert_eq!(
            merge(&[big, small]).0,
            Definition::Common {
                at: SymRef { file: 0, index: 1 },
                size: 8,
                align: 64
            }
        );
    }

    #[test]
    fn synthetic_names_only_fill_undefined_slots() {
        let mut table = SymbolTable::new();
        let mut end = sym(GLOBAL, SectionIndex::Undef, 0, 0);
        end.name = Name::new(b"_end");
        let (id, _) = table.add(SymRef { file: 0, index: 1 }, &end);
        table.define_synthetic();
        assert_eq!(
            table.get(id).definition,
            Definition::Synthetic(Synthetic::End)
        );
    }
}

//! 第一遍：给每个模块分配基址，建立全局符号表。
//!
//! 这一遍只看模块的长度和定义列表，不看程序正文里的每个字。
//! 它结束时，每个符号的绝对地址都已经确定，第二遍才能放心地改写引用。

use std::collections::BTreeMap;
use std::collections::btree_map::Entry;

use super::diag::{Diagnostic, Pos, Problem};
use super::parse::Module;

/// 玩具机器的内存有多少个字。
pub const MEMORY_SIZE: u32 = 500;

/// 一个模块被放在了哪里。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Placement<'a> {
    pub name: &'a str,
    pub base: u32,
    pub size: u32,
}

/// 符号表里的一项。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Symbol<'a> {
    pub addr: u32,
    pub module: &'a str,
    /// 定义在源文件里的位置，报重复定义和"未使用"时用。
    pub pos: Pos,
}

/// 第一遍的产物。名字都借用自源文本，不复制。
#[derive(Debug, Default)]
pub struct Layout<'a> {
    pub placements: Vec<Placement<'a>>,
    /// 用 BTreeMap 而不用 HashMap：遍历顺序按名字排序，输出每次都一样。
    pub symbols: BTreeMap<&'a str, Symbol<'a>>,
}

/// 模块长度转成 u32。调用前已经检查过总长度不超过 MEMORY_SIZE。
fn len_u32(len: usize) -> u32 {
    u32::try_from(len).expect("checked against MEMORY_SIZE")
}

/// 检查所有模块放不放得进内存。放不进是致命错误，报在第一个越界的模块上。
fn check_fits<'a>(modules: &[Module<'a>]) -> Result<(), Diagnostic<'a>> {
    let total: usize = modules.iter().map(|m| m.text.len()).sum();
    let limit = MEMORY_SIZE as usize;
    if total <= limit {
        return Ok(());
    }
    // scan 带着一个累加器往前走，产出每个模块结束处的地址。
    let (_, first_over) = modules
        .iter()
        .scan(0, |end, m| {
            *end += m.text.len();
            Some((*end, m))
        })
        .find(|&(end, _)| end > limit)
        .expect("total exceeds the limit, so some prefix does too");
    Err(Diagnostic::new(
        first_over.name.pos,
        Problem::ProgramTooLarge {
            words: total,
            memory: MEMORY_SIZE,
        },
    ))
}

pub fn run<'a>(
    modules: &[Module<'a>],
    diags: &mut Vec<Diagnostic<'a>>,
) -> Result<Layout<'a>, Diagnostic<'a>> {
    check_fits(modules)?;

    let mut layout = Layout::default();
    let mut base = 0;

    for module in modules {
        let size = len_u32(module.text.len());

        for def in &module.defs {
            let offset = if def.offset < size {
                def.offset
            } else if size == 0 {
                // 空模块里没有任何合法偏移。按偏移 0 处理会得到下一个模块的地址，
                // 甚至超出内存，所以这个定义不进符号表，引用它的地方会报未定义。
                diags.push(Diagnostic::new(
                    def.name.pos,
                    Problem::DefinitionInEmptyModule {
                        name: def.name.text,
                        module: module.name.text,
                    },
                ));
                continue;
            } else {
                diags.push(Diagnostic::new(
                    def.name.pos,
                    Problem::DefinitionOutsideModule {
                        name: def.name.text,
                        offset: def.offset,
                        module: module.name.text,
                        size,
                    },
                ));
                0
            };

            match layout.symbols.entry(def.name.text) {
                Entry::Vacant(slot) => {
                    slot.insert(Symbol {
                        addr: base + offset,
                        module: module.name.text,
                        pos: def.name.pos,
                    });
                }
                Entry::Occupied(first) => {
                    let first = first.get();
                    diags.push(Diagnostic::new(
                        def.name.pos,
                        Problem::DuplicateDefinition {
                            name: def.name.text,
                            first_module: first.module,
                            first_line: first.pos.line,
                        },
                    ));
                }
            }
        }

        layout.placements.push(Placement {
            name: module.name.text,
            base,
            size,
        });
        base += size;
    }

    Ok(layout)
}

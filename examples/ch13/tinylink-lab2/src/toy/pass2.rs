//! 第二遍：按第一遍的结果改写每个字，拼出内存映像。
//!
//! 第二遍只读第一遍的产物（基址和符号表），不再修改它们。
//! 哪些符号被用到了，单独记在本遍自己的集合里。

use std::collections::{BTreeMap, HashSet};

use super::diag::{Diagnostic, Problem};
use super::parse::{Mode, Module, Word};
use super::pass1::{Layout, MEMORY_SIZE, Placement, Symbol};

/// 一个字最多四位十进制数：一位操作码，三位操作数。
pub const MAX_WORD: u32 = 9999;

fn split(value: u32) -> (u32, u32) {
    (value / 1000, value % 1000)
}

fn join(opcode: u32, operand: u32) -> u32 {
    opcode * 1000 + operand
}

/// 一个字改写失败、但链接器按规则恢复了：恢复后的值，以及发生了什么问题。
struct Recovered<'a> {
    value: u32,
    problem: Problem<'a>,
}

/// 改写一个模块时需要的上下文。
struct ModuleCtx<'m, 'a> {
    module: &'m Module<'a>,
    placement: Placement<'a>,
    /// 使用列表里每一项有没有被 E 字引用过。
    referenced: Vec<bool>,
}

fn relocate<'a>(
    word: &Word,
    ctx: &mut ModuleCtx<'_, 'a>,
    symbols: &BTreeMap<&'a str, Symbol<'a>>,
    used: &mut HashSet<&'a str>,
) -> Result<u32, Recovered<'a>> {
    if word.value > MAX_WORD {
        return Err(Recovered {
            value: MAX_WORD,
            problem: Problem::WordTooLarge { word: word.value },
        });
    }
    let (opcode, operand) = split(word.value);
    let Placement {
        name: module,
        base,
        size,
    } = ctx.placement;

    match word.mode {
        Mode::Immediate => Ok(word.value),
        Mode::Absolute if operand >= MEMORY_SIZE => Err(Recovered {
            value: join(opcode, 0),
            problem: Problem::AbsoluteOutsideMemory {
                addr: operand,
                memory: MEMORY_SIZE,
            },
        }),
        Mode::Absolute => Ok(word.value),
        Mode::Relative if operand >= size => Err(Recovered {
            value: join(opcode, base),
            problem: Problem::RelativeOutsideModule {
                addr: operand,
                module,
                size,
            },
        }),
        Mode::Relative => Ok(join(opcode, base + operand)),
        Mode::External => {
            let index = operand as usize;
            let Some(name) = ctx.module.uses.get(index) else {
                return Err(Recovered {
                    value: word.value,
                    problem: Problem::ExternalIndexOutOfRange {
                        index,
                        module,
                        len: ctx.module.uses.len(),
                    },
                });
            };
            ctx.referenced[index] = true;
            match symbols.get(name.text) {
                Some(symbol) => {
                    used.insert(name.text);
                    Ok(join(opcode, symbol.addr))
                }
                None => Err(Recovered {
                    value: join(opcode, 0),
                    problem: Problem::Undefined { name: name.text },
                }),
            }
        }
    }
}

pub fn run<'a>(
    modules: &[Module<'a>],
    layout: &Layout<'a>,
    diags: &mut Vec<Diagnostic<'a>>,
) -> Vec<u32> {
    let mut memory = Vec::new();
    // 这里只问"在不在"，不遍历，HashSet 就够了；输出顺序由 BTreeMap 决定。
    let mut used = HashSet::new();

    // zip 在较短的一边结束时就停下，两边长度不等会悄悄漏掉模块。
    debug_assert_eq!(modules.len(), layout.placements.len());
    for (module, &placement) in modules.iter().zip(&layout.placements) {
        let mut ctx = ModuleCtx {
            module,
            placement,
            referenced: vec![false; module.uses.len()],
        };

        for word in &module.text {
            let value = relocate(word, &mut ctx, &layout.symbols, &mut used).unwrap_or_else(
                |Recovered { value, problem }| {
                    diags.push(Diagnostic::new(word.pos, problem));
                    value
                },
            );
            memory.push(value);
        }

        let unreferenced = module
            .uses
            .iter()
            .zip(&ctx.referenced)
            .enumerate()
            .filter(|(_, (_, referenced))| !**referenced);
        for (index, (name, _)) in unreferenced {
            diags.push(Diagnostic::new(
                name.pos,
                Problem::UseNotReferenced {
                    name: name.text,
                    index,
                    module: module.name.text,
                },
            ));
        }
    }

    // 规则 4 要等所有模块都改写完才能判断，按符号名的顺序报告。
    for (name, symbol) in &layout.symbols {
        if !used.contains(name) {
            diags.push(Diagnostic::new(
                symbol.pos,
                Problem::DefinedButNotUsed { name },
            ));
        }
    }

    memory
}

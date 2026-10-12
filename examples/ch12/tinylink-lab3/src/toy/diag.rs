//! 诊断信息：所有报错和警告的文本都在这一个文件里定义。
//!
//! 文本的统一规则：
//! - 渲染成 `文件:行:列: error: 说明` 或 `文件:行:列: warning: 说明`，行、列从 1 开始，Tab 算一列；
//! - 说明以小写字母开头，结尾不加句号；
//! - 能恢复的错误，在分号后面写明链接器怎么处理的，比如 `; using offset 0`；
//! - 数量词按单复数写，比如 `1 word`、`2 words`。

use std::fmt;

/// 源文件里的一个位置。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Pos {
    pub line: u32,
    pub col: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warning,
}

impl fmt::Display for Severity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Severity::Error => "error",
            Severity::Warning => "warning",
        })
    }
}

/// 链接器能发现的每一种问题。规则编号和 README 里的错误规则表一致。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Problem<'a> {
    /// 规则 1：语法错误。`found` 为 `None` 表示读到了文件末尾。
    Expected {
        expected: &'static str,
        found: Option<&'a str>,
    },
    /// 规则 1：既不是数字也不是名字的记号，比如 `12ab`。
    InvalidToken { text: &'a str },
    /// 规则 1：数字大到 u32 放不下。
    NumberTooLarge { text: &'a str },
    /// 规则 1：输入文件不是合法的 UTF-8，位置指向第一个非法字节。
    InvalidUtf8,
    /// 规则 2：同一个符号定义了不止一次。
    DuplicateDefinition {
        name: &'a str,
        first_module: &'a str,
        first_line: u32,
    },
    /// 规则 3：E 字引用的符号没有任何模块定义。
    Undefined { name: &'a str },
    /// 规则 4：符号定义了，但没有任何 E 字引用它。
    DefinedButNotUsed { name: &'a str },
    /// 规则 5：定义所在的模块一个字都没有，任何偏移都不合法。
    DefinitionInEmptyModule { name: &'a str, module: &'a str },
    /// 规则 5：定义给出的偏移超出了本模块的长度。
    DefinitionOutsideModule {
        name: &'a str,
        offset: u32,
        module: &'a str,
        size: u32,
    },
    /// 规则 6：E 字的操作数超出了使用列表的长度。
    ExternalIndexOutOfRange {
        index: usize,
        module: &'a str,
        len: usize,
    },
    /// 规则 7：使用列表里列了这个符号，但本模块没有 E 字引用它。
    UseNotReferenced {
        name: &'a str,
        index: usize,
        module: &'a str,
    },
    /// 规则 8：A 字的地址超出了内存大小。
    AbsoluteOutsideMemory { addr: u32, memory: u32 },
    /// 规则 9：R 字的地址超出了本模块的长度。
    RelativeOutsideModule {
        addr: u32,
        module: &'a str,
        size: u32,
    },
    /// 规则 10：字超过四位十进制数。
    WordTooLarge { word: u32 },
    /// 规则 11：所有模块加起来放不进内存。
    ProgramTooLarge { words: usize, memory: u32 },
}

impl Problem<'_> {
    pub fn severity(&self) -> Severity {
        match self {
            Problem::DefinedButNotUsed { .. } | Problem::UseNotReferenced { .. } => {
                Severity::Warning
            }
            _ => Severity::Error,
        }
    }
}

/// 数量加上单复数正确的单位：`words(1)` 是 `1 word`，`words(3)` 是 `3 words`。
fn words(n: usize) -> String {
    if n == 1 {
        "1 word".to_owned()
    } else {
        format!("{n} words")
    }
}

impl fmt::Display for Problem<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Problem::Expected {
                expected,
                found: Some(text),
            } => {
                write!(f, "expected {expected}, found '{text}'")
            }
            Problem::Expected {
                expected,
                found: None,
            } => {
                write!(f, "expected {expected}, found end of file")
            }
            Problem::InvalidToken { text } => write!(f, "invalid token '{text}'"),
            Problem::NumberTooLarge { text } => write!(f, "number '{text}' is too large"),
            Problem::InvalidUtf8 => write!(f, "input is not valid UTF-8"),
            Problem::DuplicateDefinition {
                name,
                first_module,
                first_line,
            } => write!(
                f,
                "symbol '{name}' is already defined in module '{first_module}' (line {first_line}); keeping the first definition"
            ),
            Problem::Undefined { name } => write!(f, "undefined symbol '{name}'; using address 0"),
            Problem::DefinedButNotUsed { name } => {
                write!(f, "symbol '{name}' is defined but never used")
            }
            Problem::DefinitionInEmptyModule { name, module } => write!(
                f,
                "symbol '{name}' is defined in module '{module}', which has no words; ignoring the definition"
            ),
            Problem::DefinitionOutsideModule {
                name,
                offset,
                module,
                size,
            } => write!(
                f,
                "symbol '{name}' is defined at offset {offset}, but module '{module}' has only {}; using offset 0",
                words(*size as usize)
            ),
            Problem::ExternalIndexOutOfRange { index, module, len } => write!(
                f,
                "external index {index} is out of range for the use list of module '{module}' (length {len}); treating the word as immediate"
            ),
            Problem::UseNotReferenced {
                name,
                index,
                module,
            } => write!(
                f,
                "symbol '{name}' at index {index} of the use list of module '{module}' is not referred to by any E word"
            ),
            Problem::AbsoluteOutsideMemory { addr, memory } => write!(
                f,
                "absolute address {addr} is outside memory of {}; using address 0",
                words(*memory as usize)
            ),
            Problem::RelativeOutsideModule { addr, module, size } => write!(
                f,
                "relative address {addr} is outside module '{module}' of {}; using offset 0",
                words(*size as usize)
            ),
            Problem::WordTooLarge { word } => {
                write!(f, "word {word} has more than 4 digits; using 9999")
            }
            Problem::ProgramTooLarge {
                words: total,
                memory,
            } => write!(
                f,
                "program needs {} but memory has only {memory}",
                words(*total)
            ),
        }
    }
}

/// 一条诊断：哪里出了什么问题。严重程度由问题本身决定。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic<'a> {
    pub pos: Pos,
    pub problem: Problem<'a>,
}

impl<'a> Diagnostic<'a> {
    pub fn new(pos: Pos, problem: Problem<'a>) -> Self {
        Diagnostic { pos, problem }
    }

    pub fn severity(&self) -> Severity {
        self.problem.severity()
    }

    /// 渲染成一行文本，`file` 是用户在命令行上给的文件名。
    pub fn render(&self, file: &str) -> String {
        format!(
            "{file}:{}:{}: {}: {}",
            self.pos.line,
            self.pos.col,
            self.severity(),
            self.problem
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_use_singular_and_plural() {
        assert_eq!(words(0), "0 words");
        assert_eq!(words(1), "1 word");
        assert_eq!(words(2), "2 words");
        let p = Problem::RelativeOutsideModule {
            addr: 1,
            module: "m",
            size: 1,
        };
        assert_eq!(
            p.to_string(),
            "relative address 1 is outside module 'm' of 1 word; using offset 0"
        );
    }
}

//! 语法分析：把记号序列读成模块列表。
//!
//! 文法（大写是记号类别，引号里是关键字）：
//!
//! ```text
//! file   := module*
//! module := "module" NAME item* "text" word* "end"
//! item   := "def" NAME NUMBER          定义：符号在本模块内的偏移
//!         | "use" NAME*                使用：E 字按下标引用这里的符号
//! word   := MODE NUMBER                MODE 是 I、A、R、E 之一
//! ```
//!
//! 关键字不能用作符号名。遇到第一个语法错误就停止。

use super::diag::{Diagnostic, Pos, Problem};
use super::lex::{Token, TokenKind, Tokens};

const KEYWORDS: [&str; 5] = ["module", "def", "use", "text", "end"];

/// 地址模式：链接器怎样改写一个字的操作数。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// I：立即数，原样保留。
    Immediate,
    /// A：绝对地址，原样保留，但要检查不超出内存。
    Absolute,
    /// R：相对于本模块起点的地址，加上模块的基址。
    Relative,
    /// E：操作数是使用列表的下标，换成那个符号的绝对地址。
    External,
}

impl Mode {
    fn from_letter(text: &str) -> Option<Mode> {
        match text {
            "I" => Some(Mode::Immediate),
            "A" => Some(Mode::Absolute),
            "R" => Some(Mode::Relative),
            "E" => Some(Mode::External),
            _ => None,
        }
    }
}

/// 源文本里的一个名字，连同它的位置。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Name<'a> {
    pub text: &'a str,
    pub pos: Pos,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Def<'a> {
    pub name: Name<'a>,
    pub offset: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Word {
    pub mode: Mode,
    pub value: u32,
    pub pos: Pos,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Module<'a> {
    pub name: Name<'a>,
    pub defs: Vec<Def<'a>>,
    pub uses: Vec<Name<'a>>,
    pub text: Vec<Word>,
}

struct Parser<'a> {
    tokens: Vec<Token<'a>>,
    next: usize,
    eof: Pos,
}

impl<'a> Parser<'a> {
    fn peek(&self) -> Option<&Token<'a>> {
        self.tokens.get(self.next)
    }

    /// 取下一个记号；没有了就报"期望 X，却读到文件末尾"。
    fn advance(&mut self, expected: &'static str) -> Result<Token<'a>, Diagnostic<'a>> {
        let token = self.tokens.get(self.next).copied().ok_or_else(|| {
            Diagnostic::new(
                self.eof,
                Problem::Expected {
                    expected,
                    found: None,
                },
            )
        })?;
        self.next += 1;
        Ok(token)
    }

    fn unexpected(token: Token<'a>, expected: &'static str) -> Diagnostic<'a> {
        Diagnostic::new(
            token.pos,
            Problem::Expected {
                expected,
                found: Some(token.text),
            },
        )
    }

    fn keyword(
        &mut self,
        keyword: &'static str,
        expected: &'static str,
    ) -> Result<Pos, Diagnostic<'a>> {
        let token = self.advance(expected)?;
        match token.kind {
            TokenKind::Name(text) if text == keyword => Ok(token.pos),
            _ => Err(Self::unexpected(token, expected)),
        }
    }

    fn symbol(&mut self) -> Result<Name<'a>, Diagnostic<'a>> {
        const EXPECTED: &str = "a symbol name";
        let token = self.advance(EXPECTED)?;
        match token.kind {
            TokenKind::Name(text) if !KEYWORDS.contains(&text) => Ok(Name {
                text,
                pos: token.pos,
            }),
            _ => Err(Self::unexpected(token, EXPECTED)),
        }
    }

    fn number(&mut self) -> Result<u32, Diagnostic<'a>> {
        const EXPECTED: &str = "a number";
        let token = self.advance(EXPECTED)?;
        match token.kind {
            TokenKind::Number(n) => Ok(n),
            TokenKind::Name(_) => Err(Self::unexpected(token, EXPECTED)),
        }
    }

    /// 下一个记号是普通名字（不是关键字）时才取走它。用于读使用列表。
    fn optional_symbol(&mut self) -> Option<Name<'a>> {
        match self.peek()?.kind {
            TokenKind::Name(text) if !KEYWORDS.contains(&text) => {
                let pos = self.peek()?.pos;
                self.next += 1;
                Some(Name { text, pos })
            }
            _ => None,
        }
    }

    fn module(&mut self) -> Result<Module<'a>, Diagnostic<'a>> {
        self.keyword("module", "'module'")?;
        let name = self.symbol()?;
        let mut defs = Vec::new();
        let mut uses = Vec::new();

        loop {
            const EXPECTED: &str = "'def', 'use' or 'text'";
            let token = self.advance(EXPECTED)?;
            match token.kind {
                TokenKind::Name("def") => {
                    let name = self.symbol()?;
                    let offset = self.number()?;
                    defs.push(Def { name, offset });
                }
                TokenKind::Name("use") => {
                    while let Some(name) = self.optional_symbol() {
                        uses.push(name);
                    }
                }
                TokenKind::Name("text") => break,
                _ => return Err(Self::unexpected(token, EXPECTED)),
            }
        }

        let mut text = Vec::new();
        loop {
            const EXPECTED: &str = "an address mode (I, A, R or E) or 'end'";
            let token = self.advance(EXPECTED)?;
            let mode = match token.kind {
                TokenKind::Name("end") => break,
                TokenKind::Name(letter) => {
                    Mode::from_letter(letter).ok_or_else(|| Self::unexpected(token, EXPECTED))?
                }
                TokenKind::Number(_) => return Err(Self::unexpected(token, EXPECTED)),
            };
            let value = self.number()?;
            text.push(Word {
                mode,
                value,
                pos: token.pos,
            });
        }

        Ok(Module {
            name,
            defs,
            uses,
            text,
        })
    }
}

pub fn parse(tokens: Tokens<'_>) -> Result<Vec<Module<'_>>, Diagnostic<'_>> {
    let mut parser = Parser {
        tokens: tokens.tokens,
        next: 0,
        eof: tokens.eof,
    };
    let mut modules = Vec::new();
    while parser.peek().is_some() {
        modules.push(parser.module()?);
    }
    Ok(modules)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::toy::lex::tokenize;

    fn parse_str(src: &str) -> Result<Vec<Module<'_>>, Diagnostic<'_>> {
        parse(tokenize(src)?)
    }

    #[test]
    fn parses_a_module() {
        let modules = parse_str("module m def x 1 use y z text R 1004 E 2000 end").unwrap();
        let m = &modules[0];
        assert_eq!(m.name.text, "m");
        assert_eq!(m.defs[0].name.text, "x");
        assert_eq!(m.defs[0].offset, 1);
        let uses: Vec<&str> = m.uses.iter().map(|n| n.text).collect();
        assert_eq!(uses, ["y", "z"]);
        let words: Vec<(Mode, u32)> = m.text.iter().map(|w| (w.mode, w.value)).collect();
        assert_eq!(words, [(Mode::Relative, 1004), (Mode::External, 2000)]);
    }

    #[test]
    fn empty_input_has_no_modules() {
        assert_eq!(parse_str("# nothing here\n").unwrap(), []);
    }

    #[test]
    fn keywords_are_not_symbols() {
        let err = parse_str("module m def text 1").unwrap_err();
        assert_eq!(
            err.problem,
            Problem::Expected {
                expected: "a symbol name",
                found: Some("text")
            }
        );
    }

    #[test]
    fn reports_end_of_file_at_last_line() {
        let err = parse_str("module m\ntext I 1\n").unwrap_err();
        assert_eq!(err.pos, Pos { line: 2, col: 9 });
        assert!(matches!(err.problem, Problem::Expected { found: None, .. }));
    }
}

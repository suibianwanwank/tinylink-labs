//! 词法分析：把源文本切成带位置的记号。
//!
//! 记号之间用空白分隔，换行和空格没有区别；`#` 到行尾是注释。
//! 记号只有两类：全是数字的是数，以字母或下划线开头、后面是字母数字下划线的是名字。

use super::diag::{Diagnostic, Pos, Problem};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenKind<'a> {
    Number(u32),
    Name(&'a str),
}

/// 一个记号。`text` 是它在源文本里的原样切片，报错时原样引用。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Token<'a> {
    pub kind: TokenKind<'a>,
    pub text: &'a str,
    pub pos: Pos,
}

/// 词法分析的结果：记号序列，加上文件末尾的位置（用于"读到文件末尾"的报错）。
pub struct Tokens<'a> {
    pub tokens: Vec<Token<'a>>,
    pub eof: Pos,
}

fn is_name(text: &str) -> bool {
    let mut chars = text.chars();
    chars
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

fn classify(text: &str, pos: Pos) -> Result<Token<'_>, Diagnostic<'_>> {
    let kind = if text.bytes().all(|b| b.is_ascii_digit()) {
        let n = text
            .parse()
            .map_err(|_| Diagnostic::new(pos, Problem::NumberTooLarge { text }))?;
        TokenKind::Number(n)
    } else if is_name(text) {
        TokenKind::Name(text)
    } else {
        return Err(Diagnostic::new(pos, Problem::InvalidToken { text }));
    };
    Ok(Token { kind, text, pos })
}

/// 行号转成 u32。输入超过 40 亿行时饱和，不会发生在真实输入上。
pub(crate) fn to_u32(n: usize) -> u32 {
    u32::try_from(n).unwrap_or(u32::MAX)
}

pub fn tokenize(src: &str) -> Result<Tokens<'_>, Diagnostic<'_>> {
    let mut tokens = Vec::new();

    for (line_index, line) in src.lines().enumerate() {
        let line_no = to_u32(line_index + 1);
        let code = line.find('#').map_or(line, |i| &line[..i]);

        // start 记录当前记号的起点：(字节下标, 列号)。
        let mut start: Option<(usize, u32)> = None;
        for (col_index, (byte, ch)) in code.char_indices().enumerate() {
            if ch.is_whitespace() {
                if let Some((s, col)) = start.take() {
                    tokens.push(classify(&code[s..byte], Pos { line: line_no, col })?);
                }
            } else if start.is_none() {
                start = Some((byte, to_u32(col_index + 1)));
            }
        }
        if let Some((s, col)) = start {
            tokens.push(classify(&code[s..], Pos { line: line_no, col })?);
        }
    }

    // 文件以换行结尾时，末尾位置算在最后一行的行尾，编辑器能直接跳过去。
    let last_line = src.lines().last().unwrap_or("");
    let eof = Pos {
        line: to_u32(src.lines().count().max(1)),
        col: to_u32(last_line.chars().count() + 1),
    };
    Ok(Tokens { tokens, eof })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kinds(src: &str) -> Vec<TokenKind<'_>> {
        tokenize(src)
            .unwrap()
            .tokens
            .into_iter()
            .map(|t| t.kind)
            .collect()
    }

    #[test]
    fn splits_on_any_whitespace_and_skips_comments() {
        assert_eq!(
            kinds("module m # comment 99\n\tdef  x 3"),
            [
                TokenKind::Name("module"),
                TokenKind::Name("m"),
                TokenKind::Name("def"),
                TokenKind::Name("x"),
                TokenKind::Number(3),
            ]
        );
    }

    #[test]
    fn positions_count_characters_and_tabs_as_one_column() {
        let toks = tokenize("a\n\t b  c").unwrap().tokens;
        let pos: Vec<(u32, u32)> = toks.iter().map(|t| (t.pos.line, t.pos.col)).collect();
        assert_eq!(pos, [(1, 1), (2, 3), (2, 6)]);
    }

    #[test]
    fn eof_position_stays_on_last_line() {
        assert_eq!(tokenize("ab\ncd\n").unwrap().eof, Pos { line: 2, col: 3 });
        assert_eq!(tokenize("").unwrap().eof, Pos { line: 1, col: 1 });
    }

    #[test]
    fn rejects_bad_tokens() {
        let err = |src| tokenize(src).err().unwrap();
        assert_eq!(
            err("  12ab").problem,
            Problem::InvalidToken { text: "12ab" }
        );
        assert_eq!(err("  12ab").pos, Pos { line: 1, col: 3 });
        assert_eq!(
            err("99999999999").problem,
            Problem::NumberTooLarge {
                text: "99999999999"
            }
        );
    }
}

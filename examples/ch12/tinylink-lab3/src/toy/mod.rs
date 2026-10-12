//! M0：两遍玩具链接器。
//!
//! 输入是一种文本目标格式，机器是一台字寻址、500 个字、每字四位十进制数的玩具机器。
//! 设计借鉴 NYU 操作系统课 Lab 1（Hubertus Franke 版，前身是 Allan Gottlieb 的作业），
//! 格式和报错文本是本项目自己定的，规则对照见 README。

pub mod diag;
pub mod lex;
pub mod parse;
pub mod pass1;
pub mod pass2;

use std::collections::BTreeMap;
use std::fmt;

use diag::{Diagnostic, Pos, Problem, Severity};
use pass1::{Placement, Symbol};

/// 链接的产物：模块放在哪里、符号表、内存映像。
#[derive(Debug)]
pub struct Image<'a> {
    pub placements: Vec<Placement<'a>>,
    pub symbols: BTreeMap<&'a str, Symbol<'a>>,
    pub memory: Vec<u32>,
}

/// 名字列的宽度：至少 10 个字符，遇到更长的名字就按最长的来，保证各列对齐。
fn column_width<'n>(names: impl Iterator<Item = &'n str>) -> usize {
    names.map(|n| n.chars().count()).max().unwrap_or(0).max(10)
}

impl fmt::Display for Image<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let module_width = column_width(self.placements.iter().map(|p| p.name));
        let symbol_width = column_width(self.symbols.keys().copied());

        writeln!(f, "Modules")?;
        for (i, p) in self.placements.iter().enumerate() {
            writeln!(
                f,
                "  {:<2} {:<module_width$} base {:03}  size {}",
                i + 1,
                p.name,
                p.base,
                p.size
            )?;
        }
        writeln!(f, "Symbols")?;
        for (name, sym) in &self.symbols {
            writeln!(f, "  {name:<symbol_width$} {:03}  {}", sym.addr, sym.module)?;
        }
        writeln!(f, "Memory")?;
        for (addr, word) in self.memory.iter().enumerate() {
            writeln!(f, "  {addr:03}: {word:04}")?;
        }
        Ok(())
    }
}

/// 一次链接的全部结果。致命错误时 `image` 为 `None`。
pub struct Outcome<'a> {
    pub image: Option<Image<'a>>,
    pub diagnostics: Vec<Diagnostic<'a>>,
}

impl Outcome<'_> {
    pub fn has_errors(&self) -> bool {
        self.diagnostics
            .iter()
            .any(|d| d.severity() == Severity::Error)
    }
}

/// 两遍链接。能恢复的问题记进 `diags` 继续往下走，致命问题用 `?` 直接返回。
fn link_into<'a>(
    src: &'a str,
    diags: &mut Vec<Diagnostic<'a>>,
) -> Result<Image<'a>, Diagnostic<'a>> {
    let modules = parse::parse(lex::tokenize(src)?)?;
    let layout = pass1::run(&modules, diags)?;
    let memory = pass2::run(&modules, &layout, diags);
    Ok(Image {
        placements: layout.placements,
        symbols: layout.symbols,
        memory,
    })
}

/// 把输入字节解码成 UTF-8 文本。失败时按规则 1 报错，位置指向第一个非法字节。
pub fn decode(bytes: &[u8]) -> Result<&str, Diagnostic<'static>> {
    std::str::from_utf8(bytes).map_err(|e| {
        let valid = std::str::from_utf8(&bytes[..e.valid_up_to()]).expect("prefix is valid UTF-8");
        let line_start = valid.rfind('\n').map_or(0, |i| i + 1);
        let pos = Pos {
            line: lex::to_u32(valid.matches('\n').count() + 1),
            col: lex::to_u32(valid[line_start..].chars().count() + 1),
        };
        Diagnostic::new(pos, Problem::InvalidUtf8)
    })
}

pub fn link(src: &str) -> Outcome<'_> {
    let mut diagnostics = Vec::new();
    let image = match link_into(src, &mut diagnostics) {
        Ok(image) => Some(image),
        Err(fatal) => {
            diagnostics.push(fatal);
            None
        }
    };
    Outcome { image, diagnostics }
}

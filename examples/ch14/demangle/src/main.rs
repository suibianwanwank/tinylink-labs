//! 一个简化的 Itanium C++ demangler：只支持命名空间（含 St 和匿名命名空间）、NK 成员函数、
//! 内建类型、P/R/K 三种复合类型、S_/S<n>_ 替换，以及几种特殊名字：
//! _ZTV/_ZTI/_ZTS（vtable、typeinfo、typeinfo name）、_ZGV（guard variable）、
//! Z...E 局部实体。遇到别的语法（模板、构造析构函数、运算符等）就拒绝。
//! 用法：mini-demangle [符号...]；不给参数时从标准输入每行读一个符号。
//! 输出格式与 c++filt 相同；不支持的符号原样输出，并在标准错误上报告。
use std::io::{self, BufRead};

struct Parser<'a> {
    s: &'a [u8],
    pos: usize,
    subs: Vec<String>, // 替换字典，按出现顺序编号：S_ 是第 0 项，S0_ 是第 1 项……
}

type R<T> = Result<T, String>;

impl<'a> Parser<'a> {
    fn peek(&self) -> Option<u8> {
        self.s.get(self.pos).copied()
    }
    fn eat(&mut self, c: u8) -> bool {
        if self.peek() == Some(c) {
            self.pos += 1;
            true
        } else {
            false
        }
    }
    fn err<T>(&self, what: &str) -> R<T> {
        Err(format!("{what} at offset {}", self.pos))
    }

    // <source-name> ::= <正整数长度> <标识符>
    fn source_name(&mut self) -> R<String> {
        let start = self.pos;
        while matches!(self.peek(), Some(b'0'..=b'9')) {
            self.pos += 1;
        }
        let n: usize = std::str::from_utf8(&self.s[start..self.pos])
            .unwrap()
            .parse()
            .or_else(|_| self.err("expected <source-name>"))?;
        if n == 0 || self.pos + n > self.s.len() {
            return self.err("bad name length");
        }
        let name = String::from_utf8_lossy(&self.s[self.pos..self.pos + n]).into_owned();
        self.pos += n;
        // 匿名命名空间的名字是 _GLOBAL__N_ 加编号，demangler 一律显示成 (anonymous namespace)
        if name.starts_with("_GLOBAL__N") {
            return Ok("(anonymous namespace)".into());
        }
        Ok(name)
    }

    // <substitution> ::= S_ | S <seq-id> _ ，seq-id 是大写 36 进制，S0_ 指第 1 项
    fn substitution(&mut self) -> R<String> {
        // 调用时已经吃掉了 'S'
        if self.eat(b'_') {
            return self.subs.first().cloned().ok_or_else(|| "S_ with empty table".into());
        }
        let mut id = 0usize;
        loop {
            match self.peek() {
                Some(c @ b'0'..=b'9') => id = id * 36 + (c - b'0') as usize,
                Some(c @ b'A'..=b'Z') => id = id * 36 + (c - b'A') as usize + 10,
                Some(b'_') => break,
                _ => return self.err("bad <seq-id>"),
            }
            self.pos += 1;
        }
        self.pos += 1; // '_'
        self.subs.get(id + 1).cloned().ok_or_else(|| format!("S{id}_ out of range"))
    }

    // <nested-name> ::= N [K] <prefix> <unqualified-name> E
    // 返回 (完整名字, 是否 const 成员函数)。is_func 为真时最后一段不进替换字典。
    fn nested_name(&mut self, is_func: bool) -> R<(String, bool)> {
        let is_const = self.eat(b'K');
        let mut parts: Vec<String> = Vec::new();
        let mut cur = String::new();
        loop {
            if self.eat(b'E') {
                break;
            }
            if parts.is_empty() && self.s[self.pos..].starts_with(b"St") {
                self.pos += 2;
                cur = "std".into(); // St 是固定缩写，本身不进字典
                parts.push(cur.clone());
                continue;
            }
            if self.eat(b'S') {
                if !parts.is_empty() {
                    return self.err("substitution in the middle of a nested name");
                }
                cur = self.substitution()?;
                parts.push(cur.clone());
                continue;
            }
            let name = self.source_name()?;
            cur = if cur.is_empty() { name } else { format!("{cur}::{name}") };
            parts.push(cur.clone());
            // 先不急着进字典：要等看到下一个字符，才知道这一段是不是最后一段
            let last = self.peek() == Some(b'E');
            if !(is_func && last) {
                self.subs.push(cur.clone());
            }
        }
        if parts.is_empty() {
            return self.err("empty nested name");
        }
        Ok((cur, is_const))
    }

    fn builtin(c: u8) -> Option<&'static str> {
        Some(match c {
            b'v' => "void",
            b'w' => "wchar_t",
            b'b' => "bool",
            b'c' => "char",
            b'a' => "signed char",
            b'h' => "unsigned char",
            b's' => "short",
            b't' => "unsigned short",
            b'i' => "int",
            b'j' => "unsigned int",
            b'l' => "long",
            b'm' => "unsigned long",
            b'x' => "long long",
            b'y' => "unsigned long long",
            b'n' => "__int128",
            b'o' => "unsigned __int128",
            b'f' => "float",
            b'd' => "double",
            b'e' => "long double",
            b'g' => "__float128",
            b'z' => "...",
            _ => return None,
        })
    }

    // <type> ::= <builtin-type> | P <type> | R <type> | K <type>
    //          | <class-enum-type> | <substitution>
    fn ty(&mut self) -> R<String> {
        let c = self.peek().ok_or("unexpected end")?;
        if let Some(b) = Self::builtin(c) {
            self.pos += 1;
            return Ok(b.into()); // 内建类型不进字典
        }
        self.pos += 1;
        let t = match c {
            b'P' => format!("{}*", self.ty()?),
            b'R' => format!("{}&", self.ty()?),
            b'K' => format!("{} const", self.ty()?),
            b'N' => return Ok(self.nested_name(false)?.0), // 嵌套名的各段在里面已经进了字典
            b'S' if self.peek() == Some(b't') => {
                self.pos += 1;
                format!("std::{}", self.source_name()?)
            }
            b'S' => return self.substitution(), // 引用替换本身不再进字典
            b'0'..=b'9' => {
                self.pos -= 1;
                self.source_name()?
            }
            _ => return self.err("unsupported type"),
        };
        self.subs.push(t.clone());
        Ok(t)
    }

    // <name> ::= <nested-name> | <unscoped-name> | <local-name>
    // 返回 (名字, 是否 const 成员函数)。<encoding> 里的名字是函数名或数据名，最后一段不进字典。
    fn name(&mut self) -> R<(String, bool)> {
        match self.peek() {
            Some(b'N') => {
                self.pos += 1;
                self.nested_name(true)
            }
            Some(b'S') if self.s.get(self.pos + 1) == Some(&b't') => {
                self.pos += 2;
                Ok((format!("std::{}", self.source_name()?), false))
            }
            // <local-name> ::= Z <function encoding> E <entity name> [<discriminator>]
            Some(b'Z') => {
                self.pos += 1;
                let func = self.encoding()?;
                if !self.eat(b'E') {
                    return self.err("expected E after local-name encoding");
                }
                let (entity, is_const) = self.name()?;
                // <discriminator> ::= _ <digit> | __ <number> _ ，c++filt 不显示它
                if self.eat(b'_') {
                    if self.eat(b'_') {
                        while matches!(self.peek(), Some(b'0'..=b'9')) { self.pos += 1; }
                        if !self.eat(b'_') { return self.err("bad discriminator"); }
                    } else if matches!(self.peek(), Some(b'0'..=b'9')) {
                        self.pos += 1;
                    } else {
                        return self.err("bad discriminator");
                    }
                }
                Ok((format!("{func}::{entity}"), is_const))
            }
            Some(b'0'..=b'9') => Ok((self.source_name()?, false)),
            _ => self.err("unsupported name"),
        }
    }

    // <encoding> ::= <name> <bare-function-type> | <name> | <special-name>
    // 参数列表在字符串末尾或局部名字的 E 处结束
    fn encoding(&mut self) -> R<String> {
        // <special-name> ::= TV <type> | TI <type> | TS <type> | GV <object name>
        for (tag, what) in [(&b"TV"[..], "vtable for "), (b"TI", "typeinfo for "),
                            (b"TS", "typeinfo name for "), (b"GV", "guard variable for ")] {
            if self.s[self.pos..].starts_with(tag) {
                self.pos += 2;
                let inner = if tag == b"GV" { self.name()?.0 } else { self.ty()? };
                return Ok(format!("{what}{inner}"));
            }
        }
        let (name, is_const) = self.name()?;
        if self.pos == self.s.len() || self.peek() == Some(b'E') {
            return Ok(name); // 数据名：变量没有类型
        }
        let mut params = Vec::new();
        while self.pos < self.s.len() && self.peek() != Some(b'E') {
            params.push(self.ty()?);
        }
        let list = if params.len() == 1 && params[0] == "void" {
            String::new()
        } else {
            params.join(", ")
        };
        Ok(format!("{name}({list}){}", if is_const { " const" } else { "" }))
    }

    // <mangled-name> ::= _Z <encoding>
    fn mangled(&mut self) -> R<String> {
        if !self.s.starts_with(b"_Z") {
            return self.err("not an Itanium name");
        }
        self.pos = 2;
        let out = self.encoding()?;
        if self.pos != self.s.len() {
            return self.err("trailing characters");
        }
        Ok(out)
    }
}

pub fn demangle(sym: &str) -> R<String> {
    let mut p = Parser { s: sym.as_bytes(), pos: 0, subs: Vec::new() };
    p.mangled()
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let syms: Vec<String> = if args.is_empty() {
        io::stdin().lock().lines().map_while(Result::ok).collect()
    } else {
        args
    };
    for s in syms {
        match demangle(&s) {
            Ok(d) => println!("{d}"),
            Err(e) => {
                eprintln!("unsupported: {s}: {e}");
                println!("{s}");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::demangle;
    #[test]
    fn substitutions() {
        assert_eq!(demangle("_ZN3geo4distERKNS_5PointES2_").unwrap(),
                   "geo::dist(geo::Point const&, geo::Point const&)");
        assert_eq!(demangle("_ZN3geo4moveERNS_5PointERKS0_").unwrap(),
                   "geo::move(geo::Point&, geo::Point const&)");
        assert_eq!(demangle("_ZNK3geo5Point4normEv").unwrap(), "geo::Point::norm() const");
        assert_eq!(demangle("_ZSt9terminatev").unwrap(), "std::terminate()");
        assert_eq!(demangle("_ZN3geo7counterE").unwrap(), "geo::counter");
        assert!(demangle("_Z7biggestIlET_S0_S0_").is_err()); // 模板不支持
    }
    #[test]
    fn special_names() {
        assert_eq!(demangle("_ZTV5Shape").unwrap(), "vtable for Shape");
        assert_eq!(demangle("_ZTI5Shape").unwrap(), "typeinfo for Shape");
        assert_eq!(demangle("_ZTS5Shape").unwrap(), "typeinfo name for Shape");
        assert_eq!(demangle("_ZTVN3geo5PointE").unwrap(), "vtable for geo::Point");
        assert_eq!(demangle("_ZTISt9exception").unwrap(), "typeinfo for std::exception");
        assert_eq!(demangle("_ZTVN10__cxxabiv117__class_type_infoE").unwrap(),
                   "vtable for __cxxabiv1::__class_type_info");
    }
    #[test]
    fn local_and_guard() {
        assert_eq!(demangle("_ZZ7next_idvE1n").unwrap(), "next_id()::n");
        assert_eq!(demangle("_ZGVZ6configvE5value").unwrap(), "guard variable for config()::value");
        assert_eq!(demangle("_ZZN3geo5Point4normEvE5cache").unwrap(), "geo::Point::norm()::cache");
        assert_eq!(demangle("_ZZNK3geo5Point4normEvE5cache_0").unwrap(),
                   "geo::Point::norm() const::cache");
        assert_eq!(demangle("_ZZ3fooRKN3geo5PointEE1v").unwrap(), "foo(geo::Point const&)::v");
        assert!(demangle("_ZZ3foovEs").is_err()); // 字符串字面量不支持
    }
}

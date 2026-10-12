// lseval：链接器脚本子集的求值器（ENTRY、SECTIONS、`.`、ALIGN、PROVIDE、ASSERT）
// 用法：lseval 脚本 输入清单
// 输入清单按命令行顺序排列，两类行：
//   SEC 文件 节名 大小 对齐 类型
//   SYM 文件 D|U 符号名
use std::collections::{HashMap, HashSet};
use std::process::exit;

#[derive(Clone)]
struct Tok { s: String, line: usize, col: usize }

enum Expr { Num(u64), Dot, Sym(Tok), Align(Box<Expr>), Bin(char, Box<Expr>, Box<Expr>) }

enum Cmd {
    SetDot(Expr),
    Assign { name: String, e: Expr, provide: bool },
    Assert { e: Expr, msg: String, at: Tok },
    Input { file: String, pats: Vec<String> },
    Out { name: String, body: Vec<Cmd> },
}

struct Input { file: String, name: String, size: u64, align: u64, placed: bool }

// 报告"文件:行:列: 消息"并退出
fn die(path: &str, t: &Tok, msg: &str) -> ! {
    eprintln!("{path}:{}:{}: {msg}", t.line, t.col);
    exit(1);
}

// 前进 n 个字符，同时维护行列号
fn adv(b: &[char], i: &mut usize, n: usize, line: &mut usize, col: &mut usize) {
    for _ in 0..n {
        if b[*i] == '\n' { *line += 1; *col = 1; } else { *col += 1; }
        *i += 1;
    }
}

fn tokenize(src: &str) -> Vec<Tok> {
    let b: Vec<char> = src.chars().collect();
    let word = |c: char| c.is_ascii_alphanumeric() || "_.*/$".contains(c);
    let (mut out, mut i, mut line, mut col) = (Vec::new(), 0, 1, 1);
    while i < b.len() {
        let (c, l0, c0) = (b[i], line, col);
        let n = if c.is_whitespace() { 1 }
            else if c == '/' && b.get(i + 1) == Some(&'*') {           // 注释
                let mut j = i + 2;
                while j + 1 < b.len() && !(b[j] == '*' && b[j + 1] == '/') { j += 1; }
                j + 2 - i
            }
            else if c == '"' { 2 + b[i + 1..].iter().position(|&x| x == '"').unwrap_or(b.len() - i - 2) }
            else if c == '=' && b.get(i + 1) == Some(&'=') { 2 }
            else if word(c) { b[i..].iter().position(|&x| !word(x)).unwrap_or(b.len() - i) }
            else { 1 };
        if !c.is_whitespace() && !(c == '/' && b.get(i + 1) == Some(&'*')) {
            out.push(Tok { s: b[i..i + n].iter().collect(), line: l0, col: c0 });
        }
        adv(&b, &mut i, n, &mut line, &mut col);
    }
    out
}

struct P<'a> { t: Vec<Tok>, i: usize, path: &'a str }
impl P<'_> {
    fn peek(&self) -> &str { self.t.get(self.i).map(|t| t.s.as_str()).unwrap_or("") }
    fn next(&mut self) -> Tok {
        match self.t.get(self.i) {
            Some(t) => { self.i += 1; t.clone() }
            None => { let last = self.t.last().unwrap().clone(); die(self.path, &last, "脚本意外结束") }
        }
    }
    fn eat(&mut self, s: &str) {
        let t = self.next();
        if t.s != s { die(self.path, &t, &format!("期望 `{s}`，遇到 `{}`", t.s)); }
    }
    fn primary(&mut self) -> Expr {
        let t = self.next();
        match t.s.as_str() {
            "." => Expr::Dot,
            "(" => { let e = self.expr(); self.eat(")"); e }
            "ALIGN" => { self.eat("("); let e = self.expr(); self.eat(")"); Expr::Align(Box::new(e)) }
            s if s.starts_with("0x") => match u64::from_str_radix(&s[2..], 16) {
                Ok(n) => Expr::Num(n), Err(_) => die(self.path, &t, "不是合法的十六进制数"),
            },
            s if s.starts_with(|c: char| c.is_ascii_digit()) => match s.parse() {
                Ok(n) => Expr::Num(n), Err(_) => die(self.path, &t, "不是合法的数"),
            },
            s if s.starts_with(|c: char| c.is_ascii_alphabetic() || "_.$".contains(c)) => Expr::Sym(t),
            _ => die(self.path, &t, &format!("这里需要一个表达式，遇到 `{}`", t.s)),
        }
    }
    fn additive(&mut self) -> Expr {
        let mut l = self.primary();
        while self.peek() == "+" || self.peek() == "-" {
            let op = self.next().s.chars().next().unwrap();
            l = Expr::Bin(op, Box::new(l), Box::new(self.primary()));
        }
        l
    }
    fn expr(&mut self) -> Expr {
        let l = self.additive();
        if self.peek() == "==" { self.next(); return Expr::Bin('=', Box::new(l), Box::new(self.additive())); }
        l
    }
    // 一条命令；inside 表示在输出节的花括号里
    fn cmd(&mut self, inside: bool) -> Cmd {
        let t = self.next();
        match t.s.as_str() {
            "." => { self.eat("="); let e = self.expr(); self.eat(";"); Cmd::SetDot(e) }
            "PROVIDE" => {
                self.eat("("); let name = self.next().s; self.eat("=");
                let e = self.expr(); self.eat(")"); self.eat(";");
                Cmd::Assign { name, e, provide: true }
            }
            "ASSERT" => {
                self.eat("("); let e = self.expr(); self.eat(",");
                let msg = self.next().s.trim_matches('"').to_string(); self.eat(")");
                if self.peek() == ";" { self.next(); }
                Cmd::Assert { e, msg, at: t }
            }
            _ if self.peek() == "=" => { self.next(); let e = self.expr(); self.eat(";"); Cmd::Assign { name: t.s, e, provide: false } }
            _ if inside && self.peek() == "(" => {
                self.next();
                let mut pats = Vec::new();
                while self.peek() != ")" { pats.push(self.next().s); }
                self.next();
                Cmd::Input { file: t.s, pats }
            }
            _ if !inside && self.peek() == ":" => {
                self.next(); self.eat("{");
                let mut body = Vec::new();
                while self.peek() != "}" { body.push(self.cmd(true)); }
                self.next();
                Cmd::Out { name: t.s, body }
            }
            _ => die(self.path, &t, &format!("不支持的语法 `{}`", t.s)),
        }
    }
}

fn glob(p: &str, s: &str) -> bool {
    match p.find('*') {
        None => p == s,
        Some(k) => {
            let (pre, rest) = (&p[..k], &p[k + 1..]);
            s.starts_with(pre) && (0..=s.len() - pre.len()).any(|j| glob(rest, &s[pre.len() + j..]))
        }
    }
}

fn align_up(x: u64, a: u64) -> u64 { if a <= 1 { x } else { x.div_ceil(a) * a } }

struct Ctx<'a> {
    path: &'a str,
    dot: u64,
    syms: HashMap<String, u64>,
    inputs: Vec<Input>,
    needed: HashSet<String>,  // 被引用却没有任何输入文件定义的符号
}

impl Ctx<'_> {
    fn eval(&self, e: &Expr) -> u64 {
        match e {
            Expr::Num(n) => *n,
            Expr::Dot => self.dot,
            Expr::Sym(t) => *self.syms.get(&t.s)
                .unwrap_or_else(|| die(self.path, t, &format!("符号 `{}` 尚未定义", t.s))),
            Expr::Align(a) => align_up(self.dot, self.eval(a)),
            Expr::Bin(op, l, r) => {
                let (l, r) = (self.eval(l), self.eval(r));
                match op { '+' => l.wrapping_add(r), '-' => l.wrapping_sub(r), _ => (l == r) as u64 }
            }
        }
    }
    fn matches(inp: &Input, file: &str, pats: &[String]) -> bool {
        !inp.placed && glob(file, &inp.file) && pats.iter().any(|p| glob(p, &inp.name))
    }
    fn run(&mut self, cmds: &[Cmd]) {
        for c in cmds {
            match c {
                Cmd::SetDot(e) => self.dot = self.eval(e),
                // PROVIDE 只在符号被引用、又没有任何输入文件定义时生效
                Cmd::Assign { name, e, provide } => {
                    if !*provide || self.needed.contains(name) {
                        let v = self.eval(e);
                        self.syms.insert(name.clone(), v);
                    }
                }
                Cmd::Assert { e, msg, at } => if self.eval(e) == 0 { die(self.path, at, &format!("ASSERT 失败：{msg}")) },
                Cmd::Input { file, pats } => {
                    for k in 0..self.inputs.len() {
                        if Self::matches(&self.inputs[k], file, pats) {
                            let inp = &mut self.inputs[k];
                            inp.placed = true;
                            self.dot = align_up(self.dot, inp.align) + inp.size;
                        }
                    }
                }
                Cmd::Out { name, body } => {
                    // 输出节的起点对齐到它所有输入节里最大的对齐
                    let mut max_al = 1;
                    for b in body {
                        if let Cmd::Input { file, pats } = b {
                            for inp in &self.inputs {
                                if Self::matches(inp, file, pats) { max_al = max_al.max(inp.align); }
                            }
                        }
                    }
                    self.dot = align_up(self.dot, max_al);
                    let start = self.dot;
                    self.run(body);
                    println!("{:<10} 0x{:x} 大小 0x{:x}", name, start, self.dot - start);
                }
            }
        }
    }
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    if a.len() != 3 { eprintln!("用法：lseval 脚本 输入清单"); exit(2); }
    let script = std::fs::read_to_string(&a[1]).unwrap_or_else(|e| { eprintln!("{}: {e}", a[1]); exit(2) });
    let mut p = P { t: tokenize(&script), i: 0, path: &a[1] };
    let (mut cmds, mut entry) = (Vec::new(), None);
    while p.i < p.t.len() {
        let t = p.next();
        match t.s.as_str() {
            "OUTPUT_ARCH" => { p.eat("("); p.next(); p.eat(")"); }      // 只认 riscv，忽略
            "ENTRY" => { p.eat("("); entry = Some(p.next().s); p.eat(")"); }
            "SECTIONS" => { p.eat("{"); while p.peek() != "}" { cmds.push(p.cmd(false)); } p.next(); }
            _ => die(&a[1], &t, &format!("不支持的顶层命令 `{}`", t.s)),
        }
    }
    let list = std::fs::read_to_string(&a[2]).unwrap_or_else(|e| { eprintln!("{}: {e}", a[2]); exit(2) });
    let (mut inputs, mut defined, mut undef) = (Vec::new(), HashSet::new(), HashSet::new());
    for l in list.lines() {
        let f: Vec<&str> = l.split_whitespace().collect();
        match f.as_slice() {
            ["SEC", file, name, size, align, _] => inputs.push(Input {
                file: file.to_string(), name: name.to_string(),
                size: size.parse().unwrap(), align: align.parse().unwrap(), placed: false }),
            ["SYM", _, "D", s] => { defined.insert(s.to_string()); }
            ["SYM", _, "U", s] => { undef.insert(s.to_string()); }
            _ => { eprintln!("{}: 无法识别的行 `{l}`", a[2]); exit(2) }
        }
    }
    let needed = undef.difference(&defined).cloned().collect();
    let mut cx = Ctx { path: &a[1], dot: 0, syms: HashMap::new(), inputs, needed };
    cx.run(&cmds);
    println!("入口 {}", entry.as_deref().unwrap_or("（脚本未指定）"));
    let mut syms: Vec<_> = cx.syms.iter().collect();
    syms.sort_by_key(|(_, v)| **v);
    for (k, v) in syms { println!("符号 {k} = 0x{v:x}"); }
    for inp in cx.inputs.iter().filter(|i| !i.placed && i.size > 0) {
        println!("孤儿节 {} {}（大小 {}）", inp.file, inp.name, inp.size);
    }
}

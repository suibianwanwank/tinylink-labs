//! 数据驱动的测试：`tests/<套件>/{golden,errors}/` 下每个输入文件都是一个用例。
//! 每个套件有自己的输入扩展名，比如 `toy` 套件是 `.tl`，`link` 套件是 `.o`。
//!
//! 一个用例由三部分组成：
//! - 输入文件。文本输入（`.tl`）的第一行必须声明预期结果，三种写法之一：
//!   `# expect: exit N`、`# expect: error "文本"`、`# expect: warning "文本"`。
//!   第二行可以写 `# args: 子命令 参数...`，代替套件默认的命令行参数，输入文件名总是放在最后。
//!   二进制输入（比如 `.o`）没法写注释，这两行放在同名的 `NAME.expect` 文件里。
//!   有多个输入的套件（`resolve`）以 `NAME.expect` 本身为用例，`# args:` 写出完整的命令行，
//!   不再追加文件名；它引用的 `.o` 文件和 `.expect` 放在同一个目录里。
//! - `NAME.out`：期望的 stdout。比较时忽略空白差异，效果相当于 `diff -b -B -E`。
//! - `NAME.err`：期望的 stderr。逐字节比较。
//!
//! 缺少 `.out` 表示 stdout 必须为空，缺少 `.err` 表示 stderr 必须为空。
//! `golden/` 下的用例必须有 `.out`，`errors/` 下的用例必须有 `.err`。
//!
//! 环境变量：
//! - `TINYLINK_CASE=子串`：只跑路径里含这个子串的用例。一个都没匹配上算失败。
//! - `TINYLINK_BLESS=1`：用实际输出覆盖 `.out`/`.err`。bless 之后要逐个检查差异再提交。

use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::process::Command;

/// 一个测试套件。
struct Suite {
    /// `tests/` 下的目录名。
    dir: &'static str,
    /// 输入文件的扩展名。
    ext: &'static str,
    /// 默认放在输入文件名前面的命令行参数，用例可以用 `# args:` 换掉。
    args: &'static [&'static str],
}

const SUITES: &[Suite] = &[
    Suite {
        dir: "toy",
        ext: "tl",
        args: &["toy"],
    },
    Suite {
        dir: "elf",
        ext: "o",
        args: &["readelf", "-S"],
    },
    Suite {
        dir: "link",
        ext: "o",
        args: &["link", "-o", "out.tmp"],
    },
    Suite {
        dir: "resolve",
        ext: "expect",
        args: &[],
    },
];

/// 测试文件第一行声明的预期结果。
#[derive(Debug, PartialEq)]
enum Expect {
    /// 精确的退出码。
    Exit(i32),
    /// 退出码为 1，并且 stderr 中有一行包含 `error: 文本`。
    Error(String),
    /// 退出码为 0，并且 stderr 中有一行包含 `warning: 文本`。
    Warning(String),
}

impl Expect {
    fn parse(first_line: &str) -> Result<Expect, String> {
        let rest = first_line
            .trim()
            .strip_prefix("# expect:")
            .ok_or_else(|| format!("first line must start with `# expect:`, got {first_line:?}"))?
            .trim();
        let (kind, arg) = rest.split_once(' ').unwrap_or((rest, ""));
        let quoted = || {
            arg.trim()
                .strip_prefix('"')
                .and_then(|s| s.strip_suffix('"'))
                .map(str::to_owned)
                .ok_or_else(|| format!("expected a quoted string after `{kind}`, got {arg:?}"))
        };
        match kind {
            "exit" => arg
                .trim()
                .parse()
                .map(Expect::Exit)
                .map_err(|e| format!("bad exit code {arg:?}: {e}")),
            "error" => quoted().map(Expect::Error),
            "warning" => quoted().map(Expect::Warning),
            other => Err(format!("unknown expectation `{other}`")),
        }
    }

    fn exit_code(&self) -> i32 {
        match self {
            Expect::Exit(code) => *code,
            Expect::Error(_) => 1,
            Expect::Warning(_) => 0,
        }
    }

    /// 声明里要求 stderr 出现的那段文本（如果有）。
    fn stderr_needle(&self) -> Option<String> {
        match self {
            Expect::Exit(_) => None,
            Expect::Error(text) => Some(format!("error: {text}")),
            Expect::Warning(text) => Some(format!("warning: {text}")),
        }
    }
}

/// 把一行里连续的空白压成一个空格，去掉行尾空白（`diff -b` 与 `-E`）。
fn squeeze(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut in_space = false;
    for ch in line.trim_end().chars() {
        if ch.is_whitespace() {
            if !in_space {
                out.push(' ');
            }
            in_space = true;
        } else {
            out.push(ch);
            in_space = false;
        }
    }
    out
}

/// 规范化整段文本：逐行压空白，再丢掉空行（`diff -B`）。
fn normalize(text: &str) -> Vec<String> {
    text.lines()
        .map(squeeze)
        .filter(|line| !line.is_empty())
        .collect()
}

/// 找到第一处不同，给出便于定位的说明。用于规范化之后的 stdout。
fn describe_mismatch(what: &str, expected: &[String], actual: &[String]) -> String {
    let at = expected
        .iter()
        .zip(actual)
        .position(|(e, a)| e != a)
        .unwrap_or(expected.len().min(actual.len()));
    let show = |lines: &[String]| {
        lines
            .get(at)
            .map_or("<end of output>".to_owned(), |l| format!("{l:?}"))
    };
    format!(
        "{what} differs at line {}:\n  expected: {}\n  actual:   {}",
        at + 1,
        show(expected),
        show(actual)
    )
}

/// 逐字节比较。不同时按行（保留行尾的 `\n` 或 `\r\n`）找第一处差异，
/// 这样"末尾少一个换行"和"CRLF 对 LF"也能指出来。
fn compare_exact(what: &str, expected: &str, actual: &str) -> Option<String> {
    if expected == actual {
        return None;
    }
    let e: Vec<&str> = expected.split_inclusive('\n').collect();
    let a: Vec<&str> = actual.split_inclusive('\n').collect();
    let at = (0..e.len().max(a.len())).find(|&i| e.get(i) != a.get(i));
    let show = |lines: &[&str], i: usize| {
        lines
            .get(i)
            .map_or("<end of output>".to_owned(), |l| format!("{l:?}"))
    };
    Some(match at {
        Some(i) => format!(
            "{what} differs at line {}:\n  expected: {}\n  actual:   {}",
            i + 1,
            show(&e, i),
            show(&a, i)
        ),
        // 理论上走不到：内容不同就一定有一行不同。保险起见打印全部字节。
        None => format!("{what} differs:\n  expected: {expected:?}\n  actual:   {actual:?}"),
    })
}

struct Case {
    input: PathBuf,
    kind: &'static str,
    suite: &'static Suite,
}

impl Case {
    fn sidecar(&self, ext: &str) -> PathBuf {
        self.input.with_extension(ext)
    }

    fn name(&self) -> String {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests");
        self.input
            .strip_prefix(&root)
            .unwrap_or(&self.input)
            .display()
            .to_string()
    }
}

fn collect_cases() -> Vec<Case> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests");
    let mut cases = Vec::new();
    for suite in SUITES {
        for kind in ["golden", "errors"] {
            let dir = root.join(suite.dir).join(kind);
            // 目录不存在说明这个套件还没有这类用例；其他读目录的错误必须让测试失败。
            let entries = match fs::read_dir(&dir) {
                Ok(entries) => entries,
                Err(e) if e.kind() == ErrorKind::NotFound => continue,
                Err(e) => panic!("cannot read {}: {e}", dir.display()),
            };
            let mut inputs: Vec<PathBuf> = entries
                .map(|e| e.expect("read directory entry").path())
                .filter(|p| p.extension().is_some_and(|ext| ext == suite.ext))
                .collect();
            // read_dir 的顺序由文件系统决定，排序后每次运行顺序一致。
            inputs.sort();
            cases.extend(inputs.into_iter().map(|input| Case { input, kind, suite }));
        }
    }
    cases
}

/// 读取可选的期望文件；不存在时视为空。
fn read_expected(path: &Path) -> String {
    fs::read_to_string(path).unwrap_or_default()
}

/// 用实际输出覆盖期望文件。输出为空时删掉可选的期望文件；
/// `required` 的期望文件（golden 的 `.out`、errors 的 `.err`）即使为空也保留。
fn bless(path: &Path, actual: &str, required: bool) {
    if actual.is_empty() && !required {
        match fs::remove_file(path) {
            Ok(()) => {}
            Err(e) if e.kind() == ErrorKind::NotFound => {}
            Err(e) => panic!("cannot remove {}: {e}", path.display()),
        }
    } else {
        fs::write(path, actual).expect("write blessed output");
    }
}

/// 用例开头的声明：预期结果，以及可选的命令行参数。
fn read_header(case: &Case) -> Result<(Expect, Vec<String>), String> {
    let expect_file = case.sidecar("expect");
    let path = if case.suite.ext == "tl" {
        &case.input
    } else {
        &expect_file
    };
    // 按字节读：有的用例故意含非法 UTF-8，声明行只要是 UTF-8 就能解析。
    let source = fs::read(path).map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    let source = String::from_utf8_lossy(&source);
    let mut lines = source.lines();
    let expect = Expect::parse(lines.next().unwrap_or(""))?;
    let args = match lines.next().and_then(|l| l.trim().strip_prefix("# args:")) {
        Some(args) => args.split_whitespace().map(str::to_owned).collect(),
        None => case.suite.args.iter().map(|&a| a.to_owned()).collect(),
    };
    Ok((expect, args))
}

fn run_case(case: &Case) -> Result<(), String> {
    let (expect, args) = read_header(case)?;

    let out_path = case.sidecar("out");
    let err_path = case.sidecar("err");
    // 在用例所在目录里用相对文件名运行，报错里的路径就和机器无关。
    let dir = case.input.parent().expect("case has a parent dir");
    let file = case.input.file_name().expect("case has a file name");
    let mut command = Command::new(env!("CARGO_BIN_EXE_tinylink"));
    command.args(&args).current_dir(dir);
    // 以 .expect 为用例的套件，命令行在 `# args:` 里写全了。
    if case.suite.ext != "expect" {
        command.arg(file);
    }
    let output = command
        .output()
        .map_err(|e| format!("cannot run tinylink: {e}"))?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    if std::env::var_os("TINYLINK_BLESS").is_some() {
        bless(&out_path, &stdout, case.kind == "golden");
        bless(&err_path, &stderr, case.kind == "errors");
    }

    // 放在 bless 之后：新写的用例可以先 bless 生成期望文件。
    match case.kind {
        "golden" if !out_path.exists() => return Err("golden case needs a .out file".into()),
        "errors" if !err_path.exists() => return Err("error case needs a .err file".into()),
        _ => {}
    }

    let mut problems = Vec::new();

    let code = output.status.code().unwrap_or(-1);
    if code != expect.exit_code() {
        problems.push(format!(
            "exit code: expected {}, got {code}",
            expect.exit_code()
        ));
    }

    if let Some(needle) = expect.stderr_needle()
        && !stderr.lines().any(|line| line.contains(&needle))
    {
        problems.push(format!("stderr has no line containing {needle:?}"));
    }

    let expected_out = normalize(&read_expected(&out_path));
    let actual_out = normalize(&stdout);
    if expected_out != actual_out {
        problems.push(describe_mismatch("stdout", &expected_out, &actual_out));
    }

    if let Some(why) = compare_exact("stderr", &read_expected(&err_path), &stderr) {
        problems.push(why);
    }

    if problems.is_empty() {
        Ok(())
    } else {
        Err(problems.join("\n"))
    }
}

#[test]
fn data_driven_cases() {
    let filter = std::env::var("TINYLINK_CASE").unwrap_or_default();
    let cases: Vec<Case> = collect_cases()
        .into_iter()
        .filter(|c| c.name().contains(&filter))
        .collect();
    // 过滤条件写错时一个用例都不跑，不能当作通过。
    assert!(
        !cases.is_empty(),
        "no case matches TINYLINK_CASE={filter:?}"
    );

    let mut failures = Vec::new();
    for case in &cases {
        match run_case(case) {
            Ok(()) => eprintln!("case {} ... ok", case.name()),
            Err(why) => {
                eprintln!("case {} ... FAILED", case.name());
                failures.push(format!("--- {}\n{why}", case.name()));
            }
        }
    }
    eprintln!("{} cases, {} failed", cases.len(), failures.len());
    assert!(failures.is_empty(), "\n{}", failures.join("\n\n"));
}

// 下面是测试框架自己的测试：比较规则写错了，所有用例的结论都不可信。

#[test]
fn expect_line_forms() {
    assert_eq!(Expect::parse("# expect: exit 0"), Ok(Expect::Exit(0)));
    assert_eq!(Expect::parse("  # expect:   exit 2  "), Ok(Expect::Exit(2)));
    assert_eq!(
        Expect::parse(r#"# expect: error "symbol 'x' is undefined""#),
        Ok(Expect::Error("symbol 'x' is undefined".into()))
    );
    assert_eq!(
        Expect::parse(r#"# expect: warning "unused""#),
        Ok(Expect::Warning("unused".into()))
    );
    assert!(Expect::parse("module a").is_err());
    assert!(Expect::parse("# expect: error unquoted").is_err());
    assert!(Expect::parse("# expect: crash").is_err());
}

#[test]
fn whitespace_rules_match_diff_b_bb_e() {
    // -b：空白的多少不算差异，行尾空白也不算。
    assert_eq!(normalize("a  b\t c  \n"), normalize("a b c\n"));
    // -b 不忽略"有"和"没有"空白的区别：行首缩进仍然算数。
    assert_ne!(normalize("  a\n"), normalize("a\n"));
    // -B：空行和只含空白的行不算差异。
    assert_eq!(normalize("a\n\n   \nb\n"), normalize("a\nb"));
    // 但内容不同仍然不同。
    assert_ne!(normalize("000: 1004\n"), normalize("000: 1005\n"));
}

#[test]
fn exact_compare_reports_line_endings() {
    assert_eq!(compare_exact("stderr", "a\n", "a\n"), None);
    // 末尾少一个换行。
    let why = compare_exact("stderr", "a\n", "a").unwrap();
    assert!(
        why.contains(r#"expected: "a\n""#) && why.contains(r#"actual:   "a""#),
        "{why}"
    );
    // CRLF 对 LF。
    let why = compare_exact("stderr", "a\nb\n", "a\r\nb\n").unwrap();
    assert!(
        why.contains("line 1") && why.contains(r#""a\r\n""#),
        "{why}"
    );
    // 多出一行。
    let why = compare_exact("stderr", "a\n", "a\nb\n").unwrap();
    assert!(
        why.contains("line 2") && why.contains("<end of output>"),
        "{why}"
    );
}

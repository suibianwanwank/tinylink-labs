//! 拿 `tinylink resolve` 的结论和 ld.lld 21、GNU ld 2.45 的实际链接结果比较。
//!
//! `tests/resolve/gen.sh` 对 `tests/resolve/{golden,errors}` 下每个 `resolve` 用例，用两个
//! 链接器链接同一组输入，把结论写进 `tests/resolve/linkers/NAME.{lld,gnu}`：
//!
//! ```text
//! status 0                 链接成功是 0，失败是 1
//! error duplicate n        报了哪些重复定义、未定义（从 stderr 里摘出名字）
//! counter w3.o .data       每个名字最后用了谁：映射文件（-Map）里它所在的输入节
//! buf c8.o COMMON          COMMON 符号分配在哪个输入的 COMMON 里
//! K *ABS* 0x2a             绝对符号的值（来自产物的 nm）
//! hook UNDEF-WEAK          没有定义的弱引用（产物 nm 里的 w）
//! missing UNDEF            没有定义、也没有被用到的强引用（产物 nm 里的 U）
//! end SYNTHETIC            输入里没有定义、产物里有：链接器合成的
//! ```
//!
//! 这里从 tinylink 的输出算出同样格式的结论。两个链接器意见一致时，tinylink 必须和它们一样；
//! 不一致时，tinylink 至少要和其中一个一样。不一致的用例列在 `DISAGREE` 里，
//! 每一处都在实验 3 的文章里讨论过，列表变了说明链接器或用例变了，要重新看一遍。

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::Command;

/// 两个链接器结论不同的用例，以及 tinylink 跟的是哪一个。
const DISAGREE: &[(&str, &str)] = &[
    // 两个值都是 0 的绝对符号：lld 报重复定义，GNU ld 不报。lld 判断"值相同"的条件里
    // 多了一个 `errOffset &&`，值为 0 时这个例外不生效。
    ("absolute-zero", "gnu"),
    // lld 遇到重复定义就停下，不再扫描重定位，所以没报 f 未定义；GNU ld 两个都报。
    ("all-at-once", "gnu"),
    // 强定义（汇编里没写 .size，大小是 0）压过 64 字节的 COMMON：GNU ld 让这个定义沿用
    // COMMON 的大小 64，lld 保留定义自己的 0。两种顺序都一样。
    ("common-then-strong", "lld"),
    // 唯一的引用在不可分配的 .debug_info 里：lld 不报未定义，GNU ld 报。
    ("debug-only-reference", "lld"),
    // hidden 的强引用，没有定义也没有重定位用到：GNU ld 报 "hidden symbol `h' isn't defined"，lld 不报。
    ("hidden-unreferenced", "lld"),
    ("strong-then-common", "lld"),
    // 没有 .bss 时，lld 把 __bss_start 定义成绝对符号 0；GNU ld 定义成段内的地址。
    ("synthetic", "gnu"),
    // 没被用到的未定义符号：lld 留在产物的符号表里（U），GNU ld 不留。
    ("unused-undefined", "lld"),
    // 弱的 COMMON 先到、弱定义后到：lld 用 COMMON；GNU ld 2.45 在这里触发了 BFD 的内部错误，链接失败。
    ("weak-common-then-weak", "lld"),
    // 弱定义先到、弱的 COMMON 后到：lld 用 COMMON，GNU ld 保留弱定义。
    ("weak-then-weak-common", "lld"),
    // 没有定义的弱引用：lld 留在产物的符号表里（w），GNU ld 不留。
    ("weak-undefined", "lld"),
];

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/resolve")
}

/// 一个 `resolve` 用例：名字、所在目录和输入文件。
struct Case {
    name: String,
    dir: PathBuf,
    inputs: Vec<String>,
}

fn cases() -> Vec<Case> {
    let mut out = Vec::new();
    for kind in ["golden", "errors"] {
        let dir = root().join(kind);
        let mut paths: Vec<PathBuf> = std::fs::read_dir(&dir)
            .expect("read case dir")
            .map(|e| e.expect("dir entry").path())
            .filter(|p| p.extension().is_some_and(|e| e == "expect"))
            .collect();
        paths.sort();
        for path in paths {
            let text = std::fs::read_to_string(&path).expect("read .expect");
            let mut lines = text.lines().skip(1);
            let args: Vec<String> = lines
                .next()
                .and_then(|l| l.strip_prefix("# args:"))
                .expect("second line is # args:")
                .split_whitespace()
                .map(str::to_owned)
                .collect();
            let skip = lines
                .next()
                .is_some_and(|l| l.starts_with("# linkers: skip"));
            if args.first().map(String::as_str) != Some("resolve") || skip {
                continue;
            }
            out.push(Case {
                name: path.file_stem().unwrap().to_string_lossy().into_owned(),
                dir: dir.clone(),
                inputs: args[1..].to_vec(),
            });
        }
    }
    out
}

/// 从 `tinylink resolve` 的输出算出结论，格式见文件开头。
fn tinylink_verdict(case: &Case) -> String {
    let output = Command::new(env!("CARGO_BIN_EXE_tinylink"))
        .arg("resolve")
        .args(&case.inputs)
        .current_dir(&case.dir)
        .output()
        .expect("run tinylink");
    let stdout = String::from_utf8(output.stdout).expect("utf-8 stdout");
    let stderr = String::from_utf8(output.stderr).expect("utf-8 stderr");
    let ok = output.status.success();

    let mut lines = vec![format!("status {}", if ok { 0 } else { 1 })];
    let mut errors = BTreeSet::new();
    for line in stderr.lines() {
        if let Some(name) = line.strip_prefix("tinylink: error: duplicate symbol: ") {
            errors.insert(format!("error duplicate {name}"));
        } else if let Some(rest) = line.strip_prefix("tinylink: error: undefined ")
            && let Some((_, name)) = rest.split_once("symbol: ")
        {
            // "undefined symbol: f"，或者带可见性的 "undefined hidden symbol: h"。
            errors.insert(format!("error undefined {name}"));
        }
    }
    lines.extend(errors);

    // 链接失败时链接器不产出映射文件，只比较报错。
    if ok {
        let mut result = Vec::new();
        let mut name = None;
        for line in stdout.lines() {
            if !line.starts_with(' ') {
                name = Some(line.to_owned());
                continue;
            }
            let Some(verdict) = line.strip_prefix("  => ") else {
                continue;
            };
            let name = name.clone().expect("verdict follows a name");
            // 结论的各项用 ", " 分开；可见性（", HIDDEN"）链接器的结论里没有，丢掉。
            let parts: Vec<&str> = verdict.split(", ").collect();
            let field = |key: &str| {
                parts
                    .iter()
                    .find_map(|p| p.strip_prefix(key))
                    .unwrap_or("?")
                    .to_owned()
            };
            let head = parts[0];
            let where_ = if let Some(file) = head.strip_prefix("COMMON from ") {
                format!(
                    "{file} COMMON size {} align {}",
                    field("size "),
                    field("align ")
                )
            } else if let Some((_, value)) = head.split_once(": absolute ") {
                format!("*ABS* {value}")
            } else if head.starts_with("undefined weak") {
                "UNDEF-WEAK".to_owned()
            } else if head == "undefined" {
                "UNDEF".to_owned()
            } else if head.starts_with("defined by the linker") {
                "SYNTHETIC".to_owned()
            } else {
                // a.o:(.data+0x0), size 4
                let (file, rest) = head.split_once(":(").expect("file:(section+offset)");
                let section = rest.rsplit_once('+').expect("section+offset").0;
                format!("{file} {section} size {}", field("size "))
            };
            result.push(format!("{name} {where_}"));
        }
        result.sort();
        lines.extend(result);
    }
    lines.join("\n")
}

/// 两份结论是否一致。链接器的结论里 `?` 表示它给不出这一项（GNU ld 的映射文件不写对齐），
/// 和任何值都算一致。
fn same(linker: &str, ours: &str) -> bool {
    let (a, b): (Vec<&str>, Vec<&str>) = (linker.lines().collect(), ours.lines().collect());
    a.len() == b.len()
        && a.iter().zip(&b).all(|(x, y)| {
            let (xs, ys): (Vec<&str>, Vec<&str>) = (x.split(' ').collect(), y.split(' ').collect());
            xs.len() == ys.len() && xs.iter().zip(&ys).all(|(p, q)| *p == "?" || p == q)
        })
}

#[test]
fn resolution_agrees_with_real_linkers() {
    let cases = cases();
    assert!(cases.len() >= 30, "only {} resolve cases", cases.len());
    let mut problems = Vec::new();
    let mut disagreements = Vec::new();
    for case in &cases {
        let read = |ext: &str| {
            let path = root().join("linkers").join(format!("{}.{ext}", case.name));
            std::fs::read_to_string(&path)
                .unwrap_or_else(|e| panic!("{}: {e}; run tests/resolve/gen.sh", path.display()))
                .trim_end()
                .to_owned()
        };
        let (lld, gnu) = (read("lld"), read("gnu"));
        let ours = tinylink_verdict(case);
        if same(&lld, &gnu) || same(&gnu, &lld) {
            if !same(&lld, &ours) {
                problems.push(format!(
                    "{}: linkers say\n{lld}\ntinylink says\n{ours}",
                    case.name
                ));
            }
            continue;
        }
        let follows = if same(&lld, &ours) {
            "lld"
        } else if same(&gnu, &ours) {
            "gnu"
        } else {
            problems.push(format!(
                "{}: lld says\n{lld}\nGNU ld says\n{gnu}\ntinylink says\n{ours}",
                case.name
            ));
            continue;
        };
        disagreements.push((case.name.clone(), follows));
    }
    assert!(problems.is_empty(), "\n{}", problems.join("\n\n"));
    disagreements.sort();
    let expected: Vec<(String, &str)> = DISAGREE
        .iter()
        .map(|&(name, follows)| (name.to_owned(), follows))
        .collect();
    assert_eq!(
        disagreements, expected,
        "the list of linker disagreements changed"
    );
}

/// 报错文字和 lld 不同的用例，以及原因。其余用例 tinylink 的 stderr 必须和 lld 逐字节相同
/// （`ld.lld:` 换成 `tinylink:`，去掉找不到入口的警告）。
const STDERR_DIFFERS: &[(&str, &str)] = &[
    (
        "absolute-zero",
        "lld 对值为 0 的绝对符号报重复定义，tinylink 跟 GNU ld 不报",
    ),
    ("all-at-once", "lld 遇到重复定义就停下，不报后面的未定义"),
    (
        "weak-and-strong-reference",
        "lld 列引用的顺序不按输入顺序，还附了 did you mean 提示",
    ),
];

#[test]
fn error_messages_match_lld() {
    let mut problems = Vec::new();
    let mut compared = 0;
    for case in cases() {
        let path = root()
            .join("linkers")
            .join(format!("{}.lld-stderr", case.name));
        let Ok(expected) = std::fs::read_to_string(&path) else {
            continue;
        };
        if STDERR_DIFFERS.iter().any(|&(name, _)| name == case.name) {
            continue;
        }
        let output = Command::new(env!("CARGO_BIN_EXE_tinylink"))
            .arg("resolve")
            .args(&case.inputs)
            .current_dir(&case.dir)
            .output()
            .expect("run tinylink");
        let actual = String::from_utf8(output.stderr).expect("utf-8 stderr");
        compared += 1;
        if actual != expected {
            problems.push(format!(
                "{}: lld says\n{expected}tinylink says\n{actual}",
                case.name
            ));
        }
    }
    assert!(compared >= 14, "only {compared} cases compared");
    assert!(problems.is_empty(), "\n{}", problems.join("\n"));
}
